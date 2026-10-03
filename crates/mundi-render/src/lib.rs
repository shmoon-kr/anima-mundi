//! Rendering: a recipient's already filtered event to a sentence in that recipient's language and
//! point of view (actor, target, others), with Korean particles, English pronouns and character widths.
//!
//! The only place in the engine where sentences are made. It sees events only through `mundi-protocol`
//! and does not depend on the simulation, so it cannot reveal what perception filtered out.
//!
//! The point of view is already in the event: the recipient is `"self"` where they are the actor
//! (`comm.say {from: self, direction: out}` is "You say, ..."), a name or "someone" elsewhere.
//! Templates are Fluent files loaded at start; tbaMUD's messages and their translations are tbaMUD
//! text and live in `third_party/tbamud/locales/<lang>/` (D9). Names, room texts and lines of mobs
//! and objects come from the content and its translation overlays, looked up by the event's IDs.
//! Lines come out with markup (D20); [`ansi`] and [`plain`] turn them into what a terminal or an
//! agent receives.

pub mod josa;

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use mundi_content::{load_locale, Locale, ZoneContent};
use mundi_protocol::{
    ArrivedHow, DayPhase, Direction, Event, InGameHow, KeywordMode, KoFinal, Lang, LeftHow, LinkState, LoginFailure,
    LoginStage, MoveFailure, Occupant, Position, PositionCommand, PositionRefusal, Refusal, RoomView, Sex, WakeFailure,
};

pub use mundi_content::markup::plain;

use josa::Final;

/// Who is looking: their language and where they want keywords (D18).
#[derive(Debug, Clone, Copy, Default)]
pub struct Viewer {
    pub lang: Lang,
    pub keywords: KeywordMode,
}

/// What the renderer knows of a being beyond the event: from the content (mobs) or from the
/// account (players).
#[derive(Debug, Clone, Default)]
struct Being {
    sex: Sex,
    keywords: Vec<String>,
}

type Overrides = Arc<RwLock<HashMap<String, Final>>>;

pub struct Renderer {
    en: FluentBundle<FluentResource>,
    ko: FluentBundle<FluentResource>,
    ko_text: Locale,
    /// By prototype ID (`tba:30:mob:3060`) or player ID (`pc:ana`).
    beings: RwLock<HashMap<String, Being>>,
    /// Particle endings set for names the rule gets wrong (D23), by the name as shown.
    finals: Overrides,
}

/// Where a name stands in a sentence (D18): `Target` is a place a command can name it from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Spot {
    Target,
    Prose,
}

impl Renderer {
    /// Loads `<locales>/en/*.ftl`, `<locales>/ko/*.ftl` and the Korean overlays, and learns the mobs
    /// of `zones` (sex and keywords).
    pub fn load(locales: &Path, zones: &[ZoneContent]) -> Result<Renderer, String> {
        let finals: Overrides = Arc::default();
        let mut ko = bundle("ko", &locales.join("ko"))?;
        let f = finals.clone();
        ko.add_function("JOSA", move |pos, _named| {
            let (Some(FluentValue::String(word)), Some(FluentValue::String(pair))) = (pos.first(), pos.get(1)) else {
                return FluentValue::Error;
            };
            let base = josa::base_of(word);
            let end = f.read().unwrap().get(&base).copied().unwrap_or_else(|| josa::final_of(word));
            match josa::particle(pair, end) {
                Some(p) => FluentValue::from(p),
                None => FluentValue::from(format!("[{pair}]")),
            }
        })
        .map_err(|e| format!("JOSA: {e:?}"))?;
        let ko_text = load_locale(&locales.join("ko")).map_err(|e| e.to_string())?;
        let mut beings = HashMap::new();
        for z in zones {
            for (id, m) in &z.mobs {
                let sex = match m.sex {
                    mundi_content::names::Sex::Male => Sex::Male,
                    mundi_content::names::Sex::Female => Sex::Female,
                    _ => Sex::Neutral,
                };
                beings.insert(id.clone(), Being { sex, keywords: m.keywords.clone() });
            }
        }
        Ok(Renderer { en: bundle("en", &locales.join("en"))?, ko, ko_text, beings: RwLock::new(beings), finals })
    }

