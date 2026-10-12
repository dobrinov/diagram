//! Tables: a group whose nodes are rows, under a header row.
//!
//! ```text
//! table options: Hosting options {
//!   option | cost  | latency | notes
//!   Local  | $0    | 40 ms   | needs an 8B model
//!   Hosted | $1.2k | 300 ms  [red]
//! }
//! ```
//!
//! A row's first cell is its label (and gives its id, `options.local`), the
//! rest are its `cells`. [`plot`] lays a table out like a chart (see
//! [`crate::charts::Plot`]): the marks to draw and each row's box.
use crate::charts::{Mark, Paint, Plot};
use crate::layout::Rect;
use crate::model::{Document, Group, Node};
use crate::text;

const SIZE: f64 = 13.0;
const LINE: f64 = 18.0;
const PAD_X: f64 = 10.0;
const PAD_Y: f64 = 8.0;
/// The widest a column gets before its text wraps.
const MAX_COL: f64 = 260.0;

/// The table a node is a row of.
pub fn table_of<'a>(doc: &'a Document, n: &Node) -> Option<&'a Group> {
    n.group
        .as_deref()
        .and_then(|g| doc.group(g))
        .filter(|g| g.table)
}

/// The rows of table `g`, in document order.
pub fn rows_of<'a>(doc: &'a Document, g: &str) -> Vec<&'a Node> {
    doc.nodes
        .iter()
        .filter(|n| n.group.as_deref() == Some(g))
        .collect()
}

/// A row's cells, its label first.
pub fn cells(n: &Node) -> Vec<String> {
    std::iter::once(n.display().to_string())
        .chain(n.cells.iter().cloned())
        .collect()
}

/// A row on hover: `Local — cost $0, latency 40 ms`.
pub fn tooltip(doc: &Document, n: &Node) -> String {
    let cols = table_of(doc, n)
        .map(|g| g.columns.clone())
        .unwrap_or_default();
    let rest: Vec<String> = n
        .cells
        .iter()
        .enumerate()
        .filter(|(_, c)| !c.is_empty())
        .map(|(k, c)| match cols.get(k + 1) {
            Some(name) if !name.is_empty() => format!("{name} {c}"),
            _ => c.clone(),
        })
        .collect();
    let mut s = n.display().to_string();
    if !rest.is_empty() {
        s.push_str(&format!(" — {}", rest.join(", ")));
    }
    if let Some(d) = &n.detail {
        s.push_str(&format!("\n{d}"));
    }
    s
}

/// The table drawn: header band, rows (tinted when they have a colour),
/// lines between rows and columns, wrapped text.
pub fn plot(doc: &Document, g: &Group) -> Plot {
    let rows = rows_of(doc, &g.id);
    let mut grid: Vec<Vec<String>> = vec![g.columns.clone()];
    grid.extend(rows.iter().map(|r| cells(r)));
    let ncols = grid.iter().map(|r| r.len()).max().unwrap_or(1).max(1);
    // each column as wide as its text, up to a limit (then it wraps)
    let mut widths: Vec<f64> = (0..ncols)
        .map(|k| {
            grid.iter()
                .filter_map(|r| r.get(k))
                .flat_map(|c| c.split('\n'))
                .map(|l| text::width(l, SIZE))
                .fold(40.0, f64::max)
                .min(MAX_COL)
                + 2.0 * PAD_X
        })
        .collect();
    if let Some(w) = g.w {
        let total: f64 = widths.iter().sum();
        for x in &mut widths {
            *x *= w / total;
        }
    }
    let w: f64 = widths.iter().sum();
    let mut p = Plot::default();
    let mut y = 0.0;
    for (i, row) in grid.iter().enumerate() {
        let lines: Vec<Vec<String>> = (0..ncols)
            .map(|k| {
                let c = row.get(k).map(|s| s.as_str()).unwrap_or("");
                text::wrap(c, SIZE, widths[k] - 2.0 * PAD_X)
            })
            .collect();
        let h = lines.iter().map(|l| l.len()).max().unwrap_or(1).max(1) as f64 * LINE + 2.0 * PAD_Y;
        let rect = Rect { x: 0.0, y, w, h };
        if i == 0 {
            p.marks.push(Mark::Rect {
                r: rect,
                paint: Paint::Header,
                round: 6.0,
            });
        } else if let Some(c) = &rows[i - 1].color {
            p.marks.push(Mark::Rect {
                r: rect,
                paint: Paint::Soft(c.clone()),
                round: 0.0,
            });
        }
        let mut x = 0.0;
        for (k, ls) in lines.iter().enumerate() {
            for (j, l) in ls.iter().enumerate() {
                p.marks.push(Mark::Text {
                    x: x + PAD_X,
                    y: y + PAD_Y + j as f64 * LINE + 13.0,
                    text: l.clone(),
                    size: SIZE,
                    anchor: "start",
                    paint: if i == 0 { Paint::Muted } else { Paint::Fg },
                    bold: i == 0 || k == 0,
                });
            }
            x += widths[k];
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
            p.rows.push((rows[i - 1].id.clone(), rect));
        }
        y += h;
    }
    // column lines
    let mut x = 0.0;
    for wk in &widths[..ncols - 1] {
        x += wk;
        p.marks.push(Mark::Line {
            x1: x,
            y1: 0.0,
            x2: x,
            y2: y,
            paint: Paint::Grid,
            width: 1.0,
        });
    }
    p.w = w;
    p.h = y.max(LINE + 2.0 * PAD_Y);
    p
}

