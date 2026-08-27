# VS Code Control Studio

Status: normative truST product specification.

Control Studio is the VS Code workspace for observing, testing, identifying,
tuning, and validating one `Control.PID` feedback loop. It uses the same
workflow for a simulated plant and a physical plant while always identifying
which source produced the evidence.

The PID algorithm is defined by
[`32-control-pid.md`](32-control-pid.md). Simulated plant behavior is defined by
[`33-simulation-plant-models.md`](33-simulation-plant-models.md), and simulated
plant configuration is defined by
[`34-vscode-plant-simulation-ui.md`](34-vscode-plant-simulation-ui.md).

The words MUST, MUST NOT, SHOULD, and MAY are normative.

## 1. Supported control loop

The first supported system is one SISO loop using `Control.PID`.

A loop definition MUST identify:

- the project and runtime;
- the controller instance;
- setpoint (SP), process variable (PV), and manipulated variable (MV);
- the engineering unit and expected range of each signal;
- direct or reverse action;
- controller mode;
- output and rate limits when configured;
- task period and observed sample time; and
- the simulated plant when the loop uses one.

Control Studio MUST reject incompatible, missing, ambiguous, or cross-runtime
bindings. It MUST NOT guess a signal from its name.

The first version does not support MIMO, cascade, ratio, split-range,
gain-scheduled, or adaptive controllers.

## 2. Opening Control Studio

`Open Control Studio` MUST be available from:

- a configured simulated plant;
- a `Control.PID` instance or configured loop; and
- the command palette.

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

## 6. Monitor

Monitor displays the current loop without changing it.

It MUST show:

- synchronized SP, PV, and MV values and trends;
- units, quality, freshness, and connection state;
- automatic, manual, or tracking state;
- output limiting or saturation;
- active controller revision; and
- active simulated-plant fault or effect when runtime evidence provides it.

The user MUST be able to start and stop a bounded recording. A recording MUST
retain synchronized samples, timing, units, quality, source, and the selected
loop revision.

Control Studio MAY show concise response error, saturation time, oscillation,
noise, and timing summaries. A diagnostic MUST identify its evidence window
and MUST use non-causal wording unless the cause is directly known.

## 7. Test

Test creates a bounded excitation and recording for identification or response
evaluation.

The first version supports:

- observation-only recording;
- a setpoint step; and
- an output step when the runtime and controller mode permit it.

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

The result MUST state whether the test completed, aborted, failed, or failed to
restore. It MUST retain the samples and the exact stop reason.

## 8. Identify

Identify creates a low-order plant model from a recording or imported data.

Before fitting, it MUST show the chosen input and output, units, sample time,
gaps, clipping, saturation, drift, excitation, and whether the data is suitable.
The user MAY exclude an interval without deleting the raw samples.

The first version supports these model structures:

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

The response control selects the declared closed-loop time `lambda`:

- `Faster`: `lambda = max(L, T / 3)`;
- `Balanced`: twice the Faster value; and
- `More robust`: four times the Faster value.

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

Both methods use positive plant gain in this version. Zero or negative gain,
nonfinite parameters, nonpositive time constants, negative delay, model error
above `0.25`, unsupported model structure, and method-specific domain failure
MUST return a deterministic reason and MUST NOT create a candidate. Data that
failed qualification because it was noisy, stale, poorly excited, or
insufficient remains an Identify failure and MUST NOT reach Tune.

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
MUST NOT optimize IAE alone. Lower score wins; an exact score tie is resolved by
the lower-complexity controller structure, then `SIMC`, then `AMIGO`, then the
stable candidate identifier.

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
conditions.

Validation MUST cover the applicable subset of:

- setpoint response;
- disturbance response;
- rise time, settling time, overshoot, and steady-state error;
- MV excursion, rate, saturation, and recovery;
- noise response;
- gain and phase margins; and
- bounded model or parameter variation.

Each requirement MUST report `Pass`, `Fail`, `Not evaluated`, or
`Inconclusive`, with the value, limit, unit, source, and reason. Model-based,
simulation-observed, recorded, and physical-observed results MUST remain
distinct.

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

After apply, Control Studio MUST keep SP, PV, MV, limits, and `Rollback`
visible during a bounded observation period. Rollback MUST restore the exact
previous confirmed controller revision or report why restoration failed.

Physical apply is allowed only with the required runtime authority and a
separate explicit approval. Experiment approval does not authorize apply.

## 13. Saved results and export

Control Studio MUST save a versioned run result through its existing workspace
result storage. Each result MUST have a stable result ID, a positive revision,
creation time, project ID, loop ID and revision, controller instance and
revision, evidence source, extension version, and the runtime and numerical
adapter versions that were available when the result was created.

The raw recording MUST retain synchronized SP, PV, and MV values together with
elapsed time, observation time, actual cycle interval, scan number, engineering
units, signal quality, freshness, controller mode, active output and rate
limits, safety state, and event markers. Missing values MUST remain missing;
save, reopen, compare, and export MUST NOT interpolate or invent them.

When applicable, the result MUST also retain:

- the requested and applied experiment, approval source, bounds, stop reason,
  abort state, restoration value, and restoration result;
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

The user MUST be able to export the complete typed workflow result as JSON and
the selected signal table as CSV. CSV MUST include elapsed time, observation
time, actual cycle interval, SP, PV, MV, units, quality, mode, safety state,
limits, source, and event markers with sufficient numeric precision to
recalculate the saved metrics. PNG remains presentation evidence only. Exported
data MUST not contain credentials or runtime secrets.

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

The main plot MUST remain visually dominant. The page MUST remain usable at
wide, split, narrow, and 200 percent zoom layouts. At narrow widths, identity,
navigation, live monitor, active task, and result MUST appear in that order.
Abort, limits, apply state, and rollback MUST remain visible when relevant.

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
   wide, narrow, dark, light, and high-contrast configurations.

Unavailable physical hardware remains an explicit external boundary and MUST
NOT be replaced by simulation evidence.
