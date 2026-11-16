// 处决时只有对面不无敌：原版的背刺、弹反处决等投技动画里，攻击方挂着“投技中攻击方无敌”（TAE 动作 68），
// 被处决的一方挂着“除投技外无敌”（TAE 动作 67），投技过程中双方都不吃其他伤害。
// 默认保留原版标志。仅显式设置 ERDTREE_TOWER_THROW_PRESERVE=0 时清敌人标志，供诊断对照：
// 你（以及队友、召唤物）处决时照旧无敌，被处决的敌人会吃到召唤物、队友和其他攻击的伤害；
// 敌人抓投你时，抓你的敌人不再无敌，你照旧无敌。
// 每帧在 ChrIns_PostPhysics（动画事件在 HavokBehavior 里已经设好、伤害在之后的 DmgMan 里结算）清除。
// 只清投技的这两个，翻滚无敌、完全无敌（TAE 94）等其他无敌不动。
use eldenring::cs::{ChrIns,WorldChrMan};
use std::{sync::{LazyLock,Mutex},time::Instant};

fn enabled(value:Option<&str>)->bool {value==Some("1")}
fn preserve_native(value:Option<&str>)->bool {value!=Some("0")}
static CONFIG:LazyLock<(bool,bool)>=LazyLock::new(||{
    let trace=enabled(std::env::var("ERDTREE_TOWER_THROW_TRACE").ok().as_deref());
    let preserve=preserve_native(std::env::var("ERDTREE_TOWER_THROW_PRESERVE").ok().as_deref());
    super::log(format!("THROW_TRACE config trace={trace} preserve_native_invincibility={preserve}"));
    (trace,preserve)
});
const TRACE_LIMIT:usize=256;
#[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
struct Sample {hp:i32,max_hp:i32,state:u32,dead:bool,death_transition:bool,attacker_guard:bool,defender_guard:bool,animation:Option<i32>}
#[derive(Default)]
struct Trace {last:Option<Sample>,lines:usize}
impl Trace {
    fn record(&mut self,sample:Sample)->bool {
        if self.lines>=TRACE_LIMIT||self.last==Some(sample){return false;}
        self.last=Some(sample);self.lines+=1;true
    }
}
struct ActorTrace {handle:eldenring::cs::FieldInsHandle,address:usize,started:Instant,trace:Trace}
static TRACES:Mutex<Vec<ActorTrace>>=Mutex::new(Vec::new());
fn observe(chr:&ChrIns) {
    let flags=&chr.modules.action_flag.action_modifiers_flags;
    let time=&chr.modules.time_act;
    let sample=Sample{hp:chr.modules.data.hp,max_hp:chr.modules.data.max_hp,
        state:unsafe {std::ptr::addr_of!(chr.modules.throw.throw_node.throw_state).cast::<u32>().read()},
        dead:chr.chr_flags1c5.death_flag(),death_transition:chr.modules.throw.flags.death_transition(),
        attacker_guard:flags.invincible_during_throw_attacker(),defender_guard:flags.invincible_excluding_throw_attacks_defender(),
        animation:time.anim_queue.get(time.read_idx as usize).map(|a|a.anim_id)};
    let Ok(mut traces)=TRACES.lock() else{return;};
    let address=std::ptr::from_ref(chr) as usize;
    let index=match traces.iter().position(|t|t.handle==chr.field_ins_handle&&t.address==address) {
        Some(index)=>index,
        None=>{
            if traces.len()>=32||!(matches!(sample.state,3..=6)||sample.attacker_guard||sample.defender_guard){return;}
            traces.push(ActorTrace{handle:chr.field_ins_handle,address,started:Instant::now(),trace:Trace::default()});
            traces.len()-1
        }
    };
    let trace=&mut traces[index];
    if trace.trace.record(sample) {
        super::log(format!("THROW_TRACE entity={} model={} handle={} address={address:x} elapsed_ms={} before_clear={sample:?}",
            chr.event_entity_id,chr.npc_id,chr.field_ins_handle,trace.started.elapsed().as_millis()));
        if trace.trace.lines==TRACE_LIMIT {super::log(format!("THROW_TRACE entity={} limit reached",chr.event_entity_id));}
    }
}

/// 清掉一个角色身上的投技无敌，返回清之前是否挂着
fn clear(chr:&mut ChrIns)->bool {
    let flags=&mut chr.modules.action_flag.action_modifiers_flags;
    let had=flags.invincible_during_throw_attacker()||flags.invincible_excluding_throw_attacks_defender();
    flags.set_invincible_during_throw_attacker(false);
    flags.set_invincible_excluding_throw_attacks_defender(false);
    had
}

/// active：旅程进行中（大赐福、标题等其他时候保持原版）
pub fn sync(world:&mut WorldChrMan,active:bool) {
    if !active {if let Ok(mut traces)=TRACES.lock(){traces.clear();}return;}
    let (trace,preserve)=*CONFIG;
    // 我方：本机玩家、所有玩家角色（联机队友）、召唤物（骨灰、灵马）——保留无敌
    let mut ours:Vec<usize>=world.player_chr_set.characters().map(|p|std::ptr::from_ref(&p.chr_ins) as usize).collect();
    ours.extend(world.main_player.as_ref().map(|p|std::ptr::from_ref(&p.chr_ins) as usize));
    ours.extend(world.summon_buddy_chr_set.characters().map(|c|std::ptr::from_ref(&*c) as usize));
    let mut cleared=0usize;
    for chr in world.chr_sets.iter().flatten().flat_map(|set|set.characters()) {
        if ours.contains(&(std::ptr::from_ref(&*chr) as usize)) {continue;}
        if trace {observe(chr);}
        if !preserve {cleared+=usize::from(clear(chr));}
    }
    if cleared>0 {log_once();}
}
fn log_once() {
    use std::sync::atomic::{AtomicBool,Ordering};
    static LOGGED:AtomicBool=AtomicBool::new(false);
    if !LOGGED.swap(true,Ordering::Relaxed) {super::log("TOWER_THROW enemy throw invincibility cleared");}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trace_keeps_lethal_and_recovery_transitions_and_bounds_output() {
        let mut trace=Trace::default();
        let hit=Sample{hp:100,state:4,..Default::default()};
        assert!(trace.record(hit));
        assert!(!trace.record(hit));
        assert!(trace.record(Sample{hp:0,..hit}));
        assert!(trace.record(Sample{hp:0,state:1,..hit}));
        assert!(trace.record(Sample{hp:0,state:6,dead:true,..hit}));
        for hp in 0..1000 {trace.record(Sample{hp,..hit});}
        assert_eq!(trace.lines,TRACE_LIMIT);
        assert!(!trace.record(hit));
    }
    #[test]
    fn only_explicit_switches_change_behavior() {
        assert!(!enabled(None));assert!(!enabled(Some("0")));assert!(!enabled(Some("true")));
        assert!(enabled(Some("1")));
        assert!(preserve_native(None));assert!(preserve_native(Some("1")));assert!(preserve_native(Some("invalid")));
        assert!(!preserve_native(Some("0")));
    }
}
