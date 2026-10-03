//! S3's check, end to end over WebSocket: two clients see each other's moves and words, each from
//! their own side; after a restart a character is where it left; agents get plain text, people colour.

use std::path::Path;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use mundi_server::{run, Config};
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

struct Server {
    stop: oneshot::Sender<()>,
    done: tokio::task::JoinHandle<Result<(), String>>,
    url: String,
}

async fn start(content: &Path, data: &Path) -> Server {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let cfg = Config {
        addr: "127.0.0.1:0".parse().unwrap(),
        content: content.to_path_buf(),
        locales: root.join("third_party/tbamud/locales"),
        tables: root.join("third_party/tbamud/tables"),
        data: data.to_path_buf(),
        seed: 7,
        hour: 12,
    };
    let (stop, shutdown) = oneshot::channel();
    let (ready, bound) = oneshot::channel();
    let done = tokio::spawn(run(cfg, shutdown, ready));
    let addr = bound.await.expect("server starts");
    Server { stop, done, url: format!("ws://{addr}") }
}

impl Server {
    async fn stop(self) {
        let _ = self.stop.send(());
        self.done.await.unwrap().unwrap();
    }
}

async fn login(url: &str, name: &str, password: &str, plain: bool) -> Ws {
    login_as(url, json!({"type": "login", "name": name, "password": password, "plain": plain})).await
}

async fn login_as(url: &str, msg: Value) -> Ws {
    let (mut ws, _) = tokio_tungstenite::connect_async(url).await.unwrap();
    until(&mut ws, |e| e["type"] == "connection.login_prompt").await;
    ws.send(Message::text(msg.to_string())).await.unwrap();
    ws
}

async fn command(ws: &mut Ws, text: &str) {
    ws.send(Message::text(json!({"type": "command", "text": text}).to_string())).await.unwrap();
}

/// Reads envelopes until one matches, and returns it.
async fn until(ws: &mut Ws, want: impl Fn(&Value) -> bool) -> Value {
    let read = async {
        loop {
            match ws.next().await {
                Some(Ok(Message::Text(t))) => {
                    let e: Value = serde_json::from_str(&t).unwrap();
                    if want(&e) {
                        return e;
                    }
                }
                Some(Ok(_)) => {}
                other => panic!("connection ended: {other:?}"),
            }
        }
    };
    tokio::time::timeout(Duration::from_secs(10), read).await.expect("the expected event arrives")
}

fn text_is(e: &Value, s: &str) -> bool {
    e["text"].as_array().is_some_and(|t| t.iter().any(|l| l == s))
}

/// Midgaard only: the start room is there and it loads fast.
fn midgaard(tmp: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let content = tmp.join("content");
    std::fs::create_dir(&content).unwrap();
    let midgaard = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/tbamud/content/30");
    std::os::unix::fs::symlink(midgaard.canonicalize().unwrap(), content.join("30")).unwrap();
    (content, tmp.join("data"))
}

