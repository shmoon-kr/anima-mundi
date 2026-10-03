//! Combat (MECHANICS §7, §8, §9, §10.1), each test pointing at its section.

use std::path::Path;

use mundi_content::{load_tables, load_zone, Tables, ZoneContent};
use mundi_protocol::{AttackRefusal, Event, HitOutcome};
use mundi_sim::{Class, Conditions, Delivery, Input, NewChar, Sim, PULSES_PER_SEC, PULSES_PER_TICK};

const BEGGAR_ROOM: &str = "tba:30:room:3044";
const DRUNK_ROOM: &str = "tba:30:room:3007";
const ZOMBIE_ROOM: &str = "tba:186:room:18620";
const TEMPLE: &str = "tba:30:room:3001";

fn world() -> (Vec<ZoneContent>, Tables) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/tbamud");
    let zones = [30, 186].iter().map(|z| load_zone(&root.join(format!("content/{z}"))).unwrap()).collect();
    (zones, load_tables(&root.join("tables")).unwrap())
}

struct T {
    sim: Sim,
    seen: Vec<Delivery>,
}

impl T {
    fn new(seed: u64) -> T {
        let (z, t) = world();
        T { sim: Sim::new(&z, &t, seed, 12), seen: vec![] }
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
    fn until(&mut self, max: u64, done: impl Fn(&Event) -> bool) -> bool {
        for _ in 0..max {
            let out = self.sim.step();
            let hit = out.iter().any(|d| done(&d.event));
            self.seen.extend(out);
            if hit {
                return true;
            }
        }
        false
    }
    fn to(&self, name: &str) -> Vec<&Event> {
        self.seen.iter().filter(|d| d.to == name).map(|d| &d.event).collect()
    }
}

#[test]
fn a_fight_to_the_death_gives_experience_and_a_corpse() {
    // §7.1-7.4 rounds and blows, §8.1 damage, §8.3 death, §8.4 corpse, §9.2 solo experience
    let mut t = T::new(1);
    t.enter("War", BEGGAR_ROOM);
    let exp_before = t.sim.save("War").unwrap().exp;
    t.cmd("War", "kill beggar");
    assert!(t.until(60 * PULSES_PER_SEC, |e| matches!(e, Event::Death { .. })), "the beggar dies within a minute");
    let hits: Vec<_> = t.to("War").into_iter().filter(|e| matches!(e, Event::Hit { .. })).collect();
    assert!(!hits.is_empty());
    assert!(t.to("War").iter().any(|e| matches!(e, Event::ExpGain { amount: 33, .. })), "100 / 3 at the same level");
    assert!(t.sim.save("War").unwrap().exp > exp_before + 33, "plus level × damage per blow");
    assert!(!t.sim.mob_places().iter().any(|(id, room)| id.starts_with("tba:30:mob:3065/") && room == BEGGAR_ROOM), "the beggar is gone");
    // The corpse lies there; looking finds it.
    t.cmd("War", "look");
    let Some(Event::Room(v)) = t.to("War").into_iter().rev().find(|e| matches!(e, Event::Room(_))) else { panic!() };
    assert!(v.objects.iter().any(|o| o.text == "The corpse of the beggar is lying here."), "{:?}", v.objects);
    // It rots in 5 ticks (config.c:77).
    t.run(5 * PULSES_PER_TICK);
    assert!(t.to("War").iter().any(|e| matches!(e, Event::Decayed { carried: false, .. })));
}

#[test]
fn a_player_dies_comes_back_and_loses_half_the_experience() {
    // §8.3: half the experience, the corpse keeps everything, back at the start room standing
    let mut t = T::new(2);
    t.enter("War", ZOMBIE_ROOM);
    // The passage is dark: without a light the zombie cannot see whom to attack.
    t.input(Input::SetLight { name: "War".into(), on: true });
    t.input(Input::Load { name: "War".into(), object: "tba:30:obj:3009".into(), carry: true });
    t.input(Input::SetPoints { name: "War".into(), hp: Some(1), mana: None, mv: None, conditions: None });
    let exp = t.sim.save("War").unwrap().exp;
    assert!(t.until(10 * 60 * PULSES_PER_SEC, |e| matches!(e, Event::SelfDied {})), "the aggressive zombie kills a 1 hp warrior");
    let s = t.sim.save("War").unwrap();
    assert_eq!(s.room.as_deref(), Some(TEMPLE));
    assert!(s.hp >= 1 && s.objects.is_empty());
    assert!(s.exp <= exp, "lost half (it gained a little per blow first)");
    assert!(t.to("War").iter().any(|e| matches!(e, Event::Hit { victim, .. } if victim == "self")));
}

#[test]
fn flee_wimpy_and_refusals() {
    // §10.1 flee, §7.2 hit refusals, §8.1 peaceful rooms
    let mut t = T::new(3);
    t.enter("War", DRUNK_ROOM);
    t.cmd("War", "hit");
    assert!(t.to("War").contains(&&Event::AttackRefused { reason: AttackRefusal::Who }));
    t.cmd("War", "hit nobody");
    assert!(t.to("War").contains(&&Event::AttackRefused { reason: AttackRefusal::NotHere }));
    t.cmd("War", "kill drunk");
    t.run(25);
    t.cmd("War", "flee");
    t.run(3);
    assert!(t.to("War").iter().any(|e| matches!(e, Event::SelfFled { .. }) || matches!(e, Event::FleeFailed { .. })));

    let mut t = T::new(4);
    t.enter("War", TEMPLE);
    t.input(Input::LoadMob { name: "War".into(), mob: "tba:30:mob:3064".into() });
    t.cmd("War", "kill drunk");
    assert!(t.to("War").contains(&&Event::AttackRefused { reason: AttackRefusal::Peaceful }), "the temple is peaceful");

    let mut t = T::new(5);
    t.enter("War", DRUNK_ROOM);
    t.cmd("War", "toggle wimpy 5");
    t.input(Input::SetPoints { name: "War".into(), hp: Some(8), mana: None, mv: None, conditions: Some(Conditions { drunk: 0, full: 24, thirst: 24 }) });
    t.cmd("War", "kill drunk");
    let _ = t.until(2 * 60 * PULSES_PER_SEC, |e| matches!(e, Event::Wimpy {}));
}

#[test]
fn aggressive_mobs_attack_who_comes_in() {
    // §7.5
    let mut t = T::new(6);
    t.enter("War", ZOMBIE_ROOM);
    assert!(!t.until(15 * PULSES_PER_SEC, |e| matches!(e, Event::Hit { .. })), "a dark passage: the zombie cannot see him");
    t.input(Input::SetLight { name: "War".into(), on: true });
    assert!(t.until(25 * PULSES_PER_SEC, |e| matches!(e, Event::Hit { .. })), "lit: within a mob turn or two (unless it wandered)");
    assert!(t.to("War").iter().any(|e| matches!(e, Event::Hit { victim, attacker, .. } if victim == "self" && attacker.contains("zombie"))));
}

#[test]
fn blows_hit_and_miss_like_thac0_says() {
    // §7.4: a level 1 warrior (THAC0 20) against AC 100 (beggar file armor 10 -> AC/10 = 10):
    // hit when 20 - roll <= 10, i.e. roll >= 10, so 11 of 20; plus strength's bonus.
    let mut hits = 0;
    let mut swings = 0;
    for seed in 0..40 {
        let mut t = T::new(100 + seed);
        t.enter("War", BEGGAR_ROOM);
        t.cmd("War", "kill beggar");
        t.until(60 * PULSES_PER_SEC, |e| matches!(e, Event::Death { .. }));
        for e in t.to("War") {
            if let Event::Hit { attacker, outcome, .. } = e {
                if attacker == "self" {
                    swings += 1;
                    if *outcome != HitOutcome::Miss {
                        hits += 1;
                    }
                }
            }
        }
    }
    let rate = hits as f64 / swings as f64;
    assert!(swings > 50 && (0.4..0.9).contains(&rate), "{hits}/{swings}");
}

#[test]
fn combat_replays_the_same() {
    let (z, tables) = world();
    let mut t = T::new(7);
    t.enter("War", BEGGAR_ROOM);
    t.cmd("War", "kill beggar");
    t.run(40 * PULSES_PER_SEC);
    let log = t.sim.input_log().to_vec();
    let last = log.last().unwrap().tick;
    let replay = Sim::replay(&z, &tables, 7, 12, &log);
    let live: Vec<_> = t.seen.iter().filter(|d| d.tick <= last).cloned().collect();
    let bytes = |d: &[Delivery]| serde_json::to_string(d).unwrap();
    assert_eq!(bytes(&replay), bytes(&live));
}
