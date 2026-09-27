# Advanced control structures for PID loops

Shared purpose and completion claims: [Control Platform Purpose and Acceptance Contract](control-platform-purpose.md).
This document retains its detailed technical or teaching authority.

This specification defines three optional structures that extend the useful
plant range of `Control.PID` without changing its
equations:

- `Control.SmithPredictorFOPDT` compensates a validated first-order-plus-dead-
  time model;
- `Control.PIDGainScheduler` interpolates validated PID tunings over one
  measured operating variable; and
- `Control.StaticDecoupler2x2` separates two interacting steady-state process
  channels before two ordinary PID controllers drive two actuators.

These structures are truST library contracts. IEC 61131-3 does not standardize
Smith prediction, gain scheduling, decoupling, or MIMO control, so they are not
IEC standard function blocks and do not create IEC deviations. Their
declarations use IEC 61131-3 Edition 3 Table 11 derived data types, Table 40
function blocks, and Tables 64 through 66 namespaces.

`Control.PID` remains defined solely by Specification 32. None of the types or
function blocks in this specification duplicates, subclasses, wraps, or
changes that controller. An application composes only the structure required
by its plant.

## 1. Public surface

### 1.1 Function blocks

| Function block | What it is | What it is used for |
|---|---|---|
| `Control.SmithPredictorFOPDT` | A first-order model with a dead-time history and measurement correction. | Supplies a delay-compensated process value to an ordinary PID when a qualified FOPDT model is valid. |
| `Control.PIDGainScheduler` | A stateful, filtered, piecewise-linear tuning interpolator. | Supplies one complete `PIDParameters` value as a plant operating point moves through a validated schedule. |
| `Control.StaticDecoupler2x2` | An invertible two-by-two steady-state gain transformation. | Converts two virtual PID commands into two physical actuator commands and maps applied actuator commands back for tracking. |

The library does not define a monolithic advanced PID or a general MIMO
controller. A qualified two-loop decoupler is the first implementation because
it preserves the understandable and already-qualified `Control.PID` execution
boundary. Dynamic decoupling, state-space control, MPC, and plants larger than
two-by-two require later specifications and benchmarks.

### 1.2 Datatypes

| Datatype | What it is | What it is used for |
|---|---|---|
| `SmithPredictorParameters` | The FOPDT model parameters and correction bound. | Configures one predictor for one process channel. |
| `SmithPredictorInitialState` | The measured and modeled values at initialization. | Starts prediction without an artificial model transient. |
| `SmithPredictorInput` | One actuator, measurement, cycle-time, and enable sample. | Updates the predictor once per controller interval. |
| `SmithPredictorOutput` | Prediction, model components, correction, and status. | Gives the PID its compensated PV and exposes enough state to diagnose model mismatch or bypass. |
| `PIDTuning` | Only the six numerical values that define PID tuning dynamics. | Separates scheduled tuning from common actuator limits and anti-windup policy. |
| `PIDGainSchedulePoint` | One operating coordinate and one validated tuning. | Defines one knot in a gain schedule. |
| `PIDGainSchedulePoints` | A fixed-capacity array of eight schedule points. | Stores a bounded schedule without allocation in a cyclic task. |
| `PIDGainScheduleParameters` | Active points, common PID policy, and scheduling filter time. | Configures one schedule atomically. |
| `PIDGainScheduleOutput` | Interpolated parameters and schedule-selection status. | Allows the caller to configure the existing PID and display clamping or invalid state. |
| `ControlVector2` | Two `LREAL` channel values. | Carries virtual or actuator commands without ambiguous array ordering. |
| `ProcessGainMatrix2x2` | Four steady-state process gains. | Describes the identified relation from two actuator deviations to two output deviations. |
| `StaticDecoupler2x2Parameters` | Gain matrix, actuator bias, and singularity tolerance. | Configures an invertible operating-point decoupler. |
| `StaticDecoupler2x2Status` | Configuration state, determinant, and inverse coefficients. | Makes invalid or ill-conditioned configuration visible. |
| `PIDParameterForm` | Names Parallel, Ideal, and interacting Series conventions. | Prevents vendor parameters from being interpreted with the wrong equation. |
| `PIDFormParameters` | One explicitly tagged three-value parameter set. | Carries `Kp, Ki, Kd` in Parallel form or `Kc, Ti, Td` in Ideal or Series form. |
| `PIDFormConversionStatus` | A stable conversion outcome. | Distinguishes a valid result, invalid numeric/domain input, and a nonrepresentable Series target. |
| `PIDFormConversionResult` | Converted values plus status. | Returns one complete conversion without modifying PID state. |

