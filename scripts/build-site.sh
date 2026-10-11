#!/usr/bin/env bash
# Builds the static GitHub Pages site (landing page, playground, examples).
# Usage: scripts/build-site.sh [OUT_DIR]   (default: ./docs)
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
OUT=${1:-$ROOT/docs}
BIN=${DIAGRAM_BIN:-$ROOT/target/release/diagram}
[ -x "$BIN" ] || { echo "missing $BIN — run 'make release' first" >&2; exit 1; }

REPO_URL=${SITE_REPO_URL:-$(git -C "$ROOT" remote get-url origin 2>/dev/null | sed -E 's#^git@github.com:#https://github.com/#; s#\.git$##' || true)}
REPO_URL=${REPO_URL:-https://github.com/dobrinov/diagram}
SITE_URL=${SITE_URL:-$(echo "$REPO_URL" | sed -nE 's#^https://github.com/([^/]+)/([^/]+)$#https://\1.github.io/\2/#p')}
SITE_URL=${SITE_URL:-/}

rm -rf "$OUT"
mkdir -p "$OUT/app/pkg" "$OUT/examples"
subst() { sed -e "s#{{REPO_URL}}#$REPO_URL#g" -e "s#{{SITE_URL}}#$SITE_URL#g" "$1" > "$2"; }

subst "$ROOT/site/index.html" "$OUT/index.html"
cp "$ROOT/site/site.css" "$ROOT/web/favicon.svg" "$OUT/"
"$BIN" embed -o "$OUT/diagram.embed.js" 2>/dev/null

# playground: the editor app without a server
cp "$ROOT/web/app.js" "$ROOT/web/editor.js" "$ROOT/web/editor.css" "$ROOT/web/favicon.svg" "$OUT/app/"
cp "$ROOT/web/pkg/diagram_wasm.js" "$ROOT/web/pkg/diagram_wasm_bg.wasm" "$OUT/app/pkg/"
STATIC='<script>window.DIAGRAM_STATIC = { shareUrl: "'"$SITE_URL"'app/", examples: [
  { name: "Checkout architecture", url: "../examples/checkout.diagram" },
  { name: "User onboarding flow", url: "../examples/onboarding.diagram" },
  { name: "Retro board", url: "../examples/retro.diagram" },
  { name: "A prompt as a mindmap", url: "../examples/prompt-mindmap.diagram" }
] };</script>'
python3 - "$ROOT/web/index.html" "$OUT/app/index.html" "$STATIC" <<'PY'
import sys
src, dst, static = sys.argv[1:4]
html = open(src).read()
marker = '<script src="pkg/diagram_wasm.js"></script>'
assert marker in html, "web/index.html lost its wasm <script>"
html = html.replace(marker, static + "\n" + marker).replace("<title>diagram</title>", "<title>Playground — diagram</title>")
open(dst, "w").write(html)
PY

# examples: source, SVG, PNG and a standalone page each
cards=""
for f in "$ROOT"/examples/*.diagram; do
  name=$(basename "$f" .diagram)
  cp "$f" "$OUT/examples/"
  "$BIN" svg "$f" --comments none -o "$OUT/examples/$name.svg" 2>/dev/null
  "$BIN" png "$f" --comments none -o "$OUT/examples/$name.png" 2>/dev/null
  "$BIN" html "$f" -o "$OUT/examples/$name.html" 2>/dev/null
  title=$(sed -n 's/^title: //p' "$f" | head -1)
  cards+="<figure><a href=\"$name.html\"><img src=\"$name.svg\" alt=\"$title\"></a><figcaption><b>$title</b><a href=\"$name.diagram\">source</a><a href=\"$name.html\">interactive</a><a href=\"$name.png\">PNG</a></figcaption></figure>"$'\n'
done
subst "$ROOT/site/examples/index.html" "$OUT/examples/index.html"
python3 - "$OUT/examples/index.html" "$cards" <<'PY'
import sys
page, cards = sys.argv[1:3]
html = open(page).read()
open(page, "w").write(html.replace("<!-- EXAMPLES -->", cards, 1))
PY

touch "$OUT/.nojekyll"
echo "site written to $OUT ($(du -sh "$OUT" | cut -f1))"
