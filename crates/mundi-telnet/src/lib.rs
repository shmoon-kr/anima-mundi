//! A telnet gateway (D5): the engine opens one WebSocket; people with a MUD client come in here.
//! Each telnet connection is a WebSocket client of Mundi like any other: a login dialogue in the
//! old style, then each line typed is a command and each event's rendered lines are printed, in
//! colour, the prompt left open at the end of its line. The gateway knows only the wire protocol.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;

const IAC: u8 = 255;
const WILL: u8 = 251;
const WONT: u8 = 252;
const SB: u8 = 250;
const SE: u8 = 240;
const ECHO: u8 = 1;

/// Settings every connection of this gateway gets.
#[derive(Debug, Clone)]
pub struct Gateway {
    /// Mundi's WebSocket, e.g. `ws://127.0.0.1:4000`.
    pub mundi: String,
    /// `en` or `ko`.
    pub lang: String,
}

/// Accepts telnet connections until the listener fails.
pub async fn serve(listener: TcpListener, gw: Gateway) -> std::io::Result<()> {
    loop {
        let (stream, _) = listener.accept().await?;
        let gw = gw.clone();
        tokio::spawn(async move {
            let _ = session(stream, gw).await;
        });
    }
}

/// Lines from a telnet client: IAC sequences dropped, CR LF or LF ends a line.
struct Lines {
    stream: tokio::net::tcp::OwnedReadHalf,
    buf: Vec<u8>,
    pending: Vec<u8>,
}

impl Lines {
    async fn next(&mut self) -> Option<String> {
        loop {
            if let Some(i) = self.buf.iter().position(|b| *b == b'\n') {
                let mut line: Vec<u8> = self.buf.drain(..=i).collect();
                line.pop();
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                return Some(String::from_utf8_lossy(&line).into_owned());
            }
            let mut chunk = [0u8; 1024];
            let n = self.stream.read(&mut chunk).await.ok()?;
            if n == 0 {
                return None;
            }
            self.pending.extend_from_slice(&chunk[..n]);
            self.strip();
        }
    }

    /// Moves what is not telnet negotiation from `pending` to `buf`; keeps an unfinished sequence.
    fn strip(&mut self) {
        let p = std::mem::take(&mut self.pending);
        let mut i = 0;
        while i < p.len() {
            if p[i] != IAC {
                self.buf.push(p[i]);
                i += 1;
                continue;
            }
            match p.get(i + 1) {
                None => {
                    self.pending = p[i..].to_vec();
                    return;
                }
                Some(&IAC) => {
                    self.buf.push(IAC);
                    i += 2;
                }
                Some(&SB) => match p[i..].windows(2).position(|w| w == [IAC, SE]) {
                    Some(end) => i += end + 2,
                    None => {
                        self.pending = p[i..].to_vec();
                        return;
                    }
                },
                // WILL, WONT, DO, DONT and their option
                Some(&c) if c >= WILL => {
                    if p.get(i + 2).is_none() {
                        self.pending = p[i..].to_vec();
                        return;
                    }
                    i += 3;
                }
                Some(_) => i += 2,
            }
        }
    }
}

async fn ask(out: &mut tokio::net::tcp::OwnedWriteHalf, lines: &mut Lines, q: &str) -> Option<String> {
    out.write_all(q.as_bytes()).await.ok()?;
    lines.next().await.map(|l| l.trim().to_string())
}

async fn session(stream: TcpStream, gw: Gateway) -> std::io::Result<()> {
    let (read, mut out) = stream.into_split();
    let mut lines = Lines { stream: read, buf: Vec::new(), pending: Vec::new() };
    out.write_all(b"\r\n   Anima Mundi\r\n\r\n").await?;
    let Some(name) = ask(&mut out, &mut lines, "By what name do you wish to be known? ").await else { return Ok(()) };
    out.write_all(&[IAC, WILL, ECHO]).await?;
    let password = ask(&mut out, &mut lines, "Password: ").await.unwrap_or_default();
    out.write_all(&[IAC, WONT, ECHO]).await?;
    out.write_all(b"\r\n").await?;
    let mut login = json!({"type": "login", "name": name, "password": password, "lang": gw.lang, "plain": false});
    let new = ask(&mut out, &mut lines, "Is this a new character? (y/N) ").await.unwrap_or_default();
    if new.to_lowercase().starts_with('y') {
        let class = ask(&mut out, &mut lines, "Class: (M)agic user, (C)leric, (T)hief, (W)arrior? ").await.unwrap_or_default();
        let class = match class.to_lowercase().chars().next() {
            Some('m') => "magic_user",
            Some('c') => "cleric",
            Some('t') => "thief",
            _ => "warrior",
        };
        let sex = ask(&mut out, &mut lines, "Sex: (M)ale, (F)emale, (N)either? ").await.unwrap_or_default();
        let sex = match sex.to_lowercase().chars().next() {
            Some('m') => "male",
            Some('f') => "female",
            _ => "neutral",
        };
        login["class"] = json!(class);
        login["sex"] = json!(sex);
    }
    let Ok((ws, _)) = tokio_tungstenite::connect_async(gw.mundi.as_str()).await else {
        out.write_all(b"The world is not there right now.\r\n").await?;
        return Ok(());
    };
    let (mut ws_out, mut ws_in) = ws.split();
    let mut logged_in = false;
    let mut after_prompt = false;
    loop {
        tokio::select! {
            frame = ws_in.next() => {
                let Some(Ok(frame)) = frame else { break };
                let Message::Text(text) = frame else { continue };
                let Ok(env) = serde_json::from_str::<Value>(text.as_str()) else { continue };
                let kind = env["type"].as_str().unwrap_or("");
                if kind == "connection.login_prompt" && !logged_in {
                    logged_in = true;
                    let _ = ws_out.send(Message::text(login.to_string())).await;
                    continue;
                }
                let shown: Vec<&str> = env["text"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
                if shown.is_empty() {
                    continue;
                }
                let mut bytes = String::new();
                if after_prompt {
                    bytes.push_str("\r\n");
                }
                if kind == "prompt" {
                    bytes.push_str(&shown.join(" "));
                    after_prompt = true;
                } else {
                    for l in &shown {
                        bytes.push_str(l);
                        bytes.push_str("\r\n");
                    }
                    after_prompt = false;
                }
                out.write_all(bytes.as_bytes()).await?;
                if kind == "connection.closed" || kind == "connection.login_failed" {
                    break;
                }
            }
            line = lines.next() => {
                let Some(line) = line else { break };
                after_prompt = false;
                let msg = json!({"type": "command", "text": line});
                if ws_out.send(Message::text(msg.to_string())).await.is_err() {
                    break;
                }
            }
        }
    }
    let _ = tokio::time::timeout(Duration::from_secs(1), ws_out.close()).await;
    Ok(())
}
