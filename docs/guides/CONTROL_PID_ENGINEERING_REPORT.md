# Control System Engineering with Control.PID

This report records what was learned while implementing and evaluating the
paper-compatible truST `Control.PID`, explains the engineering decisions behind
it, and defines a practical route from a simulated plant to a well-specified,
tuned, validated, and safely commissioned control loop.

It also turns every completed benchmark into a proposed training exercise. The
same material is intended to help a learner understand control theory and help
an experienced engineer inspect the assumptions, limits, and evidence behind a
controller candidate.

## Executive conclusion

The implemented PID algorithm is a sound baseline. The native Structured Text
controller passed equation conformance, all seven scenarios released with the
paper, and closed-loop tests covering:

- nominal and varied first-order-plus-dead-time processes;
- sampling intervals from `50 ms` through `200 ms`;
- deterministic severe scan jitter from `20 ms` through `180 ms`;
- load disturbances and scheduled process-gain changes;
- actuator saturation, slew limits, lag, and deadband;
- back-calculation and Combined anti-windup;
- noisy measurements with and without a separate low-pass filter;
- integrating, underdamped, delay-dominant, and open-loop unstable plants; and
- two independent PID instances controlling a coupled two-input, two-output
  state-space process.

No result identified an error in the PID equations, retained state, cycle-time
handling, limiting, or anti-windup implementation. The unsuccessful attempts
were valuable because they exposed unsuitable tuning or invalid test setup,
not controller defects.

The most important product conclusion is therefore:

> Keep `Control.PID` small and paper-compatible. Improve the complete control
> engineering workflow around it rather than adding identification, tuning,
> monitoring, scheduling, prediction, or multivariable behavior inside the PID
> function block.

The first improvement should be persistent run artifacts containing complete
SP, PV, MV, time, configuration, model, tuning, limits, events, quality, and
metrics. The second should convert the benchmark catalog into executable
lessons using those artifacts. The third should qualify and compare multiple
tuning methods against the same recorded evidence. That sequence gives learners
understandable experiments, gives professionals and researchers reproducible
engineering evidence, and creates the foundation needed for automatic structure
selection, robust tuning, gain scheduling, and delay compensation.

## 1. What was implemented

### 1.1 Public control library

The public library lives in namespace `Control`. It deliberately exposes one
stateful combined controller and one independent filter:

| Public item | Responsibility |
| --- | --- |
| `Control.PID` | Execute the paper's combined positional and incremental controller. |
| `Control.LowPass` | Filter one signal with one or two retained low-pass stages. |
| `PIDParameters` | Hold one atomic controller configuration. |
| `PIDInitialState` | Define every retained controller history at initialization. |
| `PIDInput` | Carry one immutable sample, including actual elapsed cycle time. |
| `PIDMode` | Select Auto, Manual, or Track behavior. |
| `PIDAntiWindup` | Select BackCalculation or Combined anti-windup. |
| `LowPassParameters` and `FilterOrder` | Configure the independent filter. |

There are no separate P, PI, PD, and PID function blocks. Exact zero integral
gain selects the positional P/PD form; nonzero integral gain selects the
incremental PI/PID form. This follows the paper and avoids four nearly
identical state machines.

### 1.2 Execution boundary

The performance tests execute the public Structured Text function block in the
truST runtime. Controller output crosses an ordinary `%Q` process-image
binding, and the simulated plant returns its measurement through an ordinary
`%I` binding. The PLC program therefore does not know whether the actuator and
sensor are simulated or physical.

This matters. Reimplementing the PID equation in a plotting language could
test a mathematical idea while missing an error in Structured Text semantics,
retained state, direct addressing, runtime I/O, or scan ordering.

### 1.3 Evidence classes

Results shall always retain their evidence class:

| Evidence | What it establishes |
| --- | --- |
| Native equation conformance | The Structured Text implementation matches declared equations and state transitions. |
| Runtime simulation | The executed controller and declared simulated plant behaved as recorded. |
| Model-predicted analysis | A declared mathematical model produced calculated response or stability margins. |
| Recorded physical response | The reviewed physical topology produced the recorded signals. |
| Commissioning acceptance | Authorized site testing satisfied the project's acceptance and safety procedure. |

Simulation evidence shall never be relabelled as physical evidence. A frequency
response calculated from an identified model shall never be described as a
directly measured physical Bode response unless the required physical
experiment was actually performed.

