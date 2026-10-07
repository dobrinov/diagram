//! The diagram document. This is also the JSON format: every other format
//! (the DSL, Mermaid, the Markdown brief) converts to or from it.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Document {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Settings::is_default")]
    pub settings: Settings,
    pub nodes: Vec<Node>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub edges: Vec<Edge>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<Group>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<Comment>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub direction: Direction,
    pub layout: LayoutKind,
    pub routing: Routing,
    pub theme: Theme,
}

impl Settings {
    pub fn is_default(&self) -> bool {
        *self == Settings::default()
    }
}

macro_rules! keyword_enum {
    ($(#[$m:meta])* $name:ident { $($variant:ident = $kw:literal $(| $alias:literal)*),+ $(,)? } default $def:ident) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "lowercase")]
        pub enum $name { $($variant),+ }
        impl Default for $name {
            fn default() -> Self { $name::$def }
        }
        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),+];
            pub fn as_str(self) -> &'static str {
                match self { $($name::$variant => $kw),+ }
            }
            pub fn parse(s: &str) -> Option<Self> {
                let s = s.trim().to_ascii_lowercase();
                match s.as_str() {
                    $($kw $(| $alias)* => Some($name::$variant),)+
                    _ => None,
                }
            }
            pub fn names() -> String {
                [$($kw),+].join(", ")
            }
        }
    };
}

keyword_enum!(
    /// Which way the layered layout flows.
    Direction { Right = "right" | "lr", Down = "down" | "tb" | "td", Left = "left" | "rl", Up = "up" | "bt" } default Right
);
keyword_enum!(LayoutKind { Layered = "layered" | "auto" | "flow", Grid = "grid" } default Layered);
keyword_enum!(Routing { Orthogonal = "orthogonal" | "ortho" | "elbow", Curved = "curved" | "curve", Straight = "straight" | "line" } default Orthogonal);
keyword_enum!(Theme { Light = "light", Dark = "dark" } default Light);
keyword_enum!(
    Shape {
        Box = "box" | "rect" | "rectangle",
        Rounded = "rounded",
        Pill = "pill" | "stadium",
        Circle = "circle" | "ellipse" | "oval",
        Diamond = "diamond" | "decision" | "rhombus",
        Hexagon = "hexagon",
        Cylinder = "cylinder" | "database" | "db",
        Parallelogram = "parallelogram" | "io",
        Document = "document" | "doc",
        Person = "person" | "actor" | "user",
        Cloud = "cloud",
        Sticky = "sticky" | "note",
        Text = "text" | "label",
    } default Box
);
keyword_enum!(Arrow { End = "end", Start = "start", Both = "both", None = "none" } default End);
keyword_enum!(Line { Solid = "solid", Dashed = "dashed", Dotted = "dotted", Thick = "thick" | "bold" } default Solid);
keyword_enum!(Status { Open = "open", Resolved = "resolved" | "done" | "closed" } default Open);

/// Named colours understood everywhere (anything else must be a `#hex`).
pub const COLORS: &[&str] = &[
    "gray", "blue", "green", "yellow", "orange", "red", "purple", "pink", "teal",
];

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Node {
    pub id: String,
    /// Shown text; empty means "use the id". `\n` breaks lines.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub label: String,
    #[serde(skip_serializing_if = "is_default")]
    pub shape: Shape,
    /// Secondary, smaller text under the label (tech stack, owner, …).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Pinned position (top-left). Unpinned nodes are placed by the layout.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub w: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h: Option<f64>,
}

impl Node {
    pub fn new(id: &str) -> Node {
        Node {
            id: id.to_string(),
            ..Default::default()
        }
    }
    pub fn display(&self) -> &str {
        if self.label.is_empty() {
            &self.id
        } else {
            &self.label
        }
    }
    pub fn pinned(&self) -> bool {
        self.x.is_some() && self.y.is_some()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Edge {
    /// Defaults to `from->to` (`from->to#2` for the second one).
    pub id: String,
    pub from: String,
    pub to: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "is_default")]
    pub arrow: Arrow,
    #[serde(skip_serializing_if = "is_default")]
    pub line: Line,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Group {
    pub id: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Enclosing group, for nested groups.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

impl Group {
    pub fn display(&self) -> &str {
        if self.label.is_empty() {
            &self.id
        } else {
            &self.label
        }
    }
}

/// A review comment. It is attached to a node, edge or group (`on`), or
/// placed on the canvas (`x`, `y`). Open comments become LLM prompts.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Comment {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on: Option<String>,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "is_default")]
    pub status: Status,
    /// How it was resolved (written by whoever resolved it, e.g. an agent).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub replies: Vec<Reply>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Reply {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    pub text: String,
}

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

/// A problem found while reading or checking a document. `line` is 1-based;
/// 0 means "not tied to a line" (e.g. JSON input).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub line: usize,
    pub message: String,
}

