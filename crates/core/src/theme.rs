//! Colours. Everything is resolved to literal values at render time so the
//! SVG needs no CSS (resvg, image viewers and e-mail clients all agree).
//!
//! The look follows FigJam: white cards with hairline borders and soft
//! shadows on a dotted canvas, sections with a solid accent title pill,
//! saturated stickies. The neutrals, edges and diff colours are schema's
//! (`../schema`), so both tools look like one product.
use crate::model::Theme;

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
