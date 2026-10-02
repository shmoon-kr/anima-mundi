//! The world as content: the types of the content format (zones, rooms, mobs, objects, shops, resets),
//! loading them from YAML, validating them, and merging per-language translation files (D10, D11).
//!
//! The types are the schema: the converter writes them and the loader reads them, so the two cannot
//! disagree (D15). Content has no behaviour and knows nothing of the simulation.
