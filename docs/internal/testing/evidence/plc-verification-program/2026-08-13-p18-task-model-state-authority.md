# P18 portable task-model ownership and authority

Date: 2026-08-13
Active rows: `VERIF-P18-003`, `VERIF-P18-005`

## Scope

This behavior-preserving slice separates portable task configuration,
scheduling state, and readiness evaluation into focused owners while
preserving the public `trust_runtime_core::task` API and all cataloged test
identities. It promotes `TaskState` and `TaskState::new` to the existing
state-initialization authority, then restores the already accepted readiness
authority to the moved `TaskReadiness` and `evaluate_task_readiness` facts.

`TaskConfig` remains an explicit `spec_gap` fact. The slice therefore does not
claim complete task-model authority, new readiness product proof, mutation
sensitivity, broad workspace validation, or Phase 18 completion.

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

The state-only checkpoint report is
`sha256:c57a56893cbd4dbb3e67eb41a449f3c4e514741af952b4e30cbbe11f6424ba5c`:
17,489 facts, zero findings, 13,836 `spec_gap`, and 3,393 `specified`.
The state invariant and evidence now use the governed `trust-runtime` product
owner; `trust-runtime-core` remains the component and test package.

## Readiness authority reuse

A fresh six-test LLVM trace at exact commit `3d5d165c` observed 6/6 tests and
reached the moved evaluator `CODE_5114EF4A...`. The trace is
`sha256:0ef8f351470ea86bb8e381a18bec0d47c801fa9fc46bdb5727a272a2ea58c931`.
It does not execute the `TaskReadiness` type declaration, so it cannot justify
that structural fact.

The initial generated execution-authority card is
`sha256:c68e8087c959a1ad8b85d2bad6f10ac6595127c142fe99276bfe1f4105863739`.
It mechanically identifies the evaluator, the five covering readiness tests,
and `RT_TASK_READINESS_001`, but that candidate route is intentionally for new
authority and requires two independent reviewers for a `safety_critical`
invariant. It was not used to recreate already accepted authority.

The repository's existing-authority generator instead had a mechanical gap:
it accepted `mapped` invariants but excluded the same authority after it had
advanced to `implemented`. An expected focused red proved that an implemented
invariant produced zero candidates. Commit `fb2ad116` makes only `mapped` and
`implemented` eligible while preserving the exact oracle, area, catalog
backlink, complete semantic review, and atomic live-reconstruction guards. The
paired green passed, the complete authority-tool suite passed 41/41, and the
verification-flow diagram plus manifest were rendered with zero drift.

That validator change produced one expected reviewed-body/risk-ack digest
finding. The exact one-rebind refresh proposal is
`sha256:12e0bc871a408421daaf0c3ad319b998ce463905342fef8a456f61de2024466a`;
its complete selection is
`sha256:d03a293ce4ba45ceb82b9bfee36bc5713a96119ce25b8108dcff12311a2c7447`,
and its atomic application is
`sha256:37d2235df495eeae6e4a4c89b175bbf3509c00d7f1dde0bc4a312e41862fbf93`.
Commit `c566a6fe` is the clean digest-refresh checkpoint.

The clean expanded authority proposal is
`sha256:f8284069e715444b740da53a021b4c810564437ec4dc83fbfd62ea75efd65d8a`
with operation-set digest
`sha256:f7143ead6771cf5deaecba2e68e0dda5ebdad9601499ad7c4dc7ee6d1c0c73f8`.
Its complete 25-group review carries forward 22 prior decisions, withholds the
expanded watchdog group as ambiguous and the three-type retain group as mixed
semantics, and accepts only the two-fact readiness group for
`RT_TASK_READINESS_001`. The selection is
`sha256:db3fafa94ec03ca01f1b4b9dff1477b8180839943dd0c14e07fa7a94fc843760`;
the atomic application is
`sha256:defdd9738d2213543e42d2f6b3c534bf7cfca0b17192d45c908de49319fcfeab`.
Check and apply modes both report one accepted group, 24 withheld groups, and
two promoted mappings. Commit `947a9d6a` records the authority restoration.

The clean post-authority report is
`sha256:ebbe5087f2e0155f1b3a43c3f91f0f8bf395599fb1087ceb0852b127a8adcce1`:
17,489 facts, zero findings, 13,834 `spec_gap`, and 3,395 `specified`.
No new readiness behavior or proof tier is claimed; the exact pre-split body,
thirteen accepted tests, producer-authentic lock, assertion reachability, and
risk acknowledgement already existed and remain bound to the same invariant.

`TaskConfig` remains explicit debt because the only nearby authority is a
cross-area omnibus scheduler invariant rather than an exact configuration
contract. It is the next task-model authority boundary.

## Focused execution and proof boundary

On `scena-rust-builder`:

- the exact task behavior-lock selector passed 7/7 before and after the split,
  and passed 7/7 after the state authority transaction;
- the final authority-reuse, selection, and code-spec contract modules passed
  41/41;
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