## 2. Why the design looks this way

### One combined controller

The combined block preserves the paper's exact branch and update order while
avoiding duplicated P, PI, PD, and PID implementations. Users still obtain P,
PI, PD, or PID behavior through the gain configuration selected by a tuner or
engineer.

### Stateful behavior belongs in function blocks

PID and filtering retain histories between scans, so function blocks are the
natural IEC 61131-3 abstraction. Configuration and sample values are structured
data because they are passed and reviewed as coherent records.

### Filtering remains separate

Noise treatment is a signal-path decision. Different applications may filter
PV, SP, feedforward, or none of them, potentially at different intervals. A
hidden internal PID filter would obscure phase lag and make the controller
equation harder to qualify. `Control.LowPass` therefore remains explicit and
independently stateful.

### Actual cycle time is an input

Derivative, integration, output-rate limits, anti-windup, and filtering depend
on elapsed time. Supplying the actual interval allows deterministic handling of
scan jitter. The severe-jitter benchmark confirmed that this boundary works
when every actual interval is passed correctly.

### Limits and anti-windup are part of execution

Real actuators have position and rate limits. Ignoring those limits allows the
integrator to accumulate an output that the actuator cannot deliver. The
controller therefore applies absolute and rate limits and offers both
paper-defined anti-windup alternatives.

### Tuning is not part of the PID block

The same correct PID behaved differently on different plants. A pure
integrating plant needed only P control in its benchmark, while the unstable
plant needed PI and the underdamped process benefited from PID. Identification
and tuning consume plant knowledge and performance objectives; execution does
not. They belong in Control Studio and qualified numerical services.

### Multivariable control is composed

The coupled benchmark used two normal PID instances, one per controlled
variable. This proved instance independence and weak-coupling behavior without
creating a giant special PID. Stronger coupling may justify a separate
decoupler or a true multivariable controller.

## 3. Quantitative results

### 3.1 Paper process and implementation features

The nominal paper process was:

```text
P(s) = exp(-0.5 s) / (1 + s)
Kp = 0.667
Ki = 0.667
Kd = 0
sample interval = 0.1 s
```

| Measurement | Result |
| --- | ---: |
| Overshoot | `0 %` |
| Settling time | `3.90 s` |
| Final error ratio | `3.76e-8` |
| Integrated absolute error | `1.39925033` |
| Model-predicted gain margin | `13.4605 dB` |
| Model-predicted phase margin | `70.8919 degrees` |
| Gain crossover | `0.66700 rad/s` |
| Phase crossover | `3.14159 rad/s` |

The margins are comfortably above the benchmark minima of `6 dB` and
`30 degrees`. The nominal response is well damped. Its settling-time and IAE
results are close to the declared benchmark limits, which reflects the chosen
Lambda PI tuning rather than an implementation instability.

The load-disturbance test changed plant output bias by `0.2`. Final error was
`0.00013781`, and IAE over the ten seconds after the event was `0.29951984`.

Back-calculation and Combined anti-windup both recovered from the paper's
saturation scenario. Their final process values after returning the setpoint
to zero were `-0.00002812` and `0.00000007`. A separate rate-limited test never
exceeded its exact `1 unit/s` configured limit.

With deterministic measurement noise, controller-output increment RMS fell
from `0.02009180` to `0.00725404` when a second-order `Control.LowPass` was used.
That is approximately a `63.9 %` reduction in noise-driven actuator movement.
The small cost is additional filtering lag, which must always be included in
validation.

### 3.2 Plant variation and sampling

The same paper tuning passed four bounded FOPDT variations:

| Gain | Time constant | Delay | Overshoot | Settling |
| ---: | ---: | ---: | ---: | ---: |
| `0.8` | `0.8 s` | `0.4 s` | `0 %` | `7.0 s` |
| `1.2` | `1.2 s` | `0.6 s` | `7.5536 %` | `5.0 s` |
| `1.2` | `0.8 s` | `0.6 s` | `3.1423 %` | `4.7 s` |
| `0.8` | `1.2 s` | `0.4 s` | `0.2387 %` | `5.1 s` |

Fixed sampling at `50`, `100`, and `200 ms` produced zero overshoot and
settling times of `3.95`, `3.90`, and `4.00 s`. This is evidence for the
declared range, not permission to use an arbitrary sampling interval.

### 3.3 Representative plant classes

