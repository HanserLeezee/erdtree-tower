use std::{sync::{LazyLock,Mutex},time::{Duration,Instant}};
use eldenring::cs::{PlayerIns,SummonBuddyManager};

#[derive(Clone,PartialEq,Eq)]
struct Key {manager:usize,owner:i32,members:Vec<u64>}
#[derive(Default)]
struct Watch {pending:Option<(Key,Duration,Duration,bool)>}
impl Watch {
    fn observe(&mut self,now:Duration,key:Option<Key>,delay:Duration)->bool {
        let Some(key)=key else{self.pending=None;return false;};
        if self.pending.as_ref().is_none_or(|(old,_,_,_)|*old!=key){self.pending=Some((key,now,delay,false));return false;}
        let (_,since,longest,fired)=self.pending.as_mut().unwrap();
        *longest=(*longest).max(delay);
        if *fired||now.saturating_sub(*since)<Duration::from_secs(5)+*longest{return false;}
        *fired=true;true
    }
}
struct Member {key:u64,custom:bool,remote:bool,disappearing:bool,delay:f32}
fn eligible(pending:i32,delay:f32,members:&[Member])->bool {
    pending<0&&delay.is_finite()&&delay<=60.0&&!members.is_empty()&&members.iter().any(|m|m.custom)
        &&members.iter().all(|m|!m.remote&&m.disappearing&&m.delay.is_finite()&&m.delay<=60.0)
}
static WATCH:Mutex<Watch>=Mutex::new(Watch{pending:None});
static START:LazyLock<Instant>=LazyLock::new(Instant::now);

// Called after the native summon-manager phase; use its existing owner-clear path.
pub(super) unsafe fn tick(manager:*mut u8,clear:usize) {
    if manager.is_null()||clear==0{return;}
    let candidate=(||{
        let player=unsafe {PlayerIns::local_player()}.ok()?;
        let owner=unsafe {player.player_game_data.as_ref()}.character_event_id as i32;
        let buddy=unsafe {&*manager.cast::<SummonBuddyManager>()};
        if buddy.request_summon_speffect_id>=0{return None;}
        let members:Vec<_>=buddy.groups.iter().filter(|pair|pair.first==owner).flat_map(|pair|pair.second.iter()).map(|group|{
            let chr=unsafe {group.chr_ins.as_ref()};let handle=chr.field_ins_handle;
            let custom=group.buddy_param_id.checked_sub(1600000000).is_some_and(|offset|offset>=0
                &&offset/40000<(erdtree_tower::rewards::CUSTOM_SPIRITS.len() as i32)&&offset%1000<3);
            Member {key:(u64::from(handle.block_id.0 as u32)<<32)|u64::from(handle.selector.0),custom,remote:group.is_remote,
                disappearing:group.disappear_requested,delay:group.disappear_delay_sec}
        }).collect();
        if !eligible(buddy.request_summon_speffect_id,buddy.buddy_disappear_delay_sec,&members){return None;}
        let mut ids:Vec<_>=members.iter().map(|m|m.key).collect();ids.sort_unstable();
        let delay=members.iter().map(|m|m.delay).fold(buddy.buddy_disappear_delay_sec,f32::max).max(0.0);
        Some((Key{manager:manager as usize,owner,members:ids},Duration::from_secs_f32(delay)))
    })();
    let (key,delay)=candidate.map_or((None,Duration::ZERO),|(key,delay)|(Some(key),delay));
    let fire=WATCH.try_lock().is_ok_and(|mut watch|watch.observe(START.elapsed(),key.clone(),delay));
    if fire&&let Some(key)=key {
        super::reset_twin();
        crate::log(format!("ASH_DISMISS fallback owner={} members={} after native delay plus 5s; invoking native owner clear",key.owner,key.members.len()));
        let original:unsafe extern "C" fn(*mut u8,i32)=unsafe {std::mem::transmute(clear)};
        unsafe {original(manager,key.owner);}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn member()->Member {Member{key:1,custom:true,remote:false,disappearing:true,delay:0.0}}
    fn key(id:u64)->Key {Key{manager:10,owner:20,members:vec![id]}}
    #[test]
    fn only_fully_dismissing_local_groups_are_eligible() {
        assert!(eligible(-1,0.0,&[member()]));
        assert!(!eligible(610000000,0.0,&[member()]));
        assert!(eligible(-1,1.0,&[member()]));
        assert!(!eligible(-1,0.0,&[]));
        assert!(!eligible(-1,0.0,&[Member{custom:false,..member()}]));
        for other in [Member{remote:true,..member()},Member{disappearing:false,..member()},Member{delay:f32::NAN,..member()}] {
            assert!(!eligible(-1,0.0,&[member(),other]));
        }
    }
    #[test]
    fn grace_is_one_shot_and_new_members_restart_it() {
        let mut watch=Watch::default();
        assert!(!watch.observe(Duration::ZERO,Some(key(1)),Duration::ZERO));
        assert!(!watch.observe(Duration::from_millis(4999),Some(key(1)),Duration::ZERO));
        assert!(watch.observe(Duration::from_secs(5),Some(key(1)),Duration::ZERO));
        assert!(!watch.observe(Duration::from_secs(20),Some(key(1)),Duration::ZERO));
        assert!(!watch.observe(Duration::from_secs(21),Some(key(2)),Duration::ZERO));
        assert!(!watch.observe(Duration::from_secs(25),Some(key(2)),Duration::ZERO));
        assert!(watch.observe(Duration::from_secs(26),Some(key(2)),Duration::ZERO));
    }
    #[test]
    fn cancelled_dismissal_or_owner_change_does_not_reuse_timer() {
        let mut watch=Watch::default();
        watch.observe(Duration::ZERO,Some(key(1)),Duration::ZERO);
        assert!(!watch.observe(Duration::from_secs(4),None,Duration::ZERO));
        assert!(!watch.observe(Duration::from_secs(5),Some(key(1)),Duration::ZERO));
        let other=Key{owner:21,..key(1)};
        assert!(!watch.observe(Duration::from_secs(10),Some(other.clone()),Duration::ZERO));
        assert!(watch.observe(Duration::from_secs(15),Some(other),Duration::ZERO));
    }
    #[test]
    fn native_delay_is_respected_before_fallback() {
        let mut watch=Watch::default();
        watch.observe(Duration::ZERO,Some(key(1)),Duration::from_secs(3));
        assert!(!watch.observe(Duration::from_secs(5),Some(key(1)),Duration::ZERO));
        assert!(!watch.observe(Duration::from_secs(7),Some(key(1)),Duration::ZERO));
        assert!(watch.observe(Duration::from_secs(8),Some(key(1)),Duration::ZERO));
    }
}
