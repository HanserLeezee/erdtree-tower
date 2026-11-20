use erdtree_tower::{Run,Class,rewards::*};

#[test]
fn pending_random_ash_rewards_migrate_to_the_cap() {
    for seed in 0..40 {
        let mut run=Run::new(seed,Class::Confessor);
        run.spirit_peak=Some(7);run.spirit_fixed_levels=false;
        let node=run.start_nodes()[0];run.enter(node).unwrap();run.win_combat().unwrap();
        let mut json=serde_json::to_value(&run).unwrap();
        json.as_object_mut().unwrap().remove("spirit_fixed_levels");
        let loaded=Run::decode(&json.to_string()).unwrap();
        assert!(loaded.spirit_fixed_levels);
        let erdtree_tower::Phase::Reward{offers,..}=&loaded.phase else {panic!()};
        let levels:Vec<_>=offers.iter().filter_map(|r|match r {erdtree_tower::Reward::Item(id)=>spirit_level(*id),_=>None}).collect();
        assert!(!levels.is_empty());assert!(levels.iter().all(|level|*level==7));
        loaded.validate().unwrap();
        assert_eq!(Run::decode(&loaded.encode().unwrap()).unwrap().phase,loaded.phase);
    }
}

#[test]
fn every_spirit_supports_39_levels_and_stops_at_the_boundary() {
    for ash in CATALOG.iter().filter(|i|i.kind==ItemKind::Spirit) {
        assert_eq!(spirit_tune_target(ash.id+10,10),Some(ash.id+11));
        assert_eq!(spirit_tune_target(ash.id,38),Some(ash.id+39));
        assert_eq!(spirit_tune_target(ash.id+39,39),None);
        for level in 0..=39 {
            assert_eq!(spirit_level(ash.id+level),Some(level as u8));
            assert_eq!(spirit_base(ash.id+level),ash.id);
            assert!(lookup(ash.id+level).is_some());
        }
        assert_eq!(spirit_level(ash.id+40),None);
    }
}

#[test]
fn high_level_ashes_survive_save_and_reward_generation() {
    let mut run=Run::new(42,Class::Confessor);run.spirit_peak=Some(39);
    assert_eq!(spirit_cap(&run),39);
    assert_eq!(Run::decode(&run.encode().unwrap()).unwrap().spirit_peak,Some(39));
    let ash=CATALOG.iter().find(|i|i.kind==ItemKind::Spirit).unwrap().id;
    for cap in [0,1,7,10,25,39,40,u32::MAX] {
        assert_eq!(capped_spirit_level(ash,cap),ash+cap.min(39));
        assert_eq!(capped_spirit_level(ash+3,cap),ash+cap.min(39));
    }
    let weapon=CATALOG.iter().find(|i|i.kind==ItemKind::Weapon).unwrap().id;
    assert_eq!(capped_spirit_level(weapon,39),weapon);
    run.spirit_peak=Some(40);assert!(run.validate().is_err());
}
