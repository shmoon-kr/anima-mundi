//! Special procedures (spec_assign.c, spec_procs.c, shop.c; MECHANICS §12.4, §15): before a command
//! does its own work, the specials of the mobs in the room may take it. Guildmasters teach, guild
//! guards stop the wrong class, shopkeepers trade.

use mundi_content::names::{Dir, ItemType, ObjFlag};
use mundi_protocol::{Direction, Known, ShopItem};

use crate::*;

impl Sim {
    /// Whether a mob's special took the command (interpreter.c special).
    pub(crate) fn special(&mut self, k: Key, cmd: &str, arg: &str) -> bool {
        let room = self.chars.get(k).unwrap().room;
        if self.guild_guard(k, cmd) {
            return true;
        }
        for m in self.people[room].clone() {
            if m == k {
                continue;
            }
            let Some(mob) = self.chars.get(m).and_then(|c| c.mob.as_ref()) else { continue };
            let proto = mob.proto.clone();
            let special = self.tables.specials.mobs.get(&proto).cloned();
            let taken = match special.as_deref() {
                Some("guild") => cmd == "practice" && !self.chars.get(k).unwrap().is_mob() && {
                    self.practice_at_guild(k, arg);
                    true
                },
                _ => false,
            };
            if taken {
                return true;
            }
            if self.world.keepers.contains(&proto) && self.shop_keeper(m, k, cmd, arg) {
                return true;
            }
        }
        false
    }

    /// The guild guards (lib/world/trg/30.trg #3000-#3003, the same rooms, ways and classes as
    /// class.c guild_info): in the guard's room, going the guild's way, only that class passes,
    /// while a guard is there awake and seeing.
    fn guild_guard(&mut self, k: Key, cmd: &str) -> bool {
        let Some(d) = DIRS.iter().position(|x| *x == cmd) else { return false };
        let c = self.chars.get(k).unwrap();
        if c.level >= 31 {
            return false;
        }
        let room = c.room;
        let room_id = self.world.rooms[room].id.clone();
        let class = c.class.map(|cl| cl.key().to_string());
        let rule = self.tables.specials.guild_guards.iter().any(|gg| {
            gg.room == room_id && Dir::ALL.iter().position(|x| *x == gg.dir) == Some(d) && !(c.mob.is_none() && class.as_deref() == Some(gg.class.as_str()))
        });
        let guard_here = self.people[room].iter().any(|m| {
            self.chars.get(*m).is_some_and(|g| g.is_mob() && g.position > Position::Sleeping && !g.has(mundi_content::names::Affect::Blind))
        });
        if rule && guard_here {
            self.deliver(k, Event::Blocked { who: SELF.into(), who_id: None });
            self.to_room(k, room, false, |who, who_id| Event::Blocked { who, who_id });
            return true;
        }
        false
    }

    // ---- guilds (spec_procs.c guild, list_skills; MECHANICS §12.4) ------------------------------

    fn class_spells(&self, k: Key) -> bool {
        matches!(self.chars.get(k).and_then(|c| c.class), Some(Class::MagicUser | Class::Cleric))
    }

    /// `practice` with no guild here: the list without an argument (act.other.c do_practice).
    pub(crate) fn practice_cmd(&mut self, k: Key, arg: &str) {
        if arg.trim().is_empty() {
            self.list_skills(k);
        } else {
            let spells = self.class_spells(k);
            self.deliver(k, Event::Practiced { skill: None, result: "cannot".into(), reason: Some("not_here".into()), spells });
        }
    }

    fn list_skills(&mut self, k: Key) {
        let c = self.chars.get(k).unwrap();
        let class = c.class.unwrap_or(Class::Warrior).key();
        let mut skills: Vec<Known> = self
            .tables
            .spells
            .iter()
            .filter(|s| s.levels.get(class).is_some_and(|l| c.level >= *l))
            .map(|s| Known { name: s.name.clone(), percent: c.skills.get(&s.name).copied().unwrap_or(0) })
            .collect();
        skills.sort_by(|a, b| a.name.cmp(&b.name));
        let (practices, spells) = (c.practices, self.class_spells(k));
        self.deliver(k, Event::Skills { practices, spells, skills });
    }

