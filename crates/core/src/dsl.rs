//! The human-friendly text format (`.diagram` files).
//!
//! ```text
//! title: Checkout
//! direction: right
//!
//! user: Customer [person]
//! group backend: Backend {
//!   api: Orders API [detail: Rust · Axum]
//!   db: Postgres [cylinder]
//! }
//! user -> api: POST /orders
//! api -> db
//! api ..> mail: receipt
//!
//! comment on api by deyan: Should payments be a separate service?
//! reply c1 by ana: Not before launch.
//! ```
//!
//! One statement per line. Full-line `#` / `//` comments are ignored here
//! (edits keep them: see [`crate::patch`]). `docs/LANGUAGE.md` is the reference.
use crate::model::*;

pub const SETTINGS: &[&str] = &[
    "title",
    "description",
    "direction",
    "layout",
    "routing",
    "theme",
];

/// Edge operators, longest first: (token, arrow, line, reversed).
const OPS: &[(&str, Arrow, Line, bool)] = &[
    ("<..>", Arrow::Both, Line::Dashed, false),
    ("<=>", Arrow::Both, Line::Thick, false),
    ("<->", Arrow::Both, Line::Solid, false),
    ("-->", Arrow::End, Line::Solid, false),
    ("..>", Arrow::End, Line::Dashed, false),
    ("<..", Arrow::End, Line::Dashed, true),
    ("->", Arrow::End, Line::Solid, false),
    ("<-", Arrow::End, Line::Solid, true),
    ("=>", Arrow::End, Line::Thick, false),
    ("<=", Arrow::End, Line::Thick, true),
    ("--", Arrow::None, Line::Solid, false),
    ("..", Arrow::None, Line::Dashed, false),
    ("==", Arrow::None, Line::Thick, false),
];

/// Parse, failing on any error.
pub fn parse(src: &str) -> Result<Document, Vec<Diagnostic>> {
    let (doc, errors) = parse_lenient(src);
    if errors.is_empty() {
        Ok(doc)
    } else {
        Err(errors)
    }
}

/// Parse as much as possible; bad lines are skipped and reported.
pub fn parse_lenient(src: &str) -> (Document, Vec<Diagnostic>) {
    let (doc, errors, _) = parse_lines(src);
    (doc, errors)
}

/// Something a line of the source declares.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Elem {
    Node(String),
    Edge(String),
    Group(String),
    Comment(String),
    Setting(String),
}

/// What one line of the source is (see [`parse_lines`]).
#[derive(Debug, Clone, Default)]
pub(crate) struct LineInfo {
    /// What the line declares or adds to.
    pub elems: Vec<Elem>,
    /// Inside a mindmap or gantt block: the line (0-based) that opened it.
    pub block: Option<usize>,
    /// The `}` closing this group.
    pub close: Option<String>,
    /// How many groups are open before the line.
    pub depth: usize,
}

/// [`parse_lenient`], and what each line of the source declares.
pub(crate) fn parse_lines(src: &str) -> (Document, Vec<Diagnostic>, Vec<LineInfo>) {
    let mut info = Vec::new();
    let mut p = Parser {
        doc: Document::default(),
        errors: Vec::new(),
        stack: Vec::new(),
        line: 0,
        map: None,
        gantt: None,
        implicit: Default::default(),
        touched: Vec::new(),
    };
    for (i, raw) in src.lines().enumerate() {
        p.line = i + 1;
        let t = raw.trim();
        let mut li = LineInfo {
            depth: p.stack.len(),
            ..Default::default()
        };
        // a line inside a mindmap or a gantt chart belongs to its block
        li.block = p
            .map
            .as_ref()
            .map(|m| m.line - 1)
            .or(p.gantt.as_ref().map(|g| g.1 - 1));
        if t.is_empty() || t.starts_with('#') || t.starts_with("//") {
            info.push(li);
            continue;
        }
        let res = if p.map.is_some() {
            p.outline_line(raw)
        } else if p.gantt.is_some() {
            p.task_line(t)
        } else {
            if t == "}" {
                li.close = p.stack.last().map(|(g, _)| g.clone());
            }
            p.statement(t)
        };
        if let Err(e) = res {
            p.errors.push(Diagnostic::new(p.line, e));
        }
        // a line that opened a block is its first line
        if li.block.is_none() && (p.map.is_some() || p.gantt.is_some()) {
            li.block = Some(i);
        }
        li.elems = std::mem::take(&mut p.touched);
        info.push(li);
    }
    if let Some(m) = &p.map {
        p.errors.push(Diagnostic::new(
            m.line,
            format!("mindmap `{}` is never closed (missing `}}`)", m.root),
        ));
    }
    if let Some((g, line)) = &p.gantt {
        p.errors.push(Diagnostic::new(
            *line,
            format!("gantt `{g}` is never closed (missing `}}`)"),
        ));
    }
    if let Some((g, line)) = p.stack.last() {
        p.errors.push(Diagnostic::new(
            *line,
            format!("group `{g}` is never closed (missing `}}`)"),
        ));
    }
    resolve_after(&mut p.doc);
    p.doc.normalize();
    for c in &p.doc.comments {
        if let Some(on) = &c.on {
            if p.doc.element(on).is_none() {
                p.errors.push(Diagnostic::new(
                    0,
                    format!("comment `{}` is on `{on}`, which does not exist", c.id),
                ));
            }
        }
    }
    (p.doc, p.errors, info)
}

struct Parser {
    doc: Document,
    errors: Vec<Diagnostic>,
    /// Open groups and the line that opened them.
    stack: Vec<(String, usize)>,
    line: usize,
    /// The mindmap whose outline is being read.
    map: Option<MapBlock>,
    /// The gantt chart whose tasks are being read, and the line that opened it.
    gantt: Option<(String, usize)>,
    /// Nodes created only because an edge mentioned them; a mindmap topic
    /// with that id adopts them.
    implicit: std::collections::HashSet<String>,
    /// What the current line declared.
    touched: Vec<Elem>,
}

struct MapBlock {
    root: String,
    line: usize,
    indent: crate::mindmap::Indent,
    /// The last topic at each depth.
    chain: Vec<String>,
}

type Attrs = Vec<(String, Option<String>)>;

impl Parser {
    fn group(&self) -> Option<String> {
        self.stack.last().map(|(g, _)| g.clone())
    }

    fn statement(&mut self, t: &str) -> Result<(), String> {
        if t == "}" {
            return self
                .stack
                .pop()
                .map(|_| ())
                .ok_or_else(|| "unexpected `}`".to_string());
        }
        let mut c = Cursor::new(t);
        let word = c.peek_word();
        // a keyword only opens a block on a line that opens one, and starts
        // a comment only when no arrow follows it: `gantt -> b` is an arrow
        let opens = t.trim_end().ends_with('{') || t.trim_end().ends_with("{}");
        let arrow_next = {
            let mut a = Cursor::new(c.rest()[word.len()..].trim_start());
            a.edge_op().is_some() || a.rest().starts_with(',')
        };
        let word = match word {
            "group" | "gantt" | "risks" | "chart" | "mindmap" if !opens => "",
            "comment" | "reply" if arrow_next => "",
            w => w,
        };
        match word {
            "group" if c.rest()[5..].starts_with(|ch: char| ch.is_whitespace()) => {
                c.advance(5);
                return self.group_decl(c, false);
            }
            // swimlanes; only as block openers, so `lane -> x` stays an edge
            "lanes" | "lane"
                if c.rest()[word.len()..].starts_with(|ch: char| ch.is_whitespace())
                    && (t.trim_end().ends_with('{') || t.trim_end().ends_with("{}")) =>
            {
                let (lanes, n) = (word == "lanes", word.len());
                c.advance(n);
                return self.group_decl(c, lanes);
            }
            "comment" if c.rest()[7..].starts_with(|ch: char| ch.is_whitespace() || ch == ':') => {
                c.advance(7);
                return self.comment(c);
            }
            "mindmap" if c.rest()[7..].starts_with(|ch: char| ch.is_whitespace()) => {
                c.advance(7);
                return self.mindmap_decl(c);
            }
            "gantt" if c.rest()[5..].starts_with(|ch: char| ch.is_whitespace()) => {
                c.advance(5);
                return self.gantt_decl(c, false);
            }
            "risks" if c.rest()[5..].starts_with(|ch: char| ch.is_whitespace()) => {
                c.advance(5);
                return self.gantt_decl(c, true);
            }
            "chart" if c.rest()[5..].starts_with(|ch: char| ch.is_whitespace()) => {
                c.advance(5);
                return self.chart_decl(c);
            }
            "reply" if c.rest()[5..].starts_with(|ch: char| ch.is_whitespace()) => {
                c.advance(5);
                return self.reply(c);
            }
            w if SETTINGS.contains(&w) && c.rest()[w.len()..].trim_start().starts_with(':') => {
                if !self.stack.is_empty() {
                    return Err(format!("`{w}` can only be set at the top level"));
                }
                let value = c.rest()[w.len()..].trim_start()[1..].trim();
                return self.setting(w, value);
            }
            _ => {}
        }
        let first = self.ref_list(&mut c)?;
        c.skip_ws();
        if let Some(op) = c.edge_op() {
            return self.edges(first, op, c);
        }
        self.node_decl(first, c)
    }

