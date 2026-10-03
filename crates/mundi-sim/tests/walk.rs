//! The walking skeleton on the converted Midgaard (zone 30): entering, looking, moving, saying,
//! quitting, each seen from each viewpoint, and the same input log giving the same deliveries.

use std::path::Path;

use mundi_content::names::{DoorState, Sector};
use mundi_content::{load_tables, load_zone, Tables, ZoneContent};
use mundi_protocol::{ArrivedHow, Direction, Event, MoveFailure, Refusal};
use mundi_sim::{Delivery, Input, Sim};

const START_ROOM: &str = "tba:30:room:3001";

fn zones(nums: &[u32]) -> Vec<ZoneContent> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/tbamud/content");
    nums.iter().map(|n| load_zone(&root.join(n.to_string())).expect("zone loads")).collect()
}

fn tables() -> Tables {
    load_tables(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/tbamud/tables")).expect("tables load")
}

fn midgaard() -> Vec<ZoneContent> {
    zones(&[30])
}

fn enter(sim: &mut Sim, name: &str, room: Option<&str>) -> Vec<Delivery> {
    sim.submit(Input::Enter { name: name.into(), room: room.map(Into::into) });
    sim.step()
}

fn cmd(sim: &mut Sim, name: &str, text: &str) -> Vec<Delivery> {
    sim.submit(Input::Command { name: name.into(), text: text.into() });
    sim.step()
}

fn to<'a>(out: &'a [Delivery], name: &str) -> Vec<&'a Event> {
    out.iter().filter(|d| d.to == name).map(|d| &d.event).filter(|e| !matches!(e, Event::Prompt { .. })).collect()
}

#[test]
fn entering_and_moving_seen_from_each_side() {
    let zones = midgaard();
    let mut sim = Sim::new(&zones, &tables(), 1, 12);
    let out = enter(&mut sim, "Ana", None);
    let ana = to(&out, "Ana");
    assert!(matches!(ana[1], Event::Room(v) if v.id.as_deref() == Some(START_ROOM) && v.occupants.is_empty()));

    let out = enter(&mut sim, "Bo", None);
    assert!(matches!(to(&out, "Ana")[..], [Event::Arrived { who, how: Some(ArrivedHow::EnteredGame), .. }] if who == "Bo"));
    let Event::Room(view) = to(&out, "Bo")[1] else { panic!() };
    assert_eq!(view.occupants[0].name, "Ana");

    // Ana walks out of the temple; Bo sees her leave, Ana sees the next room.
    let exit = view.exits.first().expect("the temple has an exit").clone();
    let out = cmd(&mut sim, "Ana", &exit.dir[..1]);
    assert!(matches!(&to(&out, "Bo")[..], [Event::Left { who, dir: Some(d), .. }] if who == "Ana" && *d == exit.dir));
    assert!(matches!(&to(&out, "Ana")[..], [Event::Room(v)] if v.id == exit.to_id));
    assert_eq!(sim.room_of("ana"), exit.to_id.as_deref());

    // Prompts close each recipient's block.
    assert!(out.iter().any(|d| d.to == "Ana" && matches!(d.event, Event::Prompt { mv: Some(_), .. })));
}

#[test]
fn say_has_three_viewpoints_and_refusals() {
    let zones = midgaard();
    let mut sim = Sim::new(&zones, &tables(), 1, 12);
    enter(&mut sim, "Ana", None);
    enter(&mut sim, "Bo", None);
    let out = cmd(&mut sim, "Ana", "say hello there");
    assert!(matches!(&to(&out, "Ana")[..], [Event::Say { from, direction: Direction::Out, text, .. }] if from == "self" && text == "hello there"));
    assert!(matches!(&to(&out, "Bo")[..], [Event::Say { from, direction: Direction::In, .. }] if from == "Ana"));
    let out = cmd(&mut sim, "Bo", "'hi");
    assert!(matches!(&to(&out, "Ana")[..], [Event::Say { text, .. }] if text == "hi"));

    for (text, reason) in [("say", Refusal::NothingToSay), ("qui", Refusal::QuitInFull), ("xyzzy", Refusal::UnknownCommand)] {
        let out = cmd(&mut sim, "Ana", text);
        assert!(matches!(&to(&out, "Ana")[..], [Event::Refused { reason: r }] if *r == reason), "{text}");
    }
}

#[test]
fn quitting_leaves_the_game_and_reports_the_place() {
    let zones = midgaard();
    let mut sim = Sim::new(&zones, &tables(), 1, 12);
    enter(&mut sim, "Ana", None);
    enter(&mut sim, "Bo", None);
    let out = cmd(&mut sim, "Ana", "quit");
    assert!(matches!(&to(&out, "Bo")[..], [Event::Left { who, dir: None, how: Some(_), .. }] if who == "Ana"));
    assert!(matches!(&to(&out, "Ana")[..], [Event::Closed { .. }]));
    let gone = sim.take_departures();
    assert_eq!(gone[0].room, START_ROOM);
    assert_eq!(sim.room_of("Ana"), None);
    // Entering again at the saved place.
    let out = enter(&mut sim, "Ana", Some(&gone[0].room));
    assert!(matches!(to(&out, "Ana")[1], Event::Room(v) if v.id.as_deref() == Some(START_ROOM)));
}

