//! Commands: tbaMUD's command table (third_party/tbamud/tables/commands.yaml), abbreviations, the
//! level and position each needs (MECHANICS §4.2), and which of them Mundi does so far.

use mundi_protocol::PositionCommand;

use crate::*;

impl Sim {
    pub(crate) fn command(&mut self, key: Key, text: &str) {
        // Any command ends hiding (interpreter.c:487, MECHANICS §3.5).
        self.chars.get_mut(key).unwrap().affects.retain(|a| *a != Affect::Hide);
        let text = text.trim_start();
        if text.is_empty() {
            return;
        }
        // A first character that is not a letter is a command by itself: `'hello` is say (§4.2).
        let (word, arg) = match text.chars().next() {
            Some(ch) if !ch.is_alphanumeric() => text.split_at(ch.len_utf8()),
            _ => text.split_once(char::is_whitespace).unwrap_or((text, "")),
        };
        let arg = arg.trim();
        let word = word.to_lowercase();
        let level = self.chars.get(key).unwrap().level;
        // The first non-social command the word begins, among those the level may use (interpreter.c:520-524).
        let entry = self.tables.commands.iter().find(|c| !c.social && c.name.starts_with(&word) && level >= c.level).cloned();
        let Some(entry) = entry else {
            self.deliver(key, Event::Refused { reason: Refusal::UnknownCommand });
            return;
        };
        let position = self.chars.get(key).unwrap().position;
        if position < protocol_position(entry.position) {
            self.deliver(key, Event::Refused { reason: refusal_for(position) });
            return;
        }
        // A mob's special in the room may take the command first (interpreter.c special).
        if self.special(key, &entry.name, arg) {
            return;
        }
        let dir = |d: usize| move |s: &mut Sim| {
            s.move_dir(key, d);
        };
        match entry.name.as_str() {
            "north" => dir(0)(self),
            "east" => dir(1)(self),
            "south" => dir(2)(self),
            "west" => dir(3)(self),
            "up" => dir(4)(self),
            "down" => dir(5)(self),
            "look" => {
                if arg.is_empty() {
                    self.look(key)
                } else {
                    self.deliver(key, Event::Refused { reason: Refusal::InvalidTarget })
                }
            }
            "say" | "'" => self.say(key, arg),
            "stand" => self.position(key, PositionCommand::Stand, arg),
            "sit" => self.position(key, PositionCommand::Sit, arg),
            "rest" => self.position(key, PositionCommand::Rest, arg),
            "sleep" => self.position(key, PositionCommand::Sleep, arg),
            "wake" => self.position(key, PositionCommand::Wake, arg),
            "qui" => self.deliver(key, Event::Refused { reason: Refusal::QuitInFull }),
            "quit" => {
                if position == Position::Fighting {
                    self.deliver(key, Event::Refused { reason: Refusal::Fighting });
                } else {
                    self.quit(key);
                }
            }
            "get" | "take" => self.get(key, arg),
            "drop" => self.drop_cmd(key, arg),
            "put" => self.put_cmd(key, arg),
            "give" => self.give(key, arg),
            "inventory" => self.inventory(key),
            "equipment" => self.equipment(key),
            "wear" => self.wear(key, arg),
            "wield" => self.wield(key, arg),
            "hold" | "grab" => self.hold(key, arg),
            "remove" => self.remove(key, arg),
            "eat" => self.eat(key, arg, false),
            "taste" => self.eat(key, arg, true),
            "drink" => self.drink(key, arg, false),
            "sip" => self.drink(key, arg, true),
            "hit" | "kill" => self.hit_cmd(key, arg),
            "flee" => self.flee_cmd(key),
            "toggle" => self.toggle(key, arg),
            "follow" => self.follow_cmd(key, arg),
            "unfollow" => self.unfollow(key),
            "group" => self.group_cmd(key, arg),
            "gsay" | "gtell" => self.gsay(key, arg),
            "report" => self.report(key),
            "split" => self.split(key, arg),
            "assist" => self.assist(key, arg),
            "kick" => self.kick(key, arg),
            "bash" => self.bash(key, arg),
            "rescue" => self.rescue(key, arg),
            "backstab" => self.backstab(key, arg),
            "cast" => self.cast(key, arg),
            "open" => self.door_cmd(key, crate::doors::DoorCmd::Open, arg),
            "close" => self.door_cmd(key, crate::doors::DoorCmd::Close, arg),
            "lock" => self.door_cmd(key, crate::doors::DoorCmd::Lock, arg),
            "unlock" => self.door_cmd(key, crate::doors::DoorCmd::Unlock, arg),
            "pick" => self.door_cmd(key, crate::doors::DoorCmd::Pick, arg),
            "practice" => self.practice_cmd(key, arg),
            "score" => self.score(key),
            "buy" | "sell" | "value" | "list" => self.deliver(key, Event::Refused { reason: Refusal::NotHereShop }),
            p if crate::group::PREFS.contains(&p) => self.auto_toggle(key, p),
            _ => self.deliver(key, Event::Refused { reason: Refusal::NotYet }),
        }
    }
}

pub(crate) fn protocol_position(p: mundi_content::names::Position) -> Position {
    use mundi_content::names::Position as P;
    match p {
        P::Dead => Position::Dead,
        P::MortallyWounded => Position::MortallyWounded,
        P::Incapacitated => Position::Incapacitated,
        P::Stunned => Position::Stunned,
        P::Sleeping => Position::Sleeping,
        P::Resting => Position::Resting,
        P::Sitting => Position::Sitting,
        P::Fighting => Position::Fighting,
        P::Standing => Position::Standing,
    }
}

/// MECHANICS §4.2 table: the refusal for a position too low for the command.
pub(crate) fn refusal_for(p: Position) -> Refusal {
    match p {
        Position::Dead => Refusal::Dead,
        Position::MortallyWounded | Position::Incapacitated => Refusal::Incapacitated,
        Position::Stunned => Refusal::Stunned,
        Position::Sleeping => Refusal::Sleeping,
        Position::Resting => Refusal::Resting,
        Position::Sitting => Refusal::Sitting,
        Position::Fighting | Position::Standing => Refusal::Fighting,
    }
}
