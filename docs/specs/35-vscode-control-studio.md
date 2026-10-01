# VS Code Control Studio

This document owns Control Studio's runtime, acquisition, model, numerical and
application contracts. The page specifications are the only layout authorities:

- [Identify](control-studio-identify-page.md)
- [Tune](control-studio-tune-page.md)
- [Validate](control-studio-validate-page.md)

The [shared three-page flow](control-studio-complete-workflow-specification.md#three-page-automatic-flow)
owns automatic continuation, compact editors, autosave and restart behavior.
The [purpose contract](control-platform-purpose.md) owns platform acceptance.
The [complete workflow specification](control-studio-complete-workflow-specification.md)
and its capability contracts own controller selection and engineering methods.
Technical result fields in this document do not imply additional visible panels.

PID execution is defined by [Specification 32](32-control-pid.md), plant dynamics
by [Specification 33](33-simulation-plant-models.md), and plant configuration by
[Specification 34](34-vscode-plant-simulation-ui.md).

The words MUST, MUST NOT, SHOULD, and MAY are normative.

## 1. Supported control loop

Every Control Studio binding declares whether its primary view is
`closed-loop` or `open-loop`. A closed-loop view keeps SP, PV, and MV
immediately visible. An open-loop view keeps the applied process input and
measured process output immediately visible and MUST NOT invent a setpoint.
The open-loop plot, signal list, values and accessible descriptions use the
same applied-input/measured-output meaning. Omit the setpoint slot and series.
If no controller mode is declared for that experiment, omit its mode display
rather than presenting a normal open-loop state as unavailable.
A lesson may add named auxiliary channels
to that same binding for extra setpoints, process variables, manipulated
variables, achieved actuator positions, disturbances, states, quality flags,
or scheduling variables. This is the only multi-signal/MIMO extension point;
it does not create another loop transport or workspace.

A loop definition MUST identify:

- the project and runtime;
- the controller or experiment instance and its exact project type;
- whether design is owned by the shared `Control.PID` designer or by the
  runnable project;
- for a closed loop, setpoint (SP), process variable (PV), and manipulated
  variable (MV);
- for an open-loop experiment, process variable (PV) and manipulated variable
  (MV), with no setpoint binding;
- the engineering unit and expected range of each signal;
- the controller action: `direct` when the output increases while the
  measurement is below the setpoint, `reverse` when it increases while the
  measurement is above it. Setup names the direction, not these words, because
  instrument practice calls the first reverse acting;
- controller mode;
- output and rate limits when configured;
- task period and observed sample time; and
- the simulated plant when the loop uses one.

Two loops MAY declare a cascade (29 September 2026): the outer loop states
`cascade: { role: "outer", partnerLoopId }` and the inner loop the same with
`role: "inner"`. The outer loop's manipulated variable is the outer controller's
output, in the inner measurement's unit and range. The inner loop's setpoint is
the outer controller's manual output, which holds a written value while the
outer controller is in Manual; the inner declaration's `referenceHold` names the
outer mode signal, its Manual value and that manual output. Each loop is
identified, tuned, validated and applied through the ordinary workflow, inner
loop first:

- the inner loop is validated with the outer controller held in Manual at the
  target, through `referenceHold`, and is applied only while the outer
  controller is in Manual: Apply refuses, before waiting or writing, with
  "Put the outer controller in Manual before applying", and its transfer guards
  the outer mode, so the runtime also refuses a switch to Auto before the task
  boundary;
- the outer loop is identified with the inner loop applied and in Auto, so its
  model includes the closed inner loop. It is validated on that model only,
  never on the project's configured plant: that run would start the program
  from its declared values, with the inner controller in Manual and away from
  the operating point the outer model was identified around.

When a controller mode signal is bound, the loop definition MAY state whether
a true value means `Auto` or `Manual`. A legacy binding that omits the polarity
uses true for `Auto`. Control Studio MUST use the declared polarity and MUST NOT
infer it from a variable name.

A lesson loop MAY define Test defaults: execution owner, experiment kind,
amplitude, maximum duration, and deterministic seed. `control-studio`
execution means Control Studio writes the reviewed excitation through the
existing runtime experiment service. `project-scripted` execution means the
runnable lesson or simulator already owns the scenario; Control Studio MUST
treat it as observation with zero external amplitude, record the
declared duration, and perform no duplicate process excitation. A
project-scripted scenario MAY declare one writeable Boolean activation binding.
When present, the authorized start MUST use the same runtime experiment service
to write that binding true, while Abort, timeout, failure, and normal completion
MUST restore it false and confirm the restored scan. The plan MUST retain this
as project activation authority, not as SP, MV, or a second excitation. A
project-scripted scenario without an activation binding remains observation
only. Retain these defaults in the study; use the page-defined controls.

Every auxiliary signal MUST have a nonempty stable `id` distinct from the
reserved primary identities `setpoint`, `processVariable`,
`manipulatedVariable`, and `actuator`, a `channel` identity,
one role (`SP`, `PV`, `MV`, `actuator`, `disturbance`, `state`, `quality`, or
`schedule`), and the same typed binding fields as a primary signal. A `quality`
binding may instead be `BOOL`, where false means bad channel quality. A channel
may contain at most one signal of each role. SP and PV in the same channel MUST
use the same engineering unit. SP, PV, actuator, state, quality, and schedule
signals are read bindings; MV signals are write bindings. A disturbance may be
read-only when measured or writeable when it is an explicitly actuated
simulation/test input.

When a loop supports saturation-and-windup analysis, its controller declaration
MUST name `saturationEvidence.requestedOutputId` and
`saturationEvidence.integralStateId`. They are the stable IDs of two distinct
read-only `state` auxiliary signals: the unlimited controller request and the
retained integral state. The request uses the manipulated-variable engineering
unit. Control Studio records those exact bindings with the declared applied
output; it never guesses them from variable names, reconstructs an unlimited
request from a clamped output, or treats a controller parameter as live state.
Without the two bindings, the Test result remains usable for its supported
operations but saturation-and-windup analysis reports the missing adapter.

All primary and auxiliary process-image bindings MUST occupy distinct bit
ranges. Exact duplicates and partial byte overlaps are ambiguous and MUST be
rejected before monitoring or experimentation. The binding also rejects a
missing or nonpositive controller/sample period; an unknown sample time is not
silently replaced by a default.

The optional `topology` field is `closed-loop` or `open-loop`; omission means
`closed-loop` for existing bindings. `closed-loop` requires a setpoint whose
unit matches the process value. `open-loop` rejects a setpoint, requires a
project-owned experiment/controller, and exposes no error or setpoint metric.
Control Studio MUST reject incompatible, missing, ambiguous, or cross-runtime
bindings. It MUST NOT guess or synthesize a signal from its name.

The controller `design` field is either `pid` or `project`. `pid` requires
controller type `Control.PID` and enables the shared PID identification,
tuning, validation, application, and rollback services. `project` identifies
an ON/OFF, manual, model-analysis, state-space, predictive, adaptive, robust,
or other controller whose governing implementation remains in the runnable
lesson project. A project-owned controller uses the same acquisition, recording,
analysis and evidence transport, but Control Studio MUST NOT present a
generic PID candidate as that controller or write `Control.PID` parameters to
it. A declared Tune / Design stage for a project-owned controller requires its
own typed design adapter; until that adapter is present the action remains
disabled with the missing-adapter reason.

A `design: "pid"` loop MUST declare a typed PID adapter before Control Studio
offers a connected P, PI, PD, or PID choice. The adapter is an explicit
controller-law record, never a variable-name convention. It identifies the
parallel `Control.PID` parameter record owner, the task-boundary apply/observe
operation, and unique compiled bindings for every field that a selected
structure can change. At a minimum the adapter maps proportional, integral and
derivative gains; it maps weights, bias, output/rate limits, tracking time and
anti-windup whenever the selected controller or tuning method changes them.
Each binding records its IEC type, engineering unit, range and parameter-record
revision.

Loops without a compatible typed adapter retain supported acquisition and
analysis operations. A missing adapter blocks only its dependent operations,
with the reason in the owning page's action area. The editor MUST NOT invent
parameter/state mappings or a different UI shell to enable Apply.

For a back-calculation adapter, a PID law MAY additionally declare
`antiWindup: { method: "back-calculation", trackingGainSymbol: "..." }`.
That symbol is one unique bound REAL/LREAL parameter in reciprocal seconds. Its
value is `1/Tt`; zero is the recorded untracked baseline, while a candidate
uses a finite positive `Tt` and writes its reciprocal through the same complete
parameter record. The law declaration, rather than a parameter name, gives
that value its tracking meaning. A loop without this adapter can display a
saturation finding but cannot offer a connected back-calculation candidate.

A PID law MAY also declare `proportionalSetpointWeightSymbol`: one unique bound
REAL/LREAL record field, dimensionless (`unit: "1"`) and bounded within 0–1,
holding β in `P = Kp·(β·SP − PV)`. Manual PI and PID settings then require β
beside the gains, the predicted response uses it, and it is written through the
same complete parameter record. A calculated candidate is the standard form and
writes β = 1 unless β is pinned.

A PID law MAY also declare `measurementFilterTimeSymbol`: one unique bound
REAL/LREAL program setting in seconds, not negative, holding the time Tf of the
first-order filter on the measurement the controller uses
(`α = Δt/(Tf + Δt)`; 0 bypasses it). Manual settings require Tf, the predicted
response and loop frequency response include the filter lag, and Tf is written with
the gains. A calculated candidate writes Tf = 0 unless Tf is pinned. The program
restarts the filter from the fresh measurement in Manual and in the Auto-entry scan,
so the exact Auto-entry check still holds.

A loop which supports the automatic matched anti-windup verification route
MUST additionally declare `controller.antiWindupVerification`. Its scenario
contains the explicit isolated-simulation scan interval, duration, deterministic
seed, per-run wall budget, finite initial symbol values, and scan-aligned
numeric events. The scenario MUST initialize the declared mode, setpoint and
retained-integral symbols, and it MUST NOT target a controller parameter. The
runtime runs the original and tracked parameter records from this one immutable
scenario; changing either record's initial values, events, timing, seed, source
or output limits makes the comparison inconclusive. A loop without this
declaration can review a candidate manually but Control Studio must explain why
it cannot run the automatic matched comparison.

The adapter declares supported structures from those actual bindings:

- P requires a complete parameter record mapping and writes integral and
  derivative gains as explicit zero terms.
- PI requires proportional and integral mappings.
- PD requires proportional and derivative mappings.
- PID requires proportional, integral and derivative mappings.

Tune MUST retain the exact mapped parameter record, not only displayed gains.
Verify executes that record against the compatible ST controller and simulated
plant. Apply validates the complete record, writes it atomically through the
declared adapter at its owning task boundary, observes the resulting revision
and retains the prior revision for rollback. A controller with missing,
ambiguous, stale, incompatible or non-atomic mappings exposes the unavailable
reason; Control Studio MUST NOT enable a structure based solely on controller
type or symbol spelling.

Cascade, ratio, split-range, gain-scheduled, adaptive, and MIMO lessons reuse
the primary plus auxiliary signal set. Their controller equations remain in
the runnable ST project; Control Studio observes, tests, analyses, and retains
evidence through the same runtime client.

## 2. Opening Control Studio

`Open Control Studio` MUST be available from:

- the permanent truST project sidebar immediately below
  `Devices & Connections`;
- a configured simulated plant;
- a configured controller, experiment, or model-analysis loop; and
- the command palette.

The sidebar destination MUST remain visible but disabled with a reason when no
project is open. Activating it with a project open MUST use the existing
Control Studio command and editor; it MUST NOT introduce a second panel or a
second runtime communication path. A configured plant's contextual action
remains the preferred route when the user wants to preserve a specific plant
or loop selection.

A runnable project MAY provide `control-loop.json` at its root and additional
loop definitions in `control-loops/*.json`. Discovery MUST enumerate both in
deterministic filename order, validate every definition, and reject duplicate
loop IDs within one project. More than one definition requires a labelled loop
chooser. Cancellation MUST leave the current editor and files unchanged.
The file contains the complete loop definition from Section 1; names or
addresses MUST NOT be inferred from the ST program. When that file exists,
activating the sidebar destination or invoking the command without arguments
MUST validate it and open that bound loop. The selected runtime remains the
scope authority; opening MUST NOT silently switch targets. In a multi-root
workspace, more than one configured project requires a labelled project/loop
chooser; cancelling leaves the current editor unchanged.
When it is absent or invalid, the sidebar MUST explain that no valid control
loop is configured instead of opening an unexplained unbound workspace. The
no-argument command follows the same rule and offers the configuration file as
a repair destination when present. If no configuration exists, it MUST offer **Configure control loop** using
explicit compiled signal selections and **Choose an example** through the
existing gallery. Discovery never writes files. Setup saves only after a complete
binding review, rejects incompatible bindings and never overwrites an existing
configuration. Cancellation leaves project files unchanged. Explicit plant and loop handoffs retain
their supplied scope and do not enter workspace discovery.

The plant handoff defined by Specification 34 MUST be consumed directly. It
MUST preserve project, runtime, plant, loop, source, and revision identity.
Control Studio MUST resolve current values through the existing truST runtime
control client. It MUST NOT introduce a second runtime communication system.

Opening the same loop MUST reveal its existing editor. A stale or invalid
handoff MUST leave the existing editor unchanged and show a useful reason.

## 3. Source and state

Every session and exported result MUST state one source:

- `Simulation`;
- `Physical plant`;
- `Recorded data`; or
- `Offline model`.

Simulation evidence MUST NOT be presented as physical evidence. Predicted
model behavior MUST NOT be presented as an observed response.

Store source, runtime identity, revisions, mode, quality and freshness with the
session and recordings. They are provenance and operation inputs, not a header
or status strip. Runtime IDs and hashes do not belong in page copy. Report a
condition that blocks an action in that action's area; stale data must not be
used as current evidence.

## 4. Page structure

### Shared truST UI/UX contract — mandatory

The normal workspace has exactly **Identify / Tune / Validate** navigation and
the controls defined in those three page specifications. Apply is an action on
Validate. Reuse the existing shared [theme](../../editors/vscode/src/webview/theme.css)
and [tokens](../../editors/vscode/src/webview/theme.ts). Do not add another UI
system, navigation row, toolbar, dashboard or general-purpose Details panel.

Every visible item must support a decision or action on that page. Labels
state what is selected, calculated or changed. Use full identification/tuning
method names; abbreviations may follow the full name, not replace it. Signal
selectors show the actual signal name and unit. Controller choices identify the
actual ST instance or declared group as well as its recommended configuration.
Settings use readable labels such as **Proportional gain (Kp)** and
**Integral gain (Ki)**, with the relevant units. An example value is never a
placeholder result or a silently accepted operating limit.

A user must be able to identify the current page, next action, selected target
and meaning of the result without opening a help page or reading a paragraph.
Short field explanations may appear on keyboard focus or hover when a term
needs clarification; they do not add permanent text. Required missing values,
errors and unavailable-action reasons must instead be visible beside the
relevant control, with a specific corrective action. Do not rely on a tooltip
alone to explain why the next action cannot run.

The compact Limits, Requirements and Channels editors have clear field labels,
units, current values and Done/Cancel actions. Show only applicable fields.
Invalid input names the field and permitted value, preserves the other edits
and places focus at the problem. Do not expose JSON, protocol names, hashes,
scan counters, proof bookkeeping or unlabeled status symbols to the user.

Keep the response plot dominant, with readable axes and a small legend. The
model result is its type and equation; the Tune result is its controller and
settings; Validate shows measured requirements and the Apply/restore action.
Measurements and predictions must be distinguishable by text and line style.
No permanent runtime-status or SP/PV/MV strip is permitted.

### 4.1 Page layout

Use the exact element order and conditional controls in the three page specs.
Layouts must fit wide, split and narrow editors, including 200 percent zoom,
without clipping controls, labels, equations or parameter values. Avoid nested
scroll areas that hide the primary action or leave too little room to read the
plot. Keep related controls aligned and return focus when a compact editor
closes. Do not add a second page layout here.

### 4.2 Navigation and operation state

Use the shared automatic flow. Navigation does not start work or overwrite a
user's deliberate page choice. One visible Abort action remains reachable while
work runs, using the same operation identity across pages. Changing a view must
not duplicate acquisition or parameter writes. Errors and required recovery
appear in the owning action area, not as permanent tab-status badges.

For a loop bound to a local simulator, an authorized Identify/Validate operation
starts that simulator through the shared lifecycle when required. Recheck target
and cancellation before and after startup. Concurrent requests share one start;
a failed start retains its reason and permits retry. A physical target is never
started or replaced implicitly. Cancelling a study does not claim to have
stopped the shared simulator.

All displayed process values and analysis results MUST use the shared
engineering-number formatter. It MUST suppress meaningless floating-point
digits, retain meaningful small non-zero values, use engineering notation when
fixed decimals would mislead, render invalid or unavailable values explicitly,
and render a mathematically unbounded positive or negative margin as infinity
rather than as missing data. SP, PV, MV, limits, plot tooltips, and matching
summaries MUST use consistent precision. Controller and model parameters MUST
retain enough displayed precision to reproduce the result. Display formatting
MUST NOT change the full-precision value used internally, edited, recorded, or
exported.

## 5. Response and analysis data

Use one shared response projection and chart implementation. Selected-model
predictions use the task's reference change, duration, controller period and
physical operating point. A zero-delay model must not acquire an extra
controller-period delay. If the current controller cannot be modelled, do not
fabricate its baseline by duplicating the tuned response.

Retain bounded display data and exact underlying samples. Preserve gaps,
quality, source, channel units and event/limit identities. Display reduction
must retain extrema and gaps; it must not invent samples or join recordings
across a runtime restart. A candidate edit changes only its unapplied result;
recompute dependent response, margins and requirement evaluations against the
same candidate/model revision. Never combine results from different revisions.

### 5.1 Measurement and prediction integrity

The lifecycle I/O cache keeps observations with their tracked debug session.
A delayed lower-scan event cannot replace a newer observation or refresh its
freshness. A different session begins with its own scan history; untracked
events cannot populate the active session. The monitor's independent rewind
and freshness checks remain in force for observations reaching that boundary.

The shared Response projection MUST retain every declared auxiliary sample's
identity, unit and quality in live and reopened recordings. A missing, bad or
forced auxiliary value is a gap in that trace; it MUST NOT remove good primary
measurements. Display reduction retains auxiliary extrema and quality/gap
boundaries. Requested output and retained controller state are distinct from
applied MV: when selected values exceed the actuator range, their shared unit
axis expands to include them while keeping the actual actuator-limit lines.
No request or state trace may be clipped to imply that it stayed within the
physical actuator limits.

Do not overlay incompatible units on one axis. Keep channels and units explicit
and time aligned. Frame the actual response variation without forcing a distant
zero or rated maximum to hide it. Preserve out-of-range values and active limit
crossings; never clip data to imply compliance. Keep constant-value numerical
roundoff from appearing as meaningful oscillation. The plotted time range ends
at the last recorded sample rather than implying future data.

Recorded events retain exact time, source and meaning. Do not infer an event
from the shape of a curve. Measured and predicted traces have distinct labels
and line styles. These data rules do not require additional plot tabs, event
lists, buttons or toolbars.

### 5.2 Analysis eligibility

These are requirements for numerical evidence, not a catalogue of UI tabs.

| Analysis | Required evidence |
| --- | --- |
| Live response | Current or retained synchronized samples |
| Recorded experiment | One retained bounded recording |
| Model fit and residuals | A fitted model and the exact estimation data |
| Prediction versus validation | A model plus independent validation data |
| Pole-zero map, Bode, and Nyquist | An accepted linear dynamic model |
| Root locus | An accepted SISO linear model and declared controller structure |
| Margins and sensitivity | Accepted model plus current or candidate controller |
| Channel response matrix and RGA | An accepted multivariable gain or transfer matrix with declared channel ordering |
| Singular values | An accepted frequency-response matrix |
| Uncertainty envelope | Declared uncertainty or an accepted model set |
| Constraint trajectories | Declared constraints and a controller/model prediction that actually evaluates them |

### 5.3 Multichannel and stability evidence

Preserve input/output ordering, channel names, units and direction in every
matrix and prediction. RGA results retain numeric values and conditioning;
complex or singular cases must not acquire an unsupported pairing recommendation.
Singular values identify dimensional, normalized or decibel conventions.

Frequency-response data includes magnitude, phase, frequency units, evaluated
range and available crossover/margin evidence. A sampled or truncated Nyquist
trace alone does not establish a complete encirclement result. Retain the
contour, unstable-pole count and any supported closed-loop conclusion. Root
locus and pole data retain the controller structure, selected/fixed parameters
and continuous s-plane or discrete z-plane convention. Sensitivity data retains
L, S, T and control sensitivity where defined, their range and peak values.
Undefined or numerically unreliable calculations retain a specific reason.

## 6. Recording acquisition

Acquisition provides synchronized signal values, units, timestamps, source,
quality, controller/plant identity, limits and operating state to the shared
workflow. Opening saved evidence cannot replace it with live values or trigger
writes. Explicit live work resolves the current runtime identity anew.

Runtime point quality is retained as `good`, `forced`, `uncertain`, `stale`, or
`bad`; it is never silently promoted to good. A forced primary signal remains
visible but blocks excitation. Uncertain, stale, or bad SP, PV, or MV blocks a
new experiment and model fit while keeping the last evidence visibly marked.

The acquisition service MUST support bounded recording start/stop. A recording MUST
retain synchronized samples, timing, units, quality, source, and the selected
loop revision.

Runtime acquisition and editor refresh are separate clocks. A recording is
complete only when a runtime-owned bounded acquisition reports every scan in
the retained interval together with its scan-boundary time. Observing only the
runtime's latest snapshot from the editor does not prove complete acquisition,
even when the observed scan numbers are consecutive. Such a fallback MUST be
labelled `Incomplete` and `UI-observed snapshots`; it MUST retain exact missing
scan ranges whenever successive observed scan numbers differ by more than one.
It MUST NOT infer the missing values, an actual PLC cycle interval, or sample
freshness. Identification and any conclusion that requires a complete record
remain unavailable until a complete runtime-owned recording is present.

For incomplete recordings, observation time and the interval between editor
observations MAY remain visible but MUST be named as editor-observation
evidence rather than runtime cycle evidence. Capacity loss, a backwards scan,
runtime restart, acquisition reset, or revision-boundary ambiguity makes the
affected recording incomplete and MUST remain visible as a boundary or gap.
Saving or exporting an incomplete recording MUST preserve that status and its
known gap metadata; it MUST NOT upgrade it to complete evidence.
Known gap ranges MUST stay bounded to the retained sample interval. If the
bounded editor fallback evicts older observations, the evidence MUST retain the
number evicted and the user-facing incomplete reason MUST report that capacity
loss even when the retained interval also contains missing scan ranges.

## 7. Experiment execution

Experiment execution creates a bounded excitation and recording for identification or response
evaluation.

The experiment service supports automatic bounded excitation, plus
manual observation-only recording, setpoint step, load step, MV step, MV pulse,
relay, PRBS, and multisine excitation. MV step remains wire-compatible with
the earlier `output_step` name. Automatic selects a setpoint step only when the
reviewed loop is in Auto with a declared setpoint; it selects an MV step only
when the reviewed loop is in Manual or is explicitly open-loop; otherwise it
reports the missing safe path. A load step targets one
declared auxiliary `disturbance` write binding; Test never guesses one from a
symbol name. MV step/pulse, relay, PRBS, and multisine require confirmed Manual
mode; setpoint and load steps require Auto.

Observation uses zero amplitude. Sequence timing is derived from the positive observed sample
period: PRBS dwell is `max(10*sample_period, 500 ms)`, while multisine samples
at `max(sample_period, 50 ms)` and uses the first three integer harmonics of
`1/duration`. PRBS uses an explicitly retained nonzero 32-bit seed. Generated
values, including both PRBS levels and every multisine point, MUST remain
inside the affected signal range. The immutable experiment plan retains kind, affected signal,
current/restoration value, requested bounds, duration, interval, seed or
frequencies when applicable, complete bounded trajectory, stop conditions, and
restoration action.

For a project-scripted lesson, the project owns experiment kind and activation;
external excitation must not duplicate it. A project may declare
`parameterPurpose: "experiment"` with `parameterBinding` and its Boolean
activation symbol (`manual_value: false`). This is valid only for project-scripted
execution with an activation binding. Parameter updates preserve typed bounds,
explicit authority, observed values and rollback through the existing owner.
Freeze the experiment settings through review, baseline and execution; release
that ownership only after terminal restoration.

A project-scripted experiment may declare `baselineSeconds`. The reviewed
maximum duration includes this baseline, the held experiment and restoration.
During the baseline the native recorder runs with activation disabled; the
existing timed-trajectory service enables the project only after that interval.
The project remains responsible for qualifying its actual measurements before
actuation. Save the consecutive native baseline and response samples together;
do not reconstruct a baseline from nominal values or a qualification flag.
Retain the baseline duration in the exact plan and reject a maximum
duration that leaves no sample interval after the baseline.

When the learner chooses how long the project holds its step, the loop names
that experiment parameter in `holdParameter`: a declared `parameterBinding`
parameter in seconds under `parameterPurpose: "experiment"`. Review reads the
applied value from the runtime and records the baseline, every scan of that
hold and one restoration sample, for example 5 s + 30 s + 0.1 s = 35.1 s.
`durationSeconds` is then the maximum and must record the longest permitted
hold. Review refuses a hold that cannot be read, lies outside its declared
bounds or exceeds that maximum.

An MV pulse writes the requested output at `0 ms` and writes the captured
baseline at half the planned duration; the return point is part of its runtime
trajectory and occurs before timeout. An MV step has only its initial requested
output point and remains held until ordinary restoration.

Automatic Test derives a bounded signed amplitude from the reviewed live signal
headroom, beginning at ten percent of its declared span. A configured
zero-amplitude observation becomes an Automatic request with at least a
ten-second duration. Its concrete step trajectory records the baseline at
`0 ms`, applies the step after ten percent of the duration, restores the
baseline at half duration, and uses the remaining interval to settle before an
independent trial. Fitting uses the held interval before that restoration; it
does not treat the required restoration as the process step.
An engineer who pins Automatic to a duration too short for those three phases
receives a rejected review with that specific reason; Control Studio does not
silently compress or omit the baseline or restoration.

For a closed loop in Manual, an MV Test writes the declared
`controller.manualOutputSignal` at its declared
`controller.manualOutputAddress` and `controller.manualOutputType`. The
recording continues to capture the actual declared MV/PV channels; the command
binding is never inferred from an observed MV name. A missing Manual-command
address or type blocks that Test with the specific configuration requirement.

Maximum duration MUST be between 1 and 3600 seconds. This bounded one-hour
ceiling accommodates the curriculum's slow thermal experiments while retaining
an unconditional runtime timeout and the always-available abort and restoration
path. A lesson duration outside this interval is invalid rather than silently
clamped.

Relay is feedback-dependent and MUST NOT be implemented as a precomputed,
time-driven square-wave trajectory. The relay plan retains the current process
value as its center, the process-value binding and unit, the current manipulated
variable as its bias and restoration value, symmetric high and low manipulated-
variable levels, and a positive process-value hysteresis half-width. The initial hysteresis half-width is derived as the larger of three
process-value resolution increments and 0.5 percent of the declared process-
value range. An explicitly configured larger value is permitted, but a value
below that noise-protection floor is rejected.

After approval the runtime writes the high relay level. It retains that level
while the process value remains inside the hysteresis band, writes the low level
when the process value reaches or exceeds the upper threshold, and writes the
high level when the process value reaches or falls below the lower threshold.
It MUST use fresh numeric process-value samples from the reviewed binding and
MUST recheck output limits before every write. A forced, missing, stale,
nonfinite, type-incompatible, or bad-quality process value; a changed binding;
an out-of-range relay level; an abort; or the maximum duration ends the relay
experiment and invokes the ordinary runtime restoration path. Closing or
reloading the editor has the same restoration requirement.

The relay preview replaces the ordinary trajectory with the process-value
center, upper and lower switching thresholds, manipulated-variable bias, high
and low levels, hysteresis, maximum duration, stop conditions, and restoration
value. A relay run retains every applied transition, crossing time, process-
value extremum, sample quality, and restoration result. Candidate calculation
is a separate reviewed action after the record has been qualified; completing
a relay run MUST NOT silently calculate or apply controller gains.

The default form MUST ask only for amplitude and maximum duration when they
cannot be derived safely. It MUST preview the affected signal, direction,
resulting bound, stop conditions, and restoration value.

Before a test can start, Control Studio MUST verify:

- fresh SP, PV, and MV;
- compatible sampling;
- controller mode and write authority;
- signal and actuator limits;
- maximum duration and timeout;
- stop and restoration paths; and
- the affected signal force state; and
- any fault or interlock that the configured loop exposes.

The first version MUST NOT invent a generic interlock or authority model. An
unknown fault, interlock, topology, or write-authority requirement MUST block a
physical test. A simulation test uses the available loop binding, runtime
authorization, force state, limits, mode, timeout, and restoration contract.

Simulation requires confirmation of the bounded test. A physical test requires
explicit approval immediately before execution. The approval MUST show the
source, signal, amplitude, duration, limits, stop conditions, and restoration
value. Any material change requires new approval.

The reviewed scan identifies the sample used for approval; it is not a frozen
scan number. Normal runtime scans MAY advance, and independently delivered
current snapshots MAY lag by a scan. Immediately before writing, the runtime
MUST enforce the declared freshness timeout and recheck the affected signal
value and type, force state, declared limits, required controller mode,
authorization, and restoration contract.

The runtime owns running, stopping, timeout, force/staleness detection, and
restoration. Closing
or reloading the editor MUST NOT leave an indefinite excitation. `Abort test`
MUST stop excitation and attempt restoration through the same runtime path.

For a Control Studio-owned Test, the runtime recording Start MUST be acknowledged
before the excitation is dispatched. Between those two operations, Control
Studio MUST durably retain one credential-free recovery owner containing the
exact loop/program/binding identity, recording ID, reviewed experiment ID and
complete reviewed plan. It MUST contain no endpoint, token, sample data,
approval token or mutable controller state. After a panel or Extension Host
reload, Control Studio may reconstruct only that recording owner, poll the
stored experiment ID and Stop the original buffer. It MUST use the stored plan
to label and retain the terminal result, then clear the owner only after the
terminal recording has been retained. If recovery cannot establish the exact
runtime identity, it remains unconfirmed and MUST NOT authorize a replacement
Test Start.

Abort has priority over every trajectory write: after the runtime accepts an
abort, no later excitation point may be queued or applied. A terminal stop
queues the restoration value but MUST report restoration as `pending` until a
subsequent runtime I/O snapshot confirms the exact typed value at the affected
address. If confirmation is not observed within the experiment freshness
timeout, the terminal status MUST be `failed_to_restore`, restoration MUST be
`failed`, and the original stop reason MUST remain available. Observation has
no write or restoration and reports `not_required`.

While an experiment runs, a forced affected signal, an unresolved or invalid
affected value, a value outside the reviewed signal limits, or stale runtime
data MUST terminate excitation and enter the same confirmed restoration path.

The result MUST state whether the test completed, aborted, failed, or failed to
restore. It MUST retain the samples and the exact stop reason.

An aborted, failed, failed-to-restore, incomplete, or unconfirmed Test record
MUST retain its received runtime-sample prefix, acquisition completeness and
exact stop/restoration reason. It is `Not qualified for Identify` even when its
received samples are otherwise usable. Retry MUST create a new reviewed plan
identity; it MUST NOT relabel or reuse the failed record as estimation or
validation evidence.

The terminal experiment record retains its outcome separately from live
runtime state. Model analysis and tuning completion must not overwrite it or
mark an unexecuted experiment complete. Unconfirmed restoration remains a
blocking recovery condition in the owning action area.

## 8. Identify

Identify creates a low-order plant model from a recording or imported data.

A saved Control Studio result MAY supply a recording to another project. Import
MUST decode the result and retain only its recording and acquisition provenance;
it MUST NOT adopt the source controller, candidate, model, tuning, verification,
commissioning or runtime configuration. The current loop keeps its identity.
The recording retains its original result ID/revision, timestamp, project, loop
revision/topology, controller revision, software, units, source classification,
and any source plant/experiment context. Raw sample identities MUST NOT be
rewritten to the destination loop. Save/Open MUST preserve this explicitly
versioned recording provenance, reject inconsistent sample/source/unit/revision
claims and leave ordinary same-loop result validation unchanged.

The imported input/output units MUST match the selected destination channels;
import does not infer unit conversion or physical plant equivalence. Imported
recordings remain historical and distinguishable from live observations. Import
issues no runtime writes and does not establish model suitability. The existing
qualification and fit owners MUST check the selected immutable recording, use
its stable source result/revision identity, and require a distinct validation
record before model acceptance for design.

A completed native recording MUST retain its original runtime recording ID in
acquisition evidence through Stop, release and saved-result serialization.
Identify MUST select the currently displayed recording and its matching Test
input mapping; an older ledger role cannot replace that selected source. The
first native Test after imported estimation can supply validation without
requiring a redundant estimation acquisition. Missing or malformed identities
cannot establish independence.

Before fitting, qualification evaluates selected inputs/outputs, units, sample
time, gaps, clipping, saturation, drift, excitation and suitability. Exclusions
never delete raw samples. The page presents the result and errors using its
current UI contract; qualification metadata is retained with the recording.

The qualification summary MUST retain raw and usable sample counts, retained
duration, median sample interval, maximum interval deviation, detected gaps,
rejected-quality samples, explicitly reported clipping and saturation samples,
baseline drift, input and output span, a time-domain signal-to-noise estimate,
the excitation class, and independent-validation availability. Coherence MUST
be recorded as `Not evaluated for time-domain excitation` for observation and step
records; it MUST never be invented from a step response. Suitability for a
same-run preview and qualification for controller design are separate states.

The first version supports these model structures:

- static gain;
- integrating process, with its delay: a step fit takes the rate gain from the
  late slope and the delay from where that ramp, extended back, meets the level
  before the step;
- first-order process;
- first-order plus dead time (FOPDT);
- second-order process; and
- second-order plus dead time (SOPDT).

A configured simulation model retains `Exact simulation model` provenance; a
fitted simulation model retains `Identified from simulation data` provenance.
These classifications are part of the model record, not permanent UI badges.

Each fitted model MUST retain parameters with units, measured versus predicted
response, validation error, residual behavior, stability, delay, operating
range, and warnings. The recommended model MUST be the simplest acceptable
model; fit percentage alone MUST NOT select it.

A model used for tuning or design MUST be fitted on an immutable estimation
run and scored on a distinct immutable validation run with a different
excitation amplitude or sequence. Control Studio MUST retain both residual
vectors and identify which run produced each one. Its initial low-order
parameter intervals are conservative search-resolution and held-out-residual
bands; they MUST NOT be labelled confidence intervals or profile likelihood.
A same-run preview may help the user inspect data, but it remains unqualified
for tuning or design.

For a bounded held-step analysis, Identify MUST retain the measured step and
restoration timestamps, baseline/final window bounds, counts, means, ranges and
endpoint slopes. The configured measurement bound, sustained-onset threshold,
sample count and every grid bound/spacing are engineering inputs, not nominal
plant coefficients. Bad, clipped, incomplete, ambiguous or nonsteady data MUST
remain available with the rejection reason. A detectable threshold crossing is
separate from fitted transport delay.

Detectable onset MUST follow the measured output direction from the baseline
mean to the final mean, independently of the input-step sign. For a falling
response, use baseline minus the configured threshold and require the declared
number of consecutive samples strictly below it; for a rising response, use
baseline plus the threshold and samples strictly above it. An isolated crossing
does not establish onset. Measurement and fitted-result views MUST report the
same crossing, and no measured output change MUST leave onset unavailable.

Window coverage MUST include every expected sampling instant inside each
requested interval, including an instant exactly at its start. A fractional
window start between sampling instants is permitted. The final window MUST lie
entirely within the held step, before restoration, and each window needs at
least two samples. A record with no input change MUST report the missing nonzero
step explicitly instead of exposing an internal indexing error.

Rejected qualification or fitting produces the owning action's specific error
while retaining the record for recovery. Window-coverage errors distinguish
missing coverage from too few samples. Client and host use the same settings
validation; invalid edits preserve other values and cannot be cleared by a
late acknowledgement for an older edit. Display formatting preserves the
full-precision measurement and fit records.

The bounded FOPDT fit fixes baseline and gain at the qualified window means and
minimizes hold-sample squared error on the explicit delay/time-constant grid.
Its separate uncertainty calculation enumerates baseline/gain/delay/time-
constant grid points consistent with both final-window bounds and every hold
sample's absolute measurement bound. Equivalent exact interval intersection is
permitted; random sampling or silent coarsening is not. Report an empty set
explicitly, never replace it with a residual-derived interval. Parameter ranges
are grid-resolved approximations, not statistical confidence or an assertion
that every combination of marginal bounds is consistent. Retain one-sided zero
delay and search-boundary findings. Timestamp shifts of up to one sample are
explicit sensitivity studies and MUST NOT relax missing-sample checks.

The accepted model MUST preserve the estimation fit when scoring a distinct
validation step with a changed amplitude. Fitting validation samples or choosing
new parameters from them is not independent validation. Changes to analysis
inputs invalidate dependent results; recordings remain immutable.
Invalidation MUST clear the selected design model, tuning/application readiness
and the current editable draft together. An empty current draft MUST supersede
older saved results on reopen; it MUST NOT resurrect their selected model.
Preserve historical results, recordings and compatible explicit settings.
Replacement admission MUST be checked before any analysis state is changed
while a runtime operation or its recovery remains active.

Saved identification state MUST retain the separate estimation and current
input records, their original identities, exclusions, model-family selection,
measurement/grid settings, acceptance threshold and review stage. A preview
MUST reopen as a preview, qualified validation MUST retain its estimation
reference, and an accepted model MUST reopen with both independent inputs.
Restore MUST recheck saved inputs through the existing qualification/fitting
owners before restoring acceptance; malformed or incompatible state MUST leave
the current analysis intact. A rejected Open MUST also preserve the current
result identity, revision, source and export contents; result metadata is adopted
only after the analysis restore succeeds. A restored preview must permit the next distinct
recording to supply validation without collecting estimation again. Saving or
restoring analysis MUST NOT acquire data or apply controller parameters.
The outer saved result must also preserve the original complete acquisition
records and provenance; derived input/output arrays alone do not replace them.

Interactive fitting and numerical saved-study replay MUST leave the extension
host responsive and use the existing bounded, cancellable analysis worker.
An edit, replacement recording, cancellation or editor disposal MUST prevent
late computation from publishing or persisting superseded evidence. A newer
Open in the same editor MUST supersede an older Open, including its result
identity and history projection. Reject malformed inputs and blocked runtime
admission before cancelling valid current work. Replay into an isolated
identification state and adopt it only after its inputs, derived model authority
and request ownership still agree. Cancellation retains the original recordings
and recoverable analysis inputs; it does not start or repeat runtime writes.

The current editable draft MUST retain identification qualification, preview or
acceptance, its settings and original recordings alongside the engineer's
settings. Check, Fit, Accept and recording selection MUST update that draft;
closing and reopening the editor MUST restore it ahead of older saved results.
Draft persistence MUST reuse the existing workspace draft writer and MUST NOT
append or replace a saved history revision. A draft is labelled as current work,
separately from an explicitly saved result. Malformed or incompatible draft data
MUST preserve the stored evidence and MUST NOT silently reactivate an older
accepted model. Import admission and invalidation obey the same shared session
boundaries as Check; independent estimation evidence remains available when the
next selected recording is intended for validation.

Editing identification fields MUST invalidate dependent model and controller
readiness immediately, before Check. The same draft MUST retain raw field text,
including temporarily empty values, the model selection, held-step option and
exclusion bounds. Reopening restores these edits without qualifying or fitting
them. A form without a recording offers Import; a form with retained data offers
Check. Accepted estimation authority is retired on edit while original recordings
and saved history remain intact. An unaccepted estimation preview remains
available for independent validation after editing the next recording's settings.
Only Check converts the raw form to validated analysis settings.

PRBS generation MUST use a declared register order, nonzero seed, amplitude,
clock interval, and sequence length. Multisine generation MUST retain every
angular frequency, amplitude, phase, sample interval, applied common scale,
and peak bound, and MUST reject tones at or above Nyquist. Frequency-response
estimation MUST retain the input/output spectra, complex response, magnitude,
phase, segmentation, and magnitude-squared coherence at every requested tone.
Its plot and table show the tones in ascending frequency with the phase
unwrapped from the lowest tone: each tone's phase lies within 180° of the
previous tone's, continuous as on a model's Bode plot.
A low-coherence tone is retained but disqualifies the response from transfer-
function fitting. A fitted transfer function MUST be scored on different
retained tones or a separate excitation run; the fitting tones alone are not
validation evidence.

If the data cannot identify an acceptable model, Identify MUST stop with a
specific reason and retain the data.

## 9. Tune

Tune creates a controller candidate. Opening Tune or changing a tuning control
MUST NOT modify the running controller.

The Tune page owns visible controls and results. Its selected target, model,
current parameter record and proposed record remain one coherent study.

The first version supports:

- automatic recommendation;
- SIMC;
- AMIGO where applicable;
- manual `Control.PID` settings; and
- imported `Control.PID` settings.

Every method MUST state when it is applicable and why it is unavailable.
Tune shows the predicted response of the candidate and of the current settings
on the selected model. For `Control.PID` Studio calculates the closed loop
itself. A law in the program that Studio does not calculate itself, such as a
switching, state-feedback or other project-owned law or a PID group on its
multivariable model, is predicted by running the program's actual
ST with the current and the candidate settings on the selected model in
isolated simulation, as Validate's model comparison does; nothing is written to
the running controller. When that simulation is unavailable, the candidate is
shown with the reason there is no prediction.
Controller candidates MUST use the exact PI, PD, or PID form, timing,
derivative filter, weights, limits, tracking, and anti-windup behavior defined
by Specification 32.

### 9.1 Qualified formula methods

The first qualified formula methods accept an identified or exact simulation
model only after that model has been accepted by Identify. They operate on the
same model, engineering units, actual sample interval, controller limits,
validation scenarios, and objective. A comparison MUST NOT refit or silently
rescale the model for one method.

`SIMC PI (Skogestad 2003)` applies to a stable, monotone, positive-gain
first-order or first-order-plus-dead-time model
`K exp(-L s)/(T s + 1)`, where `K > 0`, `T > 0`, and `L >= 0`. In the parallel
`Control.PID` convention it shall calculate:

```text
Kp = T / (K (lambda + L))
Ti = min(T, 4 (lambda + L))
Ki = Kp / Ti
Kd = 0
```

SIMC PI also applies to an accepted positive-rate-gain integrating model
`Kprime exp(-L s) / s`, where `Kprime > 0` and the effective delay `L > 0` is
resolved. `Kprime` has controlled-variable units per manipulated-variable unit
per second; it MUST NOT be displayed or used as a self-regulating steady-state
gain. In the same parallel convention it shall calculate:

```text
Kp = 1 / (Kprime (lambda + L))
Ti = 4 (lambda + L)
Ki = Kp / Ti
Kd = 0
```

The response control selects the declared closed-loop time `lambda`:

- `Faster`: `lambda = max(L, T / 3)`;
- `Balanced`: twice the Faster value; and
- `More robust`: four times the Faster value.

For an integrating model, the Faster value is its positive effective delay
`L`; Balanced and More robust use the same factors of two and four. An
integrating model without a resolved positive effective delay has no qualified
first-version response-time scale and MUST return an unavailable reason rather
than inventing one.

These three labels change only `lambda`; they do not change limits or
acceptance requirements. The selected numeric `lambda` MUST be shown in the
advanced result and saved with the candidate.

`AMIGO PI (Åström and Hägglund 2004)` applies to an accepted stable, monotone,
positive-gain FOPDT model with resolved positive dead time and `0 < L / T <= 1`.
It shall calculate:

```text
Kp = 0.15 / K + (0.35 - L T / (L + T)^2) T / (K L)
Ti = 0.35 L + 13 L T^2 / (T^2 + 12 L T + 7 L^2)
Ki = Kp / Ti
Kd = 0
```

AMIGO has no free response-speed parameter. The response choice changes the
common comparison objective and ranking, not the published AMIGO coefficients.
The UI MUST say this rather than presenting a modified formula as AMIGO.

Both methods use positive plant or rate gain in this version. Zero or negative
gain, nonfinite parameters, a nonpositive required time constant, invalid
delay, model error above `0.25`, unsupported model structure, and method-
specific domain failure MUST return a deterministic reason and MUST NOT create
a candidate. AMIGO remains unavailable for integrating models. Data that failed
qualification because it was noisy, stale, poorly excited, or insufficient
remains an Identify failure and MUST NOT reach Tune.

The native simulated plant for a selected delayed integrator MUST use
`model = "integrator_dead_time"` with scalar `gain` (rate gain), nonnegative
finite `dead_time_s`, finite `initial_delayed_input`, optional `input_bias` and
`output_bias`, and one initial state. Its equations are
`dx/dt = gain * (u(t-dead_time_s)-input_bias)` and `y = x + output_bias`.
Before recorded input history exists, use only `initial_delayed_input`.
Integrate each held-input interval exactly, including delayed transitions that
fall inside a simulation step; do not replace delay with a large time constant
or discard sub-step boundaries. Enforce the existing bounded delay-history,
reset, status and parameter-update contracts. The Control Studio selected-model
adapter maps `b/(a1*s) * exp(-L*s)` to rate `b/a1`, delay `L` and output-deviation
initial state, retaining existing operating-point offsets. Native config and
execution tests must establish delayed onset, area, output bias and reset before
claiming the actual-ST Verify route works.

Frequency results for an integrating model MUST include the integrator's
`1 / frequency` magnitude and minus-90-degree phase before delay and controller
effects are added. Treating an integrating rate gain as a static gain in a Bode
calculation is a deterministic calculation defect and MUST NOT produce margins.

Formula results are ideal/ISA time-constant PI values translated explicitly to
the parallel `Control.PID` parameters `ProportionalGain = Kp`,
`IntegralGain = Kp / Ti`, and `DerivativeGain = 0`. No series, incremental,
positional, vendor, or per-scan gain conversion is permitted. The existing
runtime passes the actual cycle interval to `PID.Update`; tuning MUST NOT bake a
nominal scan interval into `Ki`.

### 9.2 Candidate comparison and ranking

Tune MUST retain every attempted method with one of `Feasible`, `Rejected`, or
`Unavailable`. A feasible candidate is one for which all mandatory validation
requirements pass. Rejected and unavailable candidates rank after every
feasible candidate and remain visible with their reason.

Feasible candidates are ranked by task-appropriate structural complexity first.
Among equally simple qualifying candidates, use the declared response, effort
and robustness objectives under matched scenarios. Retain every raw result.
An improvement beyond mandatory requirements must not displace a simpler
qualifying design. There is no universal 10-percent equivalence band or fixed
SIMC-before-AMIGO preference; numerically/evidentially indistinguishable results
remain labelled equivalent, with deterministic presentation.

One supported constrained comparison is a deterministic bounded neighborhood around
the qualified SIMC candidate. It evaluates the formula PI, a less aggressive PI
with `0.8 Kp` and `1.25 Ti`, P by removing integral action, PD by removing
integral action and adding
`Kd = Kp min(max(L, 0.05 T), 0.25 T)`, and PID by adding the same derivative
term to the formula PI. Every non-formula candidate is labelled `Constrained
comparison`, not SIMC or AMIGO. All candidates use the same validation and
ranking function. The comparison MUST produce no candidate when none passes
mandatory requirements.

For an accepted integrating model, proportional control MUST remain eligible
because the plant already supplies integral action. With a resolved positive
delay L, P uses SIMC's proportional gain `Kp = 1/(Kprime (lambda + L))` with the
same response choices as PI; Automatic predicts it on the model as it predicts
PI, the declared input disturbance included, and refuses it with the prediction's
shortfall. PI, PD, and PID remain
eligible only when applicable and must justify their additional term through a
material benefit. For other first-version formula domains, unsupported P, PD,
or PID candidates are reported as unavailable rather than synthesized from an
unqualified rule.

Retain the winning structure, method, limiting requirement, rationale, exact
formula, lambda, scores, rejected candidates and conventions in the result.
Every generated candidate remains unapplied. The Tune page owns its concise
presentation; this record does not add a report or disclosure.

Formula authority:

- S. Skogestad, [Simple analytic rules for model reduction and PID controller
  tuning](https://doi.org/10.1016/S0959-1524(02)00062-8), *Journal of Process
  Control* 13 (2003), 291–309; and
- K. J. Åström and T. Hägglund, [Revisiting the Ziegler-Nichols step response
  method for PID control](https://doi.org/10.1016/j.jprocont.2004.01.002),
  *Journal of Process Control* 14 (2004), 635–650.

For a selected delay-free integrating plant `G(s)=k/s`, the connected P
candidate MUST use `lambda=k*Kp>0` and pole `-lambda`. In deviation coordinates
`y'=k*(u-u0+d)` with `u=Kp*(r-y)`, its constant error is
`e_inf=(u0-d)/Kp`; there is no invented controller bias. Bound separate target
and input-disturbance scenarios with
`E_inf=max(|u0|,|u0-d|)/|Kp|` and transient envelope
`(|deltaR|+E_inf)*exp(-lambda*T)`. Choose a bounded compatible gain satisfying
the declared error and settling allowances. Supplied and identified integrating
models use this same calculation and actual-ST Verify; a candidate prediction
alone never authorizes Apply.

For the supported stable first-order PD alternative, calculate the steady error
using its proportional DC gain: `|deltaR + K*u0|/(1 + K*Kp)`. Reject a nonpositive
closed-loop DC denominator or error above the declared error allowance minus
transient margin. Do not reject every PD controller merely because it lacks an
integrator: a finite error requirement may be met. Dynamic requirements still
require actual-ST verification before acceptance.

Compatible continuous SISO state-space and transfer-function representations
MUST feed the same controller calculations. The analysis adapter may calculate
`G(s)=C(sI-A)^(-1)B+D` within the existing transfer-polynomial order bound, retaining
the complete characteristic denominator: do not cancel poles that could hide
unstable internal modes. Reject nonfinite/numerically unresolved conversions or
unsupported domains explicitly. Keep model identity, units, operating point and
validation evidence unchanged. Actual-ST Verify uses the originally selected
model and its state coordinates; conversion is an analysis representation only.

Automatic recommendation MUST compare the permitted P, PI, PD, and PID structures.
It MUST choose the simplest candidate that meets the declared requirements. A
more complex controller may be selected only when simpler applicable designs
cannot satisfy the declared mandatory requirements with the required evidence.

The Automatic tuning method designs the selected structure to the declared
requirements. For a formula with a response choice it uses the choice nearest
the declared one whose predicted response on the selected model stays, from the
settling-time deadline to the end of the test, within the allowed error of the
target and within the settling band of its final value; of two equally near
choices the more robust one is used. When the requirements declare an input
disturbance, each prediction also covers it: from the operating point with the
setpoint unchanged, the disturbance added to the plant input at the start must
leave the output, from the deadline to the end of the test, within the allowed
error of the target and within the settling band of its final value. When no choice meets them and the model is
delay-free, first or second order and without zeros, Automatic places the
closed-loop poles with damping 0.8: first at the decay rate
max(4, ln(|Δr|/b))/Tₛ, at which the response envelope reaches the settling band
b from the setpoint change Δr by the settling time Tₛ but never more slowly than
the 2 % envelope, then 25 % faster at a time, for at most eight placements and
below the sample-rate limit ωh < 0.2. The controller's own zeros make the
response faster or less damped than the envelope, so each placement is checked
by its prediction; the first with non-negative gains whose predicted response
meets the requirements is used. Otherwise the structure is rejected with the
deviation of its fastest predicted response, the deadline and the limit it
misses. Without a model prediction the declared choice is used and
Validate decides. SIMC or AMIGO selected explicitly calculates its formula
unchanged.

Tune MUST provide:

- current and candidate time responses;
- control output and limit use;
- plant and open-loop Bode plots;
- gain and phase margins; and
- uncertainty or variation bounds when available.

A frequency-domain result MUST NOT appear without a valid model or measured
frequency response.

## 10. Validate

Validate determines whether a candidate satisfies the selected requirements.
It MUST compare the current controller and candidate under identical
conditions. One immutable validation-conditions record MUST be retained and
used by both simulations or executions; separate look-alike records are not
sufficient evidence. The record MUST contain:

- the complete plant and controller initial condition;
- each setpoint or load event, its affected signal, amplitude, and time;
- the exact sample-time sequence and total duration;
- the disturbance trajectory;
- the noise generator, amplitude, and deterministic seed;
- output and rate limits; and
- the model-variation cases included in the comparison.

Model-predicted validation MUST reconstruct both responses from that one
record. Simulation-observed, recorded, and physical-observed validation MUST
retain the corresponding measured event and sample sequence. If any required
condition is absent or differs between the current and candidate run, the
comparison is invalid and its dependent requirements are `Inconclusive`.

Validation MUST cover the applicable subset of:

- setpoint response;
- disturbance response;
- rise time, settling time, overshoot, and steady-state error;
- MV excursion, rate, saturation, and recovery;
- noise response;
- gain and phase margins; and
- bounded model or parameter variation.

Each requirement MUST report `Pass`, `Fail`, or `Inconclusive`, with the value,
limit, unit, source, and reason. A required scenario that was not run or has no
valid source data is `Inconclusive`; missing evidence never passes. Model-based,
simulation-observed, recorded, and physical-observed results MUST remain
distinct.

The default model-predicted noise-response limit is the larger of `120 percent`
of the current controller's noise-driven manipulated-variable root-mean-square
movement and `0.1 percent` of the configured output span. The absolute floor
prevents an almost motionless baseline from turning a small, physically
negligible increase into a relative-only rejection. The retained requirement
MUST show the resulting numeric limit and output unit. A project-specific limit
may replace this default only before the immutable validation plan is run.

A validation result MUST NOT change the running controller.

## 11. Automatic workflow orchestration

The automatic flow sequences acquisition, identification, tuning and validation. It MUST NOT implement separate versions of their algorithms.

The operation retains the selected model/data source, objective, constraints,
proposed experiment, stop/restoration policy and controller choices before
execution. The current page and shared-flow specs own how missing information
and any required physical review are presented.

For simulation, the complete bounded plan may be confirmed once. A reviewed
physical campaign may authorize its specified bounded trials together. A
materially different action outside that authorization requires renewed review.
Automatic orchestration MUST NOT enlarge or add an unapproved physical test.

Automatic orchestration MUST:

1. qualify or collect data;
2. select the simplest acceptable model;
3. generate applicable simple-controller candidates, including P, PI, PD and
   PID where appropriate, and the other structures defined by the complete
   workflow specification when the task requires them;
4. validate the candidates;
5. recommend the simplest passing candidate; and
6. present the result for review.

If a stage fails, it MUST stop at that stage, retain valid work, and explain the
nearest recovery action. If no controller passes, the result MUST be
`No acceptable controller found`; mandatory requirements MUST NOT be weakened.

Completion creates a candidate only. Automatic orchestration MUST never silently apply
controller settings.

## 12. Apply and rollback

PI/PID Auto commissioning requires a typed initialization adapter in addition
to the parameter-record adapter. Before the task receives Auto authority it
must read one fresh applied output, PV, setpoint, filter state and timestamp;
calculate the weighted proportional and derivative contributions from the
exact candidate record; initialize retained integral state so that the first
requested output equals the observed applied output within the declared
tolerance; and record the initialized controller/state revision. It must then
execute a bounded actual-ST observation with the same candidate record. Missing
state mapping, stale values, invalid timing, saturation ambiguity or a failed
first-output continuity check blocks Auto commissioning and leaves the
candidate available for Manual staging or revision. The proportional-only
initialization adapter MUST NOT be used for PI/PID.

The first supported typed adapter is `controller.pidInitialization` with this
exact shape:

```json
{
  "kind": "parallel-pid-v1",
  "integralState": { "symbol": "Main.Integral", "iecType": "LREAL", "unit": "%", "minimum": -100, "maximum": 100 },
  "derivativeState": { "symbol": "Main.PreviousWeightedError", "iecType": "LREAL", "unit": "degC", "minimum": -100, "maximum": 100 },
  "proportionalSetpointWeight": 1,
  "derivativeSetpointWeight": 1,
  "maximumSnapshotAgeMs": 100
}
```

It declares the actual retained integral and derivative/filter-history states,
their IEC representations and ranges, both setpoint weights, and how fresh the
single task-boundary snapshot must be. For this adapter, Control Studio reads
one snapshot of the applied output \(u_0\), process value \(y_0\), reference
\(r_0\), both retained states and timestamp. With candidate gains \(K_p\) and
\(K_d\), it calculates the weighted proportional contribution
\(K_p(b r_0-y_0)\), sets the retained derivative history to \(c r_0-y_0\) so
the first derivative contribution is zero, and initializes the integral state
to \(u_0-K_p(b r_0-y_0)-K_i(r_0-y_0)\Delta t\), because the declared parallel
form updates the integral once before calculating its first output. The owning runtime applies that complete state and
the mode at one task boundary, confirms the first actual output is within the
reviewed tolerance, and retains the snapshot and applied state as evidence.
The adapter is rejected if any declared state, unit, range, weight, freshness
bound or controller-law binding is incompatible. It never guesses ST variable
names or treats a parameter write as state initialization.

The same transfer protocol has an explicit `align_output` flag. It is `true`
only for the Manual-to-Auto entry, where Control Studio calculates the retained
states for first-output continuity. It is `false` for the later target change:
that transaction preserves the retained PID state, allows the reviewed target
response, and still confirms the actual ST state evolution at the task boundary.

A state-feedback law, u = u₀ + N·(r − y₀) − K·(x − x₀) in its identified
measured coordinates, has no retained state that makes Auto entry bumpless: at
entry each reference follows its measurement, so the first output moves by the
law's response to the process's deviation from the validated operating point.
Apply starts only when every measured output lies within its settling band bⱼ
of that point. Unless an Auto-entry tolerance is reviewed, the first-output
continuity check therefore allows, for plant input i, Σⱼ |Kᵢⱼ|·bⱼ + Σₖ |Nᵢₖ|·bₖ,
where j runs over the model states and k over the controlled outputs. A state
is a measured output when its column of C selects exactly that output, as in
identified measured coordinates; bⱼ is that output's settling band. A reviewed
tolerance replaces this allowance; a model with any other state keeps the exact
check.

Apply is the user's explicit action on the exact selected target and validated
candidate. The Validate page owns its confirmation and restore presentation.

Apply starts the candidate from the operating point it was validated at: every
plant input within the Auto-entry tolerance of that point and every measured
output within its settling band. An input that differs cannot settle by itself,
so Apply refuses at once and names it. When only measured outputs are outside
their bands, Apply waits for them, showing the output, its validated value and
its current value, and continues without another action once all are inside.
Nothing is written while it waits. The wait is cancellable and bounded by
max(60 s, 3 × the test duration); an output that has not settled by then is
reported with its current value, and the validated candidate stays available.
An integrating output has no level of its own: it rests wherever its inflow
balances, and its model does not depend on that level. Apply therefore takes
its present value as that output's operating point: the plant input must match
the validated point, the output needs no wait, and the validated step and the
Auto entry start from the present value.

After the target change, Apply observes the test duration in the runtime's own
time, from the runtime clock that `control.signals.inspect` reports, not in
wall-clock time: a runtime that runs behind real time, for example under load
or on a slower controller, is observed for the full duration. The observation
is bounded by max(3 × the test duration, the test duration + 60 s) of wall-clock
time; a runtime that falls further behind is reported and the previous settings
are restored.

Apply MUST use the existing runtime authorization, audit, revision, and control
transport. It MUST be atomic and reject a stale controller revision. A
simulation apply MUST remain labelled simulation and MUST NOT claim physical
deployment.

Atomic application means the complete `Control.PID.ParametersValue` structure
is validated before one scan-boundary write is queued. If any field, value,
enumeration, instance, authority, or expected revision is invalid, no field is
written, no pending revision is created, and the active revision remains
unchanged. A queued revision remains pending until the runtime reads back the
complete structure from a later scan. Control Studio MUST NOT label it applied
before that confirmation. If read-back shows another revision, Control Studio
shall report the conflict and shall not offer rollback using stale local
history.

After confirmed read-back, observe actual parameter/state adoption for the
30-second interval beginning only after that confirmation. Retain the outcome
and previous
confirmed parameter revision. Restoration uses the existing guarded transfer
and rollback contracts; report failure instead of claiming restored settings.
The Validate page owns the visible Apply/Restore action, without a status strip.

Physical apply is allowed only with the required runtime authority and a
separate explicit approval. Experiment approval does not authorize apply.

## 13. Saved results and export

Control Studio MUST save a versioned run result through its existing workspace
result storage. Each result MUST have a stable result ID, a positive revision,
creation time, project ID, loop ID and revision, controller instance and
revision, evidence source, extension version, and the runtime and numerical
adapter versions that were available when the result was created.

The raw recording MUST retain synchronized SP, PV, requested MV, and applied
MV values together with
elapsed time, observation time, actual cycle interval, scan number, engineering
units, signal quality, freshness, controller mode, active output and rate
limits, safety state, and event markers. Missing values MUST remain missing;
save, reopen, compare, and export MUST NOT interpolate or invent them.

Requested MV and applied MV MUST be separate nullable fields. The ordinary
controller-facing manipulated-variable binding is the applied plant command.
When no independently observed requested-output signal is bound, requested MV
MUST remain null; it MUST NOT be copied from applied MV merely to fill the
record.

When applicable, the result MUST also retain:

- the experiment kind, affected signal and address, requested and applied
  value, complete commanded trajectory, duration, derived interval, PRBS seed
  or multisine frequencies when applicable, approval source, bounds, stop
  conditions, stop reason, abort state, restoration value, restoration result,
  and restoration-failure reason;
- simulation or physical-plant identity, simulation configuration, effective
  seed, uncertainty, scheduled events, actuator behavior, and declared
  model-validity range;
- selected recording intervals, accepted model, fit and residual evidence,
  current controller, unapplied candidate, tuning method and objective;
- validation requirements, measured metrics, model-predicted margins,
  uncertainty results, evidence class, and conclusion.

Every observed sample and measured metric MUST keep the result's evidence
source. A Simulation result MUST NOT be loaded, copied, compared, or exported
as Physical plant evidence. Calculated or model-predicted evidence MUST remain
distinguishable from observed evidence after every save and reopen.

Saving a changed result creates the next result revision. It MUST NOT rewrite
or alias the raw recording of an earlier revision. Result history MUST be
bounded. Replacement and deletion MUST be explicit. A failed or interrupted
save MUST leave the last valid history readable and MUST report a specific
reason. Missing schema versions, unknown schema versions, malformed identity,
nonfinite samples, inconsistent units or source, inconsistent sample order, and
invalid selected intervals MUST be rejected without partially accepting the
record.

A retained runtime recording remains available for Save, Open and export when
its Test was aborted or is unqualified for identification. Keep identification
disabled for that data and preserve its stop reason, quality and restoration
result. Acquisition completeness does not establish experiment qualification
or a passed controller validation.

A terminal native recording remains immutable and available to the study as
soon as acquisition ends. Background observation cannot append to or replace
it. Saving does not qualify it as model or controller validation. Autosave and
reopening follow the shared flow; recorded data is opened on Identify.

A confirmed replacement of the bound debug runtime session invalidates cached
live inspection connections, program revisions and scan ordering even while a
saved result is displayed. The next explicit live operation observes the new
session. Preserve saved recordings and reject late inspection replies from the
previous session; a backward scan within one unchanged session remains invalid.
The live response starts a new time origin and sample window for the new
confirmed session. It MUST NOT draw an interpolated response across runtime
restart or reuse a saturation transition from the old session. Saved results
and the retained native recording remain available under their original identity.

Live observation, identification baseline and recording startup for the same loop MUST use the same
ordered signal selection, including actuator, auxiliary, controller-mode and acquisition-mode
channels. A background observation must not replace the coherent selection
needed by recording startup. Preserve the bound endpoint, loop and program
revision, and never replay a Start whose acknowledgement is uncertain.

The result service retains bounded saved revisions and their original source,
controller, model, method and acquisition identities. Restoring does not create
an observation, candidate, validation pass or runtime authority. An absent
candidate must not inherit another result's predictions or application status.

Autosave retains the study's page, signals, channel order, methods, candidate,
pins, requirements and underlying result identities. Reopening restores only
compatible saved data; it does not start a runtime or apply parameters. There is
no separate Save session, result-history panel or recording-management page.

The existing export service supports the complete typed workflow result as JSON and
the selected signal table as CSV. CSV MUST include elapsed time, observation
time, actual cycle interval, SP, PV, requested MV, applied MV, units, quality,
mode, safety state, limits, source, and event markers with sufficient numeric
precision to recalculate the saved metrics. PNG remains presentation evidence
only. Exported data MUST not contain credentials or runtime secrets.

This behavior extends the existing Control Studio result storage and export
path. The first version MUST NOT add a general artifact database,
content-addressed dependency graph, hash scheme, pagination system, background
synchronization service, or migration framework.

## 14. Failure and recovery

Control Studio MUST provide a specific state and recovery action for:

- missing or invalid loop bindings;
- stopped or unreachable runtime;
- stale or bad-quality values;
- unsuitable identification data;
- failed model fitting;
- unavailable tuning method;
- infeasible tuning requirements;
- no acceptable controller;
- aborted or interrupted experiment;
- failed restoration, apply, or rollback; and
- stale controller or candidate revision.

An error MUST preserve entered values and earlier valid results. The main plot
MUST NOT show fake example data in place of missing live evidence.

## 15. Visual, responsive, and accessible behavior

The shared UI contract in section 4 and the three page layouts govern all
rendering. Use the shared dark, light and high-contrast themes, with visible
keyboard focus, accessible names and descriptions, and equivalent keyboard
operation. Equations and response data must be accessible without adding a
permanent toolbar or report to the page.

Plots have readable units, trace identities and non-overlapping tick labels.
Colours and line styles distinguish data without colour-only meaning. Use the
same engineering-number precision as the result values. Respect reduced motion;
UI animation must not delay a value, cancellation or required recovery.
Analysis runs outside the webview thread, is bounded and remains cancellable.

## 16. Acceptance

Implementation requires numerical/runtime tests, registered extension tests
for changed behavior, and real rendered review. Documentation is not evidence
that the interface already meets the contract.

Exercise the complete three-page flow with configured and missing inputs,
automatic and manual method choice, recorded data, multiple PID instances,
multichannel mapping, failed requirements, Abort, Apply, restoration, reopening
and runtime restart. Inspect every required screen in light, dark and high
contrast at wide, split/narrow widths and 200 percent zoom, including keyboard
operation and both open and closed compact editors.

For each image and interaction, verify that the user can identify the current
page, selected signals/controller, primary action, result and next corrective
step without explanation. Reject unexplained abbreviations, ambiguous targets,
clipped fields/equations, unreadable units, unsupported controls, duplicate
status text, empty reports, a hidden Abort action or a plot crowded by controls.
Confirm only the selected controller/group is changed by explicit Apply/restore.

Physical qualification remains a separate boundary and must not be claimed from
simulated or predicted results. Record each image's usability verdict; a clean
DOM, passing unit test or attractive mockup alone does not close acceptance.

## 17. General engineer workflow

This section governs general-loop implementation where earlier PID-only or
lesson-specific restrictions conflict. These are product requirements, not
claims that the implementation already satisfies them.

### 17.1 Setup and capability boundaries

Configuration edits preserve compatible values and revalidate dependent
bindings. Cancellation leaves no saved configuration or runtime write. A late
validation reply must not accept an older draft or erase a newer error.
The current page/shared-flow contracts own missing-input presentation.

An engineer MUST be able to configure an existing ST control program through
**Configure control loop**. Setup reuses compiled/runtime symbol metadata,
autodiscovers recognized controllers, and permits explicit custom bindings
without requiring a wrapper FB. Bindings cover symbolic and addressed values,
optional SP, PV, requested/applied MV, auxiliary channels, mode, task timing,
units, scaling and limits. No signal role is inferred solely from its name.
The reused compiled binding catalog MUST include numeric process-image memory
variables as `processImage.memory` entries with their exact symbol, address and
IEC type, alongside the existing commands and feedbacks. No ST source parsing
or role-name guessing substitutes for this compiler metadata. The catalog also
provides `controlSymbols.symbols` (symbol path, IEC type, optional address) and
`controlSymbols.controllers` (instance path, compiled type, owning tasks).
Nested controller fields are enumerated from compiled instance storage and
metadata. Each task records its name and interval in milliseconds. The UI may
autodiscover known controller types from this catalog; a variable named PID
with another compiled type is still a custom controller.
The catalog also provides `controlSymbols.links`, derived by the compiler front end
from PROGRAM and FUNCTION_BLOCK bodies: for each `<output> := <instance>.<method>(
<argument> := <record>)` call, the instance, method, argument, record, assigned
output and, per record field, every symbol read on the right-hand side of its
assignments. Setup uses these links to prefill a `Control.PID` loop (its
`Configure` parameter record and its `Update` setpoint, measurement, manual
output and output) and to recognise a cascade, where one PID's output is the
only source of another PID's setpoint. The user confirms every prefilled binding;
a link is evidence, never a silent choice.
The catalog also provides `controlSymbols.writes`: every symbol that a PROGRAM or
FUNCTION_BLOCK body assigns, as absolute paths per compiled instance. Validation
starts the program from seeded values: the setpoint, manual output, mode,
parameter record and, through its initialization contract, the PID's state. When
the program writes one of the loop's seeded values itself, or calls `Initialize`
on the loop's PID, setup requires the BOOL that marks the program's startup as
done and declares it as an application state that validation starts TRUE.
Setup also requires the identification duration; it is never assumed.
The setup form also sets up an existing switching (ON/OFF) controller: the
program instance that runs it, its measurement and setpoint, its numeric output
with the low and high levels it switches between, its Auto/Manual signal and
manual output, and the three settings Tune designs: the hysteresis in the
measurement's unit, with its half-band or full-band convention, and the minimum
ON and OFF times in seconds. Setup also asks for the switched equipment's own
minimum ON and OFF times, when it has them, for example a compressor's restart
protection; blank means the equipment has none. They are declared as the output
channel's actuator dwell limits, and Tune never designs shorter minimum times.
Its retained switching states are the output latch,
which starts ON when the output starts at its high level, and optionally a dwell
timer in seconds, which starts at zero. A switching controller with a BOOL
output or other retained states uses the step-by-step setup.
The setup form also sets up an existing state-feedback controller, designed by
pole placement or LQR: the program instance that runs it, its plant input with
unit and range, its controlled output with unit, range and setpoint, and one to
three further measured outputs with unit and range. Each measured output is a
state of the model: Studio identifies the plant in these measured coordinates,
first order in each measured output (`identificationPolicy.maximumOrder` 1), so
every gain belongs to one measured signal. Per measured output the user chooses
the variable holding its gain, in the plant input's unit per that output's unit,
and for the controlled output also the variable holding the reference gain. The
program applies u = u₀ + N·(r − y₀) − Σ Kᵢ·(yᵢ − yᵢ₀) around the operating
point at which the plant is identified. The Auto/Manual signal, manual output,
identification step and duration and the startup flag are set as for a PID.
The setup form also sets up a P, PI or PID law that the program computes
itself from variables holding its gains, in parallel form: u = Kp·e + I + D with
I = u₀ + Ki·∫e dt and D = Kd·de/dt, or D = −Kd·dy/dt when the derivative acts on
the measurement y, where e is the control error in the measurement's unit, the
integral term I is in the output's unit and time is in seconds. The user
chooses the program instance that runs it, its measurement, output and
setpoint, its Auto/Manual signal and manual output, the law, per gain the
variable holding it with the range Tune may choose from, for PI and PID the
variable holding the integral term, and for PID the variable in which the
program keeps the previous error or previous measurement that its derivative is
taken from, and which of the two it is. Validation starts the integral term at
the output the loop has when it switches to Auto, with the setpoint at the
measurement, so the switch is bumpless, and starts the previous value at the
error of that moment (zero) or at the measurement, so the derivative starts at
zero. The identification step and duration and the startup flag are set as for
a PID. The loop is a project-owned controller (`controller.project`) whose
`parallel-pid` seed names the gain variables of its law: Tune designs its gains
from the model as for `Control.PID` (§9), offers only the families whose gains
the program has, and checks the result on the actual ST. A law with reset or
derivative times, a derivative filter or further retained states uses the
step-by-step setup.
The setup form also sets up a law the program computes from coefficient
variables that Studio has no design formula for, for example MPC weights, a
ratio, split-range break points, override limits or a gain schedule (§17.8 H,
I): the program instance that runs it, its measurement, output and setpoint, its
Auto/Manual signal and manual output, which kind of law it is, one to four
coefficient variables, each with its unit and the values Tune may try, and up
to three values the program keeps from scan to scan, for example an integral
term or a filter state, each with its unit and what validation starts it at:
zero, the output or the measurement. Tune runs every combination of the
values, at most 64, on the actual ST against the selected model, keeps the
combinations that meet every mandatory requirement and uses the one with the
smallest error after the settling time; of equal errors the first combination
in the listed order is used. Values of a REAL variable are rounded to the
nearest REAL. The identification step and duration and the startup flag are set
as for a PID.
The state-feedback structure also sets up LQG (§17.8 G), where the program
estimates the states from the controlled output instead of measuring them: its
estimator implements a fixed model, `x̂[k+1] = A·x̂[k] + B·u[k] + L·(y[k] −
C·x̂[k])` in deviations from the model's operating point, and feeds the
estimates back with the gains `K` and the reference gain `N`. Besides the
state-feedback fields the form asks for the model file in the project that the
estimator implements (a state-space model with one input and the controlled
output as its one output, sampled at the task period), and per model state the
estimate variable, its gain and its observer gain, as well as the process-noise
variance of each state and the measurement-noise variance, which set the
observer's trade-off between trusting the model and trusting the measurement.
The estimates start at zero, the operating point. Tune designs `K`, `L` and `N`
by LQG on the selected model, which must be the estimator's model: supply that
model file on Identify, or identify a model and write its matrices into the
program and the model file first.
All three pages remain navigable. Individual operations explain missing data,
models, adapters or permissions. Experiment ownership never disables an entire
analysis stage; project-owned excitation still forbids duplicate tool writes.

### 17.2 Models, controllers and analysis

Control Studio's models are domain-neutral: transfer functions and continuous
or discrete state-space models. It has no built-in physical families, and it
never chooses a model structure from signal units or names. A physical plant
enters as a supplied model, identified from its recordings, or simulated by the
project's own plant (Specification 33).
Accept fitted and provided transfer functions, continuous/discrete state-space
models and configured simulation models with provenance, channel order, units,
operating range, delay, sample time and validation status. Provided models can
support predicted analysis without fabricated identification records. Model
validation belongs in Identify; controller validation belongs in Validate.
Assigning configured channel names MUST keep independent predictions attached
to the same selected model revision and corresponding output. Renaming may
update those identities together, but must preserve the recorded samples and
prediction values. Unchanged channel names must not create a new revision.
Rectangular MIMO channel plots are supported; square-only mathematics is gated
per operation. Offline analysis MUST NOT require a live Control.PID instance.
Adapters distinguish PID, declared controller dynamics, state-space controllers
and actual custom ST execution. Existing numerical modules remain the engine.

Root locus varies one explicitly named parameter while holding the other
parameters fixed, recomputes each characteristic equation, and displays gain
labels, pole coordinates and selected candidate. For the shaft with J=B=1,
fixed measured-velocity feedback Kd=7, Kp=12 yields poles -2,-6; Kp=16 yields
-4,-4; Kp=20 yields -4+2j,-4-2j. Scaling Kd together with Kp is incorrect.
Timing, filters and delay approximations MUST be identified in the result.

### 17.3 Experiments and candidate evaluation

Reuse the runtime experiment service for declared multi-channel events,
initial conditions, duration, sample sequence, constraints, abort and confirmed
restoration. Evaluate engineer-declared error windows, overshoot, settling,
load recovery, MV limits/rates, saturation, switching and state constraints.
Current and candidate runs use identical immutable conditions. Missing evidence
is Inconclusive; distinguish model prediction from measured ST execution.
Formula methods retain their applicability domains. Custom parameter search
uses an explicit finite grid, trial count, per-trial duration, total budget and
abort contract, with no silent truncation. Execute actual ST against an isolated
simulated plant with complete resets, never the selected physical runtime.
Reject failed constraints before ranking qualifying candidates.

### 17.4 Application and persistence

Provide control.parameters.status/apply/rollback with typed, bounded parameter
sets, units/conventions, exact program/binding revisions and expected values.
Commit the whole set atomically at its owning task boundary after rechecking
revision and mode; never partially apply separate queued writes. Custom updates
require confirmed Manual/stopped state unless an explicit Auto adoption and
acknowledgement contract exists. Confirm observed values after commit; queued
is not applied. Rollback uses the retained previous snapshot and identical
guards. Physical application requires the existing explicit review. Retain
control.pid.* compatibility through the PID adapter.

Loop schema v2 and workflow result v4 MUST read v1 loops and v3 results. Migration
requires explicit save and preserves originals. Typed contracts cover bindings,
models, adapters, sweeps, experiments, evaluations and transactions. Panel
composition delegates discovery, analysis, execution and application to focused
modules; do not duplicate runtime transports or numerical engines.

### 17.5 Completion

The course gallery MUST use the selected lesson's name as its primary heading,
with position and equipment as compact context. Create/Open project actions
appear before expandable technical metadata and expected observations. Preserve
the full description, objective, expected observation and lesson explanation;
omit only identical repeated presentation. The disclosure must support keyboard
and pointer use, and the initial project action must remain visible at supported
window sizes and zoom levels. Use the shared truST theme and navigation.
Authored preview Markdown MUST retain its paragraph/list/table structure, inline
code/emphasis and equations. Render equations accessibly without deleting their
source meaning. Project-file references open a read-only preview of the bundled
example file and identify that scope; they do not create a project, switch the
workspace or adopt a retained project's state. Only files belonging to the
selected catalog example may be previewed. Reject escaped paths, unlisted
targets, executable URL schemes and raw HTML execution. Existing Create/Open
project actions retain their current behavior.

Prove an arbitrary non-course ST project end to end before aligning all 82
lessons. Audit each executable project against its teaching specification,
including the nine known workflow gaps. Rebuild complete example bundles and
the extension; verify Start from example against the packaged result. Tests
must cover cancellation, multiple loops, symbolic parameters, offline models,
rectangular MIMO, stale/forced/cross-runtime values, races, restarts, atomic
application and rollback. Real rendered light/dark/high-contrast evidence is
separate from native tests. Physical qualification requires a reviewed physical
target and must remain explicitly unqualified when that target is unavailable.

### 17.6 Declared root-locus contract

A v2 loop MAY declare `analysis` with a `plant` transfer function (descending
`numerator` and `denominator` coefficient arrays, `delaySeconds`, `inputUnit`,
`outputUnit`, `revision`, `source`, and `validation`). Source is `provided` or
`simulation`; validation is explicitly `unvalidated` or `validated`.
The `controller` declares a bounded parameter map (value, minimum, maximum,
unit) and descending polynomial coefficients. Each coefficient is either a
finite number or `{parameter: name, scale?: number}`. Parameter references must
exist. This is an explicit linear representation supplied by the engineer;
it is not inferred from custom ST. `rootLocus` names one parameter, its finite
values and its selected value; all other parameters stay fixed.

The declared plant input/output units MUST match the bound MV/PV units;
Control Studio MUST reject a mismatched model instead of silently converting it.
Declared root-locus analysis is available without a live controller or acquired
identification record. Its result retains selected/fixed parameters, provenance,
approximation and gain/pole coordinates. Reject unknown coefficients, zero
denominator, invalid bounds, nonfinite values, an empty or greater-than-1001
value sweep, or a selected value absent from the sweep. This numerical contract
does not add controls or tables to the Tune page and cannot authorize Apply.

Result v4 stores supplied analysis under `declaredAnalysis` as its exact
`definition` and calculated `result`, including selected parameter value,
fixed parameters and model provenance. Persistence uses the existing
result service; reopening restores that result and selection. A run
with no acquired samples records `evidence.observation: "Not observed"`.
Saving creates v4; reading v3 preserves its version and source content. Selecting
a root-locus candidate MUST NOT write the controller's live parameters.

### 17.7 Runtime scan recording

`control.recording.start` starts an observation-only recording on the existing
runtime I/O snapshot stream. Its request declares `recording_id`, `loop_revision`,
`bindings` (unique id, exact address and IEC type), `maximum_samples` and
`maximum_duration_ms`. Bounds are 1–32 signals, 2–12000 samples, and
10–3600000 milliseconds. At most two recordings may be retained concurrently.
This operation never writes or excites the process.

The runtime captures every published scan, with the actual scan sequence,
runtime timestamp, typed values and force flags, before an editor can miss an
update. `control.recording.stop` is idempotent and returns the retained samples
and acquisition evidence. `control.recording.release` frees an acknowledged
recording. Scope revisions must match on stop/release. Duplicate identities,
invalid bounds and incompatible bindings are rejected before recording.
Missing/nonmonotonic scans or absent runtime timestamps yield Incomplete;
forced or invalid values retain their quality flags and cannot be treated as
good identification samples. Capacity/duration stops are reported explicitly.
Stopping or restarting the runtime terminates an active recording with a reason.
Legacy UI-observed recordings remain explicitly incomplete.

Result v4 raw samples retain `additional` channel entries with id, channel,
role, unit, nullable value and quality. Channel identities and units must remain
consistent throughout one recording revision. JSON and CSV export retain these
channels. An open-loop result declares `loop.topology: "open-loop"`, uses no
SP value or SP unit, and must not manufacture either from the process variable.
Runtime-buffer records use runtime elapsed time and scan sequence for ordering;
coarse or adjusted wall-clock timestamps do not replace that time base.

A Stop requested while the start acknowledgement is pending MUST wait for that
acknowledgement and stop the same recording. It MUST NOT be discarded as an
unmatched operation. Project, runtime, loop ID and loop revision all belong to
the recording scope; matching revision strings alone cannot authorize a stop.
If the runtime accepted Start but its acknowledgement is lost, the persisted
owner MUST retain that unconfirmed fact across an Extension Host reload. The
reopened manual Record surface MUST name the unconfirmed Start, keep Start
disabled, and offer Stop only for the same recorded identity. Its terminal
samples remain actual runtime samples, but their evidence MUST remain
`Incomplete` with the Start-acknowledgement reason; it MUST NOT qualify
identification as complete evidence.

### 17.8 Competence target — proposed 29 September 2026, confirmed 30 September 2026

This subsection records the proposal that makes the purpose contract's user
direction of 29 September 2026 concrete. It is not an implementation
requirement until the user confirms it; the user may change any item. The
user's message of 29 September 2026, "Remember that maybe we have 2 pids, one
that feeds the other. We must handle that and all different cases. What about
mimo and other controllers ?", makes these parts binding: setup and design of a
cascade, of several independent loops and of multivariable PID groups (RGA
pairing and decentralized PI, §17.9), and setup of other controllers. On 30
September 2026 the user confirmed the rest ("all"), as these capabilities, each
specified in its own subsection before it is built:

- A. gain, phase and delay margins and the peak sensitivity Ms on Tune for
  every candidate;
- B. measured-disturbance feedforward designed from the model (static gain and
  lead/lag);
- C. a cascade tuned inner loop first, the outer loop through the closed inner
  loop, validated and applied together with one rollback;
- D. the static decoupler applied, not only calculated;
- E. Smith predictor design from the model's delay;
- F. gain-scheduling design over declared operating points;
- G. LQG and a program's own PID law with a derivative term in the setup form;
- H. MPC tuning of horizon and weights;
- I. ratio, split-range and override structures in the setup form.

- **Setup.** Any compiled truST project can be set up in one form from compiler
  metadata: single loops, a cascade (one controller's output feeding another's
  setpoint), several independent loops, multivariable channel groups, state
  feedback and custom project controllers, with symbolic or addressed signals.
- **Identification.** First- and second-order, integrating and delayed models and
  transfer functions up to third order, from step, pulse, relay, PRBS, multisine
  or recorded data, for SISO and MIMO processes, each with residual and
  independent-validation reports.
- **Design and tuning.** `Control.PID` configurations with setpoint weights and
  derivative filtering; cascades designed inner loop first and the outer loop
  through the closed inner loop; measured-disturbance feedforward; RGA pairing,
  decentralized PI and static decoupling; state feedback, LQR/LQG and MPC; gain
  scheduling.
- **Every candidate.** Gain, phase and delay margins and peak sensitivity;
  verification on the actual ST; guarded application of all affected
  controllers together, with rollback.

### 17.9 PID groups: decentralized PI design

A PID group is two or more `Control.PID` instances tuned, validated and applied
together because their processes interact: every member's output moves several
measured outputs. The group contract (`controller.group`) pairs each member's
input channel with one output channel and shares one Auto/Manual signal, so the
members change mode together.

**Setup.** The setup form offers **PID group** when the program has two or more
library PIDs. The user selects the members and, per member, confirms the
compiled measurement, output, setpoint, manual output and parameter record, as
for a single loop, plus the shared Auto/Manual signal, the identification step
of each output and one identification duration. Setup writes one group loop
whose model channels follow the member order; each member's input channel keeps
that member's identification step (`experimentStep`).

**Identification.** Identify runs one experiment per member output in one
recording, stepping each input by its own identification step, and fits one
multivariable model (§17.2).

**Design.** Tune designs one PI per member from that model:

1. Steady-state gains `K` (outputs × inputs) and the relative gain array
   `Λ = K ∘ (K⁻¹)ᵀ` are calculated for the members' pairing. A pairing whose
   diagonal relative gain is not positive, or a singular `K`, is refused, and
   the pairing with positive diagonal relative gains closest to 1 is named.
2. Each member's own path `yᵢ/uᵢ` is reduced to a first-order-plus-delay model
   from the model's step response by the two-point method: with `t₂₈` and `t₆₃`
   the times at which the response reaches 28.3 % and 63.2 % of its final
   change, `τ = 1.5·(t₆₃ − t₂₈)` and `θ = t₆₃ − τ`; when `θ` would be negative,
   `θ = 0` and `τ = t₆₃`.
3. With the other loops closed, member `i` sees the effective steady-state gain
   `Kᵢᵢ/λᵢᵢ` (effective open-loop process). SIMC designs the member's PI for
   that gain and the path's time constant and delay, with the member's own
   requirements.

The design names, per member, the path model, `λᵢᵢ` and the resulting gains.
Validate then runs the actual ST of all members against the full multivariable
model, with every member's requirements, and Apply writes all members in one
transaction with one rollback; only the group changes (§16). A static decoupler or a centralized
design (LQR/LQG, MPC) remains a separate controller structure.

### 17.10 Robustness margins on Tune (§17.8 A)

Tune shows beside every candidate whose law it can write as a linear transfer
function on the selected model its gain margin, phase margin, delay margin and
peak sensitivity Ms: a `Control.PID`, or a program's P/PI/PD/PID law set up with
a `parallel-pid` seed, including the candidate's derivative filter and the
model's delay. With the loop transfer function L(jω) of candidate and model:

- the gain margin is −20·log₁₀|L| in dB where the phase of L crosses −180°;
- the phase margin is 180° plus the phase of L where |L| = 1;
- the delay margin is the phase margin in radians divided by that gain-crossover
  frequency: the extra dead time the loop tolerates before it becomes unstable;
- Ms is the largest |1/(1 + L(jω))|, the largest factor by which feedback
  amplifies a disturbance at any frequency.

A margin the loop does not have, because the phase never crosses −180° or the
loop gain stays below one, is shown as unbounded, and so is the delay margin
without a gain crossover. The margins are calculated on the model, so they are
exactly as trustworthy as the model; switching and other nonlinear laws show
none. A state-feedback law (pole placement or LQR) with one plant input shows
the margins of its loop broken at the plant input: `L = K·(zI − A)⁻¹·B` on the
model sampled at the task period, with the gains `K` on the fed-back outputs,
evaluated up to half the sampling frequency. The delay margin is then the extra
dead time at the plant input the loop tolerates.

### 17.11 Cascade tuned together (§17.8 C)

The setup form's **Cascade** structure writes one cascade loop for two
`Control.PID` instances where the outer controller's output is the inner
controller's setpoint: the inner controller drives the plant input and
measures the inner measurement; the outer controller measures the outer
measurement against the outer setpoint. The loop's model has the plant input as
its one input and the inner and outer measurements as its two outputs.

- **Identify** puts the inner controller in Manual, steps its manual output and
  fits one model from the plant input to both measurements, first order in each
  measurement (`identificationPolicy.maximumOrder` 1), so the outer measurement
  follows the inner one as in the process.
- **Tune** designs the inner controller first, with Automatic (§9.2), on the
  path from the plant input to the inner measurement for the inner
  requirements. It then predicts on the model the closed inner loop's response
  of the outer measurement to a step of the inner setpoint, reduces that
  response to first order with delay by the two-point method of §17.9 and
  designs the outer controller on it, with Automatic, for the outer
  requirements. Without declared inner requirements the inner loop is
  designed to settle in a fifth of the outer settling time with the outer
  settling band scaled to the inner measurement's range, so the outer design's
  assumption that the inner measurement follows its setpoint holds.
- **Validate** runs the actual ST of both controllers in Auto from the model's
  operating point with the outer requirements, including a load at the plant
  input that both loops reject.
- **Apply** writes both parameter records and switches both controllers to
  Auto in one task-boundary transaction (the outer controller's mode is an
  `additional_modes` member of the transfer): the inner controller starts at the
  present plant input with its setpoint at the inner measurement, and the outer
  controller starts with its output at the inner measurement, so neither
  switch bumps. **Restore previous settings** returns both parameter records
  and both controllers to Manual in one transaction.

A cascade whose two loops are declared as separate loop files, each with its
own study, keeps the rules of §3 for those files.

### 17.12 Feedforward from a measured disturbance (§17.8 B)

The setup form's **Feedforward** structure writes one loop for a `Control.PID`
whose program adds a feedforward from a measured disturbance `d` to the
controller's `FeedForward` input. The program computes

`FeedForward = K_ff·(z + (T_lead/T_lag)·(d − z))`, where `z` is `d` filtered by
a first-order lag with time constant `T_lag`,

which is the lead/lag `K_ff·(T_lead·s + 1)/(T_lag·s + 1)` applied to `d`, with
unit steady-state gain; a program without lead/lag values computes
`FeedForward = K_ff·d`. Besides the single-loop fields the form asks for the
measured disturbance (compiled variable, unit and range), the variable Studio
steps to move the disturbance during identification (in a simulation usually
the simulated load itself), that step, the program's gain variable `K_ff` and,
optionally, its lead time, lag time and the filtered disturbance `z` it keeps
(all three or none). The loop's model has the controller output and the
measured disturbance as its inputs and the measurement as its output; the
disturbance is measured, never written by the controller.

- **Identify** puts the controller in Manual, steps its manual output and then
  the disturbance variable, each in its own interval of one recording (as a PID
  group's inputs, §17.9), and fits one model from both inputs to the
  measurement of at most second order (`identificationPolicy.maximumOrder` 2),
  so that each path can keep its own time constant. The feedforward is designed
  from both paths' gains and time constants, so the fit must agree with the
  independent recording within 5 % (`identificationPolicy.maximumNormalizedError`
  0.05): a first-order fit that shares one time constant between paths that
  differ is then not accepted.
- **Tune** designs the PI with Automatic (§9.2) on the path from the controller
  output to the measurement, for the requirements. It designs the feedforward
  from the two paths, each reduced to first order with delay by the two-point
  method of §17.9: `(K_u, τ_u, θ_u)` from the controller output and
  `(K_d, τ_d, θ_d)` from the disturbance.
  - `K_ff = −K_d/K_u`: in steady state the feedforward's change of the output
    cancels the disturbance's effect on the measurement.
  - `T_lead = τ_u` and `T_lag = τ_d + max(0, θ_d − θ_u)`: the lead cancels the
    lag of the output's path, the lag reproduces the disturbance's; a delay
    the disturbance takes longer than the output is approximated as further
    lag. When `θ_d < θ_u` the disturbance reaches the measurement before any
    action can; that part of the deviation remains for the feedback.

  Tune shows both path models, the three values and, on the model, the largest
  deviation of the measurement after a disturbance step of the identification
  step with the PI alone and with the feedforward. The feedforward acts outside
  the loop: the margins of §17.10 are those of the PI on the output's path.
- **Validate** runs the actual ST on the model in the single-loop cases of §10
  and in a measured-disturbance case: from the operating point with the setpoint
  unchanged, the disturbance steps by its identification step at the start, and
  the measurement must be within the error allowance by the settling time.
- **Apply** writes the PID gains and the feedforward values in one transaction
  and switches to Auto. The controller's `FeedForward` state starts at `K_ff`
  times the measured disturbance and `z` at the measured disturbance, so the
  switch does not bump. **Restore previous settings** returns all of them.

### 17.13 Smith predictor designed from the model (§17.8 E)

The setup form's **Smith predictor** structure writes one loop for a
`Control.PID` whose program feeds the controller the measurement plus a
correction, `y + c`, instead of the measurement `y`. The correction is the
delay-free model's response to how much the controller output `u` changed over
the model's delay:

`T·dc/dt = K·(u − u_θ) − c`, with `u_θ` the controller output `θ` seconds earlier.

With a correct model `y + c` is the measurement the process would give without
its delay, so the controller can be tuned as for the process without delay.
The program keeps the output's history over the delay, filled with the output
it is applying, before the controller's update, while the controller is not in
Auto and on its first scan, so that `c` is zero whenever the output has been
steady and the first Auto update's change of the output enters the history. Besides the single-loop fields,
whose measurement is the process measurement `y`, the form asks for the
program's variables for the model gain `K`, time constant `T` and delay `θ`
(in s), and the correction `c` it keeps.

- **Identify** is as for one loop.
- **Tune** reduces the model's step response from the controller output to the
  measurement to first order with delay by the two-point method of §17.9 and
  writes that gain, time constant and delay into the program's model variables.
  It designs the PI with Automatic (§9.2) on the model without its delay, for
  the requirements' settling time less the delay: with a correct model the
  loop's response is that of the delay-free loop, delayed by `θ`. A settling
  time no longer than the delay is refused with that reason. Tune shows the
  model values and the gains.
- **Validate** runs the actual ST on the selected model in the single-loop
  cases of §10.
- **Apply** writes the PID gains and the model values in one transaction and
  switches to Auto with `c` at zero; **Restore previous settings** returns all
  of them.

### 17.14 Gain scheduling over declared operating points (§17.8 F)

A process whose gain or speed changes with its operating point is controlled
with gains that change with it. The setup form's **Gain schedule** structure
sets up a `Control.PID` whose program sets its proportional and integral gains
each scan from a schedule table: one pair of gain variables per operating point,
interpolated between the points by a scheduling variable such as the
measurement. Besides the single-loop fields, the form asks for two to four
operating points, each with a name and the program's proportional and integral
gain variables for that point.

Setup writes one loop per operating point, named after the loop and the point,
whose parameters are that point's two gain variables (a `parallel-pid` seed,
§17.1). Each point is identified, tuned, validated and applied on its own:

- **Identify** at the point: bring the process to the operating point in Manual
  first, then identify, so the model describes the process there.
- **Tune** designs the point's PI on that model with Automatic (§9.2).
- **Validate** runs the actual ST with the program's schedule active on that
  model from its operating point, so the gains the program interpolates as the
  process moves are the ones validated.
- **Apply** writes the point's gain variables and switches to Auto; **Restore
  previous settings** returns them.

### 17.15 Static decoupler applied (§17.8 D)

A PID group (§17.9) can pass its members' outputs through a static decoupler
in the program: in Auto the program writes the plant inputs `u = D·v`, where
`v` are the members' `Control.PID` outputs and `D` is a matrix of program
variables, one per plant input and member; in Manual it writes each member's
output to its own plant input, `u = v`, so identification and Manual operation
act on the plant itself. When the program has a decoupler, the setup form's PID
group structure also asks for its variables `D_ij` (plant input `i`, member
`j`), whose unit is plant input `i`'s unit per member `j`'s.

- **Identify** is as for a PID group, in Manual.
- **Tune** calculates the steady-state gains `K` of the group model and
  `D = K⁻¹·diag(K)`: through the decoupler each member's output moves only its
  own measurement in steady state, with the gain of its own path. A singular
  `K` is refused. Each member's PI is designed as in §17.9 on the model seen
  through the decoupler, whose relative gains are then one. Tune shows `D` and
  the per-member design.
- **Validate** runs the actual ST, decoupler included, on the group model with
  every member's requirements and each setpoint alone, as in §17.9.
- **Apply** writes the PI gains and `D` in one transaction and switches the
  group to Auto. Each member's PID starts at its share of the present plant
  inputs, `v = D⁻¹·u`, so `u = D·v` does not change at the switch. **Restore
  previous settings** returns all of them and the group to Manual.

### 17.16 Trajectory feedforward designed from the model

Required by the curriculum change the user confirmed on 30 September 2026 (a
lesson on motion trajectory feedforward). A loop whose program moves its
reference along a planned trajectory `r(t)` when the setpoint changes, for
example a motion profile, may add the controller output with which the model
follows that trajectory: `FeedForward = K_v·ṙ + K_a·r̈`, with the planned rate
`ṙ` and acceleration `r̈` the program computes, added by `Control.PID`'s
`FeedForward` input. The project controller's `trajectory-feedforward-pid`
seed names the PID's three gains, the program's rate gain `K_v` and its
acceleration gain `K_a`: five distinct bound parameters.

- **Tune** designs the PID for the chosen family as Automatic designs
  `Control.PID` (§9.2), and the feedforward from the same model. For a model
  `n/(a·s² + b·s)` of controller output to measurement, an integrator with a
  lag when `a ≠ 0`, `K_v = b/n` and `K_a = a/n`: the model then follows the
  trajectory exactly. For torque to angle `1/(s(Js + B))` these are `K_v = B`
  and `K_a = J`. A model of another form has no trajectory feedforward rule,
  and Tune says so. A delay is not anticipated; its effect remains for the
  feedback. The feedforward acts outside the loop, so the margins of §17.10
  and the Auto-entry allowance of §12 are those of the PID. Tune's predicted
  response runs the program with the current and the tuned values.
- **Validate** runs the actual ST in the single-loop cases of §10; the setpoint
  change starts the program's trajectory.
- **Apply** writes the PID gains and both feedforward gains in one transaction.
  The planned rate and acceleration are zero at rest, so the switch does not
  bump; **Restore previous settings** returns all five values.
## Primary Control.PID state adapter

The primary guided PID journey executes the actual library block defined by
Specification 32. A parallel PID test equation is a separate implementation.
A schema-version-2 loop may declare `pidInitialization.kind = "control-pid-v1"`
with `states` and `maximumSnapshotAgeMs`. The seven states must bind the actual
controller instance's `StateValue.Setpoint`, `SetpointRate`, `ProcessValue`,
`ProcessValueRate`, `FeedForward`, `NominalOutput` and `SaturatedOutput` members.
Each state uses the existing declared-state scalar descriptor (LREAL, unit,
finite bounds, initialization source and bounded post-scan observation).
Missing, duplicated, incorrectly typed or incorrectly owned states are rejected.
This adapter and `parallel-pid-v1` describe different retained-state semantics.

At a stationary operating point, setpoint and process history use the measured
process coordinate, both rates are zero, and nominal/saturated history use the
applied actuator coordinate. Feedforward history uses the application's declared
initial feedforward value. All seven states are included in actual-ST Verify
and its mandatory state bounds. Initialization and live commissioning reuse the
existing declared-state task-boundary transaction, boundary-measurement formulas,
first-output observation, cancellation and recovery owners. No copied controller
equations or second transaction service may replace the actual block.

P, PI, PD and PID tuning retains the existing gain-law mapping and accepted
family. Parameter declarations address the actual configured parameter members;
startup initialization must not overwrite an isolated candidate or accepted live
settings. Actual-ST evidence must confirm the entire parameter/state/mode path.
A successful descriptor or isolated test is not full commissioning acceptance.

Applications with startup supervision may additionally declare
`pidInitialization.applicationStates` using the existing declared-state scalar
contract. These explicitly named application states, such as a completed-startup
flag, accompany the seven library states during Verify and commissioning. They
prevent application startup code from overwriting the reviewed controller history
before its first Update. The adapter must not guess a flag from its name or
substitute a looser response requirement for missing initialization. Application
states retain compiled ownership, type, bounds and post-scan checks.

## General identified-model response evidence

Every identified model retains the independent free-running predictions used
to qualify it, with timestamps, measured/predicted outputs, residuals, physical
units, source recording identity and selected model revision. Warm-up history
used to initialize a state predictor is not presented as independent prediction.
After identification, Identify shows measured versus predicted response for the
selected output. Frequency response remains an analysis view; it must not replace
available fit evidence because a general state/physical fitter selected the model.
Model replacement invalidates the comparison; reopening the same model restores
its retained evidence. All output channels retain their own units and predictions.

Time-response plots frame SP/PV and MV around their actual displayed variation.
Do not force a distant zero or rated actuator maximum into the range when that
hides the response being assessed. Declared bounds remain in the result and Limits/Requirements editors;
relevant active bounds are drawn within the visible range; data reaching a limit remains visible.
