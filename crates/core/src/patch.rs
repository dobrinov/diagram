//! Edits that keep a file as it was written.
//!
//! Ops change the document; [`patch`] then rewrites only the statements
//! whose elements changed, puts new ones next to their kind (inside their
//! section, after the last box, arrow or comment) and leaves every other
//! line, blank line and `#` note as it was. A changed mindmap or gantt chart
//! is rewritten as a whole block. The result is read back: when it would not
//! mean the new document, the canonical print is used instead, so a patch is
//! never wrong, at worst as noisy as reformatting.
use std::collections::HashSet;

use crate::dsl::{self, Elem};
use crate::model::*;

/// The source of `new`, written as close to `src` (the source of `old`) as
/// possible.
pub fn patch(src: &str, old: &Document, new: &Document) -> String {
    try_patch(src, old, new).unwrap_or_else(|| dsl::print(new))
}

/// [`patch`], or None when only reprinting the whole file would do.
fn try_patch(src: &str, old: &Document, new: &Document) -> Option<String> {
    let canon = dsl::print(new);
    // nothing changed: the source as it is (even while it has errors)
    if old == new || dsl::print(old) == canon {
        return Some(src.to_string());
    }
    let reads = |text: &str| dsl::parse(text).ok().map(|d| dsl::print(&d));
    // the source has to be what `old` was read from
    if reads(src).as_deref() != Some(dsl::print(old).as_str()) {
        return None;
    }
    let mut out = edit(src, old, new)?;
    if reads(&out).as_deref() != Some(canon.as_str()) {
        return None;
    }
    if !src.ends_with('\n') {
        out.pop();
    }
    Some(out)
}

fn pad_of(line: &str) -> &str {
    &line[..line.len() - line.trim_start().len()]
}

fn push_text(out: &mut Vec<String>, text: &str) {
    out.extend(text.lines().map(str::to_string));
}

/// Which elements a mindmap (by its centre) or a gantt chart holds in `doc`.
fn block_elems(doc: &Document, head: &Elem) -> Vec<Elem> {
    match head {
        Elem::Node(root) => {
            let forest = crate::mindmap::Forest::new(doc);
            let Some(r) = doc.nodes.iter().position(|n| n.id == *root) else {
                return Vec::new();
            };
            doc.nodes
                .iter()
                .enumerate()
                .filter(|(i, _)| forest.root[*i] == Some(r))
                .map(|(_, n)| Elem::Node(n.id.clone()))
                .collect()
        }
        Elem::Group(g) => std::iter::once(Elem::Group(g.clone()))
            .chain(
                doc.nodes
                    .iter()
                    .filter(|n| n.group.as_deref() == Some(g.as_str()))
                    .map(|n| Elem::Node(n.id.clone())),
            )
            // a sequence diagram's messages are part of it
            .chain(
                crate::sequence::messages_of(doc, g)
                    .into_iter()
                    .map(|e| Elem::Edge(e.id.clone())),
            )
            .collect(),
        _ => Vec::new(),
    }
}

/// A mindmap or gantt chart as printed, or None when `doc` has no such block.
fn block_text(doc: &Document, head: &Elem, pad: &str) -> Option<String> {
    let mut out = String::new();
    match head {
        Elem::Node(root) => {
            let n = doc.node(root)?;
            let in_map = dsl::map_members(doc);
            if !in_map(n) || n.parent.is_some() {
                return None;
            }
            dsl::map_block(doc, n, pad, &mut out);
        }
        Elem::Group(g) => {
            let g = doc.group(g)?;
            if g.chart.is_some() {
                dsl::chart_block(doc, g, pad, &mut out);
                return Some(out);
            }
            if g.table {
                dsl::table_block(doc, g, pad, &mut out);
                return Some(out);
            }
            if g.entity {
                dsl::entity_block(doc, g, pad, &mut out);
                return Some(out);
            }
            if g.adr.is_some() {
                dsl::adr_block(doc, g, pad, &mut out);
                return Some(out);
            }
            if g.sequence {
                dsl::sequence_block(doc, g, pad, &mut out);
                return Some(out);
            }
            if !g.gantt && !g.risks {
                return None;
            }
            dsl::gantt_block(doc, g, pad, &mut out);
        }
        _ => return None,
    }
    Some(out)
}

