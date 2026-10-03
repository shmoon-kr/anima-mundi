//! The tick: game time, regeneration (MECHANICS §1.2, §5), and the prompt that closes each block.

use crate::combat::{TYPE_SUFFERING, SPELL_POISON};
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

    /// The tick's work (MECHANICS §5.3): for characters hunger, drink and thirst; regeneration and
    /// poison for those stunned or better, bleeding for those worse; a player's light burning down;
    /// then corpses rot (§8.4).
    /// magic.c affect_update (MECHANICS §11.4): a tick off every spell; one at 0 goes the tick after,
    /// with its wear-off line if it is the spell's last affect.
    pub(crate) fn affect_update(&mut self) {
        let all: Vec<Key> = self.order.iter().chain(self.mobs.iter()).copied().collect();
        for k in all {
            let Some(c) = self.chars.get_mut(k) else { continue };
            let mut gone = Vec::new();
            let mut i = 0;
            while i < c.spells.len() {
                let a = &mut c.spells[i];
                if a.duration >= 1 {
                    a.duration -= 1;
                    i += 1;
                } else if a.duration == -1 {
                    i += 1;
                } else {
                    let spell = a.spell;
                    let last = c.spells.get(i + 1).is_none_or(|n| n.spell != spell || n.duration > 0);
                    c.spells.remove(i);
                    if last {
                        gone.push(spell);
                    }
                }
            }
            for spell in gone {
                self.deliver(k, Event::WoreOff { spell, name: self.spell_name(spell) });
            }
        }
    }

    pub(crate) fn spell_name(&self, n: i32) -> String {
        self.tables.spells.iter().find(|s| s.number == n).map(|s| s.name.clone()).unwrap_or_default()
    }

    pub(crate) fn point_update(&mut self) {
        let all: Vec<Key> = self.order.iter().chain(self.mobs.iter()).copied().collect();
        for k in all {
            self.conditions_tick(k);
            let Some(c) = self.chars.get(k) else { continue };
            if c.position >= Position::Stunned {
                let (hp, mana, mv) = (self.gain(k, Gain::Hit), self.gain(k, Gain::Mana), self.gain(k, Gain::Move));
                let c = self.chars.get_mut(k).unwrap();
                c.hp = (c.hp + hp).min(c.max_hp);
                c.mana = (c.mana + mana).min(c.max_mana);
                c.mv = (c.mv + mv).min(c.max_mv);
                if c.has(mundi_content::names::Affect::Poison) && self.damage(k, k, 2, SPELL_POISON, None) == -1 {
                    continue;
                }
                if self.chars.get(k).is_some_and(|c| c.position <= Position::Stunned) {
                    self.update_pos(k);
                }
            } else if c.position == Position::Incapacitated {
                if self.damage(k, k, 1, TYPE_SUFFERING, None) == -1 {
                    continue;
                }
            } else if c.position == Position::MortallyWounded && self.damage(k, k, 2, TYPE_SUFFERING, None) == -1 {
                continue;
            }
            if self.chars.get(k).is_some_and(|c| !c.is_mob()) {
                self.light_tick(k);
            }
        }
        self.corpses_rot();
    }

    /// limits.c:436-460: a corpse's timer runs down; at 0 it goes, spilling what it holds.
    fn corpses_rot(&mut self) {
        for o in self.objs.keys() {
            let Some(obj) = self.objs.get_mut(o) else { continue };
            if !obj.values.corpse() || obj.timer <= 0 {
                continue;
            }
            obj.timer -= 1;
            if obj.timer > 0 {
                continue;
            }
            let (place, text) = (obj.place, obj.short.clone());
            let id = self.obj_id(o);
            let spill = match place {
                Place::Carried(c) | Place::Worn(c, _) => {
                    self.deliver(c, Event::Decayed { text, id, carried: true });
                    self.chars.get(c).map(|ch| Place::Room(ch.room)).unwrap_or(Place::Nowhere)
                }
                Place::Room(r) => {
                    for w in self.people[r].clone() {
                        self.deliver(w, Event::Decayed { text: text.clone(), id: id.clone(), carried: false });
                    }
                    Place::Room(r)
                }
                Place::In(c) => Place::In(c),
                Place::Nowhere => Place::Nowhere,
            };
            for inner in self.objs.get(o).map(|x| x.contents.clone()).unwrap_or_default() {
                self.put(inner, spill);
            }
            self.extract_obj(o);
        }
    }

    /// limits.c gain_condition, for hunger, drink and thirst in that order (MECHANICS §6.1).
    fn conditions_tick(&mut self, k: Key) {
        let Some(c) = self.chars.get_mut(k) else { return };
        if c.is_mob() {
            return;
        }
        let was_drunk = c.conditions.drunk > 0;
        let step = |v: &mut i32| -> bool {
            if *v == -1 {
                return false;
            }
            *v = (*v - 1).clamp(0, 24);
            *v == 0
        };
        let hungry = step(&mut c.conditions.full);
        let sober = step(&mut c.conditions.drunk) && was_drunk;
        let thirsty = step(&mut c.conditions.thirst);
        if hungry {
            self.deliver(k, Event::Condition { hungry: Some(true), thirsty: None, full: None, quenched: None, sober: None, drunk: None });
        }
        if sober {
            self.deliver(k, Event::Condition { hungry: None, thirsty: None, full: None, quenched: None, sober: Some(true), drunk: None });
        }
        if thirsty {
            self.deliver(k, Event::Condition { hungry: None, thirsty: Some(true), full: None, quenched: None, sober: None, drunk: None });
        }
    }

    /// Age in game years (MECHANICS §17.2).
    pub(crate) fn age(&self, k: Key) -> i32 {
        let lived = self.tick as i64 - self.chars.get(k).map_or(0, |c| c.born);
        const PULSES_PER_YEAR: i64 = 17 * 35 * 24 * PULSES_PER_TICK as i64;
        (lived / PULSES_PER_YEAR) as i32 + 17
    }

    /// limits.c hit_gain, mana_gain, move_gain (MECHANICS §5.2).
    fn gain(&self, k: Key, what: Gain) -> i32 {
        let c = self.chars.get(k).unwrap();
        let mut g = if c.is_mob() {
            c.level
        } else {
            let curves = &self.tables.world.regen;
            let age = self.age(k);
            let mut g = graf(age, match what {
                Gain::Hit => curves.hit,
                Gain::Mana => curves.mana,
                Gain::Move => curves.moves,
            });
            g = match (what, c.position) {
                (Gain::Mana, Position::Sleeping) => g * 2,
                (Gain::Mana, Position::Resting) => g + g / 2,
                (Gain::Mana, Position::Sitting) => g + g / 4,
                (_, Position::Sleeping) => g + g / 2,
                (_, Position::Resting) => g + g / 4,
                (_, Position::Sitting) => g + g / 8,
                _ => g,
            };
            let caster = matches!(c.class, Some(Class::MagicUser | Class::Cleric));
            match what {
                Gain::Hit if caster => g /= 2,
                Gain::Mana if caster => g *= 2,
                _ => {}
            }
            if c.conditions.full == 0 || c.conditions.thirst == 0 {
                g /= 4;
            }
            g
        };
        if c.has(mundi_content::names::Affect::Poison) {
            g /= 4;
        }
        g
    }

    /// handler.c update_char_objects: the light in the light slot loses an hour (MECHANICS §3.1).
    fn light_tick(&mut self, k: Key) {
        let Some(&o) = self.chars.get(k).and_then(|c| c.equipment.get(&mundi_content::names::EquipPos::Light)) else { return };
        let Some(obj) = self.objs.get_mut(o) else { return };
        if obj.kind != mundi_content::names::ItemType::Light {
            return;
        }
        let Some(h) = obj.values.hours.filter(|h| *h > 0) else { return };
        obj.values.hours = Some(h - 1);
        let room = self.chars.get(k).unwrap().room;
        match h - 1 {
            1 => {
                self.deliver(k, Event::LightFlicker { who: SELF.into(), who_id: None });
                self.to_room(k, room, false, |who, who_id| Event::LightFlicker { who, who_id });
            }
            0 => {
                self.deliver(k, Event::LightOut { who: SELF.into(), who_id: None });
                self.to_room(k, room, false, |who, who_id| Event::LightOut { who, who_id });
            }
            _ => {}
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

#[derive(Clone, Copy)]
enum Gain {
    Hit,
    Mana,
    Move,
}
