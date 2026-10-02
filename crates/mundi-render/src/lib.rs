//! Rendering: a recipient's already filtered event to a sentence in that recipient's language and
//! point of view (actor, target, others), with Korean particles, English pronouns and character widths.
//!
//! The only place in the engine where sentences are made. It sees events only through `mundi-protocol`
//! and does not depend on the simulation, so it cannot reveal what perception filtered out.
