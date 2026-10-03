//! General behaviours that tbaMUD writes as DG Script triggers (MECHANICS §14.3). The engine knows
//! only the behaviours (welcome a zone's arrivals, outfit newcomers, guard, eat corpses, pick up
//! litter, reward drops); which mob or room does it, what it hands out, what it says and its
//! numbers are content (`tables/triggers.yaml`). Lines go out with their key so readers in another
//! language get them translated. A script's `wait` is a scheduled step.

use mundi_content::names::{EquipPos, ItemType};
use mundi_content::tables::Trigger;

use crate::*;

/// The scripts' checks run every 13 seconds (dg_scripts.h PULSE_DG_SCRIPT).
pub(crate) const PULSE_SCRIPT: u64 = 13 * PULSES_PER_SEC;

#[derive(Debug, Clone)]
pub(crate) enum Scheduled {
    Outfit { mob: Key, actor: Key, trigger: String },
    Welcome { actor: Key, zone: u32, trigger: String },
}

/// A trigger's line: its English text with the names filled in, and the reference to it.
struct Line {
    text: String,
    line: LineRef,
}

impl Sim {
    fn triggers_of(&self, ids: &[String]) -> Vec<(String, Trigger)> {
        ids.iter().filter_map(|id| self.tables.triggers.get(id).map(|t| (id.clone(), t.clone()))).collect()
    }

    fn mob_triggers(&self, k: Key) -> Vec<(String, Trigger)> {
        let Some(m) = self.chars.get(k).and_then(|c| c.mob.as_ref()) else { return vec![] };
        let ids = self.world.mob_protos.get(&m.proto).map(|p| p.triggers.clone()).unwrap_or_default();
        self.triggers_of(&ids)
    }

    fn room_triggers(&self, room: RoomIx) -> Vec<(String, Trigger)> {
        self.triggers_of(&self.world.rooms[room].triggers)
    }

    /// A line of a trigger, its `%s` filled with these beings' names in order.
    fn line(&self, id: &str, t: &Trigger, key: &str, names: &[Key]) -> Line {
        let english = match key.strip_prefix("kit.").and_then(|i| i.parse::<usize>().ok()) {
            Some(i) => t.kit.get(i).and_then(|p| p.say.clone()),
            None => t.lines.get(key).cloned(),
        }
        .unwrap_or_default();
        let named: Vec<Named> = names
            .iter()
            .map(|k| Named { name: self.chars.get(*k).map(|c| c.name.clone()).unwrap_or_default(), id: self.id_of(*k) })
            .collect();
        let mut text = english;
        for n in &named {
            text = text.replacen("%s", &n.name, 1);
        }
        Line { text, line: LineRef { id: id.into(), key: key.into(), names: named } }
    }

    /// Steps whose wait is over, in the order they were scheduled.
    pub(crate) fn run_scheduled(&mut self) {
        let (due, later): (Vec<_>, Vec<_>) = std::mem::take(&mut self.scheduled).into_iter().partition(|(t, _)| *t <= self.tick);
        self.scheduled = later;
        for (_, s) in due {
            match s {
                Scheduled::Outfit { mob, actor, trigger } => self.outfit(mob, actor, &trigger),
                Scheduled::Welcome { actor, zone, trigger } => {
                    let Some(t) = self.tables.triggers.get(&trigger).cloned() else { continue };
                    let l = self.line(&trigger, &t, "welcome", &[actor]);
                    for k in self.order.clone() {
                        if self.chars.get(k).is_some_and(|c| self.world.rooms[c.room].zone == zone) {
                            self.deliver(k, Event::Echo { text: l.text.clone(), line: Some(l.line.clone()) });
                        }
                    }
                }
            }
        }
    }

    /// Login triggers of the room one enters the game in (interpreter.c:1318): zone_welcome.
    pub(crate) fn login_triggers(&mut self, k: Key) {
        let room = self.chars.get(k).unwrap().room;
        for (id, t) in self.room_triggers(room) {
            if t.kind == "zone_welcome" {
                let at = self.tick + t.delay.unwrap_or(0) as u64 * PULSES_PER_SEC;
                let zone = self.world.rooms[room].zone;
                self.scheduled.push((at, Scheduled::Welcome { actor: k, zone, trigger: id }));
            }
        }
    }

