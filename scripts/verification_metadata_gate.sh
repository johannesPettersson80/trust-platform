#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
python3 scripts/validate_verification_metadata.py
python3 -m scripts.verification.phase16_readiness
python3 - <<'PY' | while IFS= read -r invariant; do
import sys
import tomllib
from pathlib import Path

data = tomllib.loads(Path("verification/test-catalog.toml").read_text())
for record in data.get("tests", []):
    if "case_file" not in record:
        continue
    invariants = record.get("invariants", [])
    if len(invariants) != 1:
        print(f"{record.get('id', '<unknown>')} case_file row must name exactly one invariant", file=sys.stderr)
        raise SystemExit(1)
    print(invariants[0])
PY
  python3 scripts/gen_cases.py --invariant "$invariant" --check
done
