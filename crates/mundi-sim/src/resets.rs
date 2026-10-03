//! Zone resets (MECHANICS §14.1) and making mobs (§17.3).
//!
//! Mundi runs a zone's resets as: removals, then spawns in file order, then door states. tbaMUD runs
//! one mixed list in which a removal comes before the object it puts back; the content format keeps
//! the groups apart (D16), so removals go first.

use std::collections::BTreeMap;

use mundi_content::names::{EquipPos, ResetWhen};
use mundi_content::{Load, Spawn};

use crate::entity::{parse_dice, MobPart};
use crate::*;

impl Sim {
    /// Every 10 seconds: once a minute the zones age, and one queued zone may reset (db.c zone_update).
    pub(crate) fn zone_update(&mut self) {
        if self.tick % (60 * PULSES_PER_SEC) == 0 {
            for z in &mut self.world.zone_list {
                if z.when == ResetWhen::Never {
                    continue;
                }
                if let Some(age) = z.age.as_mut() {
                    *age += 1;
                    if *age >= z.every_minutes {
                        z.age = None;
                    }
                }
            }
        }
        let ready = (0..self.world.zone_list.len()).find(|&i| {
            let z = &self.world.zone_list[i];
            z.age.is_none() && (z.when == ResetWhen::Always || (z.when == ResetWhen::WhenEmpty && self.zone_empty(z.num)))
        });
        if let Some(i) = ready {
            self.reset_zone(i);
        }
    }

    /// No player in the game is in the zone (db.c is_empty; linkless players do not count).
    fn zone_empty(&self, zone: u32) -> bool {
        !self.order.iter().filter_map(|k| self.chars.get(*k)).any(|c| c.linked && self.world.rooms[c.room].zone == zone)
    }

    pub(crate) fn reset_zone(&mut self, i: usize) {
        let resets = self.world.zone_list[i].resets.clone();
        let num = self.world.zone_list[i].num;
        for r in &resets.removes {
            let Some(&room) = self.world.index.get(&r.room) else { continue };
            let found = self.things[room].iter().copied().find(|k| self.objs.get(*k).is_some_and(|o| o.proto.as_deref() == Some(r.object.as_str())));
            if let Some(o) = found {
                self.extract_obj(o);
            }
        }
        for spawn in &resets.spawns {
            match spawn {
                Spawn::Mob(m) => {
                    let Some(&room) = self.world.index.get(&m.room) else { continue };
                    let loaded = if self.count(&m.mob) < m.limit { self.make_mob(&m.mob, room) } else { None };
                    let Some(mob) = loaded else { continue };
                    for (pos, load) in &m.equip {
                        if let Some(o) = self.load(load) {
                            self.put(o, Place::Worn(mob, *pos));
                        }
                    }
                    for load in &m.carry {
                        if let Some(o) = self.load(load) {
                            self.put(o, Place::Carried(mob));
                        }
                    }
                }
                Spawn::Object(s) => {
                    if self.count(&s.object) >= s.limit {
                        continue;
                    }
                    let Some(o) = self.make_obj(&s.object) else { continue };
                    let place = match s.room.as_deref().and_then(|r| self.world.index.get(r)) {
                        Some(&room) => Place::Room(room),
                        None => Place::Nowhere,
                    };
                    self.put(o, place);
                    self.load_contents(o, &s.contents);
                }
            }
        }
        // Doors: each room's own side (the other side is its zone's business).
        for room in 0..self.world.rooms.len() {
            if self.world.rooms[room].zone != num {
                continue;
            }
            for exit in self.world.rooms[room].exits.iter_mut().flatten() {
                if let Some(d) = exit.door.as_mut() {
                    if let Some(state) = d.reset {
                        d.state = state;
                    }
                }
            }
        }
        for d in &resets.doors {
            let Some(&room) = self.world.index.get(&d.room) else { continue };
            if let Some(door) = self.world.rooms[room].exits[world::dir_index(d.exit)].as_mut().and_then(|e| e.door.as_mut()) {
                door.state = d.state;
            }
        }
        self.world.zone_list[i].age = Some(0);
    }

