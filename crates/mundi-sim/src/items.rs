//! Objects in hand (MECHANICS §13, §6.2, §6.3): finding them by keyword, get, drop, put, give,
//! inventory, equipment, wear, wield, hold, remove, eat, taste, drink, sip.

use mundi_content::names::{EquipPos, ItemType, ObjFlag, Wear};
use mundi_protocol::{Carried, ItemAction, ItemFailure, Worn};

use crate::*;

/// A typed object name (handler.c get_number, find_all_dots; MECHANICS §13.0).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Target {
    /// `all`
    All,
    /// `all.<word>`; empty word is "all of what?"
    AllDot(String),
    /// `<word>`, `<n>.<word>` or `last.<word>`.
    One { word: String, nth: Nth },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Nth {
    Index(usize),
    Last,
}

pub(crate) fn target(arg: &str) -> Option<Target> {
    let arg = arg.to_lowercase();
    if arg == "all" {
        return Some(Target::All);
    }
    if let Some(w) = arg.strip_prefix("all.") {
        return Some(Target::AllDot(w.to_string()));
    }
    if let Some((n, w)) = arg.split_once('.') {
        if n == "last" {
            return Some(Target::One { word: w.to_string(), nth: Nth::Last });
        }
        if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) {
            let n: usize = n.parse().ok()?;
            // "0.sword" names nothing (get_obj_in_list_vis returns NULL for 0).
            return if n == 0 { None } else { Some(Target::One { word: w.to_string(), nth: Nth::Index(n) }) };
        }
    }
    Some(Target::One { word: arg, nth: Nth::Index(1) })
}

/// handler.c isname: the word begins one of the keywords; a number must be the whole keyword.
pub(crate) fn isname(word: &str, keywords: &[String]) -> bool {
    if word.is_empty() {
        return false;
    }
    keywords.iter().any(|k| {
        let k = k.to_lowercase();
        if word.starts_with(|c: char| c.is_ascii_digit()) { k == word } else { k.starts_with(word) }
    })
}

/// tbaMUD's wear slots in order with the flag each needs (act.item.c perform_wear).
const SLOTS: [(EquipPos, Wear); 18] = [
    (EquipPos::Light, Wear::Take),
    (EquipPos::FingerRight, Wear::Finger),
    (EquipPos::FingerLeft, Wear::Finger),
    (EquipPos::Neck1, Wear::Neck),
    (EquipPos::Neck2, Wear::Neck),
    (EquipPos::Body, Wear::Body),
    (EquipPos::Head, Wear::Head),
    (EquipPos::Legs, Wear::Legs),
    (EquipPos::Feet, Wear::Feet),
    (EquipPos::Hands, Wear::Hands),
    (EquipPos::Arms, Wear::Arms),
    (EquipPos::Shield, Wear::Shield),
    (EquipPos::About, Wear::About),
    (EquipPos::Waist, Wear::Waist),
    (EquipPos::WristRight, Wear::Wrist),
    (EquipPos::WristLeft, Wear::Wrist),
    (EquipPos::Wield, Wear::Wield),
    (EquipPos::Hold, Wear::Take),
];

/// The body words `wear <obj> <where>` takes (act.item.c find_eq_pos keywords) and their first slot.
const PLACES: [(&str, EquipPos); 12] = [
    ("finger", EquipPos::FingerRight),
    ("neck", EquipPos::Neck1),
    ("body", EquipPos::Body),
    ("head", EquipPos::Head),
    ("legs", EquipPos::Legs),
    ("feet", EquipPos::Feet),
    ("hands", EquipPos::Hands),
    ("arms", EquipPos::Arms),
    ("shield", EquipPos::Shield),
    ("about", EquipPos::About),
    ("waist", EquipPos::Waist),
    ("wrist", EquipPos::WristRight),
];

fn slot_name(p: EquipPos) -> String {
    p.name().to_string()
}

impl Sim {
    /// handler.c CAN_SEE_OBJ: light, not invisible to them, and its carrier seen (MECHANICS §3.5).
    pub(crate) fn can_see_obj(&self, viewer: Key, o: Key) -> bool {
        let (Some(v), Some(obj)) = (self.chars.get(viewer), self.objs.get(o)) else { return false };
        if !self.can_see_in(viewer, v.room) {
            return false;
        }
        if obj.flags.contains(&ObjFlag::Invisible) && !v.has(mundi_content::names::Affect::DetectInvis) {
            return false;
        }
        match obj.place {
            Place::Carried(c) | Place::Worn(c, _) => self.can_see(viewer, c),
            _ => true,
        }
    }

    /// handler.c get_obj_in_list_vis.
    pub(crate) fn find_obj(&self, viewer: Key, list: &[Key], word: &str, nth: Nth) -> Option<Key> {
        let mut seen = 0;
        let mut last = None;
        for &o in list {
            let Some(obj) = self.objs.get(o) else { continue };
            if !isname(word, &obj.keywords) || !self.can_see_obj(viewer, o) {
                continue;
            }
            match nth {
                Nth::Last => last = Some(o),
                Nth::Index(n) => {
                    seen += 1;
                    if seen == n {
                        return Some(o);
                    }
                }
            }
        }
        last
    }

    fn short(&self, o: Key) -> String {
        self.objs.get(o).map(|o| o.short.clone()).unwrap_or_default()
    }

    fn fail(&mut self, k: Key, action: ItemAction, reason: ItemFailure) {
        self.deliver(k, Event::ItemFailed { action, reason, text: None, id: None, keyword: None, other: None, other_id: None, slot: None });
    }

    fn fail_obj(&mut self, k: Key, action: ItemAction, reason: ItemFailure, o: Key) {
        let (text, id) = (self.short(o), self.obj_id(o));
        self.deliver(k, Event::ItemFailed { action, reason, text: Some(text), id, keyword: None, other: None, other_id: None, slot: None });
    }

    fn fail_word(&mut self, k: Key, action: ItemAction, reason: ItemFailure, word: &str) {
        self.deliver(k, Event::ItemFailed { action, reason, text: None, id: None, keyword: Some(word.to_string()), other: None, other_id: None, slot: None });
    }

