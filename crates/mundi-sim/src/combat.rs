//! Combat (MECHANICS §7, §8, §10.1): fighting and the 2-second round, one blow, damage, death and
//! corpses, fleeing, and the hit command.

use mundi_content::names::{Affect, Apply, EquipPos, ItemType, MobFlag, ObjFlag, RoomFlag};
use mundi_protocol::{AttackRefusal, DownState, HitKind, HitOutcome};

use crate::*;

/// tbaMUD's attack numbers (spells.h): weapons are 300 + the attack's index, suffering 399.
pub(crate) const TYPE_HIT: i32 = 300;
pub(crate) const TYPE_SUFFERING: i32 = 399;
/// spells.h SPELL_POISON: poison's damage each tick (MECHANICS §11.4).
pub(crate) const SPELL_POISON: i32 = 33;

/// Position as tbaMUD numbers it (MECHANICS §4.1), for the damage multiplier.
pub(crate) fn pos_number(p: Position) -> i32 {
    match p {
        Position::Dead => 0,
        Position::MortallyWounded => 1,
        Position::Incapacitated => 2,
        Position::Stunned => 3,
        Position::Sleeping => 4,
        Position::Resting => 5,
        Position::Sitting => 6,
        Position::Fighting => 7,
        Position::Standing => 8,
    }
}

impl Sim {
    // ---- what equipment and affects add (handler.c affect_modify, apply_ac) ----------------------

    /// The sum of an apply over everything worn.
    pub(crate) fn applied(&self, k: Key, what: Apply) -> i32 {
        let Some(c) = self.chars.get(k) else { return 0 };
        c.equipment
            .values()
            .filter_map(|o| self.objs.get(*o))
            .filter_map(|o| o.proto.as_ref().and_then(|p| self.world.obj_protos.get(p)))
            .flat_map(|p| p.affects.iter())
            .filter(|a| a.apply == what)
            .map(|a| a.modifier as i32)
            .sum()
    }

    pub(crate) fn abilities_now(&self, k: Key) -> Abilities {
        let mut a = self.chars.get(k).unwrap().abilities;
        a.str = (a.str + self.applied(k, Apply::Str)).clamp(0, 25);
        a.dex = (a.dex + self.applied(k, Apply::Dex)).clamp(0, 25);
        a.int = (a.int + self.applied(k, Apply::Int)).clamp(0, 25);
        a.wis = (a.wis + self.applied(k, Apply::Wis)).clamp(0, 25);
        a.con = (a.con + self.applied(k, Apply::Con)).clamp(0, 25);
        a.cha = (a.cha + self.applied(k, Apply::Cha)).clamp(0, 25);
        a
    }

    /// fight.c compute_armor_class with handler.c apply_ac (MECHANICS §7.4).
    pub(crate) fn armor_class(&self, k: Key) -> i32 {
        let c = self.chars.get(k).unwrap();
        let mut ac = c.armor + self.applied(k, Apply::Ac);
        for (pos, o) in &c.equipment {
            let Some(obj) = self.objs.get(*o) else { continue };
            if obj.kind != ItemType::Armor {
                continue;
            }
            let armor = obj.proto.as_ref().and_then(|p| self.world.obj_protos.get(p)).and_then(|p| p.values.armor).unwrap_or(0) as i32;
            let factor = match pos {
                EquipPos::Body => 3,
                EquipPos::Head | EquipPos::Legs => 2,
                _ => 1,
            };
            ac -= factor * armor;
        }
        if c.position > Position::Sleeping {
            let dex = self.abilities_now(k).dex.clamp(0, 25) as usize;
            ac += self.tables.abilities.dexterity.get(dex).map_or(0, |d| d.defensive) * 10;
        }
        ac.max(-100)
    }

    // ---- fighting state (fight.c set_fighting, stop_fighting, update_pos) -----------------------

    pub(crate) fn set_fighting(&mut self, ch: Key, vict: Key) {
        if ch == vict || self.chars.get(ch).is_none_or(|c| c.fighting.is_some()) {
            return;
        }
        if self.both_players(ch, vict) {
            return self.deliver(ch, Event::AttackRefused { reason: AttackRefusal::NotPermitted });
        }
        self.combat.insert(0, ch);
        let c = self.chars.get_mut(ch).unwrap();
        c.affects.retain(|a| *a != Affect::Sleep);
        c.fighting = Some(vict);
        c.position = Position::Fighting;
    }

