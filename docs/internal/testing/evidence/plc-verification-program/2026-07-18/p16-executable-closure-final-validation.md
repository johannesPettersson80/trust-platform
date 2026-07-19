# Phase 16 Executable Closure Final Validation

Date: 2026-07-18

This record validates the executable-closure checkpoint at clean commit
`2c372947cf3bdf229c142f3d0d583f1369c4ee21`. It records validation only. It
does not create product proof or change any suite, CI, stop gate, approved
proof producer, skill, or agent instruction.

## Local Verification Surface

The following commands ran from the clean local checkout:

- `python3 scripts/run_verification_focused_tests.py`: 828 tests passed, zero
  failures, in 1106.122 seconds.
- `scripts/verification_metadata_gate.sh`: passed with 732 metadata records,
  including the Phase 16 readiness fence and all four generated case checks.
- `python3 scripts/check_verification_tooling_selftests.py`: 33 of 33 fixtures
  passed.
- `python3 scripts/check_ignored_test_staleness.py`: 23 discovered and 23
  registered ignored tests, with zero unknown and zero catalog-mapped.
- `python3 scripts/check_test_catalog_staleness.py`: 249 catalog records were
  validated against 4,023 current scanner facts.
- `python3 scripts/check_vscode_test_registration.py`: 458 facts across 39
  test files matched 39 registrations.
- `python3 scripts/validate_test_refactor_proposals.py`: one proposal, zero
  redirects, 249 catalog records, and 4,023 scanner facts validated.

## Trust-Builder Broad Gates

The final gates ran in the isolated clean checkout
`/home/johannes/.cache/trust-platform-final-validation-2c372947` on
`trust-builder`, at the exact checkpoint above, with Rust and Cargo 1.97.0.
The canonical remote checkout was not modified.

- `just fmt`: passed in 3 seconds and produced no tracked diff.
- `just clippy`: passed in 112 seconds.
- `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/trust-platform-gate CARGO_INCREMENTAL=0 just test-all`:
  passed with exit status 0 in 636 seconds.

The explicit `CARGO_INCREMENTAL=0` setting changes build caching, not test
selection or runtime semantics. It prevented the warmed shared target from
exhausting the builder filesystem while preserving the exact `just test-all`
test surface.

## Infrastructure Recovery

Two earlier `just test-all` attempts stopped on builder disk exhaustion after
161 and 403 seconds. Neither attempt reported a test assertion failure. The
run was stopped each time, disk use was re-audited, and only generated build
and inactive cache outputs were removed before retrying.

The first no-incremental run then reached the test phase and exposed an
isolated-checkout fixture-topology problem: `openot_capstone_fenced_cross_process`
timed out waiting for its producer because the declared sibling
`../../../open-ot-ref` was absent beside the validation checkout. The source
fixture was restored as a symlink to the existing checkout at pinned revision
`137f0e765f085c262651f479be35298b836ac891`. Its three unrelated dirty files
under `examples/reactor` were preserved and are outside the consumed
`st/iec61131/src` tree.

The focused capstone rerun then passed 1 of 1 in 37.11 seconds with zero loss,
retry, rejection, or stale-record violations. The complete 636-second
`test-all` rerun also passed, including the same capstone test in 37.61
seconds. This was missing external fixture topology, not a product or test
assertion defect; no source change was made for it.

## Outcome And Boundaries

This executable batch found and fixed three real verification defects before
the final gates:

- the ADS fuzz harness no longer fails to compile against the current API;
- fuzz smoke runners now reject Cargo filters that execute zero tests;
- the WAN allowlist smoke gate now selects a live test instead of silently
  passing with zero executed tests.

The bounded 17-target fuzz campaign completed with zero crashes, and all 242
mapped catalog tests passed. The final broad gate confirms that those changes
did not regress the full workspace test surface.

The remaining boundaries are unchanged and explicit:

- `VERIF-P8-002` remains open because 52 reviewed associations do not prove an
  exhaustive disposition for 3,220 live Rust facts.
- `VERIF-P16-006` remains open for the complete catalog denominator and the
  Phase 8 exhaustive semantic nonmapping review.
- `SPEC_GAP_BEHAVIOR_LOCKED_PUBLIC_CLAIM_001` and
  `SPEC_GAP_SOURCE_BUILD_PUBLIC_CLAIM_001` remain `spec_updated`; the current
  G1 invariants lack an approved causal broad-remote proof for public-claim
  closure.
- No CI enforcement, suite, approved proof producer, stop gate, skill, or
  agent-instruction change is included.
