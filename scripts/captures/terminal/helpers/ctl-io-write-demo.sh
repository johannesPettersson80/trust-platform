#!/usr/bin/env bash
set -euo pipefail

tmpdir=$(mktemp -d)
runtime_pid=""
cleanup() {
  if [[ -n "$runtime_pid" ]] && kill -0 "$runtime_pid" >/dev/null 2>&1; then
    kill "$runtime_pid" >/dev/null 2>&1 || true
    wait "$runtime_pid" >/dev/null 2>&1 || true
  fi
  rm -rf "$tmpdir"
}
trap cleanup EXIT

cp -R examples/memory_marker_counter/. "$tmpdir/"
python - <<'PY' "$tmpdir/runtime.toml"
from pathlib import Path
import sys

path = Path(sys.argv[1])
text = path.read_text()
text = text.replace('mode = "production"', 'mode = "debug"', 1)
path.write_text(text)
PY

./target/debug/trust-runtime play --project "$tmpdir" >/tmp/trust-ctl-io-write-demo.log 2>&1 &
runtime_pid=$!
sleep 2

./target/debug/trust-runtime ctl --project "$tmpdir" io-write %QW0 42
./target/debug/trust-runtime ctl --project "$tmpdir" io-read
