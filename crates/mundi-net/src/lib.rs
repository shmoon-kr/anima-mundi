//! The network: the WebSocket server speaking the engine contract (per-recipient events plus rendered
//! text, commands in), login with the language the client asks for, and a simple telnet gateway.
//!
//! It sends what it is given (protocol events and rendered text); it does not depend on the
//! simulation or the store. Each connection is a number; what arrives is a [`NetEvent`], what goes
//! out is a text frame or a close, sent through [`Net`].

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use mundi_protocol::ClientMessage;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

pub type ConnId = u64;

#[derive(Debug)]
pub enum NetEvent {
    Connected(ConnId),
    Message(ConnId, ClientMessage),
    /// A frame that is not a client message. Its text is not kept: it may be a mistyped password.
    Malformed(ConnId),
    Disconnected(ConnId),
}

enum Out {
    Text(String),
    Close,
}

/// The sending side, cheap to clone.
#[derive(Clone, Default)]
pub struct Net {
    conns: Arc<Mutex<HashMap<ConnId, mpsc::UnboundedSender<Out>>>>,
}

impl Net {
    pub fn send(&self, conn: ConnId, text: String) {
        if let Some(tx) = self.conns.lock().unwrap().get(&conn) {
            let _ = tx.send(Out::Text(text));
        }
    }

    /// Closes after everything already sent.
    pub fn close(&self, conn: ConnId) {
        if let Some(tx) = self.conns.lock().unwrap().remove(&conn) {
            let _ = tx.send(Out::Close);
        }
    }
}

/// Listens on `addr` (port 0 picks one) and returns the bound address, the sending side, and the
/// stream of what arrives.
pub async fn listen(addr: SocketAddr) -> std::io::Result<(SocketAddr, Net, mpsc::UnboundedReceiver<NetEvent>)> {
    let listener = TcpListener::bind(addr).await?;
    let bound = listener.local_addr()?;
    let net = Net::default();
    let (events, rx) = mpsc::unbounded_channel();
    let accept_net = net.clone();
    tokio::spawn(async move {
        let mut next: ConnId = 0;
        while let Ok((stream, _)) = listener.accept().await {
            next += 1;
            let id = next;
            let (net, events) = (accept_net.clone(), events.clone());
            tokio::spawn(async move {
                let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
                let (mut sink, mut source) = ws.split();
                let (tx, mut out) = mpsc::unbounded_channel();
                net.conns.lock().unwrap().insert(id, tx);
                let _ = events.send(NetEvent::Connected(id));
                let writer = tokio::spawn(async move {
                    while let Some(o) = out.recv().await {
                        match o {
                            Out::Text(t) => {
                                if sink.send(Message::text(t)).await.is_err() {
                                    break;
                                }
                            }
                            Out::Close => {
                                let _ = sink.close().await;
                                break;
                            }
                        }
                    }
                });
                while let Some(Ok(msg)) = source.next().await {
                    match msg {
                        Message::Text(t) => {
                            let e = match serde_json::from_str::<ClientMessage>(&t) {
                                Ok(m) => NetEvent::Message(id, m),
                                Err(_) => NetEvent::Malformed(id),
                            };
                            let _ = events.send(e);
                        }
                        Message::Close(_) => break,
                        _ => {}
                    }
                }
                net.conns.lock().unwrap().remove(&id);
                let _ = events.send(NetEvent::Disconnected(id));
                let _ = writer.await;
            });
        }
    });
    Ok((bound, net, rx))
}
