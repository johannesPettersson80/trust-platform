# Control PID performance and stability evaluation

This specification defines the deterministic benchmark used to evaluate the
executed `Control.PID` implementation with truST simulated plants. It answers
whether the paper-compatible controller behaves well for a declared plant and
tuning; it does not claim that one tuning is stable for every plant.

Controller behavior remains defined by Specification 32. Plant execution
remains defined by Specification 33. This specification adds evaluation
scenarios and acceptance metrics only. It does not change the controller,
simulator, or Control Studio.

The nominal benchmark reproduces the process and tuning used by Sundström et
al. in *A Practical Guide to PID Controller Implementation*, version 3,
Sections 5.2 and 6:

```text
P(s) = exp(-0.5 s) / (1 + s)
Kp = 0.667
Ki = 0.667
Kd = 0
sample interval = 0.1 s
```

## 1. Evaluation boundary

Every time-domain benchmark shall execute the public Structured Text
`Control.PID` function block through the truST runtime. The controller command
shall reach the simulated plant through an ordinary `%Q` process-image binding,
and plant feedback shall return through an ordinary `%I` binding.

A duplicate controller equation implemented only in Rust, TypeScript, Python,
or MATLAB is not execution evidence for these time-domain benchmarks.
Analytical or independently calculated frequency-domain results may supplement
the executed traces but shall be labelled model-predicted.

Every scenario shall record at least time, setpoint, process value, controller
output, maximum absolute process value, maximum absolute controller output,
integrated absolute error, overshoot, final error, and settling time when the
metric is applicable.

The response is stable only if every recorded value is finite, the process
value remains within `10` times the largest commanded or disturbance magnitude,
and the final declared settling window remains bounded.

## 2. Metric definitions

For a setpoint step of amplitude `A`:

- absolute error is `ABS(SP - PV)`;
- integrated absolute error is the sum of absolute error multiplied by the
  actual cycle time;
- overshoot is `100 * MAX(0, PeakPV - SP) / ABS(A)` for a positive step;
- final error ratio is `ABS(SP - FinalPV) / ABS(A)`;
- settling time is the last time the response enters and thereafter remains in
  the band `MAX(0.02 * ABS(A), 0.02)` around the final setpoint; and
- controller-output rate is the output difference divided by the actual cycle
  time.

Noise activity is the root-mean-square controller-output increment after the
nominal response has entered its steady observation interval. This measures
actuator movement caused by measurement noise rather than saturation error.

## 3. Nominal paper process

The nominal paper process shall run for `20 s` with a unit setpoint step from
zero. The executed response shall satisfy:

- stable and finite for the complete run;
- overshoot no greater than `20 %`;
- settling time no greater than `4 s`;
- final error ratio no greater than `0.0001`; and
- integrated absolute error no greater than `1.4`.

These limits are benchmark acceptance limits, not tuning rules for other
plants.

## 4. Model-variation robustness

The same fixed controller shall be evaluated without retuning against these
FOPDT variations:

| Case | Gain | Time constant | Dead time |
| --- | ---: | ---: | ---: |
| low gain, fast, short delay | `0.8` | `0.8 s` | `0.4 s` |
| high gain, slow, long delay | `1.2` | `1.2 s` | `0.6 s` |
| high gain, fast, long delay | `1.2` | `0.8 s` | `0.6 s` |
| low gain, slow, short delay | `0.8` | `1.2 s` | `0.4 s` |

Each `30 s` response shall remain stable, have overshoot no greater than
`20 %`, settle within `8 s`, and finish within `2 %` of the setpoint. Passing
these cases supports only the declared variation range.

## 5. Sampling-interval sensitivity

The nominal plant and tuning shall run with actual controller and plant
intervals of `0.05 s`, `0.1 s`, and `0.2 s`. Each `30 s` response shall remain
stable, have overshoot no greater than `20 %`, settle within `8 s`, and finish
within `2 %` of the setpoint.

The controller shall receive the actual interval used by that run. A test that
changes simulator timing while passing a fixed interval to `PID.Update` is
invalid.

## 6. Load-disturbance rejection

After the nominal response has settled, the plant output bias shall step from
`0` to `0.2` at `10 s`. Without retuning, the PI controller shall:

