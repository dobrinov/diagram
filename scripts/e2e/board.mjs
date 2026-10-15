// End-to-end test of working with a big board in a real browser: a section
// dragged by its frame keeps its place (x/y on the section), ⌘F finds and
// selects, Present steps through the sections in reading order.
//   make e2e   (or: cd scripts/e2e && node board.mjs)
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
const port = 7500 + Math.floor(Math.random() * 100); // clear of the ports browsers refuse (6566, 6665–6669, …)
const dir = mkdtempSync(join(tmpdir(), "diagram-e2e-"));
const file = join(dir, "board.diagram");
writeFileSync(file, "title: Board\ngroup a: Alpha {\n  one: One\n  two: Two\n}\ngroup b: Beta {\n  three: Retry loop\n}\ngroup c: Gamma {\n  four: Four\n}\none -> two\n");
const server = spawn(bin, [file, "--no-open", "--port", String(port)], { env: { ...process.env, DIAGRAM_WEB_DIR: join(root, "web") }, stdio: "ignore" });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const saved = () => readFileSync(file, "utf8");
const MOD = process.platform === "darwin" ? "Meta" : "Control";
let failed = false;
try {
  await sleep(600);
  const browser = await chromium.launch(existsSync(chrome) ? { executablePath: chrome } : {});
  const p = await browser.newPage({ viewport: { width: 1200, height: 700 } });
  const errors = [];
  p.on("pageerror", (e) => errors.push(e.message));
  await p.goto(`http://127.0.0.1:${port}/`, { waitUntil: "domcontentloaded" });
  await p.waitForSelector(".dge-canvas svg"); await sleep(300);

  // tooltips: a tool's name, its shortcut and what it does
  await p.locator('button[data-tool="select"]').hover(); await sleep(500);
  assert.equal(await p.locator(".dge-tip .dge-tip-head").textContent(), "SelectV");
  assert.match(await p.locator(".dge-tip .dge-tip-more").textContent(), /^Click or drag a box around things/);
  await p.mouse.move(600, 300); await sleep(100);
  assert.equal(await p.locator(".dge-tip").isHidden(), true);

  // drag a section by its frame: it keeps its place, its boxes stay unpinned
  const g = await p.locator('.dg-group[data-id="a"]').boundingBox();
  await p.mouse.move(g.x + 20, g.y + 14); await p.mouse.down();
  await p.mouse.move(g.x + 140, g.y + 80, { steps: 6 }); await p.mouse.up();
  await sleep(600);
  assert.match(saved(), /group a: Alpha \[x: -?\d+, y: -?\d+\] \{\n  one: One\n  two: Two\n\}/, saved());

  // ⌘F: find, Enter selects the match
  await p.keyboard.press(MOD + "+f");
  await p.keyboard.type("retry"); await p.keyboard.press("Enter"); await sleep(300);
  assert.equal(await p.locator(".dge-find-count").textContent(), "1 of 1");
  assert.equal(await p.locator('.dg-node.sel[data-id="three"]').count(), 1);
  await p.keyboard.press("Escape");
  assert.equal(await p.locator(".dge-find").count(), 0);

  // Present: one section at a time, toolbars hidden; Esc ends
  await p.locator('button[title^="Present"]').click(); await sleep(300);
  assert.match(await p.locator(".dge-present-bar span").textContent(), /^1 \/ 3 · /);
  assert.equal(await p.locator(".dge-toolbar").isVisible(), false);
  await p.keyboard.press("ArrowRight"); await sleep(200);
  assert.match(await p.locator(".dge-present-bar span").textContent(), /^2 \/ 3 · /);
  await p.keyboard.press("Escape"); await sleep(200);
  assert.equal(await p.locator(".dge-present-bar").count(), 0);
  assert.equal(await p.locator(".dge-toolbar").isVisible(), true);
  assert.deepEqual(errors, []);
  await browser.close();
  console.log("ok — board e2e passed");
} catch (e) {
  failed = true;
  console.error(e.message || e);
  console.error("\nfile:\n" + saved());
} finally {
  server.kill();
}
process.exit(failed ? 1 : 0);