    fn setting(&mut self, key: &str, value: &str) -> Result<(), String> {
        self.touched.push(Elem::Setting(key.to_string()));
        let value = unquote(value);
        let s = &mut self.doc.settings;
        match key {
            "title" => self.doc.title = Some(value).filter(|v| !v.is_empty()),
            "description" => self.doc.description = Some(value).filter(|v| !v.is_empty()),
            "direction" => {
                s.direction = Direction::parse(&value).ok_or_else(|| {
                    format!("unknown direction `{value}` (use {})", Direction::names())
                })?
            }
            "layout" => {
                s.layout = LayoutKind::parse(&value).ok_or_else(|| {
                    format!("unknown layout `{value}` (use {})", LayoutKind::names())
                })?
            }
            "routing" => {
                s.routing = Routing::parse(&value).ok_or_else(|| {
                    format!("unknown routing `{value}` (use {})", Routing::names())
                })?
            }
            "theme" => {
                s.theme = Theme::parse(&value)
                    .ok_or_else(|| format!("unknown theme `{value}` (use {})", Theme::names()))?
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    /// `a`, `"Some label"`, `a, b, "C"`
    fn ref_list(&mut self, c: &mut Cursor) -> Result<Vec<(String, Option<String>)>, String> {
        let mut out = vec![c.reference()?];
        loop {
            c.skip_ws();
            if c.eat(',') {
                c.skip_ws();
                out.push(c.reference()?);
            } else {
                return Ok(out);
            }
        }
    }

    /// Make sure a node exists (edges create them on first mention).
    fn ensure_node(&mut self, id: &str, label: Option<&str>) {
        if self.doc.node(id).is_some() || self.doc.group(id).is_some() {
            return;
        }
        let mut n = Node::new(id);
        if let Some(l) = label {
            n.label = l.to_string();
        }
        n.group = self.group();
        self.doc.nodes.push(n);
        self.implicit.insert(id.to_string());
    }

    /// `mindmap id: Centre topic [attrs] {` … indented topics … `}`
    fn mindmap_decl(&mut self, mut c: Cursor) -> Result<(), String> {
        c.skip_ws();
        let (id, qlabel) = c.reference()?;
        let (label, attrs) = label_and_attrs_until_brace(&mut c)?;
        c.skip_ws();
        if !c.eat('{') {
            return Err("a mindmap needs a body: `mindmap id: Topic {` … `}`".into());
        }
        c.skip_ws();
        let closes_inline = c.eat('}');
        if !c.at_end() {
            return Err("put the mindmap's topics on the following lines".into());
        }
        if self.doc.group(&id).is_some() {
            return Err(format!("`{id}` is a group"));
        }
        if self.doc.node(&id).is_none() {
            self.doc.nodes.push(Node::new(&id));
        }
        self.implicit.remove(&id);
        let group = self.group();
        let n = self.doc.node_mut(&id).unwrap();
        if let Some(l) = label.or(qlabel) {
            n.label = l;
        }
        n.group = group;
        n.mindmap = Some(MapLayout::default());
        apply_node_attrs(n, &attrs)?;
        self.touched.push(Elem::Node(id.clone()));
        if !closes_inline {
            self.map = Some(MapBlock {
                root: id,
                line: self.line,
                indent: Default::default(),
                chain: Vec::new(),
            });
        }
        Ok(())
    }

    /// `gantt id: Title [start: YYYY-MM-DD] {` … one task per line … `}`
    /// A gantt chart, or a risk matrix (`risks id: Title [scale: 5] {` … one
    /// risk per line … `}`): both are a group with one item per line.
    fn gantt_decl(&mut self, mut c: Cursor, risks: bool) -> Result<(), String> {
        c.skip_ws();
        let (id, qlabel) = c.reference()?;
        let (label, attrs) = label_and_attrs_until_brace(&mut c)?;
        c.skip_ws();
        let what = if risks { "risk matrix" } else { "gantt chart" };
        if !c.eat('{') {
            return Err(if risks {
                "a risk matrix needs a body: `risks id: Title {` … `}`".into()
            } else {
                "a gantt chart needs a body: `gantt id: Title {` … `}`".into()
            });
        }
        c.skip_ws();
        let closes_inline = c.eat('}');
        if !c.at_end() {
            return Err(format!("put the {what}'s items on the following lines"));
        }
        if self.doc.node(&id).is_some() {
            return Err(format!("`{id}` is already a node"));
        }
        let parent = self.group();
        if self.doc.group(&id).is_none() {
            self.doc.groups.push(Group {
                id: id.clone(),
                ..Default::default()
            });
        }
        let g = self.doc.group_mut(&id).unwrap();
        g.parent = parent;
        if risks {
            g.risks = true;
        } else {
            g.gantt = true;
        }
        if let Some(l) = label.or(qlabel) {
            g.label = l;
        }
        for (k, v) in &attrs {
            match (k.as_str(), v) {
                ("scale" | "levels", Some(v)) if risks => {
                    g.levels = Some(
                        v.parse::<u8>()
                            .ok()
                            .filter(|n| (3..=5).contains(n))
                            .ok_or_else(|| {
                                format!("a risk matrix has 3, 4 or 5 levels, not `{v}`")
                            })?,
                    )
                }
                ("color" | "colour", Some(v)) if risks => g.color = Some(color(v)?),
                ("x", Some(v)) if risks => g.x = Some(num("x", v)?),
                ("y", Some(v)) if risks => g.y = Some(num("y", v)?),
                (c, None) if risks && valid_color(c) => g.color = Some(c.to_string()),
                (k, _) if risks => {
                    return Err(format!(
                        "unknown risk matrix attribute `{k}` (scale, color, x, y)"
                    ))
                }
                ("start", Some(v)) => g.start = Some(date(v)?),
                ("scale", Some(v)) => {
                    g.scale = Some(Scale::parse(v).ok_or_else(|| {
                        format!("unknown scale `{v}` (day, week, month, quarter)")
                    })?)
                }
                ("color" | "colour", Some(v)) => g.color = Some(color(v)?),
                ("x", Some(v)) => g.x = Some(num("x", v)?),
                ("y", Some(v)) => g.y = Some(num("y", v)?),
                (d, None) if crate::gantt::parse_date(d).is_some() => g.start = Some(d.to_string()),
                (c, None) if valid_color(c) => g.color = Some(c.to_string()),
                (k, _) => {
                    return Err(format!(
                        "unknown gantt attribute `{k}` (start, scale, color, x, y)"
                    ))
                }
            }
        }
        self.touched.push(Elem::Group(id.clone()));
        if !closes_inline {
            self.gantt = Some((id, self.line));
        }
        Ok(())
    }

    /// `chart id: Title [bar, stacked, unit: %] {` … a header row and rows of
    /// data … `}`.
    fn chart_decl(&mut self, mut c: Cursor) -> Result<(), String> {
        c.skip_ws();
        let (id, qlabel) = c.reference()?;
        let (label, attrs) = label_and_attrs_until_brace(&mut c)?;
        c.skip_ws();
        if !c.eat('{') {
            return Err("a chart needs a body: `chart id: Title [bar] {` … rows … `}`".into());
        }
        c.skip_ws();
        let closes_inline = c.eat('}');
        if !c.at_end() {
            return Err("put the chart's rows on the following lines".into());
        }
        if self.doc.node(&id).is_some() {
            return Err(format!("`{id}` is already a node"));
        }
        let parent = self.group();
        if self.doc.group(&id).is_none() {
            self.doc.groups.push(Group {
                id: id.clone(),
                ..Default::default()
            });
        }
        let g = self.doc.group_mut(&id).unwrap();
        g.parent = parent;
        g.chart = Some(g.chart.unwrap_or_default());
        if let Some(l) = label.or(qlabel) {
            g.label = l;
        }
        for (k, v) in &attrs {
            match (k.as_str(), v) {
                (k, None) if ChartKind::parse(k).is_some() => g.chart = ChartKind::parse(k),
                ("type" | "kind", Some(v)) => {
                    g.chart = Some(ChartKind::parse(v).ok_or_else(|| {
                        format!("unknown chart type `{v}` (use {})", ChartKind::names())
                    })?)
                }
                ("stacked", None) => g.stacked = true,
                ("horizontal", None) => g.horizontal = true,
                ("unit", Some(v)) => g.unit = Some(v.clone()).filter(|u| !u.is_empty()),
                ("color" | "colour", Some(v)) => g.color = Some(color(v)?),
                (c, None) if valid_color(c) => g.color = Some(c.to_string()),
                ("x", Some(v)) => g.x = Some(num("x", v)?),
                ("y", Some(v)) => g.y = Some(num("y", v)?),
                ("w" | "width", Some(v)) => g.w = Some(num("w", v)?),
                ("h" | "height", Some(v)) => g.h = Some(num("h", v)?),
                (k, _) => {
                    return Err(format!(
                        "unknown chart attribute `{k}` ({}, stacked, horizontal, unit, color, x, y, w, h)",
                        ChartKind::names()
                    ))
                }
            }
        }
        self.touched.push(Elem::Group(id.clone()));
        if !closes_inline {
            self.gantt = Some((id, self.line));
        }
        Ok(())
    }

    /// A row of a chart: `Q1 | 120 | 80` or `Linear 40` (with optional
    /// `[attrs]`); the first row is the header when its cells are not numbers.
    fn chart_line(&mut self, chart: &str, t: &str) -> Result<(), String> {
        let t = crate::mindmap::strip_bullet(t);
        let (text, attrs) = split_attrs(t);
        let mut cells: Vec<String> = text.split('|').map(|c| c.trim().to_string()).collect();
        if cells.len() == 1 {
            // `Linear 40`: a label and a number
            let one = cells.remove(0);
            match one.rsplit_once(char::is_whitespace) {
                Some((l, v)) if value(v).is_ok_and(|v| v.is_some()) => {
                    cells = vec![l.trim().to_string(), v.to_string()]
                }
                _ => cells = vec![one],
            }
        }
        let first = !self
            .doc
            .nodes
            .iter()
            .any(|n| n.group.as_deref() == Some(chart));
        let g = self.doc.group(chart).unwrap();
        if first
            && g.series.is_empty()
            && g.axis.is_none()
            && cells.len() > 1
            && cells[1..].iter().all(|c| value(c).is_err())
        {
            let g = self.doc.group_mut(chart).unwrap();
            g.axis = Some(cells[0].clone()).filter(|a| !a.is_empty());
            g.series = cells[1..].to_vec();
            self.touched.push(Elem::Group(chart.to_string()));
            return Ok(());
        }
        let label = unescape_label(&cells[0]);
        let values = cells[1..]
            .iter()
            .map(|c| value(c))
            .collect::<Result<Vec<_>, _>>()?;
        let mut attrs = parse_attrs(attrs.unwrap_or(""))?;
        let mut explicit = None;
        attrs.retain(|(k, v)| match (k.as_str(), v) {
            ("id", Some(v)) => {
                explicit = Some(v.clone());
                false
            }
            _ => true,
        });
        // rows are named within their chart: `revenue.q1`
        let wanted = explicit.clone().unwrap_or_else(|| row_id(chart, &label));
        let id = if !self.doc.id_taken(&wanted) {
            wanted
        } else if explicit.is_some() {
            return Err(format!("id `{wanted}` is already used"));
        } else {
            self.doc.fresh_id(&wanted)
        };
        let mut n = Node::new(&id);
        n.label = label;
        n.group = Some(chart.to_string());
        n.values = values;
        apply_node_attrs(&mut n, &attrs)?;
        self.touched.push(Elem::Node(n.id.clone()));
        self.doc.nodes.push(n);
        Ok(())
    }

    /// One task of a gantt chart (or the closing `}`): `Label [5d, after: x]`.
    fn task_line(&mut self, t: &str) -> Result<(), String> {
        if t == "}" {
            self.gantt = None;
            return Ok(());
        }
        let block = self.gantt.as_ref().unwrap().0.clone();
        if self.doc.group(&block).is_some_and(|g| g.chart.is_some()) {
            return self.chart_line(&block, t);
        }
        let t = crate::mindmap::strip_bullet(t);
        let (text, attrs) = split_attrs(t);
        let label = unescape_label(text.trim());
        let mut attrs = parse_attrs(attrs.unwrap_or(""))?;
        let mut explicit = None;
        attrs.retain(|(k, v)| match (k.as_str(), v) {
            ("id", Some(v)) => {
                explicit = Some(v.clone());
                false
            }
            _ => true,
        });
        let chart = self.gantt.as_ref().unwrap().0.clone();
        let wanted = explicit.clone().unwrap_or_else(|| slug(&label));
        let id = if !self.doc.id_taken(&wanted) {
            wanted
        } else if self.implicit.remove(&wanted) {
            // a task that an edge mentioned earlier
            self.doc.nodes.retain(|n| n.id != wanted);
            wanted
        } else if explicit.is_some() {
            return Err(format!("id `{wanted}` is already used"));
        } else {
            self.doc.fresh_id(&wanted)
        };
        let mut n = Node::new(&id);
        n.label = if label == id { String::new() } else { label };
        n.group = Some(chart);
        apply_node_attrs(&mut n, &attrs)?;
        self.touched.push(Elem::Node(n.id.clone()));
        self.doc.nodes.push(n);
        Ok(())
    }

    /// One topic of a mindmap outline (or the closing `}`).
    fn outline_line(&mut self, raw: &str) -> Result<(), String> {
        if raw.trim() == "}" {
            self.map = None;
            return Ok(());
        }
        let (w, t) = crate::mindmap::indent_of(raw);
        let t = crate::mindmap::strip_bullet(t.trim_end());
        let (text, attrs) = split_attrs(t);
        let label = unescape_label(text.trim());
        let mut attrs = parse_attrs(attrs.unwrap_or(""))?;
        let mut explicit = None;
        attrs.retain(|(k, v)| match (k.as_str(), v) {
            ("id", Some(v)) => {
                explicit = Some(v.clone());
                false
            }
            _ => true,
        });
        let m = self.map.as_mut().unwrap();
        let depth = m.indent.depth(w);
        m.chain.truncate(depth);
        let parent = m.chain.last().cloned().unwrap_or_else(|| m.root.clone());
        let wanted = explicit.clone().unwrap_or_else(|| slug(&label));
        let id = if !self.doc.id_taken(&wanted) {
            wanted
        } else if self.implicit.contains(&wanted) {
            self.implicit.remove(&wanted);
            wanted
        } else if explicit.is_some() {
            return Err(format!("id `{wanted}` is already used"));
        } else {
            self.doc.fresh_id(&wanted)
        };
        if self.doc.node(&id).is_none() {
            self.doc.nodes.push(Node::new(&id));
        } else {
            // an adopted node moves to where the topic is written
            self.doc.nodes.retain(|n| n.id != id);
            self.doc.nodes.push(Node::new(&id));
        }
        let n = self.doc.node_mut(&id).unwrap();
        n.label = if label == id { String::new() } else { label };
        n.parent = Some(parent);
        apply_node_attrs(n, &attrs)?;
        self.touched.push(Elem::Node(id.clone()));
        self.map.as_mut().unwrap().chain.push(id);
        Ok(())
    }

    fn node_decl(
        &mut self,
        refs: Vec<(String, Option<String>)>,
        mut c: Cursor,
    ) -> Result<(), String> {
        let (label, attrs) = label_and_attrs(&mut c)?;
        if !c.at_end() {
            return Err(format!("unexpected `{}`", c.rest()));
        }
        if refs.len() > 1 && label.is_some() {
            return Err("a label needs a single node (`a: Label`), not a list".into());
        }
        for (id, qlabel) in refs {
            if self.doc.group(&id).is_some() {
                return Err(format!("`{id}` is a group"));
            }
            let group = self.group();
            if self.doc.node(&id).is_none() {
                self.doc.nodes.push(Node::new(&id));
            }
            let n = self.doc.node_mut(&id).unwrap();
            if group.is_some() {
                n.group = group;
            }
            if let Some(l) = label.clone().or(qlabel) {
                n.label = l;
            }
            apply_node_attrs(n, &attrs)?;
            self.touched.push(Elem::Node(id));
        }
        Ok(())
    }

    fn edges(
        &mut self,
        first: Vec<(String, Option<String>)>,
        op: (Arrow, Line, bool),
        mut c: Cursor,
    ) -> Result<(), String> {
        let mut lists = vec![first];
        let mut ops = vec![op];
        loop {
            c.skip_ws();
            lists.push(self.ref_list(&mut c)?);
            c.skip_ws();
            match c.edge_op() {
                Some(op) => ops.push(op),
                None => break,
            }
        }
        let (label, attrs) = label_and_attrs(&mut c)?;
        if !c.at_end() {
            return Err(format!("unexpected `{}`", c.rest()));
        }
        for list in &lists {
            for (id, l) in list {
                self.ensure_node(id, l.as_deref());
            }
        }
        for (i, (arrow, line, reversed)) in ops.iter().enumerate() {
            for (a, _) in &lists[i] {
                for (b, _) in &lists[i + 1] {
                    let (from, to) = if *reversed { (b, a) } else { (a, b) };
                    let mut e = Edge {
                        from: from.clone(),
                        to: to.clone(),
                        label: label.clone(),
                        arrow: *arrow,
                        line: *line,
                        ..Default::default()
                    };
                    apply_edge_attrs(&mut e, &attrs)?;
                    if e.id.is_empty() {
                        e.id = self.doc.fresh_edge_id(&e.from, &e.to);
                    } else if self.doc.id_taken(&e.id) {
                        return Err(format!("id `{}` is already used", e.id));
                    }
                    self.touched.push(Elem::Edge(e.id.clone()));
                    self.doc.edges.push(e);
                }
            }
        }
        Ok(())
    }

    fn group_decl(&mut self, mut c: Cursor, lanes: bool) -> Result<(), String> {
        c.skip_ws();
        let (id, qlabel) = c.reference()?;
        let (label, attrs) = label_and_attrs_until_brace(&mut c)?;
        c.skip_ws();
        if !c.eat('{') {
            return Err("a group needs a body: `group id: Label {` … `}`".into());
        }
        c.skip_ws();
        let closes_inline = c.eat('}');
        if !c.at_end() {
            return Err("put the group's contents on the following lines".into());
        }
        if self.doc.node(&id).is_some() {
            return Err(format!("`{id}` is already a node"));
        }
        let parent = self.group();
        let again = self.doc.group(&id).is_some();
        if !again {
            self.doc.groups.push(Group {
                id: id.clone(),
                ..Default::default()
            });
        }
        let g = self.doc.group_mut(&id).unwrap();
        // a group written again at the top level keeps its place (and a
        // lane stays a lane): `lane platform { … }` adds to it
        if !again || parent.is_some() {
            g.parent = parent;
        }
        g.lanes = lanes || (again && g.lanes);
        if let Some(l) = label.or(qlabel) {
            g.label = l;
        }
        for (k, v) in &attrs {
            match (k.as_str(), v) {
                ("color" | "colour", Some(v)) => g.color = Some(color(v)?),
                ("label", Some(v)) => g.label = v.clone(),
                ("x", Some(v)) if lanes => g.x = Some(num("x", v)?),
                ("y", Some(v)) if lanes => g.y = Some(num("y", v)?),
                (c, None) if valid_color(c) => g.color = Some(c.to_string()),
                (k, _) => return Err(format!("unknown group attribute `{k}`")),
            }
        }
        self.touched.push(Elem::Group(id.clone()));
        if !closes_inline {
            self.stack.push((id, self.line));
        }
        Ok(())
    }

    /// `comment [ID] [on REF] [at X,Y] [by AUTHOR]: text [attrs]`
    fn comment(&mut self, mut c: Cursor) -> Result<(), String> {
        let mut cm = Comment::default();
        loop {
            c.skip_ws();
            if c.peek() == Some(':') || c.at_end() {
                break;
            }
            match c.peek_word() {
                "on" => {
                    c.advance(2);
                    c.skip_ws();
                    cm.on = Some(c.target()?);
                }
                "at" => {
                    c.advance(2);
                    c.skip_ws();
                    let w = c.target()?;
                    let (x, y) = w.split_once(',').ok_or("use `at X,Y`")?;
                    cm.x = Some(num("x", x)?);
                    cm.y = Some(num("y", y)?);
                }
                "by" => {
                    c.advance(2);
                    c.skip_ws();
                    cm.author = Some(author(c.reference()?));
                }
                _ if cm.id.is_empty() && cm.on.is_none() => cm.id = c.target()?,
                w => return Err(format!("unexpected `{w}` in comment (use `comment [ID] [on TARGET] [by AUTHOR]: text`)")),
            }
        }
        if !c.eat(':') {
            return Err("a comment needs text after `:`".into());
        }
        let (text, attrs) = split_attrs(c.rest());
        cm.text = unescape_label(text.trim());
        for (k, v) in parse_attrs(attrs.unwrap_or(""))? {
            match (k.as_str(), v) {
                ("status", Some(v)) => {
                    cm.status = Status::parse(&v)
                        .ok_or_else(|| format!("unknown status `{v}` (use open, resolved)"))?
                }
                ("resolved" | "done", None) => cm.status = Status::Resolved,
                ("open", None) => cm.status = Status::Open,
                ("resolution", Some(v)) => cm.resolution = Some(v),
                ("author" | "by", Some(v)) => cm.author = Some(v),
                ("x", Some(v)) => cm.x = Some(num("x", &v)?),
                ("y", Some(v)) => cm.y = Some(num("y", &v)?),
                (k, _) => return Err(format!("unknown comment attribute `{k}`")),
            }
        }
        if cm.id.is_empty() {
            cm.id = self.doc.fresh_comment_id();
        } else if self.doc.comment(&cm.id).is_some() {
            return Err(format!("comment `{}` already exists", cm.id));
        }
        self.touched.push(Elem::Comment(cm.id.clone()));
        self.doc.comments.push(cm);
        Ok(())
    }

    /// `reply ID [by AUTHOR]: text`
    fn reply(&mut self, mut c: Cursor) -> Result<(), String> {
        c.skip_ws();
        let id = c.target()?;
        c.skip_ws();
        let mut by = None;
        if c.peek_word() == "by" {
            c.advance(2);
            c.skip_ws();
            by = Some(author(c.reference()?));
            c.skip_ws();
        }
        if !c.eat(':') {
            return Err("a reply needs text after `:`".into());
        }
        let text = unescape_label(c.rest().trim());
        let cm = self.doc.comment_mut(&id).ok_or_else(|| {
            format!("no comment `{id}` to reply to (replies go after their comment)")
        })?;
        cm.replies.push(Reply { author: by, text });
        self.touched.push(Elem::Comment(id));
        Ok(())
    }
}

/// `by "Ana B."` keeps the text as written, `by ana` the word.
fn author(r: (String, Option<String>)) -> String {
    r.1.unwrap_or(r.0)
}

fn num(what: &str, v: &str) -> Result<f64, String> {
    v.trim()
        .parse::<f64>()
        .map_err(|_| format!("`{what}` must be a number, got `{}`", v.trim()))
}

fn color(v: &str) -> Result<String, String> {
    let v = v.trim().to_ascii_lowercase();
    if valid_color(&v) {
        Ok(v)
    } else {
        Err(format!(
            "unknown colour `{v}` (use {} or #rrggbb)",
            COLORS.join(", ")
        ))
    }
}

/// `: label [attrs]` or `[attrs]` or nothing.
fn label_and_attrs(c: &mut Cursor) -> Result<(Option<String>, Attrs), String> {
    c.skip_ws();
    if c.eat(':') {
        let (text, attrs) = split_attrs(c.rest());
        c.i = c.s.len();
        let label = unescape_label(text.trim());
        return Ok((
            Some(label).filter(|l| !l.is_empty()),
            parse_attrs(attrs.unwrap_or(""))?,
        ));
    }
    if c.peek() == Some('[') {
        let (text, attrs) = split_attrs(c.rest());
        if text.trim().is_empty() {
            c.i = c.s.len();
            return Ok((None, parse_attrs(attrs.unwrap_or(""))?));
        }
    }
    Ok((None, Vec::new()))
}

/// Like `label_and_attrs`, but stops before a trailing `{` / `{}`.
fn label_and_attrs_until_brace(c: &mut Cursor) -> Result<(Option<String>, Attrs), String> {
    let rest = c.rest().trim_end();
    let cut = if rest.ends_with("{}") {
        rest.len() - 2
    } else if rest.ends_with('{') {
        rest.len() - 1
    } else {
        rest.len()
    };
    let head = rest[..cut].to_string();
    let start = c.i;
    let mut sub = Cursor::new(&head);
    let out = label_and_attrs(&mut sub)?;
    if !sub.at_end() {
        return Err(format!("unexpected `{}`", sub.rest().trim()));
    }
    c.i = start + cut;
    Ok(out)
}

/// Split `text [key: value, …]` into the text and the attribute block's
/// contents. A trailing `[...]` only counts as attributes when it looks like
/// them (`key: value` items or known keywords), so `Step [draft]` stays a label.
pub fn split_attrs(text: &str) -> (&str, Option<&str>) {
    let t = text.trim_end();
    if !t.ends_with(']') {
        return (text, None);
    }
    for (pos, ch) in t.char_indices() {
        if ch != '[' {
            continue;
        }
        if let Some(end) = scan_block(&t[pos..]) {
            if pos + end == t.len() {
                let inner = &t[pos + 1..t.len() - 1];
                // attributes, or meant as attributes (`[detail: …, oops]`):
                // read them, so a mistake is an error, not label text
                if looks_like_attrs(inner) || meant_as_attrs(inner) {
                    return (&t[..pos], Some(inner));
                }
            }
        }
    }
    (text, None)
}

/// Byte length of a `[...]` block at the start of `s`, honouring quotes.
fn scan_block(s: &str) -> Option<usize> {
    let mut quoted = false;
    let mut escaped = false;
    for (i, ch) in s.char_indices().skip(1) {
        match ch {
            _ if escaped => escaped = false,
            '\\' if quoted => escaped = true,
            '"' => quoted = !quoted,
            ']' if !quoted => return Some(i + 1),
            '[' if !quoted => return None,
            _ => {}
        }
    }
    None
}

fn looks_like_attrs(inner: &str) -> bool {
    let items = split_items(inner);
    !items.is_empty()
        && items.iter().all(|item| {
            let item = item.trim();
            match item.split_once(':') {
                Some((k, _)) => {
                    !k.trim().is_empty()
                        && k.trim()
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || c == '_')
                }
                None => is_flag(item),
            }
        })
}

/// An attribute written without a value: a shape, colour, line style,
/// status, gantt date or duration.
fn is_flag(item: &str) -> bool {
    Shape::parse(item).is_some()
        || Line::parse(item).is_some()
        || valid_color(&item.to_ascii_lowercase())
        || matches!(item, "resolved" | "open" | "done" | "collapsed" | "folded" | "milestone")
        // charts
        || ChartKind::parse(item).is_some()
        || matches!(item, "stacked" | "horizontal")
        // gantt: a date or a duration
        || crate::gantt::parse_date(item).is_some()
        || (item.ends_with(['d', 'w', 'm', 'q']) && crate::gantt::parse_days(item).is_some())
}

/// Every attribute key the language knows.
const KEYS: &[&str] = &[
    "shape",
    "color",
    "colour",
    "detail",
    "label",
    "link",
    "url",
    "tags",
    "tag",
    "x",
    "y",
    "w",
    "h",
    "width",
    "height",
    "collapsed",
    "folded",
    "layout",
    "parent",
    "start",
    "days",
    "duration",
    "after",
    "milestone",
    "done",
    "progress",
    "owner",
    "p",
    "probability",
    "likelihood",
    "i",
    "impact",
    "id",
    "line",
    "arrow",
    "status",
    "resolution",
    "author",
    "by",
    "scale",
    "levels",
    "type",
    "kind",
    "unit",
];

/// Attributes whose value is free text: a `,` or `;` in it, followed by
/// something that is not an attribute, belongs to the text.
const TEXT_KEYS: &[&str] = &["detail", "label", "resolution", "owner", "author", "by"];

/// `[detail: …, …]` that does not read as attributes but starts like them.
fn meant_as_attrs(inner: &str) -> bool {
    split_items(inner).first().is_some_and(|first| {
        first
            .split_once(':')
            .is_some_and(|(k, _)| KEYS.contains(&k.trim().to_ascii_lowercase().as_str()))
    })
}

fn split_items(inner: &str) -> Vec<String> {
    // (item, the separator before it)
    let mut raw: Vec<(String, char)> = Vec::new();
    let mut cur = String::new();
    let mut sep = ',';
    let mut quoted = false;
    let mut escaped = false;
    for ch in inner.chars() {
        match ch {
            _ if escaped => {
                escaped = false;
                cur.push(ch);
            }
            '\\' if quoted => {
                escaped = true;
                cur.push(ch);
            }
            '"' => {
                quoted = !quoted;
                cur.push(ch);
            }
            ',' | ';' if !quoted => {
                raw.push((std::mem::take(&mut cur), sep));
                sep = ch;
            }
            _ => cur.push(ch),
        }
    }
    raw.push((cur, sep));
    // `detail: fast, opinionated`: the rest of a text value, not an attribute
    let mut out: Vec<String> = Vec::new();
    for (item, sep) in raw {
        let t = item.trim();
        let text_before = out.last().is_some_and(|prev: &String| {
            prev.split_once(':').is_some_and(|(k, v)| {
                TEXT_KEYS.contains(&k.trim().to_ascii_lowercase().as_str())
                    && !v.trim().starts_with('"')
            })
        });
        if !t.contains(':') && !is_flag(t) && text_before {
            let last = out.last_mut().unwrap();
            last.push(sep);
            last.push_str(&item);
        } else {
            out.push(item);
        }
    }
    out.into_iter().filter(|s| !s.trim().is_empty()).collect()
}

/// The items of an attribute block (`shape: box, red`) as key / value pairs.
pub fn parse_attr_list(inner: &str) -> Result<Attrs, String> {
    parse_attrs(inner)
}

fn parse_attrs(inner: &str) -> Result<Attrs, String> {
    let mut out = Vec::new();
    for item in split_items(inner) {
        let item = item.trim();
        match item.split_once(':') {
            Some((k, v)) if !k.contains('"') => {
                out.push((k.trim().to_ascii_lowercase(), Some(unquote(v.trim()))))
            }
            _ => out.push((item.to_ascii_lowercase(), None)),
        }
    }
    Ok(out)
}

pub fn apply_node_attrs(n: &mut Node, attrs: &Attrs) -> Result<(), String> {
    for (k, v) in attrs {
        match (k.as_str(), v) {
            ("shape", Some(v)) => {
                n.shape = Shape::parse(v)
                    .ok_or_else(|| format!("unknown shape `{v}` (use {})", Shape::names()))?
            }
            ("color" | "colour", Some(v)) => n.color = Some(color(v)?),
            ("detail", Some(v)) => n.detail = Some(unescape_label(v)).filter(|s| !s.is_empty()),
            ("label", Some(v)) => n.label = unescape_label(v),
            ("link" | "url", Some(v)) => n.link = Some(v.clone()).filter(|s| !s.is_empty()),
            ("tags" | "tag", Some(v)) => n.tags = v.split_whitespace().map(String::from).collect(),
            ("x", Some(v)) => n.x = Some(num("x", v)?),
            ("y", Some(v)) => n.y = Some(num("y", v)?),
            ("w" | "width", Some(v)) => n.w = Some(num("w", v)?),
            ("h" | "height", Some(v)) => n.h = Some(num("h", v)?),
            ("collapsed" | "folded", None) => n.collapsed = true,
            ("collapsed" | "folded", Some(v)) => n.collapsed = !matches!(v.as_str(), "false" | "no" | "0"),
            ("layout", Some(v)) => {
                n.mindmap = Some(MapLayout::parse(v).ok_or_else(|| {
                    format!("unknown mindmap layout `{v}` (use {})", MapLayout::names())
                })?)
            }
            ("parent", Some(v)) => n.parent = Some(v.clone()).filter(|s| !s.is_empty()),
            ("start", Some(v)) => n.start = Some(date(v)?),
            ("days" | "duration", Some(v)) => {
                n.days = Some(crate::gantt::parse_days(v).ok_or_else(|| {
                    format!("`{v}` is not a duration (like 5d, 2w, 1m or 1q)")
                })?)
            }
            ("after", Some(v)) => n.after = v.split_whitespace().map(String::from).collect(),
            ("milestone", None) => n.milestone = true,
            ("done", None) => n.done = Some(100.0),
            ("done" | "progress", Some(v)) => {
                n.done = Some(percent(v)?).filter(|d| *d > 0.0);
            }
            ("owner", Some(v)) => n.owner = Some(v.clone()).filter(|s| !s.is_empty()),
            ("p" | "probability" | "likelihood", Some(v)) => n.probability = Some(level(k, v)?),
            ("i" | "impact", Some(v)) => n.impact = Some(level(k, v)?),
            (k, None) if crate::gantt::parse_date(k).is_some() => n.start = Some(k.to_string()),
            (k, None) if k.ends_with(['d', 'w', 'm', 'q']) && crate::gantt::parse_days(k).is_some() => {
                n.days = crate::gantt::parse_days(k)
            }
            (k, None) if Shape::parse(k).is_some() => n.shape = Shape::parse(k).unwrap(),
            (k, None) if valid_color(k) => n.color = Some(k.to_string()),
            (k, _) => {
                return Err(format!(
                    "unknown node attribute `{k}` (shape, color, detail, link, tags, x, y, w, h, collapsed, layout, parent, start, days, after, milestone, done, owner, p, i)"
                ))
            }
        }
    }
    Ok(())
}

pub fn apply_edge_attrs(e: &mut Edge, attrs: &Attrs) -> Result<(), String> {
    for (k, v) in attrs {
        match (k.as_str(), v) {
            ("label", Some(v)) => e.label = Some(unescape_label(v)).filter(|s| !s.is_empty()),
            ("arrow", Some(v)) => {
                e.arrow = Arrow::parse(v)
                    .ok_or_else(|| format!("unknown arrow `{v}` (use {})", Arrow::names()))?
            }
            ("line" | "style", Some(v)) => {
                e.line = Line::parse(v)
                    .ok_or_else(|| format!("unknown line `{v}` (use {})", Line::names()))?
            }
            ("color" | "colour", Some(v)) => e.color = Some(color(v)?),
            ("id", Some(v)) => e.id = v.clone(),
            (k, None) if Line::parse(k).is_some() => e.line = Line::parse(k).unwrap(),
            (k, None) if valid_color(k) => e.color = Some(k.to_string()),
            (k, _) => {
                return Err(format!(
                    "unknown edge attribute `{k}` (label, arrow, line, color, id)"
                ))
            }
        }
    }
    Ok(())
}

/// `"text"` → text with escapes resolved; anything else unchanged.
pub fn unquote(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        unescape(&s[1..s.len() - 1])
    } else {
        s.to_string()
    }
}

fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(o) => out.push(o),
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Labels: `"quoted"` or bare text where `\n` is a line break.
pub fn unescape_label(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        unquote(s)
    } else {
        unescape(s)
    }
}

