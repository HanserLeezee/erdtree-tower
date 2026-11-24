// 六个武器栏、四件防具与四枚护符计入词条；卸装时撤下补充效果。
use eldenring::cs::{ChrIns,ChrInsExt,EquipParamProtector,EquipParamWeapon,PlayerGameData,SoloParamRepository,SpEffectParam};
use fromsoftware_shared::FromStatic;
use std::time::{Duration,Instant};

const AFFIX_FIRST:i32=9850000;
const AFFIX_SPAN:i32=210;
const FIRST:i32=9880000;
const SPAN:i32=1000;
const COPIES:usize=13;
pub const WEAPON_SLOTS:[usize;6]=[0,1,2,3,4,5];
fn affix(id:i32)->bool {(AFFIX_FIRST..AFFIX_FIRST+AFFIX_SPAN).contains(&id)}
fn ours(id:i32)->bool {(FIRST..FIRST+SPAN*(COPIES as i32+1)).contains(&id)&&(id-FIRST)%SPAN<AFFIX_SPAN}
fn copy(effect:i32,n:usize)->i32 {FIRST+n as i32*SPAN+effect-AFFIX_FIRST}
/// 身上的词条效果（含重复）→ 需要挂的复制效果
fn wanted(effects:&[i32],native:&[i32])->Vec<i32> {
    let mut counts=std::collections::BTreeMap::new();
    for &effect in effects.iter().filter(|e|affix(**e)) {*counts.entry(effect).or_insert(0usize)+=1;}
    counts.into_iter().flat_map(|(effect,count)|{
        let first=usize::from(native.contains(&effect));
        (first..count.min(COPIES+1)).map(move |n|copy(effect,n))
    }).collect()
}
pub fn equipped_talismans(data: &PlayerGameData) -> impl Iterator<Item = (&'static erdtree_tower::talisman_passives::Variant, u32)> + '_ {
    let equipment = &data.equipment;
    (17..=20).filter_map(move |slot| {
        let param = u32::try_from(equipment.chr_asm.equipment_param_ids[slot]).ok()?;
        let variant = erdtree_tower::talisman_passives::variant(0x20000000 | param)?;
        let index = equipment.equipment_item_idx_list[slot];
        let entry = equipment.equip_inventory_data.items_data.entry_at_slot(index)?.as_option()?;
        (entry.quantity > 0 && entry.item_id.into_inner() == variant.item.id).then_some((variant, entry.sort_id))
    })
}
#[derive(Clone, Copy, Default)]
pub struct WeaponInstance {
    pub variant: u32,
    pub effects: [u32; 3],
    pub mask: u16,
    pub tier: u8,
}

pub fn weapon_instance(data: &PlayerGameData, slot: usize) -> Option<WeaponInstance> {
    let id=u32::try_from(*data.equipment.chr_asm.equipment_param_ids.get(slot)?).ok()?;
    weapon_instance_for(data,slot,id)
}
fn weapon_instance_for(data:&PlayerGameData,slot:usize,id:u32)->Option<WeaponInstance> {
    type Key = (u32, u32, u32);
    thread_local! { static CACHE: std::cell::RefCell<[Option<(Key, Option<WeaponInstance>)>; 6]> = const { std::cell::RefCell::new([None; 6]) }; }
    let equipment = &data.equipment;
    let entry = equipment.equip_inventory_data.items_data.entry_at_slot(*equipment.equipment_item_idx_list.get(slot)?)?.as_option()?;
    if slot >= 6 || entry.quantity == 0 || entry.item_id.into_inner()/10000 != id/10000 { return None; }
    let (character,sort)=super::card_affixes::identity(data.character_id,id,entry.sort_id);
    let key = (character,sort,id);
    CACHE.with_borrow_mut(|cache| {
        if let Some((previous, result)) = cache[slot] {
            if previous == key { return result; }
        }
        let result = erdtree_tower::weapon_rolls::instance(key.0, key.1, id).map(|roll| {
            let mut result = WeaponInstance { variant: roll.variant, ..Default::default() };
            for (dest, effect) in result.effects.iter_mut().zip(roll.effects) {
                *dest = effect;
                if effect >= 9875100 {
                    result.mask |= 1 << ((effect - 9875100) / 4);
                    result.tier = ((effect - 9875100) % 4) as u8;
                }
            }
            result
        });
        cache[slot] = Some((key, result));
        result
    })
}

pub fn instances_ready() -> bool {
    (unsafe {SoloParamRepository::instance()}).ok().is_some_and(|p| p.get::<SpEffectParam>(9875000).is_some())
}
/// 只计算装备栏，不包括背包和箭矢；护符的原有效果仍由游戏处理。
#[derive(Clone,Copy)]
struct SlotHp { character:u32,item:u32,effect:i32,seen:Instant }

fn stable_slot_hp(cached:&mut Option<SlotHp>,character:u32,item:u32,resolved:Option<i32>,now:Instant)->Option<i32>{
    if let Some(effect)=resolved {
        *cached=(AFFIX_FIRST..AFFIX_FIRST+4).contains(&effect).then_some(SlotHp{character,item,effect,seen:now});
    }
    let entry=(*cached)?;
    if entry.character==character && entry.item==item && now.duration_since(entry.seen)<Duration::from_millis(350){
        Some(entry.effect)
    }else{*cached=None;None}
}
fn equipped(data:&PlayerGameData,params:&SoloParamRepository)->Vec<i32> {
    let asm=&data.equipment.chr_asm;
    let mut effects=Vec::new();
    let ready = instances_ready();
    thread_local! {static SLOT_HP:std::cell::RefCell<[Option<SlotHp>;6]>=const{std::cell::RefCell::new([None;6])};}
    let now=Instant::now();
    for slot in WEAPON_SLOTS {
        // Passive bonuses belong to the assigned inventory slots, including sheathed weapons.
        let key=eldenring::cs::ChrAsmSlot::from_index(slot as u32).expect("weapon slot");
        let id=data.equipment.equipment_entries[key].as_valid().map_or(0,|item|item.into_inner());
        let roll=ready.then(||weapon_instance_for(data,slot,id)).flatten();
        let hp=roll.as_ref().map(|r|r.effects.iter().map(|e|*e as i32)
            .find(|e|(AFFIX_FIRST..AFFIX_FIRST+4).contains(e)).unwrap_or(-1));
        let retained=SLOT_HP.with_borrow_mut(|slots|stable_slot_hp(&mut slots[slot],data.character_id,id,hp,now));
        if let Some(roll)=roll {effects.extend(roll.effects.map(|id|id as i32));continue;}
        if let Some(effect)=retained {effects.push(effect);continue;}
        if id==0 {continue;}
        let Some(weapon)=params.get::<EquipParamWeapon>(id as u32).or_else(||params.get::<EquipParamWeapon>(id as u32/100*100)) else {continue};
        effects.extend([weapon.resident_sp_effect_id(),weapon.resident_sp_effect_id1(),weapon.resident_sp_effect_id2()]);
    }
    for slot in 12..=15 {
        let Some(id)=asm.equipment_param_ids.get(slot).copied().filter(|id|*id>0) else {continue};
        let Some(armor)=params.get::<EquipParamProtector>(id as u32) else {continue};
        effects.extend([armor.resident_sp_effect_id(),armor.resident_sp_effect_id2(),armor.resident_sp_effect_id3()]);
    }
    for (variant, sort_id) in equipped_talismans(data) {
        let (character,sort)=super::card_affixes::identity(data.character_id,variant.item.id,sort_id);
        effects.extend(erdtree_tower::talisman_passives::instance_effects(variant,character,sort));
    }
    effects
}
/// 每帧：按身上的重复词条挂上或撤下复制效果（参数缺失时不挂）
pub fn sync(chr:&mut ChrIns,data:&PlayerGameData) {
    if chr.modules.data.hp<=0||chr.chr_flags1c5.death_flag() {return;}
    let Ok(params)=(unsafe {SoloParamRepository::instance()}) else{return};
    let active:Vec<i32>=chr.special_effect.entries().map(|e|e.param_id).collect();
    let want=wanted(&equipped(data,params),&active);
    let present:Vec<i32>=active.into_iter().filter(|id|ours(*id)).collect();
    for &id in present.iter().filter(|id|!want.contains(id)) {chr.remove_speffect(id);}
    for &id in want.iter().filter(|id|!present.contains(id)) {
        if params.get::<SpEffectParam>(id as u32).is_some() {chr.apply_speffect(id,false);}
        else {super::log_repeating("AFFIX_EQUIPPED missing parameter rows; update regulation.bin with the DLL");}
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn switching_preserves_only_the_same_equipped_hp_affix() {
        use super::*;
        let t=Instant::now();
        let mut slot=None;
        let hp=AFFIX_FIRST+3;
        assert_eq!(stable_slot_hp(&mut slot,1,100,Some(hp),t),Some(hp));
        assert_eq!(stable_slot_hp(&mut slot,1,100,None,t+Duration::from_millis(60)),Some(hp));
        assert_eq!(stable_slot_hp(&mut slot,1,100,None,t+Duration::from_secs(1)),None);
        for (character,item,effect) in [(1,0,None),(2,100,None),(1,101,None),(1,100,Some(-1))]{
            stable_slot_hp(&mut slot,1,100,Some(hp),t);
            assert_eq!(stable_slot_hp(&mut slot,character,item,effect,t),None);
        }
    }
    #[test]
    fn inactive_weapons_receive_the_first_effect_copy() {
        let effect=9850041;
        assert_eq!(super::wanted(&[effect],&[]),vec![9880041]);
        assert!(super::wanted(&[effect],&[effect]).is_empty());
        assert_eq!(super::wanted(&[effect;6],&[]).len(),6);
        assert_eq!(super::wanted(&[effect;6],&[effect]).len(),5);
    }
    #[test]
    fn switching_hands_and_removing_equipment_do_not_duplicate_bonuses() {
        let effect=9850041;
        for count in 1..=14 {
            let equipped=vec![effect;count];
            let held=super::wanted(&equipped,&[effect,effect]);
            let sheathed=super::wanted(&equipped,&[]);
            assert_eq!(held.len()+1,count);
            assert_eq!(sheathed.len(),count);
            assert_eq!(&sheathed[1..],held.as_slice());
        }
        assert!(super::wanted(&[],&[effect]).is_empty());
        assert_eq!(super::wanted(&[effect,effect+1],&[effect]),vec![9880042]);
    }
    #[test]
    fn each_extra_piece_with_the_same_affix_adds_one_copy() {
        let attack_rare=9850000+4*10+1;let hp_common=9850000;
        assert!(super::wanted(&[attack_rare,hp_common,-1,0],&[attack_rare,hp_common]).is_empty(),"native effects are not duplicated");
        assert_eq!(super::wanted(&[attack_rare,attack_rare],&[attack_rare]),vec![9881041]);
        assert_eq!(super::wanted(&[attack_rare;3],&[attack_rare]),vec![9881041,9882041]);
        assert_eq!(super::wanted(&[attack_rare,attack_rare+1],&[attack_rare,attack_rare+1]),Vec::<i32>::new(),"different qualities already stack natively");
        assert_eq!(super::wanted(&[hp_common;14],&[hp_common]).len(),super::COPIES,"fourteen equipment slots");
        assert!(super::wanted(&[7000,7000,12345,12345],&[]).is_empty(),"only tower affix effects");
        assert!(super::ours(9880041)&&super::ours(9881041)&&super::ours(9893209)&&!super::ours(9880250)&&!super::ours(9894000));
    }
}
