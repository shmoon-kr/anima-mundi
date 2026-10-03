//! The simulation: world state, entities and IDs, the tick, the one deterministic RNG, commands to
//! actions, the rules (docs/MECHANICS.md), and the events they cause.
//!
//! Perception lives here (D13): who can perceive an event, and what each of them perceives (light,
//! position, place, blindness, invisibility), is a fact of the simulation. Events leave this crate
//! already filtered per recipient, so an agent never learns more than a person in its place.
//!
//! This crate writes no sentence and opens no socket or database: it does not depend on the renderer,
//! the network or the store (checked by `mundi-server/tests/architecture.rs`).
//!
//! Determinism: everything from outside (a player entering, a command, a lost link) is an [`Input`],
//! applied at the start of the next [`Sim::step`] and logged with the tick it was applied in. The same
//! world, seed and input log give the same deliveries, byte for byte ([`Sim::replay`]).

mod combat;
mod commands;
mod doors;
mod enter;
mod group;
mod items;
mod levels;
mod mobact;
pub mod entity;
mod look;
mod movement;
mod objects;
mod perception;
mod positions;
mod resets;
mod rng;
mod skills;
mod specials;
mod spells;
pub mod store;
mod talk;
mod triggers;
mod tick;
pub mod world;

use std::collections::{BTreeMap, HashMap, VecDeque};

use mundi_content::names::{Affect, DoorState, RoomFlag, Sector};
use mundi_content::{Tables, ZoneContent};
use mundi_protocol::{
    ArrivedHow, CloseReason, DayPhase, Direction, Event, InGameHow, LeftHow, LinkState, MoveFailure, Occupant,
    Position, PositionCommand, PositionRefusal, Refusal, RoomExit, RoomView, WakeFailure, SELF,
};
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;
use serde::{Deserialize, Serialize};

pub use entity::{Abilities, Class, Conditions, Save, SavedObj};
use entity::{Char, Group, Obj, Place};
use store::{Key, Store};
use world::{RoomIx, World};

/// Pulses per second (MECHANICS §1.1).
pub const PULSES_PER_SEC: u64 = 10;
/// One tick, one game hour: 75 seconds (MECHANICS §1.1, §1.2).
pub const PULSES_PER_TICK: u64 = 75 * PULSES_PER_SEC;
/// Directions in command-table and exit-list order (MECHANICS §2.1, §3.2).
const DIRS: [&str; 6] = ["north", "east", "south", "west", "up", "down"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Input {
    /// A player enters the game, or takes their linkless character back. `save` is what the store
    /// kept; without one a new character is made (MECHANICS §17.2) of `new`'s class and sex.
    Enter {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        save: Option<Box<Save>>,
        #[serde(default)]
        new: NewChar,
    },
    Command { name: String, text: String },
    /// The connection went away without `quit`: the character stays, linkless (MECHANICS §5.3).
    LinkLost { name: String },
    /// Sets or clears an affect. Until spells and objects exist (S5) this is how tests and admin
    /// tools put one on; it is logged like every input, so replays see it.
    SetAffect { name: String, affect: Affect, on: bool },
    /// Whether the character holds a lit light (until objects, S5).
    SetLight { name: String, on: bool },
    /// Makes an object of a prototype in a character's pack, or on the floor of their room (tests and
    /// admin tools, logged).
    Load {
        name: String,
        object: String,
        #[serde(default)]
        carry: bool,
    },
    /// Makes a mob of a prototype in a character's room (tests and admin tools, logged).
    LoadMob { name: String, mob: String },
    /// Sets how well a character knows a skill or spell (tests and admin tools, logged; practice
    /// at a guild is the game's way).
    SetSkill { name: String, skill: String, value: i32 },
    /// Sets a character's level (tests and admin tools, logged; tbaMUD's `advance`).
    SetLevel { name: String, level: i32 },
    /// Sets hit points, mana or moves, and hunger, thirst or drink (tests and admin tools, logged).
    SetPoints {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hp: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mana: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mv: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        conditions: Option<Conditions>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        gold: Option<i64>,
    },
}

/// The choices a new character makes (interpreter.c: sex, then class).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct NewChar {
    pub class: Option<Class>,
    pub sex: mundi_protocol::Sex,
    /// Where to start instead of the start room: for tests and admin tools.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub room: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Logged {
    pub tick: u64,
    pub input: Input,
}

/// One event for one recipient, already filtered by what they perceive.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Delivery {
    pub tick: u64,
    pub to: String,
    pub event: Event,
}

/// A character that left the game this step, and what the store keeps of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Departure {
    pub name: String,
    pub save: Save,
}

