# Changelog

## 0.11.0 — 2026-10-12

- **Free text keeps its commas, and mistakes are errors.** `[detail: fast, opinionated, red]` reads as the detail "fast, opinionated" and the colour red (also `;`, and in `label`, `resolution`, `owner`). Brackets that start like attributes but don't read as them (`[shape: box, opinionated]`) are now an error with the line, instead of silently becoming part of the label (models hit this constantly).
- **Keywords are ids outside their statements:** `gantt -> b`, `risks -- b`, `comment -> x` are arrows again; `group`, `gantt`, `risks` and `mindmap` only open a block on a line that ends with `{`.
- **`prompt --context auto|near|section|all`:** a prompt about comments shows only what they are about on a big board (auto: the whole board up to 60 lines, else the commented sections; near: the commented blocks and their neighbours), and tells the model the rest exists. On a 300-element board one comment's prompt went from about 24k to 6.7k characters. Also in the WASM/JS prompt options and used by the editor.
- **Diff:** risk ratings and matrix size changes show up (and are highlighted with `--base`); title changes keep their case; description changes are listed.

## 0.10.0 — 2026-10-12

- **Risk matrices:** `risks id: Title [scale: 5] {` … `}` with one risk per line, `Label [p: 4, i: 3, owner: ana, detail: "…"]`. Drawn as a probability × impact grid with cells tinted by severity (low, medium, high, critical), each risk a numbered dot in its cell, and a list of the risks with their scores beside it; unrated risks wait in the list. Hover a dot or a row for its rating, owner and description.
- In the editor: a *Risk matrix* tool; drag a dot to another cell to re-rate it (off the grid clears it); right-click a risk → *Probability…*, *Impact…*, *Description…*, *Owner…*, *Rename*, *Delete* (also on its row in the list); right-click the matrix → *Add risk*, *Size…* (3×3, 4×4, 5×5).
- Prompts list risks worst first (`… = 16 (critical), owner ana — description`), `brief` includes them, `query` knows `kind:risk` and `kind:matrix`; ops `add_group` (`risks`, `levels`), `update_group` (`levels`), `add_node` / `update_node` (`probability`, `impact`). JS types updated.
- `examples/risks.diagram`; `scripts/e2e/risks.mjs` (part of `make e2e`).

## 0.9.0 — 2026-10-12

Tools for agents that drive boards:

- **`diagram add FILE [DSL|-]`** and the **`merge` op**: statements in the diagram language added to a board as if written at its end. New elements are created; one written again by its id gets the attributes given (`api [red]`); a group written again keeps its place. The cheapest way for a model to add a lot at once.
- **`diagram query FILE SELECTOR…`**: just part of a board, as diagram language with the sections around it: `group:`/`chart:`, `map:`, `near:`, `label~`, `kind:`, `owner:`, `tag:`, `color:`, `comments:open` (`--ids` for ids). Also `Board.query()` / `Board.select()` in the JS package and `query()` in WASM.
- **Inline ops:** `diagram apply FILE '{"op":…}'`.
- **`--focus ID,ID`** crops `svg`/`png`/`html` (and the `focus` render option) to some elements.
- **"Did you mean …?"** on ops with a mistyped id.
- `diff` (and the summary `apply` prints) now shows changes to gantt tasks and charts (dates, durations, dependencies, milestones, progress, owners, scale) and topic moves.
- `Board.merge(dsl)` in the JS package.

## 0.8.0 — 2026-10-12

