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

There is one statement per line. A line that starts with `#` or `//` is a comment for people reading the file. It is not kept when the file is reformatted, so use `comment` for review notes.

| Statement | Meaning |
|---|---|
| `id` | a node whose label is its id |
| `id: Label text [attrs]` | a node with a label |
| `"Some label"` | a node with id `some-label` |
| `a -> b` · `a -> b: label [attrs]` | an edge |
| `a -> b -> c` | a chain of edges (the label applies to each) |
| `a -> b, c` | fan-out: one edge to each node in the list |
| `group id: Label [attrs] {` … `}` | a group. Nodes declared or listed inside belong to it. Groups nest. |
| `comment [id] [on target] [at x,y] [by author]: text [attrs]` | a review comment. Ids default to `c1`, `c2`, … |
| `reply id [by author]: text` | a reply to a comment |
| `title:` `description:` `direction:` `layout:` `routing:` `theme:` | settings (top level only) |

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
| nodes | `shape`, `color`, `detail` (smaller second line), `link`, `tags` (space separated), `x`, `y` (pinned position), `w`, `h` |
| edges | `label`, `arrow` (`end`, `start`, `both`, `none`), `line` (`solid`, `dashed`, `dotted`, `thick`), `color`, `id` |
| groups | `color`, `label` |
| comments | `resolved` / `open`, `resolution`, `author`, `x`, `y` |

- **Shapes:** `box` (default), `rounded`, `pill`, `circle`, `diamond`, `hexagon`, `cylinder`, `parallelogram`, `document`, `person`, `cloud`, `sticky`, `text`
- **Colours:** `gray`, `blue`, `green`, `yellow`, `orange`, `red`, `purple`, `pink`, `teal`, or `#rrggbb`
- **Settings:** `layout: layered | grid`, `routing: orthogonal | curved | straight`, `theme: light | dark`

Nodes without `x`/`y` are placed automatically. Dragging a node in the editor pins it there. `diagram apply FILE` with `{"op":"unpin"}` releases every pinned node.

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