    /// Greet triggers of the mobs in a room someone comes into (dg_triggers.c greet_mtrigger): awake,
    /// not fighting, seeing them. outfit_newcomers: players under the trigger's level.
    pub(crate) fn greet(&mut self, actor: Key) {
        let room = self.chars.get(actor).unwrap().room;
        for mob in self.people[room].clone() {
            let Some(m) = self.chars.get(mob) else { continue };
            if mob == actor || !m.is_mob() || m.position <= Position::Sleeping || m.fighting.is_some() || !self.can_see(mob, actor) {
                continue;
            }
            for (id, t) in self.mob_triggers(mob) {
                let a = self.chars.get(actor).unwrap();
                if t.kind == "outfit_newcomers" && !a.is_mob() && t.below_level.is_some_and(|l| a.level < l) {
                    let at = self.tick + t.delay.unwrap_or(0) as u64 * PULSES_PER_SEC;
                    self.scheduled.push((at, Scheduled::Outfit { mob, actor, trigger: id }));
                }
            }
        }
    }

    /// outfit_newcomers, after its wait: wearing nothing, the whole kit worn; else the first missing
    /// piece, its line said and the piece given (30.trg #3016).
    fn outfit(&mut self, mob: Key, actor: Key, trigger: &str) {
        let (Some(_), Some(a)) = (self.chars.get(mob), self.chars.get(actor)) else { return };
        let Some(t) = self.tables.triggers.get(trigger).cloned() else { return };
        if a.equipment.is_empty() {
            let l = self.line(trigger, &t, "full", &[]);
            self.say_line(mob, &l.text, Some(l.line));
            for piece in &t.kit {
                for slot in &piece.slots {
                    if let Some(o) = self.make_obj(&piece.object) {
                        self.put(o, Place::Worn(actor, *slot));
                    }
                }
            }
            return;
        }
        let name = a.name.clone();
        let worn = |s: &Sim, slot: &EquipPos| s.chars.get(actor).is_some_and(|c| c.equipment.contains_key(slot));
        let Some(i) = t.kit.iter().position(|p| !p.full_only && p.slots.iter().any(|s| !worn(self, s))) else { return };
        if t.kit[i].say.is_some() {
            let l = self.line(trigger, &t, &format!("kit.{i}"), &[actor]);
            self.say_line(mob, &l.text, Some(l.line));
        }
        // The social (shake, sigh, roll, smile) comes with socials.
        let Some(o) = self.make_obj(&t.kit[i].object) else { return };
        self.put(o, Place::Carried(mob));
        let kw = self.objs.get(o).and_then(|x| x.keywords.first().cloned()).unwrap_or_default();
        self.give(mob, &format!("{kw} {}", name.to_lowercase()));
    }

    /// Random triggers, every 13 seconds, for mobs in a zone with a player (dg_scripts.c
    /// script_trigger_check, dg_triggers.c random_mtrigger): the first that rolls under its chance.
    pub(crate) fn script_check(&mut self) {
        for mob in self.mobs.clone() {
            let Some(c) = self.chars.get(mob) else { continue };
            let zone = self.world.rooms[c.room].zone;
            let occupied = self.order.iter().any(|k| self.chars.get(*k).is_some_and(|p| p.linked && self.world.rooms[p.room].zone == zone));
            if !occupied || c.has(mundi_content::names::Affect::Charm) {
                continue;
            }
            for (id, t) in self.mob_triggers(mob) {
                let Some(chance) = t.chance else { continue };
                if self.rand(1, 100) as i32 > chance {
                    continue;
                }
                match t.kind.as_str() {
                    "guard" => self.guard(mob, &id, &t),
                    "eat_corpses" => self.eat_corpse(mob, &id, &t),
                    "pick_up_litter" => self.pick_up_litter(mob, &t),
                    _ => {}
                }
                break;
            }
        }
    }