    fn practice_at_guild(&mut self, k: Key, arg: &str) {
        let arg = arg.trim();
        if arg.is_empty() {
            return self.list_skills(k);
        }
        let spells = self.class_spells(k);
        let c = self.chars.get(k).unwrap();
        if c.practices <= 0 {
            return self.deliver(k, Event::Practiced { skill: None, result: "cannot".into(), reason: Some("no_practices".into()), spells });
        }
        let class = c.class.unwrap_or(Class::Warrior);
        let typed = arg.to_lowercase();
        let words: Vec<&str> = typed.split_whitespace().collect();
        let found = self.tables.spells.iter().find(|s| {
            s.name.starts_with(&typed) || {
                let name: Vec<&str> = s.name.split_whitespace().collect();
                words.len() <= name.len() && words.iter().zip(&name).all(|(w, n)| n.starts_with(w))
            }
        });
        let Some(sp) = found.filter(|s| s.levels.get(class.key()).is_some_and(|l| c.level >= *l)).cloned() else {
            return self.deliver(k, Event::Practiced { skill: None, result: "cannot".into(), reason: Some("unknown_skill".into()), spells });
        };
        let p = self.tables.classes.get(class.key()).map(|t| t.practice.clone()).unwrap();
        let now = c.skills.get(&sp.name).copied().unwrap_or(0);
        if now >= p.learned {
            return self.deliver(k, Event::Practiced { skill: Some(sp.name), result: "maxed".into(), reason: None, spells });
        }
        let int = self.abilities_now(k).int.clamp(0, 25) as usize;
        let learn = self.tables.abilities.intelligence_learn.get(int).copied().unwrap_or(0);
        let gain = learn.max(p.min_gain).min(p.max_gain);
        let c = self.chars.get_mut(k).unwrap();
        c.practices -= 1;
        let new = (now + gain).min(p.learned);
        c.skills.insert(sp.name.clone(), new);
        let result = if new >= p.learned { "learned" } else { "improved" };
        self.deliver(k, Event::Practiced { skill: Some(sp.name), result: result.into(), reason: None, spells });
    }

    // ---- shops (shop.c; MECHANICS §15) ----------------------------------------------------------

    pub(crate) fn shop_of(&self, keeper: Key) -> Option<mundi_content::Shop> {
        let proto = self.chars.get(keeper)?.mob.as_ref()?.proto.clone();
        self.world.shops.iter().find(|s| s.keeper.as_deref() == Some(proto.as_str())).cloned()
    }

    /// shop.c shop_keeper: buy, sell, value and list in the shop's rooms while the keeper is awake.
    fn shop_keeper(&mut self, keeper: Key, k: Key, cmd: &str, arg: &str) -> bool {
        if !matches!(cmd, "buy" | "sell" | "value" | "list") {
            return false;
        }
        let Some(shop) = self.shop_of(keeper) else { return false };
        let room_id = self.world.rooms[self.chars.get(k).unwrap().room].id.clone();
        if !shop.rooms.contains(&room_id) || self.chars.get(keeper).is_none_or(|c| c.position <= Position::Sleeping) {
            return false;
        }
        if !self.shop_ok(keeper, k, &shop) {
            return true;
        }
        match cmd {
            "buy" => self.shop_buy(keeper, k, &shop, arg),
            "sell" => self.shop_sell(keeper, k, &shop, arg),
            "value" => self.shop_value(keeper, k, &shop, arg),
            _ => self.shop_list(keeper, k, &shop, arg),
        }
        true
    }

    /// The keeper says something to the room (do_say).
    fn keeper_says(&mut self, keeper: Key, text: &str) {
        let room = self.chars.get(keeper).unwrap().room;
        let said = text.to_string();
        self.to_room(keeper, room, false, move |from, from_id| Event::Say { from, from_id, text: said.clone(), direction: Direction::In });
    }

