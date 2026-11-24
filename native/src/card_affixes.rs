use eldenring::cs::{ItemCategory, PlayerGameData};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs::OpenOptions,
    io::{ErrorKind, Write},
    sync::{LazyLock, Mutex},
};

pub type Identity = (u32, u32);
type Owner = (u32, i32);
type Key = (u32, u32);

const MAX_DROPPED_PER_VARIANT: usize = 32;

#[derive(Default)]
struct Bindings {
    owner: Option<Owner>,
    seeds: HashMap<Key, Identity>,
    /// 上一帧背包+仓库中所有动态词条装备的 `(variant, sort_id) -> Identity` 快照。
    seen: Option<HashMap<Key, Identity>>,
    /// 离开背包但未被出售/重铸消费的装备身份队列，用于丢弃重捡、进出木箱或重载改 `sort_id` 时保词条。
    dropped: HashMap<u32, VecDeque<Identity>>,
    /// 当前商店节点的货单预览词条映射：`variant -> Identity`。
    shop_offers: HashMap<u32, Identity>,
}

static BINDINGS: LazyLock<Mutex<Bindings>> = LazyLock::new(Default::default);

pub(crate) fn variant(id: u32) -> u32 {
    if id >> 28 == 0 { id / 10000 * 10000 } else { id }
}

pub(crate) fn is_dynamic(id: u32) -> bool {
    let v = variant(id);
    if v >> 28 == 0 {
        erdtree_tower::weapon_rolls::slot_count(v).is_some()
    } else {
        erdtree_tower::talisman_passives::variant(v).is_some()
    }
}

fn parse(text: &str) -> Result<HashMap<Key, Identity>, String> {
    let mut seeds = HashMap::new();
    for line in text.lines().filter(|l| !l.is_empty()) {
        let row: Vec<_> = line
            .split_whitespace()
            .map(str::parse::<u32>)
            .collect::<Result<_, _>>()
            .map_err(|_| "词条存档格式错误")?;
        let [id, sort, a, b] = row[..] else {
            return Err("词条存档记录不完整".into());
        };
        seeds.insert((id, sort), (a, b));
    }
    Ok(seeds)
}

fn persist(owner: Owner, id: u32, sort: u32, (a, b): Identity) {
    let path = super::hub::path(owner, "affix-rolls");
    let result = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| writeln!(file, "{id} {sort} {a} {b}").and_then(|_| file.sync_data()));
    if let Err(e) = result {
        super::log(format!("CARD_AFFIX persistence failed item={id} sort={sort}: {e}"));
    }
}

pub fn select(owner: Owner) -> Result<(), String> {
    let mut state = BINDINGS.lock().map_err(|_| "词条存档锁不可用")?;
    if state.owner == Some(owner) {
        return Ok(());
    }
    let path = super::hub::path(owner, "affix-rolls");
    let seeds = match std::fs::read_to_string(path) {
        Ok(text) => parse(&text)?,
        Err(e) if e.kind() == ErrorKind::NotFound => HashMap::new(),
        Err(e) => return Err(format!("读取词条存档失败：{e}")),
    };
    *state = Bindings {
        owner: Some(owner),
        seeds,
        seen: None,
        dropped: HashMap::new(),
        shop_offers: HashMap::new(),
    };
    Ok(())
}

/// 开新局重置角色时清空上一局的词条绑定与暂存队列，避免跨局污染。
pub fn reset(owner: Owner) -> Result<(), String> {
    let path = super::hub::path(owner, "affix-rolls");
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("清理旧词条存档失败：{e}"))?;
    }
    let mut state = BINDINGS.lock().map_err(|_| "词条存档锁不可用")?;
    if state.owner == Some(owner) {
        state.seeds.clear();
        state.seen = None;
        state.dropped.clear();
        state.shop_offers.clear();
    }
    Ok(())
}

pub fn identity(character: u32, item: u32, sort: u32) -> Identity {
    BINDINGS
        .lock()
        .ok()
        .and_then(|state| {
            state
                .owner
                .filter(|o| o.0 == character)
                .and_then(|_| state.seeds.get(&(variant(item), sort)).copied())
        })
        .unwrap_or((character, sort))
}

pub fn set_shop_offers(offers: HashMap<u32, Identity>) {
    if let Ok(mut state) = BINDINGS.lock() {
        state.shop_offers = offers;
    }
}

