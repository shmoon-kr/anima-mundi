//! The committed tables and combat messages are what the converter reads from tbaMUD now.
//! Skipped when the tbaMUD tree is not there (TBAMUD_SRC, or ~/tbamud).

use std::path::{Path, PathBuf};

use mundi_content::{load_messages, load_tables};
use mundi_convert::tables::{read_messages, read_tables};

fn tbamud() -> Option<PathBuf> {
    let t = std::env::var_os("TBAMUD").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join("tbamud")))?;
    t.join("src/constants.c").exists().then_some(t)
}

fn ours() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/tbamud")
}

#[test]
fn tables_match_the_source() {
    let Some(t) = tbamud() else { return };
    assert_eq!(read_tables(&t.join("src")).unwrap(), load_tables(&ours().join("tables")).unwrap());
}

#[test]
fn messages_match_the_file() {
    let Some(t) = tbamud() else { return };
    let m = read_messages(&t.join("lib/misc/messages")).unwrap();
    assert_eq!(m, load_messages(&ours().join("messages/combat.yaml")).unwrap());
    // Every weapon attack (300-314) and suffering (399) has messages (MECHANICS §7.6, §8.2).
    for n in (300..=314).chain([399]) {
        assert!(m.attacks.iter().any(|a| a.number == n), "attack {n}");
    }
}
