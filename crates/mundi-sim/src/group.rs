//! Following and groups (MECHANICS §2.5, §7.3, §9.2, §12): follow, Vatiken's groups (act.other.c,
//! handler.c), group talk, report, split, assist and autoassist, the group's share of a kill, and
//! what the killer's auto settings take from the corpse.

use mundi_protocol::{Direction, GroupMember};

use crate::*;

/// The auto settings `autoloot` and the rest switch (act.other.c do_gen_tog).
pub(crate) const PREFS: [&str; 7] = ["autoloot", "autogold", "autosplit", "autosac", "autoassist", "autodoor", "autokey"];

impl Sim {
    // ---- following (act.movement.c do_follow, utils.c add_follower, stop_follower) ----------------

    pub(crate) fn follow_cmd(&mut self, k: Key, arg: &str) {
        let Some(word) = arg.split_whitespace().next() else {
            let master = self.chars.get(k).unwrap().master.and_then(|m| self.chars.get(m)).map(|m| m.name.clone());
            return match master {
                Some(name) => self.deliver(k, Event::GroupFailed { reason: "following".into(), who: Some(name), who_id: None }),
                None => self.group_fail(k, "follow_whom"),
            };
        };
        let Some(leader) = self.find_char_room(k, word) else {
            return self.group_fail(k, "no_person");
        };
        let c = self.chars.get(k).unwrap();
        if c.master == Some(leader) {
            let (name, id) = (self.chars.get(leader).unwrap().name.clone(), self.id_of(leader));
            return self.deliver(k, Event::GroupFailed { reason: "already_following".into(), who: Some(name), who_id: id });
        }
        if leader == k {
            return if c.master.is_none() { self.group_fail(k, "following_self") } else { self.stop_follower(k) };
        }
        // circle_follow: refuse if the leader's chain of masters reaches us.
        let mut m = Some(leader);
        while let Some(x) = m {
            if x == k {
                return self.group_fail(k, "loop");
            }
            m = self.chars.get(x).and_then(|c| c.master);
        }
        if c.master.is_some() {
            self.stop_follower(k);
        }
        self.add_follower(k, leader);
    }

    fn add_follower(&mut self, k: Key, leader: Key) {
        self.chars.get_mut(k).unwrap().master = Some(leader);
        self.chars.get_mut(leader).unwrap().followers.insert(0, k);
        let (name, id) = self.seen_name(k, leader);
        self.deliver(k, Event::GroupChange { event: "following".into(), who: name, who_id: id, formed: false });
        if self.can_see(leader, k) {
            let (me, mid) = (self.chars.get(k).unwrap().name.clone(), self.id_of(k));
            self.deliver(leader, Event::GroupChange { event: "followed_by".into(), who: me, who_id: mid, formed: false });
        }
        self.follow_seen(k, leader, false);
    }

    pub(crate) fn stop_follower(&mut self, k: Key) {
        let Some(leader) = self.chars.get(k).and_then(|c| c.master) else { return };
        let (name, id) = self.seen_name(k, leader);
        self.deliver(k, Event::GroupChange { event: "stopped_following".into(), who: name, who_id: id, formed: false });
        self.follow_seen(k, leader, true);
        if self.can_see(leader, k) {
            let (me, mid) = (self.chars.get(k).unwrap().name.clone(), self.id_of(k));
            self.deliver(leader, Event::GroupChange { event: "follower_left".into(), who: me, who_id: mid, formed: false });
        }
        self.chars.get_mut(k).unwrap().master = None;
        if let Some(l) = self.chars.get_mut(leader) {
            l.followers.retain(|f| *f != k);
        }
    }

    /// "$n starts to follow $N." / "$n stops following $N." to the rest of the room (hide-invisible).
    fn follow_seen(&mut self, k: Key, leader: Key, stopped: bool) {
        let room = self.chars.get(k).unwrap().room;
        for w in self.people[room].clone() {
            if w == k || w == leader || !self.can_see(w, k) || !self.awake_and_linked_pub(w) {
                continue;
            }
            let (who, who_id) = (self.chars.get(k).unwrap().name.clone(), self.id_of(k));
            let (l, lid) = self.seen_name(w, leader);
            self.deliver(w, Event::OccupantFollow { who, who_id, leader: l, leader_id: lid, stopped });
        }
    }

