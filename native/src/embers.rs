use std::time::{Duration,Instant};
use eldenring::{cs::{AttackElementCorrectParam,ChrIns,ChrInsExt,EquipParamGoods,EquipParamWeapon,FieldInsHandle,ReinforceParamWeapon,SoloParamRepository,SpEffectParam,WorldChrMan},param::SP_EFFECT_PARAM_ST};
use fromsoftware_shared::FromStatic;
use erdtree_tower::{Phase,Run,embers::{self,Ember,Gain,Strike,StrikeRequest}};

/// 余烬效果：scene-builder ember-patch 写入的 9670000 段
pub const FIRST:i32=9670000;
/// 无声处刑的旧常驻效果（处决状态 288）：只用于清理。挂着它时原版染红 / 染蓝凶刀的触发也会生效，处决会回血回蓝，所以不再挂上
const EXECUTION:i32=FIRST;
/// 无声处刑：致命一击后挂上 10 秒（精力消耗减半），由原生处决事件触发，不依赖处决状态
const EXECUTION_RELIEF:i32=FIRST+2;
const BLOOD:i32=FIRST+10;
const FROST:i32=FIRST+11;
const POISON:i32=FIRST+12;
const ROT:i32=FIRST+13;
const HUNTER_MARK:i32=FIRST+47;
fn buildup_effect(ember:Ember)->Option<i32> {
    match ember {Ember::Bloodlord=>Some(BLOOD),Ember::Frost=>Some(FROST),_=>None}
}
fn venom_effect(kind:u8)->Option<i32> {
    match kind {0=>Some(POISON),1=>Some(ROT),2=>Some(BLOOD),3=>Some(FROST),_=>None}
}
pub fn practice_held(held:&[Ember])->Vec<Ember> {
    held.iter().copied().filter(|e|!matches!(e,Ember::Rise|Ember::Feast|Ember::SpiritFeast|Ember::Harvest|Ember::Chain|Ember::SoulEater|Ember::Godslayer)).collect()
}
pub fn clear_practice_critical(){CRITICAL_PENDING.store(false,Ordering::Release);}
fn buildup_target(row:&mut SP_EFFECT_PARAM_ST) {
    // ChrIns::apply_speffect uses the recipient as its own source, unlike a weapon hit.
    row.set_effect_target_self_target(true);
}
fn apply_buildup(chr:&mut ChrIns,effect:i32) {
    if !matches!(effect,BLOOD|FROST|POISON|ROT) {return;}
    let Ok(params)=(unsafe {SoloParamRepository::instance_mut()}) else{return;};
    let Some(row)=params.get_mut::<SpEffectParam>(effect as u32) else{return;};
    buildup_target(row);
    chr.apply_speffect(effect,false);
}
const REVENGE:i32=FIRST+20;
/// 圣杯余响：喝圣杯瓶后的增益（造成的伤害提高、受到的伤害降低），持续时间由运行时控制
const ECHO:i32=FIRST+15;
/// 法师余烬：施法时减伤、施法时霸体（韧性伤害 ×0）、专注消耗降低（常驻）、专注充足时增伤
const BARRIER:i32=FIRST+16;
const MEDITATION:i32=FIRST+17;
const GLINTSTONE:i32=FIRST+18;
const FULL_MOON:i32=FIRST+19;
/// 灵唤师的共鸣：本机召唤物在场时挂在玩家身上（造成的伤害提高），召唤物倒下或消失就摘掉
const SPIRIT_RESONANCE:i32=FIRST+21;
/// 战技的精粹：持有即常驻，战技增伤（只对战技生效）与战技专注消耗降低各一行
const ARTS_ATTACK:i32=FIRST+22;
const ARTS_COST:i32=FIRST+23;
/// 坩埚骑士的铁壁：实际使用大盾防御时，格挡不耗精力。
const BULWARK:i32=FIRST+24;
/// 红狮子的不退：生命低于 LAST_STAND_HP_PERCENT% 时挂上（受到的削韧 ×1/3），回到以上就摘掉
const LAST_STAND:i32=FIRST+25;
pub(crate) fn last_stand(hp:i32,max:i32)->bool {hp>0&&max>0&&i64::from(hp)*100<i64::from(max)*i64::from(embers::LAST_STAND_HP_PERCENT)}
/// 鲜血骑士的破势：一次性发作在敌人身上的特效编号段（出血、睡眠、冻伤、发狂；毒、腐败、致死这些持续类不算）
const BURSTS:[(i32,i32,&str);4]=[(6400,6449,"出血"),(6450,6499,"睡眠"),(6700,6749,"冻伤"),(6750,6799,"发狂")];
fn burst_kind(id:i32)->Option<&'static str> {BURSTS.iter().find(|(low,high,_)|(*low..=*high).contains(&id)).map(|(_,_,name)|*name)}
/// 发作时把敌人的韧性压到这么低：下一下韧性伤害就失衡（直接写 0 不一定会触发失衡动作，留一点让引擎按正常流程判定）
pub const BREAK_TOUGHNESS:f32=0.001;
fn break_value(current:f32,max:f32,broken:bool)->Option<f32> {
    (current.is_finite()&&max.is_finite()&&max>0.0&&current>0.0&&!broken).then(||current.min(BREAK_TOUGHNESS))
}
pub fn prime_stance_break(chr:&mut ChrIns)->Option<(f32,f32)> {
    let armor=&mut chr.modules.super_armor;
    let before=armor.sa_durability;
    armor.sa_durability=break_value(before,armor.sa_durability_max,armor.poise_broken_state)?;
    armor.recover_time=armor.recover_time.max(5.0);
    Some((before,armor.sa_durability_max))
}
fn breaker_boss(rank:u8)->bool {rank<=1}
/// 鲜血骑士的破势：每只敌人上一帧身上的发作特效、全部特效、生命，以及上次失衡的时间（Boss 冷却用）
#[derive(Default)]
struct Break {key:u64,bursts:Vec<i32>,effects:Vec<i32>,hp:i32}
const FLASH:i32=FIRST+40;
/// 捕灵壶：捕捉成功时挂在敌人身上的一次性扣血（一下扣满最大生命，走游戏自己的死亡流程；scene-builder capture-patch）
pub const CAPTURE:i32=FIRST+29;
/// 卡利亚剑阵：挂在玩家身上触发 BehaviorParam_PC 9670044 召唤原版卡利亚圆阵（Bullet 10430100）
const PHALANX_CAST:i32=FIRST+44;
const NORMAL_ATTACK_WINDOW:Duration=Duration::from_millis(1200);
const MELEE_REACH:f32=6.5;
fn observe_normal_attack(until:&mut Option<Instant>,now:Instant,light_attack:bool,non_normal:bool) {
    if non_normal {*until=None;}
    else if light_attack {*until=Some(now+NORMAL_ATTACK_WINDOW);}
}
fn is_normal_melee_hit(attack_active:bool,attacker_is_spirit:bool,near_player_bullet:bool,dist:f32)->bool {
    attack_active&&!attacker_is_spirit&&!near_player_bullet&&dist<=MELEE_REACH
}
fn next_roll(seed:&mut u64,modulus:u32)->u32 {
    if *seed==0 {*seed=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0x9e37_79b9,|d|d.as_nanos() as u64)|1;}
    *seed^=*seed<<13;*seed^=*seed>>7;*seed^=*seed<<17;
    (*seed%u64::from(modulus)) as u32
}
/// 孤王的契约：场上仅有 1 组本机召唤物时挂在召唤物身上（伤害 +30%、减伤 20%），超过 1 组时摘除
const SPIRIT_COMMAND:i32=FIRST+50;
/// 仿身泪滴的替身：本机存活召唤物（生命 > 1）在场时挂在玩家身上，先直接减免 50% 伤害（防止被秒杀），再由运行时将等量伤害扣到召唤物身上
const SPIRIT_LINK:i32=FIRST+51;
fn spirit_link_transfer(taken:i32,mitigated_by_effect:bool,spare_hp:i32)->(i32,i32) {
    if taken<=0||spare_hp<=0 {return (0,0);}
    let share=i64::from(embers::SPIRIT_SHARE_PERCENT);
    if mitigated_by_effect {
        let denom=(100-share).max(1);
        let transfer=((i64::from(taken)*share/denom) as i32).min(spare_hp).max(0);
        (0,transfer)
    } else {
        let transfer=((i64::from(taken)*share/100) as i32).min(spare_hp).max(0);
        (transfer,transfer)
    }
}
fn spirit_regen_due(last:&mut Option<Instant>,active:bool,now:Instant)->bool {
    if !active {*last=None;return false;}
    let Some(at)=*last else{*last=Some(now);return false;};
    if now.duration_since(at)<Duration::from_secs(1) {return false;}
    *last=Some(now);true
}
fn frenzy_effect(stack:u8)->i32 {if stack<=8 {FIRST+30+i32::from(stack)}else{FIRST+100+i32::from(stack)-9}}
/// 旧版本常驻效果只用于清理；原生处决由攻击表的 350501 事件识别。
const CHAIN_RESIDENT:i32=FIRST+5;
#[cfg(test)]
const CHAIN_MARK:i32=FIRST+7;
use std::sync::atomic::{AtomicUsize,AtomicBool,Ordering};
static APPLY_ORIGINAL:AtomicUsize=AtomicUsize::new(0);
static CHAIN_OWNER:AtomicUsize=AtomicUsize::new(0);
static CRITICAL_PENDING:AtomicBool=AtomicBool::new(false);
static CRITICAL_HOOK_READY:AtomicBool=AtomicBool::new(false);
fn own_critical(owner:usize,source:usize,effect:i32)->bool {owner!=0&&source==owner&&effect==350501}
unsafe extern "C" fn observe_native_critical(target:usize,effect:i32,source:usize) {
    if own_critical(CHAIN_OWNER.load(Ordering::Relaxed),source,effect){CRITICAL_PENDING.store(true,Ordering::Release);}
    super::capture::observe_hit(target,effect,source);
}
#[unsafe(naked)]
unsafe extern "C" fn critical_entry() {
    core::arch::naked_asm!(
        "sub rsp, 0xc8", "mov [rsp+0x20], rax", "mov [rsp+0x28], rcx", "mov [rsp+0x30], rdx",
        "mov [rsp+0x38], r8", "mov [rsp+0x40], r9", "mov [rsp+0x48], r10", "mov [rsp+0x50], r11",
        "movdqu [rsp+0x60], xmm0", "movdqu [rsp+0x70], xmm1", "movdqu [rsp+0x80], xmm2",
        "movdqu [rsp+0x90], xmm3", "movdqu [rsp+0xa0], xmm4", "movdqu [rsp+0xb0], xmm5",
        "call {}", "movdqu xmm0, [rsp+0x60]", "movdqu xmm1, [rsp+0x70]", "movdqu xmm2, [rsp+0x80]",
        "movdqu xmm3, [rsp+0x90]", "movdqu xmm4, [rsp+0xa0]", "movdqu xmm5, [rsp+0xb0]",
        "mov rax, [rsp+0x20]", "mov rcx, [rsp+0x28]", "mov rdx, [rsp+0x30]", "mov r8, [rsp+0x38]",
        "mov r9, [rsp+0x40]", "mov r10, [rsp+0x48]", "mov r11, [rsp+0x50]", "add rsp, 0xc8",
        "jmp qword ptr [rip+{}]",sym observe_native_critical,sym APPLY_ORIGINAL);
}
pub unsafe fn install_critical_trace()->Result<(),String> {
    use hudhook::mh::{MhHook,MH_EnableHook};
    use std::ffi::c_void;
    let address=super::items::checked_function(0x3e8e70,&[0x48,0x8b,0xc4,0x57,0x41,0x54,0x41,0x55,0x41,0x56,0x41,0x57,0x48,0x83,0xec,0x50])? as *mut c_void;
    let hook=unsafe{MhHook::new(address,critical_entry as *mut c_void)}.map_err(|e|format!("Critical trace hook: {e:?}"))?;
    APPLY_ORIGINAL.store(hook.trampoline() as usize,Ordering::Release);
    unsafe{MH_EnableHook(address)}.ok().map_err(|e|format!("Critical trace enable: {e:?}"))?;
    CRITICAL_HOOK_READY.store(true,Ordering::Release);
    super::log("CHAIN_TEST native attack effect listener installed: 350501 / 3e8e70");Ok(())
}
#[derive(Clone,Copy,Default)]
pub struct ChainTrace {pub level:u32,pub resident:bool,pub marker:bool,pub rate:Option<f32>,pub critical_only:bool,pub effects:usize}
static CHAIN_TRACE:std::sync::Mutex<ChainTrace>=std::sync::Mutex::new(ChainTrace{level:0,resident:false,marker:false,rate:None,critical_only:false,effects:0});
static CRITICAL_EVENTS:std::sync::atomic::AtomicU32=std::sync::atomic::AtomicU32::new(0);
pub fn chain_trace()->(ChainTrace,u32) {(CHAIN_TRACE.lock().map(|t|*t).unwrap_or_default(),CRITICAL_EVENTS.load(std::sync::atomic::Ordering::Relaxed))}
pub fn begin_chain_test(){CRITICAL_EVENTS.store(0,std::sync::atomic::Ordering::Relaxed);CRITICAL_PENDING.store(false,Ordering::Release);}
fn observe_chain(chr:&ChrIns,level:u32) {
    let present:Vec<_>=chr.special_effect.entries().map(|e|e.param_id).collect();
    let active:Vec<_>=slots(Ember::Chain).unwrap().into_iter().filter(|id|present.contains(id)).collect();
    let row=unsafe{SoloParamRepository::instance()}.ok().and_then(|p|active.first().and_then(|id|p.get::<SpEffectParam>(*id as u32)));
    if let Ok(mut trace)=CHAIN_TRACE.lock(){*trace=ChainTrace{level,resident:CRITICAL_HOOK_READY.load(Ordering::Acquire),marker:CRITICAL_PENDING.load(Ordering::Acquire),
        rate:row.map(|r|r.physics_attack_rate()),critical_only:row.is_some_and(|r|r.state_info()==367&&r.throw_attack_param_change()),effects:active.len()};}
}
fn player_ids()->impl Iterator<Item=i32> {[EXECUTION,REVENGE,ECHO,BARRIER,MEDITATION,GLINTSTONE,FULL_MOON,SPIRIT_RESONANCE,SPIRIT_LINK,ARTS_ATTACK,ARTS_COST,BULWARK,LAST_STAND,CHAIN_RESIDENT].into_iter().chain((1..=embers::FRENZY_STACKS).map(frenzy_effect))}
/// 叠层余烬不设上限：每种两行轮换（灵魂收容挂在召唤物身上，其余挂在玩家身上）。
/// 层数变了就把新倍率写进没挂着的那一行、换上它，再摘掉旧的那一行；不改正在生效的那一行
fn slots(ember:Ember)->Option<[i32;2]> {
    let base=match ember {Ember::Mind=>42,Ember::HeavyCast=>45,Ember::VenomFletch=>48,Ember::Battlemage=>26,Ember::Godslayer=>60,Ember::Harvest=>62,Ember::SoulEater=>70,Ember::Chain=>80,Ember::SpiritKeeper=>90,_=>return None};
    Some([FIRST+base,FIRST+base+1])
}
/// 弓弩流派余烬：小弓(50)、长弓(51)、大弓(53)、弩(55)、弩炮(56)
const BOW_LATCH:Duration=Duration::from_millis(2500);
fn is_bow_weapon_type(wep_type:u16)->bool {matches!(wep_type,50|51|53|55|56)}
fn bow_stance_from(equipment:&eldenring::cs::ChrAsmEquipment,ids:&[i32],lookup:impl Fn(u32)->Option<u16>)->(bool,bool) {
    use eldenring::cs::{ChrAsmArmStyle as Style,ChrAsmHand as Hand};
    let is_bow=|slot:usize|ids.get(slot).copied().filter(|id|*id>0).and_then(|id|lookup(id as u32)).is_some_and(is_bow_weapon_type);
    let has_weapon=|slot:usize|ids.get(slot).copied().is_some_and(|id|id>0);
    match equipment.arm_style {
        Style::EmptyHanded=>(false,false),
        Style::RightBothHands=>{let b=is_bow(equipment.active_weapon_slot(Hand::Right) as usize);(b,b)},
        Style::LeftBothHands=>{let b=is_bow(equipment.active_weapon_slot(Hand::Left) as usize);(b,b)},
        _=>{
            let (r,l)=(equipment.active_weapon_slot(Hand::Right) as usize,equipment.active_weapon_slot(Hand::Left) as usize);
            let (rb,lb)=(is_bow(r),is_bow(l));
            let holding=rb||lb;
            let exclusive=holding&&(!has_weapon(r)||rb)&&(!has_weapon(l)||lb);
            (holding,exclusive)
        },
    }
}
fn bow_stance(player:&eldenring::cs::PlayerIns)->(bool,bool) {
    let Ok(params)=(unsafe {SoloParamRepository::instance()}) else{return (false,false);};
    bow_stance_from(&player.chr_asm.equipment,&player.chr_asm.equipment_param_ids,|id|{
        params.get::<EquipParamWeapon>(id).or_else(||params.get::<EquipParamWeapon>(id/100*100)).map(|r|r.wep_type())
    })
}
fn is_bow_hit(bow_exclusive:bool,bow_active:bool,attacker_is_spirit:bool,near_player_bullet:bool)->bool {
    !attacker_is_spirit&&bow_active&&(near_player_bullet||bow_exclusive)
}
/// 百毒淬羽：统计目标当前存在的不同异常种类（stateInfo: 2 中毒、5 猩红腐败、260 冻伤、6 出血、436 睡眠、437 发狂）；
/// 出血/睡眠/发狂等瞬发异常在触发后记忆 VENOM_BURST_SECONDS 秒，与持续型异常统一计入最多 4 种。
fn observe_ailments(state_infos:impl IntoIterator<Item=i32>,bursts:&mut [Option<Instant>;3],now:Instant)->u32 {
    let mut mask=0u8;
    let burst_until=now+Duration::from_secs(embers::VENOM_BURST_SECONDS);
    for info in state_infos {
        match info {
            2=>mask|=1<<0,
            5=>mask|=1<<1,
            260=>mask|=1<<2,
            6=>{mask|=1<<3;bursts[0]=Some(burst_until);},
            436=>{mask|=1<<4;bursts[1]=Some(burst_until);},
            437=>{mask|=1<<5;bursts[2]=Some(burst_until);},
            _=>{},
        }
    }
    for (bit,until) in [(3u8,bursts[0]),(4u8,bursts[1]),(5u8,bursts[2])] {
        if until.is_some_and(|t|now<t) {mask|=1<<bit;}
    }
    mask.count_ones().min(embers::VENOM_MAX_AILMENTS)
}
const HEAVY_CAST_LATCH:Duration=Duration::from_millis(2500);
fn selected_spell_mp()->u32 {
    let Ok(player)=(unsafe {eldenring::cs::PlayerIns::local_player()}) else{return 0;};
    let equip=&unsafe {player.player_game_data.as_ref()}.equipment.equip_magic_data;
    let Ok(slot)=usize::try_from(equip.selected_slot) else{return 0;};
    let Some(entry)=equip.entries.get(slot) else{return 0;};
    if entry.param_id<=0 {return 0;}
    let Ok(params)=(unsafe {SoloParamRepository::instance()}) else{return 0;};
    params.get::<eldenring::cs::Magic>(entry.param_id as u32).map_or(0,|row|row.mp().max(0) as u32)
}
fn heavy_cast_effective_mp(selected:u32,latch:Option<(u32,Instant)>,now:Instant)->u32 {
    latch.filter(|&(_,until)|now<until).map_or(selected,|(mp,_)|selected.max(mp))
}
const PLAYER_STACKING:[Ember;4]=[Ember::Godslayer,Ember::Harvest,Ember::SoulEater,Ember::Chain];
/// 处刑者的血宴：致命一击之后这么久之内自己打死的敌人算处决击杀（每次致命一击只算一只）
const HARVEST_WINDOW:Duration=Duration::from_secs(4);
fn holds(row:&SP_EFFECT_PARAM_ST,ember:Ember,rates:embers::Rates)->bool {
    if ember==Ember::SpiritKeeper {row.physics_attack_power_rate()==rates.attack&&row.slash_damage_cut_rate()==rates.cut}
    else if ember==Ember::Battlemage {row.slash_damage_cut_rate()==rates.cut}
    else {row.physics_attack_rate()==rates.attack&&row.max_hp_rate()==rates.hp}
}
fn write(row:&mut SP_EFFECT_PARAM_ST,ember:Ember,rates:embers::Rates) {
    if ember==Ember::Battlemage {
        let c=rates.cut;
        row.set_slash_damage_cut_rate(c);row.set_blow_damage_cut_rate(c);row.set_thrust_damage_cut_rate(c);row.set_neutral_damage_cut_rate(c);
        row.set_magic_damage_cut_rate(c);row.set_fire_damage_cut_rate(c);row.set_thunder_damage_cut_rate(c);row.set_dark_damage_cut_rate(c);
    } else if ember==Ember::SpiritKeeper {
        let a=rates.attack;
        row.set_physics_attack_power_rate(a);row.set_magic_attack_power_rate(a);row.set_fire_attack_power_rate(a);row.set_thunder_attack_power_rate(a);row.set_dark_attack_power_rate(a);
        let c=rates.cut;
        row.set_slash_damage_cut_rate(c);row.set_blow_damage_cut_rate(c);row.set_thrust_damage_cut_rate(c);row.set_neutral_damage_cut_rate(c);
        row.set_magic_damage_cut_rate(c);row.set_fire_damage_cut_rate(c);row.set_thunder_damage_cut_rate(c);row.set_dark_damage_cut_rate(c);
    } else {
        let a=rates.attack;
        row.set_physics_attack_rate(a);row.set_magic_attack_rate(a);row.set_fire_attack_rate(a);row.set_thunder_attack_rate(a);row.set_dark_attack_rate(a);
        row.set_max_hp_rate(rates.hp);
    }
}
/// 这一层数该挂的那一行；present：目标角色身上已有的效果。None 表示不挂（没有层数、参数缺失，或两行都还挂着、等下一帧）
fn scaled_slot(ember:Ember,level:u32,present:&[i32])->Option<i32> {
    let rates=embers::stack_rates(ember,level)?;
    scaled_slot_rates(ember,level,present,rates)
}
fn scaled_slot_rates(ember:Ember,level:u32,present:&[i32],rates:embers::Rates)->Option<i32> {
    let slots=slots(ember)?;
    if level==0 {return None;}
    let params=unsafe {SoloParamRepository::instance_mut()}.ok()?;
    let fits=|id:i32,params:&SoloParamRepository|params.get::<SpEffectParam>(id as u32).is_some_and(|row|holds(row,ember,rates));
    if let Some(id)=slots.iter().copied().filter(|id|fits(*id,params)).max_by_key(|id|present.contains(id)) {return Some(id);}
    let free=slots.into_iter().find(|id|!present.contains(id))?;
    write(params.get_mut::<SpEffectParam>(free as u32)?,ember,rates);
    Some(free)
}
/// 由心及物换行后等这么久再重新计算：游戏不一定当帧就按新倍率更新最大生命，立刻重算会在两个倍率之间来回换
const MIND_SETTLE:Duration=Duration::from_secs(1);
static MIND_SWAPPED:std::sync::Mutex<Option<Instant>>=std::sync::Mutex::new(None);
fn mind_hp_slot(chr:&ChrIns,present:&[i32])->Option<i32> {
    let slots=slots(Ember::Mind)?;
    let current=slots.into_iter().find(|id|present.contains(id));
    let mut swapped=MIND_SWAPPED.lock().ok()?;
    if let Some(id)=current&&swapped.is_some_and(|at|at.elapsed()<MIND_SETTLE) {return Some(id);}
    let params=unsafe {SoloParamRepository::instance()}.ok()?;
    let rate_now=current.and_then(|id|params.get::<SpEffectParam>(id as u32)).map_or(1.0,|row|row.max_hp_rate());
    let data=&chr.modules.data;
    if rate_now<=0.0||data.max_hp<=0 {return current;}
    let base=(data.max_hp as f32/rate_now).round() as i32;
    let rate=embers::mind_hp_rate(base,data.max_fp);
    // 变化不到 1% 不换（专注上限小幅波动、取整误差）
    if let Some(id)=current&&(rate-rate_now).abs()<0.01 {return Some(id);}
    let want=scaled_slot_rates(Ember::Mind,1,present,embers::Rates{attack:1.0,hp:rate,cut:1.0});
    if want.is_some()&&want!=current {*swapped=Some(Instant::now());super::log(format!("TOWER_EMBER mind base_hp={base} max_fp={} rate={rate}",data.max_fp));}
    want.or(current)
}
/// 按 scaled_slot 的结果在一个角色身上换行：先挂新的再摘旧的（屠神者摘掉最大生命倍率时不会先把当前生命压低）
fn swap<C:ChrInsExt+?Sized>(chr:&mut C,present:&[i32],ember:Ember,want:Option<i32>) {
    let Some(slots)=slots(ember) else{return};
    if let Some(id)=want&&!present.contains(&id) {chr.apply_speffect(id,false);}
    for id in slots {if present.contains(&id)&&Some(id)!=want {chr.remove_speffect(id);}}
}
/// 击倒时按血条等级记魂点：带血条的 Boss、精英（含 Boss 房的随从）、普通敌人
fn kill_rank(bar_rank:u8)->u8 {match bar_rank {0=>2,3=>0,_=>1}}
fn ready(ids:&[i32])->bool {unsafe {SoloParamRepository::instance()}.is_ok_and(|p|ids.iter().all(|id|p.get::<SpEffectParam>(*id as u32).is_some()))}
/// 低于敌人最大生命 0.3% 的掉血视为持续伤害，不算命中；同一敌人 0.3 秒内只算一次
const HIT_PERMILLE:i64=3;
const HIT_INTERVAL:Duration=Duration::from_millis(300);
pub fn buildup_hit(damage:i32,max:i32,elapsed:Option<Duration>)->bool {
    damage>0&&i64::from(damage)*1000>=i64::from(max.max(1))*HIT_PERMILLE
        &&elapsed.is_none_or(|t|t>=HIT_INTERVAL)
}

