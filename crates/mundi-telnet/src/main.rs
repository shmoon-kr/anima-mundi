//! Usage: mundi-telnet [--listen 127.0.0.1:4001] [--mundi ws://127.0.0.1:4000] [--lang en|ko]

use mundi_telnet::{Gateway, serve};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (mut listen, mut gw) = ("127.0.0.1:4001".to_string(), Gateway { mundi: "ws://127.0.0.1:4000".into(), lang: "en".into() });
    let mut i = 0;
    while i < args.len() {
        let value = args.get(i + 1).cloned();
        match (args[i].as_str(), value) {
            ("--listen", Some(v)) => listen = v,
            ("--mundi", Some(v)) => gw.mundi = v,
            ("--lang", Some(v)) => gw.lang = v,
            _ => {
                eprintln!("usage: mundi-telnet [--listen ADDR] [--mundi ws://ADDR] [--lang en|ko]");
                std::process::exit(2);
            }
        }
        i += 2;
    }
    let listener = match tokio::net::TcpListener::bind(&listen).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("mundi-telnet: {listen}: {e}");
            std::process::exit(1);
        }
    };
    println!("mundi-telnet: telnet {listen} -> {}", gw.mundi);
    tokio::select! {
        r = serve(listener, gw) => if let Err(e) = r { eprintln!("mundi-telnet: {e}") },
        _ = tokio::signal::ctrl_c() => {}
    }
}
