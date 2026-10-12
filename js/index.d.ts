// Types for the `diagram` package. The document types mirror
// crates/core/src/model.rs (the JSON format); ops mirror crates/core/src/ops.rs.

// ------------------------------------------------------------------ document

export type Direction = "right" | "down" | "left" | "up";
export type LayoutKind = "layered" | "grid";
export type Routing = "orthogonal" | "curved" | "straight";
export type Theme = "light" | "dark";
export type Shape =
  | "box" | "rounded" | "pill" | "circle" | "diamond" | "hexagon" | "cylinder"
  | "parallelogram" | "document" | "person" | "cloud" | "sticky" | "text";
/** Which way a mindmap grows from its central topic. */
export type MapLayout = "right" | "left" | "both" | "down";
export type Arrow = "end" | "start" | "both" | "none";
export type Line = "solid" | "dashed" | "dotted" | "thick";
export type Status = "open" | "resolved";
/** What one step of a gantt chart's timeline is. */
export type Scale = "day" | "week" | "month" | "quarter";
/** A named colour, or `#rgb` / `#rrggbb` / `#rrggbbaa`. */
export type Color =
  | "gray" | "blue" | "green" | "yellow" | "orange" | "red" | "purple" | "pink" | "teal"
  | `#${string}`;

/** A board: the JSON format. Fields at their default are omitted. */
export interface Document {
  title?: string;
  description?: string;
  settings?: Settings;
  nodes: Node[];
  edges?: Edge[];
  groups?: Group[];
  comments?: Comment[];
}

export interface Settings {
  /** Which way the layered layout flows (default `right`). */
  direction?: Direction;
  layout?: LayoutKind;
  routing?: Routing;
  theme?: Theme;
}

/** A box, sticky, mindmap topic or gantt task. */
export interface Node {
  id: string;
  /** Shown text; empty/absent means "use the id". `\n` breaks lines. */
  label?: string;
  /** Default `box`. */
  shape?: Shape;
  /** Secondary, smaller text under the label. */
  detail?: string;
  color?: Color;
  /** Id of the enclosing group. */
  group?: string;
  /** A URL, or `#id` to go to another element on the board. */
  link?: string;
  tags?: string[];
  /** Pinned position (top-left); unpinned nodes are placed by the layout. */
  x?: number;
  y?: number;
  w?: number;
  h?: number;
  /** Mindmap: the topic this one branches from. Siblings keep document order. */
  parent?: string;
  /** Set on a mindmap's central topic: which way the map grows. */
  mindmap?: MapLayout;
  /** Mindmap: the branches under this topic are folded away. */
  collapsed?: boolean;
  /** Gantt task: start date (`YYYY-MM-DD`). */
  start?: string;
  /** Gantt task: duration in days. */
  days?: number;
  /** Gantt task: ids of the tasks it starts after (in any chart). */
  after?: string[];
  /** Gantt task: a point in time (drawn as a diamond), not a bar. */
  milestone?: boolean;
  /** Gantt task: percent done, 0–100. */
  done?: number;
  /** Gantt task: who does it. */
  owner?: string;
  /** Risk: how likely, 1–5. */
  probability?: number;
  /** Risk: how bad, 1–5. */
  impact?: number;
}

export interface Edge {
  /** Defaults to `from->to` (`from->to#2` for the second one). */
  id: string;
  /** A node or group id. */
  from: string;
  to: string;
  label?: string;
  /** Default `end`. */
  arrow?: Arrow;
  /** Default `solid`. */
  line?: Line;
  color?: Color;
}

/** A section; also swimlanes (`lanes`) and gantt charts (`gantt`). */
export interface Group {
  id: string;
  label?: string;
  color?: Color;
  /** Enclosing group, for nested groups (and lanes). */
  parent?: string;
  /** Swimlanes: the groups inside are lanes. */
  lanes?: boolean;
  /** Gantt chart: the nodes inside are tasks on a timeline. */
  gantt?: boolean;
  /** Gantt chart: its first day (`YYYY-MM-DD`). */
  start?: string;
  /** Gantt chart: timeline resolution (week when unset). */
  scale?: Scale;
  /** Risk matrix: the nodes inside are risks. */
  risks?: boolean;
  /** Risk matrix: levels of probability and impact (3–5, 5 when unset). */
  levels?: number;
  /** Swimlanes / gantt chart: pinned position (top-left). */
  x?: number;
  y?: number;
}