    /// The keeper tells the customer (do_tell). Shop messages start with "%s " for the customer's
    /// name and may hold one "%d" for the price.
    pub(crate) fn keeper_tells(&mut self, keeper: Key, k: Key, text: &str) {
        let (from, from_id) = if self.can_see(k, keeper) {
            (self.chars.get(keeper).unwrap().name.clone(), self.id_of(keeper))
        } else {
            ("someone".into(), None)
        };
        let to = self.chars.get(k).unwrap().name.clone();
        self.deliver(k, Event::Tell { from, from_id, to, to_id: None, text: text.to_string(), direction: Direction::In });
    }

    fn shop_message(msg: &str, amount: i64) -> String {
        let m = msg.strip_prefix("%s").unwrap_or(msg).trim_start();
        m.replacen("%d", &amount.to_string(), 1)
    }

    /// shop.c is_open, is_ok_char.
    fn shop_ok(&mut self, keeper: Key, k: Key, shop: &mundi_content::Shop) -> bool {
        let hour = self.hour as i32;
        let (open1, close1) = shop.hours.first().map_or((0, 28), |h| (h.open, h.close));
        let (open2, close2) = shop.hours.get(1).map_or((0, 0), |h| (h.open, h.close));
        let closed = if open1 > hour {
            Some("Come back later!")
        } else if close1 < hour && open2 > hour {
            Some("Sorry, we have closed, but come back later.")
        } else if close1 < hour && close2 < hour {
            Some("Sorry, come back tomorrow.")
        } else {
            None
        };
        if let Some(m) = closed {
            self.keeper_says(keeper, m);
            return false;
        }
        if !self.can_see(keeper, k) {
            self.keeper_says(keeper, "I don't trade with someone I can't see!");
            return false;
        }
        let c = self.chars.get(k).unwrap();
        let (good, evil) = (c.alignment >= 350, c.alignment <= -350);
        let t = shop.trade_with;
        // shop.h TRADE_NOGOOD 1, NOEVIL 2, NONEUTRAL 4, NOMAGIC_USER 8, NOCLERIC 16, NOTHIEF 32, NOWARRIOR 64.
        let align_bad = (good && t & 1 != 0) || (evil && t & 2 != 0) || (!good && !evil && t & 4 != 0);
        if align_bad {
            self.keeper_tells(keeper, k, "Get out of here before I call the guards!");
            return false;
        }
        let class_bad = match c.class {
            Some(Class::MagicUser) => t & 8 != 0,
            Some(Class::Cleric) => t & 16 != 0,
            Some(Class::Thief) => t & 32 != 0,
            Some(Class::Warrior) => t & 64 != 0,
            None => false,
        };
        if !c.is_mob() && class_bad {
            self.keeper_tells(keeper, k, "We don't serve your kind here!");
            return false;
        }
        true
    }

    /// shop.c buy_price, sell_price (MECHANICS §15.2). The profits are C floats, so this is single
    /// precision as there; double gives some prices one less than the live server.
    fn buy_price(&self, o: Key, shop: &mundi_content::Shop, keeper: Key, buyer: Key) -> i64 {
        let cost = self.objs.get(o).map_or(0, |o| o.cost) as f32;
        let c = (self.abilities_now(keeper).cha - self.abilities_now(buyer).cha) as f32;
        (cost * shop.profit.buy as f32 * (1.0 + c / 70.0)) as i64
    }

    fn sell_price(&self, o: Key, shop: &mundi_content::Shop, keeper: Key, seller: Key) -> i64 {
        let cost = self.objs.get(o).map_or(0, |o| o.cost) as f32;
        let c = (self.abilities_now(keeper).cha - self.abilities_now(seller).cha) as f64;
        let sell = (shop.profit.sell as f32 as f64 * (1.0 - c / 70.0)) as f32;
        let buy = (shop.profit.buy as f32 as f64 * (1.0 + c / 70.0)) as f32;
        (cost * sell.min(buy)) as i64
    }

