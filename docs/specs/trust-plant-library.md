# truST Plant library

Status: native rebuild agreed with the user on 7 October 2026. This specification replaces the
capability-and-ownership design of 5–6 October and the Equipment library (specification 41,
retired). Implementation tracker: [Plant library checklist](../internal/testing/checklists/trust-plant-library-checklist.md),
section R. HMI integration: [specification 39](39-hmi-builder.md) and the
[component catalogue](hmi-component-catalogue.md).

## 1. Purpose and boundary

1. The `Plant` library gives application engineers ready equipment, instruments, controllers,
   groups and sequences written once, in IEC 61131-3 Ed.3 object-oriented ST: function blocks
   with methods, small interfaces, shallow inheritance and composition. An engineer declares an
   instance, calls it once per scan with its normal I/O, and writes only the process logic.
2. Each block owns its behaviour. No block wraps, embeds or forwards to another equipment
   library. There is one supported equipment library; the `Equipment` namespace and its
   compatibility copy are removed.
3. Blocks read normal PLC inputs and write normal PLC outputs. Whether the I/O comes from a
   driver or the simulator is runtime configuration ([specification 46](46-simulation-plant-models.md));
   the library has no simulation branches, flags or models.
4. PID loops are ordinary code around the existing `Control.PID` (specification 45, §8.1), with
   no block around the PID; filters use `Control.LowPass`; alarms are `Alarm` library instances
   (specification 40). The library adds no second control law, alarm engine or historian.
5. These blocks are ordinary process control. They are not certified safety functions; a
   safety function belongs in a safety-rated system.

## 2. Organisation

1. Namespace `Plant`; package `libraries/plant` with `src/*.st`, the HMI component
   definitions in `hmi/pairs/*.toml`, the operator panels in `hmi/panels/*.toml`, and the
   runnable examples in `examples/`. Dependencies: `Alarm` and `Control`.
2. Names: `I…` interfaces, `FB_…` function blocks, `ST_…` structures, `E_…` enumerations,
   `F_…` functions.
3. A derived function block runs only its own body (specification 04). Every concrete block
   therefore calls its family's protected `Execute` method from its body; a customer block that
   extends a Plant block does the same before or after its own logic.

## 3. Authority: modes, program inputs and operator commands

1. There is no ownership, "take control", request identifier, command context, registration
   or numeric device identity in PLC code. Who commands a device is decided by its mode.
2. `E_Mode : (Auto, Manual, OutOfService)`; the first value is the default.
   - **Auto:** the device follows the program through its `Auto…` inputs (`AutoRun`,
     `AutoOpen`, `AutoSpeed`, `AutoPosition`, `AutoPower`, …), evaluated every scan.
   - **Manual:** the device follows the operator's commands, the public methods `Start`,
     `Stop`, `Open`, `Close`, `SetSpeed`, `SetPosition`, `SetPower`, … The program's `Auto…`
     inputs are ignored.
   - **Out of service:** the device is held in its safe state (§4.6); commands are refused,
     supervision stops, its alarms are suppressed by design and its counters stand still.
3. `SetMode(Value : E_Mode) : BOOL` changes the mode; it returns `TRUE` when the mode is in
   force. A program may call it, for example a sequence that puts its equipment in Auto.
4. **Bumpless:** changing between Auto and Manual never stops, starts or moves anything. While
   in Auto the manual command follows the present command (running or stopped, open or closed,
   the present speed, position or power), so Manual continues exactly where Auto was. Changing
   to Auto hands the device to the program's inputs. Leaving Out of service starts from the
   safe state.
5. **Operator commands** return `TRUE` when accepted. They are refused, with no effect, when
   the device is not in Manual, and a start or open is also refused while the device is blocked
   (§4.2). A refused command leaves nothing pending: a refused start does not start later.
6. **Roles** are enforced at the HMI boundary, not in PLC code. Viewer: read only. Operator:
   Auto/Manual, operator commands, setpoints, alarm actions. Engineer: Out of service and
   return to service, settings, bypasses, service confirmation. The component definition
   declares the role of each command; the runtime takes it from there, never from the browser.
7. `Reset() : BOOL` clears latched faults whose cause is gone (§4.3). It is accepted in every
   mode, never starts anything and never clears an active physical condition.

## 4. Common device behaviour

Sections 4.1–4.8 hold for every actuator (motors, drives, heater, valves, cylinder). The
abstract `FB_Device` implements them once.

### 4.1 Inputs and outputs of every device