    /// A player as the world will name them, with what their account says.
    pub fn register_player(&self, name: &str, sex: Sex, ko_final: Option<KoFinal>) {
        let id = format!("pc:{}", name.to_lowercase());
        self.beings.write().unwrap().insert(id, Being { sex, keywords: vec![] });
        let mut finals = self.finals.write().unwrap();
        match ko_final {
            Some(k) => finals.insert(name.to_string(), match k {
                KoFinal::None => Final::None,
                KoFinal::Rieul => Final::Rieul,
                KoFinal::Other => Final::Other,
            }),
            None => finals.remove(name),
        };
    }

    /// The lines of one event for its recipient, with markup. English lines start with a capital,
    /// as act() writes them.
    pub fn lines(&self, event: &Event, v: Viewer) -> Vec<String> {
        let lines = self.lines_of(event, v);
        if v.lang == Lang::En { lines.iter().map(|l| cap(l)).collect() } else { lines }
    }

    fn lines_of(&self, event: &Event, v: Viewer) -> Vec<String> {
        let m = |id: &str| self.msg(v, id, &[]);
        let one = |id: &str, args: &[(&str, String)]| vec![self.msg(v, id, args)];
        let who = |name: &str, id: &Option<String>| self.name(v, name, id.as_deref(), Spot::Prose);
        match event {
            Event::LoginPrompt { stage } => match stage {
                LoginStage::Name => vec![m("login-name")],
                LoginStage::Password => vec![m("login-password")],
                _ => vec![],
            },
            Event::LoginFailed { reason } => vec![m(match reason {
                LoginFailure::InvalidName => "login-invalid-name",
                LoginFailure::WrongPassword => "login-wrong-password",
            })],
            Event::InGame { how } => vec![m(match how {
                InGameHow::Entered => "in-game-entered",
                InGameHow::Reconnected | InGameHow::TookOver => "in-game-reconnected",
            })],
            Event::Closed { detail, .. } => match detail.as_deref() {
                Some("quit") => vec![m("closed-quit")],
                _ => vec![],
            },
            Event::Prompt { hp, mp, mv } => {
                let n = |x: &Option<i32>| x.map_or("-".to_string(), |x| x.to_string());
                vec![self.msg(v, "prompt", &[("hp", n(hp)), ("mp", n(mp)), ("mv", n(mv))]) + " "]
            }
            Event::Refused { reason } => vec![m(match reason {
                Refusal::Dead => "refused-dead",
                Refusal::Incapacitated => "refused-incapacitated",
                Refusal::Stunned => "refused-stunned",
                Refusal::Sleeping => "refused-sleeping",
                Refusal::Resting => "refused-resting",
                Refusal::Sitting => "refused-sitting",
                Refusal::Fighting => "refused-fighting",
                Refusal::NothingToSay => "refused-nothing-to-say",
                Refusal::QuitInFull => "refused-quit-in-full",
                Refusal::NotHere => "refused-not-here",
                Refusal::InvalidTarget | Refusal::NoTarget => "refused-invalid-target",
                _ => "refused-unknown-command",
            })],
            Event::Room(room) => self.room(v, room),
            Event::RoomDark { blind, glowing_eyes, .. } => {
                let mut out = vec![m(if *blind { "room-blind" } else { "room-dark" })];
                out.extend((0..*glowing_eyes).map(|_| format!("{{red}}{}{{/red}}", m("glowing-eyes"))));
                out
            }
            Event::SelfPosition { position, from, awakened_by } => match awakened_by {
                Some(by) => one("pos-awakened-by", &[("who", self.name(v, by, None, Spot::Prose))]),
                None => vec![m(&position_id("pos", *position, *from))],
            },
            Event::OccupantPosition { who: w, who_id, position, from } => {
                let id = position_id("pos-room", *position, *from);
                one(&id, &[("who", who(w, who_id)), ("his", self.pronoun(who_id.as_deref(), "his").into())])
            }
            Event::PositionRefused { command, reason } => {
                let c = match command {
                    PositionCommand::Stand => "stand",
                    PositionCommand::Sit => "sit",
                    PositionCommand::Rest => "rest",
                    PositionCommand::Sleep => "sleep",
                    PositionCommand::Wake => "wake",
                };
                let r = match reason {
                    PositionRefusal::Already => "already",
                    PositionRefusal::Asleep => "asleep",
                    PositionRefusal::Fighting => "fighting",
                    PositionRefusal::Magic => "magic",
                };
                vec![m(&format!("refused-{c}-{r}"))]
            }
            Event::Woke { who: w, who_id } => one("woke", &self.pronouns(v, w, who_id.as_deref())),
            Event::WakeFailed { who: w, who_id, reason } => {
                let id = match reason {
                    WakeFailure::AlreadyAwake => "wake-failed-already-awake",
                    WakeFailure::Magic => "wake-failed-magic",
                    WakeFailure::BadShape => "wake-failed-bad-shape",
                };
                one(id, &self.pronouns(v, w, who_id.as_deref()))
            }
            Event::MoveFailed { reason, door, .. } => vec![match (reason, door) {
                (MoveFailure::Closed | MoveFailure::Locked, Some(d)) => self.msg(v, "move-closed-door", &[("door", escape(d))]),
                (MoveFailure::Closed | MoveFailure::Locked, None) => m("move-closed"),
                (MoveFailure::Exhausted, _) => m("move-exhausted"),
                (MoveFailure::Forbidden, _) => m("move-forbidden"),
                _ => m("move-no-exit"),
            }],
            Event::ZoneAboveLevel {} => vec![m("zone-above-level")],
            Event::Arrived { who: w, who_id, how, .. } => {
                let id = if *how == Some(ArrivedHow::EnteredGame) { "arrived-entered-game" } else { "arrived" };
                one(id, &[("who", who(w, who_id))])
            }
            Event::Left { who: w, who_id, dir, how } => match (how, dir) {
                (None, Some(d)) => one("left", &[("who", who(w, who_id)), ("dir", self.msg(v, &format!("dir-{d}"), &[]))]),
                (Some(LeftHow::LeftGame), _) | (None, None) => one("left-game", &[("who", who(w, who_id))]),
            },
            Event::Link { who: w, who_id, state } => match state {
                LinkState::Lost => one("link-lost", &[("who", who(w, who_id)), ("his", self.pronoun(who_id.as_deref(), "his").into())]),
                LinkState::Reconnected => one("link-reconnected", &[("who", who(w, who_id))]),
            },
            Event::WorldTime { phase } => vec![m(match phase {
                DayPhase::Sunrise => "time-sunrise",
                DayPhase::Day => "time-day",
                DayPhase::Sunset => "time-sunset",
                DayPhase::Night => "time-night",
            })],
            Event::Say { from, from_id, text, direction } => match direction {
                Direction::Out => one("say-out", &[("text", escape(text))]),
                Direction::In => one("say-in", &[("who", who(from, from_id)), ("text", escape(text))]),
            },
        }
    }

