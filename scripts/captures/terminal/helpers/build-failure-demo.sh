#!/usr/bin/env bash
set -euo pipefail

tmpdir=$(mktemp -d)
cleanup() {
  rm -rf "$tmpdir"
}
trap cleanup EXIT

cp -R examples/memory_marker_counter/. "$tmpdir/"
python - <<'PY' "$tmpdir/src/Main.st"
from pathlib import Path
import sys

path = Path(sys.argv[1])
text = path.read_text()
text = text.replace("Counter := Counter + 1;", "Counter := Counter + ;")
path.write_text(text)
PY

./target/debug/trust-runtime build --project "$tmpdir" --sources src || true
