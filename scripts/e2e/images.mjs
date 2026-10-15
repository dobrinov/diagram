// End-to-end test of images in a real browser: dropping an image file and
// pasting one make image boxes, kept as base64 assets at the end of the file.
//   make e2e   (or: cd scripts/e2e && node images.mjs)
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
const port = 8400 + Math.floor(Math.random() * 90);
const dir = mkdtempSync(join(tmpdir(), "diagram-e2e-"));
const file = join(dir, "board.diagram");
writeFileSync(file, "title: Screens\nnote: Login looks off [sticky]\n");
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

  // a 200×120 PNG drawn in the page, as a File
  const make = (color) => p.evaluate((color) => new Promise((ok) => {
    const c = document.createElement("canvas"); c.width = 200; c.height = 120;
    const g = c.getContext("2d"); g.fillStyle = color; g.fillRect(0, 0, 200, 120); g.fillStyle = "#fff"; g.fillRect(20, 20, 80, 30);
    c.toBlob((b) => { window.__img = new File([b], "shot.png", { type: "image/png" }); ok(); }, "image/png");
  }), color);

  // drop
  await make("#3b6fd8");
  await p.evaluate(() => {
    const dt = new DataTransfer(); dt.items.add(window.__img);
    const c = document.querySelector(".dge-canvas"), r = c.getBoundingClientRect();
    const at = { clientX: r.left + 700, clientY: r.top + 300, dataTransfer: dt, bubbles: true, cancelable: true };
    c.dispatchEvent(new DragEvent("dragover", at)); c.dispatchEvent(new DragEvent("drop", at));
  });
  await sleep(900);
  assert.match(saved(), /\nimage \[image: img1, x: -?\d+, y: -?\d+, w: 208, h: 128\]\n/);
  assert.match(saved(), /\n\nasset img1: data:image\/png;base64,[A-Za-z0-9+/=]+\n$/);
  assert.equal(await p.locator(".dg-image image").count(), 1);

  // paste
  await make("#e5484d");
  await p.locator(".dge-canvas").click({ position: { x: 5, y: 5 } });
  await p.evaluate(() => {
    const dt = new DataTransfer(); dt.items.add(window.__img);
    document.dispatchEvent(new ClipboardEvent("paste", { clipboardData: dt, bubbles: true, cancelable: true }));
  });
  await sleep(900);
  assert.match(saved(), /asset img2: data:image\/png;base64,/);
  assert.equal(await p.locator(".dg-image image").count(), 2);

  // deleting the box drops its image from the file
  await p.locator('.dg-node[data-id="image-2"]').click(); await p.keyboard.press("Backspace"); await sleep(700);
  assert.doesNotMatch(saved(), /asset img2/);
  assert.match(saved(), /asset img1/);

  await p.screenshot({ path: join(dir, "light.png") });
  const dk = await browser.newPage({ viewport: { width: 1200, height: 760 }, colorScheme: "dark" });
  await dk.goto(`http://127.0.0.1:${port}/`, { waitUntil: "domcontentloaded" });
  await dk.waitForSelector(".dge-canvas svg"); await sleep(400);
  await dk.screenshot({ path: join(dir, "dark.png") });
  assert.deepEqual(errors, []);
  await browser.close();
  console.log("ok — images e2e passed (screenshots: " + dir + ")");
} catch (e) {
  failed = true;
  console.error(e.message || e);
  console.error("\nfile:\n" + saved().replace(/base64,[^\n]{40,}/g, (m) => m.slice(0, 40) + "…"));
} finally {
  server.kill();
}
process.exit(failed ? 1 : 0);
