//! `mundi-convert`: tbaMUD world files (`.wld .mob .obj .zon .shp`) to the content format, once.
//! IDs are `tba:<zone>:<kind>:<vnum>` (D16). Output lives in `third_party/tbamud/` (licence).
//!
//! Usage: mundi-convert <tbaMUD lib/world> <out dir> [zone ...]
//!        mundi-convert tables <tbaMUD src> <out dir>         number tables (D21)
//!        mundi-convert messages <tbaMUD lib/misc/messages> <out file>   combat messages (MECHANICS §8.2)
//! Converts every zone (or the ones named), then loads the result back with mundi-content and checks
//! the whole world: counts, references, IDs, doors on both sides. Exit code 1 if anything is wrong.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

use mundi_content::load::yaml;
use mundi_content::{check_world, load_messages, load_tables, load_world, write_zone};
use mundi_convert::{convert_zone, tables, tba};

const HEADER: &str = "# tbaMUD-derived (third_party/tbamud/NOTICE.md). Converted from tbaMUD zone {z}.\n";

fn legends() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
        ("zone.yaml", "# reset.every_minutes: minutes between resets; reset.when: never | when_empty (no players in the zone) | always\n"),
        ("rooms.yaml", "# Entries by ID, in vnum order. Prose is folded (>): wrapped here, several paragraphs are a list, the\n\
                        # renderer wraps it per language. exits.<dir>: to (room ID), look (what `look <dir>` shows), door\n\
                        # (keywords, kind door|pickproof, key, reset: the state the zone reset puts it in).\n"),
        ("mobs.yaml", "# keywords: words to name it in commands; short: in sentences (\"the pit beast\"); long: its line in a room.\n\
                       # combat.armor is tbaMUD's armor class / 10 (lower is better); hit_points and damage: a number, or dice NdS+B when rolled.\n"),
        ("objects.yaml", "# values are named per type (armor: armor; weapon: damage, attack; drinkcon: capacity, contains, liquid...).\n\
                          # level: minimum level to use it. affects: apply + modifier while worn.\n"),
        ("resets.yaml", "# What the zone reset puts back. A spawn is a mob in a room (with what it wears and carries) or an object\n\
                         # in a room (with its contents). limit: at most this many of it may exist in the whole world; the item\n\
                         # is not loaded if there are already that many. Door states are on the doors (rooms.yaml).\n"),
        ("shops.yaml", "# keeper (mob), rooms, products (always for sale), buys (item types, optionally with keywords),\n\
                        # profit (we pay cost x buy, get cost x min(sell, buy)), messages (%s customer, %d price), hours (empty: always open).\n"),
    ])
}

const DATA_HEADER: &str = "# tbaMUD-derived (third_party/tbamud/NOTICE.md). Values only, from tbaMUD {from}, by `mundi-convert`.\n# Do not edit by hand: regenerate. The engine reads this file (D21); MECHANICS.md says what each value means.\n";

/// Writes the number tables and reads them back.
fn write_tables(src: &str, out: &str) -> Result<(), String> {
    let t = tables::read_tables(std::path::Path::new(src))?;
    let out = PathBuf::from(out);
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let write = |file: &str, from: &str, body: String| {
        std::fs::write(out.join(file), DATA_HEADER.replace("{from}", from) + &body).map_err(|e| e.to_string())
    };
    write("commands.yaml", "src/interpreter.c (cmd_info: name, position, level, in order)", yaml(&t.commands))?;
    write("abilities.yaml", "src/constants.c (str_app, dex_app, dex_app_skill, con_app, int_app, wis_app)", yaml(&t.abilities))?;
    write("spells.yaml", "src/spells.h, spell_parser.c (spello, skillo), class.c (init_spell_levels)", yaml(&t.spells))?;
    write("classes.yaml", "src/class.c (thaco, level_exp, saving_throws, prac_params)", yaml(&t.classes))?;
    write("world.yaml", "src/constants.c, limits.c, spell_parser.c, config.c, class.c do_start", yaml(&t.world))?;
    let back = load_tables(&out).map_err(|e| e.to_string())?;
    if back != t {
        return Err("tables do not read back the same".into());
    }
    println!("tables: {} commands, {} strength rows, {} classes, {} liquids, {} syllables", t.commands.len(), t.abilities.strength.len(), t.classes.len(), t.world.liquids.len(), t.world.syllables.len());
    Ok(())
}