    pub(crate) fn stop_fighting(&mut self, ch: Key) {
        self.combat.retain(|k| *k != ch);
        if let Some(c) = self.chars.get_mut(ch) {
            c.fighting = None;
            c.position = Position::Standing;
        }
        self.update_pos(ch);
    }

    /// MECHANICS §4.1.
    pub(crate) fn update_pos(&mut self, k: Key) {
        let Some(c) = self.chars.get_mut(k) else { return };
        c.position = match c.hp {
            hp if hp > 0 && c.position > Position::Stunned => return,
            hp if hp > 0 => Position::Standing,
            hp if hp <= -11 => Position::Dead,
            hp if hp <= -6 => Position::MortallyWounded,
            hp if hp <= -3 => Position::Incapacitated,
            _ => Position::Stunned,
        };
    }

    /// PK is off in this tbaMUD (config.c:52): a player may not fight a player.
    fn both_players(&self, a: Key, b: Key) -> bool {
        self.chars.get(a).is_some_and(|c| !c.is_mob()) && self.chars.get(b).is_some_and(|c| !c.is_mob())
    }

    // ---- the round (fight.c perform_violence, MECHANICS §7.1) ----------------------------------

    pub(crate) fn violence(&mut self) {
        for ch in self.combat.clone() {
            let Some(c) = self.chars.get(ch) else { continue };
            let Some(vict) = c.fighting.filter(|v| self.chars.get(*v).is_some_and(|v| v.room == c.room)) else {
                self.stop_fighting(ch);
                continue;
            };
            if c.is_mob() {
                if c.wait > 0 {
                    self.chars.get_mut(ch).unwrap().wait -= 2 * PULSES_PER_SEC as i32;
                    continue;
                }
                let c = self.chars.get_mut(ch).unwrap();
                c.wait = 0;
                if c.position < Position::Fighting {
                    let from = c.position;
                    c.position = Position::Fighting;
                    let room = c.room;
                    self.to_room(ch, room, true, |who, who_id| Event::OccupantPosition { who, who_id, position: Position::Fighting, from });
                }
            }
            if self.chars.get(ch).unwrap().position < Position::Fighting {
                self.deliver(ch, Event::AttackRefused { reason: AttackRefusal::Sitting });
                continue;
            }
            self.autoassist(ch);
            if self.chars.get(ch).and_then(|c| c.fighting) == Some(vict) {
                self.hit(ch, vict, None);
            }
        }
    }

    // ---- one blow (fight.c hit, MECHANICS §7.4) -------------------------------------------------

    /// `skill`: the skill's attack number (backstab) instead of the weapon's.
    pub(crate) fn hit(&mut self, ch: Key, vict: Key, skill: Option<i32>) {
        let (Some(c), Some(v)) = (self.chars.get(ch), self.chars.get(vict)) else { return };
        if c.room != v.room {
            return;
        }
        let weapon = c.equipment.get(&EquipPos::Wield).copied().filter(|o| self.objs.get(*o).is_some_and(|o| o.kind == ItemType::Weapon));
        let weapon_proto = weapon.and_then(|o| self.objs.get(o)).and_then(|o| o.proto.clone()).and_then(|p| self.world.obj_protos.get(&p).cloned());
        let attack_index = match &weapon_proto {
            Some(p) => p.values.attack.map(|a| mundi_content::names::Attack::ALL.iter().position(|x| *x == a).unwrap_or(0)),
            None => c.mob.as_ref().and_then(|_| {
                let proto = self.world.mob_protos.get(&c.mob.as_ref().unwrap().proto)?;
                proto.combat.bare_hand_attack.map(|a| mundi_content::names::Attack::ALL.iter().position(|x| *x == a).unwrap_or(0))
            }),
        };
        let w_type = TYPE_HIT + attack_index.unwrap_or(0) as i32;

        // THAC0 (fight.c compute_thaco).
        let a = self.abilities_now(ch);
        let base = match (c.class, c.is_mob()) {
            (Some(class), false) => self.tables.classes.get(class.key()).and_then(|t| t.thac0.get(c.level.clamp(0, 34) as usize).copied()).unwrap_or(20),
            _ => 20,
        };
        let st = self.strength_of(a);
        let (hitroll, damroll, mob_damage) = (c.hitroll, c.damroll, c.mob.as_ref().map(|m| m.damage));
        let awake = v.position > Position::Sleeping;
        let thaco = base
            - st.tohit
            - (hitroll + self.applied(ch, Apply::Hitroll))
            - ((a.int - 13) as f64 / 1.5) as i32
            - ((a.wis - 13) as f64 / 1.5) as i32;
        let victim_ac = self.armor_class(vict) / 10;
        let roll = self.rand(1, 20) as i32;
        let hits = roll == 20 || !awake || (roll != 1 && thaco - roll <= victim_ac);
        let attack = skill.unwrap_or(w_type);
        if !hits {
            self.damage(ch, vict, 0, attack, weapon);
            return;
        }
        let mut dam = st.todam + damroll + self.applied(ch, Apply::Damroll);
        dam += match (&weapon_proto, mob_damage) {
            (Some(p), _) => {
                let (n, s) = p.values.damage.as_deref().and_then(|d| d.split_once('d')).map_or((0, 0), |(n, s)| (n.parse().unwrap_or(0), s.parse().unwrap_or(0)));
                self.dice(n, s)
            }
            (None, Some((n, s))) => self.dice(n, s),
            (None, None) => self.rand(0, 2) as i32,
        };
        let vpos = self.chars.get(vict).unwrap().position;
        if vpos < Position::Fighting {
            dam *= 1 + (7 - pos_number(vpos)) / 3;
        }
        dam = dam.max(1);
        if skill == Some(SKILL_BACKSTAB) {
            dam *= backstab_mult(self.chars.get(ch).unwrap().level);
        }
        self.damage(ch, vict, dam, attack, weapon);
    }

