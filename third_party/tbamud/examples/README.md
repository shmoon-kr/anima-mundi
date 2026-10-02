# Content format example (PHASE-1-PLAN S2)

tbaMUD zone 186 (Newbie Zone) in the proposed format, for review before the converter is written.
Made by `tools/prototype_convert.py`; the Rust converter (`mundi-convert`) must produce the same content.

- `content/186/` — `zone.yaml`, `rooms.yaml`, `mobs.yaml`, `objects.yaml`, `resets.yaml` (English, the base)
- `locales/ko/186.yaml` — a Korean overlay with a few hand-translated entries (same IDs, only translatable strings)

Conventions: IDs `tba:<zone>:<kind>:<vnum>` (D16); flags and values by name; prose folded (`>`), one paragraph per
blank line, wrapped by the renderer per language (verbatim `|` for maps and art); door reset states on the doors;
entries in vnum order with a fixed key order, so a change to one room is one small diff.
