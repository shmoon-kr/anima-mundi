//! Objects in hand (MECHANICS §13, §6.2, §6.3, §3.1), each test pointing at its section.

use std::path::Path;

use mundi_content::{load_tables, load_zone, Tables, ZoneContent};
use mundi_protocol::{Event, ItemAction, ItemFailure, Refusal};
use mundi_sim::{Class, Conditions, Delivery, Input, NewChar, Sim, PULSES_PER_TICK};

const TORCH: &str = "tba:30:obj:3030";
const CANDLE: &str = "tba:30:obj:3037";
const PLATE: &str = "tba:30:obj:3040";
const BAG: &str = "tba:30:obj:3032";
const WAYBREAD: &str = "tba:30:obj:3009";
const BOTTLE: &str = "tba:30:obj:3001";
const DAGGER: &str = "tba:30:obj:3020";
const FIELD: &str = "tba:30:room:3065";

fn world() -> (Vec<ZoneContent>, Tables) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/tbamud");
    (vec![load_zone(&root.join("content/30")).unwrap()], load_tables(&root.join("tables")).unwrap())
}

struct T {
    sim: Sim,
}

impl T {
    fn new(hour: u32) -> T {
        let (z, t) = world();
        T { sim: Sim::new(&z, &t, 5, hour) }
    }
    fn enter(&mut self, name: &str, class: Class, room: Option<&str>) {
        self.sim.submit(Input::Enter { name: name.into(), save: None, new: NewChar { class: Some(class), room: room.map(Into::into), ..Default::default() } });
        self.sim.step();
    }
    fn load(&mut self, name: &str, object: &str, carry: bool) {
        self.sim.submit(Input::Load { name: name.into(), object: object.into(), carry });
        self.sim.step();
    }
    fn cmd(&mut self, name: &str, text: &str) -> Vec<Delivery> {
        self.sim.submit(Input::Command { name: name.into(), text: text.into() });
        self.sim.step()
    }
    fn saved_objects(&self, name: &str) -> Vec<(String, Option<String>)> {
        self.sim.save(name).unwrap().objects.iter().map(|o| (o.proto.clone(), o.worn.map(|w| w.name().to_string()))).collect()
    }
}

fn to<'a>(out: &'a [Delivery], name: &str) -> Vec<&'a Event> {
    out.iter().filter(|d| d.to == name).map(|d| &d.event).filter(|e| !matches!(e, Event::Prompt { .. })).collect()
}

fn failed(out: &[Delivery], name: &str) -> Option<ItemFailure> {
    to(out, name).into_iter().find_map(|e| match e {
        Event::ItemFailed { reason, .. } => Some(*reason),
        _ => None,
    })
}

#[test]
fn get_drop_and_the_lists() {
    // §13.2; this tbaMUD appends to a room, so last.<word> is the newest (handler.c:772-793)
    let mut t = T::new(12);
    t.enter("Ana", Class::Warrior, Some(FIELD));
    t.enter("Bo", Class::Warrior, Some(FIELD));
    assert_eq!(failed(&t.cmd("Ana", "get"), "Ana"), Some(ItemFailure::What));
    assert_eq!(failed(&t.cmd("Ana", "get sword"), "Ana"), Some(ItemFailure::NotHere));
    assert_eq!(failed(&t.cmd("Ana", "get all"), "Ana"), Some(ItemFailure::Nothing));
    t.load("Ana", WAYBREAD, false);
    t.load("Ana", TORCH, false);
    let out = t.cmd("Ana", "get bread");
    assert!(matches!(&to(&out, "Ana")[..], [Event::Got { text, .. }] if text == "a waybread"));
    assert!(matches!(&to(&out, "Bo")[..], [Event::OccupantItem { action: ItemAction::Get, who, .. }] if who == "Ana"));
    let out = t.cmd("Ana", "i");
    assert!(matches!(&to(&out, "Ana")[..], [Event::Inventory { items }] if items.len() == 1), "`i` is inventory");
    t.cmd("Ana", "drop bread");
    t.cmd("Ana", "get all");
    assert_eq!(t.saved_objects("Ana").len(), 2);
    // Two drops: last.<word> takes the one dropped last.
    t.load("Ana", WAYBREAD, true);
    t.cmd("Ana", "drop all.bread");
    let out = t.cmd("Bo", "get last.bread");
    assert!(matches!(&to(&out, "Bo")[..], [Event::Got { .. }]));
    assert_eq!(failed(&t.cmd("Ana", "drop sword"), "Ana"), Some(ItemFailure::NotCarried));
    // Unknown tbaMUD commands are named as not here yet; unknown words are Huh!?!
    let out = t.cmd("Ana", "q");
    assert!(matches!(to(&out, "Ana")[..], [Event::Refused { reason: Refusal::NotYet }]), "q is quaff in tbaMUD's table");
    let out = t.cmd("Ana", "xyzzy");
    assert!(matches!(to(&out, "Ana")[..], [Event::Refused { reason: Refusal::UnknownCommand }]));
}

