// End-to-end test of settling a merge: two branches change the same box in
// a git repository; the merge driver merges what it can and leaves the clash;
// the editor shows yours, the result and theirs, a side is picked (by hand,
// then by a stand-in AI from a sentence), and saving marks the file resolved.
//   make e2e   (or: cd scripts/e2e && node merge.mjs)
import { chromium } from "playwright";
import { spawn, execSync } from "node:child_process";
import { mkdtempSync, writeFileSync, readFileSync, existsSync, chmodSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..", "..");
const binDir = join(root, "target", "debug");
const chrome = process.env.CHROME || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const env = { ...process.env, PATH: binDir + ":" + process.env.PATH, GIT_AUTHOR_NAME: "t", GIT_AUTHOR_EMAIL: "t@t", GIT_COMMITTER_NAME: "t", GIT_COMMITTER_EMAIL: "t@t" };

/* A repository whose b.diagram is mid-merge with a clash on `api`'s colour. */
function conflicted(driver) {
  const dir = mkdtempSync(join(tmpdir(), "diagram-e2e-"));
  const sh = (c) => execSync(c, { cwd: dir, env, stdio: "pipe" }).toString();
  const write = (s) => writeFileSync(join(dir, "b.diagram"), s);
  sh("git init -q -b main");
  write("title: Shop\napi: Orders API\ndb: Postgres\napi -> db\n");
  sh("git add . && git commit -qm base");
  if (driver) sh("diagram merge --install && git add .gitattributes && git commit -qm attrs");
  sh("git checkout -qb theirs");
  write("title: Shop\napi: Orders API [green, detail: Rust]\ndb: Postgres\napi -> db\nqueue: Jobs\n");
  sh("git commit -qam theirs && git checkout -q main");
  write("title: Shop\napi: Orders API [red]\ndb: Postgres 16\napi -> db\n");
  sh("git commit -qam ours");
  try { sh("git merge theirs -m m"); } catch { /* the clash */ }
  return { dir, file: join(dir, "b.diagram"), unmerged: () => sh("git ls-files -u").trim() };
}

let failed = false;
const servers = [];
try {
  const browser = await chromium.launch(existsSync(chrome) ? { executablePath: chrome } : {});

  // 1. with the merge driver: pick theirs by hand
  const a = conflicted(true);
  assert.match(readFileSync(a.file, "utf8"), /^# merge: 1 conflict kept as yours \(node:api\)/);
  assert.match(readFileSync(a.file, "utf8"), /db: Postgres 16\n/);
  let port = 8600 + Math.floor(Math.random() * 90);
  servers.push(spawn(join(binDir, "diagram"), [a.file, "--no-open", "--port", String(port)], { env: { ...env, DIAGRAM_WEB_DIR: join(root, "web") }, stdio: "ignore" }));
  await sleep(600);
  const p = await browser.newPage({ viewport: { width: 1300, height: 820 } });
  const errors = [];
  p.on("pageerror", (e) => errors.push(e.message));
  await p.goto(`http://127.0.0.1:${port}/`, { waitUntil: "domcontentloaded" });
  await p.waitForSelector(".dge-merge"); await sleep(400);
  assert.match(await p.locator(".dge-merge-note").textContent(), /^1 thing was changed both ways/);
  assert.match(await p.locator(".dge-merge-row").textContent(), /node `api` “Orders API”: yours color = “red”; theirs color = “green”/);
  assert.equal(await p.locator(".dge-merge-pane svg").count(), 3);
  await p.screenshot({ path: join(a.dir, "merge.png") });
  await p.locator(".dge-merge-pick button", { hasText: "Theirs" }).click(); await sleep(200);
  await p.locator(".dge-merge button.primary", { hasText: "Save and mark resolved" }).click(); await sleep(800);
  const merged = readFileSync(a.file, "utf8");
  assert.equal(merged, "title: Shop\napi: Orders API [green, detail: Rust]\ndb: Postgres 16\nqueue: Jobs\napi -> db\n");
  assert.equal(a.unmerged(), "");
  assert.equal(await p.locator(".dge-merge").count(), 0);
  assert.match(await p.locator("#status").textContent(), /marked resolved/);

  // 2. plain git (conflict markers), settled by a sentence through the AI harness
  const b = conflicted(false);
  assert.match(readFileSync(b.file, "utf8"), /<<<<<<< /);
  const fake = join(b.dir, "fake-ai.sh");
  writeFileSync(fake, "#!/bin/sh\ncat >/dev/null\nprintf '%s\\n' '```json' '{\"pick\": {\"node:api\": \"theirs\"}, \"ops\": [{\"op\": \"update_node\", \"id\": \"api\", \"label\": \"Orders service\"}]}' '```'\n");
  chmodSync(fake, 0o755);
  port += 1;
  servers.push(spawn(join(binDir, "diagram"), [b.file, "--no-open", "--port", String(port), "--ai", fake], { env: { ...env, DIAGRAM_WEB_DIR: join(root, "web") }, stdio: "ignore" }));
  await sleep(600);
  const q = await browser.newPage({ viewport: { width: 1300, height: 820 }, colorScheme: "dark" });
  q.on("pageerror", (e) => errors.push(e.message));
  await q.goto(`http://127.0.0.1:${port}/`, { waitUntil: "domcontentloaded" });
  await q.waitForSelector(".dge-merge"); await sleep(400);
  await q.locator(".dge-merge-ask textarea").fill("their colour, and call it Orders service");
  await q.locator(".dge-merge-ask button").click(); await sleep(1500);
  assert.match(await q.locator(".dge-merge-note").textContent(), /The AI also made 1 edit/);
  assert.equal(await q.locator(".dge-merge-pick button.on").textContent(), "Theirs");
  await q.screenshot({ path: join(b.dir, "merge-dark.png") });
  await q.locator(".dge-merge button.primary").click(); await sleep(800);
  const m2 = readFileSync(b.file, "utf8");
  assert.doesNotMatch(m2, /<<<<<<<|=======|>>>>>>>/);
  assert.match(m2, /api: Orders service \[green, detail: Rust\]/, m2);
  assert.equal(b.unmerged(), "");
  assert.deepEqual(errors, []);
  await browser.close();
  console.log("ok — merge e2e passed (screenshots: " + a.dir + "/merge.png, " + b.dir + "/merge-dark.png)");
} catch (e) {
  failed = true;
  console.error(e.message || e);
} finally {
  servers.forEach((s) => s.kill());
}
process.exit(failed ? 1 : 0);
