//! tbaMUD colour codes to the content format's markup (D20).
//!
//! tbaMUD switches colours on and off inline: `@y` turns yellow on until `@n` (or another colour),
//! `@+` bold, `@_` underline, `@-` blink, `@=` reverse, `@[fRGB]` an xterm colour (channels 0..5), `@@` a literal `@`
//! (modify.c parse_at, protocol.c ProcessOutput). Unknown codes show nothing.
//! The markup is paired: `{yellow}...{/yellow}`. A new colour closes the open one first; `@n` closes
//! everything; whatever is open at the end of a paragraph is closed there and opened again at the start
//! of the next one, so every paragraph is balanced on its own. Literal braces are `{{` and `}}`.

const COLOURS: &[(char, &str)] = &[
    ('d', "black"), ('D', "grey"), ('a', "azure"), ('A', "bright_azure"), ('r', "red"), ('R', "bright_red"),
    ('g', "green"), ('G', "bright_green"), ('y', "yellow"), ('Y', "bright_yellow"), ('b', "blue"),
    ('B', "bright_blue"), ('m', "magenta"), ('M', "bright_magenta"), ('c', "cyan"), ('C', "bright_cyan"),
    ('w', "white"), ('W', "bright_white"), ('o', "orange"), ('O', "bright_orange"), ('p', "pink"),
    ('P', "bright_pink"),
];
const ATTRS: &[(char, &str)] = &[('+', "bold"), ('_', "underline"), ('-', "blink"), ('=', "reverse")];

#[derive(Default, Clone)]
struct Open {
    colour: Option<String>,
    attrs: Vec<String>,
}

impl Open {
    fn close_all(&mut self, out: &mut String) {
        if let Some(c) = self.colour.take() {
            out.push_str(&format!("{{/{c}}}"));
        }
        while let Some(a) = self.attrs.pop() {
            out.push_str(&format!("{{/{a}}}"));
        }
    }
    fn reopen(&self, out: &mut String) {
        for a in &self.attrs {
            out.push_str(&format!("{{{a}}}"));
        }
        if let Some(c) = &self.colour {
            out.push_str(&format!("{{{c}}}"));
        }
    }
}

/// One paragraph (or one line-preserving text), carrying what is still open from the previous one.
fn convert(text: &str, open: &mut Open) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    open.reopen(&mut out);
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '{' => out.push_str("{{"),
            '}' => out.push_str("}}"),
            '@' => match chars.next() {
                None => out.push('@'),
                Some('@') | Some('*') => out.push('@'),
                Some('n') => {
                    let mut o = std::mem::take(open);
                    o.close_all(&mut out);
                }
                Some('[') => {
                    // @[fRGB]: foreground, each channel 0..5 (protocol.c ColourRGB) -> xterm 16 + 36R + 6G + B.
                    // @[bRGB] (background) has no markup yet and is dropped (none in the stock world).
                    let code: String = chars.by_ref().take_while(|c| *c != ']').collect();
                    let rgb: Vec<u32> = code.chars().skip(1).filter_map(|c| c.to_digit(10)).collect();
                    if code.starts_with(['f', 'F']) && rgb.len() == 3 && rgb.iter().all(|v| *v <= 5) {
                        set_colour(&mut out, open, format!("x{}", 16 + 36 * rgb[0] + 6 * rgb[1] + rgb[2]));
                    }
                }
                Some(c) => {
                    if let Some((_, name)) = COLOURS.iter().find(|(k, _)| *k == c) {
                        set_colour(&mut out, open, name.to_string());
                    } else if let Some((_, name)) = ATTRS.iter().find(|(k, _)| *k == c) {
                        if !open.attrs.iter().any(|a| a == name) {
                            out.push_str(&format!("{{{name}}}"));
                            open.attrs.push(name.to_string());
                        }
                    }
                    // anything else: tbaMUD shows nothing
                }
            },
            c => out.push(c),
        }
    }
    let mut o = open.clone();
    o.close_all(&mut out);
    out
}

fn set_colour(out: &mut String, open: &mut Open, name: String) {
    if let Some(c) = open.colour.take() {
        out.push_str(&format!("{{/{c}}}"));
    }
    out.push_str(&format!("{{{name}}}"));
    open.colour = Some(name);
}

/// A single string (a name, a short description): balanced, nothing carried out.
pub fn markup(text: &str) -> String {
    convert(text, &mut Open::default())
}

/// Paragraphs of one text: what is open carries to the next paragraph (closed and reopened at the break).
pub fn markup_paragraphs<'a>(paras: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut open = Open::default();
    paras.into_iter().map(|p| convert(p, &mut open)).collect()
}

/// The text without markup (what an agent receives; D20): tags removed, `{{` `}}` back to braces.
pub fn plain(marked: &str) -> String {
    let mut out = String::with_capacity(marked.len());
    let mut chars = marked.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, chars.peek()) {
            ('{', Some('{')) => {
                chars.next();
                out.push('{');
            }
            ('}', Some('}')) => {
                chars.next();
                out.push('}');
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
    fn pairs_close_and_switch() {
        assert_eq!(markup("\"@RTell guide help@y\" for assistance.@n"),
                   "\"{bright_red}Tell guide help{/bright_red}{yellow}\" for assistance.{/yellow}");
        assert_eq!(markup("@rred without an end"), "{red}red without an end{/red}");
        assert_eq!(markup("@+@ybold yellow@n plain"), "{bold}{yellow}bold yellow{/yellow}{/bold} plain");
        assert_eq!(markup("mail@@host and {braces}"), "mail@host and {{braces}}");
        assert_eq!(markup("@[f511]rose@n @qunknown"), "{x203}rose{/x203} unknown");      // 16 + 36*5 + 6*1 + 1
    }

    #[test]
    fn colour_carries_across_paragraphs_but_each_is_balanced() {
        assert_eq!(markup_paragraphs(["@gstart", "still green@n done"]),
                   vec!["{green}start{/green}".to_string(), "{green}still green{/green} done".to_string()]);
    }

    #[test]
    fn agents_get_plain_text() {
        assert_eq!(plain("{bright_red}Tell{/bright_red} {{x}}"), "Tell {x}");
    }
}
