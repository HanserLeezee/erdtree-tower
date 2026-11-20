use std::{collections::HashMap,sync::LazyLock};
use crate::{Run,rewards::{Item,ItemKind}};

struct Variant {id:u32,tier:u8,kind:&'static str,name:&'static str}
static VARIANTS:LazyLock<Vec<Variant>>=LazyLock::new(||include_str!("../scene-assets/talisman-definitions.tsv").lines().map(|line|{
    let p:Vec<_>=line.split('\t').collect();Variant{id:0x20000000|p[0].parse::<u32>().unwrap(),tier:p[3].parse().unwrap(),kind:p[4],name:p[5]}
}).collect());
pub static CATALOG:LazyLock<Vec<Item>>=LazyLock::new(||VARIANTS.iter().map(|v|Item{id:v.id,name:v.name,kind:ItemKind::Talisman}).collect());
static INDEX:LazyLock<HashMap<u32,usize>>=LazyLock::new(||VARIANTS.iter().enumerate().map(|(i,v)|(v.id,i)).collect());
static GROUPS:LazyLock<Vec<Vec<usize>>>=LazyLock::new(||{
    let mut map=std::collections::BTreeMap::<&str,Vec<usize>>::new();
    for (i,v) in VARIANTS.iter().enumerate(){
        if matches!(v.kind,"experienced rider"|"finding"){continue;}
        map.entry(v.kind).or_default().push(i);
    }
    map.into_values().collect()
});
pub fn lookup(id:u32)->Option<&'static Item>{crate::talisman_passives::variant(id).map(|v|&v.item).or_else(||INDEX.get(&id).map(|i|&CATALOG[*i]))}
pub fn tier(id:u32)->Option<u8>{crate::talisman_passives::variant(id).map(|v|v.tier).or_else(||INDEX.get(&id).map(|i|VARIANTS[*i].tier))}
const TRIGGER_KINDS:[&str;24]=["Gravity Hammer","beast claw","black flame","chilling circle","chilly breath","death ground",
    "draconic discharge","extracting spirit","extracting vitality","flame fell God","glintstone pebble","glintstone shards",
    "greater phalanx guard","immortal will","lava exhaust","light rings","lighting call","magical ground","magma ball",
    "phalanx guard","sky spear assault","stomping dragon","summoners grace","thorns"];
static TRIGGERS:LazyLock<Vec<&'static Vec<usize>>>=LazyLock::new(||GROUPS.iter().filter(|g|TRIGGER_KINDS.contains(&VARIANTS[g[0]].kind)).collect());
fn triggered(run:&Run,seed:u64,repeat:bool,min:u8)->Option<u32> {
    let floor=min.min(3);let mut value=crate::mix(seed^0x726172697479)%100;let mut tier=3;
    for (i,odds) in crate::affixes::quality_odds(run).into_iter().enumerate() {if value<u64::from(odds){tier=i as u8;break;}value-=u64::from(odds);}
    tier=tier.max(floor);
    let eligible:Vec<_>=TRIGGERS.iter().flat_map(|g|g.iter()).map(|&i|&VARIANTS[i])
        .filter(|v|v.tier==tier&&(repeat||!run.claimed_items.contains(&v.id))).collect();
    if eligible.is_empty(){return None;}
    Some(eligible[(seed>>8) as usize%eligible.len()].id)
}
pub fn roll(run:&Run,original:u32,entropy:u64,repeat:bool)->u32 {
    roll_min(run,original,entropy,repeat,0)
}
pub fn roll_min(run:&Run,original:u32,entropy:u64,repeat:bool,min:u8)->u32 {
    if !run.talisman_affixes||original>>28!=2||lookup(original).is_some(){return original;}
    let seed=crate::mix(run.reward_seed()^entropy^u64::from(original)^0x74616c69736d616e);
    if run.talisman_passives {
        if seed%2==1&&let Some(id)=triggered(run,seed,repeat,min){return id;}
        if let Some(id)=crate::talisman_passives::roll(run,original,entropy,repeat,min){return id;}
    }
    if seed%2==0{return original;}
    let start=(seed>>8) as usize%GROUPS.len();
    let tier=match crate::mix(seed^0x726172697479)%100 {0..=44=>0,45..=74=>1,75..=92=>2,_=>3};
    for offset in 0..GROUPS.len() {
        let group=&GROUPS[(start+offset)%GROUPS.len()];
        let picked=group.iter().position(|i|VARIANTS[*i].tier>=tier).unwrap_or(group.len()-1);
        for n in 0..group.len(){let id=VARIANTS[group[(picked+n)%group.len()]].id;if repeat||!run.claimed_items.contains(&id){return id;}}
    }
    original
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_quality_matches_journey_odds_and_source_floor() {
        let mut run=Run::new(8,crate::Class::Samurai);
        for (act,layer) in [(1,0),(2,6),(3,12)] {
            run.act=act;run.reached_layer=layer;
            for floor in 0..=3 {
                let mut counts=[0u32;4];let mut expected=[0u32;4];
                for (tier,odds) in crate::affixes::quality_odds(&run).into_iter().enumerate(){expected[tier.max(floor)]+=odds;}
                for seed in 0..20000 {
                    let id=triggered(&run,seed,false,floor as u8).unwrap();
                    counts[tier(id).unwrap() as usize]+=1;
                }
                for tier in 0..4 {
                    assert!((counts[tier] as i64-i64::from(expected[tier])*200).abs()<300,
                        "act={act} floor={floor} tier={tier}: {counts:?}, expected={expected:?}");
                }
            }
        }
    }

    #[test]
    fn exhausted_trigger_quality_does_not_upgrade_or_duplicate() {
        let mut run=Run::new(8,crate::Class::Samurai);
        let seed=(0..10000).find(|&s|crate::mix(s^0x726172697479)%100<45).unwrap();
        run.claimed_items.extend(TRIGGERS.iter().flat_map(|g|g.iter()).map(|&i|&VARIANTS[i]).filter(|v|v.tier==0).map(|v|v.id));
        assert_eq!(triggered(&run,seed,false,0),None);
        assert_eq!(tier(triggered(&run,seed,true,0).unwrap()),Some(0));
        let remaining=run.claimed_items.pop().unwrap();
        assert_eq!(triggered(&run,seed,false,0),Some(remaining));
        assert_eq!(tier(triggered(&run,seed,false,3).unwrap()),Some(3));
    }

    #[test]
    fn riding_and_finding_variants_remain_readable_but_never_roll() {
        let excluded:Vec<_>=VARIANTS.iter().enumerate().filter(|(_,v)|matches!(v.kind,"experienced rider"|"finding")).collect();
        assert_eq!(excluded.len(),8);
        for (index,v) in excluded {
            assert!(lookup(v.id).is_some());
            assert!(!GROUPS.iter().any(|g|g.contains(&index)),"{}",v.name);
        }
        assert_eq!(GROUPS.iter().map(Vec::len).sum::<usize>(),VARIANTS.len()-8);
    }
}
