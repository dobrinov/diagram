//! Placing nodes and groups.
//!
//! Groups are laid out recursively: a group's members are arranged first, and
//! the group then takes part in its parent's layout as one block, so groups
//! never overlap each other or foreign nodes. Inside a block the layered
//! layout is a compact Sugiyama: break cycles, rank by longest path, order
//! layers by barycenter sweeps, then pull nodes towards their neighbours.
//! Pinned nodes (`x`/`y`) keep their position; their groups grow to fit.
use std::collections::HashMap;

use serde::Serialize;

use crate::mindmap::{self, Forest};
use crate::model::*;
use crate::text;

pub const LABEL_SIZE: f64 = 14.0;
pub const DETAIL_SIZE: f64 = 12.0;
pub const LINE_H: f64 = 1.3;
const NODE_GAP: f64 = 36.0;
const LAYER_GAP: f64 = 64.0;
const GROUP_PAD: f64 = 24.0;
/// Room above a group's members for its title pill.
const GROUP_HEAD: f64 = 56.0;
/// Left inset of a group's title pill.
pub const GROUP_PILL_X: f64 = 16.0;

/// Width a group needs for its title pill (bold 14px label + padding).
pub fn group_label_w(label: &str) -> f64 {
    text::width(label, 14.0) * 1.08 + 28.0 + 2.0 * GROUP_PILL_X
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn cx(&self) -> f64 {
        self.x + self.w / 2.0
    }
    pub fn cy(&self) -> f64 {
        self.y + self.h / 2.0
    }
    pub fn right(&self) -> f64 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f64 {
        self.y + self.h
    }
    pub fn union(&self, o: &Rect) -> Rect {
        let x = self.x.min(o.x);
        let y = self.y.min(o.y);
        Rect {
            x,
            y,
            w: self.right().max(o.right()) - x,
            h: self.bottom().max(o.bottom()) - y,
        }
    }
    pub fn grow(&self, d: f64) -> Rect {
        Rect {
            x: self.x - d,
            y: self.y - d,
            w: self.w + 2.0 * d,
            h: self.h + 2.0 * d,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeBox {
    pub id: String,
    #[serde(flatten)]
    pub rect: Rect,
    #[serde(skip)]
    pub lines: Vec<String>,
    #[serde(skip)]
    pub detail: Vec<String>,
    #[serde(skip)]
    pub shape: Shape,
    /// Label font size.
    #[serde(skip)]
    pub font: f64,
    /// Mindmap topics: the map's centre, depth, branch colour and the side
    /// of its parent it grows on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub map: Option<MapInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MapInfo {
    pub root: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    pub depth: usize,
    /// "right", "left" or "down": where this topic sits relative to its parent.
    pub side: &'static str,
    /// The map's layout.
    pub layout: MapLayout,
    /// Topics folded away under this one.
    pub folded: usize,
    pub children: usize,
    #[serde(skip)]
    pub style: Option<crate::mindmap::Style>,
    #[serde(skip)]
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GroupBox {
    pub id: String,
    #[serde(flatten)]
    pub rect: Rect,
    pub depth: usize,
}

#[derive(Debug, Clone, Default)]
pub struct Layout {
    pub nodes: Vec<NodeBox>,
    pub groups: Vec<GroupBox>,
    /// Bounding box of nodes and groups.
    pub bounds: Rect,
}

impl Layout {
    pub fn rect(&self, id: &str) -> Option<Rect> {
        self.nodes
            .iter()
            .find(|n| n.id == id)
            .map(|n| n.rect)
            .or_else(|| self.groups.iter().find(|g| g.id == id).map(|g| g.rect))
    }
    pub fn node(&self, id: &str) -> Option<&NodeBox> {
        self.nodes.iter().find(|n| n.id == id)
    }
}

/// Text lines and natural size of a node.
pub fn measure(n: &Node) -> (Vec<String>, Vec<String>, f64, f64) {
    let max = match n.shape {
        Shape::Sticky => 150.0,
        Shape::Text => 260.0,
        Shape::Circle | Shape::Diamond => 130.0,
        _ => 200.0,
    };
    let max = n.w.map(|w| (w - 24.0).max(20.0)).unwrap_or(max);
    let lines = text::wrap(n.display(), LABEL_SIZE, max);
    let detail = n
        .detail
        .as_deref()
        .map(|d| text::wrap(d, DETAIL_SIZE, max))
        .unwrap_or_default();
    let tw = lines
        .iter()
        .map(|l| text::width(l, LABEL_SIZE))
        .chain(detail.iter().map(|l| text::width(l, DETAIL_SIZE)))
        .fold(0.0, f64::max);
    let mut th = lines.len() as f64 * LABEL_SIZE * LINE_H;
    if !detail.is_empty() {
        th += 4.0 + detail.len() as f64 * DETAIL_SIZE * LINE_H;
    }
    let (px, py) = (18.0, 12.0);
    let (mut w, mut h) = ((tw + 2.0 * px).max(96.0), (th + 2.0 * py).max(44.0));
    match n.shape {
        Shape::Circle => {
            let d = (w.max(h) * 1.05).max(72.0);
            (w, h) = (d, d);
        }
        Shape::Diamond => (w, h) = ((tw + 2.0 * px) * 1.6, (th + 2.0 * py) * 1.7),
        Shape::Hexagon | Shape::Parallelogram => w += 28.0,
        Shape::Cylinder => h += 16.0,
        Shape::Document => h += 10.0,
        Shape::Cloud => (w, h) = (w * 1.35, h * 1.5),
        Shape::Person => (w, h) = ((tw + 12.0).max(64.0), 50.0 + th + 6.0),
        Shape::Sticky => (w, h) = ((tw + 28.0).max(150.0), (th + 28.0).max(110.0)),
        Shape::Text => (w, h) = (tw + 8.0, th + 8.0),
        _ => {}
    }
    (
        lines,
        detail,
        n.w.unwrap_or(w).round(),
        n.h.unwrap_or(h).round(),
    )
}

#[derive(Clone, Copy, PartialEq)]
enum Item {
    Node(usize),
    Group(usize),
}

struct Ctx<'a> {
    doc: &'a Document,
    /// Mindmap topics other than a centre don't take part in block layout:
    /// their map's centre stands in for the whole map.
    forest: &'a Forest,
    sizes: Vec<(f64, f64)>,
    group_sizes: Vec<(f64, f64)>,
    /// Position of each node / group relative to its parent block.
    node_pos: Vec<(f64, f64)>,
    group_pos: Vec<(f64, f64)>,
    group_index: HashMap<&'a str, usize>,
    node_index: HashMap<&'a str, usize>,
}

pub fn layout(doc: &Document) -> Layout {
    let forest = Forest::new(doc);
    let mut sizes = Vec::new();
    let mut texts = Vec::new();
    let mut styles = Vec::new();
    for (i, n) in doc.nodes.iter().enumerate() {
        let st = forest
            .in_map(i)
            .then(|| mindmap::style(n, forest.depth[i], forest.layout[i]))
            .flatten();
        let (lines, detail, w, h) = if forest.in_map(i) {
            mindmap::measure(n, st)
        } else {
            measure(n)
        };
        sizes.push((w, h));
        texts.push((lines, detail));
        styles.push(st);
    }
    // each mindmap is laid out on its own; its centre then stands for the
    // whole map (sized to it) in the layout of the rest
    let own_sizes = sizes.clone();
    let mut map_rel: Vec<Option<Rect>> = vec![None; doc.nodes.len()];
    for r in 0..doc.nodes.len() {
        if forest.root[r] == Some(r) {
            let (rects, bbox) = mindmap::layout_tree(&forest, r, &own_sizes);
            for (i, rect) in rects {
                map_rel[i] = Some(rect);
            }
            sizes[r] = (bbox.w, bbox.h);
        }
    }
    let mut ctx = Ctx {
        doc,
        forest: &forest,
        sizes,
        group_sizes: vec![(0.0, 0.0); doc.groups.len()],
        node_pos: vec![(0.0, 0.0); doc.nodes.len()],
        group_pos: vec![(0.0, 0.0); doc.groups.len()],
        group_index: doc
            .groups
            .iter()
            .enumerate()
            .map(|(i, g)| (g.id.as_str(), i))
            .collect(),
        node_index: doc
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.as_str(), i))
            .collect(),
    };
    // nodes in missing groups and groups with missing / cyclic parents are
    // treated as top level so nothing disappears
    let parent_of_node: Vec<Option<usize>> = doc
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let n = forest.root[i].map(|r| &doc.nodes[r]).unwrap_or(n);
            n.group
                .as_deref()
                .and_then(|g| ctx.group_index.get(g).copied())
        })
        .collect();
    let parent_of_group: Vec<Option<usize>> = doc
        .groups
        .iter()
        .map(|g| {
            g.parent
                .as_deref()
                .and_then(|p| ctx.group_index.get(p).copied())
                .filter(|_| {
                    !g.parent
                        .as_deref()
                        .is_some_and(|p| doc.group_chain(p).contains(&g.id))
                })
        })
        .collect();
    block(&mut ctx, None, &parent_of_node, &parent_of_group);

    // absolute positions
    let mut abs_group: Vec<Option<Rect>> = vec![None; doc.groups.len()];
    fn abs_of_group(
        i: usize,
        ctx: &Ctx,
        parents: &[Option<usize>],
        memo: &mut Vec<Option<Rect>>,
    ) -> Rect {
        if let Some(r) = memo[i] {
            return r;
        }
        let (rx, ry) = ctx.group_pos[i];
        let (ox, oy) = match parents[i] {
            Some(p) => {
                let pr = abs_of_group(p, ctx, parents, memo);
                (pr.x + GROUP_PAD, pr.y + GROUP_HEAD)
            }
            None => (0.0, 0.0),
        };
        let (w, h) = ctx.group_sizes[i];
        let r = Rect {
            x: ox + rx,
            y: oy + ry,
            w,
            h,
        };
        memo[i] = Some(r);
        r
    }
    for i in 0..doc.groups.len() {
        abs_of_group(i, &ctx, &parent_of_group, &mut abs_group);
    }
    let mut nodes: Vec<NodeBox> = Vec::new();
    // top-left of every map's bounding box
    let mut map_origin: Vec<(f64, f64)> = vec![(0.0, 0.0); doc.nodes.len()];
    for (i, n) in doc.nodes.iter().enumerate() {
        let (ox, oy) = match parent_of_node[i] {
            Some(g) => {
                let r = abs_group[g].unwrap();
                (r.x + GROUP_PAD, r.y + GROUP_HEAD)
            }
            None => (0.0, 0.0),
        };
        let (mut x, mut y) = (ox + ctx.node_pos[i].0, oy + ctx.node_pos[i].1);
        if forest.root[i] == Some(i) {
            // a pinned centre pins its map
            let rel = map_rel[i].unwrap_or_default();
            if let (Some(px), Some(py)) = (n.x, n.y) {
                (x, y) = (px - rel.x, py - rel.y);
            }
            map_origin[i] = (x, y);
        }
    }
    for (i, n) in doc.nodes.iter().enumerate() {
        let (lines, detail) = std::mem::take(&mut texts[i]);
        let mut map = None;
        let rect = if let Some(r) = forest.root[i] {
            let Some(rel) = map_rel[i] else {
                continue; // folded away
            };
            let side = match (forest.layout[i], forest.parent[i]) {
                (MapLayout::Down, _) => "down",
                (_, Some(p)) => {
                    if map_rel[p].is_some_and(|pr| rel.cx() < pr.cx()) {
                        "left"
                    } else {
                        "right"
                    }
                }
                (MapLayout::Left, None) => "left",
                _ => "right",
            };
            map = Some(MapInfo {
                root: doc.nodes[r].id.clone(),
                parent: forest.parent[i].map(|p| doc.nodes[p].id.clone()),
                depth: forest.depth[i],
                side,
                layout: forest.layout[i],
                folded: if n.collapsed {
                    forest.folded_count(i)
                } else {
                    0
                },
                children: forest.kids[i].len(),
                style: styles[i],
                color: forest.color[i].clone(),
            });
            let (mx, my) = map_origin[r];
            Rect {
                x: mx + rel.x,
                y: my + rel.y,
                w: rel.w,
                h: rel.h,
            }
        } else {
            let (ox, oy) = match parent_of_node[i] {
                Some(g) => {
                    let r = abs_group[g].unwrap();
                    (r.x + GROUP_PAD, r.y + GROUP_HEAD)
                }
                None => (0.0, 0.0),
            };
            let (w, h) = ctx.sizes[i];
            let (mut x, mut y) = (ox + ctx.node_pos[i].0, oy + ctx.node_pos[i].1);
            if let (Some(px), Some(py)) = (n.x, n.y) {
                (x, y) = (px, py);
            }
            Rect { x, y, w, h }
        };
        nodes.push(NodeBox {
            id: n.id.clone(),
            rect,
            lines,
            detail,
            shape: n.shape,
            font: mindmap::font_size(styles[i]),
            map,
        });
    }

    // groups fit their (possibly pinned) members, deepest first
    let depth = |i: usize| {
        let mut d = 0;
        let mut cur = parent_of_group[i];
        while let Some(p) = cur {
            d += 1;
            cur = parent_of_group[p];
        }
        d
    };
    let mut order: Vec<usize> = (0..doc.groups.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(depth(i)));
    let mut rects: Vec<Rect> = abs_group.iter().map(|r| r.unwrap()).collect();
    if doc.nodes.iter().any(|n| n.pinned() && n.parent.is_none()) {
        for &g in &order {
            let mut bb: Option<Rect> = None;
            for nb in &nodes {
                let i = ctx.node_index[nb.id.as_str()];
                if parent_of_node[i] == Some(g) {
                    bb = Some(bb.map_or(nb.rect, |b| b.union(&nb.rect)));
                }
            }
            for (c, p) in parent_of_group.iter().enumerate() {
                if *p == Some(g) {
                    bb = Some(bb.map_or(rects[c], |b| b.union(&rects[c])));
                }
            }
            if let Some(b) = bb {
                let label_w = group_label_w(doc.groups[g].display());
                rects[g] = Rect {
                    x: b.x - GROUP_PAD,
                    y: b.y - GROUP_HEAD,
                    w: (b.w + 2.0 * GROUP_PAD).max(label_w),
                    h: b.h + GROUP_HEAD + GROUP_PAD,
                };
            }
        }
    }
    let groups: Vec<GroupBox> = doc
        .groups
        .iter()
        .enumerate()
        .map(|(i, g)| GroupBox {
            id: g.id.clone(),
            rect: rects[i],
            depth: depth(i),
        })
        .collect();
    let bounds = nodes
        .iter()
        .map(|n| n.rect)
        .chain(groups.iter().map(|g| g.rect))
        .reduce(|a, b| a.union(&b))
        .unwrap_or_default();
    Layout {
        nodes,
        groups,
        bounds,
    }
}

