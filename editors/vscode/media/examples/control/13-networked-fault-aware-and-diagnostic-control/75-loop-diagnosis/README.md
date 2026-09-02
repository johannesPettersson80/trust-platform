# Diagnose a steam-flow loop before retuning

Course position: Lesson 75 of 84. Prerequisite: multirate and fault-aware pumping control. Next lesson: cold-storage capstone.

This lesson uses deterministic **Simulation evidence**, not physical-plant evidence.

**Lesson objective:** Diagnose eight blinded steam-loop records and select the next evidence-producing action without treating retuning as the default remedy.

## The problem

The equipment in this lesson is Industrial steam-distribution flow loop.

A modulating steam valve supplies one production user. The steam-flow setpoint
and both flow measurements use tonnes per hour (`t/h`). Valve demand and
measured valve-stem position use percent. Upstream header pressure uses bar.
The adjacent production user's valve demand also uses percent.

The complete record contains:

- the desired steam flow, called the setpoint ($SP$);
- the primary measured steam flow, called the process value ($PV$);
- an independently maintained reference flow meter;
- the controller's requested manipulated variable ($MV_{\mathrm{requested}}$);
- the measured, physically applied valve-stem position
  ($MV_{\mathrm{applied}}$);
- upstream header pressure and adjacent-user valve demand;
- flow quality, controller mode, output limits, operating region, schedule
  transitions, timestamps, and measured controller interval.

The nominal local steam path has a gain of `1.0 t/h/%`, a time constant of
`5 s`, and a dead time of `1 s`. The controller runs every `100 ms`. The
baseline proportional-integral controller has proportional gain
`2.0 %/(t/h)` and integral gain `0.2 %/(t/h*s)`. The valve is one-sided and
limited from `0%` closed to `100%` open.

![Steam-flow loop diagnosis signal path](figures/loop-diagnosis-signal-path.svg)
The diagram shows why one process-value curve is insufficient. A cause can
enter through the controller, valve, instrument, header, adjacent user, or
local plant model.

Lesson 74 used dedicated redundant signals to detect named pumping faults. In
a commissioned process area, several faults can instead produce a similar
oscillation or offset. Retuning from the process value alone can conceal a
sticking valve, tune around a drifting transmitter, request capacity the valve
cannot deliver, or amplify interaction with another steam user.

Therefore the first question is not “Which gain should I change?” It is “Which
signal changed first, and what bounded test can reject the strongest competing
explanation?”

## What we need

We need to match each abnormal steam-flow record to the next evidence-producing action instead of treating retuning as the default cure. Valve, instrument, capacity, disturbance, interaction, model, and schedule causes must remain separate until the record supports one.

## The new idea

### Establish a comparable baseline

A **performance baseline** is a qualified healthy record from the same
operating region, setpoint range, controller revision, actuator limits, sample
interval, and evidence class. It establishes normal variability and timing. It
does not define normal operation forever.

An **event** is a bounded change with a declared source, onset, and duration.
Event detection marks departure from the baseline band. Synchronized samples
let you compare the order of cause, controller reaction, actuator motion, and
process response.

### Name the controller signals precisely

The process error ($e$) is the steam-flow setpoint ($SP$) minus the primary steam
flow process value ($PV$):

$e_{k} = SP_{k} - PV_{k}$

The requested manipulated variable ($MV_{\mathrm{requested}}$) is the valve demand
calculated by the controller. The applied manipulated variable ($MV_{\mathrm{applied}}$) is
the measured valve-stem position. These can differ because a real valve has
limits, dynamics, friction, or a fault.

### Measure degradation without pretending it identifies the cause

Integrated absolute error describes the accumulated magnitude of flow error.
It uses the controller sample interval ($T$), has units of `t/h*s`, and is
calculated over a declared eligible window:

$IAE = \sum\left(|SP_{k} - PV_{k}| \cdot T\right)$

IAE ranks flow performance. It cannot tell whether poor performance came from
tuning, a valve, a transmitter, unavailable capacity, or an external event.

### Distinguish the causal signatures

An **oscillation** is a repeatable alternating component, not merely a noisy
trace. The oscillation period ($P_{\mathrm{osc}}$) is the time between equivalent
consecutive peaks. The cross-correlation lag between requested valve demand
and steam flow estimates which signal consistently leads.

Poor tuning is eligible only when requested valve movement comes first, the
measured stem follows, no limit is active, an external signal does not lead,
measurement quality is good, and the response repeats at the same operating
condition. Oscillation alone does not prove poor tuning.