    fn strength_of(&self, a: Abilities) -> mundi_content::tables::Strength {
        let i = if a.str == 18 && a.str_add > 0 {
            match a.str_add {
                ..=50 => 26,
                ..=75 => 27,
                ..=90 => 28,
                ..=99 => 29,
                _ => 30,
            }
        } else {
            a.str.clamp(0, 25) as usize
        };
        self.tables.abilities.strength[i.min(self.tables.abilities.strength.len() - 1)]
    }

    // ---- damage (fight.c damage, MECHANICS §8.1) ------------------------------------------------

    /// Returns the damage done, or -1 if the victim died.
    pub(crate) fn damage(&mut self, ch: Key, vict: Key, dam: i32, attack: i32, weapon: Option<Key>) -> i32 {
        let Some(v) = self.chars.get(vict) else { return -1 };
        if v.position == Position::Dead {
            return -1;
        }
        let room = v.room;
        if ch != vict {
            if self.both_players(ch, vict) {
                self.deliver(ch, Event::AttackRefused { reason: AttackRefusal::NotPermitted });
                return 0;
            }
            if self.world.rooms[room].flags.contains(&RoomFlag::Peaceful) {
                self.deliver(ch, Event::AttackRefused { reason: AttackRefusal::Peaceful });
                return 0;
            }
            if self.chars.get(vict).unwrap().has_flag(MobFlag::NoKill) || self.is_shopkeeper(vict) {
                self.deliver(ch, Event::AttackRefused { reason: AttackRefusal::Protected });
                return 0;
            }
            if self.chars.get(ch).is_some_and(|c| c.position > Position::Stunned && c.fighting.is_none()) {
                self.set_fighting(ch, vict);
            }
            if self.chars.get(vict).is_some_and(|c| c.position > Position::Stunned && c.fighting.is_none()) {
                self.set_fighting(vict, ch);
                let attacker_pc = self.chars.get(ch).filter(|c| !c.is_mob()).map(|c| c.name.clone());
                let v = self.chars.get_mut(vict).unwrap();
                if let (Some(name), true) = (attacker_pc, v.has_flag(MobFlag::Memory)) {
                    if !v.memory.contains(&name) {
                        v.memory.push(name);
                    }
                }
            }
            let a = self.chars.get(ch).unwrap();
            if a.has(Affect::Invisible) || a.has(Affect::Hide) {
                self.appear(ch);
            }
        }
        let mut dam = dam;
        if self.chars.get(vict).unwrap().has(Affect::Sanctuary) && dam >= 2 {
            dam /= 2;
        }
        dam = dam.clamp(0, 100);
        self.chars.get_mut(vict).unwrap().hp -= dam;
        if ch != vict {
            let level = self.chars.get(vict).unwrap().level as i64;
            self.gain_exp(ch, level * dam as i64);
        }
        self.update_pos(vict);
        self.hit_message(ch, vict, dam, attack, weapon);
        self.position_message(ch, vict, dam);
        let v = self.chars.get(vict).unwrap();
        if v.position <= Position::Stunned && v.fighting.is_some() {
            self.stop_fighting(vict);
        }
        if self.chars.get(vict).unwrap().position == Position::Dead {
            let v = self.chars.get(vict).unwrap();
            let gives_exp = ch != vict && (v.is_mob() || v.linked);
            let (vexp, vlevel, valign, vname, vpc, vgold) = (v.exp, v.level, v.alignment, v.name.clone(), !v.is_mob(), v.gold);
            if gives_exp {
                self.kill_reward(ch, vexp, vlevel, valign, vpc);
            }
            if !self.chars.get(vict).unwrap().is_mob() {
                if let Some(c) = self.chars.get_mut(ch) {
                    c.memory.retain(|n| *n != vname);
                }
            }
            self.die(vict, Some(ch));
            if ch != vict {
                self.after_kill(ch, vgold);
            }
            let _ = room;
            return -1;
        }
        dam
    }