| Kind | Member | Meaning |
|---|---|---|
| input | `Interlock : BOOL` | While TRUE the device is held in its safe state in every mode and starts/opens are refused. |
| input | `InterlockText : STRING[80]` | Why; shown to the operator. |
| input | `Permissive : BOOL := TRUE` | While FALSE a start/open is refused; a running or open device continues. |
| input | `PermissiveText : STRING[80]` | Why not. |
| output, RETAIN | `Mode : E_Mode` | Mode in force. |
| output | `Manual : BOOL`, `OutOfService : BOOL`, `InService : BOOL` | The mode as flags for symbols and command permits. |
| output | `Fault : BOOL` | A latched device fault: protection trip or supervision failure. |
| output | `Interlocked : BOOL` | `Interlock` is holding the device. |
| output | `Blocked : BOOL`, `BlockReason : STRING[80]` | A start/open would be refused now, and why. |
| output | `StartPermit : BOOL` | The operator may start/open now: Manual and not blocked. |
| output | `CommandText : STRING[80]` | Why an operator command is refused now (§4.2); empty when it is accepted. |
| output | `ReasonText : STRING[80]` | The panel's message line (§4.2): `BlockReason`, else `CommandText`. |

### 4.2 Blocking and command texts

1. `Blocked` is TRUE when a start would be refused: out of service, a fault, the interlock, or a
   missing permissive. For valves, dampers and cylinders, moving away from the fail position counts
   as the start.
2. `BlockReason` names the first cause in this order: `Out of service`, the fault's text
   (for example `Protection trip: reset after the cause is cleared`), `Interlocked: <text>`
   (`Interlocked` when the text is empty), `Not permitted: <text>` (`Not permitted`).
3. `CommandText` is `Auto: switch to Manual` in Auto, `Out of service` out of service, else
   `BlockReason` while blocked, else empty. Panels bind start/open buttons to `StartPermit`
   and stop/close/value controls to `Manual`, with `CommandText` as the reason.
4. `ReasonText` is `BlockReason` when there is one, else `CommandText`: one message line that
   names why a device is blocked even in Auto, and otherwise why a command would be refused
   (decided with the user on 7 October 2026, so a panel has no empty second line).

### 4.3 Faults, trips and reset

1. A protection trip input (an overload relay, a thermal switch, a torque switch) stops the
   device and latches `Fault` until `Reset` while the trip input is gone.
2. A supervision failure (failed to start, stop, open, close, extend or retract; conflicting
   end-position feedback) latches `Fault` and puts the device in its safe state until `Reset`.
3. After `Reset` while the condition remains, the failure returns after a full supervision
   time; a trip whose input is still TRUE stays latched.
4. A fault stops a manually started device for good: after the reset the operator starts it
   again. In Auto the program's input commands it again once nothing blocks it.

### 4.4 Feedback supervision

1. Supervision runs in Auto and Manual whenever the device has the feedback (`HasFeedback`,
   `HasOpenFeedback`, …), and only out of service is it switched off. Supervision never depends
   on who commanded the device.
2. Without feedback the device's state follows its command.
3. The supervision times are settings (§10) with the defaults of each block.

### 4.5 Alarms

1. Each device holds its alarms as `Alarm` instances (`Alarm.DigitalAlarm`, `LimitAlarm`,
   `DeviationAlarm`). The runtime finds every alarm instance itself and names it by its
   instance path; the device is the alarm's equipment ([spec 40](40-alarm-library.md) §5).
2. A failure or trip alarm stays active while its fault is latched. Alarms are suppressed by
   design out of service, and only then.
3. Every device implements `IAlarmSource` (`GetAlarmCount`, `GetAlarm(Index)`), so an
   `FB_AlarmManager` (§9.2) can summarise any device by composition.

### 4.6 Safe state

1. The safe state is: stopped for motors, drives and heaters; the de-energised position for an
   on/off valve, damper or cylinder (`FailOpen` / `FailExtended` choose which); stopped in place
   for a motor-operated valve, which has no spring; `FailPosition` (default 0 %) for a control
   valve or modulating damper.
2. A device goes to its safe state on an interlock, on a latched fault and out of service, in
   every mode. A valve's safe state is its fail position: a fail-open valve opens.

### 4.7 Counters and maintenance

1. Run devices count `Starts : UDINT` and `RunHours : LREAL`; valves, dampers and cylinders count
   `Operations : UDINT`. Counters are RETAIN and stand still out of service.
2. Run devices have the setting `ServiceInterval : LREAL := 4000.0` (h, 0 = none), the RETAIN
   output `HoursSinceService` and `ServiceDue`. `ServiceDone() : BOOL` (Engineer) restarts the
   service count; it does not change `RunHours`.

### 4.8 Restart

1. **Cold restart:** declarations and instance initialisers apply: Auto, stopped/closed, no
   fault, counters zero.
2. **Warm restart and power loss:** RETAIN members survive: the mode, the counters, the
   settings. Manual run/open commands and drive speeds or heater powers chosen in Manual are not
   retained: the device is stopped/in its safe state and must be commanded again. In Auto the
   program commands it from the first scan. Supervision starts afresh.
3. Latched faults are not retained; the first scan supervises the real feedback again.

## 5. Interfaces

Every method an HMI button calls belongs to an interface the block implements (§12.2).

