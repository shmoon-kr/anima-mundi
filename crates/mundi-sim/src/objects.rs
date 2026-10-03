//! Objects: made from prototypes, moved between rooms, characters and containers, taken out of
//! the world. New things go to the front of a list, as tbaMUD puts them (handler.c obj_to_*).

use mundi_content::names::{EquipPos, ItemType};

use crate::entity::{ObjValues, SavedObj};
use crate::*;

impl Sim {
    /// A new copy of an object prototype, nowhere yet. None if the prototype does not exist.
    pub(crate) fn make_obj(&mut self, proto: &str) -> Option<Key> {
        let p = self.world.obj_protos.get(proto)?.clone();
        let v = &p.values;
        let values = ObjValues {
            hours: v.hours,
            capacity: v.capacity,
            contains: v.contains,
            liquid: v.liquid,
            poisoned: v.poisoned,
            coins: v.coins,
        };
        let serial = self.next_serial();
        let key = self.objs.insert(Obj {
            proto: Some(proto.to_string()),
            serial,
            kind: p.kind,
            keywords: p.keywords.clone(),
            short: p.short.clone(),
            long: p.long.clone(),
            flags: p.flags.clone(),
            wear: p.wear.clone(),
            weight: p.weight,
            cost: p.cost,
            level: p.level,
            values,
            timer: -1,
            place: Place::Nowhere,
            contents: Vec::new(),
        });
        *self.counts.entry(proto.to_string()).or_default() += 1;
        Some(key)
    }

    /// An object's ID in events: `<prototype>/<serial>`, or `made/<serial>` for corpses and money.
    pub(crate) fn obj_id(&self, k: Key) -> Option<String> {
        let o = self.objs.get(k)?;
        Some(format!("{}/{}", o.proto.as_deref().unwrap_or("made"), o.serial))
    }

    /// Takes an object out of wherever it is (it stays in the world, nowhere).
    pub(crate) fn unplace(&mut self, k: Key) {
        let Some(place) = self.objs.get(k).map(|o| o.place) else { return };
        match place {
            Place::Room(r) => self.things[r].retain(|x| *x != k),
            Place::Carried(c) => {
                if let Some(c) = self.chars.get_mut(c) {
                    c.inventory.retain(|x| *x != k);
                }
            }
            Place::Worn(c, pos) => {
                if let Some(c) = self.chars.get_mut(c) {
                    c.equipment.remove(&pos);
                }
            }
            Place::In(o) => {
                if let Some(o) = self.objs.get_mut(o) {
                    o.contents.retain(|x| *x != k);
                }
            }
            Place::Nowhere => {}
        }
        if let Some(o) = self.objs.get_mut(k) {
            o.place = Place::Nowhere;
        }
    }

    pub(crate) fn put(&mut self, k: Key, place: Place) {
        self.unplace(k);
        match place {
            Place::Room(r) => self.things[r].insert(0, k),
            Place::Carried(c) => self.chars.get_mut(c).unwrap().inventory.insert(0, k),
            Place::Worn(c, pos) => {
                self.chars.get_mut(c).unwrap().equipment.insert(pos, k);
            }
            Place::In(o) => self.objs.get_mut(o).unwrap().contents.insert(0, k),
            Place::Nowhere => {}
        }
        self.objs.get_mut(k).unwrap().place = place;
    }

    /// Takes an object and everything in it out of the world.
    pub(crate) fn extract_obj(&mut self, k: Key) {
        self.unplace(k);
        let Some(o) = self.objs.remove(k) else { return };
        if let Some(p) = &o.proto {
            *self.counts.entry(p.clone()).or_default() -= 1;
        }
        for c in o.contents {
            self.extract_obj(c);
        }
    }

    /// Weight with contents (tbaMUD adds what is put in to the container's weight).
    pub(crate) fn total_weight(&self, k: Key) -> i64 {
        let Some(o) = self.objs.get(k) else { return 0 };
        o.weight + o.contents.iter().map(|c| self.total_weight(*c)).sum::<i64>()
    }

    /// What a character carries in their hands and packs: weight and number (equipment not counted,
    /// utils.h IS_CARRYING_W / _N).
    pub(crate) fn carrying(&self, c: Key) -> (i64, usize) {
        let Some(ch) = self.chars.get(c) else { return (0, 0) };
        (ch.inventory.iter().map(|o| self.total_weight(*o)).sum(), ch.inventory.len())
    }

    /// Whether a lit light is in the light slot (MECHANICS §3.1).
    pub(crate) fn holds_light(&self, c: Key) -> bool {
        if self.test_lights.contains(&c) {
            return true;
        }
        let Some(ch) = self.chars.get(c) else { return false };
        ch.equipment.get(&EquipPos::Light).and_then(|o| self.objs.get(*o)).is_some_and(|o| o.kind == ItemType::Light && o.values.hours != Some(0))
    }

    /// Everything a player has, as saved: inventory and equipment, with contents.
    pub(crate) fn save_objects(&self, c: Key) -> Vec<SavedObj> {
        let Some(ch) = self.chars.get(c) else { return vec![] };
        let one = |k: Key, worn: Option<EquipPos>| self.save_obj(k, worn);
        let mut out: Vec<SavedObj> = ch.equipment.iter().filter_map(|(pos, k)| one(*k, Some(*pos))).collect();
        out.extend(ch.inventory.iter().filter_map(|k| one(*k, None)));
        out
    }

    fn save_obj(&self, k: Key, worn: Option<EquipPos>) -> Option<SavedObj> {
        let o = self.objs.get(k)?;
        let proto = o.proto.clone()?;
        let base = self.world.obj_protos.get(&proto)?;
        let v = &base.values;
        let initial = ObjValues { hours: v.hours, capacity: v.capacity, contains: v.contains, liquid: v.liquid, poisoned: v.poisoned, coins: v.coins };
        Some(SavedObj {
            proto,
            values: if o.values == initial { ObjValues::default() } else { o.values.clone() },
            worn,
            contents: o.contents.iter().filter_map(|c| self.save_obj(*c, None)).collect(),
        })
    }

    /// Gives back a player's saved objects. Prototypes that no longer exist are dropped.
    pub(crate) fn restore_objects(&mut self, c: Key, saved: &[SavedObj]) {
        // Saved order is front first; putting each at the front means going backwards.
        for s in saved.iter().rev() {
            if let Some(k) = self.restore_obj(s) {
                let place = match s.worn {
                    Some(pos) => Place::Worn(c, pos),
                    None => Place::Carried(c),
                };
                self.put(k, place);
            }
        }
    }

    fn restore_obj(&mut self, s: &SavedObj) -> Option<Key> {
        let k = self.make_obj(&s.proto)?;
        if s.values != ObjValues::default() {
            self.objs.get_mut(k).unwrap().values = s.values.clone();
        }
        for inner in s.contents.iter().rev() {
            if let Some(i) = self.restore_obj(inner) {
                self.put(i, Place::In(k));
            }
        }
        Some(k)
    }
}
