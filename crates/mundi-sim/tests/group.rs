//! Following and groups (MECHANICS §2.5, §7.3, §9.2, §12).

use std::path::Path;

use mundi_content::{load_tables, load_zone};
use mundi_protocol::{Direction, Event};
use mundi_sim::{Class, Delivery, Input, NewChar, Sim, PULSES_PER_SEC};

const BEGGAR_ROOM: &str = "tba:30:room:3044";
const DRUNK_ROOM: &str = "tba:30:room:3007";

struct T {
    sim: Sim,
    seen: Vec<Delivery>,
}

impl T {
    fn new(seed: u64) -> T {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/tbamud");
        let z = vec![load_zone(&root.join("content/30")).unwrap()];
        T { sim: Sim::new(&z, &load_tables(&root.join("tables")).unwrap(), seed, 12), seen: vec![] }
    }
    fn input(&mut self, i: Input) {
        self.sim.submit(i);
        let out = self.sim.step();
        self.seen.extend(out);
    }
    fn enter(&mut self, name: &str, room: &str) {
        self.input(Input::Enter { name: name.into(), save: None, new: NewChar { class: Some(Class::Warrior), room: Some(room.into()), ..Default::default() } });
    }
    fn cmd(&mut self, name: &str, text: &str) {
        self.input(Input::Command { name: name.into(), text: text.into() });
        // let the one-pulse command wait pass
        self.input(Input::Command { name: name.into(), text: String::new() });
    }
    fn until(&mut self, max: u64, done: impl Fn(&Delivery) -> bool) -> bool {
        for _ in 0..max {
            let out = self.sim.step();
            let hit = out.iter().any(&done);
            self.seen.extend(out);
            if hit {
                return true;
            }
        }
        false
    }
    fn got(&self, name: &str, f: impl Fn(&Event) -> bool) -> bool {
        self.seen.iter().any(|d| d.to == name && f(&d.event))
    }
}

#[test]
fn followers_come_along_standing_only() {
    // §2.5
    let mut t = T::new(1);
    t.enter("Ana", "tba:30:room:3001");
    t.enter("Bo", "tba:30:room:3001");
    t.enter("Cy", "tba:30:room:3001");
    t.cmd("Bo", "follow ana");
    assert!(t.got("Bo", |e| matches!(e, Event::GroupChange { event, who, .. } if event == "following" && who == "Ana")));
    assert!(t.got("Ana", |e| matches!(e, Event::GroupChange { event, .. } if event == "followed_by")));
    assert!(t.got("Cy", |e| matches!(e, Event::OccupantFollow { stopped: false, .. })));
    t.cmd("Ana", "follow bo");
    assert!(t.got("Ana", |e| matches!(e, Event::GroupFailed { reason, .. } if reason == "loop")));
    t.cmd("Cy", "follow ana");
    t.cmd("Cy", "sit");
    t.cmd("Ana", "north");
    assert!(t.got("Bo", |e| matches!(e, Event::FollowMoved { leader, .. } if leader == "Ana")));
    assert_eq!(t.sim.room_of("Bo"), t.sim.room_of("Ana"));
    assert_ne!(t.sim.room_of("Cy"), t.sim.room_of("Ana"), "a sitting follower stays");
    t.cmd("Bo", "follow self");
    assert!(t.got("Bo", |e| matches!(e, Event::GroupChange { event, .. } if event == "stopped_following")));
}

#[test]
fn a_group_shares_a_kill_and_talks() {
    // §12.1, §9.2
    let mut t = T::new(2);
    t.enter("Ana", BEGGAR_ROOM);
    t.enter("Bo", BEGGAR_ROOM);
    t.cmd("Ana", "group new");
    assert!(t.got("Ana", |e| matches!(e, Event::GroupChange { event, who, .. } if event == "leader" && who == "Ana")));
    t.cmd("Bo", "group join ana");
    assert!(t.got("Ana", |e| matches!(e, Event::GroupChange { event, who, .. } if event == "joined" && who == "Bo")));
    t.cmd("Bo", "gsay hello");
    assert!(t.got("Ana", |e| matches!(e, Event::Gtell { from, direction: Direction::In, .. } if from == "Bo")));
    assert!(t.got("Bo", |e| matches!(e, Event::Gtell { direction: Direction::Out, .. })));
    t.cmd("Ana", "group");
    assert!(t.got("Ana", |e| matches!(e, Event::GroupStatus { members } if members.len() == 2 && members[0].leader)));
    t.cmd("Ana", "autoloot");
    t.cmd("Ana", "kill beggar");
    assert!(t.until(60 * PULSES_PER_SEC, |d| matches!(d.event, Event::Death { .. })));
    // The beggar was worth 100, plus what it gained hitting Ana (a mob gains level × damage per
    // blow too, fight.c:689-690): the same share each, at least (33 + 1) / 2.
    let share = |who: &str| t.seen.iter().filter(|d| d.to == who).find_map(|d| match &d.event {
        Event::ExpGain { amount, kind } if kind == "share" => Some(*amount),
        _ => None,
    });
    assert_eq!(share("Ana"), share("Bo"));
    assert!(share("Ana").unwrap() >= 17);
    assert!(t.got("Ana", |e| matches!(e, Event::Got { from: Some(c), .. } if c.contains("corpse"))), "autoloot takes from the corpse");
    // The leader leaves; the other takes over.
    t.cmd("Ana", "group leave");
    assert!(t.got("Bo", |e| matches!(e, Event::GroupChange { event, who, .. } if event == "new_leader" && who == "Bo")));
}

#[test]
fn autoassist_joins_a_group_members_fight() {
    // §7.3
    let mut t = T::new(3);
    t.enter("Ana", DRUNK_ROOM);
    t.enter("Bo", DRUNK_ROOM);
    t.cmd("Ana", "group new");
    t.cmd("Bo", "group join ana");
    t.cmd("Bo", "autoassist");
    t.cmd("Ana", "kill drunk");
    assert!(t.until(10 * PULSES_PER_SEC, |d| d.to == "Bo" && matches!(&d.event, Event::Assisted { who, .. } if who == "self")), "Bo joins the fight");
}
