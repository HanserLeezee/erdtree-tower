use eldenring::cs::{CSEventFlagMan,PlayerIns};
use fromsoftware_shared::FromStatic;

// Vanilla ItemLotParam_map acquisition flags and common event 1450's derived affinity flags.
const UNLOCKS:&[(u32,&[u32])]=&[
    (8970,&[65610,65620,65630]),(8971,&[65640,65650]),
    (8972,&[65660,65670]),(8973,&[65680,65690]),(8974,&[65720,65700,65710]),
];
/// 质变解锁要和身上的砥石刀一致：有哪把就开哪几种质变，没有的关掉。
/// 解锁标记存在存档里，而每局开始会清空背包（砥石刀也一起清掉），只补不清会让拿过一次砥石刀的角色以后每局都能随意质变
fn plan(owns:impl Fn(u32)->bool,enabled:impl Fn(u32)->bool)->(Vec<u32>,Vec<u32>) {
    let mut grant=Vec::new();let mut revoke=Vec::new();
    for (id,flags) in UNLOCKS {
        let owned=owns(*id);
        for &flag in flags.iter() {
            if owned&&!enabled(flag) {grant.push(flag);}
            if !owned&&enabled(flag) {revoke.push(flag);}
        }
    }
    (grant,revoke)
}
#[cfg(test)]
fn missing_flags(owns:impl Fn(u32)->bool,enabled:impl Fn(u32)->bool)->Vec<u32> {plan(owns,enabled).0}
/// 打开换战灰／质变界面前、每局开始清空背包后调用
pub fn sync_owned()->Result<(),String> {
    let player=unsafe {PlayerIns::local_player()}.map_err(|_|"角色未加载，无法核对砥石刀")?;
    let data=unsafe {player.player_game_data.as_ref()};
    let flags=unsafe {CSEventFlagMan::instance_mut()}.map_err(|_|"质变解锁事件系统未加载")?;
    let (grant,revoke)=plan(|id|crate::loot::count(data,0x40000000|id)>0,|id|flags.virtual_memory_flag.get_flag(id));
    for &id in &grant {flags.virtual_memory_flag.set_flag(id,true);}
    for &id in &revoke {flags.virtual_memory_flag.set_flag(id,false);}
    if !grant.is_empty(){crate::log(format!("Whetblade affinity unlocks repaired from owned goods: {grant:?}"));}
    if !revoke.is_empty(){crate::log(format!("Whetblade affinity unlocks revoked (whetblade not held): {revoke:?}"));}
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn black_whetblade_unlocks_only_its_three_affinities() {
        assert_eq!(missing_flags(|id|id==8974, |_|false),vec![65720,65700,65710]);
    }
    #[test]
    fn no_whetblade_or_only_basic_knife_does_not_unlock_affinities() {
        assert!(missing_flags(|_|false, |_|false).is_empty());
        assert!(missing_flags(|id|id==8590, |_|false).is_empty());
    }
    #[test]
    fn repairs_partial_unlock_without_rewriting_existing_flags() {
        assert_eq!(missing_flags(|id|id==8974, |id|id==65720),vec![65700,65710]);
        assert!(missing_flags(|_|true, |_|true).is_empty());
    }
    #[test]
    fn all_five_whetblades_match_vanilla_event_1450() {
        assert_eq!(missing_flags(|_|true, |_|false),vec![65610,65620,65630,65640,65650,65660,65670,65680,65690,65720,65700,65710]);
    }
    #[test]
    fn affinities_without_the_whetblade_are_revoked() {
        // 上一局拿过漆黑砥石刀，这一局背包清空：它的三种质变要关掉，其他未解锁的不动
        assert_eq!(plan(|_|false,|id|[65720,65700,65710].contains(&id)),(vec![],vec![65720,65700,65710]));
        // 只拿着铁制砥石刀：开它的三种，关掉残留的漆黑砥石刀三种
        assert_eq!(plan(|id|id==8970,|id|(65700..=65720).contains(&id)),(vec![65610,65620,65630],vec![65720,65700,65710]));
        assert_eq!(plan(|_|true,|_|true),(vec![],vec![]));
    }
}