- remain stable and finite;
- return within `2 %` of the unit setpoint by `20 s`; and
- accumulate no more than `0.35` integrated absolute error from `10 s` through
  `20 s`.

The disturbance shall be introduced by the plant event system, not by changing
the controller's process-value input directly.

## 7. Saturation, anti-windup, and rate limiting

The saturation recovery case shall use the paper's Section 6.4 control-path
process and PID parameters:

```text
P(s) = exp(-0.5 s) / (1 + 3 s)
Kp = 2.83
Ki = 2.83 / 3.25
Kd = 2.83 * 0.23
output limits = [-3.5, 3.5]
```

The setpoint shall be `4.5` from `1 s` through `20 s`, then return to zero.
Both BackCalculation with tracking time `3.25 s` and Combined anti-windup with
the same tracking time shall keep the output inside its limits, leave upper
saturation after the return step, remain finite, and return the process value
within `0.05` of zero by `50 s`.

A separate nominal-process case shall use output-rate limits `[-1, 1]` per
second. Every executed output increment shall respect the declared rate within
`1e-10`, and the response shall finish within `2 %` of the setpoint by `30 s`.

## 8. Measurement-noise filtering

Two otherwise identical nominal paper loops shall receive the same deterministic
seeded additive measurement noise. One loop shall pass the measurement directly
to `Control.PID`; the other shall pass it through a second-order
`Control.LowPass` with time constant `0.1 s` before `PID.Update`.

After the response has settled:

- both loops shall remain finite and finish within `5 %` of the setpoint; and
- the filtered loop's controller-output increment RMS shall be lower than the
  unfiltered loop's RMS.

This comparison proves the paper's separate filtering boundary. It does not
permit `PID.Update` to add an undocumented internal filter.

## 9. Frequency-domain evidence

For the nominal continuous FOPDT model and PI tuning, the open-loop frequency
response shall be evaluated over a frequency range that resolves the gain and
phase crossovers. The reported model-predicted margins shall satisfy:

- gain margin at least `6 dB`; and
- phase margin at least `30 degrees`.

Frequency-domain evidence shall remain labelled model-predicted and shall not
be presented as an observed physical-plant result.

## 10. Result interpretation

Passing every benchmark means:

- the paper-compatible controller executes correctly through the truST runtime
  and simulated process-image boundary for the declared cases;
- the paper's nominal Lambda PI tuning has acceptable tracking and disturbance
  rejection for the declared FOPDT process and variation range;
- actuator limits, rate limits, anti-windup, actual cycle time, and the separate
  LowPass boundary behave acceptably in the declared cases; and
- nominal model-predicted stability margins exceed the declared minimums.

It does not mean that arbitrary gains, arbitrary plants, invalid or nonfinite
inputs, unmodelled nonlinearities, network delays, or physical equipment are
safe or stable. Physical validation requires a reviewed topology, explicit
authorization, and observed hardware evidence.

## 10.1 Tuning-method qualification

The SIMC and AMIGO formula implementations defined by Specification 35 shall be
qualified against independently calculated published-reference cases before
their candidates are used by Control Studio. Oracle tests shall compare `Kp`,
`Ti`, and the explicit parallel-form `Ki = Kp / Ti` using a relative tolerance
of `1e-7` and shall not calculate expected values by calling production tuning
code.

Every qualified candidate shall then be executed by the public Structured Text
`Control.PID` through the runtime and `%Q/%I` simulated-plant boundary. A
TypeScript prediction is useful model-predicted evidence but does not replace
this executed-candidate test.

The comparison campaign shall use the same accepted model, constraints, actual
sample interval, scenarios, and metric definitions for every applicable method.
It shall retain method failures and unsupported-domain results. Acceptance
requires at least one explainable performance/robustness/actuator tradeoff; it
does not require or permit claiming one universal winning method.

## 11. Representative plant-class benchmark

The following cases extend the evaluation beyond the paper's nominal FOPDT
process. Each case shall execute the public Structured Text `Control.PID`
through the runtime and normal `%Q/%I` process-image boundary. The tuning is
part of each benchmark and is not reused as a universal tuning.

### 11.1 Integrating process

The integrating process shall use:

