//! The server loop: connections in, the simulation stepped ten times a second, each delivery rendered
//! for its recipient and sent, places saved. The only crate that knows all the others.
//!
//! Login is a store question (an Argon2 hash takes a while), so it runs off the loop and its answer
//! comes back as a message; the simulation never waits for it. Passwords go from the network to the
//! hash and nowhere else: they are not inputs, not logged, not in any event (PROTOCOL.md §5).

use std::collections::HashMap;
use std::io::Write;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mundi_content::{load_tables, load_world};
use mundi_net::{ConnId, Net, NetEvent};
use mundi_protocol::{ClientMessage, Envelope, Event, KoFinal, LoginFailure, LoginStage, Sex, VERSION};
use mundi_render::{ansi, plain, Renderer, Viewer};
use mundi_sim::{Input, Sim, PULSES_PER_SEC};
use mundi_store::{valid_name, Login, Store};
use tokio::sync::{mpsc, oneshot};

#[derive(Debug, Clone)]
pub struct Config {
    pub addr: SocketAddr,
    /// `third_party/tbamud/content`
    pub content: PathBuf,
    /// `third_party/tbamud/locales`
    pub locales: PathBuf,
    /// `third_party/tbamud/tables` (D21)
    pub tables: PathBuf,
    /// Where the database and the input logs go.
    pub data: PathBuf,
    pub seed: u64,
    pub hour: u32,
}

/// Places are saved this often, besides on quit and at shutdown.
const SAVE_EVERY: u64 = 10 * PULSES_PER_SEC;

struct Conn {
    viewer: Viewer,
    plain: bool,
    /// The character, once logged in.
    name: Option<String>,
    seq: u64,
}

struct LoginResult {
    conn: ConnId,
    outcome: Result<Option<Player>, String>,
}

/// A logged-in character as the store knows it.
struct Player {
    name: String,
    room: Option<String>,
    sex: Sex,
    ko_final: Option<KoFinal>,
}

fn named<T: serde::de::DeserializeOwned>(s: Option<String>) -> Option<T> {
    s.and_then(|s| serde_json::from_value(serde_json::Value::String(s)).ok())
}

fn name_of<T: serde::Serialize>(v: Option<T>) -> Option<String> {
    v.and_then(|v| serde_json::to_value(v).ok()).and_then(|j| j.as_str().map(str::to_string))
}

