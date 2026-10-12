//! Reading part of a board: selectors pick elements, and the answer is the
//! diagram language for just those (with the sections they sit in), so an
//! agent reads a few lines instead of the whole file.
//!
//! | Selector | Picks |
//! |---|---|
//! | `ID` | that box, group, arrow or comment |
//! | `group:ID` (also `chart:`, `lanes:`, `section:`) | a group and everything in it |
//! | `map:ID` | a topic (or centre) and its branches |
//! | `near:ID` | a box, its arrows and the boxes at their other ends |
//! | `label~TEXT` | boxes and groups whose label or detail contains TEXT |
//! | `kind:box\|topic\|task\|milestone\|risk\|sticky\|decision\|group\|chart\|matrix\|edge\|comment` | elements of a kind |
//! | `owner:NAME` · `tag:NAME` · `color:NAME` | boxes with that owner, tag or colour |
//! | `comments:open` · `comments` | open (or all) comments and what they are on |
//!
//! Several selectors give everything any of them picks.
use std::collections::HashSet;

use crate::model::*;

/// The ids `selectors` pick, in document order (nodes, groups, edges,
/// comments). Unknown selectors are an error.
pub fn select(doc: &Document, selectors: &[&str]) -> Result<Vec<String>, String> {
    let mut picked: HashSet<String> = HashSet::new();
    for sel in selectors {
        let sel = sel.trim();
        if sel.is_empty() {
            continue;
        }
        let found = pick(doc, sel)?;
        if found.is_empty() {
            return Err(format!("nothing matches `{sel}`"));
        }
        picked.extend(found);
    }
    let order = doc
        .nodes
        .iter()
        .map(|n| &n.id)
        .chain(doc.groups.iter().map(|g| &g.id))
        .chain(doc.edges.iter().map(|e| &e.id))
        .chain(doc.comments.iter().map(|c| &c.id));
    Ok(order.filter(|id| picked.contains(*id)).cloned().collect())
}

fn subtree_groups(doc: &Document, g: &str) -> Vec<String> {
    let mut out = vec![g.to_string()];
    loop {
        let more: Vec<String> = doc
            .groups
            .iter()
            .filter(|x| x.parent.as_ref().is_some_and(|p| out.contains(p)) && !out.contains(&x.id))
            .map(|x| x.id.clone())
            .collect();
        if more.is_empty() {
            return out;
        }
        out.extend(more);
    }
}

fn branches(doc: &Document, id: &str, out: &mut Vec<String>) {
    for c in doc.children(id) {
        if !out.contains(&c.id) {
            out.push(c.id.clone());
            branches(doc, &c.id, out);
        }
    }
}

