//! Reading tbaMUD world files into the content types. Formats as documented in tbaMUD's
//! doc/building.txt and the loader's field order (db.c parse_room, parse_mobile, parse_object,
//! load_zones; shop.c boot_the_shops). Name tables are in mundi_content::names.
//! The reference for every rule here is tools/prototype_convert.py; the two must agree (tests).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use indexmap::IndexMap;
use mundi_content::model::*;
use mundi_content::names::*;

use crate::markup::{markup, markup_paragraphs};

/// Problems the converter noticed but could convert around (unknown bits, bad references).
#[derive(Debug, Default)]
pub struct Notes(pub Vec<String>);

fn lines(path: &Path) -> Vec<String> {
    let bytes = fs::read(path).unwrap_or_default();
    // tbaMUD files are Latin-1: every byte is one character
    let text: String = bytes.iter().map(|&b| b as char).collect();
    text.replace('\r', "").split('\n').map(str::to_string).collect()
}

/// A string up to the line ending in `~`.
fn tilde(ls: &[String], mut i: usize) -> (String, usize) {
    let mut buf: Vec<&str> = Vec::new();
    while i < ls.len() {
        let s = &ls[i];
        i += 1;
        if let Some(t) = s.strip_suffix('~') {
            buf.push(t);
            return (buf.join("\n"), i);
        }
        buf.push(s);
    }
    (buf.join("\n"), i)
}

fn vnum_header(line: &str) -> Option<u32> {
    line.trim().strip_prefix('#')?.parse().ok()
}

/// A flag field: a number or letters (a = bit 0 ... z = 25, A = 26 ...).
fn bits(tok: &str) -> u64 {
    if let Ok(n) = tok.parse::<i64>() {
        return n as u64;
    }
    tok.chars().fold(0u64, |v, c| match c {
        'a'..='z' => v | 1 << (c as u32 - 'a' as u32),
        'A'..='Z' => v | 1 << (26 + c as u32 - 'A' as u32),
        _ => v,
    })
}

fn flags<T: Copy>(tok: &str, all: &[T], offset: usize, skip: &[usize], what: &str, notes: &mut Notes) -> Vec<T> {
    let v = bits(tok);
    let mut out = Vec::new();
    for b in 0..64usize {
        if v & (1 << b) == 0 || skip.contains(&b) {
            continue;
        }
        match b.checked_sub(offset).and_then(|i| all.get(i)) {
            Some(x) => out.push(*x),
            None => notes.0.push(format!("{what}: unknown bit {b}")),
        }
    }
    out
}

fn int(s: &str) -> i64 {
    s.trim().parse().unwrap_or(0)
}

/// Hard-wrapped tbaMUD text to paragraphs (one per line); maps and pictures stay verbatim.
pub fn prose(text: &str) -> Text {
    let raw = text.trim_end_matches('\n');
    let art = raw.lines().any(|l| {
        let t = l.trim();
        let inner_gap = t.contains("    ") && !t.is_empty();
        let rule = l.as_bytes().windows(3).any(|w| w.iter().all(|c| b"|\\/_=+#*".contains(c)));
        inner_gap || rule
    });
    if art {
        return Text::Preformatted(markup(&format!("{raw}\n")));
    }
    let mut paras: Vec<String> = Vec::new();
    let mut cur: Vec<&str> = Vec::new();
    for l in raw.split('\n') {
        if (l.starts_with("   ") || l.trim().is_empty()) && !cur.is_empty() {
            paras.push(cur.join(" "));
            cur.clear();
        }
        if !l.trim().is_empty() {
            cur.push(l.trim());
        }
    }
    if !cur.is_empty() {
        paras.push(cur.join(" "));
    }
    Text::Prose(markup_paragraphs(paras.iter().map(String::as_str)).join("\n"))
}

