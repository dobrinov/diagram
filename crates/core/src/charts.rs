//! Charts: a group whose nodes are rows of data, drawn as bars, lines,
//! areas, slices or points. In the DSL a chart is a small table:
//!
//! ```text
//! chart revenue: Revenue by quarter [bar, stacked, unit: k€] {
//!   quarter | EU  | US
//!   Q1      | 120 | 80
//!   Q2      | 150 | 95
//! }
//! chart share: Market share [pie] {
//!   Linear 40
//!   Jira 35
//! }
//! ```
//!
//! [`plot`] works out everything a chart draws, in coordinates from the
//! chart's content origin: layout takes each row's box from it (so rows can
//! be selected, commented on and linked), render draws its marks.
use crate::layout::Rect;
use crate::model::{ChartKind, Document, Group, Node};
use crate::text;

/// Colours of the series (or of a pie's slices), in order.
pub const PALETTE: [&str; 9] = [
    "blue", "orange", "green", "purple", "teal", "pink", "yellow", "red", "gray",
];

/// What a mark is painted with.
#[derive(Debug, Clone, PartialEq)]
pub enum Paint {
    /// A named colour or `#hex` (a series, a row's own colour).
    Color(String),
    Fg,
    Muted,
    Grid,
    /// The chart's background (gaps between slices).
    Bg,
    /// A light fill of a colour (a table row's tint).
    Soft(String),
    /// A table's header band.
    Header,
}

/// One thing a chart draws.
#[derive(Debug, Clone, PartialEq)]
pub enum Mark {
    Rect {
        r: Rect,
        paint: Paint,
        round: f64,
    },
    Line {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        paint: Paint,
        width: f64,
    },
    /// A polyline or polygon (`fill` closes it).
    Path {
        d: String,
        stroke: Option<Paint>,
        fill: Option<Paint>,
        width: f64,
        opacity: f64,
    },
    Circle {
        cx: f64,
        cy: f64,
        r: f64,
        paint: Paint,
    },
    Text {
        x: f64,
        y: f64,
        text: String,
        size: f64,
        anchor: &'static str,
        paint: Paint,
        bold: bool,
    },
}

/// A chart worked out: its size, its marks and each row's box.
#[derive(Debug, Clone, Default)]
pub struct Plot {
    pub w: f64,
    pub h: f64,
    pub marks: Vec<Mark>,
    /// (row's node id, its box)
    pub rows: Vec<(String, Rect)>,
}

/// Any plotted block drawn ([`Group::plotted`]): a chart, a table, a
/// database table or a decision record.
pub fn plot_any(doc: &Document, g: &Group) -> Plot {
    if g.entity {
        crate::entities::plot(doc, g)
    } else if g.table {
        crate::tables::plot(doc, g)
    } else if g.adr.is_some() {
        crate::adr::plot(doc, g)
    } else {
        plot(doc, g)
    }
}

/// The rows of chart `g`, in document order.
pub fn rows_of<'a>(doc: &'a Document, g: &str) -> Vec<&'a Node> {
    doc.nodes
        .iter()
        .filter(|n| n.group.as_deref() == Some(g))
        .collect()
}

/// The chart a node is a row of.
pub fn chart_of<'a>(doc: &'a Document, n: &Node) -> Option<&'a Group> {
    n.group
        .as_deref()
        .and_then(|g| doc.group(g))
        .filter(|g| g.chart.is_some())
}

/// How many series a chart has: its header's, or its widest row's.
pub fn series_count(doc: &Document, g: &Group) -> usize {
    rows_of(doc, &g.id)
        .iter()
        .map(|n| n.values.len())
        .max()
        .unwrap_or(0)
        .max(g.series.len())
        .max(1)
}

/// The name of series `k`.
pub fn series_name(g: &Group, k: usize) -> String {
    g.series
        .get(k)
        .cloned()
        .unwrap_or_else(|| format!("Series {}", k + 1))
}

