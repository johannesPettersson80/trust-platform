# VS Code Control Studio

Status: normative truST product specification.

Control Studio is the VS Code workspace for observing, testing, identifying,
tuning, and validating one configured control loop. It uses the same
workflow for a simulated plant and a physical plant while always identifying
which source produced the evidence.

The PID algorithm is defined by
[`32-control-pid.md`](32-control-pid.md). Simulated plant behavior is defined by
[`33-simulation-plant-models.md`](33-simulation-plant-models.md), and simulated
plant configuration is defined by
[`34-vscode-plant-simulation-ui.md`](34-vscode-plant-simulation-ui.md).

The words MUST, MUST NOT, SHOULD, and MAY are normative.

## 1. Supported control loop

Every Control Studio binding declares whether its primary view is
`closed-loop` or `open-loop`. A closed-loop view keeps SP, PV, and MV
immediately visible. An open-loop view keeps the applied process input and
measured process output immediately visible and MUST NOT invent a setpoint.
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
- direct or reverse action;
- controller mode;
- output and rate limits when configured;
- task period and observed sample time; and
- the simulated plant when the loop uses one.

When a controller mode signal is bound, the loop definition MAY state whether
a true value means `Auto` or `Manual`. A legacy binding that omits the polarity
uses true for `Auto`. Control Studio MUST use the declared polarity and MUST NOT
infer it from a variable name.

A lesson loop MAY define Test defaults: execution owner, experiment kind,
amplitude, maximum duration, and deterministic seed. `control-studio`
execution means Control Studio writes the reviewed excitation through the
existing runtime experiment service. `project-scripted` execution means the
runnable lesson or simulator already owns the scenario; Control Studio MUST
present it as `Observe without excitation`, use zero amplitude, record the
declared duration, and perform no duplicate process excitation. A
project-scripted scenario MAY declare one writeable Boolean activation binding.
When present, **Confirm and start** MUST use the same runtime experiment service
to write that binding true, while Abort, timeout, failure, and normal completion
MUST restore it false and confirm the restored scan. The preview MUST name this
as lesson experiment authority, not as SP, MV, or a second excitation. A
project-scripted scenario without an activation binding remains observation
only. The UI MUST show these defaults when the loop opens.

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
lesson project. A project-owned controller uses the same Monitor, Test,
recording, plots, and evidence transport, but Control Studio MUST NOT present a
generic PID candidate as that controller or write `Control.PID` parameters to
it. A declared Tune / Design stage for a project-owned controller requires its
own typed design adapter; until that adapter is present the action remains
disabled with the missing-adapter reason.

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

A runnable project MAY provide one explicit `control-loop.json` at its root.
The file contains the complete loop definition from Section 1; names or
addresses MUST NOT be inferred from the ST program. When that file exists,
activating the sidebar destination MUST validate it and open that bound loop.
When it is absent or invalid, the sidebar MUST explain that no valid control
loop is configured instead of opening an unexplained unbound workspace.

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

The header MUST show the loop, source, connection, controller mode, active
controller revision, candidate state, and data freshness. Stale data MUST stop
advancing and remain visibly historical.

## 4. Page structure

The primary navigation MUST contain exactly:

1. Monitor;
2. Test;
3. Identify;
4. Tune; and
5. Validate.

The page MUST open on the loop, not a questionnaire or feature dashboard.
Automatic Tune is a primary action that uses the five modes; it is not another
mode.

SP, PV, and MV MUST remain visible whenever live values exist. During a test,
the monitor MUST also show elapsed time, injected signal, active limits, and a
permanently visible `Abort test` action.

Advanced controls and explanations SHOULD remain collapsed until requested.
Safety state, errors, stale data, and failed requirements MUST never be hidden.

### 4.1 Fixed visual hierarchy

Control Studio MUST use one editor surface with this top-to-bottom hierarchy:

```text
┌ Loop identity · source · runtime · mode · revision · freshness ─ Automatic Tune ┐
├ Monitor ─ Test ─ Identify ─ Tune ─ Validate ────────────────────────────────────┤
├ Response ─ Signals ─ Events ───────────────── Cursor  Zoom−  Zoom+  Fit  Expand ┤
│                                                                                 │
│                         PRIMARY ENGINEERING PLOT                                │
│                                                                                 │
├ SP 35.0 °C │ PV 28.4 °C │ MV 52.0 % │ Auto │ Good │ Not saturated ────────────┤
├ Active-stage controls, evidence, decisions, warnings, and results ──────────────┤
└─────────────────────────────────────────────────────────────────────────────────┘
```

