//! Colours. Everything is resolved to literal values at render time so the
//! SVG needs no CSS (resvg, image viewers and e-mail clients all agree).
//!
//! The look follows FigJam: white cards with hairline borders and soft
//! shadows on a dotted canvas, sections with a solid accent title pill,
//! saturated stickies. The neutrals, edges and diff colours are schema's
//! (`../schema`), so both tools look like one product.
use std::cell::RefCell;
use std::collections::HashMap;

use crate::model::{Theme, ThemeDef};

/// What a theme can set: (key, what it is).
pub const KEYS: &[(&str, &str)] = &[
    ("background", "the canvas"),
    ("dots", "the canvas's dot grid"),
    ("text", "text"),
    ("muted", "secondary text: details, axes, dates"),
    ("surface", "boxes"),
    ("border", "boxes' borders, grid lines"),
    ("line", "arrows"),
    ("section", "sections, swimlanes, charts, tables"),
    ("header", "title strips and table headers"),
    ("title", "section title pills"),
    ("title-text", "text on title pills"),
    ("pin", "comment pins"),
    ("font", "the font (a CSS font list)"),
    ("radius", "boxes' corner radius (8 by default)"),
];

/// The named colours a theme can change (`blue: #3b6fd8`).
pub const NAMED_KEYS: [&str; 9] = [
    "gray", "blue", "green", "yellow", "orange", "red", "purple", "pink", "teal",
];

/// Built-in themes, written in the language.
pub const PRESETS: &[(&str, &str)] = &[
    ("paper", "theme paper [base: light] {\n  background: #f6f1e7\n  dots: #e3d9c6\n  surface: #fffdf8\n  border: #d9ccb2\n  section: #fffaf0\n  header: #f2eadb\n  text: #2b2620\n  muted: #6e6253\n  line: #a8987d\n  title: #7a4f24\n  font: Georgia, 'Iowan Old Style', serif\n  radius: 4\n}\n"),
    ("contrast", "theme contrast [base: light] {\n  background: #ffffff\n  dots: #d0d0d0\n  surface: #ffffff\n  border: #000000\n  section: #ffffff\n  header: #eeeeee\n  text: #000000\n  muted: #2b2b2b\n  line: #000000\n  title: #000000\n}\n"),
];

/// Does a board's `theme:` name a file (`./brand.theme`)?
pub fn is_file(name: &str) -> bool {
    name.contains('/') || name.ends_with(".theme")
}

