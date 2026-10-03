//! Levels (MECHANICS §9.4): what a level gives.

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
}
