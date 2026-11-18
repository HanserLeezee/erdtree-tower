//! 余烬「魔力辉剑」：魔法命中敌人时，从玩家头顶射出一把「魔法辉剑」的飞行辉剑追击那只敌人。
//! 认施法：只认 Magic 参数直接引用且自身或后续弹体能造成伤害的起始弹体，排除辅助弹体和后续子弹。
//! 同一次施法的多发弹体（150 ms 内）只算一次；每次施法最多换来一把辉剑（CAST_LIFETIME 内用掉，过期作废）。
//! 生成：拿这次施法的生成数据当模板（其余字段保持游戏自己的值），只改成辉剑弹体、魔法编号、目标、朝向和位置，
//! 直接调用原函数（不经过挂钩），所以辉剑自己不会再算成施法，不会连锁。
use std::{collections::{BTreeMap,BTreeSet},ffi::c_void,sync::{Mutex,atomic::{AtomicU64,Ordering}},time::{Duration,Instant}};
use eldenring::cs::{Bullet,Magic,SoloParamRepository,CSBulletManager,FieldInsHandle};
use fromsoftware_shared::FromStatic;

/// 原版「魔法辉剑」（MagicParam 4390）的飞行段：攻击 43900、初速 13、带追踪
pub const BULLET:i32=10439003;
pub const MAGIC:i32=4390;
const SAME_CAST:Duration=Duration::from_millis(150);
const CAST_LIFETIME:Duration=Duration::from_secs(4);
/// 辉剑出现在玩家头顶多高
const LIFT:f32=2.2;

#[repr(C,align(16))]
#[derive(Clone)]
pub struct SpawnData(pub [u8;0x110]);

struct Casts {template:Option<SpawnData>,pending:Vec<Instant>,last:Option<Instant>}
static CASTS:Mutex<Casts>=Mutex::new(Casts{template:None,pending:Vec::new(),last:None});
static ROOTS:Mutex<BTreeMap<i32,BTreeSet<i32>>>=Mutex::new(BTreeMap::new());
/// 本机玩家的 FieldInsHandle（持有余烬时每帧更新，0 为不记录）
static OWNER:AtomicU64=AtomicU64::new(0);

pub fn handle_bits(handle:&FieldInsHandle)->u64 {unsafe {std::ptr::read_unaligned((handle as *const FieldInsHandle).cast::<u64>())}}
pub fn set_owner(owner:Option<&FieldInsHandle>) {OWNER.store(owner.map_or(0,handle_bits),Ordering::Relaxed);}

fn read_i32(data:&SpawnData,at:usize)->i32 {i32::from_le_bytes(data.0[at..at+4].try_into().unwrap())}
fn write_i32(data:&mut SpawnData,at:usize,value:i32) {data.0[at..at+4].copy_from_slice(&value.to_le_bytes());}
fn sorcery(magic:i32)->bool {
    magic>0&&erdtree_tower::rewards::lookup(0x40000000|magic as u32).is_some_and(|i|i.kind==erdtree_tower::rewards::ItemKind::Sorcery)
}
fn casting_bullets(roots:&[i32],lookup:impl Fn(i32)->Option<(i32,[i32;2])>)->BTreeSet<i32> {
    roots.iter().copied().filter(|root| {
        let mut pending=vec![*root];let mut seen=BTreeSet::new();
        while let Some(id)=pending.pop() {
            if id<0||!seen.insert(id){continue;}
            let Some((attack,children))=lookup(id) else{continue};
            // Native attacks 0 and 1 are non-damaging helpers.
            if attack>1{return true;}
            pending.extend(children);
        }
        false
    }).collect()
}
fn casting_bullet(magic:i32,bullet:i32)->bool {
    let Ok(mut cache)=ROOTS.lock() else{return false};
    if let Some(roots)=cache.get(&magic){return roots.contains(&bullet);}
    let Ok(params)=(unsafe {SoloParamRepository::instance()}) else{return false};
    let Some(row)=params.get::<Magic>(magic as u32) else{return false};
    let refs=[(row.ref_category1(),row.ref_id1()),(row.ref_category2(),row.ref_id2()),
        (row.ref_category3(),row.ref_id3()),(row.ref_category4(),row.ref_id4()),
        (row.ref_category5(),row.ref_id5()),(row.ref_category6(),row.ref_id6()),
        (row.ref_category7(),row.ref_id7()),(row.ref_category8(),row.ref_id8()),
        (row.ref_category9(),row.ref_id9()),(row.ref_category10(),row.ref_id10())];
    let refs:Vec<_>=refs.into_iter().filter_map(|(category,id)|(category==1&&id>=0).then_some(id)).collect();
    let roots=casting_bullets(&refs,|id|params.get::<Bullet>(id as u32).map(|row|(row.atk_id_bullet(),[row.hit_bullet_id(),row.interval_create_bullet_id()])));
    let accepted=roots.contains(&bullet);cache.insert(magic,roots);accepted
}