pub fn clear_shop_offers() {
    if let Ok(mut state) = BINDINGS.lock() {
        state.shop_offers.clear();
    }
}

pub fn shop_offer(item: u32) -> Option<Identity> {
    let key = variant(item);
    BINDINGS
        .lock()
        .ok()
        .and_then(|state| state.shop_offers.get(&key).copied())
}

fn copies(data: &PlayerGameData, id: u32) -> HashSet<u32> {
    std::iter::once(&data.equipment.equip_inventory_data)
        .chain(data.storage.iter().map(|s| &**s))
        .flat_map(|inv| inv.items_data.normal_entries().iter().filter_map(|e| e.as_option()))
        .filter(|e| e.quantity > 0 && variant(e.item_id.into_inner()) == id)
        .map(|e| e.sort_id)
        .collect()
}

fn dynamic_inventory(data: &PlayerGameData) -> HashSet<Key> {
    std::iter::once(&data.equipment.equip_inventory_data)
        .chain(data.storage.iter().map(|s| &**s))
        .flat_map(|inv| inv.items_data.normal_entries().iter().filter_map(|e| e.as_option()))
        .filter(|e| {
            e.quantity > 0
                && matches!(e.item_id.category(), ItemCategory::Weapon | ItemCategory::Accessory)
                && is_dynamic(e.item_id.into_inner())
        })
        .map(|e| (variant(e.item_id.into_inner()), e.sort_id))
        .collect()
}

/// 发放装备并绑定预设词条身份；若 `seed` 为 `None`，仍记录新入包的 `sort_id`，防止被历史丢弃队列误覆盖。
pub fn grant(
    data: &mut PlayerGameData,
    item: u32,
    seed: Option<Identity>,
    give: impl FnOnce(&mut PlayerGameData) -> Result<(), String>,
) -> Result<(), String> {
    let id = variant(item);
    if !is_dynamic(id) {
        return give(data);
    }
    let owner = if seed.is_some() {
        Some(
            BINDINGS
                .lock()
                .map_err(|_| "词条存档锁不可用")?
                .owner
                .filter(|o| o.0 == data.character_id)
                .ok_or("角色词条存档尚未绑定")?,
        )
    } else {
        BINDINGS
            .lock()
            .ok()
            .and_then(|s| s.owner.filter(|o| o.0 == data.character_id))
    };
    let mut file = if let Some(owner) = owner.filter(|_| seed.is_some()) {
        Some(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(super::hub::path(owner, "affix-rolls"))
                .map_err(|e| format!("词条存档不可写：{e}"))?,
        )
    } else {
        None
    };
    let before = copies(data, id);
    give(data)?;
    let after = copies(data, id);
    let mut added = after.difference(&before).copied();
    if let Some(sort) = added.next().filter(|_| added.next().is_none()) {
        let mut state = BINDINGS.lock().unwrap_or_else(|p| p.into_inner());
        let resolved = if let Some((a, b)) = seed {
            state.seeds.insert((id, sort), (a, b));
            if let Some(mut file) = file.take() {
                if let Err(e) = writeln!(file, "{id} {sort} {a} {b}").and_then(|_| file.sync_data()) {
                    super::log(format!("CARD_AFFIX persistence failed item={id} sort={sort}: {e}"));
                }
            }
            (a, b)
        } else {
            (data.character_id, sort)
        };
        if let Some(seen) = state.seen.as_mut() {
            seen.insert((id, sort), resolved);
        }
    } else if seed.is_some() {
        super::log(format!("CARD_AFFIX granted item={id}, new inventory entry ambiguous"));
    }
    Ok(())
}

/// 主动销毁旧装备（如升级替换、重铸替换）后将其从快照移除，避免进入丢弃保留队列。
pub fn forget(item: u32, sort: u32) {
    let key = (variant(item), sort);
    if let Ok(mut state) = BINDINGS.lock() {
        if let Some(seen) = state.seen.as_mut() {
            seen.remove(&key);
        }
    }
}