struct Cursor<'a> {
    s: &'a str,
    i: usize,
}

impl<'a> Cursor<'a> {
    fn new(s: &'a str) -> Self {
        Cursor { s, i: 0 }
    }
    fn rest(&self) -> &'a str {
        &self.s[self.i..]
    }
    fn at_end(&self) -> bool {
        self.rest().trim().is_empty()
    }
    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }
    fn advance(&mut self, n: usize) {
        self.i = (self.i + n).min(self.s.len());
    }
    fn eat(&mut self, ch: char) -> bool {
        if self.peek() == Some(ch) {
            self.i += ch.len_utf8();
            true
        } else {
            false
        }
    }
    fn skip_ws(&mut self) {
        let r = self.rest();
        self.i += r.len() - r.trim_start().len();
    }
    fn peek_word(&self) -> &'a str {
        let r = self.rest();
        let end = r
            .find(|c: char| !(c.is_ascii_alphabetic() || c == '_'))
            .unwrap_or(r.len());
        &r[..end]
    }
    fn edge_op(&mut self) -> Option<(Arrow, Line, bool)> {
        let r = self.rest();
        let (tok, a, l, rev) = OPS.iter().find(|(tok, ..)| r.starts_with(tok))?;
        self.i += tok.len();
        Some((*a, *l, *rev))
    }
    fn string(&mut self) -> Result<String, String> {
        let r = self.rest();
        let mut escaped = false;
        for (i, ch) in r.char_indices().skip(1) {
            match ch {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => {
                    self.i += i + 1;
                    return Ok(unescape(&r[1..i]));
                }
                _ => {}
            }
        }
        Err("unterminated string".into())
    }
    fn ident(&mut self) -> Option<String> {
        let r = self.rest();
        let chars: Vec<(usize, char)> = r.char_indices().collect();
        let word = |c: char| c.is_alphanumeric() || c == '_';
        let mut end = 0;
        for (k, &(i, c)) in chars.iter().enumerate() {
            let ok = word(c)
                || ((c == '-' || c == '.')
                    && end > 0
                    && chars.get(k + 1).is_some_and(|&(_, n)| word(n)));
            if !ok {
                break;
            }
            end = i + c.len_utf8();
        }
        if end == 0 {
            return None;
        }
        self.i += end;
        Some(r[..end].to_string())
    }
    /// An id (`api`) or a quoted label (`"Orders API"` → id `orders-api`).
    fn reference(&mut self) -> Result<(String, Option<String>), String> {
        if self.peek() == Some('"') {
            let text = self.string()?;
            return Ok((slug(&text), Some(text)));
        }
        match self.ident() {
            Some(id) => Ok((id, None)),
            None => Err(match self.peek() {
                Some(c) => format!("expected a name, found `{c}`"),
                None => "expected a name".into(),
            }),
        }
    }
    /// A comment target or id: up to whitespace or `:` (`api->db#2`, `10,20`).
    fn target(&mut self) -> Result<String, String> {
        if self.peek() == Some('"') {
            return self.string();
        }
        let r = self.rest();
        let end = r
            .find(|c: char| c.is_whitespace() || c == ':')
            .unwrap_or(r.len());
        if end == 0 {
            return Err("expected a name".into());
        }
        self.i += end;
        Ok(r[..end].to_string())
    }
}