/** A review comment on an element (`on`) or on the canvas (`x`, `y`). Open comments become prompts. */
export interface Comment {
  /** `c1`, `c2`, … when not given. */
  id: string;
  /** Id of a node, edge or group; absent for a canvas comment. */
  on?: string;
  text: string;
  author?: string;
  /** Default `open`. */
  status?: Status;
  /** How it was resolved (e.g. by an agent). */
  resolution?: string;
  replies?: Reply[];
  x?: number;
  y?: number;
}

export interface Reply {
  author?: string;
  text: string;
}

/** A problem in a source. `line` is 1-based; 0 means not tied to a line (e.g. JSON input). */
export interface Diagnostic {
  line: number;
  message: string;
}

// ----------------------------------------------------------------------- ops

/**
 * Add a node. Without `id` one is made from the label. With `parent` it is a
 * mindmap topic; with `mindmap` it starts a new map.
 */
export interface AddNodeOp {
  op: "add_node";
  id?: string;
  label?: string;
  shape?: Shape | string;
  color?: Color | string;
  detail?: string;
  group?: string;
  link?: string;
  tags?: string[];
  x?: number;
  y?: number;
  w?: number;
  h?: number;
  /** Mindmap: the topic to branch from. */
  parent?: string;
  /** Position among the parent's branches (0 = first; default last). */
  index?: number;
  /** Start a mindmap here. */
  mindmap?: MapLayout | string;
  /** Gantt task: start date (`YYYY-MM-DD`). */
  start?: string;
  /** Gantt task: duration in days. */
  days?: number;
  /** Gantt task: the tasks it starts after. */
  after?: string[];
  /** Gantt task: a milestone (a point in time, not a bar). */
  milestone?: boolean;
  /** Gantt task: percent done, 0–100. */
  done?: number;
  /** Gantt task: who does it. */
  owner?: string;
  /** Risk: how likely, 1–5 (0 clears it in ops). */
  probability?: number;
  /** Risk: how bad, 1–5 (0 clears it in ops). */
  impact?: number;
}

/** Change a node. An empty string clears an optional field. */
export interface UpdateNodeOp {
  op: "update_node";
  id: string;
  label?: string;
  shape?: Shape | string;
  color?: Color | "";
  detail?: string;
  group?: string;
  link?: string;
  tags?: string[];
  w?: number;
  h?: number;
  /** Mindmap: move the topic (with its branch) under another; "" detaches it into a map of its own. */
  parent?: string;
  /** Position among its siblings. */
  index?: number;
  /** Mindmap layout of a centre; "" ends the map. */
  mindmap?: MapLayout | "";
  /** Fold or unfold the branches under a topic. */
  collapsed?: boolean;
  /** Gantt task: start date ("" clears it). */
  start?: string;
  /** Gantt task: duration in days (0 clears it). */
  days?: number;
  /** Gantt task: the tasks it starts after ([] clears it). */
  after?: string[];
  /** Gantt task: a milestone (a point in time, not a bar). */
  milestone?: boolean;
  /** Gantt task: percent done, 0–100 (0 clears it). */
  done?: number;
  /** Gantt task: who does it ("" clears it). */
  owner?: string;
  /** Risk: how likely, 1–5 (0 clears it in ops). */
  probability?: number;
  /** Risk: how bad, 1–5 (0 clears it in ops). */
  impact?: number;
}

/** Add topics from an indented outline (or a Markdown list) under `parent`; without `parent` the first line becomes a new mindmap. */
export interface AddOutlineOp {
  op: "add_outline";
  parent?: string;
  outline: string;
  index?: number;
  x?: number;
  y?: number;
}

/** Rename an id everywhere it is referenced. */
export interface RenameOp {
  op: "rename";
  id: string;
  to: string;
}

/** Remove a node or group (with its edges), an edge or a comment. A mindmap topic goes with its branch. */
export interface RemoveOp {
  op: "remove";
  id: string;
  /** Groups: keep the members (default true). Mindmap topics: keep the branches (default false). */
  keep_members?: boolean;
}

/** Pin a node, swimlanes or gantt chart at a position (top-left). Mindmap topics: relative to the centre. */
export interface MoveOp {
  op: "move";
  id: string;
  x: number;
  y: number;
}

