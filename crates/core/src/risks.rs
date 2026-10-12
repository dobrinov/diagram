//! Risk matrices: a group whose nodes are risks, placed on a grid by
//! probability (rows, likeliest on top) and impact (columns, worst on the
//! right). In the DSL a matrix is one risk per line:
//!
//! ```text
//! risks delivery: Delivery risks [scale: 5] {
//!   Bank contract slips [p: 4, i: 4, owner: ana, detail: Legal says 4 weeks]
//!   Scope creep [p: 2, i: 3]
//! }
//! ```
//!
//! A risk's `detail` is its description: the editor shows it on hover.
use crate::model::{Document, Group, Node};

/// Levels of probability and impact when a matrix does not say.
pub const DEFAULT_LEVELS: u8 = 5;

/// How many levels a matrix has (3 to 5).
pub fn levels(g: &Group) -> u8 {
    g.levels.unwrap_or(DEFAULT_LEVELS).clamp(3, 5)
}

/// Severities, from the score's share of the worst score: (name, colour).
pub const SEVERITY: [(&str, &str); 4] = [
    ("low", "green"),
    ("medium", "yellow"),
    ("high", "orange"),
    ("critical", "red"),
];

/// How serious probability `p` and impact `i` are on an `n`-level matrix
/// (an index into [`SEVERITY`]).
pub fn severity(p: u8, i: u8, n: u8) -> usize {
    let r = f64::from(p) * f64::from(i) / (f64::from(n) * f64::from(n));
    if r <= 0.16 {
        0
    } else if r <= 0.36 {
        1
    } else if r <= 0.6 {
        2
    } else {
        3
    }
}

/// The risks of matrix `g`, in document order (their numbers are 1, 2, …).
pub fn risks_of<'a>(doc: &'a Document, g: &str) -> Vec<&'a Node> {
    doc.nodes
        .iter()
        .filter(|n| n.group.as_deref() == Some(g))
        .collect()
}

/// A risk's probability and impact, when both are set (kept on the grid).
pub fn rating(n: &Node, levels: u8) -> Option<(u8, u8)> {
    Some((n.probability?.clamp(1, levels), n.impact?.clamp(1, levels)))
}

/// The matrix a node is a risk of.
pub fn matrix_of<'a>(doc: &'a Document, n: &Node) -> Option<&'a Group> {
    n.group
        .as_deref()
        .and_then(|g| doc.group(g))
        .filter(|g| g.risks)
}

/// "Probability 4 × impact 4 = 16 (critical)", or "Not rated yet".
pub fn summary(n: &Node, levels: u8) -> String {
    match rating(n, levels) {
        Some((p, i)) => format!(
            "Probability {p} × impact {i} = {} ({})",
            u16::from(p) * u16::from(i),
            SEVERITY[severity(p, i, levels)].0
        ),
        None => "Not rated yet".into(),
    }
}

/// The text shown when hovering a risk: label, rating, owner, description.
pub fn tooltip(n: &Node, levels: u8) -> String {
    let mut s = format!("{}\n{}", n.display(), summary(n, levels));
    if let Some(o) = &n.owner {
        s.push_str(&format!("\nOwner: {o}"));
    }
    if let Some(d) = &n.detail {
        s.push_str(&format!("\n\n{d}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "risks r: Risks [scale: 5] {\n  Contract slips [p: 4, i: 4, owner: ana, detail: \"Legal says 4 weeks, blocks the pilot\"]\n  Scope creep [p: 2, i: 3]\n  Unknowns\n}\n";

    #[test]
    fn matrices_round_trip_and_place_risks_by_rating() {
        let d = crate::dsl::parse(SRC).unwrap();
        assert_eq!(crate::dsl::print(&d), SRC);
        let g = d.group("r").unwrap();
        assert!(g.risks && levels(g) == 5);
        let c = d.node("contract-slips").unwrap();
        assert_eq!((c.probability, c.impact), (Some(4), Some(4)));
        let lay = crate::layout::layout(&d);
        let m = lay.group("r").unwrap().matrix.clone().unwrap();
        // probability 4 is the second row from the top, impact 4 the fourth column
        let r = lay.rect("contract-slips").unwrap();
        assert!(
            (r.cx() - (m.left + 3.5 * m.cell)).abs() < 1.0,
            "{r:?} {m:?}"
        );
        assert!((r.cy() - (m.top + 1.5 * m.cell)).abs() < 1.0, "{r:?} {m:?}");
        assert!(lay.node("contract-slips").unwrap().risk);
        assert_eq!(m.rows, ["contract-slips", "scope-creep", "unknowns"]);
        // hover text
        let svg = crate::render::render(&d, &Default::default()).svg;
        assert!(svg.contains("<title>Contract slips\nProbability 4 × impact 4 = 16 (critical)\nOwner: ana\n\nLegal says 4 weeks, blocks the pilot</title>"), "{svg}");
        assert_eq!(
            (severity(1, 5, 5), severity(3, 3, 5), severity(2, 1, 3)),
            (1, 1, 1)
        );
    }

    #[test]
    fn prompts_list_the_worst_first_and_ops_rate_risks() {
        let mut d = crate::dsl::parse(SRC).unwrap();
        let p = crate::prompt::selection(&d, &["r".to_string()]);
        assert_eq!(
            p,
            "# Risks (risk matrix, probability × impact, 5×5)\n\n- Contract slips: probability 4 × impact 4 = 16 (critical), owner ana — Legal says 4 weeks, blocks the pilot\n- Scope creep: probability 2 × impact 3 = 6 (medium)\n- Unknowns: not rated yet\n"
        );
        let ops = crate::ops::parse_ops(r#"{"ops":[{"op":"update_node","id":"unknowns","probability":3,"impact":5},{"op":"update_group","id":"r","levels":3}]}"#).unwrap();
        let old = d.clone();
        crate::ops::apply(&mut d, &ops).unwrap();
        assert_eq!(d.node("unknowns").unwrap().impact, Some(5));
        assert_eq!(levels(d.group("r").unwrap()), 3);
        let out = crate::patch::patch(SRC, &old, &d);
        assert!(
            out.contains("risks r: Risks [scale: 3] {")
                && out.contains("  Unknowns [p: 3, i: 5]\n"),
            "{out}"
        );
        assert_eq!(crate::query::select(&d, &["kind:risk"]).unwrap().len(), 3);
    }
}