/// Lay out the members of `group` (None = top level). Returns the content size
/// and records member positions relative to the content origin.
fn block(
    ctx: &mut Ctx,
    group: Option<usize>,
    pn: &[Option<usize>],
    pg: &[Option<usize>],
) -> (f64, f64) {
    let mut items: Vec<Item> = Vec::new();
    for (i, p) in pn.iter().enumerate() {
        if *p == group && ctx.forest.root[i].is_none_or(|r| r == i) {
            items.push(Item::Node(i));
        }
    }
    for (i, p) in pg.iter().enumerate() {
        if *p == group {
            let (w, h) = block(ctx, Some(i), pn, pg);
            let label_w = group_label_w(ctx.doc.groups[i].display());
            let (gw, gh) = if w == 0.0 {
                (label_w.max(180.0), GROUP_HEAD + 40.0)
            } else {
                (
                    (w + 2.0 * GROUP_PAD).max(label_w),
                    h + GROUP_HEAD + GROUP_PAD,
                )
            };
            ctx.group_sizes[i] = (gw, gh);
            items.push(Item::Group(i));
        }
    }
    if items.is_empty() {
        return (0.0, 0.0);
    }
    let size = |ctx: &Ctx, it: Item| match it {
        Item::Node(i) => ctx.sizes[i],
        Item::Group(i) => ctx.group_sizes[i],
    };
    // which item of this block contains a node or group id
    let item_of = |ctx: &Ctx, id: &str| -> Option<usize> {
        let mut cur = if let Some(&n) = ctx.node_index.get(id) {
            let n = ctx.forest.root[n].unwrap_or(n);
            if pn[n] == group {
                return items.iter().position(|it| *it == Item::Node(n));
            }
            pn[n]
        } else {
            Some(*ctx.group_index.get(id)?)
        };
        while let Some(g) = cur {
            if pg[g] == group {
                return items.iter().position(|it| *it == Item::Group(g));
            }
            cur = pg[g];
        }
        None
    };
    let mut edges: Vec<(usize, usize, f64)> = Vec::new();
    for e in &ctx.doc.edges {
        if let (Some(a), Some(b)) = (item_of(ctx, &e.from), item_of(ctx, &e.to)) {
            if a != b && !edges.iter().any(|(x, y, _)| (*x, *y) == (a, b)) {
                let lw = e
                    .label
                    .as_deref()
                    .map(|l| {
                        text::wrap(l, 12.0, 160.0)
                            .iter()
                            .map(|s| text::width(s, 12.0))
                            .fold(0.0, f64::max)
                    })
                    .unwrap_or(0.0);
                edges.push((a, b, lw));
            }
        }
    }
    let sizes: Vec<(f64, f64)> = items.iter().map(|&it| size(ctx, it)).collect();
    let pos = match ctx.doc.settings.layout {
        LayoutKind::Grid => grid(&sizes, &(0..items.len()).collect::<Vec<_>>()),
        LayoutKind::Layered => layered(&sizes, &edges, ctx.doc.settings.direction),
    };
    let (mut w, mut h) = (0.0f64, 0.0f64);
    for (k, it) in items.iter().enumerate() {
        let (x, y) = pos[k];
        w = w.max(x + sizes[k].0);
        h = h.max(y + sizes[k].1);
        match *it {
            Item::Node(i) => ctx.node_pos[i] = (x, y),
            Item::Group(i) => ctx.group_pos[i] = (x, y),
        }
    }
    (w, h)
}

