use eldenring::cs::{FieldInsHandle,WorldChrMan};
use fromsoftware_shared::FromStatic;
use std::{sync::{Mutex,atomic::{AtomicU32,Ordering}},time::Instant};

pub const HIP_HOP:i32=80720;
static SPEED:AtomicU32=AtomicU32::new(1.0f32.to_bits());
pub fn set_tempo(beat:f64){
    let duration=std::fs::read_to_string(crate::LOG_PATH.with_file_name("scene-assets").join("dance/hip-hop-motion.txt")).ok()
        .and_then(|s|s.lines().find_map(|l|l.strip_prefix("duration=").and_then(|v|v.parse::<f64>().ok())))
        .filter(|d|d.is_finite()&&*d>0.0&&*d<3600.0).unwrap_or(212.0/30.0);
    let speed=loop_speed(beat,duration);SPEED.store(speed.to_bits(),Ordering::Relaxed);
    crate::log(format!("DANCE tempo source_seconds={duration:.3} speed={speed:.4}"));
}
fn loop_speed(beat:f64,duration:f64)->f32 {
    let bars=(duration/(16.0*beat)).round().max(1.0);
    (duration/(bars*16.0*beat)).clamp(0.25,2.0) as f32
}
pub fn available()->bool {crate::LOG_PATH.with_file_name("scene-assets").join("chr/c0000_a00_hi.anibnd.dcx").is_file()&&crate::LOG_PATH.with_file_name("scene-assets").join("dance/hip-hop-motion.txt").is_file()}
struct Playback {owner:usize,handle:FieldInsHandle,previous:i32,speed:f32,requested:i32,started:Instant,retries:u8,last_restore:Option<Instant>}
static PLAYBACK:Mutex<Option<Playback>>=Mutex::new(None);
fn family(anim:i32)->i32 {(anim%1_000_000)/10*10}
fn restore(active:bool,elapsed:f32,actual:i32,requested:i32)->bool {
    (!active||elapsed>=10.0)&&family(actual)==family(requested)
}
fn restore_ready(active:bool,elapsed:f32,locked:bool,actual:i32,requested:i32)->bool {
    !locked&&(restore(active,elapsed,actual,requested)||(!active&&actual>=0&&family(actual)==family(HIP_HOP)))
}
fn exit_target(_previous:i32,_idle:i32,requested:i32,_retries:u8)->Option<i32>{
    // The event idle field is overwritten by requests; replaying a prior action can loop forever.
    (family(requested)!=0).then_some(0)
}
fn retry_delay(retries:u8)->u128 {if retries<8{250}else{1000}}
pub fn tick(){
    let requested=crate::dance_ui::take_move();
    let Ok(mut playback)=PLAYBACK.try_lock() else{return;};
    let Ok(world)=(unsafe{WorldChrMan::instance_mut()}) else{*playback=None;return;};
    let Some(player)=world.main_player.as_mut() else{*playback=None;return;};
    let chr=&mut player.chr_ins;let owner=std::ptr::from_ref(chr) as usize;
    if playback.as_ref().is_some_and(|p|p.owner!=owner||p.handle!=chr.field_ins_handle){*playback=None;}
    let t=&chr.modules.time_act;let actual=t.anim_queue.get(t.read_idx as usize).map_or(-1,|a|a.anim_id);
    if chr.modules.data.hp<=0||chr.chr_flags1c5.death_flag(){if let Some(p)=playback.take(){chr.modules.behavior.animation_speed=p.speed;}return;}
    if let Some(anim)=requested {
        let previous=playback.as_ref().map_or(actual,|p|p.previous);
        let speed=playback.as_ref().map_or(chr.modules.behavior.animation_speed,|p|p.speed);
        if previous<0{return;}
        *playback=Some(Playback{owner,handle:chr.field_ins_handle,previous,speed,requested:anim,started:Instant::now(),retries:0,last_restore:None});
        chr.modules.behavior.animation_speed=if anim==HIP_HOP{f32::from_bits(SPEED.load(Ordering::Relaxed))}else{speed};
        chr.modules.event.request_animation_id=anim;crate::gesture_trace::requested(anim);
    } else if let Some(p)=playback.as_mut() {
        if p.requested==HIP_HOP&&crate::dance_ui::dancing(){
            if family(actual)==family(HIP_HOP){chr.modules.behavior.animation_speed=f32::from_bits(SPEED.load(Ordering::Relaxed));}
            return;
        }
        let elapsed=p.started.elapsed().as_secs_f32();
        let active=crate::dance_ui::dancing();
        if !active||elapsed>=10.0 {
            chr.modules.behavior.animation_speed=p.speed;
            if actual>=0&&family(actual)!=family(p.requested)&&family(actual)!=family(HIP_HOP){
                crate::log(format!("DANCE animation exit confirmed actual={actual} retries={}",p.retries));*playback=None;return;
            }
            let locked=chr.debug_flags.disabled_movement()||chr.debug_flags.disabled_secondary_actions();
            if restore_ready(active,elapsed,locked,actual,p.requested)&&p.last_restore.is_none_or(|at|at.elapsed().as_millis()>=retry_delay(p.retries)){
                let idle=chr.modules.event.idle_anim_id;
                p.last_restore=Some(Instant::now());
                let Some(target)=exit_target(p.previous,idle,p.requested,p.retries) else{return;};
                chr.modules.event.request_animation_id=target;
                if p.retries<8||p.retries%30==0 {crate::log(format!("DANCE animation exit requested from={actual} to={target} previous={} idle={idle} attempt={}",p.previous,u16::from(p.retries)+1));}
                p.retries=if p.retries==u8::MAX{8}else{p.retries+1};
            }
        }
    }
}
#[cfg(test)]mod tests {
    #[test]fn polluted_idle_never_repeats_the_failed_pre_dance_action(){
        for attempt in 0..=255 {
            assert_eq!(super::exit_target(23030010,23030010,80720,attempt),Some(0));
            assert_eq!(super::exit_target(80720,80720,80720,attempt),Some(0));
        }
    }
    #[test]fn exit_uses_neutral_animation_and_keeps_retrying(){
        assert_eq!(super::exit_target(3000,2000000,80720,0),Some(0));
        assert_eq!(super::exit_target(3000,2000000,80720,1),Some(0));
        assert_eq!(super::exit_target(80720,0,80720,0),Some(0));
        assert_eq!(super::exit_target(-1,80720,80720,8),Some(0));
        assert_eq!(super::exit_target(3000,2000000,80720,8),Some(0));
        assert_eq!(super::exit_target(3000,2000000,0,8),None);
        assert_eq!(super::retry_delay(7),250);
        assert_eq!(super::retry_delay(8),1000);
    }
    #[test]fn exit_waits_for_menu_lock_release_and_handles_pending_finale(){
        assert!(!super::restore_ready(false,2.4,true,80800,80800));
        assert!(super::restore_ready(false,2.8,false,80800,80800));
        assert!(super::restore_ready(false,2.8,false,80720,80800));
        assert!(!super::restore_ready(false,2.8,false,3000,80800));
    }
    #[test]fn dance_loop_spans_whole_four_bar_groups(){
        for bpm in [122.0f64,87.26]{for duration in [212.0/30.0,57.3]{
            let beat=60.0/bpm;let groups=(duration/(16.0*beat)).round().max(1.0);
            assert!((duration/f64::from(super::loop_speed(beat,duration))-groups*16.0*beat).abs()<0.00001);
        }}
    }
    #[test]fn only_owned_animation_is_restored_on_exit_or_timeout(){
        assert!(super::restore(false,0.5,80800,80800));
        assert!(!super::restore(true,0.5,80800,80800));
        assert!(super::restore(true,10.0,80801,80800));
        assert!(!super::restore(false,10.0,3000,80800));
    }
}
