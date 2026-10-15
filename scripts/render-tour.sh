#!/usr/bin/env bash
# Renders the feature tour on the landing page: each site/tour/NAME.diagram
# becomes OUT/NAME.svg, in dark mode unless its `# render:` line says --light.
# A `# render: …` line adds options (e.g. --as-of DATE, --base OTHER.diagram);
# files starting with `_` are only inputs to others.
# Usage: scripts/render-tour.sh BIN OUT_DIR
set -euo pipefail
BIN=$1
OUT=$2
ROOT=$(cd "$(dirname "$0")/.." && pwd)
mkdir -p "$OUT"
cd "$ROOT/site/tour"
for f in *.diagram; do
  case "$f" in _*) continue ;; esac
  name=${f%.diagram}
  opts=$(sed -n 's/^# render: //p' "$f" | head -1)
  mode=--dark
  case " $opts " in *" --light "*) mode=""; opts=${opts/--light/} ;; esac
  # shellcheck disable=SC2086
  "$BIN" svg "$f" $mode --comments none $opts -o "$OUT/$name.svg" 2>/dev/null
done
