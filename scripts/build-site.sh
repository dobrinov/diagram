#!/usr/bin/env bash
# Builds the static GitHub Pages site (landing page, the example, playground).
# Usage: scripts/build-site.sh [OUT_DIR]   (default: ./docs)
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
OUT=${1:-$ROOT/docs}
BIN=${DIAGRAM_BIN:-$ROOT/target/release/diagram}
[ -x "$BIN" ] || { echo "missing $BIN — run 'make release' first" >&2; exit 1; }

REPO_URL=${SITE_REPO_URL:-$(git -C "$ROOT" remote get-url origin 2>/dev/null | sed -E 's#^git@github.com:#https://github.com/#; s#\.git$##' || true)}
REPO_URL=${REPO_URL:-https://github.com/dobrinov/diagram}
# https://github.com/owner/repo → https://owner.github.io/repo/ (override with SITE_URL)
SITE_URL=${SITE_URL:-$(echo "$REPO_URL" | sed -nE 's#^https://github.com/([^/]+)/([^/]+)$#https://\1.github.io/\2/#p')}
SITE_URL=${SITE_URL:-/}

rm -rf "$OUT"
mkdir -p "$OUT/app/pkg" "$OUT/examples"
subst() { sed -e "s#{{REPO_URL}}#$REPO_URL#g" -e "s#{{SITE_URL}}#$SITE_URL#g" "$1" > "$2"; }

subst "$ROOT/site/index.html" "$OUT/index.html"
# the changelog section is rendered from CHANGELOG.md
python3 - "$OUT/index.html" "$(python3 "$ROOT/scripts/changelog.py" html)" <<'PY'
import sys
page, fragment = sys.argv[1:3]
html = open(page).read()
assert "<!-- CHANGELOG -->" in html, "site/index.html lost its <!-- CHANGELOG --> marker"
open(page, "w").write(html.replace("<!-- CHANGELOG -->", fragment, 1))
PY
# the live board in the hero is site/hero.diagram
python3 - "$OUT/index.html" "$ROOT/site/hero.diagram" <<'PY'
import sys
page, hero = sys.argv[1:3]
html, board = open(page).read(), open(hero).read()
assert "{{HERO}}" in html, "site/index.html lost its {{HERO}} marker"
assert "</script" not in board.lower()
open(page, "w").write(html.replace("{{HERO}}", board.rstrip("\n"), 1))
PY
cp "$ROOT/site/site.css" "$ROOT/site/og.png" "$ROOT/site/apple-touch-icon.png" "$ROOT/site/merge.png" "$ROOT/web/favicon.svg" "$OUT/"
# the feature tour: a picture per feature, drawn from site/tour/*.diagram
"$ROOT/scripts/render-tour.sh" "$BIN" "$OUT/tour"

# one bundle (JS + inlined WASM + the editor) for every embedded board
"$BIN" embed -o "$OUT/diagram.embed.js" 2>/dev/null

# the example: its source, and a picture of the board and of each canvas in it
subst "$ROOT/site/examples/index.html" "$OUT/examples/index.html"
EX="$ROOT/examples/example.diagram"
cp "$EX" "$OUT/examples/"
"$BIN" svg "$EX" --comments none -o "$OUT/examples/board.svg" 2>/dev/null
for c in discovery plan data metrics; do
  "$BIN" svg "$EX" --canvas "$c" --comments none -o "$OUT/examples/$c.svg" 2>/dev/null
done

# playground: the editor app without a server
cp "$ROOT/web/app.js" "$ROOT/web/editor.js" "$ROOT/web/editor.css" "$ROOT/web/favicon.svg" "$OUT/app/"
cp "$ROOT/web/pkg/diagram_wasm.js" "$ROOT/web/pkg/diagram_wasm_bg.wasm" "$OUT/app/pkg/"
STATIC='<script>window.DIAGRAM_STATIC = { shareUrl: "'"$SITE_URL"'app/", examples: [
  { name: "Checkout relaunch (every feature)", url: "../examples/example.diagram" }
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

# the logo and share images carry a version from their content, so browsers
# (and favicon caches) fetch them again when they change
python3 - "$OUT" <<'PY'
import hashlib, pathlib, re, sys
out = pathlib.Path(sys.argv[1])
ver = {n: hashlib.sha1((out / n).read_bytes()).hexdigest()[:8] for n in ("favicon.svg", "og.png", "apple-touch-icon.png")}
for page in out.rglob("*.html"):
    html = page.read_text()
    new = re.sub(r'((?:\.\./)*(?:favicon\.svg|og\.png|apple-touch-icon\.png))(?=")',
                 lambda m: m.group(1) + "?v=" + ver[m.group(1).rsplit("/", 1)[-1]], html)
    if new != html:
        page.write_text(new)
PY

# analytics for the hosted site only (never in the CLI editor or user exports)
if [ -z "${SITE_NO_ANALYTICS:-}" ]; then
  python3 - "$OUT" "$ROOT/site/analytics.html" <<'PY'
import pathlib, sys
out, snippet = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2]).read_text()
for page in out.rglob("*.html"):
    html = page.read_text()
    if "</head>" in html and "stats.aparatnata.com" not in html:
        page.write_text(html.replace("</head>", snippet + "</head>", 1))
PY
fi

touch "$OUT/.nojekyll"
echo "site written to $OUT ($(du -sh "$OUT" | cut -f1))"
