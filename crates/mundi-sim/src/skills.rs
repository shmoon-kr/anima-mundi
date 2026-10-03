//! The fighting skills (MECHANICS §10.2-10.6, act.offensive.c): kick, bash, rescue, backstab.
//! A roll of 1-101 above the skill's percentage fails (101 always does).

use mundi_content::names::{Attack, EquipPos, ItemType, MobFlag, RoomFlag};

use crate::combat::SKILL_BACKSTAB;
use crate::*;

pub(crate) const SKILL_BASH: i32 = 132;
pub(crate) const SKILL_KICK: i32 = 134;

impl Sim {
    fn skill(&self, k: Key, name: &str) -> i32 {
        self.chars.get(k).and_then(|c| (!c.is_mob()).then(|| c.skills.get(name).copied()).flatten()).unwrap_or(0)
    }

    fn skill_fail(&mut self, k: Key, skill: &str, reason: &str) {
        self.deliver(k, Event::SkillResult { skill: skill.into(), ok: false, reason: Some(reason.into()), who: None, who_id: None });
    }

    /// The named person in the room, or else whom they fight if that one is here.
    fn skill_target(&self, k: Key, arg: &str) -> Option<Key> {
        if let Some(w) = arg.split_whitespace().next() {
            return self.find_char_room(k, w);
        }
        let c = self.chars.get(k)?;
        c.fighting.filter(|v| self.chars.get(*v).is_some_and(|v| v.room == c.room))
    }

    fn wait(&mut self, k: Key, pulses: i32) {
        if let Some(c) = self.chars.get_mut(k) {
            c.wait = c.wait.max(pulses);
        }
    }

    /// act.offensive.c do_kick (MECHANICS §10.3).
    pub(crate) fn kick(&mut self, k: Key, arg: &str) {
        let prob = self.skill(k, "kick");
        if prob == 0 {
            return self.skill_fail(k, "kick", "no_idea");
        }
        let Some(vict) = self.skill_target(k, arg) else { return self.skill_fail(k, "kick", "who") };
        if vict == k {
            return self.skill_fail(k, "kick", "funny");
        }
        let percent = (10 - self.armor_class(vict) / 10) * 2 + self.rand(1, 101) as i32;
        let level = self.chars.get(k).unwrap().level;
        let dam = if percent > prob { 0 } else { level / 2 };
        self.damage(k, vict, dam, SKILL_KICK, None);
        self.wait(k, 3 * 2 * PULSES_PER_SEC as i32);
    }

    /// act.offensive.c do_bash (MECHANICS §10.4).
    pub(crate) fn bash(&mut self, k: Key, arg: &str) {
        let prob = self.skill(k, "bash");
        if prob == 0 {
            return self.skill_fail(k, "bash", "no_idea");
        }
        let room = self.chars.get(k).unwrap().room;
        if self.world.rooms[room].flags.contains(&RoomFlag::Peaceful) {
            return self.deliver(k, Event::AttackRefused { reason: mundi_protocol::AttackRefusal::Peaceful });
        }
        if !self.chars.get(k).unwrap().equipment.contains_key(&EquipPos::Wield) {
            return self.skill_fail(k, "bash", "need_weapon");
        }
        let Some(vict) = self.skill_target(k, arg) else { return self.skill_fail(k, "bash", "who") };
        if vict == k {
            return self.skill_fail(k, "bash", "funny");
        }
        if self.chars.get(vict).unwrap().has_flag(MobFlag::NoKill) {
            return self.deliver(k, Event::AttackRefused { reason: mundi_protocol::AttackRefusal::Protected });
        }
        let mut percent = self.rand(1, 101) as i32;
        if self.chars.get(vict).unwrap().has_flag(MobFlag::NoBash) {
            percent = 101;
        }
        if percent > prob {
            self.damage(k, vict, 0, SKILL_BASH, None);
            if let Some(c) = self.chars.get_mut(k) {
                c.position = Position::Sitting;
            }
        } else if self.damage(k, vict, 1, SKILL_BASH, None) > 0 {
            self.wait(vict, 2 * PULSES_PER_SEC as i32);
            if self.chars.get(vict).is_some_and(|v| v.room == room) {
                self.chars.get_mut(vict).unwrap().position = Position::Sitting;
            }
        }
        self.wait(k, 2 * 2 * PULSES_PER_SEC as i32);
    }

