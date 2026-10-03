//! Looking (MECHANICS §3.2-3.4, §3.7).

use mundi_content::names::ObjFlag;
use mundi_protocol::RoomObject;

use crate::*;

impl Sim {
    /// MECHANICS §3.2.
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
            .filter(|k| **k != key && self.can_see(key, **k))
            .filter_map(|k| self.chars.get(*k).map(|c| (*k, c)))
            .filter(|(_, c)| !c.mob.as_ref().is_some_and(|m| m.long.starts_with('.')))
            .map(|(k, c)| Occupant {
                id: self.id_of(k),
                text: String::new(),
                name: c.name.clone(),
                // A mob in its default position shows its own line (MECHANICS §3.4).
                long: c.mob.as_ref().filter(|m| c.position == m.default_position && !m.long.is_empty()).map(|m| m.long.clone()),
                position: c.position,
                fighting: None,
                flags: [
                    (c.has(Affect::Invisible), "invisible"),
                    (c.has(Affect::Hide), "hidden"),
                    (!c.linked && !c.is_mob(), "linkless"),
                ]
                .iter()
                .filter(|(on, _)| *on)
                .map(|(_, f)| f.to_string())
                .collect(),
                hints: vec![],
                keywords: if c.is_mob() { c.keywords.clone() } else { vec![] },
            })
            .collect();
        let objects = self.object_list(key, &self.things[room].clone());
        let view = RoomView {
            id: Some(r.id.clone()),
            name: r.name.clone(),
            desc: r.desc.clone(),
            exits,
            objects,
            occupants,
            dark: false,
        };
        self.deliver(key, Event::Room(view));
    }

    /// A list of objects as a viewer sees it (MECHANICS §3.3): the ones they can see, the same
    /// object counted once with how many, a line starting with "." left out.
    pub(crate) fn object_list(&self, viewer: Key, list: &[Key]) -> Vec<RoomObject> {
        let detect = self.chars.get(viewer).is_some_and(|c| c.has(Affect::DetectInvis));
        let mut out: Vec<(Option<String>, String, RoomObject)> = Vec::new();
        for &k in list {
            let Some(o) = self.objs.get(k) else { continue };
            let invisible = o.flags.contains(&ObjFlag::Invisible);
            if (invisible && !detect) || o.long.starts_with('.') {
                continue;
            }
            if let Some((_, _, seen)) = out.iter_mut().find(|(p, short, _)| *p == o.proto && *short == o.short && o.proto.is_some()) {
                seen.count += 1;
                continue;
            }
            let flags = [
                (invisible, "invisible"),
                (o.flags.contains(&ObjFlag::Glow), "glow"),
                (o.flags.contains(&ObjFlag::Hum), "hum"),
            ]
            .iter()
            .filter(|(on, _)| *on)
            .map(|(_, f)| f.to_string())
            .collect();
            out.push((o.proto.clone(), o.short.clone(), RoomObject { id: self.obj_id(k), text: o.long.clone(), count: 1, keywords: o.keywords.clone(), flags }));
        }
        out.into_iter().map(|(_, _, r)| r).collect()
    }
}
