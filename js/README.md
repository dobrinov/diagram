# diagram (JavaScript / TypeScript)

Build tools on top of [diagram](../README.md) boards from Node (>= 18) and the browser: parse `.diagram` files (DSL, JSON or Mermaid), render SVG with geometry, edit through ops (or straight from a model's reply), and turn boards, comments and mindmaps into prompts.

It is the same Rust core as the CLI and the editor, compiled to WebAssembly, with a small typed ES-module API on top. No dependencies.

## Build

From the repo root (needs `wasm-pack` and the `wasm32-unknown-unknown` target, like `make wasm`):

```bash
make js                    # → js/pkg/diagram_wasm.js + diagram_wasm_bg.wasm
node --test js/test/       # also run by `make test` once js/pkg exists
```

Use it from another project with a path or `file:` dependency (`npm install ../diagram/js`) or `npm pack` in `js/`. (The name `diagram` is taken on the npm registry; pick a scoped name before publishing.)

## Quick start

### Node

```js
import { readFile, writeFile } from "node:fs/promises";
import { init, Board } from "diagram";   // or "./js/index.mjs"

await init();                            // reads pkg/diagram_wasm_bg.wasm next to the module
const board = Board.parse(await readFile("checkout.diagram", "utf8"));
board.apply([{ op: "add_node", id: "cache", label: "Redis", shape: "cylinder" }]);
await writeFile("checkout.diagram", board.source());          // DSL, diffs well
await writeFile("checkout.svg", board.render({ theme: "light" }).svg);
board.free();
```

### Browser

```html
<script type="module">
  import { init, Board } from "/js/index.mjs";
  await init();                          // fetches ./pkg/diagram_wasm_bg.wasm relative to index.mjs
  const board = Board.parse("api: Orders API\napi -> db: SQL");
  document.querySelector("#board").innerHTML = board.render().svg;
</script>
```

Serve `index.mjs` and `pkg/` together. Bundlers that understand `new URL("…", import.meta.url)` (Vite, webpack 5, esbuild with a loader) pick up the `.wasm`; otherwise pass it yourself: `await init("/assets/diagram_wasm_bg.wasm")` (also accepts bytes, a `Response`, a `WebAssembly.Module` or a promise of one). `initSync(bytes)` works where top-level `await` doesn't.

Everything except `init`/`initSync` throws a clear error until `init()` has resolved. `init()` can be called any number of times.

## API

| | |
|---|---|
| `init(input?)` / `initSync(bytes)` | Load the WASM (idempotent). |
| `Board.parse(src)` | Board from DSL, JSON or Mermaid. Throws `DiagramError` with `.errors: [{line, message}]`. |
| `Board.empty()` | An empty board. |
| `board.apply(ops)` | Ops array, `{ops}`, one op, or a model's reply text (a ```` ```json ```` block or bare JSON). Atomic: returns `{created, count}`, or throws `DiagramError` with `.index` and leaves the board unchanged. |
| `board.source(format = "dsl")` | `dsl`, `json`, `mermaid`, `brief` (Markdown for LLMs) or `svg`. |
| `board.document()` | The model as an object (`Document`: nodes, edges, groups, comments, settings). |
| `board.render(options?)` | `{svg, view, nodes, groups, edges, pins}`; options `{theme, comments: "open"\|"all"\|"none", title, transparent, padding}`. |
| `board.prompt(options?)` | Prompt from the open review comments: `{target: "diagram"\|"code"\|"discuss", reply: "ops"\|"dsl", comments, includeResolved, instructions, file}`. |
| `board.brief()` | Markdown description of the board. |
| `board.selection(ids)` | The selected elements as Markdown for a prompt. |
| `board.outline(id)` / `board.markdown(id)` | A mindmap as an indented outline / as Markdown (a prompt). |
| `board.expandPrompt(id, {instructions, count, levels})` | Small prompt to grow a mindmap topic. |
| `board.diff(base?)` / `board.diffMarkdown(base?)` | Changes since `base` (any format); also makes `render()` highlight them. `null` clears the base. |
| `board.free()` / `using board = …` | Release the WASM memory. |
| `parse(src)` | Source → `Document` (throws). |
| `render(src, options?)` | One-shot render. |
| `convert(src, to)` | Source → `dsl`, `json`, `mermaid`, `brief` or `svg`. |
| `mindmapPrompt(topic, options?)` | Prompt asking for a new mindmap. |
| `replyOutline(reply)` | A model's list reply → clean two-space outline for `add_outline`. |
| `spec()` / `jsonSchema()` / `version()` | Language reference (Markdown, includes the ops), JSON Schema, core version. |

Types are in `index.d.ts`: `Document`, `Node`, `Edge`, `Group`, `Comment`, `Op` (a union discriminated by `op`), `RenderOptions`, `RenderResult`, `PromptOptions`, `ApplyResult`, `Diff`, `DiagramError`, …

## An AI loop

Any model works: the board produces the prompt, the model answers with ops, the board applies the reply. Here with the `claude` CLI (the same seam the diagram CLI uses), but `ask` can call any API.

```js
import { execFileSync } from "node:child_process";
import { readFile, writeFile } from "node:fs/promises";
import { init, Board, DiagramError, replyOutline } from "diagram";

const ask = (prompt) => execFileSync("claude", ["-p", "--model", "haiku"], { input: prompt, encoding: "utf8" });

await init();
const file = "checkout.diagram";
const board = Board.parse(await readFile(file, "utf8"));

// 1. review comments → prompt → reply with {"ops": [...]} → applied atomically
const prompt = board.prompt({ target: "diagram", instructions: "Keep ids stable." });
try {
  const { created } = board.apply(ask(prompt));
  console.log("created", created);
} catch (e) {
  if (!(e instanceof DiagramError)) throw e;
  console.error(`reply rejected at op ${e.index ?? "?"}: ${e.message}`);   // board unchanged
}

// 2. grow a mindmap topic: tiny prompt in, outline out
const outline = replyOutline(ask(board.expandPrompt("risks", { count: 4 })));
board.apply([{ op: "add_outline", parent: "risks", outline }]);

console.log(board.diffMarkdown(await readFile(file, "utf8")));   // what the model changed
await writeFile(file, board.source());
```

For a whole new map: `mindmapPrompt("Trip to Lisbon")` → `replyOutline(reply)` → `{op: "add_outline", outline}` without a `parent`.
