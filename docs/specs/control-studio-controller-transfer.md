# Guarded controller transfer

This product contract implements the task-boundary part of the workflow's
Initialization/transfer and Apply sequences. It does not change IEC semantics.
Parameter tuning continues to use `control.parameters.*`; this operation cannot
turn an addressed variable into a writable tuning parameter.

## Sequence

Read transfer status → review exact mode/state/reference changes → queue transfer
→ recheck at the owning task boundary → write the complete set → execute ST
→ inspect first output and state → confirm successful output-image commit.

The controller remains the actuator writer. Studio writes only the declared mode,
reference and initialization/tracking state. It must calculate those values from
the actual controller convention. For `u=Kp*(r-y)`, bumpless entry at nonzero
`Kp` requires `r=y+u/Kp` unless that reference is already present. This reference
alignment is explicit, not an added controller bias. Moving to the task target
is a subsequent reviewed reference change. Unsupported state alignment blocks
transfer with its reason.

## Runtime boundary

`control.transfer.status` accepts `binding` with:

- `loop_id`, `task_name`;
- `mode`: symbolic BOOL `symbol` and `manual_value`;
- `states`: 1–16 distinct numeric members, each declaring `symbol`, `iec_type`
  (`REAL` or `LREAL`), `unit`, `minimum`, `maximum`;
- `guards`: 1–16 similarly typed members whose values must remain unchanged,
  including the verified controller gains and measurements used for alignment;
- `output`: similarly typed member identifying the controller output.

All members belong to the same compiled program with exactly the declared task
owner (or `$background`). Modes/states must be mutable, nonexternal and
nonaliased. State writes may use unaddressed members or uniquely bound `%M`
members. Input/output addresses are observation-only. Constants, in-out aliases,
ambiguous/overlapping addresses and mismatched IEC types are rejected. Existing
parameter restrictions remain unchanged.

Status returns `program_revision`, `binding_revision`, `values` for all selected
members (BOOL mode represented as boolean), and the last transaction outcome.
Program identity includes the runtime session epoch. The returned values are
observations, not requested values or write authority.

The optional `transaction` retains `id`, original `program_revision` and
`binding_revision`, `status`, `reason`, and available `observed_values` with
`confirmation` (`post-st` or `output-image`). The top-level `status` mirrors its
outcome, or is `idle` before any transaction. A fault/stop invalidates pending
authority but retains the terminal outcome under its original identity. Current
status values and revisions remain separate from that historical transaction;
an epoch change must not erase a failed output commit or leave it pending.

`control.transfer.apply` accepts the same binding, `transaction_id`, both expected
revisions, `expected_values`, `values` for the exact state set, `manual` (desired
controller mode), `maximum_output_change`, `source` and `physical_approved`.
Values and output-change tolerance must be finite, within declared ranges and
exactly representable in their IEC types. Acknowledgement is `pending`, never
confirmed adoption. Only one transfer or parameter transaction may be pending.
Compare expected numeric values in their declared IEC numeric representation:
JSON `2` and `2.0` describe the same REAL/LREAL value. BOOL remains strictly
boolean, and missing/extra members or genuinely changed numbers are rejected.

Automatic P transfers may supply `tracking` with `kind:"proportional"`,
`reference_symbol`, `manual_command_symbol`, `process_variable`, `gain_symbol`
and `align_reference`. These must name the corresponding declared state/guard
members. Status advertises this algorithm in `supported_tracking`.
The mode, reference, manual state and fixed gains remain exact guards. PV and
output are refreshed within their declared bounds instead of requiring an
unchanging UI snapshot. At the owning task boundary, use fresh PV and the last
published, successfully committed mapped output image. An uncommitted debugger
write to the output variable is not the held output. Set the manual-command
state to that held output, with its actual IEC rounding. When `align_reference`
is true, set the reference to `PV+heldOutput/Kp`; this requires Auto entry and
a finite nonzero fixed gain. Otherwise use the explicitly reviewed reference.
Reject missing mapped output evidence, invalid roles, or out-of-range calculated
states before writes. Retain boundary `initial_values` and actual `applied_values`
in the transaction. First-output continuity uses that held output. This is
output-image evidence, not physical actuator feedback. Manual explicit-value
transfers without `tracking` retain their exact-value behavior.

Before writes at the owning task boundary, recheck program/binding identity,
every expected value, mode, forcing, member ownership and actual attached-driver
authority. Reject the whole set on any mismatch. Typed mode and state writes
commit together after input acquisition and before the owning ST execution;
ordinary unrelated debug writes cannot substitute for this transaction.

After ST, retain observed mode/state/output. A changed state/mode, invalid output,
out-of-range output, or output jump exceeding the reviewed tolerance yields
`failed` with evidence; it cannot yield `applied`. A successful post-ST check is
still pending until the cycle's output commit succeeds. Output failure yields
`failed`. Successful output commit confirms `applied` as runtime/output-image
evidence only. Physical delivery requires separate measured feedback.

Retries with the same transaction identity cannot repeat a completed transfer.
Retain used transaction identities for the current program epoch, including
cancelled/rejected transactions, rather than just remembering the latest ID.
Support at most32 loop records and1024 accepted transaction IDs per epoch;
reject capacity exhaustion without evicting evidence or accepting a replay.
Loop and transaction IDs are nonempty and at most128 characters. A new runtime
epoch invalidates old requests and may release the old replay-ID budget while
retaining the bounded last outcomes.
Conflicting transactions, stale program reloads, cancellation before commit,
and rejected/failed outcomes retain explicit status. Recovery is a separately
guarded transfer to the declared Manual/tracking state using fresh evidence;
neither zero output nor restoration of obsolete state is an implicit policy.
Closing Studio cannot interrupt an already queued runtime-owned transaction.

`control.transfer.cancel` takes `loop_id`, `transaction_id` and the transaction's
`expected_program_revision`. A matching uncommitted transaction becomes
`cancelled` with no writes. The same cancellation may be retried. Once committed,
return `pending` with `confirmed:false` until its actual terminal outcome is
available; do not pretend to undo executed ST. Cancelling an already terminal
transaction returns that outcome. Wrong identity must not affect another
transaction. Cancellation requires the same Engineer role as transfer apply.

## Acceptance

Execute real ST whose first Auto output depends on both a memory-bound reference
and symbolic mode. Assert no writes before its owning task, coherent reference
and mode adoption, unchanged gain, first-output continuity and subsequent normal
updates. A competing write/force/reload before commit must reject every transfer
write. Test wrong types/ranges/ownership, input/output write attempts, duplicate
bindings, actual physical-driver authority, post-ST overwrite/output failure,
retry/cancel and guarded recovery separately. Verify saved native recording and
the actual Studio commissioning journey; native helper tests alone do not close
the product workflow.

## Client outcome decoding

Before using a returned transfer outcome, validate any provided initial values
as the complete typed mode/state/guard/output selection, and applied values as
exactly the finite numeric state members. Reject malformed capability arrays and
optional value objects; absent or null optional evidence remains unavailable.
Malformed evidence cannot authorize another write or count as confirmation.


## Transfer access roles

`control.transfer.status` is available to Viewer access. Both
`control.transfer.apply` and `control.transfer.cancel` require Engineer access.
Cancellation remains scoped to the retained transaction and program identity;
it cannot authorize a new mode/state change or bypass the existing transfer
review and task-boundary guards.