fn write_messages(src: &str, out: &str) -> Result<(), String> {
    let m = tables::read_messages(std::path::Path::new(src))?;
    let out = PathBuf::from(out);
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(&out, DATA_HEADER.replace("{from}", "lib/misc/messages") + &yaml(&m)).map_err(|e| e.to_string())?;
    if load_messages(&out).map_err(|e| e.to_string())? != m {
        return Err("messages do not read back the same".into());
    }
    let variants: usize = m.attacks.iter().map(|a| a.variants.len()).sum();
    println!("messages: {} attack types, {variants} variants", m.attacks.len());
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let sub = match args.first().map(String::as_str) {
        Some("tables") if args.len() == 3 => Some(write_tables(&args[1], &args[2])),
        Some("messages") if args.len() == 3 => Some(write_messages(&args[1], &args[2])),
        _ => None,
    };
    if let Some(result) = sub {
        return match result {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{e}");
                ExitCode::FAILURE
            }
        };
    }
    if args.len() < 2 {
        eprintln!("usage: mundi-convert <tbaMUD lib/world> <out dir> [zone ...]");
        return ExitCode::from(2);
    }
    let (world, out) = (PathBuf::from(&args[0]), PathBuf::from(&args[1]));
    let only: Vec<u32> = args[2..].iter().filter_map(|a| a.parse().ok()).collect();
    let zones = tba::read_zones(&world);
    let ids = tba::Ids::new(&zones).scan(&world);
    let mut notes = tba::Notes::default();
    let legend = legends();
    let started = std::time::Instant::now();
    for z in zones.values().filter(|z| only.is_empty() || only.contains(&z.num)) {
        let content = convert_zone(&world, z, &ids, &mut notes);
        if let Err(e) = write_zone(&out.join(z.num.to_string()), &content, &HEADER.replace("{z}", &z.num.to_string()), &legend) {
            eprintln!("zone {}: {e}", z.num);
            return ExitCode::FAILURE;
        }
    }
    let converted = started.elapsed();

    let started = std::time::Instant::now();
    let loaded = match load_world(&out) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("load: {e}");
            return ExitCode::FAILURE;
        }
    };
    let loaded_in = started.elapsed();
    let report = check_world(&loaded);
    println!("converted in {:.2}s, loaded back in {:.2}s", converted.as_secs_f64(), loaded_in.as_secs_f64());
    for (k, v) in &report.counts {
        println!("  {k:20} {v}");
    }
    println!("converter notes: {}", notes.0.len());
    for n in notes.0.iter().take(20) {
        println!("  {n}");
    }
    println!("warnings: {}", report.warnings.len());
    for w in report.warnings.iter().take(20) {
        println!("  {w}");
    }
    println!("errors: {}", report.errors.len());
    for e in report.errors.iter().take(40) {
        println!("  {e}");
    }
    let mut md = String::from("# Conversion report\n\nWritten by `mundi-convert` (tbaMUD world → content format). \
        Regenerated with the content; do not edit.\n\n## Counts\n\n");
    for (k, v) in &report.counts {
        md.push_str(&format!("- {k}: {v}\n"));
    }
    let section = |md: &mut String, title: &str, why: &str, items: &[String]| {
        md.push_str(&format!("\n## {title} ({})\n\n{why}\n\n", items.len()));
        for i in items {
            md.push_str(&format!("- {i}\n"));
        }
    };
    section(&mut md, "Converter notes", "References in the original to things that do not exist (dropped), and unknown values.", &notes.0);
    section(&mut md, "Warnings", "Doors that differ between their two sides in the original world.", &report.warnings);
    section(&mut md, "Errors", "Must be empty.", &report.errors);
    if let Err(e) = std::fs::write(out.join("CONVERSION-REPORT.md"), md) {
        eprintln!("report: {e}");
    }
    if report.errors.is_empty() { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}
