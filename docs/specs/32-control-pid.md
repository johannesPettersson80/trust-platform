# Control PID reference implementation

This specification defines the first truST implementation of the combined-form
PID controller and signal filter described in *A Practical Guide to PID
Controller Implementation*, version 3, 15 July 2026.

The implementation establishes the reference baseline before any
correction, safety extension, automatic tuning, model identification, or
alternative public controller design is introduced.

The algorithmic sources are:

- Listing 3, combined-form PID controller;
- Listing 4, second-order signal filter;
- Listing 5, combined integration anti-windup alternative;
- Sections 3.1 through 3.9, 4.1 through 4.10, and 5.1; and
- the released MATLAB and Python implementations at Git commit
  `b2b58fae03d00283988ad3a8007365085bf43391` of
  `https://github.com/copybit/PID.code`.

Where explanatory prose and executable listings disagree, the assignment order
and equations in Listings 3 through 5 define baseline behavior. The released
implementations confirm the interpretation of Listing 3. A later change to a
baseline behavior requires a separate written specification and an expected
failing native test.

IEC 61131-3 does not define a PID algorithm. This is therefore a truST library
contract, not an IEC standard function block and not an IEC deviation. Its
declarations use IEC 61131-3 Edition 3 Table 11 derived data types, Table 40
function blocks, and Tables 64 through 66 namespaces.

All public declarations belong to namespace `Control`. Applications therefore
use names such as `Control.PID` and `Control.LowPass`; declaration names do not
repeat the product name. All numeric values use `LREAL`. Durations are seconds
represented as `LREAL`, allowing each update to use its actual elapsed interval.

## 1. Complete public surface

### 1.1 Interfaces

The baseline defines no public interface. It contains one concrete reference
controller and one concrete reference filter.

### 1.2 Classes

The baseline defines no public class. Retained cyclic state belongs to function
block instances.

### 1.3 Function blocks

The baseline defines exactly two public function blocks.

| Function block | Purpose |
|---|---|
| `Control.PID` | Executes the paper's combined positional and incremental controller. |
| `Control.LowPass` | Executes the paper's independent first- or second-order signal filter. |

### 1.4 Standalone functions

The baseline defines no public standalone function. Controller and filter
calculations are methods of their respective stateful function blocks.

### 1.5 Enumerated datatypes

The baseline defines:

- `Control.PIDMode`;
- `Control.PIDAntiWindup`; and
- `Control.FilterOrder`.

### 1.6 Structured datatypes

The baseline defines:

- `Control.PIDParameters`;
- `Control.PIDInitialState`;
- `Control.PIDInput`; and
- `Control.LowPassParameters`.

There is no separate public P, PI, or PD function block in this baseline. The
paper's exact integral switch inside `Control.PID` selects the controller form.

## 2. Public datatypes

### 2.1 `PIDMode`

`PIDMode` selects one of the three operating modes implemented by Listing 3.

| Literal | Behavior |
|---|---|
| `Auto` | Calculate output from setpoint, process value, feedforward, parameters, and retained state. |
| `Manual` | Use `ManualOutput` as the nominal controller output before final limiting. |
| `Track` | Replace the retained nominal output with `TrackingOutput`, then execute the normal controller branch. |

Only these three literals belong to the baseline input domain. The paper
discusses a possible fourth Disabled mode in Section 5.1.3 but does not
implement it in Listing 3.

### 2.2 `PIDAntiWindup`

`PIDAntiWindup` selects between the two anti-windup equations supplied by the
paper. It is used only while `IntegralGain <> 0`.

| Literal | Behavior |
|---|---|
| `BackCalculation` | Apply Listing 3 line 24. Setting `TrackingTime = CycleTime` produces control-signal clamping. |
| `Combined` | Replace Listing 3 line 24 with all four lines of Listing 5. |

The enum is the mechanism by which one function block exposes the two
paper-defined alternatives. It does not change either equation.

### 2.3 `FilterOrder`

`FilterOrder` selects how many cascaded stages `LowPass` executes.

| Literal | Behavior |
|---|---|
| `FirstOrder` | Update and return the first stage. |
| `SecondOrder` | Update the first stage, then update and return the second stage. |

### 2.4 `PIDParameters`

`PIDParameters` contains one complete parameter set. `PID.Configure` copies the
record atomically so one update cannot observe a mixture of old and new
parameters, as required by Section 5.1.2.