/// 纯逻辑对账：比较前后两帧的动态词条装备集合，返回本帧新绑定的 `(variant, sort_id, identity, reason)`。
fn reconcile(
    character: u32,
    current: &HashSet<Key>,
    shop_open: bool,
    sell_open: bool,
    seeds: &mut HashMap<Key, Identity>,
    seen: &mut Option<HashMap<Key, Identity>>,
    dropped: &mut HashMap<u32, VecDeque<Identity>>,
    shop_offers: &HashMap<u32, Identity>,
) -> Vec<(u32, u32, Identity, &'static str)> {
    let Some(prev) = seen.as_mut() else {
        let initial = current
            .iter()
            .copied()
            .map(|(v, sort)| {
                let id = seeds.get(&(v, sort)).copied().unwrap_or((character, sort));
                ((v, sort), id)
            })
            .collect();
        *seen = Some(initial);
        return Vec::new();
    };

    let mut removed: Vec<(Key, Identity)> = prev
        .iter()
        .filter(|(k, _)| !current.contains(*k))
        .map(|(&k, &id)| (k, id))
        .collect();
    removed.sort_unstable_by_key(|((v, sort), _)| (*v, *sort));

    let mut added: Vec<Key> = current
        .iter()
        .copied()
        .filter(|k| !prev.contains_key(k))
        .collect();
    added.sort_unstable();

    if removed.is_empty() && added.is_empty() {
        return Vec::new();
    }

    let mut same_tick_variants = HashSet::new();
    for &(key, id) in &removed {
        prev.remove(&key);
        if !sell_open {
            same_tick_variants.insert(key.0);
            let queue = dropped.entry(key.0).or_default();
            if queue.len() >= MAX_DROPPED_PER_VARIANT {
                queue.pop_front();
            }
            queue.push_back(id);
        }
    }

    let mut bound = Vec::new();
    for (v, sort) in added {
        if let Some(&existing) = seeds.get(&(v, sort)) {
            prev.insert((v, sort), existing);
            continue;
        }
        if shop_open && let Some(&offer) = shop_offers.get(&v) {
            seeds.insert((v, sort), offer);
            prev.insert((v, sort), offer);
            bound.push((v, sort, offer, "shop"));
            continue;
        }
        if let Some(queue) = dropped.get_mut(&v) && let Some(saved) = queue.pop_back() {
            if queue.is_empty() {
                dropped.remove(&v);
            }
            seeds.insert((v, sort), saved);
            prev.insert((v, sort), saved);
            let reason = if same_tick_variants.contains(&v) {
                "sort_shifted"
            } else {
                "pickup"
            };
            bound.push((v, sort, saved, reason));
            continue;
        }
        prev.insert((v, sort), (character, sort));
    }

    bound
}

