use eldenring::{cs::WorldChrMan,rotation::Quaternion};
use std::sync::atomic::{AtomicUsize,Ordering};
static COUNT:AtomicUsize=AtomicUsize::new(1);
pub fn count()->usize{COUNT.load(Ordering::Relaxed)}
pub fn place(world:&mut WorldChrMan,slot:usize,count:usize)->Result<(),String>{
    let player=world.main_player.as_mut().ok_or("舞台角色尚未加载")?;
    if i32::from(player.current_block_id)!=0x2d010000{return Err("请先进入舞会事件房".into());}
    let p=&mut player.chr_ins.modules.physics;
    if !p.standing_on_solid_ground{return Err("等待角色在舞台落地".into());}
    let target=erdtree_tower::dance_stage::position(slot,count);
    super::spirits::landing::move_native(p,target)?;
    p.orientation=Quaternion(0.0,1.0,0.0,0.0);p.interpolated_orientation=p.orientation;
    COUNT.store(count.clamp(1,8),Ordering::Relaxed);
    super::log(format!("DANCE stage slot={slot} players={count} position={target:?}"));Ok(())
}