    /// shop.c same_obj: the same prototype and cost.
    fn same_obj(&self, a: Key, b: Key) -> bool {
        match (self.objs.get(a), self.objs.get(b)) {
            (Some(x), Some(y)) => x.proto == y.proto && x.cost == y.cost,
            _ => false,
        }
    }

    fn producing(&self, o: Key, shop: &mundi_content::Shop) -> bool {
        self.objs.get(o).and_then(|o| o.proto.as_ref()).is_some_and(|p| shop.products.contains(p))
    }

    /// shop.c get_purchase_obj: `#n` (or a number) is the n-th line of the list, else a name.
    fn purchase_obj(&self, keeper: Key, k: Key, name: &str) -> Option<Key> {
        let stock: Vec<Key> = self.chars.get(keeper)?.inventory.iter().copied().filter(|o| self.can_see_obj(k, *o) && self.objs.get(*o).is_some_and(|x| x.cost > 0)).collect();
        let n = name.strip_prefix('#').unwrap_or(name);
        if let Ok(mut index) = n.parse::<usize>() {
            let mut last: Option<Key> = None;
            for &o in &stock {
                if last.is_some_and(|l| self.same_obj(l, o)) {
                    continue;
                }
                index -= 1;
                if index == 0 {
                    return Some(o);
                }
                last = Some(o);
            }
            return None;
        }
        let (word, nth) = match crate::items::target(name)? {
            crate::items::Target::One { word, nth: crate::items::Nth::Index(n) } => (word, n),
            _ => return None,
        };
        let mut seen = 0;
        let mut last: Option<Key> = None;
        for &o in &stock {
            if !crate::items::isname(&word, &self.objs.get(o).unwrap().keywords) || last.is_some_and(|l| self.same_obj(l, o)) {
                continue;
            }
            seen += 1;
            if seen == nth {
                return Some(o);
            }
            last = Some(o);
        }
        None
    }

    /// The amount before the name: `buy 3 bread` (shop.c transaction_amt).
    fn amount(arg: &str) -> (i64, String) {
        let mut w = arg.split_whitespace();
        match (w.next(), w.next()) {
            (Some(n), Some(rest)) if n.parse::<i64>().is_ok() => (n.parse().unwrap(), rest.to_string()),
            (Some(n), None) => (1, n.to_string()),
            _ => (1, String::new()),
        }
    }

    fn times(&self, o: Option<Key>, name: &str, n: i64) -> String {
        let base = match o.and_then(|o| self.objs.get(o)) {
            Some(obj) => obj.short.clone(),
            None => {
                let w = name.split('.').next_back().unwrap_or(name);
                let a = if w.starts_with(['a', 'e', 'i', 'o', 'u']) { "an" } else { "a" };
                format!("{a} {w}")
            }
        };
        if n > 1 { format!("{base} (x {n})") } else { base }
    }