/// 行商的无底囊：投掷道具的使用动作（EquipParamGoods.goodsUseAnim）。17 飞刀与飞镖、2 投掷弯刀、3 与 42 投掷壶和抛出的石块碎片、
/// 20 附绳壶、50 辉石碎片、67 双手投出的重壶（各动作对应的道具见 README）
const THROW_ANIMS:[u8;7]=[2,3,17,20,42,50,67];
/// 持有时在本机内存里把这些道具改成“使用不消耗”（不改 regulation），记下改过的行；不再持有时只恢复这些行
static POUCH_ROWS:std::sync::Mutex<Option<Vec<u32>>>=std::sync::Mutex::new(None);
fn sync_pouch(on:bool) {
    let Ok(mut rows)=POUCH_ROWS.lock() else{return};
    if on==rows.is_some() {return;}
    let Ok(params)=(unsafe {SoloParamRepository::instance_mut()}) else{return};
    if on {
        let mut changed=Vec::new();
        for (id,row) in params.rows_mut::<EquipParamGoods>() {
            if row.goods_type()==0&&row.is_consume()&&THROW_ANIMS.contains(&row.goods_use_anim()) {row.set_is_consume(false);changed.push(id);}
        }
        super::log(format!("TOWER_EMBER pouch on: {} throwables no longer consumed",changed.len()));
        *rows=Some(changed);
    } else {
        for id in rows.take().unwrap_or_default() {if let Some(row)=params.get_mut::<EquipParamGoods>(id) {row.set_is_consume(true);}}
        super::log("TOWER_EMBER pouch off: throwables consumed again");
    }
}
/// 黄金树的恩泽：原版治疗类祷告的回复特效（changeHpPoint 为负即回复）：急救、治疗、大治疗、王之治疗、黄金树治疗（含给队友的）、
/// 赐福的恩惠、黄金树的赐福、兽性活力，以及 DLC 的远距治疗、小黄金树。圣杯瓶、道具、武器的回复不在内
const MERCY_HEALS:[u32;18]=[1642000,1642001,1642100,1642101,1642200,1642201,1642300,1642301,1642400,1642401,
    1643000,1643001,1643100,1643101,1685000,21640000,21640001,21667000];
