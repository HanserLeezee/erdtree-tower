use eldenring::cs::{CSSessionManager,ChrInsExt,FieldInsHandle,LobbyState,NpcParam,SoloParamRepository,SpEffectParam,WorldChrMan};
use fromsoftware_shared::FromStatic;
pub const GROWTH_FIRST:i32=9640000;
const TIER_FIRST:i32=9641000;
/// Seamless Co-op 2.0.x 运行时写入 SoloParamRepository 的多人缩放 SpEffect 段（复制自原版 7800，spCategory=140, stateInfo=282）：
/// - 2147200000+ / 2147201000+：普通敌人（mpc 100/200/10000100/10000200）
/// - 2147202000+：地牢 Boss（mpc 300/10000300/203000..=498000，读取 boss_health/damage/posture_scaling）
/// - 2147203000+：野外 Boss（mpc 400/10000400，读取 boss_health/damage/posture_scaling）
const SEAMLESS_SCALE_FIRST:i32=2_147_200_000;
const SEAMLESS_SCALE_END:i32=2_147_204_000;
const SEAMLESS_BOSS_DUNGEON:i32=2_147_202_000;
const SEAMLESS_BOSS_FIELD:i32=2_147_203_000;
const MULTIPLAY_STATE_INFO:u16=282;
fn area_effect(id:i32)->bool {(7000..=7280).contains(&id)||(20007000..=20007350).contains(&id)||(TIER_FIRST..TIER_FIRST+300).contains(&id)}
fn has_coop_scaling(ids:impl IntoIterator<Item=i32>)->bool {
    ids.into_iter().any(|id|(SEAMLESS_SCALE_FIRST..SEAMLESS_SCALE_END).contains(&id)||(7800..=7882).contains(&id))
}
/// 根据 NpcParam.multiPlayCorrectionParamId 与额外联机人数（1..=7）返回对应的无缝 Boss 缩放 SpEffect 编号。
/// 仅覆盖 Boss 档位（300/400/10000300/10000400 及原版命名 Boss 行）；普通怪（100/200）由引擎原生触发。
fn seamless_boss_effect(mpc_id:i32,extra_players:u8)->Option<i32> {
    if extra_players==0 {return None;}
    let offset=i32::from(extra_players-1);
    match mpc_id {
        400|10_000_400=>Some(SEAMLESS_BOSS_FIELD+offset),
        300|10_000_300|203_000..=498_000=>Some(SEAMLESS_BOSS_DUNGEON+offset),
        _=>None,
    }
}
fn coop_extra_players(party_players:usize)->u8 {
    if party_players<2||!super::seamless::active() {return 0;}
    let live=unsafe {CSSessionManager::instance()}.ok()
        .filter(|s|matches!(s.lobby_state,LobbyState::Host|LobbyState::Client))
        .map(|s|s.players.len())
        .filter(|&n|n>=2);
    live.unwrap_or(party_players).saturating_sub(1).min(7) as u8
}
fn merchant_effects(run:&erdtree_tower::Run,node:usize)->Result<Vec<i32>,String> {
    if !run.nodes.get(node).is_some_and(|n|n.kind==erdtree_tower::NodeKind::Shop){return Err("铃珠猎人不在商店节点".into());}
    let tier=include_str!("../../scene-assets/tower-scaling.txt").lines().filter(|s|!s.starts_with('#'))
        .filter_map(|s|{let p=s.split('|').map(str::parse::<i32>).collect::<Result<Vec<_>,_>>().ok()?;(p.len()==4&&p[0]==i32::from(run.act)).then(||p[3])})
        .next().ok_or("当前幕缺少 Boss 强度档位")?;
    let mut effects=vec![TIER_FIRST+tier*10];
    let step=erdtree_tower::scaling::growth_step(run,node);
    if step>0 {effects.push(GROWTH_FIRST+i32::from(step));}
    Ok(effects)
}
pub fn merchant(chr:&mut eldenring::cs::ChrIns,run:&erdtree_tower::Run,node:usize)->Result<(),String> {
    let effects=merchant_effects(run,node)?;
    let params=unsafe{SoloParamRepository::instance()}.map_err(|_|"强度参数尚未加载")?;
    if effects.iter().any(|id|params.get::<SpEffectParam>(*id as u32).is_none()){return Err("铃珠猎人强度参数缺失，请更新资源".into());}
    let old:Vec<_>=chr.special_effect.entries().map(|e|e.param_id).filter(|id|area_effect(*id)||(GROWTH_FIRST+1..=GROWTH_FIRST+11).contains(id)).collect();
    for id in &old {if !effects.contains(id){chr.remove_speffect(*id);}}
    for id in &effects {if !old.contains(id){chr.apply_speffect(*id,false);}}
    super::log(format!("MERCHANT_SCALING act={} node={node} effects={effects:?}",run.act));
    Ok(())
}
fn ready(step:u8)->bool {unsafe {SoloParamRepository::instance()}.is_ok_and(|p|p.get::<SpEffectParam>((GROWTH_FIRST+i32::from(step)) as u32).is_some())}
/// 新旅程按本幕进度给每个刷出的敌人施加一次递增效果（生命与攻击），
/// 并在无缝联机下兜底核验 Boss 多人缩放（与 MultiPlayCorrectionParam.bOverrideSpEffect=1 双保险）：
/// 联机时房主与成员都在本机给同步过来的敌人挂上同一个效果：这个效果不随网络同步，只有房主挂时，
/// 成员那边同一只敌人的最大生命更低，成员一刀把本机副本打到 0、房主那边还活着，副本卡在原地不倒、成员也拿不到卢恩。
/// 已经挂着的不重复挂
#[derive(Default)]
pub struct Growth {key:Option<(u64,u8,usize)>,applied:Vec<FieldInsHandle>,coop_checked:Vec<FieldInsHandle>,missing:bool,missing_coop:bool}
impl Growth {
    pub fn tick(&mut self,world:&mut WorldChrMan,key:(u64,u8,usize),step:u8,handles:&[FieldInsHandle],party_players:usize) {
        if self.key!=Some(key){*self=Self{key:Some(key),..Default::default()};}
        self.tick_coop_boss(world,handles,party_players);
        if step==0{return;}
        if !ready(step){if !self.missing{super::log(format!("TOWER_GROWTH missing effect {}",GROWTH_FIRST+i32::from(step)));self.missing=true;}return;}
        for handle in handles {
            if self.applied.contains(handle){continue;}
            let Some(chr)=world.chr_ins_by_handle_mut(handle) else{continue};
            if chr.modules.data.hp<=0||chr.chr_flags1c5.death_flag(){continue;}
            let id=GROWTH_FIRST+i32::from(step);
            if !chr.special_effect.entries().any(|e|e.param_id==id) {chr.apply_speffect(id,false);}
            self.applied.push(*handle);
        }
    }
    /// 无缝联机 Boss 多人缩放兜底：若参数表层的 bOverrideSpEffect=1 已让 ersc.dll 生效，则直接跳过；
    /// 否则在房主与客机两侧补挂 ersc.dll 注册的对应人数 Boss 缩放效果（spCategory=140 互斥，绝不双倍叠加）。
    fn tick_coop_boss(&mut self,world:&mut WorldChrMan,handles:&[FieldInsHandle],party_players:usize) {
        let extra=coop_extra_players(party_players);
        if extra==0 {return;}
        let Ok(params)=(unsafe {SoloParamRepository::instance()}) else{return;};
        for handle in handles {
            if self.coop_checked.contains(handle){continue;}
            let Some(chr)=world.chr_ins_by_handle_mut(handle) else{continue};
            if chr.modules.data.hp<=0||chr.chr_flags1c5.death_flag(){continue;}
            let npc_id=chr.npc_param_id;
            let Some(mpc)=params.get::<NpcParam>(npc_id as u32).map(|r|r.multi_play_correction_param_id()) else{self.coop_checked.push(*handle);continue;};
            let Some(effect_id)=seamless_boss_effect(mpc,extra) else{self.coop_checked.push(*handle);continue;};
            if has_coop_scaling(chr.special_effect.entries().map(|e|e.param_id)) {
                super::log(format!("COOP_BOSS_SCALING native ok entity={} npc={npc_id} mpc={mpc} players={} max_hp={}",chr.event_entity_id,usize::from(extra)+1,chr.modules.data.max_hp));
                self.coop_checked.push(*handle);
                continue;
            }
            if !params.get::<SpEffectParam>(effect_id as u32).is_some_and(|e|e.state_info()==MULTIPLAY_STATE_INFO) {
                if !self.missing_coop {
                    super::log(format!("COOP_BOSS_SCALING seamless effect {effect_id} unavailable (players={})",usize::from(extra)+1));
                    self.missing_coop=true;
                }
                self.coop_checked.push(*handle);
                continue;
            }
            let before_max=chr.modules.data.max_hp;
            chr.apply_speffect(effect_id,false);
            let after_max=chr.modules.data.max_hp;
            super::log(format!("COOP_BOSS_SCALING fallback applied entity={} npc={npc_id} mpc={mpc} effect={effect_id} players={} max_hp={before_max}->{after_max}",chr.event_entity_id,usize::from(extra)+1));
            self.coop_checked.push(*handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn merchant_uses_current_act_boss_tier_and_shop_layer() {
        for (act,tier) in [(1,3),(2,10),(3,16)] {
            let mut run=erdtree_tower::Run::new(42,erdtree_tower::Class::Samurai);run.act=act;
            let shops:Vec<_>=run.nodes.iter().enumerate().filter(|(_,n)|n.kind==erdtree_tower::NodeKind::Shop).map(|(id,_)|id).collect();
            assert!(!shops.is_empty());
            for id in shops {
                let effects=merchant_effects(&run,id).unwrap();
                assert_eq!(effects[0],TIER_FIRST+tier*10);
                let step=erdtree_tower::scaling::growth_step(&run,id);
                assert_eq!(effects.get(1).copied(),(step>0).then_some(GROWTH_FIRST+i32::from(step)));
                let mut old=run.clone();old.scaling_rules=0;
                assert_eq!(merchant_effects(&old,id).unwrap(),vec![TIER_FIRST+tier*10]);
            }
            assert!(merchant_effects(&run,usize::MAX).is_err());
        }
        assert!(area_effect(9641030)&&area_effect(9641160));
        assert!(!area_effect(9670070)&&!area_effect(GROWTH_FIRST+1));
    }
    #[test]
    fn seamless_boss_fallback_targets_only_boss_correction_ids_and_detects_existing_scaling() {
        assert_eq!(seamless_boss_effect(300,0),None);
        assert_eq!(seamless_boss_effect(100,1),None);
        assert_eq!(seamless_boss_effect(200,2),None);
        assert_eq!(seamless_boss_effect(10_000_100,3),None);
        assert_eq!(seamless_boss_effect(10_000_200,3),None);
        assert_eq!(seamless_boss_effect(-1,2),None);
        assert_eq!(seamless_boss_effect(300,1),Some(2_147_202_000));
        assert_eq!(seamless_boss_effect(300,2),Some(2_147_202_001));
        assert_eq!(seamless_boss_effect(10_000_300,3),Some(2_147_202_002));
        assert_eq!(seamless_boss_effect(473_000,1),Some(2_147_202_000));
        assert_eq!(seamless_boss_effect(400,1),Some(2_147_203_000));
        assert_eq!(seamless_boss_effect(10_000_400,4),Some(2_147_203_003));
        assert!(!has_coop_scaling([9641030,9640011,9660003,4410]));
        assert!(has_coop_scaling([9641030,2_147_200_000]));
        assert!(has_coop_scaling([2_147_202_001]));
        assert!(has_coop_scaling([2_147_203_000]));
        assert!(has_coop_scaling([7820]));
        assert!(has_coop_scaling([7871]));
    }
}
