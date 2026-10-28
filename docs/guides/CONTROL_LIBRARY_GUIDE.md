# Control Library Guide

The Control library is truST's PID controller and what goes around it. `PID` is one block for P,
PI, PD and PID control, with Manual, Auto and Track modes, output and rate limits and two
anti-windup methods. `LowPass` filters a measurement or a setpoint. Three optional structures help
with harder plants: a Smith predictor for dead time, a gain scheduler for a plant that changes
with its operating point, and a static decoupler for two loops that disturb each other. A function
converts PID settings written in another vendor's form.

`PID` follows *A Practical Guide to PID Controller Implementation* exactly. The contracts are
specification 45 (`docs/specs/45-control-pid.md`) for the PID and the filter and specification 37
(`docs/specs/37-control-advanced-structures.md`) for the structures. The design reasons and the
measured results are in the Control System Engineering report
(`docs/guides/CONTROL_PID_ENGINEERING_REPORT.md`).

Use the library for any feedback loop in a program. The library runs a loop; it does not tune it:
Control Studio identifies the plant and tunes the loop. For a loop that an operator runs from the
HMI, the HMI Builder's Create PLC code writes this same `PID` as ordinary code with the variables
the Plant library's PID panel operates: the operator's mode, setpoint and manual output, and one
parameter record that the HMI's settings and Control Studio share.

## Package Setup

```toml
[project]
include_paths = ["src"]

[dependencies]
Control = { path = "../../libraries/control", version = "0.1.0" }
```

The library depends on nothing else. Its names are in the namespace `Control`: write
`USING Control;` and use `PID`, or write `Control.PID` in full. Write enumeration values with their
type, for example `PIDMode#Auto`.

## Design Rules

- One instance per loop and one per filtered signal; each keeps its own history.
- Call `Configure` with the complete parameter record, `Initialize` once at startup, then `Update`
  once in every scan. Never call `Update` before both `Configure` and `Initialize`.
- `Configure` copies the whole record at once and keeps the history. Calling it in every scan
  before `Update` is cheap and makes a changed record, such as a new tuning, take effect in the
  next scan.
- `Initialize` sets the history directly and calculates nothing. Start from the current setpoint
  and measurement, zero rates, the current feedforward, and the actuator's output at startup for
  both output fields.
- Pass the actual time since the previous `Update`, in seconds and greater than zero, as
  `CycleTime`. The derivative, the integral, the rate limits, the anti-windup and the filter all
  use it, so the loop stays correct when the task's timing varies.
- P, PI, PD and PID are settings of the one `PID`. An integral gain of exactly zero selects the
  positional form, P or PD, whose output at zero error is `Bias`. Any other integral gain selects
  the incremental form, PI or PID, which adds each scan's change to the previous output.
- The PID never filters its inputs. Filter a noisy measurement with its own `LowPass` before the
  PID, and a stepped setpoint too when the derivative uses the setpoint.
- The limits act in every mode, and the anti-windup keeps the integral from running away while
  the output is limited.
- The library checks no values (specification 45, §8). Keep the parameters inside the ranges below
  and the inputs finite, and handle a failed signal and the safe output in your program.
- The structures never change the PID. They prepare its measurement (Smith predictor) or its
  parameter record (gain scheduler), or turn its outputs into actuator commands (decoupler). Their
  `Configure` returns FALSE and keeps the previous valid settings when a value is out of range.

## Blocks

| Block | Use | Methods |
| --- | --- | --- |
| `PID` | P, PI, PD or PID loop with modes, limits and anti-windup | `Configure`, `Initialize`, `Update`, `GetState` |
| `LowPass` | first- or second-order filter for one signal | `Configure`, `Initialize`, `Update` |
| `SmithPredictorFOPDT` | gives the PID a measurement predicted past the plant's dead time | `Configure`, `Initialize`, `Update` |
| `PIDGainScheduler` | interpolates the PID's tuning over the plant's operating point | `Configure`, `Initialize`, `Update` |
| `StaticDecoupler2x2` | turns two PID outputs into two actuator commands that cancel the loops' interaction | `Configure`, `ToActuators`, `ToVirtual`, `Snapshot` |

