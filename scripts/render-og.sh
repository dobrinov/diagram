#!/usr/bin/env bash
# Renders site/og.html to site/og.png (1200×630) and site/apple-touch-icon.png
# (180×180) with Playwright (scripts/e2e). The PNGs are committed, so CI needs
# no browser.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT/scripts/e2e"
[ -d node_modules ] || npm install --silent
node --input-type=module - "$ROOT" <<'JS'
import { chromium } from "playwright";
import { existsSync, readFileSync } from "node:fs";
const root = process.argv[2];
const chrome = process.env.CHROME || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const browser = await chromium.launch(existsSync(chrome) ? { executablePath: chrome } : {});
const og = await browser.newPage({ viewport: { width: 1200, height: 630 } });
await og.goto(`file://${root}/site/og.html`);
await og.waitForTimeout(300);
await og.screenshot({ path: `${root}/site/og.png` });
const icon = await browser.newPage({ viewport: { width: 180, height: 180 } });
const logo = readFileSync(`${root}/web/favicon.svg`, "utf8").replace("<svg ", '<svg width="128" height="128" ');
await icon.setContent(`<body style="margin:0;width:180px;height:180px;background:#0d1117;display:flex;align-items:center;justify-content:center">${logo}</body>`);
await icon.waitForTimeout(200);
await icon.screenshot({ path: `${root}/site/apple-touch-icon.png` });
await browser.close();
console.log("wrote site/og.png and site/apple-touch-icon.png");
JS
