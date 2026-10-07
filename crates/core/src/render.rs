//! SVG output. Colours are literal (no CSS), so the same file renders in
//! browsers, resvg (PNG export), image viewers and Markdown previews.
//! Elements carry `data-id` / `data-comment` so editors can hit-test them.
use std::collections::HashMap;
use std::fmt::Write;

use serde::{Deserialize, Serialize};

use crate::layout::{self, Layout, Rect, DETAIL_SIZE, LABEL_SIZE, LINE_H};
use crate::model::*;
use crate::route::{self, EdgePath};
use crate::text::{self, esc, n};
use crate::theme::{self, Palette};

/// How an element changed, when rendering a diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Change {
    Added,
    Removed,
    Changed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CommentMode {
    /// Pins for open comments.
    #[default]
    Open,
    /// Open and resolved (resolved ones greyed out).
    All,
    None,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RenderOptions {
    /// Overrides the document's theme.
    pub theme: Option<Theme>,
    pub comments: CommentMode,
    /// Draw the title above the diagram.
    pub title: Option<bool>,
    /// Transparent instead of the theme's background.
    pub transparent: bool,
    pub padding: Option<f64>,
    /// Diff status per element id (see `diff::merge`).
    #[serde(skip)]
    pub changes: HashMap<String, Change>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Pin {
    pub id: String,
    pub number: usize,
    pub x: f64,
    pub y: f64,
    pub resolved: bool,
}

/// The SVG plus its geometry, for interactive editors.
#[derive(Debug, Clone, Serialize)]
pub struct Rendered {
    pub svg: String,
    /// viewBox of the SVG, in diagram coordinates.
    pub view: Rect,
    pub nodes: Vec<layout::NodeBox>,
    pub groups: Vec<layout::GroupBox>,
    pub edges: Vec<EdgePath>,
    pub pins: Vec<Pin>,
}

pub fn render(doc: &Document, opts: &RenderOptions) -> Rendered {
    let lay = layout::layout(doc);
    let edges = route::route(doc, &lay);
    let t = opts.theme.unwrap_or(doc.settings.theme);
    let p = theme::palette(t);
    let pins = pins(doc, &lay, &edges, opts.comments);

    // edge labels and pins may stick out of the boxes
    let mut bounds = lay.bounds;
    let mut labels: Vec<(String, Rect, Vec<String>)> = Vec::new();
    for (e, path) in doc.edges.iter().zip(&edges) {
        if let Some(l) = e.label.as_deref().filter(|l| !l.is_empty()) {
            let lines = text::wrap(l, 12.0, 160.0);
            let w = lines
                .iter()
                .map(|s| text::width(s, 12.0))
                .fold(0.0, f64::max)
                + 10.0;
            let h = lines.len() as f64 * 12.0 * LINE_H + 4.0;
            let r = Rect {
                x: path.mid.0 - w / 2.0,
                y: path.mid.1 - h / 2.0,
                w,
                h,
            };
            bounds = bounds.union(&r);
            labels.push((e.id.clone(), r, lines));
        }
    }
    for e in &edges {
        bounds = bounds.union(
            &Rect {
                x: e.mid.0,
                y: e.mid.1,
                w: 1.0,
                h: 1.0,
            }
            .grow(16.0),
        );
    }
    for pin in &pins {
        bounds = bounds.union(&Rect {
            x: pin.x - 12.0,
            y: pin.y - 12.0,
            w: 24.0,
            h: 24.0,
        });
    }
    let show_title = opts.title.unwrap_or(true) && doc.title.is_some();
    let pad = opts.padding.unwrap_or(32.0);
    let mut view = bounds.grow(pad);
    if doc.nodes.is_empty() && doc.groups.is_empty() {
        view = Rect {
            x: 0.0,
            y: 0.0,
            w: 320.0,
            h: 160.0,
        };
    }
    let title_h = if show_title { 44.0 } else { 0.0 };
    view.y -= title_h;
    view.h += title_h;

    let mut s = String::with_capacity(8192);
    let _ = write!(
        s,
        r#"<svg xmlns="http://www.w3.org/2000/svg" class="dg" viewBox="{} {} {} {}" width="{}" height="{}" font-family="{}" data-theme="{}">"#,
        n(view.x),
        n(view.y),
        n(view.w),
        n(view.h),
        n(view.w),
        n(view.h),
        esc(text::FONT),
        t.as_str()
    );
    if let Some(title) = &doc.title {
        let _ = write!(s, "<title>{}</title>", esc(title));
    }
    if !opts.transparent {
        let _ = write!(
            s,
            r#"<rect class="dg-bg" x="{}" y="{}" width="{}" height="{}" fill="{}"/>"#,
            n(view.x),
            n(view.y),
            n(view.w),
            n(view.h),
            p.bg
        );
    }
    if show_title {
        let _ = write!(
            s,
            r#"<text class="dg-title" x="{}" y="{}" font-size="18" font-weight="600" fill="{}">{}</text>"#,
            n(view.x + pad),
            n(view.y + pad + 12.0),
            p.fg,
            esc(doc.title.as_deref().unwrap_or(""))
        );
    }

    // groups, outermost first
    s.push_str(r#"<g class="dg-groups">"#);
    let mut groups: Vec<&layout::GroupBox> = lay.groups.iter().collect();
    groups.sort_by_key(|g| g.depth);
    for gb in groups {
        let g = doc.group(&gb.id).unwrap();
        let (mut fill, mut stroke) = match &g.color {
            Some(c) => theme::tint(c, t),
            None => (p.group_fill.to_string(), p.group_stroke.to_string()),
        };
        let mut dash = "";
        if let Some(c) = opts.changes.get(&g.id) {
            (fill, stroke, dash) = change_colors(*c, p);
        }
        let r = gb.rect;
        let _ = write!(
            s,
            r#"<g class="dg-group" data-id="{}"><rect class="dg-shape" x="{}" y="{}" width="{}" height="{}" rx="12" fill="{}" stroke="{}" stroke-width="1.2"{}/><text x="{}" y="{}" font-size="13" font-weight="600" fill="{}">{}</text></g>"#,
            esc(&g.id),
            n(r.x),
            n(r.y),
            n(r.w),
            n(r.h),
            fill,
            stroke,
            dash,
            n(r.x + 14.0),
            n(r.y + 21.0),
            if g.color.is_some() {
                stroke.as_str()
            } else {
                p.muted
            },
            esc(g.display())
        );
    }
    s.push_str("</g>");

    // edges
    s.push_str(r#"<g class="dg-edges">"#);
    for (e, path) in doc.edges.iter().zip(&edges) {
        let mut color = e
            .color
            .as_deref()
            .map(|c| theme::tint(c, t).1)
            .unwrap_or_else(|| p.edge.to_string());
        let change = opts.changes.get(&e.id).copied();
        let mut width: f64 = if e.line == Line::Thick { 3.0 } else { 1.5 };
        let mut dash = match e.line {
            Line::Dashed => r#" stroke-dasharray="6 5""#,
            Line::Dotted => r#" stroke-dasharray="1.5 4" stroke-linecap="round""#,
            _ => "",
        };
        if let Some(c) = change {
            color = match c {
                Change::Added => p.add,
                Change::Removed => p.del,
                Change::Changed => p.chg,
            }
            .to_string();
            width = width.max(2.0);
            if c == Change::Removed {
                dash = r#" stroke-dasharray="6 5""#;
            }
        }
        let _ = write!(s, r#"<g class="dg-edge" data-id="{}">"#, esc(&e.id));
        let _ = write!(
            s,
            r#"<path class="dg-hit" d="{}" fill="none" stroke="transparent" stroke-width="14"/>"#,
            path.d
        );
        let _ = write!(
            s,
            r#"<path class="dg-line" d="{}" fill="none" stroke="{}" stroke-width="{}"{}/>"#,
            path.d, color, width, dash
        );
        let head = 7.0 + width * 1.5;
        if matches!(e.arrow, Arrow::End | Arrow::Both) {
            arrowhead(&mut s, path.end, head, &color);
        }
        if matches!(e.arrow, Arrow::Start | Arrow::Both) {
            arrowhead(&mut s, path.start, head, &color);
        }
        if let Some((_, r, lines)) = labels.iter().find(|(id, ..)| *id == e.id) {
            let _ = write!(
                s,
                r#"<rect class="dg-label-bg" x="{}" y="{}" width="{}" height="{}" rx="4" fill="{}" opacity="0.92"/>"#,
                n(r.x),
                n(r.y),
                n(r.w),
                n(r.h),
                p.bg
            );
            let fill = if change.is_some() {
                color.as_str()
            } else {
                p.muted
            };
            text_lines(
                &mut s,
                lines,
                r.cx(),
                r.y + 2.0 + 12.0 * 0.95,
                12.0,
                fill,
                "middle",
                "",
            );
        }
        s.push_str("</g>");
    }
    s.push_str("</g>");

    // nodes
    s.push_str(r#"<g class="dg-nodes">"#);
    for nb in &lay.nodes {
        let node = doc.node(&nb.id).unwrap();
        node_svg(&mut s, node, nb, p, t, opts.changes.get(&node.id).copied());
    }
    s.push_str("</g>");

    if !pins.is_empty() {
        s.push_str(r#"<g class="dg-pins">"#);
        for pin in &pins {
            let fill = if pin.resolved { p.pin_resolved } else { p.pin };
            let _ = write!(
                s,
                r##"<g class="dg-pin" data-comment="{}"><circle cx="{}" cy="{}" r="10" fill="{}" stroke="{}" stroke-width="2"/><text x="{}" y="{}" font-size="11" font-weight="700" text-anchor="middle" fill="#ffffff">{}</text></g>"##,
                esc(&pin.id),
                n(pin.x),
                n(pin.y),
                fill,
                p.bg,
                n(pin.x),
                n(pin.y + 4.0),
                pin.number
            );
        }
        s.push_str("</g>");
    }
    s.push_str("</svg>");
    Rendered {
        svg: s,
        view,
        nodes: lay.nodes,
        groups: lay.groups,
        edges,
        pins,
    }
}

fn change_colors(c: Change, p: &Palette) -> (String, String, &'static str) {
    match c {
        Change::Added => (
            p.add_bg.to_string(),
            p.add.to_string(),
            r#" stroke-width="2""#,
        ),
        Change::Removed => (
            p.del_bg.to_string(),
            p.del.to_string(),
            r#" stroke-dasharray="6 4""#,
        ),
        Change::Changed => (
            p.chg_bg.to_string(),
            p.chg.to_string(),
            r#" stroke-width="2""#,
        ),
    }
}

fn arrowhead(s: &mut String, (tip, dir): ((f64, f64), (f64, f64)), size: f64, color: &str) {
    let (bx, by) = (tip.0 - dir.0 * size, tip.1 - dir.1 * size);
    let (px, py) = (-dir.1 * size * 0.45, dir.0 * size * 0.45);
    let _ = write!(
        s,
        r#"<path class="dg-arrow" d="M{},{} L{},{} L{},{} Z" fill="{}"/>"#,
        n(tip.0),
        n(tip.1),
        n(bx + px),
        n(by + py),
        n(bx - px),
        n(by - py),
        color
    );
}

#[allow(clippy::too_many_arguments)]
fn text_lines(
    s: &mut String,
    lines: &[String],
    x: f64,
    first_baseline: f64,
    size: f64,
    fill: &str,
    anchor: &str,
    extra: &str,
) {
    for (i, l) in lines.iter().enumerate() {
        let _ = write!(
            s,
            r#"<text x="{}" y="{}" font-size="{}" text-anchor="{}" fill="{}"{}>{}</text>"#,
            n(x),
            n(first_baseline + i as f64 * size * LINE_H),
            n(size),
            anchor,
            fill,
            extra,
            esc(l)
        );
    }
}

fn node_svg(
    s: &mut String,
    node: &Node,
    nb: &layout::NodeBox,
    p: &Palette,
    t: Theme,
    change: Option<Change>,
) {
    let r = nb.rect;
    let (x, y, w, h) = (r.x, r.y, r.w, r.h);
    let (cx, cy) = (r.cx(), r.cy());
    let sticky = node.shape == Shape::Sticky;
    let (mut fill, mut stroke) = if sticky {
        theme::sticky(node.color.as_deref(), t)
    } else {
        match &node.color {
            Some(c) => theme::tint(c, t),
            None => (p.node_fill.to_string(), p.node_stroke.to_string()),
        }
    };
    let mut extra = String::from(r#" stroke-width="1.5""#);
    if let Some(c) = change {
        let (f, st, attrs) = change_colors(c, p);
        (fill, stroke) = (f, st);
        extra = if attrs.contains("stroke-width") {
            attrs.to_string()
        } else {
            format!(r#" stroke-width="2"{attrs}"#)
        };
    }
    let _ = write!(
        s,
        r#"<g class="dg-node" data-id="{}"{}>"#,
        esc(&node.id),
        if change == Some(Change::Removed) {
            r#" opacity="0.75""#
        } else {
            ""
        }
    );
    if let Some(link) = &node.link {
        let _ = write!(s, r#"<a href="{}" target="_blank">"#, esc(link));
    }
    let shape_attr = format!(r#"fill="{fill}" stroke="{stroke}"{extra}"#);
    match node.shape {
        Shape::Box => {
            let _ = write!(
                s,
                r#"<rect class="dg-shape" x="{}" y="{}" width="{}" height="{}" rx="6" {shape_attr}/>"#,
                n(x),
                n(y),
                n(w),
                n(h)
            );
        }
        Shape::Rounded => {
            let _ = write!(
                s,
                r#"<rect class="dg-shape" x="{}" y="{}" width="{}" height="{}" rx="{}" {shape_attr}/>"#,
                n(x),
                n(y),
                n(w),
                n(h),
                n((h / 2.0).min(16.0))
            );
        }
        Shape::Pill => {
            let _ = write!(
                s,
                r#"<rect class="dg-shape" x="{}" y="{}" width="{}" height="{}" rx="{}" {shape_attr}/>"#,
                n(x),
                n(y),
                n(w),
                n(h),
                n(h / 2.0)
            );
        }
        Shape::Circle => {
            let _ = write!(
                s,
                r#"<ellipse class="dg-shape" cx="{}" cy="{}" rx="{}" ry="{}" {shape_attr}/>"#,
                n(cx),
                n(cy),
                n(w / 2.0),
                n(h / 2.0)
            );
        }
        Shape::Diamond => {
            let _ = write!(
                s,
                r#"<path class="dg-shape" d="M{},{} L{},{} L{},{} L{},{} Z" stroke-linejoin="round" {shape_attr}/>"#,
                n(cx),
                n(y),
                n(x + w),
                n(cy),
                n(cx),
                n(y + h),
                n(x),
                n(cy)
            );
        }
        Shape::Hexagon => {
            let i = 16.0f64.min(w / 4.0);
            let _ = write!(
                s,
                r#"<path class="dg-shape" d="M{},{} L{},{} L{},{} L{},{} L{},{} L{},{} Z" stroke-linejoin="round" {shape_attr}/>"#,
                n(x + i),
                n(y),
                n(x + w - i),
                n(y),
                n(x + w),
                n(cy),
                n(x + w - i),
                n(y + h),
                n(x + i),
                n(y + h),
                n(x),
                n(cy)
            );
        }
        Shape::Parallelogram => {
            let k = 16.0f64.min(w / 4.0);
            let _ = write!(
                s,
                r#"<path class="dg-shape" d="M{},{} L{},{} L{},{} L{},{} Z" stroke-linejoin="round" {shape_attr}/>"#,
                n(x + k),
                n(y),
                n(x + w),
                n(y),
                n(x + w - k),
                n(y + h),
                n(x),
                n(y + h)
            );
        }
        Shape::Cylinder => {
            let ry = 8.0f64.min(h / 4.0);
            let rx = w / 2.0;
            let _ = write!(
                s,
                r#"<path class="dg-shape" d="M{},{} A{},{} 0 0 1 {},{} L{},{} A{},{} 0 0 1 {},{} Z" {shape_attr}/><path d="M{},{} A{},{} 0 0 0 {},{}" fill="none" stroke="{stroke}"{extra}/>"#,
                n(x),
                n(y + ry),
                n(rx),
                n(ry),
                n(x + w),
                n(y + ry),
                n(x + w),
                n(y + h - ry),
                n(rx),
                n(ry),
                n(x),
                n(y + h - ry),
                n(x),
                n(y + ry),
                n(rx),
                n(ry),
                n(x + w),
                n(y + ry)
            );
        }
        Shape::Document => {
            let wave = 8.0;
            let _ = write!(
                s,
                r#"<path class="dg-shape" d="M{},{} L{},{} L{},{} Q{},{} {},{} T{},{} Z" {shape_attr}/>"#,
                n(x),
                n(y),
                n(x + w),
                n(y),
                n(x + w),
                n(y + h - wave),
                n(x + w * 0.75),
                n(y + h - 2.0 * wave),
                n(x + w * 0.5),
                n(y + h - wave),
                n(x),
                n(y + h - wave)
            );
        }
        Shape::Cloud => {
            let pt = |u: f64, v: f64| format!("{},{}", n(x + u * w), n(y + v * h));
            let _ = write!(
                s,
                r#"<path class="dg-shape" d="M{} C{} {} {} C{} {} {} C{} {} {} C{} {} {} C{} {} {} Z" {shape_attr}/>"#,
                pt(0.24, 0.86),
                pt(0.04, 0.86),
                pt(0.0, 0.56),
                pt(0.17, 0.5),
                pt(0.12, 0.22),
                pt(0.4, 0.1),
                pt(0.52, 0.26),
                pt(0.62, 0.04),
                pt(0.92, 0.1),
                pt(0.86, 0.4),
                pt(1.02, 0.45),
                pt(1.0, 0.86),
                pt(0.78, 0.86),
                pt(0.6, 0.94),
                pt(0.4, 0.94),
                pt(0.24, 0.86)
            );
        }
        Shape::Person => {
            let _ = write!(
                s,
                r#"<rect class="dg-shape" x="{}" y="{}" width="{}" height="{}" fill="transparent" stroke="none"/>"#,
                n(x),
                n(y),
                n(w),
                n(h)
            );
            let _ = write!(
                s,
                r#"<circle cx="{}" cy="{}" r="10" {shape_attr}/>"#,
                n(cx),
                n(y + 12.0)
            );
            let _ = write!(
                s,
                r#"<path d="M{},{} C{},{} {},{} {},{} C{},{} {},{} {},{} Z" {shape_attr}/>"#,
                n(cx - 18.0),
                n(y + 46.0),
                n(cx - 18.0),
                n(y + 26.0),
                n(cx + 18.0),
                n(y + 26.0),
                n(cx + 18.0),
                n(y + 46.0),
                n(cx + 10.0),
                n(y + 48.0),
                n(cx - 10.0),
                n(y + 48.0),
                n(cx - 18.0),
                n(y + 46.0)
            );
        }
        Shape::Sticky => {
            let _ = write!(
                s,
                r#"<rect x="{}" y="{}" width="{}" height="{}" rx="3" fill="{}"/>"#,
                n(x + 2.0),
                n(y + 4.0),
                n(w),
                n(h),
                p.shadow
            );
            let _ = write!(
                s,
                r#"<rect class="dg-shape" x="{}" y="{}" width="{}" height="{}" rx="3" fill="{fill}" stroke="{stroke}" stroke-width="{}"/>"#,
                n(x),
                n(y),
                n(w),
                n(h),
                if change.is_some() { "2" } else { "0.8" }
            );
        }
        Shape::Text => {
            let _ = write!(
                s,
                r#"<rect class="dg-shape" x="{}" y="{}" width="{}" height="{}" rx="4" fill="transparent" stroke="{}"/>"#,
                n(x),
                n(y),
                n(w),
                n(h),
                if change.is_some() {
                    stroke.as_str()
                } else {
                    "none"
                }
            );
        }
    }
    // text block
    let label_h = nb.lines.len() as f64 * LABEL_SIZE * LINE_H;
    let detail_h = if nb.detail.is_empty() {
        0.0
    } else {
        4.0 + nb.detail.len() as f64 * DETAIL_SIZE * LINE_H
    };
    let block = label_h + detail_h;
    let fg = p.fg;
    let (anchor, tx, top) = match node.shape {
        Shape::Sticky => ("start", x + 14.0, y + 14.0),
        Shape::Person => ("middle", cx, y + 52.0),
        Shape::Cylinder => ("middle", cx, cy - block / 2.0 + 4.0),
        Shape::Document => ("middle", cx, cy - block / 2.0 - 4.0),
        _ => ("middle", cx, cy - block / 2.0),
    };
    let weight = if matches!(node.shape, Shape::Sticky | Shape::Text) {
        ""
    } else {
        r#" font-weight="500""#
    };
    let deco = if change == Some(Change::Removed) {
        r#" text-decoration="line-through""#
    } else {
        ""
    };
    let extra = format!("{weight}{deco}");
    text_lines(
        s,
        &nb.lines,
        tx,
        top + LABEL_SIZE * 0.95,
        LABEL_SIZE,
        fg,
        anchor,
        &extra,
    );
    if !nb.detail.is_empty() {
        text_lines(
            s,
            &nb.detail,
            tx,
            top + label_h + 4.0 + DETAIL_SIZE * 0.95,
            DETAIL_SIZE,
            p.muted,
            anchor,
            "",
        );
    }
    if node.link.is_some() {
        s.push_str("</a>");
    }
    s.push_str("</g>");
}

/// Comment pins: on the top-right corner of their element, at an edge's
/// midpoint, or where they were dropped on the canvas.
fn pins(doc: &Document, lay: &Layout, edges: &[EdgePath], mode: CommentMode) -> Vec<Pin> {
    if mode == CommentMode::None {
        return Vec::new();
    }
    let mut out: Vec<Pin> = Vec::new();
    let mut per_target: HashMap<String, usize> = HashMap::new();
    for (i, c) in doc.comments.iter().enumerate() {
        let resolved = c.status == Status::Resolved;
        if resolved && mode == CommentMode::Open {
            continue;
        }
        let (bx, by) = match c.on.as_deref() {
            Some(on) => {
                if let Some(r) = lay.rect(on) {
                    (r.right() - 4.0, r.y + 4.0)
                } else if let Some(e) = edges.iter().find(|e| e.id == on) {
                    (e.mid.0 + 12.0, e.mid.1 - 14.0)
                } else {
                    continue;
                }
            }
            None => match (c.x, c.y) {
                (Some(x), Some(y)) => (x, y),
                _ => (lay.bounds.right() + 20.0, lay.bounds.y + 12.0),
            },
        };
        let key =
            c.on.clone()
                .unwrap_or_else(|| format!("@{},{}", n(bx), n(by)));
        let k = per_target.entry(key).or_insert(0);
        out.push(Pin {
            id: c.id.clone(),
            number: i + 1,
            x: bx - *k as f64 * 18.0,
            y: by,
            resolved,
        });
        *k += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl;

    #[test]
    fn renders_every_shape() {
        let shapes: Vec<String> = Shape::ALL
            .iter()
            .map(|s| format!("{0}: {0} [{0}]", s.as_str()))
            .collect();
        let src = format!(
            "title: Shapes\n{}\nbox -> circle: label\ncomment on box: hi",
            shapes.join("\n")
        );
        let d = dsl::parse(&src).unwrap();
        let r = render(&d, &RenderOptions::default());
        assert!(r.svg.starts_with("<svg") && r.svg.ends_with("</svg>"));
        assert!(!r.svg.contains("NaN"));
        assert_eq!(
            r.svg.matches(r#"class="dg-node""#).count(),
            Shape::ALL.len()
        );
        assert_eq!(r.pins.len(), 1);
        assert!(r.svg.contains(">Shapes</text>"));
    }

    #[test]
    fn escapes_text() {
        let d = dsl::parse("a: <script>&").unwrap();
        let r = render(&d, &RenderOptions::default());
        assert!(r.svg.contains("&lt;script&gt;&amp;"));
    }

    #[test]
    fn empty_document() {
        let r = render(&Document::default(), &RenderOptions::default());
        assert!(r.svg.contains("viewBox=\"0 0 320 160\""));
    }
}
