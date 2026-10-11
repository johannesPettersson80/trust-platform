# Independent A4 budget and deadline review

Read-only source review of branch `feat/runtime-portability-a4`, base `77b91381fcf1b1850611f88b397bbfe8d45e3523`. Canonical agent files copied from the primary checkout and verified identical. This reviewer authored no production code or tests. No builds, tests, formatters or validators were executed.

The adjacent source manifest records exactly the reviewed snapshot, before resolution of the findings below. Whole-diff coverage belongs to the separate full-correction reviewer.

## Findings

1. **Required correction:** `engine/restart.rs` replaces the active state after retained/image restoration without a final physical-deadline check. The construction check occurs too early; stride sampling cannot guarantee the final work sampled the clock. Check the candidate deadline immediately before replacement and assert preservation of the old state when this final sample expires.
2. **Preparation completeness:** the run-6 fragment formatting list omits the modified `runtime/core/lifecycle.rs` include fragment. Include it. Reconcile the allocation-test unsafe inventory line locations after formatting before source freeze; case/mutation refresh does not update that inventory.
3. **Scalability observation forwarded to full reviewer:** input snapshot destination dedup scans previously saved slots and charges their length per binding. This is independent of unrelated storage but quadratic in the number of bound inputs.

## Verified by source inspection

One context-owned allowance is shared by stack dispatch, register interpretation, tier-1 instructions, portable helpers and initializer bodies. Root/Nested is explicit, nested optimized fallback cannot reset the allowance, and native target execution resumes parked caller frames on failure. No old mutable integer budget plumbing remains in the inspected entry paths. Physical-deadline charges use one shared stride; explicit entry/completion checks preserve short-call coverage. Output-image publication and engineering writes check before the final commit, avoiding an error reported after new outputs become observable. Existing original-instruction budget parity and nested-call tests retain their assertions. Portable helper and dispatcher tests assert shared remaining fuel and clock sample count. Mid-cycle deadline coverage asserts actual progress, latched fault and withheld outputs.

Run-6 orchestration includes core/default/no-default/i686/MCU checks, hosted unit and integration tests including all three new binaries, runtime vertical, Clippy, cross-target warnings, supply chain, architecture, diagrams and explicit fragment formatting. It refuses a second STARTED run, preserves independent failures and marks dependent steps unrun. Preparation and fixture generation precede the frozen-source manifest. Execution and hardware remain unverified by this review.