All numeric fields use `LREAL`. Time values are seconds. No function block
allocates memory during `Update`.

### 1.3 Pure functions

| Function | What it is | What it is used for |
|---|---|---|
| `Control.ConvertPIDForm` | A stateless conversion between explicitly tagged Parallel, Ideal, and interacting Series parameters. | Converts vendor parameters before they are copied into the existing `PIDParameters`; it never executes or reconfigures a controller. |

## 2. Smith predictor

### 2.1 Model and purpose

`SmithPredictorFOPDT` models the deviation-form process

```text
y_model(s) - y0 = K / (tau*s + 1) * (u(s) - u0)
```

with dead time `theta`. The delay-free model predicts what the process would be
doing now if dead time were absent. The delayed model predicts what the real
measurement should contain now. The feedback value is

```text
PredictedProcessValue = DelayFreeModel
                      + clamp(ProcessValue - DelayedModel,
                              -MaximumCorrection,
                              +MaximumCorrection)
```

When `MaximumCorrection <= 0`, correction is not clamped. The correction keeps
the prediction anchored to the measured process rather than allowing the model
to replace reality.

### 2.2 `SmithPredictorParameters`

| Field | Meaning |
|---|---|
| `ProcessGain` | Identified steady-state gain `K`. It may be negative. |
| `TimeConstant` | Identified first-order time constant `tau`; shall be greater than zero. |
| `DeadTime` | Identified transport delay `theta`; shall be zero or greater. |
| `OperatingInput` | Actuator operating point `u0`. |
| `OperatingOutput` | Process operating point `y0`. |
| `MaximumCorrection` | Optional absolute bound on model correction; zero or negative means unbounded. |

`Configure` returns `TRUE` and atomically replaces the parameters when the
domain is valid. It returns `FALSE` and retains the previous valid
configuration when `TimeConstant <= 0` or `DeadTime < 0`.

### 2.3 Initialization

`SmithPredictorInitialState` contains `ModelOutput` and `ProcessValue`.
`Initialize` sets the delay-free and delayed model values to `ModelOutput`,
sets the initial correction from `ProcessValue - ModelOutput`, clears elapsed
time, and fills the assumed pre-start history with that equilibrium. It does
not execute a model interval.

Prediction becomes active only after successful configuration and
initialization. Before both operations, or while the input `Enabled` field is
`FALSE`, `PredictedProcessValue` equals the measured `ProcessValue` and
`Active` is `FALSE`. The internal model and history continue to update while
the predictor is bypassed so re-enabling does not restart the model.

### 2.4 Update

`SmithPredictorInput` contains `ManipulatedValue`, `ProcessValue`, `CycleTime`,
and `Enabled`. `CycleTime` shall be greater than zero.

The delay-free first-order model uses the same trapezoidal-compatible
coefficient as `Control.LowPass`:

```text
Coefficient = CycleTime / (TimeConstant + 0.5 * CycleTime)
Target = OperatingOutput + ProcessGain * (ManipulatedValue - OperatingInput)
DelayFreeModel = PreviousDelayFreeModel
               + Coefficient * (Target - PreviousDelayFreeModel)
```

The block stores timestamped delay-free model values in a fixed 256-sample
history. It linearly interpolates the model value at
`ElapsedTime - DeadTime`. The initialization equilibrium supplies history for
times before zero.

