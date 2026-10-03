//! Entering and leaving the game (MECHANICS §8.3 for coming back after death, §17.2 for new characters).

use std::collections::BTreeMap;

use crate::entity::Save;
use crate::*;

impl Sim {
    pub(crate) fn enter(&mut self, name: &str, save: Option<Save>, new: NewChar) {
        if let Some(&key) = self.by_name.get(&key_name(name)) {
            let c = self.chars.get_mut(key).unwrap();
            c.linked = true;
            let room = c.room;
            self.deliver(key, Event::InGame { how: InGameHow::Reconnected });
            self.to_room(key, room, true, |who, who_id| Event::Link { who, who_id, state: LinkState::Reconnected });
            return;
        }
        let start = self.world.index.get(&self.tables.world.config.start_room).copied().unwrap_or(0);
        let key = match save {
            Some(s) => {
                let room = s.room.as_deref().and_then(|r| self.world.index.get(r)).copied().unwrap_or(start);
                let key = self.chars.insert(self.from_save(name, &s, room));
                self.restore_objects(key, &s.objects);
                key
            }
            None => {
                let room = new.room.as_deref().and_then(|r| self.world.index.get(r)).copied().unwrap_or(start);
                self.new_character(name, new, room)
            }
        };
        let room = self.chars.get(key).unwrap().room;
        self.by_name.insert(key_name(name), key);
        self.order.push(key);
        self.people[room].insert(0, key);
        self.deliver(key, Event::InGame { how: InGameHow::Entered });
        self.to_room(key, room, true, |who, who_id| Event::Arrived { who, who_id, from_dir: None, how: Some(ArrivedHow::EnteredGame) });
        self.look(key);
    }