| Field | Paper symbol | Meaning |
|---|---|---|
| `ProportionalGain` | `kp` | Proportional gain. |
| `IntegralGain` | `ki` | Integral gain per second. Exact zero selects positional form. |
| `DerivativeGain` | `kd` | Derivative gain in seconds. |
| `ProportionalSetpointWeight` | `b` | Setpoint weight used by the incremental proportional term. |
| `DerivativeSetpointWeight` | `c` | Setpoint weight used by derivative action. |
| `Bias` | `u0` | Positional-form output bias used when integral gain is zero. |
| `OutputMinimum` | `umin` | Absolute lower output limit. |
| `OutputMaximum` | `umax` | Absolute upper output limit. |
| `RateMinimum` | `dumin` | Most-negative allowed output rate per second. |
| `RateMaximum` | `dumax` | Most-positive allowed output rate per second. |
| `TrackingTime` | `Tt` | Back-calculation tracking time in seconds. |
| `AntiWindup` | — | Selects Listing 3 or Listing 5 anti-windup. |

The paper-defined operating domain requires:

```text
OutputMinimum <= OutputMaximum
RateMinimum <= 0 <= RateMaximum
TrackingTime > 0 when IntegralGain <> 0
```

The initial saturated output shall lie inside the absolute limits. Configured
values and input values shall support the arithmetic in this specification.
The paper does not define validation, rejection, fault, fallback, or clamping
behavior for invalid or nonfinite parameters; such behavior is outside the
baseline contract.

### 2.5 `PIDInitialState`

`PIDInitialState` supplies every retained controller state identified by
Section 5.1.1 and Appendix C. Initialization assigns these values directly and
does not execute a controller interval.

| Field | Paper state | Meaning |
|---|---|---|
| `Setpoint` | `xr` | Latest setpoint. |
| `SetpointRate` | `xdr` | Latest setpoint derivative. |
| `ProcessValue` | `xy` | Latest process value. |
| `ProcessValueRate` | `xdy` | Latest process-value derivative. |
| `FeedForward` | `xuff` | Latest feedforward contribution. |
| `NominalOutput` | `xu` | Latest unsaturated controller output. |
| `SaturatedOutput` | `xus` | Latest output after absolute and rate limits. |

A normal startup uses the latest available signals, zero rates, current
feedforward, and the actuator output expected at startup for both output
fields. The released examples initialize all values to zero unless an example
states otherwise.

### 2.6 `PIDInput`

`PIDInput` is one immutable controller sample.

| Field | Paper symbol | Meaning |
|---|---|---|
| `Setpoint` | `r` | Current setpoint. |
| `ProcessValue` | `y` | Current process value, already filtered when required. |
| `FeedForward` | `uff` | Current feedforward contribution in output units. |
| `ManualOutput` | `uman` | Nominal output requested in Manual mode. |
| `TrackingOutput` | `utrack` | Externally applied output used as the calculation base in Track mode. |
| `Mode` | `mode` | Current `PIDMode`. |
| `CycleTime` | `Dt` | Actual elapsed interval since the preceding update. |

`CycleTime` shall be greater than zero. Supplying the actual elapsed value
implements the paper's jitter compensation.

### 2.7 `LowPassParameters`

`LowPassParameters` defines one independent filter instance.

| Field | Paper symbol | Meaning |
|---|---|---|
| `TimeConstant` | `Tf` | Time constant of each filter stage in seconds. |
| `Order` | — | Selects one or two cascaded stages. |

The intended filtered operating region has `TimeConstant > 0`. The paper's
no-filter setting is `TimeConstant = CycleTime / 2`, which makes the update
coefficient equal to one.

## 3. `FUNCTION_BLOCK Control.PID`

`PID` owns the seven controller state values from `PIDInitialState`. It executes
the positional branch for P or PD control and the incremental branch for PI or
PID control exactly as selected by the paper's integral switch.

### 3.1 Public methods

| Method | Result |
|---|---|
| `Configure(Parameters : PIDParameters)` | None |
| `Initialize(State : PIDInitialState)` | None |
| `Update(Input : PIDInput)` | `LREAL` limited controller output |

### 3.2 Configuration

`Configure` atomically copies the complete parameter record. It does not alter
controller history. Parameter changes therefore retain the paper's natural
incremental behavior when integral action is active and its immediate
positional recalculation when integral action is absent.

