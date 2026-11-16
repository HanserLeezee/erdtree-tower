use erdtree_tower::{Class,NodeKind,Run,embers::{Ember,Gain,Stacks}};

#[test]
fn assists_grant_only_soul_eater_the_same_souls_as_kills() {
    for rank in 0..=2 {
        let mut assist=Stacks::default();
        let mut kill=Stacks::default();
        assist.gain(&Ember::ALL,Gain::Assist{rank});
        kill.gain(&[Ember::SoulEater],Gain::Kill{rank,spirit:false});
        assert_eq!(assist,kill);
    }
    let mut empty=Stacks::default();
    empty.gain(&[Ember::SpiritKeeper,Ember::Godslayer,Ember::Harvest],Gain::Assist{rank:2});
    assert!(empty.is_empty());
}

#[test]
fn godslayer_needs_victory_not_the_last_hit_and_never_double_counts_assists() {
    for kind in [NodeKind::Elite,NodeKind::Boss] {
        let mut run=Run::new(42,Class::Samurai);
        let id=run.nodes.iter().position(|n|n.kind==kind).unwrap();
        run.debug_jump(id).unwrap();run.enter(id).unwrap();
        run.embers.push(Ember::Godslayer);
        run.ember_stacks.gain(&run.embers,Gain::Assist{rank:2});
        assert_eq!(run.ember_stacks.level(Ember::Godslayer),0);
        run.win_combat().unwrap();
        assert_eq!(run.ember_stacks.level(Ember::Godslayer),1);
        assert!(run.win_combat().is_err());
        assert_eq!(run.ember_stacks.level(Ember::Godslayer),1);
    }
}
