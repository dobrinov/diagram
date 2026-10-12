# diagram

## Where this is going

diagram is the core of an **AI-first, all-in-one project-management tool for the agentic world**: a FigJam-like board where people draw and collaborate, discover a problem, and specify *what* should happen and *how*. Principles:

- **Simple first.** The previous attempt (Shape, in `../spec`) grew a milestone-heavy flow and was dropped for being too complex. Add features only when the owner asks; prefer fewer screens and fewer concepts.
- **Git(Hub) is the database.** Everything is text that diffs and reviews well (`.diagram` files). Collaboration happens through commits and PRs, not a server.
- **AI-first, few tokens.** Models read and write compact forms: indented outlines for mindmaps, ops JSON for edits. Never round-trip coordinates or the whole JSON model through a model when an outline or a few ops will do.
- **Any model, any harness.** AI goes through one seam: a command that reads a prompt on stdin (`--ai CMD` / `$DIAGRAM_AI`, default `claude -p --model haiku`) or the editor's `ai(prompt) → Promise<string>` option. Without one, the editor shows the prompt to copy and takes the reply back.
- **Library-first.** The owner builds small tools that are also libraries and composes them: this repo is the canvas and FigJam-style functionality; `../schema` (DB schemas, crate `schema-core`) is next to be brought onto the board as a block type. Keep `diagram-core` free of app concerns so it can be embedded.
- **Mindmaps are central**, including **prompts written as mindmaps** (centre = task, branches = Context / Constraints / Steps / Acceptance → "Copy as prompt" Markdown). See `examples/prompt-mindmap.diagram`.

A board is one `.diagram` file. Blocks (mindmaps, swimlanes and gantt charts today; schemas and more later) sit on it next to free boxes, stickies, groups, arrows and comments.

## Layout

| Path | What |
|---|---|
| `crates/core` | Model (`model.rs`, also the JSON format), DSL (`dsl.rs`), ops (`ops.rs`, the only way to change a document), layout / routing / SVG render, diff, prompts, Mermaid, `mindmap.rs` (outline parser, tree layout, Markdown, AI prompts). `LANGUAGE.md` is the language reference (`diagram spec`). |
| `crates/wasm` | wasm-bindgen API used by the editor (`Diagram` class + one-shot helpers). |
| `crates/cli` | `diagram` binary: local editor server (`server.rs`, `/api/file`, `/api/git`, `/api/ai`), exports, review commands, `ai.rs` (harness), skills. Web assets are embedded at build time. |
| `web/` | No-build editor: `editor.js` (component), `editor.css`, `app.js` (shell for the local server and the static playground), `embed.js`. `web/pkg` is generated. |
| `skills/` | Agent skills shipped in the binary (`diagram skills install`). |
| `examples/` | Example boards (also published on the site). `examples/stress/` is a v0.1 stress test with a report. |
| `scripts/e2e/` | Playwright browser tests (`make e2e`). |

Mindmap model: topics are ordinary nodes with `parent`; the centre has `mindmap: right|left|both|down`; siblings keep document order; `collapsed` folds. In layout a map is one item (its centre stands in for the whole map), so maps live in groups and next to other content.

## Commands

```bash
make wasm                 # rebuild web/pkg after changing core or wasm
make test                 # cargo tests + node tests of the WASM API
make e2e                  # browser test of the editor (installed Chrome via Playwright)
make dev                  # editor on examples/checkout.diagram, web/ served from disk
DIAGRAM_SKIP_WASM=1 cargo build   # CLI only, when web/pkg is current
DIAGRAM_WEB_DIR=web ./target/debug/diagram FILE --no-open --port N --ai CMD   # manual testing
```

## Working rules

- Test on scratch files (session scratchpad or a temp dir), not on the owner's real boards. `scripts/e2e/fake-ai.sh` is a stand-in harness; only call a real model when it matters, and say so.
- Every change goes through ops in the core; the editor, CLI, LLM replies and undo all rely on that. Keep the DSL round-trip exact (`parse(print(doc)) == doc`) and add a test when the format changes.
- Check UI changes in a real browser (screenshots via Playwright), in light and dark.
- Run `cargo fmt` only on files you change (the tree isn't fmt-clean; don't reformat unrelated files).
- Release per shipped change: bump `Cargo.toml` version, add a `CHANGELOG.md` entry, commit, tag `vX.Y.Z`. Don't push unless asked.
- On design questions: explain how it works today, propose two options (high level), build after the owner picks.
