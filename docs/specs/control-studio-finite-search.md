# Finite actual-ST parameter search

This is the bounded G07 adapter for Control Studio Spec 35 §17.3. It executes
engineer ST, not a substitute PID formula or lesson-specific model.

`control.search.status {}` inspects the active runtime's loaded source registry
and its project's `simulation.toml`, returning `project_revision`, `ready` and
explicit limitations. It does not scan unrelated ST test folders or include
runtime connection settings. Source basis is `loaded-runtime-sources`; engineers
must reload changed ST before searching it. A snapshot contains at most 64 source
files / 1 MiB source text and 256 KiB simulation configuration.

`control.search.start` accepts `expected_project_revision`,
`execution: "isolated-simulation"`, `grid`, exact `trial_count`, `step_ms`,
`duration_ms`, `seed`, `max_total_cycles`, `wall_budget_ms`,
`per_trial_wall_ms`, `initial_values`, `events`, `observations`, `requirements`
and `objective`. Each grid axis has `symbol`, `unit`, `minimum`, `maximum` and
an explicit `values` array. Observations contain `symbol` and `unit`.
Requirements contain `id`, `symbol`, `minimum`, `maximum`, `start_ms`, `end_ms`.
The objective contains `reference_symbol`, `feedback_symbol`, `error_unit`;
score is the trapezoidal sampled integral of squared error, in error_unit²·s, using actual
step duration. Bounds are checked before ranking; missing/bad evidence never
qualifies. Stable tie order is the original Cartesian trial index.

Grids have 1–4 axes, 1–16 explicit values per axis and at most 64 total trials.
The declared trial count must equal the Cartesian product. Duration is an exact
multiple of step, including samples at0 and at the requested final time.
Step is 1–1000 ms; duration at most 600000 ms; total cycles at most 50000;
at most 16 observations and 250000 sample cells. Initial values and events
cannot target swept parameters; duplicate or ambiguous writes are rejected.
All initial values, plant initial state, events, time points and seed are
identical for every candidate. Numeric BOOL values 0/1 may start project-owned
sequences; gains must be writable unaddressed REAL/LREAL program/FB members.

Each trial executes in a fresh supervised child process, compiling an immutable
source snapshot into a fresh runtime and constructing a fresh existing
SimulationController. Only in-process simulation is attached: no physical
settings, credentials, selected runtime handles or physical drivers are copied.
The existing plant pre/post-cycle functions surround actual ST execution.
Unsupported source/configuration/model fails explicitly. No model substitutions
or silently truncated grid are permitted.

`per_trial_wall_ms` (1–10000ms) bounds compilation plus all scans for each child;
`wall_budget_ms` (1–60000ms) bounds the complete job. Runtime instruction
deadlines also bound each scan. The supervisor kills/reaps a timed-out or
cancelled child. At most one worker runs per service and one globally per
process. Each child output and the total retained job output are bounded at 32 MiB.
An oversized aggregate fails explicitly, replaces that attempted trial with a
failure record and omits its evidence; it never truncates samples or ranks an
incomplete job. A new job may replace only a completed job;
there is no unbounded retained result history.

Start returns a `job_id`. Status and `control.search.cancel` subsequently require
that ID plus `expected_project_revision`; mismatched project/revision/job cannot
inspect or cancel another job. Cancel is a request until the child is reaped.
Job status is queued/running/completed/cancelled/failed. Results include every
attempted trial, its candidate, failure/constraint reason, score, actual samples
and shared immutable conditions. Unattempted trials after cancel/budget exhaustion
remain explicitly unattempted. Ranking includes qualifying trials only.

Returned run records use the existing RecordedRun shape: evidence
`ST-executed`, channels with units, numbered/time-stamped samples and good/bad
quality, complete shared RunConditions, program/model/parameter-schema revisions.
Result provenance explicitly identifies isolated simulation and loaded runtime
source snapshot. This proves no physical-device performance.

Status is Viewer; start/cancel require Engineer. Worker CLI is an internal
bounded JSON entry, `trust-runtime control-search-worker`, with no live runtime
connection. Its packaged executable dispatch needs a real binary gate in
addition to native subprocess tests.

The initial hard memory-limit adapter is Linux-only. Each child applies and
reads back a maximum 1 GiB RLIMIT_AS before reading JSON or compiling ST. The
result includes the actual `worker_memory_limit_bytes` and runtime version.
Unsupported operating systems report readiness unavailable until a native
process-memory-limit adapter exists. No new unsafe code is introduced.

Each scan has an execution deadline of the smaller of 100 ms and the declared
per-trial wall budget. A run has at most 12000 samples; seed must fit a
JavaScript safe integer so returned records satisfy the shared RecordedRun
contract. Requirement windows must have positive width and align to samples.

The editor inspects the selected local simulator before preparing a search. A
review retains its exact request, source revision and endpoint; Start consumes
that review once. Editing, switching loops or closing the editor invalidates a
pending review. Closing a running search requests cancellation on its original
endpoint. Progress polls request summaries without raw runs; complete details
are fetched once after a terminal state. A search result is never applied
implicitly. Full trial results and the reviewed conditions can be saved with
the workflow. Selecting a qualifying trial may populate parameter review only
when every declared tunable/fixed parameter is explicitly present in that trial's
grid or initial conditions. The actual runtime still requires Manual application.

Admission is restricted to a trusted native simulation host. The active
ResourceCommand simulation snapshot must report an enabled simulation and an
explicit simulation-only I/O qualification derived from the actual runtime:
all attached I/O drivers identify themselves as simulated, with no active ADS
or OPC UA client connections. Missing/unknown or physical provenance makes
readiness unavailable and prevents startup, even when simulation.toml exists
or a client requests `isolated-simulation`. The debug control bridge obtains
this snapshot from its real DebugSession simulation handle. Worker processes
clear inherited environment; only the native-test worker selector is added in
test builds. This is not a general operating-system sandbox claim.

For progress polling, `control.search.status` accepts optional
`include_results:false`, returning candidate verdicts/scores without raw runs.
`include_results:true` requests bounded detailed runs only after a terminal
state. Omitting the option preserves the original response contract. Identity
and project/revision checks apply equally to thin and detailed responses.
