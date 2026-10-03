//! The places of the world, built once from content: rooms with their exits, doors and zone.

use std::collections::HashMap;

use mundi_content::names::{DoorState, ResetWhen, RoomFlag, Sector, ZoneFlag};
use mundi_content::names::Dir;
use mundi_content::{parse_id, Mob, Object, Resets, ZoneContent};

pub type RoomIx = usize;

#[derive(Debug, Clone)]
pub struct Door {
    /// The first door keyword, which messages name the door by (MECHANICS §2.2).
    pub keyword: Option<String>,
    pub keywords: Vec<String>,
    pub state: DoorState,
    /// What the zone reset sets it to.
    pub reset: Option<DoorState>,
    pub key: Option<String>,
    pub pickproof: bool,
}

#[derive(Debug, Clone)]
pub struct Exit {
    pub to: Option<RoomIx>,
    pub door: Option<Door>,
}

#[derive(Debug, Clone)]
pub struct Room {
    pub id: String,
    pub zone: u32,
    pub name: String,
    pub desc: String,
    pub sector: Sector,
    pub flags: Vec<RoomFlag>,
    /// Indexed by `Dir::ALL` order.
    pub exits: [Option<Exit>; 6],
    /// The keywords of the room's extra descriptions (things to look at, not to take).
    pub extras: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Default)]
pub struct ZoneInfo {
    pub min_level: Option<i32>,
    pub closed: bool,
}

/// A zone's reset clock and commands (MECHANICS §14.1).
#[derive(Debug, Clone)]
pub struct ZoneState {
    pub num: u32,
    pub resets: Resets,
    pub every_minutes: i32,
    pub when: ResetWhen,
    /// Minutes since the last reset; `None` when queued for reset.
    pub age: Option<i32>,
}

#[derive(Debug, Default)]
pub struct World {
    pub rooms: Vec<Room>,
    pub index: HashMap<String, RoomIx>,
    pub zones: HashMap<u32, ZoneInfo>,
    /// Zones in load order: the order they reset in.
    pub zone_list: Vec<ZoneState>,
    pub mob_protos: HashMap<String, Mob>,
    pub obj_protos: HashMap<String, Object>,
    /// Mobs that keep a shop: protected from attack (MECHANICS §8.1, §15.1).
    pub keepers: std::collections::HashSet<String>,
    pub shops: Vec<mundi_content::Shop>,
}

impl World {
    /// Doors start open; the boot reset puts them in their zone's state (MECHANICS §14.1).
    pub fn build(zones: &[ZoneContent]) -> World {
        let mut w = World::default();
        for z in zones {
            if let Some(zone) = &z.zone {
                let num = zone.id.strip_prefix("tba:").and_then(|n| n.parse().ok()).unwrap_or(0);
                w.zones.insert(num, ZoneInfo {
                    min_level: zone.levels.as_ref().map(|l| l.min),
                    closed: zone.flags.contains(&ZoneFlag::Closed),
                });
                w.zone_list.push(ZoneState {
                    num,
                    resets: z.resets.clone(),
                    every_minutes: zone.reset.every_minutes,
                    when: zone.reset.when,
                    age: Some(0),
                });
            }
            w.mob_protos.extend(z.mobs.iter().map(|(k, v)| (k.clone(), v.clone())));
            w.obj_protos.extend(z.objects.iter().map(|(k, v)| (k.clone(), v.clone())));
            w.keepers.extend(z.shops.values().filter_map(|s| s.keeper.clone()));
            w.shops.extend(z.shops.values().cloned());
            for id in z.rooms.keys() {
                w.index.insert(id.clone(), w.rooms.len());
                let zone = parse_id(id).map(|(z, _, _)| z).unwrap_or(0);
                let r = &z.rooms[id];
                w.rooms.push(Room {
                    id: id.clone(),
                    zone,
                    name: r.name.clone(),
                    desc: r.description.paragraphs().join("\n"),
                    sector: r.sector,
                    flags: r.flags.clone(),
                    exits: Default::default(),
                    extras: r.extras.iter().map(|e| e.keywords.clone()).collect(),
                });
            }
        }
        for z in zones {
            for (id, r) in &z.rooms {
                let ix = w.index[id];
                for (dir, e) in &r.exits {
                    let exit = Exit {
                        to: e.to.as_ref().and_then(|t| w.index.get(t).copied()),
                        door: e.door.as_ref().map(|d| Door {
                            keyword: d.keywords.first().cloned(),
                            keywords: d.keywords.clone(),
                            state: DoorState::Open,
                            reset: d.reset,
                            key: d.key.clone(),
                            pickproof: d.kind == mundi_content::names::DoorKind::Pickproof,
                        }),
                    };
                    w.rooms[ix].exits[dir_index(*dir)] = Some(exit);
                }
            }
        }
        w
    }
}

pub fn dir_index(d: Dir) -> usize {
    Dir::ALL.iter().position(|x| *x == d).unwrap()
}