// ---------------------------------------------------------------- printing

pub fn is_ident(s: &str) -> bool {
    let mut c = Cursor::new(s);
    c.ident().is_some_and(|id| id.len() == s.len())
}

fn id_text(id: &str) -> String {
    if is_ident(id) {
        id.to_string()
    } else {
        quote(id)
    }
}

pub fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn label_text(s: &str) -> String {
    let needs_quotes =
        s.starts_with('"') || s.trim() != s || s.trim_end().ends_with(']') || s.is_empty();
    if needs_quotes {
        quote(s)
    } else {
        s.replace('\\', "\\\\")
            .replace('\n', "\\n")
            .replace('\t', "\\t")
    }
}

fn attr_value(v: &str) -> String {
    if v.is_empty() || v.trim() != v || v.contains([',', ';', '[', ']', '"', '\n']) {
        quote(v)
    } else {
        v.to_string()
    }
}

fn fmt_num(v: f64) -> String {
    let r = (v * 10.0).round() / 10.0;
    if r.fract() == 0.0 {
        format!("{}", r as i64)
    } else {
        format!("{r}")
    }
}

fn attrs_text(items: &[String]) -> String {
    if items.is_empty() {
        String::new()
    } else {
        format!(" [{}]", items.join(", "))
    }
}

pub(crate) fn node_line(n: &Node) -> String {
    let mut s = id_text(&n.id);
    if !n.label.is_empty() && n.label != n.id {
        s.push_str(": ");
        s.push_str(&label_text(&n.label));
    }
    let mut a = node_attrs(n);
    if let Some(l) = n.mindmap {
        a.push(format!("layout: {}", l.as_str()));
    }
    if let Some(p) = &n.parent {
        a.push(format!("parent: {}", attr_value(p)));
    }
    s + &attrs_text(&a)
}

