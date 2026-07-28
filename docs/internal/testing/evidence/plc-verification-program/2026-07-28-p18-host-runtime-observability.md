# Phase 18 host-runtime observability focused proof

Date: 2026-07-28

## Scope and authority

This coherent batch reviews fifteen existing host-runtime tests across:

- `crates/trust-runtime/src/host/execution_backend.rs`;
- `crates/trust-runtime/src/host/historian/tests.rs`;
- `crates/trust-runtime/src/host/metrics.rs`;
- `crates/trust-runtime/tests/api_smoke.rs`.

`SPEC_RUNTIME_ENGINE_001` and the 2026-07-28 product decision now own three
bounded contracts:

- execution-backend parsing, fresh construction, lazy VM materialization, and
  the effective-backend metrics projection;
- typed historian capture, sample intervals, exact and `retain.*` allowlisting,
  orderly file-backed reload, and the reviewed Prometheus projection;
- the five-sample nonwrapped cycle window, call identity and contribution,
  ranking, and disable-clear-reenable profiling lifecycle.

These are truST host-product contracts outside IEC 61131-3. No IEC decision or
deviation entry was added. The semantic lock states the excluded claims
explicitly, including arbitrary-source and IEC-wide correctness, other
production backends, cross-version compatibility, crash durability,
concurrency, wildcard grammar, window rollover, performance budgets, and a
complete Prometheus schema.

## Test-first validator proof

The assignment generator began with nine focused tests. Before its
implementation, the exact command

```text
python3 -m scripts.verification.host_runtime_observability_assignments_tests
```

failed all nine tests because
`scripts.verification.host_runtime_observability_assignments` did not exist.
After the isolated generator was implemented, the same nine tests passed.

A tenth test then required all four source paths to resolve through the closed
`runtime-remaining` nextest profile. Before the profile change, its four
subcases failed with `outside the closed runtime remaining path set`. The
minimal profile extension made the complete ten-test command green:

```text
Ran 10 tests in 1.240s
OK
```

The generator selects exactly fifteen live denominator identities and
seventy-five current assertion identities. Its reviewed routes contain 64
oracle-bound assertions, 11 setup or precondition assertions, and no
non-oracle observations. Unknown tests, unknown assertions, expression drift,
denominator identity drift, and post-map identity mismatches fail closed.

The pre-edit planner command used `--intent test-refactor` and listed every
then-changed specification, invariant, and runtime-test path. It exited 6 on
the pre-existing repository metadata baseline: more than one thousand
unrelated validation findings include
`VM_SEAM_DECLARED_TYPE_001 case_family is only allowed for equals partitions`
and legacy statuses unknown to the current validator. This harness failure is
not counted as expected-red product proof, does not override the focused green
evidence, and remains blocking before a feature push.

## Focused remote execution

Proof environment:

- host: `trust-builder` (`scena-rust-builder`);
- canonical source: `/home/johannes/projects/trust-platform`, branch
  `plc-verification-program`, HEAD
  `cde2f9e7bbc09d86e8ff72453ac88c24feafaf5f`;
- isolated destination:
  `/home/johannes/projects/trust-platform-p18-host-runtime-15-cde2f9-validation`,
  detached base `3479807f26f56b8347d588d658cd9301d6735050`;
- exact candidate source synchronized with `rsync`, excluding build caches;
- `AGENTS.md` and the complete `.codex/skills/**` copied separately and
  verified byte-for-byte against the canonical source;
- warmed target:
  `$HOME/.cache/codex-targets/trust-platform-gate`;
- disk preflight: 54 GiB available under `/home/johannes` and 3.3 GiB under
  `/tmp`, sufficient for this focused gate but not used for a broad gate.

Each evidence-producing command was visibly executed through
`ssh trust-builder`. The four commands shared these arguments:

```text
CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-gate" \
python3 scripts/run_frozen_rust_nextest_batch.py \
  --profile runtime-remaining \
  --expected-catalog-mapped 0 \
  --nonmapping-reason-code no_reviewed_spec_or_invariant_binding \
  --selection-inventory target/gate-artifacts/verification/host-runtime-15-selected.json
```

The path-specific selections and results were:

| path | selected | result |
| --- | ---: | --- |
| `crates/trust-runtime/src/host/execution_backend.rs` | 4 | 4 passed, 1,097 skipped outside the exact filter |
| `crates/trust-runtime/src/host/historian/tests.rs` | 4 | 4 passed, 1,097 skipped outside the exact filter |
| `crates/trust-runtime/src/host/metrics.rs` | 4 | 4 passed, 1,097 skipped outside the exact filter |
| `crates/trust-runtime/tests/api_smoke.rs` | 3 | 3 passed, 0 skipped |

The initial remote Python validator run lacked the intentionally excluded
ignored assertion inventory under `target/`. That harness-only
`FileNotFoundError` is not test evidence. After copying only the frozen
15-test inventory, the same ten focused validator tests passed on the builder.

## Atomic catalog result

The semantic-lock generator produced four path plans. All four compiled in
dry-run mode, and all four applied through
`scripts/catalog_frozen_static_tests.py --apply`. The transaction:

- catalog-mapped all 15 selected tests;
- bound 75 assertions to 14 exact behavior partitions across
  `RT_EXECUTION_BACKEND_SELECTION_001`,
  `RT_HISTORIAN_RECORDING_001`, and
  `RT_RUNTIME_METRICS_PROFILING_001`;
- preserved 11 reviewed setup/precondition dispositions;
- left no unclassified or non-oracle assertion;
- remained idempotent against the post-map denominator.

The metadata-only frozen gap report, generated without a repository scan,
records:

| population | before | after |
| --- | ---: | ---: |
| frozen live tests | 4,393 | 4,393 |
| `catalog_mapped` | 3,717 | 3,732 |
| `reviewed_nonmapping` | 676 | 661 |
| missing specification authority | 169 | 154 |
| owned nonmapping | 507 | 507 |
| confirmed missing tests | 0 | 0 |

## Deferred final proof

No broad workspace test, `just fmt`, `just clippy`, or `just test-all` was run.
`VERIF-P18-003` remains globally open. Assertion reachability, mutation
sensitivity, the remaining specification and mapping populations, and the
single final frozen-candidate broad gate remain open under the Phase 18
cadence.