    /// shop.c shopping_buy.
    fn shop_buy(&mut self, keeper: Key, k: Key, shop: &mundi_content::Shop, arg: &str) {
        let (want, name) = Sim::amount(arg);
        if want < 0 {
            return self.keeper_tells(keeper, k, "A negative amount?  Try selling me something.");
        }
        if name.is_empty() || want == 0 {
            return self.keeper_tells(keeper, k, "What do you want to buy??");
        }
        let Some(mut obj) = self.purchase_obj(keeper, k, &name) else {
            let m = Sim::shop_message(&shop.messages.no_such_item, 0);
            return self.keeper_tells(keeper, k, &m);
        };
        if self.buy_price(obj, shop, keeper, k) > self.chars.get(k).unwrap().gold {
            let m = Sim::shop_message(&shop.messages.you_cannot_afford, 0);
            return self.keeper_tells(keeper, k, &m);
        }
        let (mut bought, mut paid) = (0i64, 0i64);
        let mut last: Option<Key> = None;
        let cant = |s: &Sim, o: Key| {
            let (w, n) = s.carrying(k);
            n >= s.can_carry_n(k) || w + s.total_weight(o) > s.can_carry_w(k)
        };
        let first = obj;
        if cant(self, first) {
            let (_, n) = self.carrying(k);
            let reason = if n + 1 > self.can_carry_n(k) { ItemFailureShop::Items } else { ItemFailureShop::Weight };
            let kw = self.objs.get(first).and_then(|o| o.keywords.first().cloned()).unwrap_or_default();
            return self.deliver(k, Event::ShopResult { action: reason.name().into(), who: SELF.into(), who_id: None, text: kw });
        }
        loop {
            let price = self.buy_price(obj, shop, keeper, k);
            if bought >= want || self.chars.get(k).unwrap().gold < price || cant(self, obj) {
                break;
            }
            let item = if self.producing(obj, shop) {
                let proto = self.objs.get(obj).unwrap().proto.clone().unwrap();
                self.make_obj(&proto).unwrap()
            } else {
                obj
            };
            self.put(item, Place::Carried(k));
            self.chars.get_mut(k).unwrap().gold -= price;
            paid += price;
            bought += 1;
            last = Some(item);
            match self.purchase_obj(keeper, k, &name) {
                Some(next) if self.same_obj(next, item) => obj = next,
                _ => break,
            }
        }
        if bought < want {
            let have_more = self.purchase_obj(keeper, k, &name).is_some_and(|o| last.is_some_and(|l| self.same_obj(o, l)));
            let msg = if !have_more {
                format!("I only have {bought} to sell you.")
            } else if self.chars.get(k).unwrap().gold < self.buy_price(obj, shop, keeper, k) {
                format!("You can only afford {bought}.")
            } else if self.carrying(k).1 >= self.can_carry_n(k) {
                format!("You can only hold {bought}.")
            } else {
                format!("You can only carry {bought}.")
            };
            self.keeper_tells(keeper, k, &msg);
        }
        self.chars.get_mut(keeper).unwrap().gold += paid;
        let text = self.times(last, &name, bought);
        let room = self.chars.get(k).unwrap().room;
        let t = text.clone();
        self.to_room(k, room, false, move |who, who_id| Event::ShopResult { action: "buy".into(), who, who_id, text: t.clone() });
        let m = Sim::shop_message(&shop.messages.bought, paid);
        self.keeper_tells(keeper, k, &m);
        self.deliver(k, Event::ShopResult { action: "buy".into(), who: SELF.into(), who_id: None, text });
    }

    /// shop.c get_selling_obj and trade_with.
    fn selling_obj(&mut self, keeper: Key, k: Key, shop: &mundi_content::Shop, name: &str, msg: bool) -> Option<Key> {
        let inv = self.chars.get(k).unwrap().inventory.clone();
        let found = match crate::items::target(name) {
            Some(crate::items::Target::One { word, nth }) => self.find_obj(k, &inv, &word, nth),
            _ => None,
        };
        let Some(o) = found else {
            if msg {
                let m = Sim::shop_message(&shop.messages.you_dont_have_it, 0);
                self.keeper_tells(keeper, k, &m);
            }
            return None;
        };
        let obj = self.objs.get(o).unwrap();
        let verdict = if obj.cost < 1 {
            Some("You've got to be kidding, that thing is worthless!".to_string())
        } else if obj.flags.contains(&ObjFlag::NoSell) {
            Some(Sim::shop_message(&shop.messages.does_not_buy, 0))
        } else {
            let buys = shop.buys.iter().any(|b| b.kind == obj.kind && b.keywords.as_deref().is_none_or(|e| eval_keywords(e, &obj.keywords, &obj.flags)));
            let used_up = matches!(obj.kind, ItemType::Wand | ItemType::Staff) && obj.values.hours == Some(0);
            if used_up {
                Some("I don't buy used up wands or staves!".to_string())
            } else if !buys {
                Some(Sim::shop_message(&shop.messages.does_not_buy, 0))
            } else {
                None
            }
        };
        match verdict {
            Some(m) => {
                if msg {
                    self.keeper_tells(keeper, k, &m);
                }
                None
            }
            None => Some(o),
        }
    }

