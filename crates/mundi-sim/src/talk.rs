//! Talking (act.comm.c).

use crate::*;

impl Sim {
    /// act.comm.c:40-72: the room hears it first, then the speaker.
    pub(crate) fn say(&mut self, key: Key, text: &str) {
        if text.is_empty() {
            self.deliver(key, Event::Refused { reason: Refusal::NothingToSay });
            return;
        }
        let room = self.chars.get(key).unwrap().room;
        let said = text.to_string();
        self.to_room(key, room, false, move |from, from_id| Event::Say { from, from_id, text: said.clone(), direction: Direction::In });
        self.deliver(key, Event::Say { from: SELF.into(), from_id: None, text: text.into(), direction: Direction::Out });
    }
}
