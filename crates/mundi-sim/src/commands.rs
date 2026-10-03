//! Commands: the table, abbreviations and the position each needs (MECHANICS §4.2).

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
        let Some(cmd) = COMMANDS.iter().find(|c| c.name.starts_with(&word)) else {
            self.deliver(key, Event::Refused { reason: Refusal::UnknownCommand });
            return;
        };
        let position = self.chars.get(key).unwrap().position;
        if position < cmd.min {
            self.deliver(key, Event::Refused { reason: refusal_for(position) });
            return;
        }
        match cmd.action {
            Action::Move(d) => self.move_dir(key, d),
            Action::Look => {
                if arg.is_empty() {
                    self.look(key)
                } else {
                    self.deliver(key, Event::Refused { reason: Refusal::InvalidTarget })
                }
            }
            Action::Say => self.say(key, arg),
            Action::Position(p) => self.position(key, p, arg),
            Action::QuitPrefix => self.deliver(key, Event::Refused { reason: Refusal::QuitInFull }),
            Action::Quit => {
                if position == Position::Fighting {
                    self.deliver(key, Event::Refused { reason: Refusal::Fighting });
                } else {
                    self.quit(key);
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Action {
    Move(usize),
    Look,
    QuitPrefix,
    Quit,
    Say,
    Position(PositionCommand),
}

pub(crate) struct Cmd {
    name: &'static str,
    min: Position,
    action: Action,
}

/// The commands so far, in tbaMUD's table order: the first whose name starts with what was typed wins
/// (interpreter.c:67-283, MECHANICS §2.1).
pub(crate) const COMMANDS: &[Cmd] = &[
    Cmd { name: "north", min: Position::Standing, action: Action::Move(0) },
    Cmd { name: "east", min: Position::Standing, action: Action::Move(1) },
    Cmd { name: "south", min: Position::Standing, action: Action::Move(2) },
    Cmd { name: "west", min: Position::Standing, action: Action::Move(3) },
    Cmd { name: "up", min: Position::Standing, action: Action::Move(4) },
    Cmd { name: "down", min: Position::Standing, action: Action::Move(5) },
    Cmd { name: "look", min: Position::Resting, action: Action::Look },
    Cmd { name: "qui", min: Position::Dead, action: Action::QuitPrefix },
    Cmd { name: "quit", min: Position::Dead, action: Action::Quit },
    Cmd { name: "rest", min: Position::Resting, action: Action::Position(PositionCommand::Rest) },
    Cmd { name: "say", min: Position::Resting, action: Action::Say },
    Cmd { name: "sit", min: Position::Resting, action: Action::Position(PositionCommand::Sit) },
    Cmd { name: "sleep", min: Position::Sleeping, action: Action::Position(PositionCommand::Sleep) },
    Cmd { name: "stand", min: Position::Resting, action: Action::Position(PositionCommand::Stand) },
    Cmd { name: "wake", min: Position::Sleeping, action: Action::Position(PositionCommand::Wake) },
    Cmd { name: "'", min: Position::Resting, action: Action::Say },
];

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
