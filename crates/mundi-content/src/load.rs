//! Reading and writing zones, and checking a whole world.
//!
//! Writing is deterministic (entries in the order given, which the converter makes vnum order; fixed
//! key order from the types), so changing one room changes only that room's lines.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::model::*;
use crate::names::{Dir, ItemType};

pub const FILES: [&str; 6] = ["zone.yaml", "rooms.yaml", "mobs.yaml", "objects.yaml", "resets.yaml", "shops.yaml"];

#[derive(Debug)]
pub struct LoadError {
    pub file: PathBuf,
    pub message: String,
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.file.display(), self.message)
    }
}

fn read<T: DeserializeOwned + Default>(path: &Path) -> Result<T, LoadError> {
    match fs::read_to_string(path) {
        Ok(text) => serde_saphyr::from_str(&text).map_err(|e| LoadError { file: path.into(), message: e.to_string() }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
        Err(e) => Err(LoadError { file: path.into(), message: e.to_string() }),
    }
}

/// One zone directory. Missing files are empty.
pub fn load_zone(dir: &Path) -> Result<ZoneContent, LoadError> {
    let zone_path = dir.join("zone.yaml");
    let zone: Zone = serde_saphyr::from_str(
        &fs::read_to_string(&zone_path).map_err(|e| LoadError { file: zone_path.clone(), message: e.to_string() })?,
    )
    .map_err(|e| LoadError { file: zone_path, message: e.to_string() })?;
    Ok(ZoneContent {
        zone: Some(zone),
        rooms: read(&dir.join("rooms.yaml"))?,
        mobs: read(&dir.join("mobs.yaml"))?,
        objects: read(&dir.join("objects.yaml"))?,
        resets: read(&dir.join("resets.yaml"))?,
        shops: read(&dir.join("shops.yaml"))?,
    })
}

/// Every zone directory under `root` (a directory holding zone.yaml), in name order.
pub fn load_world(root: &Path) -> Result<Vec<ZoneContent>, LoadError> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(root)
        .map_err(|e| LoadError { file: root.into(), message: e.to_string() })?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join("zone.yaml").exists())
        .collect();
    dirs.sort_by_key(|p| p.file_name().and_then(|n| n.to_str()).and_then(|n| n.parse::<u32>().ok()).unwrap_or(u32::MAX));
    dirs.iter().map(|d| load_zone(d)).collect()
}

// ---------------------------------------------------------------- writing

pub fn yaml<T: Serialize>(value: &T) -> String {
    let mut opts = serde_saphyr::SerializerOptions::default();
    opts.folded_wrap_chars = 100;
    opts.min_fold_chars = 90;
    serde_saphyr::to_string_with_options(value, opts).expect("content always serializes")
}

