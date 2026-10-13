// Tests of the js/ package. Needs js/pkg: run `make js` first.
import { test } from "node:test";
import assert from "node:assert/strict";
import * as diagram from "../index.mjs";
import { init, Board, DiagramError, parse, render, convert, spec, jsonSchema, version, mindmapPrompt, replyOutline } from "../index.mjs";

test("everything needs init() first, and init() is idempotent", async () => {
  // this runs first: nothing has loaded the WASM yet in this process
  assert.throws(() => version(), /await init\(\)/);
  assert.throws(() => Board.empty(), /await init\(\)/);
  assert.throws(() => parse("a"), /await init\(\)/);
  await Promise.all([init(), init()]);
  await init();
  assert.match(version(), /^\d+\.\d+\.\d+/);
});

test("parse: DSL, JSON and Mermaid", () => {
  const doc = parse("api: Orders API\napi -> db: SQL");
  assert.deepEqual(doc.nodes.map((n) => n.id), ["api", "db"]);
  assert.equal(doc.nodes[0].label, "Orders API");
  assert.deepEqual(doc.edges[0], { id: "api->db", from: "api", to: "db", label: "SQL" });
  assert.equal(parse(JSON.stringify(doc)).edges[0].id, "api->db");
  assert.deepEqual(parse("flowchart LR\n  A --> B").nodes.map((n) => n.id), ["A", "B"]);
});

test("parse errors carry lines", () => {
  assert.throws(() => Board.parse("a\nb [shape: blob]"), (e) => {
    assert.ok(e instanceof DiagramError);
    assert.equal(e.name, "DiagramError");
    assert.equal(e.errors[0].line, 2);
    assert.match(e.errors[0].message, /unknown shape `blob`/);
    assert.match(e.message, /line 2/);
    return true;
  });
  assert.throws(() => parse('{"nodes": [{"id": "a"}], "edges": [{"from": "a", "to": "x", "id": "a"}]}'), DiagramError);
});

test("render: svg and geometry", () => {
  const b = Board.parse("api -> db\ncomment on api: cache?");
  const r = b.render({ theme: "dark", comments: "all" });
  assert.match(r.svg, /^<svg/);
  assert.deepEqual(Object.keys(r).sort(), ["canvas", "edges", "groups", "nodes", "pins", "svg", "view"]);
  const api = r.nodes.find((n) => n.id === "api");
  assert.equal(typeof api.x, "number");
  assert.equal(typeof api.w, "number");
  assert.equal(r.edges[0].id, "api->db");
  assert.match(r.edges[0].d, /^M/);
  assert.equal(r.pins[0].id, "c1");
  assert.match(render("x").svg, /data-id="x"/);
  assert.throws(() => render("x [shape: blob]"), (e) => e instanceof DiagramError && e.errors[0].line === 1);
  assert.throws(() => b.render({ theme: "sepia" }), DiagramError);
  b.free();
});

test("apply: arrays, {ops}, one op; created ids; source formats", () => {
  const b = Board.empty();
  assert.deepEqual(b.apply([{ op: "add_node", label: "Orders API" }, { op: "add_node", id: "db", shape: "cylinder" }]), {
    created: ["orders-api", "db"],
    count: 2,
  });
  b.apply({ ops: [{ op: "add_edge", from: "orders-api", to: "db", label: "SQL" }] });
  b.apply({ op: "set", key: "title", value: "Orders" });
  assert.equal(b.source(), 'title: Orders\n\norders-api: Orders API\ndb [cylinder]\n\norders-api -> db: SQL\n');
  assert.equal(b.document().title, "Orders");
  assert.match(b.source("mermaid"), /^flowchart/m);
  assert.equal(JSON.parse(b.source("json")).nodes.length, 2);
  assert.match(b.source("brief"), /Orders API/);
  assert.match(b.source("svg"), /^<svg/);
  assert.throws(() => b.source("xml"), /unknown format `xml`/);
  b.free();
});

