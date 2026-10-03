//! Doors and lockable containers (MECHANICS §2.7, act.movement.c do_gen_door): open, close,
//! unlock, lock, pick. An exit's other side changes too when it leads straight back.

use mundi_content::names::{DoorState, ItemType};

use crate::*;

/// Container lock bits (values.doc): closeable, pickproof, closed, locked.
const CLOSEABLE: i64 = 1;
const PICKPROOF: i64 = 2;
const CLOSED: i64 = 4;
const LOCKED: i64 = 8;

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum DoorCmd {
    Open,
    Close,
    Unlock,
    Lock,
    Pick,
}

impl DoorCmd {
    fn name(self) -> &'static str {
        match self {
            DoorCmd::Open => "open",
            DoorCmd::Close => "close",
            DoorCmd::Unlock => "unlock",
            DoorCmd::Lock => "lock",
            DoorCmd::Pick => "pick",
        }
    }
}

#[derive(Clone, Copy)]
enum Thing {
    Exit(usize),
    Obj(Key),
}

impl Sim {
    fn door_fail(&mut self, k: Key, cmd: DoorCmd, reason: &str, word: Option<&str>) {
        self.deliver(k, Event::DoorFailed { command: cmd.name().into(), reason: reason.into(), word: word.map(Into::into) });
    }

    /// act.movement.c find_door.
    fn find_door(&mut self, k: Key, cmd: DoorCmd, kw: &str, dir: &str) -> Option<usize> {
        let room = self.chars.get(k).unwrap().room;
        if !dir.is_empty() {
            let d = DIRS.iter().position(|x| x.starts_with(&dir.to_lowercase()));
            let Some(d) = d else {
                self.door_fail(k, cmd, "not_direction", None);
                return None;
            };
            let Some(exit) = self.world.rooms[room].exits[d].as_ref() else {
                self.door_fail(k, cmd, "nothing_there", None);
                return None;
            };
            return match exit.door.as_ref().filter(|door| !door.keywords.is_empty()) {
                Some(door) if !door.keywords.iter().any(|w| w.eq_ignore_ascii_case(kw)) => {
                    self.door_fail(k, cmd, "no_such_there", Some(kw));
                    None
                }
                _ => Some(d),
            };
        }
        if kw.is_empty() {
            self.door_fail(k, cmd, "what_want", None);
            return None;
        }
        let autodoor = self.chars.get(k).is_some_and(|c| c.is_mob() || c.prefs.contains("autodoor"));
        for d in 0..DIRS.len() {
            let Some(door) = self.world.rooms[room].exits[d].as_ref().and_then(|e| e.door.as_ref()) else { continue };
            if !crate::items::isname(&kw.to_lowercase(), &door.keywords) {
                continue;
            }
            if !autodoor {
                return Some(d);
            }
            let fits = match cmd {
                DoorCmd::Open => door.state != DoorState::Open,
                DoorCmd::Close => door.state == DoorState::Open,
                DoorCmd::Lock => door.state != DoorState::Locked,
                DoorCmd::Unlock | DoorCmd::Pick => door.state == DoorState::Locked,
            };
            if fits {
                return Some(d);
            }
        }
        self.door_fail(k, cmd, if autodoor { "none_that_can" } else { "none_here" }, Some(kw));
        None
    }

    /// act.movement.c has_key: carried, or held, by prototype.
    fn has_key(&self, k: Key, key: Option<&str>) -> bool {
        let Some(key) = key else { return false };
        let c = self.chars.get(k).unwrap();
        let is_key = |o: &Key| self.objs.get(*o).and_then(|o| o.proto.as_deref()) == Some(key);
        c.inventory.iter().any(is_key) || c.equipment.get(&mundi_content::names::EquipPos::Hold).is_some_and(is_key)
    }

