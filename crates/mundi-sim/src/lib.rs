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

mod commands;
mod enter;
mod look;
mod movement;
mod perception;
mod positions;
pub mod store;
mod talk;
mod tick;
pub mod world;

use std::collections::{BTreeMap, VecDeque};

use mundi_content::names::{Affect, DoorState, RoomFlag, Sector};
use mundi_content::{Tables, ZoneContent};
use mundi_protocol::{
    ArrivedHow, CloseReason, DayPhase, Direction, Event, InGameHow, LeftHow, LinkState, MoveFailure, Occupant,
    Position, PositionCommand, PositionRefusal, Refusal, RoomExit, RoomView, WakeFailure, SELF,
};
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;
use serde::{Deserialize, Serialize};

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
    /// A player enters the game, or takes their linkless character back. `room` is the saved place.
    Enter { name: String, room: Option<String> },
    Command { name: String, text: String },
    /// The connection went away without `quit`: the character stays, linkless (MECHANICS §5.3).
    LinkLost { name: String },
    /// Sets or clears an affect. Until spells and objects exist (S5) this is how tests and admin
    /// tools put one on; it is logged like every input, so replays see it.
    SetAffect { name: String, affect: Affect, on: bool },
    /// Whether the character holds a lit light (until objects, S5).
    SetLight { name: String, on: bool },
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

/// A character that left the game this step, and where they were: what the store saves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Departure {
    pub name: String,
    pub room: String,
}

#[derive(Debug, Clone)]
struct Char {
    name: String,
    room: RoomIx,
    position: Position,
    level: i32,
    hp: i32,
    max_hp: i32,
    mana: i32,
    max_mana: i32,
    mv: i32,
    max_mv: i32,
    linked: bool,
    queue: VecDeque<String>,
    affects: Vec<Affect>,
    light: bool,
}

impl Char {
    fn has(&self, a: Affect) -> bool {
        self.affects.contains(&a)
    }
}

pub struct Sim {
    world: World,
    /// tbaMUD's numbers (D21): terrain costs, regeneration curves, the start room, new characters.
    tables: Tables,
    chars: Store<Char>,
    by_name: BTreeMap<String, Key>,
    /// Characters in the order they entered: the order commands are taken each pulse.
    order: Vec<Key>,
    /// Who is in each room, latest arrival first (MECHANICS §2.4).
    people: Vec<Vec<Key>>,
    pending: Vec<Input>,
    tick: u64,
    hour: u32,
    #[allow(dead_code)] // the first rule that rolls dice uses it
    rng: ChaCha8Rng,
    log: Vec<Logged>,
    out: Vec<Delivery>,
    departed: Vec<Departure>,
}

impl Sim {
    /// `hour` is the game hour the world starts at (0..24).
    pub fn new(zones: &[ZoneContent], tables: &Tables, seed: u64, hour: u32) -> Sim {
        let world = World::build(zones);
        let people = vec![Vec::new(); world.rooms.len()];
        Sim {
            world,
            tables: tables.clone(),
            chars: Store::default(),
            by_name: BTreeMap::new(),
            order: Vec::new(),
            people,
            pending: Vec::new(),
            tick: 0,
            hour: hour % 24,
            rng: ChaCha8Rng::seed_from_u64(seed),
            log: Vec::new(),
            out: Vec::new(),
            departed: Vec::new(),
        }
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
            let Some(text) = self.chars.get_mut(key).and_then(|c| c.queue.pop_front()) else { continue };
            self.command(key, &text);
        }
        if self.tick % PULSES_PER_TICK == 0 {
            self.game_hour();
            self.regen();
        }
        self.prompts();
        std::mem::take(&mut self.out)
    }

    fn apply(&mut self, input: Input) {
        match input {
            Input::Enter { name, room } => self.enter(&name, room.as_deref()),
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
                if let Some(c) = self.by_name.get(&key_name(&name)).and_then(|k| self.chars.get_mut(*k)) {
                    c.light = on;
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
