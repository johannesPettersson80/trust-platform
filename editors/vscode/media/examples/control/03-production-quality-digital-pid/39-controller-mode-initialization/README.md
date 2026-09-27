# 25. Controller initialization

You will initialize one stateful controller instance from the running heater, select which component owns the output, and make automatic control available only after a valid initialization.

## The problem

Software can start while the heater is already at 35 °C and delivering 75% power. If an uninitialized integral state is treated as zero, the next automatic calculation can replace an already-applied command with zero merely because software has no history yet. Initialization must represent the physical state without taking automatic authority during the initialization scan.

## What we need

We need the first automatic calculation to agree with the heater's current
measurement and applied power. Stored state and timing must be initialized
before automatic control is allowed to take output ownership.

## The new idea

**Controller initialization** sets the stored values needed for the first
automatic calculation to agree with the current operating condition. A
**function-block instance** is one controller's persistent set of inputs,
outputs and internal state. We reuse that same instance on every update.

To reproduce the command already applied, initialize integral output as
$I=u_a-P$, where $u_a$ is applied power and $P$ the current proportional
contribution. Initialize the measurement filter and time baseline too, then
make automatic control available. Being ready does not itself transfer output
ownership away from manual operation.

An enumerated mode gives names to the ownership states:

```st
TYPE
    HeaterMode : (Manual, Initializing, Automatic);
END_TYPE
```

An enumeration defines a type with named values. `Ready` is a separate Boolean: the controller can be ready while the manual command remains selected.

An illustrative function-block interface makes the retained instance concrete:

```st
FUNCTION_BLOCK HeaterController
    VAR_INPUT
        SetpointC : REAL;
        MeasuredTemperatureC : REAL;
        KpPctPerC : REAL;
        KiPctPerCs : REAL;
        Beta : REAL;
        FilterTimeS : REAL;
        TrackingTimeS : REAL;
        AppliedPowerPct : REAL;
        CurrentUpdateTimeS : REAL;
        MeasurementValid : BOOL;
        InitializeRequest : BOOL;
        AutomaticRequest : BOOL;
    END_VAR
    VAR_OUTPUT
        Mode : HeaterMode;
        Ready : BOOL;
        RequestedPowerPct : REAL;
        AutomaticOwnsOutput : BOOL;
        AutomaticCommandPct : REAL;
        FaultLatched : BOOL;
        IntegralPct : REAL;
    END_VAR
    VAR
        FilteredTemperatureC : REAL;
        PreviousUpdateTimeS : REAL;
        InitializationDone : BOOL;
    END_VAR
    (* Add the lifecycle logic described below around the existing PI calculation. *)
END_FUNCTION_BLOCK
```

`VAR_INPUT` declares values supplied to a call; `VAR_OUTPUT` declares results and visible state. The internal `VAR` fields also retain their values between calls. The declaration above defines the interface to complete with the lifecycle logic. Supply the selected gain, weighting, filter and tracking parameters from the calling program; their calculations remain those already taught. Grouping related parameters into one record is introduced in the parameter-update lesson.

Declare one instance in the calling program and call that same instance every scan:

```st
VAR
    Heater1 : HeaterController;
END_VAR

Heater1(
    SetpointC := SetpointC,
    MeasuredTemperatureC := MeasuredTemperatureC,
    KpPctPerC := KpPctPerC,
    KiPctPerCs := KiPctPerCs,
    Beta := Beta,
    FilterTimeS := FilterTimeS,
    TrackingTimeS := TrackingTimeS,
    MeasurementValid := MeasurementValid,
    AppliedPowerPct := AppliedPowerPct,
    CurrentUpdateTimeS := CurrentUpdateTimeS,
    InitializeRequest := InitializeRequest,
    AutomaticRequest := AutomaticRequest
);
```

The name before each `:=` inside the call identifies an input; the expression after it supplies the value. Member access such as `Heater1.Ready` reads that instance's output. Reusing the instance preserves its state; constructing a fresh controller every scan would lose the integral history.

`RequestedPowerPct` is the unlimited automatic calculation. `AutomaticCommandPct` is that request after the existing limiter. The calling program is the only writer of the actuator command:

```st
IF Heater1.FaultLatched THEN
    AppliedPowerPct := 0.0;
ELSIF Heater1.AutomaticOwnsOutput THEN
    AppliedPowerPct := Heater1.AutomaticCommandPct;
ELSE
    AppliedPowerPct := LimitedManualPowerPct;
END_IF;
```