/// `NdS+B`: a number when nothing is rolled (0d0+20 is 20, 1d1+50 is 51).
pub fn amount(text: &str) -> Amount {
    let t = text.trim();
    let Some((n, rest)) = t.split_once('d') else { return Amount::Dice(t.into()) };
    let (sides, bonus) = match rest.find(['+', '-']) {
        Some(p) => (&rest[..p], int(&rest[p..])),
        None => (rest, 0),
    };
    let (Ok(n), Ok(sides)) = (n.parse::<i64>(), sides.parse::<i64>()) else { return Amount::Dice(t.into()) };
    if n == 0 || sides == 0 {
        Amount::Fixed(bonus)
    } else if sides == 1 {
        Amount::Fixed(n + bonus)
    } else if bonus == 0 {
        Amount::Dice(format!("{n}d{sides}"))
    } else {
        Amount::Dice(t.into())
    }
}

// ---------------------------------------------------------------- zones and IDs

pub struct ZoneHead {
    pub num: u32,
    pub builders: String,
    pub name: String,
    pub head: Vec<String>,
    pub commands: Vec<String>,
}

pub fn read_zones(world: &Path) -> BTreeMap<u32, ZoneHead> {
    let mut out = BTreeMap::new();
    for f in sorted(world.join("zon"), "zon") {
        let ls = lines(&f);
        let Some(num) = ls.first().and_then(|l| vnum_header(l)) else { continue };
        let (builders, i) = tilde(&ls, 1);
        let (name, i) = tilde(&ls, i);
        let head = ls.get(i).map(|l| l.split_whitespace().map(str::to_string).collect()).unwrap_or_default();
        out.insert(num, ZoneHead { num, builders: builders.trim().into(), name: name.trim().into(), head,
                                   commands: ls[(i + 1).min(ls.len())..].to_vec() });
    }
    out
}

pub fn sorted(dir: impl AsRef<Path>, ext: &str) -> Vec<std::path::PathBuf> {
    let mut v: Vec<_> = fs::read_dir(dir).map(|d| d.filter_map(|e| e.ok().map(|e| e.path())).collect()).unwrap_or_default();
    v.retain(|p| p.extension().is_some_and(|e| e == ext));
    v.sort_by_key(|p| p.file_stem().and_then(|s| s.to_str()).and_then(|s| s.parse::<u32>().ok()).unwrap_or(u32::MAX));
    v
}

/// IDs: tba:<zone>:<kind>:<vnum>. The zone of a vnum is the zone whose room range holds it.
/// Knows which rooms, mobs and objects exist, so references to missing ones are dropped with a note
/// (tbaMUD itself just fails those at run time: a key that does not exist opens nothing).
pub struct Ids {
    ranges: Vec<(u32, u32, u32)>,
    existing: BTreeMap<&'static str, std::collections::BTreeSet<u32>>,
}

impl Ids {
    pub fn new(zones: &BTreeMap<u32, ZoneHead>) -> Ids {
        let ranges = zones
            .values()
            .filter_map(|z| Some((z.num, z.head.first()?.parse().ok()?, z.head.get(1)?.parse().ok()?)))
            .collect();
        Ids { ranges, existing: BTreeMap::new() }
    }

    /// Scan the world's record headers (`#<vnum>` alone on a line) for what exists.
    pub fn scan(mut self, world: &Path) -> Ids {
        for (kind, dir) in [("room", "wld"), ("mob", "mob"), ("obj", "obj")] {
            let set = self.existing.entry(kind).or_default();
            for f in sorted(world.join(dir), dir) {
                set.extend(lines(&f).iter().filter_map(|l| vnum_header(l)));
            }
        }
        self
    }

    /// The ID if it exists (or if the world was not scanned), else None and a note.
    pub fn some(&self, kind: &'static str, vnum: i64, what: &str, notes: &mut Notes) -> Option<Id> {
        if vnum <= 0 {
            return None;
        }
        let v = vnum as u32;
        match self.existing.get(kind) {
            Some(set) if !set.contains(&v) => {
                notes.0.push(format!("{what}: no {kind} {v} in the world (dropped)"));
                None
            }
            _ => Some(self.id(kind, v)),
        }
    }
    pub fn zone_of(&self, vnum: u32) -> u32 {
        self.ranges.iter().find(|(_, lo, hi)| (*lo..=*hi).contains(&vnum)).map(|(z, ..)| *z).unwrap_or(vnum / 100)
    }
    pub fn id(&self, kind: &str, vnum: u32) -> Id {
        format!("tba:{}:{kind}:{vnum}", self.zone_of(vnum))
    }
}