impl Diagnostic {
    pub fn new(line: usize, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            line,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.line > 0 {
            write!(f, "line {}: {}", self.line, self.message)
        } else {
            write!(f, "{}", self.message)
        }
    }
}

/// What an id refers to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Element<'a> {
    Node(&'a Node),
    Edge(&'a Edge),
    Group(&'a Group),
}

impl Document {
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }
    pub fn node_mut(&mut self, id: &str) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }
    pub fn edge(&self, id: &str) -> Option<&Edge> {
        self.edges.iter().find(|e| e.id == id)
    }
    pub fn edge_mut(&mut self, id: &str) -> Option<&mut Edge> {
        self.edges.iter_mut().find(|e| e.id == id)
    }
    pub fn group(&self, id: &str) -> Option<&Group> {
        self.groups.iter().find(|g| g.id == id)
    }
    pub fn group_mut(&mut self, id: &str) -> Option<&mut Group> {
        self.groups.iter_mut().find(|g| g.id == id)
    }
    pub fn comment(&self, id: &str) -> Option<&Comment> {
        self.comments.iter().find(|c| c.id == id)
    }
    pub fn comment_mut(&mut self, id: &str) -> Option<&mut Comment> {
        self.comments.iter_mut().find(|c| c.id == id)
    }

    pub fn element(&self, id: &str) -> Option<Element<'_>> {
        self.node(id)
            .map(Element::Node)
            .or_else(|| self.edge(id).map(Element::Edge))
            .or_else(|| self.group(id).map(Element::Group))
    }

    /// Nodes and groups share one id namespace (edges may point at either).
    pub fn id_taken(&self, id: &str) -> bool {
        self.node(id).is_some() || self.group(id).is_some() || self.edge(id).is_some()
    }

    /// `base`, or `base-2`, `base-3`, … whichever is free.
    pub fn fresh_id(&self, base: &str) -> String {
        let base = if base.is_empty() { "node" } else { base };
        if !self.id_taken(base) {
            return base.to_string();
        }
        (2..)
            .map(|i| format!("{base}-{i}"))
            .find(|id| !self.id_taken(id))
            .unwrap()
    }

    pub fn fresh_comment_id(&self) -> String {
        (1..)
            .map(|i| format!("c{i}"))
            .find(|id| self.comment(id).is_none())
            .unwrap()
    }

    /// The default id for an edge between two nodes, unique in the document.
    pub fn fresh_edge_id(&self, from: &str, to: &str) -> String {
        let base = format!("{from}->{to}");
        if !self.id_taken(&base) {
            return base;
        }
        (2..)
            .map(|i| format!("{base}#{i}"))
            .find(|id| !self.id_taken(id))
            .unwrap()
    }

    /// Ancestors of a group, innermost first.
    pub fn group_chain(&self, id: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut cur = Some(id.to_string());
        while let Some(g) = cur {
            if out.contains(&g) {
                break; // cycle; `check` reports it
            }
            let parent = self.group(&g).and_then(|g| g.parent.clone());
            out.push(g);
            cur = parent;
        }
        out
    }

    /// Fill in what the formats allow to be implicit: ids of edges and
    /// comments, and nodes that are only mentioned by edges.
    pub fn normalize(&mut self) {
        let mut missing: Vec<String> = Vec::new();
        for e in &self.edges {
            for end in [&e.from, &e.to] {
                if !end.is_empty()
                    && self.node(end).is_none()
                    && self.group(end).is_none()
                    && !missing.contains(end)
                {
                    missing.push(end.clone());
                }
            }
        }
        for id in missing {
            self.nodes.push(Node::new(&id));
        }
        for i in 0..self.edges.len() {
            if self.edges[i].id.is_empty() {
                let id = self.fresh_edge_id(&self.edges[i].from.clone(), &self.edges[i].to.clone());
                self.edges[i].id = id;
            }
        }
        for i in 0..self.comments.len() {
            if self.comments[i].id.is_empty() {
                let id = self.fresh_comment_id();
                self.comments[i].id = id;
            }
        }
    }

    /// Structural problems: duplicate ids, dangling references, cycles.
    pub fn check(&self) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for id in self
            .nodes
            .iter()
            .map(|n| &n.id)
            .chain(self.groups.iter().map(|g| &g.id))
            .chain(self.edges.iter().map(|e| &e.id))
        {
            if id.is_empty() {
                out.push(Diagnostic::new(0, "an element has an empty id"));
            } else if !seen.insert(id.as_str()) {
                out.push(Diagnostic::new(0, format!("duplicate id `{id}`")));
            }
        }
        let mut cids = std::collections::HashSet::new();
        for c in &self.comments {
            if !cids.insert(c.id.as_str()) {
                out.push(Diagnostic::new(
                    0,
                    format!("duplicate comment id `{}`", c.id),
                ));
            }
            if let Some(on) = &c.on {
                if self.element(on).is_none() {
                    out.push(Diagnostic::new(
                        0,
                        format!("comment `{}` is on `{on}`, which does not exist", c.id),
                    ));
                }
            }
        }
        for e in &self.edges {
            for end in [&e.from, &e.to] {
                if self.node(end).is_none() && self.group(end).is_none() {
                    out.push(Diagnostic::new(
                        0,
                        format!("edge `{}` points at `{end}`, which does not exist", e.id),
                    ));
                }
            }
        }
        for n in &self.nodes {
            if let Some(g) = &n.group {
                if self.group(g).is_none() {
                    out.push(Diagnostic::new(
                        0,
                        format!("node `{}` is in group `{g}`, which does not exist", n.id),
                    ));
                }
            }
            if let Some(c) = &n.color {
                if !valid_color(c) {
                    out.push(Diagnostic::new(
                        0,
                        format!(
                            "node `{}`: unknown colour `{c}` (use {} or #rrggbb)",
                            n.id,
                            COLORS.join(", ")
                        ),
                    ));
                }
            }
        }
        for g in &self.groups {
            if let Some(p) = &g.parent {
                if self.group(p).is_none() {
                    out.push(Diagnostic::new(
                        0,
                        format!("group `{}` is inside `{p}`, which does not exist", g.id),
                    ));
                } else if self.group_chain(p).contains(&g.id) {
                    out.push(Diagnostic::new(
                        0,
                        format!("group `{}` contains itself", g.id),
                    ));
                }
            }
        }
        out
    }

    /// Members of a group (nodes and direct child groups), in document order.
    pub fn members(&self, group: Option<&str>) -> (Vec<&Node>, Vec<&Group>) {
        let nodes = self
            .nodes
            .iter()
            .filter(|n| n.group.as_deref() == group)
            .collect();
        let groups = self
            .groups
            .iter()
            .filter(|g| g.parent.as_deref() == group)
            .collect();
        (nodes, groups)
    }

    /// Short human description of an element, for prompts and diffs.
    pub fn describe(&self, id: &str) -> String {
        match self.element(id) {
            Some(Element::Node(n)) if n.label.is_empty() => format!("node `{id}`"),
            Some(Element::Node(n)) => format!("node `{id}` (\"{}\")", n.label.replace('\n', " ")),
            Some(Element::Group(g)) => format!("group `{id}` (\"{}\")", g.display()),
            Some(Element::Edge(e)) => {
                let label = e
                    .label
                    .as_ref()
                    .map(|l| format!(" \"{l}\""))
                    .unwrap_or_default();
                format!("edge `{}` → `{}`{label}", e.from, e.to)
            }
            None => format!("`{id}` (deleted)"),
        }
    }
}

