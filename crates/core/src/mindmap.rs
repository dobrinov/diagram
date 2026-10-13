//! Mindmaps: a central topic with branches, laid out as a tidy tree.
//!
//! A mindmap is made of ordinary nodes: the centre carries `mindmap` (which
//! way it grows) and every branch points at its topic with `parent`. So
//! comments, ops, diffs and cross-links work on mindmaps like on anything
//! else. In the DSL a map is an indented outline:
//!
//! ```text
//! mindmap offsite: Team offsite [layout: both] {
//!   Venue
//!     Who books it?
//!   Budget [red]
//!     Travel and hotels
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
///
/// Every level of a side shares one column (one row in a `down` map), so
/// sibling branches line up whatever the width of their parents, and each
/// fan of sub-topics is centred on the point its branch leaves from.
/// `pins` are topics placed by hand, relative to the centre's top-left:
/// they keep their place, their branches follow them, and their siblings
/// close up behind them.
pub fn layout_tree(
    f: &Forest,
    root: usize,
    sizes: &[(f64, f64)],
    styles: &[Option<Style>],
    pins: &[Option<(f64, f64)>],
) -> (Vec<(usize, Rect)>, Rect) {
    let mut t = Tree {
        f,
        sizes,
        styles,
        pins,
        down: f.layout[root] == MapLayout::Down,
        ext: vec![(0.0, 0.0); sizes.len()],
        cols: [Vec::new(), Vec::new()],
        placed: vec![false; sizes.len()],
        out: Vec::new(),
    };
    t.compute_ext(root);
    let (w0, h0) = sizes[root];
    let centre = Rect {
        x: 0.0,
        y: 0.0,
        w: w0,
        h: h0,
    };
    t.out.push((root, centre));
    let ks = t.flow_kids(root);
    let sides: Vec<(Vec<usize>, f64)> = match f.layout[root] {
        MapLayout::Left => vec![(ks, -1.0)],
        MapLayout::Both => {
            let span: Vec<f64> = t.ext.iter().map(|(a, b)| a + b).collect();
            let (right, left) = split_sides(&ks, &span, t.sib_gap(root));
            vec![(right, 1.0), (left, -1.0)]
        }
        _ => vec![(ks, 1.0)],
    };
    for (ks, dir) in &sides {
        t.columns(root, ks, *dir);
        t.place_kids(root, centre, ks, *dir, false);
    }
    let bbox = t
        .out
        .iter()
        .map(|(_, r)| *r)
        .reduce(|a, b| a.union(&b))
        .unwrap_or_default();
    for (_, r) in &mut t.out {
        r.x -= bbox.x;
        r.y -= bbox.y;
    }
    (
        t.out,
        Rect {
            x: 0.0,
            y: 0.0,
            w: bbox.w,
            h: bbox.h,
        },
    )
}

struct Tree<'a> {
    f: &'a Forest,
    sizes: &'a [(f64, f64)],
    styles: &'a [Option<Style>],
    pins: &'a [Option<(f64, f64)>],
    down: bool,
    /// How far every subtree reaches before and after its topic's centre
    /// (cross axis).
    ext: Vec<(f64, f64)>,
    /// Near edge (main axis) of each depth's column, per side (right, left).
    cols: [Vec<f64>; 2],
    /// Topics placed by hand that are already laid out.
    placed: Vec<bool>,
    out: Vec<(usize, Rect)>,
}