    /// act.offensive.c do_rescue (MECHANICS §10.5).
    pub(crate) fn rescue(&mut self, k: Key, arg: &str) {
        let prob = self.skill(k, "rescue");
        if prob == 0 {
            return self.skill_fail(k, "rescue", "no_idea_to");
        }
        let Some(vict) = arg.split_whitespace().next().and_then(|w| self.find_char_room(k, w)) else {
            return self.skill_fail(k, "rescue", "whom");
        };
        if vict == k {
            return self.skill_fail(k, "rescue", "flee_instead");
        }
        let me = self.chars.get(k).unwrap();
        if me.fighting == Some(vict) {
            return self.skill_fail(k, "rescue", "trying_to_kill");
        }
        let room = me.room;
        let mut foe = self.people[room].iter().copied().find(|c| self.chars.get(*c).and_then(|c| c.fighting) == Some(vict));
        let v_fights = self.chars.get(vict).unwrap().fighting;
        if foe.is_none() && v_fights.is_some() && me.fighting == v_fights {
            let f = v_fights.unwrap();
            if self.chars.get(f).and_then(|c| c.fighting) == Some(k) {
                let (n, id) = (self.chars.get(vict).unwrap().name.clone(), self.id_of(vict));
                return self.deliver(k, Event::SkillResult { skill: "rescue".into(), ok: false, reason: Some("already".into()), who: Some(n), who_id: id });
            }
            foe = Some(f);
        }
        let (vn, vid) = (self.chars.get(vict).unwrap().name.clone(), self.id_of(vict));
        let Some(foe) = foe else {
            return self.deliver(k, Event::SkillResult { skill: "rescue".into(), ok: false, reason: Some("nobody_fighting".into()), who: Some(vn), who_id: vid });
        };
        if self.rand(1, 101) as i32 > prob {
            return self.skill_fail(k, "rescue", "failed");
        }
        let (mn, mid) = (self.chars.get(k).unwrap().name.clone(), self.id_of(k));
        self.deliver(k, Event::Rescue { rescuer: SELF.into(), rescuer_id: None, rescued: vn.clone(), rescued_id: vid.clone() });
        self.deliver(vict, Event::Rescue { rescuer: mn.clone(), rescuer_id: mid.clone(), rescued: SELF.into(), rescued_id: None });
        for w in self.people[room].clone() {
            if w == k || w == vict || !self.awake_and_linked_pub(w) {
                continue;
            }
            let (a, ai) = if self.can_see(w, k) { (mn.clone(), mid.clone()) } else { ("someone".into(), None) };
            let (b, bi) = if self.can_see(w, vict) { (vn.clone(), vid.clone()) } else { ("someone".into(), None) };
            self.deliver(w, Event::Rescue { rescuer: a, rescuer_id: ai, rescued: b, rescued_id: bi });
        }
        if self.chars.get(vict).and_then(|c| c.fighting) == Some(foe) {
            self.stop_fighting(vict);
        }
        if self.chars.get(foe).is_some_and(|c| c.fighting.is_some()) {
            self.stop_fighting(foe);
        }
        if self.chars.get(k).is_some_and(|c| c.fighting.is_some()) {
            self.stop_fighting(k);
        }
        self.set_fighting(k, foe);
        self.set_fighting(foe, k);
        self.wait(vict, 2 * 2 * PULSES_PER_SEC as i32);
    }

    /// act.offensive.c do_backstab (MECHANICS §10.6).
    pub(crate) fn backstab(&mut self, k: Key, arg: &str) {
        let prob = self.skill(k, "backstab");
        if prob == 0 {
            return self.skill_fail(k, "backstab", "no_idea_to");
        }
        let Some(vict) = arg.split_whitespace().next().and_then(|w| self.find_char_room(k, w)) else {
            return self.skill_fail(k, "backstab", "who");
        };
        if vict == k {
            return self.skill_fail(k, "backstab", "yourself");
        }
        let weapon = self.chars.get(k).unwrap().equipment.get(&EquipPos::Wield).copied();
        let Some(w) = weapon else { return self.skill_fail(k, "backstab", "need_weapon") };
        let piercing = self.objs.get(w).and_then(|o| o.proto.as_ref()).and_then(|p| self.world.obj_protos.get(p)).is_some_and(|p| p.kind == ItemType::Weapon && p.values.attack == Some(Attack::Pierce));
        if !piercing {
            return self.skill_fail(k, "backstab", "wrong_weapon");
        }
        if self.chars.get(vict).unwrap().fighting.is_some() {
            return self.skill_fail(k, "backstab", "too_alert");
        }
        let v = self.chars.get(vict).unwrap();
        let awake = v.position > Position::Sleeping;
        if v.has_flag(MobFlag::Aware) && awake {
            let room = v.room;
            let (vn, vid) = (v.name.clone(), self.id_of(vict));
            let (mn, mid) = (self.chars.get(k).unwrap().name.clone(), self.id_of(k));
            self.deliver(vict, Event::Noticed { who: SELF.into(), who_id: None, by: mn.clone(), by_id: mid.clone() });
            self.deliver(k, Event::Noticed { who: vn.clone(), who_id: vid.clone(), by: SELF.into(), by_id: None });
            for w in self.people[room].clone() {
                if w != k && w != vict && self.awake_and_linked_pub(w) {
                    self.deliver(w, Event::Noticed { who: vn.clone(), who_id: vid.clone(), by: mn.clone(), by_id: mid.clone() });
                }
            }
            self.hit(vict, k, None);
            return;
        }
        let percent = self.rand(1, 101) as i32;
        if awake && percent > prob {
            self.damage(k, vict, 0, SKILL_BACKSTAB, Some(w));
        } else {
            self.hit(k, vict, Some(SKILL_BACKSTAB));
        }
        self.wait(k, 2 * 2 * PULSES_PER_SEC as i32);
    }
}
