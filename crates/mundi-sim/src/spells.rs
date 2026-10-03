//! Casting (MECHANICS §11, spell_parser.c, magic.c, spells.c): cast, the spoken words, mana, the
//! failure roll, saving throws, and the phase-1 spells (magic missile, cure light, armor, create
//! food, create water, poison, remove poison). The spell table is tbaMUD's (tables/spells.yaml).

use mundi_content::names::{Affect, Apply, ItemType, Liquid, MobFlag, RoomFlag};
use mundi_content::tables::Spell;

use crate::combat::SPELL_POISON;
use crate::entity::SpellAffect;
use crate::*;

const SPELL_ARMOR: i32 = 1;
const SPELL_CREATE_FOOD: i32 = 12;
const SPELL_CREATE_WATER: i32 = 13;
const SPELL_CURE_LIGHT: i32 = 16;
const SPELL_MAGIC_MISSILE: i32 = 32;
const SPELL_REMOVE_POISON: i32 = 43;
/// The highest spell number (spells.h MAX_SPELLS).
const MAX_SPELLS: i32 = 130;
/// What create food makes (magic.c:952-968: object vnum 10, the waybread).
const WAYBREAD: &str = "tba:0:obj:10";

/// What a spell is cast on.
#[derive(Clone, Copy)]
enum On {
    Nothing,
    Char(Key),
    Obj(Key),
}

impl Sim {
    fn cast_fail(&mut self, k: Key, reason: &str) {
        self.deliver(k, Event::SkillResult { skill: "cast".into(), ok: false, reason: Some(reason.into()), who: None, who_id: None });
    }

    /// spell_parser.c find_skill_num: the whole name begins with what was typed, or every typed
    /// word begins the matching word of the name.
    fn find_spell(&self, typed: &str) -> Option<Spell> {
        let typed = typed.to_lowercase();
        let words: Vec<&str> = typed.split_whitespace().collect();
        self.tables.spells.iter().find(|s| {
            s.name.starts_with(&typed) || {
                let name: Vec<&str> = s.name.split_whitespace().collect();
                !words.is_empty() && words.len() <= name.len() && words.iter().zip(&name).all(|(w, n)| n.starts_with(w))
            }
        }).cloned()
    }

