# Phase 18 managed-fleet focused proof

Date: 2026-07-23

## Scope

- `crates/trust-runtime/src/bin/trust-runtime/fleet.rs`
- `crates/trust-runtime/src/bin/trust-runtime/lifecycle.rs`
- `crates/trust-runtime/src/bin/trust-runtime/cli/fleet.rs`
- `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs`
- `crates/trust-runtime/tests/fleet_lifecycle_command.rs`
- `SPEC_RUNTIME_ENGINE_001#6-8-3-managed-local-fleet-scaffolding`

## Automatic catalogue and gap result

`scripts/scan_test_catalog.py` discovered nineteen focused baseline tests:
eight fleet-scaffolding units, seven lifecycle units, three fleet CLI parser
tests, and one shipped lifecycle integration. None was registered in the
curated verification catalogue.

The existing runtime specification already covered the core scaffold,
credential, manifest, control-envelope, log-tail, and lifecycle behavior.
Review added only two missing details: relative versus absolute hand-edited
project paths, and the distinction between pre-spawn log setup and post-spawn
PID publication. This is truST product behavior outside IEC 61131-3; no IEC
decision or deviation was added.

Eleven compact assertions close concrete gaps without duplicating the existing
happy-path lifecycle:

- automatic distinct available loopback ports;
- rejection of whitespace-padded names before mutation;
- malformed-manifest rejection before project mutation;
- deterministic generated sources and resource-name derivation;
- zero-port and unpadded-credential boundaries;
- missing/non-string authentication configuration;
- padded-token normalization matching runtime parsing;
- malformed or ambiguous connected control envelopes;
- endpoint truth over stale PID metadata;
- relative/absolute project-path resolution, failed-start cleanup, and direct
  proof that a live failed-start child is terminated and reaped.

## Red and fixes

Two focused regressions failed before their production changes:

- `fleet_runtime_add_rejects_whitespace_name_before_mutation` showed that
  `add_runtime(" cell ")` returned a successful response for normalized name
  `cell` and created the project. The validator now rejects leading or trailing
  whitespace before filesystem or manifest mutation.
- `fleet_runtime_auth_reader_matches_runtime_token_normalization` showed the
  lifecycle reader returning `"  padded-token  "` while runtime parsing expects
  `"padded-token"`. Lifecycle auth loading now applies full runtime
  configuration validation and the same nonempty-token trimming before any
  control exchange. Absent, blank, non-string, malformed, and missing auth
  configuration remain errors rather than false `stopped` status.

## Focused remote proof

Proof environment:

- host: `trust-builder`;
- checkout: `/home/johannes/projects/trust-platform-p18-parser-proof-20260723`;
- checkout state: detached at `3479807f26f56b8347d588d658cd9301d6735050`
  with the frozen managed-fleet source and tests synchronized from the working
  candidate;
- toolchain: `rustc 1.97.0 (2d8144b78 2026-07-07)`,
  `cargo 1.97.0 (c980f4866 2026-06-30)`;
- platform: `Linux 7.0.0-15-generic x86_64`;
- target cache:
  `$HOME/.cache/codex-targets/trust-platform-gate`;
- temporary directory: the proof checkout's `.tmp`.

Exact commands, each run from the proof checkout with `TMPDIR="$PWD/.tmp"` and
`CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-gate"`:

```text
cargo test -p trust-runtime --bin trust-runtime fleet::tests:: -- --nocapture --test-threads=1
cargo test -p trust-runtime --bin trust-runtime fleet::lifecycle::tests:: -- --nocapture --test-threads=1
cargo test -p trust-runtime --bin trust-runtime cli::tests::parse_fleet -- --nocapture --test-threads=1
cargo test -p trust-runtime --test fleet_lifecycle_command managed_runtime_lifecycle_runs_through_the_shipped_cli -- --exact --nocapture --test-threads=1
```

Command results recorded from the complete console output:

- fleet scaffolding unit module: 12 passed;
- fleet lifecycle unit module: 14 passed;
- fleet CLI parser filter: 3 passed;
- shipped add/list/start/status/logs/stop integration: 1 passed.

The integration validates JSON add and list output, starts the managed runtime,
observes endpoint-backed running status, reads logs, stops it, and confirms PID
cleanup.

## Deferred final proof

No broad workspace test, `just fmt`, `just clippy`, or `just test-all` was run.
Assertion reachability, mutation sensitivity, Windows process variation, and
the single final frozen-candidate gate remain open under the Phase 18 cadence.