/// `1200` → `1.2k`, `0.25` → `0.25`, with the chart's unit (`$`, `€`, `£`
/// and `¥` go in front).
pub fn format_value(v: f64, unit: Option<&str>) -> String {
    let a = v.abs();
    let num = if a >= 1e9 {
        format!("{}B", trim(v / 1e9))
    } else if a >= 1e6 {
        format!("{}M", trim(v / 1e6))
    } else if a >= 1e4 {
        format!("{}k", trim(v / 1e3))
    } else {
        trim(v)
    };
    match unit {
        Some(u) if ["$", "€", "£", "¥"].contains(&u) => {
            if let Some(n) = num.strip_prefix('-') {
                format!("-{u}{n}")
            } else {
                format!("{u}{num}")
            }
        }
        Some(u) => format!("{num}{u}"),
        None => num,
    }
}

fn trim(v: f64) -> String {
    let s = format!("{:.2}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".into()
    } else {
        s.into()
    }
}

/// Round axis bounds and a step for values from `lo` to `hi`.
pub fn ticks(lo: f64, hi: f64, want: usize) -> (f64, f64, f64) {
    let (lo, hi) = if (hi - lo).abs() < 1e-9 {
        (lo.min(0.0), hi.max(lo + 1.0))
    } else {
        (lo, hi)
    };
    let raw = (hi - lo) / want.max(1) as f64;
    let mag = 10f64.powf(raw.log10().floor());
    let step = mag
        * match raw / mag {
            n if n <= 1.0 => 1.0,
            n if n <= 2.0 => 2.0,
            n if n <= 2.5 => 2.5,
            n if n <= 5.0 => 5.0,
            _ => 10.0,
        };
    ((lo / step).floor() * step, (hi / step).ceil() * step, step)
}

/// A chart's description on hover: `Q1 — EU 120, US 80`.
pub fn tooltip(doc: &Document, n: &Node) -> String {
    let Some(g) = chart_of(doc, n) else {
        return n.display().to_string();
    };
    let unit = g.unit.as_deref();
    let kind = g.chart.unwrap_or_default();
    let vals: Vec<String> = n
        .values
        .iter()
        .enumerate()
        .map(|(k, v)| {
            let v = v.map_or("–".into(), |v| format_value(v, unit));
            match kind {
                ChartKind::Scatter => format!("{} {v}", ["x", "y"].get(k).copied().unwrap_or("·")),
                _ if series_count(doc, g) > 1 || !g.series.is_empty() => {
                    format!("{} {v}", series_name(g, k))
                }
                _ => v,
            }
        })
        .collect();
    let mut s = format!("{} — {}", n.display(), vals.join(", "));
    if let Some(d) = &n.detail {
        s.push_str(&format!("\n{d}"));
    }
    s
}

/// The colour of series `k`, or of a single-series chart's row.
fn series_paint(g: &Group, k: usize, row: Option<&Node>) -> Paint {
    if let Some(c) = row.and_then(|n| n.color.clone()) {
        return Paint::Color(c);
    }
    match (&g.color, k) {
        (Some(c), 0) => Paint::Color(c.clone()),
        _ => Paint::Color(PALETTE[k % PALETTE.len()].into()),
    }
}

/// Everything chart `g` draws (see [`Plot`]).
pub fn plot(doc: &Document, g: &Group) -> Plot {
    let kind = g.chart.unwrap_or_default();
    let rows = rows_of(doc, &g.id);
    let pie = matches!(kind, ChartKind::Pie | ChartKind::Donut);
    let (w, h) = (
        g.w.unwrap_or(if pie { 380.0 } else { 480.0 }).max(160.0),
        g.h.unwrap_or(if pie { 260.0 } else { 280.0 }).max(120.0),
    );
    let mut p = Plot {
        w,
        h,
        ..Default::default()
    };
    if rows.is_empty() {
        p.marks.push(Mark::Text {
            x: w / 2.0,
            y: h / 2.0,
            text: "No data yet".into(),
            size: 13.0,
            anchor: "middle",
            paint: Paint::Muted,
            bold: false,
        });
        return p;
    }
    match kind {
        ChartKind::Pie | ChartKind::Donut => pie_chart(&mut p, g, &rows, kind == ChartKind::Donut),
        ChartKind::Scatter => scatter(&mut p, g, &rows),
        _ => axes_chart(&mut p, doc, g, &rows, kind),
    }
    p
}

