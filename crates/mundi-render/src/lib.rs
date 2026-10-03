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
    ArrivedHow, DayPhase, Direction, Event, HitOutcome, InGameHow, KeywordMode, KoFinal, Lang, LeftHow, LinkState, LoginFailure,
    LineRef, LoginStage, MoveFailure, Occupant, Position, PositionCommand, PositionRefusal, Refusal, RoomView, Sex, WakeFailure,
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

/// The particle of a pair (`이/가`) that follows `word` (D23), with the names' overrides.
fn particle_after(finals: &Overrides, word: &str, pair: &str) -> String {
    let end = finals.read().unwrap().get(&josa::base_of(word)).copied().unwrap_or_else(|| josa::final_of(word));
    josa::particle(pair, end).unwrap_or_else(|| format!("[{pair}]"))
}

/// A message-file set's line for an outcome and a reader's role.
fn pick<'a>(set: &'a mundi_content::tables::MessageSet, outcome: &HitOutcome, role: &str) -> &'a Option<String> {
    let lines = match outcome {
        HitOutcome::Miss => &set.miss,
        HitOutcome::Hit => &set.hit,
        HitOutcome::Die => &set.die,
        HitOutcome::God => &set.god,
    };
    match role {
        "attacker" => &lines.attacker,
        "victim" => &lines.victim,
        _ => &lines.room,
    }
}

/// Blows the reader deals in yellow, blows they take in red (fight.c's CCYEL, CCRED).
fn colour_for(role: &str, text: String) -> String {
    match role {
        "attacker" => format!("{{yellow}}{text}{{/yellow}}"),
        "victim" => format!("{{red}}{text}{{/red}}"),
        _ => text,
    }
}