| Function | Use |
| --- | --- |
| `ConvertPIDForm` | converts PID settings between the parallel, ideal and series forms |

## Common Types

### Enumerations

| Type | Values |
| --- | --- |
| `PIDMode` | `Auto := 0`: calculate from setpoint and measurement. `Manual := 1`: the output is `ManualOutput`, still limited. `Track := 2`: restart the calculation from `TrackingOutput` in every scan |
| `PIDAntiWindup` | `BackCalculation := 0`, `Combined := 1` |
| `FilterOrder` | `FirstOrder := 0`, `SecondOrder := 1`: one stage or two in a row |
| `PIDParameterForm` | `Parallel := 0`, `Ideal := 1`, `Series := 2` |
| `PIDFormConversionStatus` | `Valid := 0`, `InvalidDomain := 1`, `NonRepresentable := 2` |

### `PIDParameters`

One complete parameter set. "Output" means the output's unit, for example %; "PV" means the
measurement's unit, for example °C.

| Field | Unit | Meaning |
| --- | --- | --- |
| `ProportionalGain` | output/PV | Kp: output change per unit of error, error = setpoint − measurement. Make all gains negative when more output lowers the measurement, as for cooling or a pump that drains |
| `IntegralGain` | output/(PV·s) | Ki = Kp/Ti, where the integral time Ti is how long a constant error takes for the integral to add as much as the proportional part. Exactly 0 selects P or PD |
| `DerivativeGain` | output·s/PV | Kd = Kp·Td, where the derivative time Td is how far ahead the derivative acts on the error's trend |
| `ProportionalSetpointWeight` | — | b, incremental form only: the share of a setpoint change the proportional part sees. 1 reacts fully; 0 lets only the integral follow, so a setpoint step gives no output kick |
| `DerivativeSetpointWeight` | — | c: the share of the setpoint's rate the derivative sees. 0 lets the derivative act on the measurement only |
| `Bias` | output | u0: the output at zero error in the positional form; unused with integral action |
| `OutputMinimum`, `OutputMaximum` | output | the output's absolute limits; `OutputMinimum <= OutputMaximum` |
| `RateMinimum`, `RateMaximum` | output/s | the fastest the output may fall and rise; `RateMinimum <= 0 <= RateMaximum`. Very large values switch rate limiting off |
| `TrackingTime` | s | Tt: how fast the anti-windup pulls the integral back while the output is limited; above 0 whenever `IntegralGain <> 0`. Equal to the cycle time, it sets the stored output to the limit at once (clamping); a value between Td and Ti is a common start |
| `AntiWindup` | — | `BackCalculation` or `Combined`, see below |

Wind-up: while the output is at a limit, the error persists and the integral keeps growing, so
the output stays at the limit long after the error has reversed. `BackCalculation` pulls the
stored output back towards the limit, faster the shorter `TrackingTime`. `Combined` first stops
any integration that pushes further into the limit, while integration away from it continues,
then pulls back the rest in the same way, at most to the limit in one scan.

### `PID` records