| Plant | Structure and gains | Overshoot | Settling | Final error ratio |
| --- | --- | ---: | ---: | ---: |
| Integrator `1/s` | P, `Kp=0.8` | `0 %` | `4.70 s` | `1.37e-11` |
| Underdamped second order | PID, `Kp=1.5`, `Ki=0.5`, `Kd=1` | `0 %` | `13.00 s` | `0.00044867` |
| Delay-dominant FOPDT | PI, `Kp=Ki=1/6` | `0 %` | `15.80 s` | `2.84e-8` |
| First order plus nonlinear actuator | PI, `Kp=1`, `Ki=0.5` | `0.4785 %` | `5.85 s` | `0.00452217` |

The delay-dominant case had IAE `5.89999991`. It was stable but much slower
than the nominal process. This is the strongest current motivation for later
model-based delay compensation rather than increasingly aggressive PID gains.

The nonlinear actuator included saturation, rising and falling slew limits,
lag, and deadband. Controller output remained inside `[0, 1.2]` and the loop
remained stable. Its IAE of `3.22629496` shows the performance cost of actuator
constraints even when the controller handles them correctly.

### 3.4 Advanced conditions

| Condition | Result |
| --- | --- |
| Open-loop unstable `1/(s-0.2)` | `13.4915 %` overshoot, `6.55 s` settling, final error `1.22e-6` |
| Scheduled gain change `0.6 -> 1.4` | Recovery in `5.0 s`, post-change IAE `1.42654772` |
| Repeated `20/180/50/150/100 ms` scan jitter | `0 %` overshoot, `3.90 s` settling |
| Coupled loop A | Final error `0.00035342`, final MV `1.35475924` |
| Coupled loop B | Final error `0.00038136`, final MV `-1.02050773` |

These tests prove bounded behavior only for the declared models, gains,
coupling, timing pattern, and duration.

## 4. What the failed attempts taught us

### Unsuitable tuning is not a PID defect

The first integrating-process proposal used PI gains `Kp=0.8`, `Ki=0.16`. It
produced `13.72 %` overshoot and `13.4 s` settling, failing its declared limits.
The plant already integrates its input, so proportional control provides zero
steady-state step error. Changing to P control produced zero overshoot and
`4.7 s` settling without changing PID production code.

The unstable plant also demonstrated the difference between stabilization and
good performance. `Kp=1`, `Ki=0.16` converged but overshot by `28.54 %`.
`Kp=2`, `Ki=1` reduced this to `21.79 %`, still outside the fixed `20 %` gate.
`Kp=3`, `Ki=1` passed at `13.49 %`. The acceptance gate was not weakened to fit
the first result.

### Invalid test infrastructure is not red evidence

The first generated ST fixture emitted large limits as integer-looking
literals and failed to compile. The first native conformance invocation could
not find its `trust-dev` executable. Neither failure exercised PID behavior.
The harness prerequisites were corrected before results were accepted.

The first jitter fixture placed an `LREAL` at `%IL0` and another at `%IL1`.
Because an `LREAL` occupies eight bytes, these addresses overlap. The resulting
corruption caused an unbounded trace. Moving the second value to `%IL8`
produced a stable response. This is an important PLC lesson: direct addresses
identify byte offsets, not variable numbers.

### Acceptance limits must precede interpretation

Changing limits after observing a poor response can hide bad tuning. Every
benchmark therefore declares its plant, tuning, sample interval, duration, and
limits before its accepted result. When a proposed tuning fails, the failure is
classified and the tuning rationale is corrected explicitly.

### Simulation truth has a boundary

The simulator is intentionally imperfect and can add limits, noise, delay,
faults, and uncertainty. It still represents declared equations, not all
unmodelled physical behavior. Passing simulation is a reason to proceed to the
next controlled validation stage, not a safety certificate.

## 5. How to define a control system correctly

A controller should not be tuned until the loop itself is defined. Use the
following control-loop record for both simulation and physical work.

### 5.1 Identity and purpose

- stable loop identifier and revision;
- controlled variable and engineering unit;
- manipulated variable and engineering unit;
- process purpose and operator-visible objective;
- evidence source: simulation, model prediction, recorded physical data, or
  current physical execution; and
- accountable owner for approval and commissioning.

### 5.2 Signal contract