    /// shop.c shopping_sell (the bank is not kept: keepers have only their gold).
    fn shop_sell(&mut self, keeper: Key, k: Key, shop: &mundi_content::Shop, arg: &str) {
        let (want, name) = Sim::amount(arg);
        if want < 0 {
            return self.keeper_tells(keeper, k, "A negative amount?  Try buying something.");
        }
        if name.is_empty() || want == 0 {
            return self.keeper_tells(keeper, k, "What do you want to sell??");
        }
        let Some(mut obj) = self.selling_obj(keeper, k, shop, &name, true) else { return };
        let unlimited = shop.flags & 4 != 0; // shop.h HAS_UNLIMITED_CASH (1 << 2)
        if !unlimited && self.chars.get(keeper).unwrap().gold < self.sell_price(obj, shop, keeper, k) {
            let m = Sim::shop_message(&shop.messages.shop_cannot_afford, 0);
            return self.keeper_tells(keeper, k, &m);
        }
        let (mut sold, mut got) = (0i64, 0i64);
        loop {
            let price = self.sell_price(obj, shop, keeper, k);
            if sold >= want || (!unlimited && self.chars.get(keeper).unwrap().gold < price) {
                break;
            }
            got += price;
            if !unlimited {
                self.chars.get_mut(keeper).unwrap().gold -= price;
            }
            sold += 1;
            if self.producing(obj, shop) {
                self.extract_obj(obj);
            } else {
                self.put(obj, Place::Carried(keeper));
            }
            match self.selling_obj(keeper, k, shop, &name, false) {
                Some(next) => obj = next,
                None => break,
            }
        }
        if sold < want {
            let more = self.selling_obj(keeper, k, shop, &name, false).is_some();
            let msg = if !more { format!("You only have {sold} of those.") } else { format!("I can only afford to buy {sold} of those.") };
            self.keeper_tells(keeper, k, &msg);
        }
        self.chars.get_mut(k).unwrap().gold += got;
        let text = self.times(None, &name, sold);
        let room = self.chars.get(k).unwrap().room;
        let t = text.clone();
        self.to_room(k, room, false, move |who, who_id| Event::ShopResult { action: "sell".into(), who, who_id, text: t.clone() });
        let m = Sim::shop_message(&shop.messages.sold, got);
        self.keeper_tells(keeper, k, &m);
        self.deliver(k, Event::ShopResult { action: "sell".into(), who: SELF.into(), who_id: None, text });
    }

    fn shop_value(&mut self, keeper: Key, k: Key, shop: &mundi_content::Shop, arg: &str) {
        let name = arg.split_whitespace().next().unwrap_or("");
        if name.is_empty() {
            return self.keeper_tells(keeper, k, "What do you want me to evaluate??");
        }
        let Some(o) = self.selling_obj(keeper, k, shop, name, true) else { return };
        let price = self.sell_price(o, shop, keeper, k);
        self.keeper_tells(keeper, k, &format!("I'll give you {price} gold coins for that!"));
    }