    /// Others in the room see an object action (act(..., TRUE, ...): only those who see the actor).
    fn others_see(&mut self, k: Key, action: ItemAction, o: Key, other: Option<(String, Option<String>)>, slot: Option<String>, liquid: Option<String>) {
        let room = self.chars.get(k).unwrap().room;
        let (text, id) = (self.short(o), self.obj_id(o));
        let (other, other_id) = other.map_or((None, None), |(t, i)| (Some(t), i));
        self.to_room(k, room, true, |who, who_id| Event::OccupantItem {
            who,
            who_id,
            action,
            text: text.clone(),
            id: id.clone(),
            other: other.clone(),
            other_id: other_id.clone(),
            slot: slot.clone(),
            liquid: liquid.clone(),
        });
    }

    /// `get <obj>`, `get all[.<w>]`, `get [n] <obj> <container>` (act.item.c do_get).
    pub(crate) fn get(&mut self, k: Key, arg: &str) {
        let args: Vec<&str> = arg.split_whitespace().collect();
        let (mut how_many, mut args) = (1usize, args.as_slice());
        if args.is_empty() {
            return self.fail(k, ItemAction::Get, ItemFailure::What);
        }
        if args.len() >= 2 && args[0].chars().all(|c| c.is_ascii_digit()) {
            how_many = args[0].parse().unwrap_or(1);
            args = &args[1..];
        }
        match args {
            [what] => self.get_from_room(k, what, how_many),
            [what, from, ..] => {
                let room = self.chars.get(k).unwrap().room;
                let inv = self.chars.get(k).unwrap().inventory.clone();
                let Some(Target::One { word, nth }) = target(from) else {
                    return self.fail_word(k, ItemAction::Get, ItemFailure::NoContainer, from);
                };
                let (cont, carried) = match self.find_obj(k, &inv, &word, nth) {
                    Some(c) => (Some(c), true),
                    None => (self.find_obj(k, &self.things[room].clone(), &word, nth), false),
                };
                let Some(cont) = cont else {
                    return self.fail_word(k, ItemAction::Get, ItemFailure::NoContainer, &word);
                };
                if self.objs.get(cont).unwrap().kind != ItemType::Container {
                    return self.fail_obj(k, ItemAction::Get, ItemFailure::NotContainer, cont);
                }
                self.get_from_container(k, cont, what, how_many, carried);
            }
            [] => self.fail(k, ItemAction::Get, ItemFailure::What),
        }
    }

    fn get_from_room(&mut self, k: Key, what: &str, how_many: usize) {
        let room = self.chars.get(k).unwrap().room;
        match target(what) {
            None => self.fail_word(k, ItemAction::Get, ItemFailure::NotHere, what),
            Some(Target::One { word, nth }) => {
                let list = self.things[room].clone();
                let Some(first) = self.find_obj(k, &list, &word, nth) else {
                    let scenery = self.world.rooms[room].extras.iter().any(|e| isname(&word, e));
                    let reason = if scenery { ItemFailure::CantTake } else { ItemFailure::NotHere };
                    return self.fail_word(k, ItemAction::Get, reason, &word);
                };
                let start = list.iter().position(|o| *o == first).unwrap();
                let mut got = 0;
                for &o in &list[start..] {
                    if got == how_many {
                        break;
                    }
                    if self.objs.get(o).is_some_and(|x| isname(&word, &x.keywords)) && self.can_see_obj(k, o) {
                        self.take(k, o, None);
                        got += 1;
                    }
                }
            }
            Some(Target::AllDot(w)) if w.is_empty() => self.fail(k, ItemAction::Get, ItemFailure::AllOfWhat),
            Some(t) => {
                let word = if let Target::AllDot(w) = &t { Some(w.clone()) } else { None };
                let list = self.things[room].clone();
                let mut found = false;
                for o in list {
                    let matches = word.as_deref().is_none_or(|w| self.objs.get(o).is_some_and(|x| isname(w, &x.keywords)));
                    if matches && self.can_see_obj(k, o) {
                        found = true;
                        self.take(k, o, None);
                    }
                }
                if !found {
                    match word {
                        None => self.fail(k, ItemAction::Get, ItemFailure::Nothing),
                        Some(w) => self.fail_word(k, ItemAction::Get, ItemFailure::NoneOf, &w),
                    }
                }
            }
        }
    }

    fn get_from_container(&mut self, k: Key, cont: Key, what: &str, how_many: usize, carried: bool) {
        if self.container_closed(cont) {
            return self.fail_obj(k, ItemAction::Get, ItemFailure::Closed, cont);
        }
        let contents = self.objs.get(cont).unwrap().contents.clone();
        let other = (self.short(cont), self.obj_id(cont));
        match target(what) {
            Some(Target::One { word, nth }) => {
                let Some(first) = self.find_obj(k, &contents, &word, nth) else {
                    let (text, id) = other;
                    return self.deliver(k, Event::ItemFailed {
                        action: ItemAction::Get,
                        reason: ItemFailure::NotHere,
                        text: None,
                        id: None,
                        keyword: Some(word),
                        other: Some(text),
                        other_id: id,
                        slot: None,
                    });
                };
                let start = contents.iter().position(|o| *o == first).unwrap();
                let mut got = 0;
                for &o in &contents[start..] {
                    if got == how_many {
                        break;
                    }
                    if self.objs.get(o).is_some_and(|x| isname(&word, &x.keywords)) && self.can_see_obj(k, o) {
                        self.take_from(k, o, cont, carried);
                        got += 1;
                    }
                }
            }
            Some(Target::AllDot(w)) if w.is_empty() => self.fail(k, ItemAction::Get, ItemFailure::AllOfWhat),
            Some(t) => {
                let word = if let Target::AllDot(w) = &t { Some(w.clone()) } else { None };
                let mut found = false;
                for o in contents {
                    let matches = word.as_deref().is_none_or(|w| self.objs.get(o).is_some_and(|x| isname(w, &x.keywords)));
                    if matches && self.can_see_obj(k, o) {
                        found = true;
                        self.take_from(k, o, cont, carried);
                    }
                }
                if !found {
                    let (text, id) = other;
                    let (reason, keyword) = match word {
                        None => (ItemFailure::Empty, None),
                        Some(w) => (ItemFailure::NoneOf, Some(w)),
                    };
                    self.deliver(k, Event::ItemFailed { action: ItemAction::Get, reason, text: Some(text), id, keyword, other: None, other_id: None, slot: None });
                }
            }
            None => self.fail_word(k, ItemAction::Get, ItemFailure::NotHere, what),
        }
    }

