//! The engine's principles, enforced by dependencies (PHASE-1-PLAN S0, D13).
//!
//! - the simulation writes no sentence and opens no socket or database: it does not reach the
//!   renderer, the network, the store, or their libraries, not even through another crate
//! - the renderer sees events only through the protocol: it does not reach the simulation, so it
//!   cannot reveal what perception filtered out
//! - the network sends what it is given: it does not reach the simulation or the store
//! - the protocol and the content depend on no other Mundi crate
//!
//! Dependencies are read from every crate's Cargo.toml and followed through other Mundi crates.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

/// crate -> (Mundi crates it must not reach, libraries it must not reach)
fn rules() -> BTreeMap<&'static str, (&'static [&'static str], &'static [&'static str])> {
    const IO_LIBS: &[&str] = &["tokio", "tokio-tungstenite", "rusqlite", "fluent", "fluent-bundle"];
    BTreeMap::from([
        ("mundi-protocol", (&["mundi-content", "mundi-sim", "mundi-render", "mundi-store", "mundi-net"][..], IO_LIBS)),
        ("mundi-content", (&["mundi-protocol", "mundi-sim", "mundi-render", "mundi-store", "mundi-net"][..], IO_LIBS)),
        ("mundi-sim", (&["mundi-render", "mundi-store", "mundi-net", "mundi-server", "mundi-convert"][..], IO_LIBS)),
        ("mundi-render", (&["mundi-sim", "mundi-store", "mundi-net"][..], &["tokio", "tokio-tungstenite", "rusqlite"][..])),
        ("mundi-store", (&["mundi-sim", "mundi-render", "mundi-net"][..], &["tokio-tungstenite", "fluent", "fluent-bundle"][..])),
        ("mundi-net", (&["mundi-sim", "mundi-store"][..], &["rusqlite"][..])),
        ("mundi-convert", (&["mundi-sim", "mundi-render", "mundi-store", "mundi-net"][..], &[][..])),
        ("mundi-telnet", (&["mundi-content", "mundi-sim", "mundi-render", "mundi-store", "mundi-net"][..], &["rusqlite", "fluent", "fluent-bundle"][..])),
        ("mundi-server", (&[][..], &[][..])),
    ])
}

/// Every dependency name of a Cargo.toml: [dependencies], [dev-dependencies], [build-dependencies],
/// and their [target.*] forms.
fn deps_of(manifest: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut in_deps = false;
    for line in manifest.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_deps = t.ends_with("dependencies]");
            continue;
        }
        if in_deps && !t.is_empty() && !t.starts_with('#') {
            if let Some(name) = t.split(['=', '.', ' ']).next() {
                out.insert(name.trim_matches('"').to_string());
            }
        }
    }
    out
}

fn manifests() -> BTreeMap<String, BTreeSet<String>> {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut out = BTreeMap::new();
    for entry in fs::read_dir(crates).unwrap() {
        let path = entry.unwrap().path().join("Cargo.toml");
        if let Ok(text) = fs::read_to_string(&path) {
            let name = path.parent().unwrap().file_name().unwrap().to_string_lossy().into_owned();
            out.insert(name, deps_of(&text));
        }
    }
    out
}

/// Everything a crate reaches: its own dependencies and, through other Mundi crates, theirs.
fn reach(name: &str, all: &BTreeMap<String, BTreeSet<String>>) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    let mut todo = vec![name.to_string()];
    while let Some(c) = todo.pop() {
        for d in all.get(&c).into_iter().flatten() {
            if seen.insert(d.clone()) && all.contains_key(d) {
                todo.push(d.clone());
            }
        }
    }
    seen
}

#[test]
fn every_crate_has_a_rule() {
    let all = manifests();
    let rules = rules();
    let missing: Vec<_> = all.keys().filter(|c| !rules.contains_key(c.as_str())).collect();
    assert!(missing.is_empty(), "add these crates to the rules in architecture.rs: {missing:?}");
}

#[test]
fn no_crate_reaches_what_it_must_not() {
    let all = manifests();
    let mut broken = Vec::new();
    for (name, (crates, libs)) in rules() {
        let reached = reach(name, &all);
        for bad in crates.iter().chain(libs.iter()) {
            if reached.contains(*bad) {
                broken.push(format!("{name} reaches {bad}"));
            }
        }
    }
    assert!(broken.is_empty(), "{broken:#?}");
}

#[test]
fn the_reader_sees_through_crates_and_target_sections() {
    let toml = "[package]\nname = \"x\"\n[dependencies]\na.workspace = true\nb = \"1\"\n\
                [target.'cfg(unix)'.dependencies]\nc = { version = \"1\" }\n[features]\nd = []\n";
    assert_eq!(deps_of(toml), BTreeSet::from(["a".into(), "b".into(), "c".into()]));
}