    /// guard: picks one it sees; spits at low charisma; joins a fight on the side of a victim who is
    /// better aligned (0 or more) than the attacker (30.trg #3009).
    fn guard(&mut self, mob: Key, id: &str, t: &Trigger) {
        let Some(g) = self.chars.get(mob) else { return };
        if g.fighting.is_some() {
            return;
        }
        let room = g.room;
        // %random.char%: one of those the guard sees, each as likely (dg_variables.c:504-518).
        let mut actor = None;
        let mut count = 0;
        for c in self.people[room].clone() {
            if c != mob && self.can_see(mob, c) {
                if self.rand(0, count) == 0 {
                    actor = Some(c);
                }
                count += 1;
            }
        }
        let Some(actor) = actor else { return };
        if t.below_charisma.is_some_and(|b| self.abilities_now(actor).cha < b) {
            let you = self.line(id, t, "spit_you", &[mob]);
            self.deliver(actor, Event::Echo { text: cap_first(&you.text), line: Some(you.line) });
            let around = self.line(id, t, "spit_room", &[mob, actor]);
            for w in self.people[room].clone() {
                if w != actor && self.awake_and_linked_pub(w) {
                    self.deliver(w, Event::Echo { text: cap_first(&around.text), line: Some(around.line.clone()) });
                }
            }
        }
        let Some(victim) = self.chars.get(actor).and_then(|a| a.fighting) else { return };
        let (aa, va) = (self.chars.get(actor).unwrap().alignment, self.chars.get(victim).map_or(-1000, |v| v.alignment));
        if aa < va && va >= 0 {
            let l = self.line(id, t, "protect", &[]);
            self.to_room(mob, room, false, |who, who_id| Event::Emote { who, who_id, text: l.text.clone(), line: Some(l.line.clone()) });
            // kill %actor.name%: the name's first word, as the command would read it.
            let aname = self.chars.get(actor).unwrap().name.clone();
            let word = aname.split_whitespace().next().unwrap_or("").to_lowercase();
            self.hit_cmd(mob, &word);
        }
    }

    /// eat_corpses: the first corpse in the room (30.trg #3010).
    fn eat_corpse(&mut self, mob: Key, id: &str, t: &Trigger) {
        let room = self.chars.get(mob).unwrap().room;
        let Some(corpse) = self.things[room].iter().copied().find(|o| self.objs.get(*o).is_some_and(|x| x.values.corpse())) else { return };
        let l = self.line(id, t, "devours", &[]);
        self.to_room(mob, room, false, |who, who_id| Event::Emote { who, who_id, text: l.text.clone(), line: Some(l.line.clone()) });
        self.extract_obj(corpse);
    }

    /// pick_up_litter: `take` everything up to the trigger's cost that is not a fountain (30.trg #3011).
    fn pick_up_litter(&mut self, mob: Key, t: &Trigger) {
        let room = self.chars.get(mob).unwrap().room;
        let Some(max) = t.max_cost else { return };
        for o in self.things[room].clone() {
            let Some(obj) = self.objs.get(o) else { continue };
            if obj.kind != ItemType::Fountain && obj.cost <= max {
                let word = obj.keywords.first().cloned().unwrap_or_default();
                self.get(mob, &word);
            }
        }
    }

    /// reward_drops, a room's drop trigger: the thing is taken and rewarded, in experience below the
    /// trigger's level, else gold (30.trg #3004). Whether it took the drop.
    pub(crate) fn drop_trigger(&mut self, actor: Key, o: Key) -> bool {
        let room = self.chars.get(actor).unwrap().room;
        let Some((id, t)) = self.room_triggers(room).into_iter().find(|(_, t)| t.kind == "reward_drops") else { return false };
        let you = self.line(&id, &t, "you", &[]);
        self.deliver(actor, Event::Echo { text: you.text, line: Some(you.line) });
        let around = self.line(&id, &t, "room", &[actor]);
        for w in self.people[room].clone() {
            if w != actor && self.awake_and_linked_pub(w) {
                self.deliver(w, Event::Echo { text: around.text.clone(), line: Some(around.line.clone()) });
            }
        }
        if let Some(r) = t.reward {
            let value = (self.objs.get(o).map_or(0, |x| x.cost) / r.per).clamp(r.min, r.max);
            if t.below_level.is_some_and(|l| self.chars.get(actor).unwrap().level < l) {
                self.gain_exp(actor, value);
            } else {
                self.chars.get_mut(actor).unwrap().gold += value;
            }
        }
        self.extract_obj(o);
        true
    }
}

fn cap_first(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}