fn pick(doc: &Document, sel: &str) -> Result<Vec<String>, String> {
    let has = |id: &str| doc.element(id).is_some();
    if let Some((key, val)) = sel.split_once(':') {
        let val = val.trim();
        let low = val.to_lowercase();
        return Ok(match key {
            "group" | "chart" | "lanes" | "section" | "lane" | "matrix" | "risks" => {
                if doc.group(val).is_none() {
                    return Err(format!("no group `{val}`"));
                }
                let groups = subtree_groups(doc, val);
                let mut out = groups.clone();
                for n in &doc.nodes {
                    if n.group.as_ref().is_some_and(|g| groups.contains(g)) {
                        out.push(n.id.clone());
                        branches(doc, &n.id, &mut out);
                    }
                }
                let inside: Vec<String> = out.clone();
                out.extend(
                    doc.edges
                        .iter()
                        .filter(|e| inside.contains(&e.from) && inside.contains(&e.to))
                        .map(|e| e.id.clone()),
                );
                out
            }
            "map" => {
                if doc.node(val).is_none() {
                    return Err(format!("no topic `{val}`"));
                }
                let mut out = vec![val.to_string()];
                branches(doc, val, &mut out);
                out
            }
            "near" => {
                if !has(val) {
                    return Err(format!("no element `{val}`"));
                }
                let mut out = vec![val.to_string()];
                for e in &doc.edges {
                    if e.from == val || e.to == val {
                        out.push(e.id.clone());
                        out.push(if e.from == val { e.to.clone() } else { e.from.clone() });
                    }
                }
                out
            }
            "kind" => {
                let in_map = crate::dsl::map_members(doc);
                let task = |n: &Node| {
                    n.group
                        .as_deref()
                        .and_then(|g| doc.group(g))
                        .is_some_and(|g| g.gantt)
                };
                match low.as_str() {
                    "edge" | "arrow" => doc.edges.iter().map(|e| e.id.clone()).collect(),
                    "comment" => doc.comments.iter().map(|c| c.id.clone()).collect(),
                    "group" | "section" => doc.groups.iter().map(|g| g.id.clone()).collect(),
                    "chart" | "gantt" => doc.groups.iter().filter(|g| g.gantt).map(|g| g.id.clone()).collect(),
                    "matrix" | "risks" => doc.groups.iter().filter(|g| g.risks).map(|g| g.id.clone()).collect(),
                    "plot" | "graph" | "data" => doc.groups.iter().filter(|g| g.chart.is_some()).map(|g| g.id.clone()).collect(),
                    "table" => doc.groups.iter().filter(|g| g.table).map(|g| g.id.clone()).collect(),
                    "lanes" | "swimlanes" => doc.groups.iter().filter(|g| g.lanes).map(|g| g.id.clone()).collect(),
                    k => doc
                        .nodes
                        .iter()
                        .filter(|n| match k {
                            "topic" => in_map(n),
                            "map" | "mindmap" => in_map(n) && n.parent.is_none(),
                            "task" => task(n),
                            "milestone" => n.milestone,
                            "risk" => crate::risks::matrix_of(doc, n).is_some(),
                            "row" | "datum" => {
                                crate::charts::chart_of(doc, n).is_some() || crate::tables::table_of(doc, n).is_some()
                            }
                            "sticky" => n.shape == Shape::Sticky,
                            "decision" => n.shape == Shape::Diamond,
                            "box" | "node" => !in_map(n) && !task(n),
                            _ => false,
                        })
                        .map(|n| n.id.clone())
                        .collect(),
                }
            }
            "owner" => doc
                .nodes
                .iter()
                .filter(|n| n.owner.as_deref().is_some_and(|o| o.to_lowercase() == low))
                .map(|n| n.id.clone())
                .collect(),
            "tag" => doc
                .nodes
                .iter()
                .filter(|n| n.tags.iter().any(|t| t.to_lowercase() == low))
                .map(|n| n.id.clone())
                .collect(),
            "color" | "colour" => doc
                .nodes
                .iter()
                .filter(|n| n.color.as_deref() == Some(low.as_str()))
                .map(|n| n.id.clone())
                .chain(
                    doc.groups
                        .iter()
                        .filter(|g| g.color.as_deref() == Some(low.as_str()))
                        .map(|g| g.id.clone()),
                )
                .collect(),
            "comments" => doc
                .comments
                .iter()
                .filter(|c| low != "open" || c.status == Status::Open)
                .flat_map(|c| std::iter::once(c.id.clone()).chain(c.on.clone()))
                .collect(),
            _ => return Err(format!("unknown selector `{key}:` (use group:, chart:, map:, near:, kind:, owner:, tag:, color:, comments:, label~)")),
        });
    }
    if let Some((key, text)) = sel.split_once('~') {
        if key != "label" {
            return Err(format!("unknown selector `{key}~` (use label~TEXT)"));
        }
        let t = text.trim().to_lowercase();
        let hit = |s: &str| s.to_lowercase().contains(&t);
        return Ok(doc
            .nodes
            .iter()
            .filter(|n| hit(n.display()) || n.detail.as_deref().is_some_and(hit))
            .map(|n| n.id.clone())
            .chain(
                doc.groups
                    .iter()
                    .filter(|g| hit(g.display()))
                    .map(|g| g.id.clone()),
            )
            .collect());
    }
    if has(sel) {
        let mut out = vec![sel.to_string()];
        // a group brings what is in it, as `group:` does
        if doc.group(sel).is_some() {
            out = pick(doc, &format!("group:{sel}"))?;
        }
        return Ok(out);
    }
    Err(format!("no element `{sel}`"))
}