/// The legend: a swatch and a name for each series, in a row along the top.
fn legend(p: &mut Plot, g: &Group, n: usize) -> f64 {
    if n < 2 && g.series.len() < 2 {
        return 0.0;
    }
    let mut x = 0.0;
    for k in 0..n {
        let name = series_name(g, k);
        p.marks.push(Mark::Rect {
            r: Rect {
                x,
                y: 2.0,
                w: 10.0,
                h: 10.0,
            },
            paint: series_paint(g, k, None),
            round: 2.0,
        });
        p.marks.push(Mark::Text {
            x: x + 14.0,
            y: 11.0,
            text: name.clone(),
            size: 11.0,
            anchor: "start",
            paint: Paint::Muted,
            bold: false,
        });
        x += 14.0 + text::width(&name, 11.0) + 16.0;
    }
    22.0
}

/// Bars (side by side or stacked, up or across), lines and areas: categories
/// along one axis, values along the other.
fn axes_chart(p: &mut Plot, doc: &Document, g: &Group, rows: &[&Node], kind: ChartKind) {
    let n = series_count(doc, g);
    let unit = g.unit.as_deref();
    let stacked = g.stacked && n > 1 && kind != ChartKind::Line;
    let across = g.horizontal && kind == ChartKind::Bar;
    let top = legend(p, g, n) + 6.0;
    // the value range: stacks add up, and bars start at zero
    let (mut lo, mut hi) = (0.0f64, 0.0f64);
    for r in rows {
        if stacked {
            let pos: f64 = r.values.iter().flatten().filter(|v| **v > 0.0).sum();
            let neg: f64 = r.values.iter().flatten().filter(|v| **v < 0.0).sum();
            hi = hi.max(pos);
            lo = lo.min(neg);
        } else {
            for v in r.values.iter().flatten() {
                hi = hi.max(*v);
                lo = lo.min(*v);
            }
        }
    }
    if kind == ChartKind::Line {
        // a line does not need zero: fit the values
        let vals: Vec<f64> = rows
            .iter()
            .flat_map(|r| r.values.iter().flatten().copied())
            .collect();
        if let (Some(a), Some(b)) = (
            vals.iter().copied().reduce(f64::min),
            vals.iter().copied().reduce(f64::max),
        ) {
            if a > 0.0 && (b - a) < b * 0.6 {
                lo = a;
            }
            hi = b;
        }
    }
    let (v0, v1, step) = ticks(lo, hi, 5);
    let labels: Vec<String> = rows
        .iter()
        .map(|r| r.display().replace('\n', " "))
        .collect();
    let tick_w = {
        let mut t = v0;
        let mut w: f64 = 0.0;
        while t <= v1 + step / 2.0 {
            w = w.max(text::width(&format_value(t, unit), 11.0));
            t += step;
        }
        w
    };
    // the plot area
    let (left, bottom) = if across {
        let lw = labels
            .iter()
            .map(|l| text::width(l, 12.0))
            .fold(30.0, f64::max)
            .min(160.0);
        (lw + 10.0, 22.0)
    } else {
        (tick_w + 10.0, 26.0)
    };
    let (px, py, pw, ph) = (left, top, p.w - left - 8.0, p.h - top - bottom);
    // value → position along the value axis
    let along = |v: f64| {
        let f = (v - v0) / (v1 - v0);
        if across {
            px + f * pw
        } else {
            py + ph - f * ph
        }
    };
    // gridlines and value labels
    let mut t = v0;
    while t <= v1 + step / 2.0 {
        let at = along(t);
        let zero = t.abs() < step / 1000.0;
        let paint = if zero { Paint::Muted } else { Paint::Grid };
        if across {
            p.marks.push(Mark::Line {
                x1: at,
                y1: py,
                x2: at,
                y2: py + ph,
                paint,
                width: 1.0,
            });
            p.marks.push(Mark::Text {
                x: at,
                y: py + ph + 15.0,
                text: format_value(t, unit),
                size: 11.0,
                anchor: "middle",
                paint: Paint::Muted,
                bold: false,
            });
        } else {
            p.marks.push(Mark::Line {
                x1: px,
                y1: at,
                x2: px + pw,
                y2: at,
                paint,
                width: 1.0,
            });
            p.marks.push(Mark::Text {
                x: px - 6.0,
                y: at + 4.0,
                text: format_value(t, unit),
                size: 11.0,
                anchor: "end",
                paint: Paint::Muted,
                bold: false,
            });
        }
        t += step;
    }
    let count = rows.len() as f64;
    let band = if across { ph / count } else { pw / count };
    // category labels (left out where they would not fit)
    for (i, l) in labels.iter().enumerate() {
        let c = (i as f64 + 0.5) * band;
        if across {
            p.marks.push(Mark::Text {
                x: px - 6.0,
                y: py + c + 4.0,
                text: fit(l, left - 10.0, 12.0),
                size: 12.0,
                anchor: "end",
                paint: Paint::Fg,
                bold: false,
            });
        } else {
            let every = ((text::width(l, 12.0) + 8.0) / band).ceil().max(1.0) as usize;
            if i % every == 0 {
                p.marks.push(Mark::Text {
                    x: px + c,
                    y: py + ph + 17.0,
                    text: fit(l, band * every as f64 - 4.0, 12.0),
                    size: 12.0,
                    anchor: "middle",
                    paint: Paint::Fg,
                    bold: false,
                });
            }
        }
    }
    let zero = along(0f64.clamp(v0, v1));
    match kind {
        ChartKind::Bar => {
            let inner = band * 0.72;
            for (i, r) in rows.iter().enumerate() {
                let start = i as f64 * band + (band - inner) / 2.0;
                let mut boxes: Vec<Rect> = Vec::new();
                let (mut up, mut down) = (0.0f64, 0.0f64);
                for k in 0..n {
                    let Some(v) = r.values.get(k).copied().flatten() else {
                        continue;
                    };
                    let (from, to, off, size) = if stacked {
                        let base = if v >= 0.0 { up } else { down };
                        if v >= 0.0 {
                            up += v;
                        } else {
                            down += v;
                        }
                        (base, base + v, 0.0, inner)
                    } else {
                        let size = inner / n as f64;
                        (0.0, v, k as f64 * size, size)
                    };
                    let (a, b) = (along(from), along(to));
                    let rect = if across {
                        Rect {
                            x: a.min(b),
                            y: py + start + off,
                            w: (a - b).abs().max(1.0),
                            h: size - 1.0,
                        }
                    } else {
                        Rect {
                            x: px + start + off,
                            y: a.min(b),
                            w: size - 1.0,
                            h: (a - b).abs().max(1.0),
                        }
                    };
                    let row = (n == 1).then_some(*r);
                    p.marks.push(Mark::Rect {
                        r: rect,
                        paint: series_paint(g, k, row),
                        round: 2.0,
                    });
                    boxes.push(rect);
                }
                let hit = boxes
                    .into_iter()
                    .reduce(|a, b| a.union(&b))
                    .unwrap_or(if across {
                        Rect {
                            x: zero - 2.0,
                            y: py + start,
                            w: 4.0,
                            h: inner,
                        }
                    } else {
                        Rect {
                            x: px + start,
                            y: zero - 2.0,
                            w: inner,
                            h: 4.0,
                        }
                    });
                p.rows.push((r.id.clone(), hit));
            }
        }
        ChartKind::Line | ChartKind::Area => {
            let x = |i: usize| px + (i as f64 + 0.5) * band;
            let mut base = vec![0.0f64; rows.len()];
            let mut tops: Vec<(f64, f64)> = vec![(f64::MAX, f64::MIN); rows.len()];
            for k in 0..n {
                let paint = series_paint(g, k, None);
                let mut pts: Vec<(usize, f64)> = Vec::new();
                for (i, r) in rows.iter().enumerate() {
                    if let Some(v) = r.values.get(k).copied().flatten() {
                        let v = if stacked { base[i] + v } else { v };
                        pts.push((i, v));
                    }
                }
                if kind == ChartKind::Area && pts.len() > 1 {
                    let mut d = String::new();
                    for (j, (i, v)) in pts.iter().enumerate() {
                        d.push_str(&format!(
                            "{}{:.1},{:.1} ",
                            if j == 0 { "M" } else { "L" },
                            x(*i),
                            along(*v)
                        ));
                    }
                    for (i, _) in pts.iter().rev() {
                        let b = if stacked {
                            base[*i]
                        } else {
                            0f64.clamp(v0, v1)
                        };
                        d.push_str(&format!("L{:.1},{:.1} ", x(*i), along(b)));
                    }
                    d.push('Z');
                    p.marks.push(Mark::Path {
                        d,
                        stroke: None,
                        fill: Some(paint.clone()),
                        width: 0.0,
                        opacity: 0.35,
                    });
                }
                if pts.len() > 1 {
                    let d: String = pts
                        .iter()
                        .enumerate()
                        .map(|(j, (i, v))| {
                            format!(
                                "{}{:.1},{:.1}",
                                if j == 0 { "M" } else { " L" },
                                x(*i),
                                along(*v)
                            )
                        })
                        .collect();
                    p.marks.push(Mark::Path {
                        d,
                        stroke: Some(paint.clone()),
                        fill: None,
                        width: 2.0,
                        opacity: 1.0,
                    });
                }
                for (i, v) in &pts {
                    let y = along(*v);
                    p.marks.push(Mark::Circle {
                        cx: x(*i),
                        cy: y,
                        r: 3.5,
                        paint: series_paint(g, k, (n == 1).then_some(rows[*i])),
                    });
                    tops[*i] = (tops[*i].0.min(y), tops[*i].1.max(y));
                }
                if stacked {
                    for (i, r) in rows.iter().enumerate() {
                        base[i] += r.values.get(k).copied().flatten().unwrap_or(0.0);
                    }
                }
            }
            for (i, r) in rows.iter().enumerate() {
                let (a, b) = tops[i];
                let (a, b) = if a > b { (zero, zero) } else { (a, b) };
                let half = (band / 2.0).min(10.0);
                p.rows.push((
                    r.id.clone(),
                    Rect {
                        x: x(i) - half,
                        y: a - 6.0,
                        w: 2.0 * half,
                        h: b - a + 12.0,
                    },
                ));
            }
        }
        _ => {}
    }
}

