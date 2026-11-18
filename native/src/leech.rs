use std::sync::{Mutex,atomic::{AtomicU64,Ordering}};
use eldenring::cs::{ChrIns,FieldInsHandle,WorldChrMan};
use erdtree_tower::leech::Hits;
use super::damage::Hit;

#[derive(Default)]
struct State {
    owner:u64,player:Option<(FieldInsHandle,usize)>,foes:Vec<(FieldInsHandle,u32)>,hits:Option<Hits>,
}
impl State {
    fn observe(&mut self,hit:&Hit) {
        if hit.hp_before<=0||hit.damage<=0||self.player!=Some((hit.target,hit.target_address)) {return;}
        let Some(attacker)=hit.attacker else{return};
        let Some((_,entity))=self.foes.iter().find(|(h,_)|*h==attacker) else{return};
        let Some(hits)=self.hits.as_mut() else{return};
        if let Some((_,count))=hits.counts.iter_mut().find(|(id,_)|id==entity) {*count=count.saturating_add(1);}
    }
}
static STATE:Mutex<State>=Mutex::new(State{owner:0,player:None,foes:Vec::new(),hits:None});
static NEXT:AtomicU64=AtomicU64::new(1);
fn observe(hit:&Hit) {if let Ok(mut state)=STATE.lock() {state.observe(hit);}}
pub fn report()->Option<Hits> {STATE.lock().ok()?.hits.clone()}
pub fn entity(chr:&ChrIns)->u32 {if chr.event_entity_id!=0 {chr.event_entity_id}else{chr.field_ins_handle.selector.0|0x80000000}}

pub struct Tracker {owner:u64}
impl Default for Tracker {fn default()->Self {Self{owner:NEXT.fetch_add(1,Ordering::Relaxed)}}}
impl Tracker {
    pub fn sync(&self,world:&WorldChrMan,key:(u64,u8,usize),handles:&[FieldInsHandle]) {
        let player=world.main_player.as_ref().map(|p|(&p.chr_ins.field_ins_handle,&p.chr_ins))
            .map(|(h,c)|(*h,c as *const ChrIns as usize));
        let foes:Vec<_>=handles.iter().filter_map(|h|world.chr_ins_by_handle(h))
            .filter(|c|c.modules.data.hp>0&&!c.chr_flags1c5.death_flag())
            .map(|c|(c.field_ins_handle,entity(c))).collect();
        if let Ok(mut state)=STATE.lock() {
            if state.owner!=self.owner { *state=State{owner:self.owner,hits:Some(Hits{key,counts:vec![]}),..Default::default()}; }
            if let Some(hits)=state.hits.as_mut() {for &(_,entity) in &foes {
                if !hits.counts.iter().any(|(id,_)|*id==entity) {hits.counts.push((entity,0));}
            }}
            state.player=player;state.foes=foes;
            super::damage::set_observer((player.is_some()&&!state.foes.is_empty()).then_some(observe as super::damage::Observer));
        }
    }
}
impl Drop for Tracker {
    fn drop(&mut self) {if let Ok(mut state)=STATE.lock()&&state.owner==self.owner {*state=State::default();super::damage::set_observer(None);}}
}

#[cfg(test)]
mod tests {
    use super::*;
    use eldenring::cs::{FieldInsSelector,BlockId};
    fn handle(n:u32)->FieldInsHandle {FieldInsHandle{selector:FieldInsSelector(0x17200000+n),block_id:BlockId::from(-1)}}
    #[test]
    fn only_real_hits_on_local_player_by_registered_boss_count() {
        let mut state=State{owner:1,player:Some((handle(1),100)),foes:vec![(handle(2),200),(handle(3),300)],hits:Some(Hits{key:(1,1,1),counts:vec![(200,0),(300,0)]})};
        let hit=Hit{attacker:Some(handle(2)),target:handle(1),target_address:100,hp_before:500,damage:20,power:[20.0,0.0,0.0,0.0,0.0],percent:[0.0;5]};
        state.observe(&hit);
        for rejected in [Hit{attacker:None,..hit},Hit{attacker:Some(handle(9)),..hit},Hit{target:handle(8),..hit},Hit{target_address:999,..hit},Hit{damage:0,..hit},Hit{hp_before:0,..hit}] {state.observe(&rejected);}
        assert_eq!(state.hits.as_ref().unwrap().counts,vec![(200,1),(300,0)]);
        state.observe(&Hit{attacker:Some(handle(3)),..hit});
        assert_eq!(state.hits.as_ref().unwrap().counts,vec![(200,1),(300,1)]);
    }
}
