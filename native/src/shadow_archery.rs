use eldenring::cs::{ChrAsmArmStyle, ChrAsmHand, FieldInsHandle, PlayerIns, WorldChrManDbgFlags};
use erdtree_tower::{Class, Phase, Run};
use fromsoftware_shared::FromStatic;

const RATE: f32 = 2.0;

fn shooting(anim: i32) -> bool {
    let set = anim / 1_000_000;
    let motion = anim % 1_000_000;
    matches!(set, 44 | 45 | 51 | 230 | 232)
        && matches!(motion, 33030..=33082 | 36000..=36025 | 36500..=36525 | 39700..=39720)
}

fn family(id: i32) -> u32 {
    u32::try_from(id).ok().map_or(0, |v| erdtree_tower::affixes::base(v/10000*10000)/1_000_000)
}

fn ammo_enabled(hands: [u32; 2], attack: u32) -> bool {
    if hands.contains(&44) { return matches!(attack, 40..=43); }
    hands.iter().any(|v| matches!(v,40..=43))
}

#[derive(Default)]
struct Speed { applied: Option<(f32, f32)> }
impl Speed {
    fn sync(&mut self, speed: &mut f32, firing: bool) {
        if let Some((before, written)) = self.applied {
            if !firing {
                if *speed == written { *speed = before; }
                self.applied = None;
            }
        } else if firing && speed.is_finite() && *speed > 0.0 {
            let before = *speed;
            *speed *= RATE;
            self.applied = Some((before, *speed));
        }
    }
}

#[derive(Default)]
pub struct State { owner: Option<(usize, FieldInsHandle)>, speed: Speed, owns_free_ammo: bool }
impl State {
    pub fn sync(&mut self, player: Option<&mut PlayerIns>, run: Option<&Run>) {
        let shadow = run.is_some_and(|r| r.class == Class::Samurai && !matches!(r.phase,Phase::Won|Phase::Dead));
        let mut free = false;
        if let Some(p) = player {
            let owner = (std::ptr::from_ref(&*p) as usize, p.chr_ins.field_ins_handle);
            if self.owner != Some(owner) { self.speed = Speed::default(); self.owner = Some(owner); }
            let asm = &p.chr_asm;
            let hand = |h| family(asm.equipment_param_ids[asm.equipment.active_weapon_slot(h) as usize]);
            let alive = shadow && p.chr_ins.modules.data.hp > 0 && !p.chr_ins.chr_flags1c5.death_flag()
                && asm.equipment.arm_style != ChrAsmArmStyle::EmptyHanded;
            let hands = [hand(ChrAsmHand::Left), hand(ChrAsmHand::Right)];
            free = alive && ammo_enabled(hands,hand(p.attack_reference_hand));
            let t = &p.chr_ins.modules.time_act;
            let firing = alive && hands.iter().any(|v| matches!(v,40..=42))
                && t.anim_queue.get(t.read_idx as usize).is_some_and(|a|shooting(a.anim_id));
            self.speed.sync(&mut p.chr_ins.modules.behavior.animation_speed, firing);
        }
        if free || self.owns_free_ammo {
            if let Ok(flags) = unsafe { WorldChrManDbgFlags::instance_mut() } {
                if free && !flags.all_no_arrow_consume {
                    flags.all_no_arrow_consume = true;
                    self.owns_free_ammo = true;
                } else if !free && self.owns_free_ammo {
                    flags.all_no_arrow_consume = false;
                    self.owns_free_ammo = false;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bow_speed_is_scoped_and_does_not_stack() {
        for set in [44,45,51] { assert!(shooting(set*1_000_000+36000)); }
        for anim in [46_037000,44_000000,44_031060,80720] {assert!(!shooting(anim));}
        let mut state=Speed::default(); let mut rate=0.8;
        state.sync(&mut rate,true);state.sync(&mut rate,true);assert_eq!(rate,1.6);
        state.sync(&mut rate,false);assert_eq!(rate,0.8);
        state.sync(&mut rate,true);rate=0.6;state.sync(&mut rate,false);assert_eq!(rate,0.6);
    }
    #[test]
    fn ammunition_excludes_ballista_and_melee() {
        for w in 40..=43 {assert!(ammo_enabled([0,w],w));}
        assert!(!ammo_enabled([1,3],3));
        assert!(!ammo_enabled([43,44],44));
        assert!(ammo_enabled([43,44],43));
    }
}