/** Move pinned nodes by an offset (a node, or every node in a group). */
export interface NudgeOp {
  op: "nudge";
  id: string;
  dx: number;
  dy: number;
}

/** Let the layout place an element again; without `id`, everything. */
export interface UnpinOp {
  op: "unpin";
  id?: string;
}

export interface AddEdgeOp {
  op: "add_edge";
  id?: string;
  from: string;
  to: string;
  label?: string;
  arrow?: Arrow | string;
  line?: Line | string;
  color?: Color | string;
}

/** Change an edge. An empty string clears an optional field. */
export interface UpdateEdgeOp {
  op: "update_edge";
  id: string;
  from?: string;
  to?: string;
  label?: string;
  arrow?: Arrow | string;
  line?: Line | string;
  color?: Color | "";
}

export interface AddGroupOp {
  op: "add_group";
  id?: string;
  label?: string;
  color?: Color | string;
  parent?: string;
  /** Nodes (or groups) to move into the new group. */
  members?: string[];
  /** Swimlanes: the groups inside are lanes. */
  lanes?: boolean;
  /** Gantt chart: the nodes inside are tasks. */
  gantt?: boolean;
  /** Risk matrix: the nodes inside are risks. */
  risks?: boolean;
  /** Risk matrix: 3, 4 or 5 levels (5 by default). */
  levels?: number;
  /** Gantt chart: its first day (`YYYY-MM-DD`). */
  start?: string;
}

/** Change a group. An empty string clears an optional field. */
export interface UpdateGroupOp {
  op: "update_group";
  id: string;
  label?: string;
  color?: Color | "";
  parent?: string;
  lanes?: boolean;
  gantt?: boolean;
  /** Gantt chart: its first day ("" clears it). */
  start?: string;
  /** Gantt chart: timeline resolution ("" resets it). */
  scale?: Scale | "";
  /** Risk matrix: 3, 4 or 5 levels (0 resets to 5). */
  levels?: number;
}

export interface AddCommentOp {
  op: "add_comment";
  id?: string;
  /** Element id; omit (and give `x`, `y`) for a canvas comment. */
  on?: string;
  text: string;
  author?: string;
  x?: number;
  y?: number;
}

export interface ReplyOp {
  op: "reply";
  /** Comment id. */
  id: string;
  text: string;
  author?: string;
}

export interface EditCommentOp {
  op: "edit_comment";
  id: string;
  text: string;
}

export interface ResolveCommentOp {
  op: "resolve_comment";
  id: string;
  resolution?: string;
}

export interface ReopenCommentOp {
  op: "reopen_comment";
  id: string;
}

/** Document settings. */
export interface SetOp {
  op: "set";
  key: "title" | "description" | "direction" | "layout" | "routing" | "theme";
  value: string;
}

/** Statements in the diagram language, added as if written at the end of the file; an element written again by its id gets the attributes given. */
export interface MergeOp {
  op: "merge";
  dsl: string;
}

/** One edit. A batch of ops is applied atomically. */
export type Op =
  | AddNodeOp | UpdateNodeOp | AddOutlineOp | RenameOp | RemoveOp | MoveOp | NudgeOp | UnpinOp
  | AddEdgeOp | UpdateEdgeOp | AddGroupOp | UpdateGroupOp
  | AddCommentOp | ReplyOp | EditCommentOp | ResolveCommentOp | ReopenCommentOp | SetOp | MergeOp;

/** What Board.apply() accepts: ops, `{ops}`, one op, or a model's reply text containing that JSON. */
export type OpsInput = Op[] | { ops: Op[] } | Op | string;

/** What a batch did. */
export interface ApplyResult {
  /** Ids created by the batch, in order (useful when ids were generated). */
  created: string[];
  /** Ops applied. */
  count: number;
}

// -------------------------------------------------------------------- render

export interface RenderOptions {
  /** Overrides the document's theme. */
  theme?: Theme;
  /** Comment pins: open ones (default), all (resolved greyed out) or none. */
  comments?: "open" | "all" | "none";
  /** Draw the document's title above the diagram (default false). */
  title?: boolean;
  /** No background. */
  transparent?: boolean;
  /** Today (`YYYY-MM-DD`): dated gantt charts mark it with a line. */
  today?: string;
  /** Crop the view to these elements (nodes or groups). */
  focus?: string[];
  padding?: number;
}

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface MapInfo {
  /** The map's central topic. */
  root: string;
  parent?: string;
  depth: number;
  /** Where this topic sits relative to its parent. */
  side: "right" | "left" | "down";
  layout: MapLayout;
  /** Topics folded away under this one. */
  folded: number;
  children: number;
}

