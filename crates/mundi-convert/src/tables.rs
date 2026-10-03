//! tbaMUD's number tables and combat messages, read from its source and lib once, written as data
//! (D21). Only values are taken: the arrays of constants.c, the `case N: return V;` tables of
//! class.c, a few settings of config.c, the message file. No code moves.

use std::path::Path;

use indexmap::IndexMap;
use mundi_content::names::Sector;
use mundi_content::tables::*;

fn read(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(bytes.iter().map(|&b| b as char).collect()) // Latin-1, like the world files
}

/// The source without comments.
fn strip_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut chars = src.chars().peekable();
    let mut in_str = false;
    while let Some(c) = chars.next() {
        if in_str {
            out.push(c);
            if c == '\\' {
                if let Some(n) = chars.next() {
                    out.push(n);
                }
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match (c, chars.peek()) {
            ('"', _) => {
                in_str = true;
                out.push(c);
            }
            ('/', Some('*')) => {
                chars.next();
                let mut prev = ' ';
                for d in chars.by_ref() {
                    if prev == '*' && d == '/' {
                        break;
                    }
                    prev = d;
                }
                out.push(' ');
            }
            ('/', Some('/')) => {
                for d in chars.by_ref() {
                    if d == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            _ => out.push(c),
        }
    }
    out
}

/// The initialiser of `name[...] = { ... };`.
fn array<'a>(src: &'a str, name: &str) -> Result<&'a str, String> {
    let at = src.find(&format!("{name}[")).ok_or(format!("no array {name}"))?;
    let open = at + src[at..].find('{').ok_or(format!("{name}: no {{"))?;
    let close = open + src[open..].find("};").ok_or(format!("{name}: no }};"))?;
    Ok(&src[open + 1..close])
}

/// `{a, b, c}` rows of integers.
fn rows(body: &str) -> Result<Vec<Vec<i64>>, String> {
    let mut out = Vec::new();
    for part in body.split('{').skip(1) {
        let inner = part.split('}').next().unwrap_or("");
        out.push(ints(inner)?);
    }
    Ok(out)
}

fn ints(s: &str) -> Result<Vec<i64>, String> {
    s.split(',').map(str::trim).filter(|t| !t.is_empty()).map(|t| t.parse().map_err(|_| format!("not a number: {t}"))).collect()
}

/// The string literals in order, up to tbaMUD's "\n" end marker.
fn strings(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = body;
    while let Some(start) = rest.find('"') {
        let after = &rest[start + 1..];
        let end = after.find('"').unwrap_or(after.len());
        let s = &after[..end];
        if s == "\\n" {
            break;
        }
        out.push(s.to_string());
        rest = &after[(end + 1).min(after.len())..];
    }
    out
}

fn function<'a>(src: &'a str, signature: &str) -> Result<&'a str, String> {
    let at = src.find(signature).ok_or(format!("no function {signature}"))?;
    let open = at + src[at..].find('{').unwrap();
    let mut depth = 0;
    for (i, c) in src[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(&src[open..open + i]);
                }
            }
            _ => {}
        }
    }
    Err(format!("{signature}: unbalanced"))
}