/// 持有时在本机内存里把这些行的回复量乘 1+MERCY_HEAL_PERCENT%（不改 regulation），记下原值；不再持有时改回原值
static MERCY_ROWS:std::sync::Mutex<Option<Vec<(u32,i32)>>>=std::sync::Mutex::new(None);
pub(crate) fn mercy_heal(point:i32)->i32 {(i64::from(point)*i64::from(100+embers::MERCY_HEAL_PERCENT)/100) as i32}
fn sync_mercy(on:bool) {
    let Ok(mut rows)=MERCY_ROWS.lock() else{return};
    if on==rows.is_some() {return;}
    let Ok(params)=(unsafe {SoloParamRepository::instance_mut()}) else{return};
    if on {
        let mut changed=Vec::new();
        for id in MERCY_HEALS {
            let Some(row)=params.get_mut::<SpEffectParam>(id) else{continue};
            let point=row.change_hp_point();
            if point<0 {row.set_change_hp_point(mercy_heal(point));changed.push((id,point));}
        }
        super::log(format!("TOWER_EMBER mercy on: {} healing incantation effects +{}%",changed.len(),embers::MERCY_HEAL_PERCENT));
        *rows=Some(changed);
    } else {
        for (id,point) in rows.take().unwrap_or_default() {if let Some(row)=params.get_mut::<SpEffectParam>(id) {row.set_change_hp_point(point);}}
        super::log("TOWER_EMBER mercy off: healing incantations restored");
    }
}
/// 战斗法师：属性补正表里预留的专用行（battlemage-patch 写进 regulation）。转换时把武器原来的补正规则复制过来，
/// 再让智力或信仰也计入物理伤害；武器参数行改用这一行，智力 / 信仰补正提到 S。都只改本机内存，读档后按存档重新写
const BATTLEMAGE_ELEMENTS:u32=9670000;
/// 战斗法师：原版满级（+25 / 失色 +10）时的目标有效补正（原版字母 S 门槛 175，取 185）；
/// 按满级对应的 ReinforceParamWeapon 补正倍率反算 +0 基础补正，避免强化后翻倍暴走。
const BATTLEMAGE_TARGET_S:f32=185.0;
fn battlemage_base_correct(vanilla_max_rate:f32)->f32 {
    BATTLEMAGE_TARGET_S/vanilla_max_rate.max(1.0)
}
struct Converted {row:u32,stat:u8,correct:f32,elements:i32}
static BATTLEMAGE:std::sync::Mutex<Vec<Converted>>=std::sync::Mutex::new(Vec::new());
/// 铁匠菜单据此显示「战斗法师」一项
pub static BATTLEMAGE_HELD:AtomicBool=AtomicBool::new(false);
fn sync_battlemage(wanted:&[(u32,u8)]) {
    let Ok(mut applied)=BATTLEMAGE.lock() else{return};
    if applied.len()==wanted.len()&&applied.iter().zip(wanted).all(|(a,(row,stat))|a.row==*row&&a.stat==*stat) {return;}
    let Ok(params)=(unsafe {SoloParamRepository::instance_mut()}) else{return};
    // 先全部改回原样，再按存档重新写
    for c in applied.drain(..) {
        if let Some(row)=params.get_mut::<EquipParamWeapon>(c.row) {
            if c.stat==0 {row.set_correct_magic(c.correct);}else{row.set_correct_faith(c.correct);}
            row.set_attack_element_correct_id(c.elements);
        }
    }
    for (slot,&(row_id,stat)) in wanted.iter().enumerate().take(embers::BATTLEMAGE_MAX_SLOTS) {
        let custom=BATTLEMAGE_ELEMENTS+slot as u32;
        let Some(row)=params.get::<EquipParamWeapon>(row_id) else{continue};
        let (correct,elements)=(if stat==0{row.correct_magic()}else{row.correct_faith()},row.attack_element_correct_id());
        let vanilla_cap=if row.origin_equip_wep1()<0 {0} else if row.origin_equip_wep16()<0 {10} else {25};
        let reinforce_type=i32::from(row.reinforce_type_id());
        let rate=if reinforce_type<0||vanilla_cap==0 {
            1.0
        } else {
            let ref_level=vanilla_cap.min(super::smith::level_max(params,row.reinforce_type_id()));
            params.get::<ReinforceParamWeapon>((reinforce_type+ref_level) as u32)
                .map_or(1.0,|rf|if stat==0 {rf.correct_magic_rate()} else {rf.correct_faith_rate()})
        };
        let target_correct=battlemage_base_correct(rate);
        let Some(source)=params.get::<AttackElementCorrectParam>(elements.max(0) as u32).map(|r|std::ptr::from_ref(r)) else{continue};
        let Some(target)=params.get_mut::<AttackElementCorrectParam>(custom) else{super::log_repeating("TOWER_EMBER battlemage: element correction rows missing; run battlemage-patch");return;};
        unsafe {std::ptr::copy_nonoverlapping(source,std::ptr::from_mut(target),1);}
        if stat==0 {target.set_is_magic_correct_by_physics(true);}else{target.set_is_faith_correct_by_physics(true);}
        let row=params.get_mut::<EquipParamWeapon>(row_id).unwrap();
        if stat==0 {row.set_correct_magic(correct.max(target_correct));}else{row.set_correct_faith(correct.max(target_correct));}
        row.set_attack_element_correct_id(custom as i32);
        applied.push(Converted{row:row_id,stat,correct,elements});
    }
    super::log(format!("TOWER_EMBER battlemage weapons: {wanted:?}"));
}
pub fn held(run:Option<&Run>)->Vec<Ember> {
    run.filter(|r|!matches!(r.phase,Phase::Dead|Phase::Won)).map(|r|r.embers.clone()).unwrap_or_default()
}
/// 玩家身上的余烬效果：以血还血、癫火狂热等只在战斗中按状态挂上；无声处刑在致命一击时由 Fight::tick 挂上
fn bulwark_guard_weapon(equipment:&eldenring::cs::ChrAsmEquipment,ids:&[i32],hand:eldenring::cs::ChrAsmHand)->bool {
    if equipment.arm_style==eldenring::cs::ChrAsmArmStyle::EmptyHanded {return false;}
    let slot=equipment.active_weapon_slot(hand) as usize;
    ids.get(slot).copied().filter(|id|*id>0)
        .is_some_and(|id|erdtree_tower::affixes::base(id as u32/100*100)/1_000_000==32)
}
pub fn bulwark_guard(player:&eldenring::cs::PlayerIns)->bool {
    bulwark_guard_weapon(&player.chr_asm.equipment,&player.chr_asm.equipment_param_ids,player.guard_reference_hand)
}
pub fn sync(chr:&mut ChrIns,run:Option<&Run>,fight:&Fight,spirit_alive:bool,spirit_link:bool,greatshield_guard:bool) {
    let mut want=Vec::new();
    let embers=held(run);
    sync_pouch(embers.contains(&Ember::Pouch));
    super::capture::sync(embers.contains(&Ember::Capture));
    if chr.modules.data.hp<=0||!run.is_some_and(|r|matches!(r.phase,Phase::Combat{..}|Phase::Node{..})) {super::capture::deactivate();}
    sync_mercy(embers.contains(&Ember::Mercy));
    let battlemage=embers.contains(&Ember::Battlemage);BATTLEMAGE_HELD.store(battlemage,Ordering::Relaxed);
    sync_battlemage(if battlemage {run.map_or(&[][..],|r|r.battlemage.as_slice())}else{&[]});
    let owner=if embers.contains(&Ember::Chain)||embers.contains(&Ember::Execution)||embers.contains(&Ember::Harvest){std::ptr::from_ref(chr) as usize}else{0};
    if CHAIN_OWNER.swap(owner,Ordering::AcqRel)!=owner{CRITICAL_PENDING.store(false,Ordering::Release);}
    if fight.revenge_active() {want.push(REVENGE);}
    if fight.echo_active() {want.push(ECHO);}
    if embers.contains(&Ember::Glintstone) {want.push(GLINTSTONE);}
    if embers.contains(&Ember::Bulwark)&&greatshield_guard {want.push(BULWARK);}
    if embers.contains(&Ember::LastStand)&&last_stand(chr.modules.data.hp,chr.modules.data.max_hp) {want.push(LAST_STAND);}
    if embers.contains(&Ember::Arts) {want.extend([ARTS_ATTACK,ARTS_COST]);}
    if fight.casting() {
        if embers.contains(&Ember::Barrier) {want.push(BARRIER);}
        if embers.contains(&Ember::Meditation) {want.push(MEDITATION);}
    }
    if fight.full_moon {want.push(FULL_MOON);}
    if resonance_active(&embers,spirit_alive) {want.push(SPIRIT_RESONANCE);}
    if embers.contains(&Ember::SpiritLink)&&spirit_link {want.push(SPIRIT_LINK);}
    if fight.frenzy>0 {want.push(frenzy_effect(fight.frenzy));}
    let present:Vec<_>=chr.special_effect.entries().map(|e|e.param_id).collect();
    let stacks=run.map(|r|r.ember_stacks).unwrap_or_default();
    for ember in PLAYER_STACKING {
        let level=if embers.contains(&ember) {stacks.level(ember)}else{0};
        let want=stacks.rates(ember).and_then(|rates|scaled_slot_rates(ember,level,&present,rates));
        swap(chr,&present,ember,want);
    }
    // 战斗法师的减伤：按智力、信仰较高者，能力值变了就换一行
    let mind=if battlemage {unsafe {eldenring::cs::PlayerIns::local_player()}.ok().map_or(0,|p|{let d=unsafe {p.player_game_data.as_ref()};d.intelligence.max(d.faith)})}else{0};
    swap(chr,&present,Ember::Battlemage,scaled_slot(Ember::Battlemage,mind,&present));
    // 由心及物：最大生命 + 最大专注 × 3，按不算这枚余烬的最大生命换算成倍率
    let mind_slot=if embers.contains(&Ember::Mind) {mind_hp_slot(chr,&present)}else{None};
    swap(chr,&present,Ember::Mind,mind_slot);
    // 重咏之威：按当前选中（或刚施放）法术/祷告的基础专注消耗动态缩放法术/祷告伤害（不影响近战武器）
    let heavy_slot=if embers.contains(&Ember::HeavyCast) {
        let mp=fight.heavy_cast_mp.max(selected_spell_mp());
        let rate=embers::heavy_cast_rate(mp);
        scaled_slot_rates(Ember::HeavyCast,mp,&present,embers::Rates{attack:rate,hp:1.0,cut:1.0})
    }else{None};
    swap(chr,&present,Ember::HeavyCast,heavy_slot);
    // 百毒淬羽：按目标当前异常种数动态缩放弓弩弹药伤害（保留 magicSubCategoryChange1=100，不影响近战/法术）
    let venom_slot=if embers.contains(&Ember::VenomFletch)&&fight.venom_ailments>0 {
        let rate=embers::venom_fletch_rate(fight.venom_ailments);
        scaled_slot_rates(Ember::VenomFletch,fight.venom_ailments,&present,embers::Rates{attack:rate,hp:1.0,cut:1.0})
    }else{None};
    swap(chr,&present,Ember::VenomFletch,venom_slot);
    for id in player_ids() {if present.contains(&id)&&!want.contains(&id){chr.remove_speffect(id);}}
    if want.is_empty()||!ready(&want) {observe_chain(chr,stacks.level(Ember::Chain));return;}
    for id in want {if !present.contains(&id){chr.apply_speffect(id,false);}}
    observe_chain(chr,stacks.level(Ember::Chain));
}
fn resonance_active(held:&[Ember],spirit_alive:bool)->bool {
    spirit_alive&&held.contains(&Ember::SpiritResonance)
}
pub fn living_spirit(world:&WorldChrMan)->bool {
    !super::spirits::local_living_handles(world).is_empty()
}
pub fn linkable_spirit(world:&WorldChrMan)->bool {
    super::spirits::local_living_handles(world).iter().filter_map(|h|world.chr_ins_by_handle(h)).any(|c|c.modules.data.hp>1)
}
fn distance(a:[f32;3],b:[f32;3])->f32 {((a[0]-b[0]).powi(2)+(a[1]-b[1]).powi(2)+(a[2]-b[2]).powi(2)).sqrt()}
/// slot：这场战斗敌人名单里的位置（联机时房主与成员一致，成员发来的敌人侧效果按它找敌人）
struct Foe {
    handle:FieldInsHandle,hp:i32,max:i32,pos:[f32;3],alive:bool,boss:bool,rank:u8,slot:u8,hit_at:Option<Instant>,participation:Participation,
    mark_hits:u8,marked_until:Option<Instant>,venom_at:Option<Instant>,recent_bursts:[Option<Instant>;3],
}
#[derive(Default)]
struct Participation {damaged:bool,settled:bool}
impl Participation {
    fn observe(&mut self,mine:bool,damage:i32,died:bool)->bool {
        self.damaged|=mine&&damage>0;
        let credit=died&&self.damaged&&!self.settled;
        self.settled|=died;
        credit
    }
}
/// 一帧的结果：噬神之蛇的饱食是否要补红瓶、让叠层余烬加层的事件、联机成员要请房主执行的敌人侧效果
#[derive(Default)]
pub struct Events {pub feast:bool,pub gains:Vec<Gain>,pub strikes:Vec<Strike>,
    /// 捕灵壶：本机玩家捕捉到的敌人（模型, 名单位置），由运行时按名单查 AI 参数后发对应的骨灰
    pub captures:Vec<(i32,u8)>}