```text
P(s) = 1 / s
Kp = 0.8
Ki = 0
Kd = 0
sample interval = 0.1 s
duration = 30 s
```

The plant's integral action makes proportional control sufficient for zero
steady-state step error; adding controller integral action is not required for
this benchmark. For a unit setpoint step, the response shall remain stable and finite, have no
more than `10 %` overshoot, settle within `12 s`, and finish within `1 %` of
the setpoint.

### 11.2 Underdamped second-order process

The underdamped process shall use:

```text
P(s) = 1 / (s^2 + 0.4 s + 1)
natural frequency = 1 rad/s
damping ratio = 0.2
Kp = 1.5
Ki = 0.5
Kd = 1.0
sample interval = 0.05 s
duration = 30 s
```

For a unit setpoint step, the response shall remain stable and finite, have no
more than `20 %` overshoot, settle within `15 s`, and finish within `2 %` of
the setpoint.

### 11.3 Delay-dominant process

The delay-dominant process shall use:

```text
P(s) = exp(-2 s) / (1 + s)
Kp = 1 / 6
Ki = 1 / 6
Kd = 0
sample interval = 0.1 s
duration = 60 s
```

For a unit setpoint step, the response shall remain stable and finite, have no
more than `20 %` overshoot, settle within `25 s`, and finish within `2 %` of
the setpoint.

### 11.4 Nonlinear actuator process

The process shall be `1 / (1 + s)` behind an actuator with output range
`[0, 1.2]`, rising and falling slew limits of `1 unit/s`, deadband `0.02`, and
first-order lag `0.2 s`. The PI controller shall use `Kp = 1`, `Ki = 0.5`,
output limits `[0, 1.2]`, Combined anti-windup with tracking time `1 s`, a
sample interval of `0.05 s`, and a duration of `30 s`.

For a unit setpoint step, the response and controller output shall remain
stable and finite, the controller output shall remain in `[0, 1.2]`, overshoot
shall not exceed `10 %`, settling time shall not exceed `15 s`, and final error
shall not exceed `3 %`.

### 11.5 Interpretation

Passing all four added cases demonstrates that the same PID implementation can
execute suitable P/PI/PID configurations over integrating, oscillatory,
delay-dominant, and actuator-nonlinear plants. It does not demonstrate that one
gain set works for all four plants. A failure shall be diagnosed before code is
changed: unsuitable gains are a tuning result, while incorrect equations,
limits, state transitions, or cycle-time handling are controller defects.

## 12. Advanced operating-condition benchmark

These cases exercise difficult operating conditions with existing simulator
models. Every controller shall be the public Structured Text `Control.PID` and
every command and measurement shall cross the normal process image.

### 12.1 Open-loop unstable plant

The scalar state-space plant shall use `A = [[0.2]]`, `B = [[1]]`,
`C = [[1]]`, and `D = [[0]]`, equivalent to `P(s) = 1 / (s - 0.2)`. The PI
controller shall use `Kp = 3`, `Ki = 1`, `Kd = 0`, sample interval `0.05 s`,
and duration `30 s`.

For a unit step, every value shall remain finite, absolute PV shall remain at
or below `10`, overshoot shall not exceed `20 %`, settling time shall not exceed
`15 s`, and final error shall not exceed `2 %`.

### 12.2 Scheduled gain change

An FOPDT plant with time constant `1 s`, dead time `0.5 s`, and initial gain
`0.6` shall use the paper PI tuning `Kp = Ki = 0.667` at `0.1 s`. At `15 s`, a
native `set_parameter` plant event shall change gain to `1.4`. The controller
shall not be retuned during the `45 s` run.

The response shall remain finite and within absolute PV `2`, finish within
`2 %` of the unit setpoint, return within the `2 %` band no later than `15 s`
after the gain change, and accumulate no more than `2.0` IAE after the event.

### 12.3 Severe deterministic scan jitter

The nominal paper plant and tuning shall use the repeated PLC interval sequence
`[0.02, 0.18, 0.05, 0.15, 0.10] s`, whose mean is `0.1 s`, for `30 s`. The
simulator shall retain a fixed `0.05 s` plant step. Each actual PLC interval,
not the mean or preceding interval, shall be passed to the corresponding
`PID.Update` call.