### 3.3 Initialization

`Initialize` assigns all seven retained states from `PIDInitialState`. It
performs no derivative calculation, integration, anti-windup calculation,
rate limiting, saturation, or output calculation.

`Update` shall not be invoked before `Configure` and `Initialize` have been
invoked for that instance. This contract does not perform the released Python
and MATLAB implementations' implicit initialization.

### 3.4 Update order

Every `Update` executes the following stages in order:

1. calculate effective absolute and rate limits;
2. calculate setpoint and process-value derivatives;
3. preload nominal state when mode is Track;
4. select Manual or automatic calculation;
5. select positional or incremental form using exact comparison
   `IntegralGain = 0`;
6. apply the configured anti-windup equation in incremental form;
7. retain nominal output;
8. apply final saturation and rate limits;
9. retain the limited output and current signal histories; and
10. return the limited output.

Changing this order changes observable state and is nonconforming.

### 3.5 Effective output limits

Effective limits are calculated from the previous saturated output, never from
the previous nominal output:

```text
LowerLimit = MAX(
    OutputMinimum,
    PreviousSaturatedOutput
        + Input.CycleTime * RateMinimum)

UpperLimit = MIN(
    OutputMaximum,
    PreviousSaturatedOutput
        + Input.CycleTime * RateMaximum)
```

These equations are evaluated in Manual, Auto, and Track. Nonbinding rate
limits use sufficiently low and high configured values, or mathematical
negative and positive infinity where supported.

### 3.6 Derivative approximations

Before mode selection:

```text
SetpointRate =
    (Input.Setpoint - PreviousSetpoint)
        / Input.CycleTime

ProcessValueRate =
    (Input.ProcessValue - PreviousProcessValue)
        / Input.CycleTime
```

Both derivatives use the actual cycle time.

### 3.7 Track preloading

When `Input.Mode = Track`, the retained nominal output is overwritten before
the integral switch:

```text
PreviousNominalOutput = Input.TrackingOutput
```

Track is not a direct-output branch. Calculation continues through either the
positional or incremental controller equation.

### 3.8 Manual branch

When `Input.Mode = Manual`:

```text
NominalOutput = Input.ManualOutput
```

No proportional, integral, derivative, or feedforward term changes the nominal
Manual value. The nominal Manual value is retained before final limiting. The
absolute and rate limits still constrain the returned output. Current signal
and derivative histories are retained at the end of the call.

### 3.9 Integral switch

When mode is not Manual, the controller selects its form using exact comparison:

```text
IF IntegralGain = 0 THEN
    execute positional form
ELSE
    execute incremental form
END_IF
```

No controller-form enum or tolerance around zero is introduced.

### 3.10 Positional form without integral action

When `IntegralGain = 0`:

```text
NominalOutput =
      Bias
    + ProportionalGain
        * (Input.Setpoint - Input.ProcessValue)
    + DerivativeGain
        * (DerivativeSetpointWeight * SetpointRate
           - ProcessValueRate)
    + Input.FeedForward
```

`ProportionalSetpointWeight` is ignored; its effective value is one. A zero
derivative gain gives P control, while a nonzero derivative gain gives PD
control. No anti-windup equation is executed.

Because this equation does not consume the retained nominal output,
`TrackingOutput` has no effect when integral gain is zero. Track therefore
produces the same positional output as Auto. This is the observable behavior
of Listing 3 and both released implementations, although Section 4.3 also
states that tracking without integral action behaves like Manual.

### 3.11 Incremental form with integral action

When `IntegralGain <> 0`, signal differences are:

```text
SetpointDifference =
    Input.Setpoint - PreviousSetpoint

ProcessValueDifference =
    Input.ProcessValue - PreviousProcessValue

SetpointRateDifference =
    SetpointRate - PreviousSetpointRate

ProcessValueRateDifference =
    ProcessValueRate - PreviousProcessValueRate
```

The output increments are:

```text
ProportionalIncrement =
    ProportionalGain
        * (ProportionalSetpointWeight * SetpointDifference
           - ProcessValueDifference)

IntegralIncrement =
    IntegralGain
        * Input.CycleTime
        * (Input.Setpoint - Input.ProcessValue)

DerivativeIncrement =
    DerivativeGain
        * (DerivativeSetpointWeight * SetpointRateDifference
           - ProcessValueRateDifference)

FeedForwardIncrement =
    Input.FeedForward - PreviousFeedForward
```

