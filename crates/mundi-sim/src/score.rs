//! `score` (act.informative.c do_score, MECHANICS §18): the values; the renderer says them.

use mundi_content::names::Affect;
use mundi_protocol::{Score, Sex};

use crate::*;

/// LVL_IMMORT: from here there is no next level to need experience for.
const LVL_IMMORT: i32 = 31;
/// spells.h SPELL_ARMOR: "You feel protected."
const SPELL_ARMOR: i32 = 1;

impl Sim {
    pub(crate) fn score(&mut self, k: Key) {
        let Some(c) = self.chars.get(k) else { return };
        if c.is_mob() {
            return;
        }
        let class = c.class.unwrap_or(Class::Warrior);
        let table = self.tables.classes.get(class.key());
        let title = table
            .map(|t| if c.sex == Sex::Female { &t.titles.female } else { &t.titles.male })
            .and_then(|titles| titles.get(c.level.clamp(0, titles.len() as i32 - 1) as usize).cloned())
            .unwrap_or_default();
        let exp_to_next = (c.level < LVL_IMMORT)
            .then(|| table.and_then(|t| t.level_exp.get(c.level as usize + 1).copied()).map(|need| need - c.exp))
            .flatten();
        // utils.c mud_time_passed: hours are ticks, 24 a day, 35 days a month; real_time_passed: seconds.
        let lived = self.tick as i64 - c.born;
        let hours = lived / PULSES_PER_TICK as i64;
        let birthday = (hours / 24) % 35 == 0 && (hours / 24 / 35) % 17 == 0;
        let secs = lived / PULSES_PER_SEC as i64;
        let fighting = (c.position == Position::Fighting).then(|| match c.fighting {
            Some(f) => self.seen_name(k, f),
            None => ("thin air".to_string(), None),
        });
        let mut states = Vec::new();
        let flags = [
            (c.conditions.drunk > 10, "intoxicated"),
            (c.conditions.full == 0, "hungry"),
            (c.conditions.thirst == 0, "thirsty"),
            (c.has(Affect::Blind) && c.level < LVL_IMMORT, "blind"),
            (c.has(Affect::Invisible), "invisible"),
            (c.has(Affect::DetectInvis), "detect_invisible"),
            (c.has(Affect::Sanctuary), "sanctuary"),
            (c.has(Affect::Poison), "poisoned"),
            (c.has(Affect::Charm), "charmed"),
            (c.spells.iter().any(|s| s.spell == SPELL_ARMOR), "armored"),
            (c.has(Affect::Infravision), "infravision"),
        ];
        for (on, name) in flags {
            if on {
                states.push(name.to_string());
            }
        }
        let score = Score {
            age: self.age(k),
            birthday,
            hp: c.hp,
            hp_max: c.max_hp,
            mp: c.mana,
            mp_max: c.max_mana,
            mv: c.mv,
            mv_max: c.max_mv,
            ac: self.armor_class(k),
            alignment: c.alignment,
            exp: c.exp,
            gold: c.gold,
            quest_points: 0,
            exp_to_next,
            quests: 0,
            played_days: secs / 86400,
            played_hours: (secs / 3600) % 24,
            name: c.name.clone(),
            title,
            level: c.level,
            position: c.position,
            fighting: fighting.as_ref().map(|f| f.0.clone()),
            fighting_id: fighting.and_then(|f| f.1),
            states,
        };
        let (hp, mp, mv) = (c.max_hp, c.max_mana, c.max_mv);
        self.deliver(k, Event::VitalsMax { hp, mp, mv });
        self.deliver(k, Event::Score(Box::new(score)));
    }
}
