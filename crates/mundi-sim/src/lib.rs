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
            .or_else(|| self.world.index.get(&self.tables.world.config.start_room))
            .copied()
            .unwrap_or(0);
        // A new character (MECHANICS §9.4); the level-up roll and classes come with S5.
        let new = &self.tables.world.config.new_character;
        let (hp, mana, mv) = (new.max_hit, new.max_mana, new.max_move);
        let key = self.chars.insert(Char {
            name: name.to_string(),
            room,
            position: Position::Standing,
            level: 1,
            hp,
            max_hp: hp,
            mana,
            max_mana: mana,
            mv,
            max_mv: mv,
            linked: true,
            queue: VecDeque::new(),
            affects: Vec::new(),
            light: false,
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
        let cost = (self.sector_cost(self.world.rooms[from].sector) + self.sector_cost(self.world.rooms[to].sector)) / 2;
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
        let me = self.chars.get(key).unwrap();
        let room = me.room;
        let r = &self.world.rooms[room];
        if me.has(Affect::Blind) {
            self.deliver(key, Event::RoomDark { id: Some(r.id.clone()), blind: true, glowing_eyes: 0 });
            return;
        }
        if !self.can_see_in(key, room) {
            // In the dark, those with infravision show as eyes (act.informative.c list_char_to_char).
            let eyes = self.people[room]
                .iter()
                .filter(|k| **k != key && !self.can_see(key, **k))
                .filter(|k| self.chars.get(**k).is_some_and(|c| c.has(Affect::Infravision)))
                .count() as u32;
            self.deliver(key, Event::RoomDark { id: Some(r.id.clone()), blind: false, glowing_eyes: eyes });
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
                flags: [
                    (c.has(Affect::Invisible), "invisible"),
                    (c.has(Affect::Hide), "hidden"),
                    (!c.linked, "linkless"),
                ]
                .iter()
                .filter(|(on, _)| *on)
                .map(|(_, f)| f.to_string())
                .collect(),
                hints: vec![],
                keywords: vec![],
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

    /// stand, sit, rest, sleep, wake (MECHANICS §4.3, act.movement.c:731-950).
    fn position(&mut self, key: Key, cmd: PositionCommand, arg: &str) {
        let c = self.chars.get(key).unwrap();
        let (from, room) = (c.position, c.room);
        let refuse = |reason| Event::PositionRefused { command: cmd, reason };
        if cmd == PositionCommand::Wake && !arg.is_empty() {
            return self.wake_other(key, arg);
        }
        let to = match (cmd, from) {
            (PositionCommand::Stand, Position::Sitting | Position::Resting) => Position::Standing,
            (PositionCommand::Sit, Position::Standing | Position::Resting) => Position::Sitting,
            (PositionCommand::Rest, Position::Standing | Position::Sitting) => Position::Resting,
            (PositionCommand::Sleep, Position::Standing | Position::Sitting | Position::Resting) => Position::Sleeping,
            (PositionCommand::Wake, Position::Sleeping) if c.has(Affect::Sleep) => {
                return self.deliver(key, refuse(PositionRefusal::Magic));
            }
            (PositionCommand::Wake, Position::Sleeping) => Position::Sitting,
            (PositionCommand::Wake, _) => return self.deliver(key, refuse(PositionRefusal::Already)),
            (PositionCommand::Sleep, Position::Sleeping) => return self.deliver(key, refuse(PositionRefusal::Already)),
            (_, Position::Fighting) => return self.deliver(key, refuse(PositionRefusal::Fighting)),
            (_, Position::Sleeping) => return self.deliver(key, refuse(PositionRefusal::Asleep)),
            _ => return self.deliver(key, refuse(PositionRefusal::Already)),
        };
        self.chars.get_mut(key).unwrap().position = to;
        self.deliver(key, Event::SelfPosition { position: to, from, awakened_by: None });
        // "$n sits down." is the one shown to those who cannot see (as "Someone"): act(..., FALSE, ...).
        let must_see = !(to == Position::Sitting && from == Position::Standing);
        self.to_room(key, room, must_see, |who, who_id| Event::OccupantPosition { who, who_id, position: to, from });
    }

    fn wake_other(&mut self, key: Key, arg: &str) {
        let c = self.chars.get(key).unwrap();
        if c.position == Position::Sleeping {
            return self.deliver(key, Event::PositionRefused { command: PositionCommand::Wake, reason: PositionRefusal::Asleep });
        }
        let room = c.room;
        let target = self.people[room].iter().copied().find(|k| {
            self.can_see(key, *k) && self.chars.get(*k).is_some_and(|t| t.name.eq_ignore_ascii_case(arg))
        });
        let Some(target) = target else {
            return self.deliver(key, Event::Refused { reason: Refusal::NotHere });
        };
        if target == key {
            return self.position(key, PositionCommand::Wake, "");
        }
        let t = self.chars.get(target).unwrap();
        let (who, who_id) = (t.name.clone(), Some(char_id(&t.name)));
        let failure = if t.position > Position::Sleeping {
            Some(WakeFailure::AlreadyAwake)
        } else if t.has(Affect::Sleep) {
            Some(WakeFailure::Magic)
        } else if t.position < Position::Sleeping {
            Some(WakeFailure::BadShape)
        } else {
            None
        };
        if let Some(reason) = failure {
            return self.deliver(key, Event::WakeFailed { who, who_id, reason });
        }
        let me = self.chars.get(key).unwrap().name.clone();
        self.deliver(key, Event::Woke { who, who_id });
        self.chars.get_mut(target).unwrap().position = Position::Sitting;
        // "You are awakened by $n." reaches the sleeper (TO_SLEEP), named only if they could see them.
        let by = if self.can_see(target, key) { me } else { "someone".into() };
        self.deliver(target, Event::SelfPosition { position: Position::Sitting, from: Position::Sleeping, awakened_by: Some(by) });
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
        let curves = self.tables.world.regen.clone();
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
            let hp = bonus(graf(17, curves.hit), 2, 4, 8, c.position);
            let mv = bonus(graf(17, curves.moves), 2, 4, 8, c.position);
            let base_mana = graf(17, curves.mana);
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

    /// Movement points a room's terrain costs (MECHANICS §2.3).
    fn sector_cost(&self, s: Sector) -> i32 {
        self.tables.world.movement_cost.get(&s).copied().unwrap_or(1)
    }

    // ---- perception (MECHANICS §3.1, §3.5, §3.6) ------------------------------------------------

    /// Whether a room is lit (MECHANICS §3.1): a light in it, else not `dark`, and indoors or in a
    /// city or in daytime.
    fn lit(&self, room: RoomIx) -> bool {
        if self.people[room].iter().any(|k| self.chars.get(*k).is_some_and(|c| c.light)) {
            return true;
        }
        let r = &self.world.rooms[room];
        if r.flags.contains(&RoomFlag::Dark) {
            return false;
        }
        if matches!(r.sector, Sector::Inside | Sector::City) {
            return true;
        }
        !matches!(self.hour, 21..=23 | 0..=4)
    }

    /// The viewer's light condition (MECHANICS §3.5): not blind, and the room lit or infravision.
    fn can_see_in(&self, viewer: Key, room: RoomIx) -> bool {
        let Some(v) = self.chars.get(viewer) else { return false };
        !v.has(Affect::Blind) && (self.lit(room) || v.has(Affect::Infravision))
    }

    /// MECHANICS §3.5: oneself always; others with light, and past invisibility and hiding only
    /// with the senses for them.
    fn can_see(&self, viewer: Key, target: Key) -> bool {
        if viewer == target {
            return true;
        }
        let (Some(v), Some(t)) = (self.chars.get(viewer), self.chars.get(target)) else { return false };
        self.can_see_in(viewer, v.room)
            && (!t.has(Affect::Invisible) || v.has(Affect::DetectInvis))
            && (!t.has(Affect::Hide) || v.has(Affect::SenseLife))
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
    Position(PositionCommand),
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
    Cmd { name: "rest", min: Position::Resting, action: Action::Position(PositionCommand::Rest) },
    Cmd { name: "say", min: Position::Resting, action: Action::Say },
    Cmd { name: "sit", min: Position::Resting, action: Action::Position(PositionCommand::Sit) },
    Cmd { name: "sleep", min: Position::Sleeping, action: Action::Position(PositionCommand::Sleep) },
    Cmd { name: "stand", min: Position::Resting, action: Action::Position(PositionCommand::Stand) },
    Cmd { name: "wake", min: Position::Sleeping, action: Action::Position(PositionCommand::Wake) },
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