test("apply: a model's reply with a ```json block", () => {
  const b = Board.parse("api -> db\ncomment on api by ana: add a cache");
  const reply = [
    "Sure! Here are the changes:",
    "",
    "```json",
    '{"ops": [',
    '  {"op": "add_node", "id": "cache", "label": "Redis", "shape": "cylinder"},',
    '  {"op": "add_edge", "from": "api", "to": "cache", "label": "reads"},',
    '  {"op": "resolve_comment", "id": "c1", "resolution": "Added Redis"}',
    "]}",
    "```",
    "",
    "Let me know if you need anything else.",
  ].join("\n");
  const r = b.apply(reply);
  assert.deepEqual(r.created, ["cache", "api->cache"]);
  assert.equal(r.count, 3);
  const doc = b.document();
  assert.equal(doc.comments[0].status, "resolved");
  assert.equal(doc.comments[0].resolution, "Added Redis");
  b.free();
});

test("a failed apply throws with the op index and leaves the board unchanged", () => {
  const b = Board.parse("a -> b");
  const before = b.source();
  assert.throws(
    () => b.apply([{ op: "add_node", id: "c" }, { op: "remove", id: "nope" }]),
    (e) => e instanceof DiagramError && e.index === 1 && /nope/.test(e.message),
  );
  assert.equal(b.source(), before);
  assert.throws(() => b.apply("no json here"), (e) => e instanceof DiagramError && e.index === undefined);
  assert.throws(() => b.apply([{ op: "frobnicate" }]), DiagramError);
  assert.throws(() => b.apply(42), TypeError);
  assert.equal(b.source(), before);
  b.free();
});

test("convert", () => {
  assert.equal(convert("flowchart LR\n A --> B", "dsl"), "A -> B\n");
  assert.match(convert("a -> b", "mermaid"), /a --> b/);
  assert.throws(() => convert("a", "xml"), (e) => e instanceof DiagramError && /unknown format/.test(e.message));
  assert.throws(() => convert("a [shape: blob]", "json"), (e) => e.errors[0].line === 1);
});

