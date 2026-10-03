//! Mobs on their own (MECHANICS §2.6, §14.1, §14.2).

use std::collections::HashMap;
use std::path::Path;

use mundi_content::names::MobFlag;
use mundi_content::{load_tables, load_zone, Spawn, ZoneContent};
use mundi_sim::{Sim, PULSES_PER_SEC};

fn zones(nums: &[u32]) -> Vec<ZoneContent> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/tbamud/content");
    nums.iter().map(|n| load_zone(&root.join(n.to_string())).unwrap()).collect()
}

fn proto(id: &str) -> &str {
    id.split('/').next().unwrap()
}

fn zone_of(room: &str) -> &str {
    room.split(':').nth(1).unwrap()
}

#[test]
fn wanderers_wander_sentinels_stay_and_stay_zone_holds() {
    let z = zones(&[30, 31]);
    let tables = load_tables(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/tbamud/tables")).unwrap();
    let mut sim = Sim::new(&z, &tables, 9, 12);
    let flags: HashMap<String, Vec<MobFlag>> = z.iter().flat_map(|z| z.mobs.iter().map(|(k, m)| (k.clone(), m.flags.clone()))).collect();
    let start: HashMap<String, String> = sim.mob_places().into_iter().collect();
    // 30 minutes of game: 180 mob turns.
    for _ in 0..30 * 60 * PULSES_PER_SEC {
        sim.step();
    }
    let now: HashMap<String, String> = sim.mob_places().into_iter().collect();
    let mut wandered = 0;
    for (id, room) in &now {
        let Some(was) = start.get(id) else { continue }; // made by a later reset
        let f = &flags[proto(id)];
        if f.contains(&MobFlag::Sentinel) {
            assert_eq!(room, was, "{id} is a sentinel");
        } else if room != was {
            wandered += 1;
        }
        if f.contains(&MobFlag::StayZone) {
            assert_eq!(zone_of(room), zone_of(was), "{id} stays in its zone");
        }
    }
    assert!(wandered > 0, "someone wandered in half an hour");
}

#[test]
fn resets_keep_to_their_limits() {
    let z = zones(&[30]);
    let tables = load_tables(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/tbamud/tables")).unwrap();
    let mut limit: HashMap<String, i32> = HashMap::new();
    for s in &z[0].resets.spawns {
        if let Spawn::Mob(m) = s {
            let l = limit.entry(m.mob.clone()).or_insert(0);
            *l = (*l).max(m.limit);
        }
    }
    let mut sim = Sim::new(&z, &tables, 2, 12);
    let count = |sim: &Sim| {
        let mut c: HashMap<String, i32> = HashMap::new();
        for (id, _) in sim.mob_places() {
            *c.entry(proto(&id).to_string()).or_default() += 1;
        }
        c
    };
    let at_boot = count(&sim);
    // Midgaard resets every 15 minutes, always; an hour is four resets. A reset loads a mob whenever
    // fewer than its limit exist anywhere, so mobs build up to their limits even when none die.
    for _ in 0..60 * 60 * PULSES_PER_SEC {
        sim.step();
    }
    let after = count(&sim);
    for (p, n) in &after {
        assert!(*n <= limit[p], "{p}: {n} > limit {}", limit[p]);
        assert!(n >= at_boot.get(p).unwrap_or(&0));
    }
    assert!(after.iter().any(|(p, n)| n > at_boot.get(p).unwrap_or(&0)), "some built up toward their limits");
}