impl Tree<'_> {
    fn pinned(&self, i: usize) -> bool {
        self.pins[i].is_some() && self.f.parent[i].is_some()
    }
    fn visible_kids(&self, i: usize) -> &[usize] {
        let ks = &self.f.kids[i];
        if ks.is_empty() || self.f.hidden[ks[0]] {
            &[]
        } else {
            ks
        }
    }
    /// Visible branches that take part in the tidy layout (not placed by hand).
    fn flow_kids(&self, i: usize) -> Vec<usize> {
        self.visible_kids(i)
            .iter()
            .copied()
            .filter(|&k| !self.pinned(k))
            .collect()
    }
    fn main(&self, i: usize) -> f64 {
        if self.down {
            self.sizes[i].1
        } else {
            self.sizes[i].0
        }
    }
    fn cross(&self, i: usize) -> f64 {
        if self.down {
            self.sizes[i].0
        } else {
            self.sizes[i].1
        }
    }
    fn sib_gap(&self, parent: usize) -> f64 {
        match (self.down, self.f.depth[parent]) {
            (true, 0) => 28.0,
            (true, _) => 16.0,
            (false, 0) => 22.0,
            (false, _) => 6.0,
        }
    }
    fn level_gap(&self, parent: usize) -> f64 {
        match (self.down, self.f.depth[parent]) {
            (true, _) => 44.0,
            (false, 0) => 64.0,
            (false, _) => 36.0,
        }
    }
    /// Where a branch meets topic `i`, from its centre (cross axis): a
    /// sub-topic is underlined, and its branches run along the underline.
    fn anchor(&self, i: usize) -> f64 {
        if !self.down && self.styles[i] == Some(Style::Topic) {
            self.sizes[i].1 / 2.0
        } else {
            0.0
        }
    }
    /// Centres of the branches `ks` relative to `i`'s centre (cross axis).
    /// The points where the branches arrive are spaced symmetrically around
    /// the point they leave `i` from: mirrored pairs of neighbours share the
    /// larger of their two gaps, so a middle branch runs straight out.
    fn fan(&self, i: usize, ks: &[usize]) -> Vec<f64> {
        // room each pair of neighbours needs, between their arrival points
        let need: Vec<f64> = ks
            .windows(2)
            .map(|w| {
                let (a, b) = (w[0], w[1]);
                self.ext[a].1 + self.sib_gap(i) + self.ext[b].0 + self.anchor(b) - self.anchor(a)
            })
            .collect();
        let mut at = 0.0;
        let mut points = vec![0.0];
        for j in 0..need.len() {
            at += need[j].max(need[need.len() - 1 - j]);
            points.push(at);
        }
        let shift = self.anchor(i) - at / 2.0;
        ks.iter()
            .zip(points)
            .map(|(&k, p)| p + shift - self.anchor(k))
            .collect()
    }
    fn compute_ext(&mut self, i: usize) {
        for k in self.visible_kids(i).to_vec() {
            self.compute_ext(k);
        }
        let half = self.cross(i) / 2.0;
        let ks = self.flow_kids(i);
        let cs = self.fan(i, &ks);
        self.ext[i] = ks.iter().zip(&cs).fold((half, half), |(b, a), (&k, &c)| {
            (b.max(self.ext[k].0 - c), a.max(c + self.ext[k].1))
        });
    }
    /// The columns of one side: the widest topic of each depth sets where
    /// the next depth starts.
    fn columns(&mut self, root: usize, ks: &[usize], dir: f64) {
        let mut widest: Vec<f64> = Vec::new();
        let mut gaps: Vec<f64> = vec![self.level_gap(root)];
        let mut stack: Vec<usize> = ks.to_vec();
        while let Some(i) = stack.pop() {
            let d = self.f.depth[i];
            if widest.len() < d {
                widest.resize(d, 0.0);
                gaps.resize(d + 1, 0.0);
            }
            widest[d - 1] = widest[d - 1].max(self.main(i));
            gaps[d] = self.level_gap(i);
            stack.extend(self.flow_kids(i));
        }
        let (w0, h0) = self.sizes[root];
        let start = if self.down {
            h0
        } else if dir > 0.0 {
            w0
        } else {
            0.0
        };
        let mut cols = vec![start];
        let mut at = start + dir * gaps[0];
        for (d, w) in widest.iter().enumerate() {
            cols.push(at);
            at += dir * (w + gaps[d + 1]);
        }
        self.cols[usize::from(dir < 0.0)] = cols;
    }
    /// `i`'s rect with its near edge at `near` (main axis), centred on `c`.
    fn rect(&self, i: usize, near: f64, c: f64, dir: f64) -> Rect {
        let (w, h) = self.sizes[i];
        if self.down {
            Rect {
                x: c - w / 2.0,
                y: near,
                w,
                h,
            }
        } else {
            Rect {
                x: if dir > 0.0 { near } else { near - w },
                y: c - h / 2.0,
                w,
                h,
            }
        }
    }
    /// Lay out the branches of `i` (drawn at `r`). `free` once a topic
    /// placed by hand is above them: the shared columns no longer apply.
    fn place_kids(&mut self, i: usize, r: Rect, ks: &[usize], dir: f64, free: bool) {
        if !ks.is_empty() {
            let d = self.f.depth[i] + 1;
            let near = match self.cols[usize::from(dir < 0.0)].get(d) {
                Some(&col) if !free => col,
                _ if self.down => r.bottom() + self.level_gap(i),
                _ if dir > 0.0 => r.right() + self.level_gap(i),
                _ => r.x - self.level_gap(i),
            };
            let c = if self.down { r.cx() } else { r.cy() };
            for (&k, kc) in ks.iter().zip(self.fan(i, ks)) {
                let kr = self.rect(k, near, c + kc, dir);
                self.out.push((k, kr));
                let kk = self.flow_kids(k);
                self.place_kids(k, kr, &kk, dir, free);
            }
        }
        for k in self.visible_kids(i).to_vec() {
            if self.pinned(k) {
                self.place_pinned(k, r, dir);
            }
        }
    }
    /// A topic placed by hand; its branches grow away from its parent.
    fn place_pinned(&mut self, k: usize, parent: Rect, dir: f64) {
        // the centre's are reached once per side
        if std::mem::replace(&mut self.placed[k], true) {
            return;
        }
        let (x, y) = self.pins[k].unwrap();
        let (w, h) = self.sizes[k];
        let r = Rect { x, y, w, h };
        let dir = if self.down {
            dir
        } else if r.cx() < parent.cx() {
            -1.0
        } else {
            1.0
        };
        self.out.push((k, r));
        let ks = self.flow_kids(k);
        self.place_kids(k, r, &ks, dir, true);
    }
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

