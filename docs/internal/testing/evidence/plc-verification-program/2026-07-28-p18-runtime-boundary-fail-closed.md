# Phase 18 runtime boundary fail-closed focused proof

Date: 2026-07-28

## Scope and authority

This coherent batch reviews twelve existing tests across:

- `crates/trust-runtime/tests/boundary_resolver.rs`;
- `crates/trust-runtime/tests/harness_fail_closed.rs`;
- `crates/trust-runtime/tests/init_fail_closed.rs`;
- `crates/trust-runtime/src/value/reference.rs`.

The batch reuses five existing invariants:

- `RT_HARNESS_BOUNDARY_IO_001`;
- `RT_HOST_POU_EXECUTION_001`;
- `RT_VARIABLE_STORAGE_REFERENCE_001`;
- `RT_CORE_VALUE_PROGRAM_MODEL_001`;
- `VM_REFERENCE_RESOLUTION_001`.

The runtime-semantics specification now states the exact indexed/dotted,
unknown, ambiguous, typo-mutation, typo-binding, declared-null-reference, and
queued-debug failure boundaries. It also retains the existing reference-path
ordering, checked array-offset, and invalid-reference authority.

IEC 61131-3 Ed.3 §6.6.6.5.1 states that the special `NULL` value is the initial
value of an interface-typed variable when it is not initialized otherwise.
The declaration spec now cites that exact clause for the existing default-NULL
rule. No IEC decision or deviation was added. Candidate reporting, harness
error codes and snapshots, fallback-creation policy, queued debug fault
visibility, and helper representation remain truST product behavior recorded
in the runtime specification and product-decision log.

## Planner and test-first validator proof

The required pre-batch planner command used `--intent test-refactor` and listed
all four reviewed source paths, the affected specifications, and the five
existing invariants. It exited 6 after the known repository-wide metadata
validation baseline, including
`VM_SEAM_DECLARED_TYPE_001 case_family is only allowed for equals partitions`,
legacy `specified` statuses rejected by the current validator, and more than
1,100 additional unrelated findings. This is a blocking planner baseline, not
expected-red product evidence, and the planner was not repeated during the
slice.

Before the new mapper implementation, the exact focused command

```text
python3 -m scripts.verification.runtime_boundary_fail_closed_assignments_tests
```

reported nine mapper-test failures because
`scripts.verification.runtime_boundary_fail_closed_assignments` was missing.
The same run also reported four exact profile failures because the reviewed
paths were outside the closed `runtime-remaining` set. After the isolated
mapper and minimal profile entries were implemented, the complete command was
green:

```text
Ran 10 tests in 1.297s
OK
```

The generator selects exactly twelve live denominator identities and
forty-five current assertion identities. Its semantic lock assigns 36 direct
product or IEC oracles, nine setup/precondition assertions, and no non-oracle
observations. It rejects unknown assertions, expression drift, denominator
identity drift, and wrong post-map generated IDs.

## Focused remote execution

Proof environment:

- host: `trust-builder` (`scena-rust-builder`);
- canonical source: `/home/johannes/projects/trust-platform`, branch
  `plc-verification-program`, HEAD
  `5c7131f5a4bd2c8bf616561a19ce40982f3954b3`;
- isolated destination:
  `/home/johannes/projects/trust-platform-p18-runtime-boundary-12-5c7131-validation`,
  detached base `3479807f26f56b8347d588d658cd9301d6735050`;
- exact candidate source synchronized with `rsync`, excluding build caches;
- `AGENTS.md` and the complete `.codex/skills/**` copied separately and
  verified byte-for-byte, including `trust-lsp-iec`;
- warmed target:
  `$HOME/.cache/codex-targets/trust-platform-gate`;
- disk preflight: 45 GiB available under `/home/johannes` and 3.3 GiB under
  `/tmp`, sufficient for this focused proof but below the final broad-gate
  target.

Every evidence-producing command was visibly executed through
`ssh trust-builder`. Each path used
`scripts/run_frozen_rust_nextest_batch.py --profile runtime-remaining`, the
saved 12-test selection inventory, exact
`no_reviewed_spec_or_invariant_binding` selection, and zero pre-existing mapped
companions.

| path | selected | result |
| --- | ---: | --- |
| `crates/trust-runtime/tests/boundary_resolver.rs` | 3 | 3 passed |
| `crates/trust-runtime/tests/harness_fail_closed.rs` | 3 | 3 passed |
| `crates/trust-runtime/tests/init_fail_closed.rs` | 3 | 3 passed |
| `crates/trust-runtime/src/value/reference.rs` | 3 | 3 passed, 1,098 skipped outside the exact filter |

The same ten mapper/profile tests also passed in the isolated builder
checkout.

## Atomic catalog result

The semantic-lock generator produced four path plans. All four compiled in
dry-run mode and all four applied through
`scripts/catalog_frozen_static_tests.py --apply`. The transaction:

- catalog-mapped all 12 selected tests;
- bound 45 assertions to nine exact behavior partitions across five existing
  invariants;
- retained nine reviewed setup/precondition dispositions;
- left no unclassified or non-oracle assertion;
- remained idempotent against the post-map denominator;
- returned the two newly completed harness coverage dimensions to `covered`
  without promoting assertion reachability, mutation sensitivity, or proof
  level.

The metadata-only frozen gap report, generated without a repository scan,
records:

| population | before | after |
| --- | ---: | ---: |
| frozen live tests | 4,393 | 4,393 |
| `catalog_mapped` | 3,732 | 3,744 |
| `reviewed_nonmapping` | 661 | 649 |
| missing specification authority | 154 | 142 |
| owned nonmapping | 507 | 507 |
| confirmed missing tests | 0 | 0 |
| specified gap-open cells | 207 | 207 |

## Deferred final proof

No broad workspace test, `just fmt`, `just clippy`, or `just test-all` was run.
`VERIF-P18-003` remains globally open. The repository-wide planner metadata
baseline, remaining specification and mapping populations, assertion
reachability, mutation sensitivity, and the single final frozen-candidate
broad gate remain open under the Phase 18 cadence.