/// Where a block sits: the group around a mindmap or gantt chart.
fn container(doc: &Document, head: &Elem) -> Option<Option<String>> {
    match head {
        Elem::Node(id) => doc.node(id).map(|n| n.group.clone()),
        Elem::Group(id) => doc.group(id).map(|g| g.parent.clone()),
        _ => None,
    }
}

/// Everything inside group `g` (nested groups, their nodes), and `g`.
fn group_subtree(doc: &Document, g: &str) -> Vec<Elem> {
    let mut groups = vec![g.to_string()];
    loop {
        let more: Vec<String> = doc
            .groups
            .iter()
            .filter(|x| x.parent.as_ref().is_some_and(|p| groups.contains(p)))
            .filter(|x| !groups.contains(&x.id))
            .map(|x| x.id.clone())
            .collect();
        if more.is_empty() {
            break;
        }
        groups.extend(more);
    }
    let forest = crate::mindmap::Forest::new(doc);
    let mut out: Vec<Elem> = groups.iter().map(|g| Elem::Group(g.clone())).collect();
    for (i, n) in doc.nodes.iter().enumerate() {
        // topics belong to their centre's group
        let at = forest.root[i].map(|r| &doc.nodes[r]).unwrap_or(n);
        if at.group.as_ref().is_some_and(|x| groups.contains(x)) {
            out.push(Elem::Node(n.id.clone()));
        }
    }
    out
}

fn exists(doc: &Document, e: &Elem) -> bool {
    match e {
        Elem::Node(id) => doc.node(id).is_some(),
        Elem::Edge(id) => doc.edge(id).is_some(),
        Elem::Group(id) => doc.group(id).is_some(),
        Elem::Comment(id) => doc.comment(id).is_some(),
        Elem::Setting(k) => dsl::setting_line(doc, k).is_some(),
    }
}

fn same(old: &Document, new: &Document, e: &Elem) -> bool {
    match e {
        Elem::Node(id) => old.node(id) == new.node(id),
        Elem::Edge(id) => old.edge(id) == new.edge(id),
        Elem::Group(id) => old.group(id) == new.group(id),
        Elem::Comment(id) => old.comment(id) == new.comment(id),
        Elem::Setting(k) => dsl::setting_line(old, k) == dsl::setting_line(new, k),
    }
}

/// An element's statement(s) as printed on its own.
fn statement(doc: &Document, e: &Elem, pad: &str, out: &mut Vec<String>) {
    match e {
        Elem::Node(id) => {
            if let Some(n) = doc.node(id) {
                out.push(format!("{pad}{}", dsl::node_line(n)));
            }
        }
        Elem::Edge(id) => {
            if let Some(x) = doc.edge(id) {
                out.push(format!("{pad}{}", dsl::edge_line(x)));
            }
        }
        Elem::Comment(id) => {
            if let Some(c) = doc.comment(id) {
                let mut s = String::new();
                dsl::comment_lines(c, &mut s);
                out.extend(s.lines().map(|l| format!("{pad}{l}")));
            }
        }
        Elem::Setting(k) => out.extend(dsl::setting_line(doc, k)),
        Elem::Group(_) => {}
    }
}

/// Where new statements of each kind go: after the last one of their kind.
#[derive(Default)]
struct Anchors {
    settings: Option<usize>,
    nodes: Option<usize>,
    blocks: Option<usize>,
    edges: Option<usize>,
    comments: Option<usize>,
    /// The first comment line, so new arrows go before the comments.
    first_comment: Option<usize>,
}

