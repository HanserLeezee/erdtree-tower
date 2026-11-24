use crate::{
    NodeKind, RuleResult, Run,
    embers::Ember,
    events::Draw,
    rewards::{CATALOG, ItemKind, SPIRIT_MAX, custom_spirit, spirit_base, spirit_level, spirit_level_now},
};

// ── 1. 余烬熔炉 ──
pub const EMBER_FIND_PRICE: u32 = 2000;

/// 余烬熔炉三选一：按种子与条件稳定抽出至多 3 个未持有余烬（排除刚销毁的余烬）。
pub fn ember_furnace_offers(
    run: &Run,
    node: usize,
    burned: Option<Ember>,
    spirits: bool,
    casting: bool,
) -> Vec<Ember> {
    let class = (run.ember_rules >= 3).then_some(run.class);
    let mut excluded = run.embers.clone();
    if let Some(b) = burned
        && !excluded.contains(&b)
    {
        excluded.push(b);
    }
    let mut pool: Vec<Ember> = Ember::ALL
        .iter()
        .copied()
        .filter(|&e| {
            e != Ember::Stride
                && !excluded.contains(&e)
                && e.fits_build(spirits, casting, class)
        })
        .collect();
    let mut seed = run.reward_seed()
        ^ (node as u64).wrapping_mul(0x9e37_79b9)
        ^ burned.map_or(0, |b| (b.index() as u64 + 1).wrapping_mul(0x517c_c1b7))
        ^ 0xe3be_f019;
    let mut pick = Vec::new();
    while pick.len() < 3 && !pool.is_empty() {
        seed = crate::mix(seed.wrapping_add(1));
        let idx = (seed as usize) % pool.len();
        pick.push(pool.swap_remove(idx));
    }
    pick
}

// ── 2. 骨灰融合炉 ──
pub const SPIRIT_FUSE_PRICE: u32 = 1000;
pub const SPIRIT_FIND_PRICE: u32 = 1000;
pub const SPIRIT_BOOST_PRICE: u32 = 1200;

pub fn spirit_devour_target(main_id: u32) -> Option<u32> {
    let level = spirit_level(main_id)?;
    if level >= SPIRIT_MAX {
        return None;
    }
    let next = level.saturating_add(3).min(SPIRIT_MAX);
    Some(spirit_base(main_id) + u32::from(next))
}

pub fn spirit_boost_target(main_id: u32) -> Option<u32> {
    let level = spirit_level(main_id)?;
    if level >= SPIRIT_MAX {
        return None;
    }
    let next = level.saturating_add(2).min(SPIRIT_MAX);
    Some(spirit_base(main_id) + u32::from(next))
}

/// 双灰融魂：投入两只骨灰，产出 1 只全新骨灰（优先定制/精英骨灰，排除已持有的基础骨灰），继承最高强化等级 +2。
pub fn spirit_fusion_result(
    run: &Run,
    node: usize,
    sacrifice_a: u32,
    sacrifice_b: u32,
    owned_bases: &[u32],
) -> Option<u32> {
    let la = spirit_level(sacrifice_a)?;
    let lb = spirit_level(sacrifice_b)?;
    let ba = spirit_base(sacrifice_a);
    let bb = spirit_base(sacrifice_b);
    let target_level = la.max(lb).saturating_add(2).min(SPIRIT_MAX);
    let salt = run.reward_seed()
        ^ (node as u64).wrapping_mul(7919)
        ^ (u64::from(ba) << 1)
        ^ (u64::from(bb) << 17)
        ^ 0x5f10_5101;
    let pick_from = |only_custom: bool, avoid_owned: bool| -> Option<u32> {
        let pool: Vec<u32> = CATALOG
            .iter()
            .filter(|i| {
                i.kind == ItemKind::Spirit
                    && i.random_eligible()
                    && i.id != ba
                    && i.id != bb
                    && (!only_custom || custom_spirit(i.id))
                    && (!avoid_owned || !owned_bases.contains(&i.id))
            })
            .map(|i| i.id)
            .collect();
        if pool.is_empty() {
            None
        } else {
            Some(pool[(crate::mix(salt) as usize) % pool.len()] + u32::from(target_level))
        }
    };
    pick_from(true, true)
        .or_else(|| pick_from(false, true))
        .or_else(|| pick_from(true, false))
        .or_else(|| pick_from(false, false))
}