/// `mindmap id: Topic [attrs] {` + the indented outline + `}`.
/// `x: …, y: …` of a pinned swimlanes or gantt chart.
fn group_pos(g: &Group) -> Vec<String> {
    [("x", g.x), ("y", g.y)]
        .into_iter()
        .filter_map(|(k, v)| v.map(|v| format!("{k}: {}", fmt_num(v))))
        .collect()
}

pub(crate) fn gantt_block(doc: &Document, g: &Group, pad: &str, out: &mut String) {
    let mut head = id_text(&g.id);
    if !g.label.is_empty() && g.label != g.id {
        head.push_str(": ");
        head.push_str(&label_text(&g.label));
    }
    let mut a = Vec::new();
    if let Some(s) = &g.start {
        a.push(format!("start: {s}"));
    }
    if let Some(sc) = g.scale {
        a.push(format!("scale: {}", sc.as_str()));
    }
    if let Some(l) = g.levels {
        a.push(format!("scale: {l}"));
    }
    if let Some(c) = &g.color {
        a.push(if c.starts_with('#') {
            format!("color: {c}")
        } else {
            c.clone()
        });
    }
    a.extend(group_pos(g));
    head.push_str(&attrs_text(&a));
    let tasks: Vec<&Node> = doc
        .nodes
        .iter()
        .filter(|n| n.group.as_deref() == Some(g.id.as_str()))
        .collect();
    let word = if g.risks { "risks" } else { "gantt" };
    if tasks.is_empty() {
        out.push_str(&format!("{pad}{word} {head} {{}}\n"));
        return;
    }
    out.push_str(&format!("{pad}{word} {head} {{\n"));
    // a task in another chart whose id a task here is named after:
    // `chart/task`, so it does not read back as this chart's
    let named = |a: &str| -> String {
        match doc.node(a).and_then(|t| t.group.as_deref()) {
            Some(c) if c != g.id && tasks.iter().any(|t| slug(t.display()) == a) => {
                format!("{c}/{a}")
            }
            _ => a.to_string(),
        }
    };
    for n in &tasks {
        let label = n.display();
        let mut m = (*n).clone();
        m.after = m.after.iter().map(|a| named(a)).collect();
        let mut a = node_attrs(&m);
        if slug(label) != n.id {
            a.insert(0, format!("id: {}", attr_value(&n.id)));
        }
        out.push_str(&format!(
            "{pad}  {}{}\n",
            crate::mindmap::outline_label(label),
            attrs_text(&a)
        ));
    }
    out.push_str(&format!("{pad}}}\n"));
}

