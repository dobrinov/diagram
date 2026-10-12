//! Gantt charts: a group whose nodes are tasks on a timeline.
//!
//! A task has a start date, a duration (`days`) and the tasks it waits for
//! (`after`). Each of these is optional: a task without a date starts when
//! the tasks it follows end, or else when the previous task ends; the first
//! one starts on the chart's `start`. In the DSL a chart is an outline:
//!
//! ```text
//! gantt launch: Launch plan [start: 2026-10-12] {
//!   Design [5d]
//!   Build [10d, after: design]
//!   Beta [2026-11-03, 1w]
//! }
//! ```
//!
//! Days are calendar days. Dates are `YYYY-MM-DD`.
use std::collections::HashMap;

use crate::model::{Document, Node, Scale};

/// Days since 1970-01-01 for a `YYYY-MM-DD` date.
pub fn parse_date(s: &str) -> Option<i64> {
    let mut it = s.trim().splitn(3, '-');
    let y: i64 = it.next()?.parse().ok()?;
    let m: i64 = it.next()?.parse().ok()?;
    let d: i64 = it.next()?.parse().ok()?;
    if !(1..=12).contains(&m) || d < 1 || d > days_in_month(y, m) || !(1000..=9999).contains(&y) {
        return None;
    }
    // Howard Hinnant's days_from_civil
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146097 + doe - 719468)
}

/// (year, month, day) of days since 1970-01-01.
pub fn civil(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

pub fn format_date(z: i64) -> String {
    let (y, m, d) = civil(z);
    format!("{y:04}-{m:02}-{d:02}")
}

/// "Oct 12": a tick on the timeline.
pub fn short_date(z: i64) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let (_, m, d) = civil(z);
    format!("{} {d}", MONTHS[(m - 1) as usize])
}

/// 0 = Monday.
pub fn weekday(z: i64) -> i64 {
    (z + 3).rem_euclid(7)
}

/// A duration: `5d`, `2w`, `1m` (30 days), `1q` (91 days), `5` (days).
pub fn parse_days(s: &str) -> Option<f64> {
    let s = s.trim();
    let (num, mul) = if let Some(n) = s.strip_suffix('w') {
        (n, 7.0)
    } else if let Some(n) = s.strip_suffix('m') {
        (n, 30.0)
    } else if let Some(n) = s.strip_suffix('q') {
        (n, 91.0)
    } else if let Some(n) = s.strip_suffix('d') {
        (n, 1.0)
    } else {
        (s, 1.0)
    };
    let v: f64 = num.trim().parse().ok()?;
    (v > 0.0 && v.is_finite()).then_some(v * mul)
}

/// `182` → `2q`, `14` → `2w`, `60` → `2m`, `5` → `5d`.
pub fn format_days(d: f64) -> String {
    for (unit, n) in [("q", 91.0), ("w", 7.0), ("m", 30.0)] {
        if d >= n && d % n == 0.0 {
            return format!("{}{unit}", (d / n) as i64);
        }
    }
    let v = if d.fract() == 0.0 {
        format!("{}", d as i64)
    } else {
        format!("{d}")
    };
    format!("{v}d")
}

/// The first day of the next month after day `z`'s.
fn next_month(z: i64) -> i64 {
    let (y, m, d) = civil(z);
    z - (d - 1) + days_in_month(y, m)
}

/// The first day of the next step of `scale` after day `z`: the next day,
/// Monday, 1st of a month or 1st of a quarter.
pub fn next_step(scale: Scale, z: i64) -> i64 {
    match scale {
        Scale::Day => z + 1,
        Scale::Week => z + 7 - weekday(z),
        Scale::Month => next_month(z),
        Scale::Quarter => {
            let mut n = next_month(z);
            while (civil(n).1 - 1) % 3 != 0 {
                n = next_month(n);
            }
            n
        }
    }
}

/// Days a chart's timeline shows for tasks spanning `span` days: whole steps
/// of its scale, and a few of them at least.
pub fn axis_days(scale: Scale, origin: Option<i64>, span: f64) -> f64 {
    let (min, unit) = match scale {
        Scale::Day | Scale::Week => (14.0, 7.0),
        Scale::Month => (91.0, 30.0),
        Scale::Quarter => (365.0, 91.0),
    };
    let want = span.ceil().max(min);
    match (scale, origin) {
        (Scale::Month | Scale::Quarter, Some(o)) => {
            let mut end = o;
            while ((end - o) as f64) < want {
                end = next_step(scale, end);
            }
            (end - o) as f64
        }
        _ => (want / unit).ceil() * unit,
    }
}

/// Width of a day on the timeline.
pub fn day_width(scale: Scale, days: f64) -> f64 {
    match scale {
        Scale::Day => 32.0,
        // long charts get narrower days
        Scale::Week => (840.0 / days).clamp(8.0, 32.0),
        Scale::Month => 4.0,
        Scale::Quarter => 1.4,
    }
}

