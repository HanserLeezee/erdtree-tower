use erdtree_tower::{Class,Run,Phase,party_reconnect::{Checkpoint,Ticket}};

#[test]
fn reconnect_advances_route_without_copying_host_build_or_granting_rewards() {
    let mut guest=Run::new(42,Class::Samurai);guest.gold=321;guest.flasks=2;
    let mut host=Run::new(42,Class::Astrologer);
    let node=host.available()[0];host.enter(node).unwrap();host.win_combat().unwrap();host.decline_reward().unwrap();
    guest.enter(node).unwrap();
    let before=guest.clone();
    let restored=Checkpoint::of(&host,7).unwrap().restore(&guest).unwrap();
    assert_eq!(restored.visited,host.visited);assert_eq!(restored.phase,Phase::Map);
    assert_eq!(restored.class,before.class);assert_eq!(restored.gold,before.gold);
    assert_eq!(restored.flasks,before.flasks);assert_eq!(restored.claimed_items,before.claimed_items);
    assert_eq!(restored.hp,before.hp);restored.validate().unwrap();
    assert_eq!(Checkpoint::of(&host,7).unwrap().restore(&restored).unwrap(),restored);
}

#[test]
fn reconnect_rejects_other_run_rollback_and_unsafe_host() {
    let guest=Run::new(42,Class::Samurai);
    assert!(Checkpoint::of(&Run::new(43,Class::Samurai),1).unwrap().restore(&guest).is_err());
    let mut host=guest.clone();let node=host.available()[0];host.enter(node).unwrap();
    assert!(Checkpoint::of(&host,1).is_err());host.win_combat().unwrap();
    assert!(Checkpoint::of(&host,1).is_err());host.decline_reward().unwrap();
    assert!(Checkpoint::of(&guest,1).unwrap().restore(&host).is_err());
}

#[test]
fn ticket_is_bound_to_character_player_host_session_and_run() {
    let run=Run::new(42,Class::Samurai);
    let ticket=Ticket{version:erdtree_tower::party_handshake::VERSION,owner:(12,0),player:2,host:1,session:99,seed:42,generation:0};
    assert!(ticket.matches((12,0),2,1,&run));
    assert!(!ticket.matches((13,0),2,1,&run));assert!(!ticket.matches((12,0),3,1,&run));
    assert!(!ticket.matches((12,0),2,3,&run));assert!(!ticket.matches((12,0),2,1,&Run::new(43,Class::Samurai)));
    assert_eq!(Ticket::decode(&ticket.encode().unwrap()).unwrap(),ticket);
    assert!(Ticket::decode("{}").is_err());assert!(Ticket::decode(&"x".repeat(4097)).is_err());
}

#[test]
fn returning_member_blocks_next_room_until_matching_report_arrives() {
    use erdtree_tower::party_game::{Gate,Report};
    let run=Run::new(42,Class::Samurai);let mut gate=Gate::new(vec![1]);gate.epoch=8;
    gate.report(1,Report::new(&run,8,false,false,true)).unwrap();assert!(gate.pause_ready());
    assert!(gate.rejoin(2));assert!(!gate.rejoin(2));assert!(!gate.rejoin(0));
    assert!(!gate.pause_ready());assert!(gate.enter(&run,run.available()[0]).is_err());
    assert!(gate.report(2,Report::new(&run,7,false,false,true)).is_err());
    gate.report(2,Report::new(&run,8,false,false,true)).unwrap();assert!(gate.pause_ready());
    assert!(gate.enter(&run,run.available()[0]).is_ok());
}

#[test]
fn reconnect_across_act_keeps_personal_resources_and_no_room_rewards() {
    use erdtree_tower::NodeKind;
    let mut host=Run::new(42,Class::Samurai);let mut guest=Run::new(42,Class::Astrologer);
    guest.gold=123;guest.flasks=1;guest.hp=25;guest.enable_contracts();
    while host.act==1 {
        let id=host.available()[0];let kind=host.nodes[id].kind;host.enter(id).unwrap();
        match kind {
            NodeKind::Monster|NodeKind::Elite|NodeKind::Boss=>{host.win_combat().unwrap();host.decline_reward().unwrap();},
            NodeKind::Camp=>host.leave_camp().unwrap(),NodeKind::Shop=>host.leave_shop().unwrap(),
            NodeKind::Chest=>{host.finish_chest().unwrap();if matches!(host.phase,Phase::Reward{..}){host.decline_reward().unwrap();}},
            NodeKind::Event=>{host.resolve_event(false).unwrap();},
        }
    }
    let restored=Checkpoint::of(&host,20).unwrap().restore(&guest).unwrap();
    assert_eq!(restored.act,2);assert_eq!(restored.gold,123);assert_eq!(restored.flasks,1);assert_eq!(restored.hp,25);
    assert_eq!(restored.class,guest.class);assert_eq!(restored.claimed_items,guest.claimed_items);
    assert_eq!(restored.contract.as_ref().unwrap().act,2);restored.validate().unwrap();
}
