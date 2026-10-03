//! S6's telnet gateway, end to end: a MUD client's dialogue, then the world's lines in colour.

use std::path::Path;
use std::time::Duration;

use mundi_server::{Config, run};
use mundi_telnet::{Gateway, serve};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;

async fn read_until(s: &mut TcpStream, want: &str) -> String {
    String::from_utf8_lossy(&read_bytes(s, want).await).into_owned()
}

async fn read_bytes(s: &mut TcpStream, want: &str) -> Vec<u8> {
    let mut got = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while !String::from_utf8_lossy(&got).contains(want) {
        let mut buf = [0u8; 4096];
        let n = tokio::time::timeout_at(deadline, s.read(&mut buf)).await.unwrap_or_else(|_| panic!("no {want:?} in {:?}", String::from_utf8_lossy(&got))).unwrap();
        assert!(n > 0, "closed before {want:?}: {:?}", String::from_utf8_lossy(&got));
        got.extend_from_slice(&buf[..n]);
    }
    got
}

#[tokio::test(flavor = "multi_thread")]
async fn a_person_with_a_telnet_client() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let tmp = tempfile::tempdir().unwrap();
    let content = tmp.path().join("content");
    std::fs::create_dir(&content).unwrap();
    std::os::unix::fs::symlink(root.join("third_party/tbamud/content/30").canonicalize().unwrap(), content.join("30")).unwrap();
    let cfg = Config {
        addr: "127.0.0.1:0".parse().unwrap(),
        content,
        locales: root.join("third_party/tbamud/locales"),
        tables: root.join("third_party/tbamud/tables"),
        data: tmp.path().join("data"),
        seed: 7,
        hour: 12,
    };
    let (stop, shutdown) = oneshot::channel();
    let (ready, bound) = oneshot::channel();
    let server = tokio::spawn(run(cfg, shutdown, ready));
    let addr = bound.await.unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let telnet = listener.local_addr().unwrap();
    tokio::spawn(serve(listener, Gateway { mundi: format!("ws://{addr}"), lang: "en".into() }));

    let mut s = TcpStream::connect(telnet).await.unwrap();
    read_until(&mut s, "known? ").await;
    s.write_all(b"Tella\r\n").await.unwrap();
    let pw = read_bytes(&mut s, "Password: ").await;
    assert!(pw.windows(3).any(|w| w == [255, 251, 1]), "echo off for the password");
    s.write_all(b"secret one\r\n").await.unwrap();
    read_until(&mut s, "new character").await;
    s.write_all(b"y\r\n").await.unwrap();
    read_until(&mut s, "Class").await;
    s.write_all(b"c\r\n").await.unwrap();
    read_until(&mut s, "Sex").await;
    s.write_all(b"f\r\n").await.unwrap();
    let world = read_until(&mut s, "H ").await;
    assert!(world.contains("\x1b["), "colour: {world:?}");
    assert!(world.contains("Temple"), "the room: {world:?}");
    s.write_all(b"score\r\n").await.unwrap();
    let score = read_until(&mut s, "(level 1)").await;
    assert!(score.contains("This ranks you as Tella the Believer (level 1)."), "{score:?}");
    s.write_all(b"quit\r\n").await.unwrap();
    let _ = stop.send(());
    server.await.unwrap().unwrap();
}
