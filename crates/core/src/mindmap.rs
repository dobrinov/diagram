//! Mindmaps: a central topic with branches, laid out as a tidy tree.
//!
//! A mindmap is made of ordinary nodes: the centre carries `mindmap` (which
//! way it grows) and every branch points at its topic with `parent`. So
//! comments, ops, diffs and cross-links work on mindmaps like on anything
//! else. In the DSL a map is an indented outline:
//!
//! ```text
//! mindmap pf: Payments France [layout: both] {
//!   Discovery
//!     Who pays today?
//!   Regulations [red]
//!     PSD2
//! }
//! ```
//!
//! The same outline (or a Markdown list) is what LLMs read and write, which
//! keeps AI edits to a few tokens per topic.
use crate::dsl;
use crate::layout::{Rect, DETAIL_SIZE, LABEL_SIZE, LINE_H};
use crate::model::*;
use crate::text;

/// Branch colours, handed out to the centre's direct branches in order.
pub const BRANCH_COLORS: [&str; 8] = [
    "blue", "green", "orange", "purple", "pink", "teal", "red", "yellow",
];

// ------------------------------------------------------------------ outline

/// One topic of an outline: its depth (0 = top) and text (label plus an
/// optional `[attrs]` block).
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub depth: usize,
    pub text: String,
    /// 1-based line in the source.
    pub line: usize,
}

/// Turns indentation into depth. Any consistent indentation works; a line
/// indented between two levels belongs to the shallower one's last child.
#[derive(Debug, Default, Clone)]
pub struct Indent {
    stack: Vec<usize>,
}

impl Indent {
    pub fn depth(&mut self, width: usize) -> usize {
        while self.stack.last().is_some_and(|&w| w > width) {
            self.stack.pop();
        }
        if self.stack.last() == Some(&width) {
            self.stack.pop();
        }
        let d = self.stack.len();
        self.stack.push(width);
        d
    }
}

/// Leading whitespace width (a tab counts as four spaces) and the rest.
pub fn indent_of(raw: &str) -> (usize, &str) {
    let mut w = 0;
    for (i, ch) in raw.char_indices() {
        match ch {
            ' ' => w += 1,
            '\t' => w += 4,
            _ => return (w, &raw[i..]),
        }
    }
    (w, "")
}

/// `- item`, `* item`, `+ item` → `item`.
pub fn strip_bullet(t: &str) -> &str {
    for b in ["- ", "* ", "+ "] {
        if let Some(rest) = t.strip_prefix(b) {
            return rest.trim_start();
        }
    }
    t
}

/// Read an outline. With `markdown`, headings (`#`, `##`, …), numbered
/// items, checkboxes, `**bold**` and code fences are understood too, so an
/// LLM's reply or a pasted Markdown list can be used as it is.
pub fn parse_outline(src: &str, markdown: bool) -> Vec<Line> {
    let mut out = Vec::new();
    let mut indent = Indent::default();
    let min_heading = src
        .lines()
        .filter_map(|l| heading_level(l.trim()))
        .min()
        .unwrap_or(1);
    let mut base = 0;
    for (i, raw) in src.lines().enumerate() {
        let (w, t) = indent_of(raw);
        let t = t.trim_end();
        if t.is_empty() || t.starts_with("//") || (markdown && t.starts_with("```")) {
            continue;
        }
        if markdown {
            if let Some(level) = heading_level(t) {
                let depth = level - min_heading;
                out.push(Line {
                    depth,
                    text: clean_md(t.trim_start_matches('#')),
                    line: i + 1,
                });
                base = depth + 1;
                indent = Indent::default();
                continue;
            }
        } else if t.starts_with('#') {
            continue;
        }
        let text = if markdown {
            clean_md(strip_number(strip_bullet(t)))
        } else {
            strip_bullet(t).to_string()
        };
        if text.is_empty() {
            continue;
        }
        out.push(Line {
            depth: base + indent.depth(w),
            text,
            line: i + 1,
        });
    }
    out
}