/// 本机玩家与本机召唤物：联机时只算他们的命中与击倒（看敌人的 last_hit_by）
fn own_handles(world:&WorldChrMan)->Vec<FieldInsHandle> {
    world.main_player.as_ref().map(|p|p.chr_ins.field_ins_handle).into_iter().chain(super::spirits::local_handles(world)).collect()
}
/// 联机时敌人的 last_hit_by 是空的（成员这边敌人的生命来自房主的同步，同步里只有生命与伤害、没有攻击者）：
/// 离这只敌人最近的是本机玩家或本机召唤物（不比队友远）就算自己的。近战准，远程法术会算给离得近的队友
/// 法术、祷告、弓箭等远程攻击：游戏的弹体记着施放者（owner_chr_handle）。最近 BULLET_MEMORY 内落在敌人
/// BULLET_REACH 米以内的弹体，按施放者是不是本机玩家或本机召唤物归属；没有弹体再按最近距离兜底
const BULLET_MEMORY:Duration=Duration::from_millis(700);
const BULLET_REACH:f32=6.0;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
enum BulletOwner {Player,Spirit,Teammate}
fn bullet_owner(foe:[f32;3],bullets:&[(Instant,[f32;3],BulletOwner)])->Option<BulletOwner> {
    bullets.iter().map(|&(_,p,owner)|(distance(foe,p),owner)).filter(|&(d,_)|d<=BULLET_REACH)
        .min_by(|a,b|a.0.total_cmp(&b.0)).map(|(_,owner)|owner)
}
#[cfg(test)]
fn bullet_owner_is_mine(foe:[f32;3],bullets:&[(Instant,[f32;3],BulletOwner)])->Option<bool> {
    bullet_owner(foe,bullets).map(|owner|!matches!(owner,BulletOwner::Teammate))
}
fn killed_by_spirit(attacker_empty:bool,attacker_is_spirit:bool,bullet:Option<BulletOwner>,foe:[f32;3],spirit_at:&[[f32;3]],non_spirit_at:&[[f32;3]])->bool {
    if !attacker_empty {return attacker_is_spirit;}
    match bullet {
        Some(BulletOwner::Spirit)=>true,
        Some(BulletOwner::Player|BulletOwner::Teammate)=>false,
        None=>nearest_is_mine(foe,spirit_at,non_spirit_at),
    }
}
fn nearest_is_mine(foe:[f32;3],mine:&[[f32;3]],others:&[[f32;3]])->bool {
    let closest=|set:&[[f32;3]]|set.iter().map(|&p|distance(foe,p)).fold(f32::INFINITY,f32::min);
    !mine.is_empty()&&closest(mine)<=closest(others)
}
/// 一场战斗内的余烬状态：按敌人生命变化判断命中与倒下，按玩家生命变化判断受伤
#[derive(Default)]
pub struct Fight {
    key:Option<(u64,u8,usize)>,foes:Vec<Foe>,player_hp:Option<i32>,flasks:Option<u8>,
    lifesteal:i64,rose:bool,dragon_ready:Option<Instant>,revenge_until:Option<Instant>,echo_until:Option<Instant>,pub frenzy:u8,
    /// 施法时：按住施法键期间与松开后 CAST_LINGER_MS 内；cast_seen 只用于每场战斗记一次日志
    cast_until:Option<Instant>,cast_seen:bool,
    /// 最近的玩家/召唤物弹体（时间、位置、归属），给联机查不到攻击者及灵魂收容判定召唤物击杀用
    bullets:Vec<(Instant,[f32;3],BulletOwner)>,
    /// 蕾娜拉的满月：专注不低于 MOON_FP_PERCENT%
    full_moon:bool,
    /// 黑刀刺客的身法：退还翻滚与致命一击扣掉的精力
    footwork:Footwork,arts_stamina:ArtsStamina,feasts:u8,missing:bool,
    /// 上一帧在场的召唤物位置（灵骸迸裂在它消失时引爆）
    spirit:Option<[f32;3]>,
    /// 上一帧玩家身上有处刑之链的致命一击标记
    critical_edge:CriticalEdge,
    /// 处刑者的血宴：最近一次致命一击的时间（之后 HARVEST_WINDOW 内自己打死的敌人算处决击杀，算过就清掉）
    critical_at:Option<Instant>,
    /// 房主：每个成员已执行到的敌人侧效果序号
    remote:Vec<(u64,u32)>,
    /// 联机时这场战斗是否已记过第一次“自己的命中”与“别人的命中”（日志，用来核对 last_hit_by 的归属）
    attributed:[bool;2],
    /// 鲜血骑士的破势：每只敌人的发作状态；probes：本场已记的校准日志条数
    breaks:Vec<Break>,probes:u8,
    capture_seed:u64,
    /// 卡利亚剑阵：最近一次轻攻击输入窗口与剑阵内置冷却到期时间
    normal_attack_until:Option<Instant>,phalanx_ready:Option<Instant>,
    /// 孤王的契约：召唤物每秒持续回血计时
    lone_king_regen:Option<Instant>,
    /// 重咏之威：当前生效的基础专注消耗，及刚施放高耗蓝法术时的短暂锁存（切法术不掉在飞弹体的加成）
    heavy_cast_mp:u32,heavy_cast_latch:Option<(u32,Instant)>,
    /// 狩猎印记 & 百毒淬羽：最近手持弓弩的锁存窗口、百毒淬羽优先追踪目标与当前生效异常种数
    bow_until:Option<Instant>,venom_target:Option<FieldInsHandle>,venom_ailments:u32,
}
impl Fight {
    pub fn observe_practice_refill(&mut self,world:&WorldChrMan) {
        for foe in &mut self.foes {
            if let Some(chr)=world.chr_ins_by_handle(&foe.handle) {foe.hp=chr.modules.data.hp;}
        }
    }
    pub fn active(&self)->bool {self.key.is_some()}
    /// 鲜血骑士的破势（只由房主或单人执行，敌人由他们计算）：敌人身上新出现一次性发作时压低韧性，下一下就失衡；
    /// Boss 与普通敌人均无冷却。在 tick 之后调用（tick 换场时会清掉状态）。
    /// 校准：敌人一帧掉了 10% 以上的生命时，把这一帧新出现的特效记进日志（每场最多 12 条），用来核对发作特效的编号段
    pub fn break_on_bursts(&mut self,world:&mut WorldChrMan,foes:&[(FieldInsHandle,u8,u8)]) {
        for &(handle,bar,slot) in foes {
            let Some(chr)=world.chr_ins_by_handle_mut(&handle) else{continue};
            let (hp,max)=(chr.modules.data.hp,chr.modules.data.max_hp);
            if hp<=0||chr.chr_flags1c5.death_flag(){continue;}
            let key=(u64::from(handle.block_id.0 as u32)<<32)|u64::from(handle.selector.0);
            let effects:Vec<i32>=chr.special_effect.entries().map(|e|e.param_id).collect();
            let bursts:Vec<i32>=effects.iter().copied().filter(|id|burst_kind(*id).is_some()).collect();
            let index=match self.breaks.iter().position(|b|b.key==key) {Some(i)=>i,None=>{self.breaks.push(Break{key,bursts:bursts.clone(),effects:effects.clone(),hp});continue;}};
            let state=&mut self.breaks[index];
            if self.probes<12&&max>0&&state.hp-hp>max/10 {
                let fresh:Vec<i32>=effects.iter().copied().filter(|id|!state.effects.contains(id)).collect();
                self.probes+=1;super::log(format!("TOWER_EMBER breaker probe slot={slot} hp {}->{hp}/{max} new_effects={fresh:?}",state.hp));
            }
            let fresh=bursts.iter().copied().find(|id|!state.bursts.contains(id));
            state.bursts=bursts;state.effects=effects;state.hp=hp;
            let Some(id)=fresh else{continue};
            let boss=breaker_boss(bar);
            if let Some((before,max))=prime_stance_break(chr) {
                super::log(format!("TOWER_EMBER breaker slot={slot} {} effect={id} boss={boss} stance {before}->{BREAK_TOUGHNESS} max={max}; next hit within 5s",burst_kind(id).unwrap_or("?")));
            } else {super::log(format!("TOWER_EMBER breaker slot={slot} effect={id}: no active breakable stance"));}
        }
    }
    pub fn revenge_active(&self)->bool {self.revenge_until.is_some_and(|until|Instant::now()<until)}
    pub fn echo_active(&self)->bool {self.echo_until.is_some_and(|until|Instant::now()<until)}
    pub fn casting(&self)->bool {self.cast_until.is_some_and(|until|Instant::now()<until)}
    /// foes：(句柄, 血条等级 enemies::bar_rank, 名单位置)；authority：房主或单人（敌人侧效果直接执行，否则交给房主）；
    /// party：联机（命中与击倒只算本机玩家与召唤物的）；keeper：灵魂收容当前层数
    /// track_kills：没有余烬也要统计击倒（星辉术士「星涌」击倒回专注用）
    pub fn tick(&mut self,world:&mut WorldChrMan,key:(u64,u8,usize),held:&[Ember],foes:&[(FieldInsHandle,u8,u8)],authority:bool,party:bool,flasks:Option<u8>,keeper:u32,track_kills:bool)->Events {
        if self.key!=Some(key) {*self=Self{key:Some(key),..Default::default()};super::glintblade::reset();}
        // 魔力辉剑：持有时才在弹体生成挂钩里记录本机玩家的施法
        super::glintblade::set_owner(world.main_player.as_ref().filter(|_|held.contains(&Ember::Glintblade)).map(|p|&p.chr_ins.field_ins_handle));
        let drank=matches!((self.flasks,flasks),(Some(before),Some(after)) if after<before);self.flasks=flasks;
        let mut events=Events::default();
        if !held.contains(&Ember::Capture) {super::capture::deactivate();}
        if held.is_empty()&&!track_kills {return events;}
        let has=|e:Ember|held.contains(&e);
        let now=Instant::now();
        let Some(player)=world.main_player.as_ref() else{return events};
        if player.chr_ins.modules.data.hp<=0||player.chr_ins.chr_flags1c5.death_flag() {super::capture::deactivate();return events;}
        // 致命一击（原生处决事件，见 install_critical_trace）：处刑之链加层，无声处刑挂上精力减半
        if has(Ember::Chain)||has(Ember::Execution)||has(Ember::Harvest) {
            let marked=CRITICAL_PENDING.swap(false,Ordering::AcqRel);
            let throwing=unsafe{std::ptr::addr_of!(player.chr_ins.modules.throw.throw_node.throw_state).cast::<u32>().read()}==3;
            if self.critical_edge.observe(marked,throwing) {
                if has(Ember::Chain) {events.gains.push(Gain::Critical);}
                if has(Ember::Harvest) {self.critical_at=Some(now);}
                let count=CRITICAL_EVENTS.fetch_add(1,std::sync::atomic::Ordering::Relaxed)+1;
                super::log(format!("CHAIN_TEST real critical marker observed count={count}"));
                if has(Ember::Execution)&&ready(&[EXECUTION_RELIEF]) {
                    world.main_player.as_mut().unwrap().chr_ins.apply_speffect(EXECUTION_RELIEF,false);
                    super::log("TOWER_EMBER execution relief applied");
                }
            }
        }
        let Some(player)=world.main_player.as_ref() else{return events};
        let captures=if has(Ember::Capture) {
            let targets=foes.iter().filter_map(|(handle,_,_)|world.chr_ins_by_handle(handle))
                .filter(|chr|chr.modules.data.hp>0).map(|chr|super::capture::Hit{target:chr.field_ins_handle,hp:chr.modules.data.hp,max:chr.modules.data.max_hp}).collect();
            super::capture::begin(&player.chr_ins,key,targets)
        } else {Vec::new()};
        let data=&player.chr_ins.modules.data;
        let (hp,max_hp,stamina,max_stamina)=(data.hp,data.max_hp,data.stamina,data.max_stamina);
        // 法师余烬：施法键（主手、副手，含武器上的施法）按住即算施法，松开后再保持一会儿，出招动作放完前都算
        let cast_pressed={
            let input=&player.chr_ins.modules.action_request.action_requests;
            input.magic_r()||input.magic_l()||input.magic_r2()||input.magic_l2()
        };
        if (has(Ember::Barrier)||has(Ember::Meditation))&&cast_pressed {
            self.cast_until=Some(now+Duration::from_millis(embers::CAST_LINGER_MS));
            if !self.cast_seen {self.cast_seen=true;super::log("TOWER_EMBER cast detected");}
        }
        if has(Ember::HeavyCast) {
            let selected=selected_spell_mp();
            if cast_pressed&&selected>0 {self.heavy_cast_latch=Some((selected,now+HEAVY_CAST_LATCH));}
            self.heavy_cast_mp=heavy_cast_effective_mp(selected,self.heavy_cast_latch,now);
        } else {
            self.heavy_cast_mp=0;self.heavy_cast_latch=None;
        }
        // 卡利亚剑阵：记录轻攻击（R1 / 非举盾 L1）输入窗口；施法、战技或重击输入清空窗口，避免误把法术/战技算成普攻
        if has(Ember::Phalanx) {
            let req=&player.chr_ins.modules.action_request;
            let (input,pressed)=(&req.action_requests,&req.new_action_presses);
            let non_normal=input.magic_r()||input.magic_l()||input.magic_r2()||input.magic_l2()
                ||pressed.l2()||input.l2()||pressed.r2()||input.r2();
            let light_attack=pressed.r1()||input.r1()||((pressed.l1()||input.l1())&&!input.guard());
            observe_normal_attack(&mut self.normal_attack_until,now,light_attack,non_normal);
        }
        // 黑刀刺客的身法：刚按下闪避键、短按松开（长按是冲刺）、刚进入致命一击时，退还这次动作扣掉的精力
        let mut refund=0;
        if has(Ember::Footwork) {
            let request=&player.chr_ins.modules.action_request;
            let (pressed,released)=(&request.new_action_presses,&request.released_actions);
            if pressed.sp_move() {self.footwork.held_since=Some(now);}
            let tap=released.sp_move()&&self.footwork.held_since.take().is_some_and(|t|now.duration_since(t)<FOOTWORK_TAP);
            let dodge=pressed.sp_move()||tap||pressed.rolling()||pressed.backstep()||pressed.emergencystep();
            let throwing=unsafe{std::ptr::addr_of!(player.chr_ins.modules.throw.throw_node.throw_state).cast::<u32>().read()}==3;
            let critical=throwing&&!self.footwork.throwing;self.footwork.throwing=throwing;
            refund=self.footwork.observe(stamina,dodge||critical,now);
            if refund>0&&!self.footwork.logged {self.footwork.logged=true;super::log(format!("TOWER_EMBER footwork refund {refund} critical={critical}"));}
        }
        if has(Ember::Arts) {
            let request=&player.chr_ins.modules.action_request;
            let (input,pressed)=(&request.action_requests,&request.new_action_presses);
            let skill=pressed.l2()||input.l2();
            let dodge=pressed.rolling()||pressed.backstep()||pressed.emergencystep();
            refund+=self.arts_stamina.observe(stamina,skill,dodge,refund>0,now);
        }
        self.full_moon=has(Ember::FullMoon)&&data.max_fp>0&&i64::from(data.fp)*100>=i64::from(data.max_fp)*i64::from(embers::MOON_FP_PERCENT);
        let p=player.chr_ins.modules.physics.position;let here=[p.0,p.1,p.2];
        let taken=self.player_hp.map_or(0,|before|(before-hp).max(0));self.player_hp=Some(hp);
        // 敌人：命中与倒下。单人时都算自己的；联机时按敌人的 last_hit_by 只算本机玩家与召唤物的（队友与队友召唤物的不算进自己的余烬）
        let spirit_handles=super::spirits::local_handles(world);
        let own=own_handles(world);
        let local_spirits=super::spirits::local_living_handles(world);
        let living=|c:&ChrIns|c.modules.data.hp>0&&!c.chr_flags1c5.death_flag()&&!c.debug_flags.character_disabled();
        let at=|c:&ChrIns|{let q=c.modules.physics.position;[q.0,q.1,q.2]};
        let spirit_at:Vec<_>=local_spirits.iter().filter_map(|h|world.chr_ins_by_handle(h)).map(at).collect();
        let mine_at:Vec<_>=std::iter::once(here).chain(spirit_at.iter().copied()).collect();
        let others_at:Vec<_>=world.player_chr_set.characters().map(|p|&p.chr_ins)
            .chain(world.summon_buddy_chr_set.characters().map(|c|&*c))
            .filter(|c|!own.contains(&c.field_ins_handle)&&living(c)).map(at).collect();
        let non_spirit_at:Vec<_>=std::iter::once(here).chain(others_at.iter().copied()).collect();
        let (holding_bow,bow_exclusive)=if has(Ember::HunterMark)||has(Ember::VenomFletch) {bow_stance(player)}else{(false,false)};
        if holding_bow {self.bow_until=Some(now+BOW_LATCH);}
        let bow_active=holding_bow||self.bow_until.is_some_and(|u|now<=u);
        if party||has(Ember::Glintblade)||has(Ember::SpiritKeeper)||has(Ember::Phalanx)||has(Ember::HunterMark)||has(Ember::VenomFletch) {
            let teammates:Vec<_>=world.player_chr_set.characters().map(|p|p.chr_ins.field_ins_handle)
                .chain(world.summon_buddy_chr_set.characters().map(|c|c.field_ins_handle))
                .filter(|h|!own.contains(h)).collect();
            self.bullets.retain(|&(t,_,_)|now.duration_since(t)<=BULLET_MEMORY);
            if let Ok(manager)=unsafe {eldenring::cs::CSBulletManager::instance()} {
                for bullet in manager.bullets() {
                    let owner=bullet.targeting_owner.owner_chr_handle;
                    let kind=if spirit_handles.contains(&owner) {BulletOwner::Spirit}
                        else if own.contains(&owner) {BulletOwner::Player}
                        else if teammates.contains(&owner) {BulletOwner::Teammate}
                        else {continue;};
                    let q=bullet.physics.position;
                    self.bullets.push((now,[q.0,q.1,q.2],kind));
                }
            }
            if self.bullets.len()>512 {let excess=self.bullets.len()-512;self.bullets.drain(..excess);}
        }
        // 自己造成的伤害（米凯拉之刃按它吸血）：自己那几只敌人每一帧掉的生命都算，不受命中判定的门槛与间隔限制
        let (mut hits,mut deaths,mut dealt)=(Vec::new(),Vec::<(usize,bool)>::new(),0i64);
        let mut captured:Vec<usize>=Vec::new();
        let mut blades:Vec<(FieldInsHandle,[f32;3])>=Vec::new();
        let mut phalanx_hits=0u32;
        let mut hunter_mark_kills=0u32;
        let mut mark_procs:Vec<usize>=Vec::new();
        let mut venom_procs:Vec<(usize,u8)>=Vec::new();
        let (mut best_ailments,mut target_ailments)=(0u32,None::<u32>);
        let sp_repo=if has(Ember::VenomFletch) {unsafe {SoloParamRepository::instance()}.ok()} else {None};
        for &(ref handle,rank,slot) in foes {
            let Some(chr)=world.chr_ins_by_handle(handle) else{continue};
            let (foe_hp,foe_max)=(chr.modules.data.hp,chr.modules.data.max_hp);
            let q=chr.modules.physics.position;let pos=[q.0,q.1,q.2];
            let alive=foe_hp>0&&!chr.chr_flags1c5.death_flag();
            let attacker=chr.last_hit_by;
            let nearest_bullet=if attacker.is_empty()&&(party||has(Ember::SpiritKeeper)) {bullet_owner(pos,&self.bullets)} else {None};
            let by_bullet=if party {nearest_bullet.map(|o|!matches!(o,BulletOwner::Teammate))} else {None};
            let mine=!party||if attacker.is_empty() {by_bullet.unwrap_or_else(||nearest_is_mine(pos,&mine_at,&others_at))} else {own.contains(&attacker)};
            let Some(index)=self.foes.iter().position(|f|f.handle==*handle) else {
                self.foes.push(Foe{handle:*handle,hp:foe_hp,max:foe_max,pos,alive,boss:rank<=1,rank,slot,hit_at:None,participation:Default::default(),mark_hits:0,marked_until:None,venom_at:None,recent_bursts:[None;3]});continue;
            };
            let foe=&mut self.foes[index];
            let drop=foe.hp-foe_hp;let lost=(foe.hp-foe_hp.max(0)).max(0);
            let died=foe.alive&&!alive;
            if !foe.alive&&alive {foe.participation=Default::default();foe.mark_hits=0;foe.marked_until=None;foe.venom_at=None;foe.recent_bursts=[None;3];}
            if has(Ember::VenomFletch)&&alive {
                let state_infos=chr.special_effect.entries().filter_map(|e|sp_repo.and_then(|p|p.get::<SpEffectParam>(e.param_id as u32)).map(|r|i32::from(r.state_info())));
                let ailments=observe_ailments(state_infos,&mut foe.recent_bursts,now);
                best_ailments=best_ailments.max(ailments);
                if self.venom_target==Some(*handle)&&ailments>0 {target_ailments=Some(ailments);}
            }
            if has(Ember::HunterMark)&&died {
                let marked_local=foe.marked_until.is_some_and(|u|now<=u);
                let marked_effect=chr.special_effect.entries().any(|e|e.param_id==HUNTER_MARK);
                if marked_local||(mine&&marked_effect) {
                    hunter_mark_kills+=1;
                    super::log(format!("TOWER_EMBER hunter_mark kill slot={slot} local={marked_local} mine={mine}"));
                }
                foe.mark_hits=0;foe.marked_until=None;
            }
            if foe.participation.observe(mine,lost,died)&&party&&!mine {
                events.gains.push(Gain::Assist{rank:kill_rank(foe.rank)});
                super::log(format!("TOWER_EMBER assist handle={handle} rank={} soul credit",foe.rank));
            }
            let hit=!died&&alive&&buildup_hit(drop,foe_max,foe.hit_at.map(|t|now.duration_since(t)));
            if hit {foe.hit_at=Some(now);}
            (foe.hp,foe.max,foe.pos,foe.alive)=(foe_hp,foe_max,pos,alive);
            // 联机：每场战斗记一次“算进自己的”与“不算的”命中或击倒，实测时据此确认 last_hit_by 的归属
            if party&&(hit||died)&&!self.attributed[usize::from(!mine)] {
                self.attributed[usize::from(!mine)]=true;
                super::log(format!("TOWER_EMBER party {} by {attacker} counted={mine}{}",if died{"kill"}else{"hit"},if !attacker.is_empty(){""}else if by_bullet.is_some(){" (bullet)"}else{" (nearest)"}));
            }
            // Native pot hits carry their own source attribution; later hits cannot overwrite it.
            for capture in captures.iter().filter(|hit|hit.target==*handle&&!chr.debug_flags.disabled_hit()) {
                let before=capture.hp;
                if let Some(chance)=embers::capture_chance_permille(before,capture.max) {
                    if self.capture_seed==0 {self.capture_seed=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0x9e37_79b9,|d|d.as_nanos() as u64)|1;}
                    self.capture_seed^=self.capture_seed<<13;self.capture_seed^=self.capture_seed>>7;self.capture_seed^=self.capture_seed<<17;
                    let roll=(self.capture_seed%1000) as u32;
                    let caught=roll<chance;
                    super::log(format!("TOWER_EMBER capture model={} hp={before}/{foe_max} chance={chance}permille roll={roll} caught={caught} killed_by_hit={died}",chr.npc_id));
                    if caught {captured.push(index);events.captures.push((chr.npc_id,slot));break;}
                } else {super::log(format!("TOWER_CAPTURE above threshold model={} hp={}/{}",chr.npc_id,before,capture.max));}
            }
            if !mine {continue;}
            // 魔力辉剑：自己的弹体刚落在这只敌人附近、它掉了血，且还有一次没换过辉剑的施法
            if has(Ember::Glintblade)&&drop>0&&alive
                &&self.bullets.iter().any(|&(t,p,owner)|!matches!(owner,BulletOwner::Teammate)&&now.duration_since(t)<=BULLET_MEMORY&&distance(p,pos)<=BULLET_REACH)
                &&super::glintblade::take_cast() {blades.push((*handle,pos));}
            // 卡利亚剑阵 & 弓弩余烬：区分近战普攻与弓弩弹体命中
            let attacker_is_spirit=if !attacker.is_empty() {spirit_handles.contains(&attacker)} else {matches!(nearest_bullet,Some(BulletOwner::Spirit))};
            let near_player_bullet=self.bullets.iter().any(|&(t,p,owner)|matches!(owner,BulletOwner::Player)&&now.duration_since(t)<=BULLET_MEMORY&&distance(p,pos)<=BULLET_REACH);
            if has(Ember::Phalanx)&&(hit||died) {
                let attack_active=self.normal_attack_until.is_some_and(|until|now<=until);
                if is_normal_melee_hit(attack_active,attacker_is_spirit,near_player_bullet,distance(here,pos)) {phalanx_hits+=1;}
            }
            if (has(Ember::HunterMark)||has(Ember::VenomFletch))&&hit&&alive
                &&is_bow_hit(bow_exclusive,bow_active,attacker_is_spirit,near_player_bullet)
            {
                if has(Ember::HunterMark) {
                    foe.mark_hits=foe.mark_hits.saturating_add(1);
                    if foe.mark_hits>=embers::HUNTER_MARK_HITS {
                        foe.mark_hits=0;
                        foe.marked_until=Some(now+Duration::from_secs(embers::HUNTER_MARK_SECONDS));
                        mark_procs.push(index);
                    }
                }
                if has(Ember::VenomFletch) {
                    self.venom_target=Some(*handle);
                    if foe.venom_at.is_none_or(|t|now.duration_since(t)>=Duration::from_millis(embers::VENOM_COOLDOWN_MS)) {
                        foe.venom_at=Some(now);
                        let kind=next_roll(&mut self.capture_seed,4) as u8;
                        venom_procs.push((index,kind));
                    }
                }
            }
            dealt+=i64::from(lost);
            if died {
                let by_spirit=killed_by_spirit(attacker.is_empty(),spirit_handles.contains(&attacker),nearest_bullet,pos,&spirit_at,&non_spirit_at);
                deaths.push((index,by_spirit));
            } else if hit {hits.push((index,drop));}
        }
        self.venom_ailments=if has(Ember::VenomFletch) {target_ailments.unwrap_or(best_ailments)} else {0};
        for (target,pos) in blades {
            match super::glintblade::spawn(here,&target,pos) {
                Ok(())=>super::log(format!("TOWER_EMBER glintblade spawned target={target}")),
                Err(error)=>super::log_repeating(format!("TOWER_EMBER glintblade: {error}")),
            }
        }
        // 召唤物：只看本机玩家的骨灰
        let local_spirits=super::spirits::local_living_handles(world);
        let spirit=local_spirits.iter().filter_map(|h|world.chr_ins_by_handle(h)).max_by_key(|c|c.modules.data.hp)
            .map(|c|{let q=c.modules.physics.position;(c.field_ins_handle,c.modules.data.hp,c.modules.data.max_hp,[q.0,q.1,q.2])});
        let vanished=self.spirit.filter(|_|spirit.is_none());self.spirit=spirit.map(|(_,_,_,pos)|pos);
        for &(dead,by_spirit) in &deaths {events.gains.push(Gain::Kill{rank:kill_rank(self.foes[dead].rank),spirit:by_spirit});}
        // 处刑者的血宴：致命一击之后不久自己打死的那一只（只算第一只，范围伤害顺带打死的不算）
        if let Some(&(dead,_))=deaths.first()&&self.critical_at.is_some_and(|at|now.duration_since(at)<HARVEST_WINDOW) {
            self.critical_at=None;let rank=kill_rank(self.foes[dead].rank);
            events.gains.push(Gain::CriticalKill{rank});super::log(format!("TOWER_EMBER harvest critical kill rank={rank}"));
        }
        // 灵魂收容：每只本机在场的召唤物挂上当前层数的效果
        if has(Ember::SpiritKeeper)&&!local_spirits.is_empty() {
            let present:Vec<i32>=local_spirits.iter().filter_map(|h|world.chr_ins_by_handle(h))
                .flat_map(|c|c.special_effect.entries().map(|e|e.param_id).collect::<Vec<_>>()).collect();
            let want=scaled_slot(Ember::SpiritKeeper,keeper,&present);
            for handle in &local_spirits {
                if let Some(chr)=world.chr_ins_by_handle_mut(handle) {
                    let own:Vec<_>=chr.special_effect.entries().map(|e|e.param_id).collect();
                    swap(chr,&own,Ember::SpiritKeeper,want);
                }
            }
        }
        // 玩家侧：所有玩家都生效
        let mut heal=0i64;
        // 仿身泪滴的替身：本机存活召唤物（生命 > 1）在场时已由 SPIRIT_LINK 实时减伤 50%，此处把等量伤害分摊扣到召唤物身上（每只至少留 1 点生命）
        let mut spirit_loss=0i32;
        if has(Ember::SpiritLink)&&taken>0&&!local_spirits.is_empty() {
            let spare_hp:i32=local_spirits.iter().filter_map(|h|world.chr_ins_by_handle(h))
                .map(|c|(c.modules.data.hp-1).max(0)).fold(0i32,|acc,v|acc.saturating_add(v));
            let mitigated=world.main_player.as_ref().is_some_and(|p|p.chr_ins.special_effect.entries().any(|e|e.param_id==SPIRIT_LINK));
            let (player_refund,transfer)=spirit_link_transfer(taken,mitigated,spare_hp);
            heal+=i64::from(player_refund);
            spirit_loss=transfer;
            if transfer>0 {super::log(format!("TOWER_EMBER spirit_link taken={taken} mitigated={mitigated} transfer={transfer} spare={spare_hp}"));}
        }
        if has(Ember::Miquella) {
            self.lifesteal+=dealt*i64::from(embers::LIFESTEAL_PERCENT);
            heal+=self.lifesteal/100;self.lifesteal%=100;
        }
        if has(Ember::Frenzy) {
            if taken>0 {self.frenzy=0;} else {self.frenzy=(self.frenzy as usize+hits.len()).min(usize::from(embers::FRENZY_STACKS)) as u8;}
        }
        if has(Ember::Revenge) {
            if !hits.is_empty() {self.revenge_until=None;}
            if taken>0 {self.revenge_until=Some(now+Duration::from_secs(embers::REVENGE_SECONDS));}
        }
        let rise=has(Ember::Rise)&&!self.rose&&i64::from(hp)*100<i64::from(max_hp)*i64::from(embers::RISE_BELOW_PERCENT);
        let dragon=has(Ember::DragonHeart)&&stamina<=0&&max_stamina>0&&self.dragon_ready.is_none_or(|t|now>=t);
        // 殉道者的苦修：掉了多少生命（按最大生命的比例），就按同样的比例回复专注
        let martyr=if has(Ember::Martyr) {taken}else{0};
        let phalanx_proc=if has(Ember::Phalanx)&&phalanx_hits>0&&self.phalanx_ready.is_none_or(|t|now>=t) {
            (0..phalanx_hits).any(|_|next_roll(&mut self.capture_seed,100)<embers::PHALANX_CHANCE_PERCENT)
        } else {false};
        if heal>0||rise||dragon||refund>0||martyr>0||phalanx_hits>0||phalanx_proc||hunter_mark_kills>0 {
            let chr=&mut world.main_player.as_mut().unwrap().chr_ins;
            let data=&mut chr.modules.data;
            if martyr>0 {
                let gain=embers::martyr_fp(martyr,data.max_hp,data.max_fp);
                if gain>0 {data.fp=(i64::from(data.fp.max(0))+i64::from(gain)).min(i64::from(data.max_fp)) as i32;}
            }
            if phalanx_hits>0 {
                let gain=i64::from(embers::phalanx_fp(data.max_fp))*i64::from(phalanx_hits);
                if gain>0 {data.fp=(i64::from(data.fp.max(0))+gain).min(i64::from(data.max_fp)) as i32;}
            }
            if hunter_mark_kills>0 {
                let hp_gain=i64::from(embers::hunter_mark_restore(data.max_hp,embers::HUNTER_MARK_HEAL_PERCENT))*i64::from(hunter_mark_kills);
                let fp_gain=i64::from(embers::hunter_mark_restore(data.max_fp,embers::HUNTER_MARK_FP_PERCENT))*i64::from(hunter_mark_kills);
                if hp_gain>0 {data.hp=(i64::from(data.hp)+hp_gain).min(i64::from(data.max_hp)) as i32;}
                if fp_gain>0 {data.fp=(i64::from(data.fp.max(0))+fp_gain).min(i64::from(data.max_fp)) as i32;}
            }
            if refund>0 {
                data.stamina=(i64::from(data.stamina.max(0))+i64::from(refund)).min(i64::from(data.max_stamina)) as i32;
                self.footwork.last=Some(data.stamina);
                self.arts_stamina.last=Some(data.stamina);
            }
            if rise {
                data.hp=data.hp.max((i64::from(data.max_hp)*i64::from(embers::RISE_TO_PERCENT)/100) as i32);self.rose=true;
                super::log(format!("TOWER_EMBER rise hp={}/{}",data.hp,data.max_hp));
            }
            if heal>0 {data.hp=(i64::from(data.hp)+heal).min(i64::from(data.max_hp)) as i32;}
            if dragon {
                data.stamina=(i64::from(data.stamina.max(0))+i64::from(data.max_stamina)*i64::from(embers::DRAGON_RESTORE_PERCENT)/100).min(i64::from(data.max_stamina)) as i32;
                self.dragon_ready=Some(now+Duration::from_secs(embers::DRAGON_COOLDOWN_SECONDS));
            }
            self.player_hp=Some(data.hp);
            if phalanx_proc&&ready(&[PHALANX_CAST]) {
                chr.apply_speffect(PHALANX_CAST,false);
                self.phalanx_ready=Some(now+Duration::from_secs(embers::PHALANX_COOLDOWN_SECONDS));
                super::log(format!("TOWER_EMBER phalanx summoned hits={phalanx_hits} cd={}s",embers::PHALANX_COOLDOWN_SECONDS));
            }
        }
        // 众灵的盛宴、孤王的契约、替身的分担：都作用在本机召唤物上
        let lone_king=has(Ember::SpiritCommand)&&super::spirits::local_living_group_count(world)==1;
        let lone_king_heal=spirit_regen_due(&mut self.lone_king_regen,lone_king,now);
        if spirit.is_some()&&(spirit_loss>0||has(Ember::SpiritCommand)||has(Ember::SpiritFeast)&&!deaths.is_empty()) {
            let feast_heal=if has(Ember::SpiritFeast) {deaths.len() as i64}else{0};
            let mut order:Vec<_>=local_spirits.iter().copied().collect();
            if spirit_loss>0 {
                order.sort_by_key(|h|std::cmp::Reverse(world.chr_ins_by_handle(h).map_or(0,|c|c.modules.data.hp)));
            }
            let mut pending_loss=i64::from(spirit_loss);
            for handle in &order {
                let Some(chr)=world.chr_ins_by_handle_mut(handle) else{continue};
                let data=&mut chr.modules.data;
                let take=pending_loss.min(i64::from((data.hp-1).max(0)));
                pending_loss-=take;
                let restored=i64::from(data.max_hp)*i64::from(embers::SPIRIT_FEAST_HEAL_PERCENT)/100*feast_heal;
                let regen=if lone_king_heal&&data.max_hp>0 {(i64::from(data.max_hp)*i64::from(embers::SPIRIT_COMMAND_REGEN_PERCENT)/100).max(1)}else{0};
                if take>0||restored>0||regen>0 {
                    data.hp=(i64::from(data.hp)-take+restored+regen).clamp(1,i64::from(data.max_hp)) as i32;
                }
                if has(Ember::SpiritCommand)&&ready(&[SPIRIT_COMMAND]) {
                    let active=chr.special_effect.entries().any(|e|e.param_id==SPIRIT_COMMAND);
                    if lone_king&&!active {chr.apply_speffect(SPIRIT_COMMAND,false);}
                    else if !lone_king&&active {chr.remove_speffect(SPIRIT_COMMAND);}
                }
            }
            if feast_heal>0 {
                let data=&mut world.main_player.as_mut().unwrap().chr_ins.modules.data;
                data.fp=(i64::from(data.fp.max(0))+i64::from(data.max_fp)*i64::from(embers::SPIRIT_FEAST_FP_PERCENT)/100*feast_heal).min(i64::from(data.max_fp)) as i32;
            }
        }
        events.feast=has(Ember::Feast)&&!deaths.is_empty()&&self.feasts<embers::FEAST_PER_FIGHT;
        if events.feast {self.feasts+=1;}
        // 敌人侧：房主与单人直接执行；联机成员算出来交给房主（命中、击倒都只算自己的）
        let mut strikes:Vec<(usize,Strike)>=Vec::new();
        for &index in &captured {if self.foes[index].alive {strikes.push((index,Strike::Capture{slot:self.foes[index].slot}));}}
        for &(index,_) in &hits {
            for ember in [Ember::Bloodlord,Ember::Frost] {if has(ember) {strikes.push((index,Strike::Buildup{slot:self.foes[index].slot,ember}));}}
        }
        for &index in &mark_procs {if self.foes[index].alive {strikes.push((index,Strike::Mark{slot:self.foes[index].slot}));}}
        for &(index,kind) in &venom_procs {if self.foes[index].alive {strikes.push((index,Strike::Venom{slot:self.foes[index].slot,kind}));}}
        let mut wound=|origin:[f32;3],radius:f32,ember:Ember,foes:&[Foe]| {
            for (index,foe) in foes.iter().enumerate() {if foe.alive&&distance(foe.pos,origin)<=radius {strikes.push((index,Strike::Wound{slot:foe.slot,ember}));}}
        };
        if has(Ember::Deathflame) {for &(dead,_) in &deaths {wound(self.foes[dead].pos,embers::DEATHFLAME_RADIUS,Ember::Deathflame,&self.foes);}}
        // 圣杯余响：喝圣杯瓶后一段时间内的增益（挂在玩家身上，由 sync 按 echo_until 挂上 / 摘掉；再喝一口重新计时）
        if has(Ember::Echo)&&drank {self.echo_until=Some(Instant::now()+Duration::from_secs(embers::ECHO_SECONDS));}
        if has(Ember::SpiritBurst) && let Some(origin)=vanished {wound(origin,embers::SPIRIT_BURST_RADIUS,Ember::SpiritBurst,&self.foes);super::log("TOWER_EMBER spirit burst");}
        if has(Ember::Thorns)&&taken>0 {
            let nearest=self.foes.iter().enumerate().filter(|(_,f)|f.alive).min_by(|(_,a),(_,b)|distance(a.pos,here).total_cmp(&distance(b.pos,here)));
            if let Some((index,foe))=nearest {strikes.push((index,Strike::Thorns{slot:foe.slot,taken:taken.min(embers::THORNS_TAKEN_LIMIT)}));}
        }
        if authority {for (index,strike) in strikes {self.strike(world,index,strike);}}
        else {events.strikes=strikes.into_iter().map(|(_,strike)|strike).collect();}
        events
    }
    /// 执行一条敌人侧效果。不由余烬直接击杀（至少留 1 点生命），避免绕过游戏的死亡流程
    fn strike(&mut self,world:&mut WorldChrMan,index:usize,strike:Strike) {
        if !ready(&[BLOOD,FROST,FLASH]) {if !self.missing{super::log("TOWER_EMBER missing ember effects; update regulation.bin");self.missing=true;}return;}
        let foe=&mut self.foes[index];
        let Some(chr)=world.chr_ins_by_handle_mut(&foe.handle) else{return};
        if chr.modules.data.hp<=0||chr.chr_flags1c5.death_flag()||chr.debug_flags.disabled_hit() {return;}
        let max=chr.modules.data.max_hp;
        let damage=match strike {
            Strike::Capture{..}=>{
                // 先压到 1 点再挂一次性扣满的特效，由游戏自己判定死亡；缺特效行时只压到 1 点，下一击就倒
                chr.modules.data.hp=1;foe.hp=1;chr.apply_speffect(FLASH,false);
                if ready(&[CAPTURE]) {chr.apply_speffect(CAPTURE,false);}
                else if !self.missing {super::log("TOWER_EMBER missing capture effect; update regulation.bin");self.missing=true;}
                return;
            },
            Strike::Buildup{ember,..}=>{if let Some(effect)=buildup_effect(ember){apply_buildup(chr,effect);}return;},
            Strike::Mark{..}=>{
                if ready(&[HUNTER_MARK]) {
                    chr.remove_speffect(HUNTER_MARK);
                    chr.apply_speffect(HUNTER_MARK,false);
                    chr.apply_speffect(FLASH,false);
                    super::log(format!("TOWER_EMBER hunter_mark applied slot={}",foe.slot));
                } else if !self.missing {super::log("TOWER_EMBER missing hunter mark effect; update regulation.bin");self.missing=true;}
                return;
            },
            Strike::Venom{kind,..}=>{if let Some(effect)=venom_effect(kind){apply_buildup(chr,effect);}return;},
            Strike::Wound{ember,..}=>embers::enemy_damage(max,embers::wound_percent(ember).unwrap_or(0),foe.boss),
            Strike::Thorns{taken,..}=>embers::thorns_damage(taken,max,foe.boss),
        };
        if damage<=0 {return;}
        chr.modules.data.hp=(chr.modules.data.hp-damage).max(1);foe.hp=chr.modules.data.hp;
        chr.apply_speffect(FLASH,false);
    }
    /// 房主：执行成员发来的敌人侧效果。每个成员按序号只执行一次；成员没带对应的余烬、或找不到那个敌人的跳过
    pub fn apply_remote(&mut self,world:&mut WorldChrMan,foes:&[(FieldInsHandle,u8,u8)],member:u64,held:&[Ember],requests:&[StrikeRequest]) {
        let entry=match self.remote.iter().position(|(m,_)|*m==member) {Some(i)=>i,None=>{self.remote.push((member,0));self.remote.len()-1}};
        let last=self.remote[entry].1;
        let mut fresh:Vec<_>=requests.iter().filter(|r|r.seq>last).copied().collect();
        fresh.sort_by_key(|r|r.seq);
        let Some(newest)=fresh.last().map(|r|r.seq) else{return};
        self.remote[entry].1=newest;
        for request in fresh {
            if !request.strike.valid(held) {super::log(format!("TOWER_EMBER member {member} strike rejected: {:?}",request.strike));continue;}
            let Some(&(handle,rank,slot))=foes.iter().find(|f|f.2==request.strike.slot()) else{continue};
            let index=match self.foes.iter().position(|f|f.handle==handle) {
                Some(index)=>index,
                None=>{
                    let Some(chr)=world.chr_ins_by_handle(&handle) else{continue};
                    let q=chr.modules.physics.position;
                    let alive=chr.modules.data.hp>0&&!chr.chr_flags1c5.death_flag();
                    self.foes.push(Foe{handle,hp:chr.modules.data.hp,max:chr.modules.data.max_hp,pos:[q.0,q.1,q.2],alive,boss:rank<=1,rank,slot,hit_at:None,participation:Default::default(),mark_hits:0,marked_until:None,venom_at:None,recent_bursts:[None;3]});
                    self.foes.len()-1
                },
            };
            self.strike(world,index,request.strike);
        }
    }
}
/// 黑刀刺客的身法：动作开始后 FOOTWORK_ARM 内的第一次精力扣减退还；扣精力可能比识别早一两帧，识别前 FOOTWORK_RECENT 内的扣减也算
const FOOTWORK_ARM:Duration=Duration::from_millis(600);
const FOOTWORK_RECENT:Duration=Duration::from_millis(150);
/// 闪避键按住不到这么久就松开算翻滚，更久是冲刺
const FOOTWORK_TAP:Duration=Duration::from_millis(500);
#[derive(Default)]
struct Footwork {last:Option<i32>,recent:Option<(i32,Instant)>,armed:Option<Instant>,held_since:Option<Instant>,throwing:bool,logged:bool}
impl Footwork {
    /// stamina：本帧精力；trigger：本帧开始翻滚或致命一击。返回要退还的精力（每次动作只退一次）
    fn observe(&mut self,stamina:i32,trigger:bool,now:Instant)->i32 {
        let drop=self.last.map_or(0,|before|(before-stamina).max(0));self.last=Some(stamina);
        if drop>0 {self.recent=Some((drop,now));}
        if trigger {
            if let Some((amount,at))=self.recent.take()&&now.duration_since(at)<=FOOTWORK_RECENT {self.armed=None;return amount;}
            self.armed=Some(now+FOOTWORK_ARM);
        }
        if drop>0&&self.armed.is_some_and(|until|now<until) {self.armed=None;self.recent=None;return drop;}
        0
    }
}
/// 战技的精粹：按下/按住 L2 战技键后的窗口内退还 15% 消耗的精力（带余数累计，不扣正常翻滚/轻攻精力）
const ARTS_STAMINA_WINDOW:Duration=Duration::from_millis(1200);
#[derive(Default)]
struct ArtsStamina {last:Option<i32>,until:Option<Instant>,rem:i64}
impl ArtsStamina {
    fn observe(&mut self,stamina:i32,skill:bool,dodge:bool,already_refunded:bool,now:Instant)->i32 {
        let drop=self.last.map_or(0,|before|(before-stamina).max(0));
        self.last=Some(stamina);
        if dodge {self.until=None;return 0;}
        if skill {self.until=Some(now+ARTS_STAMINA_WINDOW);}
        if already_refunded||drop<=0||!self.until.is_some_and(|u|now<u) {return 0;}
        self.rem+=i64::from(drop)*i64::from(embers::ARTS_STAMINA_CUT_PERCENT);
        let back=(self.rem/100) as i32;
        self.rem%=100;
        back
    }
}
#[test]
fn arts_stamina_refunds_fifteen_percent_only_during_skill_window() {
    let t=Instant::now();let ms=|n:u64|t+Duration::from_millis(n);
    let mut a=ArtsStamina::default();
    assert_eq!(a.observe(100,false,false,false,ms(0)),0);
    assert_eq!(a.observe(80,false,false,false,ms(16)),0,"normal attack without L2 is not refunded");
    assert_eq!(a.observe(80,true,false,false,ms(100)),0,"L2 pressed arms skill window");
    assert_eq!(a.observe(60,false,false,false,ms(200)),3,"20 stamina * 15% = 3");
    a.last=Some(60);
    assert_eq!(a.observe(50,false,false,false,ms(400)),1,"10 stamina * 15% = 1.5 -> 1 (rem 50)");
    a.last=Some(50);
    assert_eq!(a.observe(40,false,false,false,ms(500)),2,"10 stamina * 15% + rem 50 = 2");
    a.last=Some(40);
    assert_eq!(a.observe(20,false,true,false,ms(600)),0,"dodge clears skill window");
    a.last=Some(20);
    assert_eq!(a.observe(10,false,false,false,ms(700)),0,"stamina after dodge is not refunded");
    assert_eq!(a.observe(100,true,false,false,ms(1000)),0);
    assert_eq!(a.observe(80,false,false,false,ms(2300)),0,"expired window");
}
#[test]
fn footwork_refunds_one_drop_per_dodge_or_critical() {
    let t=Instant::now();let ms=|n:u64|t+Duration::from_millis(n);
    let mut f=Footwork::default();
    assert_eq!(f.observe(100,false,ms(0)),0);
    assert_eq!(f.observe(100,true,ms(16)),0,"dodge pressed, nothing spent yet");
    assert_eq!(f.observe(80,false,ms(33)),20,"the roll's cost comes back");
    f.last=Some(100);
    assert_eq!(f.observe(70,false,ms(50)),0,"the next attack is paid");
    f.last=Some(70);
    assert_eq!(f.observe(50,false,ms(400)),0,"no dodge, no refund");
    assert_eq!(f.observe(50,true,ms(480)),20,"spent a frame before the critical was seen");
    f.last=Some(70);
    assert_eq!(f.observe(70,true,ms(1000)),0);
    assert_eq!(f.observe(60,false,ms(1700)),0,"armed window expired");
}
#[derive(Default)]
struct CriticalEdge {counted:bool,clear_frames:u8}
impl CriticalEdge {
    fn observe(&mut self,marked:bool,throwing:bool)->bool {
        if marked||throwing {self.clear_frames=0;}else{self.clear_frames=self.clear_frames.saturating_add(1);if self.clear_frames>=2{self.counted=false;}}
        if marked&&!self.counted {self.counted=true;true}else{false}
    }
}
#[test]
fn critical_marker_counts_once_per_execution_not_per_damage_stage() {
    let mut edge=CriticalEdge::default();
    assert!(!edge.observe(false,false));assert!(!edge.observe(false,true));
    assert!(edge.observe(true,true));assert!(!edge.observe(true,true));
    assert!(!edge.observe(false,true));assert!(!edge.observe(true,true));
    assert!(!edge.observe(true,false));assert!(!edge.observe(false,false));assert!(!edge.observe(false,false));
    assert!(edge.observe(true,true));
}
#[test]
fn critical_listener_rejects_other_players_and_noncritical_effects() {
    assert!(own_critical(123,123,350501));
    assert!(!own_critical(123,456,350501));assert!(!own_critical(0,0,350501));
    for effect in [350500,350502,9670007,0,-1]{assert!(!own_critical(123,123,effect));}
}

