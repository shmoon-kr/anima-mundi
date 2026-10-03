//! Entering and leaving the game (MECHANICS §8.3 for coming back after death, §17.2 for new characters).

use crate::*;

impl Sim {
    pub(crate) fn enter(&mut self, name: &str, saved: Option<&str>) {
        if let Some(&key) = self.by_name.get(&key_name(name)) {
            let c = self.chars.get_mut(key).unwrap();
            c.linked = true;
            let room = c.room;
            self.deliver(key, Event::InGame { how: InGameHow::Reconnected });
            self.to_room(key, room, true, |who, who_id| Event::Link { who, who_id, state: LinkState::Reconnected });
            return;
        }
        let room = saved
            .and_then(|r| self.world.index.get(r))
            .or_else(|| self.world.index.get(&self.tables.world.config.start_room))
            .copied()
            .unwrap_or(0);
        // A new character (MECHANICS §9.4); the level-up roll and classes come with S5.
        let new = &self.tables.world.config.new_character;
        let (hp, mana, mv) = (new.max_hit, new.max_mana, new.max_move);
        let key = self.chars.insert(Char {
            name: name.to_string(),
            room,
            position: Position::Standing,
            level: 1,
            hp,
            max_hp: hp,
            mana,
            max_mana: mana,
            mv,
            max_mv: mv,
            linked: true,
            queue: VecDeque::new(),
            affects: Vec::new(),
            light: false,
        });
        self.by_name.insert(key_name(name), key);
        self.order.push(key);
        self.people[room].insert(0, key);
        self.deliver(key, Event::InGame { how: InGameHow::Entered });
        self.to_room(key, room, true, |who, who_id| Event::Arrived { who, who_id, from_dir: None, how: Some(ArrivedHow::EnteredGame) });
        self.look(key);
    }

    pub(crate) fn quit(&mut self, key: Key) {
        let c = self.chars.get(key).unwrap();
        let (room, name) = (c.room, c.name.clone());
        self.to_room(key, room, true, |who, who_id| Event::Left { who, who_id, dir: None, how: Some(LeftHow::LeftGame) });
        self.deliver(key, Event::Closed { reason: CloseReason::Remote, detail: Some("quit".into()) });
        self.people[room].retain(|k| *k != key);
        self.order.retain(|k| *k != key);
        self.by_name.remove(&key_name(&name));
        self.chars.remove(key);
        self.departed.push(Departure { name, room: self.world.rooms[room].id.clone() });
    }
}
