#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Observation {
    Alive,
    Dead,
    Missing,
    DifferentEntity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Waiting,
    Victory,
    PlayerDied,
    LostEntity,
    SpawnTimedOut,
}

#[derive(Clone, Debug)]
pub struct Encounter<H> {
    handle: Option<H>,
    terminal: Option<Outcome>,
}

impl<H: Copy + Eq> Default for Encounter<H> {
    fn default() -> Self {
        Self {
            handle: None,
            terminal: None,
        }
    }
}

impl<H: Copy + Eq> Encounter<H> {
    pub fn bind(&mut self, handle: H, observed_alive: bool) -> bool {
        if self.handle.is_some() || self.terminal.is_some() || !observed_alive {
            return false;
        }
        self.handle = Some(handle);
        true
    }

    pub fn handle(&self) -> Option<H> {
        self.handle
    }

    pub fn poll(
        &mut self,
        player_dead: bool,
        observation: Observation,
        spawn_expired: bool,
    ) -> Outcome {
        if let Some(terminal) = self.terminal {
            return terminal;
        }
        let outcome = if player_dead {
            Outcome::PlayerDied
        } else if self.handle.is_none() {
            if spawn_expired {
                Outcome::SpawnTimedOut
            } else {
                Outcome::Waiting
            }
        } else {
            match observation {
                Observation::Alive => Outcome::Waiting,
                Observation::Dead => Outcome::Victory,
                Observation::Missing | Observation::DifferentEntity => Outcome::LostEntity,
            }
        };
        if outcome != Outcome::Waiting {
            self.terminal = Some(outcome);
        }
        outcome
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {Weak,Strong,Boss}
/// 单人精英及事件战同时在场的敌人上限；普通战按固定波次。
pub const SOLO_CAP:usize=2;
/// 后续波次：前一波的条件满足后再等这么久才出场
pub const WAVE_GAP:std::time::Duration=std::time::Duration::from_secs(3);
pub const FIRST_WAVE_GRACE:std::time::Duration=std::time::Duration::from_secs(1);
/// 后续波次的敌人出场后这么久内不出招（防止贴脸突袭）。
pub const WAVE_GRACE:std::time::Duration=std::time::Duration::from_secs(3);
#[derive(Clone,Debug,Default)]
pub struct SpawnGap {
    blocked:bool,
    ready_since:Option<std::time::Duration>,
    admitted:bool,
}
impl SpawnGap {
    /// 同批直接出场；曾等待前批的敌人，条件满足后再等 WAVE_GAP。放行后不再重新拦截。
    pub fn allows(&mut self,now:std::time::Duration,eligible:bool)->bool {
        if self.admitted {return true;}
        if !eligible {self.blocked=true;self.ready_since=None;return false;}
        if self.blocked&&now.saturating_sub(*self.ready_since.get_or_insert(now))<WAVE_GAP{return false;}
        self.admitted=true;true
    }
    /// 已经放行、而且是等过前批才出场的（后续波次）：出场后有 WAVE_GRACE 的出招保护期
    pub fn later_wave(&self)->bool {self.admitted&&self.blocked}
    pub fn holds_attacks(&self,since_bound:std::time::Duration)->bool {self.admitted&&since_bound<if self.later_wave(){WAVE_GRACE}else{FIRST_WAVE_GRACE}}
}
/// 普通战单人和联机统一分波；其他战斗保留原有精英节奏、单人上限和 Boss 组规则。
/// previous 为本组中排在 index 之前的敌人状态，未生成的敌人记为 Waiting。
pub fn may_spawn(networked:bool,monster_node:bool,roles:&[Role],index:usize,previous:&[Outcome])->bool {
    may_spawn_paced(networked,monster_node,false,roles,index,previous)
}
/// 精英战里精英怪池的精英怎么上场（苦难律法「精英成双」）：一只一只（前一只倒下）、前一只半血、一起上
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Pacing {Apart,HalfHealth,Together}
impl Pacing {
    /// 这只精英要不要等：前面的精英还没上场，或（一只一只时）还活着，或（半血时）生命还在一半以上
    pub fn waits(self,earlier:&[(Outcome,Option<(i32,i32)>)])->bool {
        earlier.iter().any(|(outcome,health)|*outcome!=Outcome::Victory&&match self {
            Pacing::Together=>false,
            Pacing::Apart=>true,
            Pacing::HalfHealth=>health.is_none_or(|(hp,max)|i64::from(hp)*2>i64::from(max)),
        })
    }
}
/// held：这只精英按 Pacing::waits 需要等前面的精英（由运行时按生命算好）
pub fn may_spawn_paced(networked:bool,monster_node:bool,held:bool,roles:&[Role],index:usize,previous:&[Outcome])->bool {
    if roles.iter().all(|r|*r==Role::Boss) {return true;}
    if monster_node {
        let wave_start=if roles.iter().all(|r|*r==Role::Weak) {0}else{index/2*2};
        return (0..wave_start).all(|i|previous.get(i)==Some(&Outcome::Victory));
    }
    if held&&roles[index]==Role::Strong {return false;}
    if networked {return true;}
    if previous.iter().filter(|o|**o!=Outcome::Victory).count()>=SOLO_CAP {return false;}
    true
}

/// 玩家世界坐标与区块局部坐标的差值，即引擎当前的浮点原点偏移（按 8m / 32m 步进切换）。
pub fn floating_origin(world:[f32;3],block:[f32;3])->Option<[f32;3]> {
    let origin=[world[0]-block[0],world[1]-block[1],world[2]-block[2]];
    (world.iter().all(|v|v.is_finite())&&block.iter().all(|v|v.is_finite())&&origin.iter().all(|v|v.is_finite())).then_some(origin)
}
pub fn world_from_local(origin:[f32;3],local:[f32;3])->Option<[f32;3]> {
    let world=[origin[0]+local[0],origin[1]+local[1],origin[2]+local[2]];
    (origin.iter().all(|v|v.is_finite())&&local.iter().all(|v|v.is_finite())&&world.iter().all(|v|v.is_finite())).then_some(world)
}
pub fn local_from_world(origin:[f32;3],world:[f32;3])->Option<[f32;3]> {
    let local=[world[0]-origin[0],world[1]-origin[1],world[2]-origin[2]];
    (origin.iter().all(|v|v.is_finite())&&world.iter().all(|v|v.is_finite())&&local.iter().all(|v|v.is_finite())).then_some(local)
}
/// 调试刷怪请求挂起期间若浮点原点发生切换，引擎不会自动平移 `CSDebugChrCreatorInitData::spawn_position`，
/// 返回按最新原点重算的世界坐标（浮点原点步进至少 8m，此处 >0.5m 即重算）。
pub fn pending_spawn_retarget(current_world:[f32;3],local_target:[f32;3],current_origin:[f32;3])->Option<[f32;3]> {
    if !current_world.iter().all(|v|v.is_finite()){return None;}
    let desired=world_from_local(current_origin,local_target)?;
    let horiz=(desired[0]-current_world[0]).hypot(desired[2]-current_world[2]);
    let vert=(desired[1]-current_world[1]).abs();
    (horiz>0.5||vert>0.5).then_some(desired)
}
/// 单人敌人绑定后校验实际落点是否因同帧浮点原点切换或异常刷飞（水平 >6m 或垂直 >4m），
/// 若偏离则返回 `(目标世界坐标, 实际区块坐标, 水平偏离, 垂直偏离)` 供立刻拉回。
pub fn spawn_drift_target(actual_world:[f32;3],expected_local:[f32;3],current_origin:[f32;3])->Option<([f32;3],[f32;3],f32,f32)> {
    let actual_local=local_from_world(current_origin,actual_world)?;
    let desired_world=world_from_local(current_origin,expected_local)?;
    let horiz=(actual_local[0]-expected_local[0]).hypot(actual_local[2]-expected_local[2]);
    let vert=(actual_local[1]-expected_local[1]).abs();
    (horiz>6.0||vert>4.0).then_some((desired_world,actual_local,horiz,vert))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floating_origin_shift_retargets_pending_and_drifted_spawns() {
        // 复现龙王平台 Node 39 小壶故障：请求时原点 z=-296，刷出前玩家跨过 z=328 原点切至 z=-328。
        let old_origin=floating_origin([1.258,1.538,31.630],[17.258,1009.538,327.630]).unwrap();
        let new_origin=floating_origin([1.018,1.595,2.823],[17.018,1009.595,330.823]).unwrap();
        let target_local=[11.755,1009.726,330.262];
        let stale_world=world_from_local(old_origin,target_local).unwrap();
        assert!((stale_world[2]-34.262).abs()<0.01);
        // 原点未变时不改写
        assert_eq!(pending_spawn_retarget(stale_world,target_local,old_origin),None);
        // 原点切换 -32m 后，挂起请求的世界坐标自动更新为 z=+2.262
        let updated=pending_spawn_retarget(stale_world,target_local,new_origin).unwrap();
        assert!((updated[0]-stale_world[0]).abs()<0.01);
        assert!((updated[1]-stale_world[1]).abs()<0.01);
        assert!((updated[2]-2.262).abs()<0.01);
        // 同帧竞态下若敌人已按旧世界坐标刷出，实际局部坐标会偏移 +32m 到 z=362.262，触发拉回
        let (snap_world,actual_local,horiz,vert)=spawn_drift_target(stale_world,target_local,new_origin).unwrap();
        assert!((actual_local[2]-362.262).abs()<0.01);
        assert!((horiz-32.0).abs()<0.05&&vert<0.05);
        assert!((snap_world[2]-2.262).abs()<0.01);
        // 正常落点（含轻微移动）不触发拉回
        let normal_world=world_from_local(new_origin,[12.5,1009.726,331.0]).unwrap();
        assert_eq!(spawn_drift_target(normal_world,target_local,new_origin),None);
        // 非有限坐标安全拒绝
        assert_eq!(floating_origin([f32::NAN,0.0,0.0],[0.0,0.0,0.0]),None);
        assert_eq!(pending_spawn_retarget([f32::INFINITY,0.0,0.0],target_local,new_origin),None);
        assert_eq!(spawn_drift_target([0.0,f32::NAN,0.0],target_local,new_origin),None);
    }

    #[test]
    fn later_wave_attacks_wait_three_seconds_from_binding_not_wave_admission() {
        use std::time::Duration as D;
        let mut first=SpawnGap::default();assert!(first.allows(D::ZERO,true));
        assert!(first.holds_attacks(D::ZERO));
        assert!(!first.holds_attacks(D::from_secs(1)));
        let mut later=SpawnGap::default();assert!(!later.allows(D::ZERO,false));
        assert!(!later.allows(D::from_secs(10),true));
        assert!(later.allows(D::from_secs(13),true));
        assert!(later.holds_attacks(D::ZERO));
        assert!(later.holds_attacks(D::from_millis(2999)));
        assert!(!later.holds_attacks(D::from_secs(3)));
        assert!(!later.holds_attacks(D::from_secs(10)));
    }

    #[test]
    fn ordinary_rooms_use_the_same_complete_waves_in_solo_and_party() {
        use Role::*;
        use Outcome::*;
        for networked in [false,true] {
            for roles in [vec![Weak,Weak],vec![Weak,Weak,Weak]] {
                for i in 0..roles.len() {assert!(may_spawn(networked,true,&roles,i,&vec![Waiting;i]));}
            }
            let middle=[Weak,Weak,Strong];
            assert!(!may_spawn(networked,true,&middle,2,&[Victory,Waiting]));
            assert!(!may_spawn(networked,true,&middle,2,&[Victory,LostEntity]));
            assert!(may_spawn(networked,true,&middle,2,&[Victory,Victory]));
            let late=[Strong,Weak,Strong,Weak];
            assert!(may_spawn(networked,true,&late,0,&[]));
            assert!(may_spawn(networked,true,&late,1,&[Waiting]));
            for prior in [[Waiting,Victory],[Victory,Waiting],[Victory,SpawnTimedOut]] {
                assert!(!may_spawn(networked,true,&late,2,&prior));
                assert!(!may_spawn(networked,true,&late,3,&[prior[0],prior[1],Waiting]));
            }
            assert!(may_spawn(networked,true,&late,2,&[Victory,Victory]));
            assert!(may_spawn(networked,true,&late,3,&[Victory,Victory,Waiting]));
        }
    }

    #[test]
    fn later_wave_waits_three_seconds_after_spawn_conditions_clear() {
        use std::time::Duration as D;
        let mut gap=SpawnGap::default();
        assert!(!gap.allows(D::ZERO,false));
        assert!(!gap.allows(D::from_secs(10),false));
        assert!(!gap.allows(D::from_secs(20),true));
        assert!(!gap.allows(D::from_millis(22999),true));assert!(!gap.later_wave());
        assert!(gap.allows(D::from_secs(23),true));assert!(gap.later_wave(),"it waited for the earlier wave");
        assert!(gap.allows(D::from_secs(25),true));
        assert!(gap.allows(D::from_secs(26),false),"an admitted enemy must not be held again while loading or fighting");
    }
    #[test]
    fn first_wave_and_same_wave_members_have_no_extra_serial_delay() {
        use std::time::Duration as D;
        let mut first=SpawnGap::default();assert!(first.allows(D::ZERO,true));assert!(!first.later_wave(),"the first wave spawns immediately");
        assert!(first.holds_attacks(D::ZERO));assert!(first.holds_attacks(D::from_millis(999)));
        assert!(!first.holds_attacks(D::from_secs(1)));
        let mut second=[SpawnGap::default(),SpawnGap::default()];
        for gap in &mut second {assert!(!gap.allows(D::ZERO,false));assert!(!gap.allows(D::from_secs(5),true));}
        for gap in &mut second {
            assert!(gap.allows(D::from_secs(8),true));
            assert!(gap.holds_attacks(D::from_millis(2999)));
            assert!(!gap.holds_attacks(D::from_secs(3)));
        }
    }
    #[test]
    fn interrupted_spawn_eligibility_restarts_the_wave_gap() {
        use std::time::Duration as D;
        let mut gap=SpawnGap::default();
        assert!(!gap.allows(D::ZERO,false));assert!(!gap.allows(D::from_secs(1),true));
        assert!(!gap.allows(D::from_secs(2),false));assert!(!gap.allows(D::from_secs(10),true));
        assert!(!gap.allows(D::from_millis(12999),true));assert!(gap.allows(D::from_secs(13),true));
    }

    #[test]
    fn requires_a_live_bound_enemy_before_victory() {
        let mut fight = Encounter::<u64>::default();
        assert_eq!(
            fight.poll(false, Observation::Dead, false),
            Outcome::Waiting
        );
        assert!(!fight.bind(7, false));
        assert!(fight.bind(7, true));
        assert_eq!(
            fight.poll(false, Observation::Dead, false),
            Outcome::Victory
        );
    }

    #[test]
    fn missing_or_replaced_enemies_are_not_kills() {
        for observation in [Observation::Missing, Observation::DifferentEntity] {
            let mut fight = Encounter::<u64>::default();
            assert!(fight.bind(7, true));
            assert_eq!(fight.poll(false, observation, false), Outcome::LostEntity);
            assert_eq!(
                fight.poll(false, Observation::Dead, false),
                Outcome::LostEntity
            );
        }
    }

    #[test]
    fn simultaneous_death_is_not_victory() {
        let mut fight = Encounter::<u64>::default();
        fight.bind(7, true);
        assert_eq!(
            fight.poll(true, Observation::Dead, false),
            Outcome::PlayerDied
        );
    }

    #[test]
    fn late_binding_after_timeout_is_rejected() {
        let mut fight = Encounter::<u64>::default();
        assert_eq!(
            fight.poll(false, Observation::Alive, true),
            Outcome::SpawnTimedOut
        );
        assert!(!fight.bind(7, true));
    }

    #[test]
    fn binding_cannot_be_replaced_and_timeout_does_not_end_active_fight() {
        let mut fight = Encounter::<u64>::default();
        assert!(fight.bind(7, true));
        assert!(!fight.bind(8, true));
        assert_eq!(fight.handle(), Some(7));
        assert_eq!(
            fight.poll(false, Observation::Alive, true),
            Outcome::Waiting
        );
    }

    #[test]
    fn solo_waves_cap_and_boss_groups() {
        use super::{Role::*,may_spawn,Outcome::*};
        let duo=[Boss,Boss];
        assert!(may_spawn(false,false,&duo,1,&[Waiting]),"boss components spawn together");
        let monsters=[Weak,Weak,Strong,Strong];
        assert!(may_spawn(false,true,&monsters,1,&[Waiting]));
        assert!(!may_spawn(false,true,&monsters,2,&[Waiting,Waiting]),"cap of two");
        assert!(!may_spawn(false,true,&monsters,2,&[Victory,Waiting]),"strong wave waits for weak enemies");
        assert!(may_spawn(false,true,&monsters,2,&[Victory,Victory]));
        assert!(may_spawn(false,true,&monsters,3,&[Victory,Victory,Waiting]),"strong enemies arrive together");
        let elite=[Strong,Strong,Weak,Weak];
        assert!(may_spawn(false,false,&elite,1,&[Waiting]),"elite and escorts together");
        assert!(!may_spawn(false,false,&elite,2,&[Waiting,Waiting]));
        assert!(may_spawn(false,false,&elite,2,&[Victory,Waiting]),"next enemy fills a freed slot");
        // 联机：没有同时在场上限，但精英战的精英照样按「精英成双」等前一只
        let pair=[Role::Strong,Role::Strong,Role::Weak];
        assert!(may_spawn_paced(true,false,false,&pair,1,&[Waiting]),"party: no cap");
        assert!(!may_spawn_paced(true,false,true,&pair,1,&[Waiting]),"party: the second elite waits");
        assert!(may_spawn_paced(true,false,true,&pair,2,&[Waiting,Waiting]),"escorts never wait");
        assert!(may_spawn_paced(true,false,true,&[Role::Boss,Role::Boss],1,&[Waiting]),"boss groups spawn together");
        assert!(!may_spawn(true,true,&monsters,3,&[Waiting,Waiting,Waiting]),"party waits for the previous wave too");
        // 精英战：精英怪池的两只精英一只一只上；苦难律法「精英成双」时一起上；Boss 组（精英版双人组）不受影响
        let pair=[Role::Strong,Role::Strong,Role::Weak];
        assert!(!may_spawn_paced(false,false,true,&pair,1,&[Waiting]),"a held elite waits");
        assert!(may_spawn_paced(false,false,false,&pair,1,&[Waiting]),"paired elites arrive together");
        let alive_full=[(Waiting,Some((100,100)))];let alive_half=[(Waiting,Some((50,100)))];let not_yet=[(Waiting,None)];let dead=[(Victory,Some((0,100)))];
        assert!(Pacing::Apart.waits(&alive_half)&&!Pacing::Apart.waits(&dead));
        assert!(Pacing::HalfHealth.waits(&alive_full)&&Pacing::HalfHealth.waits(&not_yet)&&!Pacing::HalfHealth.waits(&alive_half)&&!Pacing::HalfHealth.waits(&dead));
        assert!(!Pacing::Together.waits(&alive_full));
        assert!(may_spawn_paced(false,false,true,&duo,1,&[Waiting]),"boss duos are untouched");
        assert!(!may_spawn_paced(true,false,true,&pair,1,&[Waiting]),"party elites follow the same pacing");
    }
}
