#!/usr/bin/env bash
set -euo pipefail

tmpdir=$(mktemp -d)
cleanup() {
  rm -rf "$tmpdir"
}
trap cleanup EXIT

cp -R examples/tutorials/10_unit_testing_101/. "$tmpdir/"
cp examples/memory_marker_counter/runtime.toml "$tmpdir/"
cp examples/memory_marker_counter/io.toml "$tmpdir/"

./target/debug/trust-runtime build --project "$tmpdir" --sources src >/dev/null
./target/debug/trust-dev test --project "$tmpdir" --output junit
