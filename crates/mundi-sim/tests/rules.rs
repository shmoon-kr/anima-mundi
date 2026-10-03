//! Rules, each test pointing at its MECHANICS section, on the converted Midgaard.

use std::path::Path;

use mundi_content::{load_tables, load_zone, Spawn, Tables, ZoneContent};
use mundi_protocol::Event;
use mundi_sim::{Class, Conditions, Delivery, Input, NewChar, Sim, PULSES_PER_TICK};

fn tables() -> Tables {
    load_tables(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/tbamud/tables")).unwrap()
}

fn zones(nums: &[u32]) -> Vec<ZoneContent> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/tbamud/content");
    nums.iter().map(|n| load_zone(&root.join(n.to_string())).unwrap()).collect()
}

fn enter(sim: &mut Sim, name: &str, class: Class, room: Option<&str>) -> Vec<Delivery> {
    sim.submit(Input::Enter { name: name.into(), save: None, new: NewChar { class: Some(class), room: room.map(Into::into), ..Default::default() } });
    sim.step()
}

fn ticks(sim: &mut Sim, n: u64) -> Vec<Delivery> {
    let mut out = vec![];
    for _ in 0..n * PULSES_PER_TICK {
        out.extend(sim.step());
    }
    out
}

fn to<'a>(out: &'a [Delivery], name: &str) -> Vec<&'a Event> {
    out.iter().filter(|d| d.to == name).map(|d| &d.event).filter(|e| !matches!(e, Event::Prompt { .. })).collect()
}

#[test]
fn new_characters_by_class() {
    // MECHANICS §17.2, §9.4
    let z = zones(&[30]);
    let t = tables();
    for seed in 0..20 {
        let mut sim = Sim::new(&z, &t, seed, 12);
        for (name, class) in [("Mag", Class::MagicUser), ("Cle", Class::Cleric), ("Thi", Class::Thief), ("War", Class::Warrior)] {
            enter(&mut sim, name, class, None);
            let s = sim.save(name).unwrap();
            let a = s.abilities;
            let order = match class {
                Class::MagicUser => [a.int, a.wis, a.dex, a.str, a.con, a.cha],
                Class::Cleric => [a.wis, a.int, a.str, a.dex, a.con, a.cha],
                Class::Thief => [a.dex, a.str, a.con, a.int, a.wis, a.cha],
                Class::Warrior => [a.str, a.dex, a.con, a.wis, a.int, a.cha],
            };
            assert!(order.windows(2).all(|w| w[0] >= w[1]), "{class:?} {order:?}");
            assert!(order.iter().all(|v| (3..=18).contains(v)));
            assert!(a.str_add == 0 || (class == Class::Warrior && a.str == 18));
            assert_eq!((s.level, s.exp), (1, 1));
            assert!(s.max_hp > 10 && s.hp == s.max_hp, "one level gained");
            assert_eq!(s.max_mana, 100, "no mana at level 1");
            assert!(s.max_mv > 82);
            assert_eq!(s.conditions, Conditions { drunk: 0, full: 24, thirst: 24 });
            assert_eq!(s.skills.get("backstab").copied(), (class == Class::Thief).then_some(10));
            assert!(s.practices >= 1);
        }
    }
}

#[test]
fn hunger_and_thirst_and_their_effect_on_regeneration() {
    // MECHANICS §6.1, §5.2
    let mut sim = Sim::new(&zones(&[30]), &tables(), 1, 12);
    enter(&mut sim, "War", Class::Warrior, None);
    let out = ticks(&mut sim, 23);
    assert!(to(&out, "War").iter().all(|e| !matches!(e, Event::Condition { .. })));
    let out = ticks(&mut sim, 1);
    assert!(to(&out, "War").contains(&&Event::Condition { hungry: Some(true), thirsty: None, full: None, quenched: None, sober: None }));
    assert!(to(&out, "War").contains(&&Event::Condition { hungry: None, thirsty: Some(true), full: None, quenched: None, sober: None }));
    let out = ticks(&mut sim, 1);
    assert_eq!(to(&out, "War").iter().filter(|e| matches!(e, Event::Condition { .. })).count(), 2, "again each tick at zero");

    // A warrior of 17 standing gains graf(17, 8,12,20,...) = 13 hit points; hungry, a quarter.
    sim.submit(Input::SetPoints { name: "War".into(), hp: Some(1), mana: None, mv: None, conditions: Some(Conditions { drunk: 0, full: 24, thirst: 24 }) });
    ticks(&mut sim, 1);
    assert_eq!(sim.save("War").unwrap().hp, 1 + 13);
    sim.submit(Input::SetPoints { name: "War".into(), hp: Some(1), mana: None, mv: None, conditions: Some(Conditions { drunk: 0, full: 0, thirst: 24 }) });
    ticks(&mut sim, 1);
    assert_eq!(sim.save("War").unwrap().hp, 1 + 13 / 4);
    // Resting: + a quarter (13 + 3).
    sim.submit(Input::SetPoints { name: "War".into(), hp: Some(1), mana: None, mv: None, conditions: Some(Conditions { drunk: 0, full: 24, thirst: 24 }) });
    sim.submit(Input::Command { name: "War".into(), text: "rest".into() });
    ticks(&mut sim, 1);
    assert_eq!(sim.save("War").unwrap().hp, 1 + 16);
}

#[test]
fn casters_regenerate_hit_points_at_half_and_mana_double() {
    // MECHANICS §5.2: mana graf(17, 4,8,12,...) = 8, ×2 for a caster; hit points 13 / 2 = 6.
    let mut sim = Sim::new(&zones(&[30]), &tables(), 1, 12);
    enter(&mut sim, "Mag", Class::MagicUser, None);
    sim.submit(Input::SetPoints { name: "Mag".into(), hp: Some(1), mana: Some(0), mv: None, conditions: None });
    ticks(&mut sim, 1);
    let s = sim.save("Mag").unwrap();
    assert_eq!((s.hp, s.mana), (1 + 6, 16));
}

#[test]
fn the_boot_reset_puts_mobs_and_objects_in_their_rooms() {
    // MECHANICS §14.1, §3.3, §3.4
    let z = zones(&[30]);
    let mob = z[0].resets.spawns.iter().find_map(|s| match s {
        Spawn::Mob(m) => Some(m.clone()),
        _ => None,
    });
    let mob = mob.expect("Midgaard resets mobs");
    let mut sim = Sim::new(&z, &tables(), 1, 12);
    let out = enter(&mut sim, "War", Class::Warrior, Some(&mob.room));
    let room = to(&out, "War").into_iter().find_map(|e| match e {
        Event::Room(v) => Some(v.clone()),
        _ => None,
    });
    let room = room.unwrap();
    let here = room.occupants.iter().find(|o| o.id.as_deref().is_some_and(|id| id.starts_with(&format!("{}/", mob.mob))));
    let here = here.expect("the mob is there");
    assert!(!here.keywords.is_empty());

    let obj = z[0].resets.spawns.iter().find_map(|s| match s {
        Spawn::Object(o) if o.room.is_some() => Some(o.clone()),
        _ => None,
    });
    if let Some(obj) = obj {
        sim.submit(Input::Enter { name: "Thi".into(), save: None, new: NewChar { room: obj.room.clone(), ..Default::default() } });
        let out = sim.step();
        let Some(Event::Room(v)) = to(&out, "Thi").into_iter().find(|e| matches!(e, Event::Room(_))) else { panic!() };
        assert!(v.objects.iter().any(|o| o.id.as_deref().is_some_and(|id| id.starts_with(&format!("{}/", obj.object)))), "{:?}", v.objects);
    }
}