    fn is_shopkeeper(&self, k: Key) -> bool {
        self.chars.get(k).and_then(|c| c.mob.as_ref()).is_some_and(|m| self.world.keepers.contains(&m.proto))
    }

    /// fight.c appear.
    fn appear(&mut self, k: Key) {
        let c = self.chars.get_mut(k).unwrap();
        c.affects.retain(|a| *a != Affect::Invisible && *a != Affect::Hide);
        let room = c.room;
        self.to_room(k, room, false, |who, who_id| Event::Appear { who, who_id });
    }

    /// The blow's events: to the attacker, the victim (even asleep), and the rest (MECHANICS §7.6).
    fn hit_message(&mut self, ch: Key, vict: Key, dam: i32, attack: i32, weapon: Option<Key>) {
        let room = self.chars.get(vict).unwrap().room;
        let dead = self.chars.get(vict).unwrap().position == Position::Dead;
        let outcome = if dam == 0 {
            HitOutcome::Miss
        } else if dead {
            HitOutcome::Die
        } else {
            HitOutcome::Hit
        };
        let weapon_type = (TYPE_HIT..TYPE_SUFFERING).contains(&attack);
        let severity = if weapon_type {
            match dam {
                0 => 0,
                ..=2 => 1,
                ..=4 => 2,
                ..=6 => 3,
                ..=10 => 4,
                ..=14 => 5,
                ..=19 => 6,
                ..=23 => 7,
                _ => 8,
            }
        } else {
            match outcome {
                HitOutcome::Miss => 0,
                HitOutcome::Hit => 4,
                _ => 8,
            }
        };
        let kind = if weapon_type { HitKind::Weapon } else if attack < 131 { HitKind::Spell } else { HitKind::Skill };
        let verb = attack_word(attack);
        let variant = Some(self.rand(1, 65535) as u32);
        let (wname, wid) = weapon.map_or((None, None), |w| (self.objs.get(w).map(|o| o.short.clone()), self.obj_id(w)));
        let name = |s: &Sim, k: Key| s.chars.get(k).unwrap().name.clone();
        let (an, vn) = (name(self, ch), name(self, vict));
        let (aid, vid) = (self.id_of(ch), self.id_of(vict));
        let make = |attacker: String, attacker_id: Option<String>, victim: String, victim_id: Option<String>| Event::Hit {
            attacker,
            attacker_id,
            victim,
            victim_id,
            verb: verb.clone(),
            severity,
            kind,
            damage: dam,
            attack,
            outcome,
            variant,
            weapon: wname.clone(),
            weapon_id: wid.clone(),
        };
        // The room first (dam_message sends to the room, then the attacker, then the victim).
        for w in self.people[room].clone() {
            if w == ch || w == vict || !self.awake_and_linked(w) {
                continue;
            }
            let (a, ai) = if self.can_see(w, ch) { (an.clone(), aid.clone()) } else { ("someone".into(), None) };
            let (v, vi) = if self.can_see(w, vict) { (vn.clone(), vid.clone()) } else { ("someone".into(), None) };
            self.deliver(w, make(a, ai, v, vi));
        }
        if ch != vict {
            let (v, vi) = if self.can_see(ch, vict) { (vn.clone(), vid.clone()) } else { ("someone".into(), None) };
            self.deliver(ch, make(SELF.into(), None, v, vi));
        }
        let (a, ai) = if ch == vict { (SELF.into(), None) } else if self.can_see(vict, ch) { (an, aid) } else { ("someone".into(), None) };
        self.deliver(vict, make(a, ai, SELF.into(), None));
    }

