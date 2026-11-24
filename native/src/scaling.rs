use eldenring::cs::{CSSessionManager,ChrInsExt,FieldInsHandle,LobbyState,NpcParam,SoloParamRepository,SpEffectParam,WorldChrMan};
use fromsoftware_shared::FromStatic;
pub const GROWTH_FIRST:i32=9640000;
/// 无尽模式第 4 幕起动态写入生命/攻击倍率的常驻敌人效果行（TowerScalingAssets 写入 regulation.bin）
pub const ENDLESS_EFFECT:i32=GROWTH_FIRST+20;
const TIER_FIRST:i32=9641000;
const ACT3_WEAK_TIER:i32=TIER_FIRST+12*10;
const ACT3_ELITE_TIER:i32=TIER_FIRST+14*10;
const ACT3_BOSS_TIER:i32=TIER_FIRST+16*10;
/// 无尽模式第 4 幕起将抽到的 1/2 幕敌人基准档位提升到第 3 幕同定位档位，确保所有池子敌人都有后期强度
fn promoted_act3_tier(current_tier:i32,npc_id:i32)->Option<i32> {
    match current_tier {
        9641020=>Some(if super::enemies::boss_npc(npc_id){ACT3_ELITE_TIER}else{ACT3_WEAK_TIER}),
        9641030=>Some(ACT3_BOSS_TIER),
        9641070=>Some(ACT3_WEAK_TIER),
        9641090=>Some(ACT3_ELITE_TIER),
        9641100=>Some(ACT3_BOSS_TIER),
        _=>None,
    }
}
fn sync_endless_effect(act:u8)->bool {
    let Some((hp,atk))=erdtree_tower::scaling::endless_rates(act) else{return false;};
    let Ok(params)=(unsafe {SoloParamRepository::instance_mut()}) else{return false;};
    let Some(row)=params.get_mut::<SpEffectParam>(ENDLESS_EFFECT as u32) else{return false;};
    row.set_max_hp_rate(hp);
    row.set_physics_attack_power_rate(atk);
    row.set_magic_attack_power_rate(atk);
    row.set_fire_attack_power_rate(atk);
    row.set_thunder_attack_power_rate(atk);
    row.set_dark_attack_power_rate(atk);
    true
}
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
    let eff_act=run.act.clamp(1,3);
    let tier=include_str!("../../scene-assets/tower-scaling.txt").lines().filter(|s|!s.starts_with('#'))
        .filter_map(|s|{let p=s.split('|').map(str::parse::<i32>).collect::<Result<Vec<_>,_>>().ok()?;(p.len()==4&&p[0]==i32::from(eff_act)).then(||p[3])})
        .next().ok_or("当前幕缺少 Boss 强度档位")?;
    let mut effects=vec![TIER_FIRST+tier*10];
    let step=erdtree_tower::scaling::growth_step(run,node);
    if step>0 {effects.push(GROWTH_FIRST+i32::from(step));}
    if erdtree_tower::scaling::endless_rates(run.act).is_some() {effects.push(ENDLESS_EFFECT);}
    Ok(effects)
}
pub fn merchant(chr:&mut eldenring::cs::ChrIns,run:&erdtree_tower::Run,node:usize)->Result<(),String> {
    let effects=merchant_effects(run,node)?;
    if erdtree_tower::scaling::endless_rates(run.act).is_some() {sync_endless_effect(run.act);}
    let params=unsafe{SoloParamRepository::instance()}.map_err(|_|"强度参数尚未加载")?;
    if effects.iter().any(|id|params.get::<SpEffectParam>(*id as u32).is_none()){return Err("铃珠猎人强度参数缺失，请更新资源".into());}
    let old:Vec<_>=chr.special_effect.entries().map(|e|e.param_id).filter(|id|area_effect(*id)||(GROWTH_FIRST+1..=GROWTH_FIRST+11).contains(id)||*id==ENDLESS_EFFECT).collect();
    for id in &old {if !effects.contains(id){chr.remove_speffect(*id);}}
    for id in &effects {if !old.contains(id){chr.apply_speffect(*id,false);}}
    super::log(format!("MERCHANT_SCALING act={} node={node} effects={effects:?}",run.act));
    Ok(())
}
fn ready(step:u8)->bool {unsafe {SoloParamRepository::instance()}.is_ok_and(|p|p.get::<SpEffectParam>((GROWTH_FIRST+i32::from(step)) as u32).is_some())}

#[derive(Default)]
struct BossSamples {
    room: Option<(u64,u8,usize)>,
    last: Vec<(FieldInsHandle,i32,usize,std::time::Instant,bool)>,
}

