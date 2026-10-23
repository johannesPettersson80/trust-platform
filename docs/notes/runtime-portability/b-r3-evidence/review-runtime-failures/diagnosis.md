# B-R3 M2 runtime regression diagnosis — source frozen

Read-only source tracing and SSH reads of the existing runtime-integration log. No builds, tests, validator commands or source edits. The complete batch was still running during diagnosis; corrections remain proposals until it finishes.

## Excess deadline sample

Observed log: physical_deadline_faults_mid_cycle_and_withholds_outputs reports 33 polls versus the unchanged expected 32. Its clock returns expired from poll 32 onward. The test reached its mid-cycle progress assertion and the fault identity matched; failure is the extra sample.

Owning path: dispatcher execute_pou_stack_with_parameter_presence always retires remaining frames after execute_with_buffers returns an error. Engine retire_frame now charges frame/activation and owned-instance cleanup work. Those charges can cross another shared deadline stride and invoke the physical clock after execution already observed expiry. The original error is kept and cleanup is necessary; suppressing cleanup or weakening the poll assertion would hide the defect.

There are three sampling routes to unify: ExecutionContext::check_execution_deadline (default trait method via EngineState::deadline_exceeded), policy::charge_work_units (direct services call after budget stride), and policy::check_entry_deadline (direct services call). A latch only in one route is insufficient.

Recommended root correction: keep an operation-scoped sticky expiration in the shared ExecutionBudget, reset by its existing operation reset. Route all three sampling paths through one check that returns the existing expiry fault without another callback after first expiration. Preserve normal fuel accounting and infallible terminal identity cleanup. Add a native boundary regression covering first expiry, repeated helper/entry/completion checks without clock resampling, cleanup preservation and reset for a new operation. Keep the existing 32-poll and withheld-output assertions unchanged.

## Two thousand input destinations exceed work budget

Observed log: two_thousand_distinct_input_bindings_fit_default_work_limit fails its execute_cycle expectation with ExecutionTimeout; the neighboring 3,000-declaration/2,000-store regression passes. The input test stages 2,000 distinct DINT bindings before snapshotting destinations.

Owning path: sample_input_image stages writes, then snapshot_destinations inserts each physical root into SavedDestinations. Its insertion_demand unconditionally sets shifts to the complete current length, even for an ascending append. For 2,000 unique keys that alone charges 0+1+...+1,999 = 1,999,000 units; reallocation, lookup, policy and write charges add more. The default cycle budget is 1,000,000. This is an artificial quadratic charge in the ascending case, independent of whether actual insertion moves any entries.

Immediate correction: compute the binary-search insertion index and charge exactly len-index shifted entries, plus len relocated entries only on actual capacity growth. Keep full replacement allocation accounting and enough comparison work for all searches. Existing descending-order demand tests should remain exact; add ascending append and mixed-order assertions so a len-only regression is caught.

That correction alone does not remove actual quadratic movement for reverse or arbitrary input binding order. For the input transaction, use a bulk journal construction path: resolve/validate staged physical destinations in original binding order, collect compact keys with source ordinals, charged-sort once, deduplicate physical aliases, then append snapshots in final key order. Keep writes in their original order so last alias assignment and partial-field semantics stay unchanged; retain first snapshots and sorted rollback. Do not sort program writes or rely on producer order. Existing paths resolving references through current local/instance roots and an active native-output journal need the same identity handling. Error precedence must be reviewed: moving later reference faults ahead of earlier clone/allocation failures is not automatically equivalent. A narrow prepared/input-batch path may be preferable to changing every incremental native-output path.

Add a native arbitrary-order input-binding corpus and alias/partial rollback cases while retaining the existing large ascending fixture and default work limit. Do not increase fuel, waive the test or return to whole-storage cloning. Logical work must describe actual bounded operations, not conceal real insertion shifts.

No correction has been implemented or validated by this review. The two retained failures are independent of flash fit and do not invalidate the measured size arithmetic; they prevent runtime verification/physical acceptance until fixed and separately authorized validation succeeds.