/// 每帧对账：
/// 1. 在商店购买界面买入的武器/品质护符自动绑定商店预览词条；
/// 2. 丢弃重捡、进出木箱或联机切图重载导致的 `sort_id` 变化自动延续原词条身份。
pub fn observe(data: &PlayerGameData, shop_open: bool, sell_open: bool) {
    let current = dynamic_inventory(data);
    let (owner, bound) = {
        let Ok(mut state) = BINDINGS.lock() else { return };
        let Some(owner) = state.owner.filter(|o| o.0 == data.character_id) else {
            return;
        };
        let Bindings {
            seeds,
            seen,
            dropped,
            shop_offers,
            ..
        } = &mut *state;
        let bound = reconcile(
            data.character_id,
            &current,
            shop_open,
            sell_open,
            seeds,
            seen,
            dropped,
            shop_offers,
        );
        (owner, bound)
    };
    for (v, sort, id, reason) in bound {
        persist(owner, v, sort, id);
        super::log(format!(
            "CARD_AFFIX bound({reason}) item={v} sort={sort} identity=({}, {})",
            id.0, id.1
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_card_identity_survives_reload_and_reinforcement() {
        let run = erdtree_tower::Run::new_personal(19, erdtree_tower::Class::Samurai, 25);
        let restored = erdtree_tower::Run::decode(&run.encode().unwrap()).unwrap();
        for item in [160030000, 537970914] {
            let card = |r, slot| erdtree_tower::weapon_rolls::card_identity(r, 3, slot, item).unwrap();
            let (a, b) = card(&run, 0);
            assert_eq!(card(&restored, 0), (a, b));
            assert_ne!(card(&run, 1), (a, b));
            let entries = parse(&format!("{item} 2598 {a} {b}\n")).unwrap();
            assert_eq!(
                entries.get(&(variant(if item >> 28 == 0 { item + 1225 } else { item }), 2598)),
                Some(&(a, b))
            );
            assert!(!entries.contains_key(&(item, 2599)));
        }
    }

    #[test]
    fn drop_and_pickup_preserves_original_affix_identity_without_rerolling() {
        let item = 537971065;
        let bound_item = 537971294;
        let card_id = (1522397528, 1685201849);
        let mut seeds = HashMap::from([((bound_item, 2970), card_id)]);
        let mut seen = None;
        let mut dropped = HashMap::new();
        let shop = HashMap::new();

        // Initial snapshot: one unbound item (sort=2968) and one card-bound item (sort=2970).
        let inv0 = HashSet::from([(item, 2968), (bound_item, 2970)]);
        assert!(reconcile(0, &inv0, false, false, &mut seeds, &mut seen, &mut dropped, &shop).is_empty());

        // Drop both items onto the ground.
        let empty = HashSet::new();
        assert!(reconcile(0, &empty, false, false, &mut seeds, &mut seen, &mut dropped, &shop).is_empty());
        assert_eq!(dropped.get(&item).map(VecDeque::len), Some(1));
        assert_eq!(dropped.get(&bound_item).map(VecDeque::len), Some(1));

        // Pick both back up with brand-new sort_ids -> exact original identities restored!
        let inv1 = HashSet::from([(item, 2975), (bound_item, 2976)]);
        let bound = reconcile(0, &inv1, false, false, &mut seeds, &mut seen, &mut dropped, &shop);
        assert_eq!(bound.len(), 2);
        assert_eq!(seeds.get(&(item, 2975)), Some(&(0, 2968)));
        assert_eq!(seeds.get(&(bound_item, 2976)), Some(&card_id));
        assert!(dropped.is_empty());

        // Drop and pick up a second time -> STILL preserves the original identities!
        reconcile(0, &empty, false, false, &mut seeds, &mut seen, &mut dropped, &shop);
        let inv2 = HashSet::from([(item, 2977), (bound_item, 2978)]);
        reconcile(0, &inv2, false, false, &mut seeds, &mut seen, &mut dropped, &shop);
        assert_eq!(seeds.get(&(item, 2977)), Some(&(0, 2968)));
        assert_eq!(seeds.get(&(bound_item, 2978)), Some(&card_id));
    }

    #[test]
    fn same_frame_sort_id_shift_preserves_identity_and_sell_does_not_stash() {
        let weapon = 194070000;
        let card_id = (111, 222);
        let mut seeds = HashMap::from([((weapon, 100), card_id)]);
        let mut seen = None;
        let mut dropped = HashMap::new();
        let shop = HashMap::new();

        reconcile(0, &HashSet::from([(weapon, 100)]), false, false, &mut seeds, &mut seen, &mut dropped, &shop);
        // Same-tick sort_id change (e.g., reload / storage move) preserves card_id.
        let shifted = reconcile(0, &HashSet::from([(weapon, 300)]), false, false, &mut seeds, &mut seen, &mut dropped, &shop);
        assert_eq!(shifted, vec![(weapon, 300, card_id, "sort_shifted")]);
        assert_eq!(seeds.get(&(weapon, 300)), Some(&card_id));

        // Selling the weapon removes it without leaving a ghost in dropped.
        reconcile(0, &HashSet::new(), false, true, &mut seeds, &mut seen, &mut dropped, &shop);
        assert!(dropped.is_empty());
    }

    #[test]
    fn shop_purchase_binds_preview_identity_even_when_already_owning_same_variant() {
        let weapon = 194070000;
        let owned_id = (10, 20);
        let shop_id = (30, 40);
        let mut seeds = HashMap::from([((weapon, 100), owned_id)]);
        let mut seen = None;
        let mut dropped = HashMap::new();
        let shop = HashMap::from([(weapon, shop_id)]);

        reconcile(0, &HashSet::from([(weapon, 100)]), false, false, &mut seeds, &mut seen, &mut dropped, &shop);
        // Buy a second copy while shop menu is open.
        let bought = reconcile(
            0,
            &HashSet::from([(weapon, 100), (weapon, 250)]),
            true,
            false,
            &mut seeds,
            &mut seen,
            &mut dropped,
            &shop,
        );
        assert_eq!(bought, vec![(weapon, 250, shop_id, "shop")]);
        assert_eq!(seeds.get(&(weapon, 100)), Some(&owned_id));
        assert_eq!(seeds.get(&(weapon, 250)), Some(&shop_id));
    }
}