**Valve stiction** is static friction that holds measured stem position while
requested demand continues to move, followed by a breakaway jump. Held-demand
travel ($TV_{\mathrm{hold}}$) is the cumulative requested-demand movement while measured
stem movement stays within `0.2` percentage points. Stem jump ($\Delta_{\mathrm{stem}}$) is
the applied-position change at breakaway. This lesson requires at least three
cycles with $TV_{\mathrm{hold}}$ greater than `2` percentage points and $\Delta_{\mathrm{stem}}$ greater
than `1` percentage point. Without stem feedback, stiction and poor tuning can
remain **Inconclusive**.

A transmitter can have a constant bias or a changing drift. Flow disagreement
($d_{\mathrm{flow}}$) is primary flow minus the independently measured reference flow:

$d_{\mathrm{flow},k} = PV_{k} - PV_{\mathrm{reference},k}$

The **drift slope** is the change in flow disagreement divided by elapsed time,
in `t/h/s`. A slope above `0.01 t/h/s`, with a qualified reference and
consistent valve behavior, supports instrument verification rather than
retuning.

**Limit occupancy** is the percentage of eligible samples at the lower or
upper applied-valve limit. The declared saturation window runs from `20.0 s`
through `59.9 s`, after constraint entry. If positive flow error persists while
the applied stem remains at `100 percent` for more than `90 percent` of that window,
the plant lacks authority. Increasing gain cannot create valve or steam
capacity.

A measured upstream disturbance must precede the controlled-flow change and
controller response. The required order is $header -> flow -> valve response$;
precedence matters because correlation alone is not causation.

Interaction is supported when another loop acts before the shared process and
local controller react. At least three repetitions of
$other MV -> header/flow -> local MV$ justify an interaction study. They do not
by themselves prove which decoupler should be used.

A model residual ($\varepsilon_{\mathrm{model}}$) is measured flow minus independently
predicted flow. A persistent residual beginning at an operating-region
transition, while sensors and valve remain eligible, supports model or
schedule validation. A schedule-aligned mismatch is not evidence for changing
feedback gains.

A **diagnostic hypothesis** names a possible cause. A **discriminating test**
is a bounded next action whose outcomes differ between the two leading
hypotheses. When signatures overlap, conflict, or lack a required signal, the
result is `Inconclusive`; the system must not invent confidence.

## Prediction

Before opening the answer key, write down the expected leading evidence and
next action for all eight records:

| Candidate | Evidence that must lead or persist | Evidence-qualified next action |
|---|---|---|
| Healthy baseline | Eligible quality, stem tracking, no sustained limit or unexplained event | Continue monitoring |
| Poor controller tuning | Requested valve first; stem follows; no limit or external leader | Controlled tuning comparison |
| Valve stiction | Three demand-ramp, stem-hold, breakaway cycles | Valve inspection or reviewed travel test |
| Transmitter drift | Primary/reference disagreement slope above `0.01 t/h/s` | Instrument verification |
| Saturation | Applied stem at `100%` for over `90%` of the event while error persists | Capacity and operating-demand review |
| Upstream disturbance | Header pressure changes before flow and local valve response | Disturbance-source investigation |
| Interaction | Adjacent valve leads shared header/flow and local valve on three events | Interaction test and coordination review |
| Model/schedule mismatch | Residual begins at an operating-region transition | Revalidate model and schedule |

These are casebook boundaries, not universal diagnostic thresholds.

## Implementation

The actual Structured Text configures one baseline controller. It exposes the
diagnostic signals but contains no scenario label, diagnosis, remedy, or
automatic gain change:

```st
Parameters.ProportionalGain := ConfiguredProportionalGain;
Parameters.IntegralGain := ConfiguredIntegralGain;
Parameters.Bias := LREAL#50.0;
Controller.Configure(Parameters := Parameters);

Input.Setpoint := Setpoint;
Input.ProcessValue := ProcessValue;
Input.CycleTime := MeasuredCycleTime;
ValveRequested := Controller.Update(Input := Input);
```

`ValveRequested` is $MV_{\mathrm{requested}}$. `ValveStemPosition` is the separately
measured $MV_{\mathrm{applied}}$. The simulator creates the faults; hidden controller
branches do not manufacture the verdict.

## Experiment in Control Studio

