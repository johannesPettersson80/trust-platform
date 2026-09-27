# Isolated actual-ST Test runs

This G06 adapter fixes preparation drift: an ordinary Test starts a fresh
configured plant and freshly compiled ST at simulation time zero. It never
tries to hold a non-equilibrium plant with an inferred actuator value while
the engineer reads the review. It shares the finite-search process supervisor,
resource limits, trusted-simulation-host admission and actual scan executor.

Operations are `control.simulation.run.start`, `.status` and `.cancel`.
Status with `{}` returns project readiness/revision, using the loaded runtime
source snapshot and simulation.toml. Start requires:

- expected_project_revision and execution:"isolated-simulation";
- step_ms, duration_ms, seed, max_total_cycles and wall_budget_ms;
- initial_values, events and observations with the same semantics as finite search;
- optional parameter_overrides, defaulting to an empty array.

Each parameter override has symbol, iec_type (REAL/LREAL, or fixed configuration BOOL), unit, convention,
minimum, maximum and value. Values must match the actual runtime representation,
be within bounds, and name writable unaddressed scalar members. Duplicate
symbols and conflicts with initial_values or events are rejected. Parameters
are applied before the first actual scan and their adoption is checked.
They become RecordedRun.parameters; the normalized symbol/unit/bounds declaration (excluding values) determines
parameterSchemaRevision; actual IEC type is checked against the compiled program. Thus Kp16 and Kp20 can share identical conditions and
parameter declarations while differing only in RecordedRun.parameters.

There are no grid, trial_count, objective, requirements or ranking fields in
this Test request. Search retains its strict Cartesian-grid validator; ordinary
runs use a separate purpose adapter. Zero parameter overrides is valid. There
is no dummy gain and no inferred equilibrium or hidden preparation cycle.

Each run uses one supervised child, with at most 10000 ms wall time including
compilation. Shared limits apply: Linux native memory limiter (1 GiB), 12000
samples/run, bounded sources/configuration, at most16 observed channels,
scan-aligned events, finite values and 32 MiB retained output. Other operating
systems remain unavailable until their native resource-limit adapter exists.

Start returns job_id. Subsequent status/cancel requests require job_id and
expected_project_revision. Job identity is also purpose-scoped: search calls
cannot inspect/cancel a Test job or vice versa. Status reports
queued/running/completed/cancelled/failed. include_results:false returns thin
progress; include_results:true retrieves the terminal genuine RecordedRun.
An ordinary run has a run field, not trial rankings or invented scores.
Execution failure/cancellation is explicit and never means validation passed.

RecordedRun uses ST-executed evidence, exact sample times/sequences and channel
quality. initialState represents declared runtime overrides; the immutable model
revision binds the configured physical initial state and plant settings. The
existing independent Validate evaluator applies engineer requirements afterward.
A two-tank run configured at h1=.27,h2=.25 must start at those values and then
show its natural dynamics, regardless of wall time spent reviewing it.

Fixed controller choices such as lesson61 P1.UseCascade may use a BOOL
parameter override with value0/1, unit1 and bounds within0..1. This applies only
to isolated ordinary Test configuration: search and live G08 gain mutation
remain REAL/LREAL. BOOL overrides cannot appear in initial_values/events and
must retain their reviewed value throughout execution; operational state that
changes during a run is not a fixed configuration parameter.

Integer process-state initial values and events use existing native coercion
with exact integral/representable input and IEC range checks, including
addressed INT scenario selectors such as lesson61 P1.TrialKind. This does not
expand integer gain mutation. Numeric/BOOL observations use the shared read
adapter, including integer signals without casting wrappers.

## Execution faults and captured prefixes

If ST or simulation execution fails after coherent samples have been captured,
terminal `failed` detailed results retain those rows in the existing optional
`run` field with the actual fault reason. Thin status remains free of raw data.
Do not append a sample for a cycle whose execution or observation did not finish.
Keep the frozen requested duration, initial conditions, event plan and planned
sample-time grid; absent later observations remain missing, not synthesized.
The prefix is ST-executed evidence, never completed verification or qualification.
A failure before the first captured sample still has no `run`.

A killed process can supply only frames actually received through the bounded
checkpoint path below. If no complete sample arrived, report the absence of
returned data explicitly.

## Bounded capture before supervisor termination

For ordinary Run-purpose workers, each completed coherent sample is transferred
through the private worker pipe before the next cycle starts. The stream carries
one immutable RecordedRun header and ordered sample frames; the complete final
result remains distinct. These are private execution messages, not PLC/ST output
or a public control protocol. Nonprotocol diagnostic lines remain bounded by
the total output cap; reserved frame types are validated strictly. Search-purpose workers keep their existing result
contract.

Thin Run status includes `captured_sample_count`: the number of complete sample
frames the supervisor has received, initially zero. It is progress evidence,
not completed verification. Cancellation, wall-budget expiry or abnormal child
exit retains the received coherent prefix in the terminal optional `run` field.
Its requested conditions/time grid remain frozen. A partial last frame is ignored;
no row is synthesized for it. A failure before the first complete sample has no
`run`. Cancellation remains `cancelled`; timeout/abnormal exit remains `failed`.

The private stream and final result together remain within the existing 32 MiB
output bound. Cap checkpoint header plus sample frames at 15 MiB and 12000 samples;
reserve the remaining output budget for the final result and metadata. Exceeding
the checkpoint budget ends execution with an explicit failure and retains the
already transferred prefix. Invalid or inconsistent frames cannot become a
qualified result. Memory retention stays bounded and terminal-only detailed
result retrieval remains unchanged.
