//! Markup in content text (D20): `{name}...{/name}`, paired, every paragraph balanced on its own.
//!
//! Names: colours (what the original world meant: `red`, `bright_red`, ..., `x0`..`x255` for xterm),
//! attributes (`bold`, `underline`, `blink`, `reverse`), and semantic styles `style:<name>` reserved for
//! engine messages (danger, emphasis...; renderers choose how they look: colour-blind palettes, themes).
//! Literal braces are `{{` and `}}`. The renderer turns markup into ANSI, CSS, or nothing; agents always
//! receive plain text, never tags to parse.

pub const COLOURS: &[&str] = &[
    "black", "grey", "azure", "bright_azure", "red", "bright_red", "green", "bright_green", "yellow", "bright_yellow",
    "blue", "bright_blue", "magenta", "bright_magenta", "cyan", "bright_cyan", "white", "bright_white", "orange",
    "bright_orange", "pink", "bright_pink",
];
pub const ATTRS: &[&str] = &["bold", "underline", "blink", "reverse"];

fn known(name: &str) -> bool {
    COLOURS.contains(&name)
        || ATTRS.contains(&name)
        || name.strip_prefix('x').and_then(|n| n.parse::<u16>().ok()).is_some_and(|n| n <= 255)
        || name.strip_prefix("style:").is_some_and(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_lowercase() || c == '_'))
}

/// The tags of a text in order ("red", "/red", ...), or why it is not valid markup.
pub fn tags(text: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, chars.peek()) {
            ('{', Some('{')) | ('}', Some('}')) => {
                chars.next();
            }
            ('{', _) => {
                let tag: String = chars.by_ref().take_while(|d| *d != '}').collect();
                let (closing, name) = match tag.strip_prefix('/') {
                    Some(n) => (true, n.to_string()),
                    None => (false, tag.clone()),
                };
                if !known(&name) {
                    return Err(format!("unknown markup {{{tag}}}"));
                }
                if closing {
                    match stack.pop() {
                        Some(open) if open == name => {}
                        Some(open) => return Err(format!("{{/{name}}} closes {{{open}}}")),
                        None => return Err(format!("{{/{name}}} closes nothing")),
                    }
                } else {
                    stack.push(name.clone());
                }
                out.push(tag);
            }
            ('}', _) => return Err("a lone } (write }} for a brace)".into()),
            _ => {}
        }
    }
    match stack.last() {
        Some(open) => Err(format!("{{{open}}} is not closed")),
        None => Ok(out),
    }
}

/// The text an agent receives: tags removed, `{{` `}}` back to braces.
pub fn plain(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, chars.peek()) {
            ('{', Some('{')) | ('}', Some('}')) => {
                chars.next();
                out.push(c);
            }
            ('{', _) => {
                for d in chars.by_ref() {
                    if d == '}' {
                        break;
                    }
                }
            }
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balanced_known_tags_only() {
        assert_eq!(tags("{red}a{/red} {{b}}").unwrap(), vec!["red", "/red"]);
        assert!(tags("{red}a").is_err());
        assert!(tags("{red}a{/green}").is_err());
        assert!(tags("{purple}a{/purple}").is_err());
        assert!(tags("{style:danger}x{/style:danger}").is_ok());
        assert!(tags("a } b").is_err());
        assert_eq!(plain("{bold}{x214}hi{/x214}{/bold} {{x}}"), "hi {x}");
    }
}
