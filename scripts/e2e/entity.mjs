// End-to-end test of database tables in a real browser: one from the
// toolbar, its columns edited as text, a foreign key drawn, a column edited.
//   make e2e   (or: cd scripts/e2e && node entity.mjs)
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
const port = 7400 + Math.floor(Math.random() * 100); // clear of the ports browsers refuse (6566, 6665–6669, …)
const dir = mkdtempSync(join(tmpdir(), "diagram-e2e-"));
const file = join(dir, "schema.diagram");
writeFileSync(file, "title: Schema\nentity teams: Teams {\n  id:   uuid [pk]\n  name: text\n}\n");
const server = spawn(bin, [file, "--no-open", "--port", String(port)], { env: { ...process.env, DIAGRAM_WEB_DIR: join(root, "web") }, stdio: "ignore" });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const saved = () => readFileSync(file, "utf8");
const MOD = process.platform === "darwin" ? "Meta" : "Control";
let failed = false;
try {
  await sleep(600);
  const browser = await chromium.launch(existsSync(chrome) ? { executablePath: chrome } : {});
  const p = await browser.newPage({ viewport: { width: 1300, height: 760 } });
  const errors = [];
  p.on("pageerror", (e) => errors.push(e.message));
  await p.goto(`http://127.0.0.1:${port}/`, { waitUntil: "domcontentloaded" });
  await p.waitForSelector(".dge-canvas svg");

  // the toolbar adds a table; its name is typed next (and its columns follow the new id)
  await p.locator('.dge-tools [data-act="entity"]').click();
  await p.waitForFunction(() => document.activeElement && document.activeElement.classList.contains("dge-inline"));
  await p.keyboard.press(MOD + "+a"); await p.keyboard.type("Users"); await p.keyboard.press("Enter");
  await sleep(600);
  assert.match(saved(), /entity users: Users \{\n  id: +uuid \[pk\]\n  created_at: timestamptz \[default: now\(\)\]\n\}/, saved());
  await p.keyboard.press("f"); await sleep(300);

  // Edit columns…
  let b = await p.locator('.dg-group[data-id="users"]').boundingBox();
  await p.mouse.click(b.x + 20, b.y + 14, { button: "right" });
  await p.locator(".dge-menu button", { hasText: "Edit columns…" }).click();
  await p.locator(".dge-chart-data").fill("id: uuid [pk]\nteam_id: uuid [fk: teams]\nemail: text [unique]\nbio: text [null]");
  await p.locator(".dge-dialog button.primary").click();
  await sleep(600);
  assert.match(saved(), /  id: +uuid \[pk\]\n  team_id: +uuid \[fk: teams\]\n  email: +text \[unique\]\n  bio: +text \[null\]\n/, saved());
  assert.equal(await p.locator(".dg-relation").count(), 1);
  assert.equal(await p.locator('.dg-node.dg-datum[data-id="users.team_id"] title').textContent(), "team_id uuid · not null · references teams");

  // a column: Edit column…
  await p.locator('.dg-node.dg-datum[data-id="users.email"]').click({ button: "right" });
  await p.locator(".dge-menu button", { hasText: "Edit column…" }).click();
  await p.waitForSelector(".dge-ask");
  await p.keyboard.press(MOD + "+a"); await p.keyboard.type("email: citext [unique]"); await p.keyboard.press("Enter");
  await sleep(600);
  assert.match(saved(), /  email: +citext \[unique\]\n/, saved());
  assert.deepEqual(errors, []);
  await browser.close();
  console.log("ok — entity e2e passed");
} catch (e) {
  failed = true;
  console.error(e.message || e);
  console.error("\nfile:\n" + saved());
} finally {
  server.kill();
}
process.exit(failed ? 1 : 0);