fn grid(sizes: &[(f64, f64)], which: &[usize]) -> Vec<(f64, f64)> {
    let mut out = vec![(0.0, 0.0); sizes.len()];
    if which.is_empty() {
        return out;
    }
    let cols = (which.len() as f64).sqrt().ceil() as usize;
    let rows = which.len().div_ceil(cols);
    let mut cw = vec![0.0f64; cols];
    let mut rh = vec![0.0f64; rows];
    for (k, &i) in which.iter().enumerate() {
        cw[k % cols] = cw[k % cols].max(sizes[i].0);
        rh[k / cols] = rh[k / cols].max(sizes[i].1);
    }
    for (k, &i) in which.iter().enumerate() {
        let (c, r) = (k % cols, k / cols);
        let x: f64 =
            cw[..c].iter().sum::<f64>() + c as f64 * NODE_GAP * 1.5 + (cw[c] - sizes[i].0) / 2.0;
        let y: f64 = rh[..r].iter().sum::<f64>() + r as f64 * NODE_GAP + (rh[r] - sizes[i].1) / 2.0;
        out[i] = (x, y);
    }
    out
}

fn layered(sizes: &[(f64, f64)], edges: &[(usize, usize, f64)], dir: Direction) -> Vec<(f64, f64)> {
    let n = sizes.len();
    let horizontal = matches!(dir, Direction::Right | Direction::Left);
    // main axis = flow direction, cross = across layers
    let main = |i: usize| if horizontal { sizes[i].0 } else { sizes[i].1 };
    let cross = |i: usize| if horizontal { sizes[i].1 } else { sizes[i].0 };

    let connected: Vec<bool> = (0..n)
        .map(|i| edges.iter().any(|(a, b, _)| *a == i || *b == i))
        .collect();
    let isolated: Vec<usize> = (0..n).filter(|&i| !connected[i]).collect();

    // 1. break cycles: DFS in declaration order; back edges are reversed
    let mut out_adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (a, b, _) in edges {
        out_adj[*a].push(*b);
    }
    let mut state = vec![0u8; n]; // 0 new, 1 on stack, 2 done
    let mut dag: Vec<(usize, usize)> = Vec::new();
    fn dfs(u: usize, adj: &[Vec<usize>], state: &mut [u8], dag: &mut Vec<(usize, usize)>) {
        state[u] = 1;
        for &v in &adj[u] {
            match state[v] {
                0 => {
                    dag.push((u, v));
                    dfs(v, adj, state, dag);
                }
                1 => dag.push((v, u)),
                _ => dag.push((u, v)),
            }
        }
        state[u] = 2;
    }
    for u in 0..n {
        if connected[u] && state[u] == 0 {
            dfs(u, &out_adj, &mut state, &mut dag);
        }
    }

    // 2. ranks: longest path, then sources move next to their successors
    let mut rank = vec![0usize; n];
    let mut changed = true;
    let mut guard = 0;
    while changed && guard < n + 2 {
        changed = false;
        guard += 1;
        for &(a, b) in &dag {
            if rank[b] < rank[a] + 1 {
                rank[b] = rank[a] + 1;
                changed = true;
            }
        }
    }
    for u in 0..n {
        let has_in = dag.iter().any(|&(_, b)| b == u);
        if connected[u] && !has_in {
            if let Some(m) = dag
                .iter()
                .filter(|&&(a, _)| a == u)
                .map(|&(_, b)| rank[b])
                .min()
            {
                rank[u] = m.saturating_sub(1);
            }
        }
    }
    let layers_n = (0..n)
        .filter(|&i| connected[i])
        .map(|i| rank[i] + 1)
        .max()
        .unwrap_or(0);
    let mut layers: Vec<Vec<usize>> = vec![Vec::new(); layers_n];
    for i in 0..n {
        if connected[i] {
            layers[rank[i]].push(i);
        }
    }

    // 3. order: barycenter sweeps
    let neighbours = |u: usize| -> Vec<usize> {
        dag.iter()
            .filter_map(|&(a, b)| {
                if a == u {
                    Some(b)
                } else if b == u {
                    Some(a)
                } else {
                    None
                }
            })
            .collect()
    };
    let nb: Vec<Vec<usize>> = (0..n).map(neighbours).collect();
    let mut order_pos = vec![0.0f64; n];
    let set_pos = |layers: &Vec<Vec<usize>>, order_pos: &mut Vec<f64>| {
        for l in layers {
            for (k, &u) in l.iter().enumerate() {
                order_pos[u] = k as f64;
            }
        }
    };
    set_pos(&layers, &mut order_pos);
    for sweep in 0..8 {
        let down = sweep % 2 == 0;
        let range: Vec<usize> = if down {
            (1..layers_n).collect()
        } else {
            (0..layers_n.saturating_sub(1)).rev().collect()
        };
        for r in range {
            let adj_rank = if down { r - 1 } else { r + 1 };
            let mut keyed: Vec<(f64, usize)> = layers[r]
                .iter()
                .map(|&u| {
                    let ps: Vec<f64> = nb[u]
                        .iter()
                        .filter(|&&v| rank[v] == adj_rank)
                        .map(|&v| order_pos[v])
                        .collect();
                    let key = if ps.is_empty() {
                        order_pos[u]
                    } else {
                        ps.iter().sum::<f64>() / ps.len() as f64
                    };
                    (key, u)
                })
                .collect();
            keyed.sort_by(|a, b| {
                a.0.partial_cmp(&b.0)
                    .unwrap()
                    .then(order_pos[a.1].partial_cmp(&order_pos[b.1]).unwrap())
            });
            layers[r] = keyed.into_iter().map(|(_, u)| u).collect();
            for (k, &u) in layers[r].iter().enumerate() {
                order_pos[u] = k as f64;
            }
        }
    }

    // 4. cross coordinates: pack, then pull towards neighbours
    let mut c = vec![0.0f64; n];
    for l in &layers {
        let mut at = 0.0;
        for &u in l {
            c[u] = at;
            at += cross(u) + NODE_GAP;
        }
    }
    for l in &layers {
        // centre each layer on the widest one
        let w: f64 = l.iter().map(|&u| cross(u) + NODE_GAP).sum::<f64>() - NODE_GAP;
        let widest = layers
            .iter()
            .map(|l| l.iter().map(|&u| cross(u) + NODE_GAP).sum::<f64>() - NODE_GAP)
            .fold(0.0, f64::max);
        for &u in l {
            c[u] += (widest - w) / 2.0;
        }
    }
    for it in 0..12 {
        let down = it % 2 == 0;
        let range: Vec<usize> = if down {
            (0..layers_n).collect()
        } else {
            (0..layers_n).rev().collect()
        };
        for r in range {
            let l = &layers[r];
            if l.is_empty() {
                continue;
            }
            let desired: Vec<f64> = l
                .iter()
                .map(|&u| {
                    let ps: Vec<f64> = nb[u]
                        .iter()
                        .filter(|&&v| rank[v] != rank[u])
                        .map(|&v| c[v] + cross(v) / 2.0)
                        .collect();
                    if ps.is_empty() {
                        c[u] + cross(u) / 2.0
                    } else {
                        ps.iter().sum::<f64>() / ps.len() as f64
                    }
                })
                .collect();
            let mut pos: Vec<f64> = Vec::with_capacity(l.len());
            for (k, &u) in l.iter().enumerate() {
                let want = desired[k] - cross(u) / 2.0;
                let min = if k == 0 {
                    f64::NEG_INFINITY
                } else {
                    pos[k - 1] + cross(l[k - 1]) + NODE_GAP
                };
                pos.push(want.max(min));
            }
            let shift: f64 = l
                .iter()
                .enumerate()
                .map(|(k, &u)| desired[k] - (pos[k] + cross(u) / 2.0))
                .sum::<f64>()
                / l.len() as f64;
            for (k, &u) in l.iter().enumerate() {
                c[u] = pos[k] + shift;
            }
        }
    }
    let min_c = (0..n)
        .filter(|&i| connected[i])
        .map(|i| c[i])
        .fold(f64::INFINITY, f64::min);
    let min_c = if min_c.is_finite() { min_c } else { 0.0 };

    // 5. main coordinates: layer thickness + gaps wide enough for labels
    let thick: Vec<f64> = layers
        .iter()
        .map(|l| l.iter().map(|&u| main(u)).fold(0.0, f64::max))
        .collect();
    let mut offset = vec![0.0f64; layers_n];
    for r in 1..layers_n {
        let label = edges
            .iter()
            .filter(|(a, b, _)| rank[*a].min(rank[*b]) == r - 1 && rank[*a] != rank[*b])
            .map(|(_, _, w)| *w)
            .fold(0.0, f64::max);
        let gap = if horizontal {
            LAYER_GAP.max(label + 40.0).min(240.0)
        } else {
            LAYER_GAP + if label > 0.0 { 16.0 } else { 0.0 }
        };
        offset[r] = offset[r - 1] + thick[r - 1] + gap;
    }
    let total_main = if layers_n > 0 {
        offset[layers_n - 1] + thick[layers_n - 1]
    } else {
        0.0
    };
    let mut out = vec![(0.0, 0.0); n];
    let mut cross_extent = 0.0f64;
    for i in 0..n {
        if !connected[i] {
            continue;
        }
        let r = rank[i];
        let mut m = offset[r] + (thick[r] - main(i)) / 2.0;
        if matches!(dir, Direction::Left | Direction::Up) {
            m = total_main - m - main(i);
        }
        let cc = c[i] - min_c;
        cross_extent = cross_extent.max(cc + cross(i));
        out[i] = if horizontal { (m, cc) } else { (cc, m) };
    }

    // 6. unconnected items: a row (or column) after the flow
    if !isolated.is_empty() {
        let start = if layers_n > 0 {
            cross_extent + NODE_GAP * 1.5
        } else {
            0.0
        };
        let per_line = if layers_n > 0 {
            ((total_main + NODE_GAP)
                / (isolated.iter().map(|&i| main(i)).fold(0.0, f64::max) + NODE_GAP))
                .floor()
                .max(1.0) as usize
        } else {
            (isolated.len() as f64).sqrt().ceil() as usize
        };
        let mut m_at = 0.0;
        let mut c_at = start;
        let mut line_thick = 0.0f64;
        for (k, &i) in isolated.iter().enumerate() {
            if k > 0 && k % per_line == 0 {
                m_at = 0.0;
                c_at += line_thick + NODE_GAP;
                line_thick = 0.0;
            }
            out[i] = if horizontal {
                (m_at, c_at)
            } else {
                (c_at, m_at)
            };
            m_at += main(i) + NODE_GAP;
            line_thick = line_thick.max(cross(i));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl;

    fn overlaps(a: &Rect, b: &Rect) -> bool {
        a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom()
    }

    #[test]
    fn flows_in_the_requested_direction() {
        let d = dsl::parse("a -> b -> c").unwrap();
        let l = layout(&d);
        let (a, b, c) = (
            l.rect("a").unwrap(),
            l.rect("b").unwrap(),
            l.rect("c").unwrap(),
        );
        assert!(a.right() < b.x && b.right() < c.x, "{a:?} {b:?} {c:?}");
        let d = dsl::parse("direction: down\na -> b").unwrap();
        let l = layout(&d);
        assert!(l.rect("a").unwrap().bottom() < l.rect("b").unwrap().y);
        let d = dsl::parse("direction: left\na -> b").unwrap();
        let l = layout(&d);
        assert!(l.rect("b").unwrap().right() < l.rect("a").unwrap().x);
    }

    #[test]
    fn nothing_overlaps_and_groups_contain_members() {
        let src = "group g: G {\n a\n b\n group h {\n  c\n }\n}\nx -> a\nx -> b\nb -> c\ny -> x\nlonely\nz -> c";
        let d = dsl::parse(src).unwrap();
        let l = layout(&d);
        for (i, a) in l.nodes.iter().enumerate() {
            for b in &l.nodes[i + 1..] {
                assert!(!overlaps(&a.rect, &b.rect), "{} overlaps {}", a.id, b.id);
            }
        }
        let g = l.rect("g").unwrap();
        for id in ["a", "b", "c", "h"] {
            let r = l.rect(id).unwrap();
            assert!(
                r.x >= g.x && r.right() <= g.right() && r.y >= g.y && r.bottom() <= g.bottom(),
                "{id} {r:?} outside {g:?}"
            );
        }
        for id in ["x", "y", "z", "lonely"] {
            assert!(!overlaps(&l.rect(id).unwrap(), &g), "{id} inside the group");
        }
    }

    #[test]
    fn cycles_and_pins() {
        let d = dsl::parse("a -> b\nb -> a\nc [x: 500, y: 400]\ngroup g {\n  c\n}").unwrap();
        let l = layout(&d);
        assert_eq!(l.rect("c").unwrap().x, 500.0);
        let g = l.rect("g").unwrap();
        assert!(g.x < 500.0 && g.right() > 500.0);
    }
}