pub fn valid_color(c: &str) -> bool {
    COLORS.contains(&c)
        || (c.starts_with('#')
            && matches!(c.len(), 4 | 7 | 9)
            && c[1..].chars().all(|ch| ch.is_ascii_hexdigit()))
}

/// Lowercase, dash-separated id from free text ("Orders API" → `orders-api`).
pub fn slug(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            out.extend(ch.to_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() {
        "node".into()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keywords_and_aliases() {
        assert_eq!(Direction::parse("LR"), Some(Direction::Right));
        assert_eq!(Shape::parse("database"), Some(Shape::Cylinder));
        assert_eq!(Shape::Cylinder.as_str(), "cylinder");
        assert_eq!(Shape::parse("nope"), None);
    }

    #[test]
    fn slugs_and_fresh_ids() {
        assert_eq!(slug("Orders API"), "orders-api");
        assert_eq!(slug("  !!"), "node");
        let mut d = Document::default();
        d.nodes.push(Node::new("a"));
        assert_eq!(d.fresh_id("a"), "a-2");
        assert_eq!(d.fresh_edge_id("a", "b"), "a->b");
    }

    #[test]
    fn normalize_creates_implicit_nodes() {
        let mut d = Document {
            edges: vec![Edge {
                from: "a".into(),
                to: "b".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        d.normalize();
        assert_eq!(d.nodes.len(), 2);
        assert_eq!(d.edges[0].id, "a->b");
        assert!(d.check().is_empty());
    }
}
