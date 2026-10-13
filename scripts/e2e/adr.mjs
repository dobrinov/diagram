// End-to-end test of decision records in a real browser: the tool adds one
// with the usual fields, the status menu changes it, "Edit as text…" rewrites
// the fields (kept by name, new ones added, missing ones removed), and
// renaming a field moves its id along.
//   make e2e   (or: cd scripts/e2e && node adr.mjs)
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
const port = 7900 + Math.floor(Math.random() * 90);
const dir = mkdtempSync(join(tmpdir(), "diagram-e2e-"));
const file = join(dir, "board.diagram");
writeFileSync(file, "title: Decisions\n");
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

  await p.locator('[data-act="adr"]').click(); await sleep(200);
  await p.keyboard.type("Use Postgres"); await p.keyboard.press("Enter"); await sleep(600);
  assert.match(saved(), /adr use-postgres: Use Postgres \[proposed\] \{\n  context: What is going on, and why decide now\?\n  options: What we considered\n  decision: What we chose\n  consequences: What gets easier or harder\n\}/, saved());

  // status
  const frame = p.locator('.dg-group[data-id="use-postgres"]');
  const fb = await frame.boundingBox();
  await p.mouse.click(fb.x + fb.width - 20, fb.y + 20, { button: "right" }); await sleep(200);
  await p.locator(".dge-menu").getByText("Status…").click(); await sleep(200);
  await p.locator(".dge-menu").getByText("Accepted").click(); await sleep(600);
  assert.match(saved(), /\[accepted\]/);

  // edit as text: keep context, change decision, drop options and consequences, add risks
  await p.mouse.click(fb.x + fb.width - 20, fb.y + 20, { button: "right" }); await sleep(200);
  await p.locator(".dge-menu").getByText("Edit as text…").click(); await sleep(200);
  await p.locator(".dge-chart-data").fill("context: What is going on, and why decide now?\ndecision: Postgres on RDS\\nOne schema per service\nrisks: Migrations on every release");
  await p.locator("button.primary", { hasText: "Save" }).click(); await sleep(600);
  assert.match(saved(), /\{\n  context: What is going on, and why decide now\?\n  decision: Postgres on RDS\\nOne schema per service\n  risks: Migrations on every release\n\}/, saved());

  // rename a field: its id follows
  await p.locator('.dg-node[data-id="use-postgres.risks"]').click({ button: "right" }); await sleep(200);
  await p.locator(".dge-menu").getByText("Rename field").click(); await sleep(200);
  await p.keyboard.press("Meta+a"); await p.keyboard.type("Risks we accept"); await p.keyboard.press("Enter"); await sleep(600);
  assert.match(saved(), /  Risks we accept: Migrations on every release\n/);
  assert.doesNotMatch(saved(), /id: /);

  await p.screenshot({ path: join(dir, "light.png") });
  const dk = await browser.newPage({ viewport: { width: 1300, height: 800 }, colorScheme: "dark" });
  await dk.goto(`http://127.0.0.1:${port}/`, { waitUntil: "domcontentloaded" });
  await dk.waitForSelector(".dge-canvas svg"); await sleep(400);
  await dk.screenshot({ path: join(dir, "dark.png") });
  assert.deepEqual(errors, []);
  await browser.close();
  console.log("ok — adr e2e passed (screenshots: " + dir + ")");
} catch (e) {
  failed = true;
  console.error(e.message || e);
  console.error("\nfile:\n" + saved());
} finally {
  server.kill();
}
process.exit(failed ? 1 : 0);
