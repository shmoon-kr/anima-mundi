//! The engine contract as Rust types: events, commands and the envelope of anima `PROTOCOL.md` part 1
//! (v0, plus the additions of D14: `id` on every entity, rendered text, exact numbers such as `damage`,
//! and the language a client asks for).
//!
//! Every other crate that moves events depends on this one, and this one depends on no other Mundi crate:
//! the simulation produces these types, the renderer and the network only consume them.