/// `Long label` cut to fit `w`.
fn fit(s: &str, w: f64, size: f64) -> String {
    if text::width(s, size) <= w {
        return s.to_string();
    }
    let mut out: String = String::new();
    for ch in s.chars() {
        if text::width(&format!("{out}{ch}…"), size) > w {
            break;
        }
        out.push(ch);
    }
    format!("{out}…")
}

/// Slices from the top, clockwise, with their share outside.
fn pie_chart(p: &mut Plot, g: &Group, rows: &[&Node], donut: bool) {
    let vals: Vec<f64> = rows
        .iter()
        .map(|r| r.values.first().copied().flatten().unwrap_or(0.0).max(0.0))
        .collect();
    let total: f64 = vals.iter().sum();
    let (cx, cy) = (p.w / 2.0, p.h / 2.0);
    let r = (p.w.min(p.h) / 2.0 - 34.0).max(30.0);
    let inner = if donut { r * 0.58 } else { 0.0 };
    let mut a = -std::f64::consts::FRAC_PI_2;
    for (i, row) in rows.iter().enumerate() {
        let share = if total > 0.0 { vals[i] / total } else { 0.0 };
        let sweep = share * std::f64::consts::TAU;
        let paint = row
            .color
            .clone()
            .map(Paint::Color)
            .unwrap_or_else(|| Paint::Color(PALETTE[i % PALETTE.len()].into()));
        let pt = |ang: f64, rad: f64| (cx + rad * ang.cos(), cy + rad * ang.sin());
        let mid = a + sweep / 2.0;
        if sweep > 0.0 {
            let d = if share >= 0.9999 {
                // a full circle (as two halves)
                let (x0, y0) = pt(a, r);
                let (x1, y1) = pt(a + std::f64::consts::PI, r);
                let mut d = format!("M{x0:.1},{y0:.1} A{r:.1},{r:.1} 0 1 1 {x1:.1},{y1:.1} A{r:.1},{r:.1} 0 1 1 {x0:.1},{y0:.1} Z");
                if inner > 0.0 {
                    let (i0, j0) = pt(a, inner);
                    let (i1, j1) = pt(a + std::f64::consts::PI, inner);
                    d.push_str(&format!(" M{i0:.1},{j0:.1} A{inner:.1},{inner:.1} 0 1 0 {i1:.1},{j1:.1} A{inner:.1},{inner:.1} 0 1 0 {i0:.1},{j0:.1} Z"));
                }
                d
            } else {
                let large = i32::from(sweep > std::f64::consts::PI);
                let (x0, y0) = pt(a, r);
                let (x1, y1) = pt(a + sweep, r);
                if inner > 0.0 {
                    let (i1, j1) = pt(a + sweep, inner);
                    let (i0, j0) = pt(a, inner);
                    format!("M{x0:.1},{y0:.1} A{r:.1},{r:.1} 0 {large} 1 {x1:.1},{y1:.1} L{i1:.1},{j1:.1} A{inner:.1},{inner:.1} 0 {large} 0 {i0:.1},{j0:.1} Z")
                } else {
                    format!("M{cx:.1},{cy:.1} L{x0:.1},{y0:.1} A{r:.1},{r:.1} 0 {large} 1 {x1:.1},{y1:.1} Z")
                }
            };
            p.marks.push(Mark::Path {
                d,
                stroke: Some(Paint::Bg),
                fill: Some(paint),
                width: 2.0,
                opacity: 1.0,
            });
        }
        // the label outside the slice
        let (lx, ly) = pt(mid, r + 12.0);
        let right = mid.cos() >= 0.0;
        let text = format!(
            "{} {}%",
            row.display().replace('\n', " "),
            (share * 100.0).round()
        );
        p.marks.push(Mark::Text {
            x: lx,
            y: ly + 4.0,
            text: text.clone(),
            size: 12.0,
            anchor: if right { "start" } else { "end" },
            paint: Paint::Fg,
            bold: false,
        });
        let tw = text::width(&text, 12.0);
        let (mx, my) = pt(mid, (r + inner) / 2.0);
        let hit = Rect {
            x: mx - 10.0,
            y: my - 10.0,
            w: 20.0,
            h: 20.0,
        }
        .union(&Rect {
            x: if right { lx } else { lx - tw },
            y: ly - 8.0,
            w: tw,
            h: 16.0,
        });
        p.rows.push((row.id.clone(), hit));
        a += sweep;
    }
    if donut {
        p.marks.push(Mark::Text {
            x: cx,
            y: cy + 6.0,
            text: format_value(total, g.unit.as_deref()),
            size: 18.0,
            anchor: "middle",
            paint: Paint::Fg,
            bold: true,
        });
    }
}

