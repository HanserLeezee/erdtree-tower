use std::{collections::BTreeSet,sync::Mutex,time::{Duration,Instant}};
use eldenring::cs::{Bullet,ChrIns,EquipParamGoods,FieldInsHandle,SoloParamRepository,SpEffectParam};
use fromsoftware_shared::FromStatic;
use eldenring::param::SP_EFFECT_PARAM_ST;

// Existing inert legacy chain marker; observed before native effect application.
pub const MARK:i32=9670007;
const POT_ANIMS:[u8;4]=[3,20,42,67];
const WINDOW:Duration=Duration::from_millis(erdtree_tower::embers::CAPTURE_WINDOW_MS);
#[derive(Clone,Copy)]
pub struct Hit {pub target:FieldInsHandle,pub hp:i32,pub max:i32}
#[derive(Default)]
struct Attempts {pending:Option<Instant>,last:Option<(i32,Instant)>}
impl Attempts {
    fn thrown(&mut self,bullet:i32,now:Instant) {
        if self.last.is_some_and(|(id,at)|id==bullet&&now.duration_since(at)<Duration::from_millis(100)){return;}
        self.pending=Some(now);
        self.last=Some((bullet,now));
    }
    fn hit(&mut self,own:bool,marker:bool,now:Instant)->bool {
        if !own||!marker{return false;}
        self.pending.take().is_some_and(|at|now.duration_since(at)<=WINDOW)
    }
}
#[derive(Default)]
struct State {
    owner:usize,handle:u64,key:Option<(u64,u8,usize)>,
    roots:BTreeSet<i32>,rows:Vec<(u32,usize,i32)>,marker:Option<[bool;7]>,
    targets:Vec<Hit>,attempts:Attempts,hits:Vec<Hit>,
}
static STATE:Mutex<State>=Mutex::new(State{owner:0,handle:0,key:None,roots:BTreeSet::new(),rows:Vec::new(),marker:None,targets:Vec::new(),attempts:Attempts{pending:None,last:None},hits:Vec::new()});

