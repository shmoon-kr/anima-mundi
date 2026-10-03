//! The DG Script triggers the party meets, as native rules (MECHANICS §14.3; tables/triggers.yaml
//! says what each script does): the welcome at login, the kind soul, cityguards, fido, the janitor,
//! the dump. A script's `wait` is a scheduled step.

use mundi_content::names::{EquipPos, ItemType};
use mundi_content::tables::Trigger;

use crate::*;

/// The scripts' checks run every 13 seconds (dg_scripts.h PULSE_DG_SCRIPT).
pub(crate) const PULSE_SCRIPT: u64 = 13 * PULSES_PER_SEC;

#[derive(Debug, Clone)]
pub(crate) enum Scheduled {
    KindSoul { mob: Key, actor: Key, trigger: String },
    Welcome { name: String, zone: u32, text: String },
}

impl Sim {
    fn mob_triggers(&self, k: Key) -> Vec<Trigger> {
        let Some(m) = self.chars.get(k).and_then(|c| c.mob.as_ref()) else { return vec![] };
        let ids = self.world.mob_protos.get(&m.proto).map(|p| p.triggers.clone()).unwrap_or_default();
        ids.iter().filter_map(|id| self.tables.triggers.get(id).cloned()).collect()
    }

    fn room_triggers(&self, room: RoomIx) -> Vec<Trigger> {
        self.world.rooms[room].triggers.iter().filter_map(|id| self.tables.triggers.get(id).cloned()).collect()
    }

    fn line(t: &Trigger, key: &str) -> String {
        t.lines.get(key).cloned().unwrap_or_default()
    }

    fn fill(line: &str, names: &[&str]) -> String {
        let mut out = line.to_string();
        for n in names {
            out = out.replacen("%s", n, 1);
        }
        out
    }

    /// Steps whose wait is over, in the order they were scheduled.
    pub(crate) fn run_scheduled(&mut self) {
        let (due, later): (Vec<_>, Vec<_>) = std::mem::take(&mut self.scheduled).into_iter().partition(|(t, _)| *t <= self.tick);
        self.scheduled = later;
        for (_, s) in due {
            match s {
                Scheduled::KindSoul { mob, actor, trigger } => self.kind_soul(mob, actor, &trigger),
                Scheduled::Welcome { name, zone, text } => {
                    let line = Sim::fill(&text, &[&name]);
                    for k in self.order.clone() {
                        if self.chars.get(k).is_some_and(|c| self.world.rooms[c.room].zone == zone) {
                            self.deliver(k, Event::Echo { text: line.clone() });
                        }
                    }
                }
            }
        }
    }

    /// Login triggers of the room one enters the game in (interpreter.c:1318).
    pub(crate) fn login_triggers(&mut self, k: Key) {
        let room = self.chars.get(k).unwrap().room;
        for t in self.room_triggers(room) {
            if t.kind == "mortal_greet" {
                let name = self.chars.get(k).unwrap().name.clone();
                let at = self.tick + t.delay.unwrap_or(0) as u64 * PULSES_PER_SEC;
                let zone = self.world.rooms[room].zone;
                self.scheduled.push((at, Scheduled::Welcome { name, zone, text: Sim::line(&t, "welcome") }));
            }
        }
    }

    /// Greet triggers of the mobs in a room someone comes into (dg_triggers.c greet_mtrigger): awake,
    /// not fighting, seeing them.
    pub(crate) fn greet(&mut self, actor: Key) {
        let room = self.chars.get(actor).unwrap().room;
        for mob in self.people[room].clone() {
            let Some(m) = self.chars.get(mob) else { continue };
            if mob == actor || !m.is_mob() || m.position <= Position::Sleeping || m.fighting.is_some() || !self.can_see(mob, actor) {
                continue;
            }
            let ids = self.world.mob_protos.get(&m.mob.as_ref().unwrap().proto).map(|p| p.triggers.clone()).unwrap_or_default();
            for id in ids {
                let Some(t) = self.tables.triggers.get(&id).cloned() else { continue };
                if t.kind == "kind_soul" {
                    let a = self.chars.get(actor).unwrap();
                    if !a.is_mob() && a.level < t.below_level.unwrap_or(5) {
                        let at = self.tick + t.delay.unwrap_or(0) as u64 * PULSES_PER_SEC;
                        self.scheduled.push((at, Scheduled::KindSoul { mob, actor, trigger: id.clone() }));
                    }
                }
            }
        }
    }

