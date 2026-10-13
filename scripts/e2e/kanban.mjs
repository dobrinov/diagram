// End-to-end test of kanban boards in a real browser: the Kanban tool adds
// To do / Doing / Done, a column's menu adds a card, and dragging a card puts
// it in the column and at the place it was dropped.
//   make e2e   (or: cd scripts/e2e && node kanban.mjs)
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
const port = 7800 + Math.floor(Math.random() * 100);
const dir = mkdtempSync(join(tmpdir(), "diagram-e2e-"));
const file = join(dir, "board.diagram");
writeFileSync(file, "title: Sprint\nlogin: Login page\n");
const server = spawn(bin, [file, "--no-open", "--port", String(port)], { env: { ...process.env, DIAGRAM_WEB_DIR: join(root, "web") }, stdio: "ignore" });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const saved = () => readFileSync(file, "utf8");
let failed = false;
try {
  await sleep(600);
  const browser = await chromium.launch(existsSync(chrome) ? { executablePath: chrome } : {});
  const p = await browser.newPage({ viewport: { width: 1300, height: 760 } });
  const errors = [];
  p.on("pageerror", (e) => errors.push(e.message));
  await p.goto(`http://127.0.0.1:${port}/`, { waitUntil: "domcontentloaded" });
  await p.waitForSelector(".dge-canvas svg"); await sleep(300);

  // the selected box goes into To do
  await p.locator('.dg-node[data-id="login"]').click();
  await p.locator('[data-act="kanban"]').click(); await sleep(200);
  await p.keyboard.type("Sprint 12"); await p.keyboard.press("Enter"); await sleep(600);
  assert.match(saved(), /kanban sprint-12: Sprint 12 \{\n  column todo: To do \{\n    login: Login page\n  \}\n  column doing: Doing \{\n  \}\n  column done: Done \{\n  \}\n\}/, saved());

  // a column's menu adds a card
  await p.locator('.dg-lane[data-id="doing"] text').click({ button: "right" }); await sleep(200);
  await p.locator(".dge-menu").getByText("Add card").click(); await sleep(200);
  await p.keyboard.press("Meta+a"); await p.keyboard.type("Orders API"); await p.keyboard.press("Enter"); await sleep(600);
  assert.match(saved(), /column doing: Doing \{\n    orders-api: Orders API\n  \}/, saved());
  assert.match(await p.locator('.dg-lane[data-id="doing"] text').textContent(), /^Doing1$/);

  // drag Login page into Doing, above Orders API
  await p.locator(".dge-canvas").click({ position: { x: 5, y: 5 } });
  const card = await p.locator('.dg-node[data-id="orders-api"]').boundingBox();
  const login = await p.locator('.dg-node[data-id="login"]').boundingBox();
  await p.mouse.move(login.x + login.width / 2, login.y + login.height / 2); await p.mouse.down();
  await p.mouse.move(card.x + card.width / 2, card.y + 4, { steps: 10 }); await p.mouse.up();
  await sleep(700);
  assert.match(saved(), /column todo: To do \{\n  \}\n  column doing: Doing \{\n    login: Login page\n    orders-api: Orders API\n  \}/, saved());
  assert.doesNotMatch(saved(), /x:/);

  // and to the bottom of Done
  const done = await p.locator('.dg-lane[data-id="done"]').boundingBox();
  const l2 = await p.locator('.dg-node[data-id="login"]').boundingBox();
  await p.mouse.move(l2.x + l2.width / 2, l2.y + l2.height / 2); await p.mouse.down();
  await p.mouse.move(done.x + done.width / 2, done.y + done.height - 30, { steps: 10 }); await p.mouse.up();
  await sleep(700);
  assert.match(saved(), /column done: Done \{\n    login: Login page\n  \}/, saved());

  await p.screenshot({ path: join(dir, "light.png") });
  const dk = await browser.newPage({ viewport: { width: 1300, height: 760 }, colorScheme: "dark" });
  await dk.goto(`http://127.0.0.1:${port}/`, { waitUntil: "domcontentloaded" });
  await dk.waitForSelector(".dge-canvas svg"); await sleep(400);
  await dk.screenshot({ path: join(dir, "dark.png") });
  assert.deepEqual(errors, []);
  await browser.close();
  console.log("ok — kanban e2e passed (screenshots: " + dir + ")");
} catch (e) {
  failed = true;
  console.error(e.message || e);
  console.error("\nfile:\n" + saved());
} finally {
  server.kill();
}
process.exit(failed ? 1 : 0);
