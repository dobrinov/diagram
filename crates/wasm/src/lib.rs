//! WebAssembly bindings. Everything crosses the boundary as strings (JSON
//! for structured values) so the JS glue stays tiny and dependency free.
use diagram_core::prompt::PromptOptions;
use diagram_core::{RenderOptions, Session};
use serde_json::{json, Value};
use wasm_bindgen::prelude::*;

fn to_json<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_else(|e| json!({ "error": e.to_string() }).to_string())
}

fn opts<T: serde::de::DeserializeOwned + Default>(s: &str) -> Result<T, String> {
    if s.trim().is_empty() {
        Ok(T::default())
    } else {
        serde_json::from_str(s).map_err(|e| format!("invalid options: {e}"))
    }
}

fn err(e: impl std::fmt::Display) -> String {
    json!({ "error": e.to_string() }).to_string()
}

/// A document being edited.
#[wasm_bindgen]
pub struct Diagram {
    inner: Session,
}

#[wasm_bindgen]
impl Diagram {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Diagram {
        Diagram {
            inner: Session::new(),
        }
    }

    /// Load DSL, JSON or Mermaid. Returns `{ok, format, errors: [{line, message}]}`.
    pub fn set_source(&mut self, src: &str) -> String {
        to_json(&self.inner.set_source(src))
    }

    /// The document as `dsl`, `json`, `mermaid`, `brief` or `svg`.
    pub fn source(&self, format: &str) -> String {
        self.inner
            .source(format)
            .unwrap_or_else(|e| format!("error: {e}"))
    }

    /// The document model as JSON.
    pub fn document(&self) -> String {
        to_json(&self.inner.doc)
    }

    pub fn errors(&self) -> String {
        to_json(&self.inner.errors)
    }

    /// Apply `{"ops": [...]}` (or an LLM reply containing it).
    /// Returns `{ok, created, count}` or `{ok: false, error, index}`.
    pub fn apply(&mut self, ops: &str) -> String {
        to_json(&self.inner.apply(ops))
    }

    /// Render with JSON options `{theme, comments: "open"|"all"|"none", title, transparent, padding}`.
    /// Returns `{svg, view, nodes, groups, edges, pins}`.
    pub fn render(&self, options: &str) -> String {
        match opts::<RenderOptions>(options) {
            Ok(o) => to_json(&self.inner.render(&o)),
            Err(e) => err(e),
        }
    }

    /// Diff against a base version (any format); `undefined` clears it.
    pub fn set_base(&mut self, src: Option<String>) -> String {
        to_json(&self.inner.set_base(src.as_deref()))
    }

    pub fn diff(&self) -> String {
        match &self.inner.base {
            Some(b) => to_json(&diagram_core::diff::diff(b, &self.inner.doc)),
            None => to_json(&json!({ "entries": [] })),
        }
    }

    pub fn diff_markdown(&self) -> String {
        self.inner.diff_markdown()
    }

    /// A prompt from the review comments. Options:
    /// `{target: "diagram"|"code"|"discuss", reply: "ops"|"dsl", comments: [ids], include_resolved, instructions, file}`.
    pub fn prompt(&self, options: &str) -> String {
        match opts::<PromptOptions>(options) {
            Ok(o) => self.inner.prompt(&o),
            Err(e) => format!("error: {e}"),
        }
    }

    pub fn brief(&self) -> String {
        diagram_core::prompt::brief(&self.inner.doc)
    }

    /// The mindmap under `id` as an indented outline (with `id` as the first line).
    pub fn outline(&self, id: &str) -> String {
        diagram_core::mindmap::outline(&self.inner.doc, id, true)
    }

    /// The mindmap under `id` as Markdown (title, sections, bullets): a
    /// prompt, followed by what its `#id` links point at.
    pub fn markdown(&self, id: &str) -> String {
        if self.inner.doc.node(id).is_none() {
            return String::new();
        }
        diagram_core::prompt::selection(&self.inner.doc, &[id.to_string()])
    }

    /// The selected items (a JSON array of ids) as Markdown to paste into a
    /// prompt: sections with their blocks, arrows between them, open comments.
    pub fn selection_markdown(&self, ids: &str) -> String {
        let ids: Vec<String> = serde_json::from_str(ids).unwrap_or_default();
        diagram_core::prompt::selection(&self.inner.doc, &ids)
    }

    /// The selected items as Markdown, with gantt forecasts as of `today`
    /// (`YYYY-MM-DD`; empty: none).
    pub fn selection_markdown_at(&self, ids: &str, today: &str) -> String {
        let ids: Vec<String> = serde_json::from_str(ids).unwrap_or_default();
        let t = diagram_core::gantt::parse_date(today);
        diagram_core::prompt::selection_at(&self.inner.doc, &ids, t)
    }

