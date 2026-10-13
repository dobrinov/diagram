// diagram: FigJam-style boards as text, from JavaScript (Node >= 18 and
// browsers). A thin, typed layer over the WASM build in ./pkg (`make js`):
// everything crosses the WASM boundary as JSON strings; this module parses
// them, maps camelCase options and turns failures into DiagramError.
import wbgInit, { initSync as wbgInitSync, Diagram, render as wRender, convert as wConvert, spec as wSpec, json_schema, version as wVersion, mindmap_prompt, reply_outline, ask_again as wasm_ask_again } from "./pkg/diagram_wasm.js";

const WASM_URL = new URL("./pkg/diagram_wasm_bg.wasm", import.meta.url);

let ready = false;
let loading = null;

/** A parse, render or apply failure. */
export class DiagramError extends Error {
  constructor(message, { errors = [], index } = {}) {
    super(message);
    this.name = "DiagramError";
    /** Problems with their 1-based line (0 = not tied to a line). */
    this.errors = errors;
    /** For a failed apply: 0-based index of the op that failed. */
    if (index !== undefined) this.index = index;
  }
}

function fromDiagnostics(errors, what = "invalid diagram") {
  const text = errors.map((e) => (e.line > 0 ? `line ${e.line}: ${e.message}` : e.message)).join("\n");
  return new DiagramError(`${what}: ${text}`, { errors });
}

const isNode = typeof process !== "undefined" && !!process.versions?.node && WASM_URL.protocol === "file:";

/**
 * Load the WASM module. Safe to call many times; await it once before using
 * anything else. In Node the .wasm next to this module is read from disk, in
 * browsers it is fetched relative to this module. Pass `input` (bytes, a URL,
 * a Response, a WebAssembly.Module, or a promise of one) to load it from
 * somewhere else.
 */
export function init(input) {
  if (ready) return Promise.resolve();
  if (!loading) {
    loading = (async () => {
      let source = input;
      if (source === undefined) {
        if (isNode) {
          const fsName = "node:fs/promises"; // kept out of bundlers' sight
          const fs = await import(/* webpackIgnore: true */ /* @vite-ignore */ fsName);
          source = await fs.readFile(WASM_URL);
        } else {
          source = WASM_URL;
        }
      }
      await wbgInit({ module_or_path: source });
      ready = true;
    })().catch((e) => {
      loading = null; // allow a retry, e.g. with an explicit input
      throw e;
    });
  }
  return loading;
}

/** Synchronous init from bytes or a compiled WebAssembly.Module (e.g. in a worker or a CLI). */
export function initSync(bytesOrModule) {
  if (ready) return;
  wbgInitSync({ module: bytesOrModule });
  ready = true;
}

function need() {
  if (!ready) throw new Error("diagram: the WASM module is not loaded; `await init()` first");
}

const json = (v) => (v === undefined || v === null ? "" : JSON.stringify(v));

function parseResult(text) {
  const v = JSON.parse(text);
  if (v && typeof v === "object" && !Array.isArray(v) && typeof v.error === "string") {
    throw new DiagramError(v.error, { errors: v.errors ?? [{ line: 0, message: v.error }] });
  }
  return v;
}