/// 弹体生成挂钩里调用（游戏线程）：本机玩家放出的魔法记为一次施法
pub fn observe(data:&SpawnData) {
    let owner=OWNER.load(Ordering::Relaxed);
    if owner==0||u64::from_le_bytes(data.0[0..8].try_into().unwrap())!=owner {return;}
    let magic=read_i32(data,0x0c);
    let bullet=read_i32(data,0x14);
    if !sorcery(magic)||!casting_bullet(magic,bullet) {return;}
    let Ok(mut casts)=CASTS.try_lock() else{return};
    let now=Instant::now();
    if casts.last.is_none_or(|at|now.duration_since(at)>SAME_CAST) {
        casts.pending.retain(|at|now.duration_since(*at)<=CAST_LIFETIME);
        casts.pending.push(now);
        super::log(format!("TOWER_GLINTBLADE cast magic={magic} bullet={bullet}"));
    }
    casts.last=Some(now);casts.template=Some(data.clone());
}
/// 用掉一次还没换过辉剑的施法
pub fn take_cast()->bool {
    let Ok(mut casts)=CASTS.lock() else{return false};
    let now=Instant::now();
    casts.pending.retain(|at|now.duration_since(*at)<=CAST_LIFETIME);
    if casts.pending.is_empty() {return false;}
    casts.pending.remove(0);true
}
pub fn reset() {if let Ok(mut casts)=CASTS.lock(){casts.pending.clear();casts.last=None;}}

/// 按模板改出一发辉剑：从 origin 飞向 target_pos，追踪 target
fn blade(template:&SpawnData,target:u64,origin:[f32;3],target_pos:[f32;3])->Option<SpawnData> {
    let start=[origin[0],origin[1]+LIFT,origin[2]];
    let to=[target_pos[0]-start[0],target_pos[1]+1.0-start[1],target_pos[2]-start[2]];
    let length=to.iter().map(|v|v*v).sum::<f32>().sqrt();
    if !(length.is_finite()&&length>0.01)||start.iter().any(|v|!v.is_finite()) {return None;}
    let dir=to.map(|v|v/length);
    let flat=(dir[0]*dir[0]+dir[2]*dir[2]).sqrt();
    let right=if flat>0.001 {[dir[2]/flat,0.0,-dir[0]/flat]}else{[1.0,0.0,0.0]};
    let up=[dir[1]*right[2]-dir[2]*right[1],dir[2]*right[0]-dir[0]*right[2],dir[0]*right[1]-dir[1]*right[0]];
    let mut data=template.clone();
    write_i32(&mut data,0x0c,MAGIC);write_i32(&mut data,0x14,BULLET);
    data.0[0x20..0x28].copy_from_slice(&target.to_le_bytes());
    for (offset,row) in [(0x50,right),(0x60,up),(0x70,dir)] {
        for (i,v) in row.into_iter().chain([0.0]).enumerate() {data.0[offset+i*4..offset+i*4+4].copy_from_slice(&v.to_le_bytes());}
    }
    for (i,v) in start.into_iter().chain([1.0]).enumerate() {data.0[0x80+i*4..0x84+i*4].copy_from_slice(&v.to_le_bytes());}
    Some(data)
}

