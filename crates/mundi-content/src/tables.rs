//! Number tables and combat messages: tbaMUD's values as data (D21), read by the engine at start.
//! Each field names its MECHANICS section; the values come from `mundi-convert tables` and
//! `mundi-convert messages` and live in `third_party/tbamud/`.

use std::path::Path;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::load::LoadError;
use crate::names::Sector;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tables {
    /// tbaMUD's command table in its order (MECHANICS §4.2): a typed word is the first name it begins.
    pub commands: Vec<CommandEntry>,
    pub abilities: Abilities,
    /// Special procedures tbaMUD assigns in code (spec_assign.c) and the guild guards (class.c).
    pub specials: Specials,
    /// Native rules for the DG Script triggers the party meets (MECHANICS §14.3), by trigger ID.
    #[serde(default)]
    pub triggers: IndexMap<String, Trigger>,
    /// Spells and skills (spell_parser.c spello/skillo, class.c init_spell_levels; MECHANICS §10, §11).
    pub spells: Vec<Spell>,
    /// By class name: magic_user, cleric, thief, warrior.
    pub classes: IndexMap<String, Class>,
    pub world: WorldTables,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandEntry {
    pub name: String,
    /// The lowest position it can be used in.
    pub position: crate::names::Position,
    /// The lowest level that may use it (31 and up: immortals).
    pub level: i32,
    /// A social (do_action): matched after every other command.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub social: bool,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Specials {
    /// Mob → its special (guild, cityguard, fido, janitor, snake, magic_user, ...).
    pub mobs: IndexMap<String, String>,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub objects: IndexMap<String, String>,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub rooms: IndexMap<String, String>,
    /// Guild guards: in `room`, going `dir`, only `class` (or none: "all" is blocked) passes.
    pub guild_guards: Vec<GuildGuard>,
}

/// One trigger as a general behaviour and its content (MECHANICS §14.3). The lines are the
/// script's, `%s` the names it puts in.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trigger {
    /// The engine's general behaviour this script is (MECHANICS §14.3): zone_welcome,
    /// outfit_newcomers, guard, eat_corpses, pick_up_litter, reward_drops, guild_guard, none.
    /// What is particular to the zone (who, which things, the lines, the numbers) is here.
    pub kind: String,
    /// lib/world/trg file and number, for reading the script.
    pub source: String,
    /// Random triggers: the percent each 13-second check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chance: Option<i32>,
    /// The script's `wait` before it acts, in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delay: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub below_level: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_cost: Option<i64>,
    /// guard: spits at those with less charisma than this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub below_charisma: Option<i32>,
    /// reward_drops: cost / per, from min to max.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reward: Option<Reward>,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub lines: IndexMap<String, String>,
    /// outfit_newcomers: what it hands out, in the order it checks; a piece's line is `kit.<index>`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kit: Vec<KitPiece>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reward {
    pub per: i64,
    pub min: i64,
    pub max: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KitPiece {
    /// The slots it checks (a pair for rings, neck, wrists): missing any, it gives this.
    pub slots: Vec<crate::names::EquipPos>,
    pub object: String,
    /// Only in the full kit (the held staff).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub full_only: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub say: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub social: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuildGuard {
    /// magic_user, cleric, thief, warrior; "all" lets no class through.
    pub class: String,
    pub room: String,
    pub dir: crate::names::Dir,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spell {
    /// spells.h: spells 1-130, skills 131 and up.
    pub number: i32,
    pub name: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub skill: bool,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub mana_max: i32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub mana_min: i32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub mana_change: i32,
    pub position: crate::names::Position,
    /// tar_*: ignore, char_room, char_world, fight_self, fight_vict, self_only, not_self, obj_inv,
    /// obj_room, obj_world, obj_equip.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub targets: Vec<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub violent: bool,
    /// mag_*: damage, affects, unaffects, points, alter_objs, groups, masses, areas, summons,
    /// creations, manual, rooms.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub routines: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wearoff: Option<String>,
    /// The level each class may learn it at.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub levels: IndexMap<String, i32>,
}

fn is_zero(n: &i32) -> bool {
    *n == 0
}

/// MECHANICS §7.4, §9.4, §12.4, §13.1. Indexed by the ability score; strength 26-30 are 18/01-50,
/// 18/51-75, 18/76-90, 18/91-99, 18/100.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Abilities {
    pub strength: Vec<Strength>,
    pub dexterity: Vec<Dexterity>,
    pub dexterity_skill: Vec<DexteritySkill>,
    /// Hit points added at each level.
    pub constitution_hitp: Vec<i32>,
    /// Percent learned per practice.
    pub intelligence_learn: Vec<i32>,
    /// Practice sessions per level.
    pub wisdom_practices: Vec<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Strength {
    pub tohit: i32,
    pub todam: i32,
    pub carry_w: i32,
    pub wield_w: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dexterity {
    pub reaction: i32,
    pub miss_att: i32,
    pub defensive: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DexteritySkill {
    pub p_pocket: i32,
    pub p_locks: i32,
    pub traps: i32,
    pub sneak: i32,
    pub hide: i32,
}

/// MECHANICS §7.4, §9.3, §10.2, §12.4. Lists are indexed by level from 0.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Class {
    pub thac0: Vec<i32>,
    /// Experience needed for each level up to the first immortal level; above it see MECHANICS §9.
    pub level_exp: Vec<i64>,
    /// By saving throw: paralysis, rod, petrification, breath, spell.
    pub saving_throws: IndexMap<String, Vec<i32>>,
    pub practice: Practice,
    /// class.c title_male, title_female: the title at each level, 0 to the implementor's.
    pub titles: Titles,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Titles {
    pub male: Vec<String>,
    pub female: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Practice {
    /// The most a skill can be learned to.
    pub learned: i32,
    pub max_gain: i32,
    pub min_gain: i32,
    /// "spell" or "skill": the word the practice list uses.
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldTables {
    /// Movement points per room terrain (MECHANICS §2.3).
    pub movement_cost: IndexMap<Sector, i32>,
    /// Liquids in tbaMUD's order: a drink container's `liquid` (MECHANICS §6.3).
    pub liquids: Vec<Liquid>,
    /// The age curves of regeneration, seven points each (MECHANICS §5.1, §5.2).
    pub regen: Regen,
    /// Spell words to what others hear, tried in order (MECHANICS §11.2).
    pub syllables: Vec<[String; 2]>,
    pub config: Config,
    /// Names of things the game makes: corpses and coins (fight.c make_corpse, handler.c money_desc).
    pub made: Made,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Made {
    /// `%s` is the dead one's name.
    pub corpse_short: String,
    pub corpse_long: String,
    pub coin_short: String,
    pub coin_long: String,
    /// Piles of coins by the most they hold; above the last, `money_more`.
    pub money: Vec<MoneyName>,
    pub money_more: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MoneyName {
    pub up_to: i64,
    pub short: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Liquid {
    pub name: String,
    pub keyword: String,
    pub colour: String,
    pub drunk: i32,
    pub full: i32,
    pub thirst: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Regen {
    pub hit: [i32; 7],
    pub mana: [i32; 7],
    pub moves: [i32; 7],
}

/// Settings of the original game (config.c, class.c do_start).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub start_room: String,
    pub tunnel_size: i32,
    pub max_exp_gain: i64,
    pub max_exp_loss: i64,
    pub npc_corpse_ticks: i32,
    pub pc_corpse_ticks: i32,
    pub new_character: NewCharacter,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewCharacter {
    pub max_hit: i32,
    pub max_mana: i32,
    pub max_move: i32,
}

/// The combat message file (MECHANICS §8.2): for each attack type, variants of what the attacker,
/// the victim and the room see when it kills, misses, hits, or meets a god. A missing line is null.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CombatMessages {
    pub attacks: Vec<AttackMessages>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttackMessages {
    /// tbaMUD's spell, skill or attack number (300-314 weapons, 399 suffering).
    pub number: i32,
    /// The name from the file's comment, for reading.
    pub name: String,
    pub variants: Vec<MessageSet>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageSet {
    pub die: Lines,
    pub miss: Lines,
    pub hit: Lines,
    pub god: Lines,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lines {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attacker: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub victim: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub room: Option<String>,
}

fn read<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, LoadError> {
    let err = |message: String| LoadError { file: path.into(), message };
    let text = std::fs::read_to_string(path).map_err(|e| err(e.to_string()))?;
    serde_saphyr::from_str(&text).map_err(|e| err(e.to_string()))
}

/// `<dir>/commands.yaml`, `abilities.yaml`, `spells.yaml`, `classes.yaml`, `world.yaml`.
pub fn load_tables(dir: &Path) -> Result<Tables, LoadError> {
    Ok(Tables {
        commands: read(&dir.join("commands.yaml"))?,
        abilities: read(&dir.join("abilities.yaml"))?,
        specials: read(&dir.join("specials.yaml"))?,
        triggers: read(&dir.join("triggers.yaml"))?,
        spells: read(&dir.join("spells.yaml"))?,
        classes: read(&dir.join("classes.yaml"))?,
        world: read(&dir.join("world.yaml"))?,
    })
}

pub fn load_messages(path: &Path) -> Result<CombatMessages, LoadError> {
    read(path)
}

/// Content lines in another language by trigger ID and line key (`locales/ko/triggers.yaml`).
pub type TriggerLines = std::collections::HashMap<String, std::collections::HashMap<String, String>>;

pub fn load_trigger_lines(path: &Path) -> Result<TriggerLines, LoadError> {
    read(path)
}
