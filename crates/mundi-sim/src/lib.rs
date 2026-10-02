//! The simulation: world state, entities and IDs, the tick, the one deterministic RNG, commands to
//! actions, the rules (docs/MECHANICS.md), and the events they cause.
//!
//! Perception lives here (D13): who can perceive an event, and what each of them perceives (light,
//! position, place, blindness, invisibility), is a fact of the simulation. Events leave this crate
//! already filtered per recipient, so an agent never learns more than a person in its place.
//!
//! This crate writes no sentence and opens no socket or database: it does not depend on the renderer,
//! the network or the store (checked by `mundi-server/tests/architecture.rs`).