    fn room(&self, v: Viewer, room: &RoomView) -> Vec<String> {
        let tr = if v.lang == Lang::Ko { room.id.as_deref().and_then(|id| self.ko_text.get(id)) } else { None };
        let name = tr.and_then(|t| t.name.clone()).unwrap_or_else(|| room.name.clone());
        let mut out = vec![format!("{{yellow}}{name}{{/yellow}}")];
        let desc = tr.and_then(|t| t.description.as_ref()).map(|d| d.paragraphs().join("\n")).unwrap_or_else(|| room.desc.clone());
        out.extend(desc.lines().filter(|l| !l.is_empty()).map(str::to_string));
        let exits: String = room
            .exits
            .iter()
            .map(|e| {
                // English: the letter you type. Korean: the word, with that letter where keywords show (D18).
                let typed = self.msg(Viewer { lang: Lang::En, ..v }, &format!("exit-{}", e.dir), &[]);
                let shown = if v.lang == Lang::En {
                    typed
                } else {
                    let word = self.msg(v, &format!("exit-{}", e.dir), &[]);
                    if v.keywords == KeywordMode::Off { word } else { format!("{word}({typed})") }
                };
                if e.closed { format!("{{red}}({shown}){{/red}} ") } else { format!("{shown} ") }
            })
            .collect();
        let exits = if exits.is_empty() { self.msg(v, "exits-none", &[]) } else { exits };
        out.push(format!("{{cyan}}[ {}: {exits}]{{/cyan}}", self.msg(v, "exits-label", &[])));
        let keywords = unique_keywords(
            room.occupants.iter().map(|o| prefer(&self.keywords_of(o), if o.name.is_empty() { o.long.as_deref().unwrap_or("") } else { &o.name })).collect(),
        );
        for (o, kw) in room.occupants.iter().zip(keywords) {
            out.push(format!("{{yellow}}{}{{/yellow}}", self.occupant_line(v, o, kw.as_deref())));
        }
        out
    }

