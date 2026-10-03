//! Fairness (S4): what an agent receives as events and what a person reads as text carry the same
//! information. In darkness, blindness, invisibility, hiding and sleep, every being named in a
//! delivery's event is named in its text, in English and in Korean, and the other way round.
//! The one allowed difference is tbaMUD's own English referring to the person you acted on by a
//! pronoun ("You wake him up."): the text then names fewer, never more.

use std::path::Path;

use mundi_content::names::Affect;
use mundi_content::{load_tables, load_zone};
use mundi_protocol::{Event, KeywordMode, Lang};
use mundi_render::{plain, Renderer, Viewer};
use mundi_sim::{Delivery, Input, Sim};

const NAMES: [&str; 3] = ["Ana", "Bo", "Cy"];

struct World {
    sim: Sim,
    render: Renderer,
    seen: Vec<Delivery>,
}

impl World {
    /// Midgaard at `hour`, the three in `room`.
    fn new(room: &str, hour: u32) -> World {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/tbamud");
        let zones = vec![load_zone(&root.join("content/30")).unwrap()];
        let tables = load_tables(&root.join("tables")).unwrap();
        let mut w = World { sim: Sim::new(&zones, &tables, 3, hour), render: Renderer::load(&root.join("locales"), &zones).unwrap(), seen: vec![] };
        for n in NAMES {
            w.input(Input::Enter { name: n.into(), room: Some(room.into()) });
        }
        w
    }

    fn input(&mut self, i: Input) -> Vec<Delivery> {
        self.sim.submit(i);
        let out = self.sim.step();
        self.seen.extend(out.clone());
        out
    }

    fn cmd(&mut self, who: &str, text: &str) -> Vec<Delivery> {
        self.input(Input::Command { name: who.into(), text: text.into() })
    }

    fn affect(&mut self, who: &str, affect: Affect) {
        self.input(Input::SetAffect { name: who.into(), affect, on: true });
    }
}

fn to<'a>(out: &'a [Delivery], who: &str) -> Vec<&'a Event> {
    out.iter().filter(|d| d.to == who).map(|d| &d.event).filter(|e| !matches!(e, Event::Prompt { .. })).collect()
}

/// The beings a delivery names: in its event (any string field equal to a name) and in its text.
fn named_in_event(e: &Event) -> Vec<&'static str> {
    let json = serde_json::to_string(e).unwrap();
    NAMES.into_iter().filter(|n| json.contains(&format!("\"{n}\""))).collect()
}

fn named_in_text(r: &Renderer, e: &Event, lang: Lang) -> Vec<&'static str> {
    let text = r.lines(e, Viewer { lang, keywords: KeywordMode::Targets }).iter().map(|l| plain(l)).collect::<Vec<_>>().join("\n");
    NAMES.into_iter().filter(|n| text.contains(n)).collect()
}

fn assert_fair(w: &World) {
    for d in &w.seen {
        for lang in [Lang::En, Lang::Ko] {
            let (event, text) = (named_in_event(&d.event), named_in_text(&w.render, &d.event, lang));
            assert!(text.iter().all(|n| event.contains(n)), "the text names more than the event: to {} ({lang:?}): {:?}", d.to, d.event);
            let by_pronoun = lang == Lang::En && matches!(d.event, Event::Woke { .. } | Event::WakeFailed { .. });
            if !by_pronoun {
                assert_eq!(event, text, "to {} ({lang:?}): {:?}", d.to, d.event);
            }
        }
    }
}

#[test]
fn the_dark_hides_names_but_not_eyes() {
    // A field of Midgaard at night: no light, outdoors.
    let mut w = World::new("tba:30:room:3065", 23);
    w.affect("Bo", Affect::Infravision);
    let out = w.cmd("Ana", "look");
    assert!(matches!(to(&out, "Ana")[..], [Event::RoomDark { glowing_eyes: 1, .. }]));
    let out = w.cmd("Bo", "look");
    assert!(matches!(to(&out, "Bo")[..], [Event::Room(v)] if v.occupants.len() == 2), "infravision sees in the dark");
    let out = w.cmd("Cy", "say who is here");
    assert!(matches!(&to(&out, "Ana")[..], [Event::Say { from, .. }] if from == "someone"));
    assert!(matches!(&to(&out, "Bo")[..], [Event::Say { from, .. }] if from == "Cy"));
    // A light makes the room lit for everyone.
    w.input(Input::SetLight { name: "Cy".into(), on: true });
    let out = w.cmd("Ana", "look");
    assert!(matches!(to(&out, "Ana")[..], [Event::Room(v)] if v.occupants.len() == 2));
    assert_fair(&w);
}