    /// act.item.c can_take_obj (MECHANICS §13.1, §13.2).
    fn can_take(&mut self, k: Key, o: Key) -> bool {
        let obj = self.objs.get(o).unwrap();
        if !obj.wear.contains(&Wear::Take) {
            self.fail_obj(k, ItemAction::Get, ItemFailure::CantTake, o);
            return false;
        }
        if !self.chars.get(k).unwrap().is_mob() {
            let (weight, count) = self.carrying(k);
            if count >= self.can_carry_n(k) {
                self.fail_obj(k, ItemAction::Get, ItemFailure::TooMany, o);
                return false;
            }
            if weight + self.total_weight(o) > self.can_carry_w(k) {
                self.fail_obj(k, ItemAction::Get, ItemFailure::TooHeavy, o);
                return false;
            }
        }
        true
    }

    fn take(&mut self, k: Key, o: Key, _from: Option<Key>) {
        if !self.can_take(k, o) {
            return;
        }
        self.put(o, Place::Carried(k));
        let (text, id) = (self.short(o), self.obj_id(o));
        self.deliver(k, Event::Got { text, id, from: None, from_id: None });
        self.others_see(k, ItemAction::Get, o, None, None, None);
        self.check_money(k, o);
    }

    fn take_from(&mut self, k: Key, o: Key, cont: Key, carried: bool) {
        if !carried && !self.can_take(k, o) {
            return;
        }
        if self.carrying(k).1 >= self.can_carry_n(k) {
            return self.fail_obj(k, ItemAction::Get, ItemFailure::HoldNoMore, o);
        }
        self.put(o, Place::Carried(k));
        let (text, id, from, from_id) = (self.short(o), self.obj_id(o), self.short(cont), self.obj_id(cont));
        self.deliver(k, Event::Got { text, id, from: Some(from.clone()), from_id: from_id.clone() });
        self.others_see(k, ItemAction::Get, o, Some((from, from_id)), None, None);
        self.check_money(k, o);
    }

    /// act.item.c get_check_money: coins become gold at once.
    fn check_money(&mut self, k: Key, o: Key) {
        let Some(obj) = self.objs.get(o) else { return };
        if obj.kind != ItemType::Money {
            return;
        }
        let amount = obj.values.coins.unwrap_or(0);
        self.extract_obj(o);
        if amount > 0 {
            self.chars.get_mut(k).unwrap().gold += amount;
            self.deliver(k, Event::Coins { amount });
        }
    }

    /// utils.h CAN_CARRY_W, CAN_CARRY_N (MECHANICS §13.1).
    pub(crate) fn can_carry_w(&self, k: Key) -> i64 {
        let c = self.chars.get(k).unwrap();
        self.strength(c).carry_w as i64
    }

    pub(crate) fn can_carry_n(&self, k: Key) -> usize {
        let c = self.chars.get(k).unwrap();
        (5 + c.abilities.dex / 2 + c.level / 2).max(0) as usize
    }

    /// The strength row for a character (MECHANICS §7.4: 18/xx is rows 26-30).
    pub(crate) fn strength(&self, c: &Char) -> mundi_content::tables::Strength {
        let a = &c.abilities;
        let i = if a.str == 18 && a.str_add > 0 {
            match a.str_add {
                ..=50 => 26,
                ..=75 => 27,
                ..=90 => 28,
                ..=99 => 29,
                _ => 30,
            }
        } else {
            a.str.clamp(0, 25) as usize
        };
        self.tables.abilities.strength[i.min(self.tables.abilities.strength.len() - 1)]
    }

    /// `drop <obj>`, `drop all[.<w>]`, `drop <n> <obj>` (act.item.c do_drop, perform_drop).
    pub(crate) fn drop_cmd(&mut self, k: Key, arg: &str) {
        let args: Vec<&str> = arg.split_whitespace().collect();
        let inv = self.chars.get(k).unwrap().inventory.clone();
        match args.as_slice() {
            [] => self.fail(k, ItemAction::Drop, ItemFailure::What),
            [n, what, ..] if n.chars().all(|c| c.is_ascii_digit()) => {
                let n: usize = n.parse().unwrap_or(1);
                let mut matching: Vec<Key> = inv.iter().copied().filter(|o| self.objs.get(*o).is_some_and(|x| isname(what, &x.keywords)) && self.can_see_obj(k, *o)).collect();
                if matching.is_empty() {
                    return self.fail_word(k, ItemAction::Drop, ItemFailure::NoneOf, what);
                }
                matching.truncate(n);
                for o in matching {
                    self.drop_one(k, o);
                }
            }
            [what, ..] => match target(what) {
                Some(Target::All) => {
                    if inv.is_empty() {
                        return self.fail(k, ItemAction::Drop, ItemFailure::Nothing);
                    }
                    for o in inv {
                        self.drop_one(k, o);
                    }
                }
                Some(Target::AllDot(w)) if w.is_empty() => self.fail(k, ItemAction::Drop, ItemFailure::AllOfWhat),
                Some(Target::AllDot(w)) => {
                    let matching: Vec<Key> = inv.iter().copied().filter(|o| self.objs.get(*o).is_some_and(|x| isname(&w, &x.keywords)) && self.can_see_obj(k, *o)).collect();
                    if matching.is_empty() {
                        return self.fail_word(k, ItemAction::Drop, ItemFailure::NoneOf, &w);
                    }
                    for o in matching {
                        self.drop_one(k, o);
                    }
                }
                Some(Target::One { word, nth }) => match self.find_obj(k, &inv, &word, nth) {
                    Some(o) => self.drop_one(k, o),
                    None => self.fail_word(k, ItemAction::Drop, ItemFailure::NotCarried, &word),
                },
                None => self.fail_word(k, ItemAction::Drop, ItemFailure::NotCarried, what),
            },
        }
    }

    fn drop_one(&mut self, k: Key, o: Key) {
        if self.objs.get(o).unwrap().flags.contains(&ObjFlag::NoDrop) {
            return self.fail_obj(k, ItemAction::Drop, ItemFailure::Cursed, o);
        }
        let (text, id) = (self.short(o), self.obj_id(o));
        self.deliver(k, Event::Used { action: ItemAction::Drop, text, id, into: None, into_id: None, slot: None, liquid: None });
        self.others_see(k, ItemAction::Drop, o, None, None, None);
        let room = self.chars.get(k).unwrap().room;
        // obj_to_room appends in this tbaMUD (handler.c:772-793): the newest is last.
        self.put(o, Place::Room(room));
    }