- exact SP, PV, MV, tracking, manual-output, and feedforward signals;
- direction of action: whether increasing MV increases or decreases PV;
- normal range, engineering range, alarm range, and credible fault range;
- update rate, timestamps, freshness, validity, and quality behavior;
- resolution, quantisation, noise, bias, dropout, and filtering; and
- non-overlapping process-image addresses with declared data types.

### 5.3 Plant and equipment contract

- process gain and sign;
- dominant time constants, dead time, integrating behavior, resonance, or
  unstable poles;
- operating-point dependence and uncertainty range;
- actuator minimum, maximum, slew, lag, deadband, stiction, and failure state;
- sensor lag, filtering, calibration, and failure state; and
- interactions with other loops, sequencing, interlocks, and safety functions.

### 5.4 Quantitative objective

Replace “works well” with numerical requirements:

- maximum overshoot and undershoot;
- settling band and maximum settling time;
- allowed steady-state error;
- disturbance magnitude and required recovery time;
- allowed IAE, ISE, or ITAE where useful;
- MV excursion, rate, saturation duration, and movement budget;
- minimum gain and phase margins;
- maximum sensitivity or complementary-sensitivity peaks when used;
- allowed noise-driven MV activity; and
- declared plant-variation and timing-jitter envelope.

Not every project needs every metric. Every metric that determines acceptance
does need a definition, unit, evidence source, and limit.

### 5.5 Mode and transition contract

Define startup state, Auto, Manual, Track, disabled or held behavior outside
the baseline where required, manual-to-auto transfer, parameter changes,
tracking of downstream applied output, disconnection, stale data, invalid
values, abort, restoration, rollback, and safe fallback ownership.

### 5.6 Safety and authorization contract

Define hard equipment limits independently from PID settings. State who may
approve an experiment, maximum excitation, duration, timeout, abort criteria,
restoration value, interlocks that remain active, and the required physical
topology review. A controller limit is not a safety function unless the safety
case explicitly establishes it as one.

## 6. Choosing P, PI, PD, or PID

| Structure | Prefer when | Main cost or risk |
| --- | --- | --- |
| P | Plant integration already removes step offset, or some offset is acceptable | Offset on self-regulating plants; positional mode changes may jump |
| PI | Most self-regulating industrial processes need zero steady-state offset | Added phase lag, overshoot, and windup risk |
| PD | Offset is acceptable and damping or prediction is valuable | Noise sensitivity; uncommon as a complete industrial regulator |
| PID | Integral accuracy and derivative damping both provide measurable benefit | More tuning dimensions and greater noise sensitivity |

Start with the simplest structure that meets the objective. Do not enable a
term merely because the function block supports it. The integrating benchmark
is a direct example: adding I action made the result worse.

Use proportional setpoint weighting to reduce setpoint aggression without
weakening disturbance response. Use derivative setpoint weighting cautiously;
derivative on a stepped setpoint can create a kick. Filter the measurement only
as much as needed, and include the filter's phase lag in stability validation.

## 7. A disciplined tuning workflow

### Step 1: qualify data

Confirm units, direction, sample timing, signal quality, operating state,
limits, and sufficient excitation. Reject stale, clipped, saturated,
unrepresentative, or poorly timestamped data.

### Step 2: choose a model family

Use the simplest model that explains the behavior needed for tuning:

- static gain for immediate scaling;
- first order for one dominant lag;
- FOPDT for one lag plus transport delay;
- integrator for inventory or position accumulation;
- second order for inertia or resonance;
- state space for coupled or unstable linear dynamics; and
- explicit nonlinear actuator and sensor behavior when constraints dominate.

Model selection is an engineering hypothesis. Record fit, residuals,
uncertainty, delay resolution, operating range, and known missing dynamics.

### Step 3: select a tuning method

The tuning method must match the model and objective. The implementation
program should qualify at least:

- Lambda/IMC or SIMC-style tuning for transparent robustness-speed control;
- AMIGO-style tuning for a broad industrial comparison;
- direct optimization of a declared objective subject to robustness and
  actuator constraints; and
- manual or imported gains as an explicit expert route.

Every adopted method needs independent reference cases and tolerances. A
library result is not trusted merely because a dependency compiled.

### Step 4: select the structure

Generate permitted P, PI, PD, and PID candidates. Reject structures that are
unnecessary, numerically invalid, too noise-sensitive, or unable to meet
steady-state requirements. Explain why the selected structure won.

### Step 5: choose the objective

