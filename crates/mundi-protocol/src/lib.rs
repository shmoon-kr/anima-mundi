//! The engine contract as Rust types: events, commands and the envelope of anima `PROTOCOL.md` part 1
//! (v0, plus the additions of D14: `id` on every entity, rendered text, exact numbers such as `damage`,
//! and the language a client asks for).
//!
//! Every other crate that moves events depends on this one, and this one depends on no other Mundi crate:
//! the simulation produces these types, the renderer and the network only consume them.
//!
//! Names and fields follow PROTOCOL.md so a Mundi event and a text-adapter event of the same thing are
//! the same JSON. Mundi only adds (D22): where it knows more it fills the optional fields (`id`, `who_id`)
//! and puts new fields beside the old ones (`Occupant.name` beside `Occupant.text`), never instead.

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

/// Where a Korean screen shows the English keyword beside a name (D18): only where a command can
/// name it (`targets`, the default), everywhere, or nowhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeywordMode {
    Always,
    #[default]
    Targets,
    Off,
}

/// For English pronouns (he, she, it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Sex {
    #[default]
    Neutral,
    Male,
    Female,
}

/// How a name ends for Korean particles, when the rule gets it wrong (D23: "Bob" is 밥).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KoFinal {
    None,
    Rieul,
    Other,
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
    /// A tbaMUD command Mundi does not do yet. Mundi addition, until phase 1 is complete.
    NotYet,
    /// A shop command outside a shop: "Sorry, but you cannot do that here!".
    NotHereShop,
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

/// The position commands (MECHANICS §4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PositionCommand {
    Stand,
    Sit,
    Rest,
    Sleep,
    Wake,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PositionRefusal {
    /// Already in that position (or awake, for wake).
    Already,
    /// Asleep: wake up first.
    Asleep,
    Fighting,
    /// Magical sleep.
    Magic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WakeFailure {
    AlreadyAwake,
    Magic,
    BadShape,
}

/// What someone did with an object (MECHANICS §13, §6.2, §6.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemAction {
    Get,
    Drop,
    Put,
    Give,
    Wear,
    Wield,
    Hold,
    Remove,
    Eat,
    Taste,
    Drink,
    Sip,
}