pub struct Renderer {
    en: FluentBundle<FluentResource>,
    ko: FluentBundle<FluentResource>,
    ko_text: Locale,
    /// By prototype ID (`tba:30:mob:3060`) or player ID (`pc:ana`).
    beings: RwLock<HashMap<String, Being>>,
    /// Particle endings set for names the rule gets wrong (D23), by the name as shown.
    finals: Overrides,
    /// Object keywords by prototype, for D18 in lists.
    obj_keywords: HashMap<String, Vec<String>>,
    /// tbaMUD's combat message file (`third_party/tbamud/messages/combat.yaml`), if there.
    combat: Option<mundi_content::CombatMessages>,
    /// Its Korean lines (`locales/ko/combat.yaml`): act() codes with particle pairs (`$N{을/를}`).
    combat_ko: Option<mundi_content::CombatMessages>,
    /// Content lines in Korean by trigger and key (`locales/ko/triggers.yaml`): `%s` names, `%s{이/가}`.
    lines_ko: HashMap<String, HashMap<String, String>>,
    /// Spells' wear-off lines from the spell table (English).
    wearoffs: HashMap<i32, String>,
    /// The engine's made things' English names (tables/world.yaml `made`), to find a corpse's owner
    /// in its English name and a pile's size.
    made: Option<mundi_content::tables::Made>,
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
            FluentValue::from(particle_after(&f, word, pair))
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
        let obj_keywords = zones.iter().flat_map(|z| z.objects.iter().map(|(id, o)| (id.clone(), o.keywords.clone()))).collect();
        let combat = locales.parent().map(|p| p.join("messages/combat.yaml")).filter(|p| p.exists()).map(|p| mundi_content::load_messages(&p)).transpose().map_err(|e| e.to_string())?;
        let ko_combat = locales.join("ko/combat.yaml");
        let combat_ko = ko_combat.exists().then(|| mundi_content::load_messages(&ko_combat)).transpose().map_err(|e| e.to_string())?;
        let ko_lines = locales.join("ko/triggers.yaml");
        let lines_ko = ko_lines.exists().then(|| mundi_content::load_trigger_lines(&ko_lines)).transpose().map_err(|e| e.to_string())?.unwrap_or_default();
        let tables = locales
            .parent()
            .map(|p| p.join("tables"))
            .filter(|p| p.join("spells.yaml").exists())
            .and_then(|p| mundi_content::load_tables(&p).ok());
        let made = tables.as_ref().map(|t| t.world.made.clone());
        let wearoffs = tables.map(|t| t.spells.into_iter().filter_map(|s| Some((s.number, s.wearoff?))).collect()).unwrap_or_default();
        Ok(Renderer { en: bundle("en", &locales.join("en"))?, ko, ko_text, beings: RwLock::new(beings), finals, obj_keywords, combat, combat_ko, lines_ko, wearoffs, made })
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
                Refusal::NotYet => "refused-not-yet",
                Refusal::NotHereShop => "refused-not-here-shop",
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
                (MoveFailure::Closed | MoveFailure::Locked, Some(d)) => self.msg(v, "move-closed-door", &[("door", self.door_name(v, d))]),
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
            Event::Condition { hungry, thirsty, sober, full, quenched, drunk } => {
                let mut out = vec![];
                if *drunk == Some(true) {
                    out.push(m("cond-drunk"));
                }
                if *quenched == Some(true) {
                    out.push(m("cond-quenched"));
                }
                if *full == Some(true) {
                    out.push(m("cond-full"));
                }
                if *hungry == Some(true) {
                    out.push(m("cond-hungry"));
                }
                if *thirsty == Some(true) {
                    out.push(m("cond-thirsty"));
                }
                if *sober == Some(true) {
                    out.push(m("cond-sober"));
                }
                out
            }
            Event::LightFlicker { who: w, who_id } if w == mundi_protocol::SELF => {
                let _ = who_id;
                vec![m("light-flicker-self")]
            }
            Event::LightFlicker { who: w, who_id } => one("light-flicker", &[("who", who(w, who_id))]),
            Event::LightOut { who: w, .. } if w == mundi_protocol::SELF => vec![m("light-out-self")],
            Event::LightOut { who: w, who_id } => one("light-out", &[("who", who(w, who_id))]),
            Event::Got { text, id, from, from_id } => match from {
                Some(c) => one("got-from", &[("p", self.thing(v, text, id)), ("c", self.thing(v, c, from_id))]),
                None => one("got", &[("p", self.thing(v, text, id))]),
            },
            Event::JunkReward { who: w, who_id, .. } => {
                if w == mundi_protocol::SELF { vec![m("junk-reward")] } else { one("room-junk-reward", &[("who", who(w, who_id))]) }
            }
            Event::Coins { amount } => {
                if *amount == 1 { vec![m("coins-one")] } else { one("coins", &[("n", amount.to_string())]) }
            }
            Event::Gave { text, id, to, to_id } => one("gave", &[("p", self.thing(v, text, id)), ("c", who(to, to_id))]),
            Event::Received { text, id, from, from_id } => one("received", &[("who", who(from, from_id)), ("p", self.thing(v, text, id))]),
            Event::Used { action, text, id, into, into_id, slot, liquid } => {
                let mid = used_id("used", *action, slot.as_deref());
                let mut args = vec![("p", self.thing(v, text, id))];
                if let Some(c) = into {
                    args.push(("c", self.thing(v, c, into_id)));
                }
                if let Some(l) = liquid {
                    args.push(("liquid", self.msg(v, &format!("liquid-{}", l.replace(' ', "-")), &[])));
                }
                one(&mid, &args)
            }
            Event::OccupantItem { who: w, who_id, action, text, id, other, other_id, slot, liquid } => {
                let mid = used_id("room", *action, slot.as_deref());
                let mut args = vec![("who", who(w, who_id)), ("p", self.thing(v, text, id)), ("his", self.pronoun(who_id.as_deref(), "his").into())];
                if let Some(c) = other {
                    let name = if *action == mundi_protocol::ItemAction::Give { who(c, other_id) } else { self.thing(v, c, other_id) };
                    args.push(("c", name));
                }
                if let Some(l) = liquid {
                    args.push(("liquid", self.msg(v, &format!("liquid-{}", l.replace(' ', "-")), &[])));
                }
                one(&mid, &args)
            }
            Event::Zapped { who: w, who_id, text, id } if w == mundi_protocol::SELF => one("zapped", &[("p", self.thing(v, text, id))]),
            Event::Zapped { who: w, who_id, text, id } => one("room-zapped", &[("who", who(w, who_id)), ("p", self.thing(v, text, id))]),
            Event::TastedStrange { who: w, drink, .. } if w == mundi_protocol::SELF => vec![m(if *drink { "strange-drink" } else { "strange-eat" })],
            Event::TastedStrange { who: w, who_id, drink } => one(if *drink { "room-strange-drink" } else { "room-strange-eat" }, &[("who", who(w, who_id))]),
            Event::Inventory { items } => {
                let mut out = vec![m("inventory")];
                if items.is_empty() {
                    out.push(m("list-nothing"));
                }
                for it in items {
                    let name = self.carried_name(v, &it.text, &it.id);
                    out.push(if it.count > 1 { format!("({:2}) {name}", it.count) } else { name });
                }
                out
            }
            Event::Equipment { slots } => {
                let mut out = vec![m("equipment")];
                if slots.is_empty() {
                    out.push(m("equipment-nothing"));
                }
                for s in slots {
                    let label = pad(&self.msg(v, &format!("slot-{}", s.slot), &[]), 21);
                    let name = if s.id.is_none() && s.text == "something" { m("worn-something") } else { self.carried_name(v, &s.text, &s.id) };
                    out.push(format!("{label}{name}"));
                }
                out
            }
            Event::ItemFailed { action, reason, text, id, keyword, other, other_id, slot } => {
                vec![self.item_failure(v, *action, *reason, text.as_deref(), id.as_deref(), keyword.as_deref(), other.as_deref(), other_id.as_deref(), slot.as_deref())]
            }
            Event::Hit { .. } => self.blow(v, event),
            Event::CombatCondition { who: w, who_id, state } => {
                let st = match state {
                    mundi_protocol::DownState::MortallyWounded => "mortally_wounded",
                    mundi_protocol::DownState::Incapacitated => "incapacitated",
                    mundi_protocol::DownState::Stunned => "stunned",
                };
                if w == mundi_protocol::SELF { vec![m(&format!("down-self-{st}"))] } else { one(&format!("down-{st}"), &[("who", who(w, who_id))]) }
            }
            Event::Death { who: w, who_id } => one("dead", &[("who", who(w, who_id))]),
            Event::SelfDied {} => vec![m("dead-self")],
            Event::DeathCry { who: Some(w), who_id, .. } => one("death-cry", &[("who", who(w, who_id))]),
            Event::DeathCry { who: None, .. } => vec![m("death-cry-nearby")],
            Event::Pain { bleeding } => vec![m(if *bleeding { "pain-bleeding" } else { "pain-hurt" })],
            Event::Wimpy {} => vec![m("wimpy-out")],
            Event::SelfFled { .. } => vec![m("fled")],
            Event::FleeFailed { reason } => vec![m(if reason == "panic" { "flee-panic" } else { "flee-bad-shape" })],
            Event::FleeSeen { who: w, who_id, failed } => one(if *failed { "flee-seen-failed" } else { "flee-seen" }, &[("who", who(w, who_id))]),
            Event::Aggro { who: w, who_id, .. } => one("aggro-remembered", &[("who", who(w, who_id))]),
            Event::Assisted { who: w, who_id, target, target_id, jumped: true } => one("assisted", &[("who", who(w, who_id)), ("target", who(target, target_id))]),
            Event::Assisted { who: w, target, .. } if w == mundi_protocol::SELF => {
                let _ = target;
                vec![m("assist-joined")]
            }
            Event::Assisted { who: w, who_id, target, .. } if target == mundi_protocol::SELF => one("assist-you", &[("who", who(w, who_id))]),
            Event::Assisted { who: w, who_id, target, target_id, .. } => one("assist-room", &[("who", who(w, who_id)), ("target", who(target, target_id))]),
            Event::Appear { who: w, who_id } => one("appear", &[("who", who(w, who_id))]),
            Event::AttackRefused { reason } => {
                let r = serde_json::to_value(reason).ok().and_then(|j| j.as_str().map(|s| s.replace('_', "-"))).unwrap_or_default();
                vec![m(&format!("attack-{r}"))]
            }
            Event::SelfHit { who: w, .. } if w == mundi_protocol::SELF => vec![m("self-hit")],
            Event::SelfHit { who: w, who_id } => one("room-self-hit", &[("who", who(w, who_id)), ("him", self.pronoun(who_id.as_deref(), "him").into())]),
            Event::ExpGain { amount, kind } => {
                let k = if kind == "share" { "exp-share" } else { "exp-solo" };
                if *amount <= 1 { vec![m(&format!("{k}-one"))] } else { one(k, &[("n", amount.to_string())]) }
            }
            Event::LevelUp { levels } => {
                if *levels == 1 { vec![m("level-up")] } else { one("levels-up", &[("n", levels.to_string())]) }
            }
            Event::Decayed { text, id, carried } => one(if *carried { "decayed-carried" } else { "decayed" }, &[("p", self.thing(v, text, id))]),
            Event::Toggle { name, value } if name == "wimpy" => {
                let refused = value.get("refused").and_then(|r| r.as_str());
                match (refused, value.get("current").and_then(|c| c.as_i64()), value.as_i64()) {
                    (Some(r), _, _) => vec![m(&format!("wimpy-{}", r.replace('_', "-")))],
                    (None, Some(0), _) => vec![m("wimpy-none")],
                    (None, Some(n), _) => one("wimpy-current", &[("n", n.to_string())]),
                    (None, None, Some(0)) => vec![m("wimpy-off")],
                    (None, None, Some(n)) => one("wimpy-set", &[("n", n.to_string())]),
                    _ => vec![],
                }
            }
            Event::SkillResult { ok: true, .. } => vec![m("skill-ok")],
            Event::SkillResult { skill, reason, who: w, who_id, .. } => {
                let r = reason.as_deref().unwrap_or("failed");
                let name = w.as_deref().map(|n| who(n, who_id)).unwrap_or_default();
                one(&format!("skill-{skill}-{r}"), &[("who", name), ("him", self.pronoun(who_id.as_deref(), "him").into())])
            }
            Event::Rescue { rescuer, rescuer_id, rescued, rescued_id } => {
                let me = mundi_protocol::SELF;
                if rescuer == me {
                    vec![m("rescue-self")]
                } else if rescued == me {
                    one("rescue-you", &[("who", who(rescuer, rescuer_id))])
                } else {
                    one("rescue-room", &[("a", who(rescuer, rescuer_id)), ("b", who(rescued, rescued_id))])
                }
            }
            Event::Noticed { who: w, who_id, by, by_id } => {
                let me = mundi_protocol::SELF;
                let pron = [("he", self.pronoun(who_id.as_deref(), "he").into()), ("him", self.pronoun(who_id.as_deref(), "him").into())];
                if w == me {
                    one("noticed-you", &[("by", who(by, by_id))])
                } else if by == me {
                    one("noticed-by-you", &[("who", who(w, who_id)), pron[0].clone(), pron[1].clone()])
                } else {
                    one("noticed-room", &[("who", who(w, who_id)), ("by", who(by, by_id)), pron[1].clone()])
                }
            }
            Event::SpellSaid { who: w, who_id, words, target, target_id, at_object, at_self } => {
                let base = [("who", who(w, who_id)), ("words", escape(words)), ("his", self.pronoun(who_id.as_deref(), "his").into())];
                match (target, at_self) {
                    (_, true) => one("said-self", &base),
                    (Some(t), _) if t == mundi_protocol::SELF => one("said-at-you", &base),
                    (Some(t), _) => {
                        let name = if *at_object { self.thing(v, t, target_id) } else { who(t, target_id) };
                        let mut a = base.to_vec();
                        a.push(("target", name));
                        one("said-at", &a)
                    }
                    (None, _) => one("said", &base),
                }
            }
            Event::SpellEffect { spell, line, who: w, who_id, text, id } => {
                let p = text.as_deref().map(|t| self.thing(v, t, id)).unwrap_or_default();
                one(&format!("spell-{spell}-{line}"), &[("who", who(w, who_id)), ("p", p)])
            }
            Event::WoreOff { spell, .. } => {
                let id = format!("wearoff-{spell}");
                if v.lang == Lang::Ko && self.ko.has_message(&id) {
                    vec![m(&id)]
                } else {
                    self.wearoffs.get(spell).cloned().into_iter().collect()
                }
            }
            Event::DoorFailed { command, reason, word } => {
                if reason == "what" {
                    return vec![m(&format!("door-what-{command}"))];
                }
                let w = word.as_deref().unwrap_or("");
                let done = match command.as_str() {
                    "open" => "opened",
                    "close" => "closed",
                    "lock" => "locked",
                    "unlock" => "unlocked",
                    _ => "picked",
                };
                let article = if v.lang == Lang::En { format!("{} {}", an(w), escape(w)) } else { escape(w) };
                one(&format!("door-{reason}"), &[("cmd", command.clone()), ("word", escape(w)), ("w", article), ("done", done.into())])
            }
            Event::DoorChanged { command, who: w, who_id, door, text, id, far } => {
                if *far {
                    return one(&format!("door-far-{command}"), &[("door", self.door_name(v, door.as_deref().unwrap_or("door")))]);
                }
                if w == mundi_protocol::SELF {
                    return vec![m(&format!("door-done-{command}"))];
                }
                let did = self.msg(v, &format!("door-did-{command}"), &[]);
                let p = text.as_deref().map(|t| self.thing(v, t, id)).unwrap_or_default();
                let mid = match (command.as_str(), text.is_some()) {
                    ("pick", false) => "door-room-pick",
                    ("pick", true) => "door-room-pick-obj",
                    (_, true) => "door-room-obj",
                    _ => "door-room",
                };
                one(mid, &[("who", who(w, who_id)), ("cmd", command.clone()), ("door", self.door_name(v, door.as_deref().unwrap_or("door"))), ("p", p), ("did", did)])
            }
            Event::Tell { from, from_id, to, to_id, text, direction } => match direction {
                Direction::In => one("tell-in", &[("who", who(from, from_id)), ("text", escape(text))]),
                Direction::Out => one("tell-out", &[("who", who(to, to_id)), ("text", escape(text))]),
            },
            Event::ShopList { items, none_matching } => {
                if *none_matching {
                    return vec![m("shop-none-of")];
                }
                if items.is_empty() {
                    return vec![m("shop-none")];
                }
                let mut out = vec![m("shop-header"), "-".repeat(76)];
                for it in items {
                    let mut name = self.thing(v, &it.text, &it.id);
                    if let Some(l) = &it.liquid {
                        let liquid = self.msg(v, &format!("liquid-{}", l.replace(' ', "-")), &[]);
                        name = self.msg(v, "shop-of", &[("item", name), ("liquid", liquid)]);
                    }
                    let available = it.count.map_or_else(|| m("shop-unlimited"), |n| n.to_string());
                    out.push(format!(" {:2})  {:>9}   {} {:6}", it.index, available, pad(&cap(&name), 48), it.price));
                }
                out
            }
            Event::ShopResult { action, who: w, who_id, text } => {
                let me = w == mundi_protocol::SELF;
                let id = match (action.as_str(), me) {
                    ("buy", true) => "shop-you-buy",
                    ("sell", true) => "shop-you-sell",
                    ("buy", false) => "shop-room-buy",
                    ("sell", false) => "shop-room-sell",
                    ("too_many", _) => "shop-too_many",
                    _ => "shop-too_heavy",
                };
                one(id, &[("who", who(w, who_id)), ("text", escape(text))])
            }
            Event::Blocked { who: w, who_id } => {
                if w == mundi_protocol::SELF { vec![m("blocked-you")] } else { one("blocked-room", &[("who", who(w, who_id)), ("his", self.pronoun(who_id.as_deref(), "his").into())]) }
            }
            Event::VitalsMax { .. } => vec![],
            Event::Score(sc) => self.score(v, sc),
            Event::Skills { practices, spells, skills } => {
                let sessions = m(if *practices == 1 { "practice-session" } else { "practice-sessions" });
                let mut out = vec![
                    self.msg(v, "practice-left", &[("n", practices.to_string()), ("sessions", sessions)]),
                    m(if *spells { "practice-know-spells" } else { "practice-know-skills" }),
                ];
                for k in skills {
                    let how = match k.percent {
                        ..0 => "how-not-learned",
                        0 => "how-not-learned",
                        ..=10 => "how-awful",
                        ..=20 => "how-bad",
                        ..=40 => "how-poor",
                        ..=55 => "how-average",
                        ..=70 => "how-fair",
                        ..=80 => "how-good",
                        ..=85 => "how-very-good",
                        _ => "how-superb",
                    };
                    out.push(format!("{} {}", pad(&k.name, 20), m(how)));
                }
                out
            }
            Event::Practiced { result, reason, spells, .. } => match (result.as_str(), reason.as_deref()) {
                ("improved", _) => vec![m("practiced-improved")],
                ("learned", _) => vec![m("practiced-improved"), m("practiced-learned")],
                ("maxed", _) => vec![m("practiced-maxed")],
                (_, Some("unknown_skill")) => vec![m(if *spells { "practiced-unknown_skill-spells" } else { "practiced-unknown_skill-skills" })],
                (_, Some(r)) => vec![m(&format!("practiced-{r}"))],
                _ => vec![],
            },
            Event::Emote { who: w, who_id, text, line } => one("emote", &[("who", who(w, who_id)), ("text", self.content_line(v, text, line))]),
            Event::Echo { text, line } => vec![self.content_line(v, text, line)],
            Event::NewCharacter {} => vec![m("new-character-1"), m("new-character-2"), m("new-character-3")],
            Event::Toggle { name, value } if value.is_boolean() => {
                vec![m(&format!("toggle-{}-{name}", if value.as_bool() == Some(true) { "on" } else { "off" }))]
            }
            Event::Toggle { .. } => vec![],
            Event::GroupChange { event: ev, who: w, who_id, formed } => {
                let key = if *formed { "leader" } else { ev.as_str() };
                let line = self.msg(v, &format!("group-{key}"), &[("who", who(w, who_id))]);
                let prefixed = matches!(ev.as_str(), "joined" | "left" | "new_leader" | "died");
                vec![if prefixed { self.group_line(v, &line) } else { line }]
            }
            Event::OccupantFollow { who: w, who_id, leader, leader_id, stopped } => {
                one(if *stopped { "room-unfollow" } else { "room-follow" }, &[("who", who(w, who_id)), ("leader", who(leader, leader_id))])
            }
            Event::FollowMoved { leader, leader_id, .. } => one("follow-moved", &[("leader", who(leader, leader_id))]),
            Event::GroupStatus { members } => {
                let mut out = vec![m("group-status")];
                for g in members {
                    let name = pad(&self.name(v, &g.name, g.id.as_deref(), Spot::Prose), 22);
                    let colour = if g.leader { "bright_green" } else { "green" };
                    out.push(format!(
                        "{name}: {{{colour}}}[{:4}/{:<4}]H [{:4}/{:<4}]M [{:4}/{:<4}]V{{/{colour}}}",
                        g.hp, g.hp_max, g.mp, g.mp_max, g.mv, g.mv_max
                    ));
                }
                out
            }
            Event::GroupReport { member: g } => {
                let args = [
                    ("who", self.name(v, &g.name, g.id.as_deref(), Spot::Prose)),
                    ("hp", g.hp.to_string()),
                    ("hpm", g.hp_max.to_string()),
                    ("mp", g.mp.to_string()),
                    ("mpm", g.mp_max.to_string()),
                    ("mv", g.mv.to_string()),
                    ("mvm", g.mv_max.to_string()),
                ];
                vec![self.group_line(v, &self.msg(v, "group-report", &args))]
            }
            Event::GroupOption { open, anonymous } => match (open, anonymous) {
                (Some(true), _) => vec![m("group-option-open")],
                (Some(false), _) => vec![m("group-option-closed")],
                (_, Some(true)) => vec![m("group-option-anonymous")],
                _ => vec![m("group-option-visible")],
            },
            Event::GroupFailed { reason, who: w, who_id } => {
                let name = w.as_deref().map(|n| self.name(v, n, who_id.as_deref(), Spot::Prose)).unwrap_or_default();
                one(&format!("gfail-{reason}"), &[("who", name), ("him", self.pronoun(who_id.as_deref(), "him").into())])
            }
            Event::Gtell { from, from_id, text, direction } => match direction {
                Direction::Out => vec![format!("{{green}}{}{{/green}}", self.msg(v, "gsay-out", &[("text", escape(text))]))],
                Direction::In => {
                    let line = format!("{{green}}{}{{/green}}", self.msg(v, "gsay-in", &[("who", who(from, from_id)), ("text", escape(text))]));
                    vec![self.group_line(v, &line)]
                }
            },
            Event::Split { from, from_id, amount, share, rest, members } => {
                let coins = if *rest == 1 { "coin" } else { "coins" }.to_string();
                let were = if *rest == 1 { "was" } else { "were" }.to_string();
                let mut args = vec![
                    ("who", who(from, from_id)),
                    ("amount", amount.to_string()),
                    ("share", share.to_string()),
                    ("n", members.to_string()),
                    ("rest", rest.to_string()),
                    ("coins", coins),
                    ("were", were),
                ];
                let me = from == mundi_protocol::SELF;
                let mut out = vec![self.msg(v, if me { "split-self" } else { "split-other" }, &args)];
                if *rest > 0 {
                    args.push(("x", String::new()));
                    out.push(self.msg(v, if me { "split-self-rest" } else { "split-other-rest" }, &args));
                }
                out
            }
            Event::WorldTime { phase } => vec![m(match phase {
                DayPhase::Sunrise => "time-sunrise",
                DayPhase::Day => "time-day",
                DayPhase::Sunset => "time-sunset",
                DayPhase::Night => "time-night",
            })],
            Event::Say { from, from_id, text, direction, line } => match direction {
                Direction::Out => one("say-out", &[("text", self.content_line(v, text, line))]),
                Direction::In => one("say-in", &[("who", who(from, from_id)), ("text", self.content_line(v, text, line))]),
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
                // tbaMUD puts a closed exit in parentheses; Korean, whose exits already carry the
                // keyword in parentheses, says 닫힘 instead (exit-closed-keyword).
                let closed = if v.keywords == KeywordMode::Off { "exit-closed" } else { "exit-closed-keyword" };
                if e.closed { format!("{{red}}{}{{/red}} ", self.msg(v, closed, &[("exit", shown)])) } else { format!("{shown} ") }
            })
            .collect();
        let exits = if exits.is_empty() { self.msg(v, "exits-none", &[]) } else { exits };
        out.push(format!("{{cyan}}[ {}: {exits}]{{/cyan}}", self.msg(v, "exits-label", &[])));
        let obj_keywords = unique_keywords(room.objects.iter().map(|o| prefer(&o.keywords, &o.text)).collect());
        for (o, kw) in room.objects.iter().zip(obj_keywords) {
            out.push(format!("{{green}}{}{{/green}}", self.object_line(v, o, kw.as_deref())));
        }
        let keywords = unique_keywords(
            room.occupants.iter().map(|o| prefer(&self.keywords_of(o), if o.name.is_empty() { o.long.as_deref().unwrap_or("") } else { &o.name })).collect(),
        );
        for (o, kw) in room.occupants.iter().zip(keywords) {
            out.push(format!("{{yellow}}{}{{/yellow}}", self.occupant_line(v, o, kw.as_deref())));
        }
        out
    }

    /// One blow for its viewer (MECHANICS §7.6, §8.2): the message file's lines when it has some for
    /// this attack and outcome (always for skills and spells; weapons only on a miss or a kill),
    /// else fight.c's damage table by severity.
    fn blow(&self, v: Viewer, e: &Event) -> Vec<String> {
        let Event::Hit { attacker, attacker_id, victim, victim_id, verb, severity, kind, attack, outcome, variant, weapon, weapon_id, .. } = e else {
            return vec![];
        };
        let me = mundi_protocol::SELF;
        let role = if victim == me { "victim" } else if attacker == me { "attacker" } else { "room" };
        let from_file = *kind != mundi_protocol::HitKind::Weapon || matches!(outcome, HitOutcome::Miss | HitOutcome::Die);
        if from_file && v.lang == Lang::Ko {
            let line = self.combat_lines(self.combat_ko.as_ref(), *attack, *variant).and_then(|set| pick(set, outcome, role).clone());
            if let Some(l) = line {
                let text = self.act_ko(v, &l, (attacker, attacker_id.as_deref()), (victim, victim_id.as_deref()), weapon.as_deref().map(|w| (w, weapon_id)));
                return vec![colour_for(role, text)];
            }
        }
        if from_file {
            if let Some(set) = self.combat_lines(self.combat.as_ref(), *attack, *variant) {
                return pick(set, outcome, role)
                    .iter()
                    .map(|l| colour_for(role, self.act(v, l, (attacker, attacker_id.as_deref()), (victim, victim_id.as_deref()), weapon.as_deref().map(|w| (w, weapon_id)))))
                    .collect();
            }
        }
        let word = self.msg(v, &format!("verb-{verb}"), &[]);
        let words = self.msg(v, &format!("verb-{verb}-s"), &[]);
        let a = self.name(v, attacker, attacker_id.as_deref(), Spot::Prose);
        let vn = self.name(v, victim, victim_id.as_deref(), Spot::Prose);
        let args = [
            ("a", a),
            ("v", vn),
            ("w", word),
            ("ws", words),
            ("ae", self.pronoun(attacker_id.as_deref(), "he").into()),
            ("as", self.pronoun(attacker_id.as_deref(), "his").into()),
            ("vm", self.pronoun(victim_id.as_deref(), "him").into()),
        ];
        vec![self.msg(v, &format!("dam-{severity}-{role}"), &args)]
    }

    /// The message file's set for an attack, the variant chosen by the event's roll.
    fn combat_lines<'a>(&self, file: Option<&'a mundi_content::CombatMessages>, attack: i32, roll: Option<u32>) -> Option<&'a mundi_content::tables::MessageSet> {
        let a = file?.attacks.iter().find(|a| a.number == attack)?;
        let n = a.variants.len().max(1);
        a.variants.get((roll.unwrap_or(1) as usize).saturating_sub(1) % n)
    }

    /// act()'s codes in a message-file line: $n $N names, $e $E he, $m $M him, $s $S his, $p the
    /// weapon, $$ a dollar; the line's first letter capitalised (comm.c perform_act).
    fn act(&self, v: Viewer, line: &str, n: (&str, Option<&str>), big_n: (&str, Option<&str>), p: Option<(&str, &Option<String>)>) -> String {
        let me = mundi_protocol::SELF;
        let who = |(name, id): (&str, Option<&str>)| if name == me { "you".to_string() } else { self.name(v, name, id, Spot::Prose) };
        let mut out = String::new();
        let mut chars = line.chars();
        while let Some(c) = chars.next() {
            if c != '$' {
                out.push(c);
                continue;
            }
            match chars.next() {
                Some('n') => out.push_str(&who(n)),
                Some('N') => out.push_str(&who(big_n)),
                Some('e') => out.push_str(self.pronoun(n.1, "he")),
                Some('E') => out.push_str(self.pronoun(big_n.1, "he")),
                Some('m') => out.push_str(self.pronoun(n.1, "him")),
                Some('M') => out.push_str(self.pronoun(big_n.1, "him")),
                Some('s') => out.push_str(self.pronoun(n.1, "his")),
                Some('S') => out.push_str(self.pronoun(big_n.1, "his")),
                Some('p') => out.push_str(&p.map(|(t, id)| self.thing(v, t, id)).unwrap_or_else(|| "something".into())),
                Some('$') => out.push('$'),
                Some(x) => {
                    out.push('$');
                    out.push(x);
                }
                None => out.push('$'),
            }
        }
        cap(&out)
    }

    /// A Korean message-file line: $n $N names (당신 for the reader), a pair in braces after a code
    /// its particle (D23), $p the weapon. The translation writes names for English's pronoun codes;
    /// any left are 그.
    fn act_ko(&self, v: Viewer, line: &str, n: (&str, Option<&str>), big_n: (&str, Option<&str>), p: Option<(&str, &Option<String>)>) -> String {
        let me = mundi_protocol::SELF;
        let who = |(name, id): (&str, Option<&str>)| if name == me { "당신".to_string() } else { self.name(v, name, id, Spot::Prose) };
        let mut out = String::new();
        let mut rest = line;
        while let Some(at) = rest.find('$') {
            out.push_str(&rest[..at]);
            let mut chars = rest[at + 1..].chars();
            let code = chars.next();
            rest = chars.as_str();
            let word = match code {
                Some('n') => who(n),
                Some('N') => who(big_n),
                Some('p') => p.map(|(t, id)| self.thing(v, t, id)).unwrap_or_else(|| "무언가".into()),
                Some('e' | 'E' | 'm' | 'M') => "그".into(),
                Some('s' | 'S') => "그의".into(),
                Some('$') => "$".into(),
                Some(x) => format!("${x}"),
                None => "$".into(),
            };
            out.push_str(&word);
            if let Some(pair) = rest.strip_prefix('{').and_then(|r| r.split_once('}')).filter(|(pair, _)| josa::particle(pair, Final::None).is_some()) {
                out.push_str(&particle_after(&self.finals, &word, pair.0));
                rest = pair.1;
            }
        }
        out.push_str(rest);
        out
    }

    /// do_score's lines (MECHANICS §18).
    fn score(&self, v: Viewer, sc: &mundi_protocol::Score) -> Vec<String> {
        let m = |id: &str, args: &[(&str, String)]| self.msg(v, id, args);
        let plural = |n: i64| if n == 1 { "" } else { "s" }.to_string();
        let mut out = vec![m(if sc.birthday { "score-age-birthday" } else { "score-age" }, &[("age", sc.age.to_string())])];
        out.push(m("score-points", &[
            ("hp", sc.hp.to_string()), ("hpmax", sc.hp_max.to_string()), ("mp", sc.mp.to_string()),
            ("mpmax", sc.mp_max.to_string()), ("mv", sc.mv.to_string()), ("mvmax", sc.mv_max.to_string()),
        ]));
        out.push(m("score-ac", &[("ac", sc.ac.to_string()), ("align", sc.alignment.to_string())]));
        out.push(m("score-exp", &[("exp", sc.exp.to_string()), ("gold", sc.gold.to_string()), ("qp", sc.quest_points.to_string())]));
        if let Some(need) = sc.exp_to_next {
            out.push(m("score-need", &[("need", need.to_string())]));
        }
        out.push(m("score-quest-points", &[("qp", sc.quest_points.to_string())]));
        out.push(m("score-quests", &[("n", sc.quests.to_string()), ("s", plural(sc.quests as i64))]));
        out.push(m("score-played", &[
            ("days", sc.played_days.to_string()), ("ds", plural(sc.played_days)),
            ("hours", sc.played_hours.to_string()), ("hs", plural(sc.played_hours)),
        ]));
        out.push(m("score-rank", &[("name", escape(&sc.name)), ("title", escape(&sc.title)), ("level", sc.level.to_string())]));
        let pos = match sc.position {
            Position::Dead => "dead",
            Position::MortallyWounded => "mortally-wounded",
            Position::Incapacitated => "incapacitated",
            Position::Stunned => "stunned",
            Position::Sleeping => "sleeping",
            Position::Resting => "resting",
            Position::Sitting => "sitting",
            Position::Fighting => "fighting",
            Position::Standing => "standing",
        };
        let foe = sc.fighting.as_deref().map(|f| self.name(v, f, sc.fighting_id.as_deref(), Spot::Prose)).unwrap_or_default();
        out.push(m(&format!("score-pos-{pos}"), &[("who", foe)]));
        for st in &sc.states {
            out.push(m(&format!("score-state-{st}"), &[]));
        }
        out
    }

    /// A door by its keyword, as tbaMUD names it ("The door seems to be closed."). Korean: the
    /// word from `door-name-<keyword>` when there is one, with the keyword where keywords show (D18:
    /// `문(door)`, what `open` takes); a keyword without one stays as it is.
    fn door_name(&self, v: Viewer, keyword: &str) -> String {
        let id = format!("door-name-{}", keyword.to_lowercase());
        if v.lang == Lang::Ko && self.ko.has_message(&id) {
            let word = self.msg(v, &id, &[]);
            return if v.keywords == KeywordMode::Off { word } else { format!("{word}({})", escape(keyword)) };
        }
        escape(keyword)
    }

    /// A content line (a trigger's): the reader's language when the overlay has it, its `%s` the
    /// names in order (당신 for the reader) and a pair after one its particle (D23); else the English.
    fn content_line(&self, v: Viewer, english: &str, line: &Option<LineRef>) -> String {
        let ko = line.as_ref().filter(|_| v.lang == Lang::Ko).and_then(|l| Some((l, self.lines_ko.get(&l.id)?.get(&l.key)?)));
        let Some((l, pattern)) = ko else { return escape(english) };
        let mut names = l.names.iter();
        let mut out = String::new();
        let mut rest = pattern.as_str();
        while let Some(at) = rest.find("%s") {
            out.push_str(&rest[..at]);
            rest = &rest[at + 2..];
            let word = match names.next() {
                Some(n) if n.name == mundi_protocol::SELF => "당신".to_string(),
                Some(n) => self.name(v, &n.name, n.id.as_deref(), Spot::Prose),
                None => String::new(),
            };
            out.push_str(&word);
            if let Some((pair, after)) = rest.strip_prefix('{').and_then(|r| r.split_once('}')).filter(|(pair, _)| josa::particle(pair, Final::None).is_some()) {
                out.push_str(&particle_after(&self.finals, &word, pair));
                rest = after;
            }
        }
        out.push_str(rest);
        out
    }

    /// A line sent to a group: "[Group] " in green before it (comm.c send_to_group).
    fn group_line(&self, v: Viewer, line: &str) -> String {
        format!("{{green}}[{{bright_green}}{}{{/bright_green}}]{{/green}} {line}", self.msg(v, "group-prefix", &[]).trim_matches(['[', ']']))
    }

    /// An object named in a sentence: its short description, Korean from the overlay.
    fn thing(&self, v: Viewer, english: &str, id: &Option<String>) -> String {
        if let Some(name) = self.made_name(v, english, id.as_deref(), false) {
            return name;
        }
        if v.lang == Lang::Ko {
            if let Some(short) = id.as_deref().and_then(|i| self.ko_text.get(proto(i))).and_then(|t| t.short.clone()) {
                return short;
            }
        }
        escape(english)
    }

    /// A thing the engine made (a corpse, coins) in the reader's language: its ID says what it is
    /// (`corpse:<whose>`, `money:<coins>`); English keeps the engine's name. `long`: its room line.
    fn made_name(&self, v: Viewer, english: &str, id: Option<&str>, long: bool) -> Option<String> {
        if v.lang == Lang::En {
            return None;
        }
        let kind = proto(id?);
        let made = self.made.as_ref()?;
        if let Some(whose) = kind.strip_prefix("corpse:") {
            // The dead one's name: Korean for a mob with one, else as the English name has it.
            let template = if long { &made.corpse_long } else { &made.corpse_short };
            let (pre, post) = template.split_once("%s")?;
            let in_english = english.strip_prefix(pre).or_else(|| english.strip_prefix(&cap(pre)))?.strip_suffix(post)?;
            let who = self.ko_text.get(whose).and_then(|t| t.short.clone()).unwrap_or_else(|| escape(in_english));
            return Some(self.msg(v, if long { "made-corpse-long" } else { "made-corpse" }, &[("who", who)]));
        }
        let coins: i64 = kind.strip_prefix("money:")?.parse().ok()?;
        let size = if coins == 1 { "coin".to_string() } else {
            made.money.iter().position(|m| coins <= m.up_to).map_or("more".to_string(), |i| i.to_string())
        };
        let name = self.msg(v, &format!("made-money-{size}"), &[]);
        Some(if long { self.msg(v, if coins == 1 { "made-coin-long" } else { "made-money-long" }, &[("what", name)]) } else { name })
    }

    /// An object in an inventory or equipment list: a place a command names it from (D18).
    fn carried_name(&self, v: Viewer, english: &str, id: &Option<String>) -> String {
        let name = self.thing(v, english, id);
        if v.lang == Lang::Ko && v.keywords != KeywordMode::Off {
            if let Some(kw) = id.as_deref().and_then(|i| self.obj_keywords.get(proto(i))).and_then(|k| prefer(k, english).into_iter().next()) {
                return format!("{name}({kw})");
            }
        }
        name
    }

    #[allow(clippy::too_many_arguments)]
    fn item_failure(
        &self,
        v: Viewer,
        action: mundi_protocol::ItemAction,
        reason: mundi_protocol::ItemFailure,
        text: Option<&str>,
        id: Option<&str>,
        keyword: Option<&str>,
        other: Option<&str>,
        other_id: Option<&str>,
        slot: Option<&str>,
    ) -> String {
        use mundi_protocol::ItemFailure as F;
        let act = serde_json::to_value(action).ok().and_then(|j| j.as_str().map(str::to_string)).unwrap_or_default();
        let mut why = serde_json::to_value(reason).ok().and_then(|j| j.as_str().map(|s| s.replace('_', "-"))).unwrap_or_default();
        if reason == F::AlreadyWearing {
            return self.msg(v, &format!("already-{}", slot.unwrap_or("hold")), &[]);
        }
        // Variants by what is known: a word not found in a container, a scenery word, none of them in a container.
        if reason == F::NotHere && other.is_some() {
            why = "not-here-in".into();
        } else if reason == F::CantTake && text.is_none() {
            why = "cant-take-scenery".into();
        } else if reason == F::NoneOf && text.is_some() {
            why = "none-of-in".into();
        }
        let word = keyword.unwrap_or("");
        let w = if v.lang == Lang::En { format!("{} {}", an(word), escape(word)) } else { escape(word) };
        let p = text.map(|t| self.thing(v, t, &id.map(str::to_string))).unwrap_or_default();
        let c = other.map(|o| self.name_or_thing(v, o, other_id)).unwrap_or_default();
        let args = [
            ("p", p),
            ("c", c),
            ("w", w),
            ("word", escape(word)),
            ("che", self.pronoun(other_id, "he").into()),
            ("chis", self.pronoun(other_id, "his").into()),
        ];
        for id in [format!("fail-{act}-{why}"), format!("fail-{why}")] {
            if self.has(v, &id) {
                return self.msg(v, &id, &args);
            }
        }
        format!("[fail-{act}-{why}]")
    }

    /// The other party of an object failure: a person (by `pc:`/mob ID) or an object.
    fn name_or_thing(&self, v: Viewer, text: &str, id: Option<&str>) -> String {
        match id {
            Some(i) if i.starts_with("pc:") || i.contains(":mob:") => self.name(v, text, Some(i), Spot::Prose),
            _ => self.thing(v, text, &id.map(str::to_string)),
        }
    }

    fn has(&self, v: Viewer, id: &str) -> bool {
        let b = if v.lang == Lang::Ko { &self.ko } else { &self.en };
        b.has_message(id) || self.en.has_message(id)
    }

    /// An object's line in a room (MECHANICS §3.3): its long description (Korean from the overlay,
    /// with the keyword where D18 puts it), the count, and tbaMUD's tags.
    fn object_line(&self, v: Viewer, o: &mundi_protocol::RoomObject, keyword: Option<&str>) -> String {
        let tr = if v.lang == Lang::Ko { o.id.as_deref().and_then(|id| self.ko_text.get(proto(id))) } else { None };
        let made = self.made_name(v, &o.text, o.id.as_deref(), true);
        let made_short = made.as_ref().and_then(|_| self.made_name(v, &o.text, o.id.as_deref(), false));
        let mut line = made.clone().or_else(|| tr.and_then(|t| t.long.clone())).unwrap_or_else(|| o.text.clone());
        if v.lang == Lang::Ko && v.keywords != KeywordMode::Off {
            if let Some(kw) = keyword {
                line = match made_short.as_deref().or(tr.and_then(|t| t.short.as_deref())) {
                    Some(short) if line.contains(short) => line.replacen(short, &format!("{short}({kw})"), 1),
                    _ => format!("{line} ({kw})"),
                };
            }
        }
        if o.count > 1 {
            line = format!("({:2}) {line}", o.count);
        }
        for flag in ["invisible", "glow", "hum"] {
            if o.flags.iter().any(|f| f == flag) {
                line = format!("{line} {}", self.msg(v, &format!("obj-flag-{flag}"), &[]));
            }
        }
        line
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
            Position::Fighting => match o.fighting.as_deref() {
                None => "occupant-fighting-air",
                Some(mundi_protocol::SELF) => "occupant-fighting-you",
                Some("") => "occupant-fighting-left",
                Some(_) => "occupant-fighting",
            },
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
        let foe = o.fighting.as_deref().map(|f| self.name(v, f, o.fighting_id.as_deref(), Spot::Prose)).unwrap_or_default();
        // The name first, its first letter up (act.informative.c:345 UPPER(*short_descr)).
        cap(&self.msg(v, id, &[("who", name), ("foe", foe)]))
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

/// `used-wear-body`, `room-get`: the template of an object action, by slot for wearing.
fn used_id(prefix: &str, action: mundi_protocol::ItemAction, slot: Option<&str>) -> String {
    use mundi_protocol::ItemAction as A;
    match (action, slot) {
        (A::Wear, Some(s)) => format!("{prefix}-wear-{s}"),
        (A::Hold, Some("light")) => format!("{prefix}-light"),
        _ => {
            let a = serde_json::to_value(action).ok().and_then(|j| j.as_str().map(str::to_string)).unwrap_or_default();
            if prefix == "room" { format!("room-{a}") } else { format!("used-{a}") }
        }
    }
}

/// "a" or "an" before a typed word (utils.h AN).
fn an(word: &str) -> &'static str {
    if word.starts_with(['a', 'e', 'i', 'o', 'u', 'A', 'E', 'I', 'O', 'U']) { "an" } else { "a" }
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
    fn korean_messages_take_particles_after_values_through_josa() {
        // D23: a value's particle depends on how it ends (155 is 백오십오: 를), so it is never
        // written in the message itself ("{ $n }을" was wrong for 155).
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/tbamud/locales/ko");
        let varying = ["으로", "이", "가", "은", "는", "을", "를", "과", "와", "로", "아", "야"];
        for entry in std::fs::read_dir(&root).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|e| e != "ftl") {
                continue;
            }
            for (i, line) in std::fs::read_to_string(&path).unwrap().lines().enumerate() {
                for (at, _) in line.match_indices(" }") {
                    let after = &line[at + 2..];
                    let Some(p) = varying.iter().find(|p| after.starts_with(*p)) else { continue };
                    let next = after[p.len()..].chars().next();
                    let is_value = line[..at].rfind("{ $").is_some_and(|open| !line[open..at].contains('}'));
                    assert!(
                        !is_value || next.is_some_and(|c| ('가'..='힣').contains(&c)),
                        "{}:{}: a particle written after a value: {line}",
                        path.display(),
                        i + 1
                    );
                }
            }
        }
    }

    #[test]
    fn every_korean_combat_line_renders_clean() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/tbamud");
        let r = Renderer::load(&root.join("locales"), &[]).unwrap();
        let ko = Viewer { lang: Lang::Ko, keywords: KeywordMode::Off };
        let Some(file) = &r.combat_ko else { return };
        let dagger = Some("tba:30:obj:3020".to_string());
        for a in &file.attacks {
            for set in &a.variants {
                for lines in [&set.die, &set.miss, &set.hit, &set.god] {
                    for l in [&lines.attacker, &lines.victim, &lines.room].into_iter().flatten() {
                        let text = r.act_ko(ko, l, ("Ana", None), ("the beggar", None), Some(("a dagger", &dagger)));
                        assert!(!text.contains('$') && !text.contains('[') && !text.contains('/'), "attack {}: {l} -> {text}", a.number);
                    }
                }
            }
        }
    }

    #[test]
    fn korean_combat_lines_take_particles_after_names() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/tbamud");
        let r = Renderer::load(&root.join("locales"), &[]).unwrap();
        let ko = Viewer { lang: Lang::Ko, keywords: KeywordMode::Off };
        r.register_player("Ana", Sex::Female, None);
        r.register_player("Bob", Sex::Male, Some(KoFinal::Other));
        let line = "$n{이/가} $N{을/를} 노렸지만 $N{은/는} 피했다! $$";
        let text = r.act_ko(ko, line, ("Ana", Some("pc:ana")), ("Bob", Some("pc:bob")), None);
        assert_eq!(text, "Ana가 Bob을 노렸지만 Bob은 피했다! $");
        let text = r.act_ko(ko, line, (mundi_protocol::SELF, None), ("Ana", Some("pc:ana")), None);
        assert_eq!(text, "당신이 Ana를 노렸지만 Ana는 피했다! $");
        assert_eq!(r.act_ko(ko, "$p{으로/로} {yellow}$N{/yellow}", ("Ana", None), ("Ana", None), None), "무언가로 {yellow}Ana{/yellow}");
    }

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
