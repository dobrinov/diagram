//! Operations: the one way to change a document. The editor, the CLI, the
//! library API and LLM responses all express edits as a list of ops:
//!
//! ```json
//! {"ops": [
//!   {"op": "add_node", "id": "cache", "label": "Redis", "shape": "cylinder", "group": "backend"},
//!   {"op": "add_edge", "from": "api", "to": "cache", "label": "reads"},
//!   {"op": "resolve_comment", "id": "c1", "resolution": "Added a cache in front of the DB"}
//! ]}
//! ```
//!
//! A batch is atomic: if one op fails, nothing is applied. In update ops an
//! empty string clears an optional field (`"color": ""`).
use serde::{Deserialize, Serialize};

use crate::model::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Op {
    AddNode {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        #[serde(default)]
        label: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shape: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        color: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        group: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        link: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tags: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        x: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        y: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        w: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        h: Option<f64>,
    },
    UpdateNode {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shape: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        color: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        group: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        link: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tags: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        w: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        h: Option<f64>,
    },
    /// Rename an id everywhere it is referenced (edges, comments, members).
    Rename {
        id: String,
        to: String,
    },
    /// Remove a node or group (with its edges) or an edge or a comment.
    Remove {
        id: String,
        /// For groups: keep the members (moved to the parent group). Default true.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        keep_members: Option<bool>,
    },
    /// Pin a node at a position (top-left corner).
    Move {
        id: String,
        x: f64,
        y: f64,
    },
    /// Move pinned nodes by an offset (a node, or every node in a group).
    Nudge {
        id: String,
        dx: f64,
        dy: f64,
    },
    /// Let the layout place a node again; without `id`, every node.
    Unpin {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
    },
    AddEdge {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        from: String,
        to: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        arrow: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        line: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        color: Option<String>,
    },
    UpdateEdge {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        arrow: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        line: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        color: Option<String>,
    },
    AddGroup {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        #[serde(default)]
        label: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        color: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent: Option<String>,
        /// Nodes (or groups) to move into the new group.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        members: Vec<String>,
    },
    UpdateGroup {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        color: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent: Option<String>,
    },
    AddComment {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        on: Option<String>,
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        author: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        x: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        y: Option<f64>,
    },
    Reply {
        id: String,
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        author: Option<String>,
    },
    EditComment {
        id: String,
        text: String,
    },
    ResolveComment {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        resolution: Option<String>,
    },
    ReopenComment {
        id: String,
    },
    /// Document settings: title, description, direction, layout, routing, theme.
    Set {
        key: String,
        value: String,
    },
}

/// What a batch did.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Applied {
    /// Ids created by the batch, in order (useful when ids were generated).
    pub created: Vec<String>,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpError {
    /// 0-based index of the failing op.
    pub index: usize,
    pub message: String,
}

impl std::fmt::Display for OpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "op #{}: {}", self.index + 1, self.message)
    }
}

/// Apply a batch atomically.
pub fn apply(doc: &mut Document, ops: &[Op]) -> Result<Applied, OpError> {
    let mut work = doc.clone();
    let mut applied = Applied::default();
    for (index, op) in ops.iter().enumerate() {
        apply_one(&mut work, op, &mut applied.created)
            .map_err(|message| OpError { index, message })?;
        applied.count += 1;
    }
    *doc = work;
    Ok(applied)
}

fn opt(v: &Option<String>) -> Option<Option<String>> {
    v.as_ref()
        .map(|s| if s.is_empty() { None } else { Some(s.clone()) })
}

fn color(c: &str) -> Result<String, String> {
    let c = c.trim().to_ascii_lowercase();
    if valid_color(&c) {
        Ok(c)
    } else {
        Err(format!(
            "unknown colour `{c}` (use {} or #rrggbb)",
            COLORS.join(", ")
        ))
    }
}

fn shape(s: &str) -> Result<Shape, String> {
    Shape::parse(s).ok_or_else(|| format!("unknown shape `{s}` (use {})", Shape::names()))
}

fn need_container(doc: &Document, g: &str) -> Result<(), String> {
    if doc.group(g).is_none() {
        return Err(format!("no group `{g}`"));
    }
    Ok(())
}

