//! Perception (MECHANICS §3.1, §3.5, §3.6, D13): who can see whom, and who an event reaches.

use crate::*;

impl Sim {
    /// Whether a room is lit (MECHANICS §3.1): a light in it, else not `dark`, and indoors or in a
    /// city or in daytime.
    pub(crate) fn lit(&self, room: RoomIx) -> bool {
        if self.people[room].iter().any(|k| self.holds_light(*k)) {
            return true;
        }
        let r = &self.world.rooms[room];
        if r.flags.contains(&RoomFlag::Dark) {
            return false;
        }
        if matches!(r.sector, Sector::Inside | Sector::City) {
            return true;
        }
        !matches!(self.hour, 21..=23 | 0..=4)
    }

    /// The viewer's light condition (MECHANICS §3.5): not blind, and the room lit or infravision.
    pub(crate) fn can_see_in(&self, viewer: Key, room: RoomIx) -> bool {
        let Some(v) = self.chars.get(viewer) else { return false };
        !v.has(Affect::Blind) && (self.lit(room) || v.has(Affect::Infravision))
    }

    /// MECHANICS §3.5: oneself always; others with light, and past invisibility and hiding only
    /// with the senses for them.
    pub(crate) fn can_see(&self, viewer: Key, target: Key) -> bool {
        if viewer == target {
            return true;
        }
        let (Some(v), Some(t)) = (self.chars.get(viewer), self.chars.get(target)) else { return false };
        self.can_see_in(viewer, v.room)
            && (!t.has(Affect::Invisible) || v.has(Affect::DetectInvis))
            && (!t.has(Affect::Hide) || v.has(Affect::SenseLife))
    }

    /// An event about `actor` to everyone else in `room` who is awake. With `must_see` only those who
    /// can see the actor get it (act()'s hide_invisible); otherwise the rest get it with "someone".
    pub(crate) fn to_room(&mut self, actor: Key, room: RoomIx, must_see: bool, make: impl Fn(String, Option<String>) -> Event) {
        let name = self.chars.get(actor).map(|c| c.name.clone()).unwrap_or_default();
        for k in self.people[room].clone() {
            if k == actor {
                continue;
            }
            let Some(c) = self.chars.get(k) else { continue };
            if c.position <= Position::Sleeping || !c.linked {
                continue;
            }
            let seen = self.can_see(k, actor);
            if must_see && !seen {
                continue;
            }
            let event = if seen { make(name.clone(), Some(char_id(&name))) } else { make("someone".into(), None) };
            self.deliver(k, event);
        }
    }
}
