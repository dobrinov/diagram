//! diagram-core: lightweight, text-first diagrams.
//!
//! ```
//! let doc = diagram_core::parse("api -> db: SQL\ncomment on api: cache this?").unwrap();
//! let svg = diagram_core::render_svg(&doc, &Default::default());
//! let prompt = diagram_core::prompt::prompt(&doc, &Default::default());
//! assert!(svg.starts_with("<svg") && prompt.contains("cache this?"));
//! ```
//!
//! - [`model`]: the document (also the JSON format)
//! - [`dsl`]: the human-friendly text format
//! - [`mermaid`]: Mermaid flowchart import/export
//! - [`mindmap`]: mindmaps (outlines, tree layout, Markdown and AI prompts)
//! - [`ops`]: edit operations (the API for editors, tools and LLMs)
//! - [`layout`], [`route`], [`render`]: placement, edges and SVG
//! - [`diff`]: comparing two versions
//! - [`prompt`]: briefs and review prompts for LLMs
//! - [`Session`]: a stateful wrapper used by the WASM build and editors
// the JSON Schema is one large `json!` literal
#![recursion_limit = "256"]
pub mod charts;
pub mod diff;
pub mod dsl;
pub mod gantt;
pub mod layout;
pub mod mermaid;
pub mod mindmap;
pub mod model;
pub mod ops;
pub mod patch;
pub mod prompt;
pub mod query;
pub mod render;
pub mod tables;
pub mod risks;
pub mod route;
pub mod text;
pub mod theme;

use serde::Serialize;
use serde_json::{json, Value};

pub use model::{Diagnostic, Document};
pub use ops::Op;
pub use render::{RenderOptions, Rendered};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Dsl,
    Json,
    Mermaid,
}

impl Format {
    pub fn parse(s: &str) -> Option<Format> {
        match s.to_ascii_lowercase().as_str() {
            "dsl" | "diagram" | "text" => Some(Format::Dsl),
            "json" => Some(Format::Json),
            "mermaid" | "mmd" => Some(Format::Mermaid),
            _ => None,
        }
    }
}

/// Guess the format of a source text.
pub fn detect(src: &str) -> Format {
    let t = src.trim_start();
    if t.starts_with('{') {
        Format::Json
    } else if mermaid::looks_like(src) {
        Format::Mermaid
    } else {
        Format::Dsl
    }
}

/// Parse DSL, JSON or Mermaid (detected).
pub fn parse(src: &str) -> Result<Document, Vec<Diagnostic>> {
    parse_as(src, detect(src))
}

pub fn parse_as(src: &str, format: Format) -> Result<Document, Vec<Diagnostic>> {
    match format {
        Format::Dsl => dsl::parse(src),
        Format::Json => from_json(src),
        Format::Mermaid => mermaid::import(src),
    }
}

pub fn from_json(src: &str) -> Result<Document, Vec<Diagnostic>> {
    let mut doc: Document = serde_json::from_str(src).map_err(|e| {
        vec![Diagnostic::new(
            e.line(),
            format!("invalid diagram JSON: {e}"),
        )]
    })?;
    doc.normalize();
    let problems = doc.check();
    if problems.is_empty() {
        Ok(doc)
    } else {
        Err(problems)
    }
}

pub fn to_json(doc: &Document) -> String {
    serde_json::to_string_pretty(doc).unwrap()
}

/// Export as `dsl`, `json`, `mermaid`, `brief` (Markdown for LLMs) or `svg`.
pub fn export(doc: &Document, format: &str) -> Result<String, String> {
    Ok(match format {
        "dsl" | "diagram" => dsl::print(doc),
        "json" => to_json(doc) + "\n",
        "mermaid" | "mmd" => mermaid::export(doc),
        "brief" | "md" | "markdown" | "llm" => prompt::brief(doc),
        "svg" => render_svg(doc, &RenderOptions::default()),
        f => {
            return Err(format!(
                "unknown format `{f}` (dsl, json, mermaid, brief, svg)"
            ))
        }
    })
}

pub fn render_svg(doc: &Document, opts: &RenderOptions) -> String {
    render::render(doc, opts).svg
}