    /// `cast '<spell>' [target]` (spell_parser.c do_cast, MECHANICS §11.1).
    pub(crate) fn cast(&mut self, k: Key, arg: &str) {
        if self.chars.get(k).unwrap().is_mob() {
            return;
        }
        let arg = arg.trim();
        if arg.is_empty() {
            return self.cast_fail(k, "what_where");
        }
        let mut parts = arg.splitn(3, '\'');
        let (_, name, rest) = (parts.next(), parts.next(), parts.next());
        let (Some(name), Some(rest)) = (name, rest) else { return self.cast_fail(k, "holy_symbols") };
        let Some(sp) = self.find_spell(name).filter(|s| s.number <= MAX_SPELLS && s.number > 0 && !name.is_empty()) else {
            return self.cast_fail(k, "what");
        };
        let c = self.chars.get(k).unwrap();
        let class = c.class.unwrap_or(Class::Warrior).key();
        let Some(&min_level) = sp.levels.get(class).filter(|l| c.level >= **l) else { return self.cast_fail(k, "dont_know") };
        let skill = c.skills.get(&sp.name).copied().unwrap_or(0);
        if skill == 0 {
            return self.cast_fail(k, "unfamiliar");
        }
        let target_word = rest.split_whitespace().next().unwrap_or("");
        let has = |t: &str| sp.targets.iter().any(|x| x == t);
        let mut on = On::Nothing;
        if !has("ignore") {
            if !target_word.is_empty() {
                let (room, inv) = (c.room, c.inventory.clone());
                let worn: Vec<Key> = c.equipment.values().copied().collect();
                if has("char_room") {
                    if let Some(t) = self.find_char_room(k, target_word) {
                        on = On::Char(t);
                    }
                }
                let obj = |s: &Sim, list: &[Key]| match crate::items::target(target_word) {
                    Some(crate::items::Target::One { word, nth }) => s.find_obj(k, list, &word, nth),
                    _ => None,
                };
                if matches!(on, On::Nothing) && has("obj_inv") {
                    if let Some(o) = obj(self, &inv) {
                        on = On::Obj(o);
                    }
                }
                if matches!(on, On::Nothing) && has("obj_equip") {
                    if let Some(o) = obj(self, &worn) {
                        on = On::Obj(o);
                    }
                }
                if matches!(on, On::Nothing) && has("obj_room") {
                    if let Some(o) = obj(self, &self.things[room].clone()) {
                        on = On::Obj(o);
                    }
                }
            } else {
                let fighting = c.fighting.filter(|v| self.chars.get(*v).is_some_and(|v| v.room == c.room));
                if has("fight_self") && fighting.is_some() {
                    on = On::Char(k);
                } else if has("fight_vict") && fighting.is_some() {
                    on = On::Char(fighting.unwrap());
                } else if has("char_room") && !sp.violent {
                    on = On::Char(k);
                } else {
                    let what = sp.targets.iter().any(|t| t.starts_with("obj_"));
                    return self.cast_fail(k, if what { "upon_what" } else { "upon_who" });
                }
            }
            if let On::Char(t) = on {
                if t == k && sp.violent {
                    return self.cast_fail(k, "self_violent");
                }
            }
            if matches!(on, On::Nothing) {
                return self.cast_fail(k, "no_target");
            }
        }
        let mana = (sp.mana_max - sp.mana_change * (c.level - min_level)).max(sp.mana_min);
        if c.mana < mana {
            return self.cast_fail(k, "no_energy");
        }
        // The failure roll: 0-101 above the skill (spell_parser.c:632-639).
        if self.rand(0, 101) as i32 > skill {
            self.chars.get_mut(k).unwrap().wait = 2 * PULSES_PER_SEC as i32;
            match on {
                On::Char(t) if t != k => {
                    self.damage_message_only(k, t, sp.number);
                }
                _ => self.cast_fail(k, "lost_concentration"),
            }
            let c = self.chars.get_mut(k).unwrap();
            c.mana = (c.mana - mana / 2).clamp(0, c.max_mana);
            if let On::Char(t) = on {
                if sp.violent && self.chars.get(t).is_some_and(|t| t.is_mob()) && self.chars.get(t).is_some_and(|t| t.fighting.is_none()) {
                    self.hit(t, k, None);
                }
            }
            return;
        }
        if self.cast_spell(k, &sp, on) {
            let c = self.chars.get_mut(k).unwrap();
            c.wait = 2 * PULSES_PER_SEC as i32;
            c.mana = (c.mana - mana).clamp(0, c.max_mana);
        }
    }

    /// The spell's miss lines from the message file, for a failed cast at someone.
    fn damage_message_only(&mut self, k: Key, t: Key, spell: i32) {
        let room = self.chars.get(k).unwrap().room;
        let (an, aid, vn, vid) = (self.chars.get(k).unwrap().name.clone(), self.id_of(k), self.chars.get(t).unwrap().name.clone(), self.id_of(t));
        let variant = Some(self.rand(1, 65535) as u32);
        let make = |a: String, ai: Option<String>, v: String, vi: Option<String>| Event::Hit {
            attacker: a,
            attacker_id: ai,
            victim: v,
            victim_id: vi,
            verb: self.spell_name(spell),
            severity: 0,
            kind: mundi_protocol::HitKind::Spell,
            damage: 0,
            attack: spell,
            outcome: mundi_protocol::HitOutcome::Miss,
            variant,
            weapon: None,
            weapon_id: None,
        };
        let to_me = make(SELF.into(), None, vn.clone(), vid.clone());
        let to_them = make(an.clone(), aid.clone(), SELF.into(), None);
        let to_room = make(an, aid, vn, vid);
        self.deliver(k, to_me);
        self.deliver(t, to_them);
        for w in self.people[room].clone() {
            if w != k && w != t && self.awake_and_linked_pub(w) {
                self.deliver(w, to_room.clone());
            }
        }
    }