    /// An occupant's line (MECHANICS §3.4) in English, plain: the screen line of D22.
    pub fn occupant(&self, o: &Occupant) -> String {
        plain(&self.occupant_line(Viewer::default(), o, None))
    }

    fn occupant_line(&self, v: Viewer, o: &Occupant, keyword: Option<&str>) -> String {
        let show_kw = v.lang == Lang::Ko && v.keywords != KeywordMode::Off;
        if let Some(long) = &o.long {
            let tr = if v.lang == Lang::Ko { o.id.as_deref().and_then(|id| self.ko_text.get(proto(id))) } else { None };
            let long = tr.and_then(|t| t.long.clone()).unwrap_or_else(|| long.clone());
            return match (show_kw, keyword, tr.and_then(|t| t.short.as_deref())) {
                (true, Some(kw), Some(short)) if long.contains(short) => long.replacen(short, &format!("{short}({kw})"), 1),
                (true, Some(kw), _) => format!("{long} ({kw})"),
                _ => long,
            };
        }
        let id = match o.position {
            Position::Standing => "occupant-standing",
            Position::Sitting => "occupant-sitting",
            Position::Resting => "occupant-resting",
            Position::Sleeping => "occupant-sleeping",
            Position::Fighting => "occupant-fighting",
            Position::Stunned => "occupant-stunned",
            Position::Incapacitated => "occupant-incapacitated",
            Position::MortallyWounded => "occupant-mortally-wounded",
            Position::Dead => "occupant-dead",
        };
        // Flags go between the name and the position, as tbaMUD: "Ana (linkless) is standing here."
        let mut name = self.name(v, &o.name, o.id.as_deref(), Spot::Target);
        if show_kw {
            if let Some(kw) = keyword {
                name = format!("{name}({kw})");
            }
        }
        for flag in ["invisible", "hidden", "linkless"] {
            if o.flags.iter().any(|f| f == flag) {
                name = format!("{name} {}", self.msg(v, &format!("flag-{flag}"), &[]));
            }
        }
        self.msg(v, id, &[("who", name)])
    }

    /// Fills the screen-line fields the simulation leaves empty (D22): plain English.
    pub fn fill(&self, event: &mut Event) {
        if let Event::Room(room) = event {
            for o in &mut room.occupants {
                o.text = self.occupant(o);
            }
        }
    }

    /// A being's name for this viewer: Korean from the overlay when there is one, "누군가" for someone
    /// unseen, the keyword beside it where D18 puts it.
    fn name(&self, v: Viewer, english: &str, id: Option<&str>, spot: Spot) -> String {
        if v.lang == Lang::En {
            return escape(english);
        }
        let Some(id) = id else {
            return if english == "someone" { "누군가".into() } else { escape(english) };
        };
        let short = self.ko_text.get(proto(id)).and_then(|t| t.short.clone());
        let name = short.clone().unwrap_or_else(|| escape(english));
        let wanted = v.keywords == KeywordMode::Always || (v.keywords == KeywordMode::Targets && spot == Spot::Target);
        let keyword = self.beings.read().unwrap().get(proto(id)).and_then(|b| prefer(&b.keywords, english).into_iter().next());
        match (wanted && spot == Spot::Prose, short, keyword) {
            (true, Some(_), Some(kw)) => format!("{name}({kw})"),
            _ => name,
        }
    }

    fn keywords_of(&self, o: &Occupant) -> Vec<String> {
        if !o.keywords.is_empty() {
            return o.keywords.clone();
        }
        o.id.as_deref().and_then(|id| self.beings.read().unwrap().get(proto(id)).map(|b| b.keywords.clone())).unwrap_or_default()
    }