#[test]
fn wear_wield_hold_remove_and_the_zap() {
    // §13.3
    let mut t = T::new(12);
    t.enter("War", Class::Warrior, None);
    t.enter("Cle", Class::Cleric, None);
    t.load("War", PLATE, true);
    let out = t.cmd("War", "wear plate");
    assert!(matches!(&to(&out, "War")[..], [Event::Used { action: ItemAction::Wear, slot: Some(s), .. }] if s == "body"));
    assert!(matches!(&to(&out, "Cle")[..], [Event::OccupantItem { action: ItemAction::Wear, .. }]));
    t.load("War", PLATE, true);
    let out = t.cmd("War", "wear plate");
    assert!(matches!(&to(&out, "War")[..], [Event::ItemFailed { reason: ItemFailure::AlreadyWearing, slot: Some(s), .. }] if s == "body"));
    t.load("War", DAGGER, true);
    t.cmd("War", "wield dagger");
    t.load("War", TORCH, true);
    let out = t.cmd("War", "hold torch");
    assert!(matches!(&to(&out, "War")[..], [Event::Used { action: ItemAction::Hold, slot: Some(s), .. }] if s == "light"));
    let worn: Vec<_> = t.saved_objects("War").into_iter().filter_map(|(_, w)| w).collect();
    assert!(worn.contains(&"body".into()) && worn.contains(&"wield".into()) && worn.contains(&"light".into()), "{worn:?}");
    t.cmd("War", "remove torch");
    assert!(!t.saved_objects("War").iter().any(|(p, w)| p == TORCH && w.is_some()));
    // A cleric holding an anti-cleric dagger is zapped: the message, then back to the pack.
    t.load("Cle", DAGGER, true);
    let out = t.cmd("Cle", "wield dagger");
    assert!(to(&out, "Cle").iter().any(|e| matches!(e, Event::Zapped { .. })));
    assert!(t.saved_objects("Cle").iter().all(|(_, w)| w.is_none()));
    assert_eq!(failed(&t.cmd("War", "wield bag"), "War"), Some(ItemFailure::NotCarried));
    t.load("War", BAG, true);
    assert_eq!(failed(&t.cmd("War", "wield bag"), "War"), Some(ItemFailure::CantWield));
}

#[test]
fn containers() {
    // §13.2 put and get from
    let mut t = T::new(12);
    t.enter("Ana", Class::Warrior, None);
    t.load("Ana", BAG, true);
    t.load("Ana", WAYBREAD, true);
    let out = t.cmd("Ana", "put bread bag");
    assert!(matches!(&to(&out, "Ana")[..], [Event::Used { action: ItemAction::Put, into: Some(c), .. }] if c == "a bag"));
    assert_eq!(failed(&t.cmd("Ana", "put bag bag"), "Ana"), Some(ItemFailure::IntoItself));
    let out = t.cmd("Ana", "get bread bag");
    assert!(matches!(&to(&out, "Ana")[..], [Event::Got { from: Some(c), .. }] if c == "a bag"));
    assert_eq!(failed(&t.cmd("Ana", "get all bag"), "Ana"), Some(ItemFailure::Empty));
    assert_eq!(failed(&t.cmd("Ana", "put bread bread"), "Ana"), Some(ItemFailure::NotContainer));
}