    pub(crate) fn unfollow(&mut self, k: Key) {
        if self.chars.get(k).unwrap().master.is_some() {
            self.stop_follower(k);
        } else {
            self.group_fail(k, "not_following");
        }
    }

    /// Everyone stops following someone leaving the world, and they stop following.
    pub(crate) fn drop_follows(&mut self, k: Key) {
        if self.chars.get(k).is_some_and(|c| c.master.is_some()) {
            self.stop_follower(k);
        }
        for f in self.chars.get(k).map(|c| c.followers.clone()).unwrap_or_default() {
            self.stop_follower(f);
        }
    }

    /// act.movement.c:357-372: followers still in the room and standing come along, the newest first.
    pub(crate) fn followers_follow(&mut self, leader: Key, from: RoomIx, d: usize) {
        for f in self.chars.get(leader).map(|c| c.followers.clone()).unwrap_or_default() {
            let Some(c) = self.chars.get(f) else { continue };
            if c.room != from || c.position < Position::Standing {
                continue;
            }
            let (name, id) = self.seen_name(f, leader);
            self.deliver(f, Event::FollowMoved { leader: name, leader_id: id, dir: Some(DIRS[d].into()) });
            self.move_dir(f, d);
        }
    }

    pub(crate) fn seen_name(&self, viewer: Key, k: Key) -> (String, Option<String>) {
        if self.can_see(viewer, k) {
            (self.chars.get(k).map(|c| c.name.clone()).unwrap_or_default(), self.id_of(k))
        } else {
            ("someone".into(), None)
        }
    }

    pub(crate) fn awake_and_linked_pub(&self, k: Key) -> bool {
        self.chars.get(k).is_some_and(|c| c.position > Position::Sleeping && c.linked)
    }

    fn group_fail(&mut self, k: Key, reason: &str) {
        self.deliver(k, Event::GroupFailed { reason: reason.into(), who: None, who_id: None });
    }

    // ---- groups (act.other.c do_group; handler.c create_group, join_group, leave_group) ---------

    /// To every connected player in a group, but `except` (comm.c send_to_group).
    fn to_group(&mut self, g: Key, except: Option<Key>, event: Event) {
        for m in self.groups.get(g).map(|g| g.members.clone()).unwrap_or_default() {
            if Some(m) == except || self.chars.get(m).is_none_or(|c| c.is_mob()) {
                continue;
            }
            self.deliver(m, event.clone());
        }
    }

    pub(crate) fn group_cmd(&mut self, k: Key, arg: &str) {
        let (word, rest) = arg.split_once(char::is_whitespace).map_or((arg, ""), |(w, r)| (w, r.trim()));
        let word = word.to_lowercase();
        let my_group = self.chars.get(k).unwrap().group;
        if word.is_empty() {
            return match my_group {
                Some(g) => self.print_group(k, g),
                None => self.group_fail(k, "option"),
            };
        }
        let is = |w: &str| w.starts_with(&word);
        if is("new") {
            if my_group.is_some() {
                return self.group_fail(k, "already_in_group");
            }
            let g = self.groups.insert(Group { leader: None, members: vec![], open: true, anonymous: false });
            self.join_group(k, g);
        } else if is("list") {
            self.group_fail(k, "list_not_yet");
        } else if is("join") {
            let Some(vict) = rest.split_whitespace().next().and_then(|w| self.find_char_room(k, w)) else {
                return self.group_fail(k, "join_who");
            };
            if vict == k {
                return self.group_fail(k, "lonely");
            }
            if my_group.is_some() {
                return self.group_fail(k, "already_part");
            }
            let Some(g) = self.chars.get(vict).unwrap().group else {
                let (n, id) = (self.chars.get(vict).unwrap().name.clone(), self.id_of(vict));
                return self.deliver(k, Event::GroupFailed { reason: "not_in_group".into(), who: Some(n), who_id: id });
            };
            if !self.groups.get(g).unwrap().open {
                return self.group_fail(k, "not_accepting");
            }
            self.join_group(k, g);
        } else if is("kick") {
            let Some(vict) = rest.split_whitespace().next().and_then(|w| self.find_char_room(k, w)) else {
                return self.group_fail(k, "kick_who");
            };
            if vict == k {
                return self.group_fail(k, "easier_ways");
            }
            let Some(g) = my_group else { return self.group_fail(k, "not_part") };
            if self.groups.get(g).unwrap().leader != Some(k) {
                return self.group_fail(k, "only_leader_kicks");
            }
            if self.chars.get(vict).unwrap().group != Some(g) {
                let (n, id) = (self.chars.get(vict).unwrap().name.clone(), self.id_of(vict));
                return self.deliver(k, Event::GroupFailed { reason: "not_member".into(), who: Some(n), who_id: id });
            }
            let (n, id) = (self.chars.get(vict).unwrap().name.clone(), self.id_of(vict));
            self.deliver(k, Event::GroupChange { event: "kicked".into(), who: n, who_id: id, formed: false });
            let me = self.chars.get(k).unwrap().name.clone();
            self.deliver(vict, Event::GroupChange { event: "kicked_out".into(), who: me, who_id: None, formed: false });
            self.leave_group(vict);
        } else if is("leave") {
            if my_group.is_none() {
                return self.group_fail(k, "not_part_any");
            }
            self.leave_group(k);
        } else if is("option") {
            let Some(g) = my_group else { return self.group_fail(k, "not_part_any") };
            if self.groups.get(g).unwrap().leader != Some(k) {
                return self.group_fail(k, "only_leader_flags");
            }
            let o = rest.to_lowercase();
            let grp = self.groups.get_mut(g).unwrap();
            if !o.is_empty() && "open".starts_with(&o) {
                grp.open = !grp.open;
                let open = grp.open;
                self.deliver(k, Event::GroupOption { open: Some(open), anonymous: None });
            } else if !o.is_empty() && "anonymous".starts_with(&o) {
                grp.anonymous = !grp.anonymous;
                let anon = grp.anonymous;
                self.deliver(k, Event::GroupOption { open: None, anonymous: Some(anon) });
            } else {
                self.group_fail(k, "flag_options");
            }
        } else {
            self.group_fail(k, "option");
        }
    }