fn heading_level(t: &str) -> Option<usize> {
    let n = t.chars().take_while(|&c| c == '#').count();
    (n > 0 && n <= 6 && t[n..].starts_with(' ')).then_some(n)
}

fn strip_number(t: &str) -> &str {
    let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && digits < 4 {
        let rest = &t[digits..];
        for sep in [". ", ") "] {
            if let Some(r) = rest.strip_prefix(sep) {
                return r.trim_start();
            }
        }
    }
    t
}

fn clean_md(t: &str) -> String {
    let mut t = t.trim();
    for cb in ["[ ] ", "[x] ", "[X] "] {
        if let Some(r) = t.strip_prefix(cb) {
            t = r;
        }
    }
    let s = t.replace("**", "").replace("__", "");
    s.trim().to_string()
}

/// How a label is written on an outline line, so it reads back unchanged.
pub fn outline_label(label: &str) -> String {
    let risky = ["- ", "* ", "+ ", "#", "//", "}"]
        .iter()
        .any(|p| label.starts_with(p))
        || label == "-"
        || label == "*";
    let printed = dsl::label_text(label);
    if risky && !printed.starts_with('"') {
        dsl::quote(label)
    } else {
        printed
    }
}

// ----------------------------------------------------------------- forest

/// Mindmap structure of a document, by node index.
#[derive(Debug, Clone)]
pub struct Forest {
    pub parent: Vec<Option<usize>>,
    pub kids: Vec<Vec<usize>>,
    /// The map's centre, for nodes that belong to a mindmap.
    pub root: Vec<Option<usize>>,
    pub depth: Vec<usize>,
    /// Folded away under a collapsed topic.
    pub hidden: Vec<bool>,
    pub layout: Vec<MapLayout>,
    /// Branch colour name or `#hex` (None: the centre's ink).
    pub color: Vec<Option<String>>,
}

impl Forest {
    pub fn new(doc: &Document) -> Forest {
        let n = doc.nodes.len();
        let index: std::collections::HashMap<&str, usize> = doc
            .nodes
            .iter()
            .enumerate()
            .map(|(i, x)| (x.id.as_str(), i))
            .collect();
        let mut parent: Vec<Option<usize>> = doc
            .nodes
            .iter()
            .map(|x| x.parent.as_deref().and_then(|p| index.get(p).copied()))
            .collect();
        // break cycles: a node whose ancestry loops loses its parent
        for i in 0..n {
            let mut cur = parent[i];
            let mut steps = 0;
            while let Some(p) = cur {
                if p == i || steps > n {
                    parent[i] = None;
                    break;
                }
                cur = parent[p];
                steps += 1;
            }
        }
        let mut kids = vec![Vec::new(); n];
        for i in 0..n {
            if let Some(p) = parent[i] {
                kids[p].push(i);
            }
        }
        let mut root = vec![None; n];
        let mut depth = vec![0; n];
        for i in 0..n {
            let mut top = i;
            let mut d = 0;
            while let Some(p) = parent[top] {
                top = p;
                d += 1;
            }
            if doc.nodes[top].mindmap.is_some() || !kids[top].is_empty() {
                root[i] = Some(top);
                depth[i] = d;
            }
        }
        let mut hidden = vec![false; n];
        for i in 0..n {
            let mut cur = parent[i];
            while let Some(p) = cur {
                if doc.nodes[p].collapsed {
                    hidden[i] = true;
                    break;
                }
                cur = parent[p];
            }
        }
        let layout = (0..n)
            .map(|i| {
                root[i]
                    .and_then(|r| doc.nodes[r].mindmap)
                    .unwrap_or_default()
            })
            .collect();
        let mut color: Vec<Option<String>> = vec![None; n];
        for i in 0..n {
            if root[i].is_none() {
                continue;
            }
            // own colour, else the nearest coloured ancestor below the
            // centre, else the colour of the branch it grows from
            let mut cur = Some(i);
            let mut c = None;
            while let Some(k) = cur {
                if parent[k].is_none() {
                    break;
                }
                if let Some(own) = &doc.nodes[k].color {
                    c = Some(own.clone());
                    break;
                }
                if depth[k] == 1 {
                    let p = parent[k].unwrap();
                    let pos = kids[p].iter().position(|&x| x == k).unwrap_or(0);
                    c = Some(BRANCH_COLORS[pos % BRANCH_COLORS.len()].to_string());
                    break;
                }
                cur = parent[k];
            }
            if parent[i].is_none() {
                c = doc.nodes[i].color.clone();
            }
            color[i] = c;
        }
        Forest {
            parent,
            kids,
            root,
            depth,
            hidden,
            layout,
            color,
        }
    }

