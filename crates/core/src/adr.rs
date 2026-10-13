//! Decision records: a group whose nodes are the parts of one decision.
//!
//! ```text
//! adr db: Use Postgres for orders [accepted] {
//!   context: Orders and payments change together.
//!   options: Postgres, DynamoDB
//!   decision: Postgres on RDS.
//!   consequences: We run migrations.
//! }
//! ```
//!
//! Each line is a field: its name is the node's label (and gives its id,
//! `db.context`), its text the node's `detail`. [`plot`] draws the record like
//! a chart (see [`crate::charts::Plot`]): the status, then each field's name
//! over its text.
use crate::charts::{Mark, Paint, Plot};
use crate::layout::Rect;
use crate::model::{Document, Group, Node};
use crate::text;

/// The statuses a decision can have, with the colour each is drawn in.
pub const STATUSES: &[(&str, &str)] = &[
    ("proposed", "blue"),
    ("accepted", "green"),
    ("rejected", "red"),
    ("superseded", "gray"),
    ("deprecated", "orange"),
];

/// The fields a new record starts with.
pub const FIELDS: &[&str] = &["context", "options", "decision", "consequences"];

const WIDTH: f64 = 420.0;
const SIZE: f64 = 13.0;
const LINE: f64 = 18.0;
const NAME: f64 = 11.0;
const GAP: f64 = 14.0;

pub fn valid_status(s: &str) -> bool {
    STATUSES.iter().any(|(n, _)| *n == s)
}

pub fn status_error(s: &str) -> String {
    let names: Vec<&str> = STATUSES.iter().map(|(n, _)| *n).collect();
    format!("unknown decision status `{s}` (use {})", names.join(", "))
}

/// The fields of record `g`, in document order.
pub fn fields_of<'a>(doc: &'a Document, g: &str) -> Vec<&'a Node> {
    doc.nodes
        .iter()
        .filter(|n| n.group.as_deref() == Some(g))
        .collect()
}

/// The record a node is a field of.
pub fn record_of<'a>(doc: &'a Document, n: &Node) -> Option<&'a Group> {
    n.group
        .as_deref()
        .and_then(|g| doc.group(g))
        .filter(|g| g.adr.is_some())
}

/// The record drawn: its status in a pill, then each field's name (small,
/// muted) over its wrapped text, with a line between fields.
pub fn plot(doc: &Document, g: &Group) -> Plot {
    let w = g.w.unwrap_or(WIDTH);
    let status = g.adr.as_deref().unwrap_or("proposed");
    let colour = STATUSES
        .iter()
        .find(|(n, _)| *n == status)
        .map_or("gray", |(_, c)| *c);
    let mut p = Plot::default();
    let label = status.to_uppercase();
    let pill = text::width(&label, NAME) * 1.1 + 18.0;
    p.marks.push(Mark::Rect {
        r: Rect {
            x: 0.0,
            y: 0.0,
            w: pill,
            h: 20.0,
        },
        paint: Paint::Soft(colour.into()),
        round: 10.0,
    });
    p.marks.push(Mark::Text {
        x: 9.0,
        y: 14.0,
        text: label,
        size: NAME,
        anchor: "start",
        paint: Paint::Color(colour.into()),
        bold: true,
    });
    let mut y = 20.0 + GAP;
    for (k, f) in fields_of(doc, &g.id).into_iter().enumerate() {
        if k > 0 {
            p.marks.push(Mark::Line {
                x1: 0.0,
                y1: y - GAP / 2.0,
                x2: w,
                y2: y - GAP / 2.0,
                paint: Paint::Grid,
                width: 1.0,
            });
        }
        let body = f.detail.as_deref().unwrap_or("");
        let lines: Vec<String> = body
            .split('\n')
            .flat_map(|l| text::wrap(l, SIZE, w - 8.0))
            .collect();
        let h = 16.0 + lines.len().max(1) as f64 * LINE;
        let rect = Rect {
            x: 0.0,
            y: y - 4.0,
            w,
            h: h + 8.0,
        };
        if let Some(c) = &f.color {
            p.marks.push(Mark::Rect {
                r: rect,
                paint: Paint::Soft(c.clone()),
                round: 4.0,
            });
        }
        p.marks.push(Mark::Text {
            x: 4.0,
            y: y + 10.0,
            text: f.display().to_uppercase(),
            size: NAME,
            anchor: "start",
            paint: Paint::Muted,
            bold: true,
        });
        if lines.iter().all(|l| l.is_empty()) {
            p.marks.push(Mark::Text {
                x: 4.0,
                y: y + 16.0 + 13.0,
                text: "—".into(),
                size: SIZE,
                anchor: "start",
                paint: Paint::Muted,
                bold: false,
            });
        }
        for (j, l) in lines.iter().enumerate() {
            p.marks.push(Mark::Text {
                x: 4.0,
                y: y + 16.0 + j as f64 * LINE + 13.0,
                text: l.clone(),
                size: SIZE,
                anchor: "start",
                paint: Paint::Fg,
                bold: false,
            });
        }
        p.rows.push((f.id.clone(), rect));
        y += h + GAP;
    }
    p.w = w;
    p.h = y - GAP + 4.0;
    p
}

