//! Shops and guilds (MECHANICS §15, §12.4).

use std::path::Path;

use mundi_content::{load_tables, load_zone};
use mundi_protocol::{Direction, Event, Refusal};
use mundi_sim::{Class, Delivery, Input, NewChar, Sim};

const BAKERY: &str = "tba:30:room:3009";
const WEAPONS: &str = "tba:30:room:3011";
const MAGE_GUARD: &str = "tba:30:room:3017";
const MAGE_GUILD: &str = "tba:30:room:3019";
const DAGGER: &str = "tba:30:obj:3020";

struct T {
    sim: Sim,
    seen: Vec<Delivery>,
}

impl T {
    fn new() -> T {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/tbamud");
        let z = vec![load_zone(&root.join("content/30")).unwrap()];
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
    fn gold(&mut self, name: &str, gold: i64) {
        self.input(Input::SetPoints { name: name.into(), hp: None, mana: None, mv: None, conditions: None, gold: Some(gold) });
    }
    fn cmd(&mut self, name: &str, text: &str) -> Vec<Event> {
        let from = self.seen.len();
        self.input(Input::Command { name: name.into(), text: text.into() });
        self.input(Input::Command { name: name.into(), text: String::new() });
        self.seen[from..].iter().filter(|d| d.to == name).map(|d| d.event.clone()).collect()
    }
    fn tells(events: &[Event]) -> Vec<String> {
        events.iter().filter_map(|e| match e {
            Event::Tell { text, direction: Direction::In, .. } => Some(text.clone()),
            _ => None,
        }).collect()
    }
}

#[test]
fn buy_at_the_charisma_price_and_list() {
    // §15.2: bread costs 10, the baker's profit 1.4; price = 10 × 1.4 × (1 + (keeper cha - mine) / 70)
    let mut t = T::new();
    t.enter("War", Class::Warrior, BAKERY);
    t.gold("War", 100);
    let list = t.cmd("War", "list");
    let Some(Event::ShopList { items, .. }) = list.iter().find(|e| matches!(e, Event::ShopList { .. })) else { panic!("{list:?}") };
    assert!(items.len() >= 3 && items.iter().all(|i| i.count.is_none()), "the baker makes everything: Unlimited");
    let cha = t.sim.save("War").unwrap().abilities.cha;
    let price = (10.0 * 1.4 * (1.0 + (11 - cha) as f64 / 70.0)) as i64;
    let out = t.cmd("War", "buy bread");
    assert!(out.iter().any(|e| matches!(e, Event::ShopResult { action, who, .. } if action == "buy" && who == "self")));
    assert_eq!(t.sim.save("War").unwrap().gold, 100 - price, "cha {cha}");
    assert!(T::tells(&out).iter().any(|m| m == &format!("That'll be {price} coins.")), "{:?}", T::tells(&out));
    let out = t.cmd("War", "buy 3 bread");
    assert!(out.iter().any(|e| matches!(e, Event::ShopResult { text, .. } if text.ends_with("(x 3)"))));
    let out = t.cmd("War", "buy unicorn");
    assert!(T::tells(&out).iter().any(|m| m == "Haven't got that on storage - try list!"));
    t.gold("War", 0);
    let out = t.cmd("War", "buy bread");
    assert!(!T::tells(&out).is_empty(), "the you-cannot-afford line");
}

#[test]
fn sell_and_value_to_the_weaponsmith() {
    // §15.2-15.3: sell = cost × min(sell × (1 - c), buy × (1 + c)); the dagger costs 10? (content)
    let mut t = T::new();
    t.enter("War", Class::Warrior, WEAPONS);
    t.input(Input::Load { name: "War".into(), object: DAGGER.into(), carry: true });
    let out = t.cmd("War", "value dagger");
    let offer = T::tells(&out).into_iter().find(|m| m.starts_with("I'll give you")).expect("an offer");
    let gold_before = t.sim.save("War").unwrap().gold;
    t.cmd("War", "sell dagger");
    let got = t.sim.save("War").unwrap().gold - gold_before;
    assert_eq!(offer, format!("I'll give you {got} gold coins for that!"));
    assert!(t.sim.save("War").unwrap().objects.is_empty());
    let out = t.cmd("War", "sell dagger");
    assert!(!T::tells(&out).is_empty(), "nothing left: the you-don't-have-it line");
}

#[test]
fn shop_commands_elsewhere_and_a_protected_keeper() {
    let mut t = T::new();
    t.enter("War", Class::Warrior, "tba:30:room:3001");
    let out = t.cmd("War", "list");
    assert!(out.iter().any(|e| matches!(e, Event::Refused { reason: Refusal::NotHereShop })));
    t.enter("Bo", Class::Warrior, BAKERY);
    let out = t.cmd("Bo", "kill baker");
    assert!(T::tells(&out).iter().any(|m| m == "Get out of here before I call the guards!"));
}

#[test]
fn guild_guards_and_practice() {
    // §12.4; class.c guild_info: the mages' guard in 3017 stops all but mages going south
    let mut t = T::new();
    t.enter("War", Class::Warrior, MAGE_GUARD);
    t.enter("Mag", Class::MagicUser, MAGE_GUARD);
    let out = t.cmd("War", "south");
    assert!(out.iter().any(|e| matches!(e, Event::Blocked { who, .. } if who == "self")));
    assert_eq!(t.sim.room_of("War"), Some(MAGE_GUARD));
    t.cmd("Mag", "south");
    assert_ne!(t.sim.room_of("Mag"), Some(MAGE_GUARD), "a mage passes");

    let mut t = T::new();
    t.enter("Mag", Class::MagicUser, MAGE_GUILD);
    let out = t.cmd("Mag", "practice");
    let Some(Event::Skills { skills, practices, .. }) = out.iter().find(|e| matches!(e, Event::Skills { .. })) else { panic!("{out:?}") };
    assert!(skills.iter().any(|s| s.name == "magic missile" && s.percent == 0));
    let before = *practices;
    let out = t.cmd("Mag", "practice magic missile");
    assert!(out.iter().any(|e| matches!(e, Event::Practiced { result, .. } if result == "improved" || result == "learned")));
    let s = t.sim.save("Mag").unwrap();
    assert_eq!(s.practices, before - 1);
    assert!(s.skills["magic missile"] >= 25, "a mage gains at least 25");
    let out = t.cmd("Mag", "practice bash");
    assert!(out.iter().any(|e| matches!(e, Event::Practiced { reason: Some(r), .. } if r == "unknown_skill")));
}