/// Why an object command did nothing. The renderer picks tbaMUD's sentence by action and reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemFailure {
    /// No argument ("Get what?").
    What,
    /// `all.` with nothing after it ("Get all of what?").
    AllOfWhat,
    /// Not in the room ("You don't see a sword here.").
    NotHere,
    /// Not carried ("You don't seem to have a sword.").
    NotCarried,
    /// Not worn ("You don't seem to be using a sword.").
    NotUsing,
    /// `all.<word>` matched nothing ("You don't seem to have any swords.").
    NoneOf,
    /// `all` matched nothing.
    Nothing,
    CantTake,
    TooMany,
    TooHeavy,
    /// A container in hand is full by count ("$p: you can't hold any more items.").
    HoldNoMore,
    NotContainer,
    Closed,
    Empty,
    /// The container was not found ("You don't have a bag.").
    NoContainer,
    /// Corpses take nothing in.
    IntoCorpse,
    WontFit,
    IntoItself,
    /// put with no container named.
    IntoWhat,
    Cursed,
    /// A cursed thing into a container on the floor.
    OutOfHand,
    NoPerson,
    /// give with no person named.
    ToWho,
    GiveSelf,
    HandsFull,
    CantCarry,
    Level,
    CantWear,
    CantWearThere,
    /// Something is already in that slot (`slot` says which).
    AlreadyWearing,
    BadLocation,
    CantWield,
    TooHeavyToWield,
    CantHold,
    NotFood,
    TooFull,
    CantFind,
    CantDrink,
    MustHold,
    MissMouth,
    StomachFull,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Carried {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub text: String,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Worn {
    pub slot: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub text: String,
}

/// Which lines of a combat message a hit used (MECHANICS §7.6, §8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HitOutcome {
    Miss,
    Hit,
    Die,
    God,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HitKind {
    Weapon,
    Skill,
    Spell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownState {
    Stunned,
    Incapacitated,
    MortallyWounded,
}

/// Why an attack did not start (MECHANICS §7.2, §8.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttackRefusal {
    /// hit with no target.
    Who,
    NotHere,
    /// Already fighting: one cannot switch with hit.
    AlreadyFighting,
    Peaceful,
    Protected,
    /// do_hit's "Player killing is not allowed.".
    NoPlayerKilling,
    /// damage()'s "Player killing is not permitted.".
    NotPermitted,
    /// Charmed, attacking the master.
    Friend,
    /// "You can't fight while sitting!!": a round lost for being down.
    Sitting,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupMember {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub hp: i32,
    pub hp_max: i32,
    pub mp: i32,
    pub mp_max: i32,
    pub mv: i32,
    pub mv_max: i32,
    pub leader: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShopItem {
    pub index: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub text: String,
    pub price: i64,
    /// None: made to order ("Unlimited").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
    /// A drink container's liquid ("a bottle of beer").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub liquid: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Known {
    pub name: String,
    pub percent: i32,
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
    /// Words a command can name it by, in the content's order. Mundi addition.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
    /// What tbaMUD shows after it: invisible, glow, hum. Mundi addition.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<String>,
}

/// One being in a room as its perceiver sees it. `text` is the screen line (PROTOCOL.md v0): the
/// simulation writes no sentence and leaves it empty, and the server fills it with the English line,
/// markup removed, before it goes out (D22). The rest are Mundi's parts of that line: `name` is what
/// they are called in sentences (a mob's short description, a player's name); `long` is a mob's own
/// line when it stands in its default position (MECHANICS §3.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Occupant {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default)]
    pub text: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub long: Option<String>,
    pub position: Position,
    /// Whom they fight (PROTOCOL.md): `self` for the reader, else the opponent as the reader sees
    /// them; empty when the opponent is no longer here; None when not fighting anyone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fighting: Option<String>,
    /// The opponent's ID, so a renderer can name them in its language. Mundi addition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fighting_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hints: Vec<String>,
    /// Words a command can name it by (a mob's keywords; empty for players, whose name is the word).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
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
        /// Beings with infravision looking this way, seen only as eyes (MECHANICS §3.4). Mundi addition.
        #[serde(default, skip_serializing_if = "is_zero")]
        glowing_eyes: u32,
    },
    /// My own position changed (PROTOCOL.md `position`); `from` is a Mundi addition.
    #[serde(rename = "position")]
    SelfPosition {
        position: Position,
        from: Position,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        awakened_by: Option<String>,
    },
    #[serde(rename = "occupant.position")]
    OccupantPosition {
        who: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        who_id: Option<String>,
        position: Position,
        from: Position,
    },
    /// A position command that changed nothing (MECHANICS §4.3). Mundi addition.
    #[serde(rename = "position.refused")]
    PositionRefused { command: PositionCommand, reason: PositionRefusal },
    /// I woke someone (`wake <name>`). Mundi addition.
    #[serde(rename = "occupant.woken")]
    Woke {
        who: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        who_id: Option<String>,
    },
    #[serde(rename = "occupant.wake_failed")]
    WakeFailed {
        who: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        who_id: Option<String>,
        reason: WakeFailure,
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
    /// Hunger and thirst at zero (each tick while so), sober again (PROTOCOL.md `condition`;
    /// `sober` is a Mundi addition).
    #[serde(rename = "condition")]
    Condition {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hungry: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        thirsty: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        full: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        quenched: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sober: Option<bool>,
        /// "You feel drunk." Mundi addition.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        drunk: Option<bool>,
    },
    /// A light down to its last hour (Mundi addition) or out (PROTOCOL.md `items.light_out`).
    #[serde(rename = "items.light_flicker")]
    LightFlicker {
        who: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        who_id: Option<String>,
    },
    #[serde(rename = "items.light_out")]
    LightOut {
        who: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        who_id: Option<String>,
    },
    #[serde(rename = "items.got")]
    Got {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from_id: Option<String>,
    },
    #[serde(rename = "items.received")]
    Received {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        from: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from_id: Option<String>,
    },
    #[serde(rename = "items.gave")]
    Gave {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        to: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to_id: Option<String>,
    },
    /// I did something with an object. `into` is the container (put, drink from), `slot` where it
    /// went (wear), `liquid` what was drunk. Mundi fills more than PROTOCOL.md `items.used`.
    #[serde(rename = "items.used")]
    Used {
        action: ItemAction,
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        into: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        into_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        slot: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        liquid: Option<String>,
    },
    /// Someone else did something with an object (Mundi addition): `other` is the container or
    /// the person given to.
    #[serde(rename = "occupant.item")]
    OccupantItem {
        who: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        who_id: Option<String>,
        action: ItemAction,
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        other: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        other_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        slot: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        liquid: Option<String>,
    },
    /// Coins picked up turn into money at once ("There were 12 coins."). Mundi addition.
    #[serde(rename = "items.coins")]
    Coins { amount: i64 },
    #[serde(rename = "items.failed")]
    ItemFailed {
        action: ItemAction,
        reason: ItemFailure,
        /// The object (its short description), when there is one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// The word as typed, for "You don't see a sword here.".
        #[serde(default, skip_serializing_if = "Option::is_none")]
        keyword: Option<String>,
        /// The container or the person.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        other: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        other_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        slot: Option<String>,
    },
    /// Put on and at once dropped back into the pack: wrong alignment or class (MECHANICS §13.3).
    #[serde(rename = "items.zapped")]
    Zapped {
        who: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        who_id: Option<String>,
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
    },
    /// Poisoned food or drink: "Oops, that tasted rather strange!" and the room's cough.
    #[serde(rename = "items.tasted_strange")]
    TastedStrange {
        who: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        who_id: Option<String>,
        drink: bool,
    },
    #[serde(rename = "items.inventory")]
    Inventory { items: Vec<Carried> },
    #[serde(rename = "items.equipment")]
    Equipment { slots: Vec<Worn> },
    /// One blow (PROTOCOL.md `combat.hit`). `attacker`/`victim` are "self" for the recipient.
    /// Mundi gives the exact damage, the attack number, and which message-file lines and variant
    /// were used, so every viewer's sentence is the same blow.
    #[serde(rename = "combat.hit")]
    Hit {
        attacker: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        attacker_id: Option<String>,
        victim: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        victim_id: Option<String>,
        /// The attack's word (slash, pierce...; a skill's or spell's name).
        verb: String,
        severity: u8,
        kind: HitKind,
        damage: i32,
        attack: i32,
        outcome: HitOutcome,
        /// The message file's variant used, if the message came from it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        variant: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        weapon: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        weapon_id: Option<String>,
    },
    #[serde(rename = "combat.condition")]
    CombatCondition {
        who: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        who_id: Option<String>,
        state: DownState,
    },
    /// Someone else died (PROTOCOL.md `combat.death`).
    #[serde(rename = "combat.death")]
    Death {
        who: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        who_id: Option<String>,
    },
    #[serde(rename = "combat.death_cry")]
    DeathCry {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        who: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        who_id: Option<String>,
        nearby: bool,
    },
    #[serde(rename = "self.died")]
    SelfDied {},
    /// "That really did HURT!" (`hurt`) or "...BLEEDING so much!" (`bleeding`). Mundi addition.
    #[serde(rename = "combat.pain")]
    Pain { bleeding: bool },
    #[serde(rename = "self.fled")]
    SelfFled {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dir: Option<String>,
    },
    #[serde(rename = "self.flee_failed")]
    FleeFailed { reason: String },
    #[serde(rename = "combat.wimpy")]
    Wimpy {},
    /// "$n panics, and attempts to flee!" (`failed: false`) or "$n tries to flee, but can't!".
    #[serde(rename = "combat.flee_seen")]
    FleeSeen {
        who: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        who_id: Option<String>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        failed: bool,
    },
    #[serde(rename = "combat.aggro")]
    Aggro {
        who: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        who_id: Option<String>,
        reason: String,
    },
    /// "$n slowly fades into existence." Mundi addition.
    /// "$n jumps to the aid of $N!" (PROTOCOL.md `combat.assist`).
    #[serde(rename = "combat.assist")]
    Assisted {
        who: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        who_id: Option<String>,
        target: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target_id: Option<String>,
        /// A helper mob's "jumps to the aid of" rather than assist's "assists".
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        jumped: bool,
    },
    /// PROTOCOL.md `group.change`: joined, leader (became leader of a new group), new_leader (took
    /// over), left, kicked (I kicked `who`), kicked_out (I was), following, followed_by,
    /// stopped_following, follower_left.
    #[serde(rename = "group.change")]
    GroupChange {
        event: String,
        who: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        who_id: Option<String>,
        /// `new_leader` of a group just made ("becomes leader"), not one that passed on ("has
        /// assumed leadership"). Mundi addition: tbaMUD has two sentences, PROTOCOL.md one event.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        formed: bool,
    },
    /// Someone else starts or stops following someone (Mundi addition).
    #[serde(rename = "occupant.follow")]
    OccupantFollow {
        who: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        who_id: Option<String>,
        leader: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        leader_id: Option<String>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        stopped: bool,
    },
    /// "You follow $N." (PROTOCOL.md `follow.moved`).
    #[serde(rename = "follow.moved")]
    FollowMoved {
        leader: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        leader_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dir: Option<String>,
    },
    #[serde(rename = "group.status")]
    GroupStatus { members: Vec<GroupMember> },
    /// "%s reports: ..." to the group (Mundi addition).
    #[serde(rename = "group.report")]
    GroupReport { member: GroupMember },
    #[serde(rename = "group.option")]
    GroupOption {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        open: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        anonymous: Option<bool>,
    },
    /// Why a group, follow, split or assist command did nothing; `who` when it names someone.
    #[serde(rename = "group.failed")]
    GroupFailed {
        reason: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        who: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        who_id: Option<String>,
    },
    #[serde(rename = "comm.gtell")]
    Gtell {
        from: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from_id: Option<String>,
        text: String,
        direction: Direction,
    },
    /// Gold shared (act.other.c do_split): `from` is "self" for the splitter. Mundi addition.
    #[serde(rename = "items.split")]
    Split {
        from: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from_id: Option<String>,
        amount: i64,
        share: i64,
        rest: i64,
        members: i64,
    },
    #[serde(rename = "combat.appear")]
    Appear {
        who: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        who_id: Option<String>,
    },
    #[serde(rename = "combat.refused")]
    AttackRefused { reason: AttackRefusal },
    /// "You hit yourself...OUCH!." and the room's "$n hits $mself, and says OUCH!". Mundi addition.
    #[serde(rename = "combat.self_hit")]
    SelfHit {
        who: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        who_id: Option<String>,
    },
    #[serde(rename = "exp.gain")]
    ExpGain { amount: i64, kind: String },
    #[serde(rename = "level.up")]
    LevelUp { levels: i32 },
    /// A corpse rotting away (MECHANICS §8.4). Mundi addition.
    #[serde(rename = "items.decayed")]
    Decayed {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        carried: bool,
    },
    /// A spell's last affect gone: its wear-off line (magic.c affect_update). Mundi addition.
    #[serde(rename = "affect.wore_off")]
    WoreOff { spell: i32, name: String },
    /// A skill's or spell's outcome when it is not a blow (PROTOCOL.md `skill.result`); `reason`
    /// names tbaMUD's sentence, `who` the person it names.
    #[serde(rename = "skill.result")]
    SkillResult {
        skill: String,
        ok: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        who: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        who_id: Option<String>,
    },
    /// The rescue's lines: `rescuer` and `rescued` are "self" for the recipient. Mundi addition.
    #[serde(rename = "combat.rescue")]
    Rescue {
        rescuer: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rescuer_id: Option<String>,
        rescued: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rescued_id: Option<String>,
    },
    /// A backstab noticed by an aware mob (act.offensive.c:158-163). Mundi addition.
    #[serde(rename = "combat.noticed")]
    Noticed {
        who: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        who_id: Option<String>,
        by: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        by_id: Option<String>,
    },
    /// The words of a spell (spell_parser.c say_spell): `words` are the spell's name to those of the
    /// caster's class, its syllables to the rest. Mundi addition.
    #[serde(rename = "spell.said")]
    SpellSaid {
        who: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        who_id: Option<String>,
        words: String,
        /// Whom or what it is cast at ("self" for the recipient), if not the caster.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target_id: Option<String>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        at_object: bool,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        at_self: bool,
    },
    /// A spell's line: `line` is "vict" (to the one it is on), "room" (the rest) or "steams" (an
    /// object). Mundi addition.
    #[serde(rename = "spell.effect")]
    SpellEffect {
        spell: i32,
        line: String,
        who: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        who_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
    },
    /// A door or container opened, closed, locked, unlocked or picked (MECHANICS §2.7). `who` is
    /// "self" for the one who did it; `door` names an exit's door, `text` a container; `far` is
    /// the room on the other side hearing it. Mundi addition.
    #[serde(rename = "door.changed")]
    DoorChanged {
        command: String,
        who: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        who_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        door: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        far: bool,
    },
    #[serde(rename = "door.failed")]
    DoorFailed {
        command: String,
        reason: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        word: Option<String>,
    },
    /// PROTOCOL.md `comm.tell`. Shopkeepers talk to customers this way.
    #[serde(rename = "comm.tell")]
    Tell {
        from: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from_id: Option<String>,
        to: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to_id: Option<String>,
        text: String,
        direction: Direction,
    },
    /// PROTOCOL.md `shop.list`.
    #[serde(rename = "shop.list")]
    ShopList {
        items: Vec<ShopItem>,
        /// A name was given and nothing matched it.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        none_matching: bool,
    },
    /// A purchase or sale done: `text` is "a bread" or "a bread (x 3)"; `who` is "self" for the
    /// customer. PROTOCOL.md `shop.result` with Mundi fields.
    #[serde(rename = "shop.result")]
    ShopResult {
        action: String,
        who: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        who_id: Option<String>,
        text: String,
    },
    /// "The guard humiliates you, and blocks your way." Mundi addition.
    #[serde(rename = "move.blocked")]
    Blocked {
        who: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        who_id: Option<String>,
    },
    /// PROTOCOL.md `char.vitals_max`: the maxima `score` shows.
    #[serde(rename = "char.vitals_max")]
    VitalsMax { hp: i32, mp: i32, mv: i32 },
    /// PROTOCOL.md `char.score`: what `score` says (MECHANICS §18). The renderer makes its lines.
    #[serde(rename = "char.score")]
    Score(Box<Score>),
    /// The practice list (PROTOCOL.md `char.skills`, with numbers and sessions left).
    #[serde(rename = "char.skills")]
    Skills { practices: i32, spells: bool, skills: Vec<Known> },
    /// PROTOCOL.md `char.practiced`: improved, learned, maxed, cannot (reason: no_practices,
    /// unknown_skill, not_here).
    #[serde(rename = "char.practiced")]
    Practiced {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        skill: Option<String>,
        result: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
        spells: bool,
    },
    /// A mob's emote ("$n savagely devours a corpse."). Mundi addition.
    #[serde(rename = "comm.emote")]
    Emote {
        who: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        who_id: Option<String>,
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        line: Option<LineRef>,
    },
    /// A script's line, already with its names (zone echoes, sends): the world's text. Mundi addition.
    #[serde(rename = "world.echo")]
    Echo {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        line: Option<LineRef>,
    },
    /// tbaMUD's start message for a character's first entry (config.c START_MESSG). Mundi addition.
    #[serde(rename = "connection.new_character")]
    NewCharacter {},
    #[serde(rename = "toggle.state")]
    Toggle {
        name: String,
        value: serde_json::Value,
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
        /// A mob's line from the content (a trigger's), for readers in another language. Mundi addition.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        line: Option<LineRef>,
    },
}

/// `score`'s values (MECHANICS §18).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Score {
    pub age: i32,
    pub birthday: bool,
    pub hp: i32,
    pub hp_max: i32,
    pub mp: i32,
    pub mp_max: i32,
    pub mv: i32,
    pub mv_max: i32,
    pub ac: i32,
    pub alignment: i32,
    pub exp: i64,
    pub gold: i64,
    pub quest_points: i32,
    /// None at immortal levels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exp_to_next: Option<i64>,
    pub quests: i32,
    pub played_days: i64,
    pub played_hours: i64,
    pub name: String,
    pub title: String,
    pub level: i32,
    pub position: Position,
    /// Whom they fight, as they see them ("thin air" when no one).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fighting: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fighting_id: Option<String>,
    /// In do_score's order: intoxicated, hungry, thirsty, blind, invisible, detect_invisible,
    /// sanctuary, poisoned, charmed, armored, infravision.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub states: Vec<String>,
}

/// The content line an event's English text came from: a trigger's ID and the line's key there
/// (`tba:30:trg:3016`, `full` or `kit.3`), and the names that fill its `%s` in order. A renderer
/// shows its own language's line when it has one, else the text. Mundi addition (D22).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LineRef {
    pub id: String,
    pub key: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub names: Vec<Named>,
}