    /// spell_parser.c cast_spell: the position and target checks, "Okay.", the words, the magic.
    fn cast_spell(&mut self, k: Key, sp: &Spell, on: On) -> bool {
        let c = self.chars.get(k).unwrap();
        let need = crate::commands::protocol_position(sp.position);
        if c.position < need {
            let r = match c.position {
                Position::Sleeping => "pos_sleeping",
                Position::Resting => "pos_resting",
                Position::Sitting => "pos_sitting",
                Position::Fighting => "pos_fighting",
                _ => "pos_other",
            };
            self.cast_fail(k, r);
            return false;
        }
        if let On::Char(t) = on {
            if sp.targets.iter().any(|x| x == "self_only") && t != k {
                self.cast_fail(k, "self_only");
                return false;
            }
            if sp.targets.iter().any(|x| x == "not_self") && t == k {
                self.cast_fail(k, "not_self");
                return false;
            }
        }
        self.deliver(k, Event::SkillResult { skill: sp.name.clone(), ok: true, reason: None, who: None, who_id: None });
        self.say_spell(k, sp, on);
        self.call_magic(k, sp, on);
        true
    }

    /// spell_parser.c say_spell: those of the caster's class hear the spell's name, the rest its
    /// syllables (tables `syllables`, the first that fits at each place).
    fn say_spell(&mut self, k: Key, sp: &Spell, on: On) {
        let mut said = String::new();
        let name = sp.name.as_str();
        let mut i = 0;
        while i < name.len() {
            match self.tables.world.syllables.iter().find(|[from, _]| !from.is_empty() && name[i..].starts_with(from.as_str())) {
                Some([from, to]) => {
                    said.push_str(to);
                    i += from.len();
                }
                None => {
                    said.push_str(&name[i..i + 1]);
                    i += 1;
                }
            }
        }
        let c = self.chars.get(k).unwrap();
        let (room, class) = (c.room, c.class);
        let (target, target_id, at_object) = match on {
            On::Char(t) if t != k => (Some(self.chars.get(t).unwrap().name.clone()), self.id_of(t), false),
            On::Obj(o) => (self.objs.get(o).map(|o| o.short.clone()), self.obj_id(o), true),
            _ => (None, None, false),
        };
        let at_self = matches!(on, On::Char(t) if t == k);
        let (me, mid) = (c.name.clone(), self.id_of(k));
        for w in self.people[room].clone() {
            if w == k || !self.awake_and_linked_pub(w) {
                continue;
            }
            let words = if self.chars.get(w).unwrap().class == class && class.is_some() { sp.name.clone() } else { said.clone() };
            let (who, who_id) = if self.can_see(w, k) { (me.clone(), mid.clone()) } else { ("someone".into(), None) };
            let to_target = matches!(on, On::Char(t) if t == w);
            self.deliver(w, Event::SpellSaid {
                who,
                who_id,
                words,
                target: if to_target { Some(SELF.into()) } else { target.clone() },
                target_id: if to_target { None } else { target_id.clone() },
                at_object,
                at_self,
            });
        }
    }

    /// magic.c mag_savingthrow: the class table (warriors' for mobs) plus saves, under 0-99.
    fn saving_throw(&mut self, k: Key, kind: &str) -> bool {
        let c = self.chars.get(k).unwrap();
        let class = if c.is_mob() { "warrior" } else { c.class.unwrap_or(Class::Warrior).key() };
        let table = self.tables.classes.get(class).and_then(|t| t.saving_throws.get(kind)).and_then(|v| v.get(c.level.clamp(0, 40) as usize).copied()).unwrap_or(99);
        let apply = match kind {
            "paralysis" => Apply::SavePara,
            "rod" => Apply::SaveRod,
            "petrification" => Apply::SavePetri,
            "breath" => Apply::SaveBreath,
            _ => Apply::SaveSpell,
        };
        let own = c.mob.as_ref().and_then(|m| self.world.mob_protos.get(&m.proto)).and_then(|p| p.saves.get(kind).copied()).unwrap_or(0) as i32;
        let save = table + own + self.applied(k, apply);
        save.max(1) < self.rand(0, 99) as i32
    }