The nominal candidate is:

```text
NominalOutput =
      PreviousNominalOutput
    + ProportionalIncrement
    + IntegralIncrement
    + DerivativeIncrement
    + FeedForwardIncrement
```

A zero derivative gain gives PI control, while a nonzero derivative gain gives
PID control. In Track, `PreviousNominalOutput` is the tracking value assigned
in Section 3.7, and all four current increments are still added.

### 3.12 Saturated candidate

The candidate used by either anti-windup equation is:

```text
SaturatedCandidate =
    MAX(MIN(NominalOutput, UpperLimit), LowerLimit)
```

### 3.13 Back-calculation anti-windup

When `AntiWindup = BackCalculation`, Listing 3 line 24 is applied exactly:

```text
NominalOutput =
    NominalOutput
    - Input.CycleTime / TrackingTime
        * (NominalOutput - SaturatedCandidate)
```

No coefficient bound is added:

- `TrackingTime = Input.CycleTime` moves nominal output directly to the
  saturated candidate and implements control-signal clamping; and
- when cycle time exceeds tracking time, the coefficient remains the value
  produced by the paper's equation even when it is greater than one.

### 3.14 Combined anti-windup

When `AntiWindup = Combined`, all of Listing 5 replaces the equation in Section
3.13. First, when the integral increment points farther into saturation:

```text
IF IntegralIncrement
       * (NominalOutput - SaturatedCandidate) > 0
THEN
    NominalOutput =
        NominalOutput
        - SIGN(NominalOutput - SaturatedCandidate)
            * MIN(
                ABS(IntegralIncrement),
                ABS(NominalOutput - SaturatedCandidate))
END_IF
```

Then use the saturated candidate already calculated before the conditional
correction, exactly as Listing 5 does:

```text
NominalOutput =
    NominalOutput
    - MIN(Input.CycleTime / TrackingTime, 1)
        * (NominalOutput - SaturatedCandidate)
```

This allows integration up to a limit, permits integration away from
saturation, and then back-calculates the remaining saturation error.

### 3.15 Final saturation and state commit

After Manual, positional, or incremental calculation:

1. retain `NominalOutput` as the new previous nominal output;
2. calculate:

   ```text
   Output = MAX(MIN(NominalOutput, UpperLimit), LowerLimit)
   ```

3. retain `Output` as the new previous saturated output;
4. retain current setpoint, setpoint rate, process value, process-value rate,
   and feedforward; and
5. return `Output`.

All seven controller histories are committed after every completed Manual,
Auto, or Track call.

## 4. `FUNCTION_BLOCK Control.LowPass`

`LowPass` implements the independent signal filter described by Section 3.9
and Listing 4. Each filtered process value, setpoint, feedforward, or other
signal requires its own instance and therefore its own retained stage values.

### 4.1 Public methods

| Method | Result |
|---|---|
| `Configure(Parameters : LowPassParameters)` | None |
| `Initialize(Input : LREAL)` | None |
| `Update(Input : LREAL, CycleTime : LREAL)` | `LREAL` filtered value |

### 4.2 Configuration and initialization

`Configure` atomically copies the filter parameters without resetting either
stage. Changing the time constant therefore continues from the retained
output. Changing order selects a different retained stage and is not specified
as bumpless.

`Initialize` assigns the current input to both retained stages. It executes no
filter interval and creates no startup transient.

### 4.3 Update equation

For each call:

```text
Coefficient =
    CycleTime / (TimeConstant + 0.5 * CycleTime)

Stage1 =
    PreviousStage1
    + Coefficient * (Input - PreviousStage1)
```

For `FirstOrder`, retain and return `Stage1`; the second stage remains
unchanged.

For `SecondOrder`, continue in the same call using the newly calculated first
stage:

```text
Stage2 =
    PreviousStage2
    + Coefficient * (Stage1 - PreviousStage2)
```

Retain both stages and return `Stage2`.

The coefficient is not clamped. `TimeConstant = CycleTime / 2` gives a
coefficient of one, causing either filter order to reproduce the current input
after one update.

## 5. Parameter and mode changes

- Configuration changes do not reset retained state.
- With integral action active, the next output adds current increments to the
  retained nominal output.
- With integral action absent, the next output is recalculated positionally and
  may jump after a mode or parameter change.
