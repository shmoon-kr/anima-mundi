//! The same happening seen from each side, in tbaMUD's English.

use std::path::Path;

use mundi_protocol::{Direction, Event, Lang, Occupant, Position, Refusal, RoomExit, RoomView, SELF};
use mundi_render::{plain, Renderer};

fn renderer() -> Renderer {
    Renderer::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/tbamud/locales")).unwrap()
}

fn line(r: &Renderer, e: &Event) -> String {
    r.lines(e, Lang::En).iter().map(|l| plain(l)).collect::<Vec<_>>().join("\n")
}

#[test]
fn say_from_both_sides_and_unseen() {
    let r = renderer();
    let say = |from: &str, d| Event::Say { from: from.into(), from_id: None, text: "hi {red}".into(), direction: d };
    assert_eq!(line(&r, &say(SELF, Direction::Out)), "You say, 'hi {red}'");
    assert_eq!(line(&r, &say("Ana", Direction::In)), "Ana says, 'hi {red}'");
    assert_eq!(line(&r, &say("someone", Direction::In)), "Someone says, 'hi {red}'");
}

#[test]
fn movement_and_refusals() {
    let r = renderer();
    let left = Event::Left { who: "Ana".into(), who_id: None, dir: Some("north".into()), how: None };
    assert_eq!(line(&r, &left), "Ana leaves north.");
    let arrived = Event::Arrived { who: "Ana".into(), who_id: None, from_dir: None, how: None };
    assert_eq!(line(&r, &arrived), "Ana has arrived.");
    assert_eq!(line(&r, &Event::Refused { reason: Refusal::UnknownCommand }), "Huh!?!");
    assert_eq!(line(&r, &Event::Refused { reason: Refusal::Sleeping }), "In your dreams, or what?");
}

#[test]
fn a_room_and_its_screen_lines() {
    let r = renderer();
    let mut room = Event::Room(RoomView {
        id: Some("tba:30:room:3001".into()),
        name: "The Temple Of Midgaard".into(),
        desc: "You are in the southern end of the temple hall.".into(),
        exits: vec![
            RoomExit { dir: "north".into(), closed: false, to_id: None },
            RoomExit { dir: "down".into(), closed: true, to_id: None },
        ],
        objects: vec![],
        occupants: vec![Occupant {
            id: None,
            text: String::new(),
            name: "Bo".into(),
            long: None,
            position: Position::Standing,
            fighting: None,
            flags: vec!["linkless".into()],
            hints: vec![],
        }],
        dark: false,
    });
    assert_eq!(
        line(&r, &room),
        "The Temple Of Midgaard\nYou are in the southern end of the temple hall.\n[ Exits: n (d) ]\nBo (linkless) is standing here."
    );
    r.fill(&mut room);
    let Event::Room(v) = room else { unreachable!() };
    assert_eq!(v.occupants[0].text, "Bo (linkless) is standing here.");
}