pub struct Sim {
    world: World,
    /// tbaMUD's numbers (D21): terrain costs, regeneration curves, the start room, new characters.
    tables: Tables,
    chars: Store<Char>,
    objs: Store<Obj>,
    by_name: BTreeMap<String, Key>,
    /// Players in the order they entered: the order commands are taken each pulse.
    order: Vec<Key>,
    /// Mobs in the order they were made: the order they act (MECHANICS §14.2).
    mobs: Vec<Key>,
    /// Who is in each room, latest arrival first (MECHANICS §2.4).
    people: Vec<Vec<Key>>,
    /// What lies in each room, latest first.
    things: Vec<Vec<Key>>,
    /// Live copies of each mob and object prototype, for reset limits (MECHANICS §14.1).
    counts: HashMap<String, i32>,
    /// The next serial for instance IDs (`<prototype>/<serial>`).
    serial: u64,
    /// Test-only light (until a lit light in the light slot replaces it).
    test_lights: Vec<Key>,
    /// Who is fighting, the one who started last first (fight.c combat_list).
    combat: Vec<Key>,
    groups: Store<Group>,
    /// Scripts' steps waiting for their time (triggers.rs).
    scheduled: Vec<(u64, triggers::Scheduled)>,
    pending: Vec<Input>,
    tick: u64,
    hour: u32,
    rng: ChaCha8Rng,
    log: Vec<Logged>,
    out: Vec<Delivery>,
    departed: Vec<Departure>,
}

impl Sim {
    /// `hour` is the game hour the world starts at (0..24).
    /// The world is reset once, as at boot (MECHANICS §14.1).
    pub fn new(zones: &[ZoneContent], tables: &Tables, seed: u64, hour: u32) -> Sim {
        let world = World::build(zones);
        let people = vec![Vec::new(); world.rooms.len()];
        let things = vec![Vec::new(); world.rooms.len()];
        let mut sim = Sim {
            world,
            tables: tables.clone(),
            chars: Store::default(),
            objs: Store::default(),
            by_name: BTreeMap::new(),
            order: Vec::new(),
            mobs: Vec::new(),
            people,
            things,
            counts: HashMap::new(),
            serial: 0,
            test_lights: Vec::new(),
            combat: Vec::new(),
            groups: Store::default(),
            scheduled: Vec::new(),
            pending: Vec::new(),
            tick: 0,
            hour: hour % 24,
            rng: ChaCha8Rng::seed_from_u64(seed),
            log: Vec::new(),
            out: Vec::new(),
            departed: Vec::new(),
        };
        for z in 0..sim.world.zone_list.len() {
            sim.reset_zone(z);
        }
        sim
    }