/// 唤灵拾遗（空手/单灰兜底）：按当前 `spirit_level_now` 给出 1 只未持有的随机骨灰。
pub fn spirit_find_result(run: &Run, node: usize, owned_bases: &[u32]) -> Option<u32> {
    let target_level = spirit_level_now(run).min(SPIRIT_MAX);
    let salt = run.reward_seed() ^ (node as u64).wrapping_mul(7919) ^ 0x5f10_f11d;
    let pool: Vec<u32> = CATALOG
        .iter()
        .filter(|i| i.kind == ItemKind::Spirit && i.random_eligible() && !owned_bases.contains(&i.id))
        .map(|i| i.id)
        .collect();
    let pool = if pool.is_empty() {
        CATALOG
            .iter()
            .filter(|i| i.kind == ItemKind::Spirit && i.random_eligible())
            .map(|i| i.id)
            .collect()
    } else {
        pool
    };
    if pool.is_empty() {
        return None;
    }
    Some(pool[(crate::mix(salt) as usize) % pool.len()] + u32::from(target_level))
}

// ── 3. 癫火祭坛 ──
pub const FRENZY_BLOOD_GOLD: u32 = 2000;
pub const FRENZY_BRAND_GOLD: u32 = 3500;
pub const FRENZY_MIN_FLASKS: u8 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrenzyPact {
    /// 永久 -1 圣杯瓶上限 → 传说词条武器 + 随机高阶/被动护符
    BurnFlask,
    /// 当前生命 -80% → 余烬三选一 + 2000 卢恩
    BloodMadness,
    /// 下场战斗易伤 +35%（赢后解除）→ 3500 卢恩 + 传说装备
    FrenzyBrand,
}

pub fn frenzy_legendary_weapon(run: &Run, node: usize, salt: u64) -> Option<u32> {
    let pool: Vec<u32> = CATALOG
        .iter()
        .filter(|i| i.kind == ItemKind::Weapon && i.random_eligible())
        .map(|i| i.id)
        .collect();
    if pool.is_empty() {
        return None;
    }
    let roll = crate::mix(run.reward_seed() ^ (node as u64).wrapping_mul(7919) ^ salt);
    let base = pool[(roll as usize) % pool.len()];
    crate::affixes::roll_min(run, base, roll, true, 3).or(Some(base))
}

pub fn frenzy_talisman(run: &Run, node: usize, salt: u64) -> u32 {
    let roll = crate::mix(run.reward_seed() ^ (node as u64).wrapping_mul(7919) ^ salt);
    let pool: Vec<u32> = CATALOG
        .iter()
        .filter(|i| i.kind == ItemKind::Talisman && i.random_eligible())
        .map(|i| i.id)
        .collect();
    let base = pool[(roll as usize) % pool.len()];
    crate::talismans::roll(run, base, roll, true)
}

// ── 4. 「坚持 30 秒」大混战 ──
pub const SURVIVAL_SECONDS: u64 = 30;
pub const SURVIVAL_COUNT: usize = 8;
pub const SURVIVAL_BASE_GOLD: u32 = 1500;
pub const SURVIVAL_KILL_GOLD: u32 = 250;

pub fn survival_gold(kills: u8) -> u32 {
    SURVIVAL_BASE_GOLD + u32::from(kills.min(SURVIVAL_COUNT as u8)) * SURVIVAL_KILL_GOLD
}

pub fn survival_full_clear(kills: u8) -> bool {
    usize::from(kills) >= SURVIVAL_COUNT
}

pub fn survival_equipment(run: &Run, node: usize, full_clear: bool) -> Option<u32> {
    let min_tier = if full_clear {
        3
    } else if run.act >= 3 {
        3
    } else {
        2
    };
    let roll = crate::mix(run.reward_seed() ^ (node as u64).wrapping_mul(7919) ^ 0x5017_0a11 ^ u64::from(full_clear));
    let pool: Vec<u32> = CATALOG
        .iter()
        .filter(|i| i.kind == ItemKind::Weapon && i.random_eligible())
        .map(|i| i.id)
        .collect();
    if pool.is_empty() {
        return None;
    }
    let base = pool[(roll as usize) % pool.len()];
    crate::affixes::roll_min(run, base, roll, true, min_tier).or(Some(base))
}

