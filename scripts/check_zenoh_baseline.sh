#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

EXPECTED="${ZENOH_BASELINE_VERSION:-1.7.2}"
EXCEPTION_FILE="${ZENOH_BASELINE_EXCEPTION_FILE:-}"

python3 - "${EXPECTED}" "${EXCEPTION_FILE}" <<'PY'
import pathlib
import re
import sys

expected = sys.argv[1].strip()
exception_file = sys.argv[2].strip()
root = pathlib.Path(".")

version_rs = root / "crates" / "trust-runtime" / "src" / "host" / "mesh" / "version.rs"
text = version_rs.read_text(encoding="utf-8")
constant_pattern = re.compile(r'pub const (ZENOH|ZENOHD)_BASELINE_VERSION: &str = "([^"]+)";')
constants = {name: value for name, value in constant_pattern.findall(text)}

missing = [name for name in ("ZENOH", "ZENOHD") if name not in constants]
if missing:
    print(f"[zenoh-baseline] FAIL: missing baseline constants in {version_rs}: {missing}")
    sys.exit(1)

mismatches = []
for key, value in constants.items():
    if value != expected:
        mismatches.append((f"{key}_BASELINE_VERSION", value, expected))

lock_path = root / "Cargo.lock"
lock_text = lock_path.read_text(encoding="utf-8")
package_pattern = re.compile(r'\[\[package\]\]\nname = "([^"]+)"\nversion = "([^"]+)"', re.MULTILINE)
zenoh_packages = [
    (name, version)
    for name, version in package_pattern.findall(lock_text)
    if name.startswith("zenoh")
]
if not zenoh_packages:
    print("[zenoh-baseline] FAIL: no zenoh packages found in Cargo.lock")
    sys.exit(1)

for name, version in zenoh_packages:
    if version != expected:
        mismatches.append((name, version, expected))

if mismatches:
    if exception_file:
        path = pathlib.Path(exception_file)
        if path.is_file() and path.read_text(encoding="utf-8").strip():
            print("[zenoh-baseline] WARN: mismatch accepted by exception file")
            for item, actual, wanted in mismatches:
                print(f"  - {item}: actual={actual} expected={wanted}")
            print(f"  exception: {path}")
            sys.exit(0)
    print("[zenoh-baseline] FAIL: zenoh baseline mismatch")
    for item, actual, wanted in mismatches:
        print(f"  - {item}: actual={actual} expected={wanted}")
    if exception_file:
        print(f"  expected non-empty exception file: {exception_file}")
    sys.exit(1)

print(f"[zenoh-baseline] PASS: all zenoh packages pinned to {expected}")
PY