/// The map under `id` as Markdown: the centre is the title and deeper topics
/// are nested bullets. When every main branch has something under it (topics
/// or a detail), main branches are sections instead (`## Context` …). Details
/// follow their topic. This is how a mindmap becomes a prompt.
pub fn to_markdown(doc: &Document, id: &str) -> String {
    let Some(root) = doc.node(id) else {
        return String::new();
    };
    let mut out = format!("# {}\n", root.display().replace('\n', " "));
    if let Some(d) = &root.detail {
        out.push_str(&format!("\n{d}\n"));
    }
    let branches = doc.children(id);
    let sections = !branches.is_empty()
        && branches
            .iter()
            .all(|c| c.detail.is_some() || !doc.children(&c.id).is_empty());
    let mut seen = vec![id.to_string()];
    if sections {
        for c in branches {
            seen.push(c.id.clone());
            out.push_str(&format!("\n## {}\n", topic_label(doc, c)));
            if let Some(d) = &c.detail {
                out.push_str(&format!("\n{d}\n"));
            }
            if !doc.children(&c.id).is_empty() {
                out.push('\n');
                bullets(doc, &c.id, 0, &mut out, &mut seen);
            }
        }
    } else if !branches.is_empty() {
        out.push('\n');
        bullets(doc, id, 0, &mut out, &mut seen);
    }
    out
}

/// A topic's label for Markdown: one line, linked when it has a link.
fn topic_label(doc: &Document, n: &crate::model::Node) -> String {
    crate::prompt::link_md(doc, &n.display().replace('\n', " "), n.link.as_deref())
}

/// The topics under `id` as nested Markdown bullets, from `depth`.
pub(crate) fn bullets(
    doc: &Document,
    id: &str,
    depth: usize,
    out: &mut String,
    seen: &mut Vec<String>,
) {
    for c in doc.children(id) {
        if seen.iter().any(|s| *s == c.id) {
            continue;
        }
        seen.push(c.id.clone());
        out.push_str(&"  ".repeat(depth));
        out.push_str(&format!("- {}", topic_label(doc, c)));
        if let Some(d) = &c.detail {
            out.push_str(&format!(": {}", d.replace('\n', " ")));
        }
        out.push('\n');
        bullets(doc, &c.id, depth + 1, out, seen);
    }
}

