//! The same happening seen from each side, in tbaMUD's English.

use std::path::Path;

use mundi_content::load_zone;
use mundi_protocol::{Direction, Event, KeywordMode, KoFinal, Lang, LinkState, Occupant, Position, Refusal, RoomExit, RoomView, Sex, SELF};
use mundi_render::{plain, Renderer, Viewer};

fn renderer() -> Renderer {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/tbamud");
    let newbie = load_zone(&root.join("content/186")).unwrap();
    Renderer::load(&root.join("locales"), &[newbie]).unwrap()
}

fn line(r: &Renderer, e: &Event) -> String {
    see(r, e, Viewer::default())
}

fn see(r: &Renderer, e: &Event, v: Viewer) -> String {
    r.lines(e, v).iter().map(|l| plain(l)).collect::<Vec<_>>().join("\n")
}

const KO: Viewer = Viewer { lang: Lang::Ko, keywords: KeywordMode::Targets };

fn person(name: &str) -> Occupant {
    Occupant {
        id: Some(format!("pc:{}", name.to_lowercase())),
        text: String::new(),
        name: name.into(),
        long: None,
        position: Position::Standing,
        fighting: None,
        flags: vec![],
        hints: vec![],
        keywords: vec![],
    }
}

fn mob(id: &str, english_long: &str) -> Occupant {
    Occupant { id: Some(id.into()), name: String::new(), long: Some(english_long.into()), ..person("x") }
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
            keywords: vec![],
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

#[test]
fn pronouns_follow_the_account() {
    let r = renderer();
    r.register_player("Ana", Sex::Female, None);
    let lost = Event::Link { who: "Ana".into(), who_id: Some("pc:ana".into()), state: LinkState::Lost };
    assert_eq!(line(&r, &lost), "Ana has lost her link.");
    let unknown = Event::Link { who: "Bo".into(), who_id: Some("pc:bo".into()), state: LinkState::Lost };
    assert_eq!(line(&r, &unknown), "Bo has lost its link.");
}

#[test]
fn korean_lines_with_particles() {
    let r = renderer();
    let say = Event::Say { from: "Vallen".into(), from_id: Some("pc:vallen".into()), text: "안녕".into(), direction: Direction::In };
    assert_eq!(see(&r, &say, KO), "Vallen이 말한다, '안녕'");
    let say = Event::Say { from: "Ana".into(), from_id: Some("pc:ana".into()), text: "hi".into(), direction: Direction::In };
    assert_eq!(see(&r, &say, KO), "Ana가 말한다, 'hi'");
    // The override for a name the rule gets wrong (D23).
    r.register_player("Bob", Sex::Male, Some(KoFinal::Other));
    let left = Event::Left { who: "Bob".into(), who_id: Some("pc:bob".into()), dir: Some("north".into()), how: None };
    assert_eq!(see(&r, &left, KO), "Bob이 북쪽으로 떠났다.");
    let left = Event::Left { who: "Ana".into(), who_id: Some("pc:ana".into()), dir: Some("west".into()), how: None };
    assert_eq!(see(&r, &left, KO), "Ana가 서쪽으로 떠났다.");
    let unseen = Event::Say { from: "someone".into(), from_id: None, text: "누구?".into(), direction: Direction::In };
    assert_eq!(see(&r, &unseen, KO), "누군가가 말한다, '누구?'");
    assert_eq!(see(&r, &Event::Refused { reason: Refusal::UnknownCommand }, KO), "뭐라고?!");
}

#[test]
fn a_korean_room_with_keywords_where_they_can_be_typed() {
    let r = renderer();
    let room = Event::Room(RoomView {
        id: Some("tba:186:room:18600".into()),
        name: "The Entrance To The Newbie Zone".into(),
        desc: "English description".into(),
        exits: vec![
            RoomExit { dir: "north".into(), closed: false, to_id: None },
            RoomExit { dir: "west".into(), closed: true, to_id: None },
        ],
        objects: vec![],
        occupants: vec![
            mob("tba:186:mob:18601/1", "A large, ugly pit beast stands here, watching you."),
            person("Ana"),
        ],
        dark: false,
    });
    let text = see(&r, &room, KO);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "초보자 지역의 입구");
    assert!(lines[1].starts_with("아아... 초보자 지역의 입구!"), "{}", lines[1]);
    assert_eq!(lines[2], "[ 출구: 북(n) (서(w)) ]");
    assert_eq!(lines[3], "크고 추한 구덩이 짐승(beast)이 여기 서서 당신을 주시하고 있다.");
    assert_eq!(lines[4], "Ana가 여기 서 있다.");

    let off = see(&r, &room, Viewer { keywords: KeywordMode::Off, ..KO });
    assert!(off.contains("[ 출구: 북 (서) ]") && off.contains("크고 추한 구덩이 짐승이 여기"), "{off}");
    // English viewers see tbaMUD's screen.
    let en = line(&r, &room);
    assert!(en.contains("[ Exits: n (w) ]") && en.contains("A large, ugly pit beast stands here, watching you."), "{en}");
}

