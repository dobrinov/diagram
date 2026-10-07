---
name: diagram-edit
description: Make precise changes to an existing .diagram (or diagram JSON / Mermaid) file — add or remove nodes and edges, regroup, restyle, rename ids — with the `diagram apply` ops API, keeping the file valid. Use when the user asks to change, update, extend or clean up an existing diagram.
---

# diagram-edit

## Steps

1. Read the file (`cat FILE`) or run `diagram brief FILE` for a structured summary with every id.
2. Small changes: edit the text directly. The format is one statement per line, so this is easy and diffs cleanly. Then run `diagram check FILE`.
3. Structural changes (renames, moving things between groups, deleting nodes together with their edges): use ops. A rename updates every edge and comment that refers to the old id.
   ```bash
   diagram apply FILE - <<'JSON'
   {"ops": [
     {"op": "rename", "id": "api", "to": "orders-api"},
     {"op": "update_node", "id": "db", "shape": "cylinder", "color": "blue"},
     {"op": "add_group", "id": "data", "label": "Data", "members": ["db", "cache"]},
     {"op": "remove", "id": "legacy"},
     {"op": "set", "key": "direction", "value": "down"}
   ]}
   JSON
   ```
   `diagram apply` prints a summary of what changed. Add `--dry-run` to preview it first.
4. Re-render any committed images, e.g. `diagram png FILE -o FILE.png`.
5. To show the user the change visually, run `diagram svg FILE --base HEAD -o /tmp/changes.svg`. It highlights additions in green, removals in red and changes in amber.

## Gotchas

- `# …` lines are notes for people. They are dropped when a tool rewrites the file. Use `comment` for anything that must survive.
- Edge ids are `from->to` (`from->to#2` for a second edge between the same pair). Get them from `diagram brief`.
- `x`/`y` pin a node. Use `{"op": "unpin"}` to hand every node back to the automatic layout.