/// An id from a label's first words: `Who books the venue and when?` →
/// `who-books-the-venue`.
pub fn short_slug(label: &str) -> String {
    let full = slug(label);
    let mut out = String::new();
    for (i, w) in full.split('-').enumerate() {
        if i == 4 || out.len() + w.len() > 32 {
            break;
        }
        if !out.is_empty() {
            out.push('-');
        }
        out.push_str(w);
    }
    if out.is_empty() {
        full.chars().take(32).collect()
    } else {
        out
    }
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
        // `[id: x]` chooses the id (when free); otherwise a short one from
        // the label's first words
        let mut attrs = attrs.unwrap_or_default();
        let wanted = attrs
            .iter()
            .position(|(k, v)| k == "id" && v.is_some())
            .map(|i| attrs.remove(i).1.unwrap());
        let id = match wanted.filter(|w| dsl::is_ident(w) && !doc.id_taken(w)) {
            Some(w) => w,
            None => doc.fresh_id(&short_slug(&label)),
        };
        let mut n = Node::new(&id);
        n.label = if label == id { String::new() } else { label };
        // attributes the model added are a bonus; bad ones are left out one by one
        for a in &attrs {
            let mut tmp = n.clone();
            if dsl::apply_node_attrs(&mut tmp, &vec![a.clone()]).is_ok() {
                n = tmp;
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
    fn levels_share_a_column_and_fans_are_centred() {
        let d = crate::dsl::parse(
            "mindmap m: M {\n  a\n    a1\n    a2\n    a3\n  A much longer branch\n    b1\n}",
        )
        .unwrap();
        let l = crate::layout::layout(&d);
        let r = |id: &str| l.rect(id).unwrap();
        // sub-topics start in one column, whatever their branch's width
        assert_eq!(r("a1").x, r("b1").x);
        // the middle sub-topic's underline is level with its branch
        assert!((r("a2").bottom() - r("a").cy()).abs() < 0.5);
        assert!((r("b1").bottom() - r("a-much-longer-branch").cy()).abs() < 0.5);
    }

    #[test]
    fn a_middle_branch_runs_straight_out() {
        // the middle branch's sub-topics reach further up than down
        let d = crate::dsl::parse(
            "mindmap m: Payments France {\n  Payment Links\n  New GLs\n    Fulll\n    Sage\n  Unintegrated\n}",
        )
        .unwrap();
        let l = crate::layout::layout(&d);
        let r = |id: &str| l.rect(id).unwrap();
        assert!((r("new-gls").cy() - r("m").cy()).abs() < 0.5);
        // and the outer two sit at the same distance from the centre
        let up = r("m").cy() - r("payment-links").cy();
        let down = r("unintegrated").cy() - r("m").cy();
        assert!((up - down).abs() < 0.5, "{up} vs {down}");
    }

    #[test]
    fn topics_placed_by_hand_keep_their_place() {
        let src = "mindmap m: M [x: 0, y: 0] {\n  a [x: -300, y: 200]\n    a1\n  b\n}";
        let d = crate::dsl::parse(src).unwrap();
        assert_eq!(crate::dsl::parse(&crate::dsl::print(&d)).unwrap(), d);
        let l = crate::layout::layout(&d);
        let (m, a, a1) = (
            l.rect("m").unwrap(),
            l.rect("a").unwrap(),
            l.rect("a1").unwrap(),
        );
        // relative to the centre; its branches grow away from the centre
        assert_eq!((a.x - m.x, a.y - m.y), (-300.0, 200.0));
        assert!(a1.right() < a.x);
        assert_eq!(l.node("a").unwrap().map.as_ref().unwrap().side, "left");
        // the siblings close up: `b` takes the first slot
        let mut d2 = d.clone();
        crate::ops::apply(
            &mut d2,
            &crate::ops::parse_ops(r#"{"op":"unpin","id":"a"}"#).unwrap(),
        )
        .unwrap();
        let l2 = crate::layout::layout(&d2);
        assert!(l2.rect("a").unwrap().x > l2.rect("m").unwrap().right());
        assert!(l.rect("b").unwrap().y < l2.rect("b").unwrap().y);
    }

    #[test]
    fn markdown_export_makes_a_prompt() {
        let d = crate::dsl::parse("mindmap p: Build checkout {\n  Context\n    Rails app\n  Constraints\n    No new services\n}").unwrap();
        let md = to_markdown(&d, "p");
        assert_eq!(md, "# Build checkout\n\n## Context\n\n- Rails app\n\n## Constraints\n\n- No new services\n");
    }

    #[test]
    fn markdown_uses_sections_only_when_every_branch_has_content() {
        let d =
            crate::dsl::parse("mindmap p: Groceries {\n  Milk\n  Fruit\n    Apples\n}").unwrap();
        assert_eq!(
            to_markdown(&d, "p"),
            "# Groceries\n\n- Milk\n- Fruit\n  - Apples\n"
        );
        let d = crate::dsl::parse("mindmap p: Solo {\n}").unwrap();
        assert_eq!(to_markdown(&d, "p"), "# Solo\n");
    }
}