/// A being named in a line: as the event names it (`self` for the reader) and its ID.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Named {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// What a client sends, one JSON object per WebSocket text frame. A Mundi addition: the text adapter
/// talks telnet. `command` carries the text of PROTOCOL.md §4 `command.send` (its `source` and `secret`
/// are the runtime's and never reach the engine).
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Logs in, creating the character the first time a name is used. `lang` picks the language of
    /// the rendered text (D14); `plain` asks for text without colour codes (agents, D20).
    /// `sex` and `ko_final` are kept for a new character (tbaMUD asks the sex when a name is new).
    Login {
        name: String,
        password: String,
        #[serde(default)]
        lang: Lang,
        #[serde(default)]
        plain: bool,
        #[serde(default)]
        keywords: KeywordMode,
        #[serde(default)]
        sex: Option<Sex>,
        #[serde(default)]
        ko_final: Option<KoFinal>,
        /// magic_user, cleric, thief or warrior (default warrior), for a new character.
        #[serde(default)]
        class: Option<String>,
    },
    Command { text: String },
    /// Display settings, any time after login.
    Settings {
        #[serde(default)]
        keywords: Option<KeywordMode>,
        #[serde(default)]
        lang: Option<Lang>,
        #[serde(default)]
        plain: Option<bool>,
    },
}

