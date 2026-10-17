// End-to-end test of links between boards: a `board.diagram#id` link opens the
// other board in place (?file=…&focus=id) and selects the element; "Link to a
// board…" offers the boards next to this one and saves a relative link.
//   make e2e   (or: cd scripts/e2e && node links.mjs)
import { chromium } from "playwright";
import { spawn } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..", "..");
const bin = join(root, "target", "debug", "diagram");
const chrome = process.env.CHROME || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const port = 7700 + Math.floor(Math.random() * 100);
const dir = mkdtempSync(join(tmpdir(), "diagram-e2e-"));
const file = join(dir, "discovery.diagram");
const other = join(dir, "plans", "delivery.diagram");
mkdirSync(join(dir, "plans"));
writeFileSync(file, "title: Discovery\nidea: Faster checkout [link: plans/delivery.diagram#m1]\nwhy: Why now\nidea -> why\n");
writeFileSync(other, "title: Delivery\nm0: Kick-off\nm1: Milestone one\nm2: Milestone two\nm0 -> m1 -> m2\n");
const server = spawn(bin, [file, "--no-open", "--port", String(port)], { env: { ...process.env, DIAGRAM_WEB_DIR: join(root, "web") }, stdio: "ignore" });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let failed = false;
try {
  await sleep(600);
  const browser = await chromium.launch(existsSync(chrome) ? { executablePath: chrome } : {});
  const p = await browser.newPage({ viewport: { width: 1200, height: 700 } });
  const errors = [];
  p.on("pageerror", (e) => errors.push(e.message));
  await p.goto(`http://127.0.0.1:${port}/`, { waitUntil: "domcontentloaded" });
  await p.waitForSelector(".dge-canvas svg"); await sleep(300);

  // in comment mode a link is part of its box: the click comments on it, nothing is followed
  await p.keyboard.press("c");
  await p.locator('.dg-link[data-node="idea"]').click(); await sleep(300);
  assert.match(p.url(), /:\d+\/$/);
  assert.equal(await p.locator(".dge-bubble textarea").getAttribute("placeholder"), "Comment on Faster checkout…");
  await p.keyboard.press("Escape");
  await p.locator('.dge-tool[data-tool="select"]').click();

  // the badge says where it goes; clicking it opens the other board on m1
  const badge = p.locator('.dg-link[data-node="idea"]');
  assert.equal(await badge.locator("title").textContent(), "Go to m1 on plans/delivery.diagram");
  await badge.click();
  await p.waitForURL(/file=plans%2Fdelivery\.diagram&focus=m1/);
  await p.waitForSelector(".dge-canvas svg"); await sleep(400);
  assert.equal(await p.locator("#file").textContent(), "plans/delivery.diagram");
  assert.equal(await p.locator('.dg-node.sel[data-id="m1"]').count(), 1);

  // edits on it go to that file; "Link to a board…" offers ../discovery.diagram
  await p.locator('.dg-node[data-id="m2"]').click({ button: "right" }); await sleep(200);
  await p.locator(".dge-menu").getByText("Link to a board…").click(); await sleep(300);
  await p.screenshot({ path: join(dir, "menu.png") });
  await p.locator(".dge-menu").getByText("../discovery.diagram", { exact: true }).click(); await sleep(700);
  assert.match(readFileSync(other, "utf8"), /m2: Milestone two \[link: \.\.\/discovery\.diagram\]/);
  assert.doesNotMatch(readFileSync(file, "utf8"), /m2/);

  // and back
  await p.locator('.dg-link[data-node="m2"]').click();
  await p.waitForURL(/file=discovery\.diagram$/);
  await p.waitForSelector('.dg-node[data-id="idea"]');

  // a link to a board that is not there says so
  await p.evaluate(() => window.diagramEditor.apply([{ op: "update_node", id: "why", link: "gone.diagram" }])); await sleep(300);
  await p.locator('.dg-link[data-node="why"]').click(); await sleep(400);
  assert.match(await p.locator(".dge-toast").textContent(), /gone\.diagram does not exist/);
  assert.match(p.url(), /file=discovery\.diagram$/);
  assert.deepEqual(errors, []);
  await p.screenshot({ path: join(dir, "final.png") });
  await browser.close();
  console.log("ok — links e2e passed (screenshot: " + join(dir, "menu.png") + ")");
} catch (e) {
  failed = true;
  console.error(e.message || e);
} finally {
  server.kill();
}
process.exit(failed ? 1 : 0);