    /// spell_parser.c call_magic and the routines of the phase-1 spells (MECHANICS §11.3).
    fn call_magic(&mut self, k: Key, sp: &Spell, on: On) {
        let room = self.chars.get(k).unwrap().room;
        let flags = &self.world.rooms[room].flags;
        if flags.contains(&RoomFlag::NoMagic) {
            self.cast_fail(k, "fizzle");
            self.to_room(k, room, false, |who, who_id| Event::SkillResult { skill: "cast".into(), ok: false, reason: Some("fizzle_room".into()), who: Some(who), who_id });
            return;
        }
        if flags.contains(&RoomFlag::Peaceful) && (sp.violent || sp.routines.iter().any(|r| r == "damage")) {
            self.cast_fail(k, "white_light");
            self.to_room(k, room, false, |who, who_id| Event::SkillResult { skill: "cast".into(), ok: false, reason: Some("white_light_room".into()), who: Some(who), who_id });
            return;
        }
        if let On::Char(t) = on {
            if self.chars.get(t).unwrap().has_flag(MobFlag::NoKill) && sp.violent {
                return self.deliver(k, Event::AttackRefused { reason: mundi_protocol::AttackRefusal::Protected });
            }
        }
        let level = self.chars.get(k).unwrap().level;
        match (sp.number, on) {
            (SPELL_MAGIC_MISSILE, On::Char(t)) => {
                let mut dam = if self.chars.get(k).unwrap().class == Some(Class::MagicUser) { self.dice(1, 8) + 1 } else { self.dice(1, 6) + 1 };
                if self.saving_throw(t, "spell") {
                    dam /= 2;
                }
                self.damage(k, t, dam, SPELL_MAGIC_MISSILE, None);
            }
            (SPELL_CURE_LIGHT, On::Char(t)) => {
                let heal = self.dice(1, 8) + 1 + level / 4;
                let c = self.chars.get_mut(t).unwrap();
                c.hp = (c.hp + heal).min(c.max_hp);
                self.update_pos(t);
                self.effect(t, sp.number, "vict", None);
            }
            (SPELL_ARMOR, On::Char(t)) => {
                // accum_duration: a second casting adds to the time (magic.c:333-339).
                let c = self.chars.get_mut(t).unwrap();
                let old = c.spells.iter().position(|s| s.spell == SPELL_ARMOR && s.apply == Some(Apply::Ac));
                let duration = 24 + old.map_or(0, |i| c.spells[i].duration);
                if let Some(i) = old {
                    c.spells.remove(i);
                }
                c.spells.insert(0, SpellAffect { spell: SPELL_ARMOR, duration, apply: Some(Apply::Ac), modifier: -20, bit: None });
                self.effect(t, sp.number, "vict", None);
            }
            (SPELL_POISON, On::Char(t)) => {
                if self.saving_throw(t, "spell") || self.chars.get(t).unwrap().affected_by(SPELL_POISON) {
                    return self.cast_fail(k, "no_effect");
                }
                if self.chars.get(t).unwrap().is_mob() && self.chars.get(t).unwrap().affects.contains(&Affect::Poison) {
                    return self.cast_fail(k, "no_effect");
                }
                let c = self.chars.get_mut(t).unwrap();
                c.spells.insert(0, SpellAffect { spell: SPELL_POISON, duration: level, apply: Some(Apply::Str), modifier: -2, bit: Some(Affect::Poison) });
                self.effect(t, sp.number, "vict", None);
                self.effect_room(t, sp.number, "room", None);
                if self.chars.get(t).is_some_and(|t| t.is_mob() && t.fighting.is_none()) {
                    self.hit(t, k, None);
                }
            }
            (SPELL_POISON, On::Obj(o)) => self.alter_poison(k, o, true),
            (SPELL_REMOVE_POISON, On::Char(t)) => {
                if !self.chars.get(t).unwrap().affected_by(SPELL_POISON) {
                    return self.cast_fail(k, "no_effect");
                }
                self.chars.get_mut(t).unwrap().spells.retain(|s| s.spell != SPELL_POISON);
                self.effect(t, sp.number, "vict", None);
                self.effect_room(t, sp.number, "room", None);
            }
            (SPELL_REMOVE_POISON, On::Obj(o)) => self.alter_poison(k, o, false),
            (SPELL_CREATE_FOOD, _) => {
                let Some(food) = self.make_obj(WAYBREAD) else { return self.cast_fail(k, "goofed") };
                self.put(food, Place::Carried(k));
                let (text, id) = (self.objs.get(food).unwrap().short.clone(), self.obj_id(food));
                self.effect_room(k, sp.number, "room", Some((text.clone(), id.clone())));
                self.effect(k, sp.number, "vict", Some((text, id)));
            }
            (SPELL_CREATE_WATER, On::Obj(o)) => {
                let obj = self.objs.get_mut(o).unwrap();
                if obj.kind != ItemType::Drinkcon {
                    return;
                }
                let v = &mut obj.values;
                let now = v.contains.unwrap_or(0);
                if v.liquid != Some(Liquid::Water) && now != 0 {
                    v.liquid = Some(Liquid::SlimeMoldJuice);
                    return;
                }
                let room_for = (v.capacity.unwrap_or(0) - now).max(0);
                if room_for > 0 {
                    v.liquid = Some(Liquid::Water);
                    v.contains = Some(now + room_for);
                    obj.weight += room_for;
                    let (text, id) = (obj.short.clone(), self.obj_id(o));
                    self.effect(k, sp.number, "vict", Some((text, id)));
                }
            }
            _ => self.cast_fail(k, "not_yet"),
        }
    }