#[cfg(test)]
mod tests {
    const SRC: &str = "table opts: Options {\n  option | cost | notes\n  Local  | $0   | no data leaves \\| ever\n  Hosted | $1k  | [red]\n}\n";

    #[test]
    fn tables_round_trip_draw_and_prompt() {
        let d = crate::dsl::parse(SRC).unwrap();
        let g = d.group("opts").unwrap();
        assert!(g.table);
        assert_eq!(g.columns, ["option", "cost", "notes"]);
        let local = d.node("opts.local").unwrap();
        assert_eq!(local.cells, ["$0", "no data leaves | ever"]);
        assert_eq!(d.node("opts.hosted").unwrap().color.as_deref(), Some("red"));
        assert_eq!(crate::dsl::parse(&crate::dsl::print(&d)).unwrap(), d);
        let lay = crate::layout::layout(&d);
        let (a, b) = (
            lay.rect("opts.local").unwrap(),
            lay.rect("opts.hosted").unwrap(),
        );
        assert!(b.y > a.y && (a.w - b.w).abs() < 1e-6);
        let svg = crate::render::render(&d, &Default::default()).svg;
        assert!(
            svg.contains("<title>Local — cost $0, notes no data leaves | ever</title>"),
            "{svg}"
        );
        let p = crate::prompt::selection(&d, &["opts".to_string()]);
        assert_eq!(p, "# Options (table)\n\n| option | cost | notes |\n|---|---|---|\n| Local | $0 | no data leaves \\| ever |\n| Hosted | $1k |  |\n");
    }

    #[test]
    fn ops_edit_tables_in_place() {
        let old = crate::dsl::parse(SRC).unwrap();
        let mut d = old.clone();
        let ops = crate::ops::parse_ops(r#"{"ops":[{"op":"update_node","id":"opts.hosted","cells":["$2k","data leaves"]},{"op":"add_node","id":"opts.hybrid","label":"Hybrid","group":"opts","cells":["$400","both"]}]}"#).unwrap();
        crate::ops::apply(&mut d, &ops).unwrap();
        let out = crate::patch::patch(SRC, &old, &d);
        assert!(
            out.contains("  Hosted | $2k  | data leaves [red]\n  Hybrid | $400 | both\n"),
            "{out}"
        );
        let mut e = d.clone();
        crate::ops::apply(
            &mut e,
            &crate::ops::parse_ops(r#"{"op":"rename","id":"opts","to":"hosting"}"#).unwrap(),
        )
        .unwrap();
        assert!(e.node("hosting.local").is_some());
    }
}