    /// `put <obj>|all|all.<w> <container>` (act.item.c do_put, perform_put).
    pub(crate) fn put_cmd(&mut self, k: Key, arg: &str) {
        let args: Vec<&str> = arg.split_whitespace().collect();
        let (what, into) = match args.as_slice() {
            [] => return self.fail(k, ItemAction::Put, ItemFailure::What),
            [w] => {
                let them = !matches!(target(w), Some(Target::One { .. }));
                return self.fail_word(k, ItemAction::Put, ItemFailure::IntoWhat, if them { "them" } else { "it" });
            }
            [w, c, ..] => (*w, *c),
        };
        let room = self.chars.get(k).unwrap().room;
        let inv = self.chars.get(k).unwrap().inventory.clone();
        let Some(Target::One { word, nth }) = target(into) else {
            return self.fail_word(k, ItemAction::Put, ItemFailure::NotHere, into);
        };
        let cont = self.find_obj(k, &inv, &word, nth).or_else(|| self.find_obj(k, &self.things[room].clone(), &word, nth));
        let Some(cont) = cont else {
            return self.fail_word(k, ItemAction::Put, ItemFailure::NotHere, &word);
        };
        if self.objs.get(cont).unwrap().kind != ItemType::Container {
            return self.fail_obj(k, ItemAction::Put, ItemFailure::NotContainer, cont);
        }
        if self.container_closed(cont) {
            return self.fail(k, ItemAction::Put, ItemFailure::Closed);
        }
        match target(what) {
            Some(Target::One { word, nth }) => match self.find_obj(k, &inv, &word, nth) {
                None => self.fail_word(k, ItemAction::Put, ItemFailure::NotCarried, &word),
                Some(o) if o == cont => self.fail(k, ItemAction::Put, ItemFailure::IntoItself),
                Some(o) => self.put_one(k, o, cont),
            },
            Some(t) => {
                let word = if let Target::AllDot(w) = &t { Some(w.clone()) } else { None };
                let mut found = false;
                for o in inv {
                    if o == cont || !self.can_see_obj(k, o) {
                        continue;
                    }
                    if word.as_deref().is_none_or(|w| self.objs.get(o).is_some_and(|x| isname(w, &x.keywords))) {
                        found = true;
                        self.put_one(k, o, cont);
                    }
                }
                if !found {
                    match word {
                        None => self.fail(k, ItemAction::Put, ItemFailure::Nothing),
                        Some(w) => self.fail_word(k, ItemAction::Put, ItemFailure::NoneOf, &w),
                    }
                }
            }
            None => self.fail_word(k, ItemAction::Put, ItemFailure::NotCarried, what),
        }
    }

    fn put_one(&mut self, k: Key, o: Key, cont: Key) {
        let c = self.objs.get(cont).unwrap();
        let other = (self.short(cont), self.obj_id(cont));
        let fail = |reason| Event::ItemFailed {
            action: ItemAction::Put,
            reason,
            text: Some(self.short(o)),
            id: self.obj_id(o),
            keyword: None,
            other: Some(other.0.clone()),
            other_id: other.1.clone(),
            slot: None,
        };
        let capacity = c.values.capacity.unwrap_or(0);
        if c.values.corpse() {
            let e = fail(ItemFailure::IntoCorpse);
            return self.deliver(k, e);
        }
        if capacity > 0 && self.total_weight(cont) + self.total_weight(o) > capacity {
            let e = fail(ItemFailure::WontFit);
            return self.deliver(k, e);
        }
        if self.objs.get(o).unwrap().flags.contains(&ObjFlag::NoDrop) && matches!(c.place, Place::Room(_)) {
            let e = fail(ItemFailure::OutOfHand);
            return self.deliver(k, e);
        }
        self.put(o, Place::In(cont));
        self.others_see(k, ItemAction::Put, o, Some(other.clone()), None, None);
        let (text, id) = (self.short(o), self.obj_id(o));
        self.deliver(k, Event::Used { action: ItemAction::Put, text, id, into: Some(other.0), into_id: other.1, slot: None, liquid: None });
    }

    /// `give <obj> <person>` (act.item.c do_give, perform_give). Gold comes with shops.
    pub(crate) fn give(&mut self, k: Key, arg: &str) {
        let args: Vec<&str> = arg.split_whitespace().collect();
        let (what, to) = match args.as_slice() {
            [] | [_] => return self.fail(k, ItemAction::Give, ItemFailure::What),
            [w, t, ..] => (*w, *t),
        };
        let Some(vict) = self.find_char_room(k, to) else {
            return self.fail(k, ItemAction::Give, ItemFailure::NoPerson);
        };
        if vict == k {
            return self.fail(k, ItemAction::Give, ItemFailure::GiveSelf);
        }
        let inv = self.chars.get(k).unwrap().inventory.clone();
        match target(what) {
            Some(Target::One { word, nth }) => match self.find_obj(k, &inv, &word, nth) {
                Some(o) => self.give_one(k, o, vict),
                None => self.fail_word(k, ItemAction::Give, ItemFailure::NotCarried, &word),
            },
            Some(Target::AllDot(w)) if w.is_empty() => self.fail(k, ItemAction::Give, ItemFailure::AllOfWhat),
            Some(t) => {
                let word = if let Target::AllDot(w) = &t { Some(w.clone()) } else { None };
                let mut found = false;
                for o in inv {
                    if self.can_see_obj(k, o) && word.as_deref().is_none_or(|w| self.objs.get(o).is_some_and(|x| isname(w, &x.keywords))) {
                        found = true;
                        self.give_one(k, o, vict);
                    }
                }
                if !found {
                    self.fail(k, ItemAction::Give, ItemFailure::Nothing);
                }
            }
            None => self.fail_word(k, ItemAction::Give, ItemFailure::NotCarried, what),
        }
    }

