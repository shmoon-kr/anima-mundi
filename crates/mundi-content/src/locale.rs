//! Translation overlays (D10, D19): `locales/<lang>/<zone>.yaml`, the translated strings of a zone by ID,
//! in the shape of the base files. Keywords and commands are never translated (D17); a missing entry
//! or field falls back to the base text.

use std::collections::HashMap;
use std::path::Path;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::load::LoadError;
use crate::model::{Id, Text};
use crate::names::Dir;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// A room's name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// A mob's or object's name in sentences.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short: Option<String>,
    /// A mob's or object's line in a room.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub long: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<Text>,
    /// An object's action description (the base `action`). Translated with the rest of the zone; no
    /// command shows it yet, but a zone file that has it must still load (zone 61 stopped the server).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub exits: IndexMap<Dir, ExitEntry>,
    /// In the order of the base entry's extras.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extras: Vec<ExtraEntry>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExitEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub look: Option<Text>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtraEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<Text>,
}

/// One language's overlays, every zone merged (IDs are unique world-wide, D16).
#[derive(Debug, Clone, Default)]
pub struct Locale {
    pub entries: HashMap<Id, Entry>,
}

impl Locale {
    pub fn get(&self, id: &str) -> Option<&Entry> {
        self.entries.get(id)
    }
}

/// Every `<number>.yaml` in `dir` (other files, such as the glossary, are not overlays).
pub fn load_locale(dir: &Path) -> Result<Locale, LoadError> {
    let mut locale = Locale::default();
    let Ok(read) = std::fs::read_dir(dir) else { return Ok(locale) };
    let mut files: Vec<_> = read
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "yaml") && p.file_stem().and_then(|s| s.to_str()).is_some_and(|s| s.parse::<u32>().is_ok()))
        .collect();
    files.sort();
    for f in files {
        let err = |message: String| LoadError { file: f.clone(), message };
        let text = std::fs::read_to_string(&f).map_err(|e| err(e.to_string()))?;
        let entries: IndexMap<Id, Entry> = serde_saphyr::from_str(&text).map_err(|e| err(e.to_string()))?;
        locale.entries.extend(entries);
    }
    Ok(locale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_korean_overlays_load() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/tbamud/locales/ko");
        let ko = load_locale(&dir).unwrap();
        let entrance = ko.get("tba:186:room:18600").expect("the Newbie Zone is translated");
        assert_eq!(entrance.name.as_deref(), Some("초보자 지역의 입구"));
        assert!(entrance.exits.contains_key(&Dir::North));
        assert!(ko.get("tba:186:mob:18601").and_then(|m| m.short.as_deref()).is_some());
        // an object's action description is part of a zone file too (zone 61's sign)
        assert!(ko.get("tba:61:obj:6121").and_then(|o| o.action.as_deref()).is_some());
    }
}
