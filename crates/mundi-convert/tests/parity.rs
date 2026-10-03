//! The Rust converter must produce what the reviewed example shows (tools/prototype_convert.py made it):
//! zone 186, loaded back through mundi-content, compared value by value.

use std::path::{Path, PathBuf};

use mundi_content::load_zone;
use mundi_convert::{convert_zone, tba};

fn world() -> Option<PathBuf> {
    let w = std::env::var_os("TBAMUD_WORLD").map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join("tbamud/lib/world")))?;
    w.join("wld").exists().then_some(w)
}

#[test]
fn newbie_zone_matches_the_reviewed_example() {
    let Some(world) = world() else {
        eprintln!("tbaMUD world files not found (TBAMUD_WORLD): skipped");
        return;
    };
    let zones = tba::read_zones(&world);
    let ids = tba::Ids::new(&zones).scan(&world);
    let mut notes = tba::Notes::default();
    let ours = convert_zone(&world, &zones[&186], &ids, &mut notes);
    let dir = std::env::temp_dir().join(format!("mundi-parity-{}", std::process::id()));
    mundi_content::write_zone(&dir, &ours, "", &Default::default()).unwrap();
    let written = load_zone(&dir).unwrap();
    let example = load_zone(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/tbamud/examples/content/186")).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    assert!(notes.0.is_empty(), "{:?}", notes.0);
    assert_eq!(written, ours, "the written files load back to what was converted");
    assert_eq!(written.zone, example.zone);
    for (id, room) in &example.rooms {
        assert_eq!(written.rooms.get(id), Some(room), "{id}");
    }
    for (id, mob) in &example.mobs {
        assert_eq!(written.mobs.get(id), Some(mob), "{id}");
    }
    for (id, obj) in &example.objects {
        assert_eq!(written.objects.get(id), Some(obj), "{id}");
    }
    assert_eq!(written.resets, example.resets);
    assert_eq!((written.rooms.len(), written.mobs.len(), written.objects.len()), (41, 15, 14));
}