    fn give_one(&mut self, k: Key, o: Key, vict: Key) {
        let v = self.chars.get(vict).unwrap();
        let (vname, vid) = (v.name.clone(), self.id_of(vict));
        let fail = |s: &Sim, reason| Event::ItemFailed {
            action: ItemAction::Give,
            reason,
            text: Some(s.short(o)),
            id: s.obj_id(o),
            keyword: None,
            other: Some(vname.clone()),
            other_id: vid.clone(),
            slot: None,
        };
        if self.objs.get(o).unwrap().flags.contains(&ObjFlag::NoDrop) {
            let e = fail(self, ItemFailure::Cursed);
            return self.deliver(k, e);
        }
        let (weight, count) = self.carrying(vict);
        if count >= self.can_carry_n(vict) {
            let e = fail(self, ItemFailure::HandsFull);
            return self.deliver(k, e);
        }
        if weight + self.total_weight(o) > self.can_carry_w(vict) {
            let e = fail(self, ItemFailure::CantCarry);
            return self.deliver(k, e);
        }
        self.put(o, Place::Carried(vict));
        let (text, id) = (self.short(o), self.obj_id(o));
        let shown_to = if self.can_see(k, vict) { vname.clone() } else { "someone".into() };
        self.deliver(k, Event::Gave { text: text.clone(), id: id.clone(), to: shown_to, to_id: if self.can_see(k, vict) { vid.clone() } else { None } });
        let me = self.chars.get(k).unwrap().name.clone();
        let (from, from_id) = if self.can_see(vict, k) { (me, self.id_of(k)) } else { ("someone".into(), None) };
        self.deliver(vict, Event::Received { text: text.clone(), id: id.clone(), from, from_id });
        // TO_NOTVICT, hide-invisible.
        let room = self.chars.get(k).unwrap().room;
        for w in self.people[room].clone() {
            if w == k || w == vict || !self.can_see(w, k) || !self.awake_listener(w) {
                continue;
            }
            let (other, other_id) = if self.can_see(w, vict) { (vname.clone(), vid.clone()) } else { ("someone".into(), None) };
            let me = self.chars.get(k).unwrap().name.clone();
            let who_id = self.id_of(k);
            self.deliver(w, Event::OccupantItem { who: me, who_id, action: ItemAction::Give, text: text.clone(), id: id.clone(), other: Some(other), other_id, slot: None, liquid: None });
        }
    }

    /// Someone in the room named by a typed word (handler.c get_char_room_vis).
    pub(crate) fn find_char_room(&self, k: Key, arg: &str) -> Option<Key> {
        let room = self.chars.get(k)?.room;
        let (word, nth) = match target(arg)? {
            Target::One { word, nth } => (word, nth),
            _ => return None,
        };
        if word == "self" || word == "me" {
            return Some(k);
        }
        let mut seen = 0;
        let mut last = None;
        for &c in &self.people[room] {
            let Some(ch) = self.chars.get(c) else { continue };
            if !isname(&word, &ch.keywords) || !self.can_see(k, c) {
                continue;
            }
            match nth {
                Nth::Last => last = Some(c),
                Nth::Index(n) => {
                    seen += 1;
                    if seen == n {
                        return Some(c);
                    }
                }
            }
        }
        last
    }

    fn awake_listener(&self, w: Key) -> bool {
        self.chars.get(w).is_some_and(|c| c.position > Position::Sleeping && c.linked)
    }

    /// `inventory` (act.informative.c do_inventory).
    pub(crate) fn inventory(&mut self, k: Key) {
        let inv = self.chars.get(k).unwrap().inventory.clone();
        let items = self.object_list(k, &inv).into_iter().map(|r| Carried { id: r.id, text: r.text, count: r.count }).collect();
        // Carried things show by their short description: the list's `text` is the short here.
        let items = self.shorts(k, &inv, items);
        self.deliver(k, Event::Inventory { items });
    }

    fn shorts(&self, _k: Key, list: &[Key], items: Vec<Carried>) -> Vec<Carried> {
        items
            .into_iter()
            .map(|mut c| {
                if let Some(o) = list.iter().find(|o| self.obj_id(**o) == c.id) {
                    c.text = self.short(*o);
                }
                c
            })
            .collect()
    }

    /// `equipment` (act.informative.c do_equipment).
    pub(crate) fn equipment(&mut self, k: Key) {
        let eq: Vec<(EquipPos, Key)> = self.chars.get(k).unwrap().equipment.iter().map(|(p, o)| (*p, *o)).collect();
        let mut slots = Vec::new();
        for (pos, _) in SLOTS {
            if let Some((_, o)) = eq.iter().find(|(p, _)| *p == pos) {
                let (text, id) = if self.can_see_obj(k, *o) { (self.short(*o), self.obj_id(*o)) } else { ("something".into(), None) };
                slots.push(Worn { slot: slot_name(pos), id, text });
            }
        }
        self.deliver(k, Event::Equipment { slots });
    }

    /// `wear <obj> [<where>]`, `wear all[.<w>]` (act.item.c do_wear).
    pub(crate) fn wear(&mut self, k: Key, arg: &str) {
        let args: Vec<&str> = arg.split_whitespace().collect();
        let inv = self.chars.get(k).unwrap().inventory.clone();
        let level = self.chars.get(k).unwrap().level;
        let Some(what) = args.first() else {
            return self.fail(k, ItemAction::Wear, ItemFailure::What);
        };
        let place = args.get(1).copied();
        match target(what) {
            Some(Target::All) | Some(Target::AllDot(_)) if place.is_some() => {
                self.fail(k, ItemAction::Wear, ItemFailure::BadLocation)
            }
            Some(Target::All) => {
                let mut any = false;
                for o in inv {
                    if !self.can_see_obj(k, o) {
                        continue;
                    }
                    let Some(pos) = self.eq_pos(o, None) else { continue };
                    any = true;
                    if level < self.objs.get(o).unwrap().level {
                        self.fail(k, ItemAction::Wear, ItemFailure::Level);
                    } else {
                        self.perform_wear(k, o, pos, ItemAction::Wear);
                    }
                }
                if !any {
                    self.fail(k, ItemAction::Wear, ItemFailure::Nothing);
                }
            }
            Some(Target::AllDot(w)) if w.is_empty() => self.fail(k, ItemAction::Wear, ItemFailure::AllOfWhat),
            Some(Target::AllDot(w)) => {
                let matching: Vec<Key> = inv.iter().copied().filter(|o| self.objs.get(*o).is_some_and(|x| isname(&w, &x.keywords)) && self.can_see_obj(k, *o)).collect();
                if matching.is_empty() {
                    return self.fail_word(k, ItemAction::Wear, ItemFailure::NoneOf, &w);
                }
                for o in matching {
                    if level < self.objs.get(o).unwrap().level {
                        self.fail(k, ItemAction::Wear, ItemFailure::Level);
                    } else if let Some(pos) = self.eq_pos(o, None) {
                        self.perform_wear(k, o, pos, ItemAction::Wear);
                    } else {
                        self.fail_obj(k, ItemAction::Wear, ItemFailure::CantWear, o);
                    }
                }
            }
            Some(Target::One { word, nth }) => {
                let Some(o) = self.find_obj(k, &inv, &word, nth) else {
                    return self.fail_word(k, ItemAction::Wear, ItemFailure::NotCarried, &word);
                };
                if level < self.objs.get(o).unwrap().level {
                    return self.fail(k, ItemAction::Wear, ItemFailure::Level);
                }
                if let Some(p) = place {
                    let Some(&(_, pos)) = PLACES.iter().find(|(w, _)| w.starts_with(&p.to_lowercase())) else {
                        return self.fail_word(k, ItemAction::Wear, ItemFailure::BadLocation, p);
                    };
                    return self.perform_wear(k, o, pos, ItemAction::Wear);
                }
                match self.eq_pos(o, None) {
                    Some(pos) => self.perform_wear(k, o, pos, ItemAction::Wear),
                    None => self.fail_obj(k, ItemAction::Wear, ItemFailure::CantWear, o),
                }
            }
            None => self.fail_word(k, ItemAction::Wear, ItemFailure::NotCarried, what),
        }
    }

