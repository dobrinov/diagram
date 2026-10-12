# The diagram language

A diagram can be written three ways. All three hold the same information, and
`diagram convert` translates between them without losing any of it.

| Format | Best for | Extension |
|---|---|---|
| **DSL**: one statement per line | people: writing by hand, reviewing in a PR | `.diagram` |
| **JSON**: the document model | programs and LLMs that emit structured output | `.diagram.json` |
| **Ops**: a JSON list of edits | LLMs and tools *changing* an existing diagram | (any) |

Mermaid flowcharts (`flowchart LR …`) can be imported and exported as well.

## DSL

```diagram
title: Checkout
direction: right            # right (default) | down | left | up

user: Customer [person]
group backend: Backend [blue] {
  api: Orders API [detail: Rust · Axum]
  db: Postgres [cylinder]
}
mail: E-mail service [cloud]
note: Retries are idempotent\n(keyed by order id) [sticky]

user -> api: POST /orders
api -> db
api ..> mail: receipt [dashed]

comment on api by deyan: Should payments be a separate service?
reply c1 by ana: Not before launch.
comment c9 at 400,20: Overall flow looks right [resolved, resolution: ok]
```

### Statements

There is one statement per line. A line that starts with `#` or `//` is a note for people reading the file; use `comment` for review notes.

Edits keep the file as it is written. Ops, the editor and `diagram apply` rewrite only the statements whose elements changed (a chain like `a -> b -> c` is split only when one of its arrows changes; a changed mindmap or gantt chart is rewritten as a block), put new statements next to their kind (inside their section, after the last box, arrow or comment), and leave every other line, blank line and `#` note alone. Only `diagram fmt` rewrites the whole file in its canonical form.

| Statement | Meaning |
|---|---|
| `id` | a node whose label is its id |
| `id: Label text [attrs]` | a node with a label |
| `"Some label"` | a node with id `some-label` |
| `a -> b` · `a -> b: label [attrs]` | an edge |
| `a -> b -> c` | a chain of edges (the label applies to each) |
| `a -> b, c` | fan-out: one edge to each node in the list |
| `group id: Label [attrs] {` … `}` | a group (a *section* in the editor): a titled frame around what is declared or listed inside. Groups nest. |
| `lanes id: Title {` … `lane id: Title {` … `}` … `}` | swimlanes: each `lane` is a band (a row when the flow runs right or left, a column when it runs down or up) with its title at the side; the flow runs across lanes |
| `gantt id: Title [start: YYYY-MM-DD] {` … `}` | a gantt chart: one task per line, `Label [5d, after: other-task]` (see *Gantt charts*) |
| `mindmap id: Topic [attrs] {` … `}` | a mindmap: the line is its central topic and the body is an indented outline (see *Mindmaps*) |
| `comment [id] [on target] [at x,y] [by author]: text [attrs]` | a review comment. Ids default to `c1`, `c2`, … |
| `reply id [by author]: text` | a reply to a comment |
| `title:` `description:` `direction:` `layout:` `routing:` `theme:` | settings (top level only). The title is metadata (briefs, export names); it is not drawn on the board (`--title` draws it in exports) |

Ids are made of letters, digits, `_`, and `-` or `.` between them (`orders-api`, `svc.v2`). Writing `\n` in a label starts a new line. Quote a label (`"…"`) when it should keep leading or trailing spaces.

Edges create any node they mention that does not exist yet. A node that is declared twice gets the union of both declarations.

### Edge operators

| | arrow at end | both ends | no arrow |
|---|---|---|---|
| solid | `->` (also `-->`) | `<->` | `--` |
| dashed | `..>` | `<..>` | `..` |
| thick | `=>` | `<=>` | `==` |

`a <- b`, `a <.. b` and `a <= b` are the same as `b -> a`, `b ..> a` and `b => a`. Dotted lines use the `[dotted]` attribute.

An edge's id is `from->to`, or `from->to#2` for the second edge between the same pair. Set `[id: x]` to give it a stable name.

### Attributes