// ---------------------------------------------------------------- rooms

pub fn read_rooms(path: &Path, ids: &Ids, notes: &mut Notes) -> IndexMap<Id, Room> {
    let ls = lines(path);
    let mut out = IndexMap::new();
    let mut i = 0;
    while i < ls.len() {
        let Some(v) = vnum_header(&ls[i]) else {
            i += 1;
            continue;
        };
        let (name, j) = tilde(&ls, i + 1);
        let (desc, j) = tilde(&ls, j);
        i = j;
        let head: Vec<&str> = ls.get(i).map(|l| l.split_whitespace().collect()).unwrap_or_default();
        i += 1;
        let what = format!("room {v}");
        let sector = head.last().and_then(|s| s.parse::<usize>().ok()).and_then(Sector::from_index).unwrap_or(Sector::Inside);
        let mut room = Room {
            name: markup(name.trim()),
            description: prose(&desc),
            sector,
            flags: head.get(1).map(|t| flags(t, RoomFlag::ALL, 0, &[], &what, notes)).unwrap_or_default(),
            exits: IndexMap::new(),
            extras: Vec::new(),
            triggers: Vec::new(),
        };
        let mut exits: BTreeMap<usize, Exit> = BTreeMap::new();
        while i < ls.len() && ls[i].trim() != "S" {
            let t = ls[i].trim().to_string();
            if let Some(d) = t.strip_prefix('D').and_then(|d| d.parse::<usize>().ok()) {
                let (look, j) = tilde(&ls, i + 1);
                let (kw, j) = tilde(&ls, j);
                let nums: Vec<i64> = ls.get(j).map(|l| l.split_whitespace().take(3).map(int).collect()).unwrap_or_default();
                i = j + 1;
                let (info, key, to) = (nums.first().copied().unwrap_or(0), nums.get(1).copied().unwrap_or(-1),
                                       nums.get(2).copied().unwrap_or(-1));
                let mut ex = Exit { to: ids.some("room", to, &format!("{what} exit {d}"), notes), look: None, door: None };
                if !look.trim().is_empty() {
                    ex.look = Some(prose(&look));
                }
                if info != 0 {
                    let mut words: Vec<String> = kw.split_whitespace().map(str::to_string).collect();
                    if words.is_empty() {
                        words.push("door".into());
                    }
                    ex.door = Some(Door {
                        keywords: words,
                        kind: if info == 2 { DoorKind::Pickproof } else { DoorKind::Door },
                        key: ids.some("obj", key, &format!("{what} door {d} key"), notes),
                        reset: None,
                    });
                }
                if Dir::from_index(d).is_some() {
                    exits.insert(d, ex);
                } else {
                    notes.0.push(format!("{what}: exit {d}"));
                }
            } else if t == "E" {
                let (kw, j) = tilde(&ls, i + 1);
                let (text, j) = tilde(&ls, j);
                i = j;
                room.extras.push(Extra { keywords: kw.split_whitespace().map(str::to_string).collect(), text: prose(&text) });
            } else {
                i += 1;
            }
        }
        room.exits = exits.into_iter().map(|(d, e)| (Dir::from_index(d).unwrap(), e)).collect();
        // Triggers follow the room's S (DG Scripts: "T <vnum>").
        room.triggers = triggers_after(&ls, i + 1, ids);
        out.insert(ids.id("room", v), room);
    }
    out
}