/// The picked elements as a board of their own: the sections they sit in
/// (without their other contents), the path from a mindmap's centre to its
/// topics, arrows and comments. Printing it gives valid diagram language.
pub fn subset(doc: &Document, ids: &[String]) -> Document {
    let want: HashSet<&str> = ids.iter().map(|s| s.as_str()).collect();
    let mut keep_nodes: HashSet<String> = HashSet::new();
    let mut keep_groups: HashSet<String> = HashSet::new();
    for n in &doc.nodes {
        if want.contains(n.id.as_str()) {
            // a topic needs the topics above it
            let mut at = Some(n);
            while let Some(x) = at {
                if !keep_nodes.insert(x.id.clone()) {
                    break;
                }
                at = x.parent.as_deref().and_then(|p| doc.node(p));
            }
        }
    }
    for g in &doc.groups {
        if want.contains(g.id.as_str()) {
            keep_groups.insert(g.id.clone());
        }
    }
    // the sections around what is kept
    let mut around: Vec<String> = doc
        .nodes
        .iter()
        .filter(|n| keep_nodes.contains(&n.id))
        .filter_map(|n| n.group.clone())
        .chain(
            doc.groups
                .iter()
                .filter(|g| keep_groups.contains(&g.id))
                .filter_map(|g| g.parent.clone()),
        )
        .collect();
    while let Some(g) = around.pop() {
        if keep_groups.insert(g.clone()) {
            if let Some(p) = doc.group(&g).and_then(|g| g.parent.clone()) {
                around.push(p);
            }
        }
    }
    Document {
        title: doc.title.clone(),
        description: None,
        themes: Vec::new(),
        settings: Settings {
            palette: None,
            ..doc.settings.clone()
        },
        nodes: doc
            .nodes
            .iter()
            .filter(|n| keep_nodes.contains(&n.id))
            .cloned()
            .collect(),
        groups: doc
            .groups
            .iter()
            .filter(|g| keep_groups.contains(&g.id))
            .cloned()
            .collect(),
        edges: doc
            .edges
            .iter()
            .filter(|e| want.contains(e.id.as_str()))
            .cloned()
            .collect(),
        comments: doc
            .comments
            .iter()
            .filter(|c| want.contains(c.id.as_str()))
            .cloned()
            .collect(),
    }
}

/// [`select`] then [`subset`], printed: what an agent asked to see.
pub fn query(doc: &Document, selectors: &[&str]) -> Result<String, String> {
    let ids = select(doc, selectors)?;
    Ok(crate::dsl::print(&subset(doc, &ids)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn board() -> Document {
        crate::dsl::parse("title: Shop\nweb: Web app\ngroup back: Backend {\n  api: Orders API [owner: ana]\n  db: Postgres [cylinder]\n}\nmindmap m: Goals {\n  Fast\n    Under 1s\n  Cheap\n}\nweb -> api\napi -> db: SQL\ncomment on db by deyan: Which version?\n").unwrap()
    }

    #[test]
    fn selectors_pick_parts_of_a_board() {
        let d = board();
        let ids = |s: &[&str]| select(&d, s).unwrap();
        assert_eq!(ids(&["group:back"]), ["api", "db", "back", "api->db"]);
        assert_eq!(
            ids(&["near:api"]),
            ["web", "api", "db", "web->api", "api->db"]
        );
        assert_eq!(ids(&["label~post"]), ["db"]);
        assert_eq!(ids(&["owner:ana"]), ["api"]);
        assert_eq!(ids(&["comments:open"]), ["db", "c1"]);
        assert_eq!(ids(&["kind:topic"]), ["m", "fast", "under-1s", "cheap"]);
        assert!(select(&d, &["group:nope"]).is_err());
        assert!(select(&d, &["label~zzz"]).is_err());
    }

    #[test]
    fn answers_are_diagram_language() {
        let d = board();
        assert_eq!(
            query(&d, &["near:web"]).unwrap(),
            "title: Shop\n\nweb: Web app\n\ngroup back: Backend {\n  api: Orders API [owner: ana]\n}\n\nweb -> api\n"
        );
        // a deep topic comes with the topics above it
        assert_eq!(
            query(&d, &["under-1s"]).unwrap(),
            "title: Shop\n\nmindmap m: Goals {\n  Fast\n    Under 1s\n}\n"
        );
    }
}