| Type | Fields |
| --- | --- |
| `PIDInput` | `Setpoint`, `ProcessValue` (the measurement, already filtered), `FeedForward` (a known disturbance's effect, in output units, added to the output), `ManualOutput`, `TrackingOutput`, `Mode`, `CycleTime` (s) |
| `PIDInitialState` | `Setpoint`, `SetpointRate`, `ProcessValue`, `ProcessValueRate`, `FeedForward`, `NominalOutput` (the stored output before limiting), `SaturatedOutput` (after limiting) |
| `LowPassParameters` | `TimeConstant` (s, of each stage), `Order` |

### Structure records

| Type | Fields |
| --- | --- |
| `SmithPredictorParameters` | `ProcessGain`, `TimeConstant` (s, above 0), `DeadTime` (s, 0 or more), `OperatingInput`, `OperatingOutput`, `MaximumCorrection` (0 or less: unbounded) |
| `SmithPredictorInitialState` | `ModelOutput`, `ProcessValue` |
| `SmithPredictorInput` | `ManipulatedValue` (the command actually sent), `ProcessValue`, `CycleTime`, `Enabled` |
| `SmithPredictorOutput` | `PredictedProcessValue`, `DelayFreeModel`, `DelayedModel`, `Correction`, `Active`, `HistoryComplete` |
| `PIDTuning` | the six tuning fields of `PIDParameters`: `ProportionalGain`, `IntegralGain`, `DerivativeGain`, `ProportionalSetpointWeight`, `DerivativeSetpointWeight`, `TrackingTime` |
| `PIDGainSchedulePoint` | `OperatingPoint`, `Tuning` |
| `PIDGainSchedulePoints` | `ARRAY[0..7] OF PIDGainSchedulePoint` |
| `PIDGainScheduleParameters` | `Points` (rising operating points), `PointCount` (2 to 8), `CommonParameters` (bias, limits and anti-windup for every point; its tuning fields are ignored), `FilterTimeConstant` (s; 0 for none) |
| `PIDGainScheduleOutput` | `Parameters`, `FilteredOperatingPoint`, `LowerIndex`, `UpperIndex`, `Fraction`, `Clamped`, `Valid` |
| `ControlVector2` | `First`, `Second` |
| `ProcessGainMatrix2x2` | `G11`, `G12`, `G21`, `G22`: the steady-state change of measurement 1 or 2 per change of actuator 1 or 2 |
| `StaticDecoupler2x2Parameters` | `GainMatrix`, `ActuatorBias` (the actuators' operating point), `SingularityTolerance` (above 0) |
| `StaticDecoupler2x2Status` | `Configured`, `Determinant`, `Inverse11`, `Inverse12`, `Inverse21`, `Inverse22` |
| `PIDFormParameters` | `Form`, `Proportional`, `Integral`, `Derivative` |
| `PIDFormConversionResult` | `Parameters`, `Status` |

## Interface Summary

```st
PID.Configure(Parameters : PIDParameters)
PID.Initialize(State : PIDInitialState)
PID.Update(Input : PIDInput) : LREAL                    the limited output
PID.GetState() : PIDInitialState                        a copy of the history; changes nothing
LowPass.Configure(Parameters : LowPassParameters)
LowPass.Initialize(Input : LREAL)                       both stages start at Input
LowPass.Update(Input : LREAL, CycleTime : LREAL) : LREAL
SmithPredictorFOPDT.Configure(Parameters : SmithPredictorParameters) : BOOL
SmithPredictorFOPDT.Initialize(State : SmithPredictorInitialState)
SmithPredictorFOPDT.Update(Input : SmithPredictorInput) : SmithPredictorOutput
PIDGainScheduler.Configure(Parameters : PIDGainScheduleParameters) : BOOL
PIDGainScheduler.Initialize(OperatingPoint : LREAL)
PIDGainScheduler.Update(OperatingPoint : LREAL, CycleTime : LREAL) : PIDGainScheduleOutput
StaticDecoupler2x2.Configure(Parameters : StaticDecoupler2x2Parameters) : BOOL
StaticDecoupler2x2.ToActuators(VirtualCommand : ControlVector2) : ControlVector2
StaticDecoupler2x2.ToVirtual(ActuatorCommand : ControlVector2) : ControlVector2
StaticDecoupler2x2.Snapshot() : StaticDecoupler2x2Status
ConvertPIDForm(Input : PIDFormParameters, Target : PIDParameterForm) : PIDFormConversionResult
```

### `PID`

Each `Update`, in this order:

1. The scan's limits: the absolute limits, narrowed by the rate limits around the previous output.
2. The rates of the setpoint and of the measurement.
3. In Track, the stored output is replaced by `TrackingOutput`.
4. In Manual, the output is `ManualOutput`. Otherwise the positional form (P, PD) or the
   incremental form (PI, PID) calculates it; the incremental form adds the scan's proportional,
   integral, derivative and feedforward changes to the stored output, then applies the
   anti-windup.
5. The output is limited, stored with the scan's signals, and returned.

Changes of mode and parameters:

- With integral action, Manual to Auto does not jump: in Manual the PID keeps its history, so Auto
  continues from the manual output. Track to Auto continues from the tracked output.
- Without integral action, the output is recalculated at once and may jump at a change of mode or
  parameters, and Track gives the same output as Auto.
- A new parameter record never resets the history; with integral action, new gains alone do not
  make the output jump.
- `GetState().NominalOutput` is the stored output after the anti-windup, not the request before
  limiting.

The observation extension, beside `GetState`: every `Update` ends by setting two internal members,
`ModeValue : PIDMode` to the mode the call ran in and `ModeObserved : BOOL` to TRUE
(specification 45, §3.1). Tools read them; Control Studio confirms Manual mode from them before it
changes a controller's parameters. The members are `NON_RETAIN`: after a restart `ModeObserved` is
FALSE until the first `Update`, also in an instance declared `RETAIN`, and an online change keeps
them until the next `Update`. They change nothing the controller calculates.

The simulation extension: a `PID` also has two internal members, `HeldMeasurement : LREAL` and
`MeasurementHeld : BOOL`, which only a simulated plant writes, through a `pid:` feedback target
that names the instance (specification 46, §13.2). While a measurement is held, every `Update`
works with it as `Input.ProcessValue`; the program's own variables keep what the program wrote, and
`StateValue.ProcessValue` shows the value the controller used. Without such a plant
`MeasurementHeld` stays FALSE and `Update` computes exactly as described above. The members are
`NON_RETAIN`: a restart removes a held measurement, also in an instance declared `RETAIN`, and so
does a newly loaded program, until the plant publishes again.

### `LowPass`

Each stage moves towards its input by `CycleTime / (TimeConstant + CycleTime / 2)` of the
difference; the second order puts two such stages in a row. A filter delays the signal, and delay
makes a loop harder to control, so use the smallest time constant that quiets the output.
`TimeConstant` equal to half the cycle time passes the signal through unchanged. A changed time
constant continues from the stored value.

### `SmithPredictorFOPDT`

Dead time is the delay between a change of the output and the first response of the measurement,
for example while air travels along a duct to the sensor. A PID cannot see the effect of its output
during that time and must be tuned slowly. The predictor runs a model of the plant with a gain, a
time constant and a dead time (first order plus dead time, FOPDT) and gives the PID what the
measurement would be now without the delay, corrected by the measured difference between plant and
model.

In every scan, update it with the command actually sent to the actuator and with the measurement,
and pass `PredictedProcessValue` to the PID as its `ProcessValue`. When `Active` is FALSE it passes
the measurement through: it is not configured or initialized, `Enabled` is FALSE, or its 256 stored
samples do not reach back over the dead time (`HistoryComplete` FALSE; keep `DeadTime` below 255
cycle times). Use the plain measurement whenever the history is incomplete, the measurement is bad,
the plant is outside the model's range, or `Correction` sits at `MaximumCorrection`: a correction at
its bound means the model no longer fits.

### `PIDGainScheduler`

A plant whose gain or speed changes with its operating point needs a different tuning at different
points. The scheduler holds up to eight tunings, each validated at its operating point, and
interpolates in a straight line between neighbouring points, on an operating point filtered with
`FilterTimeConstant`. Below the first point and above the last it keeps the end tuning and reports
`Clamped`.

In every scan, pass `Parameters` of its output to `PID.Configure` before `PID.Update`. When
`Clamped` is TRUE or `Valid` is FALSE, configure a fixed safe tuning instead and show that the loop
is outside its schedule. The PID keeps its history on every reconfiguration.

### `StaticDecoupler2x2`

Two loops interact when each actuator moves both measurements: heating a climate chamber lowers its
humidity, and an evaporative humidifier cools it. From the steady-state gains of both actuators on both
measurements, the decoupler turns the two PID outputs, the virtual commands, into actuator commands
that cancel the interaction in steady state.

Pass the two PID outputs to `ToActuators` and limit its result at the actuators; the decoupler
limits nothing itself. When an actuator limit binds, map the applied commands back with `ToVirtual`
and run both PIDs in Track with those values as `TrackingOutput`, so neither winds up. `Configure`
refuses a matrix that is nearly singular, one whose determinant relative to the size of its gains
is not above `SingularityTolerance`. It cancels steady-state interaction only; loops whose
interaction paths have very different speeds need more than this.

### `ConvertPIDForm`

Controllers state their PID settings in different forms, with error e and output u:

- parallel, as `PID` uses: u = Kp·e + Ki·∫e dt + Kd·de/dt;
- ideal: u = Kc·(e + (1/Ti)·∫e dt + Td·de/dt);
- series, or interacting: an ideal PI part multiplied by a PD part, as in many older controllers;
  in parallel form Kp = Kc·(1 + Td/Ti), Ki = Kc/Ti, Kd = Kc·Td.

`Valid` returns the converted values. `InvalidDomain` means the input is out of range: a zero proportional or
integral gain, Ti of 0 or less, Td below 0, gains of different signs, or a value that is not finite
or larger than 1e100.
`NonRepresentable` means the parallel values have no series form. On failure the values are 0.
Check the old controller's units first: some state Ti and Td in minutes, or the integral in
repeats per minute.

## Examples

These are short teaching examples, each showing one point, with a task that runs every 100 ms
(`CycleTime := 0.1`); a task whose interval varies passes the measured time instead. Record fields
left out of an initializer keep their default, 0 for numbers. The complete, runnable project, with its I/O bindings, simulated
plant, modes and the steps for adapting it to another plant, is `examples/control-pid-commissioning`;
the control course in `examples/control/` teaches the controller lesson by lesson.

A PI loop holds a temperature with a heater:

```iecst
USING Control;

PROGRAM Heating
VAR
    TemperatureC AT %IL0 : LREAL;
    HeaterPct AT %QL0 : LREAL;
    SetpointC : LREAL := 60.0;
    Loop : PID;
    Parameters : PIDParameters := (
        ProportionalGain := 4.0,
        IntegralGain := 0.2,
        DerivativeGain := 0.0,
        ProportionalSetpointWeight := 1.0,
        DerivativeSetpointWeight := 0.0,
        Bias := 0.0,
        OutputMinimum := 0.0,
        OutputMaximum := 100.0,
        RateMinimum := -1000.0,
        RateMaximum := 1000.0,
        TrackingTime := 10.0,
        AntiWindup := PIDAntiWindup#BackCalculation
    );
    Start : PIDInitialState;
    Input : PIDInput;
    Initialized : BOOL;
END_VAR
    Loop.Configure(Parameters := Parameters);
    IF NOT Initialized THEN
        Start.Setpoint := SetpointC;
        Start.ProcessValue := TemperatureC;
        Start.NominalOutput := HeaterPct;
        Start.SaturatedOutput := HeaterPct;
        Loop.Initialize(State := Start);
        Initialized := TRUE;
    END_IF;
    Input.Setpoint := SetpointC;
    Input.ProcessValue := TemperatureC;
    Input.Mode := PIDMode#Auto;
    Input.CycleTime := 0.1;
    HeaterPct := Loop.Update(Input := Input);
END_PROGRAM
```

The operator chooses Manual or Auto and sets the manual output on the HMI (`AutoMode`,
`ManualPct`). The PID runs in both modes. In Auto the manual value follows the output, so a switch to
Manual holds the valve where it is; in Manual the PID keeps its history, so the switch back to Auto
continues from the manual output. The rate limits of 10 %/s also slow the operator's changes:

```iecst
USING Control;

PROGRAM Flow
VAR
    FlowM3h AT %IL0 : LREAL;
    ValvePct AT %QL0 : LREAL;
    AutoMode : BOOL;
    ManualPct : LREAL;
    SetpointM3h : LREAL := 12.0;
    Loop : PID;
    Parameters : PIDParameters := (
        ProportionalGain := 0.8,
        IntegralGain := 0.4,
        ProportionalSetpointWeight := 1.0,
        OutputMinimum := 0.0,
        OutputMaximum := 100.0,
        RateMinimum := -10.0,
        RateMaximum := 10.0,
        TrackingTime := 1.0,
        AntiWindup := PIDAntiWindup#Combined
    );
    Start : PIDInitialState;
    Input : PIDInput;
    Initialized : BOOL;
END_VAR
    Loop.Configure(Parameters := Parameters);
    IF NOT Initialized THEN
        Start.Setpoint := SetpointM3h;
        Start.ProcessValue := FlowM3h;
        Start.NominalOutput := ValvePct;
        Start.SaturatedOutput := ValvePct;
        Loop.Initialize(State := Start);
        Initialized := TRUE;
    END_IF;
    IF AutoMode THEN
        Input.Mode := PIDMode#Auto;
        ManualPct := ValvePct;
    ELSE
        Input.Mode := PIDMode#Manual;
    END_IF;
    Input.Setpoint := SetpointM3h;
    Input.ProcessValue := FlowM3h;
    Input.ManualOutput := ManualPct;
    Input.CycleTime := 0.1;
    ValvePct := Loop.Update(Input := Input);
END_PROGRAM
```

An interlock closes the steam valve of a heat exchanger while the product pump is stopped. The
program then sends 0 % to the valve and runs the PID in Track with that value as `TrackingOutput`:
in every scan the PID restarts its calculation from what the valve really gets, so it does not wind
up, and when the pump runs again it continues from the closed valve. Use the same pattern whenever
something else drives the actuator, such as a hand station, an override selector or a sequence.
Track needs integral action:

```iecst
USING Control;

PROGRAM Exchanger
VAR
    ProductC AT %IL0 : LREAL;
    PumpRunning AT %IX0.0 : BOOL;
    SteamValvePct AT %QL0 : LREAL;
    SetpointC : LREAL := 72.0;
    Loop : PID;
    Parameters : PIDParameters := (
        ProportionalGain := 3.0,
        IntegralGain := 0.1,
        ProportionalSetpointWeight := 1.0,
        OutputMinimum := 0.0,
        OutputMaximum := 100.0,
        RateMinimum := -1000.0,
        RateMaximum := 1000.0,
        TrackingTime := 15.0,
        AntiWindup := PIDAntiWindup#BackCalculation
    );
    Start : PIDInitialState;
    Input : PIDInput;
    Request : LREAL;
    Initialized : BOOL;
END_VAR
    Loop.Configure(Parameters := Parameters);
    IF NOT Initialized THEN
        Start.Setpoint := SetpointC;
        Start.ProcessValue := ProductC;
        Loop.Initialize(State := Start);
        Initialized := TRUE;
    END_IF;
    Input.Setpoint := SetpointC;
    Input.ProcessValue := ProductC;
    IF PumpRunning THEN
        Input.Mode := PIDMode#Auto;
    ELSE
        Input.Mode := PIDMode#Track;
        Input.TrackingOutput := 0.0;
    END_IF;
    Input.CycleTime := 0.1;
    Request := Loop.Update(Input := Input);
    IF PumpRunning THEN
        SteamValvePct := Request;
    ELSE
        SteamValvePct := 0.0;
    END_IF;
END_PROGRAM
```

A noisy level measurement is smoothed by a second-order `LowPass` with 2 s per stage before it
reaches the PID:

```iecst
USING Control;

PROGRAM Level
VAR
    LevelM AT %IL0 : LREAL;
    PumpPct AT %QL0 : LREAL;
    SetpointM : LREAL := 1.5;
    Smoothing : LowPass;
    FilterSettings : LowPassParameters := (TimeConstant := 2.0, Order := FilterOrder#SecondOrder);
    LevelFilteredM : LREAL;
    Loop : PID;
    Parameters : PIDParameters := (
        ProportionalGain := 40.0,
        IntegralGain := 2.0,
        ProportionalSetpointWeight := 1.0,
        OutputMinimum := 0.0,
        OutputMaximum := 100.0,
        RateMinimum := -1000.0,
        RateMaximum := 1000.0,
        TrackingTime := 10.0,
        AntiWindup := PIDAntiWindup#BackCalculation
    );
    Start : PIDInitialState;
    Input : PIDInput;
    Initialized : BOOL;
END_VAR
    Smoothing.Configure(Parameters := FilterSettings);
    Loop.Configure(Parameters := Parameters);
    IF NOT Initialized THEN
        Smoothing.Initialize(Input := LevelM);
        Start.Setpoint := SetpointM;
        Start.ProcessValue := LevelM;
        Loop.Initialize(State := Start);
        Initialized := TRUE;
    END_IF;
    LevelFilteredM := Smoothing.Update(Input := LevelM, CycleTime := 0.1);
    Input.Setpoint := SetpointM;
    Input.ProcessValue := LevelFilteredM;
    Input.Mode := PIDMode#Auto;
    Input.CycleTime := 0.1;
    PumpPct := Loop.Update(Input := Input);
END_PROGRAM
```

A loop moves from an old controller whose settings are in ideal form (Kc 2.5, Ti 120 s, Td 10 s).
The program converts them once and stays in Manual if they are not valid:

```iecst
USING Control;

PROGRAM Migrated
VAR
    TemperatureC AT %IL0 : LREAL;
    HeaterPct AT %QL0 : LREAL;
    SetpointC : LREAL := 80.0;
    OldSettings : PIDFormParameters := (
        Form := PIDParameterForm#Ideal,
        Proportional := 2.5,
        Integral := 120.0,
        Derivative := 10.0
    );
    Converted : PIDFormConversionResult;
    SettingsValid : BOOL;
    Loop : PID;
    Parameters : PIDParameters := (
        ProportionalSetpointWeight := 1.0,
        OutputMinimum := 0.0,
        OutputMaximum := 100.0,
        RateMinimum := -1000.0,
        RateMaximum := 1000.0,
        TrackingTime := 30.0,
        AntiWindup := PIDAntiWindup#BackCalculation
    );
    Start : PIDInitialState;
    Input : PIDInput;
    Initialized : BOOL;
END_VAR
    IF NOT Initialized THEN
        Converted := ConvertPIDForm(Input := OldSettings, Target := PIDParameterForm#Parallel);
        SettingsValid := Converted.Status = PIDFormConversionStatus#Valid;
        Parameters.ProportionalGain := Converted.Parameters.Proportional;
        Parameters.IntegralGain := Converted.Parameters.Integral;
        Parameters.DerivativeGain := Converted.Parameters.Derivative;
    END_IF;
    Loop.Configure(Parameters := Parameters);
    IF NOT Initialized THEN
        Start.Setpoint := SetpointC;
        Start.ProcessValue := TemperatureC;
        Loop.Initialize(State := Start);
        Initialized := TRUE;
    END_IF;
    Input.Setpoint := SetpointC;
    Input.ProcessValue := TemperatureC;
    IF SettingsValid THEN
        Input.Mode := PIDMode#Auto;
    ELSE
        Input.Mode := PIDMode#Manual;
        Input.ManualOutput := 0.0;
    END_IF;
    Input.CycleTime := 0.1;
    HeaterPct := Loop.Update(Input := Input);
END_PROGRAM
```

A pump fills a tank that drains through an outlet by gravity. The outflow grows with the square
root of the level, so the tank responds differently at low and at high level, and one tuning is
either slow at one end or restless at the other. Three tunings, each validated at its level, are
scheduled on the level. Outside the schedule the program uses the fixed tuning `Safe`, whose
limits and anti-windup also serve every point:

```iecst
USING Control;

PROGRAM Tank
VAR
    LevelM AT %IL0 : LREAL;
    PumpPct AT %QL0 : LREAL;
    SetpointM : LREAL := 1.0;
    Low : PIDTuning := (ProportionalGain := 40.0, IntegralGain := 0.8, ProportionalSetpointWeight := 1.0, TrackingTime := 25.0);
    Middle : PIDTuning := (ProportionalGain := 40.0, IntegralGain := 0.46, ProportionalSetpointWeight := 1.0, TrackingTime := 43.0);
    High : PIDTuning := (ProportionalGain := 40.0, IntegralGain := 0.36, ProportionalSetpointWeight := 1.0, TrackingTime := 55.0);
    Safe : PIDParameters := (
        ProportionalGain := 40.0,
        IntegralGain := 0.36,
        ProportionalSetpointWeight := 1.0,
        OutputMinimum := 0.0,
        OutputMaximum := 100.0,
        RateMinimum := -1000.0,
        RateMaximum := 1000.0,
        TrackingTime := 55.0,
        AntiWindup := PIDAntiWindup#BackCalculation
    );
    Settings : PIDGainScheduleParameters;
    Schedule : PIDGainScheduler;
    Scheduled : PIDGainScheduleOutput;
    ScheduleConfigured : BOOL;
    OutsideSchedule : BOOL;
    Loop : PID;
    Start : PIDInitialState;
    Input : PIDInput;
    Initialized : BOOL;
END_VAR
    IF NOT Initialized THEN
        Settings.Points[0].OperatingPoint := 0.4;
        Settings.Points[0].Tuning := Low;
        Settings.Points[1].OperatingPoint := 1.2;
        Settings.Points[1].Tuning := Middle;
        Settings.Points[2].OperatingPoint := 2.0;
        Settings.Points[2].Tuning := High;
        Settings.PointCount := 3;
        Settings.CommonParameters := Safe;
        Settings.FilterTimeConstant := 5.0;
        ScheduleConfigured := Schedule.Configure(Parameters := Settings);
        Schedule.Initialize(OperatingPoint := LevelM);
    END_IF;
    Scheduled := Schedule.Update(OperatingPoint := LevelM, CycleTime := 0.1);
    OutsideSchedule := NOT ScheduleConfigured OR NOT Scheduled.Valid OR Scheduled.Clamped;
    IF OutsideSchedule THEN
        Loop.Configure(Parameters := Safe);
    ELSE
        Loop.Configure(Parameters := Scheduled.Parameters);
    END_IF;
    IF NOT Initialized THEN
        Start.Setpoint := SetpointM;
        Start.ProcessValue := LevelM;
        Loop.Initialize(State := Start);
        Initialized := TRUE;
    END_IF;
    Input.Setpoint := SetpointM;
    Input.ProcessValue := LevelM;
    Input.Mode := PIDMode#Auto;
    Input.CycleTime := 0.1;
    PumpPct := Loop.Update(Input := Input);
END_PROGRAM
```

## Validation

```bash
cargo build -p trust-dev
cargo test -p trust-runtime --test control_pid_library
cargo test -p trust-runtime --test control_pid_state_observation
cargo test -p trust-runtime --test control_pid_mode_observation
cargo test -p trust-runtime --test control_pid_performance
cargo test -p trust-runtime --test control_pid_mimo_performance
cargo test -p trust-runtime --test control_pid_commissioning_example
cargo test -p trust-runtime --test control_guide
cargo test -p trust-runtime --lib simulation::plant::pid_signal_tests
```

`control_pid_library` runs the library's Structured Text conformance tests in
`crates/trust-runtime/tests/fixtures/control_pid` with `trust-dev`, so build that first or set
`TRUST_DEV_BIN`: the PID's equations, modes, limits and anti-windup, the filter, and the three
structures and the conversion. `control_pid_state_observation` checks `GetState`, and
`control_pid_mode_observation` checks `ModeValue` and `ModeObserved`. The two
performance tests run closed loops against simulated plants; their results are simulation evidence,
not proof on a physical plant. `control_pid_commissioning_example` checks the commissioning project,
`control_guide` compiles and runs every example of this guide, and `pid_signal_tests` checks the
simulation extension against a simulated plant's `pid:` target.
