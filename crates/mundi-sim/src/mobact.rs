//! What mobs do on their own every 10 seconds (MECHANICS §14.2, mobact.c mobile_activity): pick up
//! the best thing in the room, wander. Aggression, memory and helping come with combat (S5 step 4);
//! special procedures with shops and guilds.

use mundi_content::names::{ItemType, MobFlag, RoomFlag, Wear};
use mundi_protocol::ItemAction;

use crate::*;

impl Sim {
    pub(crate) fn mobile_activity(&mut self) {
        // character_list puts the newest first.
        for k in self.mobs.clone().into_iter().rev() {
            let Some(c) = self.chars.get(k) else { continue };
            if c.position <= Position::Sleeping || c.position == Position::Fighting {
                continue;
            }
            if c.has_flag(MobFlag::Scavenger) {
                self.scavenge(k);
            }
            let Some(c) = self.chars.get(k) else { continue };
            if !c.has_flag(MobFlag::Sentinel) && c.position == Position::Standing {
                let door = self.rand(0, 18) as usize;
                if door < DIRS.len() && self.mob_may_go(k, door) {
                    self.move_dir(k, door);
                }
            }
        }
    }

    /// One time in 11, the most valuable thing (cost over 1) the mob can take and see.
    fn scavenge(&mut self, k: Key) {
        let room = self.chars.get(k).unwrap().room;
        if self.things[room].is_empty() || self.rand(0, 10) != 0 {
            return;
        }
        let (weight, count) = self.carrying(k);
        let (cap_w, cap_n) = (self.can_carry_w(k), self.can_carry_n(k));
        let mut best: Option<(Key, i64)> = None;
        for &o in &self.things[room] {
            let Some(obj) = self.objs.get(o) else { continue };
            let gettable = obj.wear.contains(&Wear::Take) && count < cap_n && weight + self.total_weight(o) <= cap_w && self.can_see_obj(k, o);
            if gettable && obj.cost > best.map_or(1, |(_, c)| c) {
                best = Some((o, obj.cost));
            }
        }
        let Some((o, _)) = best else { return };
        self.put(o, Place::Carried(k));
        let (text, id) = (self.objs.get(o).unwrap().short.clone(), self.obj_id(o));
        // act(..., FALSE, ...): those who cannot see the mob hear "Someone gets ...".
        self.to_room(k, room, false, |who, who_id| Event::OccupantItem {
            who,
            who_id,
            action: ItemAction::Get,
            text: text.clone(),
            id: id.clone(),
            other: None,
            other_id: None,
            slot: None,
            liquid: None,
        });
        if self.objs.get(o).is_some_and(|x| x.kind == ItemType::Money) {
            // Mobs keep coins as gold too (get_check_money is the same for them).
            let amount = self.objs.get(o).unwrap().values.coins.unwrap_or(0);
            self.extract_obj(o);
            self.chars.get_mut(k).unwrap().gold += amount;
        }
    }

    /// CAN_GO, and not into no_mob or death rooms, and within the zone for stay_zone (MECHANICS §2.6).
    fn mob_may_go(&self, k: Key, d: usize) -> bool {
        let c = self.chars.get(k).unwrap();
        let Some(exit) = self.world.rooms[c.room].exits[d].as_ref() else { return false };
        let Some(to) = exit.to else { return false };
        if exit.door.as_ref().is_some_and(|door| door.state != mundi_content::names::DoorState::Open) {
            return false;
        }
        let flags = &self.world.rooms[to].flags;
        if flags.contains(&RoomFlag::NoMob) || flags.contains(&RoomFlag::Death) {
            return false;
        }
        if c.has_flag(MobFlag::StayZone) && self.world.rooms[to].zone != self.world.rooms[c.room].zone {
            return false;
        }
        !c.has(mundi_content::names::Affect::Charm)
    }
}
