# Phase 0 Golden Fixture Behavior Locks

Date: 2026-07-04

## Scope

This slice added fixture-backed behavior locks for current public JSON surfaces:

- ADS client disabled status.
- ADS server disabled/default status.
- OPC UA server/client communication capability status when not configured.
- Legacy `status.io_drivers` JSON.
- Negative status behavior proving disabled, missing, or faulted connectors do
  not report healthy on existing public surfaces.
- Deterministic discovery responses for current Modbus TCP listener behavior,
  MQTT TCP listener behavior, ADS client no-target discovery, ADS server
  unavailable warning, OPC UA server-only warning, and EtherCAT this-host
  warning.

The Modbus and MQTT listener fixtures intentionally preserve the current
honesty problem: a plain TCP listener is reported with `confidence: "observed"`.
The tests normalize the ephemeral localhost port to `$ADDR` and
`$SANITIZED_ADDR` so the fixture remains stable.

## Files

- `crates/trust-runtime/src/control/tests/goldens.rs`
- `crates/trust-runtime/tests/fixtures/connectors/phase0/ads/client_disabled.json`
- `crates/trust-runtime/tests/fixtures/connectors/phase0/ads/server_disabled.json`
- `crates/trust-runtime/tests/fixtures/connectors/phase0/opcua/capabilities_not_configured.json`
- `crates/trust-runtime/tests/fixtures/connectors/phase0/io_driver/status_io_drivers.json`
- `crates/trust-runtime/tests/fixtures/connectors/phase0/discovery/*.json`

## Fixture Update Notes

- Added `phase0/ads/server_disabled.json` from the current
  `ads.server.status` disabled/default output. The first fixture draft included
  null fields inside `status.connections`; the remote fixture comparison proved
  those fields are omitted by the public serde contract, so the fixture was
  corrected to match current output exactly.
- Added `phase0/discovery/ads_client_no_targets.json` from a loopback `/32`
  ADS client discovery request. This locks the current no-candidate ADS client
  discovery response without requiring a live TwinCAT/ADS target.
- No existing Phase 0 fixture value was deliberately changed in this slice.

## Local Validation

```sh
rustfmt --check crates/trust-runtime/src/control/tests/goldens.rs
git diff --check -- crates/trust-runtime/src/control/tests/goldens.rs crates/trust-runtime/tests/fixtures/connectors/phase0
```

Result:

```text
rustfmt --check passed for crates/trust-runtime/src/control/tests/goldens.rs.
git diff --check passed for the touched Phase 0 golden test and fixtures.
```

## Remote Validation

Remote path: `$HOME/projects/trust-platform-rtconn-validation`

Disk preflight:

```sh
ssh trust-builder 'df -hT /home/johannes /tmp && du -xhd1 "$HOME/projects" 2>/dev/null | sort -h | tail -20 && du -xhd1 "$HOME/.cache" 2>/dev/null | sort -h | tail -20'
```

Result:

```text
/home/johannes: 79G free
/tmp: 6.8G free
$HOME/.cache/codex-targets: 4.6G after the warmed focused test target
```

Command:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" cargo test -p trust-runtime phase0 --lib'
```

Result:

```text
running 5 tests
test control::tests::phase0_missing_or_failed_connectors_do_not_report_healthy ... ok
test control::tests::phase0_io_driver_status_matches_legacy_golden ... ok
test control::tests::phase0_ads_status_matches_disabled_goldens ... ok
test control::tests::phase0_opcua_status_matches_capability_goldens ... ok
test control::tests::phase0_discovery_matches_current_goldens ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 827 filtered out
```

## Common Gate Attempt

Command:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && cargo fmt --check --all'
```

Result: failed on pre-existing formatting drift outside this Phase 0 slice:

```text
crates/trust-debug/src/adapter/tests_part_04.rs
crates/trust-runtime/tests/io_cycle.rs
```

The Phase 0 touched-file checks passed, but `RTCONN-P0-GATE-003` remains open
because the common per-phase gate is not green on the exact validation copy.

## Common Gate Re-run

Date: 2026-07-04

The workspace formatting drift was fixed with rustfmt-only edits in:

- `crates/trust-debug/src/adapter/tests_part_04.rs`
- `crates/trust-runtime/tests/io_cycle.rs`

Local check:

```sh
rustfmt --check crates/trust-debug/src/adapter/tests_part_04.rs crates/trust-runtime/tests/io_cycle.rs
git diff --check -- crates/trust-debug/src/adapter/tests_part_04.rs crates/trust-runtime/tests/io_cycle.rs
```

Result: passed.

Remote path: `$HOME/projects/trust-platform-rtconn-validation`

Commands:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && cargo fmt --check --all'
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" just fmt'
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" just clippy'
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" just test'
```

Results:

```text
cargo fmt --check --all: passed
just fmt: passed
just clippy: passed, finished in 3m25s
just test: passed; cargo-nextest missing fallback ran cargo test -p trust-runtime --lib with 819 passed, 16 ignored
```

## Remaining Phase 0 Gaps

- None for Phase 0. The common gate is now green on the isolated validation copy.
