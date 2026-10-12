<img src="web/favicon.svg" width="64" alt="diagram logo">

# diagram

A lightweight FigJam for engineers. You can write a diagram in a small, readable language or draw it in a browser editor. Colleagues comment on boxes and arrows, and one click turns those comments into a prompt that Claude Code, Codex or any other LLM can act on. The reply is applied to the diagram, and the comments are resolved.

It is written in Rust and compiled to WebAssembly, so a single engine drives the CLI, the browser editor, the embeddable JS library and the Rust crate.

**[Try it in your browser](https://dobrinov.github.io/diagram/app/)** (no install needed) · [Website](https://dobrinov.github.io/diagram/) · [Examples](https://dobrinov.github.io/diagram/examples/) · [Language reference](crates/core/LANGUAGE.md)

![The diagram editor: code pane, canvas with groups, shapes and comment pins, inspector and comment threads](assets/screenshot.png)

```diagram
title: Checkout
user: Customer [person]
group backend: Backend [blue] {
  api: Orders API [detail: Rust · Axum]
  db: Postgres [cylinder]
}
user -> api: POST /orders
api -> db
api ..> mail: receipt

comment on api by deyan: Should payments be a separate service?
reply c1 by ana: Not before launch.
```

```bash
diagram docs/checkout.diagram                      # open the editor; it saves to the file as you work
diagram prompt docs/checkout.diagram --target code # open comments → a task for your coding agent
diagram apply docs/checkout.diagram reply.md       # apply the LLM's reply (ops JSON or a whole diagram)
diagram png docs/checkout.diagram --base main      # PNG of what this branch changed
```

## Mindmaps

Press **T** and click, type the central topic, then keep typing: **Tab** adds a branch, **Enter** a sibling, **⇧Tab** outdents (on the centre it adds a new centre above it), arrows walk the map, **⌥↑/↓** reorders, **.** folds. Drag a topic onto another to move its branch. Paste a Markdown list to get a map. Maps sit on the same board as boxes, stickies and arrows, and topics can be linked and commented on like anything else.

```diagram
mindmap offsite: Team offsite [layout: both] {
  Venue
    Who books it?
  Budget [red]
    Travel and hotels
  Prompt: build checkout
    Context
    Constraints
}
```

**✦ AI that costs few tokens.** *Grow this topic* sends a model only the map as an outline plus the path to the topic, and gets indented lines back, about 5–15 tokens per topic. Any harness works: the editor takes an `ai(prompt) → Promise<string>` function, and `diagram FILE --ai "claude -p --model haiku"` (or `codex exec -`, `llm`, `ollama run …`) wires the local editor and `diagram expand` to a CLI. Without one, the editor shows the prompt to copy and takes the reply back.

**Mindmaps as prompts.** *Copy as prompt* (or `diagram outline FILE --md`) turns a map into Markdown: the centre is the title and topics are nested bullets; when every main branch has something under it, main branches become `##` sections. It works on any selection too (right-click or `⌘⇧C`): sections become headings with their blocks as bullets, followed by the arrows between them and their open comments. Swimlanes read as numbered steps in flow order (decisions say which step each answer leads to), gantt tasks with their start and end dates. A `#id` link pulls the block it points at into the prompt, so a prompt map can reference a flow or a plan on the same board.

## Why

Whiteboard tools are great for talking. Their output, though, ends up in a format that neither git nor an LLM can work with. diagram keeps the friendly parts (drag boxes, drop sticky notes, comment on anything) and stores the result as **text you can diff, review and feed to an agent**.

- **Two languages, one model.**
  - The **DSL** is for people: one statement per line, so it reads well in a PR.
  - The **JSON model** (with a JSON Schema) and **ops** are for programs and LLMs: strict, validated edits that are applied all at once or not at all.
  - **Mermaid** can be imported and exported. Everything converts to everything else without loss (`diagram convert`).
- **A real editor.**
  - Double-click to add a box.
  - Drag a node's dot onto another node to connect them, or onto empty space to create a new connected node.
  - Sticky notes, titled sections (`⌘G`; drag blocks in or out), multi-select, duplicate, arrow-key nudging, an inspector, undo and redo.
  - A live code pane (DSL, JSON or Mermaid) that stays in sync with the canvas both ways.
  - Light and dark themes, plus pan and zoom.
- **Flowcharts, swimlanes and gantt charts.**
  - Boxes, decisions and 11 more shapes, with labelled, dashed or reversed arrows (right-click a block or an arrow).
  - Swimlanes (`lanes { lane … }`): bands the flow runs across, lined up step by step.
  - Gantt charts (`gantt { Design [5d]  Build [2w, after: design] }`): a timeline by day, week, month or quarter. Drag a bar to reschedule it, drag its end to change its duration, draw an arrow between bars to make one wait for the other. Milestones, progress and owners; a forecast from today shows late work and everything it pushes back (`diagram schedule FILE` lists it).
  - Charts (`chart [bar] { quarter | EU | US  Q1 | 120 | 80 }`): bars (stacked, horizontal), lines, areas, pie, donut and scatter from a small table that diffs like text. Hover a bar for its values; *Edit data…* in the editor.
  - Risk matrices (`risks { Contract slips [p: 4, i: 4, detail: "…"] }`): probability × impact on a heat-map grid, each risk a numbered dot with its description on hover. Drag a dot to another cell to re-rate it.
- **Links between elements.** Right-click a block → *Link to…* and click another element; the block's link badge takes you there (`link: #id`). Web links work the same way.
- **Comments become prompts.**
  - Press **C** and click a block (or empty canvas) to start a thread, right where you clicked. Threads support replies, resolving and reopening.
  - **Prompt** builds a task from the open threads. You choose what the agent should do: *update the diagram*, *change the code* or *discuss*. You also choose the reply format: ops or a whole diagram.
  - **Apply reply** takes the LLM's answer as it is (fenced JSON or a ```` ```diagram ```` block), applies it atomically and resolves the comments with a note on what was done.
- **Export anywhere.**
  - Images: SVG and PNG. PNG is rasterised by resvg in the CLI and by a canvas in the browser.
  - Documents: a self-contained interactive HTML page.
  - For pasting: Mermaid (for GitHub Markdown) and a Markdown *brief* that gives an LLM the diagram as context (the source with every id, plus gantt dates and swimlane steps in order).
  - A **share link** that carries the whole diagram, compressed, in the URL.
- **Git-aware.** `--base main` (or any ref or file) highlights what changed: green for added, red for removed, amber for changed. The editor's *Compare ▾* button does the same against any git version.
- **Usable as a library.** Use the Rust crate `diagram-core` on servers. In the browser, use one `diagram.embed.js` file (JS with the WASM inlined) to render diagrams or mount the full editor. Agent skills teach Claude Code to draw, edit and review diagrams.

## Install

Requirements: Rust with the `wasm32-unknown-unknown` target, and `wasm-pack`.

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
git clone https://github.com/dobrinov/diagram && cd diagram
make install        # builds web/pkg (WASM), then `cargo install --path crates/cli`
```

A plain `cargo install --path crates/cli` also works. Its `build.rs` runs wasm-pack whenever the WASM bundle is missing or out of date.

## Usage

```
diagram [FILE]                     open FILE in the editor (created if missing;
                                   no FILE starts a new untitled.diagram)
diagram new FILE [--template flow|architecture|board]

diagram svg  FILE [-o out.svg]     static SVG       ┐ --dark --transparent --title
diagram png  FILE [-o out.png]     PNG (--scale 2)  │ --comments open|all|none
diagram html FILE [-o out.html]    interactive page ┘ --base FILE|GIT_REF  (highlight changes)
diagram convert FILE --to dsl|json|mermaid|brief
diagram brief FILE                 Markdown description for LLM context

diagram comments FILE [--all] [--json]
diagram comment  FILE [--on ID] [--by NAME] TEXT
diagram reply    FILE COMMENT_ID TEXT
diagram resolve  FILE COMMENT_ID [--note TEXT]
diagram prompt   FILE [--target diagram|code|discuss] [--reply ops|dsl] [--comment c1,c2] [--all] [--context auto|near|section|all]
diagram apply    FILE [PATCH|-] [--dry-run]

diagram outline FILE [ID] [--md]   diagram expand FILE ID [--ask TEXT] | --new TOPIC   (--ai CMD)
diagram check FILE...   diagram fmt FILE... [--check]   diagram diff OLD NEW|GIT_REF
diagram spec [--json-schema]   diagram embed [-o diagram.embed.js]   diagram skills install [--global]
```

`diagram FILE` starts a local server (from port 5591) and opens the editor. Every change is saved to the file. If something else edits the file, such as your `$EDITOR` or an agent, the editor reloads it within a second or two. Press `⌘S` to save right away.

### Editor shortcuts

| | |
|---|---|
| `V` `T` `N` `S` `C` `M` | select · mindmap · box · sticky · connect · comment |
| `Tab` · `Enter` · `⇧Tab` · `F2` · `.` | mindmap: branch · sibling · outdent · rename · fold |
| double-click | add a box / edit a label |
| drag a node's dot | connect, or drop on empty space to create a connected node |
| `⌘G` · `⌘D` · `⌘A` | section · duplicate · select all |
| `Enter` · `Delete` · arrows | edit label · delete · nudge (`⇧` for ×5) |
| `⌘Z` · `⇧⌘Z` | undo · redo |
| `F` · `0` · `+` `-` · space+drag | fit · 100% · zoom · pan |
| `⌘/` · `?` | toggle code · show shortcuts |

## The language

The full reference is in [LANGUAGE.md](crates/core/LANGUAGE.md), and `diagram spec` prints it. The short version:

| | |
|---|---|
| `id: Label [attrs]` | node. Attrs: `shape`, `color`, `detail`, `link`, `tags`, `x`/`y` (pinned) |
| `a -> b: label` | edge. `->` solid, `..>` dashed, `=>` thick, `--` / `..` / `==` no arrow, `<->` both |
| `a -> b -> c`, `a -> b, c` | chains and fan-out |
| `group id: Label [color] { … }` | groups, which can be nested |
| `comment [id] on TARGET by WHO: text` · `reply id: text` | review threads; add `[resolved, resolution: …]` to close one |
| `title:` `direction:` `layout:` `routing:` `theme:` | settings |

The 13 shapes are box, rounded, pill, circle, diamond, hexagon, cylinder, parallelogram, document, person, cloud, sticky and text. There are 9 named colours, and any `#hex` value also works.

For LLMs there is an **ops** language for precise edits:

```json
{"ops": [
  {"op": "add_node", "id": "cache", "label": "Redis", "shape": "cylinder", "group": "backend"},
  {"op": "add_edge", "from": "api", "to": "cache", "label": "reads"},
  {"op": "rename", "id": "api", "to": "orders-api"},
  {"op": "resolve_comment", "id": "c1", "resolution": "Added a read-through cache"}
]}
```

## The review loop

1. **Draw:** `diagram docs/checkout.diagram`, or ask Claude Code to draw it (see *Agent skills*).
2. **Review:** colleagues open the same file (or a share link, or an HTML export) and leave comments.
3. **Prompt:** use the **Prompt** button or `diagram prompt FILE --target code`. The prompt contains the diagram, each open thread with its replies, and exact instructions for replying. On a big board it shows only the sections the comments are about (`--context near|section|all` to choose), so one comment doesn't cost the whole board in tokens.
4. **Apply:** paste the reply into **Apply reply** or run `diagram apply FILE reply.md`. The changes are applied atomically, and each comment is resolved with a note on what was done. Only the statements that changed are rewritten: the rest of the file, its blank lines and `#` notes stay as you wrote them, so the commit diff shows just the change. Comments on deleted elements are kept as canvas comments, so feedback is never lost silently.

## Using it as a library

### Browser (any app, no build step)

```bash
diagram embed -o public/diagram.embed.js
```

```html
<script src="/diagram.embed.js"></script>
<div id="board" style="height:600px"></div>
<script>
  const editor = await Diagram.mount("#board", {
    source,                      // DSL, JSON or Mermaid
    mode: "edit",                // or "view"
    author: currentUser.name,
    onChange: (src) => save(src),
  });
  editor.apply({ ops: [...] });                 // programmatic edits
  editor.setBase(previousVersion);              // highlight changes
  const prompt = editor.prompt({ target: "code" });
  const svg = await Diagram.render("a -> b", { theme: "dark" });
  Diagram.renderAll("pre > code.language-diagram");   // render ```diagram fences in Markdown
</script>
```

`<div data-diagram data-mode="view">…source…</div>` mounts itself. If you serve `web/` and `web/pkg` yourself, load `pkg/diagram_wasm.js`, `editor.js` and `editor.css`, then call `DiagramEditor.create(el, { wasm: wasm_bindgen, … })`.

### JavaScript / TypeScript (Node and browsers)

`js/` is a typed ES-module package around the WASM build, for tools built on top of diagram: parse, render, apply ops or a model's reply, prompts, mindmap outlines, diffs. Build it with `make js`; see [js/README.md](js/README.md).

```js
import { init, Board } from "./js/index.mjs";
await init();
const board = Board.parse("api -> db");
board.apply(modelReply);                       // ```json {"ops": [...]}``` → atomic edit, throws DiagramError
const { svg } = board.render({ theme: "dark" });
```

### Rust

```toml
[dependencies]
diagram-core = { git = "https://github.com/dobrinov/diagram" }
```

```rust
let mut doc = diagram_core::parse(&source)?;                  // DSL, JSON or Mermaid
let svg = diagram_core::render_svg(&doc, &Default::default());
let ops = diagram_core::ops::parse_ops(&llm_reply)?;          // tolerant of chat formatting
diagram_core::ops::apply(&mut doc, &ops)?;                    // atomic
let prompt = diagram_core::prompt::prompt(&doc, &Default::default());
let text = diagram_core::dsl::print(&doc);                    // store this
```

### In SpecDeck

SpecDeck is already a Rust server with a no-build JS front end and a WASM canvas, so diagram fits in without adding any new tooling:

- **Server:** add `diagram-core` as a git dependency of `crates/server`. Store diagrams as DSL text in a `diagrams` table, or as `.diagram` files in the project repo. To include review feedback in the prompt composer, call `diagram_core::prompt::prompt(&doc, &PromptOptions { target: Target::Code, file: Some(path), .. })` alongside Spec Kit's own review comments.
- **Web:** copy `diagram.embed.js` into `web/` (or `make embed`) and mount an editor on a board section: `Diagram.mount(el, { source, author, onChange: (src) => api.put(…) })`. Editor `change` events map naturally onto SpecDeck's SSE refresh.
- **Agents:** run `diagram skills install` in the project repo, so the harnesses SpecDeck launches can read, edit and resolve diagram comments with the CLI.

## Agent skills

```bash
diagram skills install            # into .claude/skills of the current repo
diagram skills install --global   # into ~/.claude/skills
```

| Skill | What it does |
|---|---|
| `diagram-draw` | Draw an architecture, flow or board diagram (from a description or by reading the codebase), validate it and render it |
| `diagram-review` | Act on open comments: change the code or the diagram, then resolve or reply to each thread |
| `diagram-edit` | Make precise structural edits with ops (renames, regrouping, removals) |
| `diagram-embed` | Use the JS bundle or the Rust crate in another project |
| `diagram-mindmap` | Create and grow mindmaps with outlines and `add_outline`, and turn them into prompts |

## Repository layout

| Path | What it is |
|---|---|
| `crates/core` | Model, DSL parser and printer, JSON (+ schema), Mermaid, ops, layout (layered Sugiyama with compound groups, and grid), edge routing, SVG renderer, diff, prompts |
| `crates/wasm` | `wasm-bindgen` bindings (`Diagram` class and one-shot helpers) |
| `crates/cli` | The `diagram` binary: local editor server, exports (PNG via resvg), review commands, embed bundle, skills |
| `web/` | No-build editor component (`editor.js`, `editor.css`), app shell (`app.js`) and embed API (`embed.js`). `web/pkg` is generated. |
| `js/` | Typed JS/TS package for Node and browsers (`make js`; `js/pkg` is generated) |
| `skills/` | Agent skills shipped inside the binary |
| `scripts/e2e/` | Browser tests of the editor (Playwright, `make e2e`) |
| `site/`, `scripts/build-site.sh` | GitHub Pages: landing page, playground and examples (`make site` → `docs/`) |

```bash
make test     # cargo tests + node tests of the WASM API (and of js/ once `make js` has run)
make js       # build the js/ package's WASM into js/pkg
make e2e      # browser test of the editor (uses the installed Chrome)
make dev      # serve web/ from disk while hacking on the editor
make site     # build the static site into docs/
```

## Limitations

- Edges are routed with spread ports, separate lanes and rounded elbows, but they don't avoid obstacles. In a dense diagram, a line can cross a box. Pinning nodes, changing `direction` or using `routing: curved` usually helps.
- `#` comment lines in a `.diagram` file are dropped when a tool rewrites the file (canvas edits, `apply`, `fmt`). Use `comment` statements for anything that must survive.
- Collaboration happens through files, git, share links and HTML exports. There is no real-time multi-user server (yet).

## License

MIT