/// 射出一把辉剑（游戏线程）
pub fn spawn(origin:[f32;3],target:&FieldInsHandle,target_pos:[f32;3])->Result<(),String> {
    let template=CASTS.lock().ok().and_then(|c|c.template.clone()).ok_or("还没有记录到施法")?;
    let data=blade(&template,handle_bits(target),origin,target_pos).ok_or("辉剑方向无效")?;
    let original=super::star_stream::original().ok_or("弹体生成接口未就绪")?;
    let manager=unsafe {CSBulletManager::instance_mut()}.map_err(|_|"弹体管理器未就绪")? as *mut CSBulletManager as *mut c_void;
    let (mut out,mut error)=(0i32,0i32);
    unsafe {original(manager,&mut out,&data,&mut error)};
    if out==-1 {return Err(format!("辉剑生成失败 code={error}"));}
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pebble_auxiliary_emission_is_not_a_second_cast() {
        let roots=casting_bullets(&[10400000,10400099],|id|match id {
            10400000=>Some((40000,[-1,-1])),10400099=>Some((1,[-1,-1])),_=>None,
        });
        assert_eq!(roots.into_iter().collect::<Vec<_>>(),vec![10400000]);
    }
    #[test]
    fn delayed_spell_counts_its_launcher_but_not_the_child_or_visual() {
        let roots=casting_bullets(&[100,200],|id|match id {
            100=>Some((0,[101,-1])),101=>Some((40000,[-1,-1])),
            200=>Some((0,[201,-1])),201=>Some((1,[-1,-1])),_=>None,
        });
        assert!(roots.contains(&100));assert!(!roots.contains(&101));assert!(!roots.contains(&200));
    }
    #[test]
    fn cycles_and_missing_bullets_cannot_create_casts() {
        assert!(casting_bullets(&[100,999],|id|match id {100=>Some((0,[101,-1])),101=>Some((1,[100,-1])),_=>None}).is_empty());
    }
    #[test]
    fn blade_keeps_the_template_and_only_rewrites_bullet_target_and_pose() {
        let template=SpawnData([0x3a;0x110]);
        let data=blade(&template,77,[0.0,0.0,0.0],[3.0,-1.2,4.0]).unwrap();
        assert_eq!(read_i32(&data,0x0c),MAGIC);assert_eq!(read_i32(&data,0x14),BULLET);
        assert_eq!(u64::from_le_bytes(data.0[0x20..0x28].try_into().unwrap()),77);
        let f=|at:usize|f32::from_le_bytes(data.0[at..at+4].try_into().unwrap());
        assert_eq!((f(0x80),f(0x84),f(0x88),f(0x8c)),(0.0,LIFT,0.0,1.0));
        // 朝向指向目标胸口（目标位置 +1 米）
        let dir=[f(0x70),f(0x74),f(0x78)];let to:[f32;3]=[3.0,-1.2+1.0-LIFT,4.0];let len=(to[0]*to[0]+to[1]*to[1]+to[2]*to[2]).sqrt();
        for i in 0..3 {assert!((dir[i]-to[i]/len).abs()<1e-5);}
        for i in 0..0x110 {
            let rewritten=(0x0c..0x10).contains(&i)||(0x14..0x18).contains(&i)||(0x20..0x28).contains(&i)||(0x50..0x90).contains(&i);
            if !rewritten {assert_eq!(data.0[i],0x3a,"byte {i:#x} must come from the template");}
        }
        assert!(blade(&template,77,[0.0,0.0,0.0],[0.0,LIFT-1.0,0.0]).is_none(),"zero-length aim is rejected");
    }
}