/** Where a node was placed (board coordinates). */
export interface NodeBox extends Rect {
  id: string;
  /** Mindmap topics. */
  map?: MapInfo;
  /** A gantt task (drawn as a bar). */
  task?: boolean;
  /** A risk: drawn as a numbered dot on its matrix. */
  risk?: boolean;
}

/** Where a risk matrix's grid and list are drawn. */
export interface MatrixBox {
  levels: number;
  cell: number;
  x0: number;
  y0: number;
  list: number;
  /** Risks, in order (numbered from 1). */
  rows: string[];
  /** The grid's top-left on the board. */
  left: number;
  top: number;
}

export interface GanttBox {
  /** Width of the task-name column. */
  names: number;
  /** Where day 0 starts (offset in the chart). */
  x0: number;
  /** Width of one day. */
  day: number;
  /** Days shown. */
  days: number;
  /** The first day (days since 1970), when the chart has dates. */
  origin?: number;
  /** Task ids, top to bottom. */
  rows: string[];
  /** Where day 0 is on the board (x). */
  left: number;
  scale: Scale;
}

export interface GroupBox extends Rect {
  id: string;
  depth: number;
  /** Swimlanes: this group holds lanes. */
  lanes?: boolean;
  /** A lane: title strip on the left (`row`) or on top (`column`). */
  lane?: "row" | "column";
  /** A lane's title strip: its width (rows) or height (columns). */
  head?: number;
  gantt?: GanttBox;
  /** A risk matrix's grid. */
  matrix?: MatrixBox;
}

export interface EdgePath {
  id: string;
  /** SVG path data. */
  d: string;
  /** Where the label (or a comment pin) goes. */
  mid: [number, number];
}

/** A comment pin. */
export interface Pin {
  /** Comment id. */
  id: string;
  number: number;
  x: number;
  y: number;
  resolved: boolean;
}

/** The SVG plus the geometry of everything on it, for interactive tools. */
export interface RenderResult {
  svg: string;
  /** viewBox of the SVG, in board coordinates. */
  view: Rect;
  nodes: NodeBox[];
  groups: GroupBox[];
  edges: EdgePath[];
  pins: Pin[];
}

// ------------------------------------------------------------------- prompts

export interface PromptOptions {
  /** Change the diagram (default), the code it describes, or just discuss. */
  target?: "diagram" | "code" | "discuss";
  /** How diagram changes should come back: ops JSON (default) or the whole DSL. */
  reply?: "ops" | "dsl";
  /** Only these comment ids (all open ones when empty). */
  comments?: string[];
  /** Also include resolved comments, as context. */
  includeResolved?: boolean;
  /** Extra instructions appended to the prompt. */
  instructions?: string;
  /** Path of the diagram file, so code agents can update it. */
  file?: string;
}

export interface ExpandOptions {
  /** What the person wants ("risks for each", "questions to ask legal"). */
  instructions?: string;
  /** About how many new topics per level (default 3–6). */
  count?: number;
  /** How many levels to add (default 1). */
  levels?: number;
}

// ---------------------------------------------------------------------- diff

export interface DiffEntry {
  kind: "node" | "edge" | "group" | "comment" | "setting";
  id: string;
  change: "added" | "removed" | "changed";
  /** For `changed`: [field, old value, new value]. */
  fields?: [string, string, string][];
}

/** What changed between two versions (positions are ignored). */
export interface Diff {
  entries: DiffEntry[];
}

// ------------------------------------------------------------------- formats

/** Output formats of Board.source() and convert(). */
export type Format = "dsl" | "json" | "mermaid" | "brief" | "svg";

// ----------------------------------------------------------------------- api

/** A parse, render or apply failure. */
export class DiagramError extends Error {
  name: "DiagramError";
  /** Problems with their line (parse failures); `[{line: 0, message}]` otherwise. */
  errors: Diagnostic[];
  /** For a failed apply: 0-based index of the op that failed (absent when the ops could not be read). */
  index?: number;
}

