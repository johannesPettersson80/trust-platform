#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="${OUT_DIR:-${ROOT_DIR}/target/gate-artifacts/mqtt-mosquitto-e2e}"
mkdir -p "${OUT_DIR}"

for required in cargo grep mosquitto mosquitto_sub python3 sed sort timeout; do
  if ! command -v "${required}" >/dev/null 2>&1; then
    echo "[mqtt-mosquitto-e2e] ERROR: required command not found: ${required}" >&2
    exit 1
  fi
done

temp_dir="$(mktemp -d /tmp/trust-mqtt-mosquitto-e2e.XXXXXX)"
project_dir="${temp_dir}/project"
broker_log="${OUT_DIR}/mosquitto.log"
runtime_log="${OUT_DIR}/runtime.log"
capture_log="${OUT_DIR}/subscriber.log"
build_log="${OUT_DIR}/build.log"
broker_pid=""
subscriber_pid=""
runtime_pid=""

cleanup() {
  local exit_code=$?
  set +e
  if [[ -n "${runtime_pid}" ]] && kill -0 "${runtime_pid}" 2>/dev/null; then
    kill "${runtime_pid}" 2>/dev/null
    wait "${runtime_pid}" 2>/dev/null
  fi
  if [[ -n "${subscriber_pid}" ]] && kill -0 "${subscriber_pid}" 2>/dev/null; then
    kill "${subscriber_pid}" 2>/dev/null
    wait "${subscriber_pid}" 2>/dev/null
  fi
  if [[ -n "${broker_pid}" ]] && kill -0 "${broker_pid}" 2>/dev/null; then
    kill "${broker_pid}" 2>/dev/null
    wait "${broker_pid}" 2>/dev/null
  fi
  rm -rf -- "${temp_dir}"
  exit "${exit_code}"
}
trap cleanup EXIT INT TERM

broker_port="$(python3 - <<'PY'
import socket

with socket.socket() as listener:
    listener.bind(("127.0.0.1", 0))
    print(listener.getsockname()[1])
PY
)"

cp -R "${ROOT_DIR}/examples/communication/mqtt_traffic_light" "${project_dir}"
control_socket="${temp_dir}/runtime.sock"
sed -i "s#127.0.0.1:1883#127.0.0.1:${broker_port}#" "${project_dir}/io.toml"
sed -i "s#/tmp/trust-runtime-mqtt-traffic-light.sock#${control_socket}#" \
  "${project_dir}/runtime.toml"

: > "${broker_log}"
: > "${runtime_log}"
: > "${capture_log}"
: > "${build_log}"

mosquitto -p "${broker_port}" -v >"${broker_log}" 2>&1 &
broker_pid=$!
for _attempt in $(seq 1 50); do
  if python3 - "${broker_port}" <<'PY'
import socket
import sys

try:
    with socket.create_connection(("127.0.0.1", int(sys.argv[1])), timeout=0.1):
        pass
except OSError:
    raise SystemExit(1)
PY
  then
    break
  fi
  sleep 0.1
done
if ! kill -0 "${broker_pid}" 2>/dev/null; then
  echo "[mqtt-mosquitto-e2e] ERROR: Mosquitto exited before the test" >&2
  exit 1
fi

cd "${ROOT_DIR}"
cargo build -p trust-runtime --bin trust-runtime >"${build_log}" 2>&1
runtime_bin="${CARGO_TARGET_DIR:-${ROOT_DIR}/target}/debug/trust-runtime"
if [[ ! -x "${runtime_bin}" ]]; then
  echo "[mqtt-mosquitto-e2e] ERROR: runtime binary missing: ${runtime_bin}" >&2
  exit 1
fi
"${runtime_bin}" build --project "${project_dir}" --sources src >>"${build_log}" 2>&1

timeout 15s mosquitto_sub \
  -h 127.0.0.1 \
  -p "${broker_port}" \
  -t 'traffic/north/#' \
  -v \
  -C 60 >"${capture_log}" &
subscriber_pid=$!
sleep 0.2

"${runtime_bin}" run --project "${project_dir}" >"${runtime_log}" 2>&1 &
runtime_pid=$!
for _attempt in $(seq 1 50); do
  if [[ -S "${control_socket}" ]]; then
    break
  fi
  sleep 0.1
done
if [[ ! -S "${control_socket}" ]]; then
  echo "[mqtt-mosquitto-e2e] ERROR: runtime control socket was not created" >&2
  exit 1
fi
"${runtime_bin}" ctl --endpoint "unix://${control_socket}" status \
  >"${OUT_DIR}/status.log" 2>&1
grep -q '^state=running fault=none ' "${OUT_DIR}/status.log"
wait "${subscriber_pid}"
subscriber_pid=""

"${runtime_bin}" ctl --endpoint "unix://${control_socket}" shutdown \
  >"${OUT_DIR}/shutdown.log" 2>&1
wait "${runtime_pid}"
runtime_pid=""

if [[ "$(wc -l < "${capture_log}")" -ne 60 ]]; then
  echo "[mqtt-mosquitto-e2e] ERROR: expected exactly 60 MQTT messages" >&2
  exit 1
fi
for topic in green yellow red; do
  grep -q "^traffic/north/${topic} true$" "${capture_log}"
  grep -q "^traffic/north/${topic} false$" "${capture_log}"
done
if grep -q "trust/io/in" "${broker_log}"; then
  echo "[mqtt-mosquitto-e2e] ERROR: runtime subscribed to default trust/io/in" >&2
  exit 1
fi

mosquitto_version="$({ mosquitto -h 2>&1 || true; } | sed -n '1p')"
echo "broker=${mosquitto_version}"
echo "messages=$(wc -l < "${capture_log}")"
sort -u "${capture_log}"
echo "default_input_subscription=absent"
echo "RESULT=PASS"
