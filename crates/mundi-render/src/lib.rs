//! Rendering: a recipient's already filtered event to a sentence in that recipient's language and
//! point of view (actor, target, others), with Korean particles, English pronouns and character widths.
//!
//! The only place in the engine where sentences are made. It sees events only through `mundi-protocol`
//! and does not depend on the simulation, so it cannot reveal what perception filtered out.
//!
//! The point of view is already in the event: the recipient is `"self"` where they are the actor
//! (`comm.say {from: self, direction: out}` is "You say, ..."), a name or "someone" elsewhere.
//! Templates are Fluent files loaded at start (tbaMUD's English is tbaMUD text and lives in
//! `third_party/tbamud/locales/en/`, D9). Lines come out with markup (D20); [`ansi`] and [`plain`]
//! turn them into what a terminal or an agent receives.

use std::path::Path;

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use mundi_protocol::{
    ArrivedHow, DayPhase, Direction, Event, InGameHow, Lang, LeftHow, LinkState, LoginFailure, LoginStage, MoveFailure,
    Occupant, Position, Refusal,
};

pub use mundi_content::markup::plain;

pub struct Renderer {
    en: FluentBundle<FluentResource>,
}

impl Renderer {
    /// Loads every `.ftl` file of `<dir>/en`.
    pub fn load(dir: &Path) -> Result<Renderer, String> {
        let mut en = FluentBundle::new_concurrent(vec!["en".parse().unwrap()]);
        en.set_use_isolating(false);
        let en_dir = dir.join("en");
        let mut files: Vec<_> = std::fs::read_dir(&en_dir)
            .map_err(|e| format!("{}: {e}", en_dir.display()))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "ftl"))
            .collect();
        files.sort();
        for f in files {
            let text = std::fs::read_to_string(&f).map_err(|e| format!("{}: {e}", f.display()))?;
            let res = FluentResource::try_new(text).map_err(|(_, errs)| format!("{}: {errs:?}", f.display()))?;
            en.add_resource(res).map_err(|errs| format!("{}: {errs:?}", f.display()))?;
        }
        Ok(Renderer { en })
    }

    /// The lines of one event for its recipient, with markup. Korean comes with S4; until then every
    /// language gets English.
    pub fn lines(&self, event: &Event, _lang: Lang) -> Vec<String> {
        let m = |id: &str| self.msg(id, &[]);
        let who = |id: &str, who: &str| self.msg(id, &[("who", cap(&escape(who)))]);
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
                let n = |v: &Option<i32>| v.map_or("-".to_string(), |v| v.to_string());
                vec![self.msg("prompt", &[("hp", n(hp)), ("mp", n(mp)), ("mv", n(mv))]) + " "]
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
                Refusal::InvalidTarget | Refusal::NoTarget | Refusal::NotHere => "refused-invalid-target",
                _ => "refused-unknown-command",
            })],
            Event::Room(v) => {
                let mut out = vec![format!("{{yellow}}{}{{/yellow}}", v.name)];
                out.extend(v.desc.lines().filter(|l| !l.is_empty()).map(str::to_string));
                let exits: String = v
                    .exits
                    .iter()
                    .map(|e| {
                        let d = &e.dir[..1];
                        if e.closed { format!("{{red}}({d}){{/red}} ") } else { format!("{d} ") }
                    })
                    .collect();
                let exits = if exits.is_empty() { m("exits-none") } else { exits };
                out.push(format!("{{cyan}}[ Exits: {exits}]{{/cyan}}"));
                out.extend(v.occupants.iter().map(|o| format!("{{yellow}}{}{{/yellow}}", self.occupant(o))));
                out
            }
            Event::RoomDark { blind, .. } => vec![m(if *blind { "room-blind" } else { "room-dark" })],
            Event::MoveFailed { reason, door, .. } => vec![match (reason, door) {
                (MoveFailure::Closed | MoveFailure::Locked, Some(d)) => self.msg("move-closed-door", &[("door", escape(d))]),
                (MoveFailure::Closed | MoveFailure::Locked, None) => m("move-closed"),
                (MoveFailure::Exhausted, _) => m("move-exhausted"),
                (MoveFailure::Forbidden, _) => m("move-forbidden"),
                _ => m("move-no-exit"),
            }],
            Event::ZoneAboveLevel {} => vec![m("zone-above-level")],
            Event::Arrived { who: w, how, .. } => vec![match how {
                Some(ArrivedHow::EnteredGame) => who("arrived-entered-game", w),
                None => who("arrived", w),
            }],
            Event::Left { who: w, dir, how, .. } => vec![match (how, dir) {
                (Some(LeftHow::LeftGame), _) => who("left-game", w),
                (None, Some(d)) => self.msg("left", &[("who", cap(&escape(w))), ("dir", d.clone())]),
                (None, None) => who("left-game", w),
            }],
            Event::Link { who: w, state, .. } => vec![match state {
                LinkState::Lost => who("link-lost", w),
                LinkState::Reconnected => who("link-reconnected", w),
            }],
            Event::WorldTime { phase } => vec![m(match phase {
                DayPhase::Sunrise => "time-sunrise",
                DayPhase::Day => "time-day",
                DayPhase::Sunset => "time-sunset",
                DayPhase::Night => "time-night",
            })],
            Event::Say { from, text, direction, .. } => vec![match direction {
                Direction::Out => self.msg("say-out", &[("text", escape(text))]),
                Direction::In => self.msg("say-in", &[("who", cap(&escape(from))), ("text", escape(text))]),
            }],
        }
    }

    /// An occupant's line (MECHANICS §3.4), with markup. A mob in its default position shows its own line.
    pub fn occupant(&self, o: &Occupant) -> String {
        if let Some(long) = &o.long {
            return long.clone();
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
        let mut name = cap(&escape(&o.name));
        if o.flags.iter().any(|f| f == "linkless") {
            name = format!("{name} {}", self.msg("flag-linkless", &[]));
        }
        self.msg(id, &[("who", name)])
    }

    /// Fills the screen-line fields the simulation leaves empty (D22): plain English.
    pub fn fill(&self, event: &mut Event) {
        if let Event::Room(v) = event {
            for o in &mut v.occupants {
                o.text = plain(&self.occupant(o));
            }
        }
    }

    fn msg(&self, id: &str, args: &[(&str, String)]) -> String {
        let Some(pattern) = self.en.get_message(id).and_then(|m| m.value()) else {
            return format!("[{id}]");
        };
        let mut fa = FluentArgs::new();
        for (k, v) in args {
            fa.set(*k, FluentValue::from(v.clone()));
        }
        let mut errors = Vec::new();
        self.en.format_pattern(pattern, Some(&fa), &mut errors).into_owned()
    }
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
}