#[test]
fn always_puts_keywords_in_sentences_too() {
    let r = renderer();
    let arrived = Event::Arrived { who: "the pit beast".into(), who_id: Some("tba:186:mob:18601/1".into()), from_dir: None, how: None };
    assert_eq!(see(&r, &arrived, KO), "구덩이 짐승이 왔다.");
    assert_eq!(see(&r, &arrived, Viewer { keywords: KeywordMode::Always, ..KO }), "구덩이 짐승(beast)이 왔다.");
}

#[test]
fn korean_templates_are_complete_and_name_known_pairs() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/tbamud/locales");
    let ids = |lang: &str| -> Vec<String> {
        std::fs::read_to_string(dir.join(lang).join("messages.ftl"))
            .unwrap()
            .lines()
            .filter_map(|l| l.split_once(" = ").map(|(k, _)| k.trim().to_string()))
            .filter(|k| !k.starts_with('#'))
            .collect()
    };
    let ko = ids("ko");
    for id in ids("en") {
        assert!(ko.contains(&id), "ko/messages.ftl has no {id}");
    }
    let text = std::fs::read_to_string(dir.join("ko/messages.ftl")).unwrap();
    for call in text.split("JOSA(").skip(1) {
        let pair = call.split('"').nth(1).unwrap();
        assert!(mundi_render::josa::PAIRS.contains(&pair), "unknown particle pair {pair}");
    }
}

#[test]
fn object_sentences_in_both_languages() {
    use mundi_protocol::{ItemAction, ItemFailure, Worn};
    let r = renderer();
    r.register_player("Ana", Sex::Female, None);
    let bread = || Some("tba:186:obj:18601/3".to_string());
    let got = Event::Got { text: "a waybread".into(), id: None, from: None, from_id: None };
    assert_eq!(line(&r, &got), "You get a waybread.");
    let wear = Event::OccupantItem {
        who: "Ana".into(),
        who_id: Some("pc:ana".into()),
        action: ItemAction::Wear,
        text: "a breast plate".into(),
        id: None,
        other: None,
        other_id: None,
        slot: Some("body".into()),
        liquid: None,
    };
    assert_eq!(line(&r, &wear), "Ana wears a breast plate on her body.");
    let miss = |kw: &str| Event::ItemFailed { action: ItemAction::Get, reason: ItemFailure::NotHere, text: None, id: None, keyword: Some(kw.into()), other: None, other_id: None, slot: None };
    assert_eq!(line(&r, &miss("sword")), "You don't see a sword here.");
    assert_eq!(line(&r, &miss("apple")), "You don't see an apple here.");
    assert_eq!(see(&r, &miss("sword"), KO), "여기엔 sword가 보이지 않는다.");
    let eq = Event::Equipment { slots: vec![Worn { slot: "body".into(), id: None, text: "a breast plate".into() }] };
    assert_eq!(line(&r, &eq), "You are using:\n<worn on body>       a breast plate");
    assert_eq!(line(&r, &Event::Inventory { items: vec![] }), "You are carrying:\n  Nothing.");
    // A Korean object name from the overlay, with its particle.
    let k = Event::Got { text: "a newbie dagger".into(), id: bread(), from: None, from_id: None };
    let ko = see(&r, &k, KO);
    assert_eq!(ko, "반짝이는 초보자의 검을 집었다.");
}

#[test]
fn blows_from_each_side() {
    use mundi_protocol::{HitKind, HitOutcome};
    let r = renderer();
    r.register_player("Ana", Sex::Female, None);
    let blow = |attacker: &str, victim: &str, damage: i32, severity: u8, outcome| Event::Hit {
        attacker: attacker.into(),
        attacker_id: (attacker == "Ana").then(|| "pc:ana".into()),
        victim: victim.into(),
        victim_id: None,
        verb: "slash".into(),
        severity,
        kind: HitKind::Weapon,
        damage,
        attack: 303,
        outcome,
        variant: Some(1),
        weapon: None,
        weapon_id: None,
    };
    assert_eq!(line(&r, &blow("self", "the beggar", 8, 4, HitOutcome::Hit)), "You slash the beggar hard.");
    assert_eq!(line(&r, &blow("Ana", "self", 8, 4, HitOutcome::Hit)), "Ana slashes you hard.");
    assert_eq!(line(&r, &blow("Ana", "the beggar", 1, 1, HitOutcome::Hit)), "Ana tickles the beggar as she slashes it.");
    assert_eq!(line(&r, &blow("Ana", "the beggar", 30, 8, HitOutcome::Hit)), "Ana OBLITERATES the beggar with her deadly slash!!");
    // A miss with a weapon comes from the message file when it has the attack (303: slash).
    let miss = line(&r, &blow("self", "the beggar", 0, 0, HitOutcome::Miss));
    assert!(!miss.is_empty() && miss.contains("beggar"), "{miss}");
    assert_eq!(see(&r, &blow("Ana", "the beggar", 8, 4, HitOutcome::Hit), KO), "Ana가 베기로 the beggar를 세게 맞혔다.");
}