#[test]
fn eating_and_drinking() {
    // §6.2, §6.3
    let mut t = T::new(12);
    t.enter("Ana", Class::Warrior, None);
    t.load("Ana", WAYBREAD, true);
    assert_eq!(failed(&t.cmd("Ana", "eat bread"), "Ana"), Some(ItemFailure::TooFull), "full 24 > 20");
    t.sim.submit(Input::SetPoints { name: "Ana".into(), hp: None, mana: None, mv: None, conditions: Some(Conditions { drunk: 0, full: 2, thirst: 2 }), gold: None });
    let out = t.cmd("Ana", "eat bread");
    assert!(to(&out, "Ana").iter().any(|e| matches!(e, Event::Used { action: ItemAction::Eat, .. })), "{:?}", to(&out, "Ana"));
    assert!(to(&out, "Ana").iter().any(|e| matches!(e, Event::Condition { full: Some(true), .. })), "2 + 24 > 20: full");
    assert_eq!(t.sim.save("Ana").unwrap().conditions.full, 24);
    assert!(t.saved_objects("Ana").is_empty(), "eaten");

    assert_eq!(failed(&{ t.load("Ana", BOTTLE, true); t.cmd("Ana", "drink bottle") }, "Ana"), Some(ItemFailure::StomachFull), "full > 20 and thirsty");
    // Beer: drunk 3, so amount = (25 - thirst) / 3; the bottle holds 8.
    t.sim.submit(Input::SetPoints { name: "Ana".into(), hp: None, mana: None, mv: None, conditions: Some(Conditions { drunk: 0, full: 2, thirst: 2 }), gold: None });
    let out = t.cmd("Ana", "drink bottle");
    assert!(to(&out, "Ana").iter().any(|e| matches!(e, Event::Used { action: ItemAction::Drink, liquid: Some(l), .. } if l == "beer")));
    let c = t.sim.save("Ana").unwrap().conditions;
    let amount = 8; // (25 - 2) / 3 = 7, the bottle has 8
    let amount = (amount as i32).min((25 - 2) / 3);
    assert_eq!(c.drunk, 3 * amount / 4);
    assert_eq!(c.thirst, 2 + 5 * amount / 4);
    let left = t.sim.save("Ana").unwrap().objects.iter().find(|o| o.proto == BOTTLE).unwrap().values.contains;
    assert_eq!(left, Some(8 - amount as i64));
    t.cmd("Ana", "drop all.bottle");
    assert_eq!(failed(&t.cmd("Ana", "drink bottle"), "Ana"), Some(ItemFailure::MustHold));
}

#[test]
fn a_light_burns_down_and_goes_out() {
    // §3.1: one hour per tick in the light slot; at 1 it flickers, at 0 it dies; -1 never does.
    // In the dark one cannot see one's own pack (utils.h LIGHT_OK), so the torch is taken in hand at 20:00.
    let mut t = T::new(20);
    t.enter("Ana", Class::Warrior, Some(FIELD));
    t.enter("Bo", Class::Warrior, Some(FIELD));
    t.load("Ana", TORCH, true);
    t.cmd("Ana", "hold torch");
    let mut all = vec![];
    for _ in 0..2 * PULSES_PER_TICK {
        all.extend(t.sim.step());
    }
    let out = t.cmd("Bo", "look");
    assert!(matches!(to(&out, "Bo")[..], [Event::Room(_)]), "22:00, the field lit by the torch");
    for _ in 0..24 * PULSES_PER_TICK {
        all.extend(t.sim.step());
    }
    assert!(to(&all, "Ana").iter().any(|e| matches!(e, Event::LightFlicker { .. })));
    assert!(to(&all, "Ana").iter().any(|e| matches!(e, Event::LightOut { .. })));
    assert!(to(&all, "Bo").iter().any(|e| matches!(e, Event::LightOut { .. })));
    let out = t.cmd("Bo", "look");
    assert!(matches!(to(&out, "Bo")[..], [Event::RoomDark { .. }]), "22:00 next day, out: dark");

    let mut t = T::new(20);
    t.enter("Ana", Class::Warrior, Some(FIELD));
    t.enter("Bo", Class::Warrior, Some(FIELD));
    t.load("Ana", CANDLE, true);
    t.cmd("Ana", "hold candle");
    for _ in 0..50 * PULSES_PER_TICK {
        t.sim.step();
    }
    let out = t.cmd("Bo", "look");
    assert!(matches!(to(&out, "Bo")[..], [Event::Room(_)]), "a candle of -1 hours never goes out");
}

#[test]
fn objects_come_back_with_the_save() {
    let mut t = T::new(12);
    t.enter("Ana", Class::Warrior, None);
    t.load("Ana", BAG, true);
    t.load("Ana", WAYBREAD, true);
    t.cmd("Ana", "put bread bag");
    t.load("Ana", PLATE, true);
    t.cmd("Ana", "wear plate");
    t.cmd("Ana", "quit");
    let save = t.sim.take_departures().remove(0).save;
    t.sim.submit(Input::Enter { name: "Ana".into(), save: Some(Box::new(save.clone())), new: NewChar::default() });
    t.sim.step();
    assert_eq!(t.sim.save("Ana").unwrap().objects, save.objects);
    let bag = save.objects.iter().find(|o| o.proto == BAG).unwrap();
    assert_eq!(bag.contents[0].proto, WAYBREAD);
}