#[tokio::test(flavor = "multi_thread")]
async fn one_scene_in_english_and_korean_at_once() {
    let tmp = tempfile::tempdir().unwrap();
    let (content, data) = midgaard(tmp.path());
    let server = start(&content, &data).await;
    let mut ana = login(&server.url, "Ana", "pw-a", true).await;
    until(&mut ana, |e| e["type"] == "room").await;
    let mut min = login_as(&server.url, json!({"type": "login", "name": "Min", "password": "pw-m", "plain": true, "lang": "ko", "sex": "female"})).await;
    until(&mut ana, |e| text_is(e, "Min has entered the game.")).await;
    let room = until(&mut min, |e| e["type"] == "room").await;
    let text: Vec<&str> = room["text"].as_array().unwrap().iter().map(|l| l.as_str().unwrap()).collect();
    assert_eq!(text[0], "미드가르드 신전");
    let exits = text.iter().find(|l| l.starts_with("[ 출구: ")).expect("a Korean exits line");
    assert!(exits.contains("북(n)") || exits.contains("동(e)") || exits.contains("남(s)"), "{exits}");
    assert!(text.contains(&"Ana가 여기 서 있다."), "{text:?}");
    assert_eq!(room["data"]["name"], "The Temple Of Midgaard", "the event stays the world's, only the text is Korean");

    command(&mut ana, "say hello").await;
    until(&mut ana, |e| text_is(e, "You say, 'hello'")).await;
    until(&mut min, |e| text_is(e, "Ana가 말한다, 'hello'")).await;
    command(&mut min, "say 안녕").await;
    until(&mut min, |e| text_is(e, "당신은 말한다, '안녕'")).await;
    until(&mut ana, |e| text_is(e, "Min says, '안녕'")).await;

    // Keywords off: the same exits, without what to type.
    min.send(Message::text(json!({"type": "settings", "keywords": "off"}).to_string())).await.unwrap();
    command(&mut min, "look").await;
    let room = until(&mut min, |e| e["type"] == "room").await;
    let exits = room["text"].as_array().unwrap().iter().find(|l| l.as_str().unwrap().starts_with("[ 출구")).unwrap().clone();
    assert!(!exits.as_str().unwrap().contains('('), "{exits}");

    // Ana's link drops; the English pronoun follows her account (no sex given: "its"), Min's text is Korean.
    drop(ana);
    until(&mut min, |e| text_is(e, "Ana의 연결이 끊겼다.")).await;
    server.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn two_clients_see_each_other_and_places_survive_a_restart() {
    let tmp = tempfile::tempdir().unwrap();
    let (content, data) = midgaard(tmp.path());

    let server = start(&content, &data).await;
    let mut ana = login(&server.url, "ana", "first-pw", false).await;
    let room = until(&mut ana, |e| e["type"] == "room").await;
    assert_eq!(room["data"]["id"], "tba:30:room:3001");
    assert!(room["text"][0].as_str().unwrap().starts_with("\x1b[33m"), "people get colour");

    let mut bo = login(&server.url, "Bo", "bo-pw", true).await;
    until(&mut ana, |e| text_is(e, "Bo has entered the game.")).await;
    let seen = until(&mut bo, |e| e["type"] == "room").await;
    assert_eq!(seen["data"]["occupants"][0]["text"], "Ana is standing here.", "the screen line is filled (D22)");
    assert_eq!(seen["data"]["occupants"][0]["name"], "Ana");
    assert!(!seen["text"].to_string().contains("\\u001b"), "agents get plain text");

    command(&mut ana, "say hello there").await;
    until(&mut ana, |e| text_is(e, "You say, 'hello there'")).await;
    let heard = until(&mut bo, |e| e["type"] == "comm.say").await;
    assert_eq!(heard["data"]["from"], "Ana");
    assert!(text_is(&heard, "Ana says, 'hello there'"));

    let exit = room["data"]["exits"][0].clone();
    let dir = exit["dir"].as_str().unwrap();
    command(&mut ana, &dir[..1]).await;
    until(&mut bo, |e| text_is(e, &format!("Ana leaves {dir}."))).await;
    let moved = until(&mut ana, |e| e["type"] == "room").await;
    assert_eq!(moved["data"]["id"], exit["to_id"]);

    command(&mut ana, "quit").await;
    until(&mut ana, |e| text_is(e, "Goodbye, friend.. Come back soon!")).await;
    server.stop().await;

    // The same data directory: Ana is where she quit; a wrong password is refused.
    let server = start(&content, &data).await;
    let mut wrong = login(&server.url, "Ana", "nope", true).await;
    until(&mut wrong, |e| e["type"] == "connection.login_failed" && e["data"]["reason"] == "wrong_password").await;
    let mut ana = login(&server.url, "Ana", "first-pw", true).await;
    let back = until(&mut ana, |e| e["type"] == "room").await;
    assert_eq!(back["data"]["id"], exit["to_id"]);
    server.stop().await;

    // Nothing kept the passwords.
    for entry in std::fs::read_dir(&data).unwrap() {
        let bytes = std::fs::read(entry.unwrap().path()).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        assert!(!text.contains("first-pw") && !text.contains("bo-pw"));
    }
}
