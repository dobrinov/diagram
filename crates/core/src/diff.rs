//! What changed between two versions of a diagram (positions are ignored).
use std::collections::HashMap;

use serde::Serialize;

use crate::model::*;
use crate::render::Change;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Entry {
    /// node, edge, group, comment or setting
    pub kind: &'static str,
    pub id: String,
    pub change: Change,
    /// For `changed`: field, old value, new value.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<(String, String, String)>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Diff {
    pub entries: Vec<Entry>,
}

impl Diff {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

fn s<T: std::fmt::Debug>(v: &T) -> String {
    format!("{v:?}")
}

fn opt(v: &Option<String>) -> String {
    v.clone().unwrap_or_default()
}

fn fields_node(a: &Node, b: &Node) -> Vec<(String, String, String)> {
    let mut f = Vec::new();
    let mut cmp = |name: &str, x: String, y: String| {
        if x != y {
            f.push((name.to_string(), x, y));
        }
    };
    cmp("label", a.display().to_string(), b.display().to_string());
    cmp("shape", a.shape.as_str().into(), b.shape.as_str().into());
    cmp("detail", opt(&a.detail), opt(&b.detail));
    cmp("color", opt(&a.color), opt(&b.color));
    cmp("group", opt(&a.group), opt(&b.group));
    cmp("link", opt(&a.link), opt(&b.link));
    cmp("tags", a.tags.join(" "), b.tags.join(" "));
    // mindmap topics and gantt tasks
    cmp("parent", opt(&a.parent), opt(&b.parent));
    let days = |n: &Node| n.days.map(crate::gantt::format_days).unwrap_or_default();
    cmp("start", opt(&a.start), opt(&b.start));
    cmp("days", days(a), days(b));
    cmp("after", a.after.join(" "), b.after.join(" "));
    cmp(
        "milestone",
        (a.milestone as u8).to_string(),
        (b.milestone as u8).to_string(),
    );
    let done = |n: &Node| n.done.map(|d| format!("{d}%")).unwrap_or_default();
    cmp("done", done(a), done(b));
    cmp("owner", opt(&a.owner), opt(&b.owner));
    // database columns
    let col = |n: &Node| n.column.clone().unwrap_or_default();
    let (ca, cb) = (col(a), col(b));
    cmp("type", ca.ty.clone(), cb.ty.clone());
    let flags = |c: &crate::model::Column| {
        [(c.pk, "pk"), (c.unique, "unique"), (c.null, "null")]
            .iter()
            .filter(|(on, _)| *on)
            .map(|(_, k)| *k)
            .collect::<Vec<_>>()
            .join(" ")
    };
    cmp("keys", flags(&ca), flags(&cb));
    cmp(
        "references",
        ca.fk.clone().unwrap_or_default(),
        cb.fk.clone().unwrap_or_default(),
    );
    cmp(
        "default",
        ca.default.clone().unwrap_or_default(),
        cb.default.clone().unwrap_or_default(),
    );
    cmp(
        "relationship",
        ca.rel.clone().unwrap_or_default(),
        cb.rel.clone().unwrap_or_default(),
    );
    // risks
    let level = |v: Option<u8>| v.map(|v| v.to_string()).unwrap_or_default();
    cmp("probability", level(a.probability), level(b.probability));
    cmp("impact", level(a.impact), level(b.impact));
    f
}

fn fields_edge(a: &Edge, b: &Edge) -> Vec<(String, String, String)> {
    let mut f = Vec::new();
    let mut cmp = |name: &str, x: String, y: String| {
        if x != y {
            f.push((name.to_string(), x, y));
        }
    };
    cmp("from", a.from.clone(), b.from.clone());
    cmp("to", a.to.clone(), b.to.clone());
    cmp("label", opt(&a.label), opt(&b.label));
    cmp("arrow", a.arrow.as_str().into(), b.arrow.as_str().into());
    cmp("line", a.line.as_str().into(), b.line.as_str().into());
    cmp("color", opt(&a.color), opt(&b.color));
    f
}

fn fields_group(a: &Group, b: &Group) -> Vec<(String, String, String)> {
    let mut f = Vec::new();
    for (name, x, y) in [
        ("label", a.display().to_string(), b.display().to_string()),
        ("color", opt(&a.color), opt(&b.color)),
        ("parent", opt(&a.parent), opt(&b.parent)),
        ("start", opt(&a.start), opt(&b.start)),
        (
            "levels",
            a.levels.map(|l| l.to_string()).unwrap_or_default(),
            b.levels.map(|l| l.to_string()).unwrap_or_default(),
        ),
        (
            "scale",
            a.scale.map(|s| s.as_str().to_string()).unwrap_or_default(),
            b.scale.map(|s| s.as_str().to_string()).unwrap_or_default(),
        ),
    ] {
        if x != y {
            f.push((name.to_string(), x, y));
        }
    }
    f
}

pub fn diff(old: &Document, new: &Document) -> Diff {
    let mut entries = Vec::new();
    let s1 = &old.settings;
    let s2 = &new.settings;
    for (k, x, y) in [
        ("title", opt(&old.title), opt(&new.title)),
        ("description", opt(&old.description), opt(&new.description)),
        (
            "direction",
            s(&s1.direction).to_lowercase(),
            s(&s2.direction).to_lowercase(),
        ),
        (
            "layout",
            s(&s1.layout).to_lowercase(),
            s(&s2.layout).to_lowercase(),
        ),
        (
            "routing",
            s(&s1.routing).to_lowercase(),
            s(&s2.routing).to_lowercase(),
        ),
    ] {
        if x != y {
            entries.push(Entry {
                kind: "setting",
                id: k.into(),
                change: Change::Changed,
                fields: vec![(k.into(), x, y)],
            });
        }
    }
    macro_rules! compare {
        ($kind:literal, $list:ident, $get:ident, $fields:ident) => {
            for b in &new.$list {
                match old.$get(&b.id) {
                    None => entries.push(Entry {
                        kind: $kind,
                        id: b.id.clone(),
                        change: Change::Added,
                        fields: vec![],
                    }),
                    Some(a) => {
                        let f = $fields(a, b);
                        if !f.is_empty() {
                            entries.push(Entry {
                                kind: $kind,
                                id: b.id.clone(),
                                change: Change::Changed,
                                fields: f,
                            });
                        }
                    }
                }
            }
            for a in &old.$list {
                if new.$get(&a.id).is_none() {
                    entries.push(Entry {
                        kind: $kind,
                        id: a.id.clone(),
                        change: Change::Removed,
                        fields: vec![],
                    });
                }
            }
        };
    }
    compare!("group", groups, group, fields_group);
    compare!("node", nodes, node, fields_node);
    compare!("edge", edges, edge, fields_edge);
    let fields_comment = |a: &Comment, b: &Comment| {
        let mut f = Vec::new();
        if a.status != b.status {
            f.push((
                "status".to_string(),
                a.status.as_str().to_string(),
                b.status.as_str().to_string(),
            ));
        }
        if a.text != b.text {
            f.push(("text".to_string(), a.text.clone(), b.text.clone()));
        }
        if a.replies.len() != b.replies.len() {
            f.push((
                "replies".to_string(),
                a.replies.len().to_string(),
                b.replies.len().to_string(),
            ));
        }
        f
    };
    compare!("comment", comments, comment, fields_comment);
    Diff { entries }
}

/// A document holding both versions (removed elements are put back) and the
/// change of every element, for rendering the diff.
pub fn merge(old: &Document, new: &Document) -> (Document, HashMap<String, Change>) {
    let d = diff(old, new);
    let mut merged = new.clone();
    let mut changes = HashMap::new();
    for e in &d.entries {
        if e.kind == "setting" || e.kind == "comment" {
            continue;
        }
        changes.insert(e.id.clone(), e.change);
        if e.change != Change::Removed {
            continue;
        }
        match e.kind {
            "group" => {
                let mut g = old.group(&e.id).unwrap().clone();
                if g.parent
                    .as_ref()
                    .is_some_and(|p| new.group(p).is_none() && old.group(p).is_none())
                {
                    g.parent = None;
                }
                merged.groups.push(g);
            }
            "node" => merged.nodes.push(old.node(&e.id).unwrap().clone()),
            "edge" => merged.edges.push(old.edge(&e.id).unwrap().clone()),
            _ => {}
        }
    }
    // removed nodes may sit in groups that only the new version lacks
    for n in &mut merged.nodes {
        if n.group
            .as_ref()
            .is_some_and(|g| merged.groups.iter().all(|x| &x.id != g))
        {
            n.group = None;
        }
    }
    (merged, changes)
}

pub fn markdown(d: &Diff, old: &Document, new: &Document) -> String {
    if d.is_empty() {
        return "No changes.\n".into();
    }
    let mut out = String::new();
    let count = |c: Change| {
        d.entries
            .iter()
            .filter(|e| e.change == c && e.kind != "setting")
            .count()
    };
    out.push_str(&format!(
        "**{} added, {} removed, {} changed**\n\n",
        count(Change::Added),
        count(Change::Removed),
        count(Change::Changed)
    ));
    for e in &d.entries {
        let doc = if e.change == Change::Removed {
            old
        } else {
            new
        };
        let what = match e.kind {
            "setting" => format!("setting `{}`", e.id),
            "comment" => {
                let c = doc.comment(&e.id).unwrap();
                format!("comment `{}` \"{}\"", e.id, c.text)
            }
            _ => doc.describe(&e.id),
        };
        let sign = match e.change {
            Change::Added => "+",
            Change::Removed => "−",
            Change::Changed => "~",
        };
        out.push_str(&format!("- {sign} {what}"));
        let f: Vec<String> = e
            .fields
            .iter()
            .map(|(k, a, b)| format!("{k}: `{a}` → `{b}`"))
            .collect();
        if !f.is_empty() {
            out.push_str(&format!(" ({})", f.join(", ")));
        }
        out.push('\n');
    }
    let moves = schedule_moves(old, new);
    if !moves.is_empty() {
        out.push_str("\n**Schedule**\n\n");
        for m in moves {
            out.push_str(&format!("- {m}\n"));
        }
    }
    out
}

/// What a change does to the plan: milestones and chart ends that move
/// (`milestone `M1` moves 2026-11-05 → 2026-11-12 (+7d)`).
pub fn schedule_moves(old: &Document, new: &Document) -> Vec<String> {
    let mut out = Vec::new();
    // the last day of each task, and of each chart
    let ends = |doc: &Document| {
        let mut m: HashMap<String, i64> = HashMap::new();
        for g in doc.groups.iter().filter(|g| g.gantt) {
            let s = crate::gantt::schedule(doc, &g.id);
            let Some(o) = s.origin else { continue };
            let mut last = None;
            for (i, start, days) in s.tasks {
                let end = o + (start + days).ceil() as i64 - i64::from(days > 0.0);
                m.insert(doc.nodes[i].id.clone(), end);
                last = last.max(Some(end));
            }
            if let Some(l) = last {
                m.insert(format!("{}#end", g.id), l);
            }
        }
        m
    };
    let (a, b) = (ends(old), ends(new));
    let date = crate::gantt::format_date;
    for g in new.groups.iter().filter(|g| g.gantt) {
        for n in new
            .nodes
            .iter()
            .filter(|n| n.milestone && n.group.as_deref() == Some(&g.id))
        {
            if let (Some(x), Some(y)) = (a.get(&n.id), b.get(&n.id)) {
                if x != y {
                    out.push(format!(
                        "milestone `{}` (\"{}\") moves {} → {} ({:+}d)",
                        n.id,
                        n.display(),
                        date(*x),
                        date(*y),
                        y - x
                    ));
                }
            }
        }
        let k = format!("{}#end", g.id);
        if let (Some(x), Some(y)) = (a.get(&k), b.get(&k)) {
            if x != y {
                out.push(format!(
                    "chart `{}` (\"{}\") ends {} → {} ({:+}d)",
                    g.id,
                    g.display(),
                    date(*x),
                    date(*y),
                    y - x
                ));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl;

    #[test]
    fn detects_changes_and_merges() {
        let a = dsl::parse("a -> b\nb -> c\nc: C").unwrap();
        let b = dsl::parse("a -> b: calls\nb -> d\nc: Cee [x: 5, y: 5]").unwrap();
        let d = diff(&a, &b);
        let find = |id: &str| d.entries.iter().find(|e| e.id == id).map(|e| e.change);
        assert_eq!(find("a->b"), Some(Change::Changed));
        assert_eq!(find("b->c"), Some(Change::Removed));
        assert_eq!(find("d"), Some(Change::Added));
        assert_eq!(find("c"), Some(Change::Changed));
        let (m, changes) = merge(&a, &b);
        assert!(m.edge("b->c").is_some() && m.check().is_empty());
        assert_eq!(changes.get("b->d"), Some(&Change::Added));
        let md = markdown(&d, &a, &b);
        assert!(md.contains("label: `` → `calls`"), "{md}");
        assert!(diff(&b, &b).is_empty());
    }

    #[test]
    fn plan_changes_report_moved_milestones() {
        let a = dsl::parse("gantt g: Plan [start: 2026-10-12] {\n  Build [5d]\n  M1 [after: build, milestone]\n}\n").unwrap();
        let b = dsl::parse("gantt g: Plan [start: 2026-10-12] {\n  Build [1w]\n  M1 [after: build, milestone]\n}\n").unwrap();
        let md = markdown(&diff(&a, &b), &a, &b);
        assert!(md.contains("**Schedule**\n\n- milestone `m1` (\"M1\") moves 2026-10-17 → 2026-10-19 (+2d)\n- chart `g` (\"Plan\") ends 2026-10-17 → 2026-10-19 (+2d)\n"), "{md}");
    }
}
