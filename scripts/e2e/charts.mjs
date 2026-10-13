// End-to-end test of charts in a real browser: a chart from the toolbar,
// Edit data…, Type…, a row's Values…, hover text, and dragging a chart by a bar.
//   make e2e   (or: cd scripts/e2e && node charts.mjs)
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
const port = 7600 + Math.floor(Math.random() * 100); // clear of the ports browsers refuse (6000, …)
const dir = mkdtempSync(join(tmpdir(), "diagram-e2e-"));
const file = join(dir, "charts.diagram");
writeFileSync(file, "title: Charts\n");
const server = spawn(bin, [file, "--no-open", "--port", String(port)], {
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
  const p = await browser.newPage({ viewport: { width: 1300, height: 800 } });
  const errors = [];
  p.on("pageerror", (e) => errors.push(e.message));
  await p.goto(`http://127.0.0.1:${port}/`, { waitUntil: "domcontentloaded" });
  await p.waitForSelector(".dge-canvas svg");

  // the toolbar adds a bar chart; its title is typed next
  await p.locator('.dge-tools [data-act="chart"]').click();
  await p.waitForFunction(() => document.activeElement && document.activeElement.classList.contains("dge-inline"));
  await p.keyboard.press(MOD + "+a"); await p.keyboard.type("Revenue"); await p.keyboard.press("Enter");
  await sleep(600);
  assert.match(saved(), /chart revenue: Revenue \[bar\] \{\n  quarter \| Plan \| Actual\n  Q1 \| 10 \| 8\n/, saved());
  await p.keyboard.press("f"); await sleep(300);

  // Edit data…: a new table
  const frame = p.locator('.dg-group[data-id="revenue"]');
  let b = await frame.boundingBox();
  await p.mouse.click(b.x + 20, b.y + 14, { button: "right" });
  await p.locator(".dge-menu button", { hasText: "Edit data…" }).click();
  await p.locator(".dge-chart-data").fill("quarter | EU | US\nQ1 | 120 | 80\nQ2 | 150 | 95\nQ3 | 170 | 130");
  await p.locator(".dge-dialog button.primary").click();
  await sleep(600);
  assert.match(saved(), /  quarter \| EU \| US\n  Q1 \| 120 \| 80\n  Q2 \| 150 \| 95\n  Q3 \| 170 \| 130\n\}/, saved());

  // a row: hover text, then Values…
  const q3 = p.locator('.dg-node.dg-datum[data-id="revenue.q3"]');
  assert.equal(await q3.locator("title").textContent(), "Q3 — EU 170, US 130");
  await q3.click({ button: "right" });
  await p.locator(".dge-menu button", { hasText: "Values…" }).click();
  await p.waitForSelector(".dge-ask");
  await p.keyboard.press(MOD + "+a"); await p.keyboard.type("175, 140"); await p.keyboard.press("Enter");
  await sleep(600);
  assert.match(saved(), /  Q3 \| 175 \| 140\n/, saved());

  // Type… → Lines
  b = await frame.boundingBox();
  await p.mouse.click(b.x + 20, b.y + 14, { button: "right" });
  await p.locator(".dge-menu button", { hasText: "Type…" }).click();
  await p.locator(".dge-menu button", { hasText: "Lines" }).click();
  await sleep(600);
  assert.match(saved(), /chart revenue: Revenue \[line\]/, saved());

  // dragging a row moves the whole chart
  const qb = await p.locator('.dg-node.dg-datum[data-id="revenue.q1"]').boundingBox();
  await p.mouse.move(qb.x + qb.width / 2, qb.y + qb.height / 2); await p.mouse.down();
  await p.mouse.move(qb.x + qb.width / 2 + 80, qb.y + qb.height / 2 + 40, { steps: 6 }); await p.mouse.up();
  await sleep(600);
  assert.match(saved(), /chart revenue: Revenue \[line, x: -?\d+, y: -?\d+\]/, saved());
  assert.deepEqual(errors, []);
  await p.screenshot({ path: join(dir, "final.png") });
  await browser.close();
  console.log("ok — charts e2e passed (screenshot: " + join(dir, "final.png") + ")");
} catch (e) {
  failed = true;
  console.error(e.message || e);
  console.error("\nfile:\n" + saved());
} finally {
  server.kill();
}
process.exit(failed ? 1 : 0);
