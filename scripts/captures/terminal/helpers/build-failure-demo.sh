#!/usr/bin/env bash
set -euo pipefail

tmpdir=$(mktemp -d)
cleanup() {
  rm -rf "$tmpdir"
}
trap cleanup EXIT

cp -R examples/memory_marker_counter/. "$tmpdir/"
# The capture container has no Python: sed breaks the statement, and grep checks that it did.
sed -i 's/Counter := Counter + 1;/Counter := Counter + ;/' "$tmpdir/src/Main.st"
grep -q 'Counter := Counter + ;' "$tmpdir/src/Main.st"

./target/debug/trust-runtime build --project "$tmpdir" --sources src || true