Attributes go in square brackets at the end of the line as `key: value` pairs separated by commas. A bare shape, colour or line-style name also works: `[cylinder, blue]`.

| On | Keys |
|---|---|
| nodes | `shape`, `color`, `detail` (smaller second line), `link` (a web page, or `#id` to go to another element on the board), `tags` (space separated), `x`, `y` (pinned position; on a mindmap topic, relative to its centre), `w`, `h`; mindmap topics also `collapsed`, `id`, and a centre `layout` |
| edges | `label`, `arrow` (`end`, `start`, `both`, `none`), `line` (`solid`, `dashed`, `dotted`, `thick`), `color`, `id` |
| groups | `color`, `label` |
| comments | `resolved` / `open`, `resolution`, `author`, `x`, `y` |

- **Shapes:** `box` (default), `rounded`, `pill`, `circle`, `diamond`, `hexagon`, `cylinder`, `parallelogram`, `document`, `person`, `cloud`, `sticky`, `text`
- **Colours:** `gray`, `blue`, `green`, `yellow`, `orange`, `red`, `purple`, `pink`, `teal`, or `#rrggbb`
- **Settings:** `layout: layered | grid`, `routing: orthogonal | curved | straight`, `theme: light | dark`

Nodes without `x`/`y` are placed automatically. Dragging a node in the editor pins it there. `diagram apply FILE` with `{"op":"unpin"}` releases every pinned node.

### Swimlanes

```diagram
lanes order: Order to cash {
  lane customer: Customer {
    place: Place order [pill]
    pay: Pay invoice
  }
  lane sales: Sales {
    check: Credit OK? [diamond]
    confirm: Confirm order
  }
  lane finance: Finance [blue] {
    invoice: Send invoice
  }
}
place -> check
check -> confirm: yes
check -> place: no
confirm -> invoice -> pay
```

- `lanes` is a group whose groups are lanes; in JSON it is a group with `"lanes": true`, and its lanes are the groups whose `parent` it is.
- Everything in all lanes is ranked together, so a step in one lane lines up with the steps before and after it in the others. Lanes share one length; a lane is as tall (or wide) as what it holds.
- Lanes are laid out automatically: `x`/`y` on a block inside a lane are ignored. `lanes id: Title [x: 400, y: 120] {` pins the whole swimlanes (top-left), as dragging them in the editor does. Edges can leave the lanes for anything else on the board.

### Gantt charts

```diagram
gantt launch: Launch plan [start: 2026-10-12] {
  Design [5d]
  Build [2w, after: design]
  Docs [3d, after: design, purple]
  Beta [2026-11-03, 1w]
  Launch day [after: beta docs, milestone, green]
  QA [1w, after: build, done: 40%, owner: ana]
}
```

- Each line is a task: its label, then optional `[attrs]`. Ids come from labels as in mindmaps (`[id: x]` to choose one).
- `5d` / `2w` / `1m` (30 days) / `1q` (91 days), or `days: 5`, is the duration (1 day if left out). `2026-11-03` (or `start: …`) fixes the start. `after: a b` starts it when those tasks end. Otherwise a task starts when the one above it ends; the first starts on the chart's `start`.
- `after:` names a task of the same chart first (its id, or its label as an id: `after: design` is this chart's Design even if another chart's took the id `design`), then any task on the board, so a task can wait for one in another chart (drawn dashed; both charts need dates). `chart/task` names a task of another chart when this one has a task of the same name.
- `milestone` makes a task a point in time (a diamond on the day it falls), `done: 60%` (or `done` for all of it) shades the part that is done, and `owner: ana` shows who does it next to the task's name.
- Days are calendar days. A colour colours the bar. In JSON, the chart is a group with `"gantt": true` and `start`; tasks are its nodes with `start`, `days`, `after`, `milestone`, `done` and `owner`.
- Bars are tasks like any node: comment on them, link them to anything on the board.
- `x`/`y` on the chart (`[start: …, x: 400, y: 120]`) pin it, as dragging it in the editor does.
- `scale: day | week | month | quarter` sets the timeline's resolution (week when left out): lines and dates at each day, Monday, 1st of the month or quarter. In JSON it is the group's `scale`.
- In the editor: drag a bar to reschedule it and its end to change its duration (right-click → *Duration…* to type one); draw an arrow from one bar to another to make the second wait for the first (right-click the dependency line to remove it, or a task → *Starts after…*); right-click a task → *Colour…* to colour its bar, *Progress…*, *Owner…* or *Make it a milestone*; a red line marks today; right-click the chart → *Scale…*.