    /// magic.c mag_alter_objs: poison or cleanse food and drink.
    fn alter_poison(&mut self, k: Key, o: Key, poison: bool) {
        let obj = self.objs.get_mut(o).unwrap();
        let kind_ok = matches!(obj.kind, ItemType::Drinkcon | ItemType::Fountain | ItemType::Food);
        let is = obj.values.poisoned == Some(true);
        if !kind_ok || is == poison {
            return self.cast_fail(k, "no_effect");
        }
        obj.values.poisoned = Some(poison);
        let (text, id) = (obj.short.clone(), self.obj_id(o));
        let spell = if poison { SPELL_POISON } else { SPELL_REMOVE_POISON };
        self.effect(k, spell, "steams", Some((text.clone(), id.clone())));
        self.effect_room(k, spell, "steams", Some((text, id)));
    }

    /// A spell's line to the one it is on (`to_vict`).
    fn effect(&mut self, t: Key, spell: i32, line: &str, obj: Option<(String, Option<String>)>) {
        let (text, id) = obj.map_or((None, None), |(t, i)| (Some(t), i));
        self.deliver(t, Event::SpellEffect { spell, line: line.into(), who: SELF.into(), who_id: None, text, id });
    }

    /// A spell's line to the rest of the room (`to_room`, hide-invisible).
    fn effect_room(&mut self, t: Key, spell: i32, line: &str, obj: Option<(String, Option<String>)>) {
        let room = self.chars.get(t).unwrap().room;
        let (text, id) = obj.map_or((None, None), |(t, i)| (Some(t), i));
        let line = line.to_string();
        self.to_room(t, room, true, |who, who_id| Event::SpellEffect { spell, line: line.clone(), who, who_id, text: text.clone(), id: id.clone() });
    }
}
