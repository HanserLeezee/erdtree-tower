//! 调试刷怪请求的兜底。
//! 敌人、服务 NPC、粪金龟、木桩都走游戏的调试角色生成器：设好 `debug_chr_creator.spawn` 后，
//! 游戏要等模型读完才真正生成，第一次见到的模型要好几秒。这期间玩家死亡、快速结算、重试或被传走，
//! 原来只有粪金龟和木桩会撤回请求，敌人和 NPC 的请求一直挂着，会在读新地图时才被处理。
//! 这里记下每次请求时的地图，每帧检查：传送中、角色不在或已经换了地图，就撤掉请求。
//! 角色死亡本身不撤：联机或无缝复活后可能还在原地，死亡后的读档会走到上面三种情况。
use eldenring::cs::{ChrDebugSpawnRequest,GameMan,WorldChrMan};
use fromsoftware_shared::F32Vector4;
use std::sync::Mutex;

#[derive(Clone,Copy,Debug,PartialEq)]
struct Issued {block:i32,chr:i32,npc:i32,local:Option<[f32;3]>}
static ISSUED:Mutex<Option<Issued>>=Mutex::new(None);

fn player_origin(world:&WorldChrMan)->Option<[f32;3]> {
    let player=world.main_player.as_ref()?;
    let p=&player.chr_ins.modules.physics.position;
    let b=&player.block_position;
    erdtree_tower::encounter::floating_origin([p.0,p.1,p.2],[b.x,b.y,b.z])
}

/// 代替 `world.spawn_debug_character`：记下发出请求时所在的地图与区块局部目标，
/// 挂起期间若引擎切换浮点原点，自动同步平移 `init_data.spawn_position`。
pub fn spawn(world:&mut WorldChrMan,request:&ChrDebugSpawnRequest) {
    let block=world.main_player.as_ref().map_or(-1,|p|i32::from(p.current_block_id));
    let local=player_origin(world).and_then(|origin|erdtree_tower::encounter::local_from_world(origin,[request.pos_x,request.pos_y,request.pos_z]));
    *ISSUED.lock().unwrap_or_else(|e|e.into_inner())=Some(Issued{block,chr:request.chr_id,npc:request.npc_param_id,local});
    world.spawn_debug_character(request);
}

/// 请求是否该撤：warp=游戏正在传送，player=当前地图
fn stale(issued_block:i32,warp:bool,player:Option<i32>)->Option<&'static str> {
    if warp {return Some("warp");}
    match player {
        None=>Some("no_player"),
        Some(block) if block!=issued_block=>Some("map_changed"),
        Some(_)=>None,
    }
}

pub fn tick(world:&mut WorldChrMan,game:&GameMan) {
    let mut slot=ISSUED.lock().unwrap_or_else(|e|e.into_inner());
    let Some(issued)=*slot else {return;};
    // 已经生成完，或者生成器被别的请求占用了：不再归我们管
    if !world.debug_chr_creator.spawn||world.debug_chr_creator.init_data.npc_param_id!=issued.npc {*slot=None;return;}
    let player=world.main_player.as_ref().map(|p|i32::from(p.current_block_id));
    if let Some(reason)=stale(issued.block,game.warp_requested,player) {
        world.debug_chr_creator.spawn=false;
        world.debug_chr_creator.last_created_chr=None;
        *slot=None;
        super::log(format!("DEBUG_SPAWN cancelled reason={reason} chr=c{:04} npc={} issued_block={:08x} current={}",issued.chr,issued.npc,issued.block,player.map_or("-".into(),|b|format!("{b:08x}"))));
        return;
    }
    if let (Some(local),Some(origin))=(issued.local,player_origin(world)) {
        let cur=world.debug_chr_creator.init_data.spawn_position;
        if let Some(updated)=erdtree_tower::encounter::pending_spawn_retarget([cur.0,cur.1,cur.2],local,origin) {
            world.debug_chr_creator.init_data.spawn_position=F32Vector4(updated[0],updated[1],updated[2],0.0);
            super::log(format!(
                "DEBUG_SPAWN origin_shift corrected chr=c{:04} npc={} old=[{:.2},{:.2},{:.2}] new=[{:.2},{:.2},{:.2}] local=[{:.2},{:.2},{:.2}]",
                issued.chr,issued.npc,cur.0,cur.1,cur.2,updated[0],updated[1],updated[2],local[0],local[1],local[2]
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::stale;
    #[test]
    fn pending_spawns_are_dropped_once_the_scene_is_gone() {
        assert_eq!(stale(0x2d010000,false,Some(0x2d010000)),None,"same map: keep waiting for the model");
        assert_eq!(stale(0x2d010000,true,Some(0x2d010000)),Some("warp"));
        assert_eq!(stale(0x2d010000,false,None),Some("no_player"));
        assert_eq!(stale(0x2d010000,false,Some(0x0b0a0000)),Some("map_changed"));
    }
}