If 256 retained samples no longer reach the requested delayed time, the block
shall not extrapolate. It returns the measured process value, sets `Active` and
`HistoryComplete` to `FALSE`, and continues updating history. This deterministic
bypass tells the application that its task interval is too short for the
configured dead time and fixed history capacity.

`SmithPredictorOutput` contains:

| Field | Meaning |
|---|---|
| `PredictedProcessValue` | Value passed to `PIDInput.ProcessValue`. |
| `DelayFreeModel` | Current model response without transport delay. |
| `DelayedModel` | Interpolated model response at the delayed time. |
| `Correction` | Applied measured-minus-delayed-model correction after its optional bound. |
| `Active` | Prediction, rather than measured-value bypass, is being returned. |
| `HistoryComplete` | The retained history covers the configured delay. |

At zero dead time, the delayed and delay-free model values are equal and the
equation reduces exactly to the measured process value.

### 2.5 Composition and limitations

The application updates the predictor from the actuator command actually sent
to the plant, assigns `PredictedProcessValue` to `PIDInput.ProcessValue`, and
then executes the existing PID. The predictor does not own setpoint, PID gains,
actuator limits, anti-windup, mode, or tracking.

A Smith predictor shall be enabled only inside the gain, time-constant, dead-
time, operating-range, and data-quality bounds of its identified model. It
does not make an unstable, nonlinear, changing, or poorly identified plant
safe. A sustained correction near `MaximumCorrection` is model-mismatch
evidence, not successful compensation.

The application shall use ordinary measured-PV feedback whenever predictor
history is incomplete, measurement quality is invalid, the operating point is
outside the accepted model range, or the correction reaches its configured
bound. The internal model may continue updating, but fallback status shall be
visible and re-entry shall require all validity conditions to recover. A
clipped correction shall never be presented as a valid active prediction.

## 3. PID gain scheduler

### 3.1 Purpose and schedule

`PIDGainScheduler` interpolates tunings over one measurable scheduling
variable such as flow, speed, load, level, or temperature. It never selects a
tuning from control error alone and never identifies or tunes a plant online.
Every schedule point shall already be validated.

`PIDTuning` contains `ProportionalGain`, `IntegralGain`, `DerivativeGain`,
`ProportionalSetpointWeight`, `DerivativeSetpointWeight`, and `TrackingTime`.
Limits, bias, and `PIDAntiWindup` remain common policy in one `PIDParameters`
record so scheduling cannot silently change actuator protection.

`PIDGainSchedulePoint` contains `OperatingPoint` and `Tuning`.
`PIDGainSchedulePoints` is `ARRAY[0..7] OF PIDGainSchedulePoint`.

`PIDGainScheduleParameters` contains:

| Field | Meaning |
|---|---|
| `Points` | Up to eight schedule points. |
| `PointCount` | Number of active points; shall be from two through eight. |
| `CommonParameters` | Bias, output/rate limits, and anti-windup policy copied to every result. Its six tuning fields are ignored. |
| `FilterTimeConstant` | First-order smoothing time for the scheduling variable; zero disables smoothing and negative values are invalid. |

Active points shall be strictly increasing by `OperatingPoint`. Every point
with nonzero integral gain shall have positive `TrackingTime`.
`CommonParameters` shall satisfy the parameter-domain inequalities from
Specification 32. `Configure` returns `FALSE` and retains the previous valid
schedule if any condition fails; otherwise it atomically installs the complete
schedule and returns `TRUE`.

### 3.2 Initialization and update

`Initialize(OperatingPoint)` initializes the filtered scheduling value without
producing a tuning. `Update(OperatingPoint, CycleTime)` requires positive
`CycleTime` and successful configuration and initialization.

When `FilterTimeConstant > 0`:

```text
alpha = CycleTime / (FilterTimeConstant + CycleTime)
Filtered = PreviousFiltered + alpha * (OperatingPoint - PreviousFiltered)
```

Otherwise `Filtered = OperatingPoint`.

The filtered value is clamped to the schedule endpoints. Inside the schedule,
the block finds the adjacent points and calculates

