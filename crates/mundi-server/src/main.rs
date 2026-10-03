//! `mundi-server`: wires the crates together. The only crate that knows all of them.
//!
//! Usage: mundi-server [--addr 127.0.0.1:4000] [--data data] [--content third_party/tbamud/content]
//!                     [--locales third_party/tbamud/locales] [--tables third_party/tbamud/tables] [--seed N] [--hour H] [--speed N]
//! Clients speak WebSocket: one JSON message per frame (`{"type":"login",...}`, `{"type":"command","text":"look"}`),
//! and receive one envelope per frame. Ctrl-C saves and stops.

use std::path::PathBuf;
use std::process::ExitCode;

use mundi_server::{run, Config};

#[tokio::main]
async fn main() -> ExitCode {
    let mut cfg = Config {
        addr: "127.0.0.1:4000".parse().unwrap(),
        content: PathBuf::from("third_party/tbamud/content"),
        locales: PathBuf::from("third_party/tbamud/locales"),
        tables: PathBuf::from("third_party/tbamud/tables"),
        data: PathBuf::from("data"),
        seed: 1,
        hour: 12,
        speed: 1,
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    for pair in args.chunks(2) {
        let [flag, value] = pair else {
            eprintln!("{} needs a value", pair[0]);
            return ExitCode::from(2);
        };
        let ok = match flag.as_str() {
            "--addr" => value.parse().map(|a| cfg.addr = a).is_ok(),
            "--data" => {
                cfg.data = value.into();
                true
            }
            "--content" => {
                cfg.content = value.into();
                true
            }
            "--tables" => {
                cfg.tables = value.into();
                true
            }
            "--locales" => {
                cfg.locales = value.into();
                true
            }
            "--seed" => value.parse().map(|s| cfg.seed = s).is_ok(),
            "--hour" => value.parse().map(|h| cfg.hour = h).is_ok(),
            "--speed" => value.parse().map(|n: u32| cfg.speed = n.max(1)).is_ok(),
            _ => false,
        };
        if !ok {
            eprintln!("bad argument: {flag} {value}");
            return ExitCode::from(2);
        }
    }
    let (stop, shutdown) = tokio::sync::oneshot::channel();
    let (ready, bound) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        let _ = stop.send(());
    });
    tokio::spawn(async move {
        if let Ok(addr) = bound.await {
            eprintln!("mundi-server: listening on ws://{addr}");
        }
    });
    match run(cfg, shutdown, ready).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("mundi-server: {e}");
            ExitCode::FAILURE
        }
    }
}
