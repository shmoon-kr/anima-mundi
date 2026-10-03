//! The places of the world, built once from content: rooms with their exits, doors and zone.

use std::collections::HashMap;

use mundi_content::names::{DoorState, RoomFlag, Sector, ZoneFlag};
use mundi_content::names::Dir;
use mundi_content::{parse_id, ZoneContent};

pub type RoomIx = usize;

#[derive(Debug, Clone)]
pub struct Door {
    /// The first door keyword, which messages name the door by (MECHANICS §2.2).
    pub keyword: Option<String>,
    pub state: DoorState,
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
}

#[derive(Debug, Clone, Default)]
pub struct ZoneInfo {
    pub min_level: Option<i32>,
    pub closed: bool,
}

#[derive(Debug, Default)]
pub struct World {
    pub rooms: Vec<Room>,
    pub index: HashMap<String, RoomIx>,
    pub zones: HashMap<u32, ZoneInfo>,
}

impl World {
    /// Doors start in the state their zone reset puts them in, as at boot (MECHANICS §14.1).
    pub fn build(zones: &[ZoneContent]) -> World {
        let mut w = World::default();
        for z in zones {
            if let Some(zone) = &z.zone {
                let num = zone.id.strip_prefix("tba:").and_then(|n| n.parse().ok()).unwrap_or(0);
                w.zones.insert(num, ZoneInfo {
                    min_level: zone.levels.as_ref().map(|l| l.min),
                    closed: zone.flags.contains(&ZoneFlag::Closed),
                });
            }
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
                            state: d.reset.unwrap_or(DoorState::Open),
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