- Changing integral gain from nonzero to exact zero selects positional form on
  the next call.
- Changing integral gain from zero to nonzero selects incremental form on the
  next call and uses the nominal output retained by the preceding positional
  call.
- Manual-to-Auto transfer with integral action is bumpless when setpoint,
  process value, feedforward, parameters, and limiting conditions are
  unchanged, because Manual retained nominal output and current histories.
- Manual-to-Auto transfer without integral action is governed by the
  positional equation and is not guaranteed to be bumpless.
- Track-to-Auto transfer with integral action uses the nominal state established
  from the tracking value plus current increments.
- Track without integral action produces the same result as Auto.

## 6. Filtering boundary

`PID.Update` does not filter its own setpoint or process value. The caller passes
outputs from independent `LowPass` instances when filtering is required.

The process value should be filtered when noise would cause unacceptable
proportional or derivative output movement. A stepped setpoint should also be
filtered when nonzero derivative setpoint weight would otherwise create an
unacceptable derivative spike.

The filter may execute at a different interval from the controller. Each call
uses the actual cycle time supplied to that call.

## 7. Native conformance oracles

Native executable tests shall invoke the public methods above. Running the
released Python or MATLAB implementation is reference comparison, not a native
truST conformance test.

Numeric comparisons use absolute tolerance
`1e-12 * MAX(1, ABS(Expected))` unless a test deliberately targets a documented
`LREAL` rounding boundary. Limits are nonbinding unless explicitly stated.

### 7.1 Positional P/PD calculation

Configure:

```text
ProportionalGain = 2
IntegralGain = 0
DerivativeGain = 0.5
DerivativeSetpointWeight = 0.25
Bias = 1
OutputMinimum = -100
OutputMaximum = 100
RateMinimum = -1000
RateMaximum = 1000
```

Initialize all histories to zero. Auto updates shall return:

| Setpoint | ProcessValue | FeedForward | CycleTime | Output |
|---:|---:|---:|---:|---:|
| 1 | 0.2 | 0.3 | 0.1 | 3.15 |
| 1 | 0.4 | 0.3 | 0.2 | 2.0 |

The second call proves that derivative calculation uses actual cycle time and
previous accepted signals.

### 7.2 Incremental PI/PID calculation

Configure:

```text
ProportionalGain = 2
IntegralGain = 0.5
DerivativeGain = 0.1
ProportionalSetpointWeight = 0.5
DerivativeSetpointWeight = 0
TrackingTime = 1
AntiWindup = BackCalculation
```

Initialize all histories to zero. Auto updates shall return:

| Setpoint | ProcessValue | FeedForward | CycleTime | Output |
|---:|---:|---:|---:|---:|
| 1 | 0.0 | 0.2 | 0.1 | 1.25 |
| 1 | 0.2 | 0.3 | 0.1 | 0.79 |

The second result adds proportional increment `-0.4`, integral increment
`0.04`, derivative increment `-0.2`, and feedforward increment `0.1` to the
retained nominal output `1.25`.

Repeating the sequence with derivative gain zero shall return `1.25` and
`0.99`, proving PI behavior uses the same incremental equation without a
derivative contribution.

### 7.3 Integral-form switching

One continuous native sequence shall:

1. execute with nonzero integral gain;
2. configure integral gain to exact zero and prove that the next call uses the
   positional equation and Bias;
3. leave signals unchanged, restore nonzero integral gain, and prove that the
   next call uses the nominal output retained by the positional call; and
4. change proportional gain while integral action remains enabled and prove
   that no positional recomputation is introduced.

This test establishes the combined-form switch that is central to Listing 3.

### 7.4 Manual output, rate limiting, and nominal history

Configure zero proportional and derivative gains, integral gain `1`, output
limits `[0, 10]`, rate limits `[-2, 3]`, tracking time `0.5`, and
BackCalculation. Initialize nominal and saturated outputs to `5` and other
histories to zero.

1. A Manual update requesting `9` with cycle time `0.5` shall return `6.5`.
2. A following Auto update with zero setpoint and process value and the same
   cycle time shall return `8.0`.

The second result proves that Manual retained nominal value `9`, while the next
rate interval used previous saturated output `6.5`.

### 7.5 Tracking with integral action

Configure proportional gain `2`, integral gain `1`, derivative gain zero,
proportional setpoint weight `0.5`, and nonbinding limits. Initialize all
histories to zero.