    fn join_group(&mut self, k: Key, g: Key) {
        let grp = self.groups.get_mut(g).unwrap();
        grp.members.push(k);
        let became_leader = grp.leader.is_none();
        if became_leader {
            grp.leader = Some(k);
        }
        self.chars.get_mut(k).unwrap().group = Some(g);
        let (name, id) = (self.chars.get(k).unwrap().name.clone(), self.id_of(k));
        let event = if became_leader { "new_leader" } else { "joined" };
        self.to_group(g, None, Event::GroupChange { event: event.into(), who: name, who_id: id, formed: became_leader });
    }

    /// handler.c leave_group: everyone hears it, the leaver too; a new leader is drawn if needed.
    pub(crate) fn leave_group(&mut self, k: Key) {
        let Some(g) = self.chars.get(k).and_then(|c| c.group) else { return };
        let (name, id) = (self.chars.get(k).unwrap().name.clone(), self.id_of(k));
        self.to_group(g, None, Event::GroupChange { event: "left".into(), who: name, who_id: id, formed: false });
        let grp = self.groups.get_mut(g).unwrap();
        grp.members.retain(|m| *m != k);
        self.chars.get_mut(k).unwrap().group = None;
        let grp = self.groups.get(g).unwrap();
        if grp.members.is_empty() {
            self.groups.remove(g);
            return;
        }
        if grp.leader == Some(k) {
            let n = grp.members.len();
            let pick = self.rand(0, n as i64 - 1) as usize;
            let new = self.groups.get(g).unwrap().members[pick];
            self.groups.get_mut(g).unwrap().leader = Some(new);
            let (name, id) = (self.chars.get(new).unwrap().name.clone(), self.id_of(new));
            self.to_group(g, None, Event::GroupChange { event: "new_leader".into(), who: name, who_id: id, formed: false });
        }
    }

    fn member(&self, m: Key, leader: Option<Key>) -> GroupMember {
        let c = self.chars.get(m).unwrap();
        GroupMember {
            name: c.name.clone(),
            id: self.id_of(m),
            hp: c.hp,
            hp_max: c.max_hp,
            mp: c.mana,
            mp_max: c.max_mana,
            mv: c.mv,
            mv_max: c.max_mv,
            leader: leader == Some(m),
        }
    }

    fn print_group(&mut self, k: Key, g: Key) {
        let grp = self.groups.get(g).unwrap().clone();
        let members = grp.members.iter().map(|m| self.member(*m, grp.leader)).collect();
        self.deliver(k, Event::GroupStatus { members });
    }

