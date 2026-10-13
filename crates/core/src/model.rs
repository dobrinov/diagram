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
    /// Themes written on the board (`theme brand [base: light] { … }`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub themes: Vec<ThemeDef>,
}

/// A theme, in a light and a dark mode: what it changes in both (`values`)
/// and in each mode — colours by role (`background`, `text`, `line`, …), the
/// named colours (`blue: #3b6fd8`), `font` and `radius`. See
/// [`crate::theme::KEYS`]. A board drawn light uses the light mode.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeDef {
    pub id: String,
    /// (key, value) for both modes, in the order written.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<(String, String)>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub light: Vec<(String, String)>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub dark: Vec<(String, String)>,
}

impl ThemeDef {
    /// A key in mode `t` (its own value, or the one for both modes).
    pub fn get(&self, t: Theme, key: &str) -> Option<&str> {
        fn find<'a>(l: &'a [(String, String)], key: &str) -> Option<&'a str> {
            l.iter()
                .rev()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.as_str())
        }
        find(self.mode(t), key).or_else(|| find(&self.values, key))
    }

    /// What mode `t` sets on its own.
    pub fn mode(&self, t: Theme) -> &[(String, String)] {
        match t {
            Theme::Light => &self.light,
            Theme::Dark => &self.dark,
        }
    }

    /// Everything mode `t` has: both modes' values, then its own.
    pub fn values_in(&self, t: Theme) -> Vec<(String, String)> {
        self.values.iter().chain(self.mode(t)).cloned().collect()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub direction: Direction,
    pub layout: LayoutKind,
    pub routing: Routing,
    pub theme: Theme,
    /// A theme by name: one written on the board, a preset (`paper`,
    /// `blueprint`, `midnight`, `contrast`) or a file (`./brand.theme`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub palette: Option<String>,
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
keyword_enum!(
    /// Which way a mindmap grows from its central topic.
    MapLayout { Right = "right", Left = "left", Both = "both" | "balanced", Down = "down" | "org" } default Right
);
keyword_enum!(Arrow { End = "end", Start = "start", Both = "both", None = "none" } default End);
keyword_enum!(Line { Solid = "solid", Dashed = "dashed", Dotted = "dotted", Thick = "thick" | "bold" } default Solid);
keyword_enum!(
    /// What a chart block draws.
    ChartKind { Bar = "bar" | "column" | "columns" | "bars", Line = "line", Area = "area", Pie = "pie", Donut = "donut" | "doughnut", Scatter = "scatter" | "xy" } default Bar
);
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
    /// Mindmap branch: the topic this node hangs under. Siblings keep
    /// document order.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Set on a mindmap's central topic: which way the map grows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mindmap: Option<MapLayout>,
    /// The branches under this topic are folded away.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub collapsed: bool,
    /// Gantt task: start date (`YYYY-MM-DD`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    /// Gantt task: duration in days.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub days: Option<f64>,
    /// Gantt task: the tasks it starts after.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub after: Vec<String>,
    /// Gantt task: a milestone, a point in time instead of a bar.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub milestone: bool,
    /// Gantt task: how much is done, in percent (0–100).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub done: Option<f64>,
    /// Gantt task: who does it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// Risk: how likely it is (1 to the matrix's levels).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probability: Option<u8>,
    /// Risk: how bad it would be (1 to the matrix's levels).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub impact: Option<u8>,
    /// A chart's row: one value per series (None where there is none).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<Option<f64>>,
    /// A table's row: its cells after the first (which is its label).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cells: Vec<String>,
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
    /// Swimlanes: the groups inside are lanes, laid out as bands the flow
    /// runs across.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub lanes: bool,
    /// Gantt chart: the nodes inside are tasks on a timeline.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub gantt: bool,
    /// Gantt chart: its first day (`YYYY-MM-DD`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    /// Gantt chart: what one step of its timeline is (week when unset).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<Scale>,
    /// Risk matrix: the nodes inside are risks, placed by probability and
    /// impact.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub risks: bool,
    /// Risk matrix: how many levels probability and impact have (3 to 5; 5
    /// when unset).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub levels: Option<u8>,
    /// A chart: its nodes are rows of data, drawn as bars, lines, slices, …
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chart: Option<ChartKind>,
    /// Chart: series on top of each other instead of side by side.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub stacked: bool,
    /// Bar chart: bars run left to right.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub horizontal: bool,
    /// Chart: what values are in (`%`, `k€`, `ms`; `$ € £ ¥` go in front).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// Chart: the series' names (the header row), and what the rows are.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub series: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub axis: Option<String>,
    /// A table: its nodes are rows; these are its columns' names.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub table: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<String>,
    /// Chart: its size (content, without the frame).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub w: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h: Option<f64>,
    /// Placed by hand (top-left): a section, swimlanes, a chart, … keeps its
    /// place and its insides are laid out in it. Without it a group goes
    /// where the layout (or its pinned members) puts it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
}

/// The resolution of a gantt chart's timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scale {
    Day,
    #[default]
    Week,
    Month,
    Quarter,
}