| Interface | Members |
|---|---|
| `IDevice` | `SetMode(Value : E_Mode) : BOOL`, `GetMode() : E_Mode`, `Reset() : BOOL` |
| `IRunDevice EXTENDS IDevice` | `Start() : BOOL`, `Stop() : BOOL`, `IsRunning() : BOOL` |
| `ISpeedDevice EXTENDS IRunDevice` | `SetSpeed(Value : LREAL) : BOOL` |
| `IPowerDevice EXTENDS IRunDevice` | `SetPower(Value : LREAL) : BOOL` |
| `IReversingDevice EXTENDS IDevice` | `StartForward() : BOOL`, `StartReverse() : BOOL`, `Stop() : BOOL` |
| `ITwoSpeedDevice EXTENDS IDevice` | `StartSlow() : BOOL`, `StartFast() : BOOL`, `Stop() : BOOL` |
| `IOpenCloseDevice EXTENDS IDevice` | `Open() : BOOL`, `Close() : BOOL`, `IsOpen() : BOOL`, `IsClosed() : BOOL` |
| `ITravelStop` | `StopTravel() : BOOL` |
| `IExtendRetractDevice EXTENDS IDevice` | `Extend() : BOOL`, `Retract() : BOOL`, `IsExtended() : BOOL`, `IsRetracted() : BOOL` |
| `IPositionDevice EXTENDS IDevice` | `SetPosition(Value : LREAL) : BOOL`, `GetPosition() : LREAL` |
| `IServiceable` | `ServiceDone() : BOOL` |
| `IDutyMember` | `IsAvailable() : BOOL`, `Demand(Run : BOOL)`, `IsRunning() : BOOL`, `GetRunHours() : LREAL` |
| `IMeasurement` | `GetValue() : LREAL`, `GetQuality() : E_Quality` |
| `ITotal` | `ResetTotal() : BOOL` |
| `ICounter` | `ResetCount() : BOOL` |
| `ITare` | `Tare() : BOOL`, `ClearTare() : BOOL` |
| `ISetpoint` | `SetSetpoint(Value : LREAL) : BOOL` |
| `IBypass` | `Bypass(Index : INT) : BOOL`, `RemoveBypass(Index : INT) : BOOL` |
| `IAlarmSource` | `GetAlarmCount() : INT`, `GetAlarm(Index : INT) : REF_TO Alarm.Alarm` |
| `IAlarmManager` | `AcknowledgeAll() : BOOL`, `Silence() : BOOL` |
| `ISequence` | `Start`, `Stop`, `Hold`, `Resume`, `Abort`, `Reset` (each `: BOOL`), `GetState() : E_SequenceState` |
| `ISequenceManager` | `StopAll() : BOOL`, `AbortAll() : BOOL` |

Getters have no side effects. An out-of-range `GetAlarm` index returns `NULL`.

## 6. Equipment

Settings are marked *(s)*: public RETAIN variables with metadata in the component definition
(§10). Engineering configuration such as `HasFeedback` or `FailOpen` is an input, wired once in
the call or set by the instance initialiser.

### 6.1 Motor family

`FB_RunDevice` (abstract) implements run control once; `FB_Motor` is the concrete block, and
`FB_Pump`, `FB_Fan`, `FB_Blower`, `FB_Agitator` and `FB_Conveyor` extend it for their symbol and
name only.

| Kind | Members |
|---|---|
| inputs | `AutoRun`, `RunningFeedback`, `Trip`, `HasFeedback := TRUE` |
| outputs | `RunCommand`, `Running`, `FailedToStart`, `FailedToStop`, `Tripped`; RETAIN `Starts`, `RunHours`, `HoursSinceService`; `ServiceDue` |
| settings | `StartTimeout : TIME := T#5s`, `StopTimeout : TIME := T#5s`, `ServiceInterval` |
| methods | `Start`, `Stop`, `Reset`, `SetMode`, `ServiceDone`; duty member methods |

1. `RunCommand` is the contactor output. In Auto it follows `AutoRun OR` a group's demand
   (`Demand`, §8); in Manual the operator's last accepted Start/Stop.
2. A start needs `StartPermit`-style conditions (not blocked); a running motor keeps running
   when the permissive drops, and stops on an interlock, a fault or out of service.
3. With feedback: commanded to run and no feedback for `StartTimeout` → `FailedToStart`;
   commanded to stop and still running for `StopTimeout` → `FailedToStop`. `Trip` latches
   `Tripped`. Each latches `Fault` (§4.3). Alarms: `TripAlarm` (High, "Protection trip"),
   `FailedToStartAlarm` (Medium), `FailedToStopAlarm` (High).
4. `Running` is the feedback, or the command without feedback. `Starts` counts rising edges of
   `Running`.

### 6.2 Variable-speed motor family

`FB_VariableSpeedMotor EXTENDS FB_RunDevice IMPLEMENTS ISpeedDevice`; `FB_VariableSpeedPump`,
`FB_VariableSpeedFan` and `FB_DosingPump` extend it.