    /// An English pronoun ("he", "him" or "his") for a being, by its sex; "it" when unknown.
    fn pronoun(&self, id: Option<&str>, case: &str) -> &'static str {
        let sex = id.and_then(|id| self.beings.read().unwrap().get(proto(id)).map(|b| b.sex)).unwrap_or_default();
        match (sex, case) {
            (Sex::Male, "he") => "he",
            (Sex::Male, "him") => "him",
            (Sex::Male, _) => "his",
            (Sex::Female, "he") => "she",
            (Sex::Female, _) => "her",
            (Sex::Neutral, "his") => "its",
            (Sex::Neutral, _) => "it",
        }
    }

    /// The arguments of a sentence about someone: their name and pronouns.
    fn pronouns(&self, v: Viewer, name: &str, id: Option<&str>) -> Vec<(&'static str, String)> {
        vec![
            ("who", self.name(v, name, id, Spot::Prose)),
            ("he", self.pronoun(id, "he").into()),
            ("him", self.pronoun(id, "him").into()),
            ("his", self.pronoun(id, "his").into()),
        ]
    }

    fn msg(&self, v: Viewer, id: &str, args: &[(&str, String)]) -> String {
        let bundle = if v.lang == Lang::Ko { &self.ko } else { &self.en };
        let pattern = bundle.get_message(id).and_then(|m| m.value()).or_else(|| self.en.get_message(id).and_then(|m| m.value()));
        let Some(pattern) = pattern else { return format!("[{id}]") };
        let mut fa = FluentArgs::new();
        for (k, val) in args {
            fa.set(*k, FluentValue::from(val.clone()));
        }
        let mut errors = Vec::new();
        bundle.format_pattern(pattern, Some(&fa), &mut errors).into_owned()
    }
}

fn bundle(lang: &str, dir: &Path) -> Result<FluentBundle<FluentResource>, String> {
    let mut b = FluentBundle::new_concurrent(vec![lang.parse().unwrap()]);
    b.set_use_isolating(false);
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "ftl"))
        .collect();
    files.sort();
    for f in files {
        let text = std::fs::read_to_string(&f).map_err(|e| format!("{}: {e}", f.display()))?;
        let res = FluentResource::try_new(text).map_err(|(_, errs)| format!("{}: {errs:?}", f.display()))?;
        b.add_resource(res).map_err(|errs| format!("{}: {errs:?}", f.display()))?;
    }
    Ok(b)
}

/// `pos-standing-sitting`: the template of a position change. Falling asleep reads the same from any position.
fn position_id(prefix: &str, to: Position, from: Position) -> String {
    let n = |p: Position| match p {
        Position::Standing => "standing",
        Position::Sitting => "sitting",
        Position::Resting => "resting",
        Position::Sleeping => "sleeping",
        Position::Fighting => "fighting",
        _ => "down",
    };
    if to == Position::Sleeping { format!("{prefix}-sleeping") } else { format!("{prefix}-{}-{}", n(to), n(from)) }
}

/// Keywords that are words of the English name first: "the pit beast" is named `beast` before
/// `pitbeast` (D18 shows what people would type).
fn prefer(keywords: &[String], english: &str) -> Vec<String> {
    let words: Vec<String> = english.split(|c: char| !c.is_alphanumeric()).map(str::to_lowercase).collect();
    let (mut named, rest): (Vec<String>, Vec<String>) = keywords.iter().cloned().partition(|k| words.contains(&k.to_lowercase()));
    named.extend(rest);
    named
}

/// A mob's or object's instance ID is its prototype's ID with `/<n>`; translations are by prototype.
fn proto(id: &str) -> &str {
    id.split('/').next().unwrap_or(id)
}

/// For each entity in a list, a word that names it alone in that list (D18): its first keyword no
/// other entity shares, else `n.word` with tbaMUD's counting (the n-th in the list with that word).
/// None for entities without keywords (players: their name is the word).
fn unique_keywords(lists: Vec<Vec<String>>) -> Vec<Option<String>> {
    lists
        .iter()
        .enumerate()
        .map(|(i, kws)| {
            let first = kws.first()?;
            let alone = kws.iter().find(|k| lists.iter().enumerate().all(|(j, other)| j == i || !other.contains(k)));
            Some(match alone {
                Some(k) => k.clone(),
                None => {
                    let n = lists[..=i].iter().filter(|o| o.contains(first)).count();
                    if n == 1 { first.clone() } else { format!("{n}.{first}") }
                }
            })
        })
        .collect()
}