    /// `report` (act.other.c do_report): the numbers to the whole group.
    pub(crate) fn report(&mut self, k: Key) {
        let Some(g) = self.chars.get(k).unwrap().group else { return self.group_fail(k, "not_member_any") };
        let leader = self.groups.get(g).unwrap().leader;
        let member = self.member(k, leader);
        self.to_group(g, None, Event::GroupReport { member });
    }

    /// `gsay` / `gtell` (act.comm.c do_gsay).
    pub(crate) fn gsay(&mut self, k: Key, text: &str) {
        let Some(g) = self.chars.get(k).unwrap().group else { return self.group_fail(k, "not_member_a") };
        if text.is_empty() {
            return self.group_fail(k, "gsay_what");
        }
        let (name, id) = (self.chars.get(k).unwrap().name.clone(), self.id_of(k));
        self.to_group(g, Some(k), Event::Gtell { from: name, from_id: id, text: text.into(), direction: Direction::In });
        self.deliver(k, Event::Gtell { from: SELF.into(), from_id: None, text: text.into(), direction: Direction::Out });
    }

    /// `split <amount>` (act.other.c do_split, MECHANICS §12.2).
    pub(crate) fn split(&mut self, k: Key, arg: &str) {
        if self.chars.get(k).unwrap().is_mob() {
            return;
        }
        let Some(amount) = arg.split_whitespace().next().and_then(|a| a.parse::<i64>().ok()) else {
            return self.group_fail(k, "split_how_many");
        };
        if amount <= 0 {
            return self.group_fail(k, "split_cant");
        }
        if amount > self.chars.get(k).unwrap().gold {
            return self.group_fail(k, "split_not_enough");
        }
        let room = self.chars.get(k).unwrap().room;
        let Some(g) = self.chars.get(k).unwrap().group else { return self.group_fail(k, "split_whom") };
        let others: Vec<Key> = self.groups.get(g).unwrap().members.iter().copied().filter(|m| *m != k && self.chars.get(*m).is_some_and(|c| !c.is_mob() && c.room == room)).collect();
        let num = others.len() as i64 + 1;
        if num < 2 {
            return self.group_fail(k, "split_whom");
        }
        let (share, rest) = (amount / num, amount % num);
        self.chars.get_mut(k).unwrap().gold -= share * (num - 1);
        let (name, id) = (self.chars.get(k).unwrap().name.clone(), self.id_of(k));
        for o in others {
            self.chars.get_mut(o).unwrap().gold += share;
            self.deliver(o, Event::Split { from: name.clone(), from_id: id.clone(), amount, share, rest, members: num });
        }
        self.deliver(k, Event::Split { from: SELF.into(), from_id: None, amount, share, rest, members: num });
    }

    /// autoloot and the rest (act.other.c do_gen_tog).
    pub(crate) fn auto_toggle(&mut self, k: Key, pref: &str) {
        let c = self.chars.get_mut(k).unwrap();
        let on = !c.prefs.contains(pref);
        if on {
            c.prefs.insert(pref.to_string());
        } else {
            c.prefs.remove(pref);
        }
        self.deliver(k, Event::Toggle { name: pref.into(), value: serde_json::json!(on) });
    }

    // ---- assisting (act.offensive.c do_assist; fight.c:968-988) ---------------------------------

