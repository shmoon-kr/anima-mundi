//! The network: the WebSocket server speaking the engine contract (per-recipient events plus rendered
//! text, commands in), login with the language the client asks for, and a simple telnet gateway.
//!
//! It sends what it is given (protocol events and rendered text); it does not depend on the
//! simulation or the store.