/// The "T <vnum>" lines from `from` up to the next record.
fn triggers_after(ls: &[String], from: usize, ids: &Ids) -> Vec<Id> {
    let mut out = Vec::new();
    let mut i = from;
    while i < ls.len() && vnum_header(&ls[i]).is_none() && !ls[i].starts_with('$') {
        if let Some(n) = ls[i].strip_prefix("T ").and_then(|t| t.trim().parse::<u32>().ok()) {
            out.push(ids.id("trg", n));
        }
        i += 1;
    }
    out
}

// ---------------------------------------------------------------- mobs

pub fn read_mobs(path: &Path, ids: &Ids, notes: &mut Notes) -> IndexMap<Id, Mob> {
    let ls = lines(path);
    let mut out = IndexMap::new();
    let mut i = 0;
    while i < ls.len() {
        let Some(v) = vnum_header(&ls[i]) else {
            i += 1;
            continue;
        };
        let what = format!("mob {v}");
        let (kw, j) = tilde(&ls, i + 1);
        let (short, j) = tilde(&ls, j);
        let (long, j) = tilde(&ls, j);
        let (desc, j) = tilde(&ls, j);
        i = j;
        let get = |k: usize| -> Vec<String> { ls.get(k).map(|l| l.split_whitespace().map(str::to_string).collect()).unwrap_or_default() };
        let (f, stats, money, pos) = (get(i), get(i + 1), get(i + 2), get(i + 3));
        i += 4;
        let mut espec: BTreeMap<String, i64> = BTreeMap::new();
        if f.last().is_some_and(|x| x == "E") {
            while i < ls.len() && ls[i].trim() != "E" {
                if let Some((k, val)) = ls[i].split_once(':') {
                    if let Ok(n) = val.trim().parse::<i64>() {
                        espec.insert(k.trim().into(), n);
                    }
                }
                i += 1;
            }
        }
        let is_npc = MobFlag::ALL.iter().position(|m| *m == MobFlag::IsNpc).unwrap();
        let mut mob_flags = f.first().map(|t| flags(t, MobFlag::ALL, 0, &[is_npc], &what, notes)).unwrap_or_default();
        mob_flags.retain(|m| *m != MobFlag::IsNpc);
        let affects = if f.len() >= 10 { flags(&f[4], Affect::ALL, 1, &[0], &what, notes) } else { Vec::new() };
        let s = |k: usize| stats.get(k).map(String::as_str).unwrap_or("0");
        let ab = [("Str", "str"), ("StrAdd", "str_add"), ("Int", "int"), ("Wis", "wis"), ("Dex", "dex"), ("Con", "con"),
                  ("Cha", "cha")];
        let sv = [("SavingPara", "para"), ("SavingRod", "rod"), ("SavingPetri", "petri"), ("SavingBreath", "breath"),
                  ("SavingSpell", "spell")];
        let mob = Mob {
            keywords: kw.split_whitespace().map(str::to_string).collect(),
            short: markup(short.trim()),
            long: markup(long.trim()),
            description: prose(&desc),
            level: int(s(0)) as i32,
            sex: pos.get(2).and_then(|x| x.parse().ok()).and_then(Sex::from_index).unwrap_or(Sex::Neutral),
            alignment: f.get(f.len().saturating_sub(2)).map(|x| int(x) as i32).unwrap_or(0),
            flags: mob_flags,
            affects,
            combat: Combat {
                thac0: int(s(1)) as i32,
                armor: int(s(2)) as i32,
                hit_points: amount(s(3)),
                damage: amount(s(4)),
                bare_hand_attack: espec.get("BareHandAttack").and_then(|n| Attack::from_index(*n as usize)),
            },
            abilities: ab.iter().filter_map(|(k, n)| espec.get(*k).map(|v| (n.to_string(), *v))).collect(),
            saves: sv.iter().filter_map(|(k, n)| espec.get(*k).map(|v| (n.to_string(), *v))).collect(),
            gold: money.first().map(|x| int(x)).unwrap_or(0),
            exp: money.get(1).map(|x| int(x)).unwrap_or(0),
            position: Positions {
                load: pos.first().and_then(|x| x.parse().ok()).and_then(Position::from_index).unwrap_or(Position::Standing),
                default: pos.get(1).and_then(|x| x.parse().ok()).and_then(Position::from_index).unwrap_or(Position::Standing),
            },
            triggers: triggers_after(&ls, i, ids),
        };
        out.insert(ids.id("mob", v), mob);
    }
    out
}

