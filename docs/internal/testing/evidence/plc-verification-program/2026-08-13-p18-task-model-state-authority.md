# P18 portable task-model ownership and state authority

Date: 2026-08-13
Active rows: `VERIF-P18-003`, `VERIF-P18-005`

## Scope

This behavior-preserving slice separates portable task configuration,
scheduling state, and readiness evaluation into focused owners while
preserving the public `trust_runtime_core::task` API and all cataloged test
identities. It promotes only `TaskState` and `TaskState::new` to the existing
state-initialization authority.

`TaskConfig`, `TaskReadiness`, and `evaluate_task_readiness` remain explicit
`spec_gap` facts. The slice therefore does not claim complete task-model
authority, assertion reachability, mutation sensitivity, broad workspace
validation, or Phase 18 completion.

## Behavior lock and ownership split

Before the refactor, the exact `task::tests::` nextest selector passed 7/7.
Commit `87dbdc51` moves `TaskConfig` to `task/config.rs`, `TaskState` and its
constructor to `task/state.rs`, and `TaskReadiness` plus
`evaluate_task_readiness` to `task/readiness.rs`. `task.rs` keeps compatibility
reexports and the existing tests, so callers and catalog identities do not
change. The same selector passed 7/7 afterward and again after the authority
transaction.

`cargo check -p trust-runtime-core --no-default-features`, targeted rustfmt,
diagram rendering, and diagram-drift validation passed on the isolated
builder. The runtime execution PlantUML source, SVG, and manifest now show the
three task-model responsibilities separately.

## Identity refresh

The clean report at `87dbdc51` exposed exactly five moved identities and five
stale pre-split identities. The guarded refresh proposal is
`sha256:ab348b467801221fc535e7711ab6d1a540ec2bcb7ff1d050c63e41cc65926026`
with operation-set digest
`sha256:100352ec4cc27480af667c56204809d840a23862e297e7fe7905094ccb5867b5`.
The reviewed selection is
`sha256:467e14426dd6926bac174e563313844cce9a5d82f821c355c034214c8f352955`.
It removes all five stale facts and registers all five successors against the
same historical runtime-safety gap. Check and apply modes passed; the atomic
application is
`sha256:51be6342ac69c1f3990160f028337af6b63e507c9d2ca6987e5fb19d9b506949`.
The 34 focused refresh/selection/contract tests passed, and commit `941b79cd`
is the recoverable identity checkpoint.

## Semantic review

The clean 17,489-fact report at `941b79cd` had zero findings. Its complete
authority-reuse proposal is
`sha256:394c7978e944996842c5718563b659db8c48c20d66094b520b57fe7b67c7913d`
with operation-set digest
`sha256:0dbf28f8728a1d5137305d83f0e5815db1a25efc36bfd7959f1541c5f3b1fc3b`.
The 23 unchanged ambiguity, mixed-semantics, and claim-mismatch decisions were
carried forward exactly. Only the singular two-fact `task/state.rs` group was
accepted for `RT_CORE_TASK_STATE_INITIALIZATION_001`.

The complete selection is
`sha256:8755868f0e8cfa874aa698628adf1faddb9fbe0c8034803eb683c4b176351290`.
Check mode and the atomic live reconstruction passed with 24 groups reviewed,
one accepted, 23 withheld, and two mappings promoted. The application is
`sha256:2a674d0f4c01abf7b2fccf5918d46c50b0a6f8652b6933ce9a0eb5174f23d612`;
commit `3d5d165c56aa3f66efa4d343b65b9fdb7a93f015` records the clean authority
checkpoint.

The post-state report is
`sha256:c57a56893cbd4dbb3e67eb41a449f3c4e514741af952b4e30cbbe11f6424ba5c`:
17,489 facts, zero findings, 13,836 `spec_gap`, and 3,393 `specified`.
The state invariant and evidence now use the governed `trust-runtime` product
owner; `trust-runtime-core` remains the component and test package.

## Readiness authority boundary

A fresh six-test LLVM trace at exact commit `3d5d165c` observed 6/6 tests and
reached the moved evaluator `CODE_5114EF4A...`. The trace is
`sha256:0ef8f351470ea86bb8e381a18bec0d47c801fa9fc46bdb5727a272a2ea58c931`.
It does not execute the `TaskReadiness` type declaration, so it cannot justify
that structural fact.

The generated authority card is
`sha256:c68e8087c959a1ad8b85d2bad6f10ac6595127c142fe99276bfe1f4105863739`.
It mechanically identifies the evaluator, the five covering readiness tests,
and `RT_TASK_READINESS_001`. That invariant is `safety_critical`; the generated
route requires two authority reviewers independent of the proposer. No such
review was available in this slice. The evaluator was not promoted, no
reviewer identity was fabricated, and the invariant was not downgraded.

`TaskConfig` also remains explicit debt because the only nearby authority is a
cross-area omnibus scheduler invariant rather than an exact configuration
contract. These three gaps are the honest next authority boundary.

## Focused execution and proof boundary

On `scena-rust-builder`:

- the exact task behavior-lock selector passed 7/7 before and after the split,
  and passed 7/7 after the state authority transaction;
- the authority-reuse, candidate, and code-spec contract modules passed 23/23;
- the identity refresh/selection/contract modules passed 34/34;
- the fresh focused LLVM trace observed 6/6 tests with exit code zero;
- the no-default core check, diagram rendering, and diagram drift passed.

The canonical planner remains globally blocked by pre-existing Phase 18
metadata debt. No `just fmt`, `just clippy`, `just test-all`, strict preflight,
GitHub candidate matrix, or `VERIF-P18-016` broad gate ran.

## SOLID, KISS, and DRY review

- SOLID: configuration data, mutable scheduling state, and readiness policy
  have separate source owners.
- KISS: parent-module reexports preserve the established API and test paths
  without adapters.
- DRY: identity refresh and authority promotion use the canonical proposal,
  complete-selection, and atomic live-reconstruction tools.

## Bootstrap

Canonical source and local checkout:
`/home/johannes/projects/trust-platform`, branch
`plc-verification-program`. Isolated destination:
`trust-builder:/home/johannes/projects/trust-platform-p18-portable-d53471c2`.
Both checkouts matched the active commit before evidence production.
`AGENTS.md` matched
`sha256:76576b5464443f4128a67c325527d23e59da46c019da7b79f39454dabc250263`;
the complete `.codex/skills/**` aggregate matched
`sha256:3f84c064eff30aca31d55c51034a05a7e2ae4a53d2c69fb6f1827585b7ac73a7`.

`VERIF-P18-003` and `VERIF-P18-005` remain globally open.
