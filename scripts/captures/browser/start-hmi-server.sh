#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
TRUST_RUNTIME_BIN="${TRUST_RUNTIME_BIN:-$ROOT_DIR/target/debug/trust-runtime}"
PROJECT_DIR="${TRUST_CAPTURE_HMI_PROJECT:-$ROOT_DIR/examples/tutorials/12_hmi_pid_process_dashboard}"
CAPTURE_PROJECT_DIR=$(mktemp -d)
RUNTIME_PID=""

if [[ ! -x "$TRUST_RUNTIME_BIN" ]]; then
  echo "Missing trust-runtime binary at $TRUST_RUNTIME_BIN. Run cargo build -p trust-runtime first." >&2
  exit 1
fi

cleanup() {
  if [[ -n "$RUNTIME_PID" ]] && kill -0 "$RUNTIME_PID" >/dev/null 2>&1; then
    kill "$RUNTIME_PID" >/dev/null 2>&1 || true
    wait "$RUNTIME_PID" >/dev/null 2>&1 || true
  fi
  rm -rf "$CAPTURE_PROJECT_DIR"
}

terminate() {
  cleanup
  exit 0
}

trap cleanup EXIT
trap terminate INT TERM

cp -R "$PROJECT_DIR/." "$CAPTURE_PROJECT_DIR/"
python - <<'PY' "$CAPTURE_PROJECT_DIR/runtime.toml"
from pathlib import Path
import sys

path = Path(sys.argv[1])
text = path.read_text()
text = text.replace('mode = "production"', 'mode = "debug"', 1)
path.write_text(text)
PY
python - <<'PY' "$CAPTURE_PROJECT_DIR/io.toml"
from pathlib import Path
import sys

path = Path(sys.argv[1])
text = path.read_text()
text = text.replace('driver = "loopback"', 'driver = "simulated"', 1)
path.write_text(text)
PY
cat >"$CAPTURE_PROJECT_DIR/simulation.toml" <<'EOF'
[simulation]
enabled = true
seed = 42
time_scale = 1

[[disturbances]]
at_ms = 0
kind = "set"
target = "%IX0.0"
value = "TRUE"
EOF

"$TRUST_RUNTIME_BIN" build --project "$CAPTURE_PROJECT_DIR" --sources src >/dev/null
"$TRUST_RUNTIME_BIN" run --project "$CAPTURE_PROJECT_DIR" --simulation &
RUNTIME_PID=$!

for _ in $(seq 1 60); do
  if "$TRUST_RUNTIME_BIN" ctl --project "$CAPTURE_PROJECT_DIR" status >/dev/null 2>&1; then
    break
  fi
  sleep 0.5
done

sleep 1

wait "$RUNTIME_PID"