    /// shop.c shopping_list: the keeper's things, the same ones counted on one line.
    fn shop_list(&mut self, keeper: Key, k: Key, shop: &mundi_content::Shop, arg: &str) {
        let name = arg.split_whitespace().next().unwrap_or("").to_lowercase();
        let stock: Vec<Key> = self.chars.get(keeper).unwrap().inventory.iter().copied().filter(|o| self.can_see_obj(k, *o) && self.objs.get(*o).is_some_and(|x| x.cost > 0)).collect();
        let mut groups: Vec<(Key, u32)> = Vec::new();
        for o in stock {
            match groups.last_mut() {
                Some((l, n)) if self.same_obj(*l, o) => *n += 1,
                _ => groups.push((o, 1)),
            }
        }
        let mut items = Vec::new();
        for (i, (o, n)) in groups.iter().enumerate() {
            let obj = self.objs.get(*o).unwrap();
            if !name.is_empty() && !crate::items::isname(&name, &obj.keywords) {
                continue;
            }
            let liquid = (obj.kind == ItemType::Drinkcon && obj.values.contains.unwrap_or(0) != 0)
                .then(|| obj.values.liquid.map(|l| self.tables.world.liquids.get(mundi_content::names::Liquid::ALL.iter().position(|x| *x == l).unwrap_or(0)).map(|x| x.name.clone()).unwrap_or_default()))
                .flatten();
            items.push(ShopItem {
                index: i as u32 + 1,
                id: self.obj_id(*o),
                text: obj.short.clone(),
                price: self.buy_price(*o, shop, keeper, k),
                count: (!self.producing(*o, shop)).then_some(*n),
                liquid,
            });
        }
        let none_matching = !name.is_empty() && items.is_empty() && !groups.is_empty();
        self.deliver(k, Event::ShopList { items, none_matching });
    }
}

/// Why a purchase could not even start.
enum ItemFailureShop {
    Items,
    Weight,
}

impl ItemFailureShop {
    fn name(&self) -> &'static str {
        match self {
            ItemFailureShop::Items => "too_many",
            ItemFailureShop::Weight => "too_heavy",
        }
    }
}

/// shop.c evaluate_expression: words joined by | + (or), & * (and), ^ ' (not), grouped by ( [ {.
/// A word that names an extra flag tests the flag, else the keywords.
pub(crate) fn eval_keywords(expr: &str, keywords: &[String], flags: &[ObjFlag]) -> bool {
    let expr = expr.trim();
    if expr.is_empty() {
        return true;
    }
    let tokens: Vec<String> = {
        let mut out = Vec::new();
        let mut word = String::new();
        for c in expr.chars() {
            if "([{)]}|+&*^'".contains(c) || c.is_whitespace() {
                if !word.is_empty() {
                    out.push(std::mem::take(&mut word));
                }
                if !c.is_whitespace() {
                    out.push(c.to_string());
                }
            } else {
                word.push(c);
            }
        }
        if !word.is_empty() {
            out.push(word);
        }
        out
    };
    fn atom(t: &[String], i: &mut usize, kw: &[String], fl: &[ObjFlag]) -> bool {
        match t.get(*i).map(String::as_str) {
            Some("^") | Some("'") => {
                *i += 1;
                !atom(t, i, kw, fl)
            }
            Some("(") | Some("[") | Some("{") => {
                *i += 1;
                let v = or(t, i, kw, fl);
                *i += 1;
                v
            }
            Some(w) => {
                *i += 1;
                let w = w.to_lowercase();
                match ObjFlag::ALL.iter().find(|f| f.name().eq_ignore_ascii_case(&w)) {
                    Some(f) => fl.contains(f),
                    None => kw.iter().any(|k| k.eq_ignore_ascii_case(&w)),
                }
            }
            None => true,
        }
    }
    fn and(t: &[String], i: &mut usize, kw: &[String], fl: &[ObjFlag]) -> bool {
        let mut v = atom(t, i, kw, fl);
        while matches!(t.get(*i).map(String::as_str), Some("&") | Some("*")) {
            *i += 1;
            v &= atom(t, i, kw, fl);
        }
        v
    }
    fn or(t: &[String], i: &mut usize, kw: &[String], fl: &[ObjFlag]) -> bool {
        let mut v = and(t, i, kw, fl);
        while matches!(t.get(*i).map(String::as_str), Some("|") | Some("+")) {
            *i += 1;
            v |= and(t, i, kw, fl);
        }
        v
    }
    let mut i = 0;
    or(&tokens, &mut i, keywords, flags)
}
