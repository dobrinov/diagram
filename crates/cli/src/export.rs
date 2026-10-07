//! Standalone outputs: the embeddable JS bundle and self-contained HTML.
use diagram_core::{Document, RenderOptions};
use serde_json::json;

use crate::assets::{self, base64, text};

/// A single classic <script> that defines `window.Diagram` (and
/// `window.DiagramEditor`) with the WASM module inlined as base64.
pub fn embed_bundle() -> String {
    let mut s = String::with_capacity(assets::WASM.len() * 4 / 3 + 200_000);
    s.push_str(&format!(
        "/*! diagram {} embed bundle — https://github.com/dobrinov/diagram — MIT */\n",
        env!("CARGO_PKG_VERSION")
    ));
    s.push_str("(function(){\n\"use strict\";\n");
    s.push_str(text(assets::GLUE_JS));
    s.push_str("\nvar DIAGRAM_WASM_B64 = \"");
    s.push_str(&base64(assets::WASM));
    s.push_str("\";\nvar DIAGRAM_CSS = ");
    s.push_str(&ascii_js(
        &serde_json::to_string(text(assets::EDITOR_CSS)).unwrap(),
    ));
    s.push_str(";\nvar DIAGRAM_WASM_INIT = function(bytes){ return wasm_bindgen({ module_or_path: bytes }).then(function(){ return wasm_bindgen; }); };\n");
    s.push_str(&ascii_js(text(assets::EDITOR_JS)));
    s.push('\n');
    s.push_str(&ascii_js(text(assets::EMBED_JS)));
    s.push_str("\n})();\n");
    ascii_js(&s)
}

/// Escape non-ASCII characters (`\uXXXX`) so the bundle works on pages that
/// do not declare a charset. Only used on code whose non-ASCII characters sit
/// in string literals and comments.
fn ascii_js(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    for c in src.chars() {
        if c.is_ascii() {
            out.push(c);
        } else {
            let mut buf = [0u16; 2];
            for unit in c.encode_utf16(&mut buf) {
                out.push_str(&format!("\\u{unit:04x}"));
            }
        }
    }
    out
}

/// JSON that is safe to place inside a <script> element.
fn script_json(v: &serde_json::Value) -> String {
    serde_json::to_string(v)
        .unwrap()
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026")
}

pub fn html(doc: &Document, base: Option<&Document>, opts: &RenderOptions) -> String {
    let rendered = match base {
        Some(b) => diagram_core::render_diff(b, doc, opts),
        None => diagram_core::render::render(doc, opts),
    };
    let title = doc.title.clone().unwrap_or_else(|| "Diagram".into());
    let data = json!({
        "source": diagram_core::dsl::print(doc),
        "base": base.map(diagram_core::dsl::print),
        "mode": "view",
        "theme": opts.theme.map(|t| t.as_str()),
    });
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="generator" content="diagram {version}">
<link rel="icon" type="image/svg+xml" href="data:image/svg+xml;base64,{favicon}">
<title>{title}</title>
<style>html,body{{margin:0;height:100%;}}body{{font:14px/1.4 -apple-system,BlinkMacSystemFont,"Segoe UI",Helvetica,Arial,sans-serif;}}#diagram>svg{{max-width:100%;height:auto;display:block;margin:auto}}</style>
</head>
<body>
<div id="diagram" style="height:100vh">{svg}</div>
<script type="application/json" id="diagram-data">{data}</script>
<script>{bundle}</script>
<script>Diagram.mount(document.getElementById("diagram"), JSON.parse(document.getElementById("diagram-data").textContent));</script>
</body>
</html>
"#,
        version = env!("CARGO_PKG_VERSION"),
        favicon = base64(assets::FAVICON),
        title = diagram_core::text::esc(&title),
        svg = rendered.svg,
        data = script_json(&data),
        bundle = embed_bundle().replace("</script", "<\\/script"),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn bundle_is_ascii() {
        assert_eq!(super::ascii_js("a → b"), "a \\u2192 b");
        assert!(super::embed_bundle().is_ascii());
    }
}
