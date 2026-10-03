//! Doors (MECHANICS §2.7).

use std::path::Path;

use mundi_content::{load_tables, load_zone};
use mundi_protocol::{Event, MoveFailure};
use mundi_sim::{Class, Delivery, Input, NewChar, Sim};

const WEST: &str = "tba:31:room:3110";
const EAST: &str = "tba:31:room:3111";
const KEY: &str = "tba:31:obj:3105";
const GRATE_ROOM: &str = "tba:31:room:3129";

struct T {
    sim: Sim,
    seen: Vec<Delivery>,
}

impl T {
    fn new() -> T {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/tbamud");
        let z = [30, 31].iter().map(|n| load_zone(&root.join(format!("content/{n}"))).unwrap()).collect::<Vec<_>>();
        T { sim: Sim::new(&z, &load_tables(&root.join("tables")).unwrap(), 1, 12), seen: vec![] }
    }
    fn input(&mut self, i: Input) {
        self.sim.submit(i);
        let out = self.sim.step();
        self.seen.extend(out);
    }
    fn enter(&mut self, name: &str, class: Class, room: &str) {
        self.input(Input::Enter { name: name.into(), save: None, new: NewChar { class: Some(class), room: Some(room.into()), ..Default::default() } });
    }
    fn cmd(&mut self, name: &str, text: &str) -> Vec<Event> {
        let from = self.seen.len();
        self.input(Input::Command { name: name.into(), text: text.into() });
        self.input(Input::Command { name: name.into(), text: String::new() });
        self.seen[from..].iter().filter(|d| d.to == name).map(|d| d.event.clone()).collect()
    }
    fn failed(&mut self, name: &str, text: &str) -> Option<String> {
        self.cmd(name, text).into_iter().find_map(|e| match e {
            Event::DoorFailed { reason, .. } => Some(reason),
            _ => None,
        })
    }
}

#[test]
fn a_locked_door_with_its_key() {
    let mut t = T::new();
    t.enter("Ana", Class::Warrior, WEST);
    t.enter("Bo", Class::Warrior, EAST);
    assert_eq!(t.failed("Ana", "open"), Some("what".into()));
    assert_eq!(t.failed("Ana", "open door"), Some("seems_locked".into()));
    assert_eq!(t.failed("Ana", "unlock door"), Some("no_key".into()));
    assert!(t.cmd("Ana", "east").iter().any(|e| matches!(e, Event::MoveFailed { reason: MoveFailure::Closed, .. })));
    t.input(Input::Load { name: "Ana".into(), object: KEY.into(), carry: true });
    assert!(t.cmd("Ana", "unlock door").iter().any(|e| matches!(e, Event::DoorChanged { command, who, .. } if command == "unlock" && who == "self")));
    assert_eq!(t.failed("Bo", "unlock door"), Some("wasnt_locked".into()), "the other side unlocked too");
    t.cmd("Ana", "open door");
    assert!(t.seen.iter().any(|d| d.to == "Bo" && matches!(&d.event, Event::DoorChanged { far: true, command, .. } if command == "open")), "heard from the other side");
    assert_eq!(t.failed("Ana", "open door"), Some("currently_open".into()));
    t.cmd("Ana", "east");
    assert_eq!(t.sim.room_of("Ana"), Some(EAST));
    t.cmd("Ana", "close door");
    t.cmd("Ana", "lock door");
    assert_eq!(t.failed("Bo", "open door"), Some("seems_locked".into()));
    // autokey: unlock on the way.
    t.cmd("Ana", "autokey");
    let out = t.cmd("Ana", "open door");
    assert!(out.iter().any(|e| matches!(e, Event::DoorFailed { reason, .. } if reason == "locked_have_key")));
    assert!(out.iter().any(|e| matches!(e, Event::DoorChanged { command, .. } if command == "open")));
}

#[test]
fn a_pickproof_grate_resists() {
    let mut t = T::new();
    t.enter("Thi", Class::Thief, GRATE_ROOM);
    assert_eq!(t.failed("Thi", "pick grate"), Some("resists".into()));
    assert_eq!(t.failed("Thi", "open grate south"), Some("seems_locked".into()));
    // Naming a direction whose exit has no door: "You can't open that!".
    assert_eq!(t.failed("Thi", "open grate north"), Some("cant".into()));
}
