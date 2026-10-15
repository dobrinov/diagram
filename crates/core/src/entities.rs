//! Database tables (`entity`): a group whose nodes are columns.
//!
//! ```text
//! entity users: Users {
//!   id:      uuid [pk]
//!   email:   text [unique]
//!   team_id: uuid [fk: teams.id]
//!   bio:     text [null]
//! }
//! ```
//!
//! A column's id is its table's and its name (`users.email`). A foreign key
//! (`fk: teams.id`, or `fk: teams` for its primary key) is drawn as a line
//! from the column to the one it references, and takes part in the layout
//! like an arrow. [`plot`] lays a table out like a chart (see
//! [`crate::charts::Plot`]).
use crate::charts::{Mark, Paint, Plot};
use crate::layout::Rect;
use crate::model::{Arrow, Document, Edge, Group, Line, Node};
use crate::text;

const SIZE: f64 = 13.0;
const ROW: f64 = 26.0;
const PAD_X: f64 = 10.0;
const BADGE: f64 = 24.0;

/// The table a node is a column of.
pub fn entity_of<'a>(doc: &'a Document, n: &Node) -> Option<&'a Group> {
    n.group
        .as_deref()
        .and_then(|g| doc.group(g))
        .filter(|g| g.entity)
}

/// The columns of table `g`, in document order.
pub fn columns_of<'a>(doc: &'a Document, g: &str) -> Vec<&'a Node> {
    doc.nodes
        .iter()
        .filter(|n| n.group.as_deref() == Some(g))
        .collect()
}

/// The column a foreign key points at: `teams.id` as is, `teams` as its
/// primary key.
pub fn target(doc: &Document, fk: &str) -> Option<String> {
    if doc.node(fk).is_some() {
        return Some(fk.to_string());
    }
    let g = doc.group(fk).filter(|g| g.entity)?;
    columns_of(doc, &g.id)
        .into_iter()
        .find(|c| c.column.as_ref().is_some_and(|c| c.pk))
        .map(|c| c.id.clone())
}

/// How many of one side go with the other, in crow's-foot notation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cardinality {
    /// Exactly one (`||`).
    One,
    /// Zero or one (`o|`).
    ZeroOne,
    /// Zero or more (`o<`).
    Many,
}

/// The two ends of foreign key `col`: how many rows it references (one, or
/// zero or one when it may be NULL), and how many rows reference each of
/// those (zero or one when it is unique, otherwise many).
pub fn cardinality(col: &crate::model::Column) -> (Cardinality, Cardinality) {
    let parent = if col.null {
        Cardinality::ZeroOne
    } else {
        Cardinality::One
    };
    let child = if col.unique || col.pk {
        Cardinality::ZeroOne
    } else {
        Cardinality::Many
    };
    (child, parent)
}

/// The id of the line a foreign key is drawn as.
pub fn relation_id(column: &str) -> String {
    format!("fk:{column}")
}

/// The board with its foreign keys as arrows (for layout and drawing), or
/// None when it has none.
pub fn with_relations(doc: &Document) -> Option<Document> {
    let rels: Vec<Edge> = doc
        .nodes
        .iter()
        .filter_map(|n| {
            let to = target(doc, n.column.as_ref()?.fk.as_deref()?)?;
            Some(Edge {
                id: relation_id(&n.id),
                from: n.id.clone(),
                to,
                label: n.column.as_ref()?.rel.clone(),
                arrow: Arrow::End,
                line: Line::Solid,
                ..Default::default()
            })
        })
        .collect();
    if rels.is_empty() {
        return None;
    }
    let mut d = doc.clone();
    d.edges.extend(rels);
    Some(d)
}

/// A column on hover: `email text · unique · references teams.id`.
pub fn tooltip(n: &Node) -> String {
    let c = n.column.clone().unwrap_or_default();
    let mut bits = vec![format!("{} {}", n.display(), c.ty).trim().to_string()];
    if c.pk {
        bits.push("primary key".into());
    }
    if c.unique {
        bits.push("unique".into());
    }
    bits.push(if c.null {
        "null".into()
    } else {
        "not null".into()
    });
    if let Some(f) = &c.fk {
        bits.push(format!("references {f}"));
    }
    if let Some(d) = &c.default {
        bits.push(format!("default {d}"));
    }
    let mut s = bits.join(" · ");
    if let Some(d) = &n.detail {
        s.push_str(&format!("\n{d}"));
    }
    s
}

