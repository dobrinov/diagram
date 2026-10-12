// End-to-end test of risk matrices in a real browser: hover text, dragging a
// dot to another cell, the Probability… menu, deleting from the list, and a
// new matrix from the toolbar.
//   make e2e   (or: cd scripts/e2e && node risks.mjs)
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
const port = 5890 + Math.floor(Math.random() * 100);
const dir = mkdtempSync(join(tmpdir(), "diagram-e2e-"));
const file = join(dir, "risks.diagram");
writeFileSync(file, `risks delivery: Delivery risks {
  Contract slips [p: 4, i: 4, owner: ana, detail: "Legal says 4 weeks"]
  Scope creep [p: 2, i: 3]
  GDPR review [p: 3, i: 2]
  Unknowns
}
`);
const server = spawn(bin, [file, "--no-open", "--port", String(port)], {
  env: { ...process.env, DIAGRAM_WEB_DIR: join(root, "web") },
  stdio: "ignore",
});
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
  await p.waitForSelector(".dge-canvas svg .dg-risk");
  await sleep(300);
  const dot = (id) => p.locator(`.dg-node.dg-risk[data-id="${id}"]`);

  // hovering a dot: the risk is under the pointer, with its description as the tooltip
  let b = await dot("contract-slips").boundingBox();
  const title = await p.evaluate(([x, y]) => document.elementFromPoint(x, y)?.closest(".dg-risk")?.querySelector("title")?.textContent, [b.x + b.width / 2, b.y + b.height / 2]);
  assert.equal(title, "Contract slips\nProbability 4 × impact 4 = 16 (critical)\nOwner: ana\n\nLegal says 4 weeks");

  // drag Scope creep to the top-right cell: probability 5, impact 5
  b = await dot("scope-creep").boundingBox();
  const cell = await p.evaluate(() => { const c = document.querySelectorAll(".dg-matrix rect")[4].getBoundingClientRect(); return { x: c.x + c.width / 2, y: c.y + c.height / 2 }; });
  await p.mouse.move(b.x + b.width / 2, b.y + b.height / 2); await p.mouse.down();
  await p.mouse.move(cell.x, cell.y, { steps: 8 }); await p.mouse.up();
  await sleep(600);
  assert.match(saved(), /  Scope creep \[p: 5, i: 5\]\n/, saved());

  // right-click → Probability… → 1
  await dot("gdpr-review").click({ button: "right" });
  await p.locator(".dge-menu button", { hasText: "Probability…" }).click();
  await p.locator(".dge-menu button", { hasText: "1 · Very low" }).click();
  await sleep(600);
  assert.match(saved(), /  GDPR review \[p: 1, i: 2\]\n/, saved());

  // a row of the list stands for its risk: right-click → Delete
  await p.locator('.dg-risk-row[data-node="unknowns"]').click({ button: "right" });
  await p.locator(".dge-menu button", { hasText: "Delete" }).click();
  await sleep(600);
  assert.doesNotMatch(saved(), /Unknowns/);

  // the toolbar adds a matrix with one risk in the middle
  await p.locator('.dge-tools [data-act="risks"]').click();
  await sleep(300); await p.keyboard.press("Escape"); await sleep(600);
  assert.match(saved(), /risks risks: Risks \{\n  New risk \[id: risk-1, p: 3, i: 3\]\n\}/, saved());
  assert.deepEqual(errors, []);
  await browser.close();
  console.log("ok — risks e2e passed");
} catch (e) {
  failed = true;
  console.error(e.message || e);
  console.error("\nfile:\n" + saved());
} finally {
  server.kill();
}
process.exit(failed ? 1 : 0);
