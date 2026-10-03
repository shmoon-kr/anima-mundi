//! Looking (MECHANICS §3.2-3.4, §3.7).

use crate::*;

impl Sim {
    /// MECHANICS §3.2. Objects and mobs come with the zone resets (S5).
    pub(crate) fn look(&mut self, key: Key) {
        let me = self.chars.get(key).unwrap();
        let room = me.room;
        let r = &self.world.rooms[room];
        if me.has(Affect::Blind) {
            self.deliver(key, Event::RoomDark { id: Some(r.id.clone()), blind: true, glowing_eyes: 0 });
            return;
        }
        if !self.can_see_in(key, room) {
            // In the dark, those with infravision show as eyes (act.informative.c list_char_to_char).
            let eyes = self.people[room]
                .iter()
                .filter(|k| **k != key && !self.can_see(key, **k))
                .filter(|k| self.chars.get(**k).is_some_and(|c| c.has(Affect::Infravision)))
                .count() as u32;
            self.deliver(key, Event::RoomDark { id: Some(r.id.clone()), blind: false, glowing_eyes: eyes });
            return;
        }
        let exits = r
            .exits
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                let e = e.as_ref()?;
                let to = e.to?;
                let closed = e.door.as_ref().is_some_and(|d| d.state != DoorState::Open);
                Some(RoomExit { dir: DIRS[i].into(), closed, to_id: Some(self.world.rooms[to].id.clone()) })
            })
            .collect();
        let occupants = self.people[room]
            .iter()
            .filter(|k| **k != key)
            .filter_map(|k| self.chars.get(*k).map(|c| (k, c)))
            .filter(|(k, _)| self.can_see(key, **k))
            .map(|(_, c)| Occupant {
                id: Some(char_id(&c.name)),
                text: String::new(),
                name: c.name.clone(),
                long: None,
                position: c.position,
                fighting: None,
                flags: [
                    (c.has(Affect::Invisible), "invisible"),
                    (c.has(Affect::Hide), "hidden"),
                    (!c.linked, "linkless"),
                ]
                .iter()
                .filter(|(on, _)| *on)
                .map(|(_, f)| f.to_string())
                .collect(),
                hints: vec![],
                keywords: vec![],
            })
            .collect();
        let view = RoomView {
            id: Some(r.id.clone()),
            name: r.name.clone(),
            desc: r.desc.clone(),
            exits,
            objects: vec![],
            occupants,
            dark: false,
        };
        self.deliver(key, Event::Room(view));
    }
}