/// Render `new` with what changed since `old` highlighted.
pub fn render_diff(old: &Document, new: &Document, opts: &RenderOptions) -> Rendered {
    let (merged, changes) = diff::merge(old, new);
    let mut o = opts.clone();
    o.changes = changes;
    render::render(&merged, &o)
}

/// The language reference (Markdown), for people and for LLM context.
pub fn spec() -> String {
    include_str!("../LANGUAGE.md").replace("OPS_REFERENCE", prompt::OPS_REFERENCE)
}

/// JSON Schema of the document format.
pub fn json_schema() -> Value {
    let e = |v: &[&str]| json!({"type": "string", "enum": v});
    let names = |all: Vec<&'static str>| all;
    let shapes = names(model::Shape::ALL.iter().map(|s| s.as_str()).collect());
    let color = json!({"type": "string", "description": "gray, blue, green, yellow, orange, red, purple, pink, teal or #rrggbb"});
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://github.com/dobrinov/diagram/schema.json",
        "title": "diagram document",
        "type": "object",
        "required": ["nodes"],
        "additionalProperties": false,
        "properties": {
            "title": {"type": "string"},
            "description": {"type": "string"},
            "settings": {"type": "object", "additionalProperties": false, "properties": {
                "direction": e(&["right", "down", "left", "up"]),
                "layout": e(&["layered", "grid"]),
                "routing": e(&["orthogonal", "curved", "straight"]),
                "theme": e(&["light", "dark"])
            }},
            "nodes": {"type": "array", "items": {"type": "object", "required": ["id"], "additionalProperties": false, "properties": {
                "id": {"type": "string", "pattern": "^[\\p{L}\\p{N}_]([\\p{L}\\p{N}_]|[-.](?=[\\p{L}\\p{N}_]))*$"},
                "label": {"type": "string", "description": "defaults to the id; \\n breaks lines"},
                "shape": e(&shapes),
                "detail": {"type": "string", "description": "smaller second line"},
                "color": color,
                "group": {"type": "string", "description": "id of the enclosing group"},
                "link": {"type": "string", "description": "a URL, or #id to go to another element on the board"},
                "tags": {"type": "array", "items": {"type": "string"}},
                "x": {"type": "number"}, "y": {"type": "number"}, "w": {"type": "number"}, "h": {"type": "number"},
                "parent": {"type": "string", "description": "mindmap: id of the topic this one branches from"},
                "mindmap": e(&["right", "left", "both", "down"]),
                "collapsed": {"type": "boolean", "description": "mindmap: branches under this topic are folded"},
                "start": {"type": "string", "format": "date", "description": "gantt task: start date (YYYY-MM-DD)"},
                "days": {"type": "number", "exclusiveMinimum": 0, "description": "gantt task: duration in days"},
                "after": {"type": "array", "items": {"type": "string"}, "description": "gantt task: ids of the tasks it starts after (in any chart)"},
                "milestone": {"type": "boolean", "description": "gantt task: a point in time (drawn as a diamond), not a bar"},
                "done": {"type": "number", "minimum": 0, "maximum": 100, "description": "gantt task: percent done"},
                "owner": {"type": "string", "description": "gantt task or risk: who does it (owns it)"},
                "probability": {"type": "integer", "minimum": 1, "maximum": 5, "description": "risk: how likely"},
                "impact": {"type": "integer", "minimum": 1, "maximum": 5, "description": "risk: how bad"},
                "values": {"type": "array", "items": {"type": ["number", "null"]}, "description": "chart row: one value per series"},
                "cells": {"type": "array", "items": {"type": "string"}, "description": "table row: its cells after the first (the label)"}
            }}},
            "edges": {"type": "array", "items": {"type": "object", "required": ["from", "to"], "additionalProperties": false, "properties": {
                "id": {"type": "string", "description": "defaults to from->to"},
                "from": {"type": "string"}, "to": {"type": "string"},
                "label": {"type": "string"},
                "arrow": e(&["end", "start", "both", "none"]),
                "line": e(&["solid", "dashed", "dotted", "thick"]),
                "color": color
            }}},
            "groups": {"type": "array", "items": {"type": "object", "required": ["id"], "additionalProperties": false, "properties": {
                "id": {"type": "string"}, "label": {"type": "string"}, "color": color, "parent": {"type": "string"},
                "lanes": {"type": "boolean", "description": "swimlanes: the groups inside are lanes"},
                "gantt": {"type": "boolean", "description": "gantt chart: the nodes inside are tasks"},
                "start": {"type": "string", "format": "date", "description": "gantt chart: its first day (YYYY-MM-DD)"},
                "scale": e(&["day", "week", "month", "quarter"]),
                "risks": {"type": "boolean", "description": "risk matrix: the nodes inside are risks"},
                "levels": {"type": "integer", "minimum": 3, "maximum": 5, "description": "risk matrix: levels of probability and impact (5 by default)"},
                "chart": e(&["bar", "line", "area", "pie", "donut", "scatter"]),
                "stacked": {"type": "boolean", "description": "chart: series stacked"},
                "horizontal": {"type": "boolean", "description": "bar chart: bars across"},
                "unit": {"type": "string", "description": "chart: what values are in ($ € £ ¥ go in front)"},
                "series": {"type": "array", "items": {"type": "string"}, "description": "chart: the series' names"},
                "axis": {"type": "string", "description": "chart: what the rows are"},
                "table": {"type": "boolean", "description": "table: the nodes inside are rows"},
                "columns": {"type": "array", "items": {"type": "string"}, "description": "table: the columns' names"},
                "w": {"type": "number"}, "h": {"type": "number"},
                "x": {"type": "number", "description": "swimlanes / gantt chart: pinned left edge"},
                "y": {"type": "number", "description": "swimlanes / gantt chart: pinned top edge"}
            }}},
            "comments": {"type": "array", "items": {"type": "object", "required": ["text"], "additionalProperties": false, "properties": {
                "id": {"type": "string", "description": "defaults to c1, c2, …"},
                "on": {"type": "string", "description": "id of a node, edge or group; omit for a canvas comment"},
                "text": {"type": "string"},
                "author": {"type": "string"},
                "status": e(&["open", "resolved"]),
                "resolution": {"type": "string"},
                "replies": {"type": "array", "items": {"type": "object", "required": ["text"], "additionalProperties": false, "properties": {"author": {"type": "string"}, "text": {"type": "string"}}}},
                "x": {"type": "number"}, "y": {"type": "number"}
            }}}
        }
    })
}

