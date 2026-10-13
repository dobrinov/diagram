//! Edge geometry: which side of a box an edge leaves from, ports spread
//! along that side, and orthogonal / curved / straight paths between them.
use std::collections::HashMap;

use serde::Serialize;

use crate::layout::{Layout, Rect};
use crate::model::*;
use crate::text::n;

const STUB: f64 = 18.0;
const RADIUS: f64 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

impl Side {
    fn normal(self) -> (f64, f64) {
        match self {
            Side::Left => (-1.0, 0.0),
            Side::Right => (1.0, 0.0),
            Side::Top => (0.0, -1.0),
            Side::Bottom => (0.0, 1.0),
        }
    }
    fn horizontal(self) -> bool {
        matches!(self, Side::Left | Side::Right)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EdgePath {
    pub id: String,
    /// SVG path data.
    pub d: String,
    /// Where the label (or a comment pin) goes.
    pub mid: (f64, f64),
    /// Arrowhead tip and the direction it points (unit vector).
    #[serde(skip)]
    pub end: ((f64, f64), (f64, f64)),
    #[serde(skip)]
    pub start: ((f64, f64), (f64, f64)),
}

/// Shapes whose outline does not follow their bounding box: edges attach at
/// the middle of a side instead of being spread along it.
fn spreadable(layout: &Layout, id: &str) -> bool {
    match layout.node(id) {
        Some(nb) => matches!(
            nb.shape,
            Shape::Box
                | Shape::Rounded
                | Shape::Pill
                | Shape::Cylinder
                | Shape::Document
                | Shape::Sticky
                | Shape::Text
        ),
        None => true,
    }
}

fn choose_sides(a: &Rect, b: &Rect, dir: Direction) -> (Side, Side) {
    let dx = b.cx() - a.cx();
    let dy = b.cy() - a.cy();
    let gap_x = dx.abs() - (a.w + b.w) / 2.0;
    let gap_y = dy.abs() - (a.h + b.h) / 2.0;
    let flow_h = matches!(dir, Direction::Right | Direction::Left);
    let horizontal = match (gap_x > 8.0, gap_y > 8.0) {
        (true, false) => true,
        (false, true) => false,
        (true, true) => {
            if flow_h {
                gap_x * 1.6 > gap_y
            } else {
                gap_x > gap_y * 1.6
            }
        }
        (false, false) => gap_x > gap_y,
    };
    if horizontal {
        if dx >= 0.0 {
            (Side::Right, Side::Left)
        } else {
            (Side::Left, Side::Right)
        }
    } else if dy >= 0.0 {
        (Side::Bottom, Side::Top)
    } else {
        (Side::Top, Side::Bottom)
    }
}

fn side_point(r: &Rect, side: Side, t: f64) -> (f64, f64) {
    match side {
        Side::Left => (r.x, r.y + r.h * t),
        Side::Right => (r.right(), r.y + r.h * t),
        Side::Top => (r.x + r.w * t, r.y),
        Side::Bottom => (r.x + r.w * t, r.bottom()),
    }
}

/// Where the line from the centre of `r` towards `p` leaves the shape.
fn clip(r: &Rect, shape: Option<Shape>, p: (f64, f64)) -> (f64, f64) {
    let (cx, cy) = (r.cx(), r.cy());
    let (dx, dy) = (p.0 - cx, p.1 - cy);
    if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
        return (cx, cy);
    }
    let (hw, hh) = (r.w / 2.0, r.h / 2.0);
    let t = match shape {
        Some(Shape::Circle) | Some(Shape::Cloud) => {
            1.0 / ((dx / hw).powi(2) + (dy / hh).powi(2)).sqrt()
        }
        Some(Shape::Diamond) => 1.0 / (dx.abs() / hw + dy.abs() / hh),
        _ => (hw / dx.abs()).min(hh / dy.abs()),
    };
    (cx + dx * t, cy + dy * t)
}

pub fn route(doc: &Document, layout: &Layout) -> Vec<EdgePath> {
    let dir = doc.settings.direction;
    let routing = doc.settings.routing;
    struct Plan {
        a: Rect,
        b: Rect,
        sa: Side,
        sb: Side,
        pa: (f64, f64),
        pb: (f64, f64),
    }
    let mut plans: Vec<Option<Plan>> = Vec::new();
    for e in &doc.edges {
        // messages of a sequence diagram are drawn straight across (below)
        if crate::sequence::sequence_of(doc, e).is_some() {
            plans.push(None);
            continue;
        }
        let (Some(a), Some(b)) = (layout.rect(&e.from), layout.rect(&e.to)) else {
            plans.push(None);
            continue;
        };
        let (sa, sb) = choose_sides(&a, &b, dir);
        plans.push(Some(Plan {
            a,
            b,
            sa,
            sb,
            pa: (0.0, 0.0),
            pb: (0.0, 0.0),
        }));
    }

    // spread the ends that share a side, ordered by where the other end is
    let mut slots: HashMap<(String, Side), Vec<(usize, bool, f64)>> = HashMap::new();
    for (i, e) in doc.edges.iter().enumerate() {
        let Some(p) = &plans[i] else { continue };
        if e.from == e.to {
            continue;
        }
        let key_a = if p.sa.horizontal() {
            p.b.cy()
        } else {
            p.b.cx()
        };
        let key_b = if p.sb.horizontal() {
            p.a.cy()
        } else {
            p.a.cx()
        };
        slots
            .entry((e.from.clone(), p.sa))
            .or_default()
            .push((i, true, key_a));
        slots
            .entry((e.to.clone(), p.sb))
            .or_default()
            .push((i, false, key_b));
    }
    for ((id, side), mut ends) in slots {
        ends.sort_by(|x, y| x.2.partial_cmp(&y.2).unwrap().then(x.0.cmp(&y.0)));
        let count = ends.len() as f64;
        let spread = spreadable(layout, &id);
        for (k, (i, is_a, _)) in ends.into_iter().enumerate() {
            let p = plans[i].as_mut().unwrap();
            let r = if is_a { p.a } else { p.b };
            let t = if spread && count > 1.0 {
                // keep ports away from rounded corners
                let span = (count - 1.0) * 18.0;
                let len = if side.horizontal() { r.h } else { r.w };
                let span = span.min(len * 0.7);
                0.5 + (k as f64 / (count - 1.0) - 0.5) * span / len
            } else {
                0.5
            };
            let pt = side_point(&r, side, t);
            if is_a {
                p.pa = pt;
            } else {
                p.pb = pt;
            }
        }
    }
    // shapes that aren't boxes: attach on the outline, not the bounding box
    for (i, e) in doc.edges.iter().enumerate() {
        let Some(p) = plans[i].as_mut() else { continue };
        for (id, is_a) in [(&e.from, true), (&e.to, false)] {
            let Some(nb) = layout.node(id) else { continue };
            if !spreadable(layout, id) {
                let r = nb.rect;
                let side = if is_a { p.sa } else { p.sb };
                let (nx, ny) = side.normal();
                let target = (r.cx() + nx * r.w, r.cy() + ny * r.h);
                let pt = if nb.shape == Shape::Person {
                    side_point(&r, side, 0.5)
                } else {
                    clip(&r, Some(nb.shape), target)
                };
                if is_a {
                    p.pa = pt;
                } else {
                    p.pb = pt;
                }
            }
        }
    }

    // lanes: forward orthogonal edges crossing the same gap get their own
    // middle segment so parallel edges do not overlap
    let mut lane: HashMap<usize, f64> = HashMap::new();
    if routing == Routing::Orthogonal {
        let mut gaps: HashMap<(bool, i64, i64), Vec<usize>> = HashMap::new();
        for (i, p) in plans.iter().enumerate() {
            let Some(p) = p else { continue };
            if doc.edges[i].from == doc.edges[i].to {
                continue;
            }
            let h = p.sa.horizontal();
            let (a0, b0) = if h {
                (p.pa.0, p.pb.0)
            } else {
                (p.pa.1, p.pb.1)
            };
            let (ca, cb) = if h {
                (p.pa.1, p.pb.1)
            } else {
                (p.pa.0, p.pb.0)
            };
            if (ca - cb).abs() < 1.0 {
                continue;
            }
            let lo = (a0.min(b0) / 8.0).round() as i64;
            let hi = (a0.max(b0) / 8.0).round() as i64;
            gaps.entry((h, lo, hi)).or_default().push(i);
        }
        for ((h, _, _), mut ids) in gaps {
            if ids.len() < 2 {
                continue;
            }
            let key = |i: &usize| {
                let p = plans[*i].as_ref().unwrap();
                if h {
                    (p.pa.1 + p.pb.1) / 2.0
                } else {
                    (p.pa.0 + p.pb.0) / 2.0
                }
            };
            ids.sort_by(|x, y| key(x).partial_cmp(&key(y)).unwrap());
            let count = ids.len() as f64;
            for (k, i) in ids.into_iter().enumerate() {
                let p = plans[i].as_ref().unwrap();
                let (a0, b0) = if h {
                    (p.pa.0, p.pb.0)
                } else {
                    (p.pa.1, p.pb.1)
                };
                let (lo, hi) = (a0.min(b0) + STUB, a0.max(b0) - STUB);
                if hi > lo {
                    lane.insert(i, lo + (hi - lo) * (k as f64 + 1.0) / (count + 1.0));
                }
            }
        }
    }

    let mut out = Vec::new();
    for (i, e) in doc.edges.iter().enumerate() {
        if let Some(pts) = crate::sequence::message_points(doc, layout, e) {
            let mut path = polyline(&e.id, &pts);
            // the label sits on the message, just above its line
            path.mid = if e.from == e.to {
                let w = e
                    .label
                    .as_deref()
                    .map_or(0.0, |l| crate::text::width(l, 12.0));
                (pts[1].0 + 14.0 + w / 2.0, pts[1].1 + 9.0)
            } else {
                ((pts[0].0 + pts[1].0) / 2.0, pts[0].1 - 12.0)
            };
            out.push(path);
            continue;
        }
        let Some(p) = &plans[i] else { continue };
        if e.from == e.to {
            out.push(self_loop(&e.id, &p.a));
            continue;
        }
        let path = match routing {
            Routing::Straight => {
                let sa = layout.node(&e.from).map(|n| n.shape);
                let sb = layout.node(&e.to).map(|n| n.shape);
                let pa = clip(&p.a, sa, (p.b.cx(), p.b.cy()));
                let pb = clip(&p.b, sb, (p.a.cx(), p.a.cy()));
                polyline(&e.id, &[pa, pb])
            }
            Routing::Curved => curve(&e.id, p.pa, p.sa, p.pb, p.sb),
            Routing::Orthogonal => {
                let pts = orthogonal(p.pa, p.sa, p.pb, p.sb, &p.a, &p.b, lane.get(&i).copied());
                polyline(&e.id, &pts)
            }
        };
        out.push(path);
    }
    out
}

fn orthogonal(
    pa: (f64, f64),
    sa: Side,
    pb: (f64, f64),
    sb: Side,
    a: &Rect,
    b: &Rect,
    lane: Option<f64>,
) -> Vec<(f64, f64)> {
    // work in "horizontal" coordinates and swap back for vertical sides
    let h = sa.horizontal();
    let sw = |p: (f64, f64)| if h { p } else { (p.1, p.0) };
    let (p1, p2) = (sw(pa), sw(pb));
    let (na, nb) = (sw(sa.normal()).0, sw(sb.normal()).0);
    let (ra, rb) = if h {
        (*a, *b)
    } else {
        (
            Rect {
                x: a.y,
                y: a.x,
                w: a.h,
                h: a.w,
            },
            Rect {
                x: b.y,
                y: b.x,
                w: b.h,
                h: b.w,
            },
        )
    };
    let forward = (p2.0 - p1.0) * na > 2.0 * STUB - 1.0 && na == -nb;
    let pts: Vec<(f64, f64)> = if forward {
        if (p1.1 - p2.1).abs() < 1.0 {
            vec![p1, (p2.0, p1.1)]
        } else {
            let mx = lane.unwrap_or((p1.0 + p2.0) / 2.0);
            vec![p1, (mx, p1.1), (mx, p2.1), p2]
        }
    } else {
        // around: leave, go past both boxes on the shorter side, come back
        let o1 = p1.0 + na * STUB;
        let o2 = p2.0 + nb * STUB;
        let above = ra.y.min(rb.y) - STUB;
        let below = ra.bottom().max(rb.bottom()) + STUB;
        let my = if (p1.1 - above).abs() + (p2.1 - above).abs()
            < (p1.1 - below).abs() + (p2.1 - below).abs()
        {
            above
        } else {
            below
        };
        vec![p1, (o1, p1.1), (o1, my), (o2, my), (o2, p2.1), p2]
    };
    pts.into_iter().map(sw).collect()
}

fn unit(from: (f64, f64), to: (f64, f64)) -> (f64, f64) {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let l = (dx * dx + dy * dy).sqrt().max(1e-9);
    (dx / l, dy / l)
}

fn polyline(id: &str, pts: &[(f64, f64)]) -> EdgePath {
    // drop zero-length segments, which would break the rounded corners
    let mut p: Vec<(f64, f64)> = Vec::new();
    for &q in pts {
        if p.last()
            .is_none_or(|l: &(f64, f64)| (l.0 - q.0).abs() > 0.5 || (l.1 - q.1).abs() > 0.5)
        {
            p.push(q);
        }
    }
    if p.len() == 1 {
        p.push(p[0]);
    }
    let mut d = format!("M{},{}", n(p[0].0), n(p[0].1));
    for i in 1..p.len() {
        if i + 1 < p.len() {
            let (prev, cur, next) = (p[i - 1], p[i], p[i + 1]);
            let l1 = ((cur.0 - prev.0).powi(2) + (cur.1 - prev.1).powi(2)).sqrt();
            let l2 = ((next.0 - cur.0).powi(2) + (next.1 - cur.1).powi(2)).sqrt();
            let r = RADIUS.min(l1 / 2.0).min(l2 / 2.0);
            let u1 = unit(prev, cur);
            let u2 = unit(cur, next);
            let a = (cur.0 - u1.0 * r, cur.1 - u1.1 * r);
            let b = (cur.0 + u2.0 * r, cur.1 + u2.1 * r);
            d.push_str(&format!(
                " L{},{} Q{},{} {},{}",
                n(a.0),
                n(a.1),
                n(cur.0),
                n(cur.1),
                n(b.0),
                n(b.1)
            ));
        } else {
            d.push_str(&format!(" L{},{}", n(p[i].0), n(p[i].1)));
        }
    }
    // label at half the length
    let lens: Vec<f64> = p
        .windows(2)
        .map(|w| ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt())
        .collect();
    let total: f64 = lens.iter().sum();
    let mut acc = 0.0;
    let mut mid = p[0];
    for (k, l) in lens.iter().enumerate() {
        if acc + l >= total / 2.0 && *l > 0.0 {
            let t = (total / 2.0 - acc) / l;
            mid = (
                p[k].0 + (p[k + 1].0 - p[k].0) * t,
                p[k].1 + (p[k + 1].1 - p[k].1) * t,
            );
            break;
        }
        acc += l;
    }
    let last = p.len() - 1;
    EdgePath {
        id: id.to_string(),
        d,
        mid,
        end: (p[last], unit(p[last - 1], p[last])),
        start: (p[0], unit(p[1], p[0])),
    }
}

fn curve(id: &str, pa: (f64, f64), sa: Side, pb: (f64, f64), sb: Side) -> EdgePath {
    let dist = ((pb.0 - pa.0).powi(2) + (pb.1 - pa.1).powi(2)).sqrt();
    let k = (dist * 0.45).max(30.0);
    let (na, nb) = (sa.normal(), sb.normal());
    let c1 = (pa.0 + na.0 * k, pa.1 + na.1 * k);
    let c2 = (pb.0 + nb.0 * k, pb.1 + nb.1 * k);
    let d = format!(
        "M{},{} C{},{} {},{} {},{}",
        n(pa.0),
        n(pa.1),
        n(c1.0),
        n(c1.1),
        n(c2.0),
        n(c2.1),
        n(pb.0),
        n(pb.1)
    );
    let bez = |t: f64| {
        let u = 1.0 - t;
        let x =
            u * u * u * pa.0 + 3.0 * u * u * t * c1.0 + 3.0 * u * t * t * c2.0 + t * t * t * pb.0;
        let y =
            u * u * u * pa.1 + 3.0 * u * u * t * c1.1 + 3.0 * u * t * t * c2.1 + t * t * t * pb.1;
        (x, y)
    };
    EdgePath {
        id: id.to_string(),
        d,
        mid: bez(0.5),
        end: (pb, unit(c2, pb)),
        start: (pa, unit(c1, pa)),
    }
}

fn self_loop(id: &str, r: &Rect) -> EdgePath {
    let s = (r.right() - 22.0, r.y);
    let e = (r.right(), r.y + 22.0);
    let d = format!(
        "M{},{} C{},{} {},{} {},{}",
        n(s.0),
        n(s.1),
        n(s.0),
        n(s.1 - 44.0),
        n(e.0 + 44.0),
        n(e.1),
        n(e.0),
        n(e.1)
    );
    EdgePath {
        id: id.to_string(),
        d,
        mid: (r.right() + 14.0, r.y - 14.0),
        end: (e, (-1.0, 0.0)),
        start: (s, (0.0, 1.0)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{dsl, layout::layout};

    #[test]
    fn routes_every_edge() {
        for routing in ["orthogonal", "curved", "straight"] {
            let d = dsl::parse(&format!(
                "routing: {routing}\na -> b\na -> c\nb -> a\nc -> c\ngroup g {{\n x\n}}\na -> g"
            ))
            .unwrap();
            let l = layout(&d);
            let paths = route(&d, &l);
            assert_eq!(paths.len(), 5);
            for p in &paths {
                assert!(
                    p.d.starts_with('M') && !p.d.contains("NaN"),
                    "{routing}: {p:?}"
                );
            }
        }
    }

    #[test]
    fn straight_forward_edge_is_a_line() {
        let d = dsl::parse("a -> b").unwrap();
        let l = layout(&d);
        let p = &route(&d, &l)[0];
        assert_eq!(p.d.matches('L').count(), 1, "{}", p.d);
        assert_eq!(p.end.1, (1.0, 0.0));
    }
}