/// A chart: its header row and one row per node.
pub(crate) fn chart_block(doc: &Document, g: &Group, pad: &str, out: &mut String) {
    let mut head = id_text(&g.id);
    if !g.label.is_empty() && g.label != g.id {
        head.push_str(": ");
        head.push_str(&label_text(&g.label));
    }
    let mut a = vec![g.chart.unwrap_or_default().as_str().to_string()];
    if g.stacked {
        a.push("stacked".into());
    }
    if g.horizontal {
        a.push("horizontal".into());
    }
    if let Some(u) = &g.unit {
        a.push(format!("unit: {}", attr_value(u)));
    }
    if let Some(c) = &g.color {
        a.push(if c.starts_with('#') {
            format!("color: {c}")
        } else {
            c.clone()
        });
    }
    for (k, v) in [("w", g.w), ("h", g.h)] {
        if let Some(v) = v {
            a.push(format!("{k}: {}", fmt_num(v)));
        }
    }
    a.extend(group_pos(g));
    head.push_str(&attrs_text(&a));
    let rows: Vec<&Node> = doc
        .nodes
        .iter()
        .filter(|n| n.group.as_deref() == Some(g.id.as_str()))
        .collect();
    if rows.is_empty() && g.series.is_empty() {
        out.push_str(&format!("{pad}chart {head} {{}}\n"));
        return;
    }
    out.push_str(&format!("{pad}chart {head} {{\n"));
    let cell = |s: &str| s.replace('|', "/");
    if !g.series.is_empty() {
        let mut cells = vec![cell(g.axis.as_deref().unwrap_or(""))];
        cells.extend(g.series.iter().map(|s| cell(s)));
        out.push_str(&format!("{pad}  {}\n", cells.join(" | ").trim_start()));
    }
    for n in rows {
        let label = n.display();
        let mut cells = vec![cell(&crate::mindmap::outline_label(label))];
        cells.extend(n.values.iter().map(|v| fmt_value(*v)));
        let mut m = n.clone();
        m.values.clear();
        let mut a = node_attrs(&m);
        if row_id(&g.id, label) != n.id {
            a.insert(0, format!("id: {}", attr_value(&n.id)));
        }
        out.push_str(&format!("{pad}  {}{}\n", cells.join(" | "), attrs_text(&a)));
    }
    out.push_str(&format!("{pad}}}\n"));
}

pub(crate) fn map_block(doc: &Document, root: &Node, pad: &str, out: &mut String) {
    let mut head = id_text(&root.id);
    if !root.label.is_empty() && root.label != root.id {
        head.push_str(": ");
        head.push_str(&label_text(&root.label));
    }
    let mut a = node_attrs(root);
    if let Some(l) = root.mindmap.filter(|l| *l != MapLayout::default()) {
        a.push(format!("layout: {}", l.as_str()));
    }
    head.push_str(&attrs_text(&a));
    let kids = doc.children(&root.id);
    if kids.is_empty() {
        out.push_str(&format!("{pad}mindmap {head} {{}}\n"));
        return;
    }
    out.push_str(&format!("{pad}mindmap {head} {{\n"));
    fn walk(
        doc: &Document,
        id: &str,
        depth: usize,
        pad: &str,
        out: &mut String,
        seen: &mut Vec<String>,
    ) {
        for c in doc.children(id) {
            if seen.contains(&c.id) {
                continue;
            }
            seen.push(c.id.clone());
            let label = c.display();
            let mut a = node_attrs(c);
            if slug(label) != c.id {
                a.insert(0, format!("id: {}", attr_value(&c.id)));
            }
            out.push_str(&format!(
                "{pad}{}{}{}\n",
                "  ".repeat(depth),
                crate::mindmap::outline_label(label),
                attrs_text(&a)
            ));
            walk(doc, &c.id, depth + 1, pad, out, seen);
        }
    }
    let mut seen = vec![root.id.clone()];
    walk(doc, &root.id, 1, pad, out, &mut seen);
    out.push_str(&format!("{pad}}}\n"));
}

/// What gantt tasks' `after:` names mean: a task of the same chart first
/// (by id, then by its label's slug, so `after: design` is this chart's
/// Design even when another chart's took the id `design`), then any id on
/// the board. `chart/task` names a task of another chart.
fn resolve_after(doc: &mut Document) {
    let tasks: Vec<(String, String, String)> = doc
        .nodes
        .iter()
        .filter(|n| {
            n.group
                .as_deref()
                .and_then(|g| doc.group(g))
                .is_some_and(|g| g.gantt)
        })
        .map(|n| (n.group.clone().unwrap(), n.id.clone(), slug(n.display())))
        .collect();
    let find = |chart: &str, name: &str| -> Option<String> {
        let mine = || tasks.iter().filter(|t| t.0 == chart);
        if let Some(t) = mine().find(|t| t.1 == name) {
            return Some(t.1.clone());
        }
        let by_label: Vec<_> = mine().filter(|t| t.2 == name).collect();
        match by_label[..] {
            [t] => Some(t.1.clone()),
            _ => None,
        }
    };
    for n in &mut doc.nodes {
        let Some(chart) = n.group.clone() else {
            continue;
        };
        if !tasks.iter().any(|t| t.0 == chart) {
            continue;
        }
        for a in &mut n.after {
            let found = match a.split_once('/') {
                Some((c, t)) => find(c, t),
                None => find(&chart, a),
            };
            if let Some(id) = found {
                *a = id;
            }
        }
    }
}

/// A chart row's id: the chart's, then the row's label (`revenue.q1`).
pub fn row_id(chart: &str, label: &str) -> String {
    format!("{chart}.{}", slug(label))
}

