//! The engine contract as Rust types: events, commands and the envelope of anima `PROTOCOL.md` part 1
//! (v0, plus the additions of D14: `id` on every entity, rendered text, exact numbers such as `damage`,
//! and the language a client asks for).
//!
//! Every other crate that moves events depends on this one, and this one depends on no other Mundi crate:
//! the simulation produces these types, the renderer and the network only consume them.
//!
//! Names and fields follow PROTOCOL.md so a Mundi event and a text-adapter event of the same thing are
//! the same JSON. Where Mundi knows more it fills the optional fields (`id`, `who_id`); where the text
//! adapter put a screen line (`Occupant.text`) Mundi gives the parts (`name`, `position`) and leaves the
//! sentence to the renderer.

use serde::{Deserialize, Serialize};

/// Protocol version (PROTOCOL.md §2).
pub const VERSION: u32 = 0;

/// What the perceiver calls themself (PROTOCOL.md §1).
pub const SELF: &str = "self";

/// One message to one recipient (PROTOCOL.md §2). `tick` and `text` are Mundi additions:
/// the simulation pulse the event happened in, and the sentences rendered for this recipient (D14).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Envelope {
    pub v: u32,
    pub t: f64,
    pub tick: u64,
    pub seq: u64,
    pub agent: String,
    #[serde(flatten)]
    pub event: Event,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub text: Vec<String>,
}

/// Languages a client may ask for (D14). Unknown or missing means English.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    #[default]
    En,
    Ko,
}

impl Lang {
    pub fn parse(s: &str) -> Lang {
        match s {
            "ko" => Lang::Ko,
            _ => Lang::En,
        }
    }
}

/// Body positions (MECHANICS §4.1), lowest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Position {
    Dead,
    MortallyWounded,
    Incapacitated,
    Stunned,
    Sleeping,
    Resting,
    Sitting,
    Fighting,
    Standing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoginStage {
    Name,
    Password,
    ConfirmName,
    PressReturn,
    Menu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoginFailure {
    InvalidName,
    WrongPassword,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InGameHow {
    Entered,
    Reconnected,
    TookOver,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloseReason {
    Remote,
    Local,
    Error,
}

/// Why a command was not carried out (PROTOCOL.md §3 `command.refused`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Refusal {
    Dead,
    Incapacitated,
    Stunned,
    Sleeping,
    Resting,
    Sitting,
    Fighting,
    UnknownCommand,
    NoTarget,
    Linkless,
    NotHere,
    NotAllowed,
    InvalidTarget,
    /// `say` with nothing to say. Mundi addition.
    NothingToSay,
    /// `qui`: quit must be typed in full (MECHANICS §4.2). Mundi addition.
    QuitInFull,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MoveFailure {
    NoExit,
    Closed,
    Locked,
    NeedBoat,
    Guarded,
    Forbidden,
    Exhausted,
    TooRelaxed,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    In,
    Out,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArrivedHow {
    EnteredGame,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeftHow {
    /// `$n has left the game.` Mundi addition, the counterpart of `ArrivedHow::EnteredGame`.
    LeftGame,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkState {
    Lost,
    Reconnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DayPhase {
    Sunrise,
    Day,
    Sunset,
    Night,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomExit {
    pub dir: String,
    pub closed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomObject {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub text: String,
    pub count: u32,
}

/// One being in a room as its perceiver sees it. `name` is what they are called in sentences
/// (a mob's short description, a player's name); `long` is a mob's own line when it stands in its
/// default position (MECHANICS §3.4), which the renderer shows as it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Occupant {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub long: Option<String>,
    pub position: Position,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fighting: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hints: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomView {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    /// The room's description, paragraphs separated by "\n". Empty when brief mode left it out.
    pub desc: String,
    pub exits: Vec<RoomExit>,
    pub objects: Vec<RoomObject>,
    pub occupants: Vec<Occupant>,
    pub dark: bool,
}

/// Game events (PROTOCOL.md §3). Only the ones the simulation produces so far; the rest are added
/// with the rules that cause them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Event {
    #[serde(rename = "connection.login_prompt")]
    LoginPrompt { stage: LoginStage },
    #[serde(rename = "connection.login_failed")]
    LoginFailed { reason: LoginFailure },
    #[serde(rename = "connection.in_game")]
    InGame { how: InGameHow },
    #[serde(rename = "connection.closed")]
    Closed {
        reason: CloseReason,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    #[serde(rename = "prompt")]
    Prompt { hp: Option<i32>, mp: Option<i32>, mv: Option<i32> },
    #[serde(rename = "command.refused")]
    Refused { reason: Refusal },
    #[serde(rename = "room")]
    Room(RoomView),
    #[serde(rename = "room.dark")]
    RoomDark {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        blind: bool,
    },
    #[serde(rename = "move.failed")]
    MoveFailed {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dir: Option<String>,
        reason: MoveFailure,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        door: Option<String>,
    },
    #[serde(rename = "zone.above_level")]
    ZoneAboveLevel {},
    #[serde(rename = "occupant.arrived")]
    Arrived {
        who: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        who_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from_dir: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        how: Option<ArrivedHow>,
    },
    #[serde(rename = "occupant.left")]
    Left {
        who: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        who_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dir: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        how: Option<LeftHow>,
    },
    #[serde(rename = "occupant.link")]
    Link {
        who: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        who_id: Option<String>,
        state: LinkState,
    },
    #[serde(rename = "world.time")]
    WorldTime { phase: DayPhase },
    #[serde(rename = "comm.say")]
    Say {
        from: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from_id: Option<String>,
        text: String,
        direction: Direction,
    },
}

/// A text command from a client (PROTOCOL.md §4, the part the engine sees: `source` and `secret`
/// are the runtime's and never reach the simulation).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Command {
    pub text: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_is_the_protocol_shape() {
        let e = Envelope {
            v: VERSION,
            t: 1.5,
            tick: 7,
            seq: 1,
            agent: "Vallen".into(),
            event: Event::Say { from: SELF.into(), from_id: None, text: "hi".into(), direction: Direction::Out },
            text: vec!["You say, 'hi'".into()],
        };
        let json = serde_json::to_value(&e).unwrap();
        assert_eq!(json["type"], "comm.say");
        assert_eq!(json["data"]["from"], "self");
        assert_eq!(json["data"]["direction"], "out");
        assert_eq!(json["text"][0], "You say, 'hi'");
        let back: Envelope = serde_json::from_value(json).unwrap();
        assert_eq!(back, e);
    }

    #[test]
    fn empty_struct_events_and_optional_fields() {
        let json = serde_json::to_string(&Event::ZoneAboveLevel {}).unwrap();
        assert_eq!(json, r#"{"type":"zone.above_level","data":{}}"#);
        let json = serde_json::to_string(&Event::RoomDark { id: None, blind: false }).unwrap();
        assert_eq!(json, r#"{"type":"room.dark","data":{}}"#);
    }
}