    /// MECHANICS §17.2: stats by 4d6-drop-lowest in the class's order, level 1, one level gained.
    fn new_character(&mut self, name: &str, new: NewChar, room: RoomIx) -> Key {
        let class = new.class.unwrap_or(Class::Warrior);
        let mut rolls: Vec<i32> = (0..6)
            .map(|_| {
                let r: Vec<i32> = (0..4).map(|_| self.rand(1, 6) as i32).collect();
                r.iter().sum::<i32>() - r.iter().min().unwrap()
            })
            .collect();
        rolls.sort_unstable_by(|a, b| b.cmp(a));
        let t = |i: usize| rolls[i];
        let mut a = match class {
            Class::MagicUser => Abilities { int: t(0), wis: t(1), dex: t(2), str: t(3), con: t(4), cha: t(5), str_add: 0 },
            Class::Cleric => Abilities { wis: t(0), int: t(1), str: t(2), dex: t(3), con: t(4), cha: t(5), str_add: 0 },
            Class::Thief => Abilities { dex: t(0), str: t(1), con: t(2), int: t(3), wis: t(4), cha: t(5), str_add: 0 },
            Class::Warrior => Abilities { str: t(0), dex: t(1), con: t(2), wis: t(3), int: t(4), cha: t(5), str_add: 0 },
        };
        if class == Class::Warrior && a.str == 18 {
            a.str_add = self.rand(0, 100) as i32;
        }
        let base = self.tables.world.config.new_character.clone();
        let mut skills = BTreeMap::new();
        if class == Class::Thief {
            for (s, v) in [("sneak", 10), ("hide", 5), ("steal", 15), ("backstab", 10), ("pick lock", 10), ("track", 10)] {
                skills.insert(s.to_string(), v);
            }
        }
        let key = self.chars.insert(Char {
            name: name.to_string(),
            keywords: vec![key_name(name)],
            mob: None,
            room,
            position: Position::Standing,
            class: Some(class),
            sex: new.sex,
            level: 1,
            exp: 1,
            gold: 0,
            alignment: 0,
            abilities: a,
            hp: base.max_hit,
            max_hp: base.max_hit,
            mana: base.max_mana,
            max_mana: base.max_mana,
            mv: base.max_move,
            max_mv: base.max_move,
            hitroll: 0,
            damroll: 0,
            armor: 100,
            conditions: Conditions { drunk: 0, full: 24, thirst: 24 },
            practices: 0,
            skills,
            born: self.tick as i64,
            affects: Vec::new(),
            inventory: Vec::new(),
            equipment: BTreeMap::new(),
            linked: true,
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
        self.advance_level(key);
        let c = self.chars.get_mut(key).unwrap();
        (c.hp, c.mana, c.mv) = (c.max_hp, c.max_mana, c.max_mv);
        key
    }

    fn from_save(&self, name: &str, s: &Save, room: RoomIx) -> Char {
        Char {
            name: name.to_string(),
            keywords: vec![key_name(name)],
            mob: None,
            room,
            position: Position::Standing,
            class: Some(s.class),
            sex: s.sex,
            level: s.level,
            exp: s.exp,
            gold: s.gold,
            alignment: s.alignment,
            abilities: s.abilities,
            // Coming back: nothing at or below zero (db.c:3738-3748, MECHANICS §8.3).
            hp: s.hp.max(1),
            max_hp: s.max_hp,
            mana: s.mana.max(1),
            max_mana: s.max_mana,
            mv: s.mv.max(1),
            max_mv: s.max_mv,
            hitroll: 0,
            damroll: 0,
            armor: 100,
            conditions: s.conditions,
            practices: s.practices,
            skills: s.skills.clone(),
            born: self.tick as i64 - s.lived,
            affects: Vec::new(),
            inventory: Vec::new(),
            equipment: BTreeMap::new(),
            linked: true,
            queue: VecDeque::new(),
            fighting: None,
            wait: 0,
            wimpy: s.wimpy,
            memory: Vec::new(),
            master: None,
            followers: Vec::new(),
            group: None,
            prefs: s.prefs.iter().cloned().collect(),
            spells: s.spells.clone(),
        }
    }

    /// What the store keeps of a player now.
    pub(crate) fn save_of(&self, k: Key) -> Save {
        let c = self.chars.get(k).unwrap();
        Save {
            room: Some(self.world.rooms[c.room].id.clone()),
            class: c.class.unwrap_or(Class::Warrior),
            sex: c.sex,
            level: c.level,
            exp: c.exp,
            gold: c.gold,
            alignment: c.alignment,
            abilities: c.abilities,
            hp: c.hp,
            max_hp: c.max_hp,
            mana: c.mana,
            max_mana: c.max_mana,
            mv: c.mv,
            max_mv: c.max_mv,
            conditions: c.conditions,
            practices: c.practices,
            skills: c.skills.clone(),
            lived: self.tick as i64 - c.born,
            objects: self.save_objects(k),
            prefs: c.prefs.iter().cloned().collect(),
            wimpy: c.wimpy,
            spells: c.spells.clone(),
        }
    }

    pub(crate) fn quit(&mut self, key: Key) {
        let c = self.chars.get(key).unwrap();
        let (room, name) = (c.room, c.name.clone());
        self.to_room(key, room, true, |who, who_id| Event::Left { who, who_id, dir: None, how: Some(LeftHow::LeftGame) });
        self.deliver(key, Event::Closed { reason: CloseReason::Remote, detail: Some("quit".into()) });
        let save = self.save_of(key);
        self.remove_char(key);
        self.departed.push(Departure { name, save });
    }

    /// Takes a character and everything they have out of the world.
    pub(crate) fn remove_char(&mut self, key: Key) {
        let Some(c) = self.chars.get(key) else { return };
        let (room, name) = (c.room, c.name.clone());
        let things: Vec<Key> = c.inventory.iter().chain(c.equipment.values()).copied().collect();
        for o in things {
            self.extract_obj(o);
        }
        if let Some(m) = &self.chars.get(key).unwrap().mob {
            *self.counts.entry(m.proto.clone()).or_default() -= 1;
        } else {
            self.by_name.remove(&key_name(&name));
        }
        self.leave_group(key);
        self.drop_follows(key);
        if self.chars.get(key).is_some_and(|c| c.fighting.is_some()) {
            self.stop_fighting(key);
        }
        for k in self.combat.clone() {
            if self.chars.get(k).and_then(|c| c.fighting) == Some(key) {
                self.stop_fighting(k);
            }
        }
        self.people[room].retain(|k| *k != key);
        self.order.retain(|k| *k != key);
        self.mobs.retain(|k| *k != key);
        self.test_lights.retain(|k| *k != key);
        self.chars.remove(key);
    }
}