The plot workspace MUST span the available editor width. At a 1080-pixel-high
wide viewport its normal plot canvas MUST be at least 420 pixels high and
SHOULD occupy approximately half of the usable editor height. The chart MUST
not be compressed merely to keep an empty task area visible below it.

The live-value strip MUST remain immediately adjacent to the plot and MUST show
the primary SP, PV, and MV, engineering units, controller mode, point quality,
and saturation or limit state. It MUST use compact text, not dashboard gauges.

The active-stage workspace appears below the live-value strip. On wide layouts
it MAY use two columns for a short input form and a result summary. It MUST NOT
place a permanently open parameter inspector beside the main plot. Detailed
settings, model coefficients, and numerical tables use progressive disclosure.

### 4.2 Header and workflow navigation

The header MUST use human-readable identity first. A controller instance name
or revision label is shown normally; a hash is shortened only after the useful
name and the complete value remains available in a tooltip and accessible
description. Runtime and controller state MUST use readable text with adequate
contrast; they MUST NOT resemble disabled debug metadata.

Each workflow tab MUST expose one of `Ready`, `Needs data`, `In progress`,
`Complete`, `Warning`, or `Failed` through text or an accessible tooltip. A
bare unexplained dot is insufficient. Selecting a stage with unmet
prerequisites MUST open that stage and explain the missing prerequisite and
the next action; it MUST NOT present a blank panel.

Automatic Tune remains in the header. It MUST be visually secondary to a
currently active Abort or safety action.

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

## 5. Shared plot behavior

Control Studio MUST use one shared chart implementation for all plots.

Every plot MUST:

- identify signals by label and line style as well as colour;
- show axis names, units, and source;
- render missing or bad-quality samples as gaps;
- mark stale boundaries, experiments, limits, and important events;
- provide cursor values, zoom, reset, and a tabular accessible alternative;
- distinguish measured, identified, and predicted data; and
- preserve the current time window when the user changes mode.

The webview MUST receive bounded display data. It MUST NOT retain an unbounded
sample array or invent interpolated samples.

### 5.1 Plot workspace and visible controls

The plots belonging to the active workflow stage and the essential chart
controls MUST always be visible directly above the chart. They MUST NOT be
hidden inside a generic `Plot tools` disclosure, nested catalogue, or
recommended-versus-complete mode. The toolbar contains the stage's direct plot
tabs followed by `Inspect`, zoom out, zoom in, `Fit`, `Expand`, and `Export`.
`Signals` remains a direct Monitor tab rather than a menu. While a Tune plot is
being edited, `Undo` and `Redo` appear in the same toolbar. PNG, SVG when
supported, CSV, and the accessible table are direct Export actions.
Figure export MUST contain the currently visible plot; selecting Bode and
pressing `Export` MUST NOT silently export the Monitor response instead.

Mouse-wheel or trackpad zoom operates around the cursor. Dragging pans when
zoomed. A horizontal selection MAY zoom to an interval. Keyboard users use
Left/Right to move the cursor, `+`/`-` to zoom, Shift+Left/Right to pan, Home
to fit, and Escape to leave expanded mode. Every action MUST have a tooltip,
visible focus, and an accessible name.

The current cursor MUST show exact time and every visible signal in a compact
tooltip using the configured engineering precision. A vertical cursor line
MUST remain visible while inspecting. Linked time-domain plots MUST share the
same cursor and time window.

`Expand` MUST enlarge the plot workspace within the current VS Code editor; it
MUST NOT open a browser, a second webview, or a separate communication path.
The toolbar, primary live-value strip, active safety state, and Abort action
remain available in expanded mode.

### 5.2 Direct stage plots

Every plot has exactly one obvious home in the five-stage workflow. Selecting
a workflow stage shows that stage's plot tabs directly in one horizontal row:

| Stage | Direct plot tabs |
| --- | --- |
| Monitor | `Response`, `Signals`, `Events` |
| Test | `Response`, `Experiment`, `Limits` |
| Identify | `Fit`, `Residuals`, `Validation data`, `Poles and zeros` |
| Tune | `Response`, `Bode`, `Root locus`, `Nyquist`, `Nichols`, `Sensitivity` |
| Validate | `Requirements`, `Response`, `Robustness`, `Constraints` |

For a multivariable loop, the same stages gain only the directly relevant tabs:

- Identify adds `Channel responses` and `RGA`;
- Tune adds `Singular values` and `Interaction`; and
- Validate applies `Requirements`, `Robustness`, and `Constraints` to all
  declared channels.