/// Points at (first value, second value), each labelled.
fn scatter(p: &mut Plot, g: &Group, rows: &[&Node]) {
    let pts: Vec<(usize, f64, f64)> = rows
        .iter()
        .enumerate()
        .filter_map(|(i, r)| {
            Some((
                i,
                r.values.first().copied().flatten()?,
                r.values.get(1).copied().flatten()?,
            ))
        })
        .collect();
    let xs = pts.iter().map(|p| p.1);
    let ys = pts.iter().map(|p| p.2);
    // headroom: labels sit to the right of their points
    let (x0, x1, xs_) = ticks(
        xs.clone().fold(f64::MAX, f64::min).min(0.0),
        xs.fold(f64::MIN, f64::max).max(1.0) * 1.2,
        5,
    );
    let (y0, y1, ys_) = ticks(
        ys.clone().fold(f64::MAX, f64::min).min(0.0),
        ys.fold(f64::MIN, f64::max).max(1.0) * 1.05,
        5,
    );
    let (left, bottom, top) = (40.0, 34.0, 8.0);
    let (px, py, pw, ph) = (left, top, p.w - left - 12.0, p.h - top - bottom);
    let sx = |v: f64| px + (v - x0) / (x1 - x0) * pw;
    let sy = |v: f64| py + ph - (v - y0) / (y1 - y0) * ph;
    let mut t = x0;
    while t <= x1 + xs_ / 2.0 {
        p.marks.push(Mark::Line {
            x1: sx(t),
            y1: py,
            x2: sx(t),
            y2: py + ph,
            paint: Paint::Grid,
            width: 1.0,
        });
        p.marks.push(Mark::Text {
            x: sx(t),
            y: py + ph + 15.0,
            text: trim(t),
            size: 11.0,
            anchor: "middle",
            paint: Paint::Muted,
            bold: false,
        });
        t += xs_;
    }
    let mut t = y0;
    while t <= y1 + ys_ / 2.0 {
        p.marks.push(Mark::Line {
            x1: px,
            y1: sy(t),
            x2: px + pw,
            y2: sy(t),
            paint: Paint::Grid,
            width: 1.0,
        });
        p.marks.push(Mark::Text {
            x: px - 6.0,
            y: sy(t) + 4.0,
            text: trim(t),
            size: 11.0,
            anchor: "end",
            paint: Paint::Muted,
            bold: false,
        });
        t += ys_;
    }
    // axis names from the header
    if let Some(n) = g.series.first() {
        p.marks.push(Mark::Text {
            x: px + pw,
            y: py + ph + 30.0,
            text: format!("{n} →"),
            size: 11.0,
            anchor: "end",
            paint: Paint::Muted,
            bold: false,
        });
    }
    if let Some(n) = g.series.get(1) {
        p.marks.push(Mark::Text {
            x: px + 4.0,
            y: py + 10.0,
            text: format!("↑ {n}"),
            size: 11.0,
            anchor: "start",
            paint: Paint::Muted,
            bold: false,
        });
    }
    for (i, x, y) in pts {
        let r = rows[i];
        let (cx, cy) = (sx(x), sy(y));
        p.marks.push(Mark::Circle {
            cx,
            cy,
            r: 5.0,
            paint: series_paint(g, 0, Some(r)),
        });
        let label = r.display().replace('\n', " ");
        p.marks.push(Mark::Text {
            x: cx + 8.0,
            y: cy + 4.0,
            text: label.clone(),
            size: 11.0,
            anchor: "start",
            paint: Paint::Fg,
            bold: false,
        });
        p.rows.push((
            r.id.clone(),
            Rect {
                x: cx - 7.0,
                y: cy - 7.0,
                w: 14.0 + 8.0 + text::width(&label, 11.0),
                h: 14.0,
            },
        ));
    }
    for r in rows {
        if !p.rows.iter().any(|(id, _)| *id == r.id) {
            p.rows.push((
                r.id.clone(),
                Rect {
                    x: px,
                    y: py + ph,
                    w: 1.0,
                    h: 1.0,
                },
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_and_ticks_read_well() {
        assert_eq!(format_value(1200.0, None), "1200");
        assert_eq!(format_value(12_500.0, Some("€")), "€12.5k");
        assert_eq!(format_value(-3.0, Some("$")), "-$3");
        assert_eq!(format_value(40.0, Some("%")), "40%");
        assert_eq!(ticks(0.0, 170.0, 5), (0.0, 200.0, 50.0));
        assert_eq!(ticks(-20.0, 95.0, 5), (-25.0, 100.0, 25.0));
    }

    const SRC: &str = "chart rev: Revenue [bar, stacked, unit: k€] {\n  quarter | EU | US\n  Q1 | 120 | 80\n  Q2 | 150 | -\n}\nchart share: Share [pie] {\n  Linear | 40\n  Jira | 60 [red]\n}\n";

    #[test]
    fn charts_round_trip_with_rows_named_in_their_chart() {
        let d = crate::dsl::parse(SRC).unwrap();
        assert_eq!(crate::dsl::print(&d), SRC);
        let g = d.group("rev").unwrap();
        assert_eq!(
            (g.chart, g.stacked, g.series.clone(), g.axis.as_deref()),
            (
                Some(ChartKind::Bar),
                true,
                vec!["EU".to_string(), "US".to_string()],
                Some("quarter")
            )
        );
        assert_eq!(d.node("rev.q2").unwrap().values, vec![Some(150.0), None]);
        // `Linear 40` is the same as `Linear | 40`
        let e = crate::dsl::parse("chart s: S [pie] {\n  Linear 40\n  GitHub Projects 15\n}\n")
            .unwrap();
        assert_eq!(
            e.node("s.github-projects").unwrap().values,
            vec![Some(15.0)]
        );
        assert_eq!(
            crate::dsl::parse("chart s: S {\n  a | x\n  b | 1\n}")
                .unwrap()
                .group("s")
                .unwrap()
                .series,
            ["x"]
        );
    }

    #[test]
    fn charts_draw_rows_you_can_point_at() {
        let d = crate::dsl::parse(SRC).unwrap();
        let lay = crate::layout::layout(&d);
        let (q1, q2) = (lay.rect("rev.q1").unwrap(), lay.rect("rev.q2").unwrap());
        assert!(lay.node("rev.q1").unwrap().datum);
        // Q1 (200 stacked) is taller than Q2 (150), and to its left
        assert!(q1.h > q2.h && q1.x < q2.x, "{q1:?} {q2:?}");
        let svg = crate::render::render(&d, &Default::default()).svg;
        assert!(
            svg.contains("dg-chart") && svg.contains("<title>Q1 — EU 120k€, US 80k€</title>"),
            "{svg}"
        );
        assert!(svg.contains(">Jira 60%</text>"));
        let p = crate::prompt::selection(&d, &["rev".to_string(), "share".to_string()]);
        assert!(p.contains("## Revenue (bar chart, stacked, values in k€)\n\n| quarter | EU | US |\n|---|---|---|\n| Q1 | 120 | 80 |\n| Q2 | 150 | - |\n"), "{p}");
        assert!(p.contains("| Jira | 60 | 60% |"), "{p}");
    }

    #[test]
    fn ops_change_charts_and_keep_the_file() {
        let old = crate::dsl::parse(SRC).unwrap();
        let mut d = old.clone();
        let ops = crate::ops::parse_ops(r#"{"ops":[{"op":"update_node","id":"rev.q2","values":[150,95]},{"op":"add_node","id":"rev.q3","label":"Q3","group":"rev","values":[170,130]},{"op":"update_group","id":"share","chart":"donut"}]}"#).unwrap();
        crate::ops::apply(&mut d, &ops).unwrap();
        let out = crate::patch::patch(SRC, &old, &d);
        assert_eq!(
            out,
            SRC.replace("  Q2 | 150 | -\n", "  Q2 | 150 | 95\n  Q3 | 170 | 130\n")
                .replace("[pie]", "[donut]")
        );
    }
}