    pub fn in_map(&self, i: usize) -> bool {
        self.root[i].is_some()
    }

    /// Topics folded away under `i`.
    pub fn folded_count(&self, i: usize) -> usize {
        let mut count = 0;
        let mut stack: Vec<usize> = self.kids[i].clone();
        while let Some(k) = stack.pop() {
            count += 1;
            stack.extend(self.kids[k].iter().copied());
        }
        count
    }
}

/// How a mindmap topic is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// The central topic: a filled, bold pill.
    Centre,
    /// A main branch: an outlined card in the branch colour.
    Branch,
    /// A sub-topic: text on a coloured underline the branch runs into.
    Topic,
    /// A sub-topic in an org-chart (`down`) map: a small card.
    Card,
}

/// None when the node has an explicit shape (it is then drawn like any node).
pub fn style(n: &Node, depth: usize, layout: MapLayout) -> Option<Style> {
    if n.shape != Shape::Box {
        return None;
    }
    Some(match depth {
        0 => Style::Centre,
        1 => Style::Branch,
        _ if layout == MapLayout::Down => Style::Card,
        _ => Style::Topic,
    })
}

pub fn font_size(style: Option<Style>) -> f64 {
    match style {
        Some(Style::Centre) => 17.0,
        Some(Style::Branch) => 15.0,
        _ => LABEL_SIZE,
    }
}

/// Text lines and size of a mindmap topic.
pub fn measure(n: &Node, st: Option<Style>) -> (Vec<String>, Vec<String>, f64, f64) {
    let Some(st) = st else {
        return crate::layout::measure(n);
    };
    let size = font_size(Some(st));
    let (max, px, py, min_w) = match st {
        Style::Centre => (260.0, 26.0, 15.0, 120.0),
        Style::Branch => (240.0, 16.0, 10.0, 64.0),
        Style::Topic => (280.0, 6.0, 6.0, 24.0),
        Style::Card => (200.0, 12.0, 8.0, 64.0),
    };
    let max = n.w.map(|w| (w - 2.0 * px).max(20.0)).unwrap_or(max);
    let lines = text::wrap(n.display(), size, max);
    let detail = n
        .detail
        .as_deref()
        .map(|d| text::wrap(d, DETAIL_SIZE, max))
        .unwrap_or_default();
    let bold = if st == Style::Topic { 1.0 } else { 1.06 };
    let tw = lines
        .iter()
        .map(|l| text::width(l, size) * bold)
        .chain(detail.iter().map(|l| text::width(l, DETAIL_SIZE)))
        .fold(0.0, f64::max);
    let mut th = lines.len() as f64 * size * LINE_H;
    if !detail.is_empty() {
        th += 2.0 + detail.len() as f64 * DETAIL_SIZE * LINE_H;
    }
    let w = (tw + 2.0 * px).max(min_w).ceil();
    let h = (th + 2.0 * py).ceil() + if st == Style::Topic { 2.0 } else { 0.0 };
    (lines, detail, n.w.unwrap_or(w), n.h.unwrap_or(h))
}