/// Elements that were renamed: same place in the document, a new id.
fn renames(old: &Document, new: &Document) -> std::collections::HashMap<Elem, Elem> {
    let mut out = std::collections::HashMap::new();
    let mut pair = |a: &str, b: &str, gone: bool, fresh: bool, kind: fn(String) -> Elem| {
        if a != b && gone && fresh {
            out.insert(kind(a.to_string()), kind(b.to_string()));
        }
    };
    for (o, n) in old.nodes.iter().zip(&new.nodes) {
        pair(
            &o.id,
            &n.id,
            new.node(&o.id).is_none(),
            old.node(&n.id).is_none(),
            Elem::Node,
        );
    }
    for (o, n) in old.edges.iter().zip(&new.edges) {
        pair(
            &o.id,
            &n.id,
            new.edge(&o.id).is_none(),
            old.edge(&n.id).is_none(),
            Elem::Edge,
        );
    }
    for (o, n) in old.groups.iter().zip(&new.groups) {
        pair(
            &o.id,
            &n.id,
            new.group(&o.id).is_none(),
            old.group(&n.id).is_none(),
            Elem::Group,
        );
    }
    out
}

fn edit(src: &str, old: &Document, new: &Document) -> Option<String> {
    let (_, errors, mut info) = dsl::parse_lines(src);
    if !errors.is_empty() {
        return None;
    }
    // a renamed element keeps its line: read the old source with new ids
    let renamed = renames(old, new);
    if !renamed.is_empty() {
        let group = |g: &Option<String>| {
            g.as_ref()
                .map(|g| match renamed.get(&Elem::Group(g.clone())) {
                    Some(Elem::Group(n)) => n.clone(),
                    _ => g.clone(),
                })
        };
        for li in &mut info {
            for e in &mut li.elems {
                if let Some(n) = renamed.get(e) {
                    *e = n.clone();
                }
            }
            li.close = group(&li.close);
        }
        // and compare with the old document as if it had the new ids
        let mut o = old.clone();
        for (a, b) in &renamed {
            match (a, b) {
                (Elem::Node(a), Elem::Node(b)) => {
                    if let Some(n) = o.nodes.iter_mut().find(|n| n.id == *a) {
                        n.id = b.clone();
                    }
                }
                (Elem::Edge(a), Elem::Edge(b)) => {
                    if let Some(e) = o.edges.iter_mut().find(|e| e.id == *a) {
                        e.id = b.clone();
                    }
                }
                (Elem::Group(a), Elem::Group(b)) => {
                    if let Some(g) = o.groups.iter_mut().find(|g| g.id == *a) {
                        g.id = b.clone();
                    }
                }
                _ => {}
            }
        }
        for n in &mut o.nodes {
            n.group = group(&n.group);
        }
        for g in &mut o.groups {
            g.parent = group(&g.parent);
        }
        let fresh: HashSet<Elem> = renamed.into_values().collect();
        return edit_lines(src, &o, new, &info, &fresh);
    }
    edit_lines(src, old, new, &info, &HashSet::new())
}