/// Never prints the password (PROTOCOL.md §5).
impl std::fmt::Debug for ClientMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientMessage::Login { name, lang, plain, keywords, sex, ko_final, class, .. } => f
                .debug_struct("Login")
                .field("name", name)
                .field("password", &"***")
                .field("lang", lang)
                .field("plain", plain)
                .field("keywords", keywords)
                .field("sex", sex)
                .field("ko_final", ko_final)
                .field("class", class)
                .finish(),
            ClientMessage::Command { text } => f.debug_struct("Command").field("text", text).finish(),
            ClientMessage::Settings { keywords, lang, plain } => {
                f.debug_struct("Settings").field("keywords", keywords).field("lang", lang).field("plain", plain).finish()
            }
        }
    }
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
            event: Event::Say { from: SELF.into(), from_id: None, text: "hi".into(), direction: Direction::Out, line: None },
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
    fn client_messages_hide_the_password() {
        let m: ClientMessage = serde_json::from_str(r#"{"type":"login","name":"Ana","password":"hunter2","lang":"ko"}"#).unwrap();
        assert!(matches!(&m, ClientMessage::Login { lang: Lang::Ko, plain: false, .. }));
        assert!(!format!("{m:?}").contains("hunter2"));
        let c: ClientMessage = serde_json::from_str(r#"{"type":"command","text":"look"}"#).unwrap();
        assert_eq!(c, ClientMessage::Command { text: "look".into() });
    }

    #[test]
    fn empty_struct_events_and_optional_fields() {
        let json = serde_json::to_string(&Event::ZoneAboveLevel {}).unwrap();
        assert_eq!(json, r#"{"type":"zone.above_level","data":{}}"#);
        let json = serde_json::to_string(&Event::RoomDark { id: None, blind: false, glowing_eyes: 0 }).unwrap();
        assert_eq!(json, r#"{"type":"room.dark","data":{}}"#);
    }
}