fn new_id(doc: &Document, wanted: Option<&str>, label: &str) -> Result<String, String> {
    match wanted.filter(|s| !s.is_empty()) {
        Some(id) => {
            let id = if crate::dsl::is_ident(id) {
                id.to_string()
            } else {
                slug(id)
            };
            if doc.id_taken(&id) {
                Err(format!("id `{id}` is already used"))
            } else {
                Ok(id)
            }
        }
        None => Ok(doc.fresh_id(&slug(if label.is_empty() { "node" } else { label }))),
    }
}

fn apply_one(doc: &mut Document, op: &Op, created: &mut Vec<String>) -> Result<(), String> {
    match op {
        Op::AddNode {
            id,
            label,
            shape: sh,
            color: c,
            detail,
            group,
            link,
            tags,
            x,
            y,
            w,
            h,
        } => {
            let id = new_id(doc, id.as_deref(), label)?;
            if let Some(g) = group.as_deref().filter(|g| !g.is_empty()) {
                need_container(doc, g)?;
            }
            let mut n = Node::new(&id);
            n.label = if label == &id {
                String::new()
            } else {
                label.clone()
            };
            if let Some(s) = sh {
                n.shape = shape(s)?;
            }
            if let Some(c) = c.as_deref().filter(|c| !c.is_empty()) {
                n.color = Some(color(c)?);
            }
            n.detail = detail.clone().filter(|d| !d.is_empty());
            n.group = group.clone().filter(|g| !g.is_empty());
            n.link = link.clone().filter(|l| !l.is_empty());
            n.tags = tags.clone();
            (n.x, n.y, n.w, n.h) = (*x, *y, *w, *h);
            doc.nodes.push(n);
            created.push(id);
        }
        Op::UpdateNode {
            id,
            label,
            shape: sh,
            color: c,
            detail,
            group,
            link,
            tags,
            w,
            h,
        } => {
            if let Some(Some(g)) = opt(group) {
                need_container(doc, &g)?;
            }
            let n = doc.node_mut(id).ok_or_else(|| format!("no node `{id}`"))?;
            if let Some(l) = label {
                n.label = if l == id { String::new() } else { l.clone() };
            }
            if let Some(s) = sh {
                n.shape = shape(s)?;
            }
            if let Some(c) = opt(c) {
                n.color = c.map(|c| color(&c)).transpose()?;
            }
            if let Some(d) = opt(detail) {
                n.detail = d;
            }
            if let Some(g) = opt(group) {
                n.group = g;
            }
            if let Some(l) = opt(link) {
                n.link = l;
            }
            if let Some(t) = tags {
                n.tags = t.clone();
            }
            if let Some(w) = w {
                n.w = Some(*w).filter(|v| *v > 0.0);
            }
            if let Some(h) = h {
                n.h = Some(*h).filter(|v| *v > 0.0);
            }
        }
        Op::Rename { id, to } => {
            if id == to {
                return Ok(());
            }
            if !crate::dsl::is_ident(to) {
                return Err(format!(
                    "`{to}` is not a valid id (letters, digits, _, - and .)"
                ));
            }
            if doc.id_taken(to) {
                return Err(format!("id `{to}` is already used"));
            }
            if let Some(n) = doc.node_mut(id) {
                n.id = to.clone();
            } else if let Some(g) = doc.group_mut(id) {
                g.id = to.clone();
                for n in &mut doc.nodes {
                    if n.group.as_deref() == Some(id) {
                        n.group = Some(to.clone());
                    }
                }
                for g in &mut doc.groups {
                    if g.parent.as_deref() == Some(id) {
                        g.parent = Some(to.clone());
                    }
                }
            } else if let Some(e) = doc.edge_mut(id) {
                e.id = to.clone();
            } else if let Some(c) = doc.comment(id).map(|c| c.id.clone()) {
                if doc.comment(to).is_some() {
                    return Err(format!("comment `{to}` already exists"));
                }
                doc.comment_mut(&c).unwrap().id = to.clone();
                return Ok(());
            } else {
                return Err(format!("nothing has the id `{id}`"));
            }
            // edges keep their auto ids in step with their endpoints
            let mut renamed: Vec<(String, String)> = Vec::new();
            for i in 0..doc.edges.len() {
                let e = &mut doc.edges[i];
                let mut touched = false;
                if e.from == *id {
                    e.from = to.clone();
                    touched = true;
                }
                if e.to == *id {
                    e.to = to.clone();
                    touched = true;
                }
                if touched && e.id.contains("->") {
                    let old = e.id.clone();
                    e.id = String::new();
                    let (f, t) = (doc.edges[i].from.clone(), doc.edges[i].to.clone());
                    let fresh = doc.fresh_edge_id(&f, &t);
                    doc.edges[i].id = fresh.clone();
                    renamed.push((old, fresh));
                }
            }
            for c in &mut doc.comments {
                if c.on.as_deref() == Some(id) {
                    c.on = Some(to.clone());
                } else if let Some((_, new)) = renamed
                    .iter()
                    .find(|(old, _)| Some(old.as_str()) == c.on.as_deref())
                {
                    c.on = Some(new.clone());
                }
            }
        }
        Op::Remove { id, keep_members } => {
            if doc.node(id).is_some() {
                doc.nodes.retain(|n| n.id != *id);
                let gone: Vec<String> = doc
                    .edges
                    .iter()
                    .filter(|e| e.from == *id || e.to == *id)
                    .map(|e| e.id.clone())
                    .collect();
                doc.edges.retain(|e| e.from != *id && e.to != *id);
                detach_comments(doc, id, &gone);
            } else if let Some(g) = doc.group(id).cloned() {
                let keep = keep_members.unwrap_or(true);
                if keep {
                    for n in &mut doc.nodes {
                        if n.group.as_deref() == Some(id) {
                            n.group = g.parent.clone();
                        }
                    }
                    for c in &mut doc.groups {
                        if c.parent.as_deref() == Some(id) {
                            c.parent = g.parent.clone();
                        }
                    }
                } else {
                    let mut doomed_groups = vec![id.clone()];
                    let mut changed = true;
                    while changed {
                        changed = false;
                        for c in &doc.groups {
                            if c.parent.as_ref().is_some_and(|p| doomed_groups.contains(p))
                                && !doomed_groups.contains(&c.id)
                            {
                                doomed_groups.push(c.id.clone());
                                changed = true;
                            }
                        }
                    }
                    let doomed_nodes: Vec<String> = doc
                        .nodes
                        .iter()
                        .filter(|n| n.group.as_ref().is_some_and(|g| doomed_groups.contains(g)))
                        .map(|n| n.id.clone())
                        .collect();
                    for nid in doomed_nodes {
                        apply_one(
                            doc,
                            &Op::Remove {
                                id: nid,
                                keep_members: None,
                            },
                            created,
                        )?;
                    }
                    doc.groups
                        .retain(|c| c.id == *id || !doomed_groups.contains(&c.id));
                }
                doc.groups.retain(|c| c.id != *id);
                let gone: Vec<String> = doc
                    .edges
                    .iter()
                    .filter(|e| e.from == *id || e.to == *id)
                    .map(|e| e.id.clone())
                    .collect();
                doc.edges.retain(|e| e.from != *id && e.to != *id);
                detach_comments(doc, id, &gone);
            } else if doc.edge(id).is_some() {
                doc.edges.retain(|e| e.id != *id);
                detach_comments(doc, id, &[]);
            } else if doc.comment(id).is_some() {
                doc.comments.retain(|c| c.id != *id);
            } else {
                return Err(format!("nothing has the id `{id}`"));
            }
        }
        Op::Move { id, x, y } => {
            let n = doc.node_mut(id).ok_or_else(|| format!("no node `{id}`"))?;
            n.x = Some(x.round());
            n.y = Some(y.round());
        }
        Op::Nudge { id, dx, dy } => {
            let targets: Vec<String> = if doc.node(id).is_some() {
                vec![id.clone()]
            } else if doc.group(id).is_some() {
                doc.nodes
                    .iter()
                    .filter(|n| {
                        n.group
                            .as_ref()
                            .is_some_and(|g| doc.group_chain(g).contains(id))
                    })
                    .map(|n| n.id.clone())
                    .collect()
            } else {
                return Err(format!("no node or group `{id}`"));
            };
            for t in targets {
                let n = doc.node_mut(&t).unwrap();
                if let (Some(x), Some(y)) = (n.x, n.y) {
                    n.x = Some((x + dx).round());
                    n.y = Some((y + dy).round());
                }
            }
        }
        Op::Unpin { id } => {
            for n in &mut doc.nodes {
                if id.as_ref().is_none_or(|id| *id == n.id) {
                    n.x = None;
                    n.y = None;
                }
            }
        }
        Op::AddEdge {
            id,
            from,
            to,
            label,
            arrow,
            line,
            color: c,
        } => {
            for end in [from, to] {
                if doc.node(end).is_none() && doc.group(end).is_none() {
                    return Err(format!("no node or group `{end}` (add it first)"));
                }
            }
            let id = match id.as_deref().filter(|s| !s.is_empty()) {
                Some(id) if doc.id_taken(id) => return Err(format!("id `{id}` is already used")),
                Some(id) => id.to_string(),
                None => doc.fresh_edge_id(from, to),
            };
            let mut e = Edge {
                id: id.clone(),
                from: from.clone(),
                to: to.clone(),
                label: label.clone().filter(|l| !l.is_empty()),
                ..Default::default()
            };
            if let Some(a) = arrow {
                e.arrow = Arrow::parse(a)
                    .ok_or_else(|| format!("unknown arrow `{a}` (use {})", Arrow::names()))?;
            }
            if let Some(l) = line {
                e.line = Line::parse(l)
                    .ok_or_else(|| format!("unknown line `{l}` (use {})", Line::names()))?;
            }
            if let Some(c) = c.as_deref().filter(|c| !c.is_empty()) {
                e.color = Some(color(c)?);
            }
            doc.edges.push(e);
            created.push(id);
        }
        Op::UpdateEdge {
            id,
            from,
            to,
            label,
            arrow,
            line,
            color: c,
        } => {
            for end in [from, to].into_iter().flatten() {
                if doc.node(end).is_none() && doc.group(end).is_none() {
                    return Err(format!("no node or group `{end}`"));
                }
            }
            let e = doc.edge_mut(id).ok_or_else(|| format!("no edge `{id}`"))?;
            if let Some(f) = from {
                e.from = f.clone();
            }
            if let Some(t) = to {
                e.to = t.clone();
            }
            if let Some(l) = opt(label) {
                e.label = l;
            }
            if let Some(a) = arrow {
                e.arrow = Arrow::parse(a)
                    .ok_or_else(|| format!("unknown arrow `{a}` (use {})", Arrow::names()))?;
            }
            if let Some(l) = line {
                e.line = Line::parse(l)
                    .ok_or_else(|| format!("unknown line `{l}` (use {})", Line::names()))?;
            }
            if let Some(c) = opt(c) {
                e.color = c.map(|c| color(&c)).transpose()?;
            }
        }
        Op::AddGroup {
            id,
            label,
            color: c,
            parent,
            members,
        } => {
            let id = new_id(
                doc,
                id.as_deref(),
                if label.is_empty() { "group" } else { label },
            )?;
            if let Some(p) = parent.as_deref().filter(|p| !p.is_empty()) {
                need_container(doc, p)?;
            }
            let mut g = Group {
                id: id.clone(),
                label: if *label == id {
                    String::new()
                } else {
                    label.clone()
                },
                parent: parent.clone().filter(|p| !p.is_empty()),
                ..Default::default()
            };
            if let Some(c) = c.as_deref().filter(|c| !c.is_empty()) {
                g.color = Some(color(c)?);
            }
            doc.groups.push(g);
            for m in members {
                if let Some(n) = doc.node_mut(m) {
                    n.group = Some(id.clone());
                } else if doc.group(m).is_some() && m != &id {
                    doc.group_mut(m).unwrap().parent = Some(id.clone());
                } else {
                    return Err(format!("no node or group `{m}` to put in `{id}`"));
                }
            }
            created.push(id);
        }
        Op::UpdateGroup {
            id,
            label,
            color: c,
            parent,
        } => {
            if let Some(Some(p)) = opt(parent) {
                need_container(doc, &p)?;
                if doc.group_chain(&p).contains(id) {
                    return Err(format!("`{p}` is inside `{id}`"));
                }
            }
            let g = doc
                .group_mut(id)
                .ok_or_else(|| format!("no group `{id}`"))?;
            if let Some(l) = label {
                g.label = l.clone();
            }
            if let Some(c) = opt(c) {
                g.color = c.map(|c| color(&c)).transpose()?;
            }
            if let Some(p) = opt(parent) {
                g.parent = p;
            }
        }
        Op::AddComment {
            id,
            on,
            text,
            author,
            x,
            y,
        } => {
            if let Some(on) = on.as_deref().filter(|s| !s.is_empty()) {
                if doc.element(on).is_none() {
                    return Err(format!("nothing has the id `{on}` to comment on"));
                }
            }
            let id = match id.as_deref().filter(|s| !s.is_empty()) {
                Some(id) if doc.comment(id).is_some() => {
                    return Err(format!("comment `{id}` already exists"))
                }
                Some(id) => id.to_string(),
                None => doc.fresh_comment_id(),
            };
            doc.comments.push(Comment {
                id: id.clone(),
                on: on.clone().filter(|s| !s.is_empty()),
                text: text.clone(),
                author: author.clone().filter(|s| !s.is_empty()),
                x: *x,
                y: *y,
                ..Default::default()
            });
            created.push(id);
        }
        Op::Reply { id, text, author } => {
            let c = doc
                .comment_mut(id)
                .ok_or_else(|| format!("no comment `{id}`"))?;
            c.replies.push(Reply {
                author: author.clone().filter(|s| !s.is_empty()),
                text: text.clone(),
            });
        }
        Op::EditComment { id, text } => {
            doc.comment_mut(id)
                .ok_or_else(|| format!("no comment `{id}`"))?
                .text = text.clone();
        }
        Op::ResolveComment { id, resolution } => {
            let c = doc
                .comment_mut(id)
                .ok_or_else(|| format!("no comment `{id}`"))?;
            c.status = Status::Resolved;
            if let Some(r) = opt(resolution) {
                c.resolution = r;
            }
        }
        Op::ReopenComment { id } => {
            let c = doc
                .comment_mut(id)
                .ok_or_else(|| format!("no comment `{id}`"))?;
            c.status = Status::Open;
        }
        Op::Set { key, value } => {
            let v = value.trim();
            let s = &mut doc.settings;
            match key.as_str() {
                "title" => doc.title = Some(v.to_string()).filter(|v| !v.is_empty()),
                "description" => doc.description = Some(v.to_string()).filter(|v| !v.is_empty()),
                "direction" => {
                    s.direction = Direction::parse(v).ok_or_else(|| {
                        format!("unknown direction `{v}` (use {})", Direction::names())
                    })?
                }
                "layout" => {
                    s.layout = LayoutKind::parse(v).ok_or_else(|| {
                        format!("unknown layout `{v}` (use {})", LayoutKind::names())
                    })?
                }
                "routing" => {
                    s.routing = Routing::parse(v).ok_or_else(|| {
                        format!("unknown routing `{v}` (use {})", Routing::names())
                    })?
                }
                "theme" => {
                    s.theme = Theme::parse(v)
                        .ok_or_else(|| format!("unknown theme `{v}` (use {})", Theme::names()))?
                }
                k => {
                    return Err(format!(
                        "unknown setting `{k}` (use {})",
                        crate::dsl::SETTINGS.join(", ")
                    ))
                }
            }
        }
    }
    Ok(())
}

