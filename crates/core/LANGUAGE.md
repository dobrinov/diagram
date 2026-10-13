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
| `table id: Title {` … `}` | a table: a header row, then rows of cells separated by `\|` (see *Tables*) |
| `chart id: Title [bar] {` … `}` | a chart: a small table, `Q1 \| 120 \| 80` (see *Charts*) |
| `risks id: Title [scale: 5] {` … `}` | a risk matrix: one risk per line, `Label [p: 4, i: 3, owner: ana, detail: "…"]` (see *Risk matrices*) |
| `mindmap id: Topic [attrs] {` … `}` | a mindmap: the line is its central topic and the body is an indented outline (see *Mindmaps*) |
| `comment [id] [on target] [at x,y] [by author]: text [attrs]` | a review comment. Ids default to `c1`, `c2`, … |
| `reply id [by author]: text` | a reply to a comment |
| `theme id [base: light] {` … `}` | a theme: `key: value` lines (see *Themes*) |
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
| groups | `color`, `label`, `x`, `y` (placed by hand: the section keeps its place and its boxes are laid out in it) |
| comments | `resolved` / `open`, `resolution`, `author`, `x`, `y` |

- **Shapes:** `box` (default), `rounded`, `pill`, `circle`, `diamond`, `hexagon`, `cylinder`, `parallelogram`, `document`, `person`, `cloud`, `sticky`, `text`
- **Colours:** `gray`, `blue`, `green`, `yellow`, `orange`, `red`, `purple`, `pink`, `teal`, or `#rrggbb`
- **Settings:** `layout: layered | grid`, `routing: orthogonal | curved | straight`, `theme: light | dark | paper | blueprint | midnight | contrast | <a theme's id> | ./file.theme`

Nodes without `x`/`y` are placed automatically. Dragging a node in the editor pins it there. `diagram apply FILE` with `{"op":"unpin"}` releases every pinned node.

### Themes

```diagram
theme: brand

theme brand {
  font: "IBM Plex Sans", Inter, sans-serif
  radius: 4
  blue: #3b6fd8
  light {
    background: #faf9f6
    text: #1f2328
    title: #5b5bd6
  }
  dark {
    background: #101418
    text: #e6edf3
    title: #8b8bf0
  }
}
```

