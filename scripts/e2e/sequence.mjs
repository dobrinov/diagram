// End-to-end test of sequence diagrams in a real browser: the tool adds one
// with a request and its reply, a message's menu moves it up, and dragging
// a participant sideways reorders the participants.
//   make e2e   (or: cd scripts/e2e && node sequence.mjs)
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
const port = 8100 + Math.floor(Math.random() * 90);
const dir = mkdtempSync(join(tmpdir(), "diagram-e2e-"));
const file = join(dir, "board.diagram");
writeFileSync(file, "title: Flows\n");
const server = spawn(bin, [file, "--no-open", "--port", String(port)], { env: { ...process.env, DIAGRAM_WEB_DIR: join(root, "web") }, stdio: "ignore" });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const saved = () => readFileSync(file, "utf8");
let failed = false;
try {
  await sleep(600);
  const browser = await chromium.launch(existsSync(chrome) ? { executablePath: chrome } : {});
  const p = await browser.newPage({ viewport: { width: 1300, height: 800 } });
  const errors = [];
  p.on("pageerror", (e) => errors.push(e.message));
  await p.goto(`http://127.0.0.1:${port}/`, { waitUntil: "domcontentloaded" });
  await p.waitForSelector(".dge-canvas svg"); await sleep(300);

  await p.locator('[data-act="sequence"]').click(); await sleep(200);
  await p.keyboard.type("Checkout"); await p.keyboard.press("Enter"); await sleep(600);
  assert.match(saved(), /sequence checkout: Checkout \{\n  client: Client\n  server: Server\n  client -> server: request\n  server \.\.> client: response\n\}/, saved());
  assert.equal(await p.locator(".dg-lifelines line").count(), 2);

  // the reply first
  await p.locator('.dg-edge[data-id="server->client"] .dg-hit, .dg-edge[data-id="server->client"] path').first().click({ button: "right", force: true }); await sleep(200);
  await p.locator(".dge-menu").getByText("Move up").click(); await sleep(600);
  assert.match(saved(), /  server \.\.> client: response\n  client -> server: request\n\}/, saved());

  // drag Server to the left of Client
  await p.locator(".dge-canvas").click({ position: { x: 5, y: 5 } });
  const c = await p.locator('.dg-node[data-id="client"]').boundingBox();
  const s = await p.locator('.dg-node[data-id="server"]').boundingBox();
  await p.mouse.move(s.x + s.width / 2, s.y + s.height / 2); await p.mouse.down();
  await p.mouse.move(c.x + 4, c.y + c.height / 2, { steps: 10 }); await p.mouse.up();
  await sleep(700);
  assert.match(saved(), /sequence checkout: Checkout \{\n  server: Server\n  client: Client\n/, saved());
  assert.doesNotMatch(saved(), /x:/);
  const c2 = await p.locator('.dg-node[data-id="client"]').boundingBox();
  const s2 = await p.locator('.dg-node[data-id="server"]').boundingBox();
  assert.ok(s2.x < c2.x);

  await p.screenshot({ path: join(dir, "light.png") });
  const dk = await browser.newPage({ viewport: { width: 1300, height: 800 }, colorScheme: "dark" });
  await dk.goto(`http://127.0.0.1:${port}/`, { waitUntil: "domcontentloaded" });
  await dk.waitForSelector(".dge-canvas svg"); await sleep(400);
  await dk.screenshot({ path: join(dir, "dark.png") });
  assert.deepEqual(errors, []);
  await browser.close();
  console.log("ok — sequence e2e passed (screenshots: " + dir + ")");
} catch (e) {
  failed = true;
  console.error(e.message || e);
  console.error("\nfile:\n" + saved());
} finally {
  server.kill();
}
process.exit(failed ? 1 : 0);
