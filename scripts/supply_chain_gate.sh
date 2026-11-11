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
python3 -m unittest scripts.tests.test_supply_chain_gate

# Positional arguments preserve empty lists and literal spaces on system Bash 3.2
# as well as current Bash. They avoid Bash 4's mapfile and empty-array nounset traps.
set --
while IFS= read -r argument; do
  set -- "$@" "$argument"
done < <(python3 - <<'PY'
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
  local graph=$1
  shift
  cargo audit --json --file "$graph" "$@"
}

# The root, the standalone firmware workspaces and the two fuzz harnesses are independent locked
# graphs, each read --locked, so that a lock its manifests have left behind fails here too
# (specification 24, "Dependency Exceptions"). Retain every reachable result instead of allowing
# one graph failure to hide the other.
status=0
cargo deny --locked check advisories licenses bans sources || status=1
cargo deny --locked --manifest-path firmware/trust-esp32c6/Cargo.toml \
  --config "$ROOT_DIR/deny.toml" check advisories licenses bans sources || status=1
cargo deny --locked --manifest-path firmware/trust-stm32u5a9/Cargo.toml \
  --config "$ROOT_DIR/deny.toml" check advisories licenses bans sources || status=1
# The fuzz harnesses build the root's crates: when a dependency change leaves their locks behind,
# refresh each from the root lock, so that it keeps the versions the product ships, for example
# `cp Cargo.lock fuzz/Cargo.lock && cargo metadata --format-version 1 --manifest-path fuzz/Cargo.toml >/dev/null`.
cargo deny --locked --manifest-path fuzz/Cargo.toml \
  --config "$ROOT_DIR/deny.toml" check advisories licenses bans sources || status=1
cargo deny --locked --manifest-path crates/trust-ads-server/fuzz/Cargo.toml \
  --config "$ROOT_DIR/deny.toml" check advisories licenses bans sources || status=1
run_cargo_audit Cargo.lock "$@" | python3 scripts/check_cargo_audit_policy.py \
  --allowlist scripts/cargo-audit-yanked-allowlist.json || status=1
run_cargo_audit firmware/trust-esp32c6/Cargo.lock "$@" | \
  python3 scripts/check_cargo_audit_policy.py \
    --allowlist scripts/firmware-cargo-audit-yanked-allowlist.json || status=1
run_cargo_audit firmware/trust-stm32u5a9/Cargo.lock "$@" | \
  python3 scripts/check_cargo_audit_policy.py \
    --allowlist scripts/firmware-cargo-audit-yanked-allowlist.json || status=1
run_cargo_audit fuzz/Cargo.lock "$@" | python3 scripts/check_cargo_audit_policy.py \
  --allowlist scripts/cargo-audit-yanked-allowlist.json || status=1
run_cargo_audit crates/trust-ads-server/fuzz/Cargo.lock "$@" | \
  python3 scripts/check_cargo_audit_policy.py \
    --allowlist scripts/cargo-audit-yanked-allowlist.json || status=1
exit "$status"