test("prompts, brief and selection", () => {
  const b = Board.parse("api -> db\ncomment on api by ana: rename this?\ncomment on db: done [resolved]");
  const p = b.prompt({ target: "code" });
  assert.match(p, /rename this\?/);
  assert.match(p, /## Comments to address \(1\)/);
  assert.match(b.prompt({ includeResolved: true }), /## Comments to address \(2\)[^]*\*\*c2\*\* on node `db` \(resolved\)/);
  assert.match(b.prompt({ instructions: "Be brief." }), /Be brief\./);
  assert.throws(() => b.prompt({ target: "nope" }), /invalid options/);
  assert.match(b.brief(), /## Source\n\n```diagram\n[^]*comment c1 on api by ana: rename this\?/);
  assert.match(b.selection(["api"]), /rename this\?/);
  b.free();
});

test("mindmaps: outline, markdown, expand and generate prompts", () => {
  const b = Board.parse("mindmap m: Trip {\n  Food\n    Nata\n}");
  const food = b.render().nodes.find((n) => n.id === "food");
  assert.equal(food.map.parent, "m");
  assert.equal(food.map.depth, 1);
  const outline = replyOutline("```\n- Pasta\n  - Carbonara\n```");
  assert.equal(outline, "Pasta\n  Carbonara\n");
  assert.deepEqual(b.apply([{ op: "add_outline", parent: "food", outline }]).created, ["pasta", "carbonara"]);
  assert.equal(b.outline("m"), "Trip\n  Food\n    Nata\n    Pasta\n      Carbonara\n");
  assert.match(b.markdown("m"), /^# Trip\n\n## Food\n/);
  assert.match(b.selection(["m"]), /^# Trip/);
  assert.throws(() => b.outline("nope"), DiagramError);
  assert.throws(() => b.markdown("nope"), DiagramError);
  assert.match(b.expandPrompt("food", { count: 3, instructions: "desserts" }), /Topic to grow: Trip > Food/);
  assert.throws(() => b.expandPrompt("nope"), /no node `nope`/);
  assert.match(mindmapPrompt("Lisbon", { levels: 2 }), /Make a mindmap about: Lisbon/);
  b.free();
});

test("diff against a base", () => {
  const b = Board.parse("a -> b");
  assert.deepEqual(b.diff(), { entries: [] });
  b.apply([{ op: "add_node", id: "c" }, { op: "update_node", id: "a", label: "Alpha" }]);
  const d = b.diff("a -> b");
  assert.deepEqual(d.entries.find((e) => e.id === "c"), { kind: "node", id: "c", change: "added" });
  assert.deepEqual(d.entries.find((e) => e.id === "a").fields, [["label", "a", "Alpha"]]);
  assert.match(b.diffMarkdown(), /\+ node `c`/);
  assert.match(b.render().svg, /<svg/);
  assert.throws(() => b.diff("x [shape: blob]"), DiagramError);
  assert.deepEqual(b.diff(null), { entries: [] });
  b.free();
});

test("free, dispose and the static helpers", () => {
  const b = Board.empty();
  b.free();
  b.free(); // twice is fine
  assert.throws(() => b.source(), /freed/);
  if (typeof Symbol.dispose === "symbol") {
    const c = Board.parse("a");
    c[Symbol.dispose]();
    assert.throws(() => c.document(), /freed/);
  }
  assert.match(spec(), /# The diagram language/);
  assert.equal(jsonSchema().title, "diagram document");
  assert.equal(typeof diagram.initSync, "function");
});

test("gantt fields, today, and edits that keep the file as written", () => {
  const b = Board.parse("# plan\ngantt g: Plan [start: 2026-10-12] {\n  Build [5d]\n}\n\nnote: Ask ops\n");
  b.apply([{ op: "update_node", id: "build", done: 60, owner: "ana" }, { op: "add_node", id: "ga", label: "GA", group: "g", milestone: true, after: ["build"] }]);
  assert.equal(b.source(), "# plan\ngantt g: Plan [start: 2026-10-12] {\n  Build [5d, done: 60%, owner: ana]\n  GA [after: build, milestone]\n}\n\nnote: Ask ops\n");
  const r = b.render({ today: "2026-10-14" });
  assert.match(r.svg, /dg-today/);
  assert.match(b.selection(["g"]), /- GA: 2026-10-17, milestone, after Build\n/);
  b.free();
});

test("tools for agents: query, select, merge, focus, suggestions", () => {
  const b = Board.parse("web: Web\ngroup back: Backend {\n  api: Orders API\n  db: Postgres\n}\nweb -> api\napi -> db\n");
  assert.deepEqual(b.select("near:api"), ["web", "api", "db", "web->api", "api->db"]);
  assert.match(b.query("group:back"), /group back: Backend \{\n  api: Orders API\n  db: Postgres\n\}\n\napi -> db\n/);
  assert.throws(() => b.query("label~nothing"), DiagramError);
  assert.deepEqual(b.merge("group back {\n  cache: Redis\n}\napi -> cache\napi [red]").created, ["cache", "api->cache"]);
  assert.match(b.source(), /  db: Postgres\n  cache: Redis\n\}/);
  const full = b.render().view, crop = b.render({ focus: ["db"] }).view;
  assert.ok(crop.w < full.w);
  assert.throws(() => b.apply({ op: "update_node", id: "dbb", color: "red" }), /did you mean `db`/);
  b.free();
});

test("risk matrices", () => {
  const b = Board.parse("risks r: Risks {\n  Contract slips [p: 4, i: 4, detail: \"Legal says 4 weeks\"]\n}\n");
  b.apply([{ op: "add_node", id: "creep", label: "Scope creep", group: "r", probability: 2, impact: 3 }]);
  const r = b.render();
  assert.equal(r.groups[0].matrix.levels, 5);
  assert.ok(r.nodes.every((n) => n.risk));
  assert.match(r.svg, /<title>Contract slips\nProbability 4 × impact 4 = 16 \(critical\)\n\nLegal says 4 weeks<\/title>/);
  assert.match(b.selection(["r"]), /- Scope creep: probability 2 × impact 3 = 6 \(medium\)/);
  b.free();
});

test("gantt forecasts: schedule, prompts, render", () => {
  const b = Board.parse("gantt a: A [start: 2026-10-12] {\n  Build [10d, done: 20%]\n  Ship [after: build, milestone]\n}\n");
  const s = b.schedule({ today: "2026-10-20" });
  const ship = s.charts[0].tasks.find((t) => t.id === "ship");
  assert.deepEqual([ship.start, ship.forecast_start, ship.slip, ship.status], ["2026-10-22", "2026-10-28", 6, "pushed back"]);
  assert.match(b.selection(["a"], { today: "2026-10-20" }), /- Ship: 2026-10-22, milestone, after Build, forecast 2026-10-28 \(\+6d, pushed back\)/);
  assert.match(b.brief({ today: new Date(2026, 9, 20) }), /forecast end 2026-10-27 \(\+6d, behind\)/);
  assert.match(b.render({ today: "2026-10-20" }).svg, /dg-slip/);
  b.free();
});

test("charts", () => {
  const b = Board.parse("chart rev: Revenue [bar, unit: k€] {\n  quarter | EU | US\n  Q1 | 120 | 80\n}\n");
  b.apply([{ op: "add_node", id: "rev.q2", label: "Q2", group: "rev", values: [150, null] }]);
  assert.match(b.source(), /  Q2 \| 150 \| -\n/);
  const r = b.render();
  assert.equal(r.groups[0].chart, "bar");
  assert.ok(r.nodes.every((n) => n.datum));
  assert.match(b.selection(["rev"]), /\| quarter \| EU \| US \|\n\|---\|---\|---\|\n\| Q1 \| 120 \| 80 \|/);
  b.free();
});

test("tables", () => {
  const b = Board.parse("table t: Options {\n  option | cost\n  Local | $0\n}\n");
  b.apply([{ op: "add_node", id: "t.hosted", label: "Hosted", group: "t", cells: ["$1k"], index: 0 }]);
  assert.match(b.source(), /  option \| cost\n  Hosted \| \$1k\n  Local  \| \$0\n/);
  assert.match(b.selection(["t"]), /\| option \| cost \|\n\|---\|---\|\n\| Hosted \| \$1k \|/);
  assert.ok(b.render().groups[0].table);
  b.free();
});

test("themes", () => {
  const b = Board.parse("theme: ./brand.theme\napi: API [blue]\n");
  b.addThemeFile("./brand.theme", "theme brand {\n  blue: #3b6fd8\n  dark {\n    background: #101418\n  }\n}\n");
  const r = b.render({ theme: "dark" });
  assert.deepEqual([r.canvas.background, r.canvas.dark], ["#101418", true]);
  assert.match(r.svg, /#3b6fd8/);
  b.apply({ op: "theme", source: "theme own {\n  text: #222222\n}" });
  b.apply({ op: "set", key: "theme", value: "own" });
  assert.deepEqual(b.themes().board, ["own"]);
  assert.match(b.source(), /theme: own\n[^]*theme own \{\n  text: #222222\n\}/);
  assert.ok(b.themes().presets.includes("blueprint"));
  b.free();
});

test("ask prompts", async () => {
  const { askAgain } = await import("../index.mjs");
  const b = Board.parse("api: Orders API\ndb: Postgres\nmail: Mail\napi -> db\n");
  const p = b.askPrompt("Add a replica", ["db"]);
  assert.match(p, /Selected: `db`/);
  assert.doesNotMatch(p, /mail: Mail/);
  assert.match(askAgain(p, "{oops", "bad JSON"), /Error: bad JSON/);
  b.free();
});