// ---------------------------------------------------------------- objects

fn values(kind: ItemType, v: [i64; 4], ids: &Ids, what: &str, notes: &mut Notes) -> Values {
    let mut out = Values::default();
    match kind {
        ItemType::Light => out.hours = Some(v[2]),
        ItemType::Weapon => {
            out.damage = Some(format!("{}d{}", v[1], v[2]));
            out.attack = Attack::from_index(v[3] as usize);
            if out.attack.is_none() {
                out.raw = Some(v.to_vec());
            }
        }
        ItemType::Armor => out.armor = Some(v[0]),
        ItemType::Drinkcon | ItemType::Fountain => {
            out.capacity = Some(v[0]);
            out.contains = Some(v[1]);
            out.liquid = usize::try_from(v[2]).ok().and_then(Liquid::from_index);
            out.poisoned = Some(v[3] != 0);
        }
        ItemType::Food => {
            out.hours = Some(v[0]);
            out.poisoned = Some(v[3] != 0);
        }
        ItemType::Money => out.coins = Some(v[0]),
        ItemType::Container => {
            out.capacity = Some(v[0]);
            out.lock_flags = Some(v[1]);
            out.key = ids.some("obj", v[2], &format!("{what} container key"), notes);
            out.corpse = Some(v[3] != 0);
        }
        _ => out.raw = Some(v.to_vec()),
    }
    out
}

pub fn read_objects(path: &Path, ids: &Ids, notes: &mut Notes) -> IndexMap<Id, Object> {
    let ls = lines(path);
    let mut out = IndexMap::new();
    let mut i = 0;
    while i < ls.len() {
        let Some(v) = vnum_header(&ls[i]) else {
            i += 1;
            continue;
        };
        let what = format!("obj {v}");
        let (kw, j) = tilde(&ls, i + 1);
        let (short, j) = tilde(&ls, j);
        let (long, j) = tilde(&ls, j);
        let (action, j) = tilde(&ls, j);
        i = j;
        let get = |k: usize| -> Vec<String> { ls.get(k).map(|l| l.split_whitespace().map(str::to_string).collect()).unwrap_or_default() };
        let (head, vals, num) = (get(i), get(i + 1), get(i + 2));
        i += 3;
        let kind = head.first().and_then(|x| x.parse().ok()).and_then(ItemType::from_index).unwrap_or(ItemType::Other);
        let mut v4 = [0i64; 4];
        for (k, x) in vals.iter().take(4).enumerate() {
            v4[k] = int(x);
        }
        let n = |k: usize| num.get(k).map(|x| int(x)).unwrap_or(0);
        let mut obj = Object {
            kind,
            keywords: kw.split_whitespace().map(str::to_string).collect(),
            short: markup(short.trim()),
            long: markup(long.trim()),
            action: (!action.trim().is_empty()).then(|| markup(action.trim())),
            flags: if head.len() >= 13 { flags(&head[1], ObjFlag::ALL, 0, &[], &what, notes) } else { Vec::new() },
            wear: if head.len() >= 13 { flags(&head[5], Wear::ALL, 0, &[], &what, notes) } else { Vec::new() },
            values: values(kind, v4, ids, &what, notes),
            weight: n(0),
            cost: n(1),
            rent: n(2),
            level: n(3) as i32,
            affects: Vec::new(),
            extras: Vec::new(),
            triggers: Vec::new(),
        };
        while i < ls.len() && vnum_header(&ls[i]).is_none() && !ls[i].starts_with('$') {
            if let Some(t) = ls[i].strip_prefix("T ") {
                if let Ok(n) = t.trim().parse::<u32>() {
                    obj.triggers.push(ids.id("trg", n));
                }
                i += 1;
            } else if ls[i].starts_with('A') {
                let a: Vec<i64> = ls.get(i + 1).map(|l| l.split_whitespace().map(int).collect()).unwrap_or_default();
                if let (Some(&loc), Some(&m)) = (a.first(), a.get(1)) {
                    match Apply::from_code(loc) {
                        Some(apply) => obj.affects.push(ObjAffect { apply, modifier: m }),
                        None if loc == 0 => {}
                        None => notes.0.push(format!("{what}: unknown apply {loc}")),
                    }
                }
                i += 2;
            } else if ls[i].starts_with('E') {
                let (k, j) = tilde(&ls, i + 1);
                let (text, j) = tilde(&ls, j);
                i = j;
                obj.extras.push(Extra { keywords: k.split_whitespace().map(str::to_string).collect(), text: prose(&text) });
            } else {
                i += 1;
            }
        }
        out.insert(ids.id("obj", v), obj);
    }
    out
}

