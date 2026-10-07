---
name: diagram-draw
description: Draw a diagram (architecture, flow, sequence of steps, org or idea board) as a .diagram file with the `diagram` CLI, render it to PNG/SVG and open it in the editor. Use when the user asks to sketch, draw, visualise or diagram something, or wants a picture of how parts of a codebase fit together.
---

# diagram-draw

Diagrams are plain-text `.diagram` files. People edit them in a FigJam-like browser editor (`diagram FILE`), and they diff cleanly in git.

## Steps

1. If you are not sure of the syntax, read the language reference with `diagram spec`. The essentials:
   ```diagram
   title: Checkout
   direction: right                    # right | down | left | up
   user: Customer [person]
   group backend: Backend [blue] {
     api: Orders API [detail: Rust · Axum]
     db: Postgres [cylinder]
   }
   user -> api: POST /orders           # -> solid, ..> dashed, => thick, -- no arrow, <-> both
   api -> db
   ```
   Shapes: box, rounded, pill, circle, diamond, hexagon, cylinder, parallelogram, document, person, cloud, sticky, text.
   Colours: gray, blue, green, yellow, orange, red, purple, pink, teal.
2. Gather the facts first. When drawing a codebase, read the code (entry points, services, queues, databases, external APIs) and draw what is actually there. Use short labels, put tech details in `detail:`, and group by deployable unit or bounded context.
3. Write the file, e.g. `docs/diagrams/<name>.diagram`. Leave out `x`/`y`, because the layout places nodes automatically.
4. Validate it, then render it:
   ```bash
   diagram check docs/diagrams/checkout.diagram
   diagram png docs/diagrams/checkout.diagram -o docs/diagrams/checkout.png   # or: diagram svg …
   ```
   Look at the PNG. If it is cramped, try `direction: down`, fewer edges or groups.
5. Offer to open the editor: `diagram docs/diagrams/checkout.diagram` (run it in the background; it serves on localhost and opens the browser).

## Tips

- Prefer 5–20 nodes. Split a big system into several diagrams.
- Edge labels should name the interaction (`charge`, `order.created`), not repeat the node names.
- Use `comment on <id>: …` to flag open questions you could not answer from the code.
- `diagram convert FILE --to mermaid` gives a Mermaid version for GitHub Markdown.