#[test]
fn closed_doors_stop_movement() {
    // Midgaard's doors have no reset and stand open; Midgaard's southern half closes some.
    let zones = zones(&[30, 31]);
    let (room, dir, keyword) = zones[1]
        .rooms
        .iter()
        .find_map(|(id, r)| {
            r.exits.iter().find_map(|(d, e)| {
                let door = e.door.as_ref()?;
                (e.to.is_some() && door.reset.is_some_and(|s| s != DoorState::Open))
                    .then(|| (id.clone(), d.name(), door.keywords.first().cloned()))
            })
        })
        .expect("zone 31 has a closed door");
    let mut sim = Sim::new(&zones, &tables(), 1, 12);
    enter(&mut sim, "Ana", Some(&room));
    let out = cmd(&mut sim, "Ana", dir);
    assert!(matches!(&to(&out, "Ana")[..], [Event::MoveFailed { reason: MoveFailure::Closed, door, .. }] if *door == keyword));
    assert_eq!(sim.room_of("Ana"), Some(room.as_str()));
}

#[test]
fn outdoors_is_dark_at_night_and_hides_who_arrives() {
    let zones = midgaard();
    let room = zones[0]
        .rooms
        .iter()
        .find(|(_, r)| !matches!(r.sector, Sector::Inside | Sector::City) && r.flags.is_empty())
        .map(|(id, _)| id.clone())
        .expect("Midgaard has fields");
    let mut sim = Sim::new(&zones, &tables(), 1, 23);
    let out = enter(&mut sim, "Ana", Some(&room));
    assert!(matches!(to(&out, "Ana")[1], Event::RoomDark { .. }));
    let out = enter(&mut sim, "Bo", Some(&room));
    assert!(to(&out, "Ana").is_empty(), "arrivals in the dark are not seen");
    let out = cmd(&mut sim, "Bo", "say who is there");
    assert!(matches!(&to(&out, "Ana")[..], [Event::Say { from, from_id: None, .. }] if from == "someone"));
}

#[test]
fn one_command_per_character_per_pulse() {
    let zones = midgaard();
    let mut sim = Sim::new(&zones, &tables(), 1, 12);
    enter(&mut sim, "Ana", None);
    sim.submit(Input::Command { name: "Ana".into(), text: "say one".into() });
    sim.submit(Input::Command { name: "Ana".into(), text: "say two".into() });
    let first = sim.step();
    let second = sim.step();
    assert!(matches!(&to(&first, "Ana")[..], [Event::Say { text, .. }] if text == "one"));
    assert!(matches!(&to(&second, "Ana")[..], [Event::Say { text, .. }] if text == "two"));
}

#[test]
fn replaying_the_input_log_gives_the_same_bytes() {
    let zones = midgaard();
    let mut sim = Sim::new(&zones, &tables(), 42, 12);
    let mut live = Vec::new();
    live.extend(enter(&mut sim, "Ana", None));
    live.extend(enter(&mut sim, "Bo", None));
    for _ in 0..5 {
        live.extend(sim.step());
    }
    for (who, text) in [("Ana", "look"), ("Bo", "say hi"), ("Ana", "n"), ("Ana", "s"), ("Bo", "quit")] {
        live.extend(cmd(&mut sim, who, text));
    }
    sim.submit(Input::LinkLost { name: "Ana".into() });
    live.extend(sim.step());
    let log = sim.input_log().to_vec();

    let bytes = |d: &[Delivery]| d.iter().map(|d| serde_json::to_string(d).unwrap() + "\n").collect::<String>();
    let again = Sim::replay(&zones, &tables(), 42, 12, &log);
    assert_eq!(bytes(&again), bytes(&live));
    assert_eq!(bytes(&Sim::replay(&zones, &tables(), 42, 12, &log)), bytes(&again));
}

#[test]
fn positions_follow_the_table() {
    use mundi_protocol::{Position, PositionCommand, PositionRefusal};
    let zones = midgaard();
    let mut sim = Sim::new(&zones, &tables(), 1, 12);
    enter(&mut sim, "Ana", None);
    let steps = [
        ("stand", Err((PositionCommand::Stand, PositionRefusal::Already))),
        ("sit", Ok((Position::Sitting, Position::Standing))),
        ("sit", Err((PositionCommand::Sit, PositionRefusal::Already))),
        ("rest", Ok((Position::Resting, Position::Sitting))),
        ("sleep", Ok((Position::Sleeping, Position::Resting))),
        ("sleep", Err((PositionCommand::Sleep, PositionRefusal::Already))),
        ("wake", Ok((Position::Sitting, Position::Sleeping))),
        ("wake", Err((PositionCommand::Wake, PositionRefusal::Already))),
        ("stand", Ok((Position::Standing, Position::Sitting))),
    ];
    for (text, want) in steps {
        let out = cmd(&mut sim, "Ana", text);
        match (&to(&out, "Ana")[..], want) {
            ([Event::SelfPosition { position, from, .. }], Ok((p, f))) => assert_eq!((*position, *from), (p, f), "{text}"),
            ([Event::PositionRefused { command, reason }], Err((c, r))) => assert_eq!((*command, *reason), (c, r), "{text}"),
            (got, _) => panic!("{text}: {got:?}"),
        }
    }
    // Asleep, moving is refused by the command table (MECHANICS §4.2).
    cmd(&mut sim, "Ana", "sleep");
    let out = cmd(&mut sim, "Ana", "north");
    assert!(matches!(to(&out, "Ana")[..], [Event::Refused { reason: Refusal::Sleeping }]));
}
