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
./target/debug/trust-runtime play --project "$tmpdir" >/tmp/trust-ctl-status-demo.log 2>&1 &
runtime_pid=$!
sleep 2

./target/debug/trust-runtime ctl --project "$tmpdir" status
