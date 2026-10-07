//! Colours. Everything is resolved to literal values at render time so the
//! SVG needs no CSS (resvg, image viewers and e-mail clients all agree).
use crate::model::Theme;

#[derive(Debug, Clone)]
pub struct Palette {
    pub bg: &'static str,
    pub fg: &'static str,
    pub muted: &'static str,
    pub node_fill: &'static str,
    pub node_stroke: &'static str,
    pub edge: &'static str,
    pub group_fill: &'static str,
    pub group_stroke: &'static str,
    pub sticky_fill: &'static str,
    pub sticky_stroke: &'static str,
    pub shadow: &'static str,
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
    bg: "#ffffff",
    fg: "#1f2328",
    muted: "#656d76",
    node_fill: "#ffffff",
    node_stroke: "#8c959f",
    edge: "#6e7781",
    group_fill: "#f6f8fa",
    group_stroke: "#d0d7de",
    sticky_fill: "#fff1a6",
    sticky_stroke: "#e9d36a",
    shadow: "#1f232814",
    pin: "#7048e8",
    pin_resolved: "#8c959f",
    add: "#1a7f37",
    add_bg: "#dafbe1",
    del: "#cf222e",
    del_bg: "#ffebe9",
    chg: "#bf8700",
    chg_bg: "#fff8c5",
};

pub const DARK: Palette = Palette {
    bg: "#0d1117",
    fg: "#e6edf3",
    muted: "#8d96a0",
    node_fill: "#161b22",
    node_stroke: "#6e7681",
    edge: "#8d96a0",
    group_fill: "#ffffff08",
    group_stroke: "#30363d",
    sticky_fill: "#5c4d12",
    sticky_stroke: "#7d6a1c",
    shadow: "#00000055",
    pin: "#9775fa",
    pin_resolved: "#6e7681",
    add: "#3fb950",
    add_bg: "#12261e",
    del: "#f85149",
    del_bg: "#2d1416",
    chg: "#d29922",
    chg_bg: "#2b2111",
};

pub fn palette(t: Theme) -> &'static Palette {
    match t {
        Theme::Light => &LIGHT,
        Theme::Dark => &DARK,
    }
}

/// (fill, stroke) for a named or `#hex` colour.
pub fn tint(name: &str, t: Theme) -> (String, String) {
    let dark = t == Theme::Dark;
    let pair = |l: (&str, &str), d: (&str, &str)| {
        let (f, s) = if dark { d } else { l };
        (f.to_string(), s.to_string())
    };
    match name {
        "gray" => pair(("#f1f3f5", "#868e96"), ("#21262d", "#8b949e")),
        "blue" => pair(("#e7f0ff", "#3b6fd8"), ("#132339", "#58a6ff")),
        "green" => pair(("#e6f6ea", "#2f9e44"), ("#11271a", "#3fb950")),
        "yellow" => pair(("#fff6cc", "#d4a106"), ("#2d250b", "#d29922")),
        "orange" => pair(("#fff0e0", "#e8590c"), ("#2f1d0f", "#f0883e")),
        "red" => pair(("#ffeaea", "#e03131"), ("#301416", "#f85149")),
        "purple" => pair(("#f1eaff", "#7048e8"), ("#221a3a", "#a371f7")),
        "pink" => pair(("#ffe8f3", "#d6336c"), ("#2f1424", "#f778ba")),
        "teal" => pair(("#e3f8f6", "#0c8599"), ("#0f2629", "#39c5cf")),
        hex if hex.starts_with('#') => {
            let full = expand_hex(hex);
            (format!("{full}{}", if dark { "33" } else { "22" }), full)
        }
        _ => {
            let p = palette(t);
            (p.node_fill.to_string(), p.node_stroke.to_string())
        }
    }
}

/// Sticky notes use a saturated pastel instead of the outline style.
pub fn sticky(name: Option<&str>, t: Theme) -> (String, String) {
    let dark = t == Theme::Dark;
    let pick = |l: &str, d: &str| (if dark { d } else { l }).to_string();
    let fill = match name.unwrap_or("yellow") {
        "gray" => pick("#e9ecef", "#30363d"),
        "blue" => pick("#cfe2ff", "#1f3a5f"),
        "green" => pick("#cdf2d6", "#1d3f2a"),
        "orange" => pick("#ffd8b5", "#4a2c14"),
        "red" => pick("#ffcfcf", "#4a1d20"),
        "purple" => pick("#e0d4ff", "#352a58"),
        "pink" => pick("#ffd0e6", "#4a1f38"),
        "teal" => pick("#c3f0eb", "#163f43"),
        hex if hex.starts_with('#') => expand_hex(hex),
        _ => palette(t).sticky_fill.to_string(),
    };
    (fill, palette(t).sticky_stroke.to_string())
}

fn expand_hex(h: &str) -> String {
    if h.len() == 4 {
        let c: Vec<char> = h[1..].chars().collect();
        format!("#{0}{0}{1}{1}{2}{2}", c[0], c[1], c[2])
    } else {
        h[..7.min(h.len())].to_string()
    }
}
