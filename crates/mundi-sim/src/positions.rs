//! Positions: stand, sit, rest, sleep, wake (MECHANICS §4.3).

use crate::*;

impl Sim {
    /// stand, sit, rest, sleep, wake (MECHANICS §4.3, act.movement.c:731-950).
    pub(crate) fn position(&mut self, key: Key, cmd: PositionCommand, arg: &str) {
        let c = self.chars.get(key).unwrap();
        let (from, room) = (c.position, c.room);
        let refuse = |reason| Event::PositionRefused { command: cmd, reason };
        if cmd == PositionCommand::Wake && !arg.is_empty() {
            return self.wake_other(key, arg);
        }
        let to = match (cmd, from) {
            (PositionCommand::Stand, Position::Sitting | Position::Resting) => Position::Standing,
            (PositionCommand::Sit, Position::Standing | Position::Resting) => Position::Sitting,
            (PositionCommand::Rest, Position::Standing | Position::Sitting) => Position::Resting,
            (PositionCommand::Sleep, Position::Standing | Position::Sitting | Position::Resting) => Position::Sleeping,
            (PositionCommand::Wake, Position::Sleeping) if c.has(Affect::Sleep) => {
                return self.deliver(key, refuse(PositionRefusal::Magic));
            }
            (PositionCommand::Wake, Position::Sleeping) => Position::Sitting,
            (PositionCommand::Wake, _) => return self.deliver(key, refuse(PositionRefusal::Already)),
            (PositionCommand::Sleep, Position::Sleeping) => return self.deliver(key, refuse(PositionRefusal::Already)),
            (_, Position::Fighting) => return self.deliver(key, refuse(PositionRefusal::Fighting)),
            (_, Position::Sleeping) => return self.deliver(key, refuse(PositionRefusal::Asleep)),
            _ => return self.deliver(key, refuse(PositionRefusal::Already)),
        };
        self.chars.get_mut(key).unwrap().position = to;
        self.deliver(key, Event::SelfPosition { position: to, from, awakened_by: None });
        // "$n sits down." is the one shown to those who cannot see (as "Someone"): act(..., FALSE, ...).
        let must_see = !(to == Position::Sitting && from == Position::Standing);
        self.to_room(key, room, must_see, |who, who_id| Event::OccupantPosition { who, who_id, position: to, from });
    }

    pub(crate) fn wake_other(&mut self, key: Key, arg: &str) {
        let c = self.chars.get(key).unwrap();
        if c.position == Position::Sleeping {
            return self.deliver(key, Event::PositionRefused { command: PositionCommand::Wake, reason: PositionRefusal::Asleep });
        }
        let room = c.room;
        let target = self.people[room].iter().copied().find(|k| {
            self.can_see(key, *k) && self.chars.get(*k).is_some_and(|t| t.name.eq_ignore_ascii_case(arg))
        });
        let Some(target) = target else {
            return self.deliver(key, Event::Refused { reason: Refusal::NotHere });
        };
        if target == key {
            return self.position(key, PositionCommand::Wake, "");
        }
        let t = self.chars.get(target).unwrap();
        let (who, who_id) = (t.name.clone(), Some(char_id(&t.name)));
        let failure = if t.position > Position::Sleeping {
            Some(WakeFailure::AlreadyAwake)
        } else if t.has(Affect::Sleep) {
            Some(WakeFailure::Magic)
        } else if t.position < Position::Sleeping {
            Some(WakeFailure::BadShape)
        } else {
            None
        };
        if let Some(reason) = failure {
            return self.deliver(key, Event::WakeFailed { who, who_id, reason });
        }
        let me = self.chars.get(key).unwrap().name.clone();
        self.deliver(key, Event::Woke { who, who_id });
        self.chars.get_mut(target).unwrap().position = Position::Sitting;
        // "You are awakened by $n." reaches the sleeper (TO_SLEEP), named only if they could see them.
        let by = if self.can_see(target, key) { me } else { "someone".into() };
        self.deliver(target, Event::SelfPosition { position: Position::Sitting, from: Position::Sleeping, awakened_by: Some(by) });
    }
}