```text
Fraction = (Filtered - Lower.OperatingPoint)
         / (Upper.OperatingPoint - Lower.OperatingPoint)
```

Each of the six `PIDTuning` fields is interpolated with the same fraction. The
other `PIDParameters` fields are copied unchanged from `CommonParameters`.

`PIDGainScheduleOutput` contains `Parameters`, `FilteredOperatingPoint`,
`LowerIndex`, `UpperIndex`, `Fraction`, `Clamped`, and `Valid`. Before valid
configuration and initialization, `Valid` is `FALSE` and the retained last
output is returned unchanged.

The caller passes `Output.Parameters` to `PID.Configure` and then calls
`PID.Update`. Reconfiguration does not reset PID history. With integral action,
the existing incremental controller therefore avoids an artificial output
step solely from replacing a gain record. Positional P/PD control may change
output immediately, as already defined by Specification 32.

### 3.3 Limitations

The schedule is one-dimensional, bounded, and piecewise linear. It does not
extrapolate beyond validated endpoints, choose a model, or guarantee stability
between points. Qualification shall test the endpoints, every interval, and
the maximum expected rate of operating-point movement.

An application shall treat `Clamped=TRUE` as outside the qualified schedule,
not as permission to claim scheduled performance. It shall select one declared
fixed safe tuning, preserve the existing PID history while reconfiguring, and
expose that fallback state. A return into range may resume interpolation only
through the same filtered operating value. Qualification shall traverse every
breakpoint in both directions, cross both range boundaries, and prove bounded
MV movement during scheduled-to-fallback and fallback-to-scheduled transfer.

## 4. Static two-by-two decoupler

### 4.1 Purpose and model

For steady-state actuator deviations `du` and output deviations `dy`, the
identified gain relation is

```text
[dy1]   [g11 g12] [du1]
[dy2] = [g21 g22] [du2]
```

Two PIDs produce independent virtual process-effect commands `v`. The
decoupler calculates physical actuator commands

```text
u = ActuatorBias + inverse(G) * v
```

This cancels steady-state interaction when the matrix represents the plant.
`ToVirtual` performs the forward mapping

```text
v_applied = G * (u_applied - ActuatorBias)
```

so actual limited actuator commands can be returned to the PIDs through their
existing tracking inputs.

### 4.2 Datatypes and configuration

`ControlVector2` has named fields `First` and `Second`.
`ProcessGainMatrix2x2` has named fields `G11`, `G12`, `G21`, and `G22`.

`StaticDecoupler2x2Parameters` contains `GainMatrix`, `ActuatorBias`, and
`SingularityTolerance`. The tolerance shall be greater than zero.

Configuration calculates

```text
Determinant = g11*g22 - g12*g21
MatrixScale = max(abs(g11)+abs(g12), abs(g21)+abs(g22))
            * max(abs(g11)+abs(g21), abs(g12)+abs(g22))
RelativeDeterminant = abs(Determinant) / MatrixScale
```

`SingularityTolerance` is a dimensionless lower bound on
`RelativeDeterminant`, not an absolute determinant with unit-dependent scale.
It shall be greater than zero. Configuration succeeds only when
`MatrixScale>0` and `RelativeDeterminant>SingularityTolerance`. On success it
atomically retains the gain matrix, bias, determinant, and inverse. On failure
it returns `FALSE` and retains the previous valid configuration. Before a
successful configuration both mapping methods return a zero vector.

`StaticDecoupler2x2Status` contains `Configured`, `Determinant`, `Inverse11`,
`Inverse12`, `Inverse21`, and `Inverse22`. `Snapshot` returns this status.

### 4.3 Mapping behavior

For virtual command `v`, `ToActuators` returns:

```text
u1 = bias1 + inverse11*v1 + inverse12*v2
u2 = bias2 + inverse21*v1 + inverse22*v2
```

For an applied actuator command `u`, `ToVirtual` returns:

```text
v1 = g11*(u1-bias1) + g12*(u2-bias2)
v2 = g21*(u1-bias1) + g22*(u2-bias2)
```

