use std::sync::{Mutex,atomic::{AtomicBool,AtomicU64,Ordering}};
use eldenring::cs::{ChrIns,FieldInsHandle,WorldChrMan};
use erdtree_tower::boss_affixes::adaptation::Adaptation;
use super::damage::Hit;

#[derive(Clone,Copy,PartialEq,Eq)]
struct Identity {handle:FieldInsHandle,address:usize}
struct Target {identity:Identity,state:Adaptation}
#[derive(Default)]
struct State {owner:u64,targets:Vec<Target>}
impl State {
    fn sync(&mut self,owner:u64,targets:&[Identity]) {
        if self.owner!=owner {self.targets.clear();self.owner=owner;}
        self.targets.retain(|t|targets.contains(&t.identity));
        for &identity in targets {
            if !self.targets.iter().any(|t|t.identity==identity) {self.targets.push(Target{identity,state:Adaptation::default()});}
        }
    }
    fn clear(&mut self,owner:u64) {if self.owner==owner {self.targets.clear();self.owner=0;}}
    fn hit(&mut self,hit:&Hit)->Option<i32> {
        if hit.hp_before<=0||hit.damage<=0 {return None;}
        let kind=hit.pure_type()?;
        let target=self.targets.iter_mut().find(|t|t.identity==Identity{handle:hit.target,address:hit.target_address})?;
        let mut damage=[0;5];damage[kind]=hit.damage as u32;
        Some(target.state.receive(damage,true)[kind] as i32)
    }
}
static STATE:Mutex<State>=Mutex::new(State{owner:0,targets:Vec::new()});
static NEXT:AtomicU64=AtomicU64::new(1);
static READY:AtomicBool=AtomicBool::new(false);

fn filter(hit:&Hit)->Option<i32> {STATE.lock().ok()?.hit(hit)}
pub fn install() {READY.store(true,Ordering::Release);super::log("ADAPTATION pre-HP filter ready");}
pub fn ready()->bool {READY.load(Ordering::Acquire)}

pub struct Tracker {owner:u64}
impl Default for Tracker {fn default()->Self {Self{owner:NEXT.fetch_add(1,Ordering::Relaxed)}}}
impl Tracker {
    pub fn sync(&self,world:&WorldChrMan,handles:&[FieldInsHandle]) {
        if !ready() {return;}
        // Collect engine data before taking the callback lock; never call engine functions under it.
        let targets:Vec<_>=handles.iter().filter_map(|h|world.chr_ins_by_handle(h))
            .filter(|c|c.modules.data.hp>0&&!c.chr_flags1c5.death_flag()&&!c.debug_flags.disabled_hit())
            .map(|c|Identity{handle:c.field_ins_handle,address:c as *const ChrIns as usize}).collect();
        if let Ok(mut state)=STATE.lock() {
            state.sync(self.owner,&targets);
            super::damage::set_filter((!targets.is_empty()).then_some(filter as super::damage::Filter));
        }
    }
}
impl Drop for Tracker {
    fn drop(&mut self) {
        if let Ok(mut state)=STATE.lock()&&state.owner==self.owner {state.clear(self.owner);super::damage::set_filter(None);}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eldenring::cs::{FieldInsSelector,BlockId};
    fn handle()->FieldInsHandle {FieldInsHandle{selector:FieldInsSelector(0x17200000),block_id:BlockId::from(-1)}}
    fn hit(address:usize,kind:usize)->Hit {
        let mut power=[0.0;5];power[kind]=100.0;
        Hit{attacker:None,target:handle(),target_address:address,hp_before:1000,power,percent:[0.0;5],damage:100}
    }
    fn identity(address:usize)->Identity {Identity{handle:handle(),address}}
    #[test]
    fn separate_targets_keep_stacks_across_ticks_and_ignore_foreign_hits() {
        let mut state=State::default();state.sync(1,&[identity(10),identity(20)]);
        assert_eq!(state.hit(&hit(30,0)),None);
        for expected in [100,90,80,70,60,50,50] {
            assert_eq!(state.hit(&hit(10,0)),Some(expected));
            state.sync(1,&[identity(10),identity(20)]);
        }
        assert_eq!(state.hit(&hit(20,0)),Some(100));
        assert_eq!(state.hit(&hit(10,2)),Some(100));
        assert_eq!(state.hit(&hit(10,0)),Some(100));
    }
    #[test]
    fn mixed_percent_hits_and_invalid_damage_do_not_advance_stacks() {
        let mut state=State::default();state.sync(1,&[identity(10)]);
        assert_eq!(state.hit(&hit(10,1)),Some(100));
        let mut mixed=hit(10,1);mixed.percent[2]=1.0;
        assert_eq!(state.hit(&mixed),None);
        let mut dead=hit(10,1);dead.hp_before=0;assert_eq!(state.hit(&dead),None);
        let mut zero=hit(10,1);zero.damage=0;assert_eq!(state.hit(&zero),None);
        assert_eq!(state.hit(&hit(10,1)),Some(90));
    }
    #[test]
    fn removed_targets_retries_and_old_owner_cleanup_cannot_leak_stacks() {
        let mut state=State::default();state.sync(1,&[identity(10)]);
        state.hit(&hit(10,0));state.sync(1,&[]);
        assert_eq!(state.hit(&hit(10,0)),None);
        state.sync(1,&[identity(10)]);assert_eq!(state.hit(&hit(10,0)),Some(100));
        state.sync(2,&[identity(10)]);state.clear(1);
        assert_eq!(state.hit(&hit(10,0)),Some(100));
        state.clear(2);assert_eq!(state.hit(&hit(10,0)),None);
    }
}
