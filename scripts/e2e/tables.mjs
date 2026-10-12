// End-to-end test of tables in a real browser: a table from the toolbar,
// Edit as text…, Edit row…, Add row below, a row's colour, hover text.
//   make e2e   (or: cd scripts/e2e && node tables.mjs)
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
const port = 6090 + Math.floor(Math.random() * 100);
const dir = mkdtempSync(join(tmpdir(), "diagram-e2e-"));
const file = join(dir, "tables.diagram");
writeFileSync(file, "title: Tables\n");
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

  // the toolbar adds a table; its title is typed next
  await p.locator('.dge-tools [data-act="table"]').click();
  await p.waitForFunction(() => document.activeElement && document.activeElement.classList.contains("dge-inline"));
  await p.keyboard.press(MOD + "+a"); await p.keyboard.type("Options"); await p.keyboard.press("Enter");
  await sleep(600);
  assert.match(saved(), /table options: Options \{\n  item +\| owner \| notes\n  First\n  Second\n\}/, saved());
  await p.keyboard.press("f"); await sleep(300);

  // Edit as text…
  const frame = p.locator('.dg-group[data-id="options"]');
  let b = await frame.boundingBox();
  await p.mouse.click(b.x + 20, b.y + 14, { button: "right" });
  await p.locator(".dge-menu button", { hasText: "Edit as text…" }).click();
  await p.locator(".dge-chart-data").fill("option | cost | notes\nLocal | $0 | no data leaves\nHosted | $1k | data leaves");
  await p.locator(".dge-dialog button.primary").click();
  await sleep(600);
  assert.match(saved(), /  option \| cost \| notes\n  Local  \| \$0   \| no data leaves\n  Hosted \| \$1k  \| data leaves\n/, saved());

  // hover text, Edit row…, Add row below, Colour…
  const hosted = p.locator('.dg-node.dg-datum[data-id="options.hosted"]');
  assert.equal(await hosted.locator("title").textContent(), "Hosted — cost $1k, notes data leaves");
  await hosted.click({ button: "right" });
  await p.locator(".dge-menu button", { hasText: "Edit row…" }).click();
  await p.waitForSelector(".dge-ask");
  await p.keyboard.press(MOD + "+a"); await p.keyboard.type("Hosted | $2k | data leaves the laptop"); await p.keyboard.press("Enter");
  await sleep(500);
  await p.locator('.dg-node.dg-datum[data-id="options.local"]').click({ button: "right" });
  await p.locator(".dge-menu button", { hasText: "Add row below" }).click();
  await sleep(500);
  await hosted.click({ button: "right" });
  await p.locator(".dge-menu button", { hasText: "Colour…" }).click();
  await p.locator(".dge-menu button", { hasText: "Red" }).click();
  await sleep(600);
  assert.match(saved(), /  Local +\| \$0 +\| no data leaves\n  New row\n  Hosted +\| \$2k +\| data leaves the laptop \[red\]\n/, saved());
  assert.deepEqual(errors, []);
  await p.keyboard.press("f"); await sleep(300);
  await p.screenshot({ path: join(dir, "final.png") });
  await browser.close();
  console.log("ok — tables e2e passed (screenshot: " + join(dir, "final.png") + ")");
} catch (e) {
  failed = true;
  console.error(e.message || e);
  console.error("\nfile:\n" + saved());
} finally {
  server.kill();
}
process.exit(failed ? 1 : 0);
