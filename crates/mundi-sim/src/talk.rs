//! Talking (act.comm.c).

use crate::*;

impl Sim {
    /// act.comm.c:40-72: the room hears it first, then the speaker.
    pub(crate) fn say(&mut self, key: Key, text: &str) {
        self.say_line(key, text, None);
    }

    /// A say whose words are a content line (a mob's script), so readers can have it translated.
    pub(crate) fn say_line(&mut self, key: Key, text: &str, line: Option<LineRef>) {
        if text.is_empty() {
            self.deliver(key, Event::Refused { reason: Refusal::NothingToSay });
            return;
        }
        let room = self.chars.get(key).unwrap().room;
        let said = text.to_string();
        let heard = line.clone();
        self.to_room(key, room, false, move |from, from_id| Event::Say { from, from_id, text: said.clone(), direction: Direction::In, line: heard.clone() });
        self.deliver(key, Event::Say { from: SELF.into(), from_id: None, text: text.into(), direction: Direction::Out, line });
    }
}