/// A blank line between top-level entries, for reading and for diffs.
fn spaced(text: String) -> String {
    let mut out = String::with_capacity(text.len() + text.len() / 20);
    for (i, line) in text.lines().enumerate() {
        if i > 0 && (line.starts_with("tba:") || line.starts_with("\"tba:")) {
            out.push('\n');
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Write a zone directory; `header` (comment lines) goes on top of every file, `legend` per file.
pub fn write_zone(dir: &Path, z: &ZoneContent, header: &str, legend: &BTreeMap<&str, &str>) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let put = |name: &str, body: String| -> std::io::Result<()> {
        let path = dir.join(name);
        let text = format!("{header}{}{}", legend.get(name).copied().unwrap_or(""), spaced(body));
        fs::write(path, text)
    };
    if let Some(zone) = &z.zone {
        put("zone.yaml", yaml(zone))?;
    }
    put("rooms.yaml", yaml(&z.rooms))?;
    put("mobs.yaml", yaml(&z.mobs))?;
    put("objects.yaml", yaml(&z.objects))?;
    put("resets.yaml", yaml(&z.resets))?;
    if !z.shops.is_empty() {
        put("shops.yaml", yaml(&z.shops))?;
    }
    Ok(())
}

// ---------------------------------------------------------------- IDs and checks

/// `tba:<zone>:<kind>:<vnum>` -> (zone, kind, vnum).
pub fn parse_id(id: &str) -> Option<(u32, &str, u32)> {
    let mut it = id.split(':');
    let (ns, zone, kind, vnum) = (it.next()?, it.next()?, it.next()?, it.next()?);
    if ns != "tba" || it.next().is_some() || !matches!(kind, "room" | "mob" | "obj" | "shop") {
        return None;
    }
    Some((zone.parse().ok()?, kind, vnum.parse().ok()?))
}

#[derive(Debug, Default)]
pub struct Report {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub counts: BTreeMap<&'static str, usize>,
}

/// Check a whole world: IDs well formed and in their zone, every reference resolves, doors match
/// on both sides (warnings: the original world has a few one-sided doors).
pub fn check_world(zones: &[ZoneContent]) -> Report {
    let mut r = Report::default();
    let mut ranges: Vec<(u32, u32, u32)> = Vec::new();
    let (mut rooms, mut mobs, mut objs) = (BTreeMap::new(), BTreeSet::new(), BTreeMap::new());
    for z in zones {
        let Some(zone) = &z.zone else { continue };
        if let Some(&[lo, hi]) = zone.source.vnums.get(..2).and_then(|s| <&[u32; 2]>::try_from(s).ok()) {
            let num = zone.id.strip_prefix("tba:").and_then(|n| n.parse().ok()).unwrap_or(u32::MAX);
            ranges.push((num, lo, hi));
        }
        rooms.extend(z.rooms.iter().map(|(k, v)| (k.clone(), v)));
        mobs.extend(z.mobs.keys().cloned());
        objs.extend(z.objects.iter().map(|(k, v)| (k.clone(), v)));
    }
    *r.counts.entry("zones").or_default() = zones.len();
    *r.counts.entry("rooms").or_default() = rooms.len();
    *r.counts.entry("mobs").or_default() = mobs.len();
    *r.counts.entry("objects").or_default() = objs.len();
    *r.counts.entry("shops").or_default() = zones.iter().map(|z| z.shops.len()).sum();
    let zone_of = |vnum: u32| ranges.iter().find(|(_, lo, hi)| (*lo..=*hi).contains(&vnum)).map(|(n, ..)| *n);

    let check_id = |id: &str, kind: &str, r: &mut Report| match parse_id(id) {
        None => r.errors.push(format!("{id}: not a tba:<zone>:<kind>:<vnum> ID")),
        Some((z, k, v)) => {
            if k != kind {
                r.errors.push(format!("{id}: expected a {kind}"));
            }
            if kind == "room" && zone_of(v).is_some_and(|zz| zz != z) {
                r.errors.push(format!("{id}: vnum {v} belongs to zone {}", zone_of(v).unwrap()));
            }
        }
    };
    for z in zones {
        for id in z.rooms.keys() {
            check_id(id, "room", &mut r);
        }
        for id in z.mobs.keys() {
            check_id(id, "mob", &mut r);
        }
        for id in z.objects.keys() {
            check_id(id, "obj", &mut r);
        }
        for id in z.shops.keys() {
            check_id(id, "shop", &mut r);
        }
    }

    let room = |id: &str, what: &str, r: &mut Report| {
        if !rooms.contains_key(id) {
            r.errors.push(format!("{what}: no room {id}"));
        }
    };
    let obj = |id: &str, what: &str, r: &mut Report| {
        if !objs.contains_key(id) {
            r.errors.push(format!("{what}: no object {id}"));
        }
    };
    fn loads(l: &Load, what: &str, objs: &BTreeMap<Id, &Object>, r: &mut Report) {
        if !objs.contains_key(&l.object) {
            r.errors.push(format!("{what}: no object {}", l.object));
        }
        for c in &l.contents {
            loads(c, what, objs, r);
        }
    }
    for z in zones {
        for (id, rm) in &z.rooms {
            for (d, ex) in &rm.exits {
                if let Some(to) = &ex.to {
                    room(to, &format!("{id} exit {}", d.name()), &mut r);
                }
                if let Some(key) = ex.door.as_ref().and_then(|dr| dr.key.as_ref()) {
                    obj(key, &format!("{id} door {}", d.name()), &mut r);
                }
            }
        }
        for (id, o) in &z.objects {
            if let Some(key) = &o.values.key {
                obj(key, &format!("{id} container key"), &mut r);
            }
        }
        let zid = z.zone.as_ref().map(|z| z.id.clone()).unwrap_or_default();
        for s in &z.resets.spawns {
            match s {
                Spawn::Mob(m) => {
                    if !mobs.contains(&m.mob) {
                        r.errors.push(format!("{zid} reset: no mob {}", m.mob));
                    }
                    room(&m.room, &format!("{zid} reset of {}", m.mob), &mut r);
                    for l in m.equip.values().chain(&m.carry) {
                        loads(l, &format!("{zid} reset of {}", m.mob), &objs, &mut r);
                    }
                }
                Spawn::Object(o) => {
                    obj(&o.object, &format!("{zid} reset"), &mut r);
                    if let Some(rm) = &o.room {
                        room(rm, &format!("{zid} reset of {}", o.object), &mut r);
                    }
                    for l in &o.contents {
                        loads(l, &format!("{zid} reset of {}", o.object), &objs, &mut r);
                    }
                }
            }
        }
        for d in &z.resets.doors {
            room(&d.room, &format!("{zid} door reset"), &mut r);
        }
        for (id, sh) in &z.shops {
            match &sh.keeper {
                Some(k) if !mobs.contains(k) => r.errors.push(format!("{id}: no keeper {k}")),
                None => r.warnings.push(format!("{id}: no keeper")),
                _ => {}
            }
            for rm in &sh.rooms {
                room(rm, id, &mut r);
            }
            for p in &sh.products {
                obj(p, id, &mut r);
            }
        }
    }

    // doors on both sides
    for (id, rm) in &rooms {
        for (d, ex) in &rm.exits {
            let (Some(door), Some(to)) = (&ex.door, &ex.to) else { continue };
            let back = rooms.get(to).and_then(|t| t.exits.get(&opposite(*d)));
            match back {
                Some(b) if b.to.as_deref() == Some(id.as_str()) => match &b.door {
                    None => r.warnings.push(format!("{id} {}: a door here, none on the other side", d.name())),
                    Some(bd) => {
                        if bd.reset != door.reset {
                            r.warnings.push(format!("{id} {}: reset {:?} here, {:?} on the other side", d.name(),
                                                    door.reset, bd.reset));
                        }
                        if bd.key != door.key {
                            r.warnings.push(format!("{id} {}: key differs from the other side", d.name()));
                        }
                    }
                },
                _ => {}
            }
        }
    }
    // markup (D20): every paragraph and every short string balanced, known names only
    let mut marked = 0usize;
    let mut text = |where_: &str, s: &str, r: &mut Report| match crate::markup::tags(s) {
        Ok(t) => marked += usize::from(!t.is_empty()),
        Err(e) => r.errors.push(format!("{where_}: {e}")),
    };
    for z in zones {
        for (id, rm) in &z.rooms {
            text(&format!("{id} name"), &rm.name, &mut r);
            for p in rm.description.paragraphs() {
                text(&format!("{id} description"), p, &mut r);
            }
            for (d, ex) in &rm.exits {
                for p in ex.look.iter().flat_map(|l| l.paragraphs()) {
                    text(&format!("{id} exit {} look", d.name()), p, &mut r);
                }
            }
            for ex in &rm.extras {
                for p in ex.text.paragraphs() {
                    text(&format!("{id} extra"), p, &mut r);
                }
            }
        }
        for (id, m) in &z.mobs {
            for (k, s) in [("short", &m.short), ("long", &m.long)] {
                text(&format!("{id} {k}"), s, &mut r);
            }
            for p in m.description.paragraphs() {
                text(&format!("{id} description"), p, &mut r);
            }
        }
        for (id, o) in &z.objects {
            for (k, s) in [("short", &o.short), ("long", &o.long)] {
                text(&format!("{id} {k}"), s, &mut r);
            }
            for p in o.extras.iter().flat_map(|e| e.text.paragraphs()) {
                text(&format!("{id} extra"), p, &mut r);
            }
        }
    }
    *r.counts.entry("texts with markup").or_default() = marked;
    *r.counts.entry("preformatted texts").or_default() =
        rooms.values().filter(|rm| matches!(rm.description, Text::Preformatted(_))).count();
    r
}

pub fn opposite(d: Dir) -> Dir {
    match d {
        Dir::North => Dir::South,
        Dir::South => Dir::North,
        Dir::East => Dir::West,
        Dir::West => Dir::East,
        Dir::Up => Dir::Down,
        Dir::Down => Dir::Up,
    }
}

/// Entries of a zone by kind, for tools that walk content without caring about the type.
pub fn ids(z: &ZoneContent) -> IndexMap<&'static str, Vec<&Id>> {
    IndexMap::from([
        ("rooms", z.rooms.keys().collect()),
        ("mobs", z.mobs.keys().collect()),
        ("objects", z.objects.keys().collect()),
        ("shops", z.shops.keys().collect()),
    ])
}

/// Items of a type, for quick lookups in tests.
pub fn objects_of(z: &ZoneContent, kind: ItemType) -> Vec<&Id> {
    z.objects.iter().filter(|(_, o)| o.kind == kind).map(|(k, _)| k).collect()
}
