// End-to-end test of canvases inside a board: a new canvas comes with a button
// on the board, double-clicking it opens the canvas (and the trail leads back),
// what is added inside is written inside its block, the link's style can
// change, and deleting the link asks before deleting the canvas.
//   make e2e   (or: cd scripts/e2e && node canvases.mjs)
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
const port = 8500 + Math.floor(Math.random() * 90);
const dir = mkdtempSync(join(tmpdir(), "diagram-e2e-"));
const file = join(dir, "board.diagram");
writeFileSync(file, "title: Checkout\napi: Orders API\n");
const server = spawn(bin, [file, "--no-open", "--port", String(port)], { env: { ...process.env, DIAGRAM_WEB_DIR: join(root, "web") }, stdio: "ignore" });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const saved = () => readFileSync(file, "utf8");
let failed = false;
try {
  await sleep(600);
  const browser = await chromium.launch(existsSync(chrome) ? { executablePath: chrome } : {});
  const p = await browser.newPage({ viewport: { width: 1200, height: 760 } });
  const errors = [];
  p.on("pageerror", (e) => errors.push(e.message));
  await p.goto(`http://127.0.0.1:${port}/`, { waitUntil: "domcontentloaded" });
  await p.waitForSelector(".dge-canvas svg"); await sleep(300);

  // a new canvas, with a button on the board
  await p.locator(".dge-canvas").click({ button: "right", position: { x: 900, y: 500 } }); await sleep(200);
  await p.locator(".dge-menu").getByText("New canvas here…").click(); await sleep(200);
  await p.keyboard.type("Payments flow"); await p.keyboard.press("Enter"); await sleep(700);
  assert.match(saved(), /payments-flow-link: Payments flow \[pill, canvas: payments-flow, x: -?\d+, y: -?\d+\]/, saved());
  assert.match(saved(), /\ncanvas payments-flow: Payments flow \{\}\n$/, saved());
  assert.equal(await p.locator(".dg-open").count(), 1);

  // open it: an empty board and the trail; add a box inside
  await p.locator('.dg-node[data-id="payments-flow-link"]').dblclick(); await sleep(500);
  assert.equal(await p.locator(".dge-trail").textContent(), "Checkout›Payments flow");
  assert.match(p.url(), /canvas=payments-flow/);
  assert.equal(await p.locator('.dg-node[data-id="api"]').count(), 0);
  await p.locator(".dge-canvas").dblclick({ position: { x: 600, y: 400 } }); await sleep(200);
  await p.keyboard.type("Confirm card"); await p.keyboard.press("Enter"); await sleep(700);
  assert.match(saved(), /canvas payments-flow: Payments flow \{\n  confirm-card: Confirm card \[x: -?\d+, y: -?\d+\]\n\}\n$/, saved());
  assert.match(saved(), /^title: Checkout\napi: Orders API\n/);

  // a reload comes back to the canvas; the trail leads back up
  await p.reload({ waitUntil: "domcontentloaded" }); await p.waitForSelector(".dge-canvas svg"); await sleep(400);
  assert.equal(await p.locator('.dg-node[data-id="confirm-card"]').count(), 1);
  await p.screenshot({ path: join(dir, "inside.png") });
  await p.locator(".dge-trail button", { hasText: "Checkout" }).click(); await sleep(400);
  assert.equal(await p.locator(".dge-trail").count(), 0);
  assert.equal(await p.locator('.dg-node[data-id="api"]').count(), 1);
  assert.doesNotMatch(p.url(), /canvas=/);

  // the link's style
  await p.locator('.dg-node[data-id="payments-flow-link"]').click({ button: "right" }); await sleep(200);
  await p.locator(".dge-menu").getByText("Link style…").click(); await sleep(200);
  await p.locator(".dge-menu").getByText("Card").click(); await sleep(600);
  assert.match(saved(), /payments-flow-link: Payments flow \[canvas: payments-flow/);

  // deleting the link asks; Cancel keeps both, Delete removes both
  await p.locator('.dg-node[data-id="payments-flow-link"]').click(); await p.keyboard.press("Backspace"); await sleep(300);
  assert.match(await p.locator(".dge-modal").textContent(), /also deletes the canvas “Payments flow” \(1 inside\)/);
  await p.locator(".dge-modal button", { hasText: "Cancel" }).click(); await sleep(300);
  assert.match(saved(), /canvas payments-flow/);
  await p.locator('.dg-node[data-id="payments-flow-link"]').click(); await p.keyboard.press("Backspace"); await sleep(300);
  await p.screenshot({ path: join(dir, "confirm.png") });
  await p.locator(".dge-modal button", { hasText: "Delete" }).click(); await sleep(700);
  assert.doesNotMatch(saved(), /canvas/);

  const dk = await browser.newPage({ viewport: { width: 1200, height: 760 }, colorScheme: "dark" });
  writeFileSync(file, "title: Checkout\npay: Payments [pill, canvas: payments]\n\ncanvas payments: Payments flow {\n  a: Create intent\n  b: Confirm card\n  a -> b\n}\n");
  await dk.goto(`http://127.0.0.1:${port}/?canvas=payments`, { waitUntil: "domcontentloaded" });
  await dk.waitForSelector(".dge-canvas svg"); await sleep(500);
  await dk.screenshot({ path: join(dir, "dark-inside.png") });
  await dk.locator(".dge-trail button").click(); await sleep(400);
  await dk.screenshot({ path: join(dir, "dark-top.png") });
  assert.deepEqual(errors, []);
  await browser.close();
  console.log("ok — canvases e2e passed (screenshots: " + dir + ")");
} catch (e) {
  failed = true;
  console.error(e.message || e);
  console.error("\nfile:\n" + saved());
} finally {
  server.kill();
}
process.exit(failed ? 1 : 0);