    pub(crate) fn door_cmd(&mut self, k: Key, cmd: DoorCmd, arg: &str) {
        let mut words = arg.split_whitespace();
        let (Some(kw), dir) = (words.next(), words.next().unwrap_or("")) else {
            return self.door_fail(k, cmd, "what", None);
        };
        let room = self.chars.get(k).unwrap().room;
        let inv = self.chars.get(k).unwrap().inventory.clone();
        // generic_find in the pack and the room; only a container counts.
        let obj = match crate::items::target(kw) {
            Some(crate::items::Target::One { word, nth }) => self.find_obj(k, &inv, &word, nth).or_else(|| self.find_obj(k, &self.things[room].clone(), &word, nth)),
            _ => None,
        }
        .filter(|o| self.objs.get(*o).is_some_and(|o| o.kind == ItemType::Container));
        let thing = match obj {
            Some(o) => Thing::Obj(o),
            None => match self.find_door(k, cmd, kw, dir) {
                Some(d) => Thing::Exit(d),
                None => return,
            },
        };
        let (openable, closed, locked, pickproof, key) = match thing {
            Thing::Obj(o) => {
                let obj = self.objs.get(o).unwrap();
                let f = obj.values.lock_flags.unwrap_or(0);
                let key = obj.proto.as_ref().and_then(|p| self.world.obj_protos.get(p)).and_then(|p| p.values.key.clone());
                (f & CLOSEABLE != 0, f & CLOSED != 0, f & LOCKED != 0, f & PICKPROOF != 0, key)
            }
            Thing::Exit(d) => match self.world.rooms[room].exits[d].as_ref().and_then(|e| e.door.as_ref()) {
                Some(door) => (true, door.state != DoorState::Open, door.state == DoorState::Locked, door.pickproof, door.key.clone()),
                None => (false, false, false, false, None),
            },
        };
        let (need_open, need_closed, need_locked, need_unlocked) = match cmd {
            DoorCmd::Open => (false, true, false, true),
            DoorCmd::Close => (true, false, false, false),
            DoorCmd::Unlock | DoorCmd::Pick => (false, true, true, false),
            DoorCmd::Lock => (false, true, false, true),
        };
        let autokey = self.chars.get(k).is_some_and(|c| !c.is_mob() && c.prefs.contains("autokey"));
        let has_key = self.has_key(k, key.as_deref());
        if !openable {
            self.door_fail(k, cmd, "cant", None)
        } else if closed && need_open {
            self.door_fail(k, cmd, "already_closed", None)
        } else if !closed && need_closed {
            self.door_fail(k, cmd, "currently_open", None)
        } else if !locked && need_locked {
            self.door_fail(k, cmd, "wasnt_locked", None)
        } else if locked && need_unlocked && autokey && has_key {
            self.door_fail(k, cmd, "locked_have_key", None);
            self.do_door(k, thing, DoorCmd::Unlock);
            self.do_door(k, thing, cmd);
        } else if locked && need_unlocked && autokey {
            self.door_fail(k, cmd, "locked_no_key", None)
        } else if locked && need_unlocked {
            self.door_fail(k, cmd, "seems_locked", None)
        } else if !has_key && matches!(cmd, DoorCmd::Lock | DoorCmd::Unlock) {
            self.door_fail(k, cmd, "no_key", None)
        } else if cmd == DoorCmd::Pick {
            // act.movement.c ok_pick: 1-101 against the skill plus dexterity's lock bonus.
            let dex = self.abilities_now(k).dex.clamp(0, 25) as usize;
            let skill = self.chars.get(k).unwrap().skills.get("pick lock").copied().unwrap_or(0) + self.tables.abilities.dexterity_skill.get(dex).map_or(0, |d| d.p_locks);
            let percent = self.rand(1, 101) as i32;
            if key.is_none() {
                self.door_fail(k, cmd, "no_keyhole", None)
            } else if pickproof {
                self.door_fail(k, cmd, "resists", None)
            } else if percent > skill {
                self.door_fail(k, cmd, "failed_pick", None)
            } else {
                self.do_door(k, thing, cmd)
            }
        } else {
            self.do_door(k, thing, cmd)
        }
    }