// ------------------------------------------------------------------ layout

/// Place the visible topics of the map centred on `root`. Rects are
/// relative to the map's bounding box (whose top-left is 0,0).
pub fn layout_tree(f: &Forest, root: usize, sizes: &[(f64, f64)]) -> (Vec<(usize, Rect)>, Rect) {
    let lay = f.layout[root];
    let down = lay == MapLayout::Down;
    let cross = |i: usize| if down { sizes[i].0 } else { sizes[i].1 };
    let visible_kids = |i: usize| -> Vec<usize> {
        if f.kids[i].is_empty() || f.hidden.get(f.kids[i][0]).copied().unwrap_or(false) {
            Vec::new()
        } else {
            f.kids[i].clone()
        }
    };
    let sib_gap = |parent: usize| -> f64 {
        match (down, f.depth[parent]) {
            (true, 0) => 28.0,
            (true, _) => 16.0,
            (false, 0) => 22.0,
            (false, _) => 6.0,
        }
    };
    let level_gap = |parent: usize| -> f64 {
        match (down, f.depth[parent]) {
            (true, _) => 44.0,
            (false, 0) => 64.0,
            (false, _) => 36.0,
        }
    };

    // cross-axis span of every subtree
    let mut span = vec![0.0f64; sizes.len()];
    fn compute(
        i: usize,
        span: &mut Vec<f64>,
        kids: &dyn Fn(usize) -> Vec<usize>,
        cross: &dyn Fn(usize) -> f64,
        gap: &dyn Fn(usize) -> f64,
    ) -> f64 {
        let ks = kids(i);
        let mut total = 0.0;
        for (n, &k) in ks.iter().enumerate() {
            total += compute(k, span, kids, cross, gap);
            if n > 0 {
                total += gap(i);
            }
        }
        span[i] = cross(i).max(total);
        span[i]
    }
    compute(root, &mut span, &visible_kids, &cross, &sib_gap);

    let mut rects: Vec<(usize, Rect)> = Vec::new();
    // place `i` with its near edge at `near` (main axis) and centred on `c`
    #[allow(clippy::too_many_arguments)]
    fn place(
        i: usize,
        near: f64,
        c: f64,
        dir: f64,
        down: bool,
        sizes: &[(f64, f64)],
        span: &[f64],
        kids: &dyn Fn(usize) -> Vec<usize>,
        gap: &dyn Fn(usize) -> f64,
        level: &dyn Fn(usize) -> f64,
        out: &mut Vec<(usize, Rect)>,
    ) {
        let (w, h) = sizes[i];
        let r = if down {
            Rect {
                x: c - w / 2.0,
                y: near,
                w,
                h,
            }
        } else if dir > 0.0 {
            Rect {
                x: near,
                y: c - h / 2.0,
                w,
                h,
            }
        } else {
            Rect {
                x: near - w,
                y: c - h / 2.0,
                w,
                h,
            }
        };
        out.push((i, r));
        let ks = kids(i);
        if ks.is_empty() {
            return;
        }
        let next = if down {
            r.bottom() + level(i)
        } else if dir > 0.0 {
            r.right() + level(i)
        } else {
            r.x - level(i)
        };
        place_kids(
            &ks,
            next,
            c,
            dir,
            down,
            sizes,
            span,
            kids,
            gap,
            gap(i),
            level,
            out,
        );
    }
    #[allow(clippy::too_many_arguments)]
    fn place_kids(
        ks: &[usize],
        near: f64,
        c: f64,
        dir: f64,
        down: bool,
        sizes: &[(f64, f64)],
        span: &[f64],
        kids: &dyn Fn(usize) -> Vec<usize>,
        gap: &dyn Fn(usize) -> f64,
        g: f64,
        level: &dyn Fn(usize) -> f64,
        out: &mut Vec<(usize, Rect)>,
    ) {
        let total: f64 = ks.iter().map(|&k| span[k]).sum::<f64>() + g * (ks.len() as f64 - 1.0);
        let mut at = c - total / 2.0;
        for &k in ks {
            place(
                k,
                near,
                at + span[k] / 2.0,
                dir,
                down,
                sizes,
                span,
                kids,
                gap,
                level,
                out,
            );
            at += span[k] + g;
        }
    }

    let (w0, h0) = sizes[root];
    rects.push((
        root,
        Rect {
            x: 0.0,
            y: 0.0,
            w: w0,
            h: h0,
        },
    ));
    let ks = visible_kids(root);
    let g = sib_gap(root);
    let lg = level_gap(root);
    match lay {
        MapLayout::Down => place_kids(
            &ks,
            h0 + lg,
            w0 / 2.0,
            1.0,
            true,
            sizes,
            &span,
            &visible_kids,
            &sib_gap,
            g,
            &level_gap,
            &mut rects,
        ),
        MapLayout::Right => place_kids(
            &ks,
            w0 + lg,
            h0 / 2.0,
            1.0,
            false,
            sizes,
            &span,
            &visible_kids,
            &sib_gap,
            g,
            &level_gap,
            &mut rects,
        ),
        MapLayout::Left => place_kids(
            &ks,
            -lg,
            h0 / 2.0,
            -1.0,
            false,
            sizes,
            &span,
            &visible_kids,
            &sib_gap,
            g,
            &level_gap,
            &mut rects,
        ),
        MapLayout::Both => {
            let (right, left) = split_sides(&ks, &span, g);
            place_kids(
                &right,
                w0 + lg,
                h0 / 2.0,
                1.0,
                false,
                sizes,
                &span,
                &visible_kids,
                &sib_gap,
                g,
                &level_gap,
                &mut rects,
            );
            place_kids(
                &left,
                -lg,
                h0 / 2.0,
                -1.0,
                false,
                sizes,
                &span,
                &visible_kids,
                &sib_gap,
                g,
                &level_gap,
                &mut rects,
            );
        }
    }
    let bbox = rects
        .iter()
        .map(|(_, r)| *r)
        .reduce(|a, b| a.union(&b))
        .unwrap_or_default();
    for (_, r) in &mut rects {
        r.x -= bbox.x;
        r.y -= bbox.y;
    }
    (
        rects,
        Rect {
            x: 0.0,
            y: 0.0,
            w: bbox.w,
            h: bbox.h,
        },
    )
}