This procedure tests one declared lesson change under controlled conditions. Open each degraded record under the same displayed identity and classify its causal signature without reading the sealed scenario label. The declared event lasts 60 seconds. It restores Primary steam flow to 50 t/h afterward. The deterministic Simulation seed is 20260829.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait until the header shows **Simulation**, **Running**, and current data. Confirm Steam-flow setpoint (SP, t/h); Primary steam flow (PV, t/h) and Reference steam flow (PV, t/h); Requested valve demand (MV, %) and Applied valve-stem position (MV, %), including units, limits, mode, and quality. The initial operating point is Steam-flow setpoint = 50 t/h, Primary steam flow = 50 t/h, Reference steam flow = 50 t/h, Requested valve demand = 50 %, Applied valve-stem position = 50 %, Upstream header pressure = 8 bar, Adjacent-user valve demand = 0 %, Operating region = 0 (dimensionless). The unresolved baseline problem is: A process-value oscillation or offset is a symptom shared by different controller, actuator, measurement, capacity, disturbance, interaction, and model causes. The industrial steam-distribution area record must show steam-flow setpoint in t/h and valve-stem position in percent before diagnosis.
   - **Decision before action:** Read **Prediction** above, then state what should move first and in which direction. Use this decision rule: Use synchronized causal signatures and a discriminating test to select the responsible engineering action; report Inconclusive when required evidence is absent.
   - **Successful evidence:** The Response plot advances, the displayed values are finite and current, and **Start recording** is enabled. Select **Start recording** only after those conditions are visible.
   - **Recovery:** If recording is disabled or the plot reports unavailable or stale samples, stop the Simulator, correct the displayed runtime or binding problem, restart it, and repeat the baseline check.
   - **Record:** Write down the initial SP, PV, requested/applied MV where available, controller mode, and your predicted response before opening Test.

2. **Test** — Open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Set **Experiment** = **Observe without excitation**, **Amplitude** = **0 t/h**, and **Maximum duration (s)** = **60**. The affected signal is **Primary steam flow**; it starts at 50 t/h and must restore to 50 t/h.
   - **Decision before action:** Before selecting **Review test**, predict the response direction and confirm the requested change remains inside Primary steam flow 0 t/h to 120 t/h; Reference steam flow 0 t/h to 120 t/h; Requested valve demand 0 % to 100 %; Applied valve-stem position 0 % to 100 %; Upstream header pressure 0 bar to 16 bar.
   - **Successful evidence:** Select **Review test**, verify the signal, requested bound, duration, restoration, and stop conditions, then select **Confirm and start**. The state changes to **Running**, **Abort test** remains available, and the terminal state confirms completion and restoration while Response shows the measured SP/PV/MV sequence.
   - **Recovery:** For a rejected, stale, unsafe, unavailable, failed, or aborted state, read the displayed reason, restore Primary steam flow to 50 t/h, wait for current data, and review a new test.
   - **Record:** Compare the response with your prediction and retain the terminal state, restoration result, and deterministic seed **20260829**.

3. **Identify** — After **Stop recording**, open **truST sidebar → Control Studio → Identify**.
   - **Baseline check:** Use **Qualify recording** to inspect sample count, duration, timing, excitation, and clipping. **Model selection**, **Fit estimation run**, and **Accept model for design** are the shared low-order model workflow; they do not replace the project-owned **Blinded evidence-based loop diagnosis** model and must not be accepted as this lesson's design authority.
   - **Decision before action:** Decide whether the recording contains the input/output change needed for **Blinded evidence-based loop diagnosis** before fitting anything.
   - **Successful evidence:** A qualified recording has current, finite, sufficiently excited data. Use the project-owned model described under **Implementation** and **Evidence and decision**, and record its assumptions, units, validity range, and rejection rule.
   - **Recovery:** If qualification reports stale, clipped, poorly excited, or insufficient data, return to Test and repeat the bounded experiment; never force or relabel a rejected fit.
   - **Record:** State which model is retained or rejected and why, explicitly identifying the runnable project as the model authority.

4. **Tune / Design** — Open **truST sidebar → Control Studio → Tune**.
   - **Baseline check:** **Design method** and **Generate candidate** are disabled because **Blinded evidence-based loop diagnosis** is designed by the runnable project; shared Control.PID tuning is unavailable. Do not select an unrelated PID method.
   - **Decision before action:** Apply the equations and choices under **Implementation**, then compare the declared alternatives under **Evidence and decision** before accepting **Blinded evidence-based loop diagnosis**.
   - **Successful evidence:** Use **Monitor → Signals** to correlate the implemented project-owned design with its named live states and outputs. A disabled shared designer is only a visible ownership boundary, not successful design evidence.
   - **Recovery:** If the project-owned prerequisites or qualification flags are false, return to Test or Identify as directed by the lesson and do not bypass them with a generic PID candidate.
   - **Record:** State why the project-owned design is eligible, conditional, or rejected, including the decisive trade-off.

