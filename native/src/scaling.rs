use eldenring::cs::{ChrInsExt,FieldInsHandle,SoloParamRepository,SpEffectParam,WorldChrMan};
use fromsoftware_shared::FromStatic;
pub const GROWTH_FIRST:i32=9640000;
const TIER_FIRST:i32=9641000;
fn area_effect(id:i32)->bool {(7000..=7280).contains(&id)||(20007000..=20007350).contains(&id)||(TIER_FIRST..TIER_FIRST+300).contains(&id)}
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
/// 新旅程按本幕进度给每个刷出的敌人施加一次递增效果（生命与攻击）。
/// 联机时房主与成员都在本机给同步过来的敌人挂上同一个效果：这个效果不随网络同步，只有房主挂时，
/// 成员那边同一只敌人的最大生命更低，成员一刀把本机副本打到 0、房主那边还活着，副本卡在原地不倒、成员也拿不到卢恩。
/// 已经挂着的不重复挂
#[derive(Default)]
pub struct Growth {key:Option<(u64,u8,usize)>,applied:Vec<FieldInsHandle>,missing:bool}
impl Growth {
    pub fn tick(&mut self,world:&mut WorldChrMan,key:(u64,u8,usize),step:u8,handles:&[FieldInsHandle]) {
        if self.key!=Some(key){*self=Self{key:Some(key),..Default::default()};}
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
}