    /// fight.c:715-749.
    fn position_message(&mut self, ch: Key, vict: Key, dam: i32) {
        let v = self.chars.get(vict).unwrap();
        let (room, pos, hp, max) = (v.room, v.position, v.hp, v.max_hp);
        let state = match pos {
            Position::MortallyWounded => Some(DownState::MortallyWounded),
            Position::Incapacitated => Some(DownState::Incapacitated),
            Position::Stunned => Some(DownState::Stunned),
            _ => None,
        };
        if let Some(state) = state {
            let must_see = state == DownState::MortallyWounded;
            self.to_room(vict, room, must_see, |who, who_id| Event::CombatCondition { who, who_id, state });
            self.deliver(vict, Event::CombatCondition { who: SELF.into(), who_id: None, state });
            return;
        }
        if pos == Position::Dead {
            self.to_room(vict, room, false, |who, who_id| Event::Death { who, who_id });
            self.deliver(vict, Event::SelfDied {});
            return;
        }
        if dam > max / 4 {
            self.deliver(vict, Event::Pain { bleeding: false });
        }
        if hp < max / 4 {
            self.deliver(vict, Event::Pain { bleeding: true });
            let v = self.chars.get(vict).unwrap();
            if ch != vict && v.is_mob() && v.has_flag(MobFlag::Wimpy) {
                self.flee(vict);
            }
        }
        let v = self.chars.get(vict);
        if let Some(v) = v.filter(|v| !v.is_mob() && v.wimpy > 0 && ch != vict && v.hp > 0 && v.hp < v.wimpy && v.fighting.is_some()) {
            let _ = v;
            self.deliver(vict, Event::Wimpy {});
            self.flee(vict);
        }
    }

    fn awake_and_linked(&self, k: Key) -> bool {
        self.chars.get(k).is_some_and(|c| c.position > Position::Sleeping && c.linked)
    }

    // ---- death (fight.c die, raw_kill, make_corpse; MECHANICS §8.3, §8.4) -----------------------

    pub(crate) fn die(&mut self, vict: Key, killer: Option<Key>) {
        let exp = self.chars.get(vict).unwrap().exp;
        self.gain_exp(vict, -(exp / 2));
        if self.chars.get(vict).unwrap().fighting.is_some() {
            self.stop_fighting(vict);
        }
        for k in self.combat.clone() {
            if self.chars.get(k).and_then(|c| c.fighting) == Some(vict) {
                self.stop_fighting(k);
            }
        }
        let c = self.chars.get_mut(vict).unwrap();
        c.affects.clear();
        c.position = Position::Standing;
        let _ = killer;
        self.death_cry(vict);
        // raw_kill tells the group; extracting the character (the menu, for a player) drops the group
        // and the follows (fight.c:307-308, handler.c extract_char_final).
        if let Some(g) = self.chars.get(vict).unwrap().group {
            let (name, id) = (self.chars.get(vict).unwrap().name.clone(), self.id_of(vict));
            for m in self.groups.get(g).map(|g| g.members.clone()).unwrap_or_default() {
                if m != vict && self.chars.get(m).is_some_and(|c| !c.is_mob()) {
                    self.deliver(m, Event::GroupChange { event: "died".into(), who: name.clone(), who_id: id.clone() });
                }
            }
        }
        if !self.chars.get(vict).unwrap().is_mob() {
            self.leave_group(vict);
            self.drop_follows(vict);
        }
        self.update_pos(vict);
        self.make_corpse(vict);
        if self.chars.get(vict).unwrap().is_mob() {
            self.remove_char(vict);
        } else {
            self.come_back(vict);
        }
    }