The decoupler deliberately performs no limiting. Physical output and rate
limits remain explicit at the actuator boundary. When those limits bind, the
application maps the applied actuator values back with `ToVirtual` and uses the
result as tracking values for the two PIDs. Hidden clipping inside the
decoupler would prevent correct anti-windup.

### 4.4 Limitations

This is a steady-state decoupler. It is appropriate when the interaction paths
have sufficiently similar dynamics over the qualified bandwidth. It is not a
claim of dynamic decoupling, arbitrary MIMO stability, constraint optimization,
or robustness to a singular or changing gain matrix. Strong interaction with
materially different delays or poles requires a later dynamic decoupler,
state-space controller, or MPC specification.

## 5. PID parameter-form conversion

`PIDFormParameters.Form` determines the meaning of its numerical fields:

| Form | `Proportional` | `Integral` | `Derivative` |
|---|---|---|---|
| `Parallel` | `Kp` | `Ki` in 1/s | `Kd` in s |
| `Ideal` | `Kc` | `Ti` in s | `Td` in s |
| `Series` | `Kc` | `Ti` in s | `Td` in s |

Ideal form is `Kc(1 + 1/(Ti s) + Td s)`. Interacting Series form is
`Kc(1 + 1/(Ti s))(1 + Td s)`. Conversion first obtains equivalent Parallel
coefficients:

```text
Parallel: Kp = Kp,              Ki = Ki,      Kd = Kd
Ideal:    Kp = Kc,              Ki = Kc/Ti,   Kd = Kc*Td
Series:   Kp = Kc*(1 + Td/Ti), Ki = Kc/Ti,   Kd = Kc*Td
```

It then produces the requested target. For Ideal, `Kc=Kp`, `Ti=Kp/Ki`, and
`Td=Kd/Kp`. For Series, let `q=Kp^2-4*Ki*Kd`; a real interacting
representation exists only when `q>=0`. The non-cancelling root is
`Kc=(Kp+SIGN(Kp)*sqrt(q))/2`, followed by `Ti=Kc/Ki` and `Td=Kd/Kc`.

The qualified domain requires every input magnitude to be at most `1e100`, a
nonzero proportional/controller gain, a nonzero integral coefficient,
positive `Ti`, and nonnegative `Td`. Direct- and reverse-acting controllers are
both supported: Parallel `Kp`, `Ki`, and `Kd` shall have one common sign,
except that `Kd` may be zero. A target equal to the source still validates and
returns a normalized copy. `Valid` is returned only when the converted values
also satisfy the target domain. Invalid input returns `InvalidDomain`; a
negative Series discriminant returns `NonRepresentable`. On failure the
numeric result fields are zero. The function has no retained state and never
changes `Control.PID`.

## 6. Required real-world examples

Every advanced structure shall ship as a runnable truST project discoverable
through the existing VS Code example gallery. An example shall represent a
recognizable physical process, use engineering units, connect the controller
through ordinary `%Q` and `%I` process-image addresses, and run against the
existing plant simulator. A dimensionless academic transfer function alone is
not an acceptable product example.

Each advanced control project shall live in its numbered learner module at
`examples/control/<module-number>-<module-name>/<lesson-number>-<lesson-slug>/`,
with its stable `control-<lesson-slug>` identity and complete relative path
declared in the gallery manifest. The leaf shall remain a self-contained
runnable project; module directories contain no alternate controller
implementation or project configuration. Engineering lookup by technique,
plant problem, or workflow belongs in `examples/control/README.md`, not in a
second folder taxonomy.

Each example shall contain:

- a README explaining the equipment, manipulated variable, controlled
  variable, disturbance, and why the advanced structure is appropriate;
- controller Structured Text using the public `Control` library rather than a
  copied control equation;
- `simulation.toml` with declared units, operating point, limits, sample time,
  deterministic seed, uncertainty, and at least one realistic event;
- an ordinary PID baseline and an advanced-structure run under identical
  conditions;
- recorded SP, PV, MV, limits, status, actual cycle interval, and the internal
  advanced-structure signals needed to understand the result;
