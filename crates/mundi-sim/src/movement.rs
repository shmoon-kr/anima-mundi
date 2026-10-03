//! Moving between rooms (MECHANICS §2).

use crate::*;

impl Sim {
    /// MECHANICS §2.2 to §2.4. Boats, flying, water breathing, tunnels, followers and death rooms come
    /// with the rules they need (S5).
    /// Whether the move happened.
    pub(crate) fn move_dir(&mut self, key: Key, d: usize) -> bool {
        let c = self.chars.get(key).unwrap();
        let from = c.room;
        let dir = DIRS[d].to_string();
        let exit = self.world.rooms[from].exits[d].as_ref();
        let Some(to) = exit.and_then(|e| e.to) else {
            self.deliver(key, Event::MoveFailed { dir: Some(dir), reason: MoveFailure::NoExit, door: None });
            return false;
        };
        if let Some(door) = exit.and_then(|e| e.door.as_ref()).filter(|d| d.state != DoorState::Open) {
            let door = door.keyword.clone();
            self.deliver(key, Event::MoveFailed { dir: Some(dir), reason: MoveFailure::Closed, door });
            return false;
        }
        let zone = self.world.zones.get(&self.world.rooms[to].zone).cloned().unwrap_or_default();
        let mob = c.is_mob();
        if !mob && zone.min_level.is_some_and(|min| min > c.level) {
            self.deliver(key, Event::ZoneAboveLevel {});
        }
        if zone.closed {
            self.deliver(key, Event::MoveFailed { dir: Some(dir), reason: MoveFailure::Forbidden, door: None });
            return false;
        }
        let cost = (self.sector_cost(self.world.rooms[from].sector) + self.sector_cost(self.world.rooms[to].sector)) / 2;
        // Only players pay and are stopped by tiredness (act.movement.c:252-264, MECHANICS §2.3).
        let c = self.chars.get_mut(key).unwrap();
        if !mob {
            if c.mv < cost {
                self.deliver(key, Event::MoveFailed { dir: Some(dir), reason: MoveFailure::Exhausted, door: None });
                return false;
            }
            c.mv -= cost;
        }
        let left = dir.clone();
        self.to_room(key, from, true, move |who, who_id| Event::Left { who, who_id, dir: Some(left.clone()), how: None });
        self.people[from].retain(|k| *k != key);
        self.people[to].insert(0, key);
        self.chars.get_mut(key).unwrap().room = to;
        self.to_room(key, to, true, |who, who_id| Event::Arrived { who, who_id, from_dir: None, how: None });
        if !mob {
            self.look(key);
        }
        self.greet(key);
        self.followers_follow(key, from, d);
        true
    }

    /// Movement points a room's terrain costs (MECHANICS §2.3).
    pub(crate) fn sector_cost(&self, s: Sector) -> i32 {
        self.tables.world.movement_cost.get(&s).copied().unwrap_or(1)
    }
}
