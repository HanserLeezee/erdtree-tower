use eldenring::cs::{ChrAsmArmStyle, ChrAsmHand, ChrInsExt, FieldInsHandle, PlayerIns};
use std::collections::HashMap;
use std::sync::LazyLock;

const FIRST: i32 = 9875000;
const ICE: u8 = 2;
const CAST: u8 = 3;
const CHARGE: u8 = 4;
const GUARD: u8 = 5;
const FULL: u8 = 6;
const LOW: u8 = 7;
const RECOVERY: u8 = 8;
const DAMAGE_WATCH: u8 = 9;
const HIT_MARK: i32 = FIRST + 10;
const STORM: i32 = FIRST + 11;

static TRAITS: LazyLock<HashMap<i32, (u16, u8)>> = LazyLock::new(|| {
    include_str!("../../scene-assets/weapon-traits.tsv").lines().map(|line| {
        let mut fields=line.split('\t');
        let mut next=||fields.next().expect("weapon trait columns");
        (next().parse().unwrap(), (next().parse().unwrap(),next().parse().unwrap()))
    }).collect()
});

fn rank(weapon:Option<(u16,u8)>, kind:u8)->Option<u8> {
    weapon.filter(|(mask,_)|mask & (1<<kind)!=0).map(|(_,tier)|tier)
}

fn weapon_trait(id: i32) -> Option<(u16,u8)> {
    TRAITS.get(&(id / 10000 * 10000)).copied()
}

#[derive(Default)]
pub struct State {
    owner: Option<FieldInsHandle>,
    guard_until: f64,
    guard_tier: u8,
    recovery_until: f64,
    ice_ready: f64,
    watch: bool,
    hp: i32,
    max_hp: i32,
}

impl State {
    pub fn sync(&mut self, player: &mut PlayerIns, now: f64) {
        let asm = &player.chr_asm;
        let get = |hand| {
            if asm.equipment.arm_style == ChrAsmArmStyle::EmptyHanded { return None; }
            weapon_trait(asm.equipment_param_ids[asm.equipment.active_weapon_slot(hand) as usize])
        };
        let hands = [get(ChrAsmHand::Left), get(ChrAsmHand::Right)];
        let attack = get(player.attack_reference_hand);
        let guard = get(player.guard_reference_hand);
        let chr = &mut player.chr_ins;
        let hp = chr.modules.data.hp;
        let max = chr.modules.data.max_hp;
        if self.owner != Some(chr.field_ins_handle) || hp <= 0 || self.hp <= 0 {
            *self = Self { owner: Some(chr.field_ins_handle), ..Self::default() };
        }
        let mut active = [false; 64];
        let (mut charging, mut guarded) = (false, false);
        for e in chr.special_effect.entries() {
            match e.param_id {
                100280 => charging = true,
                174 => guarded = true,
                id if (FIRST..FIRST + 64).contains(&id) => active[(id-FIRST) as usize] = true,
                _ => {}
            }
        }
        let alive = hp > 0 && max > 0 && !chr.chr_flags1c5.death_flag();
        let strongest = |kind| hands.iter().filter_map(|h|rank(*h,kind)).max().filter(|_|alive);
        let held = |kind| strongest(kind).is_some();
        if !held(GUARD) { self.guard_until = 0.0; }
        else if let Some(t)=rank(guard,GUARD).filter(|_|guarded) { self.guard_until = now + 2.0; self.guard_tier=t; }
        if !held(RECOVERY) { self.recovery_until = 0.0; }
        else if self.watch && !active[DAMAGE_WATCH as usize] && self.max_hp == max && hp < self.hp {
            self.recovery_until = now + 2.0;
        }
        if active[(HIT_MARK-FIRST) as usize] {
            chr.remove_speffect(HIT_MARK);
            if let Some(t)=rank(attack,ICE).filter(|_|alive && now>=self.ice_ready) {
                chr.apply_speffect(STORM, false);
                self.ice_ready = now + f64::from(6-t);
            }
        }
        for (kind, desired) in [
            (CAST, rank(attack,CAST)),
            (CHARGE, rank(attack,CHARGE).filter(|_|charging)),
            (GUARD, strongest(GUARD).filter(|_|now<self.guard_until).map(|t|t.min(self.guard_tier))),
            (FULL, strongest(FULL).filter(|_|hp==max)),
            (LOW, strongest(LOW).filter(|_|i64::from(hp)*10<=i64::from(max)*3)),
            (RECOVERY, strongest(RECOVERY).filter(|_|now<self.recovery_until)),
            (DAMAGE_WATCH, strongest(RECOVERY).map(|_|0)),
        ] {
            for tier in 0..4 {
                let offset=(tier*16+kind) as usize;
                let id=FIRST+offset as i32;
                let want=alive && desired==Some(tier);
                if want && !active[offset] { chr.apply_speffect(id,false); }
                else if !want && active[offset] { chr.remove_speffect(id); }
            }
        }
        self.watch = held(RECOVERY);
        self.hp = hp;
        self.max_hp = max;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reinforced_and_infused_variants_keep_their_trait() {
        for (&id, &kind) in TRAITS.iter() {
            assert_eq!(weapon_trait(id + 1200 + 25), Some(kind));
            assert!(kind.0 < (1 << 9) && kind.1 < 4);
        }
        assert!((0..=RECOVERY).all(|k| (0..4).all(|tier|TRAITS.values().any(|&v| rank(Some(v),k)==Some(tier)))));
        assert_eq!(weapon_trait(3_000_025), None);
    }
}