There is no separate advanced mode, plot browser, plot catalogue, learner
mode, professional mode, recommended group, or all-plots group. A user who
wants Bode selects `Tune` and then `Bode`; a user who wants RGA selects
`Identify` and then `RGA`.

Every applicable tab remains visible. An unavailable tab is disabled and its
tooltip states the exact prerequisite, for example `Identify and accept a
dynamic model first`. A short prerequisite sentence also appears in the plot
area when the disabled tab receives keyboard focus. The UI MUST NOT require a
user to complete a prerequisite before learning where the resulting plot will
appear.

Control Studio remembers the selected plot separately for each workflow stage
and loop. When a selected plot becomes ineligible because data or identity
changes, the stage returns to its first valid tab and explains why. It MUST NOT
display stale analysis under a new loop identity or model revision.

The minimum eligibility rules are:

| Plot | Required evidence |
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

### 5.3 Signal selection and MIMO behavior

The primary loop's SP, PV, and MV MUST appear by default, and the live-value
strip MUST always retain them even if the user changes the plotted signals.
Selecting the direct `Signals` tab replaces the plot with one flat signal list.
It is not a drawer, popup, nested tree, or separate configuration page.

Each row contains one checkbox, friendly name, role, symbol, unit, live value,
and quality. Checking or clearing it updates the Response plot immediately.
The first rows are always primary SP, PV, and MV. Remaining rows follow their
declared order. The selection MUST persist while the user moves between
workflow stages and when a compatible analysis session is saved and reopened.

For a multivariable loop, one plain channel heading separates each channel,
for example `Overhead composition` and `Bottoms composition`. A channel
heading has one checkbox that selects or clears its declared SP, PV, MV,
actuator, and measured-disturbance rows. There are no Show/Hide menus or nested
channel controls. Returning to `Response` shows the chosen signals.

Adding a signal MUST NOT silently place incompatible units on an existing
axis. Signals with the same unit share one axis. Two compatible unit groups MAY
use clearly labelled left and right axes. Three or more unit groups MUST use
vertically aligned plot lanes with a common time axis and cursor instead of
adding more overlaid axes. Each lane is labelled with channel, role, and unit.

MIMO analytical plots MUST retain explicit input and output ordering. Matrix
rows and columns MUST use channel names, units, and direction; hovering or
keyboard focus on a matrix cell identifies the exact input-output pair.
Relative Gain Array values MUST be shown as numbers and not only by colour.
Singular-value plots MUST state whether values are dimensional, normalized, or
in decibels. Pairing recommendations MUST retain the underlying matrix and
must not hide a poorly conditioned or singular result.

### 5.4 Axes, lines, limits, and events

Axes MUST use the actual declared engineering ranges and units. A manipulated
variable declared as 0 through 100 percent MUST use a 0 through 100 percent
axis; it MUST NOT be autoscaled to 0 through 1 merely because current samples
are near zero. Common aliases such as `degC` MUST render as conventional
notation such as `°C` without changing the stored unit identity.

SP, PV, MV, candidate, disturbance, and limit traces MUST have stable line
styles in addition to colour. Normal dense traces SHOULD render without a
symbol at every sample; symbols appear for the cursor, sparse points, events,
or when zoomed sufficiently. Overlapping traces MUST remain distinguishable by
line style, z-order, and the cursor readout.

The fitted time range MUST end at the last retained sample rather than at an
arbitrary rounded future time. Streaming MAY leave a small bounded right-side
margin, but it MUST NOT imply that future samples exist.

Configured high/low limits, output saturation transitions, test start/end,
setpoint or load changes, mode changes, excluded intervals, stale boundaries,
and declared simulation events MUST be rendered as labelled lines, bands, or
markers. An event marker's tooltip MUST state its time, source, and meaning.
Markers MUST originate from explicit runtime, experiment, recording, or loop
configuration evidence; Control Studio MUST NOT infer an event from the visual
shape of a response.

The direct `Events` tab MUST present the retained event evidence as one
chronological list with time, event class, and a readable description. Stored
identifiers such as `recording_started` MUST be rendered as ordinary language,
not exposed as internal protocol text. Opening a retained recording MUST make
this tab usable even when the live runtime is disconnected. The list MUST not
invent lifecycle events that are absent from retained evidence.

Measured traces use solid lines. Model predictions and uncertainty envelopes
MUST be identified explicitly and use a distinct dashed line or band. A legend
MUST state `Measured`, `Simulated`, `Identified model`, or `Predicted` where
provenance could otherwise be confused.

