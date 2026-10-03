//! Korean particles (D23): which of a pair follows a word, decided by how the word ends.

/// How a word ends, for choosing a particle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Final {
    /// No final consonant: 가, 는, 를, 와, 로, 야.
    None,
    /// ㄹ: like no final for 으로/로 (로), like a final for the rest.
    Rieul,
    /// Any other final consonant: 이, 은, 을, 과, 으로, 아.
    Other,
    /// Cannot tell: both forms, 이(가).
    Unknown,
}

impl Final {
    pub fn parse(s: &str) -> Option<Final> {
        match s {
            "none" => Some(Final::None),
            "rieul" => Some(Final::Rieul),
            "other" => Some(Final::Other),
            _ => None,
        }
    }
}

/// The pairs templates may name, written "with final/without final".
pub const PAIRS: &[&str] = &["이/가", "은/는", "을/를", "과/와", "으로/로", "아/야"];

/// The particle of `pair` ("이/가") after a word ending in `f`. None if the pair is not known.
pub fn particle(pair: &str, f: Final) -> Option<String> {
    if !PAIRS.contains(&pair) {
        return None;
    }
    let (with, without) = pair.split_once('/')?;
    Some(match (f, pair) {
        (Final::Rieul, "으로/로") => without.to_string(),
        (Final::Rieul | Final::Other, _) => with.to_string(),
        (Final::None, _) => without.to_string(),
        // 이(가), 으로(로): the form with the final first, as Korean writes it.
        (Final::Unknown, _) => format!("{with}({without})"),
    })
}

/// The word a particle follows: markup, one trailing parenthesis (the keyword, D18), and trailing
/// quotes, stops and spaces left out.
pub fn base_of(text: &str) -> String {
    let mut s = strip_markup(text);
    let trimmed = s.trim_end();
    if trimmed.ends_with(')') {
        if let Some(open) = trimmed.rfind('(') {
            s = trimmed[..open].to_string();
        }
    }
    s.trim_end_matches(|c: char| c.is_whitespace() || "'\".,!?~…".contains(c)).to_string()
}

/// How a displayed name ends: the last character of [`base_of`] decides.
pub fn final_of(text: &str) -> Final {
    let s = base_of(text);
    let s = s.as_str();
    let Some(last) = s.chars().last() else { return Final::Unknown };
    match last {
        '가'..='힣' => match (last as u32 - 0xAC00) % 28 {
            0 => Final::None,
            8 => Final::Rieul,
            _ => Final::Other,
        },
        '0'..='9' => number_final(s),
        c if c.is_ascii_alphabetic() => latin_final(s),
        _ => Final::Unknown,
    }
}

/// A number as Korean reads it: trailing zeros make 십 (ㅂ), 백 (ㄱ), 천 (ㄴ), 만 (ㄴ); otherwise
/// the last digit: 영 ㅇ, 일 ㄹ, 이 -, 삼 ㅁ, 사 -, 오 -, 육 ㄱ, 칠 ㄹ, 팔 ㄹ, 구 -.
fn number_final(s: &str) -> Final {
    let digits: String = s.chars().rev().take_while(|c| c.is_ascii_digit()).collect();
    let zeros = digits.chars().take_while(|c| *c == '0').count();
    if zeros > 0 && zeros < digits.len() {
        return Final::Other; // 십, 백, 천, 만, 억: all end in a consonant
    }
    match digits.chars().next() {
        Some('1' | '7' | '8') => Final::Rieul,
        Some('0' | '3' | '6') => Final::Other,
        _ => Final::None,
    }
}

/// D23 rule A: l is ㄹ; m, n, ng have a final; everything else (vowels, r, s, stops) has none.
fn latin_final(s: &str) -> Final {
    let lower = s.to_ascii_lowercase();
    match lower.chars().last() {
        Some('l') => Final::Rieul,
        Some('m' | 'n') => Final::Other,
        Some('g') if lower.ends_with("ng") => Final::Other,
        _ => Final::None,
    }
}

fn strip_markup(text: &str) -> String {
    mundi_content::markup::plain(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(word: &str, pair: &str) -> String {
        format!("{word}{}", particle(pair, final_of(word)).unwrap())
    }

    #[test]
    fn hangul_finals() {
        // ① final or not
        for (w, p, want) in [
            ("짐승", "이/가", "짐승이"),
            ("개", "이/가", "개가"),
            ("문지기", "은/는", "문지기는"),
            ("검", "을/를", "검을"),
            ("빵", "과/와", "빵과"),
            ("물", "과/와", "물과"),
            ("나무", "아/야", "나무야"),
            ("누군가", "이/가", "누군가가"),
        ] {
            assert_eq!(with(w, p), want);
        }
    }

    #[test]
    fn rieul_takes_ro() {
        // ② ㄹ: 로, not 으로; other finals 으로
        assert_eq!(with("칼", "으로/로"), "칼로");
        assert_eq!(with("길", "으로/로"), "길로");
        assert_eq!(with("검", "으로/로"), "검으로");
        assert_eq!(with("바다", "으로/로"), "바다로");
        assert_eq!(with("칼", "이/가"), "칼이");
    }

    #[test]
    fn numbers_and_latin() {
        // ③ digits by their Korean reading
        for (w, want) in [("0", "0이"), ("1", "1이"), ("2", "2가"), ("3", "3이"), ("4", "4가"), ("5", "5가"), ("6", "6이"),
            ("7", "7이"), ("8", "8이"), ("9", "9가"), ("10", "10이"), ("20", "20이"), ("100", "100이"), ("1000", "1000이"), ("12", "12가")] {
            assert_eq!(with(w, "이/가"), want, "{w}");
        }
        assert_eq!(with("7", "으로/로"), "7로");
        assert_eq!(with("6", "으로/로"), "6으로");
        assert_eq!(with("2", "으로/로"), "2로");
        // ③ Latin by rule A
        for (w, want) in [("Ana", "Ana가"), ("Vallen", "Vallen이"), ("Paul", "Paul이"), ("Tom", "Tom이"), ("Jung", "Jung이"),
            ("Peter", "Peter가"), ("Mark", "Mark가"), ("Bob", "Bob가"), ("Chris", "Chris가")] {
            assert_eq!(with(w, "이/가"), want, "{w}");
        }
        assert_eq!(with("Paul", "으로/로"), "Paul로");
        assert_eq!(with("Vallen", "으로/로"), "Vallen으로");
    }

    #[test]
    fn keyword_in_parentheses_and_markup() {
        // ④ the Korean before the parenthesis decides
        assert_eq!(with("짐승(beast)", "이/가"), "짐승(beast)이");
        assert_eq!(with("개(fido)", "이/가"), "개(fido)가");
        assert_eq!(with("칼(sword)", "으로/로"), "칼(sword)로");
        assert_eq!(with("{yellow}짐승{/yellow}", "을/를"), "{yellow}짐승{/yellow}을");
        assert_eq!(with("문지기(2.guard)", "은/는"), "문지기(2.guard)는");
    }

    #[test]
    fn unknown_endings_write_both() {
        assert_eq!(with("龍", "이/가"), "龍이(가)");
        assert_eq!(with("", "으로/로"), "으로(로)");
        assert_eq!(with("★", "을/를"), "★을(를)");
        assert_eq!(particle("에게/게", Final::None), None);
    }
}