/// Balanced map: the first branches go right until about half the height
/// is used, the rest go left (both sides read top to bottom).
pub fn split_sides(ks: &[usize], span: &[f64], gap: f64) -> (Vec<usize>, Vec<usize>) {
    if ks.len() < 2 {
        return (ks.to_vec(), Vec::new());
    }
    let total: f64 = ks.iter().map(|&k| span[k] + gap).sum();
    let mut acc = 0.0;
    let mut cut = ks.len();
    for (n, &k) in ks.iter().enumerate() {
        let next = acc + span[k] + gap;
        if n > 0 && (next - total / 2.0) > (total / 2.0 - acc) {
            cut = n;
            break;
        }
        acc = next;
    }
    let cut = cut.clamp(1, ks.len() - 1);
    (ks[..cut].to_vec(), ks[cut..].to_vec())
}

// ------------------------------------------------------- outline in, out

/// The map under `id` as an indented outline (two spaces per level).
/// `with_root` includes `id` itself as the first line.
pub fn outline(doc: &Document, id: &str, with_root: bool) -> String {
    let mut out = String::new();
    fn walk(doc: &Document, id: &str, depth: usize, out: &mut String, seen: &mut Vec<String>) {
        if seen.iter().any(|s| s == id) {
            return;
        }
        seen.push(id.to_string());
        for c in doc.children(id) {
            out.push_str(&"  ".repeat(depth));
            out.push_str(&c.display().replace('\n', " "));
            out.push('\n');
            walk(doc, &c.id, depth + 1, out, seen);
        }
    }
    let mut seen = Vec::new();
    if with_root {
        if let Some(n) = doc.node(id) {
            out.push_str(&n.display().replace('\n', " "));
            out.push('\n');
        }
        walk(doc, id, 1, &mut out, &mut seen);
    } else {
        walk(doc, id, 0, &mut out, &mut seen);
    }
    out
}

