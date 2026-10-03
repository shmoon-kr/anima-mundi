//! The converter only reads tbaMUD: every file under lib/world is byte-for-byte the same after a full
//! conversion (the world is also the running test server's).

use std::collections::BTreeMap;
use std::hash::{DefaultHasher, Hasher};
use std::path::{Path, PathBuf};

use mundi_convert::{convert_zone, tba};

fn world() -> Option<PathBuf> {
    let w = std::env::var_os("TBAMUD_WORLD").map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join("tbamud/lib/world")))?;
    w.join("wld").exists().then_some(w)
}

/// path -> (size, modified time, content hash) of every file under `dir`.
fn fingerprint(dir: &Path) -> BTreeMap<PathBuf, (u64, std::time::SystemTime, u64)> {
    let mut out = BTreeMap::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            let meta = e.metadata().unwrap();
            if meta.is_dir() {
                todo.push(p);
            } else {
                let mut h = DefaultHasher::new();
                h.write(&std::fs::read(&p).unwrap());
                out.insert(p, (meta.len(), meta.modified().unwrap(), h.finish()));
            }
        }
    }
    out
}

#[test]
fn a_full_conversion_leaves_the_tbamud_world_untouched() {
    let Some(world) = world() else {
        eprintln!("tbaMUD world files not found (TBAMUD_WORLD): skipped");
        return;
    };
    let before = fingerprint(&world);
    let zones = tba::read_zones(&world);
    let ids = tba::Ids::new(&zones).scan(&world);
    let mut notes = tba::Notes::default();
    let out = std::env::temp_dir().join(format!("mundi-readonly-{}", std::process::id()));
    for z in zones.values() {
        let content = convert_zone(&world, z, &ids, &mut notes);
        mundi_content::write_zone(&out.join(z.num.to_string()), &content, "", &Default::default()).unwrap();
    }
    std::fs::remove_dir_all(&out).ok();
    let after = fingerprint(&world);
    assert_eq!(before.len(), after.len(), "files appeared or disappeared under {}", world.display());
    let changed: Vec<_> = before.iter().filter(|(p, f)| after.get(*p) != Some(f)).map(|(p, _)| p).collect();
    assert!(changed.is_empty(), "changed by the converter: {changed:?}");
    assert!(before.len() > 1000);
}
