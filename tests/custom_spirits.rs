use erdtree_tower::{Class,Run,rewards::{self,ItemKind}};

fn definitions()->Vec<Vec<&'static str>> {
    include_str!("../scene-assets/custom-spirit-definitions.tsv").lines().map(|l|l.split('|').collect()).collect()
}
#[test]
fn added_spirits_have_bounded_groups_and_matching_descriptions() {
    let defs=definitions();
    for d in &defs[12..] {
        let count:u8=d.get(10).expect("explicit summon count").parse().unwrap();
        let tier:u8=d[4].parse().unwrap();
        assert!((1..=3).contains(&count));
        assert!((1..=3).contains(&tier));
        if tier==3 {assert_eq!(count,1,"legendary elites stay single");}
        if count>1 {assert!(d[8].contains(if count==2 {"两"}else{"三"}));}
    }
    assert_eq!(defs.iter().find(|d|d[0]=="308000").unwrap()[10],"3");
    assert_eq!(defs.iter().find(|d|d[0]=="304000").unwrap()[10],"1");
    assert_eq!(defs.iter().find(|d|d[0]=="309000").unwrap()[10],"1");
}
#[test]
fn all_custom_spirits_have_names_quality_and_forty_levels() {
    let defs=definitions();assert_eq!(defs.len(),98);
    let rarities:std::collections::HashMap<u32,u8>=include_str!("../scene-assets/reward-rarities.tsv").lines().map(|l|{
        let (id,tier)=l.split_once('\t').unwrap();(id.parse().unwrap(),tier.parse().unwrap())
    }).collect();
    let mut ids=std::collections::HashSet::new();
    for d in defs {
        let base=0x40000000|d[0].parse::<u32>().unwrap();assert!(ids.insert(base));
        let item=rewards::lookup(base).expect("custom spirit in reward catalog");
        assert_eq!(item.kind,ItemKind::Spirit);assert_eq!(item.name,d[7]);
        // 只能捕捉获得的（Boss 骨灰）不进奖励与商店
        let capture=d.get(9).is_some_and(|f|f.split(',').any(|f|f=="capture"));
        assert_eq!(item.random_eligible(),!capture,"{}",d[7]);assert_eq!(rewards::capture_only(base),capture);
        assert_eq!(rarities[&base],d[4].parse::<u8>().unwrap());
        for level in 0..=39 {
            assert_eq!(rewards::spirit_level(base+level),Some(level as u8));
            assert!(rewards::lookup(base+level).is_some());
        }
        assert_eq!(rewards::spirit_tune_target(base+39,39),None);
    }
}

#[test]
fn custom_ashes_persist_and_use_the_progress_cap() {
    let mut run=Run::new(42,Class::Confessor);run.spirit_peak=Some(39);
    for d in definitions() {run.claimed_items.push(0x40000000|d[0].parse::<u32>().unwrap()+39);}
    let loaded=Run::decode(&run.encode().unwrap()).unwrap();assert_eq!(loaded.claimed_items,run.claimed_items);
    for id in loaded.claimed_items {
        for cap in [0,7,39] {
            assert_eq!(rewards::capped_spirit_level(id,cap)-rewards::spirit_base(id),cap);
        }
    }
}
