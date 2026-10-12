//! Text measurement without fonts: Helvetica-like advance widths are close
//! enough for sizing boxes, and keep the output identical everywhere.

pub const FONT: &str =
    "Inter, -apple-system, BlinkMacSystemFont, 'Segoe UI', Helvetica, Arial, sans-serif";

/// Advance width of a character in em.
pub fn char_w(c: char) -> f64 {
    match c {
        'i' | 'j' | 'l' | '\'' | '|' | '.' | ',' | ':' | ';' | '!' => 0.24,
        ' ' | 'f' | 't' | 'I' | '(' | ')' | '[' | ']' | '/' | '\\' | '-' => 0.30,
        'r' | '"' | '`' => 0.34,
        'm' | 'M' => 0.84,
        'w' => 0.74,
        'W' => 0.95,
        'A'..='Z' => 0.68,
        '0'..='9' | 'a'..='z' => 0.56,
        c if c.is_ascii() => 0.58,
        c if is_wide(c) => 1.0,
        _ => 0.62,
    }
}

fn is_wide(c: char) -> bool {
    matches!(c as u32, 0x1100..=0x115F | 0x2E80..=0xA4CF | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF | 0xFE30..=0xFE4F | 0xFF00..=0xFF60 | 0xFFE0..=0xFFE6 | 0x1F300..=0x1FAFF)
}

thread_local! {
    /// While a monospace theme is drawn: every character is this wide (em).
    static MONO: std::cell::Cell<Option<f64>> = const { std::cell::Cell::new(None) };
}

/// Measure for a theme's font: a monospace one has one width for all.
pub fn set_font(font: &str) {
    let f = font.to_ascii_lowercase();
    MONO.with(|m| m.set((f.contains("mono") || f.contains("courier")).then_some(0.62)));
}

pub fn width(s: &str, size: f64) -> f64 {
    match MONO.with(|m| m.get()) {
        Some(w) => {
            s.chars()
                .map(|c| if is_wide(c) { 1.0 } else { w })
                .sum::<f64>()
                * size
        }
        None => s.chars().map(char_w).sum::<f64>() * size,
    }
}

/// Greedy word wrap to `max` px; explicit `\n` always breaks.
pub fn wrap(s: &str, size: f64, max: f64) -> Vec<String> {
    let mut out = Vec::new();
    for para in s.split('\n') {
        let mut line = String::new();
        for word in para.split(' ') {
            let candidate = if line.is_empty() {
                word.to_string()
            } else {
                format!("{line} {word}")
            };
            if width(&candidate, size) <= max || line.is_empty() && width(word, size) <= max {
                line = candidate;
                continue;
            }
            if !line.is_empty() {
                out.push(std::mem::take(&mut line));
            }
            // a single word wider than the box is broken by characters
            let mut chunk = String::new();
            for ch in word.chars() {
                if !chunk.is_empty() && width(&chunk, size) + char_w(ch) * size > max {
                    out.push(std::mem::take(&mut chunk));
                }
                chunk.push(ch);
            }
            line = chunk;
        }
        out.push(line);
    }
    out
}

pub fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            c => o.push(c),
        }
    }
    o
}

/// Compact number for SVG attributes.
pub fn n(v: f64) -> String {
    let r = (v * 10.0).round() / 10.0;
    if r.fract() == 0.0 {
        format!("{}", r as i64)
    } else {
        format!("{r:.1}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_words_and_newlines() {
        assert_eq!(wrap("a b", 14.0, 1000.0), vec!["a b"]);
        assert_eq!(wrap("one\ntwo", 14.0, 1000.0), vec!["one", "two"]);
        let lines = wrap("the quick brown fox jumps over the lazy dog", 14.0, 80.0);
        assert!(
            lines.len() > 2 && lines.iter().all(|l| width(l, 14.0) <= 80.0),
            "{lines:?}"
        );
        assert!(wrap("supercalifragilistic", 14.0, 40.0).len() > 1);
    }

    #[test]
    fn escapes() {
        assert_eq!(esc("<a & \"b\">"), "&lt;a &amp; &quot;b&quot;&gt;");
        assert_eq!(n(1.25), "1.3");
        assert_eq!(n(2.0), "2");
    }
}