// ---------------------------------------------------------------- zone and resets

pub fn zone(z: &ZoneHead, notes: &mut Notes) -> Zone {
    let h = &z.head;
    let num = |k: usize| h.get(k).map(|x| int(x)).unwrap_or(0);
    Zone {
        id: format!("tba:{}", z.num),
        name: z.name.clone(),
        builders: z.builders.clone(),
        levels: (h.len() >= 10 && num(8) > 0).then(|| Levels { min: num(8) as i32, max: num(9) as i32 }),
        reset: ZoneReset {
            every_minutes: num(2) as i32,
            when: ResetWhen::from_index(num(3) as usize).unwrap_or(ResetWhen::Always),
        },
        flags: if h.len() >= 10 { flags(&h[4], ZoneFlag::ALL, 0, &[], &format!("zone {}", z.num), notes) } else { Vec::new() },
        source: Source { vnums: vec![num(0) as u32, num(1) as u32] },
    }
}

/// A node of the reset tree that can hold contents: a ground object, a mob's equipment or inventory
/// item, or something inside one of those (child indices).
#[derive(Clone)]
enum Root {
    Spawn(usize),
    Equip(usize, EquipPos),
    Carry(usize, usize),
}

#[derive(Clone)]
struct NodePath {
    root: Root,
    children: Vec<usize>,
}

fn node<'a>(spawns: &'a mut [Spawn], at: &NodePath) -> Option<&'a mut Vec<Load>> {
    let mut list: &mut Vec<Load> = match &at.root {
        Root::Spawn(s) => match spawns.get_mut(*s)? {
            Spawn::Object(o) => &mut o.contents,
            Spawn::Mob(_) => return None,
        },
        Root::Equip(s, pos) => match spawns.get_mut(*s)? {
            Spawn::Mob(m) => &mut m.equip.get_mut(pos)?.contents,
            _ => return None,
        },
        Root::Carry(s, k) => match spawns.get_mut(*s)? {
            Spawn::Mob(m) => &mut m.carry.get_mut(*k)?.contents,
            _ => return None,
        },
    };
    for &c in &at.children {
        list = &mut list.get_mut(c)?.contents;
    }
    Some(list)
}

