use erdtree_tower::leech::{Deltas,Hits};

#[test]
fn only_new_hits_in_this_fight_heal_the_attacking_boss() {
    let key=(42,2,3);let mut deltas=Deltas::default();
    let mut report=Hits{key,counts:vec![(100,1),(200,0)]};
    assert_eq!(deltas.take(key,7,&report),vec![(100,1)]);
    assert!(deltas.take(key,7,&report).is_empty(),"repeated reports and passive HP loss cannot heal");
    report.counts[0].1=3;
    assert_eq!(deltas.take(key,7,&report),vec![(100,2)]);
    report.counts[0].1=2;assert!(deltas.take(key,7,&report).is_empty());
    report.counts[0].1=3;assert!(deltas.take(key,7,&report).is_empty());
    report.key=(42,2,4);assert!(deltas.take(key,7,&report).is_empty());
}

#[test]
fn teammates_count_independently_and_a_new_fight_resets_counts() {
    let key=(42,2,3);let report=Hits{key,counts:vec![(100,1)]};let mut deltas=Deltas::default();
    assert_eq!(deltas.take(key,1,&report),vec![(100,1)]);
    assert_eq!(deltas.take(key,2,&report),vec![(100,1)]);
    assert_eq!(Deltas::default().take(key,1,&report),vec![(100,1)]);
}

#[test]
fn party_reports_keep_hit_ownership_and_reject_wrong_scope() {
    use erdtree_tower::{Class,Run,party_game::{Gate,Report}};
    let run=Run::new(42,Class::Samurai);let mut gate=Gate::new(vec![1,2]);
    let mut report=Report::new(&run,gate.epoch,true,true,true);
    report.leech_hits=Some(Hits{key:(42,1,3),counts:vec![(100,1)]});
    gate.report(1,report.clone()).unwrap();gate.report(2,report.clone()).unwrap();
    assert_eq!(gate.leech_hits(1),vec![(2,report.leech_hits.clone().unwrap())]);
    report.leech_hits.as_mut().unwrap().key.0=99;assert!(gate.report(2,report).is_err());
    let legacy=Report::new(&run,gate.epoch,true,true,true);
    let mut json=serde_json::to_value(&legacy).unwrap();json.as_object_mut().unwrap().remove("leech_hits");
    let decoded:Report=serde_json::from_value(json).unwrap();assert!(decoded.leech_hits.is_none());
}