/** What init() accepts to load the WASM from somewhere else. */
export type InitInput =
  | ArrayBuffer | ArrayBufferView
  | WebAssembly.Module
  | URL
  | string
  | Response
  | Request
  | Promise<ArrayBuffer | ArrayBufferView | WebAssembly.Module | Response>;

/**
 * Load the WASM module. Idempotent; await it once before anything else.
 * Node reads `pkg/diagram_wasm_bg.wasm` next to the module; browsers fetch it
 * relative to the module. An explicit `input` wins.
 */
export function init(input?: InitInput): Promise<void>;

/** Synchronous init from the .wasm bytes or a compiled module. */
export function initSync(bytesOrModule: ArrayBuffer | ArrayBufferView | WebAssembly.Module): void;

/** A board (one `.diagram` document) being read or edited. */
export class Board {
  private constructor();

  /** An empty board. */
  static empty(): Board;

  /** Parse DSL, JSON or Mermaid (detected). @throws DiagramError with `.errors` */
  static parse(src: string): Board;

  /**
   * Apply ops atomically. Accepts ops, `{ops}`, one op, or a model's reply
   * text containing the JSON (a ```json block or bare braces).
   * @throws DiagramError (with `.index` when an op failed); the board is then unchanged.
   */
  apply(ops: OpsInput): ApplyResult;

  /** The board as text (`dsl` by default). */
  source(format?: Format): string;

  /** The document model. */
  document(): Document;

  /** Lay out and render. When a diff base is set, changes are highlighted. */
  render(options?: RenderOptions): RenderResult;

  /** A prompt asking a model to act on the review comments. */
  prompt(options?: PromptOptions): string;

  /** A Markdown description of the board, for LLM context. */
  brief(): string;

  /** The mindmap under topic `id` as an indented outline (`id` first). @throws DiagramError if there is no such node */
  outline(id: string): string;

  /** The mindmap under topic `id` as Markdown (title, sections, bullets). @throws DiagramError if there is no such node */
  markdown(id: string): string;

  /** The selected elements as Markdown to paste into a prompt. */
  selection(ids: string[]): string;

  /**
   * Part of the board as diagram language, picked by selectors: `ID`, `group:ID`, `chart:ID`,
   * `map:ID`, `near:ID`, `label~TEXT`, `kind:box|topic|task|milestone|sticky|decision|group|edge|comment`,
   * `owner:NAME`, `tag:NAME`, `color:NAME`, `comments:open`. Several selectors: everything any picks.
   * @throws DiagramError when a selector is unknown or picks nothing
   */
  query(...selectors: (string | string[])[]): string;

  /** The ids the selectors pick (see query), in document order. */
  select(...selectors: (string | string[])[]): string[];

  /** Add statements in the diagram language (op `merge`). */
  merge(dsl: string): ApplyResult;

  /**
   * What changed since `base` (any format). Setting a base also makes
   * render() highlight changes; `null` clears it; omit to reuse the last.
   * @throws DiagramError when `base` is invalid
   */
  diff(base?: string | null): Diff;

  /** Like diff(), as Markdown. */
  diffMarkdown(base?: string | null): string;

  /** A small prompt asking a model to grow mindmap topic `id`. Feed the reply to replyOutline() and an `add_outline` op. */
  expandPrompt(id: string, options?: ExpandOptions): string;

  /** Release the WASM memory. */
  free(): void;

  [Symbol.dispose](): void;
}

/** Parse a source into a document. @throws DiagramError */
export function parse(src: string): Document;

/** Render a source in one go. @throws DiagramError */
export function render(src: string, options?: RenderOptions): RenderResult;

/** Convert a source to another format. @throws DiagramError */
export function convert(src: string, to: Format): string;

/** The language reference (Markdown), including the ops reference: good LLM context. */
export function spec(): string;

/** JSON Schema of the document format. */
export function jsonSchema(): Record<string, unknown>;

/** Version of the diagram core. */
export function version(): string;

/** A prompt asking a model for a new mindmap about `topic`. Feed the reply to replyOutline() and an `add_outline` op. */
export function mindmapPrompt(topic: string, options?: ExpandOptions): string;

/** A model's reply (bullets, numbering, code fences…) as a clean two-space outline for `add_outline`. */
export function replyOutline(reply: string): string;
