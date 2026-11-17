use erdtree_tower::boss_affixes::adaptation::Adaptation;

#[test]
fn adaptation_rolls_in_new_runs_and_keeps_old_rules_unchanged() {
    use erdtree_tower::{Class,Run,NodeKind,boss_affixes::{self,Affix}};
    let mut seen=false;
    for seed in 0..300 {
        let mut run=Run::new(seed,Class::Samurai);
        assert_eq!(run.boss_affix_rules,2);
        let node=run.nodes.iter().position(|n|n.kind==NodeKind::Boss).unwrap();
        seen|=boss_affixes::roll(&run,node).contains(&Affix::Adaptive);
        for rules in [0,1] {
            run.boss_affix_rules=rules;
            assert!(!boss_affixes::roll(&run,node).contains(&Affix::Adaptive));
            assert_eq!(Run::decode(&run.encode().unwrap()).unwrap().boss_affix_rules,rules);
        }
    }
    assert!(seen);
}

#[test]
fn first_hit_is_full_then_ten_percent_steps_stop_at_fifty() {
    for channel in 0..5 {
        let mut state=Adaptation::default();
        let mut hit=[0;5];hit[channel]=100;
        for expected in [100,90,80,70,60,50,50,50] {
            assert_eq!(state.receive(hit,true)[channel],expected);
        }
    }
}

#[test]
fn switching_pure_type_starts_over_instead_of_remembering_old_stacks() {
    let mut state=Adaptation::default();
    for _ in 0..6 {state.receive([100,0,0,0,0],true);}
    assert_eq!(state.receive([0,0,100,0,0],true),[0,0,100,0,0]);
    assert_eq!(state.receive([0,0,100,0,0],true),[0,0,90,0,0]);
    assert_eq!(state.receive([100,0,0,0,0],true),[100,0,0,0,0]);
}

#[test]
fn mixed_damage_bypasses_resistance_and_does_not_change_stacks() {
    let mut state=Adaptation::default();
    for _ in 0..3 {state.receive([100,0,0,0,0],true);}
    let before=state;
    for a in 0..5 {for b in a+1..5 {
        let mut hit=[0;5];hit[a]=100;hit[b]=1;
        assert_eq!(state.receive(hit,true),hit);
        assert_eq!(state,before);
    }}
    assert_eq!(state.receive([100,0,0,0,0],true),[70,0,0,0,0]);
}

#[test]
fn zero_and_periodic_damage_do_not_change_adaptation() {
    let mut state=Adaptation::default();
    state.receive([0,100,0,0,0],true);
    let before=state;
    assert_eq!(state.receive([0;5],true),[0;5]);
    assert_eq!(state.receive([0,0,0,100,0],false),[0,0,0,100,0]);
    assert_eq!(state,before);
}

#[test]
fn small_and_large_hits_never_overflow_or_become_immunity() {
    let mut state=Adaptation::default();
    for _ in 0..8 {assert_eq!(state.receive([1,0,0,0,0],true),[1,0,0,0,0]);}
    assert_eq!(state.receive([u32::MAX,0,0,0,0],true)[0],2_147_483_648);
    assert_eq!(Adaptation::default().receive([100,0,0,0,0],true)[0],100);
}