/// The `case` tables of a function: every `return V;` with the labels in force, outermost first.
/// Labels are numbers, or names resolved by `names` (CLASS_THIEF -> "thief", LVL_IMMORT -> 31).
fn cases(body: &str, names: &dyn Fn(&str) -> Option<String>) -> Result<Vec<(Vec<String>, i64)>, String> {
    let mut out = Vec::new();
    // Each `switch` opens a level; a label replaces the label at its level.
    let mut labels: Vec<String> = Vec::new();
    let mut depth_of: Vec<i32> = Vec::new();
    let mut depth = 0;
    let mut pending_levels: Vec<String> = Vec::new();
    let tokens: Vec<&str> = body.split(|c: char| c.is_whitespace() || c == ';' || c == '(' || c == ')').filter(|t| !t.is_empty()).collect();
    let mut i = 0;
    while i < tokens.len() {
        match tokens[i] {
            "switch" => {
                depth_of.push(depth);
                pending_levels.clear();
            }
            "{" => depth += 1,
            "}" => {
                depth -= 1;
                while depth_of.last().is_some_and(|d| *d >= depth) {
                    depth_of.pop();
                    labels.truncate(depth_of.len());
                }
            }
            "case" => {
                let raw = tokens[i + 1].trim_end_matches(':');
                let label = if raw.parse::<i64>().is_ok() { raw.to_string() } else { names(raw).ok_or(format!("unknown label {raw}"))? };
                let level = depth_of.len();
                if labels.len() >= level {
                    labels.truncate(level - 1);
                }
                labels.push(label.clone());
                pending_levels.push(label);
                i += 1;
            }
            "return" => {
                if let Some(v) = tokens.get(i + 1).and_then(|t| t.parse::<i64>().ok()) {
                    // Fall-through labels share the value: `case 31: case 32: return 0;`
                    let base = &labels[..labels.len().saturating_sub(1)];
                    for l in pending_levels.drain(..) {
                        let mut key = base.to_vec();
                        key.push(l);
                        if key.len() == labels.len() {
                            out.push((key, v));
                        }
                    }
                }
                pending_levels.clear();
            }
            t if t.contains('{') || t.contains('}') => {
                for c in t.chars() {
                    if c == '{' {
                        depth += 1;
                    } else if c == '}' {
                        depth -= 1;
                        while depth_of.last().is_some_and(|d| *d >= depth) {
                            depth_of.pop();
                            labels.truncate(depth_of.len());
                        }
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    Ok(out)
}

const CLASSES: [(&str, &str); 4] =
    [("CLASS_MAGIC_USER", "magic_user"), ("CLASS_CLERIC", "cleric"), ("CLASS_THIEF", "thief"), ("CLASS_WARRIOR", "warrior")];
const SAVES: [(&str, &str); 5] =
    [("SAVING_PARA", "paralysis"), ("SAVING_ROD", "rod"), ("SAVING_PETRI", "petrification"), ("SAVING_BREATH", "breath"), ("SAVING_SPELL", "spell")];

fn label(name: &str) -> Option<String> {
    CLASSES.iter().chain(SAVES.iter()).find(|(c, _)| *c == name).map(|(_, n)| n.to_string()).or(match name {
        "LVL_IMMORT" => Some("31".into()),
        "LVL_GOD" => Some("32".into()),
        "LVL_GRGOD" => Some("33".into()),
        "LVL_IMPL" => Some("34".into()),
        _ => None,
    })
}

/// A list indexed by level from the (labels, value) pairs whose labels start with `prefix`.
fn by_level(cases: &[(Vec<String>, i64)], prefix: &[&str]) -> Result<Vec<i64>, String> {
    let mut out: Vec<Option<i64>> = Vec::new();
    for (k, v) in cases {
        if k.len() == prefix.len() + 1 && k.iter().zip(prefix).all(|(a, b)| a == b) {
            let level: usize = k[prefix.len()].parse().map_err(|_| format!("level {}", k[prefix.len()]))?;
            if out.len() <= level {
                out.resize(level + 1, None);
            }
            out[level] = Some(*v);
        }
    }
    out.into_iter().enumerate().map(|(l, v)| v.ok_or(format!("{prefix:?}: no level {l}"))).collect()
}

fn setting(src: &str, decl: &str) -> Result<i64, String> {
    let at = src.find(decl).ok_or(format!("no setting {decl}"))?;
    let rest = &src[at + decl.len()..];
    let value = rest.trim_start().trim_start_matches('=').trim_start();
    value.split(|c: char| !c.is_ascii_digit() && c != '-').next().unwrap().parse().map_err(|_| format!("{decl}: not a number"))
}

fn assignment(body: &str, lhs: &str) -> Result<i64, String> {
    let at = body.find(lhs).ok_or(format!("no {lhs}"))?;
    let rest = body[at + lhs.len()..].trim_start().trim_start_matches('=').trim_start();
    rest.split(';').next().unwrap().trim().parse().map_err(|_| format!("{lhs}: not a number"))
}

fn i32s(v: &[i64]) -> Vec<i32> {
    v.iter().map(|&x| x as i32).collect()
}

/// Reads `<src>/constants.c`, `class.c`, `config.c`, `spell_parser.c`, `limits.c`.
pub fn read_tables(src: &Path) -> Result<Tables, String> {
    let constants = strip_comments(&read(&src.join("constants.c"))?);
    let class = strip_comments(&read(&src.join("class.c"))?);
    let config = strip_comments(&read(&src.join("config.c"))?);
    let parser = strip_comments(&read(&src.join("spell_parser.c"))?);
    let limits = strip_comments(&read(&src.join("limits.c"))?);

    let field = |r: &Vec<i64>, i: usize| r.get(i).copied().unwrap_or(0) as i32;
    let abilities = Abilities {
        strength: rows(array(&constants, "str_app")?)?
            .iter()
            .map(|r| Strength { tohit: field(r, 0), todam: field(r, 1), carry_w: field(r, 2), wield_w: field(r, 3) })
            .collect(),
        dexterity: rows(array(&constants, "dex_app")?)?
            .iter()
            .map(|r| Dexterity { reaction: field(r, 0), miss_att: field(r, 1), defensive: field(r, 2) })
            .collect(),
        dexterity_skill: rows(array(&constants, "dex_app_skill")?)?
            .iter()
            .map(|r| DexteritySkill { p_pocket: field(r, 0), p_locks: field(r, 1), traps: field(r, 2), sneak: field(r, 3), hide: field(r, 4) })
            .collect(),
        constitution_hitp: rows(array(&constants, "con_app")?)?.iter().map(|r| field(r, 0)).collect(),
        intelligence_learn: rows(array(&constants, "int_app")?)?.iter().map(|r| field(r, 0)).collect(),
        wisdom_practices: rows(array(&constants, "wis_app")?)?.iter().map(|r| field(r, 0)).collect(),
    };

    let thaco = cases(function(&class, "int thaco(")?, &label)?;
    let exp = cases(function(&class, "int level_exp(")?, &label)?;
    let saves = cases(function(&class, "byte saving_throws(")?, &label)?;
    let prac = rows(array(&class, "prac_params")?.replace("SPELL", "0").replace("SKILL", "1").as_str())?;
    let mut classes = IndexMap::new();
    for (i, (_, name)) in CLASSES.iter().enumerate() {
        let mut saving_throws = IndexMap::new();
        for (_, s) in SAVES {
            saving_throws.insert(s.to_string(), i32s(&by_level(&saves, &[name, s])?));
        }
        classes.insert(name.to_string(), Class {
            thac0: i32s(&by_level(&thaco, &[name])?),
            level_exp: by_level(&exp, &[name])?,
            saving_throws,
            practice: Practice {
                learned: prac[0][i] as i32,
                max_gain: prac[1][i] as i32,
                min_gain: prac[2][i] as i32,
                kind: if prac[3][i] == 0 { "spell" } else { "skill" }.into(),
            },
        });
    }

    let costs = ints(array(&constants, "movement_loss")?)?;
    let movement_cost = Sector::ALL.iter().zip(&costs).map(|(s, c)| (*s, *c as i32)).collect();
    let (names, keywords, colours) =
        (strings(array(&constants, "drinks")?), strings(array(&constants, "drinknames")?), strings(array(&constants, "color_liquid")?));
    let aff = rows(array(&constants, "drink_aff")?)?;
    let liquids = (0..names.len())
        .map(|i| Liquid {
            name: names[i].clone(),
            keyword: keywords.get(i).cloned().unwrap_or_default(),
            colour: colours.get(i).cloned().unwrap_or_default(),
            // Indexed by the condition numbers: DRUNK 0, HUNGER 1, THIRST 2 (structs.h:493-495).
            drunk: aff[i][0] as i32,
            full: aff[i][1] as i32,
            thirst: aff[i][2] as i32,
        })
        .collect();
    let curve = |gain_fn: &str| -> Result<[i32; 7], String> {
        let body = function(&limits, gain_fn)?;
        let at = body.find("graf(").ok_or(format!("{gain_fn}: no graf"))?;
        // graf(age(ch)->year, p0, ..., p6); — the first argument has its own parentheses.
        let call = &body[at + 5..at + body[at..].find(';').unwrap()];
        let args = call.trim_end().trim_end_matches(')');
        let v: Vec<i32> = args.split(',').skip(1).map(|a| a.trim().parse().unwrap_or(0)).collect();
        v.try_into().map_err(|_| format!("{gain_fn}: not seven points"))
    };
    let syls = strings(array(&parser, "syls")?);
    let syllables = syls.chunks(2).take_while(|p| p.len() == 2 && !p[0].is_empty()).map(|p| [p[0].clone(), p[1].clone()]).collect();
    let start = function(&class, "void do_start(")?;
    let world = WorldTables {
        movement_cost,
        liquids,
        regen: Regen { hit: curve("int hit_gain(")?, mana: curve("int mana_gain(")?, moves: curve("int move_gain(")? },
        syllables,
        config: Config {
            start_room: format!("tba:30:room:{}", setting(&config, "room_vnum mortal_start_room")?),
            tunnel_size: setting(&config, "int tunnel_size")? as i32,
            max_exp_gain: setting(&config, "int max_exp_gain")?,
            max_exp_loss: setting(&config, "int max_exp_loss")?,
            npc_corpse_ticks: setting(&config, "int max_npc_corpse_time")? as i32,
            pc_corpse_ticks: setting(&config, "int max_pc_corpse_time")? as i32,
            new_character: NewCharacter {
                max_hit: assignment(start, "GET_MAX_HIT(ch)")? as i32,
                max_mana: assignment(start, "GET_MAX_MANA(ch)")? as i32,
                max_move: assignment(start, "GET_MAX_MOVE(ch)")? as i32,
            },
        },
    };
    Ok(Tables { abilities, classes, world })
}

/// Reads tbaMUD's `lib/misc/messages`: `M`, the number, then twelve lines (die, miss, hit, god, each
/// attacker / victim / room); `#` is no line. Comments `* name number` name the next block.
pub fn read_messages(path: &Path) -> Result<CombatMessages, String> {
    let text = read(path)?;
    let mut attacks: Vec<AttackMessages> = Vec::new();
    let mut name = String::new();
    let mut lines = text.lines().map(|l| l.trim_end_matches('\r'));
    while let Some(l) = lines.next() {
        if let Some(c) = l.strip_prefix('*') {
            let c = c.trim();
            // "* burning hands 5": the name without the trailing number.
            name = match c.rsplit_once(' ') {
                Some((n, num)) if num.parse::<i32>().is_ok() => n.trim().to_string(),
                _ => c.to_string(),
            };
            continue;
        }
        if l.trim() != "M" {
            continue;
        }
        let number: i32 = lines.next().unwrap_or("").trim().parse().map_err(|_| format!("{}: bad number after M", path.display()))?;
        let mut get = || -> Result<Option<String>, String> {
            let l = lines.next().ok_or(format!("{}: message {number} ends early", path.display()))?;
            Ok(if l.trim() == "#" { None } else { Some(crate::markup::markup(l)) })
        };
        let mut three = || -> Result<Lines, String> { Ok(Lines { attacker: get()?, victim: get()?, room: get()? }) };
        let set = MessageSet { die: three()?, miss: three()?, hit: three()?, god: three()? };
        match attacks.iter_mut().find(|a| a.number == number) {
            Some(a) => a.variants.push(set),
            None => attacks.push(AttackMessages { number, name: name.clone(), variants: vec![set] }),
        }
    }
    attacks.sort_by_key(|a| a.number);
    Ok(CombatMessages { attacks })
}
