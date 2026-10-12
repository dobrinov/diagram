// End-to-end test of themes in a real browser: a preset from the picker,
// Edit theme… (a board's own theme), and a theme file the board names.
//   make e2e   (or: cd scripts/e2e && node themes.mjs)
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
const dir = mkdtempSync(join(tmpdir(), "diagram-e2e-"));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const servers = [];
function serve(file) {
  const port = 6190 + Math.floor(Math.random() * 200);
  servers.push(spawn(bin, [file, "--no-open", "--port", String(port)], { env: { ...process.env, DIAGRAM_WEB_DIR: join(root, "web") }, stdio: "ignore" }));
  return port;
}
const canvasBg = (p) => p.evaluate(() => document.querySelector(".dge-canvas").style.getPropertyValue("--canvas"));
let failed = false;
const file = join(dir, "board.diagram");
try {
  writeFileSync(file, "title: Themed\napi: Orders API [blue]\n");
  writeFileSync(join(dir, "brand.theme"), "theme brand [base: dark] {\n  background: #101418\n  blue: #3b6fd8\n}\n");
  writeFileSync(join(dir, "branded.diagram"), "theme: ./brand.theme\napi: Orders API [blue]\n");
  const port = serve(file), port2 = serve(join(dir, "branded.diagram"));
  await sleep(700);
  const browser = await chromium.launch(existsSync(chrome) ? { executablePath: chrome } : {});
  const p = await browser.newPage({ viewport: { width: 1200, height: 700 }, colorScheme: "light" });
  const errors = [];
  p.on("pageerror", (e) => errors.push(e.message));
  await p.goto(`http://127.0.0.1:${port}/`, { waitUntil: "domcontentloaded" });
  await p.waitForSelector(".dge-canvas svg");
  const pick = 'select[title="Theme of this board"]';

  // a preset
  await p.selectOption(pick, "paper"); await sleep(600);
  assert.match(readFileSync(file, "utf8"), /^title: Themed\ntheme: paper\n/);
  assert.equal(await canvasBg(p), "#f6f1e7");

  // Edit theme…: starts from the preset, saved as the board's own
  await p.selectOption(pick, "__edit");
  await p.waitForSelector(".dge-chart-data");
  assert.match(await p.locator(".dge-chart-data").inputValue(), /^theme paper-custom \[base: light\] \{\n  background: #f6f1e7/);
  await p.locator(".dge-chart-data").fill("theme mine [base: light] {\n  background: #fafafa\n  blue: #5b5bd6\n}");
  await p.locator(".dge-dialog button.primary").click(); await sleep(600);
  assert.match(readFileSync(file, "utf8"), /theme: mine\n[^]*theme mine \[base: light\] \{\n  background: #fafafa\n  blue: #5b5bd6\n\}/);
  assert.equal(await p.locator(pick).inputValue(), "mine");
  assert.match(await p.locator(".dge-canvas svg").first().innerHTML(), /#5b5bd6/);

  // a theme file next to the board, through the server
  await p.goto(`http://127.0.0.1:${port2}/`, { waitUntil: "domcontentloaded" });
  await p.waitForSelector(".dge-canvas svg"); await sleep(400);
  assert.equal(await canvasBg(p), "#101418");
  assert.deepEqual(errors, []);
  await browser.close();
  console.log("ok — themes e2e passed");
} catch (e) {
  failed = true;
  console.error(e.message || e);
  console.error("\nfile:\n" + readFileSync(file, "utf8"));
} finally {
  servers.forEach((s) => s.kill());
}
process.exit(failed ? 1 : 0);