/// The (first) theme in a theme file's text, under the name the board uses
/// for it.
pub fn from_file(name: &str, src: &str) -> Result<ThemeDef, String> {
    let doc = crate::dsl::parse(src).map_err(|e| {
        e.iter()
            .map(|d| format!("{name}:{}: {}", d.line, d.message))
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    let mut t = doc
        .themes
        .into_iter()
        .next()
        .ok_or_else(|| format!("{name}: no `theme … {{ }}` in it"))?;
    t.id = name.to_string();
    Ok(t)
}

/// A built-in theme by name.
pub fn preset(name: &str) -> Option<ThemeDef> {
    let (_, src) = PRESETS.iter().find(|(n, _)| *n == name)?;
    crate::dsl::parse(src).ok()?.themes.into_iter().next()
}

thread_local! {
    /// Palettes made from themes, by theme (made once each).
    static MADE: RefCell<HashMap<String, &'static Palette>> = RefCell::new(HashMap::new());
    /// The named colours the theme being drawn changes.
    static NAMED_NOW: RefCell<Vec<(String, String)>> = const { RefCell::new(Vec::new()) };
}

/// The palette of a theme: its base's, with what it sets.
pub fn palette_for(def: &ThemeDef) -> &'static Palette {
    let key = format!("{def:?}");
    if let Some(p) = MADE.with(|m| m.borrow().get(&key).copied()) {
        return p;
    }
    let mut p = palette(def.base).clone();
    let leak = |v: &str| -> &'static str { Box::leak(v.to_string().into_boxed_str()) };
    for (k, v) in &def.values {
        let v = v.as_str();
        match k.as_str() {
            "background" => p.bg = leak(v),
            "dots" => p.dot = leak(v),
            "text" => {
                p.fg = leak(v);
                if def.get("title").is_none() {
                    p.group_accent = leak(v);
                }
            }
            "muted" => p.muted = leak(v),
            "surface" => p.node_fill = leak(v),
            "border" => p.node_stroke = leak(v),
            "line" => p.edge = leak(v),
            "section" => p.group_fill = leak(v),
            "header" => p.header = leak(v),
            "title" => p.group_accent = leak(v),
            "title-text" => p.on_accent = leak(v),
            "pin" => p.pin = leak(v),
            "font" => p.font = leak(v),
            "radius" => p.radius = v.parse().unwrap_or(p.radius),
            _ => {}
        }
    }
    let made: &'static Palette = Box::leak(Box::new(p));
    MADE.with(|m| m.borrow_mut().insert(key, made));
    made
}

/// While drawing with a theme: the named colours it changes.
pub fn set_named(def: Option<&ThemeDef>) {
    let named: Vec<(String, String)> = def
        .map(|d| {
            d.values
                .iter()
                .filter(|(k, _)| NAMED_KEYS.contains(&k.as_str()))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    NAMED_NOW.with(|n| *n.borrow_mut() = named);
}

#[derive(Debug, Clone)]
pub struct Palette {
    pub bg: &'static str,
    pub dot: &'static str,
    pub fg: &'static str,
    pub muted: &'static str,
    pub node_fill: &'static str,
    pub node_stroke: &'static str,
    pub edge: &'static str,
    pub group_fill: &'static str,
    /// Title strips (swimlanes): schema's table header.
    pub header: &'static str,
    pub group_accent: &'static str,
    /// Text on an accent fill (title pills, avatars).
    pub on_accent: &'static str,
    /// Sticky text stays dark: stickies are pastel in both themes.
    pub sticky_fg: &'static str,
    pub shadow_opacity: f64,
    pub pin: &'static str,
    pub pin_resolved: &'static str,
    pub add: &'static str,
    pub add_bg: &'static str,
    pub del: &'static str,
    pub del_bg: &'static str,
    pub chg: &'static str,
    pub chg_bg: &'static str,
    /// The font (a CSS font list).
    pub font: &'static str,
    /// Boxes' corner radius.
    pub radius: f64,
}

pub const LIGHT: Palette = Palette {
    bg: "#f4f5f7",
    dot: "#d3d8de",
    fg: "#1c2026",
    muted: "#5b636e",
    node_fill: "#ffffff",
    node_stroke: "#cfd4db",
    edge: "#9aa2ad",
    group_fill: "#ffffff",
    header: "#f7f8fa",
    group_accent: "#1c2026",
    on_accent: "#ffffff",
    sticky_fg: "#1c2026",
    shadow_opacity: 0.1,
    pin: "#6f45d6",
    pin_resolved: "#858d98",
    add: "#1b7f3a",
    add_bg: "#e6f5ea",
    del: "#c62a31",
    del_bg: "#fcebec",
    chg: "#935f00",
    chg_bg: "#fcf2d9",
    font: crate::text::FONT,
    radius: 8.0,
};

pub const DARK: Palette = Palette {
    bg: "#0f1114",
    dot: "#242930",
    fg: "#e3e7ec",
    muted: "#9ba4af",
    node_fill: "#181b21",
    node_stroke: "#353c46",
    edge: "#5f6873",
    group_fill: "#16191e",
    header: "#1d2127",
    group_accent: "#e3e7ec",
    on_accent: "#ffffff",
    sticky_fg: "#1c2026",
    shadow_opacity: 0.4,
    pin: "#ab91f8",
    pin_resolved: "#6c757f",
    add: "#5cc47a",
    add_bg: "#13281b",
    del: "#f07a7f",
    del_bg: "#33181b",
    chg: "#e3ae4c",
    chg_bg: "#2d2311",
    font: crate::text::FONT,
    radius: 8.0,
};

pub fn palette(t: Theme) -> &'static Palette {
    match t {
        Theme::Light => &LIGHT,
        Theme::Dark => &DARK,
    }
}

/// The colours a named or `#hex` colour resolves to.
#[derive(Debug, Clone)]
pub struct Tint {
    /// Saturated: title pills, card strips, avatars, coloured edges.
    pub accent: String,
    /// Very light: section frames and coloured cards.
    pub frame: String,
    /// Light: filled shapes (diamond, circle, …).
    pub soft: String,
    /// Frame border (accent at ~33%).
    pub border: String,
}

/// FigJam's palette: (name, accent, frame fill, soft fill).
const NAMED: [(&str, &str, &str, &str); 9] = [
    ("gray", "#757575", "#f7f7f5", "#f0f0f0"),
    ("blue", "#0d99ff", "#eef7ff", "#dcefff"),
    ("green", "#14ae5c", "#eefaf3", "#d9f4e4"),
    ("yellow", "#e5a000", "#fff8e1", "#ffefbf"),
    ("orange", "#f2780c", "#fff4ea", "#ffe3cc"),
    ("red", "#f24822", "#fff2ef", "#ffdfd8"),
    ("purple", "#9747ff", "#f6f0ff", "#ebdfff"),
    ("pink", "#e84393", "#fff0f7", "#ffdcec"),
    ("teal", "#00a3b8", "#e9fafc", "#d2f3f7"),
];

/// Accent colours handed out to people without an explicit colour, so a
/// team page gets distinct avatars.
pub const AVATARS: [&str; 8] = [
    "#0d99ff", "#9747ff", "#14ae5c", "#f2780c", "#f24822", "#00a3b8", "#e84393", "#7b61ff",
];

pub fn tint(name: &str, t: Theme) -> Option<Tint> {
    // a named colour the theme changes is that colour, as a hex one
    if let Some(v) = NAMED_NOW.with(|n| {
        n.borrow()
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
    }) {
        if v.starts_with('#') && v != name {
            return tint(&v, t);
        }
    }
    let dark = t == Theme::Dark;
    let (accent, frame, soft) = if let Some(&(_, a, f, s)) = NAMED.iter().find(|(n, ..)| *n == name)
    {
        if dark {
            (a.to_string(), format!("{a}1a"), format!("{a}33"))
        } else {
            (a.to_string(), f.to_string(), s.to_string())
        }
    } else if name.starts_with('#') {
        let a = expand_hex(name);
        let (fa, sa) = if dark { ("1a", "33") } else { ("12", "26") };
        (a.clone(), format!("{a}{fa}"), format!("{a}{sa}"))
    } else {
        return None;
    };
    Some(Tint {
        border: format!("{accent}55"),
        accent,
        frame,
        soft,
    })
}

/// Sticky notes: FigJam's saturated pastels, the same in both themes.
pub fn sticky(name: Option<&str>) -> String {
    match name.unwrap_or("yellow") {
        "yellow" => "#ffe299".into(),
        "green" => "#b3efbd".into(),
        "blue" => "#b8e2ff".into(),
        "pink" => "#ffc2ec".into(),
        "purple" => "#d9c2ff".into(),
        "orange" => "#ffcd99".into(),
        "red" => "#ffc7c2".into(),
        "teal" => "#b5eef5".into(),
        "gray" => "#e6e6e6".into(),
        hex if hex.starts_with('#') => expand_hex(hex),
        _ => "#ffe299".into(),
    }
}

/// A stable avatar colour for an id.
pub fn avatar(id: &str) -> &'static str {
    let h = id
        .bytes()
        .fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32));
    AVATARS[h as usize % AVATARS.len()]
}

