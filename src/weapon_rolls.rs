use crate::Run;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::LazyLock};

// The variant keeps its stat scaling and damage type. The audit lists the
// resident slots that the builder can replace without losing intrinsic effects.
#[derive(Clone, Copy)]
struct Spec { family: u32, tier: u8, slots: usize }

static SPECS: LazyLock<HashMap<u32, Spec>> = LazyLock::new(|| {
    include_str!("../scene-assets/affix-v2-audit.tsv").lines().skip(1).filter_map(|line| {
        let p: Vec<_> = line.split('\t').collect();
        (p[2] == "weapon").then(|| (p[0].parse().unwrap(), Spec {
            family: p[1].parse::<u32>().unwrap()/1_000_000,
            tier: p[3].parse().unwrap(),
            slots: p[8].split(',').filter(|v| !v.is_empty()).count(),
        }))
    }).collect()
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source { Reward, Shop }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Roll {
    pub variant: u32,
    pub effects: Vec<u32>,
}

fn draw_offer(run:&Run,source:Source,node:usize,slot:usize,variant:u32,count:usize,pool:Vec<u32>)->Roll {
    let source_seed = match source { Source::Reward => 0x726577617264, Source::Shop => 0x73686f70 };
    let seed = [source_seed, u64::from(run.act), node as u64, slot as u64, u64::from(variant)]
        .into_iter().fold(run.reward_seed(), |seed, value| crate::mix(seed ^ value));
    draw(seed, variant, count, pool)
}

fn draw(mut seed: u64, variant: u32, count: usize, mut pool: Vec<u32>) -> Roll {
    let mut effects=Vec::with_capacity(count);
    for _ in 0..count {
        seed=crate::mix(seed);
        effects.push(pool.swap_remove((seed % pool.len() as u64) as usize));
    }
    Roll { variant, effects }
}

fn passives(tier:u32)->Vec<u32> {(0..21).map(|k|9_850_000+k*10+tier).collect()}

/// Only the extra passive slots on quality variants are rolled. Intrinsic and
/// Ruby talisman effects belong to the original accessory parameter rows.
pub fn talisman_offer(run:&Run,source:Source,node:usize,slot:usize,item:u32)->Option<Roll> {
    let variant=crate::talisman_passives::variant(item)?;
    Some(draw_offer(run,source,node,slot,item,variant.effects.len(),passives(u32::from(variant.tier))))
}

/// Stable for one offer, including after reopening the menu or loading a save.
/// Slash support is supplied by the weapon's behavior data (not family alone).
pub fn offer(run: &Run, source: Source, node: usize, slot: usize, weapon: u32, charged_slash: bool) -> Option<Roll> {
    let variant = weapon / 10_000 * 10_000;
    let spec = SPECS.get(&variant)?;
    Some(draw_offer(run, source, node, slot, variant, spec.slots, weapon_pool(*spec, charged_slash)))
}

pub fn instance(character: u32, sort: u32, weapon: u32) -> Option<Roll> {
    let variant = weapon / 10_000 * 10_000;
    let spec = *SPECS.get(&variant)?;
    static ICE: LazyLock<std::collections::HashSet<u32>> = LazyLock::new(||
        include_str!("../scene-assets/weapon-instance-ice.tsv").lines().map(|id| id.parse().unwrap()).collect());
    let seed = crate::mix((u64::from(character) << 32) | u64::from(sort)) ^ u64::from(variant);
    Some(draw(seed, variant, spec.slots, weapon_pool(spec, ICE.contains(&(weapon / 100 * 100)))))
}

pub fn card_identity(run: &Run, node: usize, slot: usize, item: u32) -> Option<(u32,u32)> {
    item_identity(run, 0x63617264, node, slot, item)
}

pub fn shop_identity(run: &Run, node: usize, slot: usize, item: u32) -> Option<(u32,u32)> {
    item_identity(run, 0x73686f70, node, slot, item)
}

fn item_identity(run: &Run, salt: u64, node: usize, slot: usize, item: u32) -> Option<(u32,u32)> {
    let variant = if item >> 28 == 0 { item / 10000 * 10000 } else { item };
    if !SPECS.contains_key(&variant) && crate::talisman_passives::variant(variant).is_none() {return None;}
    let seed = [u64::from(run.act), node as u64, slot as u64, u64::from(variant)].into_iter()
        .fold(run.reward_seed() ^ salt, |seed, value| crate::mix(seed ^ value));
    Some(((seed >> 32) as u32, seed as u32))
}

pub fn slot_count(id: u32) -> Option<usize> {
    SPECS.get(&id).map(|s|s.slots)
}

fn weapon_pool(spec: Spec, charged_slash: bool) -> Vec<u32> {
    let tier = u32::from(spec.tier);
    let mut pool = passives(tier);
    let specialty = match spec.family {
        11..=13 => Some(0), 40..=44 => Some(1), 3|8|66 if charged_slash => Some(2),
        33|34 => Some(3), 4|15|23 => Some(4), 30..=32|62 => Some(5), _ => None,
    };
    pool.extend([6,7,8].into_iter().chain(specialty).map(|k| 9_875_100+k*4+tier));
    pool
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shop_identity_is_stable_and_distinct_from_cards() {
        let run = Run::new_personal(42, crate::Class::Samurai, 19);
        let reloaded = Run::decode(&run.encode().unwrap()).unwrap();
        for item in [160030000u32, 160030008, 537970914] {
            let shop0 = shop_identity(&run, 8, 0, item).unwrap();
            assert_eq!(shop0, shop_identity(&reloaded, 8, 0, item).unwrap());
            assert_ne!(shop0, shop_identity(&run, 8, 1, item).unwrap());
            assert_ne!(shop0, card_identity(&run, 8, 0, item).unwrap());
        }
        assert_eq!(shop_identity(&run, 8, 0, 160030000), shop_identity(&run, 8, 0, 160030008));
        assert!(shop_identity(&run, 8, 0, 1000000).is_none());
    }

    #[test]
    fn paired_instance_test() {
        let roll = |sort, id| instance(0, sort, id).unwrap();
        assert_ne!(roll(2598,160030000),roll(2599,160030000));
        assert_eq!(roll(2598,160030000),roll(2598,160031225));
        assert_eq!(roll(2598,160030000).effects.len(),3);
        assert!(instance(0,2598,1000000).is_none());
    }
    #[test]
    fn quality_talisman_copies_roll_only_their_extra_slots() {
        let run=Run::new_personal(42,crate::Class::Samurai,19);
        let loaded=Run::decode(&run.encode().unwrap()).unwrap();
        for v in crate::talisman_passives::VARIANTS.iter(){
            let id=v.item.id;
            let offer=talisman_offer(&run,Source::Shop,4,1,id).unwrap();
            assert_eq!(offer,talisman_offer(&loaded,Source::Shop,4,1,id).unwrap());
            assert_eq!(offer.variant,id);
            assert_eq!(offer.effects.len(),[1,1,2,3][usize::from(v.tier)]);
            let mut seen=std::collections::HashSet::new();
            assert!(offer.effects.iter().all(|e|seen.insert(e)&&(9850000..9850210).contains(e)&&e%10==u32::from(v.tier)));
            assert!(talisman_offer(&run,Source::Reward,0,0,v.base).is_none());
        }
        let id=crate::talisman_passives::VARIANTS[3].item.id;
        let rolls:std::collections::HashSet<_>=(0..32).map(|slot|talisman_offer(&run,Source::Reward,2,slot,id).unwrap().effects).collect();
        assert!(rolls.len()>16);
        for item in crate::talismans::CATALOG.iter(){assert!(talisman_offer(&run,Source::Shop,1,1,item.id).is_none());}
    }
    #[test]
    fn same_offer_survives_load_and_upgrades_while_distinct_copies_roll_independently() {
        let run=Run::new_personal(42,crate::Class::Samurai,19);
        let reload=Run::decode(&run.encode().unwrap()).unwrap();
        let roll=|r,slot,id|offer(r,Source::Reward,4,slot,id,false).unwrap();
        let one=roll(&run,0,160030000);
        assert_eq!(one,roll(&reload,0,160031225));
        assert_eq!(one.effects.len(),3);
        assert_ne!(one,roll(&run,1,160030000));
        assert_ne!(one,offer(&run,Source::Shop,4,0,160030000,false).unwrap());
        assert_eq!(one,serde_json::from_str(&serde_json::to_string(&one).unwrap()).unwrap());
    }
    #[test]
    fn all_weapon_rows_preserve_slot_counts_quality_and_unique_effects() {
        let run=Run::new(42,crate::Class::Samurai);
        for (&id,spec) in SPECS.iter() {
            let rolled=offer(&run,Source::Shop,0,0,id,false).unwrap();
            assert_eq!(rolled.effects.len(),spec.slots);
            let mut seen=std::collections::HashSet::new();
            for e in rolled.effects {
                assert!(seen.insert(e));
                if e<9_875_100 {assert_eq!((e-9_850_000)%10,u32::from(spec.tier));}
                else {assert_eq!((e-9_875_100)%4,u32::from(spec.tier));assert_ne!((e-9_875_100)/4,2);}
            }
        }
    }
}