/** A Date or `YYYY-MM-DD` as `YYYY-MM-DD` (a Date in its local day). */
function isoDate(d) {
  if (typeof d === "string") return d;
  const pad = (n) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

function renderOptions(o = {}) {
  const { theme, comments, title, transparent, padding, today, focus } = o;
  return { theme, comments, title, transparent, padding, today: today && isoDate(today), focus };
}

function promptOptions(o = {}) {
  const out = {
    target: o.target,
    reply: o.reply,
    comments: o.comments,
    include_resolved: o.includeResolved ?? o.include_resolved,
    instructions: o.instructions,
    file: o.file,
    context: o.context,
  };
  return out;
}

function expandOptions(o = {}) {
  const { instructions, count, levels } = o;
  return { instructions, count, levels };
}

function opsText(ops) {
  if (typeof ops === "string") return ops;
  if (Array.isArray(ops)) return JSON.stringify({ ops });
  if (ops && typeof ops === "object") return JSON.stringify(ops.ops ? ops : { ops: [ops] });
  throw new TypeError("apply: expected an array of ops, {ops: [...]}, or a reply string");
}

function textOrThrow(out) {
  const r = JSON.parse(out);
  if (!r.ok) throw new DiagramError(r.error);
  return r.text;
}

/** A board (one `.diagram` document) being read or edited. Wraps the WASM `Diagram`. */
export class Board {
  #d;

  /** @private use Board.parse() or Board.empty() */
  constructor(d) {
    need();
    this.#d = d ?? new Diagram();
  }

  /** An empty board. */
  static empty() {
    return new Board();
  }

  /** Parse DSL, JSON or Mermaid (detected). Throws DiagramError with `.errors` when invalid. */
  static parse(src) {
    need();
    const d = new Diagram();
    const r = JSON.parse(d.set_source(String(src)));
    if (!r.ok) {
      d.free();
      throw fromDiagnostics(r.errors);
    }
    return new Board(d);
  }

  #live() {
    if (!this.#d) throw new Error("diagram: this board was freed");
    return this.#d;
  }

  /**
   * Apply ops atomically: an array of ops, `{ops: [...]}`, one op, or a model's
   * reply text containing that JSON (a ```json block or bare braces).
   * Returns `{created, count}`. On failure throws DiagramError (`.index` is
   * the failing op when known) and the board is unchanged.
   */
  apply(ops) {
    const r = JSON.parse(this.#live().apply(opsText(ops)));
    if (!r.ok) throw new DiagramError(r.error, { index: r.index });
    return { created: r.created, count: r.count };
  }

  /** The board as `dsl` (default), `json`, `mermaid`, `brief` (Markdown for LLMs) or `svg`. */
  source(format = "dsl") {
    return textOrThrow(this.#live().source_result(format));
  }

  /** The document model (the JSON format) as an object. */
  document() {
    return JSON.parse(this.#live().document());
  }

  /** Lay out and render. Returns the SVG and the geometry of every element. */
  render(options) {
    return parseResult(this.#live().render(json(renderOptions(options))));
  }

  /** A prompt asking a model to act on the open review comments. */
  prompt(options) {
    return textOrThrow(this.#live().prompt_result(json(promptOptions(options))));
  }

  /** A Markdown description of the board, for LLM context. */
  brief({ today } = {}) {
    return today ? this.#live().brief_at(isoDate(today)) : this.#live().brief();
  }

  /** The mindmap under topic `id` as an indented outline (two spaces per level, `id` first). */
  outline(id) {
    const out = this.#live().outline(id);
    if (!out) throw new DiagramError(`no node \`${id}\``);
    return out;
  }

  /** The mindmap under topic `id` as Markdown (title, sections, bullets): a prompt. */
  markdown(id) {
    const out = this.#live().markdown(id);
    if (!out) throw new DiagramError(`no node \`${id}\``);
    return out;
  }

  /** The selected elements as Markdown to paste into a prompt; with `{today}`, gantt tasks also get their forecast. */
  selection(ids, { today } = {}) {
    return today
      ? this.#live().selection_markdown_at(JSON.stringify(ids ?? []), isoDate(today))
      : this.#live().selection_markdown(JSON.stringify(ids ?? []));
  }

  /** A prompt asking a model to change the board as `request` says, about `ids` (empty: the whole board); the reply goes to apply(). */
  askPrompt(request, ids = []) {
    return this.#live().ask_prompt(JSON.stringify(ids), String(request));
  }

  /** Every gantt chart's tasks, planned against the forecast as of `today` (a Date or YYYY-MM-DD; default: now). */
  schedule({ today } = {}) {
    return parseResult(this.#live().schedule(isoDate(today ?? new Date())));
  }

  /** Part of the board as diagram language, picked by selectors (`group:back`, `near:api`, `label~ledger`, …). */
  query(...selectors) {
    return parseResult(this.#live().query(JSON.stringify(selectors.flat()))).source;
  }

  /** The ids the selectors pick, in document order. */
  select(...selectors) {
    return parseResult(this.#live().query(JSON.stringify(selectors.flat()))).ids;
  }

  /** Add statements in the diagram language (an element written again by its id gets the attributes given). */
  addThemeFile(name, source) {
    parseResult(this.#live().add_theme_file(String(name), String(source)));
  }

  themes() {
    return JSON.parse(this.#live().themes());
  }

  merge(dsl) {
    return this.apply([{ op: "merge", dsl: String(dsl) }]);
  }

  #base(base) {
    if (base === undefined) return;
    const r = JSON.parse(this.#live().set_base(base === null ? undefined : String(base)));
    if (!r.ok) throw fromDiagnostics(r.errors, "invalid base version");
  }

  /**
   * What changed since `base` (any format). Passing `base` also makes
   * render() highlight the changes; `null` clears it; omit it to reuse the
   * last one.
   */
  diff(base) {
    this.#base(base);
    return JSON.parse(this.#live().diff());
  }

  /** Like diff(), as Markdown. */
  diffMarkdown(base) {
    this.#base(base);
    return this.#live().diff_markdown();
  }

  /** A small prompt asking a model to grow the mindmap topic `id`; reply → replyOutline() → `add_outline`. */
  expandPrompt(id, options) {
    return parseResult(this.#live().expand_prompt(id, json(expandOptions(options)))).prompt;
  }

  /** Release the WASM memory. The board can't be used afterwards. */
  free() {
    if (this.#d) {
      this.#d.free();
      this.#d = null;
    }
  }
}

// `using board = Board.parse(src)` where the runtime supports it
if (typeof Symbol.dispose === "symbol") Board.prototype[Symbol.dispose] = Board.prototype.free;

/** Parse a source (DSL, JSON or Mermaid) into a document object. Throws DiagramError when invalid. */
export function parse(src) {
  const b = Board.parse(src);
  try {
    return b.document();
  } finally {
    b.free();
  }
}

/** Render a source in one go. */
export function render(src, options) {
  need();
  return parseResult(wRender(String(src), json(renderOptions(options))));
}

/** Convert a source to `dsl`, `json`, `mermaid`, `brief` or `svg`. */
export function convert(src, to) {
  need();
  const r = JSON.parse(wConvert(String(src), to));
  if (!r.ok) throw fromDiagnostics(r.errors, "cannot convert");
  return r.text;
}

/** The language reference (Markdown): good LLM context. */
export function spec() {
  need();
  return wSpec();
}

/** JSON Schema of the document format. */
export function jsonSchema() {
  need();
  return JSON.parse(json_schema());
}

/** Version of the diagram core. */
export function version() {
  need();
  return wVersion();
}

/** A prompt asking a model for a new mindmap about `topic`; reply → replyOutline() → `add_outline`. */
/** The ask prompt again after a reply that could not be applied: the reply and the error, to correct. */
export function askAgain(prompt, reply, error) {
  need();
  return wasm_ask_again(String(prompt), String(reply), String(error));
}

export function mindmapPrompt(topic, options) {
  need();
  return mindmap_prompt(String(topic), json(expandOptions(options)));
}

/** A model's reply (bullets, numbering, code fences…) as a clean outline for `add_outline`. */
export function replyOutline(reply) {
  need();
  return reply_outline(String(reply));
}
