//! The world as content: the types of the content format (zones, rooms, mobs, objects, shops, resets),
//! loading them from YAML, validating them, and merging per-language translation files (D10, D11).
//!
//! The types are the schema: the converter writes them and the loader reads them, so the two cannot
//! disagree (D15). Content has no behaviour and knows nothing of the simulation.

pub mod load;
pub mod locale;
pub mod markup;
pub mod model;
pub mod names;
pub mod tables;

pub use load::{check_world, load_world, load_zone, parse_id, write_zone, Report};
pub use locale::{load_locale, Locale};
pub use model::*;
pub use tables::{load_messages, load_tables, load_trigger_lines, CombatMessages, Tables, TriggerLines};