    /// act.item.c find_eq_pos without a body word: the last of these flags the object has wins.
    fn eq_pos(&self, o: Key, _place: Option<&str>) -> Option<EquipPos> {
        let wear = &self.objs.get(o)?.wear;
        let mut pos = None;
        for (w, p) in [
            (Wear::Finger, EquipPos::FingerRight),
            (Wear::Neck, EquipPos::Neck1),
            (Wear::Body, EquipPos::Body),
            (Wear::Head, EquipPos::Head),
            (Wear::Legs, EquipPos::Legs),
            (Wear::Feet, EquipPos::Feet),
            (Wear::Hands, EquipPos::Hands),
            (Wear::Arms, EquipPos::Arms),
            (Wear::Shield, EquipPos::Shield),
            (Wear::About, EquipPos::About),
            (Wear::Waist, EquipPos::Waist),
            (Wear::Wrist, EquipPos::WristRight),
        ] {
            if wear.contains(&w) {
                pos = Some(p);
            }
        }
        pos
    }

    /// act.item.c perform_wear and equip_char's alignment and class check (MECHANICS §13.3).
    fn perform_wear(&mut self, k: Key, o: Key, mut pos: EquipPos, action: ItemAction) {
        let need = SLOTS.iter().find(|(p, _)| *p == pos).map(|(_, w)| *w).unwrap();
        if !self.objs.get(o).unwrap().wear.contains(&need) {
            return self.fail_obj(k, action, ItemFailure::CantWearThere, o);
        }
        let worn = |s: &Sim, p: EquipPos| s.chars.get(k).unwrap().equipment.contains_key(&p);
        let second = match pos {
            EquipPos::FingerRight => Some(EquipPos::FingerLeft),
            EquipPos::Neck1 => Some(EquipPos::Neck2),
            EquipPos::WristRight => Some(EquipPos::WristLeft),
            _ => None,
        };
        if let Some(s) = second {
            if worn(self, pos) {
                pos = s;
            }
        }
        if worn(self, pos) {
            return self.deliver(k, Event::ItemFailed {
                action,
                reason: ItemFailure::AlreadyWearing,
                text: None,
                id: None,
                keyword: None,
                other: None,
                other_id: None,
                slot: Some(slot_name(pos)),
            });
        }
        // The message first, then equip_char may refuse it (handler.c:590-596).
        self.others_see(k, action, o, None, Some(slot_name(pos)), None);
        let (text, id) = (self.short(o), self.obj_id(o));
        self.deliver(k, Event::Used { action, text: text.clone(), id: id.clone(), into: None, into_id: None, slot: Some(slot_name(pos)), liquid: None });
        if self.invalid_for(k, o) {
            let room = self.chars.get(k).unwrap().room;
            self.deliver(k, Event::Zapped { who: SELF.into(), who_id: None, text: text.clone(), id: id.clone() });
            self.to_room(k, room, true, |who, who_id| Event::Zapped { who, who_id, text: text.clone(), id: id.clone() });
            return;
        }
        self.put(o, Place::Worn(k, pos));
    }

    /// handler.c invalid_align, class.c invalid_class (MECHANICS §13.3).
    fn invalid_for(&self, k: Key, o: Key) -> bool {
        let c = self.chars.get(k).unwrap();
        let f = &self.objs.get(o).unwrap().flags;
        let align = c.alignment;
        let bad_align = (f.contains(&ObjFlag::AntiEvil) && align <= -350)
            || (f.contains(&ObjFlag::AntiGood) && align >= 350)
            || (f.contains(&ObjFlag::AntiNeutral) && align > -350 && align < 350);
        let bad_class = match c.class {
            Some(Class::MagicUser) => f.contains(&ObjFlag::AntiMage),
            Some(Class::Cleric) => f.contains(&ObjFlag::AntiCleric),
            Some(Class::Thief) => f.contains(&ObjFlag::AntiThief),
            Some(Class::Warrior) => f.contains(&ObjFlag::AntiWarrior),
            None => false,
        };
        bad_align || bad_class
    }

    /// `wield <obj>` (act.item.c do_wield).
    pub(crate) fn wield(&mut self, k: Key, arg: &str) {
        let Some(word) = arg.split_whitespace().next() else {
            return self.fail(k, ItemAction::Wield, ItemFailure::What);
        };
        let inv = self.chars.get(k).unwrap().inventory.clone();
        let Some(Target::One { word, nth }) = target(word) else {
            return self.fail_word(k, ItemAction::Wield, ItemFailure::NotCarried, word);
        };
        let Some(o) = self.find_obj(k, &inv, &word, nth) else {
            return self.fail_word(k, ItemAction::Wield, ItemFailure::NotCarried, &word);
        };
        let obj = self.objs.get(o).unwrap();
        let c = self.chars.get(k).unwrap();
        if !obj.wear.contains(&Wear::Wield) {
            self.fail(k, ItemAction::Wield, ItemFailure::CantWield)
        } else if obj.weight > self.strength(c).wield_w as i64 {
            self.fail(k, ItemAction::Wield, ItemFailure::TooHeavyToWield)
        } else if c.level < obj.level {
            self.fail(k, ItemAction::Wield, ItemFailure::Level)
        } else {
            self.perform_wear(k, o, EquipPos::Wield, ItemAction::Wield)
        }
    }