Expose a compact user choice such as More robust, Balanced, or Faster. Map that
choice to explicit numerical weights and constraints. Professionals may inspect
and override the advanced values; learners should not need to begin with a wall
of parameters.

### Step 6: predict and validate

Compare current and candidate controllers using:

- setpoint and disturbance time responses;
- overshoot, settling, final error, IAE/ISE/ITAE;
- MV peak, rate, movement, and saturation recovery;
- noise response;
- Bode magnitude and phase where the model permits them;
- gain margin, phase margin, and uncertainty envelope; and
- actuator and sensor nonidealities.

### Step 7: apply separately

Tuning completion shall never imply application. Review gains, dimensions,
limits, source, model, evidence, and revision. Apply only after a separate
confirmation, monitor a bounded observation interval, and retain exact
rollback state.

## 8. Turning every benchmark into a training example

Each example should be a runnable truST project, not a screenshot or prose-only
exercise. It should contain controller ST, `simulation.toml`, declared
objectives, a deterministic seed, a native test, expected metrics, learner
questions, and an optional advanced task.

### Common lesson format

Every lesson should use the same progression:

1. **Purpose:** what physical behavior the plant represents.
2. **Prediction:** what the learner expects before running it.
3. **Configure:** connect SP, PV, and MV and inspect units and limits.
4. **Monitor:** verify initial state, quality, direction, and mode.
5. **Experiment:** approve and run one bounded test.
6. **Identify:** select and qualify a model.
7. **Tune:** compare permitted controller structures and methods.
8. **Validate:** test performance, robustness, constraints, and noise.
9. **Explain:** state why the accepted candidate passed and alternatives lost.
10. **Export:** retain configuration, complete trace, metrics, and conclusion.

### Proposed lesson catalog

| Lesson | Main concept | Learner experiment | Expected insight |
| --- | --- | --- | --- |
| Paper FOPDT reference | Closed-loop fundamentals | Compare open loop and Lambda PI unit steps | A well-damped PI removes offset and trades speed for robustness |
| Bounded plant variation | Robustness | Run the same gains over four gain/time/delay variants | One pass does not establish a robustness envelope |
| Sample interval | Discrete execution | Compare `50`, `100`, and `200 ms` | Actual sampling affects increments, derivatives, limits, and fidelity |
| Severe scan jitter | Actual elapsed time | Run the repeated `20–180 ms` sequence | Passing actual `CycleTime` preserves the declared response |
| Load disturbance | Regulation versus tracking | Apply the `0.2` plant bias after settling | Integral action restores the controlled variable |
| Saturation and anti-windup | Constraint handling | Compare no useful recovery, BackCalculation, and Combined behavior | Nominal output state matters while the actuator is limited |
| Rate limiting | Actuator protection | Apply a step with `1 unit/s` MV rate | A controller can be stable while response speed is actuator-limited |
| Measurement noise | Filtering tradeoff | Compare raw and filtered PV with identical seed | Filtering reduces MV movement but adds lag |
| Integrating plant | Structure selection | Compare P with the rejected PI proposal | An integrating plant may not need controller integral action |
| Underdamped second order | Derivative damping | Compare PI and PID candidates | D action can add damping but must be checked against noise |
| Delay-dominant plant | Fundamental delay limit | Increase PI aggression, then compare future predictor support | Faster PID tuning cannot remove transport delay safely |
| Nonlinear actuator | Real final-control behavior | Enable saturation, slew, lag, and deadband one at a time | Actuator behavior can dominate closed-loop performance |
| Open-loop unstable plant | Stabilization | Compare the three attempted PI tunings | Convergence alone is not acceptable transient performance |
| Scheduled gain change | Operating-point dependence | Change gain from `0.6` to `1.4` | Fixed PID may recover, while scheduling could improve consistency |
| Coupled two-loop plant | Loop interaction | Step each setpoint separately and together | Independent PIDs work for weak coupling; stronger interaction needs analysis |
| Evidence integrity | Test validity | Intentionally overlap `%IL0` and `%IL1`, then correct to `%IL8` | Invalid I/O mapping can look like an unstable controller |

### Learning levels

**Foundation:** paper FOPDT, sample interval, load disturbance, saturation, and
noise. Learners identify SP, PV, MV, offset, overshoot, settling, and limits.

**Practitioner:** integrating, underdamped, delay-dominant, nonlinear actuator,
and bounded variation. Learners select structure and justify tuning tradeoffs.

