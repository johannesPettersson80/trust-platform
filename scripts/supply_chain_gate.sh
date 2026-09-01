#!/usr/bin/env bash
set -euo pipefail

python3 scripts/check_dependency_exceptions.py
python3 -m unittest discover \
  -s .codex/skills/trust-ci-release-gates/scripts \
  -p 'release_candidate_*_tests.py'
cargo deny check advisories licenses bans sources
python3 scripts/test_check_cargo_audit_policy.py

mapfile -t audit_ignore_args < <(python3 - <<'PY'
import tomllib

with open("deny.toml", "rb") as source:
    data = tomllib.load(source)
for entry in data.get("advisories", {}).get("ignore", []):
    advisory = entry.get("id")
    if advisory:
        print("--ignore")
        print(advisory)
PY
)

run_cargo_audit() {
  if (( ${#audit_ignore_args[@]} )); then
    cargo audit --json "${audit_ignore_args[@]}"
  else
    cargo audit --json
  fi
}

run_cargo_audit | python3 scripts/check_cargo_audit_policy.py \
  --allowlist scripts/cargo-audit-yanked-allowlist.json