/// A document being edited: the current version, the last parse errors and
/// an optional base version to diff against. Every method takes and returns
/// JSON-friendly values so it maps 1:1 onto the WASM API.
#[derive(Debug, Clone, Default)]
pub struct Session {
    pub doc: Document,
    pub base: Option<Document>,
    pub errors: Vec<Diagnostic>,
    /// The DSL as it was loaded, and what it said: `source("dsl")` keeps it
    /// as written and rewrites only what changed since (see [`patch`]).
    written: Option<(String, Document)>,
    /// Themes from files the board names (`theme: ./brand.theme`).
    pub themes: Vec<model::ThemeDef>,
}

impl Session {
    pub fn new() -> Session {
        Session::default()
    }

    /// Load source text (format detected). On errors in DSL input, the valid
    /// statements are still loaded so editors keep showing something useful.
    pub fn set_source(&mut self, src: &str) -> Value {
        let format = detect(src);
        let (doc, errors) = match format {
            Format::Dsl => dsl::parse_lenient(src),
            _ => match parse_as(src, format) {
                Ok(d) => (d, Vec::new()),
                Err(e) => (self.doc.clone(), e),
            },
        };
        self.written = (format == Format::Dsl).then(|| (src.to_string(), doc.clone()));
        self.doc = doc;
        self.errors = errors;
        json!({"ok": self.errors.is_empty(), "format": format, "errors": self.errors})
    }

    pub fn set_document(&mut self, doc: Document) {
        self.doc = doc;
        self.errors.clear();
        self.written = None;
    }

