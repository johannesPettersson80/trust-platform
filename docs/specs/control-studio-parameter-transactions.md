# Generic Control Studio parameter transactions

This companion specifies Spec 35 §17.4. These are truST product operations, not
IEC language semantics. The existing `control.pid.*` protocol remains compatible.

`control.parameters.status` accepts `{binding}`. A binding is
`{loop_id, task_name, parameters, mode}`. Each parameter contains `symbol`
(program-instance path), `iec_type` (`REAL` or `LREAL`), nonempty `unit` and
`convention`, and finite `minimum`/`maximum`. `mode` contains a BOOL `symbol`
and BOOL `manual_value`. Only program/FB instance scalar members are supported
in this first adapter; aliases, addressed parameters and shared global ownership
need separate adapters. All paths must belong to programs owned exclusively by
the declared task. `task_name: "$background"` selects unscheduled programs.
Sets are nonempty, have at most 64 members, and cannot duplicate a symbol or
include the mode symbol. Unknown fields and unsupported types are rejected.

Status returns runtime-derived `program_revision`, hash-derived
`binding_revision`, `active_revision`, actual `values`, `status` (`idle`,
`pending`, `applied`, `rejected`), optional `reason`, and optional
`previous_revision`. Program identity includes the compiled program/type/task
metadata and the debug session. A reload or restart invalidates old identities.
Binding identity hashes the complete declaration, including units and limits.
Status never claims that a queued change is applied.

`control.parameters.apply` accepts binding, `expected_program_revision`,
`expected_binding_revision`, `expected_active_revision`, `expected_values`,
`values`, `source` (`simulation` or `physical`), and `physical_approved`.
Both value objects must contain exactly the declared symbols. Values must be
finite, within bounds and representable as the declared IEC type. Physical
requests require immediate explicit approval. Custom Auto adoption is not
supported by this adapter: Manual must be observed both when queueing and at
the owning task's next execution boundary.

At that boundary the runtime rechecks compiled program identity, binding,
expected actual values, ownership, declared types, bounds, Manual mode, and
absence of variable forcing. It stages every replacement in cloned storage and
commits the entire storage once, before that task executes. A failure commits
nothing and records a rejection. Confirmation observes all parameter values
after that task executes; controller rewriting any value is a rejection, not
successful application. A request for another task remains pending until that
task runs. Competing pending sets are rejected to avoid hidden overlap.

`control.parameters.rollback` accepts the same guard/review fields as apply,
plus `previous_revision`, without `values`. It restores the exact retained
previous values through the same boundary and confirmation path. External
parameter changes invalidate rollback. Failed/terminated execution interrupts
pending transactions. Results and history are scoped to the active debug
session. This API alone does not establish physical-device qualification.

Closing a Control Studio panel while a reviewed Manual apply or rollback is
pending does not cancel, repeat, or replace that native transaction. On reopen,
the extension inspects the same scoped runtime status and reports Pending,
Applied, or Rejected with the observed values. It MUST NOT reuse the closed
panel's review token or issue a second apply or rollback. A changed program,
binding, debug session, or missing terminal status is unresolved and blocks
dependent work until the engineer makes a fresh review.

Actual I/O provenance is checked again at commit. `IoDriver::is_simulated()`
defaults to false; the in-process simulated and loopback drivers explicitly
return true. Any other attached driver requires physical approval even if the
client labels its request `simulation`. This prevents a client source label
from bypassing the physical review. A mixed simulated/physical runtime follows
the physical rule. There is no device-qualification claim from these tests.

Status is an inspection operation (Viewer); apply and rollback require Engineer.
Known limitation: REAL candidates must be explicitly rounded to their exact
single-precision runtime value before submission. Integer/enum gains, addressed
parameter storage and custom Auto adoption remain unsupported and are rejected.
The existing PID adapter remains independently supported.

The version-2 loop file may declare `parameterBinding` using the exact runtime
binding above; its `loop_id` must equal the enclosing loop id. Version 1 may
not silently accept this field. The editor rejects malformed declarations,
unknown fields, duplicate symbols, a parameter sharing the Manual signal,
nonfinite or reversed bounds, and unsupported types before any runtime call.
Optional `analysisParameterSymbols` maps every declared analysis parameter to
a unique parameter symbol. Each mapped parameter has matching units and bounds
that contain the analysis range; missing or ambiguous mappings disable adoption.
Saving a supplied-model analysis classifies its model evidence as Model-predicted.

The application owner inspects the bound runtime and retains an exact reviewed
set of current/candidate values, IEC types, units, conventions, mode and runtime
revisions. A candidate gain maps to an ST parameter only through declared
`analysisParameterSymbols`. Changing input values, target, mappings or review
invalidates the old authority. Apply binds the retained review to its endpoint
and waits for post-task confirmation. Rollback uses its own exact-value review.
Physical application requires the existing immediate explicit authorization.
Numerical or validation labels cannot create application authority. The current
Tune/Validate specs own visible controls; no raw-symbol or JSON parameter panel
is prescribed here.

The root-locus application journey must also execute the selected ST gain in the
simulated shaft, record the bounded step, retain the final restored Manual state,
and assess the saved angle/torque trace against the stated bounds. Manual
recording has its own acquisition budget: the configured experiment duration
must not silently end acquisition before post-experiment restoration is captured.

## Operation identity and access roles

Status is a Viewer operation. Apply, rollback and cancel require Engineer access.
Current status advertises `supports_request_id: true` and the last
`transaction_id`. Studio assigns that identity before dispatch and persists it
with the reviewed program/binding revisions. A lost acknowledgement is resolved
by observing the same operation; Studio never repeats Apply as a retry.

An unconfirmed Manual operation retains its owner and exposes Retry recovery in
the current panel. A failed recovery review must keep that route available.
Dependent commands and editable task inputs remain locked until reconciliation;
an explicitly confirmed replacement-runtime review unlocks the next study
without claiming that the old operation completed or replaying its writes.

`control.parameters.cancel` accepts `loop_id`, `transaction_id`,
`expected_program_revision` and `expected_binding_revision`. Cancellation before
dispatch fences that identity so a delayed Apply cannot execute. Cancellation of
an uncommitted queued operation rejects it without changing parameters. A committed
operation is reconciled through its post-ST status and requires a separately
reviewed rollback when restoration is needed. Old epochs, mismatched bindings and
unrelated pending owners are rejected.