### 5.5 Advanced plot requirements

Bode plots MUST show magnitude and phase on aligned logarithmic-frequency
axes, with crossover frequencies and available gain, phase, and delay margins.
Frequency units (`Hz` or `rad/s`) and magnitude representation (`ratio` or
`dB`) MUST be explicit and switchable without changing the underlying data.
Magnitude and phase MUST read as one coordinated Bode figure: the frequency
range, zoom window, pan position, and inspection frequency are shared; only one
legend is shown; and inspecting either panel exposes the matching values in
both panels. The two panels MUST remain vertically aligned when stacked and
MUST NOT look like unrelated charts. The magnitude panel omits redundant
frequency labels when the phase panel immediately below provides the shared
frequency axis.

The fitted Bode frequency window MUST contain the plant's useful dynamics and
the sampled gain and phase crossovers. For a delay model it MUST NOT extend
through many additional dead-time rotations merely to display a mathematically
valid but unreadable phase tail. Wider qualified numerical evidence MAY be
retained for export, but the initial visible window MUST keep crossover,
margins, and the dominant dynamic range legible.

Nyquist plots MUST show the complete evaluated contour, direction, the
`-1 + 0j` critical point, encirclement result, open-loop unstable-pole count,
and the resulting closed-loop stability conclusion. A truncated positive-
frequency trace MUST NOT be presented as a complete Nyquist proof.

Root-locus plots MUST show open-loop poles and zeros, branch direction, the
current gain, candidate gain, and declared design regions or requirements.
Pole-zero maps MUST state the continuous `s` plane or discrete `z` plane; the
unit circle is mandatory in the `z` plane.

Sensitivity views MUST provide at least loop response `L`, sensitivity `S`,
complementary sensitivity `T`, and control sensitivity when defined. They MUST
show the frequency range, peak values, bandwidth, and the controller/model
revision used. Undefined or numerically unreliable values render as
unavailable with a reason.

### 5.6 Direct graphical controller design

When an accepted model and editable controller candidate exist, the `Bode`,
`Root locus`, and `Nichols` Tune tabs MUST allow direct graphical tuning on the
plot. The user manipulates visible crossover, pole, zero, gain, or compensator
handles appropriate to the selected controller structure. Every handle MUST
have a precise keyboard alternative and an accessible description containing
the parameter name, current value, unit, and permitted range.

Every continuous graphical handle MUST also provide an exact numeric input in
the same compact editing row. The handle and numeric input are two synchronized
controls for the same value, not separate editing modes. The row MUST show the
parameter's engineering unit and the candidate's currently evaluated gain and
phase margins. A valid edit updates the handle, exact value, margins, and plots
from the same candidate revision. Invalid text or an out-of-range number MUST
leave the last valid candidate visible and explain the permitted range.

A graphical edit changes only the unapplied candidate. It MUST NOT modify the
running controller, begin a plant experiment, or apply gains. The plot MUST
show the current controller and edited candidate with stable, distinct line
styles. Invalid, nonfinite, structurally impossible, or out-of-range edits are
rejected at the interaction boundary, leave the last valid candidate intact,
and explain the violated constraint next to the plot.

Every accepted candidate edit MUST immediately recompute the candidate's
frequency response, closed-loop response, stability margins, sensitivity,
constraint predictions, and validation requirements from one consistent
candidate revision. The visible plot, live candidate summary, and requirement
status update together; switching directly to another Tune or Validate tab
shows that same revision without an additional run or save action. While a
bounded calculation is in progress, the previous valid result remains visible
and labelled `Updating`. A failed calculation restores the previous valid
candidate and reports the reason. Results from different candidate revisions
MUST NOT be combined.

Declared requirements appear directly on their applicable plots as subtle
graphical regions or boundaries:

- overshoot and settling-time regions on time-response plots;
- crossover-frequency, bandwidth, gain-margin, and phase-margin regions on
  Bode or Nichols plots;
- damping, natural-frequency, settling-time, and stability regions on a root
  locus or pole-zero map;
- peak-sensitivity and robustness bounds on sensitivity plots; and
- actuator, rate, output, and process constraints on response and constraint
  plots.

Each region MUST state its requirement, unit, boundary, and Pass, Fail, or
Inconclusive result in text as well as graphics. Regions are shown whenever
their declared requirement applies; they are not hidden behind a plot-layer
menu. Overlapping regions MUST remain distinguishable without obscuring the
measured or predicted traces.