/// Comments on a removed element stay, as canvas comments, so feedback is
/// never lost silently.
fn detach_comments(doc: &mut Document, id: &str, edges: &[String]) {
    for c in &mut doc.comments {
        if c.on
            .as_deref()
            .is_some_and(|on| on == id || edges.iter().any(|e| e == on))
        {
            let what = c.on.take().unwrap();
            c.text = format!("[on removed `{what}`] {}", c.text);
        }
    }
}

/// Read ops from JSON: `{"ops": [...]}`, `[...]` or a single op. Accepts an
/// LLM reply too: the first ```json fenced block (or bare JSON) is used.
pub fn parse_ops(text: &str) -> Result<Vec<Op>, String> {
    let body = extract_json(text).ok_or("no JSON found (expected {\"ops\": [...]})")?;
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|e| format!("invalid JSON: {e}"))?;
    let list = match v {
        serde_json::Value::Array(a) => a,
        serde_json::Value::Object(mut o) if o.contains_key("ops") => match o.remove("ops") {
            Some(serde_json::Value::Array(a)) => a,
            _ => return Err("`ops` must be an array".into()),
        },
        o @ serde_json::Value::Object(_) => vec![o],
        _ => return Err("expected an object or an array of ops".into()),
    };
    list.into_iter()
        .enumerate()
        .map(|(i, v)| serde_json::from_value::<Op>(v).map_err(|e| format!("op #{}: {e}", i + 1)))
        .collect()
}