/// The table drawn: one row per column — a key badge, the name (bold for
/// the primary key), the type and `?` for a nullable one.
pub fn plot(doc: &Document, g: &Group) -> Plot {
    let cols = columns_of(doc, &g.id);
    let name_w = cols
        .iter()
        .map(|c| text::width(c.display(), SIZE))
        .fold(60.0, f64::max);
    let type_w = cols
        .iter()
        .map(|c| text::width(&c.column.clone().unwrap_or_default().ty, 12.0) + 10.0)
        .fold(40.0, f64::max);
    let w = BADGE + name_w + 16.0 + type_w + PAD_X;
    let mut p = Plot::default();
    for (i, c) in cols.iter().enumerate() {
        let y = i as f64 * ROW;
        let col = c.column.clone().unwrap_or_default();
        let rect = Rect {
            x: 0.0,
            y,
            w,
            h: ROW,
        };
        if let Some(t) = &c.color {
            p.marks.push(Mark::Rect {
                r: rect,
                paint: Paint::Soft(t.clone()),
                round: 0.0,
            });
        }
        if i > 0 {
            p.marks.push(Mark::Line {
                x1: 0.0,
                y1: y,
                x2: w,
                y2: y,
                paint: Paint::Grid,
                width: 1.0,
            });
        }
        let badge = match (col.pk, col.fk.is_some()) {
            (true, _) => Some(("PK", "yellow")),
            (false, true) => Some(("FK", "blue")),
            _ if col.unique => Some(("UQ", "purple")),
            _ => None,
        };
        if let Some((b, colour)) = badge {
            p.marks.push(Mark::Text {
                x: 4.0,
                y: y + ROW / 2.0 + 4.0,
                text: b.into(),
                size: 9.5,
                anchor: "start",
                paint: Paint::Color(colour.into()),
                bold: true,
            });
        }
        p.marks.push(Mark::Text {
            x: BADGE,
            y: y + ROW / 2.0 + 4.5,
            text: c.display().to_string(),
            size: SIZE,
            anchor: "start",
            paint: Paint::Fg,
            bold: col.pk,
        });
        let ty = if col.null {
            format!("{}?", col.ty)
        } else {
            col.ty.clone()
        };
        p.marks.push(Mark::Text {
            x: w - PAD_X,
            y: y + ROW / 2.0 + 4.5,
            text: ty,
            size: 12.0,
            anchor: "end",
            paint: Paint::Muted,
            bold: false,
        });
        p.rows.push((c.id.clone(), rect));
    }
    p.w = w;
    p.h = (cols.len() as f64 * ROW).max(ROW);
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "entity teams: Teams {\n  id:   uuid [pk]\n  name: text [unique]\n}\nentity users: Users {\n  id:      uuid [pk]\n  team_id: uuid [fk: teams]\n  bio:     text [null, detail: Shown on the profile]\n  tags:    text[]\n}\n";

    #[test]
    fn er_notation_from_the_keys() {
        let src = "entity users: Users {\n  id: uuid [pk]\n}\nentity orders: Orders {\n  buyer_id: uuid [null, fk: users, rel: placed by]\n}\nentity profiles: Profiles {\n  user_id [pk, fk: users]\n}\n";
        let d = crate::dsl::parse(src).unwrap();
        assert_eq!(crate::dsl::print(&d), src);
        let col = |id: &str| d.node(id).unwrap().column.clone().unwrap();
        use Cardinality::*;
        // many orders per user, each with zero or one buyer
        assert_eq!(cardinality(&col("orders.buyer_id")), (Many, ZeroOne));
        // one-to-one: a profile per user at most, each for exactly one user
        assert_eq!(cardinality(&col("profiles.user_id")), (ZeroOne, One));
        let out = crate::render::render(&d, &Default::default());
        assert!(
            out.svg.contains("dg-card") && out.svg.contains(">placed by<"),
            "{}",
            out.svg
        );
    }

    #[test]
    fn tables_round_trip_and_keys_become_lines() {
        let d = crate::dsl::parse(SRC).unwrap();
        assert_eq!(crate::dsl::print(&d), SRC);
        let c = d.node("users.team_id").unwrap().column.clone().unwrap();
        assert_eq!((c.ty.as_str(), c.fk.as_deref()), ("uuid", Some("teams")));
        assert_eq!(
            d.node("users.tags").unwrap().column.clone().unwrap().ty,
            "text[]"
        );
        assert!(d.node("users.bio").unwrap().column.clone().unwrap().null);
        assert_eq!(target(&d, "teams").as_deref(), Some("teams.id"));
        let r = with_relations(&d).unwrap();
        assert!(r
            .edges
            .iter()
            .any(|e| e.id == "fk:users.team_id" && e.to == "teams.id"));
        let out = crate::render::render(&d, &Default::default());
        assert!(out.svg.contains("dg-relation"));
        assert!(
            out.svg
                .contains("<title>team_id uuid · not null · references teams</title>"),
            "{}",
            out.svg
        );
        // the relation is not an arrow you can pick
        assert!(!out.svg.contains("data-id=\"fk:users.team_id\""));
    }

    #[test]
    fn schema_changes_show_column_by_column() {
        let old = crate::dsl::parse(SRC).unwrap();
        let new = crate::dsl::parse(
            &SRC.replace("  bio:     text [null, detail: Shown on the profile]\n", "")
                .replace(
                    "tags:    text[]",
                    "tags:    jsonb\n  deleted_at: timestamptz [null]",
                ),
        )
        .unwrap();
        let md = crate::diff::markdown(&crate::diff::diff(&old, &new), &old, &new);
        assert!(
            md.contains("− node `users.bio`") && md.contains("+ node `users.deleted_at`"),
            "{md}"
        );
        assert!(
            md.contains("~ node `users.tags` (\"tags\") (type: `text[]` → `jsonb`)"),
            "{md}"
        );
        // the review picture keeps the dropped column, in red
        let r = crate::render_diff(&old, &new, &Default::default());
        assert!(r.svg.contains("data-id=\"users.bio\""));
        let p = crate::prompt::selection(&new, &["users".to_string()]);
        assert!(
            p.contains("| column | type | key | null | references | default |")
                && p.contains("| team_id | uuid |  | no | teams |  |"),
            "{p}"
        );
    }
}