    /// act.movement.c do_doorcmd.
    fn do_door(&mut self, k: Key, thing: Thing, cmd: DoorCmd) {
        let room = self.chars.get(k).unwrap().room;
        let (who, who_id) = (SELF.to_string(), None::<String>);
        match thing {
            Thing::Obj(o) => {
                let obj = self.objs.get_mut(o).unwrap();
                let f = obj.values.lock_flags.get_or_insert(0);
                match cmd {
                    DoorCmd::Open => *f &= !CLOSED,
                    DoorCmd::Close => *f |= CLOSED,
                    DoorCmd::Lock => *f |= LOCKED,
                    DoorCmd::Unlock => *f &= !LOCKED,
                    DoorCmd::Pick => *f ^= LOCKED,
                }
                let (text, on_floor) = (obj.short.clone(), matches!(obj.place, Place::Room(_)));
                let id = self.obj_id(o);
                self.deliver(k, Event::DoorChanged { command: cmd.name().into(), who, who_id, door: None, text: Some(text.clone()), id: id.clone(), far: false });
                if on_floor {
                    let c = cmd.name().to_string();
                    self.to_room(k, room, false, |who, who_id| Event::DoorChanged { command: c.clone(), who, who_id, door: None, text: Some(text.clone()), id: id.clone(), far: false });
                }
            }
            Thing::Exit(d) => {
                let set = |s: DoorState| match cmd {
                    DoorCmd::Open => DoorState::Open,
                    DoorCmd::Close => DoorState::Closed,
                    DoorCmd::Lock => DoorState::Locked,
                    DoorCmd::Unlock => DoorState::Closed,
                    DoorCmd::Pick => if s == DoorState::Locked { DoorState::Closed } else { DoorState::Locked },
                };
                let exit = self.world.rooms[room].exits[d].as_mut().unwrap();
                let to = exit.to;
                let door = exit.door.as_mut().unwrap();
                door.state = set(door.state);
                let keyword = door.keyword.clone();
                // The other side, if it leads straight back.
                let rev = rev_dir(d);
                let mut far: Option<(RoomIx, Option<String>)> = None;
                if let Some(to) = to {
                    if let Some(b) = self.world.rooms[to].exits[rev].as_mut().filter(|b| b.to == Some(room)) {
                        if let Some(bd) = b.door.as_mut() {
                            bd.state = set(bd.state);
                            far = Some((to, bd.keyword.clone()));
                        }
                    }
                }
                self.deliver(k, Event::DoorChanged { command: cmd.name().into(), who, who_id, door: Some(keyword.clone().unwrap_or_else(|| "door".into())), text: None, id: None, far: false });
                let c = cmd.name().to_string();
                let kw = keyword.unwrap_or_else(|| "door".into());
                self.to_room(k, room, false, |who, who_id| Event::DoorChanged { command: c.clone(), who, who_id, door: Some(kw.clone()), text: None, id: None, far: false });
                if let (Some((other, back_kw)), DoorCmd::Open | DoorCmd::Close) = (far, cmd) {
                    let kw = back_kw.unwrap_or_else(|| "door".into());
                    for w in self.people[other].clone() {
                        self.deliver(w, Event::DoorChanged { command: c.clone(), who: String::new(), who_id: None, door: Some(kw.clone()), text: None, id: None, far: true });
                    }
                }
            }
        }
    }

    /// A container's closed bit (get and put check it).
    pub(crate) fn container_closed(&self, o: Key) -> bool {
        self.objs.get(o).is_some_and(|o| o.values.lock_flags.unwrap_or(0) & CLOSED != 0)
    }
}

/// The direction back: north-south, east-west, up-down.
fn rev_dir(d: usize) -> usize {
    [2, 3, 0, 1, 5, 4][d]
}