    /// 30.trg #3016, after its wait: nothing worn, the whole kit worn; else the first missing piece.
    fn kind_soul(&mut self, mob: Key, actor: Key, trigger: &str) {
        let (Some(_), Some(a)) = (self.chars.get(mob), self.chars.get(actor)) else { return };
        let Some(t) = self.tables.triggers.get(trigger).cloned() else { return };
        if a.equipment.is_empty() {
            self.say(mob, &Sim::line(&t, "full"));
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
        let Some(piece) = t.kit.iter().find(|p| !p.full_only && p.slots.iter().any(|s| !worn(self, s))).cloned() else { return };
        if let Some(say) = &piece.say {
            self.say(mob, &Sim::fill(say, &[&name]));
        }
        // The social (shake, sigh, roll, smile) comes with socials.
        let Some(o) = self.make_obj(&piece.object) else { return };
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
            for t in self.mob_triggers(mob) {
                let Some(chance) = t.chance else { continue };
                if self.rand(1, 100) as i32 > chance {
                    continue;
                }
                match t.kind.as_str() {
                    "cityguard" => self.cityguard(mob, &t),
                    "fido" => self.fido(mob, &t),
                    "janitor" => self.janitor(mob, &t),
                    _ => {}
                }
                break;
            }
        }
    }

    /// 30.trg #3009.
    fn cityguard(&mut self, mob: Key, t: &Trigger) {
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
        let (gname, aname) = (self.chars.get(mob).unwrap().name.clone(), self.chars.get(actor).unwrap().name.clone());
        if self.abilities_now(actor).cha < 6 {
            self.deliver(actor, Event::Echo { text: cap_first(&Sim::fill(&Sim::line(t, "spit_you"), &[&gname])) });
            let around = cap_first(&Sim::fill(&Sim::line(t, "spit_room"), &[&gname, &aname]));
            for w in self.people[room].clone() {
                if w != actor && self.awake_and_linked_pub(w) {
                    self.deliver(w, Event::Echo { text: around.clone() });
                }
            }
        }
        let Some(victim) = self.chars.get(actor).and_then(|a| a.fighting) else { return };
        let (aa, va) = (self.chars.get(actor).unwrap().alignment, self.chars.get(victim).map_or(-1000, |v| v.alignment));
        if aa < va && va >= 0 {
            let text = Sim::line(t, "protect");
            self.to_room(mob, room, false, |who, who_id| Event::Emote { who, who_id, text: text.clone() });
            // kill %actor.name%: the name's first word, as the command would read it.
            let word = aname.split_whitespace().next().unwrap_or("").to_lowercase();
            self.hit_cmd(mob, &word);
        }
    }

    /// 30.trg #3010: the first corpse in the room.
    fn fido(&mut self, mob: Key, t: &Trigger) {
        let room = self.chars.get(mob).unwrap().room;
        let Some(corpse) = self.things[room].iter().copied().find(|o| self.objs.get(*o).is_some_and(|x| x.values.corpse())) else { return };
        let text = Sim::line(t, "devours");
        self.to_room(mob, room, false, |who, who_id| Event::Emote { who, who_id, text: text.clone() });
        self.extract_obj(corpse);
    }

    /// 30.trg #3011: `take` everything cheap that is not a fountain.
    fn janitor(&mut self, mob: Key, t: &Trigger) {
        let room = self.chars.get(mob).unwrap().room;
        let max = t.max_cost.unwrap_or(15);
        for o in self.things[room].clone() {
            let Some(obj) = self.objs.get(o) else { continue };
            if obj.kind != ItemType::Fountain && obj.cost <= max {
                let word = obj.keywords.first().cloned().unwrap_or_default();
                self.get(mob, &word);
            }
        }
    }

    /// 30.trg #3004, a drop trigger: the dump rewards and keeps the thing. Whether it took the drop.
    pub(crate) fn drop_trigger(&mut self, actor: Key, o: Key) -> bool {
        let room = self.chars.get(actor).unwrap().room;
        let Some(t) = self.room_triggers(room).into_iter().find(|t| t.kind == "dump") else { return false };
        let name = self.chars.get(actor).unwrap().name.clone();
        self.deliver(actor, Event::Echo { text: Sim::line(&t, "you") });
        let around = Sim::fill(&Sim::line(&t, "room"), &[&name]);
        for w in self.people[room].clone() {
            if w != actor && self.awake_and_linked_pub(w) {
                self.deliver(w, Event::Echo { text: around.clone() });
            }
        }
        let value = (self.objs.get(o).map_or(0, |x| x.cost) / 10).clamp(1, 50);
        if self.chars.get(actor).unwrap().level < t.below_level.unwrap_or(3) {
            self.gain_exp(actor, value);
        } else {
            self.chars.get_mut(actor).unwrap().gold += value;
        }
        self.extract_obj(o);
        true
    }
}

fn cap_first(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}