Limit the manual request with the same 0–100% decision before this selection. `AppliedPowerPct` supplied to the block reports the selected command; assigning a block input alone would not write the actuator. The fault output is initially false; its latching behavior is developed in the measurement-validity lesson. During Initializing, validate PV, the 0–100% applied command and the current timestamp, set filtered temperature to PV, calculate the current weighted $P$, set $I=u_{\mathrm{applied}}-P$, set Ready, return Mode to Manual and keep AutomaticOwnsOutput false. A repeated initialization request is ignored after the one-shot gate unless a new explicit reset clears it. No ordinary integral increment occurs on the initialization scan. A later new automatic request may select automatic output. Keep the scan clock current while manual control remains selected, and establish the control-time baseline again at automatic entry so no inactive interval is integrated.

## Prediction

At 35 °C, 75% applied power and zero error, predict $P=0$, $I=75$, and an automatic request of 75% after readiness. Predict that a zeroed integral would produce 0% before error develops. Predict that manual output remains the actuator owner during initialization and that invalid PV, invalid applied power or a backward timestamp blocks Ready and automatic mode.

## Implementation

Use one consistent interface for this lesson and Lessons 26–28: a controller instance receives setpoint, valid measured PV, applied power, elapsed time and mode requests; it exposes mode, readiness, requested power, applied power, integral state and fault status. There is one output selector: manual output or automatic requested output writes the actuator, never both.

The controller retains state between scans. The retained fields needed here are mode, integral output, filtered-measurement state, previous update time, an initialization gate and readiness. A valid PV initializes the measurement state to PV. A valid applied output and current weighted proportional term initialize the integral state so automatic output would reconstruct the applied output. At 35 °C with 75% applied power, $P=0$ and $I=75$; at another setpoint or PV, use $I=u_{\mathrm{applied}}-P$, not a fixed 75% bias.

Implement only the lifecycle boundary around the existing weighted/filter-aware PI calculation. Use the current filtered PV and Lesson 22 proportional setpoint weight to calculate $P$; do not replace the controller with a new P-only rule. Keep requested and applied power separate, and let the single output selector choose manual or automatic command.

Initialization at experiment time 1 s should set the timestamp baseline to the current update time. Automatic enable at time 2 s is a separate event. Store and expose the initialization gate and readiness so a repeated request cannot add applied output to the integral again.

## Experiment in Control Studio

1. Start the heater simulator with PV 35 °C, manual applied power 75%, setpoint 35 °C, valid data, filter state 35 °C and delay history 75%. Display mode, readiness, PV, weighted $P$, $I$, requested/applied power, timestamp and owner.
2. In **Control Studio → Monitor → Response**, inspect the live signals; **Test → Confirm and start** establishes time zero and Test automatically records the 10 s sequence. At experiment time 1 s issue one initialization request while Manual remains selected. At 2 s issue the separate automatic-enable request. Do not change setpoint, load, gains or manual output.
3. Compare the first automatic request with 75% and inspect that only one output writer is active. Repeat the initialization request on paper or after a full reset to verify the one-shot gate; do not combine histories in one run.
4. Stop if PV leaves 0–80 °C, applied command leaves 0–100%, data is invalid, or timestamp validation fails. On stop cancel automatic control, command zero heat, wait for restoration, and fully reset the simulator before repeating. Select **Abort test** for an early stop and stop the simulator if it is unresponsive.

## Evidence and decision

The boundary record must show valid PV and applied output, filtered state initialized to PV, current weighted $P$, $I=u_{\mathrm{applied}}-P$, timestamp baseline, Manual ownership during initialization, and Ready before Automatic. At the hot reset, the expected reconstructed request is 75%; this is a state-alignment check, not a claim that 75% is correct at every operating point.

If invalid initial inputs prevent readiness, report that automatic control was blocked. Fault latching and multi-step recovery are handled in Lesson 28. A record containing those events supports the ownership and initialization behavior under the tested conditions; it does not prove physical sensor correctness.

## Conclusion

A stateful controller instance must initialize its retained filter, integral and timing state from the actual heater while manual control remains the sole output owner. Readiness and automatic authority are separate events, so software startup does not replace an already-applied command with an uninitialized value.

## Reason for the next lesson

After initialization, manual operation can change applied power while automatic control is inactive. The controller must track that changing output so the authority transfer remains continuous.

[Bumpless transfer](../40-bumpless-transfer/README.md) aligns the integral state with manual output at the transfer boundary.