/// Lines across the timeline: (days from the first day, label). Dated charts
/// mark days, Mondays, months or quarters; undated ones count steps.
pub fn ticks(scale: Scale, origin: Option<i64>, days: f64) -> Vec<(f64, String)> {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let mut out = Vec::new();
    let Some(o) = origin else {
        let step = match scale {
            Scale::Day => 1.0,
            Scale::Week => 7.0,
            Scale::Month => 30.0,
            Scale::Quarter => 91.0,
        };
        let mut d = 0.0;
        while d <= days {
            let k = (d / step) as i64 + 1;
            out.push((
                d,
                match scale {
                    Scale::Day => format!("{k}"),
                    Scale::Week => format!("Week {k}"),
                    Scale::Month => format!("Month {k}"),
                    Scale::Quarter => format!("Q{k}"),
                },
            ));
            d += step;
        }
        return out;
    };
    let label = |z: i64, first: bool| {
        let (y, m, d) = civil(z);
        let mon = MONTHS[(m - 1) as usize];
        match scale {
            Scale::Day if d == 1 || first => format!("{mon} {d}"),
            Scale::Day => format!("{d}"),
            Scale::Week => short_date(z),
            Scale::Month if m == 1 || first => format!("{mon} {y}"),
            Scale::Month => mon.to_string(),
            Scale::Quarter => format!("Q{} {y}", (m - 1) / 3 + 1),
        }
    };
    let on_step = next_step(scale, o - 1) == o;
    let mut z = o;
    if !on_step {
        // weeks start on the next Monday; a partial first month or quarter
        // is labelled where the chart starts
        if scale != Scale::Week {
            out.push((0.0, label(o, true)));
        }
        z = next_step(scale, o);
    }
    while ((z - o) as f64) <= days {
        let first = out.is_empty();
        out.push(((z - o) as f64, label(z, first)));
        z = next_step(scale, z);
    }
    out
}

/// A task without a duration takes this long.
pub const DEFAULT_DAYS: f64 = 1.0;

/// How long a task takes on the timeline; a milestone takes no time.
pub fn task_days(n: &Node) -> f64 {
    if n.milestone {
        0.0
    } else {
        n.days.unwrap_or(DEFAULT_DAYS)
    }
}

/// The day each task on the board ends (days since 1970, dated charts
/// only), with tasks that wait for tasks in other charts placed after them.
pub fn board_ends(doc: &Document) -> HashMap<String, f64> {
    let charts: Vec<&str> = doc
        .groups
        .iter()
        .filter(|g| g.gantt)
        .map(|g| g.id.as_str())
        .collect();
    let mut ends: HashMap<String, f64> = HashMap::new();
    // charts can wait for each other in any order: settle a few rounds
    for _ in 0..charts.len().clamp(1, 8) {
        let mut next = HashMap::new();
        for c in &charts {
            let s = schedule_with(doc, c, &ends);
            if let Some(o) = s.origin {
                for (i, start, days) in s.tasks {
                    next.insert(doc.nodes[i].id.clone(), o as f64 + start + days);
                }
            }
        }
        if next == ends {
            break;
        }
        ends = next;
    }
    ends
}

/// When each task of the chart `group` starts and how long it takes, in days
/// from the chart's first day (`origin`, days since 1970 when the chart has
/// dates). Tasks in document order.
#[derive(Debug, Clone, Default)]
pub struct Schedule {
    pub origin: Option<i64>,
    /// (node index, start offset, days)
    pub tasks: Vec<(usize, f64, f64)>,
    /// Days from the first day to the end of the last task.
    pub span: f64,
}

pub fn schedule(doc: &Document, group: &str) -> Schedule {
    // waits for a task in another chart: place the whole board first
    let elsewhere = doc.nodes.iter().any(|n| {
        n.group.as_deref() == Some(group)
            && n.after.iter().any(|a| {
                doc.node(a)
                    .is_some_and(|t| t.group.as_deref() != Some(group))
            })
    });
    if elsewhere {
        schedule_with(doc, group, &board_ends(doc))
    } else {
        schedule_with(doc, group, &HashMap::new())
    }
}

