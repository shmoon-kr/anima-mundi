use mundi_content::model::Text;
use mundi_content::load::yaml;

#[test]
fn prose_round_trips_without_gaining_a_newline() {
    let long = "word ".repeat(40).trim_end().to_string() + "\nSecond paragraph.";
    let out = yaml(&Text::Prose(long.clone()));
    eprintln!("{out}");
    let back: Text = serde_saphyr::from_str(&out).unwrap();
    assert_eq!(back, Text::Prose(long));
}

#[test]
fn unknown_keys_and_misspelt_names_are_load_errors() {
    use indexmap::IndexMap;
    use mundi_content::model::Room;
    let ok = "tba:1:room:1:\n  name: A\n  description: B\n  sector: city\n  flags: [indoors]\n";
    assert!(serde_saphyr::from_str::<IndexMap<String, Room>>(ok).is_ok());
    let typo_flag = ok.replace("[indoors]", "[indors]");
    assert!(serde_saphyr::from_str::<IndexMap<String, Room>>(&typo_flag).is_err());
    let unknown_key = format!("{ok}  colour: red\n");
    assert!(serde_saphyr::from_str::<IndexMap<String, Room>>(&unknown_key).is_err());
}

#[test]
fn ids_name_their_kind_and_zone() {
    assert_eq!(mundi_content::parse_id("tba:186:mob:18602"), Some((186, "mob", 18602)));
    assert_eq!(mundi_content::parse_id("tba:186:18602"), None);
    assert_eq!(mundi_content::parse_id("tba:186:npc:18602"), None);
}