/// The map under `id` as Markdown: the centre is the title, main branches
/// are sections and deeper topics are nested bullets. Details follow their
/// topic. This is how a mindmap becomes a prompt.
pub fn to_markdown(doc: &Document, id: &str) -> String {
    let Some(root) = doc.node(id) else {
        return String::new();
    };
    let mut out = format!("# {}\n", root.display().replace('\n', " "));
    if let Some(d) = &root.detail {
        out.push_str(&format!("\n{d}\n"));
    }
    fn walk(doc: &Document, id: &str, depth: usize, out: &mut String, seen: &mut Vec<String>) {
        if seen.iter().any(|s| s == id) {
            return;
        }
        seen.push(id.to_string());
        for c in doc.children(id) {
            let mut label = c.display().replace('\n', " ");
            if let Some(l) = &c.link {
                label = format!("[{label}]({l})");
            }
            if depth == 1 {
                out.push_str(&format!("\n## {label}\n"));
                if let Some(d) = &c.detail {
                    out.push_str(&format!("\n{d}\n"));
                }
                if !doc.children(&c.id).is_empty() {
                    out.push('\n');
                }
            } else {
                out.push_str(&"  ".repeat(depth - 2));
                out.push_str(&format!("- {label}"));
                if let Some(d) = &c.detail {
                    out.push_str(&format!(": {}", d.replace('\n', " ")));
                }
                out.push('\n');
            }
            walk(doc, &c.id, depth + 1, out, seen);
        }
    }
    let mut seen = Vec::new();
    walk(doc, id, 1, &mut out, &mut seen);
    out
}

/// Add an outline under `parent` (or as a new mindmap when `parent` is None).
/// Top-level lines become children of `parent`, starting at sibling position
/// `index` (end when None). A reply that repeats the parent as its single
/// top line is unwrapped. Returns the created ids.
pub fn insert_outline(
    doc: &mut Document,
    parent: Option<&str>,
    lines: &[Line],
    index: Option<usize>,
    at: Option<(f64, f64)>,
) -> Result<Vec<String>, String> {
    let mut lines: Vec<Line> = lines.to_vec();
    if lines.is_empty() {
        return Err("the outline is empty".into());
    }
    let min = lines.iter().map(|l| l.depth).min().unwrap_or(0);
    for l in &mut lines {
        l.depth -= min;
    }
    if let Some(p) = parent {
        let pn = doc.node(p).ok_or_else(|| format!("no node `{p}`"))?;
        let tops: Vec<&Line> = lines.iter().filter(|l| l.depth == 0).collect();
        let same = |t: &str| {
            let (label, _) = dsl::split_attrs(t);
            label.trim().eq_ignore_ascii_case(pn.display().trim())
        };
        if tops.len() == 1 && same(&tops[0].text) && lines.len() > 1 {
            lines.remove(0);
            for l in &mut lines {
                l.depth = l.depth.saturating_sub(1);
            }
        }
    }
    let mut created: Vec<String> = Vec::new();
    // last created node at each depth
    let mut chain: Vec<String> = Vec::new();
    let mut top_n = 0usize;
    for l in &lines {
        let (label, attrs) = split_label(&l.text);
        let id = doc.fresh_id(&slug(&label));
        let mut n = Node::new(&id);
        n.label = if label == id { String::new() } else { label };
        if let Some(a) = attrs {
            // attributes the model added are a bonus; bad ones are ignored
            let mut tmp = n.clone();
            if dsl::apply_node_attrs(&mut tmp, &a).is_ok() {
                n = tmp;
                n.id = id.clone();
            }
        }
        chain.truncate(l.depth);
        let depth = chain.len();
        let parent_id = if depth == 0 {
            parent.map(String::from)
        } else {
            chain.last().cloned()
        };
        match &parent_id {
            Some(p) => n.parent = Some(p.clone()),
            None => {
                n.mindmap = Some(MapLayout::default());
                if top_n == 0 {
                    if let Some((x, y)) = at {
                        n.x = Some(x.round());
                        n.y = Some(y.round());
                    }
                }
            }
        }
        doc.nodes.push(n);
        if depth == 0 {
            if let (Some(_), Some(i)) = (parent, index) {
                doc.move_among_siblings(&id, Some(i + top_n));
            }
            top_n += 1;
        }
        chain.push(id.clone());
        created.push(id);
    }
    Ok(created)
}

