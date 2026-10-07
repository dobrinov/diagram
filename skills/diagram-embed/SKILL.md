---
name: diagram-embed
description: Use diagram as a library — render diagrams in a web page or docs site, embed the editor in an app, or call the Rust crate from a backend. Use when the user wants diagrams inside their own product, docs or Markdown.
---

# diagram-embed

## In a web page (no build step)

```bash
diagram embed -o public/diagram.embed.js     # one file: JS + WASM (~1 MB, gzip ~300 KB)
```

```html
<script src="/diagram.embed.js"></script>

<!-- read-only viewer with pan/zoom, comments, export -->
<div data-diagram data-mode="view" style="height:420px"><script type="text/plain">
api -> db: SQL
</script></div>

<script>
  // full editor
  Diagram.mount("#editor", { source, mode: "edit", author: "ana", onChange: (src) => save(src) });
  // or render to an SVG string
  Diagram.render("a -> b").then((svg) => (el.innerHTML = svg));
  // Markdown sites: turn ```diagram fences into pictures
  Diagram.renderAll("pre > code.language-diagram");
</script>
```

The editor instance has `getSource(fmt)`, `setSource`, `apply(ops)`, `setBase(src)` (highlights changes), `prompt(opts)`, `exportSVG()`, `exportPNG()`, `on("change"|"select", fn)` and `destroy()`.

## Standalone HTML

`diagram html FILE -o page.html` writes one self-contained file with the viewer and the diagram. You can attach it to a PR or send it in chat.

## Rust

```toml
diagram-core = { git = "https://github.com/dobrinov/diagram" }
```

```rust
let doc = diagram_core::parse(&source)?;                     // DSL, JSON or Mermaid
let svg = diagram_core::render_svg(&doc, &Default::default());
let prompt = diagram_core::prompt::prompt(&doc, &Default::default());
diagram_core::ops::apply(&mut doc, &diagram_core::ops::parse_ops(llm_reply)?)?;
```

A server should store the DSL text: it is small, diffable and readable by both people and LLMs.