    fn count(&self, proto: &str) -> i32 {
        self.counts.get(proto).copied().unwrap_or(0)
    }

    /// One object of a reset, under its limit, with its contents.
    fn load(&mut self, l: &Load) -> Option<Key> {
        if self.count(&l.object) >= l.limit {
            return None;
        }
        let o = self.make_obj(&l.object)?;
        self.load_contents(o, &l.contents);
        Some(o)
    }

    fn load_contents(&mut self, into: Key, contents: &[Load]) {
        for l in contents {
            if let Some(o) = self.load(l) {
                self.put(o, Place::In(into));
            }
        }
    }

    /// A new mob of a prototype in a room (MECHANICS §17.3).
    pub(crate) fn make_mob(&mut self, proto: &str, room: RoomIx) -> Option<Key> {
        let p = self.world.mob_protos.get(proto)?.clone();
        let (n, s, b) = parse_dice(&p.combat.hit_points);
        let hp = (self.dice(n, s) + b).max(1);
        let (dn, ds, db) = parse_dice(&p.combat.damage);
        let ab = |k: &str| p.abilities.get(k).map(|v| *v as i32).unwrap_or(11);
        let pos = |q: mundi_content::names::Position| match q {
            mundi_content::names::Position::Dead => Position::Dead,
            mundi_content::names::Position::MortallyWounded => Position::MortallyWounded,
            mundi_content::names::Position::Incapacitated => Position::Incapacitated,
            mundi_content::names::Position::Stunned => Position::Stunned,
            mundi_content::names::Position::Sleeping => Position::Sleeping,
            mundi_content::names::Position::Resting => Position::Resting,
            mundi_content::names::Position::Sitting => Position::Sitting,
            mundi_content::names::Position::Fighting => Position::Fighting,
            mundi_content::names::Position::Standing => Position::Standing,
        };
        let sex = match p.sex {
            mundi_content::names::Sex::Male => mundi_protocol::Sex::Male,
            mundi_content::names::Sex::Female => mundi_protocol::Sex::Female,
            mundi_content::names::Sex::Neutral => mundi_protocol::Sex::Neutral,
        };
        let serial = self.next_serial();
        let key = self.chars.insert(Char {
            name: p.short.clone(),
            keywords: p.keywords.clone(),
            mob: Some(MobPart {
                proto: proto.to_string(),
                serial,
                flags: p.flags.clone(),
                default_position: pos(p.position.default),
                long: p.long.clone(),
                damage: (dn, ds),
            }),
            room,
            position: pos(p.position.load),
            class: None,
            sex,
            level: p.level,
            exp: p.exp,
            gold: p.gold,
            alignment: p.alignment,
            abilities: Abilities { str: ab("str"), str_add: ab("str_add").min(100) * (p.abilities.contains_key("str_add") as i32), int: ab("int"), wis: ab("wis"), dex: ab("dex"), con: ab("con"), cha: ab("cha") },
            hp,
            max_hp: hp,
            mana: 10,
            max_mana: 10,
            mv: 50,
            max_mv: 50,
            hitroll: 20 - p.combat.thac0,
            damroll: db,
            armor: 10 * p.combat.armor,
            conditions: Conditions::NEVER,
            practices: 0,
            skills: BTreeMap::new(),
            born: self.tick as i64,
            affects: p.affects.clone(),
            inventory: Vec::new(),
            equipment: BTreeMap::<EquipPos, Key>::new(),
            linked: false,
            queue: VecDeque::new(),
            fighting: None,
            wait: 0,
            wimpy: 0,
            memory: Vec::new(),
            master: None,
            followers: Vec::new(),
            group: None,
            prefs: Default::default(),
            spells: Vec::new(),
        });
        *self.counts.entry(proto.to_string()).or_default() += 1;
        self.mobs.push(key);
        self.people[room].insert(0, key);
        Some(key)
    }
}
