//! Skills and spells (MECHANICS §10, §11).

use std::path::Path;

use mundi_content::{load_tables, load_zone};
use mundi_protocol::{Event, HitOutcome};
use mundi_sim::{Class, Delivery, Input, NewChar, Sim, PULSES_PER_SEC, PULSES_PER_TICK};

const BEGGAR_ROOM: &str = "tba:30:room:3044";
const DRUNK_ROOM: &str = "tba:30:room:3007";
const DAGGER: &str = "tba:30:obj:3020";

struct T {
    sim: Sim,
    seen: Vec<Delivery>,
}

impl T {
    fn new(seed: u64) -> T {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/tbamud");
        let z = [0, 30].iter().map(|n| load_zone(&root.join(format!("content/{n}"))).unwrap()).collect::<Vec<_>>();
        T { sim: Sim::new(&z, &load_tables(&root.join("tables")).unwrap(), seed, 12), seen: vec![] }
    }
    fn input(&mut self, i: Input) {
        self.sim.submit(i);
        let out = self.sim.step();
        self.seen.extend(out);
    }
    fn enter(&mut self, name: &str, class: Class, room: &str) {
        self.input(Input::Enter { name: name.into(), save: None, new: NewChar { class: Some(class), room: Some(room.into()), ..Default::default() } });
    }
    fn skill(&mut self, name: &str, skill: &str, value: i32) {
        self.input(Input::SetSkill { name: name.into(), skill: skill.into(), value });
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
    fn reason(&self, name: &str, r: &str) -> bool {
        self.got(name, |e| matches!(e, Event::SkillResult { reason: Some(x), .. } if x == r))
    }
}

#[test]
fn kick_bash_and_their_waits() {
    // §10.3, §10.4
    let mut t = T::new(1);
    t.enter("War", Class::Warrior, DRUNK_ROOM);
    t.cmd("War", "kick drunk");
    assert!(t.reason("War", "no_idea"), "no kick learned yet");
    t.skill("War", "kick", 80);
    t.cmd("War", "kick drunk");
    assert!(t.got("War", |e| matches!(e, Event::Hit { attack: 134, attacker, .. } if attacker == "self")), "a kick, hit or miss, from the message file's kick");
    // The kick's wait (6 s): a say right after waits.
    t.cmd("War", "say after");
    t.run(2);
    assert!(!t.got("War", |e| matches!(e, Event::Say { .. })), "still waiting");
    t.run(60);
    assert!(t.got("War", |e| matches!(e, Event::Say { .. })));

    t.skill("War", "bash", 80);
    t.cmd("War", "bash drunk");
    assert!(t.reason("War", "need_weapon"));
}

#[test]
fn a_thief_backstabs_with_a_dagger() {
    // §10.6: thieves start with backstab 10; the dagger pierces
    let mut t = T::new(2);
    t.enter("Thi", Class::Thief, BEGGAR_ROOM);
    t.input(Input::Load { name: "Thi".into(), object: DAGGER.into(), carry: true });
    t.cmd("Thi", "wield dagger");
    t.run(2);
    t.cmd("Thi", "backstab beggar");
    assert!(t.got("Thi", |e| matches!(e, Event::Hit { attack: 131, .. })), "a backstab blow");
}

#[test]
fn rescue_turns_the_foe() {
    // §10.5
    let mut t = T::new(3);
    t.enter("Ana", Class::Warrior, DRUNK_ROOM);
    t.enter("Bo", Class::Warrior, DRUNK_ROOM);
    t.cmd("Bo", "kill drunk");
    t.run(3);
    t.skill("Ana", "rescue", 100);
    t.cmd("Ana", "rescue bo");
    let rescued = t.got("Ana", |e| matches!(e, Event::Rescue { rescuer, .. } if rescuer == "self"));
    let failed = t.reason("Ana", "failed");
    assert!(rescued || failed, "100 still fails on a 101");
    if rescued {
        assert!(t.got("Bo", |e| matches!(e, Event::Rescue { rescued, .. } if rescued == "self")));
    }
}

#[test]
fn casting_magic_missile_mana_and_words() {
    // §11.1-11.3: mana max(25 - 3 × (1 - 1), 10) = 25
    let mut t = T::new(4);
    t.enter("Mag", Class::MagicUser, BEGGAR_ROOM);
    t.enter("War", Class::Warrior, BEGGAR_ROOM);
    t.cmd("Mag", "cast 'magic missile' beggar");
    assert!(t.reason("Mag", "unfamiliar"), "known by level, but not practised");
    t.cmd("Mag", "cast magic missile");
    assert!(t.reason("Mag", "holy_symbols"));
    t.skill("Mag", "magic missile", 95);
    let mana = t.sim.save("Mag").unwrap().mana;
    t.cmd("Mag", "cast 'mag mis' beggar");
    let after = t.sim.save("Mag").unwrap().mana;
    let cast = t.got("Mag", |e| matches!(e, Event::SkillResult { ok: true, .. }));
    if cast {
        assert_eq!(after, mana - 25);
        assert!(t.got("Mag", |e| matches!(e, Event::Hit { attack: 32, .. })));
        // Others of another class hear the syllables: "magic missile" -> "...".
        assert!(t.got("War", |e| matches!(e, Event::SpellSaid { words, .. } if words != "magic missile")));
    } else {
        assert_eq!(after, mana - 25 / 2, "a failed cast costs half");
    }
}

#[test]
fn cure_light_armor_and_its_wear_off() {
    // §11.3, §11.4
    let mut t = T::new(5);
    t.enter("Cle", Class::Cleric, BEGGAR_ROOM);
    t.skill("Cle", "cure light", 100);
    t.skill("Cle", "armor", 100);
    t.input(Input::SetPoints { name: "Cle".into(), hp: Some(1), mana: None, mv: None, conditions: None });
    t.cmd("Cle", "cast 'cure light'");
    t.run(25);
    if t.got("Cle", |e| matches!(e, Event::SpellEffect { spell: 16, .. })) {
        let hp = t.sim.save("Cle").unwrap().hp;
        assert!((3..=11).contains(&hp), "1 + 1d8 + 1 + 0: {hp}");
    }
    t.cmd("Cle", "cast 'armor'");
    t.run(25);
    if t.got("Cle", |e| matches!(e, Event::SpellEffect { spell: 1, .. })) {
        let armor = t.sim.save("Cle").unwrap().spells.iter().find(|s| s.spell == 1).cloned().unwrap();
        assert_eq!((armor.duration, armor.modifier), (24, -20));
        t.run(26 * PULSES_PER_TICK);
        assert!(t.got("Cle", |e| matches!(e, Event::WoreOff { spell: 1, .. })));
        assert!(t.sim.save("Cle").unwrap().spells.iter().all(|s| s.spell != 1));
    }
}

#[test]
fn poison_hurts_each_tick_and_remove_poison_ends_it() {
    // §11.3, §11.4: clerics learn poison at 8, remove poison at 10
    let mut t = T::new(6);
    t.enter("Cle", Class::Cleric, BEGGAR_ROOM);
    t.enter("Bo", Class::Warrior, BEGGAR_ROOM);
    t.cmd("Cle", "cast 'poison' bo");
    assert!(t.reason("Cle", "dont_know"));
    t.input(Input::SetLevel { name: "Cle".into(), level: 10 });
    for (s, v) in [("poison", 100), ("remove poison", 100)] {
        t.skill("Cle", s, v);
    }
    // A save (or a 101) can stop it: cast until it takes.
    for _ in 0..30 {
        if t.sim.save("Bo").unwrap().spells.iter().any(|s| s.spell == 33) {
            break;
        }
        t.input(Input::SetPoints { name: "Cle".into(), hp: None, mana: Some(100), mv: None, conditions: None });
        t.cmd("Cle", "cast 'poison' bo");
        t.run(25);
    }
    let poison = t.sim.save("Bo").unwrap().spells.into_iter().find(|s| s.spell == 33).expect("poisoned in 30 tries");
    assert_eq!((poison.duration, poison.modifier), (10, -2), "the caster's level in ticks, strength -2");
    assert!(t.got("Bo", |e| matches!(e, Event::SpellEffect { spell: 33, line, .. } if line == "vict")));
    let hp = t.sim.save("Bo").unwrap().hp;
    t.run(PULSES_PER_TICK);
    assert!(t.got("Bo", |e| matches!(e, Event::Hit { attack: 33, victim, .. } if victim == "self")), "poison's tick");
    // Regeneration is a quarter while poisoned, then 2 off: never more than + 13/4 - 2.
    assert!(t.sim.save("Bo").unwrap().hp <= hp + 13 / 4);
    for _ in 0..30 {
        if !t.sim.save("Bo").unwrap().spells.iter().any(|s| s.spell == 33) {
            break;
        }
        t.input(Input::SetPoints { name: "Cle".into(), hp: None, mana: Some(100), mv: None, conditions: None });
        t.cmd("Cle", "cast 'remove poison' bo");
        t.run(25);
    }
    assert!(!t.sim.save("Bo").unwrap().spells.iter().any(|s| s.spell == 33));
    assert!(t.got("Bo", |e| matches!(e, Event::SpellEffect { spell: 43, .. })));
}

#[test]
fn create_food_makes_a_waybread() {
    // §11.3: cleric 2; object vnum 10
    let mut t = T::new(7);
    t.enter("Cle", Class::Cleric, BEGGAR_ROOM);
    t.skill("Cle", "create food", 100);
    t.cmd("Cle", "cast 'create food'");
    assert!(t.reason("Cle", "dont_know"));
    t.input(Input::SetLevel { name: "Cle".into(), level: 2 });
    for _ in 0..10 {
        if t.sim.save("Cle").unwrap().objects.iter().any(|o| o.proto == "tba:0:obj:10") {
            break;
        }
        t.input(Input::SetPoints { name: "Cle".into(), hp: None, mana: Some(100), mv: None, conditions: None });
        t.cmd("Cle", "cast 'create food'");
        t.run(25);
    }
    assert!(t.sim.save("Cle").unwrap().objects.iter().any(|o| o.proto == "tba:0:obj:10"));
    assert!(t.got("Cle", |e| matches!(e, Event::SpellEffect { spell: 12, line, .. } if line == "vict")));
    let _ = (HitOutcome::Miss, PULSES_PER_SEC);
}