**Advanced:** unstable plant, scheduled gain, severe jitter, and coupled loops.
Learners separate stabilization, robustness, timing, and interaction problems.

**Research:** compare identification and tuning algorithms using retained raw
traces, uncertainty ensembles, independent numerical oracles, and reproducible
reports.

### Documentation generated from evidence

Training documentation should read the same versioned run artifact used by
Control Studio. Tables and plots can then be regenerated from actual results.
Expected values remain executable assertions, while explanatory text states why
the result matters. Screenshots illustrate interaction but never replace native
tests or trace data.

## 9. Recommended implementation program

### Priority 0: persistent run artifacts

Implement this first because every later comparison depends on it.

Each completed or aborted run should retain:

- schema version, run ID, creation time, project and controller revision;
- evidence class and Simulation or Physical source;
- plant configuration, effective random seed, events, and uncertainty;
- controller structure, parameters, state initialization, limits, and modes;
- signal identities, units, quality, and complete timestamped SP/PV/MV trace;
- test plan, approval boundary, abort/restoration result, and stop reason;
- calculated metrics with definitions, units, windows, and limits;
- model, fit, uncertainty, tuning method, candidate comparison, and margins;
- software versions and numerical-adapter versions; and
- the existing Control Studio result revision used to save and reopen the run.

JSON should preserve the complete typed artifact. CSV should export selected
signals for external analysis. PNG is presentation only. Tests must prove
deterministic replay, bounded storage, schema rejection, unit preservation, and
that simulation artifacts cannot be relabelled as physical.

This extends the existing Control Studio saved-result and export contract. It
does not require a new artifact database, content-addressed graph, hash scheme,
pagination framework, or parallel persistence service.

### Priority 1: executable training projects

Package the lesson catalog as small example projects that reuse the benchmark
plant configurations and assertions. Add them to the example catalog and
Control Studio learning path. Each lesson starts with safe defaults and hides
advanced parameters behind explanations, while keeping the files editable.

### Priority 2: qualified tuning comparison

Implement truST-owned adapters for selected tuning algorithms. Begin with
Lambda/IMC or SIMC-style tuning and AMIGO-style tuning, then add constrained
optimization. Compare all methods on the complete benchmark catalog using
identical metrics and limits.

Do not select the winner by IAE alone. A candidate must first satisfy stability,
uncertainty, MV, rate, saturation, noise, and implementation constraints.

### Priority 3: automatic structure selection

Given qualified data and a qualified model, evaluate P, PI, PD, and PID only
where each structure is meaningful. Produce a recommendation with reasons,
rejected alternatives, uncertainty, and limitations. The result remains
unapplied until explicit review and confirmation.

### Priority 4: robustness and uncertainty campaigns

Generate deterministic bounded ensembles over model gain, time constants,
delay, damping, actuator behavior, noise, and timing. Report worst case and
distribution summaries without hiding failed members. Preserve the seed and
each exact case so every failure is reproducible.

### Priority 5: composable advanced-control components

Only after the baseline comparison program is working, specify and test
separate components for:

- setpoint filtering or reference shaping;
- feedforward calculation;
- gain scheduling between independently validated parameter sets;
- cascade, ratio, and split-range coordination;
- static or dynamic multivariable decoupling;
- Smith-predictor-style delay compensation; and
- downstream applied-output tracking when actuator feedback differs from the
  requested MV.

These components should compose with `Control.PID`; they should not enlarge its
equation or retained state.

### Priority 6: runtime signal-quality and safety policy

The paper baseline does not define nonfinite inputs, stale data, disabled or
fault modes, or safe output. Specify these at the loop-supervision boundary.
Projects need explicit behavior for invalid PV, lost timestamps, stale MV
feedback, disconnection, initialization, restart, and restoration failure.

Safety policy must remain independent from performance tuning and must fail
closed according to the reviewed application topology.

### Priority 7: bounded physical commissioning

Reuse the Monitor, Test, Identify, Tune, and Validate sequence for physical
plants only after topology review and immediate approval of the exact action.
Start with passive recording, then the smallest useful bounded excitation.
Compare physical and simulated residuals, revise the model, revalidate, apply
separately, observe, and retain rollback.

## 10. Acceptance criteria for future improvements

Every improvement shall follow specification, test, then code:

1. Define observable behavior, units, limits, failure behavior, and evidence
   class in a written specification.