impl Run {
    pub fn can_burn_flask(&self) -> bool {
        self.flask_limit > FRENZY_MIN_FLASKS
    }

    pub fn resolve_ember_furnace(&mut self, burned: Option<Ember>, picked: Ember, price: u32) -> RuleResult<()> {
        let id = self.active_node(NodeKind::Event)?;
        if self.event_draw(id) != Draw::EmberFurnace || self.regional_event.settled {
            return Err("当前不是可用的余烬熔炉事件".into());
        }
        if self.regional_event.opened != 0 {
            return Err("这里的余烬熔炉已经使用过".into());
        }
        if picked == Ember::Stride {
            return Err("无效的目标余烬".into());
        }
        if let Some(b) = burned {
            if !self.embers.contains(&b) {
                return Err("未持有要投入熔炉的余烬".into());
            }
            if b == picked {
                return Err("不能替换为同一枚余烬".into());
            }
        }
        if self.embers.contains(&picked) {
            return Err("已经持有该余烬".into());
        }
        if self.gold < price {
            return Err("卢恩不足，余烬熔炉未消耗".into());
        }
        let mut next = self.clone();
        next.gold -= price;
        if let Some(b) = burned {
            next.embers.retain(|&e| e != b);
            next.ember_stacks.clear_ember(b);
            if b == Ember::Battlemage {
                next.battlemage.clear();
            }
        }
        next.ember_rewards = true;
        next.embers.push(picked);
        next.regional_event.opened = 1;
        next.validate()?;
        *self = next;
        Ok(())
    }

    pub fn resolve_spirit_furnace(&mut self, price: u32, granted_id: u32) -> RuleResult<()> {
        let id = self.active_node(NodeKind::Event)?;
        if self.event_draw(id) != Draw::SpiritFurnace || self.regional_event.settled {
            return Err("当前不是可用的骨灰融合炉事件".into());
        }
        if self.regional_event.opened != 0 {
            return Err("这里的骨灰融合炉已经使用过".into());
        }
        let Some(level) = spirit_level(granted_id) else {
            return Err("融合目标不是有效的骨灰".into());
        };
        if self.gold < price {
            return Err("卢恩不足，骨灰融合炉未消耗".into());
        }
        let mut next = self.clone();
        next.gold -= price;
        next.spirit_peak = Some(next.spirit_peak.unwrap_or(0).max(level).min(SPIRIT_MAX));
        if !next.claimed_items.contains(&granted_id) {
            next.claimed_items.push(granted_id);
        }
        next.regional_event.opened = 1;
        next.validate()?;
        *self = next;
        Ok(())
    }

    pub fn resolve_frenzy_altar(
        &mut self,
        pact: FrenzyPact,
        ember_pick: Option<Ember>,
        items: &[u32],
    ) -> RuleResult<u32> {
        let id = self.active_node(NodeKind::Event)?;
        if self.event_draw(id) != Draw::FrenzyAltar || self.regional_event.settled {
            return Err("当前不是可用的癫火祭坛事件".into());
        }
        if self.regional_event.opened != 0 {
            return Err("这里的癫火祭坛已经完成契约".into());
        }
        let mut next = self.clone();
        let mut bonus_gold = 0u32;
        match pact {
            FrenzyPact::BurnFlask => {
                if !next.can_burn_flask() {
                    return Err("圣杯瓶上限已达保底，无法再献祭".into());
                }
                next.frenzy_flask_cut = next.frenzy_flask_cut.saturating_add(1);
                next.flask_limit = next.full_flask_limit();
                next.flasks = next.flasks.min(next.flask_limit);
                next.clamp_flask_blue();
            }
            FrenzyPact::BloodMadness => {
                next.hp = (next.hp / 5).max(1);
                if let Some(ember) = ember_pick {
                    if ember == Ember::Stride || next.embers.contains(&ember) {
                        return Err("无效或已持有的余烬".into());
                    }
                    next.ember_rewards = true;
                    next.embers.push(ember);
                }
                bonus_gold = FRENZY_BLOOD_GOLD;
                next.gold = next.gold.checked_add(bonus_gold).ok_or("卢恩数量超限")?;
            }
            FrenzyPact::FrenzyBrand => {
                next.frenzy_curse = true;
                bonus_gold = FRENZY_BRAND_GOLD;
                next.gold = next.gold.checked_add(bonus_gold).ok_or("卢恩数量超限")?;
            }
        }
        for &item in items {
            if !next.claimed_items.contains(&item) {
                next.claimed_items.push(item);
            }
        }
        next.regional_event.opened = 1;
        next.validate()?;
        *self = next;
        Ok(bonus_gold)
    }