/// [`edit`], with `info` read as if by `old`; `renamed` elements are written
/// again (their lines have their old ids).
fn edit_lines(
    src: &str,
    old: &Document,
    new: &Document,
    info: &[dsl::LineInfo],
    renamed: &HashSet<Elem>,
) -> Option<String> {
    let same = |e: &Elem| !renamed.contains(e) && same(old, new, e);
    let lines: Vec<&str> = src.lines().collect();
    let in_map = dsl::map_members(new);
    let in_map_old = dsl::map_members(old);

    // a group may change its title or colour; one that is gone takes its
    // lines with it, but only when everything in it is gone too
    for g in &old.groups {
        match new.group(&g.id) {
            Some(n)
                if n.parent != g.parent
                    || n.lanes != g.lanes
                    || n.kanban != g.kanban
                    || n.sequence != g.sequence
                    || n.gantt != g.gantt
                    || n.risks != g.risks
                    || n.chart.is_some() != g.chart.is_some()
                    || n.table != g.table
                    || n.entity != g.entity
                    || n.adr.is_some() != g.adr.is_some() =>
            {
                return None
            }
            Some(_) => {}
            None => {
                if group_subtree(old, &g.id).iter().any(|e| exists(new, e)) {
                    return None;
                }
            }
        }
    }

    // 1. what stays where it is written
    let mut placed: HashSet<Elem> = HashSet::new();
    for (i, li) in info.iter().enumerate() {
        if li.block == Some(i) {
            let Some(head) = li.elems.first() else {
                return None;
            };
            if exists(new, head) {
                if container(old, head) != container(new, head) {
                    return None;
                }
                placed.extend(block_elems(new, head));
            }
            continue;
        }
        if li.block.is_some() {
            continue;
        }
        for e in &li.elems {
            let keep = match e {
                Elem::Node(id) => match (old.node(id), new.node(id)) {
                    (Some(o), Some(n)) => o.group == n.group && !in_map(n) && !in_map_old(o),
                    _ => false,
                },
                // a setting stays on its line, even at its default
                Elem::Setting(_) => true,
                _ => exists(new, e),
            };
            if keep {
                placed.insert(e.clone());
            }
        }
    }

    // 2. the lines, kept or rewritten, with what is new in a section added
    // before its closing `}`
    let mut out: Vec<String> = Vec::new();
    let mut done: HashSet<Elem> = HashSet::new();
    let mut at = Anchors::default();
    let mut i = 0;
    while i < lines.len() {
        let (line, li) = (lines[i], &info[i]);
        let pad = pad_of(line);
        if li.block == Some(i) {
            // a mindmap or gantt chart: kept as written unless it changed
            let head = li.elems.first()?;
            let end = (i..lines.len())
                .take_while(|&j| info[j].block == Some(i))
                .last()
                .unwrap_or(i);
            match (block_text(old, head, pad), block_text(new, head, pad)) {
                (_, None) => {}
                (Some(o), Some(n))
                    if o == n && !block_elems(new, head).iter().any(|e| renamed.contains(e)) =>
                {
                    out.extend(lines[i..=end].iter().map(|l| l.to_string()))
                }
                (_, Some(n)) => push_text(&mut out, &n),
            }
            done.extend(block_elems(new, head));
            if li.depth == 0 {
                at.blocks = Some(out.len());
            }
            i = end + 1;
            continue;
        }
        if let Some(g) = &li.close {
            if new.group(g).is_some() {
                let inner = format!("{pad}  ");
                for n in new
                    .nodes
                    .iter()
                    .filter(|n| n.group.as_deref() == Some(g.as_str()))
                {
                    let e = Elem::Node(n.id.clone());
                    if placed.contains(&e) || done.contains(&e) {
                        continue;
                    }
                    if !in_map(n) {
                        statement(new, &e, &inner, &mut out);
                        done.insert(e);
                    } else if n.parent.is_none() {
                        let mut s = String::new();
                        dsl::map_block(new, n, &inner, &mut s);
                        push_text(&mut out, &s);
                        done.extend(block_elems(new, &e));
                    }
                }
                for child in new
                    .groups
                    .iter()
                    .filter(|c| c.parent.as_deref() == Some(g.as_str()))
                {
                    if old.group(&child.id).is_none() {
                        let mut s = String::new();
                        dsl::group_block(new, child, li.depth, &mut s, &in_map);
                        push_text(&mut out, &s);
                        done.extend(group_subtree(new, &child.id));
                    }
                }
                out.push(line.to_string());
                if li.depth == 1 {
                    at.blocks = Some(out.len());
                }
            }
            i += 1;
            continue;
        }
        if li.elems.is_empty() {
            // blank lines, `#` notes
            out.push(line.to_string());
            i += 1;
            continue;
        }
        if let [Elem::Group(g)] = &li.elems[..] {
            match new.group(g) {
                None => {}
                Some(_) if same(&li.elems[0]) => out.push(line.to_string()),
                Some(ng) => {
                    let open = if line.trim_end().ends_with("{}") {
                        " {}"
                    } else {
                        " {"
                    };
                    out.push(format!("{pad}{}{open}", dsl::group_header(new, ng)));
                }
            }
            done.insert(li.elems[0].clone());
            i += 1;
            continue;
        }
        let unchanged = li.elems.iter().all(|e| placed.contains(e) && same(e));
        if unchanged {
            out.push(line.to_string());
            done.extend(li.elems.iter().cloned());
        } else {
            for e in &li.elems {
                if placed.contains(e) && !done.contains(e) {
                    statement(new, e, pad, &mut out);
                    done.insert(e.clone());
                }
            }
        }
        for e in &li.elems {
            match e {
                Elem::Setting(_) => at.settings = Some(out.len()),
                Elem::Node(_) if li.depth == 0 => at.nodes = Some(out.len()),
                Elem::Edge(_) => at.edges = Some(out.len()),
                Elem::Comment(_) => {
                    at.first_comment.get_or_insert(out.len() - 1);
                    at.comments = Some(out.len());
                }
                _ => {}
            }
        }
        i += 1;
    }

    // 3. what is new and has no section to go into: next to its kind
    let mut inserts: Vec<(usize, bool, Vec<String>)> = Vec::new();
    let mut add = |anchor: Option<usize>, fallback: usize, chunk: Vec<String>| {
        if !chunk.is_empty() {
            inserts.push((anchor.unwrap_or(fallback), anchor.is_none(), chunk));
        }
    };
    let mut chunk = Vec::new();
    for k in dsl::SETTINGS {
        let e = Elem::Setting(k.to_string());
        if exists(new, &e) && !done.contains(&e) {
            statement(new, &e, "", &mut chunk);
        }
    }
    add(at.settings, 0, std::mem::take(&mut chunk));
    let after_settings = at.settings.unwrap_or(0);
    for n in &new.nodes {
        let e = Elem::Node(n.id.clone());
        if done.contains(&e) || placed.contains(&e) || in_map(n) {
            continue;
        }
        match &n.group {
            Some(_) => return None, // in a section with no `}` to put it before
            None => {
                // a bare node that an arrow mentions needs no line
                let bare = n.label.is_empty() && *n == Node::new(&n.id);
                if bare && new.edges.iter().any(|x| x.from == n.id || x.to == n.id) {
                    continue;
                }
                statement(new, &e, "", &mut chunk);
            }
        }
    }
    add(at.nodes, after_settings, std::mem::take(&mut chunk));
    let after_nodes = at.nodes.unwrap_or(after_settings);
    for n in &new.nodes {
        let e = Elem::Node(n.id.clone());
        if !in_map(n) || done.contains(&e) || placed.contains(&e) {
            continue;
        }
        if n.parent.is_some() || n.group.is_some() {
            return None;
        }
        if !chunk.is_empty() {
            chunk.push(String::new());
        }
        let mut s = String::new();
        dsl::map_block(new, n, "", &mut s);
        push_text(&mut chunk, &s);
        done.extend(block_elems(new, &e));
    }
    for g in new.groups.iter().filter(|g| g.parent.is_none()) {
        let e = Elem::Group(g.id.clone());
        if done.contains(&e) || placed.contains(&e) {
            continue;
        }
        if !chunk.is_empty() {
            chunk.push(String::new());
        }
        let mut s = String::new();
        dsl::group_block(new, g, 0, &mut s, &in_map);
        push_text(&mut chunk, &s);
        done.extend(group_subtree(new, &g.id));
    }
    add(at.blocks, after_nodes, std::mem::take(&mut chunk));
    for x in &new.edges {
        let e = Elem::Edge(x.id.clone());
        if !done.contains(&e) {
            statement(new, &e, "", &mut chunk);
        }
    }
    let end = out.len();
    add(
        at.edges,
        at.first_comment.unwrap_or(end),
        std::mem::take(&mut chunk),
    );
    for c in &new.comments {
        let e = Elem::Comment(c.id.clone());
        if !done.contains(&e) {
            statement(new, &e, "", &mut chunk);
        }
    }
    add(at.comments, end, chunk);

    // later positions first, so earlier ones stay put (and at the same spot,
    // the later kind first); a new section is set off by blank lines
    let mut inserts: Vec<(usize, (usize, bool, Vec<String>))> =
        inserts.into_iter().enumerate().collect();
    inserts.sort_by(|a, b| (b.1 .0, b.0).cmp(&(a.1 .0, a.0)));
    for (_, (pos, fresh, mut chunk)) in inserts {
        if fresh {
            if pos > 0 && out.get(pos - 1).is_some_and(|l| !l.trim().is_empty()) {
                chunk.insert(0, String::new());
            }
            if out.get(pos).is_some_and(|l| !l.trim().is_empty()) {
                chunk.push(String::new());
            }
        }
        out.splice(pos..pos, chunk);
    }
    let mut text = out.join("\n");
    text.push('\n');
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edited(src: &str, ops: &str) -> String {
        let old = dsl::parse(src).unwrap();
        let mut new = old.clone();
        crate::ops::apply(&mut new, &crate::ops::parse_ops(ops).unwrap()).unwrap();
        patch(src, &old, &new)
    }

    const SRC: &str = "title: Shop\n\n# people\nweb, mobile -> cdn -> gw\n\ngroup back: Backend [blue] {\n  # the main one\n  api: Orders API\n  db: Postgres [cylinder]\n}\n\napi -> db\ncomment on api by deyan: Split it?\n";

    #[test]
    fn untouched_lines_stay_as_written() {
        // a label: one line changes
        let out = edited(
            SRC,
            r#"{"ops":[{"op":"update_node","id":"db","label":"Postgres 16"}]}"#,
        );
        assert_eq!(
            out,
            SRC.replace("db: Postgres [cylinder]", "db: Postgres 16 [cylinder]")
        );
        // a new box goes into its section, a new arrow after the arrows
        let out = edited(
            SRC,
            r#"{"ops":[{"op":"add_node","id":"cache","label":"Redis","group":"back"},{"op":"add_edge","from":"api","to":"cache"}]}"#,
        );
        assert_eq!(
            out,
            SRC.replace(
                "  db: Postgres [cylinder]\n}",
                "  db: Postgres [cylinder]\n  cache: Redis\n}"
            )
            .replace("api -> db\n", "api -> db\napi -> cache\n")
        );
        // a resolved comment, a new title
        let out = edited(
            SRC,
            r#"{"ops":[{"op":"resolve_comment","id":"c1"},{"op":"set","key":"title","value":"Store"}]}"#,
        );
        assert_eq!(
            out,
            SRC.replace("title: Shop", "title: Store").replace(
                "comment on api by deyan: Split it?\n",
                "comment c1 on api by deyan: Split it? [resolved]\n"
            )
        );
    }

    #[test]
    fn chains_split_only_when_they_change() {
        let out = edited(SRC, r#"{"ops":[{"op":"remove","id":"mobile->cdn"}]}"#);
        assert!(out.contains("web -> cdn\ncdn -> gw\n"), "{out}");
        assert!(
            out.contains("# people\n") && out.contains("  # the main one\n  api: Orders API\n"),
            "{out}"
        );
    }

    #[test]
    fn blocks_and_new_sections() {
        let src = "mindmap m: Goals {\n  Fast\n}\n\n# plans\ngantt g: Plan [start: 2026-10-12] {\n  Design [5d]\n}\n";
        let out = edited(
            src,
            r#"{"ops":[{"op":"add_outline","parent":"fast","outline":"Under 1s"}]}"#,
        );
        assert_eq!(out, src.replace("  Fast\n", "  Fast\n    Under 1s\n"));
        let out = edited(
            src,
            r#"{"ops":[{"op":"add_node","id":"a","label":"Alone"},{"op":"add_edge","from":"a","to":"fast"},{"op":"add_comment","on":"a","text":"Why?"}]}"#,
        );
        assert!(out.starts_with("a: Alone\n\nmindmap m"), "{out}");
        assert!(
            out.ends_with("}\n\na -> fast\n\ncomment c1 on a: Why?\n"),
            "{out}"
        );
        // a group removed with what is in it
        let src = "x: X\ngroup s: S {\n  a: A\n}\nx -> a\n";
        let out = edited(
            src,
            r#"{"ops":[{"op":"remove","id":"s","keep_members":false}]}"#,
        );
        assert_eq!(out, "x: X\n");
    }
}

#[cfg(test)]
mod examples {
    use super::*;

    /// Typical edits on every example board: each one keeps the file as
    /// written (no fallback to reprinting it) and changes few lines.
    #[test]
    fn edits_on_the_examples_stay_small() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
        let mut files: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .chain(std::fs::read_dir(dir.join("stress")).unwrap())
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "diagram"))
            .collect();
        files.sort();
        let mut report = String::new();
        for f in files {
            let src = std::fs::read_to_string(&f).unwrap();
            let old = dsl::parse(&src).unwrap();
            let in_map = dsl::map_members(&old);
            let plain = old.nodes.iter().find(|n| !in_map(n) && n.group.is_none());
            let grouped = old.nodes.iter().find(|n| !in_map(n) && n.group.is_some());
            let topic = old.nodes.iter().find(|n| n.parent.is_some());
            let mut batches: Vec<(String, String)> = Vec::new();
            if let Some(n) = plain.or(grouped) {
                batches.push((
                    "label".into(),
                    format!(
                        r#"{{"op":"update_node","id":"{}","label":"Renamed box"}}"#,
                        n.id
                    ),
                ));
                batches.push((
                    "rename".into(),
                    format!(r#"{{"op":"rename","id":"{}","to":"zz-renamed"}}"#, n.id),
                ));
                batches.push((
                    "remove".into(),
                    format!(r#"{{"op":"remove","id":"{}"}}"#, n.id),
                ));
                batches.push((
                    "comment".into(),
                    format!(r#"{{"op":"add_comment","on":"{}","text":"Why?"}}"#, n.id),
                ));
            }
            if let Some(n) = grouped {
                batches.push((
                    "add in group".into(),
                    format!(
                        r#"{{"op":"add_node","id":"zz-new","label":"New","group":"{}"}}"#,
                        n.group.as_ref().unwrap()
                    ),
                ));
            }
            if let Some(e) = old.edges.first() {
                batches.push((
                    "edge label".into(),
                    format!(
                        r#"{{"op":"update_edge","id":"{}","label":"changed"}}"#,
                        e.id
                    ),
                ));
                batches.push((
                    "edge remove".into(),
                    format!(r#"{{"op":"remove","id":"{}"}}"#, e.id),
                ));
            }
            if let (Some(a), Some(b)) = (old.nodes.first(), old.nodes.last()) {
                batches.push((
                    "edge add".into(),
                    format!(
                        r#"{{"op":"add_edge","from":"{}","to":"{}","label":"new"}}"#,
                        a.id, b.id
                    ),
                ));
            }
            if let Some(t) = topic {
                batches.push((
                    "outline".into(),
                    format!(
                        r#"{{"op":"add_outline","parent":"{}","outline":"Child\n  Grandchild"}}"#,
                        t.id
                    ),
                ));
            }
            if let Some(c) = old.comments.first() {
                batches.push((
                    "resolve".into(),
                    format!(
                        r#"{{"op":"resolve_comment","id":"{}","resolution":"done"}}"#,
                        c.id
                    ),
                ));
            }
            for (what, op) in batches {
                let mut new = old.clone();
                let ops = crate::ops::parse_ops(&format!(r#"{{"ops":[{op}]}}"#)).unwrap();
                if crate::ops::apply(&mut new, &ops).is_err() {
                    continue;
                }
                let canon = dsl::print(&new);
                let Some(out) = try_patch(&src, &old, &new) else {
                    report.push_str(&format!(
                        "{}: {what}: fell back to reprinting\n",
                        f.display()
                    ));
                    continue;
                };
                assert_eq!(dsl::print(&dsl::parse(&out).unwrap()), canon);
                let changed = |a: &str, b: &str| {
                    let (a, b): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
                    a.iter().filter(|l| !b.contains(l)).count()
                        + b.iter().filter(|l| !a.contains(l)).count()
                };
                // one statement (a comment with its replies, a chain split
                // up, a mindmap or gantt block) changes, give or take
                let n = changed(&src, &out);
                if n > 12 {
                    report.push_str(&format!("{}: {what}: {n} lines changed\n", f.display()));
                }
            }
        }
        assert!(report.is_empty(), "{report}");
    }
}
