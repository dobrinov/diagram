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
    /// Draw the document's title above the diagram (off by default: the title
    /// is metadata, used by briefs and export names).
    pub title: Option<bool>,
    /// Transparent instead of the theme's background.
    pub transparent: bool,
    pub padding: Option<f64>,
    /// Today (`YYYY-MM-DD`): gantt charts mark it with a line.
    pub today: Option<String>,
    /// Only show these elements (nodes, groups): the view is cropped to them.
    #[serde(default)]
    pub focus: Vec<String>,
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
    let path_of = |id: &str| edges.iter().find(|p| p.id == id);
    for e in &doc.edges {
        let Some(path) = path_of(&e.id) else { continue };
        if let Some(l) = e.label.as_deref().filter(|l| !l.is_empty()) {
            let lines = text::wrap(l, 12.0, 160.0);
            let w = lines
                .iter()
                .map(|s| text::width(s, 12.0))
                .fold(0.0, f64::max)
                + 14.0;
            let h = lines.len() as f64 * 12.0 * LINE_H + 8.0;
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
    let show_title = opts.title.unwrap_or(false) && doc.title.is_some();
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
    // a crop around some elements, e.g. to look at one block
    if let Some(r) = opts
        .focus
        .iter()
        .filter_map(|id| lay.rect(id))
        .reduce(|a, b| a.union(&b))
    {
        view = r.grow(pad);
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
    let _ = write!(
        s,
        r##"<defs><filter id="dg-shadow" x="-20%" y="-20%" width="140%" height="150%"><feDropShadow dx="0" dy="3" stdDeviation="4" flood-color="#000000" flood-opacity="{}"/></filter><pattern id="dg-dots" width="20" height="20" patternUnits="userSpaceOnUse"><circle cx="1" cy="1" r="1" fill="{}"/></pattern></defs>"##,
        p.shadow_opacity, p.dot
    );
    if !opts.transparent {
        let _ = write!(
            s,
            r#"<g class="dg-bg"><rect x="{0}" y="{1}" width="{2}" height="{3}" fill="{4}"/><rect x="{0}" y="{1}" width="{2}" height="{3}" fill="url(#dg-dots)"/></g>"#,
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
            r#"<text class="dg-title" x="{}" y="{}" font-size="20" font-weight="700" fill="{}">{}</text>"#,
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
        let tint = g.color.as_deref().and_then(|c| theme::tint(c, t));
        let (mut fill, mut stroke, mut accent) = match &tint {
            Some(k) => (k.frame.clone(), k.border.clone(), k.accent.clone()),
            None => (
                p.group_fill.to_string(),
                p.node_stroke.to_string(),
                p.group_accent.to_string(),
            ),
        };
        let mut attrs = String::from(r#" stroke-width="1""#);
        if let Some(c) = opts.changes.get(&g.id) {
            let extra;
            (fill, stroke, extra) = change_colors(*c, p);
            accent = stroke.clone();
            // the change style may bring its own stroke-width; never emit two
            if extra.contains("stroke-width") {
                attrs = extra.to_string();
            } else {
                attrs.push_str(extra);
            }
        }
        let r = gb.rect;
        let label = g.display();
        if let Some(kind) = gb.lane {
            // a swimlane: a band with its title in a strip at the side (rows) or on top
            let head_fill = tint
                .as_ref()
                .map(|k| k.soft.clone())
                .unwrap_or_else(|| p.header.to_string());
            let (hw, hh, tx, ty, line) = if kind == "row" {
                (
                    gb.head,
                    r.h,
                    r.x + 14.0,
                    r.y + r.h / 2.0 + 5.0,
                    (r.x + gb.head, r.y, r.x + gb.head, r.bottom()),
                )
            } else {
                (
                    r.w,
                    gb.head,
                    r.x + 14.0,
                    r.y + gb.head / 2.0 + 5.0,
                    (r.x, r.y + gb.head, r.right(), r.y + gb.head),
                )
            };
            let _ = write!(
                s,
                r#"<g class="dg-group dg-lane" data-id="{}"><rect class="dg-shape" x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}"{}/><rect x="{}" y="{}" width="{}" height="{}" fill="{}"/><line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/><text x="{}" y="{}" font-size="13" font-weight="700" fill="{}">{}</text></g>"#,
                esc(&g.id),
                n(r.x),
                n(r.y),
                n(r.w),
                n(r.h),
                fill,
                stroke,
                attrs,
                n(r.x + 0.5),
                n(r.y + 0.5),
                n(hw - 1.0),
                n(hh - 1.0),
                head_fill,
                n(line.0),
                n(line.1),
                n(line.2),
                n(line.3),
                stroke,
                n(tx),
                n(ty),
                p.fg,
                esc(label)
            );
            continue;
        }
        let pill_w = layout::group_label_w(label) - 2.0 * layout::GROUP_PILL_X;
        let on_accent = if tint.is_none() && t == Theme::Dark {
            p.bg
        } else {
            p.on_accent
        };
        let _ = write!(
            s,
            r#"<g class="dg-group" data-id="{}"><rect class="dg-shape" x="{}" y="{}" width="{}" height="{}" rx="18" fill="{}" stroke="{}"{}/><rect x="{}" y="{}" width="{}" height="28" rx="8" fill="{}"/><text x="{}" y="{}" font-size="14" font-weight="700" fill="{}">{}</text></g>"#,
            esc(&g.id),
            n(r.x),
            n(r.y),
            n(r.w),
            n(r.h),
            fill,
            stroke,
            attrs,
            n(r.x + layout::GROUP_PILL_X),
            n(r.y + 12.0),
            n(pill_w),
            accent,
            n(r.x + layout::GROUP_PILL_X + 14.0),
            n(r.y + 31.0),
            on_accent,
            esc(label)
        );
        if let Some(gb_t) = &gb.gantt {
            let today = opts.today.as_deref().and_then(crate::gantt::parse_date);
            gantt_svg(&mut s, doc, gb, gb_t, &lay, p, today);
        }
    }
    s.push_str("</g>");

    // mindmap branches, under everything else
    let maps: Vec<&layout::NodeBox> = lay.nodes.iter().filter(|nb| nb.map.is_some()).collect();
    if !maps.is_empty() {
        s.push_str(r#"<g class="dg-branches">"#);
        for nb in &maps {
            branch_svg(&mut s, nb, &lay, p, t, opts.changes.get(&nb.id).copied());
        }
        s.push_str("</g>");
    }

    // edges
    s.push_str(r#"<g class="dg-edges">"#);
    // labels go above every line, so no edge is drawn across another's label
    let mut label_svg = String::new();
    for e in &doc.edges {
        let Some(path) = path_of(&e.id) else { continue };
        let mut color = e
            .color
            .as_deref()
            .and_then(|c| theme::tint(c, t))
            .map(|k| k.accent)
            .unwrap_or_else(|| p.edge.to_string());
        let change = opts.changes.get(&e.id).copied();
        let mut width: f64 = if e.line == Line::Thick { 3.5 } else { 2.0 };
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
            width = width.max(2.5);
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
        let head = 6.0 + width * 1.5;
        if matches!(e.arrow, Arrow::End | Arrow::Both) {
            arrowhead(&mut s, path.end, head, &color);
        }
        if matches!(e.arrow, Arrow::Start | Arrow::Both) {
            arrowhead(&mut s, path.start, head, &color);
        }
        s.push_str("</g>");
        if let Some((_, r, lines)) = labels.iter().find(|(id, ..)| *id == e.id) {
            let s = &mut label_svg;
            let _ = write!(
                s,
                r#"<g class="dg-edge dg-edge-label" data-id="{}">"#,
                esc(&e.id)
            );
            let _ = write!(
                s,
                r#"<rect class="dg-label-bg" x="{}" y="{}" width="{}" height="{}" rx="6" fill="{}" stroke="{}"/>"#,
                n(r.x),
                n(r.y),
                n(r.w),
                n(r.h),
                p.node_fill,
                p.node_stroke
            );
            let fill = if change.is_some() {
                color.as_str()
            } else {
                p.muted
            };
            text_lines(
                s,
                lines,
                r.cx(),
                r.y + 4.0 + 12.0 * 0.95,
                12.0,
                fill,
                "middle",
                r#" font-weight="500""#,
            );
            s.push_str("</g>");
        }
    }
    s.push_str("</g>");
    if !label_svg.is_empty() {
        s.push_str(r#"<g class="dg-edge-labels">"#);
        s.push_str(&label_svg);
        s.push_str("</g>");
    }

    // nodes
    s.push_str(r#"<g class="dg-nodes">"#);
    for nb in &lay.nodes {
        let node = doc.node(&nb.id).unwrap();
        let change = opts.changes.get(&node.id).copied();
        match nb.map.as_ref().and_then(|m| m.style) {
            _ if nb.task => task_svg(&mut s, node, nb, p, t, change),
            Some(st) => map_node_svg(&mut s, node, nb, st, p, t, change),
            None => node_svg(&mut s, node, nb, p, t, change),
        }
    }
    s.push_str("</g>");
    // a badge on linked blocks: a web page, or `#id` for another element
    let linked: Vec<(&layout::NodeBox, &str)> = lay
        .nodes
        .iter()
        .filter_map(|nb| Some((nb, doc.node(&nb.id)?.link.as_deref()?)))
        .collect();
    if !linked.is_empty() {
        s.push_str(r#"<g class="dg-links">"#);
        for (nb, link) in linked {
            let (x, y) = (nb.rect.right() - 2.0, nb.rect.bottom() - 2.0);
            let open = match web_link(doc.node(&nb.id).unwrap()) {
                Some(l) => format!(r#"<a href="{}" target="_blank">"#, esc(l)),
                None => String::new(),
            };
            let _ = write!(
                s,
                r#"{open}<g class="dg-link" data-node="{}" data-link="{}" transform="translate({},{})"><title>{}</title><circle r="10" fill="{}" stroke="{}" stroke-width="2"/><path transform="translate(-6.5,-6.5) scale(0.54)" d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71" fill="none" stroke="{}" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round"/></g>{}"#,
                esc(&nb.id),
                esc(link),
                n(x),
                n(y),
                esc(&link_title(doc, link)),
                p.group_accent,
                p.node_fill,
                p.on_accent,
                if open.is_empty() { "" } else { "</a>" }
            );
        }
        s.push_str("</g>");
    }
    // folded branches show how much is tucked away
    let folds: Vec<&&layout::NodeBox> = maps
        .iter()
        .filter(|nb| nb.map.as_ref().is_some_and(|m| m.folded > 0))
        .collect();
    if !folds.is_empty() {
        s.push_str(r#"<g class="dg-folds">"#);
        for nb in folds {
            let m = nb.map.as_ref().unwrap();
            let (x, y) = fold_point(nb);
            let accent = branch_color(m, p, t);
            let _ = write!(
                s,
                r#"<g class="dg-fold" data-fold="{}"><circle cx="{}" cy="{}" r="10" fill="{}" stroke="{}" stroke-width="1.5"/><text x="{}" y="{}" font-size="10" font-weight="700" text-anchor="middle" fill="{}">{}</text></g>"#,
                esc(&nb.id),
                n(x),
                n(y),
                p.node_fill,
                accent,
                n(x),
                n(y + 3.5),
                accent,
                m.folded
            );
        }
        s.push_str("</g>");
    }

    if !pins.is_empty() {
        s.push_str(r#"<g class="dg-pins">"#);
        for pin in &pins {
            let fill = if pin.resolved { p.pin_resolved } else { p.pin };
            let _ = write!(
                s,
                r##"<g class="dg-pin" data-comment="{}"><circle cx="{}" cy="{}" r="11" fill="{}" stroke="{}" stroke-width="2.5" filter="url(#dg-shadow)"/><text x="{}" y="{}" font-size="11" font-weight="700" text-anchor="middle" fill="#ffffff">{}</text></g>"##,
                esc(&pin.id),
                n(pin.x),
                n(pin.y),
                fill,
                p.node_fill,
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

/// The colour a mindmap topic's branch is drawn in.
fn branch_color(m: &layout::MapInfo, p: &Palette, t: Theme) -> String {
    m.color
        .as_deref()
        .and_then(|c| theme::tint(c, t))
        .map(|k| k.accent)
        .unwrap_or_else(|| p.group_accent.to_string())
}

/// Where a branch leaves a topic (towards `side`) or arrives at it.
fn anchor(nb: &layout::NodeBox, side: &str, outgoing: bool) -> (f64, f64) {
    let r = nb.rect;
    let topic = nb.map.as_ref().and_then(|m| m.style) == Some(crate::mindmap::Style::Topic);
    let y = if topic { r.bottom() } else { r.cy() };
    match (side, outgoing) {
        ("down", true) => (r.cx(), r.bottom()),
        ("down", false) => (r.cx(), r.y),
        ("left", true) | ("right", false) => (r.x, y),
        _ => (r.right(), y),
    }
}

fn fold_point(nb: &layout::NodeBox) -> (f64, f64) {
    let m = nb.map.as_ref().unwrap();
    let side = if m.depth == 0 {
        match m.layout {
            MapLayout::Down => "down",
            MapLayout::Left => "left",
            _ => "right",
        }
    } else {
        m.side
    };
    let (x, y) = anchor(nb, side, true);
    match side {
        "down" => (x, y + 12.0),
        "left" => (x - 12.0, y),
        _ => (x + 12.0, y),
    }
}

fn branch_svg(
    s: &mut String,
    nb: &layout::NodeBox,
    lay: &Layout,
    p: &Palette,
    t: Theme,
    change: Option<Change>,
) {
    let m = nb.map.as_ref().unwrap();
    let Some(parent) = m.parent.as_deref().and_then(|id| lay.node(id)) else {
        return;
    };
    let a = anchor(parent, m.side, true);
    let b = anchor(nb, m.side, false);
    let d = if m.side == "down" {
        let dy = (b.1 - a.1) / 2.0;
        format!(
            "M{},{} C{},{} {},{} {},{}",
            n(a.0),
            n(a.1),
            n(a.0),
            n(a.1 + dy),
            n(b.0),
            n(b.1 - dy),
            n(b.0),
            n(b.1)
        )
    } else {
        let dx = (b.0 - a.0) / 2.0;
        format!(
            "M{},{} C{},{} {},{} {},{}",
            n(a.0),
            n(a.1),
            n(a.0 + dx),
            n(a.1),
            n(b.0 - dx),
            n(b.1),
            n(b.0),
            n(b.1)
        )
    };
    let width = match m.depth {
        1 => 3.0,
        2 => 2.2,
        _ => 1.6,
    };
    let (color, dash) = match change {
        Some(Change::Added) => (p.add.to_string(), ""),
        Some(Change::Removed) => (p.del.to_string(), r#" stroke-dasharray="6 4""#),
        Some(Change::Changed) => (p.chg.to_string(), ""),
        None => (branch_color(m, p, t), ""),
    };
    let _ = write!(
        s,
        r#"<path class="dg-branch" data-id="{}" d="{}" fill="none" stroke="{}" stroke-width="{}" stroke-linecap="round"{}/>"#,
        esc(&nb.id),
        d,
        color,
        width,
        dash
    );
}

/// A gantt chart's date header, week lines, task names and dependencies
/// (drawn with the chart's frame, under the bars).
fn gantt_svg(
    s: &mut String,
    doc: &Document,
    gb: &layout::GroupBox,
    g: &layout::GanttBox,
    lay: &Layout,
    p: &Palette,
    today: Option<i64>,
) {
    let (ox, oy) = (
        gb.rect.x + layout::GROUP_PAD,
        gb.rect.y + layout::GROUP_HEAD,
    );
    let top = oy + layout::GANTT_HEAD;
    let bottom = top + g.rows.len().max(1) as f64 * layout::GANTT_ROW;
    let right = ox + g.x0 + g.days * g.day;
    s.push_str(r#"<g class="dg-gantt">"#);
    // rows
    for k in 0..=g.rows.len().max(1) {
        let y = top + k as f64 * layout::GANTT_ROW;
        let _ = write!(
            s,
            r#"<line x1="{}" y1="{y}" x2="{}" y2="{y}" stroke="{}" stroke-width="1"/>"#,
            n(ox),
            n(right),
            p.node_stroke,
            y = n(y)
        );
    }
    // a line and a label at each step of the scale (Mondays, 1st of the
    // month, … when the chart has dates)
    let ticks = crate::gantt::ticks(g.scale, g.origin, g.days);
    let mut free = f64::MIN; // labels do not overlap: the later one is left out
    for (k, (d, label)) in ticks.iter().enumerate() {
        let d = *d;
        let x = ox + g.x0 + d * g.day;
        let w = text::width(label, 11.0) + 8.0;
        // a short first month or quarter gives its label to the next one
        let partial = k == 0
            && matches!(g.scale, Scale::Month | Scale::Quarter)
            && ticks.get(1).is_some_and(|t| t.0 * g.day < w);
        let _ = write!(
            s,
            r#"<line x1="{x}" y1="{}" x2="{x}" y2="{}" stroke="{}" stroke-width="1"/>"#,
            n(top - 6.0),
            n(bottom),
            p.node_stroke,
            x = n(x)
        );
        if d < g.days && x >= free && !partial {
            free = x + w;
            let _ = write!(
                s,
                r#"<text x="{}" y="{}" font-size="11" fill="{}">{}</text>"#,
                n(x + 4.0),
                n(top - 10.0),
                p.muted,
                esc(label)
            );
        }
    }
    // today
    if let (Some(t), Some(o)) = (today, g.origin) {
        let d = (t - o) as f64 + 0.5;
        if (0.0..g.days).contains(&d) {
            let x = ox + g.x0 + d * g.day;
            let _ = write!(
                s,
                r##"<g class="dg-today"><line x1="{x}" y1="{}" x2="{x}" y2="{}" stroke="#e03131" stroke-width="1.5"/><circle cx="{x}" cy="{}" r="3" fill="#e03131"/></g>"##,
                n(top - 4.0),
                n(bottom),
                n(top - 4.0),
                x = n(x)
            );
        }
    }
    // task names, and who does each
    for (k, id) in g.rows.iter().enumerate() {
        let Some(node) = doc.node(id) else { continue };
        let owner = node
            .owner
            .as_deref()
            .map(|o| {
                format!(
                    r#"<tspan dx="8" font-size="12" fill="{}">{}</tspan>"#,
                    p.muted,
                    esc(o)
                )
            })
            .unwrap_or_default();
        let _ = write!(
            s,
            r#"<text x="{}" y="{}" font-size="13" fill="{}">{}{owner}</text>"#,
            n(ox),
            n(top + k as f64 * layout::GANTT_ROW + layout::GANTT_ROW / 2.0 + 4.5),
            p.fg,
            esc(node.display())
        );
    }
    // dependencies: from the end of a task to the start of the one after it
    for id in &g.rows {
        let (Some(node), Some(to)) = (doc.node(id), lay.rect(id)) else {
            continue;
        };
        for a in &node.after {
            let Some(from) = lay.rect(a) else { continue };
            // waits for a task in another chart: dashed
            let dash = if doc.node(a).is_some_and(|t| t.group != node.group) {
                r#" stroke-dasharray="5 4""#
            } else {
                ""
            };
            let (x1, y1, x2, y2) = (from.right(), from.cy(), to.x, to.cy());
            let mid = x1 + 6.0;
            let path = if !dash.is_empty() {
                // from another chart: along the row's top line, clear of the
                // task names, then down into the bar
                let yb = to.y - (layout::GANTT_ROW - layout::GANTT_BAR) / 2.0;
                format!(
                    "M{},{} H{} V{} H{} V{} H{}",
                    n(x1),
                    n(y1),
                    n(mid),
                    n(yb),
                    n(x2 - 8.0),
                    n(y2),
                    n(x2 - 3.0)
                )
            } else if x2 - x1 >= 12.0 {
                format!(
                    "M{},{} H{} V{} H{}",
                    n(x1),
                    n(y1),
                    n(mid),
                    n(y2),
                    n(x2 - 3.0)
                )
            } else {
                // the next task starts before this one ends: go round
                let below = (y1 + y2) / 2.0;
                format!(
                    "M{},{} H{} V{} H{} V{} H{}",
                    n(x1),
                    n(y1),
                    n(x1 + 6.0),
                    n(below),
                    n(x2 - 8.0),
                    n(y2),
                    n(x2 - 3.0)
                )
            };
            let _ = write!(
                s,
                r#"<g class="dg-dep" data-from="{}" data-to="{}"><path d="{path}" fill="none" stroke="{}" stroke-width="1.5"{dash}/><path d="M{},{} l-5,-3.5 v7 z" fill="{}"/><path d="{path}" fill="none" stroke="transparent" stroke-width="10" pointer-events="stroke"/></g>"#,
                esc(a),
                esc(id),
                p.edge,
                n(x2),
                n(y2),
                p.edge
            );
        }
    }
    s.push_str("</g>");
}

/// A gantt task: a bar, with its duration inside when it fits.
fn task_svg(
    s: &mut String,
    node: &Node,
    nb: &layout::NodeBox,
    p: &Palette,
    t: Theme,
    change: Option<Change>,
) {
    let r = nb.rect;
    let k = theme::tint(node.color.as_deref().unwrap_or("blue"), t);
    let (mut fill, mut stroke) = match &k {
        Some(k) => (k.accent.clone(), k.accent.clone()),
        None => (p.edge.to_string(), p.edge.to_string()),
    };
    if let Some(c) = change {
        let (_, st, _) = change_colors(c, p);
        stroke = st.clone();
        fill = st;
    }
    if node.milestone {
        let _ = write!(
            s,
            r#"<g class="dg-node dg-task dg-milestone" data-id="{}"><path class="dg-shape" d="M{},{} L{},{} L{},{} L{},{} Z" fill="{}" stroke="{}" stroke-width="1"/></g>"#,
            esc(&node.id),
            n(r.cx()),
            n(r.y),
            n(r.right()),
            n(r.cy()),
            n(r.cx()),
            n(r.y + r.h),
            n(r.x),
            n(r.cy()),
            fill,
            stroke
        );
        return;
    }
    let days = crate::gantt::task_days(node);
    let done = node.done.unwrap_or(0.0).clamp(0.0, 100.0);
    let _ = write!(
        s,
        r#"<g class="dg-node dg-task" data-id="{}"><rect class="dg-shape" x="{}" y="{}" width="{}" height="{}" rx="5" fill="{}" stroke="{}" stroke-width="1"{}/>"#,
        esc(&node.id),
        n(r.x),
        n(r.y),
        n(r.w),
        n(r.h),
        fill,
        stroke,
        // in progress: the part still to do is lighter
        if done > 0.0 && done < 100.0 {
            r#" fill-opacity="0.45""#
        } else {
            ""
        }
    );
    if done > 0.0 && done < 100.0 {
        let _ = write!(
            s,
            r#"<rect x="{}" y="{}" width="{}" height="{}" rx="5" fill="{}" pointer-events="none"/>"#,
            n(r.x),
            n(r.y),
            n(r.w * done / 100.0),
            n(r.h),
            fill
        );
    }
    let mut label = crate::gantt::format_days(days);
    if done >= 100.0 {
        label.push_str(" ✓");
    } else if done > 0.0 {
        label.push_str(&format!(" · {}%", done.round()));
    }
    if r.w >= text::width(&label, 11.0) + 12.0 {
        let _ = write!(
            s,
            r#"<text x="{}" y="{}" font-size="11" font-weight="600" fill="{}">{}</text>"#,
            n(r.x + 6.0),
            n(r.cy() + 4.0),
            p.on_accent,
            esc(&label)
        );
    }
    s.push_str("</g>");
}

fn map_node_svg(
    s: &mut String,
    node: &Node,
    nb: &layout::NodeBox,
    st: crate::mindmap::Style,
    p: &Palette,
    t: Theme,
    change: Option<Change>,
) {
    use crate::mindmap::Style;
    let m = nb.map.as_ref().unwrap();
    let r = nb.rect;
    let mut accent = branch_color(m, p, t);
    let mut fill = p.node_fill.to_string();
    let mut dash = "";
    if let Some(c) = change {
        let (f, stroke, attrs) = change_colors(c, p);
        accent = stroke;
        fill = f;
        if attrs.contains("dasharray") {
            dash = r#" stroke-dasharray="6 4""#;
        }
    }
    let _ = write!(
        s,
        r#"<g class="dg-node dg-topic" data-id="{}" data-depth="{}"{}>"#,
        esc(&node.id),
        m.depth,
        if change == Some(Change::Removed) {
            r#" opacity="0.75""#
        } else {
            ""
        }
    );
    if let Some(link) = web_link(node) {
        let _ = write!(s, r#"<a href="{}" target="_blank">"#, esc(link));
    }
    let lifted = r#" filter="url(#dg-shadow)""#;
    let (fg, weight, anchor_kind) = match st {
        Style::Centre => {
            let (bg, ink) = if change.is_some() {
                (fill.clone(), p.fg.to_string())
            } else if node.color.is_some() {
                (accent.clone(), p.on_accent.to_string())
            } else {
                (p.group_accent.to_string(), p.bg.to_string())
            };
            let _ = write!(
                s,
                r#"<rect class="dg-shape" x="{}" y="{}" width="{}" height="{}" rx="{}" fill="{}" stroke="{}" stroke-width="{}"{dash}{lifted}/>"#,
                n(r.x),
                n(r.y),
                n(r.w),
                n(r.h),
                n((r.h / 2.0).min(18.0)),
                bg,
                if change.is_some() {
                    accent.as_str()
                } else {
                    "none"
                },
                if change.is_some() { 2.0 } else { 0.0 }
            );
            (ink, "700", "middle")
        }
        Style::Branch | Style::Card => {
            let (rx, sw) = if st == Style::Branch {
                (12.0, 2.0)
            } else {
                (8.0, 1.5)
            };
            let _ = write!(
                s,
                r#"<rect class="dg-shape" x="{}" y="{}" width="{}" height="{}" rx="{}" fill="{}" stroke="{}" stroke-width="{}"{dash}{lifted}/>"#,
                n(r.x),
                n(r.y),
                n(r.w),
                n(r.h),
                n(rx),
                fill,
                accent,
                sw
            );
            (
                p.fg.to_string(),
                if st == Style::Branch { "600" } else { "500" },
                "middle",
            )
        }
        Style::Topic => {
            let _ = write!(
                s,
                r#"<rect class="dg-shape" x="{}" y="{}" width="{}" height="{}" rx="4" fill="{}" stroke="none"/><path d="M{},{} L{},{}" stroke="{}" stroke-width="{}" stroke-linecap="round"{dash}/>"#,
                n(r.x),
                n(r.y),
                n(r.w),
                n(r.h),
                if change.is_some() {
                    fill.as_str()
                } else {
                    "transparent"
                },
                n(r.x),
                n(r.bottom()),
                n(r.right()),
                n(r.bottom()),
                accent,
                if m.depth == 2 { 2.2 } else { 1.6 }
            );
            (
                p.fg.to_string(),
                "500",
                if m.side == "left" { "end" } else { "start" },
            )
        }
    };
    let size = nb.font;
    let label_h = nb.lines.len() as f64 * size * LINE_H;
    let detail_h = if nb.detail.is_empty() {
        0.0
    } else {
        2.0 + nb.detail.len() as f64 * DETAIL_SIZE * LINE_H
    };
    let pad = if st == Style::Topic { 6.0 } else { 0.0 };
    let tx = match anchor_kind {
        "start" => r.x + pad,
        "end" => r.right() - pad,
        _ => r.cx(),
    };
    let top = if st == Style::Topic {
        r.y + pad
    } else {
        r.cy() - (label_h + detail_h) / 2.0
    };
    let deco = if change == Some(Change::Removed) {
        r#" text-decoration="line-through""#
    } else {
        ""
    };
    text_lines(
        s,
        &nb.lines,
        tx,
        top + size * 0.95,
        size,
        &fg,
        anchor_kind,
        &format!(r#" font-weight="{weight}"{deco}"#),
    );
    if !nb.detail.is_empty() {
        let muted = if st == Style::Centre && change.is_none() {
            fg.as_str()
        } else {
            p.muted
        };
        text_lines(
            s,
            &nb.detail,
            tx,
            top + label_h + 2.0 + DETAIL_SIZE * 0.95,
            DETAIL_SIZE,
            muted,
            anchor_kind,
            r#" opacity="0.85""#,
        );
    }
    if web_link(node).is_some() {
        s.push_str("</a>");
    }
    s.push_str("</g>");
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
    let tint = node.color.as_deref().and_then(|c| theme::tint(c, t));
    let card = matches!(node.shape, Shape::Box | Shape::Rounded);
    // cards stay white with a coloured strip; other shapes take a soft fill
    let (mut fill, mut stroke, mut width) = match (&tint, node.shape) {
        (_, Shape::Sticky) => (
            theme::sticky(node.color.as_deref()),
            "none".to_string(),
            0.0,
        ),
        (Some(k), _) if card => (k.frame.clone(), p.node_stroke.to_string(), 1.0),
        (Some(k), _) => (k.soft.clone(), k.accent.clone(), 1.5),
        (None, _) => (p.node_fill.to_string(), p.node_stroke.to_string(), 1.0),
    };
    let mut accent = tint
        .as_ref()
        .map(|k| k.accent.clone())
        .unwrap_or_else(|| theme::avatar(&node.id).to_string());
    let mut dash = "";
    if let Some(c) = change {
        let (f, st, attrs) = change_colors(c, p);
        accent = st.clone();
        (fill, stroke, width) = (f, st, 2.0);
        if attrs.contains("dasharray") {
            dash = r#" stroke-dasharray="6 4""#;
        }
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
    if let Some(link) = web_link(node) {
        let _ = write!(s, r#"<a href="{}" target="_blank">"#, esc(link));
    }
    let lifted = r#" filter="url(#dg-shadow)""#;
    let shape_attr = format!(
        r#"fill="{fill}" stroke="{stroke}" stroke-width="{}"{dash}{lifted}"#,
        n(width)
    );
    let line_attr = format!(
        r#"fill="none" stroke="{stroke}" stroke-width="{}"{dash}"#,
        n(width)
    );
    let rect = |s: &mut String, rx: f64| {
        let _ = write!(
            s,
            r#"<rect class="dg-shape" x="{}" y="{}" width="{}" height="{}" rx="{}" {shape_attr}/>"#,
            n(x),
            n(y),
            n(w),
            n(h),
            n(rx)
        );
    };
    match node.shape {
        Shape::Box | Shape::Rounded => {
            let rx = if node.shape == Shape::Box {
                8.0
            } else {
                (h / 2.0).min(14.0)
            };
            rect(s, rx);
            if tint.is_some() && change.is_none() {
                // status strip along the top edge, clipped to the corners
                let strip = 6.0;
                let dx = (rx * rx - (rx - strip) * (rx - strip)).sqrt();
                let _ = write!(
                    s,
                    r#"<path d="M{},{} A{},{} 0 0 1 {},{} L{},{} A{},{} 0 0 1 {},{} Z" fill="{}"/>"#,
                    n(x + rx - dx),
                    n(y + strip),
                    n(rx),
                    n(rx),
                    n(x + rx),
                    n(y),
                    n(x + w - rx),
                    n(y),
                    n(rx),
                    n(rx),
                    n(x + w - rx + dx),
                    n(y + strip),
                    accent
                );
            }
        }
        Shape::Pill => rect(s, h / 2.0),
        Shape::Sticky => rect(s, 3.0),
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
                r#"<path class="dg-shape" d="M{},{} A{},{} 0 0 1 {},{} L{},{} A{},{} 0 0 1 {},{} Z" {shape_attr}/><path d="M{},{} A{},{} 0 0 0 {},{}" {line_attr}/>"#,
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
            // an initials avatar, like a FigJam / SpecDeck person card
            let _ = write!(
                s,
                r#"<rect class="dg-shape" x="{}" y="{}" width="{}" height="{}" rx="8" fill="transparent" stroke="none"/>"#,
                n(x),
                n(y),
                n(w),
                n(h)
            );
            let _ = write!(
                s,
                r#"<circle cx="{}" cy="{}" r="20" fill="{}" stroke="{}" stroke-width="2"{lifted}/><text x="{}" y="{}" font-size="13" font-weight="700" text-anchor="middle" fill="{}">{}</text>"#,
                n(cx),
                n(y + 22.0),
                accent,
                p.node_fill,
                n(cx),
                n(y + 26.5),
                p.on_accent,
                esc(&initials(node.display()))
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
    let sticky = node.shape == Shape::Sticky;
    let (fg, detail_fg) = if sticky {
        (p.sticky_fg, "#1c202699")
    } else {
        (p.fg, p.muted)
    };
    let (anchor, tx, top) = match node.shape {
        Shape::Sticky => ("start", x + 14.0, y + 14.0),
        Shape::Person => ("middle", cx, y + 52.0),
        Shape::Cylinder => ("middle", cx, cy - block / 2.0 + 4.0),
        Shape::Document => ("middle", cx, cy - block / 2.0 - 4.0),
        _ => ("middle", cx, cy - block / 2.0),
    };
    let weight = match node.shape {
        Shape::Text => "",
        Shape::Sticky => r#" font-weight="500""#,
        _ => r#" font-weight="600""#,
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
            detail_fg,
            anchor,
            "",
        );
    }
    if web_link(node).is_some() {
        s.push_str("</a>");
    }
    s.push_str("</g>");
}

/// A node's link when it is a web page (`#id` links point at elements).
fn web_link(node: &Node) -> Option<&str> {
    node.link.as_deref().filter(|l| !l.starts_with('#'))
}

/// The tooltip of a link badge: where it goes.
fn link_title(doc: &Document, link: &str) -> String {
    match link.strip_prefix('#') {
        Some(id) => {
            let name = doc
                .node(id)
                .map(|n| n.display().replace('\n', " "))
                .or_else(|| doc.group(id).map(|g| g.display().to_string()));
            match name {
                Some(n) => format!("Go to {n}"),
                None => format!("Go to {id} (not on the board)"),
            }
        }
        None => link.to_string(),
    }
}

/// "Ana Petrova" → "AP", "Postgres" → "PO".
fn initials(name: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().collect();
    let s: String = if words.len() >= 2 {
        words
            .iter()
            .take(2)
            .filter_map(|w| w.chars().next())
            .collect()
    } else {
        name.chars()
            .filter(|c| c.is_alphanumeric())
            .take(2)
            .collect()
    };
    s.to_uppercase()
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
        // the title is metadata: drawn only when asked for
        assert!(!r.svg.contains(">Shapes</text>"));
        let titled = RenderOptions {
            title: Some(true),
            ..RenderOptions::default()
        };
        assert!(render(&d, &titled).svg.contains(">Shapes</text>"));
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