- `theme:` picks the board's theme: a preset (`paper`, `blueprint`, `midnight`, `contrast`), the id of a `theme` block on the board, or a file next to the board (`theme: ./brand.theme`, a file holding one `theme … { }` block), so a team can keep one brand theme in the repo. `theme: light` / `dark` (the default theme) sets the mode exports use.
- Every theme has a light and a dark mode, and a board follows the viewer's mode (the editor's light/dark switch; `--dark` for exports). Lines at the top of a theme apply in both modes; `light { … }` and `dark { … }` hold what differs. Anything a mode leaves out comes from the default light or dark look.
- Keys: `background`, `dots`, `text`, `muted` (details, axes), `surface` (boxes), `border`, `line` (arrows), `section`, `header` (title strips, table headers), `title` (section title pills), `title-text`, `pin`, any named colour (`blue: #3b6fd8`, used wherever `blue` is), `font` (a CSS font list; monospace fonts are measured as such) and `radius` (boxes' corners). Colours are `#hex`; an unknown key is an error. (`theme id [base: dark] { … }`, from before themes had two modes, reads as that mode.)
- The theme applies to the board, its exports (a file theme goes into an HTML export) and the canvas around it in the editor. In the editor: the theme picker in the toolbar, and *Edit theme…* (the theme as text; a preset is copied into a new one).
- Ops: `{"op":"theme","source":"theme brand { … }"}` adds or replaces a theme, `{"op":"set","key":"theme","value":"brand"}` uses it, `{"op":"remove","id":"brand"}` removes it. In JSON: `themes` (`{id, values, light, dark}`, each a list of `[key, value]`) and `settings.palette`.

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
- **Forecast:** as of today (in the editor; `--today` or `--as-of DATE` for exports, `brief`, `outline --md`), an unfinished task that has started is forecast to take until today plus the part not done yet (by its `done`), and everything that waits for it moves with it, in other charts too. A slip is drawn as a red dashed extension with `+5d` (a hollow diamond where a milestone will land). Prompts add `forecast end 2026-10-26 (+5d, behind)`; `diagram schedule FILE [CHART] [--as-of DATE] [--json]` lists every task's plan, forecast and status (done, late, behind, in progress, pushed back, upcoming); `diff` lists the milestones and chart ends a plan change moves.
- Days are calendar days. A colour colours the bar. In JSON, the chart is a group with `"gantt": true` and `start`; tasks are its nodes with `start`, `days`, `after`, `milestone`, `done` and `owner`.
- Bars are tasks like any node: comment on them, link them to anything on the board.
- `x`/`y` on the chart (`[start: …, x: 400, y: 120]`) pin it, as dragging it in the editor does.
- `scale: day | week | month | quarter` sets the timeline's resolution (week when left out): lines and dates at each day, Monday, 1st of the month or quarter. In JSON it is the group's `scale`.
- In the editor: drag a bar to reschedule it and its end to change its duration (right-click → *Duration…* to type one); draw an arrow from one bar to another to make the second wait for the first (right-click the dependency line to remove it, or a task → *Starts after…*); right-click a task → *Colour…* to colour its bar, *Progress…*, *Owner…* or *Make it a milestone*; a red line marks today; right-click the chart → *Scale…*.

### Tables

```diagram
table options: Hosting options {
  option | cost  | latency | notes
  Local  | $0    | 40 ms   | Needs an 8B model on every laptop
  Hosted | $1.2k | 300 ms  | Data leaves the laptop [red]
}
```

- The first line is the header (the columns' names); each line after it is a row, cells separated by `|` (`\|` for a `|` in a cell, `\n` for a line break). Columns are lined up when the file is written; long cells wrap.
- A row's first cell is its label and gives its id within the table (`options.local`), so arrows, comments and links can point at a row; hovering it shows its cells by column. `[attrs]` at the end of a row: a colour tints it, `detail`, `link`, `id`. `w:` on the table sets its width.
- In the editor: the *Table* tool; right-click a table → *Edit as text…*, *Add row*, *Add column*; right-click a row → *Edit row…* (its cells, separated by `|`), *Add row below*, *Colour…*, *Delete row*. Renaming a row's label renames its id; renaming a table renames its rows.
- Prompts show it as a Markdown table. In JSON a table is a group with `table` and `columns`; its rows are nodes whose `label` is the first cell and `cells` the rest. Ops can place a row with `index`.

### Charts

```diagram
chart revenue: Revenue by quarter [bar, stacked, unit: k€] {
  quarter | EU  | US
  Q1      | 120 | 80
  Q2      | 150 | 95
}
chart share: Market share [pie, unit: %] {
  Jira 45
  Linear 25
  Other 30
}
```

- The type: `bar` (also `stacked`, `horizontal`), `line`, `area` (`stacked`), `pie`, `donut` (the total in the middle) or `scatter` (each row is `label | x | y`). `unit: %` / `k€` / `ms` goes after values (`$ € £ ¥` in front); `w:` / `h:` size it; `x:` / `y:` pin it.
- Each line is a row: its label, then one value per series, separated by `|` (`-` or nothing for none; `1,200` and `40%` are numbers). One value can follow the label after a space (`Jira 45`). A first row whose cells are not numbers is the header: what the rows are, then the series' names (shown in the legend).
- Rows are nodes with ids from their chart and label (`revenue.q1`), so arrows, comments and links can point at a bar, a point or a slice; hovering one shows its values. A colour on a row colours its bar or slice. Renaming a chart renames its rows.
- In the editor: the *Chart* tool; right-click a chart → *Edit data…* (the table as text), *Type…*, *Stacked*, *Horizontal bars*, *Unit…*; right-click a row → *Values…*, *Rename*, *Colour…*. Dragging a row moves the chart.
- Prompts show a chart as a Markdown table (a pie also with each slice's share). In JSON a chart is a group with `chart` (and `stacked`, `horizontal`, `unit`, `series`, `axis`); its rows are nodes with `values`.

### Risk matrices

```diagram
risks delivery: Delivery risks [scale: 5] {
  Bank contract slips [p: 4, i: 4, owner: ana, detail: "Legal says 4 weeks; it blocks the pilot"]
  Local model too weak for ops [p: 3, i: 3]
  Scope creep [p: 2, i: 3]
  Unknown unknowns
}
```

- Each line is a risk: its label, then `p:` (probability, also `probability:` / `likelihood:`) and `i:` (impact), from 1 to the matrix's `scale` (3, 4 or 5; 5 when left out). `detail` is its description, `owner` who watches it. Ids come from labels as in gantt charts.
- The matrix draws a grid of probability (rows, likeliest on top) by impact (columns), cells tinted by severity (probability × impact against the worst score: low, medium, high, critical), each risk a numbered dot in its cell, and the list of risks with their scores beside it. A risk without both levels is a hollow dot in the list.
- Hovering a dot or a row shows the risk's rating, owner and description. In the editor, drag a dot to another cell to re-rate it (off the grid clears the rating), or right-click it → *Probability…*, *Impact…*, *Description…*, *Owner…*; right-click the matrix → *Add risk*, *Size…*.
- Prompts list the risks worst first: `Bank contract slips: probability 4 × impact 4 = 16 (critical), owner ana — Legal says …`. In JSON the matrix is a group with `"risks": true` (and `levels`); risks are its nodes with `probability` and `impact`.
- Risks are nodes: arrows (a mitigation → the risk it reduces), comments and links work on them. `x`/`y` on the matrix pin it.

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

`diagram apply FILE reply.md` accepts a chat reply as it is: it uses the first ```json block in the text. One op can go inline: `diagram apply FILE '{"op":"remove","id":"cache"}'`. A mistyped id gets a suggestion (`no node `apii` (did you mean `api`?)`).

### Writing and reading parts of a board

- **`merge` / `diagram add FILE`** take statements in the diagram language and add them as if they were written at the end of the file: new elements are created, and an element written again by its id gets the attributes given (`api [red]`, `build [3w, done: 50%]`). A group written again keeps its place, so `lane platform {` … `}` adds to that lane. This is the cheapest way for a model to add a lot at once. (Topics and tasks written in a repeated `mindmap` / `gantt` block are added as new ones; change existing ones by id.)
- **`diagram query FILE SELECTOR…`** prints just part of the board, as diagram language with the sections it sits in: `ID`, `group:ID` (also `chart:`, `lanes:`), `map:ID` (a topic and its branches), `near:ID` (a box, its arrows and their other ends), `label~TEXT`, `kind:box|topic|task|milestone|sticky|decision|group|edge|comment`, `owner:NAME`, `tag:NAME`, `color:NAME`, `comments:open`. Several selectors give everything any of them picks; `--ids` prints only ids.
- **`--focus ID,ID`** on `svg` / `png` / `html` crops the picture to those elements.
