use erdtree_tower::{Run,Class,rewards,talismans};

#[test]
fn current_runs_roll_triggered_and_original_quality_talismans_with_floors() {
    let manifest:Vec<serde_json::Value>=serde_json::from_str(include_str!("../scene-assets/talisman-manifest.json")).unwrap();
    let mut families=std::collections::BTreeMap::new();
    for row in manifest.iter().filter(|r|r["onHit"]==true){
        let tier=row["tier"].as_u64().unwrap();
        assert!((2..=3).contains(&tier));
        let id=0x20000000|row["id"].as_u64().unwrap() as u32;
        families.insert((row["kind"].as_str().unwrap(),tier),id);
    }
    let triggered:std::collections::HashSet<_>=families.into_values().collect();
    assert_eq!(triggered.len(),37);
    let mut seen=std::collections::HashSet::new();let mut passive=0;
    for seed in 0..1000 {
        let mut run=Run::new(seed,Class::Samurai);
        for floor in 0..=3 {
            let id=talismans::roll_min(&run,0x20000000|1000,7,false,floor);
            assert!(talismans::tier(id).is_some_and(|t|t>=floor));
            assert_eq!(id,talismans::roll_min(&Run::decode(&run.encode().unwrap()).unwrap(),0x20000000|1000,7,false,floor));
            if triggered.contains(&id){seen.insert(id);}else{assert!(erdtree_tower::talisman_passives::variant(id).is_some());passive+=1;}
        }
        run.claimed_items.extend(triggered.iter().copied());
        let id=talismans::roll(&run,0x20000000|1000,7,false);
        assert!(!triggered.contains(&id),"owned triggered variants must not repeat");
    }
    assert_eq!(seen,triggered,"every triggered variant remains reachable");assert!(passive>0);
}

#[test]
fn passive_and_triggered_talismans_are_real_items_with_all_rarities() {
    assert_eq!(talismans::CATALOG.len(),370);
    let mut tiers=std::collections::HashSet::new();
    for item in talismans::CATALOG.iter(){assert_eq!(item.kind,rewards::ItemKind::Talisman);assert_eq!(rewards::lookup(item.id).unwrap().name,item.name);tiers.insert(talismans::tier(item.id).unwrap());}
    assert_eq!(tiers.len(),4);
    assert!(talismans::CATALOG.iter().any(|i|i.name.contains("汲灵")));
    assert!(talismans::CATALOG.iter().any(|i|i.name.contains("雷龙爆发")));
    assert!(talismans::CATALOG.iter().any(|i|i.name.contains("最大")||i.name.contains("生命护符")));
}

#[test]
fn old_saves_keep_original_rewards_and_new_runs_reach_the_new_pool() {
    let mut seen=std::collections::HashSet::new();
    for seed in 0..2000 {
        let run=Run::new(seed,Class::Samurai);let base=0x20000000|1000;
        let id=talismans::roll(&run,base,7,false);assert_eq!(id,talismans::roll(&run,base,7,false));
        if erdtree_tower::talisman_passives::variant(id).is_some(){assert_eq!(erdtree_tower::affixes::base(id),base);}
        if talismans::lookup(id).is_some(){seen.insert(id);}
        let mut old=run.clone();old.talisman_affixes=false;assert_eq!(talismans::roll(&old,base,7,false),base);
    }
    assert!(seen.len()>4);
    let run=Run::new(42,Class::Samurai);let mut json:serde_json::Value=serde_json::from_str(&run.encode().unwrap()).unwrap();
    json.as_object_mut().unwrap().remove("talisman_affixes");let old=Run::decode(&json.to_string()).unwrap();assert!(!old.talisman_affixes);
}

#[test]
fn battle_rewards_expose_new_talismans_and_claimed_variants_do_not_repeat() {
    let (mut seen,mut triggered)=(0,0);
    for seed in 0..500 {
        let mut run=Run::new(seed,Class::Samurai);let node=run.start_nodes()[0];
        for reward in rewards::offers(&run,node) {if let erdtree_tower::Reward::Item(id)=reward {if talismans::lookup(id).is_some(){seen+=1;if talismans::CATALOG.iter().any(|i|i.id==id){triggered+=1;}}}}
        run.claimed_items.extend(talismans::CATALOG.iter().map(|i|i.id));
        run.claimed_items.extend(erdtree_tower::talisman_passives::VARIANTS.iter().map(|v|v.item.id));
        assert!(rewards::offers(&run,node).iter().all(|r|!matches!(r,erdtree_tower::Reward::Item(id) if talismans::lookup(*id).is_some())));
    }
    assert!(seen>20&&triggered>0);
}