    /// The brief, with gantt forecasts as of `today` (`YYYY-MM-DD`).
    pub fn brief_at(&self, today: &str) -> String {
        diagram_core::prompt::brief_at(&self.inner.doc, diagram_core::gantt::parse_date(today))
    }

    /// Every gantt chart's tasks, planned against the forecast as of `today`
    /// (`YYYY-MM-DD`): `{today, charts: [{id, label, tasks: [{id, start, end,
    /// days, done, milestone, forecast_start, forecast_end, slip, status}]}]}`.
    pub fn schedule(&self, today: &str) -> String {
        let Some(t) = diagram_core::gantt::parse_date(today) else {
            return err(format!("`{today}` is not a date (YYYY-MM-DD)"));
        };
        let doc = &self.inner.doc;
        let charts: Vec<Value> = doc
            .groups
            .iter()
            .filter(|g| g.gantt)
            .map(|g| json!({"id": g.id, "label": g.display(), "tasks": diagram_core::gantt::track(doc, &g.id, t)}))
            .collect();
        json!({"today": diagram_core::gantt::format_date(t), "charts": charts}).to_string()
    }

    /// Part of the board as diagram language: `selectors` is a JSON array
    /// (`["group:back", "near:api"]`, see `diagram help`). Returns
    /// `{source, ids}` or `{error}`.
    pub fn query(&self, selectors: &str) -> String {
        let sels: Vec<String> = match serde_json::from_str(selectors) {
            Ok(s) => s,
            Err(e) => return err(format!("selectors: {e}")),
        };
        let sels: Vec<&str> = sels.iter().map(|s| s.as_str()).collect();
        match diagram_core::query::select(&self.inner.doc, &sels) {
            Ok(ids) => json!({
                "source": diagram_core::dsl::print(&diagram_core::query::subset(&self.inner.doc, &ids)),
                "ids": ids,
            })
            .to_string(),
            Err(e) => err(e),
        }
    }

    /// A small prompt asking a model to grow the topic `id`.
    /// Options: `{instructions, count, levels}`. Returns `{prompt}` or `{error}`.
    pub fn expand_prompt(&self, id: &str, options: &str) -> String {
        let o: diagram_core::mindmap::ExpandOptions = match opts(options) {
            Ok(o) => o,
            Err(e) => return err(e),
        };
        match diagram_core::mindmap::expand_prompt(&self.inner.doc, id, &o) {
            Ok(p) => json!({ "prompt": p }).to_string(),
            Err(e) => err(e),
        }
    }
}

/// A prompt asking a model for a new mindmap about `topic`.
#[wasm_bindgen]
pub fn mindmap_prompt(topic: &str, options: &str) -> String {
    let o: diagram_core::mindmap::ExpandOptions = opts(options).unwrap_or_default();
    diagram_core::mindmap::generate_prompt(topic, &o)
}

/// A model's reply as an outline (two spaces per level), ready for `add_outline`.
#[wasm_bindgen]
pub fn reply_outline(reply: &str) -> String {
    diagram_core::mindmap::parse_reply(reply)
        .iter()
        .map(|l| format!("{}{}\n", "  ".repeat(l.depth), l.text))
        .collect()
}

impl Default for Diagram {
    fn default() -> Self {
        Self::new()
    }
}

/// One-shot: source → `{svg, view, …}` or `{error}`.
#[wasm_bindgen]
pub fn render(src: &str, options: &str) -> String {
    let o: RenderOptions = match opts(options) {
        Ok(o) => o,
        Err(e) => return err(e),
    };
    match diagram_core::parse(src) {
        Ok(doc) => to_json(&diagram_core::render::render(&doc, &o)),
        Err(e) => to_json(
            &json!({ "error": e.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("\n"), "errors": e }),
        ),
    }
}

/// One-shot conversion: `{ok, text}` or `{ok: false, errors}`.
#[wasm_bindgen]
pub fn convert(src: &str, to: &str) -> String {
    let v: Value = match diagram_core::parse(src) {
        Ok(doc) => match diagram_core::export(&doc, to) {
            Ok(text) => json!({ "ok": true, "text": text }),
            Err(e) => json!({ "ok": false, "errors": [{ "line": 0, "message": e }] }),
        },
        Err(e) => json!({ "ok": false, "errors": e }),
    };
    v.to_string()
}

#[wasm_bindgen]
pub fn spec() -> String {
    diagram_core::spec()
}

#[wasm_bindgen]
pub fn json_schema() -> String {
    diagram_core::json_schema().to_string()
}

#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