    /// Runs an input log on a fresh world and returns every delivery.
    pub fn replay(zones: &[ZoneContent], tables: &Tables, seed: u64, hour: u32, log: &[Logged]) -> Vec<Delivery> {
        let mut sim = Sim::new(zones, tables, seed, hour);
        let mut all = Vec::new();
        let last = log.last().map_or(0, |l| l.tick);
        let mut next = log.iter().peekable();
        while sim.tick < last {
            while let Some(l) = next.next_if(|l| l.tick == sim.tick + 1) {
                sim.submit(l.input.clone());
            }
            all.extend(sim.step());
        }
        all
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn input_log(&self) -> &[Logged] {
        &self.log
    }

    pub fn has_room(&self, id: &str) -> bool {
        self.world.index.contains_key(id)
    }

    /// Where a character in the game is now.
    pub fn room_of(&self, name: &str) -> Option<&str> {
        let c = self.chars.get(*self.by_name.get(&key_name(name))?)?;
        Some(&self.world.rooms[c.room].id)
    }

    /// Every character in the game and where they are, in the order they entered.
    pub fn places(&self) -> Vec<(String, String)> {
        self.order
            .iter()
            .filter_map(|k| self.chars.get(*k))
            .map(|c| (c.name.clone(), self.world.rooms[c.room].id.clone()))
            .collect()
    }

    /// What the store keeps of every player in the game now.
    pub fn saves(&self) -> Vec<(String, Save)> {
        self.order.iter().filter_map(|k| Some((self.chars.get(*k)?.name.clone(), self.save_of(*k)))).collect()
    }

    /// Every mob: its ID (`<prototype>/<serial>`) and room (for tests and tools).
    pub fn mob_places(&self) -> Vec<(String, String)> {
        self.mobs.iter().filter_map(|k| Some((self.id_of(*k)?, self.world.rooms[self.chars.get(*k)?.room].id.clone()))).collect()
    }

    /// What a player would save now (for tests and tools).
    pub fn save(&self, name: &str) -> Option<Save> {
        Some(self.save_of(*self.by_name.get(&key_name(name))?))
    }

    /// Characters that left the game in the last steps, drained.
    pub fn take_departures(&mut self) -> Vec<Departure> {
        std::mem::take(&mut self.departed)
    }

    /// Queues an input for the next step.
    pub fn submit(&mut self, input: Input) {
        self.pending.push(input);
    }

    /// One pulse (MECHANICS §1.1): inputs, then at most one command per character, then the tick work.
    pub fn step(&mut self) -> Vec<Delivery> {
        self.tick += 1;
        for input in std::mem::take(&mut self.pending) {
            self.log.push(Logged { tick: self.tick, input: input.clone() });
            self.apply(input);
        }
        for key in self.order.clone() {
            // A command waits out the wait state (comm.c:931-976): one pulse after any command, more
            // after some (hit, skills).
            let Some(c) = self.chars.get_mut(key) else { continue };
            if c.wait > 0 {
                c.wait -= 1;
                continue;
            }
            let Some(text) = c.queue.pop_front() else { continue };
            self.command(key, &text);
        }
        self.run_scheduled();
        if self.tick % triggers::PULSE_SCRIPT == 0 {
            self.script_check();
        }
        if self.tick % (10 * PULSES_PER_SEC) == 0 {
            self.zone_update();
            self.mobile_activity();
        }
        if self.tick % (2 * PULSES_PER_SEC) == 0 {
            self.violence();
        }
        if self.tick % PULSES_PER_TICK == 0 {
            self.game_hour();
            self.affect_update();
            self.point_update();
        }
        self.prompts();
        std::mem::take(&mut self.out)
    }

    fn apply(&mut self, input: Input) {
        match input {
            Input::Enter { name, save, new } => self.enter(&name, save.map(|s| *s), new),
            Input::Command { name, text } => {
                if let Some(c) = self.by_name.get(&key_name(&name)).and_then(|k| self.chars.get_mut(*k)) {
                    c.queue.push_back(text);
                }
            }
            Input::SetAffect { name, affect, on } => {
                if let Some(c) = self.by_name.get(&key_name(&name)).and_then(|k| self.chars.get_mut(*k)) {
                    c.affects.retain(|a| *a != affect);
                    if on {
                        c.affects.push(affect);
                    }
                }
            }
            Input::SetLight { name, on } => {
                if let Some(&k) = self.by_name.get(&key_name(&name)) {
                    self.test_lights.retain(|x| *x != k);
                    if on {
                        self.test_lights.push(k);
                    }
                }
            }
            Input::Load { name, object, carry } => {
                if let Some(&k) = self.by_name.get(&key_name(&name)) {
                    if let Some(o) = self.make_obj(&object) {
                        let room = self.chars.get(k).unwrap().room;
                        self.put(o, if carry { Place::Carried(k) } else { Place::Room(room) });
                    }
                }
            }
            Input::SetLevel { name, level } => {
                if let Some(c) = self.by_name.get(&key_name(&name)).and_then(|k| self.chars.get_mut(*k)) {
                    c.level = level;
                }
            }
            Input::SetSkill { name, skill, value } => {
                if let Some(c) = self.by_name.get(&key_name(&name)).and_then(|k| self.chars.get_mut(*k)) {
                    c.skills.insert(skill, value);
                }
            }
            Input::LoadMob { name, mob } => {
                if let Some(&k) = self.by_name.get(&key_name(&name)) {
                    let room = self.chars.get(k).unwrap().room;
                    self.make_mob(&mob, room);
                }
            }
            Input::SetPoints { name, hp, mana, mv, conditions, gold } => {
                if let Some(c) = self.by_name.get(&key_name(&name)).and_then(|k| self.chars.get_mut(*k)) {
                    c.gold = gold.unwrap_or(c.gold);
                    c.hp = hp.unwrap_or(c.hp);
                    c.mana = mana.unwrap_or(c.mana);
                    c.mv = mv.unwrap_or(c.mv);
                    c.conditions = conditions.unwrap_or(c.conditions);
                }
            }
            Input::LinkLost { name } => {
                let Some(&key) = self.by_name.get(&key_name(&name)) else { return };
                let Some(c) = self.chars.get_mut(key) else { return };
                c.linked = false;
                c.queue.clear();
                let room = c.room;
                self.to_room(key, room, true, |who, who_id| Event::Link { who, who_id, state: LinkState::Lost });
            }
        }
    }

    fn deliver(&mut self, to: Key, event: Event) {
        let Some(c) = self.chars.get(to) else { return };
        if !c.linked {
            return;
        }
        self.out.push(Delivery { tick: self.tick, to: c.name.clone(), event });
    }
}

fn key_name(name: &str) -> String {
    name.to_lowercase()
}

/// A player's ID in events: `pc:<name>`, the same across sessions.
pub fn char_id(name: &str) -> String {
    format!("pc:{}", key_name(name))
}

impl Sim {
    /// A character's ID in events: `pc:<name>` for players, `<prototype>/<serial>` for mobs.
    fn id_of(&self, k: Key) -> Option<String> {
        let c = self.chars.get(k)?;
        Some(match &c.mob {
            Some(m) => format!("{}/{}", m.proto, m.serial),
            None => char_id(&c.name),
        })
    }

    fn next_serial(&mut self) -> u64 {
        self.serial += 1;
        self.serial
    }
}