pub fn resets(z: &ZoneHead, ids: &Ids, rooms: &mut IndexMap<Id, Room>, notes: &mut Notes) -> Resets {
    let mut out = Resets::default();
    let mut last_mob: Option<usize> = None;
    // P puts an object into "the copy of <container> most recently loaded" (doc/building.txt):
    // remember where the latest copy of every object is
    let mut latest: BTreeMap<u32, NodePath> = BTreeMap::new();
    for line in &z.commands {
        let f: Vec<&str> = line.split_whitespace().collect();
        let Some(cmd) = f.first().filter(|c| c.len() == 1 && "MOGEPDR".contains(**c)) else { continue };
        let a: Vec<i64> = f[1..].iter().take(5).map_while(|x| x.parse::<i64>().ok()).collect();
        let arg = |k: usize| a.get(k).copied().unwrap_or(0);
        let always = arg(0) == 0;
        let what = format!("zone {} {}", z.num, line.split('(').next().unwrap_or(line).trim());
        match *cmd {
            "M" => {
                let (Some(mob), Some(room)) = (ids.some("mob", arg(1), &what, notes), ids.some("room", arg(3), &what, notes)) else {
                    last_mob = None;
                    continue;
                };
                out.spawns.push(Spawn::Mob(MobSpawn {
                    mob,
                    room,
                    limit: arg(2) as i32,
                    equip: IndexMap::new(),
                    carry: Vec::new(),
                }));
                last_mob = Some(out.spawns.len() - 1);
            }
            "O" => {
                let Some(object) = ids.some("obj", arg(1), &what, notes) else { continue };
                out.spawns.push(Spawn::Object(ObjectSpawn {
                    object,
                    room: ids.some("room", arg(3), &what, notes),
                    limit: arg(2) as i32,
                    contents: Vec::new(),
                }));
                latest.insert(arg(1) as u32, NodePath { root: Root::Spawn(out.spawns.len() - 1), children: vec![] });
            }
            "E" | "G" => {
                let Some(m) = last_mob else {
                    notes.0.push(format!("{what}: no mob before it"));
                    continue;
                };
                let Some(object) = ids.some("obj", arg(1), &what, notes) else { continue };
                let load = Load { object, limit: arg(2) as i32, even_if_mob_not_loaded: always, contents: Vec::new() };
                let Spawn::Mob(ms) = &mut out.spawns[m] else { continue };
                if *cmd == "E" {
                    match EquipPos::from_index(arg(3) as usize) {
                        Some(pos) => {
                            ms.equip.insert(pos, load);
                            latest.insert(arg(1) as u32, NodePath { root: Root::Equip(m, pos), children: vec![] });
                        }
                        None => notes.0.push(format!("{what}: equipment position {}", arg(3))),
                    }
                } else {
                    ms.carry.push(load);
                    latest.insert(arg(1) as u32, NodePath { root: Root::Carry(m, ms.carry.len() - 1), children: vec![] });
                }
            }
            "P" => {
                let Some(object) = ids.some("obj", arg(1), &what, notes) else { continue };
                let load = Load { object, limit: arg(2) as i32, even_if_mob_not_loaded: false, contents: Vec::new() };
                let container = arg(3) as u32;
                let Some(at) = latest.get(&container).cloned() else {
                    notes.0.push(format!("{what}: container {container} not loaded before it"));
                    continue;
                };
                match node(&mut out.spawns, &at) {
                    Some(list) => {
                        list.push(load);
                        let mut path = at.clone();
                        path.children.push(list.len() - 1);
                        latest.insert(arg(1) as u32, path);
                    }
                    None => notes.0.push(format!("{what}: container {container} cannot hold it")),
                }
            }
            "D" => {
                let Some(room) = ids.some("room", arg(1), &what, notes) else { continue };
                let (dir, state) = (Dir::from_index(arg(2) as usize),
                                          DoorState::from_index(arg(3) as usize));
                let (Some(dir), Some(state)) = (dir, state) else {
                    notes.0.push(format!("{what}: bad door command"));
                    continue;
                };
                match rooms.get_mut(&room).and_then(|r| r.exits.get_mut(&dir)).and_then(|e| e.door.as_mut()) {
                    Some(door) => door.reset = Some(state),
                    None => out.doors.push(DoorReset { room, exit: dir, state }),
                }
            }
            "R" => {
                if let (Some(room), Some(object)) = (ids.some("room", arg(1), &what, notes), ids.some("obj", arg(2), &what, notes)) {
                    out.removes.push(Remove { room, object });
                }
            }
            _ => {}
        }
    }
    out
}

// ---------------------------------------------------------------- shops