impl Scale {
    pub fn as_str(self) -> &'static str {
        match self {
            Scale::Day => "day",
            Scale::Week => "week",
            Scale::Month => "month",
            Scale::Quarter => "quarter",
        }
    }

    pub fn parse(s: &str) -> Option<Scale> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "day" | "days" | "daily" => Scale::Day,
            "week" | "weeks" | "weekly" => Scale::Week,
            "month" | "months" | "monthly" => Scale::Month,
            "quarter" | "quarters" | "quarterly" => Scale::Quarter,
            _ => return None,
        })
    }
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

    /// Mindmap branches under `id`, in order.
    pub fn children(&self, id: &str) -> Vec<&Node> {
        self.nodes
            .iter()
            .filter(|n| n.parent.as_deref() == Some(id))
            .collect()
    }

    /// The central topic of the mindmap `id` belongs to (itself for a root),
    /// or None when `id` is not part of a mindmap.
    pub fn map_root(&self, id: &str) -> Option<&Node> {
        let n = self.node(id)?;
        let top = self.ancestors(id).last().copied().unwrap_or(n);
        let is_map = top.mindmap.is_some() || top.id != n.id || !self.children(&top.id).is_empty();
        is_map.then_some(top)
    }

    /// Ancestors of a mindmap topic, nearest first.
    pub fn ancestors(&self, id: &str) -> Vec<&Node> {
        let mut out: Vec<&Node> = Vec::new();
        let mut cur = self.node(id).and_then(|n| n.parent.as_deref());
        while let Some(p) = cur {
            match self.node(p) {
                Some(n) if !out.iter().any(|o| o.id == n.id) && n.id != id => {
                    out.push(n);
                    cur = n.parent.as_deref();
                }
                _ => break,
            }
        }
        out
    }

    /// `id` and everything under it, depth first.
    pub fn subtree(&self, id: &str) -> Vec<String> {
        fn walk(doc: &Document, id: &str, out: &mut Vec<String>) {
            if out.iter().any(|o| o == id) {
                return;
            }
            out.push(id.to_string());
            for c in doc.children(id) {
                walk(doc, &c.id, out);
            }
        }
        let mut out = Vec::new();
        walk(self, id, &mut out);
        out
    }

    /// Move `id` to sibling position `index` among its parent's branches
    /// (last when None). Order only matters between siblings.
    pub fn move_among_siblings(&mut self, id: &str, index: Option<usize>) {
        let Some(pos) = self.nodes.iter().position(|n| n.id == id) else {
            return;
        };
        let node = self.nodes.remove(pos);
        let sibs: Vec<usize> = match (&node.parent, &node.group) {
            (Some(p), _) => self
                .nodes
                .iter()
                .enumerate()
                .filter(|(_, n)| n.parent.as_ref() == Some(p))
                .map(|(i, _)| i)
                .collect(),
            // a row of a table or chart, a gantt task, a box in a section
            (None, Some(g)) => self
                .nodes
                .iter()
                .enumerate()
                .filter(|(_, n)| n.parent.is_none() && n.group.as_ref() == Some(g))
                .map(|(i, _)| i)
                .collect(),
            (None, None) => Vec::new(),
        };
        let at = match (index, sibs.last()) {
            (Some(i), _) if i < sibs.len() => sibs[i],
            (_, Some(&last)) => last + 1,
            (_, None) => pos.min(self.nodes.len()),
        };
        self.nodes.insert(at, node);
    }

    /// Hang `id` under `parent` at sibling position `index` (None: last), or
    /// detach it into a mindmap of its own (`parent` None).
    pub fn set_parent(
        &mut self,
        id: &str,
        parent: Option<&str>,
        index: Option<usize>,
    ) -> Result<(), String> {
        if self.node(id).is_none() {
            return Err(format!("no node `{id}`"));
        }
        if let Some(p) = parent {
            if self.node(p).is_none() {
                return Err(format!("no node `{p}` to branch from"));
            }
            if self.subtree(id).iter().any(|s| s == p) {
                return Err(format!("`{p}` is inside `{id}`'s branch"));
            }
        }
        let n = self.node_mut(id).unwrap();
        match parent {
            Some(p) => {
                n.parent = Some(p.to_string());
                n.mindmap = None;
                n.x = None;
                n.y = None;
                n.group = None;
            }
            None => {
                n.parent = None;
                if n.mindmap.is_none() {
                    n.mindmap = Some(MapLayout::default());
                }
            }
        }
        self.move_among_siblings(id, index);
        Ok(())
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
        // a topic with branches and no parent is a mindmap's centre; branches
        // live in their map, not in a group of their own
        for i in 0..self.nodes.len() {
            if self.nodes[i].parent.is_some() {
                self.nodes[i].group = None;
                self.nodes[i].mindmap = None;
            }
            if self.nodes[i].parent.is_none()
                && self.nodes[i].mindmap.is_none()
                && self
                    .nodes
                    .iter()
                    .any(|n| n.parent.as_deref() == Some(&self.nodes[i].id))
            {
                self.nodes[i].mindmap = Some(MapLayout::default());
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
            if let Some(p) = &n.parent {
                if self.node(p).is_none() {
                    out.push(Diagnostic::new(
                        0,
                        format!("node `{}` branches from `{p}`, which is not a node", n.id),
                    ));
                } else if self.ancestors(&n.id).len() < self.ancestors(p).len() + 1 {
                    out.push(Diagnostic::new(
                        0,
                        format!("node `{}` is its own ancestor", n.id),
                    ));
                }
            }
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
    /// A lane: a group directly inside a swimlanes group.
    pub fn is_lane(&self, g: &Group) -> bool {
        g.parent
            .as_deref()
            .and_then(|p| self.group(p))
            .is_some_and(|p| p.lanes)
    }

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
