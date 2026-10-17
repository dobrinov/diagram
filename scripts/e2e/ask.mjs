// End-to-end test of the AI button (Add what you know, Rethink) in a real browser, with a stand-in harness
// (fake-ops.sh): the changes are shown, then kept; a reply that does not
// apply is sent back and the corrected one shown, then discarded; Rethink
// shows the model's note with its changes.
//   make e2e   (or: cd scripts/e2e && node ask.mjs)
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
const port = 6390 + Math.floor(Math.random() * 100);
const dir = mkdtempSync(join(tmpdir(), "diagram-e2e-"));
const file = join(dir, "board.diagram");
const start = "title: Shop\napi: Orders API\ndb: Postgres [cylinder]\napi -> db\n";
writeFileSync(file, start);
const server = spawn(bin, [file, "--no-open", "--port", String(port), "--ai", join(here, "fake-ops.sh")], {
  env: { ...process.env, DIAGRAM_WEB_DIR: join(root, "web") },
  stdio: "ignore",
});
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
  await p.waitForSelector(".dge-canvas svg");

  // ⌘K: ask; the changes are shown before they are kept
  await p.locator(".dge-canvas").click({ position: { x: 20, y: 20 } });
  await p.keyboard.press(MOD + "+k");
  await p.locator(".dge-dialog textarea").fill("Add a cache the API reads from");
  await p.locator(".dge-dialog button.primary").click();
  await p.waitForSelector(".dge-preview-bar");
  assert.match(await p.locator(".dge-preview-bar").textContent(), /1 op/);
  assert.equal(await p.locator('.dg-node[data-id="cache"]').count(), 1);
  await sleep(600);
  assert.equal(saved(), start, "nothing is saved before Keep");
  await p.locator(".dge-preview-bar button.primary").click();
  await sleep(700);
  assert.match(saved(), /cache: Redis \[cylinder\]/);
  assert.match(saved(), /api -> cache: reads/);
  assert.equal(await p.locator(".dge-preview-bar").count(), 0);

  // a reply that does not apply goes back with its error; the fixed one is shown; Discard
  const kept = saved();
  // the AI button in the corner opens its menu; Add what you know
  await p.locator(".dge-ai-fab").click();
  assert.equal(await p.locator(".dge-ai-menu .dge-ai-item").count(), 2);
  await p.locator(".dge-ai-item", { hasText: "Add what you know" }).click();
  await p.locator(".dge-dialog textarea").fill("Make the database red, twice");
  await p.locator(".dge-dialog button.primary").click();
  await p.waitForSelector(".dge-preview-bar");
  await p.locator(".dge-preview-bar button", { hasText: "Discard" }).click();
  await sleep(600);
  assert.equal(saved(), kept);
  assert.equal(await p.locator(".dge-preview-bar").count(), 0);

  // Rethink (from the right-click menu on the empty board): the note comes with the changes
  await p.locator(".dge-canvas").click({ position: { x: 20, y: 300 }, button: "right" });
  await p.locator(".dge-menu button, .dge-menu [role=menuitem]", { hasText: "Rethink the board" }).first().click();
  await p.waitForSelector(".dge-preview-bar");
  assert.match(await p.locator(".dge-preview-bar").textContent(), /Put the storage in its own section\. \(1 op\)/);
  await p.locator(".dge-preview-bar button.primary").click();
  await sleep(700);
  assert.match(saved(), /group storage: Storage \{\n\s+db: Postgres/);
  assert.deepEqual(errors, []);
  await browser.close();
  console.log("ok — ask e2e passed");
} catch (e) {
  failed = true;
  console.error(e.message || e);
  console.error("\nfile:\n" + saved());
} finally {
  server.kill();
}
process.exit(failed ? 1 : 0);
