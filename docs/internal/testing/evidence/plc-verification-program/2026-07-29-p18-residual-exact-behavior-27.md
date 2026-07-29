# Phase 18 residual exact-behavior campaign

Date: 2026-07-29

Status: accepted focused progress across `VERIF-P18-003` through
`VERIF-P18-006`; no global row closes from this campaign.

## Frozen source and execution boundary

- Local canonical checkout:
  `/home/johannes/projects/trust-platform`
- Branch: `plc-verification-program`
- Clean detached builder checkout:
  `trust-builder:/home/johannes/projects/trust-platform-p18-residual-source`
- Builder host: `scena-rust-builder`
- Behavior implementation commit:
  `68629aacfaa89a34de946e9f952adfa9fb3f6459`
- Accepted consolidated source commit:
  `4c35370499ff2423a22de327a67be444f5c5fdf5`
- The destination `AGENTS.md` and complete `.codex/skills/**` tree were copied
  from and verified byte-identical to the canonical checkout before editing or
  execution.

The accepted focused campaign is bound to
`4c35370499ff2423a22de327a67be444f5c5fdf5`. It ran 26 exact tests in 15
command groups, observed 26 passes, and retained zero failures or zero-test
filters. Those tests close all 27 previously confirmed behavior demands through
31 exact behavior-to-test links. Seventeen tests were already cataloged; nine
live scanner facts were promoted in the atomic transaction.

## Test-first defect repair

IEC 61131-3 Table 46 requires the explicit `TP_TIME`, `TON_TIME`, and
`TOF_TIME` timer names. The focused HIR and runtime tests both failed first by
assertion on the clean `0.24.59` source because all three names produced
`UndefinedType` diagnostics and their instances were consequently non-callable:

```text
cargo test -p trust-hir --test semantic_type_checking \
  phase18_behavior_closure::explicit_time_timer_function_blocks_are_accepted \
  -- --exact

cargo test -p trust-runtime --test fb_timers_full \
  explicit_time_timer_variants_execute_with_time_values -- --exact
```

The implementation registers the three explicit aliases in HIR and runtime,
uses the fixed `TIME` parameter family, and updates the IEC standard-function
block specification and coverage record. The same HIR test passed after the
repair. Runtime proof was then consolidated into the existing cataloged
`timer_variants` test, which passed with all six base and explicit-time timer
instances. The change is IEC-conforming, so it requires neither an IEC decision
nor a deviation.

The same source batch also added or strengthened focused proof for:

- HIR assignment targets, configuration task/program binding, diagnostic
  cascade suppression, integer `FOR` controls, and explicit-time timers;
- trust-dev CI exit-code precedence and non-assertion runtime-error
  classification;
- absent deferred PLCopen Motion public surfaces;
- bytecode metadata version-gate atomicity;
- narrow-string lower/upper index bounds and replacement precedence;
- existing exact time, LSP lifecycle/path identity, debug authorization and
  hook/buffer, VM materialization, and verification-validator behavior.

Every selected focused test passed on the clean builder checkout. No broad
workspace test, `fmt`, `clippy`, or `test-all` gate ran.

## Atomic mapping result

The fail-closed transaction was compiled in dry-run mode before it modified the
catalog. Atomic apply then updated the catalog, denominator, accepted
proposal/decision provenance, sixteen invariant records, and
`verification/p18/review-locks/residual-27-semantic-lock.json`.

The resulting frozen equations are:

```text
live test facts                       = 4,409
catalog_mapped                        = 3,942
reviewed_nonmapping                   =   467
catalog_mapped + reviewed_nonmapping  = 4,409
missing specification authority       =     0
confirmed missing behavior demands    =     0
missing behavior candidates           =     0
specified gap-open coverage cells     =   208
```

The 467 owned nonmapping facts and 208 gap-open coverage cells remain explicit
debt. Nine current behavior-bearing invariants still need exact per-behavior
catalog markers; eleven historical placeholder invariants contain no behavior
rows or tests and require reviewed retirement. The repository-wide staleness
validator also remains red on the pre-existing Phase 18 metadata/proof
baseline. Consequently, this campaign is not evidence for global
`P18-003`, `P18-004`, `P18-005`, or `P18-006` closure.

## Retained machine-readable evidence

The directory
`docs/internal/testing/evidence/plc-verification-program/2026-07-29/p18-residual-exact-behavior-27/`
contains:

- the pre- and post-transaction frozen gap reports;
- the 27-demand assignment plan and exact campaign plan;
- the 26-test campaign result and all 15 raw command artifacts;
- 26 fail-closed assertion inventories with 282 extracted assertions and zero
  unresolved helpers;
- the live 4,409-test scanner artifact; and
- the semantic review lock.

The local verification-tree aggregate SHA-256 matched the clean builder
verification-tree aggregate after transfer:
`a392395452cea5138b386a137634e479965b536abacf751dde57b1dddce66a2d`.
