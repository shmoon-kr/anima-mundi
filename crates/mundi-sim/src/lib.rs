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

pub mod store;
pub mod world;

use std::collections::{BTreeMap, VecDeque};

use mundi_content::names::{DoorState, RoomFlag, Sector};
use mundi_content::ZoneContent;
use mundi_protocol::{
    ArrivedHow, CloseReason, DayPhase, Direction, Event, InGameHow, LeftHow, LinkState, MoveFailure, Occupant,
    Position, Refusal, RoomExit, RoomView, SELF,
};
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;
use serde::{Deserialize, Serialize};

use store::{Key, Store};
use world::{sector_cost, RoomIx, World};

/// Pulses per second (MECHANICS §1.1).
pub const PULSES_PER_SEC: u64 = 10;
/// One tick, one game hour: 75 seconds (MECHANICS §1.1, §1.2).
pub const PULSES_PER_TICK: u64 = 75 * PULSES_PER_SEC;
/// Where new characters and characters with no saved place enter (config.c:183).
pub const START_ROOM: &str = "tba:30:room:3001";
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
}

pub struct Sim {
    world: World,
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
    pub fn new(zones: &[ZoneContent], seed: u64, hour: u32) -> Sim {
        let world = World::build(zones);
        let people = vec![Vec::new(); world.rooms.len()];
        Sim {
            world,
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
    pub fn replay(zones: &[ZoneContent], seed: u64, hour: u32, log: &[Logged]) -> Vec<Delivery> {
        let mut sim = Sim::new(zones, seed, hour);
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

    // ---- entering and leaving -------------------------------------------------------------------

    fn enter(&mut self, name: &str, saved: Option<&str>) {
        if let Some(&key) = self.by_name.get(&key_name(name)) {
            let c = self.chars.get_mut(key).unwrap();
            c.linked = true;
            let room = c.room;
            self.deliver(key, Event::InGame { how: InGameHow::Reconnected });
            self.to_room(key, room, true, |who, who_id| Event::Link { who, who_id, state: LinkState::Reconnected });
            return;
        }
        let room = saved
            .and_then(|r| self.world.index.get(r))
            .or_else(|| self.world.index.get(START_ROOM))
            .copied()
            .unwrap_or(0);
        // A new character (MECHANICS §9.4); the level-up roll and classes come with S5.
        let key = self.chars.insert(Char {
            name: name.to_string(),
            room,
            position: Position::Standing,
            level: 1,
            hp: 10,
            max_hp: 10,
            mana: 100,
            max_mana: 100,
            mv: 82,
            max_mv: 82,
            linked: true,
            queue: VecDeque::new(),
        });
        self.by_name.insert(key_name(name), key);
        self.order.push(key);
        self.people[room].insert(0, key);
        self.deliver(key, Event::InGame { how: InGameHow::Entered });
        self.to_room(key, room, true, |who, who_id| Event::Arrived { who, who_id, from_dir: None, how: Some(ArrivedHow::EnteredGame) });
        self.look(key);
    }

    fn quit(&mut self, key: Key) {
        let c = self.chars.get(key).unwrap();
        let (room, name) = (c.room, c.name.clone());
        self.to_room(key, room, true, |who, who_id| Event::Left { who, who_id, dir: None, how: Some(LeftHow::LeftGame) });
        self.deliver(key, Event::Closed { reason: CloseReason::Remote, detail: Some("quit".into()) });
        self.people[room].retain(|k| *k != key);
        self.order.retain(|k| *k != key);
        self.by_name.remove(&key_name(&name));
        self.chars.remove(key);
        self.departed.push(Departure { name, room: self.world.rooms[room].id.clone() });
    }

    // ---- commands (MECHANICS §4.2) --------------------------------------------------------------

    fn command(&mut self, key: Key, text: &str) {
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

    /// MECHANICS §2.2 to §2.4. Boats, flying, water breathing, tunnels, followers and death rooms come
    /// with the rules they need (S5).
    fn move_dir(&mut self, key: Key, d: usize) {
        let c = self.chars.get(key).unwrap();
        let from = c.room;
        let dir = DIRS[d].to_string();
        let exit = self.world.rooms[from].exits[d].as_ref();
        let Some(to) = exit.and_then(|e| e.to) else {
            self.deliver(key, Event::MoveFailed { dir: Some(dir), reason: MoveFailure::NoExit, door: None });
            return;
        };
        if let Some(door) = exit.and_then(|e| e.door.as_ref()).filter(|d| d.state != DoorState::Open) {
            let door = door.keyword.clone();
            self.deliver(key, Event::MoveFailed { dir: Some(dir), reason: MoveFailure::Closed, door });
            return;
        }
        let zone = self.world.zones.get(&self.world.rooms[to].zone).cloned().unwrap_or_default();
        if zone.min_level.is_some_and(|min| min > c.level) {
            self.deliver(key, Event::ZoneAboveLevel {});
        }
        if zone.closed {
            self.deliver(key, Event::MoveFailed { dir: Some(dir), reason: MoveFailure::Forbidden, door: None });
            return;
        }
        let cost = (sector_cost(self.world.rooms[from].sector) + sector_cost(self.world.rooms[to].sector)) / 2;
        let c = self.chars.get_mut(key).unwrap();
        if c.mv < cost {
            self.deliver(key, Event::MoveFailed { dir: Some(dir), reason: MoveFailure::Exhausted, door: None });
            return;
        }
        c.mv -= cost;
        let left = dir.clone();
        self.to_room(key, from, true, move |who, who_id| Event::Left { who, who_id, dir: Some(left.clone()), how: None });
        self.people[from].retain(|k| *k != key);
        self.people[to].insert(0, key);
        self.chars.get_mut(key).unwrap().room = to;
        self.to_room(key, to, true, |who, who_id| Event::Arrived { who, who_id, from_dir: None, how: None });
        self.look(key);
    }

    /// MECHANICS §3.2. Objects and mobs come with the zone resets (S5).
    fn look(&mut self, key: Key) {
        let room = self.chars.get(key).unwrap().room;
        let r = &self.world.rooms[room];
        if !self.can_see_in(key, room) {
            let id = Some(r.id.clone());
            self.deliver(key, Event::RoomDark { id, blind: false });
            return;
        }
        let exits = r
            .exits
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                let e = e.as_ref()?;
                let to = e.to?;
                let closed = e.door.as_ref().is_some_and(|d| d.state != DoorState::Open);
                Some(RoomExit { dir: DIRS[i].into(), closed, to_id: Some(self.world.rooms[to].id.clone()) })
            })
            .collect();
        let occupants = self.people[room]
            .iter()
            .filter(|k| **k != key)
            .filter_map(|k| self.chars.get(*k).map(|c| (k, c)))
            .filter(|(k, _)| self.can_see(key, **k))
            .map(|(_, c)| Occupant {
                id: Some(char_id(&c.name)),
                text: String::new(),
                name: c.name.clone(),
                long: None,
                position: c.position,
                fighting: None,
                flags: if c.linked { vec![] } else { vec!["linkless".into()] },
                hints: vec![],
            })
            .collect();
        let view = RoomView {
            id: Some(r.id.clone()),
            name: r.name.clone(),
            desc: r.desc.clone(),
            exits,
            objects: vec![],
            occupants,
            dark: false,
        };
        self.deliver(key, Event::Room(view));
    }

    /// act.comm.c:40-72: the room hears it first, then the speaker.
    fn say(&mut self, key: Key, text: &str) {
        if text.is_empty() {
            self.deliver(key, Event::Refused { reason: Refusal::NothingToSay });
            return;
        }
        let room = self.chars.get(key).unwrap().room;
        let said = text.to_string();
        self.to_room(key, room, false, move |from, from_id| Event::Say { from, from_id, text: said.clone(), direction: Direction::In });
        self.deliver(key, Event::Say { from: SELF.into(), from_id: None, text: text.into(), direction: Direction::Out });
    }

    // ---- the tick (MECHANICS §1.2, §5) ----------------------------------------------------------

    fn game_hour(&mut self) {
        self.hour = (self.hour + 1) % 24;
        let phase = match self.hour {
            5 => DayPhase::Sunrise,
            6 => DayPhase::Day,
            21 => DayPhase::Sunset,
            22 => DayPhase::Night,
            _ => return,
        };
        for key in self.order.clone() {
            let c = self.chars.get(key).unwrap();
            if c.position > Position::Sleeping && !self.world.rooms[c.room].flags.contains(&RoomFlag::Indoors) {
                self.deliver(key, Event::WorldTime { phase });
            }
        }
    }

    /// MECHANICS §5.2 for a character of age 17 with no class yet; hunger, thirst and poison come with S5.
    fn regen(&mut self) {
        for key in self.order.clone() {
            let c = self.chars.get_mut(key).unwrap();
            if c.position < Position::Stunned {
                continue;
            }
            let bonus = |base: i32, sleep: i32, rest: i32, sit: i32, pos: Position| match pos {
                Position::Sleeping => base + base / sleep,
                Position::Resting => base + base / rest,
                Position::Sitting => base + base / sit,
                _ => base,
            };
            let hp = bonus(graf(17, [8, 12, 20, 32, 16, 10, 4]), 2, 4, 8, c.position);
            let mv = bonus(graf(17, [16, 20, 24, 20, 16, 12, 10]), 2, 4, 8, c.position);
            let base_mana = graf(17, [4, 8, 12, 16, 12, 10, 8]);
            let mana = match c.position {
                Position::Sleeping => base_mana * 2,
                Position::Resting => base_mana + base_mana / 2,
                Position::Sitting => base_mana + base_mana / 4,
                _ => base_mana,
            };
            c.hp = (c.hp + hp).min(c.max_hp);
            c.mana = (c.mana + mana).min(c.max_mana);
            c.mv = (c.mv + mv).min(c.max_mv);
        }
    }

    /// The end of a block of output: everyone who got something this pulse gets their numbers.
    fn prompts(&mut self) {
        let mut seen = Vec::new();
        for d in &self.out {
            if !seen.contains(&d.to) {
                seen.push(d.to.clone());
            }
        }
        for name in seen {
            let Some(c) = self.by_name.get(&key_name(&name)).and_then(|k| self.chars.get(*k)) else { continue };
            let event = Event::Prompt { hp: Some(c.hp), mp: Some(c.mana), mv: Some(c.mv) };
            self.out.push(Delivery { tick: self.tick, to: name, event });
        }
    }

    // ---- perception (MECHANICS §3.1, §3.5, §3.6) ------------------------------------------------

    /// Whether the room is lit for this viewer. Lights and infravision come with objects and affects.
    fn can_see_in(&self, _viewer: Key, room: RoomIx) -> bool {
        let r = &self.world.rooms[room];
        if r.flags.contains(&RoomFlag::Dark) {
            return false;
        }
        if matches!(r.sector, Sector::Inside | Sector::City) {
            return true;
        }
        !matches!(self.hour, 21..=23 | 0..=4)
    }

    fn can_see(&self, viewer: Key, target: Key) -> bool {
        if viewer == target {
            return true;
        }
        let Some(v) = self.chars.get(viewer) else { return false };
        self.can_see_in(viewer, v.room)
    }

    /// An event about `actor` to everyone else in `room` who is awake. With `must_see` only those who
    /// can see the actor get it (act()'s hide_invisible); otherwise the rest get it with "someone".
    fn to_room(&mut self, actor: Key, room: RoomIx, must_see: bool, make: impl Fn(String, Option<String>) -> Event) {
        let name = self.chars.get(actor).map(|c| c.name.clone()).unwrap_or_default();
        for k in self.people[room].clone() {
            if k == actor {
                continue;
            }
            let Some(c) = self.chars.get(k) else { continue };
            if c.position <= Position::Sleeping || !c.linked {
                continue;
            }
            let seen = self.can_see(k, actor);
            if must_see && !seen {
                continue;
            }
            let event = if seen { make(name.clone(), Some(char_id(&name))) } else { make("someone".into(), None) };
            self.deliver(k, event);
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

#[derive(Clone, Copy)]
enum Action {
    Move(usize),
    Look,
    QuitPrefix,
    Quit,
    Say,
}

struct Cmd {
    name: &'static str,
    min: Position,
    action: Action,
}

/// The commands so far, in tbaMUD's table order: the first whose name starts with what was typed wins
/// (interpreter.c:67-283, MECHANICS §2.1).
const COMMANDS: &[Cmd] = &[
    Cmd { name: "north", min: Position::Standing, action: Action::Move(0) },
    Cmd { name: "east", min: Position::Standing, action: Action::Move(1) },
    Cmd { name: "south", min: Position::Standing, action: Action::Move(2) },
    Cmd { name: "west", min: Position::Standing, action: Action::Move(3) },
    Cmd { name: "up", min: Position::Standing, action: Action::Move(4) },
    Cmd { name: "down", min: Position::Standing, action: Action::Move(5) },
    Cmd { name: "look", min: Position::Resting, action: Action::Look },
    Cmd { name: "qui", min: Position::Dead, action: Action::QuitPrefix },
    Cmd { name: "quit", min: Position::Dead, action: Action::Quit },
    Cmd { name: "say", min: Position::Resting, action: Action::Say },
    Cmd { name: "'", min: Position::Resting, action: Action::Say },
];

/// MECHANICS §4.2 table: the refusal for a position too low for the command.
fn refusal_for(p: Position) -> Refusal {
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

/// The age curve (MECHANICS §5.1).
fn graf(age: i32, p: [i32; 7]) -> i32 {
    match age {
        ..15 => p[0],
        15..=29 => p[1] + (age - 15) * (p[2] - p[1]) / 15,
        30..=44 => p[2] + (age - 30) * (p[3] - p[2]) / 15,
        45..=59 => p[3] + (age - 45) * (p[4] - p[3]) / 15,
        60..=79 => p[4] + (age - 60) * (p[5] - p[4]) / 20,
        _ => p[6],
    }
}

fn key_name(name: &str) -> String {
    name.to_lowercase()
}

/// A player's ID in events: `pc:<name>`, the same across sessions.
pub fn char_id(name: &str) -> String {
    format!("pc:{}", key_name(name))
}
