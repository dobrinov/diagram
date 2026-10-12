---
name: diagram-mindmap
description: Create, grow and reorganise mindmaps in .diagram files (brainstorms, discovery maps, project breakdowns, prompts written as mindmaps) with few tokens — indented outlines in, `add_outline` ops for edits, Markdown out. Use when the user asks for a mindmap, wants to brainstorm or break a topic down, or wants a mindmap turned into a prompt.
---

# diagram-mindmap

A mindmap is an indented outline inside a `mindmap` block. Write and read it as an outline: it is the cheapest form for you and the clearest for people.

## Create

```diagram
mindmap launch: Launch plan [layout: both] {
  Discovery
    Interview the ops team
    Current volumes
  Build
    Checkout flow
  Risks [red]
    Bank partner delays
}
```

Save it in a `.diagram` file (or add the block to an existing board), then `diagram check FILE`. Keep topics short (2–7 words); put longer text in `[detail: …]`.

## Grow or change a map without rewriting the file

- Read it: `diagram outline FILE [ID]` (ids are the slugs of the labels: `Current volumes` → `current-volumes`; `diagram brief FILE` lists them all).
- Add topics under a topic, as an outline:
  ```bash
  diagram apply FILE - <<'JSON'
  {"ops": [{"op": "add_outline", "parent": "risks", "outline": "Fraud\n  Card testing\nRegulatory changes"}]}
  JSON
  ```
- Move a branch: `{"op":"update_node","id":"checkout-flow","parent":"discovery","index":0}`. Fold one: `"collapsed": true`. Remove a topic with its branch: `{"op":"remove","id":"x"}` (`"keep_members": true` lifts its branches up instead).
- New map from scratch next to other content: `{"op":"add_outline","outline":"Topic\n  Branch\n    Sub","x":0,"y":600}`.
- `diagram expand FILE ID [--ask "risks only"]` asks the configured AI harness (`--ai CMD` / `$DIAGRAM_AI`) for subtopics and adds them.

## Mindmap → prompt

`diagram outline FILE ID --md` prints the map as Markdown: the centre is the title, deeper topics are bullets, and main branches are `##` sections when every one of them has topics or a detail under it (otherwise the whole map is one nested list). Use it as the prompt for the task the map describes (e.g. a map with Context / Constraints / Steps / Acceptance branches). Topics with `[link: #id]` pull the linked block (swimlanes, a gantt chart, a section, another map) into the prompt after the map.

## Gotchas

- Inside a `mindmap` block every line is a topic. Edges, comments and other nodes go outside the block.
- Don't give branch topics `x`/`y`; only the centre is placed. Move a map by moving its centre.
- A label that starts with `- `, `#` or `}` must be quoted: `"# of users"`.