### Mindmaps

```diagram
mindmap offsite: Team offsite [layout: both, x: 0, y: 0] {
  Venue
    Who books it?
    City vs countryside
  Budget [red]
    Travel and hotels
  Agenda [collapsed]
    Workshops
  Food [id: catering]
}
offsite-note: Ask finance about limits [sticky]
offsite-note ..> who-books-it
```

- The header line is the **central topic** (same syntax as a node). Its `layout` is `right` (default), `left`, `both` (balanced) or `down` (org chart). `x`/`y` pin the centre; the whole map moves with it.
- Each body line is a **topic**: its label, then optional `[attrs]`. Indentation makes the tree (any consistent indent; Markdown bullets `- ` / `* ` are allowed, so a pasted list works).
- Topic ids come from their labels (`Who books it?` → `who-books-it`); a clash gets `-2`. Write `[id: x]` to choose one. Edges and comments use these ids, so mindmap topics can be linked to anything on the board.
- Each level of a side lines up in one column, and each branch's sub-topics fan out evenly around it. A topic with `x`/`y` was placed by hand: the position is relative to the centre's top-left, its own branches follow it, and its siblings close up. `{"op":"unpin","id":ID}` puts it back in the automatic layout (the editor's *Reset arrangement* does that for the whole map).
- `[collapsed]` folds the branches under a topic. A colour on a topic colours its whole branch; main branches get distinct colours automatically.
- In JSON a topic is a node with `parent` (the topic it branches from); the centre has `mindmap` (its layout). Siblings keep document order.
- `diagram outline FILE` prints a map as an outline and `--md` as Markdown (the centre is the title, topics are bullets; main branches are `##` sections when each has something under it): that is how a mindmap becomes a prompt. A topic with `link: #id` reads "label (see “target”)", and the block it points at (a flow, a plan, a section, another map) follows as a section of its own, so one prompt map can gather context from the whole board. `diagram expand FILE ID` grows a topic with your AI harness.

## JSON

The JSON form is the document model itself. Only `nodes` is required. Every other field has a default and can be left out.

```json
{
  "title": "Checkout",
  "settings": {"direction": "right", "layout": "layered", "routing": "orthogonal", "theme": "light"},
  "groups": [{"id": "backend", "label": "Backend", "color": "blue"}],
  "nodes": [
    {"id": "user", "label": "Customer", "shape": "person"},
    {"id": "api", "label": "Orders API", "group": "backend", "detail": "Rust · Axum"},
    {"id": "db", "label": "Postgres", "shape": "cylinder", "group": "backend"}
  ],
  "edges": [
    {"from": "user", "to": "api", "label": "POST /orders"},
    {"from": "api", "to": "db", "line": "dashed", "arrow": "end"}
  ],
  "comments": [
    {"id": "c1", "on": "api", "author": "deyan", "text": "Split payments out?", "status": "open",
     "replies": [{"author": "ana", "text": "Not before launch."}]}
  ]
}
```

`diagram spec --json-schema` prints the JSON Schema.

## Ops

An ops batch changes a diagram without rewriting it, and it is the safest format to ask an LLM for. A batch is applied all at once or not at all. In `update_*` ops, an empty string clears a field.

```json
{"ops": [
  {"op": "add_node", "id": "cache", "label": "Redis", "shape": "cylinder", "group": "backend"},
  {"op": "add_edge", "from": "api", "to": "cache", "label": "reads"},
  {"op": "resolve_comment", "id": "c1", "resolution": "Kept one service; added a cache"}
]}
```

OPS_REFERENCE

`diagram apply FILE reply.md` accepts a chat reply as it is: it uses the first ```json block in the text.