A Track update with setpoint `1`, process value `0`, tracking output `7`, and
cycle time `0.1` shall return `8.1`. The tracking value becomes the calculation
base, then proportional increment `1` and integral increment `0.1` are added.

### 7.6 Tracking without integral action

Configure proportional gain `2`, integral and derivative gains zero, Bias `1`,
and nonbinding limits. Initialize all histories to zero.

A Track update with setpoint `1`, process value `0`, and tracking output `7`
shall return `3`, not `7`. This locks the Listing 3 and released-code behavior
before any later correction of non-integral tracking.

### 7.7 Back calculation and control-signal clamping

Configure zero proportional and derivative gains, integral gain `4`, output
limits `[0, 3]`, nonbinding rate limits, and initialize all histories to zero.

With BackCalculation and tracking time `2`:

| Setpoint | ProcessValue | CycleTime | Output |
|---:|---:|---:|---:|
| 1 | 0 | 1 | 3 |
| -0.125 | 0 | 1 | 3 |

With tracking time equal to cycle time `1`, the same calls shall return `3` and
`2.5`. The second result distinguishes retained back-calculated nominal state
`3.5` from clamped nominal state `3`.

### 7.8 Combined anti-windup

Configure zero proportional and derivative gains, integral gain `1`, output
limits `[0, 3]`, tracking time `2`, and Combined anti-windup. Initialize nominal
output to `4`, saturated output to `3`, and other histories to zero.

| Setpoint | ProcessValue | CycleTime | Output |
|---:|---:|---:|---:|
| 1 | 0 | 1 | 3 |
| -0.75 | 0 | 1 | 2.75 |

After the first call, conditional correction reduces the candidate from `5` to
`4`, then bounded back calculation retains nominal output `3.5`. The second
call proves integration away from saturation is permitted.

### 7.9 Feedforward

Native tests shall prove:

- with integral gain zero, the complete current feedforward value is added by
  the positional equation;
- with integral gain nonzero, only current feedforward minus previous
  feedforward is added;
- Manual mode does not add feedforward; and
- current feedforward history is retained after Manual.

### 7.10 Setpoint weights

Native tests shall prove:

- proportional setpoint weight affects the proportional increment only while
  integral gain is nonzero;
- positional form always uses effective proportional weight one;
- derivative setpoint weight affects both positional derivative action and the
  incremental derivative contribution; and
- neither weight alters the process-value part of its term.

### 7.11 Filter calculations

Initialize both stages to `2`, use input `4`, cycle time `1`, and time constant
`1.5`. The coefficient is `0.5`.

- FirstOrder shall return `3.0`.
- SecondOrder shall return `2.5` on the first call and `3.0` on a second call
  with the same input and cycle time.
- With time constant equal to half the cycle time, both orders shall return the
  current input after one update.

### 7.12 Independent instances

Two independently initialized PID instances shall be updated with different
input and parameter sequences in an interleaved order. Each output shall equal
its output when executed alone. The same test applies to two `LowPass`
instances. This proves all retained history is instance-local.

### 7.13 Released scenario equivalence

Native integration tests shall reproduce the controller and filter input
sequences from the seven released paper examples:

1. Manual-to-Auto transfer;
2. PI-to-P-to-PI switching and rate limitation;
3. proportional setpoint weighting;
4. feedforward, saturation, clamping, and back calculation;
5. first- and second-order measurement filtering;
6. gain scheduling with tracking; and
7. selector control with tracking.

Selected pre-transition, transition, saturation-entry, saturation-exit, and
settled outputs shall match the equations in this specification. Executing
Python or MATLAB from the test runner does not constitute a native test.

## 8. Baseline boundary

The following behaviors are not supplied by this specification because they
are not implemented by Listings 3 through 5:

- separate public P, PI, PD, and PID types;
- Disabled, Hold, Safe, or Fault modes;
- input-quality handling and safe-output policy;
- validation and rejection of invalid or nonfinite values;
- repaired tracking without integral action;
- bumpless positional parameter or mode changes;
- bounded Listing 3 back calculation when cycle time exceeds tracking time;
- downstream applied-output feedback;
- persistent-state serialization;
- process identification, automatic tuning, or performance monitoring; and
- cascade, ratio, split-range, Smith-predictor, or actuator blocks.

Each addition or correction requires its own written specification and native
test after this reference baseline has passing native conformance tests.
