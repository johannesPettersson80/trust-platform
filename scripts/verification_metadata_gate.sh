#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
python3 scripts/validate_verification_metadata.py
python3 -m scripts.verification.phase16_readiness
python3 - <<'PY' | while IFS=$'\t' read -r generator invariant; do
import sys
import tomllib
from pathlib import Path

GENERATORS = {
    "gen_cases.py v1": "gen_cases.py",
    "gen_cases_v2.py v1": "gen_cases_v2.py",
}

data = tomllib.loads(Path("verification/test-catalog.toml").read_text())
for record in data.get("tests", []):
    if "case_file" not in record:
        continue
    case_path = Path(record["case_file"])
    case_data = tomllib.loads(case_path.read_text())
    provenance = case_data.get("case_provenance_kind", "generated_decision_table_v1")
    if provenance == "hand_authored_state_machine_v1":
        continue
    if provenance != "generated_decision_table_v1":
        print(
            f"{record.get('id', '<unknown>')} has unsupported case provenance {provenance!r}",
            file=sys.stderr,
        )
        raise SystemExit(1)
    invariants = record.get("invariants", [])
    if len(invariants) != 1:
        print(f"{record.get('id', '<unknown>')} case_file row must name exactly one invariant", file=sys.stderr)
        raise SystemExit(1)
    generator = case_data.get("generator")
    generator_script = GENERATORS.get(generator)
    if generator_script is None:
        print(
            f"{record.get('id', '<unknown>')} has unsupported case generator {generator!r}",
            file=sys.stderr,
        )
        raise SystemExit(1)
    print(f"{generator_script}\t{invariants[0]}")
PY
  python3 "scripts/$generator" --invariant "$invariant" --check
done