    /// `hold <obj>`, `grab <obj>` (act.item.c do_grab): a light goes to the light slot.
    pub(crate) fn hold(&mut self, k: Key, arg: &str) {
        let Some(word) = arg.split_whitespace().next() else {
            return self.fail(k, ItemAction::Hold, ItemFailure::What);
        };
        let inv = self.chars.get(k).unwrap().inventory.clone();
        let Some(Target::One { word, nth }) = target(word) else {
            return self.fail_word(k, ItemAction::Hold, ItemFailure::NotCarried, word);
        };
        let Some(o) = self.find_obj(k, &inv, &word, nth) else {
            return self.fail_word(k, ItemAction::Hold, ItemFailure::NotCarried, &word);
        };
        let obj = self.objs.get(o).unwrap();
        if self.chars.get(k).unwrap().level < obj.level {
            return self.fail(k, ItemAction::Hold, ItemFailure::Level);
        }
        if obj.kind == ItemType::Light {
            return self.perform_wear(k, o, EquipPos::Light, ItemAction::Hold);
        }
        let holdable = obj.wear.contains(&Wear::Hold) || matches!(obj.kind, ItemType::Wand | ItemType::Staff | ItemType::Scroll | ItemType::Potion);
        if !holdable {
            return self.fail(k, ItemAction::Hold, ItemFailure::CantHold);
        }
        self.perform_wear(k, o, EquipPos::Hold, ItemAction::Hold)
    }

    /// `remove <obj>|all|all.<w>` (act.item.c do_remove, perform_remove).
    pub(crate) fn remove(&mut self, k: Key, arg: &str) {
        let Some(what) = arg.split_whitespace().next() else {
            return self.fail(k, ItemAction::Remove, ItemFailure::What);
        };
        let eq: Vec<(EquipPos, Key)> = SLOTS
            .iter()
            .filter_map(|(p, _)| self.chars.get(k).unwrap().equipment.get(p).map(|o| (*p, *o)))
            .collect();
        match target(what) {
            Some(Target::All) => {
                if eq.is_empty() {
                    return self.fail(k, ItemAction::Remove, ItemFailure::Nothing);
                }
                for (pos, _) in eq {
                    self.remove_one(k, pos);
                }
            }
            Some(Target::AllDot(w)) if w.is_empty() => self.fail(k, ItemAction::Remove, ItemFailure::AllOfWhat),
            Some(Target::AllDot(w)) => {
                let matching: Vec<EquipPos> = eq.iter().filter(|(_, o)| self.objs.get(*o).is_some_and(|x| isname(&w, &x.keywords)) && self.can_see_obj(k, *o)).map(|(p, _)| *p).collect();
                if matching.is_empty() {
                    return self.fail_word(k, ItemAction::Remove, ItemFailure::NoneOf, &w);
                }
                for pos in matching {
                    self.remove_one(k, pos);
                }
            }
            Some(Target::One { word, nth }) => {
                let list: Vec<Key> = eq.iter().map(|(_, o)| *o).collect();
                match self.find_obj(k, &list, &word, nth) {
                    Some(o) => {
                        let pos = eq.iter().find(|(_, x)| *x == o).unwrap().0;
                        self.remove_one(k, pos)
                    }
                    None => self.fail_word(k, ItemAction::Remove, ItemFailure::NotUsing, &word),
                }
            }
            None => self.fail_word(k, ItemAction::Remove, ItemFailure::NotUsing, what),
        }
    }

    fn remove_one(&mut self, k: Key, pos: EquipPos) {
        let Some(&o) = self.chars.get(k).unwrap().equipment.get(&pos) else { return };
        if self.objs.get(o).unwrap().flags.contains(&ObjFlag::NoDrop) {
            return self.fail_obj(k, ItemAction::Remove, ItemFailure::Cursed, o);
        }
        if self.carrying(k).1 >= self.can_carry_n(k) {
            return self.fail_obj(k, ItemAction::Remove, ItemFailure::TooMany, o);
        }
        self.put(o, Place::Carried(k));
        let (text, id) = (self.short(o), self.obj_id(o));
        self.deliver(k, Event::Used { action: ItemAction::Remove, text, id, into: None, into_id: None, slot: Some(slot_name(pos)), liquid: None });
        self.others_see(k, ItemAction::Remove, o, None, Some(slot_name(pos)), None);
    }

    /// `eat <food>`, `taste <food>` (act.item.c do_eat, MECHANICS §6.2).
    pub(crate) fn eat(&mut self, k: Key, arg: &str, taste: bool) {
        let action = if taste { ItemAction::Taste } else { ItemAction::Eat };
        if self.chars.get(k).unwrap().is_mob() {
            return;
        }
        let Some(word) = arg.split_whitespace().next() else {
            return self.fail(k, action, ItemFailure::What);
        };
        let inv = self.chars.get(k).unwrap().inventory.clone();
        let found = match target(word) {
            Some(Target::One { word, nth }) => self.find_obj(k, &inv, &word, nth),
            _ => None,
        };
        let Some(food) = found else {
            return self.fail_word(k, action, ItemFailure::NotCarried, word);
        };
        let obj = self.objs.get(food).unwrap();
        let is_food = obj.kind == ItemType::Food;
        if taste && matches!(obj.kind, ItemType::Drinkcon | ItemType::Fountain) {
            return self.drink_from(k, food, true);
        }
        if !is_food {
            return self.fail(k, action, ItemFailure::NotFood);
        }
        if self.chars.get(k).unwrap().conditions.full > 20 {
            return self.fail(k, action, ItemFailure::TooFull);
        }
        let (text, id) = (self.short(food), self.obj_id(food));
        self.deliver(k, Event::Used { action, text, id, into: None, into_id: None, slot: None, liquid: None });
        self.others_see(k, action, food, None, None, None);
        let fills = self.food_left(food);
        let amount = if taste { 1 } else { fills };
        let c = self.chars.get_mut(k).unwrap();
        if c.conditions.full >= 0 {
            c.conditions.full = (c.conditions.full + amount).clamp(0, 24);
        }
        if c.conditions.full > 20 {
            self.deliver(k, Event::Condition { hungry: None, thirsty: None, full: Some(true), quenched: None, sober: None, drunk: None });
        }
        if self.objs.get(food).unwrap().values.poisoned == Some(true) {
            let room = self.chars.get(k).unwrap().room;
            self.deliver(k, Event::TastedStrange { who: SELF.into(), who_id: None, drink: false });
            self.to_room(k, room, false, |who, who_id| Event::TastedStrange { who, who_id, drink: false });
            self.poison(k, amount * 2);
        }
        if taste {
            // A taste takes one hour of food off it; the last one is eaten (act.item.c:1061-1066).
            let left = self.food_left(food) - 1;
            if left <= 0 {
                self.fail(k, ItemAction::Taste, ItemFailure::Empty);
                self.extract_obj(food);
            } else if let Some(o) = self.objs.get_mut(food) {
                o.values.hours = Some(left as i64);
            }
        } else {
            self.extract_obj(food);
        }
    }

