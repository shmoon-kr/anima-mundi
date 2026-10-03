//! Experience and levels (MECHANICS §9): gains and losses, a kill's share, what a level gives.

use crate::*;

impl Sim {
    /// One level gained, at the level the character now has (class.c advance_level).
    pub(crate) fn advance_level(&mut self, k: Key) {
        let c = self.chars.get(k).unwrap();
        let (class, level, con, wis) = (c.class.unwrap_or(Class::Warrior), c.level, c.abilities.con, c.abilities.wis);
        let ab = &self.tables.abilities;
        let at = |v: &Vec<i32>, i: i32| v.get(i.clamp(0, v.len() as i32 - 1) as usize).copied().unwrap_or(0);
        let con_hitp = at(&ab.constitution_hitp, con);
        let wis_bonus = at(&ab.wisdom_practices, wis);
        let (hp_roll, mana, moves) = match class {
            Class::MagicUser | Class::Cleric => {
                let hp = if class == Class::MagicUser { self.rand(3, 8) } else { self.rand(5, 10) };
                let mana = self.rand(level as i64, (1.5 * level as f64) as i64).min(10);
                (hp, mana, self.rand(0, 2))
            }
            Class::Thief => (self.rand(7, 13), 0, self.rand(1, 3)),
            Class::Warrior => (self.rand(10, 15), 0, self.rand(1, 3)),
        };
        let practices = match class {
            Class::MagicUser | Class::Cleric => wis_bonus.max(2),
            _ => wis_bonus.clamp(1, 2),
        };
        let c = self.chars.get_mut(k).unwrap();
        c.max_hp += (con_hitp + hp_roll as i32).max(1);
        if level > 1 {
            c.max_mana += mana as i32;
        }
        c.max_mv += (moves as i32).max(1);
        c.practices += practices;
    }

    /// limits.c gain_exp (MECHANICS §9.1): capped gains raise levels up to 30, losses floor at 0.
    pub(crate) fn gain_exp(&mut self, k: Key, gain: i64) {
        let cfg = self.tables.world.config.clone();
        let Some(c) = self.chars.get_mut(k) else { return };
        if c.is_mob() {
            c.exp += gain;
            return;
        }
        if c.level < 1 || c.level >= 31 {
            return;
        }
        if gain > 0 {
            c.exp += gain.min(cfg.max_exp_gain);
            let class = c.class.unwrap_or(Class::Warrior);
            let mut levels = 0;
            loop {
                let c = self.chars.get(k).unwrap();
                let next = self.tables.classes.get(class.key()).and_then(|t| t.level_exp.get(c.level as usize + 1).copied());
                match next {
                    Some(need) if c.level < 30 && c.exp >= need => {
                        self.chars.get_mut(k).unwrap().level += 1;
                        self.advance_level(k);
                        levels += 1;
                    }
                    _ => break,
                }
            }
            if levels > 0 {
                self.deliver(k, Event::LevelUp { levels });
            }
        } else if gain < 0 {
            let c = self.chars.get_mut(k).unwrap();
            c.exp = (c.exp + gain.max(-cfg.max_exp_loss)).max(0);
        }
    }

    /// A kill's experience (fight.c solo_gain, MECHANICS §9.2), from the victim's experience before
    /// death halves it. Groups come with step 5.
    pub(crate) fn kill_gain(&mut self, k: Key, vexp: i64, vlevel: i32, valign: i32) {
        let Some(c) = self.chars.get(k) else { return };
        let cap = self.tables.world.config.max_exp_gain;
        let mut exp = (vexp / 3).min(cap);
        let most = if c.is_mob() { 4 } else { 8 };
        exp += (exp * (vlevel - c.level).min(most) as i64 / 8).max(0);
        exp = exp.max(1);
        let is_mob = c.is_mob();
        if !is_mob {
            self.deliver(k, Event::ExpGain { amount: exp, kind: "solo".into() });
        }
        self.gain_exp(k, exp);
        let c = self.chars.get_mut(k).unwrap();
        c.alignment += (-valign - c.alignment) / 16;
    }

}
