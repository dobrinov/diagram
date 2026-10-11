// End-to-end test of mindmap editing in a real browser.
//   make e2e   (or: cd scripts/e2e && npm install && node mindmap.mjs)
// Starts `diagram` on a scratch file with web/ served from disk and a fake AI
// harness, types an outline, navigates, reorders, folds, drags, grows a topic
// with "AI" and pastes a list, then checks the saved file.
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
const port = 5790 + Math.floor(Math.random() * 100);
const dir = mkdtempSync(join(tmpdir(), "diagram-e2e-"));
const file = join(dir, "board.diagram");
writeFileSync(file, "title: Test board\n");

const server = spawn(bin, [file, "--no-open", "--port", String(port), "--ai", join(here, "fake-ai.sh")], {
  env: { ...process.env, DIAGRAM_WEB_DIR: join(root, "web") },
  stdio: "ignore",
});
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const saved = () => readFileSync(file, "utf8");
let failed = false;
try {
  await sleep(600);
  const browser = await chromium.launch(existsSync(chrome) ? { executablePath: chrome } : {});
  const p = await browser.newPage({ viewport: { width: 1400, height: 850 } });
  const errors = [];
  p.on("pageerror", (e) => errors.push(e.message));
  p.on("console", (m) => m.type() === "error" && errors.push(m.text()));
  await p.goto(`http://127.0.0.1:${port}/`, { waitUntil: "domcontentloaded" });
  await p.waitForSelector(".dge-canvas svg");
  const key = async (k) => { await p.keyboard.press(k); await sleep(60); };
  const type = (t) => p.keyboard.type(t);
  const topic = (id) => p.locator(`.dg-node[data-id="${id}"]`);

  // type an outline
  await p.locator(".dge-canvas").click({ position: { x: 300, y: 300 } });
  await key("t");
  await p.locator(".dge-canvas").click({ position: { x: 400, y: 400 } });
  await p.waitForSelector(".dge-inline");
  await type("Launch plan"); await key("Enter");
  await key("Tab"); await type("Discovery"); await key("Enter");
  await type("Build"); await key("Enter");
  await type("Ship"); await key("Enter");
  await key("Escape"); // an empty new topic is dropped
  await topic("discovery").click();
  await key("Tab"); await type("Interviews"); await key("Tab"); await type("Ops team"); await key("Enter");
  await type("Finance"); await key("Enter"); await key("Escape");
  await sleep(700);
  assert.match(saved(), /mindmap launch-plan: Launch plan \[x: \d+, y: \d+\] \{\n  Discovery\n    Interviews\n      Ops team\n      Finance\n  Build\n  Ship\n\}/);

  // navigate, reorder, fold
  await topic("ship").click();
  await key("ArrowUp"); await key("ArrowUp");
  assert.equal(await p.locator(".dge-inspector h3 code").textContent(), "discovery");
  await key("Alt+ArrowDown");
  await key("."); await sleep(300);
  assert.equal(await p.locator(".dg-fold").count(), 1);
  await key(".");
  await key("f"); await sleep(200);

  // drag a branch onto another topic
  const s = await topic("ship").boundingBox(), t = await topic("interviews").boundingBox();
  await p.mouse.move(s.x + s.width / 2, s.y + s.height / 2);
  await p.mouse.down();
  await p.mouse.move(s.x + 30, s.y + 30, { steps: 4 });
  await p.mouse.move(t.x + t.width / 2, t.y + t.height / 2, { steps: 8 });
  await p.mouse.up();
  await sleep(600);

  // grow a topic with the (fake) AI
  await topic("build").click();
  await p.locator(".dge-ai button").click();
  await sleep(1500);

  // paste a Markdown list onto the empty canvas: a new map below the first
  await key("Escape");
  await p.evaluate(() => {
    const dt = new DataTransfer();
    dt.setData("text/plain", "# Ideas\n- One\n- Two\n  - Two b");
    document.querySelector(".dge-canvas").focus();
    document.dispatchEvent(new ClipboardEvent("paste", { clipboardData: dt, bubbles: true }));
  });
  await sleep(800);
  await p.screenshot({ path: join(dir, "final.png") });
  await browser.close();

  const out = saved();
  assert.match(out, /  Build\n    Market size\n      SMB\n    Competitors\n    Pricing\n  Discovery\n    Interviews\n      Ops team\n      Finance\n      Ship\n\}/, out);
  assert.match(out, /mindmap ideas: Ideas \[x: \d+, y: \d+\] \{\n  One\n  Two\n    Two b\n\}/, out);
  assert.deepEqual(errors, []);
  console.log(`ok — mindmap e2e passed (screenshot: ${join(dir, "final.png")})`);
} catch (e) {
  failed = true;
  console.error(e.message || e);
  console.error(`file:\n${saved()}\nscratch dir: ${dir}`);
} finally {
  server.kill();
}
process.exit(failed ? 1 : 0);