The current controller and active candidate are always compared. `Pin design`
retains up to three additional candidates in one compact comparison row above
the plot. Each pinned item shows its name, controller structure, key tuning
values, validation result, source, and remove action. Selecting an item makes
it the inspected comparison without opening a catalogue or manager. All
compared designs use the same model revision, experiment evidence,
requirements, axes, and numerical method; incompatible saved designs are
identified and cannot be presented as a valid comparison.

Graphical candidate edits form one bounded undo history for the active loop and
model revision. `Undo`, `Redo`, Ctrl/Cmd+Z, and Ctrl/Cmd+Shift+Z restore the
complete candidate and all linked results atomically. Applying, replacing the
model, changing the loop, or closing the session creates an explicit history
boundary. Undo and redo MUST never alter the running controller or cross that
boundary.

### 5.7 Plot empty, stale, and failure states

An empty plot MUST retain its axes and toolbar and contain exactly one concise
explanation of what is missing and one direct next action that produces the
required evidence. Examples include `Start the simulator to receive live
samples`, `Record a safe experiment`, and `Accept a model before opening
Bode`. It MUST NOT show fabricated demonstration curves, multiple competing
actions, or decorative empty-state graphics.

On stale or disconnected data, the last trace remains visible as historical,
the cursor remains usable, and a clear stale boundary and timestamp are shown.
New analysis and experiments are blocked while inspection and export remain
available.

## 6. Monitor

Monitor displays the current loop without changing it.

It MUST show:

- synchronized SP, PV, and MV values and trends;
- units, quality, freshness, and connection state;
- automatic, manual, or tracking state;
- output limiting or saturation;
- active controller revision; and
- active simulated-plant fault or effect when runtime evidence provides it.

Runtime point quality is retained as `good`, `forced`, `uncertain`, `stale`, or
`bad`; it is never silently promoted to good. A forced primary signal remains
visible but blocks excitation. Uncertain, stale, or bad SP, PV, or MV blocks a
new experiment and model fit while keeping the last evidence visibly marked.

The user MUST be able to start and stop a bounded recording. A recording MUST
retain synchronized samples, timing, units, quality, source, and the selected
loop revision.

Control Studio MAY show concise response error, saturation time, oscillation,
noise, and timing summaries. A diagnostic MUST identify its evidence window
and MUST use non-causal wording unless the cause is directly known.

## 7. Test

Test creates a bounded excitation and recording for identification or response
evaluation.

The Test page supports exactly observation-only recording, setpoint step, load
step, MV step, MV pulse, relay, PRBS, and multisine excitation. MV step remains
wire-compatible with the earlier `output_step` name. A load step targets one
declared auxiliary `disturbance` write binding; Test never guesses one from a
symbol name. MV step/pulse, relay, PRBS, and multisine require confirmed Manual
mode; setpoint and load steps require Auto.

The normal form asks only for amplitude and maximum duration. Observation uses
zero amplitude. Sequence timing is derived from the positive observed sample
period: PRBS dwell is `max(10*sample_period, 500 ms)`, while multisine samples
at `max(sample_period, 50 ms)` and uses the first three integer harmonics of
`1/duration`. PRBS uses an explicitly retained nonzero 32-bit seed. Generated
values, including both PRBS levels and every multisine point, MUST remain
inside the affected signal range. The preview retains kind, affected signal,
current/restoration value, requested bounds, duration, interval, seed or
frequencies when applicable, complete bounded trajectory, stop conditions, and
restoration action.

Maximum duration MUST be between 1 and 3600 seconds. This bounded one-hour
ceiling accommodates the curriculum's slow thermal experiments while retaining
an unconditional runtime timeout and the always-available abort and restoration
path. A lesson duration outside this interval is invalid rather than silently
clamped.

Relay is feedback-dependent and MUST NOT be implemented as a precomputed,
time-driven square-wave trajectory. The relay plan retains the current process
value as its center, the process-value binding and unit, the current manipulated
variable as its bias and restoration value, symmetric high and low manipulated-
variable levels, and a positive process-value hysteresis half-width. The normal
form continues to ask only for manipulated-variable amplitude and maximum
duration. The initial hysteresis half-width is derived as the larger of three
process-value resolution increments and 0.5 percent of the declared process-
value range; Advanced settings MAY allow an explicit larger value but MUST NOT
allow a value below that noise-protection floor.

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

## 8. Identify

Identify creates a low-order plant model from a recording or imported data.

Before fitting, it MUST show the chosen input and output, units, sample time,
gaps, clipping, saturation, drift, excitation, and whether the data is suitable.
The user MAY exclude an interval without deleting the raw samples.