    pub fn begin_survival(&mut self) -> RuleResult<()> {
        let id = self.active_node(NodeKind::Event)?;
        if self.event_draw(id) != Draw::Survival || self.regional_event.settled {
            return Err("当前不是可用的生存大混战事件".into());
        }
        if self.regional_event.opened != 0 || self.regional_event.guards {
            return Err("生存挑战已经开始或已完成".into());
        }
        self.regional_event.opened = 1;
        self.regional_event.guards = true;
        self.validate()?;
        Ok(())
    }

    pub fn skip_survival(&mut self) -> RuleResult<()> {
        let id = self.active_node(NodeKind::Event)?;
        if self.event_draw(id) != Draw::Survival || self.regional_event.settled {
            return Err("当前不是可用的生存大混战事件".into());
        }
        if self.regional_event.opened != 0 || self.regional_event.guards {
            return Err("生存挑战已经开始或已完成".into());
        }
        self.regional_event.opened = 3;
        self.validate()?;
        Ok(())
    }

    pub fn finish_survival(&mut self, kills: u8, items: &[u32]) -> RuleResult<u32> {
        let id = self.active_node(NodeKind::Event)?;
        if self.event_draw(id) != Draw::Survival {
            return Err("当前不是生存大混战事件".into());
        }
        if self.regional_event.opened != 1 || !self.regional_event.guards {
            return Err("生存挑战尚未开始或已结算".into());
        }
        let reward = survival_gold(kills);
        let mut next = self.clone();
        next.gold = next.gold.checked_add(reward).ok_or("卢恩数量超限")?;
        for &item in items {
            if !next.claimed_items.contains(&item) {
                next.claimed_items.push(item);
            }
        }
        next.regional_event.guards = false;
        next.regional_event.opened = 3;
        next.validate()?;
        *self = next;
        Ok(reward)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Class;

    fn run_at(draw: Draw, class: Class) -> (Run, usize) {
        let mut run = Run::new(77, class);
        let node = run.nodes.iter().position(|n| n.kind == NodeKind::Event).unwrap();
        run.debug_jump(node).unwrap();
        run.event_test = Some((run.act, node, draw));
        run.enter(node).unwrap();
        (run, node)
    }

    #[test]
    fn ember_furnace_swaps_clears_stacks_and_supports_empty_handed_find() {
        let (mut run, node) = run_at(Draw::EmberFurnace, Class::Samurai);
        run.gold = 5000;
        let find_offers = ember_furnace_offers(&run, node, None, false, false);
        assert_eq!(find_offers.len(), 3);
        let mut find_run = run.clone();
        find_run.resolve_ember_furnace(None, find_offers[0], EMBER_FIND_PRICE).unwrap();
        assert_eq!(find_run.gold, 3000);
        assert!(find_run.embers.contains(&find_offers[0]));
        assert!(find_run.resolve_ember_furnace(None, find_offers[1], EMBER_FIND_PRICE).is_err());

        run.embers.push(Ember::SoulEater);
        run.ember_stacks.souls = 12;
        let swap_offers = ember_furnace_offers(&run, node, Some(Ember::SoulEater), false, false);
        assert_eq!(swap_offers.len(), 3);
        assert!(!swap_offers.contains(&Ember::SoulEater));
        run.resolve_ember_furnace(Some(Ember::SoulEater), swap_offers[0], 0).unwrap();
        assert!(!run.embers.contains(&Ember::SoulEater));
        assert_eq!(run.ember_stacks.souls, 0);
        assert!(run.embers.contains(&swap_offers[0]));
        let loaded = Run::decode(&run.encode().unwrap()).unwrap();
        assert_eq!(loaded, run);
    }

    #[test]
    fn spirit_furnace_devour_fusion_and_find_preserve_caps_and_peak() {
        let (mut run, node) = run_at(Draw::SpiritFurnace, Class::Confessor);
        run.gold = 3000;
        let spirits: Vec<u32> = CATALOG.iter().filter(|i| i.kind == ItemKind::Spirit && i.random_eligible()).take(2).map(|i| i.id).collect();
        let a = spirits[0] + 4;
        let b = spirits[1] + 6;
        assert_eq!(spirit_devour_target(a), Some(spirits[0] + 7));
        assert_eq!(spirit_boost_target(a), Some(spirits[0] + 6));
        assert_eq!(spirit_devour_target(spirits[0] + u32::from(SPIRIT_MAX)), None);
        let fused = spirit_fusion_result(&run, node, a, b, &[spirits[0], spirits[1]]).unwrap();
        assert_eq!(spirit_level(fused), Some(8));
        assert_ne!(spirit_base(fused), spirits[0]);
        assert_ne!(spirit_base(fused), spirits[1]);
        run.resolve_spirit_furnace(SPIRIT_FUSE_PRICE, fused).unwrap();
        assert_eq!(run.gold, 2000);
        assert_eq!(run.spirit_peak, Some(8));
        assert!(run.resolve_spirit_furnace(0, fused).is_err());
        let found = spirit_find_result(&run, node, &[spirits[0]]).unwrap();
        assert!(spirit_level(found).is_some());
    }

    #[test]
    fn frenzy_altar_flask_cut_blood_and_curse_persist_and_clear_on_win() {
        let (run, node) = run_at(Draw::FrenzyAltar, Class::Vagabond);
        let before_limit = run.flask_limit;
        let mut burn = run.clone();
        let wep = frenzy_legendary_weapon(&burn, node, 1).unwrap();
        let tal = frenzy_talisman(&burn, node, 2);
        burn.resolve_frenzy_altar(FrenzyPact::BurnFlask, None, &[wep, tal]).unwrap();
        assert_eq!(burn.flask_limit, before_limit - 1);
        assert_eq!(burn.frenzy_flask_cut, 1);
        let loaded = Run::decode(&burn.encode().unwrap()).unwrap();
        assert_eq!(loaded.flask_limit, before_limit - 1);

        let mut blood = run.clone();
        let before_hp = blood.hp;
        let pick = ember_furnace_offers(&blood, node, None, false, false)[0];
        assert_eq!(blood.resolve_frenzy_altar(FrenzyPact::BloodMadness, Some(pick), &[]).unwrap(), FRENZY_BLOOD_GOLD);
        assert_eq!(blood.hp, (before_hp / 5).max(1));
        assert!(blood.embers.contains(&pick));

        let mut brand = run.clone();
        assert_eq!(brand.resolve_frenzy_altar(FrenzyPact::FrenzyBrand, None, &[wep]).unwrap(), FRENZY_BRAND_GOLD);
        assert!(brand.frenzy_curse);
        brand.leave_regional_event().unwrap();
        let fight_node = brand.nodes.iter().position(|n| n.kind == NodeKind::Monster).unwrap();
        brand.debug_jump(fight_node).unwrap();
        brand.enter(fight_node).unwrap();
        assert!(brand.frenzy_curse);
        brand.win_combat().unwrap();
        assert!(!brand.frenzy_curse);
    }

    #[test]
    fn survival_tracks_guards_gold_and_single_settlement() {
        let (mut run, node) = run_at(Draw::Survival, Class::Hero);
        assert_eq!(survival_gold(0), 1500);
        assert_eq!(survival_gold(8), 3500);
        assert!(survival_full_clear(8));
        assert!(!survival_full_clear(7));
        run.begin_survival().unwrap();
        assert!(run.regional_event.guards);
        assert!(run.leave_regional_event().is_err());
        let eq = survival_equipment(&run, node, true).unwrap();
        let reward = run.finish_survival(8, &[eq]).unwrap();
        assert_eq!(reward, 3500);
        assert!(!run.regional_event.guards);
        assert!(run.finish_survival(8, &[]).is_err());
        run.leave_regional_event().unwrap();
    }
}