/// act() capitalises the first letter of a line (comm.c).
fn cap(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Text that came from a player is shown as it is: its braces are not markup.
pub fn escape(s: &str) -> String {
    s.replace('{', "{{").replace('}', "}}")
}

/// Columns a text takes on a terminal: Hangul, CJK and full-width forms take two.
pub fn width(text: &str) -> usize {
    plain(text)
        .chars()
        .map(|c| match c as u32 {
            0x1100..=0x115F | 0x2E80..=0x303E | 0x3041..=0x33FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xA960..=0xA97F
            | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF | 0xFE30..=0xFE4F | 0xFF00..=0xFF60 | 0xFFE0..=0xFFE6 => 2,
            _ => 1,
        })
        .sum()
}

/// `text` padded with spaces to `columns` (left-aligned), for tables in any language.
pub fn pad(text: &str, columns: usize) -> String {
    let w = width(text);
    if w >= columns { text.to_string() } else { format!("{text}{}", " ".repeat(columns - w)) }
}

/// Markup to ANSI for a terminal. Closing a tag resets and reapplies the tags still open.
pub fn ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    let mut open: Vec<String> = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, chars.peek()) {
            ('{', Some('{')) | ('}', Some('}')) => {
                chars.next();
                out.push(c);
            }
            ('{', _) => {
                let tag: String = chars.by_ref().take_while(|d| *d != '}').collect();
                if let Some(name) = tag.strip_prefix('/') {
                    if let Some(i) = open.iter().rposition(|t| t == name) {
                        open.remove(i);
                    }
                    out.push_str("\x1b[0m");
                    for t in &open {
                        out.push_str(&sgr(t));
                    }
                } else {
                    out.push_str(&sgr(&tag));
                    open.push(tag);
                }
            }
            _ => out.push(c),
        }
    }
    if !open.is_empty() {
        out.push_str("\x1b[0m");
    }
    out
}

fn sgr(tag: &str) -> String {
    const BASE: &[(&str, &str)] = &[
        ("black", "30"), ("red", "31"), ("green", "32"), ("yellow", "33"), ("blue", "34"), ("magenta", "35"),
        ("cyan", "36"), ("white", "37"), ("grey", "90"), ("orange", "38;5;208"), ("pink", "38;5;218"), ("azure", "38;5;39"),
    ];
    let code = match tag {
        "bold" => "1".to_string(),
        "underline" => "4".to_string(),
        "blink" => "5".to_string(),
        "reverse" => "7".to_string(),
        t if t.starts_with('x') && t[1..].parse::<u8>().is_ok() => format!("38;5;{}", &t[1..]),
        t => {
            let (bright, name) = match t.strip_prefix("bright_") {
                Some(n) => (true, n),
                None => (false, t),
            };
            match BASE.iter().find(|(n, _)| *n == name) {
                Some((_, c)) if bright && c.len() == 2 => format!("1;{c}"),
                Some((_, c)) => c.to_string(),
                None => return String::new(), // style:<name>: themes come later
            }
        }
    };
    format!("\x1b[{code}m")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi_reapplies_outer_tags() {
        assert_eq!(ansi("{red}a{bold}b{/bold}c{/red}"), "\x1b[31ma\x1b[1mb\x1b[0m\x1b[31mc\x1b[0m");
        assert_eq!(ansi("{{x}}"), "{x}");
        assert_eq!(plain("{yellow}Temple{/yellow} {{ok}}"), "Temple {ok}");
    }

    #[test]
    fn player_text_is_not_markup() {
        assert_eq!(plain(&escape("{red}hi")), "{red}hi");
    }

    #[test]
    fn keywords_unique_in_the_list() {
        let l = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        // The Newbie Zone: several "newbie"s, each with a word of its own, else counted.
        let got = unique_keywords(vec![l(&["newbie", "clueless"]), l(&["newbie", "zombie"]), l(&["newbie"]), l(&["newbie"]), vec![]]);
        assert_eq!(got, vec![Some("clueless".into()), Some("zombie".into()), Some("3.newbie".into()), Some("4.newbie".into()), None]);
        assert_eq!(unique_keywords(vec![l(&["beast", "pit"])]), vec![Some("beast".into())]);
    }

    #[test]
    fn widths() {
        assert_eq!(width("짐승(beast)"), 11);
        assert_eq!(width("{yellow}abc{/yellow}"), 3);
        assert_eq!(pad("칼", 4), "칼  ");
        assert_eq!(width(&pad("검은 칼", 10)), 10);
    }
}