/// A chart cell: a number (`1200`, `1,200`, `-3.5`, `40%`), or `-` / empty
/// for none.
fn value(v: &str) -> Result<Option<f64>, String> {
    let t = v.trim();
    if t.is_empty() || t == "-" || t == "–" {
        return Ok(None);
    }
    let n = t.trim_end_matches('%').replace([',', '_'], "");
    n.parse::<f64>()
        .ok()
        .filter(|x| x.is_finite())
        .map(Some)
        .ok_or_else(|| format!("`{v}` is not a number"))
}

/// A chart value as written: `120`, `3.5`, `-` for none.
pub(crate) fn fmt_value(v: Option<f64>) -> String {
    v.map_or("-".to_string(), fmt_num)
}

/// A risk's probability or impact: 1 to 5.
fn level(k: &str, v: &str) -> Result<u8, String> {
    v.trim()
        .parse::<u8>()
        .ok()
        .filter(|n| (1..=5).contains(n))
        .ok_or_else(|| format!("`{k}` is a level from 1 to 5, not `{v}`"))
}

/// A percentage: `60%` or `60`, 0 to 100.
fn percent(v: &str) -> Result<f64, String> {
    v.trim()
        .trim_end_matches('%')
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|d| (0.0..=100.0).contains(d))
        .ok_or_else(|| format!("`{v}` is not a percentage (like 60%)"))
}

/// A `YYYY-MM-DD` date, checked.
fn date(v: &str) -> Result<String, String> {
    crate::gantt::parse_date(v)
        .map(crate::gantt::format_date)
        .ok_or_else(|| format!("`{v}` is not a date (YYYY-MM-DD)"))
}

fn node_attrs(n: &Node) -> Vec<String> {
    let mut a = Vec::new();
    // gantt tasks read best with their schedule first
    if let Some(s) = &n.start {
        a.push(s.clone());
    }
    if let Some(d) = n.days {
        a.push(crate::gantt::format_days(d));
    }
    if !n.after.is_empty() {
        a.push(format!("after: {}", n.after.join(" ")));
    }
    if n.milestone {
        a.push("milestone".into());
    }
    match n.done {
        Some(d) if d >= 100.0 => a.push("done".into()),
        Some(d) => a.push(format!("done: {}%", fmt_num(d))),
        None => {}
    }
    if let Some(p) = n.probability {
        a.push(format!("p: {p}"));
    }
    if let Some(i) = n.impact {
        a.push(format!("i: {i}"));
    }
    if let Some(o) = &n.owner {
        a.push(format!("owner: {}", attr_value(o)));
    }
    if n.shape != Shape::default() {
        a.push(n.shape.as_str().to_string());
    }
    if let Some(c) = &n.color {
        a.push(if c.starts_with('#') {
            format!("color: {c}")
        } else {
            c.clone()
        });
    }
    if let Some(d) = &n.detail {
        a.push(format!("detail: {}", attr_value(&d.replace('\n', "\\n"))));
    }
    if let Some(l) = &n.link {
        a.push(format!("link: {}", attr_value(l)));
    }
    if !n.tags.is_empty() {
        a.push(format!("tags: {}", attr_value(&n.tags.join(" "))));
    }
    for (k, v) in [("x", n.x), ("y", n.y), ("w", n.w), ("h", n.h)] {
        if let Some(v) = v {
            a.push(format!("{k}: {}", fmt_num(v)));
        }
    }
    if n.collapsed {
        a.push("collapsed".into());
    }
    a
}

pub(crate) fn edge_line(e: &Edge) -> String {
    let base = |l: Line| match l {
        Line::Solid | Line::Dotted => ("->", "<->", "--"),
        Line::Dashed => ("..>", "<..>", ".."),
        Line::Thick => ("=>", "<=>", "=="),
    };
    let (end, both, none) = base(e.line);
    let mut a = Vec::new();
    let op = match e.arrow {
        Arrow::End => end,
        Arrow::Both => both,
        Arrow::None => none,
        Arrow::Start => {
            a.push("arrow: start".to_string());
            end
        }
    };
    if e.line == Line::Dotted {
        a.push("dotted".into());
    }
    if let Some(c) = &e.color {
        a.push(if c.starts_with('#') {
            format!("color: {c}")
        } else {
            c.clone()
        });
    }
    let auto = format!("{}->{}", e.from, e.to);
    if !e.id.starts_with(&auto) {
        a.push(format!("id: {}", attr_value(&e.id)));
    }
    let mut s = format!("{} {op} {}", id_text(&e.from), id_text(&e.to));
    if let Some(l) = &e.label {
        s.push_str(": ");
        s.push_str(&label_text(l));
    }
    s + &attrs_text(&a)
}

pub(crate) fn comment_lines(c: &Comment, out: &mut String) {
    let mut s = format!("comment {}", id_text(&c.id));
    if let Some(on) = &c.on {
        s.push_str(&format!(
            " on {}",
            if on.contains(char::is_whitespace) || on.contains(':') {
                quote(on)
            } else {
                on.clone()
            }
        ));
    }
    if let (Some(x), Some(y)) = (c.x, c.y) {
        s.push_str(&format!(" at {},{}", fmt_num(x), fmt_num(y)));
    }
    if let Some(a) = &c.author {
        s.push_str(&format!(" by {}", id_text(a)));
    }
    s.push_str(": ");
    s.push_str(&label_text(&c.text));
    let mut a = Vec::new();
    if c.status == Status::Resolved {
        a.push("resolved".to_string());
    }
    if let Some(r) = &c.resolution {
        a.push(format!(
            "resolution: {}",
            attr_value(&r.replace('\n', "\\n"))
        ));
    }
    out.push_str(&s);
    out.push_str(&attrs_text(&a));
    out.push('\n');
    for r in &c.replies {
        let by = r
            .author
            .as_ref()
            .map(|a| format!(" by {}", id_text(a)))
            .unwrap_or_default();
        out.push_str(&format!(
            "reply {}{by}: {}\n",
            id_text(&c.id),
            label_text(&r.text)
        ));
    }
}

/// Canonical text form. `parse(print(doc)) == doc` for normalized documents.
/// A setting's line (`title: …`), or None when it has its default.
pub(crate) fn setting_line(doc: &Document, key: &str) -> Option<String> {
    let s = &doc.settings;
    let def = Settings::default();
    match key {
        "title" => doc
            .title
            .as_ref()
            .map(|t| format!("title: {}", t.replace('\n', " "))),
        "description" => doc
            .description
            .as_ref()
            .map(|d| format!("description: {}", d.replace('\n', " "))),
        "direction" => {
            (s.direction != def.direction).then(|| format!("direction: {}", s.direction.as_str()))
        }
        "layout" => (s.layout != def.layout).then(|| format!("layout: {}", s.layout.as_str())),
        "routing" => (s.routing != def.routing).then(|| format!("routing: {}", s.routing.as_str())),
        "theme" => (s.theme != def.theme).then(|| format!("theme: {}", s.theme.as_str())),
        _ => None,
    }
}

/// Which nodes are mindmap topics (or centres).
pub(crate) fn map_members(doc: &Document) -> impl Fn(&Node) -> bool + '_ {
    let forest = crate::mindmap::Forest::new(doc);
    move |n: &Node| {
        doc.nodes
            .iter()
            .position(|x| x.id == n.id)
            .is_some_and(|i| forest.in_map(i))
    }
}

/// A group's opening line without its `{`: `group id: Label [attrs]`.
pub(crate) fn group_header(doc: &Document, g: &Group) -> String {
    let word = if g.lanes {
        "lanes"
    } else if doc.is_lane(g) {
        "lane"
    } else {
        "group"
    };
    let mut line = format!("{word} {}", id_text(&g.id));
    if !g.label.is_empty() && g.label != g.id {
        line.push_str(": ");
        line.push_str(&label_text(&g.label));
    }
    let mut a = Vec::new();
    if let Some(c) = &g.color {
        a.push(if c.starts_with('#') {
            format!("color: {c}")
        } else {
            c.clone()
        });
    }
    a.extend(group_pos(g));
    line.push_str(&attrs_text(&a));
    line
}

/// A group (section, swimlanes, lane or gantt chart) with what is inside it.
pub(crate) fn group_block(
    doc: &Document,
    g: &Group,
    depth: usize,
    out: &mut String,
    in_map: &dyn Fn(&Node) -> bool,
) {
    let pad = "  ".repeat(depth);
    if g.chart.is_some() {
        chart_block(doc, g, &pad, out);
        return;
    }
    if g.gantt || g.risks {
        gantt_block(doc, g, &pad, out);
        return;
    }
    out.push_str(&format!("{pad}{} {{\n", group_header(doc, g)));
    let (nodes, groups) = doc.members(Some(&g.id));
    for n in nodes.iter().filter(|n| !in_map(n)) {
        out.push_str(&format!("{pad}  {}\n", node_line(n)));
    }
    for n in nodes.iter().filter(|n| in_map(n) && n.parent.is_none()) {
        map_block(doc, n, &format!("{pad}  "), out);
    }
    for child in groups {
        group_block(doc, child, depth + 1, out, in_map);
    }
    out.push_str(&format!("{pad}}}\n"));
}