    fn death_cry(&mut self, k: Key) {
        let room = self.chars.get(k).unwrap().room;
        self.to_room(k, room, false, |who, who_id| Event::DeathCry { who: Some(who), who_id, nearby: false });
        for d in 0..DIRS.len() {
            let Some(exit) = self.world.rooms[room].exits[d].as_ref() else { continue };
            let Some(to) = exit.to else { continue };
            if exit.door.as_ref().is_some_and(|door| door.state != mundi_content::names::DoorState::Open) {
                continue;
            }
            // send_to_room: everyone there, asleep or not.
            for w in self.people[to].clone() {
                self.deliver(w, Event::DeathCry { who: None, who_id: None, nearby: true });
            }
        }
    }

    fn make_corpse(&mut self, k: Key) {
        let c = self.chars.get(k).unwrap();
        let (room, name, is_mob, linked, gold) = (c.room, c.name.clone(), c.is_mob(), c.linked, c.gold);
        let made = self.tables.world.made.clone();
        let cfg = &self.tables.world.config;
        let timer = if is_mob { cfg.npc_corpse_ticks } else { cfg.pc_corpse_ticks };
        let serial = self.next_serial();
        let corpse = self.objs.insert(Obj {
            proto: None,
            serial,
            kind: ItemType::Container,
            keywords: vec!["corpse".into()],
            short: made.corpse_short.replace("%s", &name),
            long: made.corpse_long.replace("%s", &name),
            flags: vec![ObjFlag::NoDonate],
            wear: vec![mundi_content::names::Wear::Take],
            weight: 0,
            cost: 0,
            level: 0,
            values: crate::entity::ObjValues { capacity: Some(0), corpse: Some(true), ..Default::default() },
            timer,
            place: Place::Nowhere,
            contents: Vec::new(),
        });
        let c = self.chars.get(k).unwrap();
        let things: Vec<Key> = c.inventory.iter().copied().chain(c.equipment.values().copied()).collect();
        for o in things {
            self.put(o, Place::In(corpse));
        }
        if gold > 0 && (is_mob || linked) {
            let m = self.make_money(gold);
            self.put(m, Place::In(corpse));
        }
        self.chars.get_mut(k).unwrap().gold = 0;
        self.put(corpse, Place::Room(room));
    }

    /// A dead player comes back as from the menu (MECHANICS §8.3, the approved rule): at the start
    /// room, standing, nothing at or below zero.
    fn come_back(&mut self, k: Key) {
        let room = self.chars.get(k).unwrap().room;
        let start = self.world.index.get(&self.tables.world.config.start_room).copied().unwrap_or(0);
        self.people[room].retain(|x| *x != k);
        let c = self.chars.get_mut(k).unwrap();
        c.room = start;
        c.position = Position::Standing;
        c.hp = c.hp.max(1);
        c.mana = c.mana.max(1);
        c.mv = c.mv.max(1);
        c.wait = 0;
        self.people[start].insert(0, k);
        self.deliver(k, Event::InGame { how: InGameHow::Entered });
        self.to_room(k, start, true, |who, who_id| Event::Arrived { who, who_id, from_dir: None, how: Some(ArrivedHow::EnteredGame) });
        self.look(k);
    }

    // ---- fleeing (act.offensive.c do_flee, MECHANICS §10.1) -------------------------------------

    pub(crate) fn flee(&mut self, k: Key) {
        if self.chars.get(k).is_none_or(|c| c.position < Position::Fighting) {
            return self.deliver(k, Event::FleeFailed { reason: "bad_shape".into() });
        }
        for _ in 0..6 {
            let d = self.rand(0, DIRS.len() as i64 - 1) as usize;
            let c = self.chars.get(k).unwrap();
            let Some(exit) = self.world.rooms[c.room].exits[d].as_ref() else { continue };
            let Some(to) = exit.to else { continue };
            if exit.door.as_ref().is_some_and(|door| door.state != mundi_content::names::DoorState::Open) || self.world.rooms[to].flags.contains(&RoomFlag::Death) {
                continue;
            }
            let room = c.room;
            let was = c.fighting;
            self.to_room(k, room, true, |who, who_id| Event::FleeSeen { who, who_id, failed: false });
            if self.move_dir(k, d) {
                self.deliver(k, Event::SelfFled { dir: Some(DIRS[d].into()) });
                if let Some(opp) = was {
                    if !self.chars.get(k).unwrap().is_mob() {
                        if let Some(o) = self.chars.get(opp) {
                            let loss = (o.max_hp - o.hp) as i64 * o.level as i64;
                            self.gain_exp(k, -loss);
                        }
                    }
                    self.stop_fighting(k);
                    if self.chars.get(opp).and_then(|o| o.fighting) == Some(k) {
                        self.stop_fighting(opp);
                    }
                }
            } else {
                self.to_room(k, room, true, |who, who_id| Event::FleeSeen { who, who_id, failed: true });
            }
            return;
        }
        self.deliver(k, Event::FleeFailed { reason: "panic".into() });
    }