    pub(crate) fn assist(&mut self, k: Key, arg: &str) {
        if self.chars.get(k).unwrap().fighting.is_some() {
            return self.group_fail(k, "assist_fighting");
        }
        let Some(word) = arg.split_whitespace().next() else { return self.group_fail(k, "assist_whom") };
        let Some(helpee) = self.find_char_room(k, word) else { return self.group_fail(k, "no_person") };
        if helpee == k {
            return self.group_fail(k, "assist_self");
        }
        let room = self.chars.get(k).unwrap().room;
        let opponent = self.chars.get(helpee).unwrap().fighting.or_else(|| {
            self.people[room].iter().copied().find(|o| self.chars.get(*o).and_then(|c| c.fighting) == Some(helpee))
        });
        let (hn, hid) = (self.chars.get(helpee).unwrap().name.clone(), self.id_of(helpee));
        let Some(opponent) = opponent else {
            return self.deliver(k, Event::GroupFailed { reason: "nobody_fighting".into(), who: Some(hn), who_id: hid });
        };
        if !self.can_see(k, opponent) {
            return self.deliver(k, Event::GroupFailed { reason: "cant_see_fighting".into(), who: Some(hn), who_id: hid });
        }
        if self.chars.get(k).is_some_and(|c| !c.is_mob()) && self.chars.get(opponent).is_some_and(|c| !c.is_mob()) {
            return self.group_fail(k, "cannot_kill_players");
        }
        let (me, mid) = (self.chars.get(k).unwrap().name.clone(), self.id_of(k));
        self.deliver(k, Event::Assisted { who: SELF.into(), who_id: None, target: hn.clone(), target_id: hid.clone(), jumped: false });
        if self.can_see(helpee, k) {
            self.deliver(helpee, Event::Assisted { who: me.clone(), who_id: mid.clone(), target: SELF.into(), target_id: None, jumped: false });
        }
        for w in self.people[room].clone() {
            if w == k || w == helpee || !self.awake_and_linked_pub(w) {
                continue;
            }
            let (a, ai) = self.seen_name(w, k);
            let (t, ti) = self.seen_name(w, helpee);
            self.deliver(w, Event::Assisted { who: a, who_id: ai, target: t, target_id: ti, jumped: false });
        }
        self.hit(k, opponent, None);
    }

    /// fight.c:968-988: the fighter's group members who are mobs or have autoassist, standing, not
    /// fighting, in the room and able to see the fighter, assist by name.
    pub(crate) fn autoassist(&mut self, k: Key) {
        let Some(g) = self.chars.get(k).and_then(|c| c.group) else { return };
        let room = self.chars.get(k).unwrap().room;
        let name = self.chars.get(k).unwrap().name.clone();
        for m in self.groups.get(g).map(|g| g.members.clone()).unwrap_or_default() {
            if m == k {
                continue;
            }
            let Some(c) = self.chars.get(m) else { continue };
            let wants = c.is_mob() || c.prefs.contains("autoassist");
            if wants && c.room == room && c.fighting.is_none() && c.position == Position::Standing && self.can_see(m, k) {
                self.assist(m, &name);
            }
        }
    }

    // ---- a kill's share and the corpse (fight.c group_gain, MECHANICS §9.2, §12.3) --------------

    /// The group's share when the killer is grouped; else the solo gain.
    pub(crate) fn kill_reward(&mut self, k: Key, vexp: i64, vlevel: i32, valign: i32, victim_pc: bool) {
        let Some(g) = self.chars.get(k).and_then(|c| c.group) else { return self.kill_gain(k, vexp, vlevel, valign) };
        let room = self.chars.get(k).unwrap().room;
        let here: Vec<Key> = self.groups.get(g).unwrap().members.iter().copied().filter(|m| self.chars.get(*m).is_some_and(|c| c.room == room)).collect();
        let n = here.len().max(1) as i64;
        let cfg = self.tables.world.config.clone();
        let mut tot = vexp / 3 + n - 1;
        if victim_pc {
            tot = tot.min(cfg.max_exp_loss * 2 / 3);
        }
        let base = (tot / n).max(1);
        for m in here {
            let share = base.max(1).min(cfg.max_exp_gain);
            if self.chars.get(m).is_some_and(|c| !c.is_mob()) {
                self.deliver(m, Event::ExpGain { amount: share, kind: "share".into() });
            }
            self.gain_exp(m, share);
            let c = self.chars.get_mut(m).unwrap();
            c.alignment += (-valign - c.alignment) / 16;
        }
    }

    /// fight.c:784-813: autosplit (grouped, gold), else autogold; then autoloot. autosac comes with
    /// sacrifice.
    pub(crate) fn after_kill(&mut self, k: Key, gold: i64) {
        let Some(c) = self.chars.get(k) else { return };
        if c.is_mob() {
            return;
        }
        let grouped = c.group.is_some();
        if grouped && gold > 0 && c.prefs.contains("autosplit") {
            self.get(k, "all.coin last.corpse");
            self.split(k, &gold.to_string());
        } else if self.chars.get(k).unwrap().prefs.contains("autogold") {
            self.get(k, "all.coin last.corpse");
        }
        if self.chars.get(k).is_some_and(|c| c.prefs.contains("autoloot")) {
            self.get(k, "all last.corpse");
        }
    }
}
