# Changelog

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
