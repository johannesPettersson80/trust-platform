#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

python3 scripts/check_dependency_exceptions.py
python3 -m unittest scripts.tests.test_cargo_target_lease
python3 -m unittest discover \
  -s .codex/skills/trust-ci-release-gates/scripts \
  -p 'release_candidate_*_tests.py'
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
    cargo audit --json --file "$1" "${audit_ignore_args[@]}"
  else
    cargo audit --json --file "$1"
  fi
}

# The root and standalone firmware are independent locked graphs. Retain every
# reachable result instead of allowing one graph failure to hide the other.
status=0
cargo deny --locked check advisories licenses bans sources || status=1
cargo deny --locked --manifest-path firmware/trust-nucleo-f401re/Cargo.toml \
  check --config "$ROOT_DIR/deny.toml" advisories licenses bans sources || status=1
run_cargo_audit Cargo.lock | python3 scripts/check_cargo_audit_policy.py \
  --allowlist scripts/cargo-audit-yanked-allowlist.json || status=1
run_cargo_audit firmware/trust-nucleo-f401re/Cargo.lock | \
  python3 scripts/check_cargo_audit_policy.py \
    --allowlist scripts/firmware-cargo-audit-yanked-allowlist.json || status=1
exit "$status"