#[cfg(test)]
mod tests {
    #[test]
    fn stack_updates_preserve_spell_coverage_and_critical_restrictions() {
        for ember in [Ember::SoulEater,Ember::Godslayer] {
            let mut row:SP_EFFECT_PARAM_ST=unsafe{std::mem::zeroed()};
            row.set_mag_param_change(true);row.set_miracle_param_change(true);
            for level in [1,10,100] {
                let rates=embers::stack_rates(ember,level).unwrap();
                write(&mut row,ember,rates);
                assert!(row.mag_param_change()&&row.miracle_param_change());
                assert_eq!([row.physics_attack_rate(),row.magic_attack_rate(),row.fire_attack_rate(),row.thunder_attack_rate(),row.dark_attack_rate()],[rates.attack;5]);
            }
        }
        let mut chain:SP_EFFECT_PARAM_ST=unsafe{std::mem::zeroed()};
        chain.set_state_info(367);chain.set_throw_attack_param_change(true);
        write(&mut chain,Ember::Chain,embers::stack_rates(Ember::Chain,10).unwrap());
        assert!(!chain.mag_param_change()&&!chain.miracle_param_change());
        assert_eq!(chain.state_info(),367);assert!(chain.throw_attack_param_change());
    }
    #[test]
    fn participation_survives_teammate_hits_and_credits_death_once() {
        let mut credit=super::Participation::default();
        assert!(!credit.observe(true,1,false));
        assert!(!credit.observe(false,500,false));
        assert!(credit.observe(false,100,true));
        assert!(!credit.observe(false,0,true));
        let mut spectator=super::Participation::default();
        assert!(!spectator.observe(true,0,false));
        assert!(!spectator.observe(true,-100,false));
        assert!(!spectator.observe(false,100,true));
        let mut last_hit=super::Participation::default();
        assert!(last_hit.observe(true,1,true));
        assert!(!last_hit.observe(true,1,true));
    }
    #[test]
    fn bulwark_requires_the_actual_guard_weapon_to_be_a_greatshield() {
        use eldenring::cs::{ChrAsmArmStyle as Style,ChrAsmEquipment,ChrAsmEquipmentSlots,ChrAsmHand as Hand};
        let mut equipment=ChrAsmEquipment{arm_style:Style::OneHanded,selected_slots:ChrAsmEquipmentSlots{
            left_weapon_slot:0,right_weapon_slot:0,left_arrow_slot:0,right_arrow_slot:0,left_bolt_slot:0,right_bolt_slot:0}};
        let mut ids=[-1;22];ids[0]=32000039;ids[1]=1000000;ids[2]=31000000;
        assert!(super::bulwark_guard_weapon(&equipment,&ids,Hand::Left));
        assert!(!super::bulwark_guard_weapon(&equipment,&ids,Hand::Right));
        equipment.arm_style=Style::RightBothHands;
        assert!(!super::bulwark_guard_weapon(&equipment,&ids,Hand::Left),"greatshield on the back is not the guarding weapon");
        equipment.arm_style=Style::LeftBothHands;
        assert!(super::bulwark_guard_weapon(&equipment,&ids,Hand::Right),"two-handing the left greatshield");
        equipment.arm_style=Style::OneHanded;equipment.selected_slots.left_weapon_slot=1;
        assert!(!super::bulwark_guard_weapon(&equipment,&ids,Hand::Left),"switching to a medium shield removes the effect");
        equipment.arm_style=Style::RightBothHands;ids[1]=32000099;
        assert!(super::bulwark_guard_weapon(&equipment,&ids,Hand::Right));
        ids[1]=-1;
        assert!(!super::bulwark_guard_weapon(&equipment,&ids,Hand::Right));
        equipment.arm_style=Style::EmptyHanded;ids[1]=32000000;
        assert!(!super::bulwark_guard_weapon(&equipment,&ids,Hand::Right));
        equipment.arm_style=Style::RightBothHands;
        for id in [30000000,31000000,1000000,33000000] {
            ids[1]=id;assert!(!super::bulwark_guard_weapon(&equipment,&ids,Hand::Right));
        }
        let affixed=erdtree_tower::affixes::CATALOG.iter()
            .find(|i|erdtree_tower::affixes::base(i.id)/1_000_000==32).expect("affixed greatshield");
        ids[1]=(affixed.id+39) as i32;
        assert!(super::bulwark_guard_weapon(&equipment,&ids,Hand::Right));
    }
    use super::*;
    #[test]
    fn mercy_raises_heals_by_half_and_keeps_the_sign() {
        assert_eq!((mercy_heal(-230),mercy_heal(-8),mercy_heal(-115)),(-345,-12,-172));
        assert!(MERCY_HEALS.iter().all(|id|(1642000..1700000).contains(id)||(21640000..21670000).contains(id)));
    }
    #[test]
    fn unattributed_ranged_hits_follow_the_bullet_owner() {
        let now=Instant::now();let foe=[20.0,0.0,0.0];
        assert_eq!(bullet_owner_is_mine(foe,&[(now,[19.0,0.0,0.0],BulletOwner::Player),(now,[30.0,0.0,0.0],BulletOwner::Teammate)]),Some(true),"my spell landed on it");
        assert_eq!(bullet_owner_is_mine(foe,&[(now,[18.0,0.0,0.0],BulletOwner::Spirit),(now,[20.5,0.0,0.0],BulletOwner::Teammate)]),Some(false),"the teammate's bolt is closer");
        assert_eq!(bullet_owner_is_mine(foe,&[(now,[0.0,0.0,0.0],BulletOwner::Player)]),None,"no bullet near it, fall back to distance");
    }
    #[test]
    fn spirit_keeper_only_credits_local_spirit_kills() {
        let foe=[0.0,0.0,0.0];
        assert!(killed_by_spirit(false,true,None,foe,&[],&[[0.5,0.0,0.0]]),"direct spirit kill via last_hit_by");
        assert!(!killed_by_spirit(false,false,Some(BulletOwner::Spirit),foe,&[[0.1,0.0,0.0]],&[[5.0,0.0,0.0]]),"player last_hit_by never counts even if spirit is standing next to foe");
        assert!(killed_by_spirit(true,false,Some(BulletOwner::Spirit),foe,&[[10.0,0.0,0.0]],&[[0.5,0.0,0.0]]),"ranged spirit bullet kill when last_hit_by is empty");
        assert!(!killed_by_spirit(true,false,Some(BulletOwner::Player),foe,&[[0.5,0.0,0.0]],&[[10.0,0.0,0.0]]),"player bullet kill does not count");
        assert!(killed_by_spirit(true,false,None,foe,&[[1.0,0.0,0.0]],&[[4.0,0.0,0.0]]),"closest local spirit fallback");
        assert!(!killed_by_spirit(true,false,None,foe,&[[4.0,0.0,0.0]],&[[1.0,0.0,0.0]]),"player closer than spirit does not count");
        assert!(!killed_by_spirit(true,false,None,foe,&[],&[[1.0,0.0,0.0]]),"no spirit alive");
    }
    #[test]
    fn unattributed_hits_go_to_the_nearest_side() {
        let foe=[0.0,0.0,0.0];
        assert!(nearest_is_mine(foe,&[[1.0,0.0,0.0]],&[[3.0,0.0,0.0]]),"melee next to the enemy");
        assert!(!nearest_is_mine(foe,&[[5.0,0.0,0.0]],&[[2.0,0.0,0.0]]),"a teammate is closer");
        assert!(nearest_is_mine(foe,&[[9.0,0.0,0.0],[0.5,0.0,0.0]],&[[2.0,0.0,0.0]]),"own spirit is closest");
        assert!(nearest_is_mine(foe,&[[40.0,0.0,0.0]],&[]),"alone in the arena");
        assert!(!nearest_is_mine(foe,&[],&[]));
    }
    #[test]
    fn last_stand_turns_on_below_forty_percent() {
        assert!(last_stand(399,1000)&&!last_stand(400,1000)&&!last_stand(0,1000)&&!last_stand(10,0));
    }
    #[test]
    fn stance_break_preserves_broken_and_unsupported_states_and_boss_classification() {
        assert_eq!(break_value(110.0,110.0,false),Some(BREAK_TOUGHNESS));
        for (current,max,broken) in [(0.0,0.0,false),(0.0,110.0,false),(-1.0,110.0,false),(110.0,110.0,true),(f32::NAN,110.0,false)] {
            assert_eq!(break_value(current,max,broken),None);
        }
        assert_eq!([0,1,2,3].map(breaker_boss),[true,true,false,false]);
    }
    #[test]
    fn resonance_tracks_living_summons_in_camp_before_combat_and_after_combat() {
        use erdtree_tower::{Run,Class,Phase};
        let mut run=Run::new(42,Class::Vagabond);run.embers.push(Ember::SpiritResonance);
        for phase in [Phase::Map,Phase::Node{id:0},Phase::Combat{id:0}] {
            run.phase=phase;
            assert!(resonance_active(&held(Some(&run)),true));
            assert!(!resonance_active(&held(Some(&run)),false));
        }
        assert!(!resonance_active(&held(None),true));
        assert!(!resonance_active(&[],true));
    }
    #[test]
    fn practice_buildup_uses_only_the_two_combat_effects() {
        let effects:Vec<_>=Ember::ALL.into_iter().filter_map(buildup_effect).collect();
        assert_eq!(effects,vec![BLOOD,FROST]);
        assert!(buildup_hit(15,5000,None));
        assert!(!buildup_hit(0,5000,None));
        assert!(!buildup_hit(14,5000,None));
        assert!(!buildup_hit(100,5000,Some(Duration::from_millis(299))));
        assert!(buildup_hit(100,5000,Some(Duration::from_millis(300))));
    }
    #[test]
    fn practice_keeps_conditional_bonuses_without_battle_rewards() {
        let held=practice_held(&Ember::ALL);
        for e in [Ember::FullMoon,Ember::Frenzy,Ember::Echo,Ember::Revenge,Ember::Barrier,Ember::Meditation,Ember::Footwork,Ember::Bloodlord,Ember::Frost,Ember::SpiritKeeper] {assert!(held.contains(&e));}
        for e in [Ember::Rise,Ember::Feast,Ember::SpiritFeast,Ember::Harvest,Ember::Chain,Ember::SoulEater,Ember::Godslayer] {assert!(!held.contains(&e));}
    }
    #[test]
    fn buildup_target_allows_direct_application_without_changing_strength() {
        let mut row:SP_EFFECT_PARAM_ST=unsafe{std::mem::zeroed()};
        row.set_effect_target_oppose_target(true);
        row.set_blood_attack_power(40);row.set_freeze_attack_power(40);
        assert!(!row.effect_target_self_target());
        buildup_target(&mut row);
        assert!(row.effect_target_self_target()&&row.effect_target_oppose_target());
        assert!(!row.effect_target_friendly_target());
        assert_eq!((row.blood_attack_power(),row.freeze_attack_power()),(40,40));
        buildup_target(&mut row);
        assert!(row.effect_target_self_target());
    }
    #[test]
    fn breaker_counts_only_burst_ailments() {
        for (id,kind) in [(6400,Some("出血")),(6416,Some("出血")),(6450,Some("睡眠")),(6701,Some("冻伤")),(6754,Some("发狂"))] {assert_eq!(burst_kind(id),kind,"{id}");}
        for id in [6399,6501,6601,6801,6850,9670010,9670011] {assert_eq!(burst_kind(id),None,"poison, rot, blight, death and our own buildup are not bursts: {id}");}
    }
    #[test]
    fn effect_ids_do_not_overlap() {
        let ids:Vec<_>=player_ids().collect();
        assert_eq!(ids.len(),14+usize::from(embers::FRENZY_STACKS));
        assert!([SPIRIT_RESONANCE,SPIRIT_LINK,ARTS_ATTACK,ARTS_COST,BULWARK].into_iter().all(|id|ids.contains(&id)));
        assert!(!ids.contains(&EXECUTION_RELIEF),"the relief expires on its own, sync never strips it");
        assert_eq!(frenzy_effect(embers::FRENZY_STACKS),FIRST+131);
        let mut all=ids.clone();all.extend([BLOOD,FROST,POISON,ROT,FLASH,PHALANX_CAST,HUNTER_MARK,SPIRIT_COMMAND,CHAIN_MARK,CHAIN_RESIDENT+1,EXECUTION+1,EXECUTION_RELIEF]);
        all.extend(Ember::ALL.into_iter().filter_map(slots).flatten());
        let count=all.len();all.sort();all.dedup();assert_eq!(all.len(),count,"effect ids overlap");
        assert!(all.iter().all(|id|(FIRST..FIRST+200).contains(id)),"inside the ember range");
        assert!(Ember::ALL.into_iter().all(|e|(e.stacking()||matches!(e,Ember::Battlemage|Ember::Mind|Ember::HeavyCast|Ember::VenomFletch))==slots(e).is_some()));
        assert!(PLAYER_STACKING.iter().all(|e|e.stacking()&&*e!=Ember::SpiritKeeper));
        assert_eq!([0,1,2,3].map(kill_rank),[2,1,1,0]);
    }
    #[test]
    fn phalanx_only_triggers_on_close_non_bullet_normal_attacks() {
        let now=Instant::now();
        let mut window=None;
        observe_normal_attack(&mut window,now,true,false);
        assert!(window.is_some_and(|u|u>now));
        observe_normal_attack(&mut window,now,true,true);
        assert!(window.is_none(),"spell/AoW/heavy input clears light-attack window");
        assert!(is_normal_melee_hit(true,false,false,2.5));
        assert!(!is_normal_melee_hit(false,false,false,2.5));
        assert!(!is_normal_melee_hit(true,true,false,2.5),"spirit hits do not trigger phalanx");
        assert!(!is_normal_melee_hit(true,false,true,2.5),"player bullet hits do not loop phalanx");
        assert!(!is_normal_melee_hit(true,false,false,MELEE_REACH+0.1));
    }
    #[test]
    fn lone_king_regen_ticks_every_second_and_resets_when_inactive() {
        let start=Instant::now();
        let mut last=None;
        assert!(!spirit_regen_due(&mut last,false,start));
        assert!(last.is_none());
        assert!(!spirit_regen_due(&mut last,true,start),"first active frame arms timer");
        assert!(!spirit_regen_due(&mut last,true,start+Duration::from_millis(999)));
        assert!(spirit_regen_due(&mut last,true,start+Duration::from_secs(1)));
        assert!(!spirit_regen_due(&mut last,true,start+Duration::from_millis(1500)));
        assert!(!spirit_regen_due(&mut last,false,start+Duration::from_secs(2)),"second group clears timer");
        assert!(last.is_none());
    }
    #[test]
    fn heavy_cast_latch_keeps_high_fp_bonus_briefly_when_swapping_spells() {
        let now=Instant::now();
        let latch=Some((57,now+HEAVY_CAST_LATCH));
        assert_eq!(heavy_cast_effective_mp(7,latch,now+Duration::from_secs(1)),57);
        assert_eq!(heavy_cast_effective_mp(62,latch,now+Duration::from_secs(1)),62);
        assert_eq!(heavy_cast_effective_mp(7,latch,now+HEAVY_CAST_LATCH),7);
        assert_eq!(heavy_cast_effective_mp(24,None,now),24);
    }
    #[test]
    fn bow_stance_and_hit_classification_require_active_bow_or_crossbow() {
        use eldenring::cs::{ChrAsmArmStyle as Style,ChrAsmEquipment,ChrAsmEquipmentSlots};
        let mut eq=ChrAsmEquipment{arm_style:Style::OneHanded,selected_slots:ChrAsmEquipmentSlots{
            left_weapon_slot:0,right_weapon_slot:0,left_arrow_slot:0,right_arrow_slot:0,left_bolt_slot:0,right_bolt_slot:0}};
        let mut ids=[-1;22];
        let lookup=|id:u32|match id {40000000=>Some(50),41000000=>Some(51),42000000=>Some(53),43000000=>Some(55),44000000=>Some(56),9000000=>Some(15),_=>None};
        // Two-handing a bow: exclusive bow stance (point-blank hits count even without bullet)
        ids[1]=41000000;eq.arm_style=Style::RightBothHands;
        assert_eq!(bow_stance_from(&eq,&ids,lookup),(true,true));
        assert!(is_bow_hit(true,true,false,false));
        assert!(!is_bow_hit(true,true,true,true),"spirit hits never count as bow hits");
        // Bow on back while two-handing katana: not holding bow
        ids[0]=41000000;ids[1]=9000000;eq.arm_style=Style::RightBothHands;
        assert_eq!(bow_stance_from(&eq,&ids,lookup),(false,false));
        // Crossbow in left + katana in right (one-handed): requires player bullet near target
        ids[0]=43000000;ids[1]=9000000;eq.arm_style=Style::OneHanded;
        assert_eq!(bow_stance_from(&eq,&ids,lookup),(true,false));
        assert!(!is_bow_hit(false,true,false,false),"melee katana swing while off-handing crossbow must not trigger");
        assert!(is_bow_hit(false,true,false,true),"crossbow bolt with bullet near target triggers");
        // All 5 bow/crossbow/ballista weapon types recognized
        for t in [50u16,51,53,55,56] {assert!(is_bow_weapon_type(t));}
        for t in [1u16,9,15,57] {assert!(!is_bow_weapon_type(t));}
    }
    #[test]
    fn venom_ailments_count_distinct_persistent_and_recent_burst_statuses() {
        let now=Instant::now();
        let mut bursts=[None;3];
        assert_eq!(observe_ailments([],&mut bursts,now),0);
        // Poison + Rot active
        assert_eq!(observe_ailments([2,5,2],&mut bursts,now),2,"duplicate stateInfo only counts once");
        // Hemorrhage bursts, then disappears next frame but stays remembered for 10s
        assert_eq!(observe_ailments([2,5,6],&mut bursts,now),3);
        assert_eq!(observe_ailments([2,5],&mut bursts,now+Duration::from_secs(5)),3);
        // Add Frostbite -> reaches cap 4
        assert_eq!(observe_ailments([2,5,260,437],&mut bursts,now+Duration::from_secs(6)),4);
        // After burst window expires, only persistent Poison+Rot remain
        assert_eq!(observe_ailments([2,5],&mut bursts,now+Duration::from_secs(17)),2);
        assert_eq!([0,1,2,3].map(venom_effect),[Some(POISON),Some(ROT),Some(BLOOD),Some(FROST)]);
        assert_eq!(venom_effect(4),None);
    }
    #[test]
    fn spirit_link_transfers_mitigated_half_without_double_healing() {
        assert_eq!(spirit_link_transfer(500,true,2000),(0,500),"when SPIRIT_LINK already cut 1000->500, transfer 500 to spirit with 0 extra player heal");
        assert_eq!(spirit_link_transfer(500,true,120),(0,120),"capped by spirit spare HP");
        assert_eq!(spirit_link_transfer(1000,false,2000),(500,500),"fallback path without SpEffect refunds 500 and transfers 500");
        assert_eq!(spirit_link_transfer(500,true,0),(0,0));
    }
    #[test]
    fn battlemage_base_correct_targets_vanilla_max_s_scaling() {
        // Standard +25 / Somber +10 (rate 1.80) -> base ~102.78, reaching 185.0 at vanilla max
        let std_base=battlemage_base_correct(1.80);
        assert!((std_base*1.80-185.0).abs()<0.01);
        assert!(std_base>100.0&&std_base<105.0);
        // Magic +25 (rate 2.35) -> base ~78.72, reaching 185.0 at +25
        let magic_base=battlemage_base_correct(2.35);
        assert!((magic_base*2.35-185.0).abs()<0.01);
        // Unreinforceable or rate <= 1.0 falls back to 185.0 without dividing by zero
        assert_eq!(battlemage_base_correct(1.0),185.0);
        assert_eq!(battlemage_base_correct(0.0),185.0);
    }
}