- **JavaScript / TypeScript package** in `js/` (`make js`): an ES module for Node ≥ 18 and browsers with `index.d.ts` types. `await init()`, then `Board.parse(src)` / `Board.empty()` with `apply(ops | {ops} | model reply)` (atomic; throws `DiagramError` with the failing op's `index`), `source()`, `document()`, `render()`, `prompt()`, `brief()`, `outline()`, `markdown()`, `selection(ids)`, `diff(base)`, `expandPrompt()`; one-shot `parse`, `render`, `convert`, `spec`, `jsonSchema`, `version`, `mindmapPrompt`, `replyOutline`. Objects in and out, camelCase options; `Op` is a typed union of every op. `make test` runs its tests once `js/pkg` is built. Marked private for now: the npm name `diagram` is taken.
- The WASM `markdown(id)` returns an empty string for an unknown id again.

## 0.7.0 — 2026-10-12

- **Edits keep the file as written.** Ops, the editor, `diagram apply` (also a whole ```diagram reply), `comment`, `reply` and `resolve` rewrite only the statements whose elements changed, put new statements next to their kind (a box inside its section, an arrow after the last arrow, a comment after the comments) and leave every other line, blank line and `#` note alone. A chain like `web, mobile -> cdn -> gw` is split only when one of its arrows changes; a renamed box keeps its line; a changed mindmap or gantt chart is rewritten as a block. The result is read back before it is written, and the whole file is reprinted only when the edit can't be placed (e.g. moving a section into another one). A 6-op edit of a 60-line board now changes 10 lines instead of 40. `diagram fmt` still reprints everything.

## 0.6.0 — 2026-10-12

- **Milestones:** `[milestone]` makes a task a point in time, drawn as a diamond on its day (right-click → *Make it a milestone*).
- **Progress:** `done: 60%` (or `done`) shades the part of the bar that is done and shows `5d · 60%` / `5d ✓` (right-click → *Progress…*).
- **Owners:** `owner: ana` shows next to the task's name (right-click → *Owner…*).
- **Today:** a red line marks today on dated charts in the editor and its exports (`--today` for `diagram svg|png|html`).
- **Dependencies between charts:** a task can wait for a task in another chart (draw an arrow between them, or `after:` its id); the line is dashed and runs along the row line, clear of the task names. Both charts need dates.
- **Task names are per chart:** `after: design` means this chart's Design, even when another chart's Design took the id `design` (it used to link the other chart's task). `chart/task` names a task of another chart when this one has one of the same name.
- Prompts list milestones, progress and owners: `Build: 2026-10-12 → 2026-10-16, 5d, 60% done, owner ana`. The ops reference for models now covers gantt charts.

## 0.5.0 — 2026-10-12

- **Links pull in what they point at:** in *Copy as prompt* (and `outline --md`, `markdown()`), a block with `link: #id` reads "label (see “Target”)", and the target (swimlanes, a gantt chart, a section, a mindmap or a single block with its arrows) follows as a section headed "— linked from “label”". Links inside it are followed too (up to 8 blocks), and nothing is repeated. Web links stay Markdown links.
- **Swimlanes as numbered steps:** prompts list the steps in the order the flow runs, each with its lane; decisions are marked, and where a step does not just lead to the next one its arrows name the step they go to (`yes → 5. Confirm order`).
- **Gantt tasks with end dates** in prompts: `Design: 2026-10-12 → 2026-10-16, 5d`.
- **Shorter brief:** `diagram brief` is now the title and counts, gantt dates and swimlane steps (what the source doesn't spell out), then the source with every id and comment. It used to list every node and arrow and then repeat them in the source (about half the size now on an architecture board), and it ignored gantt charts.

## 0.4.2 — 2026-10-12

- **Colour from the right-click menu:** *Colour…* on a gantt task (its bar), a box, an arrow, a section or a lane, and *Branch colour…* on a mindmap topic. Until now colours could only be set in the hidden inspector panel or in the file.

## 0.4.1 — 2026-10-12

- **Gantt drag preview:** while you drag a bar, it snaps to where it will land and the tasks after it move along; a label above it shows its dates (`Sat 17 Oct – Wed 21 Oct · 5 days`). Dragging the end of a bar shows the same label. Dragging a bar out of its chart shows it leaving.

## 0.4.0 — 2026-10-12

- **Gantt scale:** `scale: day | week | month | quarter` on a chart (right-click it → *Scale…*). Lines and dates at each day, Monday, month or quarter; month and quarter labels carry the year when it changes. Durations also take `1m` (30 days) and `1q` (91 days), and are written that way when they divide evenly.
- **Task durations in the editor:** drag the end of a bar (snaps to a day; a week on the month scale, 30 days on the quarter scale), or right-click → *Duration…* and type `5d`, `2w`, `1m`, `1q`. *Duration…* replaces *Longer* / *Shorter*.
- **Dependencies in the editor:** draw an arrow from one bar to another (the dot past the end of a bar, or Connect X) and the second task starts when the first ends (`after:`), instead of an arrow. Loops are refused. Right-click a dependency line → *Remove dependency*, or a task → *Starts after…* to tick the tasks it waits for.
- **Links between elements:** right-click a block → *Link to…*, then click the element it should take you to (`link: #id`). Linked blocks get a link badge; clicking it pans and zooms to the target and selects it (a web link opens in a new tab). *Remove link* takes it off. Exports keep web links clickable.

## 0.3.3 — 2026-10-12

- The open file's name sits at the right of the top bar, next to *Saved* (`notes.diagram · Saved`).

## 0.3.2 — 2026-10-12

- **Move swimlanes and gantt charts:** drag them by their frame, title or a lane (or nudge with the arrow keys); they keep their place. Saved as `x`/`y` on the `lanes` or `gantt` line, which the `move`, `nudge` and `unpin` ops now handle. Inside, they are still laid out automatically.
- **Deleting a section deletes what is in it:** Delete on a section, swimlanes, lane or gantt chart removes it with its blocks, lanes and tasks, and the arrows to them; right-click → *Delete* does the same. *Remove section* / *Remove lane* / *Remove chart* still keep the blocks. *Remove swimlanes* now removes the lanes too instead of leaving them as sections.
- **Fixed:** removing a group with its contents failed when it held a mindmap, and left arrows to the lanes inside it dangling.

## 0.3.1 — 2026-10-12

- **Copy any selection as a prompt:** right-click a block, section, swimlanes, gantt chart or a multi-selection → *Copy as prompt* (or `⌘⇧C`). Markdown with the board's title, the blocks as bullets under their sections' headings (gantt tasks with dates and durations), then the arrows between them and their open comments. Right-clicking a multi-selection also offers *Put in a section* and *Delete*. Core: `prompt::selection`; WASM: `selection_markdown(ids)`.
- **Mindmap prompts:** main branches are `##` sections only when every one of them has topics or a detail under it; otherwise the map is one nested list under the title (a shopping list stays a list).
- New mindmaps keep opening to the right.

## 0.3.0 — 2026-10-12

Flowcharts, swimlanes, gantt charts and comments on the board; a simpler editor that looks like schema; tidier mindmaps.

- **Comments:** C (or the Comment button) is comment mode, with a speech-bubble cursor. Click a block to comment on it, or anywhere on the canvas for a free comment; a comment box opens where you clicked. Pins open their thread in a popover: reply, resolve, delete.
- **Flowcharts:** a Decision tool (D, a diamond) next to Box. Right-click a box → Rename, *Shape…* (all 13 shapes), Delete; right-click an arrow → label, dashed / solid, reverse, delete. Connect moved to X. `update_edge` with new ends keeps an automatic id (`a->b`) in step, comments included.
- **Swimlanes:** `lanes id: Title { lane a: A { … } … }` (a group with `"lanes": true`). Lanes are bands (rows when the flow runs right, columns when it runs down) with their titles at the side; everything in them is ranked together, so the flow lines up across lanes. In the editor: the Swimlanes button (selected blocks go in the first lane), drop a block into a lane to move it there, right-click a lane to rename it, add one after it or remove it.
- **Gantt charts:** `gantt id: Title [start: 2026-10-12] { Design [5d]  Build [2w, after: design] … }` (a group with `"gantt": true`; tasks are nodes with `start`, `days`, `after`). Drawn as a timeline: week dates, task names, bars and dependency arrows. In the editor: the Gantt button (a chart starting today), drag a bar to reschedule it, right-click a task to make it longer or shorter or add one after it.
- **Examples:** `examples/swimlanes.diagram`, `examples/gantt.diagram`.

- **Layout:** every level of a side lines up in one column (one row in org charts), so sub-topics sit at the same distance whatever the width of their branch, and each fan of sub-topics is centred on the branch it grows from. Subtrees reserve only the space they use above and below, so maps stay compact.
- **Placed by hand:** drop a topic on empty canvas and it stays there (saved as `x`/`y` relative to the map's centre); its branches follow it and grow away from the centre, and its siblings close up. The `move` op now works on topics.
- **Reset:** *Auto-place* (one topic) and *Reset arrangement* (the whole map) in the topic inspector put topics back in the automatic layout; *Tidy up* still resets everything.
- **Editor, simpler:** the code pane and the inspector / comments panel are off for now (`code: true` / `panel: true`, or `?sidebars` in the local editor, bring them back). The Prompt and Compare buttons are gone from the toolbar.
- **Floating tool bar** at the bottom centre: Select (V) and Hand (H), then the blocks: Mindmap, Box, Sticky, Connect.
- **Context menus:** right-click a mindmap topic → *Copy as prompt* copies that topic and everything under it as Markdown.
- **Title:** double-click the board's title on the canvas to rename it.
- **Design aligned with schema:** schema's colours, control sizes, radii, menus, toasts, zoom control and top bar, in the editor and in rendered SVG / PNG (canvas, cards, edges, diff colours, comment pins).
- **Sections:** the Section button in the tool bar (or ⌘G) puts the selected blocks in a titled section (a `group`); drop a block into a section to add it, drag it out to take it out; right-click a section to rename or remove it.
- **No title on the board:** `title:` is metadata now (briefs, export names). It is not drawn; `--title` draws it in exports. New files start without one.
- **Mindmaps:** right-click a topic → *Add parent* (⇧Tab on the centre): a new topic above it; above the centre it becomes the new centre. Branches are spaced symmetrically, so a middle branch runs straight out.
- **Fixed:** an undo within the autosave delay was overwritten by the pending save.
- The logo no longer links to the repo.
- `diagram` with no FILE starts a new `untitled.diagram` (`untitled-2.diagram`, … when taken) instead of reopening `diagram.diagram`.

## 0.2.0 — 2026-10-11

Mindmaps, as the first FigJam-style building block of the board.

- **Language:** `mindmap id: Topic [layout: right|left|both|down] { …indented outline… }`. Topics are ordinary nodes (`parent` in JSON), so comments, ops, diffs and cross-links work on them. Folding (`[collapsed]`), branch colours, Markdown bullets in the outline.
- **Layout and look:** tidy-tree layout per map, balanced (`both`) and org-chart (`down`) maps. The centre is a pill, main branches are outlined cards, sub-topics sit on coloured underlines and curved branches. A map is one block on the board, next to boxes and stickies.
- **Editor:** T places a map; Tab / Enter / ⇧Tab / F2 / arrows / ⌥↑↓ / . to outline by keyboard; + and fold controls; drag a topic to reparent or reorder; paste a Markdown list; copy a branch as an outline (⌘C) or a map as a Markdown prompt.
- **AI with few tokens:** ✦ Grow this topic and ✦ Mindmap about… send an outline-sized prompt and read an outline back. Pluggable: `ai(prompt)` option in the editor, `--ai CMD` / `$DIAGRAM_AI` for the CLI and the local server (default `claude -p --model haiku` when installed); without a harness the prompt is shown to copy.
- **Ops:** `add_outline`; `parent` / `index` / `mindmap` / `collapsed` on `add_node` and `update_node`; removing a topic removes its branch (`keep_members` lifts it).
- **CLI:** `diagram outline FILE [ID] [--md]`, `diagram expand FILE ID [--ask …]` and `--new TOPIC`.
- **Mermaid:** `mindmap` diagrams import and export.
- New agent skill `diagram-mindmap`.

## 0.1.1 — 2026-10-11

Visual polish from the stress test: group title pills, dotted canvas background, soft shadows, refreshed theme colours. Fixed invalid SVG (duplicate `stroke-width`) when diffing added or changed groups. Stress-test examples and report in `examples/stress/`.

## 0.1.0 — 2026-10-07

First release: DSL, JSON and Mermaid formats, ops API, layered/grid layout with groups, SVG/PNG/HTML export, browser editor with comments, comment-to-prompt and reply application, git-aware diffs, embed bundle, Rust crate and agent skills.