The response shall remain finite and within absolute PV `10`, have no more than
`20 %` overshoot, settle within `8 s`, and finish within `2 %` of setpoint.

### 12.4 Coupled two-loop plant

The two-input, two-output state-space plant shall use:

```text
A = [[-1.0, 0.2], [0.15, -1.2]]
B = [[1.0, 0.25], [0.2, 1.0]]
C = [[1.0, 0.0], [0.0, 1.0]]
D = [[0.0, 0.0], [0.0, 0.0]]
```

Two independent PI instances shall each use `Kp = 0.8`, `Ki = 0.4`, `Kd = 0`
at `0.05 s`. Their setpoints shall be `1.0` and `-0.5` for `40 s`. Each loop
shall use its own `%Q/%I` pair without a decoupling equation outside the public
PID instances.

All values shall remain finite; absolute PV and MV shall remain at or below
`5`; and each final error shall be no greater than `2 %` of its setpoint
amplitude. The final `5 s` of each loop shall remain entirely within that `2 %`
band.

### 12.5 Interpretation

Passing these cases demonstrates execution against one declared unstable
plant, one declared gain transition, one declared jitter pattern, and one
declared coupled process. It does not establish robust stability for arbitrary
uncertainty, arbitrary jitter, stronger coupling, packet loss, computation
overruns, or physical equipment. Those require separately specified bounds and
evidence.

## 13. Executable learning projects

The benchmark shall also be available as sixteen runnable simulation projects
in the existing VS Code example catalog. A lesson is a normal truST project,
not a separate tutorial format. Each lesson directory shall contain
`trust-lsp.toml`, `runtime.toml`, `io.toml`, `simulation.toml`, a runnable
`src/Main.st`, the existing simulator launch/settings files, `README.md`, and
`lesson.json`.

`lesson.json` identifies the lesson, level, benchmark section, the native
benchmark test that establishes its expected result, expected metrics, and the
ordered stages that apply. It is navigation metadata only; the referenced
native test and its direct assertions remain the evidence authority.

Every README shall introduce the plant and control problem, state that all
results are simulation evidence, and use these sections: Objective, Predict,
Configure, Monitor, Test, Identify, Tune, Validate, Explain, Export, Questions,
and Advanced exercise. A stage that is intentionally not performed shall say
why. Values and acceptance limits shall cite this specification rather than a
separate teaching equation.

The catalog shall contain these projects:

| Lesson id | Level | Benchmark behavior |
| --- | --- | --- |
| `control-nominal-fopdt` | Foundation | nominal paper FOPDT and Lambda PI |
| `control-sampling-interval` | Foundation | `50`, `100`, and `200 ms` actual intervals |
| `control-load-disturbance` | Foundation | plant-side `0.2` bias event |
| `control-saturation-antiwindup` | Foundation | BackCalculation and Combined recovery |
| `control-measurement-noise` | Foundation | raw and second-order filtered PV with one seed |
| `control-rate-limit` | Foundation | actuator rate limited to `1 unit/s` |
| `control-integrating-plant` | Practitioner | integrating plant with P control |
| `control-underdamped-pid` | Practitioner | underdamped second-order plant with PID |
| `control-delay-dominant` | Practitioner | delay-dominant FOPDT with robust PI |
| `control-nonlinear-actuator` | Practitioner | saturation, slew, lag, and deadband |
| `control-bounded-variation` | Practitioner | the four declared FOPDT variations |
| `control-open-loop-unstable` | Advanced | unstable scalar state-space plant |
| `control-scheduled-gain` | Advanced | native gain event from `0.6` to `1.4` |
| `control-severe-jitter` | Advanced | actual `20/180/50/150/100 ms` cycle sequence |
| `control-coupled-two-loop` | Advanced | two independent PIs on the declared MIMO plant |
| `control-evidence-integrity` | Advanced | `%IL0`/`%IL1` overlap corrected to `%IL0`/`%IL8` |

Each project shall start with bounded deterministic simulation settings and
ordinary process-image bindings. It shall reuse `Control.PID`, the simulator,
Control Studio, saved-result export, and the native benchmark tests defined by
this specification. It shall not contain a second controller equation, a
teaching-only plant implementation, or values labelled as physical evidence.
