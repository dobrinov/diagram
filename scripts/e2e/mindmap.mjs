// End-to-end test of mindmap editing in a real browser.
//   make e2e   (or: cd scripts/e2e && npm install && node mindmap.mjs)
// Starts `diagram` on a scratch file with web/ served from disk and a fake AI
// harness, types an outline, navigates, reorders, folds, drags, grows a topic
// with "AI", copies a branch and then a selection as a prompt from the context menu, pastes a list, places a topic by hand and resets it, pans with the hand tool, adds a parent above a centre, wraps a map in a section and removes it, places a decision and reshapes it, puts a map in swimlanes, makes a gantt chart, comments on a topic and resolves it, then checks
// the saved file.
import { chromium } from "playwright";
import { spawn } from "node:child_process";
import { mkdtempSync, writeFileSync, readFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..", "..");
const bin = join(root, "target", "debug", "diagram");
const chrome = process.env.CHROME || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const port = 5790 + Math.floor(Math.random() * 100);
const dir = mkdtempSync(join(tmpdir(), "diagram-e2e-"));
const file = join(dir, "board.diagram");
writeFileSync(file, "title: Test board\n");

const server = spawn(bin, [file, "--no-open", "--port", String(port), "--ai", join(here, "fake-ai.sh")], {
  env: { ...process.env, DIAGRAM_WEB_DIR: join(root, "web") },
  stdio: "ignore",
});
const MOD = process.platform === "darwin" ? "Meta" : "Control";
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const saved = () => readFileSync(file, "utf8");
let failed = false;
try {
  await sleep(600);
  const browser = await chromium.launch(existsSync(chrome) ? { executablePath: chrome } : {});
  const p = await browser.newPage({ viewport: { width: 1400, height: 850 } });
  const errors = [];
  p.on("pageerror", (e) => errors.push(e.message));
  p.on("console", (m) => m.type() === "error" && errors.push(m.text()));
  await p.goto(`http://127.0.0.1:${port}/?sidebars`, { waitUntil: "domcontentloaded" });
  await p.waitForSelector(".dge-canvas svg");
  const key = async (k) => { await p.keyboard.press(k); await sleep(60); };
  const type = (t) => p.keyboard.type(t);
  const topic = (id) => p.locator(`.dg-node[data-id="${id}"]`);

  // type an outline
  await p.locator(".dge-canvas").click({ position: { x: 300, y: 300 } });
  await key("t");
  await p.locator(".dge-canvas").click({ position: { x: 400, y: 400 } });
  await p.waitForSelector(".dge-inline");
  await type("Launch plan"); await key("Enter");
  await key("Tab"); await type("Discovery"); await key("Enter");
  await type("Build"); await key("Enter");
  await type("Ship"); await key("Enter");
  await key("Escape"); // an empty new topic is dropped
  await topic("discovery").click();
  await key("Tab"); await type("Interviews"); await key("Tab"); await type("Ops team"); await key("Enter");
  await type("Finance"); await key("Enter"); await key("Escape");
  await sleep(700);
  assert.match(saved(), /mindmap launch-plan: Launch plan \[x: \d+, y: \d+\] \{\n  Discovery\n    Interviews\n      Ops team\n      Finance\n  Build\n  Ship\n\}/);

  // navigate, reorder, fold
  await topic("ship").click();
  await key("ArrowUp"); await key("ArrowUp");
  assert.equal(await p.locator(".dge-inspector h3 code").textContent(), "discovery");
  await key("Alt+ArrowDown");
  await key("."); await sleep(300);
  assert.equal(await p.locator(".dg-fold").count(), 1);
  await key(".");
  await key("f"); await sleep(200);

  // drag a branch onto another topic
  const s = await topic("ship").boundingBox(), t = await topic("interviews").boundingBox();
  await p.mouse.move(s.x + s.width / 2, s.y + s.height / 2);
  await p.mouse.down();
  await p.mouse.move(s.x + 30, s.y + 30, { steps: 4 });
  await p.mouse.move(t.x + t.width / 2, t.y + t.height / 2, { steps: 8 });
  await p.mouse.up();
  await sleep(600);

  // right-click a topic: Copy as prompt takes that level down
  await p.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  await topic("discovery").click({ button: "right" });
  await p.locator(".dge-menu button", { hasText: "Copy as prompt" }).click();
  await sleep(200);
  assert.equal(await p.evaluate(() => navigator.clipboard.readText()), "# Discovery\n\n## Interviews\n\n- Ops team\n- Finance\n- Ship\n");
  assert.equal(await p.locator(".dge-menu").isHidden(), true);

  // grow a topic with the (fake) AI
  await topic("build").click();
  await p.locator(".dge-ai button").click();
  await sleep(1500);

  // paste a Markdown list onto the empty canvas: a new map below the first
  await key("Escape");
  await p.evaluate(() => {
    const dt = new DataTransfer();
    dt.setData("text/plain", "# Ideas\n- One\n- Two\n  - Two b");
    document.querySelector(".dge-canvas").focus();
    document.dispatchEvent(new ClipboardEvent("paste", { clipboardData: dt, bubbles: true }));
  });
  await sleep(800);

  // drop a topic on empty canvas: it stays there; Reset arrangement undoes it
  await key("f"); await sleep(300);
  const o = await topic("one").boundingBox();
  await p.mouse.move(o.x + o.width / 2, o.y + o.height / 2);
  await p.mouse.down();
  await p.mouse.move(o.x + 60, o.y + 60, { steps: 4 });
  await p.mouse.move(o.x + 160, o.y + 130, { steps: 8 });
  await p.mouse.up();
  await sleep(600);
  assert.match(saved(), /\n  One \[x: -?\d+, y: -?\d+\]\n/, saved());
  await p.screenshot({ path: join(dir, "placed.png") });
  await topic("one").click();
  await p.getByRole("button", { name: "Reset arrangement" }).click();
  await sleep(600);
  assert.match(saved(), /\n  One\n/, saved());

  // the hand tool (floating bar) pans: dragging over a topic leaves the file as it is
  await p.locator('.dge-tools [data-tool="hand"]').click();
  const before = saved(), hb = await topic("build").boundingBox();
  await p.mouse.move(hb.x + 5, hb.y + 5); await p.mouse.down();
  await p.mouse.move(hb.x + 80, hb.y + 40, { steps: 5 }); await p.mouse.up();
  await sleep(500);
  assert.equal(saved(), before);
  assert.equal(Math.round((await topic("build").boundingBox()).x - hb.x), 75);
  await p.locator('.dge-tools [data-tool="select"]').click();

  // add a parent above a map's centre: it becomes the new centre; Escape on an empty one undoes it
  const ideasBefore = saved();
  await topic("ideas").click({ button: "right" });
  await p.locator(".dge-menu button", { hasText: "Add parent" }).click();
  await p.waitForSelector(".dge-inline");
  await type("Brainstorm"); await key("Enter");
  await sleep(600);
  assert.match(saved(), /mindmap brainstorm: Brainstorm \[x: -?\d+, y: -?\d+\] \{\n  Ideas\n    One\n    Two\n      Two b\n\}/, saved());
  await topic("brainstorm").click();
  await key("Shift+Tab"); await p.waitForSelector(".dge-inline"); await key("Escape");
  await sleep(600);
  assert.match(saved(), /mindmap brainstorm: Brainstorm/, saved());
  // naming it and adding it are two undo steps
  await key(MOD + "+z"); await key(MOD + "+z"); await sleep(600);
  assert.equal(saved(), ideasBefore);

  // a titled section around a map (⌘G), then removed from its context menu: the map stays
  const sectionBefore = saved();
  await topic("ideas").click();
  await key(MOD + "+g");
  await p.waitForSelector(".dge-inline"); await key(MOD + "+a"); await type("Later"); await key("Enter");
  await sleep(600);
  assert.match(saved(), /group later: Later \{\n  mindmap ideas: Ideas/, saved());
  const sec = await p.locator('.dg-group[data-id="later"]').boundingBox();
  await p.mouse.click(sec.x + 12, sec.y + sec.height - 8, { button: "right" });
  await p.locator(".dge-menu button", { hasText: "Remove section" }).click();
  await sleep(600);
  assert.equal(saved(), sectionBefore);

  // flowcharts: D places a decision; right-click → Shape… changes it
  const cb = await p.locator(".dge-canvas").boundingBox(), spot = { x: cb.width - 90, y: 50 };
  await p.locator(".dge-canvas").click({ position: spot });
  await key("d");
  await p.locator(".dge-canvas").click({ position: spot });
  await p.waitForFunction(() => document.activeElement && document.activeElement.classList.contains("dge-inline"));
  await type("Approved?"); await key("Enter");
  await sleep(500);
  assert.match(saved(), /\napproved: Approved\? \[diamond, x: -?\d+, y: -?\d+\]\n/, saved());
  await topic("approved").click({ button: "right" });
  await p.locator(".dge-menu button", { hasText: "Shape…" }).click();
  await p.locator(".dge-menu button", { hasText: "Hexagon" }).click();
  await sleep(500);
  assert.match(saved(), /\napproved: Approved\? \[hexagon,/, saved());
  // two blocks selected: Copy as prompt takes both, the map with its branches
  await topic("approved").click();
  await topic("ideas").click({ modifiers: ["Shift"] });
  await topic("approved").click({ button: "right" });
  await p.locator(".dge-menu button", { hasText: "Copy as prompt" }).click();
  await sleep(200);
  const both = await p.evaluate(() => navigator.clipboard.readText());
  assert.match(both, /^# .+\n\n/, both);
  assert.match(both, /^- Approved\?$/m, both);
  assert.match(both, /^- Ideas$/m, both);
  await key("Escape");
  await topic("approved").click();
  await topic("approved").click({ button: "right" });
  await p.locator(".dge-menu button", { hasText: "Delete" }).click();
  await sleep(500);

  // swimlanes around a map: the map goes in the first lane; removing them keeps the map
  const lanesBefore = saved();
  await topic("ideas").click();
  await p.locator('.dge-tools [data-act="lanes"]').click();
  await p.waitForFunction(() => document.activeElement && document.activeElement.classList.contains("dge-inline"));
  await key(MOD + "+a"); await type("Teams"); await key("Enter");
  await sleep(600);
  assert.match(saved(), /lanes teams: Teams \{\n  lane lane: Lane 1 \{\n    mindmap ideas: Ideas/, saved());
  const lb = await p.locator('.dg-group[data-id="teams"]').boundingBox();
  await p.mouse.click(lb.x + 30, lb.y + 20, { button: "right" });
  await p.locator(".dge-menu button", { hasText: "Remove swimlanes" }).click();
  await sleep(400);
  // remove, name, create: three steps back
  await key(MOD + "+z"); await key(MOD + "+z"); await key(MOD + "+z"); await sleep(600);
  assert.equal(saved(), lanesBefore);

  // a gantt chart: two tasks starting today; Longer adds a day; removing the chart keeps the tasks
  const ganttBefore = saved();
  await key("Escape");
  await p.locator('.dge-tools [data-act="gantt"]').click();
  await p.waitForFunction(() => document.activeElement && document.activeElement.classList.contains("dge-inline"));
  await key(MOD + "+a"); await type("Roadmap"); await key("Enter");
  await sleep(600);
  assert.match(saved(), /gantt roadmap: Roadmap \[start: \d{4}-\d\d-\d\d\] \{\n  Task 1 \[5d\]\n  Task 2 \[3d, after: task-1\]\n\}/, saved());
  await key("f"); await sleep(300);
  await topic("task-1").click({ button: "right" });
  await p.locator(".dge-menu button", { hasText: "Longer" }).click();
  await sleep(500);
  assert.match(saved(), /  Task 1 \[6d\]\n/, saved());
  // create, name, longer: three steps back
  await key(MOD + "+z"); await key(MOD + "+z"); await key(MOD + "+z"); await sleep(600);
  assert.equal(saved(), ganttBefore);

  // C: comment mode; click a topic, type, Enter; then its pin opens the thread to resolve it
  await key("f"); await sleep(300);
  await key("c");
  await topic("build").click();
  await p.waitForFunction(() => document.activeElement && document.activeElement.closest(".dge-bubble"));
  await type("Too big for one sprint?"); await key("Enter");
  await sleep(600);
  assert.match(saved(), /\ncomment c1 on build by [^:]+: Too big for one sprint\?\n/, saved());
  await p.locator('.dg-pin[data-comment="c1"]').click();
  await p.locator(".dge-bubble button", { hasText: "Resolve" }).click();
  await sleep(600);
  assert.match(saved(), /Too big for one sprint\? \[resolved\]/, saved());

  await p.screenshot({ path: join(dir, "final.png") });
  await browser.close();

  const out = saved();
  assert.match(out, /  Build\n    Market size\n      SMB\n    Competitors\n    Pricing\n  Discovery\n    Interviews\n      Ops team\n      Finance\n      Ship\n\}/, out);
  assert.match(out, /mindmap ideas: Ideas \[x: \d+, y: \d+\] \{\n  One\n  Two\n    Two b\n\}/, out);
  assert.deepEqual(errors, []);
  console.log(`ok — mindmap e2e passed (screenshot: ${join(dir, "final.png")})`);
} catch (e) {
  failed = true;
  console.error(e.message || e);
  console.error(`file:\n${saved()}\nscratch dir: ${dir}`);
} finally {
  server.kill();
}
process.exit(failed ? 1 : 0);