#[test]
fn blindness_invisibility_and_hiding() {
    let mut w = World::new("tba:30:room:3001", 12);
    w.affect("Ana", Affect::Blind);
    let out = w.cmd("Ana", "look");
    assert!(matches!(to(&out, "Ana")[..], [Event::RoomDark { blind: true, .. }]));
    let out = w.cmd("Bo", "say can you see me");
    assert!(matches!(&to(&out, "Ana")[..], [Event::Say { from, from_id: None, .. }] if from == "someone"));
    w.input(Input::SetAffect { name: "Ana".into(), affect: Affect::Blind, on: false });

    w.affect("Bo", Affect::Invisible);
    w.affect("Cy", Affect::DetectInvis);
    let out = w.cmd("Ana", "look");
    assert!(matches!(to(&out, "Ana")[..], [Event::Room(v)] if v.occupants.iter().all(|o| o.name != "Bo")));
    let out = w.cmd("Cy", "look");
    assert!(matches!(to(&out, "Cy")[..], [Event::Room(v)] if v.occupants.iter().any(|o| o.name == "Bo" && o.flags.contains(&"invisible".into()))));
    let out = w.cmd("Bo", "sit");
    assert!(matches!(&to(&out, "Ana")[..], [Event::OccupantPosition { who, .. }] if who == "someone"), "'$n sits down.' reaches those who cannot see");
    let out = w.cmd("Bo", "stand");
    assert!(to(&out, "Ana").is_empty(), "the other position messages do not");
    w.input(Input::SetAffect { name: "Bo".into(), affect: Affect::Invisible, on: false });

    w.affect("Cy", Affect::Hide);
    let out = w.cmd("Ana", "look");
    assert!(matches!(to(&out, "Ana")[..], [Event::Room(v)] if v.occupants.iter().all(|o| o.name != "Cy")));
    w.cmd("Cy", "look"); // any command ends hiding
    let out = w.cmd("Ana", "look");
    assert!(matches!(to(&out, "Ana")[..], [Event::Room(v)] if v.occupants.iter().any(|o| o.name == "Cy")));
    assert_fair(&w);
}

#[test]
fn sleepers_hear_nothing_until_woken() {
    let mut w = World::new("tba:30:room:3001", 12);
    let out = w.cmd("Ana", "sleep");
    assert!(matches!(to(&out, "Ana")[..], [Event::SelfPosition { .. }]));
    assert!(matches!(&to(&out, "Bo")[..], [Event::OccupantPosition { who, .. }] if who == "Ana"));
    let out = w.cmd("Bo", "say wake up");
    assert!(to(&out, "Ana").is_empty());
    let out = w.cmd("Ana", "look");
    assert!(matches!(to(&out, "Ana")[..], [Event::Refused { .. }]), "asleep: 'In your dreams, or what?'");
    let out = w.cmd("Bo", "wake ana");
    assert!(matches!(&to(&out, "Bo")[..], [Event::Woke { who, .. }] if who == "Ana"));
    assert!(matches!(&to(&out, "Ana")[..], [Event::SelfPosition { awakened_by: Some(by), .. }] if by == "Bo"));
    let out = w.cmd("Bo", "wake ana");
    assert!(matches!(&to(&out, "Bo")[..], [Event::WakeFailed { .. }]), "already awake");
    assert_fair(&w);

    let text = |e: &Event| w.render.lines(e, Viewer::default()).iter().map(|l| plain(l)).collect::<Vec<_>>();
    let woken = w.seen.iter().find(|d| matches!(d.event, Event::SelfPosition { awakened_by: Some(_), .. })).unwrap();
    assert_eq!(text(&woken.event), ["You are awakened by Bo."]);
}