- stated acceptance metrics and generated results; and
- a short guided exercise plus an advanced engineering exercise.

### 6.1 Heated-air duct with downstream sensor

The Smith-predictor example represents an industrial heated-air duct. Heater
power in percent is the manipulated variable and downstream air temperature in
degrees Celsius is the controlled variable. Air transport from the heater to
the sensor creates material dead time; duct thermal mass supplies the first-
order lag. The scenario includes bounded heater power and slew rate, ambient or
inlet-temperature disturbance, sensor noise, and declared model mismatch.

The comparison shall show the same PI tuning with measured-PV feedback and
Smith-predicted feedback. It shall record the delay-free model, delayed model,
measurement correction, predictor active/history status, temperature response,
and heater demand. Acceptance requires a material reduction in settling time
or IAE without violating the same overshoot and heater constraints.

### 6.2 Gravity-drained water tank

The gain-scheduling example represents a tank filled by a variable-speed pump
and drained through a gravity outlet. Pump command in percent is the
manipulated variable and liquid level in centimetres is the controlled and
scheduling variable. Because outlet flow varies with the square root of level,
the effective process gain and time constant differ at low, middle, and high
levels.

The simulator shall use a declared nonlinear/state-space approximation or
bounded operating-region model supported by Specification 33; the README shall
state the approximation. The comparison shall use one fixed PI tuning and a
three-or-more-point validated schedule across low, middle, and high setpoints,
including transitions and a demand/outlet disturbance. It shall record the
filtered scheduling value, active segment, interpolation fraction, selected
gains, tank level, and pump command. Acceptance requires more consistent
settling/overshoot across regions without changing pump limits.

### 6.3 Temperature-and-humidity climate chamber

The decoupling example represents a climate chamber with two actuators and two
measurements. Heater power and humidifier command are the manipulated
variables; chamber temperature and relative humidity are the controlled
variables. Heating changes relative humidity, while humidification and its
evaporative effect change temperature, creating strong interaction.

The simulator shall use its two-input/two-output state-space plant with gains,
time constants, actuator limits, and sensor units documented as a bounded
linearization around one operating point. The comparison shall run the same two
PIDs first directly and then through `StaticDecoupler2x2`, with identical
temperature and humidity setpoint steps and a door-opening or inlet-air
disturbance. It shall record both virtual commands, both physical actuator
commands, both PVs, cross-coupling peaks, IAE, settling, and decoupler status.
Acceptance requires lower cross-coupling and no worse constraint compliance.

These examples are simulation models of real equipment, not evidence from real
equipment. Their documentation shall say so directly.

## 7. Native conformance and performance evidence

Native Structured Text tests shall prove:

- Ideal and Series conversion to Parallel, every reverse conversion, direct-
  and reverse-acting signs, unchanged same-form conversion, invalid-domain
  rejection, and nonrepresentable Series rejection;
- the Smith model equation, fractional dead-time interpolation, correction,
  zero-delay reduction, bypass, correction bound, and history-exhaustion
  behavior;
- gain-schedule endpoint clamping, interval selection, interpolation of every
  tuning field, common-policy preservation, smoothing, and rejection without
  partial apply; and
- decoupler inversion, biased forward/reverse mapping, round trip, singular
  rejection without partial apply, and unconfigured zero output.

The native tests use independent numerical values, not production functions,
to calculate expected results.

Executed closed-loop runtime benchmarks shall additionally compare:

1. the existing PI alone and the same PI using Smith prediction on the
   delay-dominant plant from Specification 36;
2. one fixed PID tuning and a validated schedule across at least three plant
   operating regions, including transitions; and
3. two independent PIDs with and without decoupling on a strongly coupled
   two-by-two plant.

Every comparison uses identical plant inputs, limits, sample intervals, and
metric definitions within the pair. Acceptance requires finite bounded traces,
respected actuator constraints, and a declared improvement in the metric the
structure is intended to address. A successful simulation remains simulation
evidence and shall not be presented as physical-plant proof.