1. Additional inputs `AutoSpeed`, `SpeedFeedback`, `HasSpeedFeedback := FALSE`; outputs
   `SpeedSetpoint` (the drive's reference, %), `Speed` (feedback, else the setpoint);
   settings `MinSpeed := 0.0`, `MaxSpeed := 100.0`, `SpeedDeviation := 5.0` (%),
   `DeviationTime := T#10s`.
2. `SpeedSetpoint` is 0 % while stopped, else the Auto or Manual speed clamped to
   `MinSpeed…MaxSpeed`: a drive started at 0 % with `MinSpeed > 0` runs at `MinSpeed`.
3. **A drive starts at 0 %:** the manual speed is 0 % after any restart and returns to 0 % when
   the drive stops; the operator may set a speed while stopped and then start. While running in
   Auto the manual speed follows `AutoSpeed`, so Manual continues at the same speed.
4. `SetSpeed` is accepted in Manual and clamped to `MinSpeed…MaxSpeed`; a non-finite value is
   refused.
5. With speed feedback, running with `|SpeedFeedback − SpeedSetpoint| > SpeedDeviation` for
   `DeviationTime` raises `SpeedDeviationAlarm` (Low); it latches no fault.

### 6.3 Heater

`FB_Heater EXTENDS FB_RunDevice IMPLEMENTS IPowerDevice`.

1. `RunCommand` drives the main contactor; `Trip` is the over-temperature switch. Additional
   input `AutoPower` (%); outputs `PowerOutput` (% to a thyristor controller) and
   `PulseOutput` (time-proportioned for a solid-state relay); settings `MaxPower := 100.0`,
   `PulsePeriod : TIME := T#10s`.
2. **A heater starts at 0 %:** as for a drive (§6.2.3), and `PowerOutput` is 0 % while stopped.

### 6.4 Reversing motor

`FB_ReversingMotor EXTENDS FB_Device IMPLEMENTS IReversingDevice, IAlarmSource, IServiceable`.

1. Inputs `AutoForward`, `AutoReverse`, `ForwardFeedback`, `ReverseFeedback`, `Trip`,
   `HasFeedback := TRUE`; outputs `ForwardCommand`, `ReverseCommand` (never both), `Running`,
   `Forward`, `Reverse`, the counters and service count of §4.7; settings `StartTimeout`,
   `StopTimeout`, `ServiceInterval`, `ReversalDelay : TIME := T#2s`.
2. A change of direction stops the motor and waits `ReversalDelay` after it has stopped before
   the other direction is commanded. Both Auto inputs TRUE means stop.

### 6.5 Two-speed motor

`FB_TwoSpeedMotor EXTENDS FB_Device IMPLEMENTS ITwoSpeedDevice, IAlarmSource, IServiceable`.

1. Inputs `AutoRun`, `AutoFast`, `SlowFeedback`, `FastFeedback`, `Trip`, `HasFeedback := TRUE`;
   outputs `SlowCommand`, `FastCommand` (never both), `Running`, `Fast`, the counters and service
   count of §4.7; settings `StartTimeout`, `StopTimeout`, `ServiceInterval`,
   `ChangeoverDelay : TIME := T#1s`.
2. Changing speed while running drops both outputs for `ChangeoverDelay`.

### 6.6 On/off valve and damper

`FB_OnOffValve EXTENDS FB_Device IMPLEMENTS IOpenCloseDevice, IAlarmSource`; `FB_Damper`
extends it.

| Kind | Members |
|---|---|
| inputs | `AutoOpen`, `OpenFeedback`, `ClosedFeedback`, `HasOpenFeedback := TRUE`, `HasClosedFeedback := TRUE`, `FailOpen := FALSE` |
| outputs | `OpenCommand`, `Solenoid`, `Opened`, `Closed`, `Moving`, `FailedToOpen`, `FailedToClose`, `FeedbackConflict`, `OpenPermit`, `ClosePermit`; RETAIN `Operations` |
| settings | `OpenTimeout : TIME := T#10s`, `CloseTimeout : TIME := T#10s` |

1. `OpenCommand` is the commanded position; `Solenoid := OpenCommand XOR FailOpen`. The safe
   state sets `OpenCommand := FailOpen`, so `Solenoid` is de-energised.
2. Supervision: not open within `OpenTimeout` → `FailedToOpen`; not closed within
   `CloseTimeout` → `FailedToClose`; both end switches → `FeedbackConflict`; leaving the
   commanded end position counts as a failure to hold it. Each latches `Fault`.
3. Without a feedback the corresponding end position follows the command.
4. Moving away from the fail position needs the permissive and is refused while blocked; moving to
   it is always accepted in Manual. `OpenPermit` and `ClosePermit` give the panel's permits.

### 6.7 Motor-operated valve

`FB_MotorValve EXTENDS FB_Device IMPLEMENTS IOpenCloseDevice, ITravelStop, IAlarmSource`.

1. Inputs `AutoOpen`, `OpenLimit`, `ClosedLimit`, `TorqueTrip`, `Trip`; outputs
   `OpenCommand`, `CloseCommand` (never both), `Opened`, `Closed`, `Moving`, `Operations`;
   setting `TravelTimeout : TIME := T#60s`.
2. A motor drives until its end limit, then stops. `StopTravel` (Manual) stops in place.
   The safe state stops in place. Travel beyond `TravelTimeout` or a torque/thermal trip
   latches `Fault`.

### 6.8 Cylinder

`FB_Cylinder EXTENDS FB_Device IMPLEMENTS IExtendRetractDevice, IAlarmSource`.

1. Inputs `AutoExtend`, `ExtendedSensor`, `RetractedSensor`, `HasExtendedSensor := TRUE`,
   `HasRetractedSensor := TRUE`, `DoubleActing := FALSE`; outputs `ExtendCommand` (solenoid A),
   `RetractCommand` (solenoid B, double-acting only), `Extended`, `Retracted`, `Moving`,
   `ExtendPermit`, `RetractPermit`, `Operations`; settings `ExtendTimeout`, `RetractTimeout`
   (`T#5s`).
2. Single-acting: the spring retracts it, so the safe state is retracted and extending is the
   start. Double-acting: the safe state de-energises both solenoids and the cylinder stays where it
   is; every movement is a start.
3. Supervision as for the on/off valve.

### 6.9 Control valve and modulating damper

`FB_ControlValve EXTENDS FB_Device IMPLEMENTS IPositionDevice, IAlarmSource`;
`FB_ModulatingDamper` extends it.

1. Inputs `AutoPosition` (%), `PositionFeedback` (%), `HasFeedback := FALSE`,
   `FailPosition := 0.0`; outputs `PositionSetpoint` (%), `Position` (feedback, else the
   setpoint), `InPosition`, `Deviating`, `ManualPosition`; settings `MinPosition := 0.0`, `MaxPosition := 100.0`,
   `PositionDeviation := 5.0` (%), `DeviationTime : TIME := T#10s`.
2. `PositionSetpoint` is the clamped Auto or Manual position; the safe state is `FailPosition`
   (not clamped). Signal inversion for reverse-acting positioners belongs to the I/O scaling.
3. With feedback, a deviation beyond `PositionDeviation` for `DeviationTime` raises
   `DeviationAlarm` (Low) without latching a fault.
4. Leaving the fail position is the start: it needs the permissive; once released the valve keeps
   following its position when the permissive drops. `StartPermit` gates `SetPosition`.

## 7. Instruments and the tank

### 7.1 Analog input

`FB_AnalogInput IMPLEMENTS IMeasurement, IAlarmSource`; `FB_LevelInput`, `FB_PressureInput`,
`FB_TemperatureInput`, `FB_AnalyzerInput` and `FB_FlowInput` extend it.

1. Inputs `Raw : LREAL` (the channel value, for example mA), `SignalFault : BOOL` (the I/O
   module's diagnosis); public `Unit : STRING[16]` (set by the instance initialiser).
2. Settings: `RawLow := 4.0`, `RawHigh := 20.0`, `RangeLow := 0.0`, `RangeHigh := 100.0`,
   `BadBelow := 3.6`, `BadAbove := 21.0`, `FilterTime : TIME := T#0s`, `HighHighEnabled`,
   `HighEnabled`, `LowEnabled`, `LowLowEnabled` (`FALSE`), `HighHighLimit := 95.0`,
   `HighLimit := 90.0`, `LowLimit := 10.0`, `LowLowLimit := 5.0`, `Deadband := 1.0`,
   `AlarmDelay : TIME := T#0s`.
3. `Value` is `Raw` scaled linearly from `RawLow…RawHigh` to `RangeLow…RangeHigh`, filtered by
   `Control.LowPass` when `FilterTime > 0`. `Percent` is its position in the range.
4. `Quality` is `Bad` with `SignalFault`, a non-finite `Raw`, `Raw` below `BadBelow` or above
   `BadAbove`, or a zero raw span; else `Good`. While bad, `Value` holds the last good value,
   `Fault` is TRUE, `SignalAlarm` (Medium, "Signal fault") is active and the limit alarms are
   suppressed so a broken wire raises one alarm, not four.
5. Limit alarms (`HighHighAlarm` High, `HighAlarm` Medium, `LowAlarm` Medium, `LowLowAlarm`
   High) are `Alarm.LimitAlarm`s with `Deadband` and `AlarmDelay`. A limit that is not enabled,
   and every limit while the signal is bad, is suppressed by design: it does not annunciate.
   Outputs `HighHigh`, `High`, `Low`, `LowLow` are TRUE while their alarm is active and not
   suppressed.
6. `FB_FlowInput` adds a totaliser (§7.3) on its own `Value`: outputs `Total` (RETAIN, so it
   reads right from the restart on), setting
   `TotalPer : TIME := T#1h` (the flow unit's time base), `ResetTotal()` (Operator).

### 7.2 Digital input

`FB_DigitalInput IMPLEMENTS IAlarmSource`; `FB_LevelSwitch` and `FB_PressureSwitch` extend it.

1. Inputs `Input`, `SignalFault`; outputs `Value` (debounced, inverted as set), `Quality`;
   settings `Invert := FALSE`, `Debounce : TIME := T#0s`, `AlarmEnabled := FALSE`,
   `AlarmWhen := TRUE`, `AlarmText : STRING[80] := 'Switch active'`.
2. `SwitchAlarm` (Medium) is active while enabled and `Value = AlarmWhen`; `SignalAlarm` while
   `SignalFault`.

### 7.3 Totaliser and counter

1. `FB_Totalizer IMPLEMENTS ITotal`: input `Measurement : IMeasurement` (composition), else
   `Rate` (quantity per `TotalPer`) and `RateGood := TRUE`;
   output RETAIN `Total`; settings `TotalPer : TIME := T#1h`, `CutOff := 0.0` (rates at or
   below are not counted). A bad rate is not counted. `ResetTotal()` zeroes it.
2. `FB_Counter IMPLEMENTS ICounter`: input `Pulse` (counts rising edges); outputs RETAIN `Count`,
   `Done` (`Preset > 0` and `Count >= Preset`); setting `Preset : UDINT := 0`; `ResetCount()`.

### 7.4 Weigh scale

`FB_WeighScale EXTENDS FB_AnalogInput IMPLEMENTS ITare`: outputs `Net := Value − TareWeight`,
RETAIN `TareWeight`, `Stable` (value within `StableBand` for `StableTime`); settings
`StableBand := 0.5`, `StableTime : TIME := T#2s`. `Tare` is accepted only while stable and good.

### 7.5 Tank

`FB_Tank IMPLEMENTS IMeasurement`: input `LevelInstrument : IMeasurement` (composition); outputs
`Level`, `Percent`, `Volume`, `Quality`; settings `Height := 100.0` (level at 100 %),
`Capacity := 0.0` (volume at `Height`, 0 = no volume). The tank owns no filter and no alarms;
its level instrument owns them.

## 8. Controllers

### 8.1 PID loop

A PID loop is no block of this library. It is ordinary ST around the Control library's
`Control.PID` (specification 45), executed through `Configure`, `Initialize` and `Update` as the
control course does: Create PLC code writes it ([spec 39 §10.5](39-hmi-builder.md)), and an
engineer may write the same code by hand. A block around the PID, holding its own copy of the
gains, would hide the PID's parameter record from Control Studio; the user decided on
7 October 2026 that loops are ordinary code.

1. The loop `TIC101` is the PID `TIC101 : Control.PID` with variables beside it, named after it:
   `TIC101_Input` (`Control.PIDInput`), `TIC101_Start` (`Control.PIDInitialState`) and
   `TIC101_Initialized`, rebuilt after a restart; and, `RETAIN` so a restart resumes without a
   bump and without losing what was set, `TIC101_Parameters` (`Control.PIDParameters`, the one
   record the HMI's settings and Control Studio change), `TIC101_Manual` (the operator's mode,
   TRUE Manual, default TRUE), `TIC101_Setpoint`, `TIC101_ManualOutput` and `TIC101_Output`.
2. Every scan the code configures the PID from `TIC101_Parameters`, passes the setpoint, the
   manual output and the task's cycle time, and takes the measurement from an instrument
   through `IMeasurement` while its quality is Good. A bad measurement holds the output in
   Manual (`PIDMode#Manual` with the held output) and clears `TIC101_Initialized`; the first
   good measurement initialises the PID from it and from the present output, so the loop
   continues without a bump. A restart initialises it the same way.
3. In Auto the manual output follows the output, so Manual takes over where the loop is; from
   Manual the PID continues from the manual output (specification 45 §3), so Auto starts without
   a bump.
4. The three gains share one sign (specification 37, §conversion domain): positive for direct
   action, negative for reverse action, when the output must rise while the measurement is above
   the setpoint. Defaults: `ProportionalGain` 1.0, `IntegralGain` 0.1, `DerivativeGain` 0,
   output 0 to 100 %, rate limits ±1·10⁹ per second (off), `TrackingTime` 1 s, back-calculation
   anti-windup.
5. The library's PID panel (`plant-pid`) and component definition bind these variables
   (`{instance}_Setpoint`, §8.5 of spec 39); the operator's mode, setpoint and manual output are
   written through write rules, which Create PLC code adds (spec 39 §12.2).

### 8.2 Cascade

Two loops are a cascade when the outer loop's output is the inner loop's setpoint
(`TIC201_Setpoint := TIC200_Output`); while the inner loop is in Manual the outer loop tracks the
inner loop's measurement (`PIDMode#Track`), so closing the cascade is bumpless. Create PLC code
writes this when the outer controller is connected to the inner one; each loop is operated
through its own PID panel.

### 8.3 On/off controller

`FB_OnOffController IMPLEMENTS ISetpoint`: input `Measurement : IMeasurement` (composition), else
`ProcessValue` and `ProcessValueGood := TRUE`;
outputs `Demand : BOOL` for a device's `Auto…` input and `MeasuredValue`; RETAIN `Setpoint`; settings
`Hysteresis := 1.0`, `Heating := TRUE` (demand below the setpoint; FALSE for cooling),
`SetpointMin`, `SetpointMax`. A bad process value drops the demand.

## 9. Conditions, groups, managers and sequences

### 9.1 Condition sets

`FB_Interlocks` and `FB_Permissives` (`IBypass`) and `FB_Protection` (no bypass) share
`FB_ConditionSet`.

1. Inputs `Condition1…Condition8 : BOOL := TRUE` (TRUE = healthy) and `Text1…Text8`; outputs
   `Healthy` (all healthy or bypassed), `Active` (= NOT `Healthy`), `Text` (the first-out
   unhealthy condition's text, held until all are healthy again), `FirstOut`, `States[1..8]`,
   `Bypassed[1..8]`, `BypassActive`, and `InUse[1..8]`: TRUE while the condition has a text or is
   not healthy. The panels show a condition's row only while it is in use, so a set of three
   conditions shows three rows, and an unhealthy condition without a text is never hidden
   (refined on 7 October 2026, when every panel listed all eight rows).
2. `Bypass(Index)` (Engineer) bypasses one condition for the setting
   `BypassTime : TIME := T#1h`, after which the bypass ends by itself; `RemoveBypass` ends it
   early. A bypass is never retained.
3. Use: `P101(Interlock := P101Trips.Active, InterlockText := P101Trips.Text)`.

### 9.2 Groups

`FB_DutyStandbyGroup`, `FB_LeadLagGroup` and `FB_StagedGroup` share `FB_Group`.

1. Members by composition: inputs `Member1…Member4 : IDutyMember` — a group never registers.
   A member runs from the group only in Auto; its own interlocks, faults and supervision apply.
2. Outputs `RunningCount`, `AvailableCount`, `Target` (members wanted running), `Shortfall`,
   `Selected[1..4]`; alarm `ShortfallAlarm` (High, "Not enough equipment available") after
   `ShortfallDelay : TIME := T#10s`.
3. Selection: a member is started when more are required, choosing the available member with
   the fewest run hours (`Preferred : INT := 0` first when set); a member that becomes
   unavailable is replaced; members are started and stopped one at a time, `StartDelay` /
   `StopDelay` (`T#2s`) apart.
4. `FB_DutyStandbyGroup`: input `Run`; one member. `FB_LeadLagGroup`: input `Required : INT`.
   `FB_StagedGroup`: input `Demand` (%) → members by equal stages with `Hysteresis := 5.0` %
   and `MinRunTime`, `MinStopTime` (`T#0s`).

### 9.3 Alarm manager

`FB_AlarmManager IMPLEMENTS IAlarmManager`: inputs `Source1…Source16 : IAlarmSource`; outputs
`ActiveCount` (the active alarms that are not suppressed, shelved or out of service),
`UnacknowledgedCount`, `Horn` (TRUE from a new unacknowledged alarm until `Silence`). `AcknowledgeAll` acknowledges the alarms present now; a later occurrence stays
unacknowledged. The HMI alarm list does not need a manager: the runtime lists every alarm.

### 9.4 Sequences

`FB_Sequence` (abstract) `IMPLEMENTS ISequence`.

1. `E_SequenceState : (Idle, Running, Held, Stopping, Stopped, Completed, Aborted, Failed)`.
   Outputs `State`, `CurrentStep : INT`, `StepName : STRING[80]`, `StepTime : TIME` (frozen while
   held), `Message : STRING[120]` (what it waits for, or why it failed), and for the panel
   `StateText` and the permits `CanStart`, `CanHold`, `CanResume`, `CanStop`, `CanAbort`,
   `CanReset`.
2. Commands (Operator): `Start` (Idle → Running at step 0), `Hold` (Running → Held), `Resume`
   (Held → Running), `Stop` (Running/Held → Stopping), `Abort` (Running/Held/Stopping →
   Aborted), `Reset` (Stopped/Completed/Aborted/Failed → Idle). Any other command is refused.
3. The application extends it: its body calls `Execute()`, which applies the command and the
   step timer and returns TRUE while Running; its CASE on `CurrentStep` uses the protected helpers
   `GoTo(Number, Name)`, `Wait(Text)`, `Complete()`, `Fail(Text)`, `StopDone()` and
   `StepTimeout(Limit, Text) : BOOL`. Outputs that command equipment are ordinary outputs wired
   to the devices' `Auto…` inputs; Stopping is handled by the application, which calls
   `StopDone()`.
4. Nothing is retained: after any restart the sequence is Idle.
5. `FB_SequenceManager IMPLEMENTS ISequenceManager`: inputs `Sequence1…Sequence8 : ISequence`;
   outputs `RunningCount` (Running or Stopping: still commanding equipment), `HeldCount`,
   `FailedCount`; `StopAll`, `AbortAll`. The program
   executes each sequence; the manager only observes and commands them.

## 10. Settings

1. A setting is a public RETAIN variable of the block (`VAR PUBLIC RETAIN`), with its default in
   the declaration. An instance initialiser overrides it for one instance:
   `P102 : Plant.FB_Pump := (StartTimeout := T#10s);`.
2. Its metadata is declared once, in the block's component definition (`[[settings]]` in
   `hmi/pairs/*.toml`, [catalogue §8](hmi-component-catalogue.md)): member, title in plain
   words, unit, minimum, maximum, step or choices, and an optional one-line help. The editor's
   inspector, the runtime's Settings dialog and its validation read that same description.
3. Changing a setting while running is an explicit Engineer action in the operator Settings
   dialog: the runtime validates every value against the metadata and the variable's type and
   writes all changed values of one Apply between two scans, or none. It reports the result
   beside Apply and reads the values back.
4. There is no settings provider, transaction log, revision counter or settings method in PLC
   code. A block that needs a consistent set reads its settings once per scan; a customer block
   gets the same Settings dialog by declaring public RETAIN variables and `[[settings]]`.

## 11. Retention

Following IEC 61131-3:2013 §6.5.6.1–6.5.6.2 and [specification 10 §6.7](10-runtime-semantics.md):
RETAIN variables declared inside a function block, in every instance a global or a program
variable holds and in the instances nested in it, are saved in the runtime's retain store and
restored on a warm restart, including after power loss. Variables declared NON_RETAIN, and those
without a qualifier in a block instance that is not itself declared RETAIN, start from their
initial values. Retained values do not include references or interface values.

## 12. HMI integration

1. **Component definitions** (`hmi/pairs/*.toml`) map each block to its symbol, panel, symbol
   bindings, settings metadata, hardware terminals and dependencies. The runtime, the editor
   and `hmi check` read the same file with one parser.
2. **Commands:** panel buttons name an interface and a method; the runtime calls it only when
   the instance implements that interface and the user has the command's role, takes the
   method's BOOL result as accepted or refused, and shows `CommandText` when refused. The
   button's `interlock` permit (`StartPermit`, `Manual`) greys it with the same reason.
3. **Discovery:** the runtime finds alarms, devices and sequences by walking instances; their
   names are instance paths (`Main.P101`). Nothing is registered.
4. The supplied panels show the tag in the title, the mode selector (Auto/Manual, Operator),
   the commands, the state text, the counters, and an Engineer-only Out of service / Return to
   service pair and Settings button.

## 13. Examples and manual

1. `libraries/plant/examples/` holds runnable projects: `equipment` (one of each block),
   `crusher_lubrication` (duty/standby pumps, a permissive, a sequence), `wastewater_station`
   (a level loop, the real `Control.PID` in ordinary code, a staged group of variable-speed pumps),
   `packaging_machine` (conveyor, cylinder, reject sequence) and `sequences` (customer sequences
   and the sequence overview).
2. The Plant manual ([guide](../guides/PLANT_LIBRARY_GUIDE.md) and public page) shows each block
   with a complete example; a native test compiles every manual example.

## 14. Acceptance (native tests)

Native runtime tests execute the real library sources and assert at least:
1. fail positions: a fail-open valve opens on interlock, fault and out of service;
2. supervision active in Auto and Manual whoever commanded, off only out of service;
3. bumpless Auto↔Manual for a motor, a drive and a control valve;
4. refused manual commands in Auto, with `CommandText`; no pending start after refusal;
5. trips and supervision faults latch until Reset, and Reset never starts;
6. drives and heaters at 0 % after restart and after a stop;
7. retention of settings, counters and mode across a power loss (retain store round trip),
   and loss of manual commands;
8. the PID loop Create PLC code writes compiles with the library and runs: bumpless Manual to
   Auto, the manual output following in Auto, a bad measurement holding the output and the loop
   starting again without a bump, and a warm restart keeping its values (the generator's tests,
   spec 39 §10.5);
9. groups replace a failed member, rotate by run hours and raise a shortfall;
10. sequence lifecycle transitions and refusals;
11. analog quality, scaling, filtering, suppression of limit alarms while bad;
12. every manual example compiles and runs.

## 15. Removed with this rebuild

Ownership (Acquire/Release, owners, take control); command contexts, request identifiers and
receipts; registration, object identities and numeric IDs; settings providers, transactions,
definition pages and audits; the event, recording and maintenance managers (the runtime's
history, trends and alarm records serve them; devices keep their own counters); cross-task
request queues; the `Equipment` namespace, its compatibility copy, faceplates and pairs. On
7 October 2026 also `FB_PIDLoop`, `FB_CascadeLoop`, `ILoop`, `E_LoopMode` and the cascade panel: a
loop is ordinary code around `Control.PID` (§8.1).

## 16. References

[Spec 45 — Control.PID](45-control-pid.md), [spec 40 — Alarm library](40-alarm-library.md),
[spec 39 — HMI](39-hmi-builder.md), [component catalogue](hmi-component-catalogue.md),
[control platform purpose](control-platform-purpose.md). Research behind the patterns (not a
certification claim): [PLCopen OOP guidelines](https://www.plcopen.org/news/plcopen-releases-guidelines-for-object-orientation/),
[methods, properties and inheritance](https://stefanhenneken.net/2017/04/23/iec-61131-3-methods-properties-and-inheritance/),
[ISA-88 equipment modules](https://www.merobix.com/blog/what-is-an-isa-88-equipment-module).