/// [`schedule`], with when tasks in other charts end (days since 1970).
fn schedule_with(doc: &Document, group: &str, ends: &HashMap<String, f64>) -> Schedule {
    let g = doc.group(group);
    let idx: Vec<usize> = (0..doc.nodes.len())
        .filter(|&i| doc.nodes[i].group.as_deref() == Some(group))
        .collect();
    let dated: Vec<i64> = idx
        .iter()
        .filter_map(|&i| doc.nodes[i].start.as_deref().and_then(parse_date))
        .collect();
    let origin = g
        .and_then(|g| g.start.as_deref())
        .and_then(parse_date)
        .or_else(|| dated.iter().copied().min());
    let mut tasks = Vec::new();
    // `after` can point forward; settle a few rounds so order does not matter
    let mut starts: Vec<f64> = vec![0.0; idx.len()];
    for _ in 0..idx.len().clamp(1, 32) {
        let mut end: std::collections::HashMap<&str, f64> = Default::default();
        let mut prev_end = 0.0f64;
        for (k, &i) in idx.iter().enumerate() {
            let n = &doc.nodes[i];
            let days = task_days(n);
            let start = match (n.start.as_deref().and_then(parse_date), origin) {
                (Some(d), Some(o)) => (d - o) as f64,
                _ if !n.after.is_empty() => n
                    .after
                    .iter()
                    .map(|a| {
                        end.get(a.as_str()).copied().unwrap_or_else(|| {
                            // not placed yet this round: use the last round's
                            idx.iter()
                                .position(|&j| doc.nodes[j].id == *a)
                                .map(|p| starts[p] + task_days(&doc.nodes[idx[p]]))
                                // in another chart: when it ends there
                                .or_else(|| Some(ends.get(a.as_str())? - origin? as f64))
                                .unwrap_or(0.0)
                        })
                    })
                    .fold(0.0, f64::max),
                _ => prev_end,
            };
            starts[k] = start;
            end.insert(n.id.as_str(), start + days);
            prev_end = start + days;
        }
    }
    for (k, &i) in idx.iter().enumerate() {
        tasks.push((i, starts[k], task_days(&doc.nodes[i])));
    }
    let lo = tasks.iter().map(|t| t.1).fold(0.0, f64::min);
    if lo < 0.0 {
        // a task dated before the chart's start moves the first day back
        for t in &mut tasks {
            t.1 -= lo;
        }
    }
    let origin = origin.map(|o| o + lo.floor() as i64);
    let span = tasks.iter().map(|t| t.1 + t.2).fold(0.0, f64::max);
    Schedule {
        origin,
        tasks,
        span,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_round_trip() {
        for s in ["1970-01-01", "2026-10-12", "2024-02-29", "2000-12-31"] {
            assert_eq!(format_date(parse_date(s).unwrap()), s);
        }
        assert_eq!(parse_date("2026-02-30"), None);
        assert_eq!(parse_date("2026-10-12").map(weekday), Some(0)); // a Monday
        assert_eq!(short_date(parse_date("2026-10-12").unwrap()), "Oct 12");
        assert_eq!(
            (parse_days("2w"), parse_days("5d"), parse_days("0d")),
            (Some(14.0), Some(5.0), None)
        );
    }

    #[test]
    fn timelines_have_a_scale() {
        let o = parse_date("2026-10-14"); // a Wednesday
        assert_eq!(ticks(Scale::Week, o, 14.0)[0], (5.0, "Oct 19".to_string()));
        let t = ticks(Scale::Month, o, axis_days(Scale::Month, o, 10.0));
        assert_eq!(
            t.iter().map(|t| t.1.as_str()).collect::<Vec<_>>(),
            ["Oct 2026", "Nov", "Dec", "Jan 2027", "Feb"]
        );
        assert_eq!(t[1].0, 18.0);
        let t = ticks(Scale::Quarter, o, axis_days(Scale::Quarter, o, 10.0));
        assert_eq!(t[0].1, "Q4 2026");
        assert_eq!(t[1], (79.0, "Q1 2027".to_string()));
        assert_eq!(ticks(Scale::Day, o, 3.0)[0].1, "Oct 14");
        assert_eq!(ticks(Scale::Day, None, 2.0).len(), 3);
        assert_eq!(
            (parse_days("1m"), parse_days("2q")),
            (Some(30.0), Some(182.0))
        );
        assert_eq!(
            [182.0, 14.0, 60.0, 5.0, 1.5].map(format_days),
            ["2q", "2w", "2m", "5d", "1.5d"]
        );
    }

    #[test]
    fn tasks_follow_each_other() {
        let d = crate::dsl::parse(
            "gantt g: Plan [start: 2026-10-12] {\n  Design [5d]\n  Build [10d, after: design]\n  Docs [2d, after: design]\n  Beta [2026-11-03, 1w]\n  Launch\n}",
        )
        .unwrap();
        let s = schedule(&d, "g");
        let at = |id: &str| {
            let t = s.tasks.iter().find(|t| d.nodes[t.0].id == id).unwrap();
            (t.1, t.2)
        };
        assert_eq!(s.origin, parse_date("2026-10-12"));
        assert_eq!(at("design"), (0.0, 5.0));
        assert_eq!(at("build"), (5.0, 10.0));
        assert_eq!(at("docs"), (5.0, 2.0));
        assert_eq!(at("beta"), (22.0, 7.0));
        assert_eq!(at("launch"), (29.0, 1.0));
        assert_eq!(s.span, 30.0);
    }

    #[test]
    fn milestones_progress_and_owners() {
        let src = "gantt g: Plan [start: 2026-10-12] {\n  Build [5d, done: 60%, owner: ana]\n  Docs [2d, done]\n  Launch [after: build, milestone]\n}\n";
        let d = crate::dsl::parse(src).unwrap();
        assert_eq!(crate::dsl::print(&d), src);
        let (b, l) = (d.node("build").unwrap(), d.node("launch").unwrap());
        assert_eq!((b.done, b.owner.as_deref()), (Some(60.0), Some("ana")));
        assert_eq!(d.node("docs").unwrap().done, Some(100.0));
        assert!(l.milestone);
        let s = schedule(&d, "g");
        let at = |id: &str| {
            s.tasks
                .iter()
                .find(|t| d.nodes[t.0].id == id)
                .map(|t| (t.1, t.2))
        };
        // a milestone takes no time, on the day its task ends
        assert_eq!(at("launch"), Some((5.0, 0.0)));
        let lay = crate::layout::layout(&d);
        let (m, bar) = (lay.rect("launch").unwrap(), lay.rect("build").unwrap());
        assert!((m.cx() - bar.right()).abs() < 1e-6, "{m:?} {bar:?}");
        let svg = crate::render::render(
            &d,
            &crate::render::RenderOptions {
                today: Some("2026-10-14".into()),
                ..Default::default()
            },
        )
        .svg;
        assert!(svg.contains("dg-milestone") && svg.contains("dg-today"));
        assert!(svg.contains(">ana</tspan>") && svg.contains("5d · 60%") && svg.contains("2d ✓"));
        let p = crate::prompt::selection(&d, &["g".to_string()]);
        assert!(
            p.contains("- Build: 2026-10-12 → 2026-10-16, 5d, 60% done, owner ana\n"),
            "{p}"
        );
        assert!(
            p.contains("- Launch: 2026-10-17, milestone, after Build\n"),
            "{p}"
        );
    }

    #[test]
    fn after_means_this_chart_and_reaches_other_charts() {
        // both charts have a Design; each Build waits for its own
        let src = "gantt a: A [start: 2026-10-12] {\n  Design [5d]\n  Build [2d, after: design]\n}\ngantt b: B [start: 2026-10-12] {\n  Design [10d]\n  Build [2d, after: design]\n  Ship [1d, after: build a/build]\n}\n";
        let d = crate::dsl::parse(src).unwrap();
        assert_eq!(d.node("build-2").unwrap().after, ["design-2"]);
        // it reads back the way it was written
        let out = crate::dsl::print(&d);
        assert!(out.contains("  Ship [1d, after: build-2 a/build]"), "{out}");
        assert_eq!(crate::dsl::parse(&out).unwrap(), d);
        // Ship (in B) waits for A's Build too, which ends later than B's
        let mut d = d;
        d.node_mut("build").unwrap().days = Some(20.0);
        let s = schedule(&d, "b");
        let ship = s.tasks.iter().find(|t| d.nodes[t.0].id == "ship").unwrap();
        assert_eq!(ship.1, 25.0);
    }

    #[test]
    fn charts_round_trip_and_lay_out_as_a_timeline() {
        let src =
            "gantt g: Plan [start: 2026-10-12] {\n  Design [5d]\n  Build [2w, after: design]\n}\n";
        let d = crate::dsl::parse(src).unwrap();
        assert_eq!(crate::dsl::print(&d), src);
        let l = crate::layout::layout(&d);
        let m = crate::dsl::parse(
            "gantt g: Plan [scale: quarter] {\n  A [2m]\n  B [1q, after: a]\n}\n",
        )
        .unwrap();
        assert_eq!(m.groups[0].scale, Some(crate::model::Scale::Quarter));
        assert_eq!((m.nodes[0].days, m.nodes[1].days), (Some(60.0), Some(91.0)));
        assert!(crate::dsl::print(&m).contains("[scale: quarter]"));
        let (a, b) = (l.rect("design").unwrap(), l.rect("build").unwrap());
        let gb = l.group("g").unwrap().gantt.clone().unwrap();
        // bars: one row each, as long as the task, the next starting where the first ends
        assert!(l.node("design").unwrap().task);
        assert_eq!(b.y - a.y, crate::layout::GANTT_ROW);
        assert!((a.w - 5.0 * gb.day).abs() < 1e-6 && (b.w - 14.0 * gb.day).abs() < 1e-6);
        assert!((b.x - a.right()).abs() < 1e-6);
        assert_eq!(gb.rows, vec!["design", "build"]);
        assert_eq!(gb.origin, parse_date("2026-10-12"));
    }
}