    // ---- commands -------------------------------------------------------------------------------

    /// `hit` and `kill` (act.offensive.c do_hit, MECHANICS §7.2).
    pub(crate) fn hit_cmd(&mut self, k: Key, arg: &str) {
        let Some(word) = arg.split_whitespace().next() else {
            return self.deliver(k, Event::AttackRefused { reason: AttackRefusal::Who });
        };
        let Some(vict) = self.find_char_room(k, word) else {
            return self.deliver(k, Event::AttackRefused { reason: AttackRefusal::NotHere });
        };
        if vict == k {
            let room = self.chars.get(k).unwrap().room;
            self.deliver(k, Event::SelfHit { who: SELF.into(), who_id: None });
            self.to_room(k, room, false, |who, who_id| Event::SelfHit { who, who_id });
            return;
        }
        if self.both_players(k, vict) {
            return self.deliver(k, Event::AttackRefused { reason: AttackRefusal::NoPlayerKilling });
        }
        let c = self.chars.get(k).unwrap();
        if c.position == Position::Standing && c.fighting != Some(vict) {
            let (mine, theirs) = (self.abilities_now(k).dex, self.abilities_now(vict).dex);
            if mine > theirs || (mine == theirs && self.rand(1, 2) == 1) {
                self.hit(k, vict, None);
            } else {
                self.hit(vict, k, None);
            }
            if let Some(c) = self.chars.get_mut(k) {
                c.wait = c.wait.max(2 * PULSES_PER_SEC as i32 + 2);
            }
        } else {
            self.deliver(k, Event::AttackRefused { reason: AttackRefusal::AlreadyFighting });
        }
    }

    /// `flee` (MECHANICS §10.1).
    pub(crate) fn flee_cmd(&mut self, k: Key) {
        self.flee(k);
    }

    /// `toggle wimpy [n]` (act.informative.c:2364-2391).
    pub(crate) fn toggle(&mut self, k: Key, arg: &str) {
        let mut words = arg.split_whitespace();
        match words.next() {
            Some(w) if "wimpy".starts_with(w) => {
                let c = self.chars.get(k).unwrap();
                let value = match words.next() {
                    None => serde_json::json!({"current": c.wimpy}),
                    Some(n) => match n.parse::<i32>() {
                        Ok(n) if n < 0 => serde_json::json!({"refused": "negative"}),
                        Ok(n) if n > c.max_hp => serde_json::json!({"refused": "above_max"}),
                        Ok(n) if n > c.max_hp / 2 => serde_json::json!({"refused": "above_half"}),
                        Ok(n) => {
                            self.chars.get_mut(k).unwrap().wimpy = n;
                            serde_json::json!(n)
                        }
                        Err(_) => serde_json::json!({"refused": "not_a_number"}),
                    },
                };
                self.deliver(k, Event::Toggle { name: "wimpy".into(), value });
            }
            _ => self.deliver(k, Event::Refused { reason: Refusal::NotYet }),
        }
    }
}

/// The skills' attack numbers (spells.h).
pub(crate) const SKILL_BACKSTAB: i32 = 131;

/// class.c backstab_mult (MECHANICS §10.6).
pub(crate) fn backstab_mult(level: i32) -> i32 {
    match level {
        ..=7 => 2,
        ..=13 => 3,
        ..=20 => 4,
        ..=28 => 5,
        ..=30 => 6,
        _ => 20,
    }
}

/// The word of an attack: a weapon's attack, or a skill's or spell's name (spell_parser.c spello).
pub(crate) fn attack_word(attack: i32) -> String {
    match attack {
        TYPE_HIT..TYPE_SUFFERING => mundi_content::names::Attack::from_index((attack - TYPE_HIT) as usize).map_or("hit".into(), |a| a.name().to_string()),
        TYPE_SUFFERING => "suffering".into(),
        131 => "backstab".into(),
        132 => "bash".into(),
        133 => "hide".into(),
        134 => "kick".into(),
        32 => "magic missile".into(),
        33 => "poison".into(),
        _ => format!("attack {attack}"),
    }
}