The qualification summary MUST retain raw and usable sample counts, retained
duration, median sample interval, maximum interval deviation, detected gaps,
rejected-quality samples, explicitly reported clipping and saturation samples,
baseline drift, input and output span, a time-domain signal-to-noise estimate,
the excitation class, and independent-validation availability. Coherence MUST
be shown as `Not evaluated for time-domain excitation` for observation and step
records; it MUST never be invented from a step response. Suitability for a
same-run preview and qualification for controller design are separate states.

The first version supports these model structures:

- static gain;
- integrating process;
- first-order process;
- first-order plus dead time (FOPDT);
- second-order process; and
- second-order plus dead time (SOPDT).

For simulation, the configured model MAY be selected directly but MUST be
labelled `Exact simulation model`. A fitted model remains labelled
`Identified from simulation data`.

Each fitted model MUST show parameters with units, measured versus predicted
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

PRBS generation MUST use a declared register order, nonzero seed, amplitude,
clock interval, and sequence length. Multisine generation MUST retain every
angular frequency, amplitude, phase, sample interval, applied common scale,
and peak bound, and MUST reject tones at or above Nyquist. Frequency-response
estimation MUST retain the input/output spectra, complex response, magnitude,
phase, segmentation, and magnitude-squared coherence at every requested tone.
A low-coherence tone is retained but disqualifies the response from transfer-
function fitting. A fitted transfer function MUST be scored on different
retained tones or a separate excitation run; the fitting tones alone are not
validation evidence.

If the data cannot identify an acceptable model, Identify MUST stop with a
specific reason and retain the data.

## 9. Tune

Tune creates a controller candidate. Opening Tune or changing a tuning control
MUST NOT modify the running controller.

The default view MUST show the selected model, current controller, one
recommended candidate, a `More robust` to `Faster` control, predicted response,
control effort, stability margins, and the reason for the recommendation.

The first version supports:

- automatic recommendation;
- SIMC;
- AMIGO where applicable;
- manual `Control.PID` settings; and
- imported `Control.PID` settings.

Every method MUST state when it is applicable and why it is unavailable.
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

Feasible candidates are ranked using the selected response objective and the
same model-predicted scenarios. The score MUST include tracking error,
time-weighted tracking error, controller-output movement, peak and rate-limit
use, noise-driven movement, stability margins, and bounded model variation. It
MUST NOT optimize IAE alone. A candidate replaces the current deterministic
choice only when its score is at least `10 percent` lower; smaller differences
are not treated as materially distinguishable model predictions. Within that
equivalence band, resolve the choice by lower controller complexity, then
`SIMC`, then `AMIGO`, then the stable candidate identifier. The selected
candidate MUST appear first while every raw score remains visible.

The first constrained comparison is a deterministic bounded neighborhood around
the qualified SIMC candidate. It evaluates the formula PI, a less aggressive PI
with `0.8 Kp` and `1.25 Ti`, P by removing integral action, PD by removing
integral action and adding
`Kd = Kp min(max(L, 0.05 T), 0.25 T)`, and PID by adding the same derivative
term to the formula PI. Every non-formula candidate is labelled `Constrained
comparison`, not SIMC or AMIGO. All candidates use the same validation and
ranking function. The comparison MUST produce no candidate when none passes
mandatory requirements.

For an accepted integrating model, proportional control MUST remain eligible
because the plant already supplies integral action. PI, PD, and PID remain
eligible only when applicable and must justify their additional term through a
material benefit. For other first-version formula domains, unsupported P, PD,
or PID candidates are reported as unavailable rather than synthesized from an
unqualified rule.

The compact Tune view shows Method and Response plus the winning structure,
method, limiting requirement, and concise reason. Exact formulas, `lambda`,
scores, rejected candidates, conventions, and numerical values remain under
advanced disclosure. Every generated candidate remains unapplied.

Formula authority:

- S. Skogestad, [Simple analytic rules for model reduction and PID controller
  tuning](https://doi.org/10.1016/S0959-1524(02)00062-8), *Journal of Process
  Control* 13 (2003), 291–309; and
- K. J. Åström and T. Hägglund, [Revisiting the Ziegler-Nichols step response
  method for PID control](https://doi.org/10.1016/j.jprocont.2004.01.002),
  *Journal of Process Control* 14 (2004), 635–650.

Automatic recommendation MUST compare the permitted P, PI, PD, and PID structures.
It MUST choose the simplest candidate that meets the declared requirements. A
more complex controller may be selected only when it provides a material
benefit without unacceptable noise, actuator activity, or robustness loss.

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

## 11. Automatic Tune

Automatic Tune sequences the existing Test, Identify, Tune, and Validate
functions. It MUST NOT implement separate versions of their algorithms.

Before starting, it MUST show one concise plan containing:

- whether existing data can be reused;
- the proposed bounded test when new data is required;
- model structures to evaluate;
- permitted controller structures and tuning methods;
- validation requirements; and
- whether the final candidate may be applied or only exported.

For simulation, the complete bounded plan may be confirmed once. For a
physical plant, explicit approval is required immediately before each physical
test. Automatic Tune MUST NOT silently enlarge or repeat a physical test.

Automatic Tune MUST:

1. qualify or collect data;
2. select the simplest acceptable model;
3. generate applicable PI, PD, and PID candidates;
4. validate the candidates;
5. recommend the simplest passing candidate; and
6. present the result for review.

If a stage fails, it MUST stop at that stage, retain valid work, and explain the
nearest recovery action. If no controller passes, the result MUST be
`No acceptable controller found`; mandatory requirements MUST NOT be weakened.

Completion creates a candidate only. Automatic Tune MUST never silently apply
controller settings.

## 12. Apply and rollback

Applying a candidate requires a separate explicit review showing current and
candidate settings, source, validation result, units, limits, and warnings.

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

After apply, Control Studio MUST keep SP, PV, MV, limits, and `Rollback`
visible during a 30-second bounded observation period beginning only after
confirmed read-back. The application state MUST show that duration. Rollback
MUST restore the exact
previous confirmed controller revision or report why restoration failed.

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

The result selector MUST list saved revisions for the active project and loop.
Opening one revision restores its recording and engineering result without
changing the running controller. Comparing two revisions MUST use their saved
data and MUST identify source, controller revision, method, and creation time.
An empty result history is a normal initial state, not a failed operation. The
Validate pane MUST retain its neutral empty-state explanation until a result
operation actually fails, and a failure MUST show the specific returned reason.

`Save session` MUST store the complete analysis workspace with the result:
active workflow stage, selected direct plot for every stage, selected signals
and channel order, compatible axis and frequency-unit choices, cursor and zoom
windows, expanded state, active candidate, pinned designs, declared graphical
requirements, and the exact model, controller, evidence, and calculation
revisions behind them. Reopening the session MUST restore that state without
starting a runtime, changing the running controller, or applying the candidate.
If live evidence has advanced since the save, the restored session remains
visibly historical until the user explicitly returns to the current session.

In this specification, a saved plot layout means the directly selected stage
plots, signal lanes, channel ordering, axis choices, and inspection windows. It
does not introduce draggable dashboards, user-created panels, a plot catalogue,
or separately configurable learner and professional layouts. Missing or
incompatible session state MUST fall back to the nearest valid direct plot,
retain all compatible state, and explain exactly what could not be restored.

The user MUST be able to export the complete typed workflow result as JSON and
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

Control Studio MUST use the existing shared truST VS Code theme, typography,
controls, icons, and interaction patterns. It MUST NOT introduce a private
palette, remote font, ornamental dashboard, or duplicate runtime controls.

Layout MUST use the shared 4-pixel base spacing system: 4 pixels for tightly
related control internals and 8-pixel multiples for gaps, padding, and section
rhythm. Space and thin one-pixel VS Code separators establish hierarchy.
Decorative cards, nested bordered containers, shadows, glow, gradients, and
large ornamental headers MUST NOT compete with the engineering plot.

Plot typography MUST use VS Code's editor and interface font variables with
tabular numerals for measured values. Axis labels and legends MUST remain
legible but visually secondary to the data. Overlapping tick labels MUST be
suppressed and tick density MUST decrease at narrow widths. Grid lines MUST be
subtle, axis lines MUST not be repeated when grid lines already establish the
scale, and primary traces MUST remain the strongest marks in the plot.
Internal scroll regions use the platform's thin scrollbar treatment derived
from VS Code scrollbar variables; they MUST NOT introduce a bright or oversized
browser-default scrollbar. Toolbar groups use spacing and one-pixel separators,
not cards or extra menus, to distinguish plot selection from plot actions.

Plot colours MUST come from the shared VS Code-derived truST palette and remain
stable for the same signal role and candidate identity across time-domain,
frequency-domain, stability, MIMO, comparison, export, and restored-session
views. Primary measured SP, PV, and MV traces use two-pixel strokes. Secondary
signals, identified models, predictions, and comparison traces use visually
lighter strokes or bands while retaining required contrast. Requirement bands
and nominal-versus-uncertainty envelopes MUST be subtle, labelled, and drawn
behind the primary traces. Point markers remain absent on dense traces until
the user hovers, places the cursor, or zooms far enough for individual samples
to be meaningful.

Nominal predictions MUST use a distinct line inside their uncertainty band.
The band MUST state the uncertainty definition, confidence or bound level when
defined, model set or variation source, and candidate revision. Colour alone
MUST NOT distinguish nominal, lower bound, upper bound, measured response, or
an unqualified extrapolation. Missing uncertainty remains explicitly
`Not evaluated`; it MUST NOT be represented as a zero-width band.

Cursor movement and candidate-result updates MAY use a 120-to-160 millisecond
transition when it improves continuity without delaying values or safety
state. Other decoration MUST NOT animate. All nonessential transitions are
disabled when VS Code or the operating system requests reduced motion. Linked
plots MUST update their cursor smoothly from the same exact sample or
frequency; animation MUST NOT create different displayed values between plots.

The main plot MUST remain visually dominant. The page MUST remain usable at
wide, split, narrow, and 200 percent zoom layouts. At narrow widths, identity,
navigation, live monitor, active task, and result MUST appear in that order.
Abort, limits, apply state, and rollback MUST remain visible when relevant.

At wide widths the plot toolbar remains on one line and the active-stage form
and result MAY use two columns. At split widths the toolbar may wrap into two
compact rows, but the active plot tabs, `Fit`, and `Expand` remain visible
without opening an overflow menu. At narrow widths the workflow navigation and
plot controls wrap into compact rows so every plot and chart action remains
directly visible. The live-value strip wraps, analytical multi-panel plots
stack, and the flat signal list uses the available editor width. The primary
plot MUST remain at least 260 pixels high. At 200 percent zoom, controls MUST
wrap without horizontal scrolling and MUST NOT overlap the plot, values, or
one another.

Expanded plot mode uses the full Control Studio editor below the fixed header
and workflow navigation. It MUST preserve VS Code escape behavior, keyboard
focus, plot selection, signal selection, cursor position, zoom window, live
values, source, quality, limits, and an active Abort action.

All controls MUST be keyboard operable with visible focus. Charts MUST expose
an accessible name, summary, selected values, and tabular alternative. The
completed surface MUST remain understandable in the shared dark, light, and
high-contrast themes without relying on colour alone.

Analysis MUST run outside the webview UI thread and remain bounded. If an
analysis operation becomes long-running, it MUST expose cancellation before it
ships. The UI MUST remain responsive, and live display data MUST remain bounded.

## 16. Acceptance

Acceptance requires executable tests for numerical results and runtime state
changes, registered VS Code extension tests for visible behavior, and rendered
verification of the complete workflows in the shipped extension.

The minimum rendered journeys are:

1. open and monitor a simulated loop;
2. test, identify, tune, validate, and complete Automatic Tune in simulation;
3. approve and abort a bounded physical test without silently applying gains;
4. recover from invalid configuration, stale data, failed identification,
   failed tuning, disconnect, and restoration failure; and
5. complete the supported simulation workflow with keyboard navigation at
   wide, narrow, dark, light, and high-contrast configurations;
6. discover unavailable advanced plots, satisfy their prerequisites, and
   inspect Bode, Nyquist, root-locus or pole-zero, sensitivity, and validation
   evidence without losing the live monitor; and
7. select and compare multiple channels from a multivariable loop, inspect its
   channel matrix, RGA and singular values, and verify unit-safe aligned lanes
   at wide and narrow widths; and
8. tune one candidate graphically from the direct Bode, root-locus, and Nichols
   tabs, observe linked response, margin, requirement, and validation updates,
   pin and compare designs, undo and redo an edit, save the session, reopen it,
   and recover the same evidence-bound analysis workspace without changing the
   running controller.

Rendered acceptance MUST reject a chart with mismatched units or ranges,
unlabelled dual axes, clipped controls, unexplained status dots, hidden
essential plot controls, arbitrary future-axis padding, unreadable hashes,
colour-only identity, or a plot that is unnecessarily small while unused
editor space remains.

Rendered acceptance MUST capture every target layout defined above and MUST
also prove direct graphical tuning, linked candidate updates, graphical
requirements, pinned comparison, save and restore, uncertainty envelopes, and
undo and redo. The accepted screenshots MUST show real rendered state from the
shipped extension; mockups, static source assertions, and stale captures do not
close these requirements.

Unavailable physical hardware remains an explicit external boundary and MUST
NOT be replaced by simulation evidence.
