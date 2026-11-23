use crate::Run;

/// 本幕进度对应的递增档：首层 0，Boss 层 GROWTH_STEPS；仅新旅程启用
pub const GROWTH_STEPS:u8=11;
/// 无尽模式第 4 幕起每多一幕的敌人倍率（叠乘在第 3 幕基准与层内成长之上）：生命 ×1.30、攻击 ×1.15
pub const ENDLESS_HP_STEP:f32=1.30;
pub const ENDLESS_ATK_STEP:f32=1.15;
/// 返回无尽模式第 4 幕及之后的额外幕数倍率 (max_hp_rate, attack_power_rate)；1–3 幕返回 None
pub fn endless_rates(act:u8)->Option<(f32,f32)> {
    if act<=3 {return None;}
    let extra=i32::from(act-3);
    Some((ENDLESS_HP_STEP.powi(extra).min(1000.0),ENDLESS_ATK_STEP.powi(extra).min(100.0)))
}
pub fn growth_step(run:&Run,node:usize)->u8 {
    if run.scaling_rules==0 {return 0;}
    let (Some(node),Some(last))=(run.nodes.get(node),run.nodes.iter().map(|n|n.layer).max()) else{return 0};
    if last==0 {return 0;}
    ((node.layer.min(last)*usize::from(GROWTH_STEPS)+last/2)/last) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Class,NodeKind};
    #[test]
    fn growth_rises_through_the_act_and_peaks_at_the_boss() {
        for seed in 0..50 {
            let run=Run::new(seed,Class::Samurai);
            let boss=run.nodes.iter().position(|n|n.kind==NodeKind::Boss).unwrap();
            assert_eq!(growth_step(&run,boss),GROWTH_STEPS);
            for id in run.start_nodes() {assert_eq!(growth_step(&run,id),0);}
            for (a,b) in run.nodes.iter().enumerate().flat_map(|(a,n)|n.next.iter().map(move|b|(a,*b))) {assert!(growth_step(&run,a)<=growth_step(&run,b));}
        }
    }
    #[test]
    fn old_runs_keep_flat_enemy_strength() {
        let mut run=Run::new(3,Class::Samurai);let boss=run.nodes.iter().position(|n|n.kind==NodeKind::Boss).unwrap();
        let mut json:serde_json::Value=serde_json::from_str(&run.encode().unwrap()).unwrap();json.as_object_mut().unwrap().remove("scaling_rules");
        let old=Run::decode(&json.to_string()).unwrap();assert_eq!(growth_step(&old,boss),0);
        run.scaling_rules=0;assert_eq!(growth_step(&run,boss),0);
    }
    #[test]
    fn endless_unlock_requires_non_debug_win_at_280_law() {
        use crate::journal::{Outcome,Record};
        let run=Run::new(7,Class::Samurai);
        let mut won_low=Record::new(&run,vec![],vec![],0);won_low.result=Outcome::Won;
        assert!(!crate::endless_unlocked(&[]));
        assert!(!crate::endless_unlocked(std::slice::from_ref(&won_low)));
        let full_law=(0..crate::law::NODES.len()).fold(crate::law::Law::default(),|l,id|if crate::law::Law::listed(id){l.toggle(id)}else{l});
        assert!(full_law.score()>=crate::ENDLESS_UNLOCK_LAW);
        let mut dead_high=won_low.clone();dead_high.law=full_law;dead_high.result=Outcome::Dead;
        assert!(!crate::endless_unlocked(std::slice::from_ref(&dead_high)));
        let mut debug_high=won_low.clone();debug_high.law=full_law;debug_high.debug=true;
        assert!(!crate::endless_unlocked(std::slice::from_ref(&debug_high)));
        let mut won_high=won_low.clone();won_high.law=full_law;
        assert!(crate::endless_unlocked(std::slice::from_ref(&won_high)));
    }
    #[test]
    fn endless_rates_and_multi_act_progression_work_past_act_three() {
        assert_eq!(endless_rates(1),None);
        assert_eq!(endless_rates(2),None);
        assert_eq!(endless_rates(3),None);
        let (hp4,atk4)=endless_rates(4).unwrap();
        assert!((hp4-1.30).abs()<1e-5 && (atk4-1.15).abs()<1e-5);
        let (hp6,atk6)=endless_rates(6).unwrap();
        assert!(hp6>hp4 && atk6>atk4);

        let mut run=Run::new(42,Class::Samurai);
        run.enable_contracts();
        run.set_endless(true).unwrap();
        run.validate().unwrap();
        for expected_act in 1..=30u8 {
            assert_eq!(run.act,expected_act);
            if run.contract_pending() {
                let seed=run.reward_seed();
                let class=run.class;
                let c=run.contract.as_mut().unwrap();
                if c.options()>0 {
                    c.choose(0,seed,class).unwrap();
                    if c.deepened {c.finish_deepen().unwrap();}
                    else {c.skip_gift().unwrap();}
                } else {
                    c.skip().unwrap();
                }
            }
            let _theme=crate::themes::for_run(&run);
            assert!(crate::party_game::act_seed(run.initial_seed.unwrap_or(run.seed),run.act).is_ok());
            assert!(run.physick_supply.queue(run.act,0));
            run.physick_supply.complete();
            let boss=run.nodes.iter().position(|n|n.kind==NodeKind::Boss).unwrap();
            run.debug_jump(boss).unwrap();
            run.enter(boss).unwrap();
            run.win_combat().unwrap();
            assert!(matches!(run.phase,crate::Phase::Reward{id,..} if id==boss));
            run.claim_reward(0).unwrap();
            assert_eq!(run.phase,crate::Phase::Map);
            assert_eq!(run.act,expected_act+1);
            let restored=Run::decode(&run.encode().unwrap()).unwrap();
            assert_eq!(restored,run);
        }
        // 30 幕后所有誓约已满级，契约自动完成不卡流程
        assert!(!run.contract_pending());
        let rec=crate::journal::Record::new(&run,vec![],vec![],0);
        assert!(rec.endless && rec.act==31);
        let score=crate::score::Score::of(&rec);
        assert_eq!(score.rank(),'S');
        assert_eq!(score.bosses,30);
        assert!(score.floors>=30*crate::score::FLOORS);
    }
}
