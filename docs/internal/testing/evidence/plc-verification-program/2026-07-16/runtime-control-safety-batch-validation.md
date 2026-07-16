# Runtime control safety batch validation

Date: 2026-07-16

Product and architecture validation checkpoint:
`83471ef90e483c859bf9961941f7e387b52d65e7`

Final report source checkpoint:
`16e332f091ff984b3e92ee343adbcb5b9ca23be1`

Report refresh and final verification checkpoint:
`d0c7594b1d2254d66a1b61bfc5c9d5b7e4294133`

## Outcome

This batch closed four written specification gaps and promoted five invariants
from S0 to G1 using producer-authentic targeted proof or current-contract
behavior locks:

- `SPEC_GAP_DEBUG_AUTHORIZATION_001`;
- `SPEC_GAP_CONTROL_AUTHORIZATION_MATRIX_001`;
- `SPEC_GAP_DEBUG_PAUSE_WATCHDOG_001`; and
- `SPEC_GAP_RUNTIME_FORCE_LIFECYCLE_001`.

The promoted invariants are `DEBUG_AUTH_001`, `DEBUG_PAUSE_001`,
`SEC_AUTHZ_001`, `RT_SAFE_FORCE_001`, and `RT_SAFE_PANIC_001`. No broad gate
was converted into causal promotion evidence, so the batch stops honestly at
G1.

## Product defect and fix

The authorization trace produced a genuine red result at
`77e8f3fd2605bf991033d09aa96031930d482d8d`: five denied-role cases returned
human-readable denials without the written stable `insufficient_role` wire
code. The three allowed Admin and Engineer cases already passed.

Commit `ff5fb52f6f7fbbe724acdf9d668e62ddf1cfce5d` fixes the product behavior by
classifying reviewed control operations in one internal registry and returning
the stable denial code before dispatch. The paired green proof passes all eight
cases with the same case-file and execution-contract digests.

Pause/watchdog exclusion, force lifecycle clearing, and scan-thread panic
containment were already correct. Their baseline and compare records have
identical per-case result digests, so no product defect or manufactured red was
claimed for those behaviors.

## Tests and contracts

The batch adds or binds four hand-authored, cataloged runtime trace runners:

- `TEST_CONTROL_AUTHORIZATION_TRACE_001`;
- `TEST_DEBUG_PAUSE_TRACE_001`;
- `TEST_RUNTIME_FORCE_LIFECYCLE_001`; and
- `TEST_RUNTIME_PANIC_TRACE_001`.

The product implementation keeps operation classification in
`control/operation_registry.rs`, policy evaluation in `control/policy.rs`, and
the authorization trace assertions in their own test module. The pause and
panic traces are separate integration-test modules. This keeps the runtime
control boundary single-purpose and avoids extending an existing large test or
policy file.

## Remote product validation

The heavy Rust gates ran once at the end of product and architecture work on
the clean `trust-builder` checkpoint
`83471ef90e483c859bf9961941f7e387b52d65e7`, using
`CARGO_TARGET_DIR=$HOME/.cache/codex-targets/trust-platform-gate`:

- `just fmt`: passed;
- `just clippy`: passed;
- `just test-all`: passed with no failures;
- `cargo test -p trust-runtime --test api_smoke`: 3/3 passed;
- `cargo test -p trust-runtime --test debug_control`: 20/20 passed;
- `cargo test -p trust-runtime --test complete_program`: 1/1 passed; and
- `cargo test -p trust-runtime --test runtime_reliability`: 4/4 passed.

The builder toolchain was `rustc 1.97.0 (2d8144b78 2026-07-07)` and
`cargo 1.97.0 (c980f4866 2026-06-30)`. The later commits through
`d0c7594b1d2254d66a1b61bfc5c9d5b7e4294133` change only verification tests,
generated evidence, and report bindings, so the Rust checkpoint remains the
exact product candidate validated by the broad gates.

## Final verification validation

At the clean report checkpoint on `trust-builder`:

- `python3 scripts/run_verification_focused_tests.py`: 782/782 passed in
  411.945 seconds;
- `python3 scripts/validate_verification_metadata.py`: 550 records before this
  batch-validation row was indexed;
