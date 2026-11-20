use eldenring::cs::{CSCamera,WorldChrMan};
use fromsoftware_shared::{FromStatic,F32Vector4,F32ViewMatrix};
use std::sync::Mutex;

struct Saved {owner:usize,block:i32,camera:usize,matrix:F32ViewMatrix,locked:bool,facing:[f32;2],eye:Option<[f32;3]>,target:Option<[f32;3]>,updated:std::time::Instant}
static SAVED:Mutex<Option<Saved>>=Mutex::new(None);

fn frame(eye:[f32;3],target:[f32;3])->F32ViewMatrix {
    let d=[target[0]-eye[0],target[1]-eye[1],target[2]-eye[2]];let len=d.iter().map(|v|v*v).sum::<f32>().sqrt();
    let f=[d[0]/len,d[1]/len,d[2]/len];let h=f[0].hypot(f[2]);let r=[f[2]/h,0.0,-f[0]/h];
    let up=[f[1]*r[2],f[2]*r[0]-f[0]*r[2],-f[1]*r[0]];
    F32ViewMatrix::new(F32Vector4(r[0],r[1],r[2],0.0),F32Vector4(up[0],up[1],up[2],0.0),F32Vector4(f[0],f[1],f[2],0.0),F32Vector4(eye[0],eye[1],eye[2],1.0))
}

pub fn tick(_: &eldenring::fd4::FD4TaskData) {
    let Ok(mut saved)=SAVED.try_lock() else{return;};
    let Ok(world)=(unsafe{WorldChrMan::instance_mut()}) else{*saved=None;return;};
    let Some(player)=world.main_player.as_ref() else{*saved=None;if crate::dance_ui::dancing(){crate::dance_ui::cancel();}return;};
    let owner=std::ptr::from_ref(player) as usize;let block=i32::from(player.current_block_id);
    let Some(mut ptr)=world.chr_cam else{*saved=None;return;};
    let camera=unsafe{ptr.as_mut()};let address=std::ptr::from_ref(camera) as usize;
    if saved.as_ref().is_some_and(|s|s.owner!=owner||s.camera!=address||s.block!=block){*saved=None;crate::dance_ui::cancel();}
    let alive=player.chr_ins.modules.data.hp>0&&!player.chr_ins.chr_flags1c5.death_flag();
    if !crate::dance_ui::dancing()||!alive {
        if let Some(s)=saved.take(){camera.pers_cam.matrix=s.matrix;camera.is_movement_locked=s.locked;camera.request_camera_reset=true;crate::log("DANCE camera restored");}
        if !alive&&crate::dance_ui::dancing(){crate::dance_ui::cancel();}return;
    }
    let physics=&player.chr_ins.modules.physics;
    if saved.is_none(){
        let yaw=physics.orientation.to_euler_angles().1;
        if !yaw.is_finite(){return;}
        *saved=Some(Saved{owner,block,camera:address,matrix:camera.pers_cam.matrix,locked:camera.is_movement_locked,facing:[-yaw.sin(),-yaw.cos()],eye:None,target:None,updated:std::time::Instant::now()});
        crate::log("DANCE camera front view");
    }
    let p=&physics.position;let position=[p.0,p.1,p.2];if !position.iter().all(|v|v.is_finite()){return;}
    let shot=erdtree_tower::dance_stage::shot(crate::dance_ui::timeline(),crate::dance_ui::beat(),crate::dance_stage::count());
    let s=saved.as_mut().unwrap();let [x,z]=s.facing;let (sin,cos)=shot.yaw.sin_cos();
    let center=if shot.group&&block==0x2d010000{[0.0,position[1],5.5]}else{position};
    let desired=[center[0]+(x*cos+z*sin)*shot.distance,center[1]+shot.height,center[2]+(z*cos-x*sin)*shot.distance];
    let aim=[center[0],center[1]+shot.target_height,center[2]];
    let alpha=1.0-(-s.updated.elapsed().as_secs_f32().min(0.1)*3.0).exp();s.updated=std::time::Instant::now();
    let eye=s.eye.get_or_insert(desired);for i in 0..3{eye[i]+=(desired[i]-eye[i])*alpha;}
    let target=s.target.get_or_insert(aim);for i in 0..3{target[i]+=(aim[i]-target[i])*alpha;}
    let matrix=frame(*eye,*target);
    camera.is_movement_locked=true;camera.pers_cam.matrix=matrix;
    if let Ok(render)=unsafe{CSCamera::instance_mut()}{render.pers_cam_1.matrix=matrix;}
}

#[cfg(test)]mod tests {
    #[test]fn camera_faces_the_actor_from_four_metres_in_front(){
        for facing in [[0.0,-1.0],[1.0,0.0],[0.6,0.8]] {
            let m=super::frame([10.0+facing[0]*4.0,20.0,30.0+facing[1]*4.0],[10.0,20.0,30.0]);
            assert!((m.3.0+m.2.0*4.0-10.0).abs()<0.0001);
            assert!((m.3.2+m.2.2*4.0-30.0).abs()<0.0001);
            assert!((m.0.0*m.2.0+m.0.2*m.2.2).abs()<0.0001);
        }
    }
}
