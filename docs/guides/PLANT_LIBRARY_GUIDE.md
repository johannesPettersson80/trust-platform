# Plant Library Guide

The Plant library holds a plant's equipment as function blocks: motors, pumps, fans, drives,
heaters, valves, dampers and cylinders; analog and digital instruments, totals and tanks; an
on/off controller; interlocks, permissives and protection; duty/standby, lead/lag and staged
groups; sequences; and alarm and sequence managers. Each block comes with its symbol, its panel
and its settings for the HMI Builder. A PID loop is no block of the library: it is ordinary code
around the Control library's `Control.PID`, which Create PLC code writes and the library's PID
panel operates. The specification is [trust-plant-library](../specs/trust-plant-library.md).

## Add the package

```toml
[project]
include_paths = ["src"]

[dependencies]
Plant = { path = "../../libraries/plant", version = "0.1.0" }
```

Adjust the path to the project. Plant uses the Alarm and Control libraries, which it declares
itself. Write `USING Plant;` in a source to use the short names of its enumerations, such as
`E_Mode#Manual`; the blocks are written with their namespace, such as `Plant.FB_Pump`.

## How every device works

**Who commands it.** A device is in one of three modes:

| Mode | Who commands the device | Typical use |
| --- | --- | --- |
| `Auto` | the program, through the device's `Auto…` inputs (`AutoRun`, `AutoOpen`, `AutoSpeed`, `AutoPosition`) | normal operation |
| `Manual` | the operator, through the device's methods (`Start`, `Stop`, `Open`, `Close`, `SetSpeed`, `SetPosition`) | commissioning, local operation |
| `OutOfService` | nobody: the device is held in its safe state, its supervision and alarms are off | maintenance |

A mode change never starts, stops or moves anything: a running pump switched to Manual keeps
running. The program changes the mode with `SetMode(Value := …)`; at the HMI an operator may
switch between Auto and Manual and an engineer may take a device out of service. A method returns
`TRUE` when the device accepts the command; a refused command changes nothing, and the device's
`CommandText` says why ("Auto: switch to Manual", "Interlocked: Tank level high").

**What stops it.** `Interlock := TRUE` holds the device in its safe state in every mode, with
`InterlockText` as the reason. `Permissive := FALSE` refuses a start but leaves a running device
running. A protection trip (`Trip`) and a supervision failure (failed to start, failed to stop,
end-position conflict) latch a `Fault` that stops the device until `Reset` is called after the
cause has gone. Drives and heaters start at 0 % after a stop and after a restart.

**What it reports.** Every device has `Mode`, `Manual`, `OutOfService`, `Fault`, `Interlocked`,
`Blocked` with `BlockReason`, `StartPermit` and `CommandText`. Its alarms are Alarm library alarms
that the runtime lists in the alarm list without any further code.

**Its settings.** A device's settings are public RETAIN members with defaults, such as
`StartTimeout`. Set them for one instance in its declaration:
`P101 : Plant.FB_Pump := (StartTimeout := T#10s);`. An engineer can change them later from the
device's panel; RETAIN keeps the changed value through a restart and a power loss.

A first complete program: a pump that starts when a tank is low, stops when it is full, and is
interlocked by a high-high level switch. The hardware signals are ordinary variables, mapped to I/O
in the configuration.

```iecst
USING Plant;

PROGRAM Main
VAR
    LevelSignal : LREAL;
    HighHighSwitch : BOOL;
    PumpRunning : BOOL;
    PumpRun : BOOL;

    LT101 : Plant.FB_LevelInput := (Unit := 'm', RangeHigh := 4.0);
    LSHH101 : Plant.FB_LevelSwitch;
    P101 : Plant.FB_Pump := (StartTimeout := T#10s);
    Filling : BOOL;
END_VAR
    LT101(Raw := LevelSignal);
    LSHH101(Input := HighHighSwitch);

    IF LT101.Value < 1.0 THEN
        Filling := TRUE;
    ELSIF LT101.Value > 3.5 THEN
        Filling := FALSE;
    END_IF;

    P101(AutoRun := Filling, RunningFeedback := PumpRunning,
        Interlock := LSHH101.Value, InterlockText := 'Tank level high-high');
    PumpRun := P101.RunCommand;
END_PROGRAM
```

## Equipment

### Motors, pumps, fans, blowers, agitators and conveyors