pub fn print(doc: &Document) -> String {
    let mut out = String::new();
    let head: Vec<String> = SETTINGS
        .iter()
        .filter_map(|k| setting_line(doc, k))
        .collect();
    let mut sections: Vec<String> = Vec::new();
    if !head.is_empty() {
        sections.push(head.join("\n") + "\n");
    }

    let in_map = map_members(doc);
    let is_root = |n: &Node| in_map(n) && n.parent.is_none();
    let (top_nodes, top_groups) = doc.members(None);
    // groups whose parent is missing are printed at the top level too
    let orphans: Vec<&Group> = doc
        .groups
        .iter()
        .filter(|g| g.parent.as_ref().is_some_and(|p| doc.group(p).is_none()))
        .collect();
    let mut groups = String::new();
    for g in top_groups.into_iter().chain(orphans) {
        group_block(doc, g, 0, &mut groups, &in_map);
    }
    let mut maps = String::new();
    for n in doc
        .nodes
        .iter()
        .filter(|n| is_root(n) && n.group.as_ref().is_none_or(|g| doc.group(g).is_none()))
    {
        if !maps.is_empty() {
            maps.push('\n');
        }
        map_block(doc, n, "", &mut maps);
    }
    let mut nodes = String::new();
    for n in top_nodes.into_iter().filter(|n| !in_map(n)).chain(
        doc.nodes
            .iter()
            .filter(|n| n.group.as_ref().is_some_and(|g| doc.group(g).is_none()))
            .filter(|n| !in_map(n)),
    ) {
        // an undecorated node that an edge mentions needs no line of its own
        let bare = n.label.is_empty()
            && *n
                == Node {
                    id: n.id.clone(),
                    group: n.group.clone(),
                    ..Default::default()
                };
        if bare && n.group.is_none() && doc.edges.iter().any(|e| e.from == n.id || e.to == n.id) {
            continue;
        }
        nodes.push_str(&node_line(n));
        nodes.push('\n');
    }
    let edges: String = doc.edges.iter().map(|e| edge_line(e) + "\n").collect();
    let mut comments = String::new();
    for c in &doc.comments {
        comment_lines(c, &mut comments);
    }
    for part in [nodes, maps, groups, edges, comments] {
        if !part.is_empty() {
            sections.push(part);
        }
    }
    out.push_str(&sections.join("\n"));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = r#"
title: Checkout
direction: down

# people
user: Customer [person]
group backend: Backend [blue] {
  api: Orders API [detail: Rust · Axum, x: 10, y: 20]
  db: Postgres [cylinder]
  group jobs {
    worker
  }
}
user -> api: POST /orders
api -> db -> "Read replica"
api ..> mail, worker: async [red]
a <- b
c <-> d [thick]
comment on api by deyan: Should payments be separate?
reply c1 by "Ana B": Not before launch.
comment review at 10,20: Overall looks good [resolved, resolution: "ok"]
"#;

    #[test]
    fn parses_everything() {
        let d = parse(SRC).unwrap();
        assert_eq!(d.title.as_deref(), Some("Checkout"));
        assert_eq!(d.settings.direction, Direction::Down);
        let api = d.node("api").unwrap();
        assert_eq!(api.label, "Orders API");
        assert_eq!(api.group.as_deref(), Some("backend"));
        assert_eq!(api.detail.as_deref(), Some("Rust · Axum"));
        assert_eq!(api.x, Some(10.0));
        assert_eq!(d.node("db").unwrap().shape, Shape::Cylinder);
        assert_eq!(d.node("user").unwrap().shape, Shape::Person);
        assert_eq!(d.node("worker").unwrap().group.as_deref(), Some("jobs"));
        assert_eq!(d.group("jobs").unwrap().parent.as_deref(), Some("backend"));
        assert_eq!(d.node("read-replica").unwrap().label, "Read replica");
        assert!(d.edge("db->read-replica").is_some());
        let e = d.edge("api->worker").unwrap();
        assert_eq!(
            (e.line, e.color.as_deref(), e.label.as_deref()),
            (Line::Dashed, Some("red"), Some("async"))
        );
        assert!(d.edge("b->a").is_some());
        assert_eq!(d.edge("c->d").unwrap().arrow, Arrow::Both);
        assert_eq!(d.edge("c->d").unwrap().line, Line::Thick);
        assert_eq!(d.comments.len(), 2);
        assert_eq!(d.comments[0].id, "c1");
        assert_eq!(d.comments[0].replies[0].author.as_deref(), Some("Ana B"));
        assert_eq!(d.comments[1].status, Status::Resolved);
        assert_eq!(d.comments[1].x, Some(10.0));
    }

    #[test]
    fn round_trips() {
        let d = parse(SRC).unwrap();
        let text = print(&d);
        let mut again = parse(&text).unwrap_or_else(|e| panic!("{e:?}\n{text}"));
        let mut d = d;
        d.nodes.sort_by(|a, b| a.id.cmp(&b.id));
        let printed_again = print(&again);
        again.nodes.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(d, again, "\n{text}");
        assert_eq!(printed_again, text);
    }

    #[test]
    fn labels_keep_brackets_and_newlines() {
        let d = parse("a: Step [draft]\nb: Two\\nlines\nc: \"  padded \"").unwrap();
        assert_eq!(d.node("a").unwrap().label, "Step [draft]");
        assert_eq!(d.node("b").unwrap().label, "Two\nlines");
        assert_eq!(d.node("c").unwrap().label, "  padded ");
        assert_eq!(parse(&print(&d)).unwrap(), d);
    }

    #[test]
    fn hyphenated_ids_and_tight_operators() {
        let d = parse("web-app->api.v2\nx-->y\np..q").unwrap();
        assert!(d.edge("web-app->api.v2").is_some());
        assert!(d.edge("x->y").is_some());
        assert_eq!(d.edge("p->q").unwrap().arrow, Arrow::None);
    }

    #[test]
    fn reports_errors_with_lines() {
        let (_, errs) = parse_lenient("a [shape: blob]\n}\ngroup g {\n");
        let lines: Vec<usize> = errs.iter().map(|e| e.line).collect();
        assert_eq!(lines, vec![1, 2, 3], "{errs:?}");
    }

    const MAP: &str = r##"title: Trip
a -> food

mindmap trip: Lisbon trip [layout: both] {
  Food
    Pastéis de nata
    - Seafood
    Food [collapsed]
      Snacks
  "# not a heading"
  Days [red, id: plan]
    Belém [link: https://example.com]
}
group g {
  mindmap inner {}
}
"##;

    #[test]
    fn mindmaps_parse_and_round_trip() {
        let d = parse(MAP).unwrap_or_else(|e| panic!("{e:?}"));
        let trip = d.node("trip").unwrap();
        assert_eq!(trip.mindmap, Some(MapLayout::Both));
        // the edge's implicit `food` is adopted by the topic
        assert_eq!(d.node("food").unwrap().parent.as_deref(), Some("trip"));
        assert_eq!(d.node("seafood").unwrap().parent.as_deref(), Some("food"));
        let food2 = d.node("food-2").unwrap();
        assert!(food2.collapsed);
        assert_eq!(d.node("snacks").unwrap().parent.as_deref(), Some("food-2"));
        assert_eq!(d.node("plan").unwrap().color.as_deref(), Some("red"));
        assert_eq!(d.node("not-a-heading").unwrap().label, "# not a heading");
        assert_eq!(d.node("inner").unwrap().group.as_deref(), Some("g"));
        let kids: Vec<&str> = d.children("trip").iter().map(|n| n.id.as_str()).collect();
        assert_eq!(kids, vec!["food", "not-a-heading", "plan"]);
        let text = print(&d);
        let again = parse(&text).unwrap_or_else(|e| panic!("{e:?}\n{text}"));
        assert_eq!(print(&again), text);
        let mut a = d.clone();
        let mut b = again.clone();
        a.nodes.sort_by(|x, y| x.id.cmp(&y.id));
        b.nodes.sort_by(|x, y| x.id.cmp(&y.id));
        assert_eq!(a, b, "\n{text}");
    }

    #[test]
    fn mindmap_errors() {
        let (_, errs) = parse_lenient("mindmap m {\n  a\n");
        assert!(errs[0].message.contains("never closed"), "{errs:?}");
        let (_, errs) = parse_lenient("x\nmindmap m {\n  A [id: x]\n}");
        assert!(errs[0].message.contains("already used"), "{errs:?}");
    }

    #[test]
    fn free_text_keeps_its_commas_and_mistakes_are_errors() {
        let d = parse("a: Linear [detail: fast, opinionated, red]\nb: Risk [p: 4, i: 4, detail: Legal says 4 weeks; it blocks the pilot]\nc: Array[int]\n").unwrap();
        let a = d.node("a").unwrap();
        assert_eq!(
            (a.label.as_str(), a.detail.as_deref(), a.color.as_deref()),
            ("Linear", Some("fast, opinionated"), Some("red"))
        );
        assert_eq!(
            d.node("b").unwrap().detail.as_deref(),
            Some("Legal says 4 weeks; it blocks the pilot")
        );
        assert_eq!(d.node("c").unwrap().label, "Array[int]");
        // and it reads back the same
        assert_eq!(parse(&print(&d)).unwrap(), d);
        // what is not text and not an attribute is an error, not a label
        let e = parse("a: Linear [shape: box, opinionated]").unwrap_err();
        assert!(
            e[0].message
                .contains("unknown node attribute `opinionated`"),
            "{e:?}"
        );
        let e = parse("a: Linear [detail: fast, oops: x]").unwrap_err();
        assert!(e[0].message.contains("`oops`"), "{e:?}");
    }

    #[test]
    fn keywords_are_ids_outside_their_statements() {
        let d = parse("risks -- b\ngantt -> group\ncomment -> mindmap: x\ngroup g: G {\n  a\n}\n")
            .unwrap();
        assert_eq!(d.edges.len(), 3);
        assert!(d.node("risks").is_some() && d.node("comment").is_some() && d.group("g").is_some());
        assert_eq!(print(&parse(&print(&d)).unwrap()), print(&d));
    }
}