    fn food_left(&self, food: Key) -> i32 {
        self.objs.get(food).and_then(|o| o.values.hours).unwrap_or(0) as i32
    }

    /// Poisoned by food or drink for `hours` ticks (act.item.c affect_join without adding: a new
    /// poison replaces the old, MECHANICS §11.4).
    pub(crate) fn poison(&mut self, k: Key, hours: i32) {
        let c = self.chars.get_mut(k).unwrap();
        if c.level >= 31 {
            return;
        }
        c.spells.retain(|s| !(s.spell == crate::combat::SPELL_POISON && s.apply.is_none()));
        c.spells.insert(0, crate::entity::SpellAffect { spell: crate::combat::SPELL_POISON, duration: hours, apply: None, modifier: 0, bit: Some(mundi_content::names::Affect::Poison) });
    }

    /// `drink <container>`, `sip <container>` (act.item.c do_drink, MECHANICS §6.3).
    pub(crate) fn drink(&mut self, k: Key, arg: &str, sip: bool) {
        let action = if sip { ItemAction::Sip } else { ItemAction::Drink };
        if self.chars.get(k).unwrap().is_mob() {
            return;
        }
        let Some(word) = arg.split_whitespace().next() else {
            return self.fail(k, action, ItemFailure::What);
        };
        let (room, inv) = (self.chars.get(k).unwrap().room, self.chars.get(k).unwrap().inventory.clone());
        let found = match target(word) {
            Some(Target::One { word, nth }) => self.find_obj(k, &inv, &word, nth).or_else(|| self.find_obj(k, &self.things[room].clone(), &word, nth)),
            _ => None,
        };
        let Some(o) = found else {
            return self.fail(k, action, ItemFailure::CantFind);
        };
        self.drink_from(k, o, sip)
    }

    fn drink_from(&mut self, k: Key, o: Key, sip: bool) {
        let action = if sip { ItemAction::Sip } else { ItemAction::Drink };
        let obj = self.objs.get(o).unwrap().clone();
        if !matches!(obj.kind, ItemType::Drinkcon | ItemType::Fountain) {
            return self.fail(k, action, ItemFailure::CantDrink);
        }
        if obj.kind == ItemType::Drinkcon && matches!(obj.place, Place::Room(_)) {
            return self.fail(k, action, ItemFailure::MustHold);
        }
        let cond = self.chars.get(k).unwrap().conditions;
        if cond.drunk > 10 && cond.thirst > 0 {
            let room = self.chars.get(k).unwrap().room;
            self.fail(k, action, ItemFailure::MissMouth);
            self.to_room(k, room, true, |who, who_id| Event::ItemFailed {
                action,
                reason: ItemFailure::MissMouth,
                text: None,
                id: None,
                keyword: None,
                other: Some(who),
                other_id: who_id,
                slot: None,
            });
            return;
        }
        if cond.full > 20 && cond.thirst > 0 {
            return self.fail(k, action, ItemFailure::StomachFull);
        }
        let capacity = obj.values.capacity.unwrap_or(0);
        let now = obj.values.contains.unwrap_or(0);
        let unlimited = capacity < 0 || now < 0;
        if !unlimited && now == 0 {
            return self.fail(k, action, ItemFailure::Empty);
        }
        let liquid_ix = obj.values.liquid.map(|l| mundi_content::names::Liquid::ALL.iter().position(|x| *x == l).unwrap_or(0)).unwrap_or(0);
        let liq = self.tables.world.liquids.get(liquid_ix).cloned().unwrap_or_else(|| self.tables.world.liquids[0].clone());
        let (text, id) = (obj.short.clone(), self.obj_id(o));
        self.others_see(k, action, o, None, None, Some(liq.name.clone()));
        self.deliver(k, Event::Used { action, text, id, into: None, into_id: None, slot: None, liquid: Some(liq.name.clone()) });
        let mut amount = if sip {
            1
        } else if liq.drunk > 0 {
            (25 - cond.thirst) / liq.drunk
        } else {
            self.rand(3, 10) as i32
        };
        if !unlimited {
            amount = amount.min(now as i32);
            let w = (amount as i64).min(obj.weight);
            self.objs.get_mut(o).unwrap().weight -= w;
        }
        let c = self.chars.get_mut(k).unwrap();
        let gain = |v: &mut i32, d: i32| {
            if *v >= 0 {
                *v = (*v + d).clamp(0, 24);
            }
        };
        gain(&mut c.conditions.drunk, liq.drunk * amount / 4);
        gain(&mut c.conditions.full, liq.full * amount / 4);
        gain(&mut c.conditions.thirst, liq.thirst * amount / 4);
        let after = c.conditions;
        if after.drunk > 10 {
            self.deliver(k, Event::Condition { hungry: None, thirsty: None, full: None, quenched: None, sober: None, drunk: Some(true) });
        }
        if after.thirst > 20 {
            self.deliver(k, Event::Condition { hungry: None, thirsty: None, full: None, quenched: Some(true), sober: None, drunk: None });
        }
        if after.full > 20 {
            self.deliver(k, Event::Condition { hungry: None, thirsty: None, full: Some(true), quenched: None, sober: None, drunk: None });
        }
        if obj.values.poisoned == Some(true) && self.chars.get(k).unwrap().level < 31 {
            let room = self.chars.get(k).unwrap().room;
            self.deliver(k, Event::TastedStrange { who: SELF.into(), who_id: None, drink: true });
            self.to_room(k, room, true, |who, who_id| Event::TastedStrange { who, who_id, drink: true });
            self.poison(k, amount * 3);
        }
        if !unlimited {
            let v = &mut self.objs.get_mut(o).unwrap().values;
            let left = (now - amount as i64).max(0);
            v.contains = Some(left);
            if left == 0 {
                v.liquid = None;
                v.poisoned = Some(false);
            }
        }
    }
}