/// `Label [attrs]` → label and parsed attributes.
fn split_label(t: &str) -> (String, Option<Vec<(String, Option<String>)>>) {
    let (label, attrs) = dsl::split_attrs(t);
    let label = dsl::unescape_label(label.trim());
    let attrs = attrs.and_then(|a| dsl::parse_attr_list(a).ok());
    (label, attrs)
}

// ---------------------------------------------------------------- prompts

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ExpandOptions {
    /// What the person wants ("risks for each", "questions to ask legal").
    pub instructions: Option<String>,
    /// About how many new topics per level (default 3–6).
    pub count: Option<usize>,
    /// How many levels to add (default 1).
    pub levels: Option<usize>,
}

const REPLY_RULES: &str = "Reply with ONLY the new topics as an indented outline: one topic per line, two spaces per level, no bullets, no numbering, no commentary, no code fence. Keep each topic short (2–7 words) and write in the language the map is written in.";

/// A small prompt asking a model to grow the map at `id`. It carries only
/// what the model needs: the path from the centre, the siblings and what is
/// already there (the whole map when it is small).
pub fn expand_prompt(doc: &Document, id: &str, o: &ExpandOptions) -> Result<String, String> {
    let n = doc.node(id).ok_or_else(|| format!("no node `{id}`"))?;
    let root = doc.map_root(id).unwrap_or(n);
    let mut path: Vec<String> = doc
        .ancestors(id)
        .iter()
        .rev()
        .map(|a| a.display().replace('\n', " "))
        .collect();
    path.push(n.display().replace('\n', " "));
    let size = doc.subtree(&root.id).len();
    let mut s = String::from("You are helping someone build a mindmap.\n\n");
    if size <= 80 {
        s.push_str("The map so far:\n```\n");
        s.push_str(&outline(doc, &root.id, true));
        s.push_str("```\n\n");
    } else {
        s.push_str(&format!("Map: {}\n", root.display().replace('\n', " ")));
        if let Some(p) = doc.node(id).and_then(|n| n.parent.as_deref()) {
            let sib: Vec<String> = doc
                .children(p)
                .iter()
                .filter(|c| c.id != id)
                .map(|c| c.display().replace('\n', " "))
                .collect();
            if !sib.is_empty() {
                s.push_str(&format!("Next to it: {}\n", sib.join("; ")));
            }
        }
        let under = outline(doc, id, false);
        if !under.is_empty() {
            s.push_str(&format!("Already under it:\n```\n{under}```\n"));
        }
        s.push('\n');
    }
    s.push_str(&format!("Topic to grow: {}\n", path.join(" > ")));
    if let Some(d) = &n.detail {
        s.push_str(&format!("About it: {d}\n"));
    }
    let count = o
        .count
        .map(|c| c.to_string())
        .unwrap_or_else(|| "3–6".into());
    let levels = o.levels.unwrap_or(1).clamp(1, 4);
    s.push_str(&format!(
        "\nTask: add about {count} new subtopics under \"{}\"{}. Don't repeat what is already there.{}\n",
        n.display().replace('\n', " "),
        if levels > 1 {
            format!(", going up to {levels} levels deep")
        } else {
            String::new()
        },
        o.instructions
            .as_deref()
            .filter(|i| !i.trim().is_empty())
            .map(|i| format!("\nWhat the person wants: {}", i.trim()))
            .unwrap_or_default()
    ));
    s.push_str(REPLY_RULES);
    s.push('\n');
    Ok(s)
}

