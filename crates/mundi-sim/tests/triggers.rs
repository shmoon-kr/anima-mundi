//! The triggers the party meets, as native rules (MECHANICS §14.3, tables/triggers.yaml), as seen
//! on the live server.

use std::path::Path;

use mundi_content::{load_tables, load_zone};
use mundi_protocol::{Direction, Event};
use mundi_sim::{Class, Delivery, Input, NewChar, Sim, PULSES_PER_SEC};

const TEMPLE: &str = "tba:30:room:3001";
const DONATION: &str = "tba:30:room:3063";
const DUMP: &str = "tba:30:room:3030";

struct T {
    sim: Sim,
    seen: Vec<Delivery>,
}

impl T {
    fn new() -> T {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/tbamud");
        let z = vec![load_zone(&root.join("content/30")).unwrap()];
        T { sim: Sim::new(&z, &load_tables(&root.join("tables")).unwrap(), 4, 12), seen: vec![] }
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
    }
    fn run(&mut self, pulses: u64) {
        for _ in 0..pulses {
            let out = self.sim.step();
            self.seen.extend(out);
        }
    }
    fn got(&self, name: &str, f: impl Fn(&Event) -> bool) -> bool {
        self.seen.iter().any(|d| d.to == name && f(&d.event))
    }
}

#[test]
fn a_new_player_is_welcomed() {
    // 30.trg #3017 and config.c START_MESSG; no kit (seen live: do_start comes before the check)
    let mut t = T::new();
    t.enter("Old", "tba:30:room:3014");
    t.enter("Ana", TEMPLE);
    assert!(t.got("Ana", |e| matches!(e, Event::NewCharacter {})));
    t.run(5 * PULSES_PER_SEC);
    let welcome = |e: &Event| matches!(e, Event::Echo { text } if text == "A booming voice announces, 'Welcome Ana to the realm!'");
    assert!(t.got("Ana", welcome) && t.got("Old", welcome), "the whole zone hears it");
    assert!(t.sim.save("Ana").unwrap().objects.is_empty(), "no kit at login");
}

#[test]
fn the_kind_soul_dresses_the_naked_and_fills_a_gap() {
    // 30.trg #3016, seen live: the whole kit worn, two seconds after coming in
    let mut t = T::new();
    t.enter("Ana", TEMPLE);
    t.cmd("Ana", "east");
    assert_eq!(t.sim.room_of("Ana"), Some(DONATION));
    t.run(3 * PULSES_PER_SEC);
    assert!(t.got("Ana", |e| matches!(e, Event::Say { text, direction: Direction::In, .. } if text == "get some clothes on! Here, I will help.")));
    let worn = t.sim.save("Ana").unwrap().objects.iter().filter(|o| o.worn.is_some()).count();
    assert_eq!(worn, 18, "every slot");
    // Without body armour: out and back in, and it says so and gives a breast plate.
    t.cmd("Ana", "remove plate");
    t.cmd("Ana", "drop plate");
    t.cmd("Ana", "west");
    t.cmd("Ana", "east");
    t.run(3 * PULSES_PER_SEC);
    assert!(t.got("Ana", |e| matches!(e, Event::Say { text, .. } if text == "you won't get far without some body armor Ana.")));
    assert!(t.got("Ana", |e| matches!(e, Event::Received { text, .. } if text == "a breast plate")));
}

#[test]
fn the_janitor_and_fido_clean_up() {
    // 30.trg #3011, #3010: random 100% each 13-second check
    // A field whose one exit leads to a no_mob room: the janitor cannot wander off.
    let mut t = T::new();
    t.enter("Ana", "tba:30:room:3065");
    t.input(Input::LoadMob { name: "Ana".into(), mob: "tba:30:mob:3061".into() });
    t.input(Input::Load { name: "Ana".into(), object: "tba:30:obj:3010".into(), carry: true });
    t.cmd("Ana", "drop bread");
    t.run(14 * PULSES_PER_SEC);
    // A mob in an event carries its own ID (`<prototype>/<serial>`), not a player's.
    assert!(t.got("Ana", |e| matches!(e, Event::OccupantItem { who_id: Some(id), .. } if id.starts_with("tba:30:mob:3061/"))));
    assert!(t.got("Ana", |e| matches!(e, Event::OccupantItem { who, text, .. } if who == "the janitor" && text == "a bread")), "{:?}", t.seen.iter().filter(|d| d.to == "Ana" && !matches!(d.event, Event::Prompt { .. } | Event::Room(_))).map(|d| &d.event).collect::<Vec<_>>());

    let mut t = T::new();
    t.enter("War", "tba:30:room:3044");
    t.input(Input::LoadMob { name: "War".into(), mob: "tba:30:mob:3062".into() });
    t.cmd("War", "kill beggar");
    let died = (0..60 * PULSES_PER_SEC).any(|_| {
        let out = t.sim.step();
        let d = out.iter().any(|d| matches!(d.event, Event::Death { .. }) || matches!(d.event, Event::Hit { outcome: mundi_protocol::HitOutcome::Die, .. }));
        t.seen.extend(out);
        d
    });
    assert!(died);
    t.run(14 * PULSES_PER_SEC);
    assert!(t.got("War", |e| matches!(e, Event::Emote { text, .. } if text == "savagely devours a corpse.")), "fido eats the beggar");
}

#[test]
fn the_dump_rewards_a_drop() {
    // 30.trg #3004: cost / 10 within 1-50, as experience below level 3
    let mut t = T::new();
    t.enter("Ana", DUMP);
    t.input(Input::Load { name: "Ana".into(), object: "tba:30:obj:3040".into(), carry: true });
    let exp = t.sim.save("Ana").unwrap().exp;
    t.cmd("Ana", "drop plate");
    assert!(t.got("Ana", |e| matches!(e, Event::Echo { text } if text == "You are awarded for outstanding performance.")));
    assert!(t.sim.save("Ana").unwrap().objects.is_empty());
    let gain = t.sim.save("Ana").unwrap().exp - exp;
    assert!((1..=50).contains(&gain), "{gain}");
}
