//! The tick: game time, regeneration (MECHANICS §1.2, §5), and the prompt that closes each block.

use crate::*;

impl Sim {
    pub(crate) fn game_hour(&mut self) {
        self.hour = (self.hour + 1) % 24;
        let phase = match self.hour {
            5 => DayPhase::Sunrise,
            6 => DayPhase::Day,
            21 => DayPhase::Sunset,
            22 => DayPhase::Night,
            _ => return,
        };
        for key in self.order.clone() {
            let c = self.chars.get(key).unwrap();
            if c.position > Position::Sleeping && !self.world.rooms[c.room].flags.contains(&RoomFlag::Indoors) {
                self.deliver(key, Event::WorldTime { phase });
            }
        }
    }

    /// MECHANICS §5.2 for a character of age 17 with no class yet; hunger, thirst and poison come with S5.
    pub(crate) fn regen(&mut self) {
        let curves = self.tables.world.regen.clone();
        for key in self.order.clone() {
            let c = self.chars.get_mut(key).unwrap();
            if c.position < Position::Stunned {
                continue;
            }
            let bonus = |base: i32, sleep: i32, rest: i32, sit: i32, pos: Position| match pos {
                Position::Sleeping => base + base / sleep,
                Position::Resting => base + base / rest,
                Position::Sitting => base + base / sit,
                _ => base,
            };
            let hp = bonus(graf(17, curves.hit), 2, 4, 8, c.position);
            let mv = bonus(graf(17, curves.moves), 2, 4, 8, c.position);
            let base_mana = graf(17, curves.mana);
            let mana = match c.position {
                Position::Sleeping => base_mana * 2,
                Position::Resting => base_mana + base_mana / 2,
                Position::Sitting => base_mana + base_mana / 4,
                _ => base_mana,
            };
            c.hp = (c.hp + hp).min(c.max_hp);
            c.mana = (c.mana + mana).min(c.max_mana);
            c.mv = (c.mv + mv).min(c.max_mv);
        }
    }

    /// The end of a block of output: everyone who got something this pulse gets their numbers.
    pub(crate) fn prompts(&mut self) {
        let mut seen = Vec::new();
        for d in &self.out {
            if !seen.contains(&d.to) {
                seen.push(d.to.clone());
            }
        }
        for name in seen {
            let Some(c) = self.by_name.get(&key_name(&name)).and_then(|k| self.chars.get(*k)) else { continue };
            let event = Event::Prompt { hp: Some(c.hp), mp: Some(c.mana), mv: Some(c.mv) };
            self.out.push(Delivery { tick: self.tick, to: name, event });
        }
    }
}

/// The age curve (MECHANICS §5.1).
pub(crate) fn graf(age: i32, p: [i32; 7]) -> i32 {
    match age {
        ..15 => p[0],
        15..=29 => p[1] + (age - 15) * (p[2] - p[1]) / 15,
        30..=44 => p[2] + (age - 30) * (p[3] - p[2]) / 15,
        45..=59 => p[3] + (age - 45) * (p[4] - p[3]) / 15,
        60..=79 => p[4] + (age - 60) * (p[5] - p[4]) / 20,
        _ => p[6],
    }
}