pub fn trace_boss_hp(world:&WorldChrMan, run:&erdtree_tower::Run, node:usize, role:&str, handles:&[FieldInsHandle]) {
    use eldenring::cs::{CSSessionManager,NpcParam};
    if !run.nodes.get(node).is_some_and(|n| n.kind==erdtree_tower::NodeKind::Boss) {return;}
    thread_local! {static SAMPLES:std::cell::RefCell<BossSamples>=Default::default();}
    SAMPLES.with_borrow_mut(|state| {
        let key=(run.seed,run.act,node);
        if state.room!=Some(key) {state.room=Some(key);state.last.clear();}
        let players=unsafe {CSSessionManager::instance()}.map_or(0,|s|s.players.len());
        for handle in handles {
            let Some(chr)=world.chr_ins_by_handle(handle) else{continue};
            let vitals=&chr.modules.data;
            if vitals.max_hp<=0 {continue;}
            if let Some((_,max,count,start,settled))=state.last.iter_mut().find(|s|s.0==*handle) {
                let due=!*settled && start.elapsed().as_secs()>=2;
                if *max==vitals.max_hp && *count==players && !due {continue;}
                *settled|=due;*max=vitals.max_hp;*count=players;
            } else {state.last.push((*handle,vitals.max_hp,players,std::time::Instant::now(),false));}
            let params=unsafe {SoloParamRepository::instance()}.ok();
            let base_hp=params.and_then(|p|p.get::<NpcParam>(chr.npc_param_id as u32)).map(|p|p.hp());
            let effects:Vec<_>=chr.special_effect.entries().filter_map(|e|{
                let rate=params?.get::<SpEffectParam>(e.param_id as u32)?.max_hp_rate();
                (rate!=1.0).then_some((e.param_id,rate))
            }).collect();
            let rules=super::party_compat::profile().and_then(|r|r.as_ref().ok()).map(|p|(p.rules[4],p.rules[7]));
            super::log(format!("BOSS_HP role={role} session_players={players} act={} node={node} growth={} npc={} entity={} base_hp={base_hp:?} hp={}/{} enemy_boss_settings={rules:?} hp_effects={effects:?}",run.act,erdtree_tower::scaling::growth_step(run,node),chr.npc_param_id,chr.event_entity_id,vitals.hp,vitals.max_hp));
        }
    });
}
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
        let endless=erdtree_tower::scaling::endless_rates(key.1).is_some();
        if step==0&&!endless{return;}
        if step>0&&!ready(step){if !self.missing{super::log(format!("TOWER_GROWTH missing effect {}",GROWTH_FIRST+i32::from(step)));self.missing=true;}return;}
        let endless_ready=endless&&sync_endless_effect(key.1);
        for handle in handles {
            if self.applied.contains(handle){continue;}
            let Some(chr)=world.chr_ins_by_handle_mut(handle) else{continue};
            if chr.modules.data.hp<=0||chr.chr_flags1c5.death_flag(){continue;}
            if endless {
                let npc_id=chr.npc_param_id;
                let lower:Vec<_>=chr.special_effect.entries().map(|e|e.param_id).filter_map(|id|promoted_act3_tier(id,npc_id).map(|next|(id,next))).collect();
                for (old_id,next_id) in lower {
                    chr.remove_speffect(old_id);
                    if !chr.special_effect.entries().any(|e|e.param_id==next_id) {chr.apply_speffect(next_id,false);}
                }
                if endless_ready&&!chr.special_effect.entries().any(|e|e.param_id==ENDLESS_EFFECT) {
                    chr.apply_speffect(ENDLESS_EFFECT,false);
                }
            }
            if step>0 {
                let id=GROWTH_FIRST+i32::from(step);
                if !chr.special_effect.entries().any(|e|e.param_id==id) {chr.apply_speffect(id,false);}
            }
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
    #[test]
    fn endless_scaling_promotes_act1_act2_tiers_and_appends_endless_effect() {
        assert_eq!(promoted_act3_tier(9641020,0),Some(9641120));
        assert_eq!(promoted_act3_tier(9641020,950000001),Some(9641140));
        assert_eq!(promoted_act3_tier(9641030,0),Some(9641160));
        assert_eq!(promoted_act3_tier(9641070,0),Some(9641120));
        assert_eq!(promoted_act3_tier(9641090,0),Some(9641140));
        assert_eq!(promoted_act3_tier(9641100,0),Some(9641160));
        assert_eq!(promoted_act3_tier(9641120,0),None);
        assert_eq!(promoted_act3_tier(9641160,0),None);
        let mut run=erdtree_tower::Run::new(42,erdtree_tower::Class::Samurai);
        run.endless=true;
        run.act=5;
        let shop=run.nodes.iter().position(|n|n.kind==erdtree_tower::NodeKind::Shop).unwrap();
        let effects=merchant_effects(&run,shop).unwrap();
        assert_eq!(effects[0],TIER_FIRST+160);
        assert!(effects.contains(&ENDLESS_EFFECT));
    }
}