fn expand_hex(h: &str) -> String {
    if h.len() == 4 {
        let c: Vec<char> = h[1..].chars().collect();
        format!("#{0}{0}{1}{1}{2}{2}", c[0], c[1], c[2])
    } else {
        h[..7.min(h.len())].to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "theme: brand\n\ntheme brand [base: dark] {\n  background: #101418\n  title: #5b5bd6\n  blue: #3b6fd8\n  font: 'IBM Plex Mono', monospace\n  radius: 2\n}\n\napi: API [blue]\n";

    #[test]
    fn themes_round_trip_and_draw() {
        let d = crate::dsl::parse(SRC).unwrap();
        assert_eq!(crate::dsl::print(&d), SRC);
        assert_eq!(d.settings.palette.as_deref(), Some("brand"));
        let t = &d.themes[0];
        assert_eq!((t.base, t.get("radius")), (Theme::Dark, Some("2")));
        let r = crate::render::render(&d, &Default::default());
        assert!(
            r.svg.contains("#101418")
                && r.svg.contains("#3b6fd8")
                && r.svg.contains("IBM Plex Mono"),
            "{}",
            r.svg
        );
        assert!(r.canvas.dark && r.canvas.background == "#101418");
        // after drawing, nothing of the theme stays behind
        assert!((crate::text::width("ab", 10.0) - 11.2).abs() < 1e-9);
        // every preset reads
        for (n, _) in PRESETS {
            assert!(preset(n).is_some(), "{n}");
        }
    }

    #[test]
    fn mistakes_in_themes_are_errors() {
        let e = crate::dsl::parse("theme t {\n  backgrund: #fff\n}").unwrap_err();
        assert!(
            e[0].message.contains("unknown theme key `backgrund`"),
            "{e:?}"
        );
        let e = crate::dsl::parse("theme t {\n  text: dark\n}").unwrap_err();
        assert!(e[0].message.contains("is a colour"), "{e:?}");
    }

    #[test]
    fn ops_add_replace_and_remove_themes_and_files_are_found_by_name() {
        let mut d = crate::dsl::parse("a").unwrap();
        let ops = crate::ops::parse_ops(r#"{"ops":[{"op":"theme","source":"theme t [base: light] {\n  text: #111111\n}"},{"op":"set","key":"theme","value":"t"},{"op":"theme","source":"theme t [base: light] {\n  text: #222222\n}"}]}"#).unwrap();
        crate::ops::apply(&mut d, &ops).unwrap();
        assert_eq!(d.themes.len(), 1);
        assert_eq!(d.themes[0].get("text"), Some("#222222"));
        crate::ops::apply(
            &mut d,
            &crate::ops::parse_ops(r#"{"op":"remove","id":"t"}"#).unwrap(),
        )
        .unwrap();
        assert!(d.themes.is_empty() && d.settings.palette.is_none());
        // a theme from a file, by the name the board uses for it
        let b = crate::dsl::parse("theme: ./brand.theme\na").unwrap();
        let f = from_file(
            "./brand.theme",
            "theme brand [base: light] {\n  background: #faf9f6\n}",
        )
        .unwrap();
        let r = crate::render::render(
            &b,
            &crate::render::RenderOptions {
                themes: vec![f],
                ..Default::default()
            },
        );
        assert_eq!(r.canvas.background, "#faf9f6");
    }
}