/// The JSON payload inside a chat reply: a fenced block or the outermost braces.
pub fn extract_json(text: &str) -> Option<&str> {
    for fence in ["```json", "```JSON", "```"] {
        if let Some(start) = text.find(fence) {
            let rest = &text[start + fence.len()..];
            if let Some(end) = rest.find("```") {
                let body = rest[..end].trim();
                if body.starts_with('{') || body.starts_with('[') {
                    return Some(body);
                }
            }
        }
    }
    let t = text.trim();
    if t.starts_with('{') || t.starts_with('[') {
        return Some(t);
    }
    let start = t.find(['{', '['])?;
    let end = t.rfind(['}', ']'])?;
    (end > start).then(|| &t[start..=end])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl;

    fn doc() -> Document {
        dsl::parse("group b: Backend {\n api\n db\n}\nweb -> api -> db\ncomment on api: split?\ncomment on api->db: index?").unwrap()
    }

    #[test]
    fn add_connect_and_resolve() {
        let mut d = doc();
        let ops = parse_ops(
            r#"Here you go:
```json
{"ops": [
  {"op": "add_node", "label": "Redis cache", "shape": "cylinder", "group": "b"},
  {"op": "add_edge", "from": "api", "to": "redis-cache", "label": "reads"},
  {"op": "resolve_comment", "id": "c1", "resolution": "kept as one service"}
]}
```"#,
        )
        .unwrap();
        let r = apply(&mut d, &ops).unwrap();
        assert_eq!(r.created, vec!["redis-cache", "api->redis-cache"]);
        assert_eq!(d.node("redis-cache").unwrap().group.as_deref(), Some("b"));
        assert_eq!(d.comment("c1").unwrap().status, Status::Resolved);
        assert!(d.check().is_empty(), "{:?}", d.check());
    }

    #[test]
    fn batches_are_atomic() {
        let mut d = doc();
        let before = d.clone();
        let ops =
            parse_ops(r#"[{"op":"add_node","id":"x"},{"op":"add_edge","from":"x","to":"nope"}]"#)
                .unwrap();
        let err = apply(&mut d, &ops).unwrap_err();
        assert_eq!(err.index, 1);
        assert_eq!(d, before);
    }

    #[test]
    fn rename_updates_references() {
        let mut d = doc();
        apply(
            &mut d,
            &[Op::Rename {
                id: "api".into(),
                to: "orders".into(),
            }],
        )
        .unwrap();
        assert!(d.edge("web->orders").is_some() && d.edge("orders->db").is_some());
        assert_eq!(d.comment("c1").unwrap().on.as_deref(), Some("orders"));
        assert_eq!(d.comment("c2").unwrap().on.as_deref(), Some("orders->db"));
        assert!(d.check().is_empty(), "{:?}", d.check());
    }

    #[test]
    fn remove_keeps_feedback() {
        let mut d = doc();
        apply(
            &mut d,
            &[Op::Remove {
                id: "api".into(),
                keep_members: None,
            }],
        )
        .unwrap();
        assert_eq!(d.edges.len(), 0);
        assert!(d
            .comments
            .iter()
            .all(|c| c.on.is_none() && c.text.starts_with("[on removed")));
        let mut d = doc();
        apply(
            &mut d,
            &[Op::Remove {
                id: "b".into(),
                keep_members: Some(false),
            }],
        )
        .unwrap();
        assert!(d.node("api").is_none() && d.group("b").is_none() && d.node("web").is_some());
    }

    #[test]
    fn rejects_unknown_fields() {
        assert!(parse_ops(r#"{"op":"add_node","colour":"red"}"#).is_err());
    }
}