/// A prompt asking a model for a whole new mindmap about `topic`.
pub fn generate_prompt(topic: &str, o: &ExpandOptions) -> String {
    let levels = o.levels.unwrap_or(2).clamp(1, 4);
    format!(
        "Make a mindmap about: {}\n{}\nThe first line is the central topic (no indent); its main branches (4–7) are indented by two spaces, and their subtopics (2–5 each) by four{}.\n{}\n",
        topic.trim(),
        o.instructions
            .as_deref()
            .filter(|i| !i.trim().is_empty())
            .map(|i| format!("What the person wants: {}\n", i.trim()))
            .unwrap_or_default(),
        if levels > 2 {
            format!(", up to {levels} levels below the centre")
        } else {
            String::new()
        },
        REPLY_RULES.replace("the new topics", "the mindmap")
    )
}

/// Topics from a model's reply: the fenced block if there is one, read as
/// Markdown so headings, bullets and numbering all work.
pub fn parse_reply(text: &str) -> Vec<Line> {
    let body = fenced(text).unwrap_or(text);
    parse_outline(body, true)
}

fn fenced(text: &str) -> Option<&str> {
    let start = text.find("```")?;
    let rest = &text[start + 3..];
    let nl = rest.find('\n')?;
    let body = &rest[nl + 1..];
    let end = body.find("```")?;
    Some(&body[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outlines_from_markdown_and_indentation() {
        let ls = parse_outline("Root\n  A\n    a1\n  B\n", false);
        let d: Vec<usize> = ls.iter().map(|l| l.depth).collect();
        assert_eq!(d, vec![0, 1, 2, 1]);
        let md = "# Plan\n## Discovery\n- Who pays?\n  - Cards\n1. **Regulation**\n";
        let ls = parse_outline(md, true);
        let got: Vec<(usize, &str)> = ls.iter().map(|l| (l.depth, l.text.as_str())).collect();
        assert_eq!(
            got,
            vec![
                (0, "Plan"),
                (1, "Discovery"),
                (2, "Who pays?"),
                (3, "Cards"),
                (2, "Regulation")
            ]
        );
    }

    #[test]
    fn insert_unwraps_a_repeated_parent() {
        let mut d = crate::dsl::parse("mindmap m: Trip {\n  Food\n}").unwrap();
        let created = insert_outline(
            &mut d,
            Some("food"),
            &parse_reply("```\nFood\n  Pasta\n  Pizza\n```"),
            None,
            None,
        )
        .unwrap();
        assert_eq!(created, vec!["pasta", "pizza"]);
        assert_eq!(d.node("pizza").unwrap().parent.as_deref(), Some("food"));
    }

    #[test]
    fn balanced_maps_use_both_sides() {
        let d = crate::dsl::parse("mindmap m: M [layout: both] {\n  a\n  b\n  c\n  d\n}").unwrap();
        let l = crate::layout::layout(&d);
        let m = l.rect("m").unwrap();
        assert!(l.rect("a").unwrap().x > m.right());
        assert!(l.rect("d").unwrap().right() < m.x);
    }

    #[test]
    fn markdown_export_makes_a_prompt() {
        let d = crate::dsl::parse("mindmap p: Build checkout {\n  Context\n    Rails app\n  Constraints\n    No new services\n}").unwrap();
        let md = to_markdown(&d, "p");
        assert_eq!(md, "# Build checkout\n\n## Context\n\n- Rails app\n\n## Constraints\n\n- No new services\n");
    }
}