fn effects(row:&eldenring::param::BULLET_PARAM_ST)->[i32;5] {
    [row.sp_effect_id0(),row.sp_effect_id1(),row.sp_effect_id2(),row.sp_effect_id3(),row.sp_effect_id4()]
}
fn set_effect(row:&mut eldenring::param::BULLET_PARAM_ST,slot:usize,value:i32) {
    match slot {0=>row.set_sp_effect_id0(value),1=>row.set_sp_effect_id1(value),2=>row.set_sp_effect_id2(value),3=>row.set_sp_effect_id3(value),4=>row.set_sp_effect_id4(value),_=>unreachable!()}
}
fn free_effect(values:[i32;5])->Option<usize> {values.iter().position(|v|*v<=0)}
fn marker_targets(row:&SP_EFFECT_PARAM_ST)->[bool;7] {
    [row.effect_target_attacker(),row.effect_target_self_target(),row.effect_target_oppose_target(),
        row.effect_target_friendly_target(),row.effect_target_self(),row.effect_target_friend(),row.effect_target_enemy()]
}
fn set_marker_targets(row:&mut SP_EFFECT_PARAM_ST,flags:[bool;7]) {
    row.set_effect_target_attacker(flags[0]);row.set_effect_target_self_target(flags[1]);row.set_effect_target_oppose_target(flags[2]);
    row.set_effect_target_friendly_target(flags[3]);row.set_effect_target_self(flags[4]);row.set_effect_target_friend(flags[5]);row.set_effect_target_enemy(flags[6]);
}
fn route_marker_to_enemy(row:&mut SP_EFFECT_PARAM_ST)->[bool;7] {
    let saved=marker_targets(row);
    set_marker_targets(row,[false,false,true,false,false,false,true]);
    saved
}
pub fn sync(on:bool) {
    let Ok(mut state)=STATE.lock() else{return};
    if on==state.marker.is_some(){return;}
    let Ok(params)=(unsafe {SoloParamRepository::instance_mut()}) else{return};
    if !on {
        for (id,slot,old) in state.rows.drain(..) {if let Some(row)=params.get_mut::<Bullet>(id) {if effects(row)[slot]==MARK{set_effect(row,slot,old);}}}
        if let Some(saved)=state.marker {if let Some(row)=params.get_mut::<SpEffectParam>(MARK as u32){set_marker_targets(row,saved);}}
        *state=State::default();return;
    }
    let Some(marker)=params.get_mut::<SpEffectParam>(MARK as u32) else{super::log_repeating("TOWER_CAPTURE missing inert marker; update regulation.bin");return;};
    state.marker=Some(route_marker_to_enemy(marker));
    let roots:BTreeSet<i32>=params.rows::<EquipParamGoods>().filter(|(_,row)|row.goods_type()==0&&row.ref_category()==1&&POT_ANIMS.contains(&row.goods_use_anim()))
        .flat_map(|(_,row)|[row.ref_id_default(),row.ref_id_1()]).filter(|id|*id>=0).collect();
    let mut pending:Vec<_>=roots.iter().copied().collect();let mut seen=BTreeSet::new();
    while let Some(id)=pending.pop() {
        if id<0||!seen.insert(id){continue;}
        let Some(row)=params.get_mut::<Bullet>(id as u32) else{continue};
        pending.extend([row.hit_bullet_id(),row.interval_create_bullet_id()]);
        let values=effects(row);
        if let Some(slot)=free_effect(values) {
            state.rows.push((id as u32,slot,values[slot]));set_effect(row,slot,MARK);
        } else {super::log(format!("TOWER_CAPTURE no free hit effect slot bullet={id}"));}
    }
    state.roots=roots;
    super::log(format!("TOWER_CAPTURE native pot tracking roots={} bullets={} marker_target=enemy",state.roots.len(),state.rows.len()));
}
pub fn begin(owner:&ChrIns,key:(u64,u8,usize),targets:Vec<Hit>)->Vec<Hit> {
    let Ok(mut state)=STATE.lock() else{return Vec::new()};
    let address=std::ptr::from_ref(owner) as usize;
    if state.owner!=address||state.key!=Some(key) {state.attempts=Attempts::default();state.hits.clear();}
    state.owner=address;state.handle=super::glintblade::handle_bits(&owner.field_ins_handle);state.key=Some(key);state.targets=targets;
    std::mem::take(&mut state.hits)
}
pub fn deactivate() {
    if let Ok(mut state)=STATE.lock(){state.owner=0;state.handle=0;state.key=None;state.targets.clear();state.attempts=Attempts::default();state.hits.clear();}
}
pub fn observe_spawn(data:&super::glintblade::SpawnData) {
    let Ok(mut state)=STATE.lock() else{return};
    if state.owner==0||u64::from_le_bytes(data.0[..8].try_into().unwrap())!=state.handle{return;}
    let bullet=i32::from_le_bytes(data.0[0x14..0x18].try_into().unwrap());
    if !state.roots.contains(&bullet){return;}
    state.attempts.thrown(bullet,Instant::now());
    super::log(format!("TOWER_CAPTURE projectile emitted bullet={bullet}"));
}
pub fn observe_hit(target:usize,effect:i32,source:usize) {
    if effect!=MARK{return;}
    let Ok(mut state)=STATE.lock() else{return};
    if state.owner==0||source!=state.owner||target==0{return;}
    // The native ChrIns effect function supplies live recipient and source pointers.
    let chr=unsafe {&*(target as *const ChrIns)};
    let Some(hit)=state.targets.iter().find(|hit|hit.target==chr.field_ins_handle).copied() else{return};
    if state.attempts.hit(true,true,Instant::now()) {
        if state.hits.len()<16 {state.hits.push(hit);}
        super::log(format!("TOWER_CAPTURE pot hit target={} hp={}/{}",hit.target,hit.hp,hit.max));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_marker_routes_to_the_hit_enemy_and_restores_without_changing_damage() {
        let mut row:SP_EFFECT_PARAM_ST=unsafe {std::mem::zeroed()};
        let original=[true,true,false,false,true,true,false];
        set_marker_targets(&mut row,original);
        row.set_change_hp_rate(0.0);row.set_change_hp_point(0);row.set_state_info(0);row.set_vfx_id(-1);
        let saved=route_marker_to_enemy(&mut row);
        assert_eq!(saved,original);
        assert!(!row.effect_target_attacker(),"native damage code redirects attacker effects before the hit observer");
        assert_eq!(marker_targets(&row),[false,false,true,false,false,false,true]);
        assert_eq!((row.change_hp_rate(),row.change_hp_point(),row.state_info(),row.vfx_id()),(0.0,0,0,-1));
        set_marker_targets(&mut row,saved);assert_eq!(marker_targets(&row),original);
    }
    #[test]
    fn unlimited_same_pot_creates_a_new_attempt_each_throw() {
        let now=Instant::now();let mut a=Attempts::default();
        for i in 0..3 {let at=now+Duration::from_secs(i);a.thrown(100,at);assert!(a.hit(true,true,at));assert!(!a.hit(true,true,at));}
    }
    #[test]
    fn summons_melee_and_teammates_do_not_spend_the_pot() {
        let now=Instant::now();let mut a=Attempts::default();a.thrown(100,now);
        assert!(!a.hit(false,true,now));assert!(!a.hit(true,false,now));assert!(a.hit(true,true,now));
    }
    #[test]
    fn duplicate_emissions_and_expired_pots_do_not_grant_extra_attempts() {
        let now=Instant::now();let mut a=Attempts::default();a.thrown(100,now);a.thrown(100,now+Duration::from_millis(10));
        assert!(a.hit(true,true,now+Duration::from_millis(20)));assert!(!a.hit(true,true,now+Duration::from_millis(30)));
        a.thrown(100,now+Duration::from_secs(1));assert!(!a.hit(true,true,now+Duration::from_secs(6)));
    }
    #[test]
    fn marker_preserves_poison_frost_and_other_existing_effects() {
        assert_eq!(free_effect([473,0,0,0,0]),Some(1));
        assert_eq!(free_effect([482,483,-1,0,0]),Some(2));
        assert_eq!(free_effect([1,2,3,4,5]),None);
    }
}