- `scripts/verification_metadata_gate.sh`: passed, including the Phase 16
  report-only product fence and all generated-case checks;
- `python3 scripts/check_verification_tooling_selftests.py --report
  docs/internal/testing/evidence/plc-verification-program/2026-07-11/p6a-tooling-selftest-fixture-report.md`:
  33/33 production-catcher fixtures passed;
- all 15 installed report pairs passed their production at-rest validators;
- `python3 scripts/check_diagram_drift.py`: passed; and
- `git diff --check`: passed.

Before the final checkpoint, the canonical focused suite exposed seven stale
verification tripwires caused by the new Rust facts, catalog mappings, coverage
states, and eligible oracle. Commit
`16e332f091ff984b3e92ee343adbcb5b9ca23be1` refreshes only those measured
expectations and retargets one self-test from the now-proven force invariant to
the still-open MQTT lifecycle invariant. The focused repair run passed 79/79,
and the production-catcher self-test remained 33/33.

## Generated report refresh

All 15 report pairs were generated from clean detached worktrees at
`16e332f091ff984b3e92ee343adbcb5b9ca23be1` with timestamp
`2026-07-16T10:33:00+02:00`. Every generator and at-rest validator exited zero,
and each worktree returned clean before its next generator.

| Report | JSON SHA-256 |
| --- | --- |
| Test-class completeness | `7fe3a03e7e3f9d60544ac7bbf4d34ac73bc1c1aca18390c2fcb8bdba692a48ef` |
| Coverage-matrix gaps | `324e5f0d02f71a6f64f311eed394b38e73d7704f16982e1759bd6b6aed805a9b` |
| Malformed-input coverage | `b2bfadecdcf5723cfe5caab459223ed833fd0990a92f78ec5efec9bc765d4013` |
| Unmapped-test debt | `5a227b1fca9cd60c22936c5ca55807df8ea0fcda3354f36c4c50faf81ba8cdfd` |
| Test-refactor assessment | `860ffc8dfb5328eb93c2a7e511af6808bc3a379f5a9c8ada57528910db30a2ce` |
| Ignored-test inventory | `35171c5e163fc10e96f92e8cf9ad62b559dc2837d35d2bea470b0edf8f25e4ec` |
| Phase 5 suite audit | `51907d6d0880e902410a9bb2afa8e87475f6b1915e474889e45d8eb9cb4a7b65` |
| Invariant-seed audit | `70fc5c1d4871944bde62c79c114e1c0e2385515035a3573ae5c3df18cd1d44c9` |
| Specification completeness | `996f74cd9e09d44743e19be88d6b099875c157e93ff96fd013c04dc48badd940` |
| Requirement/oracle audit | `5c5df957e6b0fee3c2cdf3777702bb35d3fc0425c9c5df936941497931caebda` |
| Conformance alignment | `77afc53ff0fa38644d3dd4bc332058d7d885f53c9f668a37a97f5dc5f5119632` |
| Runtime-anomaly audit | `58f6205a8aecebfe5142b400148a4fc1f871eea3e28e65307ddfbcb2c09808dc` |
| Fuzz-program audit | `b894ca54d4bcf40735826ef757096553280a48e614905620591aedb123f2d49e` |
| Mutation program | `082593560d0172bc08a359e6a5002c14fa3c58a9adafc3d87e217cb724412ee3` |
| Specification-source audit | `79dd1b4d57dac3819d5e1d5ebdf591a2338c61d7566cb8bfae30b1191a04a7b1` |

## Honest remaining posture

- The gap register contains 18 closed, 15 open, and 1 `spec_updated` record.
- The invariant register contains 53 records: 39 at S0, 5 at G1, and 9 at G2.
- Coverage contains 21 covered cells, 24 `spec_gap` cells, and 23 `gap_open`
  cells; 63/80 required family slots remain missing.
- Requirement/oracle mapping reports 29 eligible and 24 missing oracles.
- The hand-owned catalog maps 182/3,966 scanner facts; 3,784 remain unmapped.
- Conformance alignment remains 0/21 explicitly linked.
- CI, workflows, suites, approved proof producers, and enforcement posture are
  unchanged.
- Version metadata is synchronized at 0.24.50; tagging and public release are
  deferred until the change reaches `main`.