#[cfg(test)]
mod tests {
    const SRC: &str = "adr db: Use Postgres for orders [accepted] {\n  context: Orders and payments change together.\\nOne transaction.\n  options: Postgres, DynamoDB [id: opts]\n  decision: Postgres on RDS [green]\n  consequences:\n}\n";

    #[test]
    fn records_round_trip_draw_and_prompt() {
        let d = crate::dsl::parse(SRC).unwrap();
        let g = d.group("db").unwrap();
        assert_eq!(g.adr.as_deref(), Some("accepted"));
        let ctx = d.node("db.context").unwrap();
        assert_eq!(ctx.label, "context");
        assert_eq!(
            ctx.detail.as_deref(),
            Some("Orders and payments change together.\nOne transaction.")
        );
        assert_eq!(
            d.node("opts").unwrap().detail.as_deref(),
            Some("Postgres, DynamoDB")
        );
        assert_eq!(
            d.node("db.decision").unwrap().color.as_deref(),
            Some("green")
        );
        assert_eq!(d.node("db.consequences").unwrap().detail, None);
        let text = crate::dsl::print(&d);
        assert_eq!(text, SRC);
        assert_eq!(crate::dsl::parse(&text).unwrap(), d);
        // fields stack, the record keeps its width
        let lay = crate::layout::layout(&d);
        let (a, b) = (lay.rect("db.context").unwrap(), lay.rect("opts").unwrap());
        assert!(b.y > a.bottom() - 1.0 && (a.w - b.w).abs() < 1e-6);
        let svg = crate::render::render(&d, &Default::default()).svg;
        assert!(
            svg.contains(">ACCEPTED<") && svg.contains(">CONTEXT<"),
            "{svg}"
        );
        let p = crate::prompt::selection(&d, &["db".to_string()]);
        assert_eq!(p, "# Use Postgres for orders (decision record, accepted)\n\n**context:** Orders and payments change together. One transaction.\n**options:** Postgres, DynamoDB\n**decision:** Postgres on RDS\n**consequences:** —\n");
        // status through ops; a bad one is refused
        let mut d = d;
        crate::ops::apply(
            &mut d,
            &crate::ops::parse_ops(r#"{"op":"update_group","id":"db","adr":"superseded"}"#)
                .unwrap(),
        )
        .unwrap();
        assert!(crate::dsl::print(&d).starts_with("adr db: Use Postgres for orders [superseded] {"));
        assert!(crate::ops::apply(
            &mut d,
            &crate::ops::parse_ops(r#"{"op":"update_group","id":"db","adr":"maybe"}"#).unwrap()
        )
        .is_err());
        assert!(crate::dsl::parse("adr x: X [status: maybe] {}\n").is_err());
        // renaming the record renames its fields
        crate::ops::apply(
            &mut d,
            &crate::ops::parse_ops(r#"{"op":"rename","id":"db","to":"store"}"#).unwrap(),
        )
        .unwrap();
        assert!(d.node("store.context").is_some());
    }
}
