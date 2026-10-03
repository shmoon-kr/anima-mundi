//! The content format (D10, PHASE-1-PLAN S2). These types are the schema: the converter writes them,
//! the loader reads them (D15), and unknown keys are rejected.
//!
//! A zone is a directory: `zone.yaml`, `rooms.yaml`, `mobs.yaml`, `objects.yaml`, `resets.yaml`, `shops.yaml`.
//! Entries are keyed by ID, `tba:<zone>:<kind>:<vnum>` for converted tbaMUD content (D16).

use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::names::*;

pub type Id = String;

// ---------------------------------------------------------------- text

/// Prose (paragraphs, wrapped by the renderer per language) or verbatim text (maps, pictures, tables:
/// shown exactly as written). In memory prose is its paragraphs joined by "\n".
///
/// In YAML a one-paragraph prose is a folded block (`>`); several paragraphs are a list of folded
/// blocks, one per paragraph, so paragraph breaks are explicit and survive any YAML tool's folding.
/// Verbatim text is `{preformatted: |...}` so the difference survives loading.
#[derive(Debug, Clone, PartialEq)]
pub enum Text {
    Prose(String),
    Preformatted(String),
}

impl Text {
    pub fn as_str(&self) -> &str {
        match self {
            Text::Prose(s) | Text::Preformatted(s) => s,
        }
    }

    pub fn paragraphs(&self) -> Vec<&str> {
        match self {
            Text::Prose(s) => s.split('\n').collect(),
            Text::Preformatted(s) => vec![s.as_str()],
        }
    }
}

impl Serialize for Text {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Text::Prose(t) if t.contains('\n') => {
                let paras: Vec<serde_saphyr::FoldString> =
                    t.split('\n').map(|p| serde_saphyr::FoldString(p.to_string())).collect();
                paras.serialize(s)
            }
            Text::Prose(t) => serde_saphyr::FoldString(t.clone()).serialize(s),
            Text::Preformatted(t) => {
                #[derive(Serialize)]
                struct Pre {
                    preformatted: serde_saphyr::LitString,
                }
                Pre { preformatted: serde_saphyr::LitString(t.clone()) }.serialize(s)
            }
        }
    }
}

impl<'de> Deserialize<'de> for Text {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Prose(String),
            Paragraphs(Vec<String>),
            Pre {
                preformatted: String,
            },
        }
        // a folded block keeps a final line break ("clip"); a paragraph never ends with one
        let clean = |p: &str| p.trim_end_matches('\n').to_string();
        Ok(match Raw::deserialize(d)? {
            Raw::Prose(s) => Text::Prose(clean(&s)),
            Raw::Paragraphs(ps) => Text::Prose(ps.iter().map(|p| clean(p)).collect::<Vec<_>>().join("\n")),
            Raw::Pre { preformatted } => Text::Preformatted(preformatted),
        })
    }
}

/// A number, or dice `NdS` / `NdS+B` when something is rolled (constant dice are written as numbers).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Amount {
    Fixed(i64),
    Dice(String),
}

fn flow<S: Serializer, T: Serialize>(v: &[T], s: S) -> Result<S::Ok, S::Error> {
    serde_saphyr::FlowSeq(v).serialize(s)
}

fn is_false(b: &bool) -> bool {
    !*b
}

