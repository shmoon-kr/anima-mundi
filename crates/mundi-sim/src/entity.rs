//! Characters (players and mobs) and objects: the state the rules change. A player's lasting part is
//! a [`Save`], which the store keeps and `Input::Enter` brings back, so a replay needs nothing else.

use std::collections::{BTreeMap, VecDeque};

use mundi_content::names::{Affect, EquipPos, ItemType, Liquid, MobFlag, ObjFlag, Wear};
use mundi_content::{Amount, Id};
use mundi_protocol::{Position, Sex};
use serde::{Deserialize, Serialize};

use crate::store::Key;
use crate::world::RoomIx;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Class {
    MagicUser,
    Cleric,
    Thief,
    Warrior,
}

impl Class {
    /// The class's name in the tables (`classes.yaml`).
    pub fn key(self) -> &'static str {
        match self {
            Class::MagicUser => "magic_user",
            Class::Cleric => "cleric",
            Class::Thief => "thief",
            Class::Warrior => "warrior",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Abilities {
    pub str: i32,
    /// 18/xx strength, 0 when none.
    pub str_add: i32,
    pub int: i32,
    pub wis: i32,
    pub dex: i32,
    pub con: i32,
    pub cha: i32,
}

/// Hunger, thirst, drunkenness (MECHANICS §6.1): 0..24, -1 never changes (mobs, immortals).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conditions {
    pub drunk: i32,
    pub full: i32,
    pub thirst: i32,
}

impl Conditions {
    pub const NEVER: Conditions = Conditions { drunk: -1, full: -1, thirst: -1 };
}

/// What a mob keeps of its prototype.
#[derive(Debug, Clone)]
pub(crate) struct MobPart {
    pub proto: Id,
    pub serial: u64,
    pub flags: Vec<MobFlag>,
    pub default_position: Position,
    pub long: String,
    pub damage: (i32, i32),
    pub exp: i64,
}

#[derive(Debug, Clone)]
pub(crate) struct Char {
    /// A player's name or a mob's short description ("the pit beast").
    pub name: String,
    pub keywords: Vec<String>,
    pub mob: Option<MobPart>,
    pub room: RoomIx,
    pub position: Position,
    pub class: Option<Class>,
    pub sex: Sex,
    pub level: i32,
    pub exp: i64,
    pub gold: i64,
    pub alignment: i32,
    pub abilities: Abilities,
    pub hp: i32,
    pub max_hp: i32,
    pub mana: i32,
    pub max_mana: i32,
    pub mv: i32,
    pub max_mv: i32,
    pub hitroll: i32,
    pub damroll: i32,
    /// Base armor class, before armor and dexterity (MECHANICS §7.4).
    pub armor: i32,
    pub conditions: Conditions,
    pub practices: i32,
    pub skills: BTreeMap<String, i32>,
    /// The tick the character was born on, for age (MECHANICS §17.2).
    pub born: i64,
    pub affects: Vec<Affect>,
    pub inventory: Vec<Key>,
    pub equipment: BTreeMap<EquipPos, Key>,
    pub linked: bool,
    pub queue: VecDeque<String>,
}

impl Char {
    pub fn has(&self, a: Affect) -> bool {
        self.affects.contains(&a)
    }

    pub fn is_mob(&self) -> bool {
        self.mob.is_some()
    }

    pub fn has_flag(&self, f: MobFlag) -> bool {
        self.mob.as_ref().is_some_and(|m| m.flags.contains(&f))
    }
}

/// An object's mutable values: what the content gives, as the rules change it (a drink container's
/// contents, a light's hours, a corpse's timer).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjValues {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hours: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capacity: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contains: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub liquid: Option<Liquid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poisoned: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coins: Option<i64>,
    /// A container's lock bits (closeable 1, pickproof 2, closed 4, locked 8: values.doc).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lock_flags: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corpse: Option<bool>,
}

impl ObjValues {
    pub fn corpse(&self) -> bool {
        self.corpse == Some(true)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Place {
    Room(RoomIx),
    Carried(Key),
    Worn(Key, EquipPos),
    In(Key),
    /// Loaded for scripts (a reset with no room), or a corpse's or a shop's held stock.
    Nowhere,
}

#[derive(Debug, Clone)]
pub(crate) struct Obj {
    /// Its prototype; None for made things (corpses, money).
    pub proto: Option<Id>,
    pub serial: u64,
    pub kind: ItemType,
    pub keywords: Vec<String>,
    pub short: String,
    pub long: String,
    pub flags: Vec<ObjFlag>,
    pub wear: Vec<Wear>,
    pub weight: i64,
    pub cost: i64,
    pub level: i32,
    pub values: ObjValues,
    /// Ticks until it goes (corpses); -1 none.
    pub timer: i32,
    pub place: Place,
    pub contents: Vec<Key>,
}

/// A carried object as saved: its prototype and what changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedObj {
    pub proto: Id,
    #[serde(default, skip_serializing_if = "is_default")]
    pub values: ObjValues,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worn: Option<EquipPos>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contents: Vec<SavedObj>,
}

fn is_default(v: &ObjValues) -> bool {
    *v == ObjValues::default()
}

/// A player's lasting state: what the store keeps between sessions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Save {
    pub room: Option<String>,
    pub class: Class,
    pub sex: Sex,
    pub level: i32,
    pub exp: i64,
    pub gold: i64,
    pub alignment: i32,
    pub abilities: Abilities,
    pub hp: i32,
    pub max_hp: i32,
    pub mana: i32,
    pub max_mana: i32,
    pub mv: i32,
    pub max_mv: i32,
    pub conditions: Conditions,
    pub practices: i32,
    #[serde(default)]
    pub skills: BTreeMap<String, i32>,
    /// Game ticks lived, for age (MECHANICS §17.2).
    pub lived: i64,
    #[serde(default)]
    pub objects: Vec<SavedObj>,
}

/// Rolls an amount (MECHANICS §17.1): a fixed number, or `NdS+B` dice.
pub(crate) fn parse_dice(a: &Amount) -> (i32, i32, i32) {
    match a {
        Amount::Fixed(n) => (0, 0, *n as i32),
        Amount::Dice(d) => {
            let (dice, bonus) = match d.split_once(['+', '-']) {
                Some((dice, b)) => {
                    let sign = if d.contains('-') { -1 } else { 1 };
                    (dice, sign * b.trim().parse::<i32>().unwrap_or(0))
                }
                None => (d.as_str(), 0),
            };
            let (n, s) = dice.split_once('d').unwrap_or(("0", "0"));
            (n.trim().parse().unwrap_or(0), s.trim().parse().unwrap_or(0), bonus)
        }
    }
}