`FB_Motor`, `FB_Pump`, `FB_Fan`, `FB_Blower`, `FB_Agitator` and `FB_Conveyor` behave alike; they
differ in name and symbol.

| Inputs | Outputs | Settings | Commands |
| --- | --- | --- | --- |
| `AutoRun`, `RunningFeedback`, `Trip`, `HasFeedback` (`TRUE`), `Interlock`, `InterlockText`, `Permissive`, `PermissiveText` | `RunCommand` (the contactor), `Running`, `FailedToStart`, `FailedToStop`, `Tripped`, `ServiceDue`; RETAIN `Starts`, `RunHours`, `HoursSinceService` | `StartTimeout` (`T#5s`), `StopTimeout` (`T#5s`), `ServiceInterval` (4000 h, 0 for none) | `Start`, `Stop`, `Reset`, `ServiceDone` |

Without a running contact, declare `HasFeedback := FALSE`: `Running` then follows `RunCommand`
and nothing is supervised.

### Variable-speed motors, pumps, fans and dosing pumps

`FB_VariableSpeedMotor`, `FB_VariableSpeedPump`, `FB_VariableSpeedFan` and `FB_DosingPump` add a
speed to the motor above.

| Inputs | Outputs | Settings | Commands |
| --- | --- | --- | --- |
| `AutoSpeed` (%), `SpeedFeedback`, `HasSpeedFeedback` | `SpeedSetpoint` (the drive's reference, 0 % while stopped), `Speed`, `ManualSpeed`, `SpeedDeviating` | `MinSpeed` (0 %), `MaxSpeed` (100 %), `SpeedDeviation` (5 %), `DeviationTime` (`T#10s`) | `SetSpeed(Value := …)` |

The speed is clamped to `MinSpeed…MaxSpeed` while running; a manual start from 0 % runs at
`MinSpeed`.

### Heater

`FB_Heater` adds `AutoPower` (%), outputs `PowerOutput` (%) and `PulseOutput` (time-proportioned
for a solid-state relay, `PulsePeriod`), `ManualPower`, the setting `MaxPower` (100 %) and the
command `SetPower(Value := …)`. Like a drive it starts at 0 %.

### Reversing and two-speed motors

`FB_ReversingMotor` has `AutoForward`, `AutoReverse`, feedback per direction, outputs
`ForwardCommand` and `ReverseCommand` (never both) and waits `ReversalDelay` (`T#2s`) after a stop
before the other direction; commands `StartForward`, `StartReverse`, `Stop`. `FB_TwoSpeedMotor` has
`AutoRun`, `AutoFast`, outputs `SlowCommand` and `FastCommand` and drops both for
`ChangeoverDelay` (`T#1s`) when the speed changes; commands `StartSlow`, `StartFast`, `Stop`. Both
count starts, running hours and the hours since the last service like a motor.

### Valves and dampers

`FB_OnOffValve` and `FB_Damper`:

| Inputs | Outputs | Settings | Commands |
| --- | --- | --- | --- |
| `AutoOpen`, `OpenFeedback`, `ClosedFeedback`, `HasOpenFeedback`, `HasClosedFeedback` (`TRUE`), `FailOpen` | `Solenoid`, `OpenCommand`, `Opened`, `Closed`, `Moving`, `FailedToOpen`, `FailedToClose`, `FeedbackConflict`, `OpenPermit`, `ClosePermit`; RETAIN `Operations` | `OpenTimeout`, `CloseTimeout` (`T#10s`) | `Open`, `Close`, `Reset` |

`FailOpen` says where the valve goes when its solenoid loses power. The safe state is that fail
position, so an interlock, a fault or Out of service sends a fail-open valve open and a fail-closed
valve closed. Moving away from the fail position counts as a start: it needs the permissive.

`FB_MotorValve` drives a motor-operated valve with open and close contactors, a travel timeout,
torque and thermal trips, and `StopTravel`. `FB_Cylinder` drives a single- or double-acting
cylinder with `AutoExtend`, end-position supervision and `Extend` / `Retract` commands.

### Control valves and modulating dampers

`FB_ControlValve` and `FB_ModulatingDamper`:

| Inputs | Outputs | Settings | Commands |
| --- | --- | --- | --- |
| `AutoPosition` (%), `PositionFeedback`, `HasFeedback`, `FailPosition` (%) | `PositionSetpoint`, `Position`, `InPosition`, `ManualPosition`, `Deviating` | `MinPosition` (0 %), `MaxPosition` (100 %), `PositionDeviation` (5 %), `DeviationTime` (`T#10s`) | `SetPosition(Value := …)` |

An interlock or Out of service sends the valve to `FailPosition`. A deviation between setpoint
and feedback raises a low-priority alarm; it does not latch a fault.

A drive and a valve together: a fail-closed control valve that the program opens step by step,
and a variable-speed pump that runs at 60 % once the valve is open beyond 20 %.

```iecst
USING Plant;

PROGRAM Main
VAR
    PumpRunning : BOOL;
    PumpRun : BOOL;
    PumpReference : LREAL;
    ValveFeedback : LREAL;
    ValveReference : LREAL;

    P201 : Plant.FB_VariableSpeedPump := (MinSpeed := 30.0, HasFeedback := FALSE);
    FV201 : Plant.FB_ControlValve := (HasFeedback := TRUE, PositionDeviation := 10.0);
    Opening : LREAL;
END_VAR
    IF Opening < 80.0 THEN
        Opening := Opening + 1.0;
    END_IF;
    FV201(AutoPosition := Opening, PositionFeedback := ValveFeedback, FailPosition := 0.0);
    P201(AutoRun := FV201.Position > 20.0, AutoSpeed := 60.0, RunningFeedback := PumpRunning);
    ValveReference := FV201.PositionSetpoint;
    PumpRun := P201.RunCommand;
    PumpReference := P201.SpeedSetpoint;
END_PROGRAM
```

## Instruments

### Analog instruments

`FB_AnalogInput`, and with their own symbols `FB_LevelInput`, `FB_PressureInput`,
`FB_TemperatureInput`, `FB_AnalyzerInput` and `FB_FlowInput`:

| Inputs | Outputs | Settings |
| --- | --- | --- |
| `Raw` (the channel's value, for example in mA), `SignalFault` (the module's diagnosis) | `Value`, `Percent`, `Quality` (`E_Quality`), `Fault`, `HighHigh`, `High`, `Low`, `LowLow` | `Unit`, `RawLow` (4), `RawHigh` (20), `RangeLow` (0), `RangeHigh` (100), `BadBelow` (3.6), `BadAbove` (21), `FilterTime`, `HighHighEnabled` … `LowLowEnabled`, `HighHighLimit` … `LowLowLimit`, `Deadband`, `AlarmDelay` |

`Value` is `Raw` scaled from `RawLow…RawHigh` to `RangeLow…RangeHigh`, filtered by
`Control.LowPass` when `FilterTime` is set. A signal outside `BadBelow…BadAbove`, a module fault or
a value that is not a number make the quality `Bad`: `Value` keeps its last good reading, one
"Signal fault" alarm is raised and the limit alarms are suppressed. The defaults suit a 4–20 mA
signal; for a signal already in engineering units set `RawLow`/`RawHigh` equal to the range and
widen `BadBelow`/`BadAbove`.

`FB_FlowInput` adds `Total` (RETAIN), `TotalPer` (the time base of the flow unit, `T#1h` for
m³/h) and `ResetTotal`.

### Digital instruments, totals and counters

`FB_DigitalInput`, `FB_LevelSwitch` and `FB_PressureSwitch` debounce (`Debounce`) and optionally
invert (`Invert`) a contact; with `AlarmEnabled` they raise an alarm with `AlarmText` while the
value equals `AlarmWhen`. `FB_Totalizer` totals a rate (`Rate`, or `Measurement` taken from an
instrument) into `Total`; `FB_Counter` counts the rising edges of `Pulse` up to `Preset`.

### Weigh scale and tank

`FB_WeighScale` is an analog instrument with `Net`, `TareWeight`, `Stable` and `Tare` (accepted
only while the weight is stable). `FB_Tank` turns a level instrument into level, percent and
volume: connect the instrument to `LevelInstrument` and set `Height` and `Capacity`.

A tank with its level transmitter, a flow meter and a weigh scale:

```iecst
USING Plant;

PROGRAM Main
VAR
    LevelSignal : LREAL;
    FlowSignal : LREAL;
    WeightSignal : LREAL;

    LT301 : Plant.FB_LevelInput := (Unit := 'm', RangeHigh := 3.0,
        HighEnabled := TRUE, HighLimit := 2.7, LowEnabled := TRUE, LowLimit := 0.3, Deadband := 0.05);
    T301 : Plant.FB_Tank := (Height := 3.0, Capacity := 12.0);
    FT301 : Plant.FB_FlowInput := (Unit := 'm³/h', RangeHigh := 50.0);
    WT301 : Plant.FB_WeighScale := (Unit := 'kg', RangeHigh := 500.0);
    Volume : LREAL;
END_VAR
    LT301(Raw := LevelSignal);
    T301(LevelInstrument := LT301);
    FT301(Raw := FlowSignal);
    WT301(Raw := WeightSignal);
    Volume := T301.Volume;
END_PROGRAM
```

## Controllers

### PID loop

A PID loop is ordinary code around the Control library's `Control.PID`, run through its
`Configure`, `Initialize` and `Update` methods, with no block around it. Placing a PID controller
in the HMI Builder and pressing Create PLC code writes it (specification 39, §10.5); written by
hand it is the same code. Its values are variables beside the PID, named after it:

| Variable | Kept through a restart | Holds |
| --- | --- | --- |
| `LIC401` (`Control.PID`), `LIC401_Input`, `LIC401_Start`, `LIC401_Initialized` | no | The PID, its input and start records, whether it has started |
| `LIC401_Parameters` (`Control.PIDParameters`) | yes | The gains and limits: what the HMI's settings and Control Studio change |
| `LIC401_Manual` | yes | The operator's mode: `TRUE` is Manual, `FALSE` Auto |
| `LIC401_Setpoint`, `LIC401_ManualOutput`, `LIC401_Output` | yes | The setpoint, the manual output and the output |

In Auto the manual output follows the output, so Manual takes over where the loop is; from Manual
the PID continues from the manual output, so Auto starts without a bump. While the measurement is
bad the loop holds its output in Manual, and it starts again, without a bump, from the next good
measurement. The three gains share one sign: negative gains make the output rise while the
measurement is above the setpoint, as for a pump emptying a tank. The library's PID panel shows
the loop's mode, measurement, setpoint, output and trend; the operator needs write rules for the
mode, the setpoint and the manual output, which Create PLC code adds.

A level loop: the controller keeps a tank at 1.5 m with its outlet valve.

```iecst
USING Plant;
USING Control;

PROGRAM Main
VAR
    LevelSignal : LREAL;
    ValveReference : LREAL;

    LT401 : Plant.FB_LevelInput := (Unit := 'm', RangeHigh := 3.0);
    LV401 : Plant.FB_ControlValve;
    LIC401 : Control.PID;
    LIC401_Input : Control.PIDInput;
    LIC401_Start : Control.PIDInitialState;
    LIC401_Initialized : BOOL;
END_VAR
VAR RETAIN
    LIC401_Parameters : Control.PIDParameters := (ProportionalGain := -20.0, IntegralGain := -1.0,
        DerivativeGain := 0.0, ProportionalSetpointWeight := 1.0, DerivativeSetpointWeight := 0.0,
        Bias := 0.0, OutputMinimum := 0.0, OutputMaximum := 100.0, RateMinimum := -1000000000.0,
        RateMaximum := 1000000000.0, TrackingTime := 1.0, AntiWindup := PIDAntiWindup#BackCalculation);
    LIC401_Manual : BOOL := FALSE;
    LIC401_Setpoint : LREAL := 1.5;
    LIC401_ManualOutput : LREAL;
    LIC401_Output : LREAL;
END_VAR
    LT401(Raw := LevelSignal);
    LIC401.Configure(Parameters := LIC401_Parameters);
    LIC401_Input.Setpoint := LIC401_Setpoint;
    LIC401_Input.ManualOutput := LIC401_ManualOutput;
    LIC401_Input.CycleTime := 0.1;
    IF LT401.GetQuality() = E_Quality#Good THEN
        LIC401_Input.ProcessValue := LT401.GetValue();
        IF NOT LIC401_Initialized THEN
            LIC401_Start.Setpoint := LIC401_Setpoint;
            LIC401_Start.ProcessValue := LIC401_Input.ProcessValue;
            LIC401_Start.NominalOutput := LIC401_Output;
            LIC401_Start.SaturatedOutput := LIC401_Output;
            LIC401.Initialize(State := LIC401_Start);
            LIC401_Initialized := TRUE;
        END_IF;
        IF LIC401_Manual THEN
            LIC401_Input.Mode := PIDMode#Manual;
        ELSE
            LIC401_Input.Mode := PIDMode#Auto;
        END_IF;
    ELSE
        LIC401_Input.Mode := PIDMode#Manual;
        LIC401_Initialized := FALSE;
    END_IF;
    LIC401_Output := LIC401.Update(Input := LIC401_Input);
    IF LIC401_Input.Mode = PIDMode#Auto THEN
        LIC401_ManualOutput := LIC401_Output;
    END_IF;
    LV401(AutoPosition := LIC401_Output, FailPosition := 0.0);
    ValveReference := LV401.PositionSetpoint;
END_PROGRAM
```

### Cascade and on/off control

Two loops are a cascade when one loop's output is the other's setpoint: connect the outer
controller to the inner one in the HMI Builder. Create PLC code then writes
`TIC201_Setpoint := TIC200_Output`, and while the inner loop is in Manual the outer loop tracks the
inner loop's measurement, so closing the cascade is bumpless. `FB_OnOffController` switches
`Demand` with a `Hysteresis` around its `Setpoint`; wire `Demand` to a device's `AutoRun`.

## Interlocks, permissives and protection

`FB_Interlocks`, `FB_Permissives` and `FB_Protection` each take up to eight conditions
`Condition1…Condition8`, `TRUE` meaning healthy, with texts `Text1…Text8`. They give `Healthy`,
`Active`, `Text` (the first condition that went bad) and `FirstOut`. Wire an interlock set's
`Active` and `Text` to a device's `Interlock` and `InterlockText`, and a permissive set's
`Healthy` and `Text` to `Permissive` and `PermissiveText`. An engineer may bypass an interlock or
permissive condition for `BypassTime` (`T#1h`); protection cannot be bypassed. These are process
conditions, not certified safety functions.

```iecst
USING Plant;

PROGRAM Main
VAR
    OilPressureOk : BOOL;
    GuardClosed : BOOL;
    EmergencyStopOk : BOOL;
    MotorRunning : BOOL;
    MotorRun : BOOL;

    StartConditions : Plant.FB_Permissives;
    StopConditions : Plant.FB_Interlocks;
    M501 : Plant.FB_Conveyor;
END_VAR
    StartConditions(Condition1 := OilPressureOk, Text1 := 'Oil pressure low',
        Condition2 := GuardClosed, Text2 := 'Guard open');
    StopConditions(Condition1 := EmergencyStopOk, Text1 := 'Emergency stop pressed');
    M501(AutoRun := TRUE, RunningFeedback := MotorRunning,
        Permissive := StartConditions.Healthy, PermissiveText := StartConditions.Text,
        Interlock := StopConditions.Active, InterlockText := StopConditions.Text);
    MotorRun := M501.RunCommand;
END_PROGRAM
```

## Groups

`FB_DutyStandbyGroup` (input `Run`: one member runs), `FB_LeadLagGroup` (input `Required`: that
many members run) and `FB_StagedGroup` (input `Demand` in %: members are added and removed in equal
stages with `Hysteresis`, `MinRunTime` and `MinStopTime`) command up to four members,
`Member1…Member4`. A group starts the available member with the fewest running hours first
(`Preferred` first when set), replaces a member that trips or is taken into Manual, starts and
stops members `StartDelay` / `StopDelay` apart, and raises a shortfall alarm when it cannot run as
many as it should. Its members stay in Auto: the group's demand reaches them through their
`Demand` method and is added to their own `AutoRun`, so leave `AutoRun` unconnected for a member
that only the group commands.

```iecst
USING Plant;

PROGRAM Main
VAR
    Pump1Running : BOOL;
    Pump2Running : BOOL;
    Pump1Run : BOOL;
    Pump2Run : BOOL;
    CoolingWanted : BOOL := TRUE;

    P601 : Plant.FB_Pump;
    P602 : Plant.FB_Pump;
    Coolant : Plant.FB_DutyStandbyGroup := (StartDelay := T#1s);
    Alarms : Plant.FB_AlarmManager;
END_VAR
    Coolant(Run := CoolingWanted, Member1 := P601, Member2 := P602);
    P601(RunningFeedback := Pump1Running);
    P602(RunningFeedback := Pump2Running);
    Pump1Run := P601.RunCommand;
    Pump2Run := P602.RunCommand;
    Alarms(Source1 := P601, Source2 := P602, Source3 := Coolant);
END_PROGRAM
```

## Sequences

A sequence extends `FB_Sequence`. Its body calls `Execute()`, which applies the operator's
commands (`Start`, `Hold`, `Resume`, `Stop`, `Abort`, `Reset`) and the step timer, and returns
`TRUE` while the sequence is running; the steps are a `CASE` on `CurrentStep` using the helpers
`GoTo(Number, Name)`, `Wait(Text)`, `Complete()`, `Fail(Text)` and `StepTimeout(Limit, Text)`.
While it is stopping the application brings the plant to rest and calls `StopDone()`. The
sequence's outputs command equipment through the devices' `Auto…` inputs. Nothing of a sequence
is retained: after a restart it is idle.

```iecst
USING Plant;

FUNCTION_BLOCK MixSequence EXTENDS Plant.FB_Sequence
VAR_INPUT
    Level : LREAL;
    FullLevel : LREAL := 2.0;
    MixingTime : TIME := T#2m;
END_VAR
VAR_OUTPUT
    FillValve : BOOL;
    Mixer : BOOL;
END_VAR
    FillValve := FALSE;
    Mixer := FALSE;
    IF Execute() THEN
        CASE CurrentStep OF
            0:
                GoTo(Number := 10, Name := 'Fill');
            10:
                FillValve := TRUE;
                Wait(Text := 'Filling');
                IF Level >= FullLevel THEN
                    GoTo(Number := 20, Name := 'Mix');
                ELSE
                    StepTimeout(Limit := T#20m, Text := 'The tank did not fill within 20 min');
                END_IF;
            20:
                Mixer := TRUE;
                IF StepTime >= MixingTime THEN
                    Complete();
                END_IF;
        END_CASE;
    ELSIF State = E_SequenceState#Stopping THEN
        StopDone();
    END_IF;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    LevelSignal : LREAL;
    ValveOut : BOOL;
    MixerOut : BOOL;

    LT701 : Plant.FB_LevelInput := (Unit := 'm', RangeHigh := 3.0);
    XV701 : Plant.FB_OnOffValve := (HasOpenFeedback := FALSE, HasClosedFeedback := FALSE);
    M701 : Plant.FB_Agitator := (HasFeedback := FALSE);
    Batch : MixSequence;
    Sequences : Plant.FB_SequenceManager;
END_VAR
    LT701(Raw := LevelSignal);
    Batch(Level := LT701.Value);
    Sequences(Sequence1 := Batch);
    XV701(AutoOpen := Batch.FillValve);
    M701(AutoRun := Batch.Mixer);
    ValveOut := XV701.Solenoid;
    MixerOut := M701.RunCommand;
END_PROGRAM
```

## Alarm and sequence managers

`FB_AlarmManager` takes up to sixteen alarm sources (`Source1…Source16`, any device, instrument or
group) and gives `ActiveCount`, `UnacknowledgedCount` and `Horn`, with `AcknowledgeAll` and
`Silence`. The HMI's alarm list does not need it: the runtime lists every alarm. Use it for a horn
and for counts on an overview. `FB_SequenceManager` observes up to eight sequences
(`Sequence1…Sequence8`) and gives `RunningCount`, `HeldCount` and `FailedCount`, with `StopAll`
and `AbortAll`.

## In the HMI Builder

Each block has a component definition in `libraries/plant/hmi/pairs` and a panel in
`libraries/plant/hmi/panels`. Drag a component from the Equipment, Instruments, Controllers or
Groups and sequences sections onto a page; the inspector shows its tag, its hardware status and its
key settings with their real values. **Connect hardware** opens Devices & Connections for it.
**Create PLC code** then writes one declaration and one call per new device into marked regions of
the program; **Create HMI** does the opposite and places a symbol for each declared device.

At run time the device's panel shows its state, its mode, Start and Stop (greyed in Auto, with the
reason beside them) and, for an engineer, its settings.

## Restart and power loss

A device's mode, counters and settings are RETAIN: they survive a warm restart and a power loss,
also for an instance inside another function block. Commands, faults and alarms are not retained:
after a restart a device is stopped, with no fault, in the mode it had. A cold restart starts every
device from its declaration.
