use std::{sync::{Mutex,atomic::{AtomicBool,Ordering}},time::Instant};
use eldenring::cs::{GestureParam,SoloParamRepository,WorldChrMan};
use fromsoftware_shared::FromStatic;

static ENABLED:AtomicBool=AtomicBool::new(false);
static STATE:Mutex<Trace>=Mutex::new(Trace{active:None,recent:Vec::new(),enabled:false,pending:None});
struct Active {owner:u64,gesture:u32,anim:i32,started:Instant,last_time:f32,length:f32}
struct Trace {active:Option<Active>,recent:Vec<String>,enabled:bool,pending:Option<(i32,Instant)>}
pub fn requested(anim:i32) {
    if let Ok(mut state)=STATE.lock(){state.pending=Some((anim,Instant::now()));}
    crate::log(format!("DANCE requested anim={anim}"));
}
fn gesture_family(anim:i32)->Option<i32> {(anim>=0).then(||(anim%1_000_000)/10*10)}
fn restarted(previous:f32,current:f32)->bool {previous.is_finite()&&current.is_finite()&&current+0.1<previous}
impl Trace {
    fn end(&mut self,reason:&str) {
        let Some(a)=self.active.take() else{return;};
        let line=format!("GESTURE_END gesture={} animation={} length={:.3}s observed={:.3}s last_play_time={:.3}s reason={reason}",a.gesture,a.anim,a.length,a.started.elapsed().as_secs_f32(),a.last_time);
        crate::log(&line);self.recent.push(line);
        if self.recent.len()>16{self.recent.remove(0);}
    }
}
pub fn tick(world:&WorldChrMan) {
    let enabled=crate::dance_ui::dancing()||(ENABLED.load(Ordering::Relaxed)&&crate::debug_menu::enabled());
    let Ok(mut state)=STATE.try_lock() else{return;};
    if let Some((expected,at))=state.pending {
        let actual=world.main_player.as_ref().and_then(|p|{let t=&p.chr_ins.modules.time_act;t.anim_queue.get(t.read_idx as usize).map(|a|a.anim_id)});
        if actual.and_then(gesture_family)==gesture_family(expected) {
            crate::log(format!("DANCE confirmed requested={expected} actual={actual:?}"));state.pending=None;
        } else if at.elapsed().as_secs_f32()>=1.0 {
            crate::log(format!("DANCE unconfirmed requested={expected} actual={actual:?}"));state.pending=None;
        }
    }
    if enabled!=state.enabled{state.end("capture_toggle");state.enabled=enabled;crate::log(format!("GESTURE_CAPTURE enabled={enabled}; gesture families only"));}
    if !enabled{return;}
    let Some(p)=world.main_player.as_ref() else{state.end("player_unloaded");return;};
    if p.chr_ins.modules.data.hp<=0||p.chr_ins.chr_flags1c5.death_flag(){state.end("player_dead");return;}
    let time=&p.chr_ins.modules.time_act;
    let Some(anim)=time.anim_queue.get(time.read_idx as usize) else{state.end("animation_unavailable");return;};
    let Some(family)=gesture_family(anim.anim_id) else{state.end("gesture_left");return;};
    let Ok(params)=(unsafe {SoloParamRepository::instance()}) else{state.end("params_unavailable");return;};
    let requested=p.chr_ins.modules.action_request.requested_gesture;
    let gesture=params.rows::<GestureParam>().filter(|(_,g)|g.msg_anim_id()==family)
        .min_by_key(|(id,_)|if *id as i32==requested{0}else{1}).map(|(id,_)|id);
    let Some(gesture)=gesture else{state.end("gesture_left");return;};
    if !anim.play_time.is_finite()||!anim.anim_length.is_finite(){state.end("invalid_timing");return;}
    let h=p.chr_ins.field_ins_handle;let owner=(u64::from(h.block_id.0 as u32)<<32)|u64::from(h.selector.0);
    let changed=state.active.as_ref().is_some_and(|a|a.owner!=owner||a.anim!=anim.anim_id||a.gesture!=gesture||restarted(a.last_time,anim.play_time));
    if changed{state.end("animation_change_or_loop");}
    if state.active.is_none(){
        crate::log(format!("GESTURE_START gesture={gesture} animation={} length={:.3}s play_time={:.3}s",anim.anim_id,anim.anim_length,anim.play_time));
        state.active=Some(Active{owner,gesture,anim:anim.anim_id,started:Instant::now(),last_time:anim.play_time,length:anim.anim_length});
    }
    if let Some(a)=state.active.as_mut(){a.last_time=anim.play_time;a.length=anim.anim_length;}
}
pub fn draw(ui:&hudhook::imgui::Ui) {
    if !ui.collapsing_header("姿势采集",hudhook::imgui::TreeNodeFlags::empty()){return;}
    let mut on=ENABLED.load(Ordering::Relaxed);
    if ui.checkbox("采集本机手势／姿势",&mut on){ENABLED.store(on,Ordering::Relaxed);}
    ui.text_wrapped("开启后关闭调试菜单，从原版手势菜单做几个姿势即可。仅记录姿势；编号与时长写入 tower-preview.log。实际播放时长包含提前取消，不等于完整动画长度。");
    if let Ok(state)=STATE.try_lock(){
        if let Some(a)=&state.active{ui.text(format!("当前姿势 {} · 动画 {} · 长度 {:.2} 秒",a.gesture,a.anim,a.length));}
        for line in state.recent.iter().rev().take(4){ui.text_wrapped(line);}
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn family_keeps_gesture_phases_and_does_not_map_attacks_to_gestures() {
        assert_eq!(super::gesture_family(80971),Some(80970));
        assert_eq!(super::gesture_family(10080970),Some(80970));
        assert_eq!(super::gesture_family(-1),None);
        assert_ne!(super::gesture_family(3000),Some(80970));
        assert!(super::restarted(2.0,0.0));assert!(!super::restarted(2.0,1.99));
    }
}