pub fn read_shops(path: &Path, ids: &Ids, notes: &mut Notes) -> IndexMap<Id, Shop> {
    let ls = lines(path);
    let mut out = IndexMap::new();
    let mut i = 0;
    let numbers_until_minus1 = |i: &mut usize| -> Vec<String> {
        let mut v = Vec::new();
        while *i < ls.len() {
            let tok = ls[*i].split(';').next().unwrap_or("").trim().to_string();
            *i += 1;
            if tok.starts_with("-1") {
                break;
            }
            if !tok.is_empty() {
                v.push(tok);
            }
        }
        v
    };
    while i < ls.len() {
        let t = ls[i].trim().trim_end_matches('~');
        let Some(v) = t.strip_prefix('#').and_then(|n| n.parse::<u32>().ok()) else {
            i += 1;
            continue;
        };
        let what = format!("shop {v}");
        i += 1;
        let products = numbers_until_minus1(&mut i);
        let (Some(bp), Some(sp)) = (ls.get(i).and_then(|x| x.trim().parse::<f64>().ok()),
                                    ls.get(i + 1).and_then(|x| x.trim().parse::<f64>().ok())) else {
            notes.0.push(format!("{what}: profits"));
            continue;
        };
        i += 2;
        let types = numbers_until_minus1(&mut i);
        let mut msgs = Vec::new();
        for _ in 0..7 {
            let (m, j) = tilde(&ls, i);
            msgs.push(m.trim().to_string());
            i = j;
        }
        let num = |at: usize| ls.get(at).map(|x| int(x)).unwrap_or(0);
        let (temper, flags_, keeper, trade) = (num(i), num(i + 1), num(i + 2), num(i + 3));
        i += 4;
        let rooms = numbers_until_minus1(&mut i);
        let hours: Vec<i64> = (0..4).map(|k| num(i + k)).collect();
        i += 4;
        let item_names: Vec<&str> = ItemType::ALL.iter().map(|t| t.name()).collect();
        let buys = types
            .iter()
            .filter_map(|t| {
                // "<type> [keywords]": the type is a number or a name; keywords may follow with or
                // without a space ("9leather", "LIGHT torch", "8[gold|titanium|iron]")
                let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
                let (kind, rest) = if !digits.is_empty() {
                    (digits.parse::<usize>().ok().and_then(ItemType::from_index), t[digits.len()..].trim())
                } else {
                    let first = t.split_whitespace().next().unwrap_or("");
                    (item_names.iter().position(|n| n.eq_ignore_ascii_case(first)).and_then(ItemType::from_index),
                     t[first.len()..].trim())
                };
                match kind {
                    Some(kind) => Some(Buy { kind, keywords: (!rest.is_empty()).then(|| rest.to_string()) }),
                    None => {
                        notes.0.push(format!("{what}: item type {t}"));
                        None
                    }
                }
            })
            .collect();
        let m = |k: usize| msgs.get(k).cloned().unwrap_or_default();
        out.insert(ids.id("shop", v), Shop {
            keeper: ids.some("mob", keeper, &what, notes),
            rooms: rooms.iter().filter_map(|r| r.parse::<i64>().ok()).filter_map(|r| ids.some("room", r, &what, notes)).collect(),
            products: products.iter().filter_map(|p| p.parse::<i64>().ok()).filter_map(|p| ids.some("obj", p, &what, notes)).collect(),
            buys,
            profit: Profit { buy: bp, sell: sp },
            messages: ShopMessages { no_such_item: m(0), you_dont_have_it: m(1), does_not_buy: m(2),
                                     shop_cannot_afford: m(3), you_cannot_afford: m(4), bought: m(5), sold: m(6) },
            temper: temper as i32,
            flags: flags_,
            trade_with: trade,
            hours: hours
                .chunks(2)
                .filter(|c| !(c[0] == 0 && c[1] == 28) && !(c[0] == 0 && c[1] == 0))
                .map(|c| Hours { open: c[0] as i32, close: c[1] as i32 })
                .collect(),
        });
    }
    out
}