// ---------------------------------------------------------------- zone

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Zone {
    pub id: Id,
    pub name: String,
    pub builders: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub levels: Option<Levels>,
    pub reset: ZoneReset,
    #[serde(default, skip_serializing_if = "Vec::is_empty", serialize_with = "flow")]
    pub flags: Vec<ZoneFlag>,
    pub source: Source,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Levels {
    pub min: i32,
    pub max: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneReset {
    pub every_minutes: i32,
    pub when: ResetWhen,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    /// The tbaMUD vnum range of the zone (its rooms).
    #[serde(serialize_with = "flow")]
    pub vnums: Vec<u32>,
}

// ---------------------------------------------------------------- rooms

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Room {
    pub name: String,
    pub description: Text,
    pub sector: Sector,
    #[serde(default, skip_serializing_if = "Vec::is_empty", serialize_with = "flow")]
    pub flags: Vec<RoomFlag>,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub exits: IndexMap<Dir, Exit>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extras: Vec<Extra>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Exit {
    /// The room it leads to; None for an exit to nowhere (a closed-off passage in the original).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Id>,
    /// What `look <dir>` shows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub look: Option<Text>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub door: Option<Door>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Door {
    #[serde(serialize_with = "flow")]
    pub keywords: Vec<String>,
    pub kind: DoorKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<Id>,
    /// The state the zone reset puts it in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset: Option<DoorState>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Extra {
    #[serde(serialize_with = "flow")]
    pub keywords: Vec<String>,
    pub text: Text,
}

// ---------------------------------------------------------------- mobs

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mob {
    #[serde(serialize_with = "flow")]
    pub keywords: Vec<String>,
    /// In sentences: "the pit beast".
    pub short: String,
    /// Its line in a room.
    pub long: String,
    pub description: Text,
    pub level: i32,
    pub sex: Sex,
    pub alignment: i32,
    #[serde(default, skip_serializing_if = "Vec::is_empty", serialize_with = "flow")]
    pub flags: Vec<MobFlag>,
    #[serde(default, skip_serializing_if = "Vec::is_empty", serialize_with = "flow")]
    pub affects: Vec<Affect>,
    pub combat: Combat,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub abilities: IndexMap<String, i64>,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub saves: IndexMap<String, i64>,
    pub gold: i64,
    pub exp: i64,
    pub position: Positions,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Combat {
    pub thac0: i32,
    /// tbaMUD armor class / 10 (lower is better).
    pub armor: i32,
    pub hit_points: Amount,
    pub damage: Amount,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bare_hand_attack: Option<Attack>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Positions {
    pub load: Position,
    pub default: Position,
}

// ---------------------------------------------------------------- objects

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Object {
    #[serde(rename = "type")]
    pub kind: ItemType,
    #[serde(serialize_with = "flow")]
    pub keywords: Vec<String>,
    pub short: String,
    /// Its line on the ground.
    pub long: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty", serialize_with = "flow")]
    pub flags: Vec<ObjFlag>,
    #[serde(serialize_with = "flow")]
    pub wear: Vec<Wear>,
    pub values: Values,
    pub weight: i64,
    pub cost: i64,
    pub rent: i64,
    /// Minimum level to use it.
    pub level: i32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affects: Vec<ObjAffect>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extras: Vec<Extra>,
}

/// An object's values, named per type (armor: armor; weapon: damage, attack; drinkcon: capacity, ...).
/// Types without named values keep tbaMUD's four numbers in `raw`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Values {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub armor: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub damage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attack: Option<Attack>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lock_flags: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<Id>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corpse: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "flow_opt")]
    pub raw: Option<Vec<i64>>,
}

fn flow_opt<S: Serializer>(v: &Option<Vec<i64>>, s: S) -> Result<S::Ok, S::Error> {
    match v {
        Some(v) => flow(v, s),
        None => s.serialize_none(),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjAffect {
    pub apply: Apply,
    pub modifier: i64,
}

// ---------------------------------------------------------------- resets

/// What the zone reset puts back. Door reset states are on the doors (rooms.yaml).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resets {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spawns: Vec<Spawn>,
    /// Door states for doors this zone cannot hold (rooms elsewhere, or exits without a door).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub doors: Vec<DoorReset>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removes: Vec<Remove>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Spawn {
    Mob(MobSpawn),
    Object(ObjectSpawn),
}

/// A mob in a room, with what it wears and carries (they load only if the mob does, unless marked).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MobSpawn {
    pub mob: Id,
    pub room: Id,
    /// At most this many of it may exist in the whole world.
    pub limit: i32,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub equip: IndexMap<EquipPos, Load>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub carry: Vec<Load>,
}

/// An object on the ground of a room (or nowhere: kept for scripts), with its contents.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectSpawn {
    pub object: Id,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub room: Option<Id>,
    pub limit: i32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contents: Vec<Load>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Load {
    pub object: Id,
    pub limit: i32,
    /// tbaMUD runs a command with if-flag 0 even when the previous one did not load.
    #[serde(default, skip_serializing_if = "is_false")]
    pub even_if_mob_not_loaded: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contents: Vec<Load>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DoorReset {
    pub room: Id,
    pub exit: Dir,
    pub state: DoorState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Remove {
    pub room: Id,
    pub object: Id,
}

// ---------------------------------------------------------------- shops

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shop {
    /// None: the original names no keeper (the shop cannot trade until one is given).
    pub keeper: Option<Id>,
    #[serde(serialize_with = "flow")]
    pub rooms: Vec<Id>,
    /// Always for sale.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub products: Vec<Id>,
    /// Item types it buys, optionally only with these keywords.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub buys: Vec<Buy>,
    /// We pay cost x buy; we get cost x min(sell, buy) (both nudged by charisma).
    pub profit: Profit,
    pub messages: ShopMessages,
    pub temper: i32,
    pub flags: i64,
    pub trade_with: i64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hours: Vec<Hours>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Buy {
    #[serde(rename = "type")]
    pub kind: ItemType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keywords: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profit {
    pub buy: f64,
    pub sell: f64,
}

/// The keeper's lines. `%s` is the customer's name, `%d` a price (tbaMUD's placeholders).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShopMessages {
    pub no_such_item: String,
    pub you_dont_have_it: String,
    pub does_not_buy: String,
    pub shop_cannot_afford: String,
    pub you_cannot_afford: String,
    pub bought: String,
    pub sold: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hours {
    pub open: i32,
    pub close: i32,
}

// ---------------------------------------------------------------- a whole zone

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ZoneContent {
    pub zone: Option<Zone>,
    pub rooms: IndexMap<Id, Room>,
    pub mobs: IndexMap<Id, Mob>,
    pub objects: IndexMap<Id, Object>,
    pub resets: Resets,
    pub shops: IndexMap<Id, Shop>,
}