2. Add the smallest native executable or rendered interaction test.
3. Run it before production implementation and obtain the expected behavioral
   assertion failure when behavior is genuinely missing.
4. Implement only the required behavior and obtain green from the same test.
5. Run the complete PID benchmark catalog to detect regressions.
6. Preserve complete run artifacts and current UI evidence where visible.
7. Keep simulation, model prediction, recorded physical data, and commissioned
   acceptance distinct.

A new tuning method is accepted only when it passes independent reference
oracles and improves or usefully trades declared metrics without violating
robustness or constraints. A new advanced-control component is accepted only
when it outperforms the best applicable PID baseline on the cases it claims to
address.

## 11. What should not be changed now

The current evidence does not justify changing the paper-compatible PID
equations, adding a hidden derivative filter, merging all controller structures
into separate duplicate blocks, embedding identification or tuning inside the
function block, or treating two coupled PID instances as proof for arbitrary
MIMO systems.

The current implementation should remain the regression baseline while the
engineering environment around it becomes richer.

## 12. Results of the next implementation stage

The recommended sequence has now been implemented around the unchanged
paper-compatible `Control.PID`:

- Control Studio saves complete versioned run revisions, reopens and compares
  them, and exports typed JSON plus full-precision signal CSV without changing
  the evidence class.
- Sixteen benchmark cases are ordinary runnable truST simulation lessons using
  the same controller, process-image path, simulator, metrics, and result
  repository as non-lesson projects.
- SIMC PI and AMIGO PI are qualified against independent published-reference
  constants. Unsupported domains fail deterministically instead of producing
  plausible-looking gains.
- A bounded comparison evaluates applicable P, PI, PD, and PID candidates with
  the same model, running-controller limits, setpoint weights, anti-windup,
  scenarios, uncertainty, and objective. Ranking includes IAE, ISE, ITAE, MV
  movement, peak and rate use, noise-driven movement, and stability margins.
- Formula candidates execute through the actual Structured Text `Control.PID`
  and simulator. The tuning side does not contain a second PID equation.
- The nominal FOPDT oracle demonstrates an explainable tradeoff rather than a
  universal winner: in the native run SIMC settled in about `3.9 s` with IAE
  about `1.40`, while AMIGO settled in about `6.8 s` with IAE about `2.16` and
  a different control-effort profile; both remained stable with zero measured
  overshoot in that case.

The Control Studio result retains the winning method, exact parameters, SIMC
`lambda` where applicable, all rejected or unavailable alternatives, common
metrics, requirements, source, and conclusion. Generated settings remain
unapplied until the separate review and apply action.

## 13. Current limitations

- Saved Control Studio results retain raw samples, but the full sixteen-project
  qualification campaign currently records method status and native benchmark
  summaries rather than checking every campaign trace into the repository.
- The stability margins are model-predicted, not measured physical frequency
  responses.
- The uncertainty cases are bounded examples, not a complete robust-control
  proof.
- The MIMO test uses weak linear coupling and independent PIDs, not a general
  multivariable controller.
- No physical plant was operated for this evaluation.
- Nonfinite input, stale-data, disabled/fault mode, and safe-output policy are
  outside the paper-compatible PID baseline.
- Gain scheduling, Smith prediction, and dynamic decoupling remain proposed
  composable components until separately specified and proven.
- The first bounded structure comparison is deliberately small and
  deterministic; it is not a general nonlinear optimizer.
- SIMC and AMIGO are qualified only for their declared stable positive-gain
  low-order domains. Strong coupling, unstable-model tuning, delay predictors,
  and other plant classes remain explicit unsupported boundaries.
- No independent novice session or physical-plant experiment was performed.

## 14. Recommended next action

Run an observed novice lesson session and a separately authorized physical-loop
commissioning trial. Use what those reveal to improve terminology and
commissioning guidance before expanding the numerical domain. Then qualify any
new predictor, scheduling, feedforward, cascade, or decoupling component against
the same saved evidence and best applicable PID baseline.

That sequence gives learners understandable experiments, gives professionals
auditable evidence, and gives future automatic tuning a trustworthy numerical
foundation.

## Related documentation

- [Control Studio](../operate/control-studio.md)
- [Simulation](../operate/simulation.md)
- [Build, Validate, Test](../operate/build-validate-test.md)
- [Safety and Commissioning](../operate/safety-and-commissioning.md)
- [Learning paths](../examples/learning-paths.md)
