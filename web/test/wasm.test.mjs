// Exercises the WASM API the editor and the embed library use.
// Run with `node --test web/test/` after `make wasm`.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const pkg = new URL("../pkg/", import.meta.url);
// the bundle is a classic script defining `wasm_bindgen`; evaluate it globally
(0, eval)(readFileSync(new URL("diagram_wasm.js", pkg), "utf8") + "\nglobalThis.wasm_bindgen = wasm_bindgen;");
await globalThis.wasm_bindgen({ module_or_path: new Uint8Array(readFileSync(new URL("diagram_wasm_bg.wasm", pkg))) });
const w = globalThis.wasm_bindgen;

test("parses, renders and reports errors", () => {
  const d = new w.Diagram();
  const ok = JSON.parse(d.set_source("api: Orders API\napi -> db: SQL"));
  assert.equal(ok.ok, true);
  const r = JSON.parse(d.render("{}"));
  assert.match(r.svg, /^<svg/);
  assert.equal(r.nodes.length, 2);
  assert.equal(r.edges[0].id, "api->db");
  const bad = JSON.parse(d.set_source("a [shape: blob]"));
  assert.equal(bad.ok, false);
  assert.equal(bad.errors[0].line, 1);
  d.free();
});

test("ops, comments and prompts", () => {
  const d = new w.Diagram();
  d.set_source("a -> b");
  const res = JSON.parse(d.apply(JSON.stringify({ ops: [
    { op: "add_comment", on: "a", text: "rename this?", author: "ana" },
    { op: "add_node", label: "Cache", shape: "cylinder" },
  ] })));
  assert.equal(res.ok, true);
  assert.deepEqual(res.created, ["c1", "cache"]);
  assert.match(d.source("dsl"), /comment c1 on a by ana: rename this\?/);
  const p = d.prompt(JSON.stringify({ target: "code" }));
  assert.match(p, /rename this\?/);
  assert.match(d.brief(), /## Source\n\n```diagram\n[^]*comment c1 on a by ana: rename this\?/);
  const fail = JSON.parse(d.apply('{"ops":[{"op":"remove","id":"nope"}]}'));
  assert.equal(fail.ok, false);
  d.free();
});

test("one-shot helpers", () => {
  assert.equal(JSON.parse(w.convert("flowchart LR\n A --> B", "dsl")).text, "A -> B\n");
  assert.match(JSON.parse(w.render("x", "{}")).svg, /data-id="x"/);
  assert.equal(JSON.parse(w.json_schema()).title, "diagram document");
  assert.match(w.spec(), /# The diagram language/);
});

test("mindmaps: outline in, outline and markdown out, prompts", () => {
  const d = new w.Diagram();
  const ok = JSON.parse(d.set_source("mindmap m: Trip {\n  Food\n    Nata\n}"));
  assert.equal(ok.ok, true);
  const r = JSON.parse(d.render("{}"));
  const food = r.nodes.find((n) => n.id === "food");
  assert.equal(food.map.parent, "m");
  assert.equal(food.map.depth, 1);
  const res = JSON.parse(d.apply(JSON.stringify({ ops: [{ op: "add_outline", parent: "food", outline: w.reply_outline("```\n- Pasta\n  - Carbonara\n```") }] })));
  assert.deepEqual(res.created, ["pasta", "carbonara"]);
  assert.equal(d.outline("m"), "Trip\n  Food\n    Nata\n    Pasta\n      Carbonara\n");
  assert.match(d.markdown("m"), /^# Trip\n\n## Food\n/);
  assert.match(JSON.parse(d.expand_prompt("food", "{}")).prompt, /Topic to grow: Trip > Food/);
  assert.match(w.mindmap_prompt("Lisbon", "{}"), /Make a mindmap about: Lisbon/);
  d.free();
});