/// Runs until `shutdown` fires. `ready` gets the bound address once the world is loaded.
pub async fn run(cfg: Config, shutdown: oneshot::Receiver<()>, ready: oneshot::Sender<SocketAddr>) -> Result<(), String> {
    let zones = load_world(&cfg.content).map_err(|e| e.to_string())?;

    std::fs::create_dir_all(&cfg.data).map_err(|e| e.to_string())?;
    let db = cfg.data.join("mundi.db");
    let mut store = Store::open(&db).map_err(|e| e.to_string())?;
    let tables = load_tables(&cfg.tables).map_err(|e| e.to_string())?;
    let mut sim = Sim::new(&zones, &tables, cfg.seed, cfg.hour);
    let renderer = Renderer::load(&cfg.locales, &zones)?;
    drop(zones);

    let started = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let log_path = cfg.data.join(format!("inputs-{started}.jsonl"));
    let mut log = std::fs::File::create(&log_path).map_err(|e| e.to_string())?;
    writeln!(log, "{}", serde_json::json!({"seed": cfg.seed, "hour": cfg.hour, "content": cfg.content, "tables": cfg.tables})).map_err(|e| e.to_string())?;
    let mut logged = 0;

    let (addr, net, mut incoming) = mundi_net::listen(cfg.addr).await.map_err(|e| e.to_string())?;
    let _ = ready.send(addr);
    let (login_tx, mut logins) = mpsc::unbounded_channel::<LoginResult>();
    let mut conns: HashMap<ConnId, Conn> = HashMap::new();
    let mut by_char: HashMap<String, ConnId> = HashMap::new();
    let mut pulse = tokio::time::interval(Duration::from_millis(1000 / PULSES_PER_SEC));
    let mut shutdown = shutdown;

    loop {
        tokio::select! {
            _ = &mut shutdown => break,
            _ = pulse.tick() => {}
        }
        while let Ok(e) = incoming.try_recv() {
            match e {
                NetEvent::Connected(c) => {
                    conns.insert(c, Conn { viewer: Viewer::default(), plain: false, name: None, seq: 0 });
                    send(&net, &renderer, &mut conns, c, sim.tick(), Event::LoginPrompt { stage: LoginStage::Name });
                }
                NetEvent::Message(c, ClientMessage::Login { name, password, lang, plain, keywords, sex, ko_final }) => {
                    let Some(conn) = conns.get_mut(&c) else { continue };
                    if conn.name.is_some() {
                        continue;
                    }
                    conn.viewer = Viewer { lang, keywords };
                    conn.plain = plain;
                    if !valid_name(&name) {
                        send(&net, &renderer, &mut conns, c, sim.tick(), Event::LoginFailed { reason: LoginFailure::InvalidName });
                        continue;
                    }
                    let (db, tx) = (db.clone(), login_tx.clone());
                    tokio::task::spawn_blocking(move || {
                        let outcome = (|| {
                            let s = Store::open(&db).map_err(|e| e.to_string())?;
                            let e = |e: mundi_store::Error| e.to_string();
                            Ok(match s.login(&name, &password).map_err(e)? {
                                Login::WrongPassword => None,
                                created @ (Login::Ok | Login::Created) => {
                                    if created == Login::Created {
                                        s.set_profile(&name, name_of(sex).as_deref(), name_of(ko_final).as_deref()).map_err(e)?;
                                    }
                                    let display = s.display(&name).map_err(e)?.unwrap_or(name);
                                    let room = s.room(&display).map_err(e)?;
                                    let (sex, ko_final) = s.profile(&display).map_err(e)?;
                                    Some(Player { name: display, room, sex: named(sex).unwrap_or_default(), ko_final: named(ko_final) })
                                }
                            })
                        })();
                        let _ = tx.send(LoginResult { conn: c, outcome });
                    });
                }
                NetEvent::Message(c, ClientMessage::Command { text }) => {
                    if let Some(name) = conns.get(&c).and_then(|k| k.name.clone()) {
                        sim.submit(Input::Command { name, text });
                    }
                }
                NetEvent::Message(c, ClientMessage::Settings { keywords, lang, plain }) => {
                    if let Some(conn) = conns.get_mut(&c) {
                        conn.viewer.keywords = keywords.unwrap_or(conn.viewer.keywords);
                        conn.viewer.lang = lang.unwrap_or(conn.viewer.lang);
                        conn.plain = plain.unwrap_or(conn.plain);
                    }
                }
                NetEvent::Malformed(_) => {}
                NetEvent::Disconnected(c) => {
                    if let Some(name) = conns.remove(&c).and_then(|k| k.name) {
                        if by_char.get(&name.to_lowercase()) == Some(&c) {
                            by_char.remove(&name.to_lowercase());
                            sim.submit(Input::LinkLost { name });
                        }
                    }
                }
            }
        }
        while let Ok(LoginResult { conn: c, outcome }) = logins.try_recv() {
            match outcome {
                Ok(Some(Player { name, room, sex, ko_final })) if conns.contains_key(&c) => {
                    renderer.register_player(&name, sex, ko_final);
                    // A second login takes the character over: the old connection goes.
                    if let Some(old) = by_char.insert(name.to_lowercase(), c) {
                        if let Some(k) = conns.get_mut(&old) {
                            k.name = None;
                        }
                        net.close(old);
                    }
                    conns.get_mut(&c).unwrap().name = Some(name.clone());
                    let room = sim.room_of(&name).map(str::to_string).or(room).filter(|r| sim.has_room(r));
                    sim.submit(Input::Enter { name, room });
                }
                Ok(None) => send(&net, &renderer, &mut conns, c, sim.tick(), Event::LoginFailed { reason: LoginFailure::WrongPassword }),
                Ok(Some(_)) => {}
                Err(e) => {
                    eprintln!("login: {e}");
                    net.close(c);
                }
            }
        }

        for d in sim.step() {
            let Some(&c) = by_char.get(&d.to.to_lowercase()) else { continue };
            let closing = matches!(d.event, Event::Closed { .. });
            send(&net, &renderer, &mut conns, c, d.tick, d.event);
            if closing {
                by_char.remove(&d.to.to_lowercase());
                conns.remove(&c);
                net.close(c);
            }
        }
        let gone = sim.take_departures();
        if !gone.is_empty() {
            let _ = store.save_rooms(gone.iter().map(|g| (g.name.as_str(), g.room.as_str())));
        }
        if sim.tick() % SAVE_EVERY == 0 {
            save_all(&mut store, &sim);
        }
        let entries = &sim.input_log()[logged..];
        for e in entries {
            let _ = writeln!(log, "{}", serde_json::to_string(e).unwrap());
        }
        logged += entries.len();
    }
    save_all(&mut store, &sim);
    let _ = log.flush();
    Ok(())
}

fn save_all(store: &mut Store, sim: &Sim) {
    let places = sim.places();
    if let Err(e) = store.save_rooms(places.iter().map(|(n, r)| (n.as_str(), r.as_str()))) {
        eprintln!("save: {e}");
    }
}

/// Renders an event for one connection and sends it as an envelope.
fn send(net: &Net, renderer: &Renderer, conns: &mut HashMap<ConnId, Conn>, c: ConnId, tick: u64, mut event: Event) {
    let Some(conn) = conns.get_mut(&c) else { return };
    let text = renderer
        .lines(&event, conn.viewer)
        .iter()
        .map(|l| if conn.plain { plain(l) } else { ansi(l) })
        .collect();
    renderer.fill(&mut event);
    conn.seq += 1;
    let envelope = Envelope {
        v: VERSION,
        t: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64(),
        tick,
        seq: conn.seq,
        agent: conn.name.clone().unwrap_or_default(),
        event,
        text,
    };
    net.send(c, serde_json::to_string(&envelope).unwrap());
}