5. **Validate** — Open **truST sidebar → Control Studio → Validate**.
   - **Baseline check:** **Run required validation** is disabled because no shared Control.PID candidate exists. **Requirement details** and **Save result** cannot be used to claim a shared result; project-owned validation is executed by the runnable project's declared oracle.
   - **Decision before action:** Use the predeclared comparisons: correct_diagnosis_count == 8 (dimensionless) (tolerance 0 (dimensionless)); incorrect_diagnosis_count == 0 (dimensionless) (tolerance 0 (dimensionless)); correct_remedy_count == 8 (dimensionless) (tolerance 0 (dimensionless)); stiction_held_demand_travel > 2 pp (tolerance 0.1 pp).
   - **Successful evidence:** Correlate the unrounded project-owned results with the named live signals and events in Monitor. Each applicable requirement must have a value, unit, limit, tolerance, evidence class, and Pass, Conditional pass, Fail, or Inconclusive disposition.
   - **Recovery:** If the project run aborts, becomes stale, fails, or violates restoration, discard that run and repeat the declared Test. Never present a disabled button or retained README table as newly executed evidence.
   - **Record:** State the validation disposition and name every failed or inconclusive requirement before continuing.

## Evidence and decision

### Measures and comparison

Define every measure before reading its value:

- **Correct diagnosis count** is the number of complete records whose supported
  diagnosis matches the sealed answer. Higher is better.
- **Incorrect diagnosis count** counts unsupported classifications. Zero is
  required.
- **Justified inconclusive count** counts records deliberately withheld because
  required evidence is missing. It is reported separately, not treated as an
  error or silently forced into a label.
- **Remedy-selection accuracy** is the fraction whose next action matches the
  evidence-qualified owner and action. Retuning is not the default.
- Event precedence uses seconds with one-sample (`0.1 s`) uncertainty.
- $P_{\mathrm{osc}}$ uses seconds; $TV_{\mathrm{hold}}$ and $\Delta_{\mathrm{stem}}$ use percentage points; drift slope
  uses `t/h/s`; model-residual mean uses `t/h`; limit occupancy uses percent;
  and IAE uses `t/h*s`.

The source signals, window, sample interval, validity prerequisite, and desired
direction must remain in the exported artifact. Do not rank a value from an
ineligible record.

### Validation

The native executable test compiles the shipped Structured Text with the
shared Control library and executes the same controller against all eight
deterministic cases. Its independent oracle does not read the sealed label
until after classification. Acceptance requires eight correct diagnoses,
eight correct next actions, zero incorrect diagnoses, no automatic gain
change, and an `Inconclusive` result when stem feedback is deliberately
removed from the tuning-versus-stiction record.

The learner-facing result table and plots may be populated only from retained
run artifacts. Simulation results must never be labelled as measurements from
a physical steam plant.

![Valve-stiction and sensor-drift fingerprints built from independent diagnostic signals](figures/loop-diagnosis-evidence.svg)

[Complete trace](figures/loop-diagnosis-evidence-trace.csv) · [Decision summary](figures/loop-diagnosis-evidence-summary.csv)

## Conclusion

Explain each decision as a causal sequence: external event, measured process
response, controller request, actual valve motion, and recovery. Then name the
signal that rejects the strongest alternative.

- A pinned valve is an authority problem, not proof that gain is too small.
- Primary/reference divergence is an instrument problem until proven otherwise.
- Stem hold and breakaway distinguish stiction from controller-only cycling.
- External precedence distinguishes a disturbance or interaction from a local
  controller initiating the response.
- A residual aligned with a schedule transition requires model validation.
- Only controller-led repetition with eligible valve, measurement, limits, and
  disturbances supports a controlled retuning experiment.

## Reason for the next lesson

You can now decide whether evidence belongs to controls tuning, valve
maintenance, instrumentation, equipment capacity, process operations,
interaction engineering, model/schedule ownership, or further data
acquisition. These eight deterministic cases do not prove universal diagnostic
coverage or physical-plant health.

Lesson 76 starts the capstones. You will receive a real control problem and
must select and combine previously learned methods without being told the
controller or remedy in advance.