    pub fn source(&self, format: &str) -> Result<String, String> {
        match (&self.written, format) {
            (Some((src, old)), "dsl" | "diagram") => Ok(patch::patch(src, old, &self.doc)),
            _ => export(&self.doc, format),
        }
    }

    pub fn set_base(&mut self, src: Option<&str>) -> Value {
        match src {
            None => {
                self.base = None;
                json!({"ok": true})
            }
            Some(s) => match parse(s) {
                Ok(d) => {
                    self.base = Some(d);
                    json!({"ok": true})
                }
                Err(e) => json!({"ok": false, "errors": e}),
            },
        }
    }

    /// Apply ops (JSON or an LLM reply containing JSON).
    pub fn apply(&mut self, ops_text: &str) -> Value {
        let ops = match ops::parse_ops(ops_text) {
            Ok(o) => o,
            Err(e) => return json!({"ok": false, "error": e}),
        };
        self.apply_ops(&ops)
    }

    pub fn apply_ops(&mut self, ops: &[Op]) -> Value {
        match ops::apply(&mut self.doc, ops) {
            Ok(r) => json!({"ok": true, "created": r.created, "count": r.count}),
            Err(e) => json!({"ok": false, "error": e.to_string(), "index": e.index}),
        }
    }

    pub fn render(&self, opts: &RenderOptions) -> Rendered {
        let mut o = opts.clone();
        o.themes.extend(self.themes.iter().cloned());
        match &self.base {
            Some(b) => render_diff(b, &self.doc, &o),
            None => render::render(&self.doc, &o),
        }
    }

    /// A theme file the board names (`theme: ./brand.theme`), by that name.
    pub fn add_theme_file(&mut self, name: &str, src: &str) -> Result<(), String> {
        let t = theme::from_file(name, src)?;
        self.themes.retain(|x| x.id != t.id);
        self.themes.push(t);
        Ok(())
    }

    pub fn diff_markdown(&self) -> String {
        match &self.base {
            Some(b) => diff::markdown(&diff::diff(b, &self.doc), b, &self.doc),
            None => "No base version to compare with.\n".into(),
        }
    }

    pub fn prompt(&self, opts: &prompt::PromptOptions) -> String {
        prompt::prompt(&self.doc, opts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_convert_losslessly() {
        let src = "title: T\ngroup g: G {\n a: A [cylinder, blue]\n}\na -> b: x [dashed]\ncomment on a by me: hm\nreply c1: ok";
        let doc = parse(src).unwrap();
        let json = to_json(&doc);
        assert_eq!(detect(&json), Format::Json);
        let back = parse(&json).unwrap();
        assert_eq!(back, doc);
        let mmd = export(&doc, "mermaid").unwrap();
        assert_eq!(detect(&mmd), Format::Mermaid);
        assert!(spec().contains("add_node") && !spec().contains("OPS_REFERENCE"));
        assert!(json_schema()["properties"]["nodes"].is_object());
    }

    #[test]
    fn session_keeps_good_parts_on_errors() {
        let mut s = Session::new();
        let r = s.set_source("a -> b\nc [shape: blob]");
        assert_eq!(r["ok"], false);
        assert_eq!(s.doc.nodes.len(), 3);
        let r = s.apply(r#"{"ops":[{"op":"add_edge","from":"b","to":"c"}]}"#);
        assert_eq!(r["ok"], true, "{r}");
        s.set_base(Some("a -> b"));
        let out = s.render(&RenderOptions::default());
        assert!(
            out.svg.contains(theme::LIGHT.add),
            "added elements are green"
        );
        assert!(s.diff_markdown().contains("+ edge `b` → `c`"));
    }

    #[test]
    fn diffed_groups_are_valid_svg() {
        let old = parse("a -> b").unwrap();
        let new = parse("a -> b\ngroup g: G {\n c\n}").unwrap();
        let svg = render_diff(&old, &new, &RenderOptions::default()).svg;
        for tag in svg.split('<') {
            assert!(
                tag.matches(" stroke-width=").count() <= 1,
                "duplicate attribute in <{tag}"
            );
        }
    }
}
