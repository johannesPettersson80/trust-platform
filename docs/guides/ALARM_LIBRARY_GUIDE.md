# Alarm Library Guide

The Alarm library defines alarms as function blocks, following ISA-18.2 (IEC 62682) and EEMUA 191.
Every alarm is an instance in the program. The program calls it every scan and can use its outputs
for horns, beacons and interlocks without any HMI. The truST runtime lists every alarm on its own,
shows them in the runtime display, applies operator actions and records every transition. The
contract is specification 40, `docs/specs/40-alarm-library.md`.

Use the detection blocks for the usual cases: a condition, a limit, a deviation, or a command its
feedback does not follow. Extend the base block when your alarm needs its own detection; it then
gets the whole lifecycle, the records and the display.

## Package Setup

```toml
[project]
include_paths = ["src"]

[dependencies]
Alarm = { path = "../../libraries/alarm", version = "0.1.0" }
```

The program writes `USING Alarm;`. The Plant library uses this library for the alarms of its
blocks, so a project with Plant already has it.

## Design Rules

- One instance per alarm, called once in every scan with its inputs.
- The alarm's identity is its instance path, for example `Tank.LevelHigh`. The alarm list, the
  records and the OpenOT history name it so; the HMI files hold no alarm definitions.
- Three priorities, `High`, `Medium` and `Low`: most alarms Low, few High. A trip is not a
  priority: the alarm that caused it is a High alarm marked first out, and the trip itself is an
  event. Information is never an alarm; the program sends it as an OpenOT message record.
- Operator actions reach the alarm through its `Request` member, which the alarm applies in its
  next scan and then clears. The program uses the methods `Acknowledge` (a local push button) and
  `Reset` (a latched alarm).
- Declared in a `VAR RETAIN` section, an alarm keeps its state, its shelving and its out-of-service
  mark across a warm restart; otherwise it starts `Normal`.
- A value that is not finite counts as beyond a limit: a broken measurement is not hidden.
- Blocks that hold alarms, such as the Plant library's motors and valves, hold them as members
  (`P101.FailedToStartAlarm`), so the display shows each alarm on its equipment.

## Blocks

| Block | Extends | Active while |
| --- | --- | --- |
| `Alarm` | | the base: lifecycle, message, priority, delays, latching, shelving, suppression by design, out of service, first out; own detection calls its protected method `Evaluate(Condition)` |
| `DigitalAlarm` | `Alarm` | `Condition` is TRUE |
| `LimitAlarm` | `Alarm` | High: from `Value > Limit` until `Value < Limit - Deadband`; Low: from `Value < Limit` until `Value > Limit + Deadband` |
| `DeviationAlarm` | `Alarm` | from `ABS(Value - Reference) > Limit` until it is below `Limit - Deadband` |
| `DiscrepancyAlarm` | `Alarm` | `Command <> Feedback`; `OnDelay` is the time the equipment may take to follow |
| `FirstOutGroup` | | marks the first alarm of its group to become active, until the group is reset |

## Common Types

- `AlarmPriority`: `Low := 1`, `Medium := 2`, `High := 3`.
- `AlarmState`: `Normal`, `ActiveUnacknowledged`, `ActiveAcknowledged`, `ClearedUnacknowledged`.
- `LimitDirection`: `High`, `Low`.
- `AlarmRequest`: `Acknowledge`, `Shelve` (a duration), `Unshelve`, `Reset`, `Comment`,
  `RemoveFromService`, `ReturnToService`; written by the runtime for an operator's action.

## Lifecycle

| From | Event | To |
| --- | --- | --- |
| `Normal` | the condition becomes active (after `OnDelay`) | `ActiveUnacknowledged` |
| `ActiveUnacknowledged` | acknowledged | `ActiveAcknowledged` |
| `ActiveUnacknowledged` | the condition clears (after `OffDelay`), not latching | `ClearedUnacknowledged` |
| `ActiveAcknowledged` | the condition clears, not latching | `Normal` |
| `ClearedUnacknowledged` | acknowledged | `Normal` |
| `ClearedUnacknowledged` | the condition becomes active again | `ActiveUnacknowledged` |

- An alarm that clears before it is acknowledged stays listed until it is acknowledged.
- `RaiseCount` grows by one each time the alarm becomes active.
- **Shelved** by an operator for 1 s to 24 h, the alarm does not annunciate and needs no
  acknowledgement, but its condition is still followed. It returns by itself when the time ends,
  or when an operator unshelves it, as `ActiveUnacknowledged` if its condition holds.
- **Suppressed by design** while the program's `Suppressed` input is TRUE (for example the low-flow
  alarm of a stopped pump), and **out of service** while an engineer has taken it out for
  maintenance: in both cases it does not annunciate and returns as from shelving.
- **Latching:** the alarm stays active after its condition clears, until `Reset()` or an
  operator's reset, which take effect only once the condition is gone.
- **First out:** of the alarms whose `Group` refers to one `FirstOutGroup`, the first to become
  active after the group's last `Reset()` has `FirstOut` TRUE until the group is reset again.

## Interface Summary

```st
Alarm             Message, Priority (Low), OnDelay (0 s), OffDelay (0 s), Latching, Suppressed,
                  Group (REF_TO FirstOutGroup)
                  -> Active, Unacknowledged, State, Shelved, OutOfService, SuppressedByDesign,
                     FirstOut, Annunciating, RaiseCount
                  METHOD Acknowledge; METHOD Reset; METHOD PROTECTED Evaluate(Condition)
DigitalAlarm      Condition
LimitAlarm        Value, Limit, Deadband (0), Direction (High)
DeviationAlarm    Value, Reference, Limit, Deadband (0)
DiscrepancyAlarm  Command, Feedback
FirstOutGroup     -> Epoch; METHOD Reset; METHOD Join : UDINT (used by the alarms)
```

`Annunciating` is TRUE while the alarm is active or unacknowledged and not shelved, suppressed or
out of service: the output for a horn or a beacon.

## Operating From The HMI

The runtime display shows the alarms without any HMI file:

- the top bar's counter of annunciating alarms, in the mark and colour of the highest priority,
  blinking while any is unacknowledged, and beside it the newest alarm;
- the alarm list: unacknowledged first, then by priority, then newest first; shelved, suppressed
  and out-of-service alarms listed apart with their counts, never hidden without a trace;
- on each symbol, the mark of its equipment's annunciating alarms, which opens the alarm;
- the event log of alarm transitions, operator actions and command outcomes.

High is a red diamond, Medium an orange triangle and Low a yellow circle, each with "!".

| Action | Role | Request field |
| --- | --- | --- |
| Acknowledge | Operator | `Acknowledge` |
| Shelve for a duration (1 s to 24 h) | Operator | `Shelve` |
| Unshelve | Operator | `Unshelve` |
| Reset a latched alarm | Operator | `Reset` |
| Comment | Operator | `Comment` |
| Out of service | Engineer | `RemoveFromService` |
| Back in service | Engineer | `ReturnToService` |

Each action needs an HMI session with the role, goes to the audit log with the user's name, and is
confirmed when the alarm shows it applied. The library defines the actions and their roles, so the
runtime's write policy needs no entry for them. With `[runtime.openot]` enabled, every transition
is published as an OpenOT condition record; those records are the alarm history.

## Examples

Level limits of a tank, high and high-high, each with its own priority:

```iecst
USING Alarm;

PROGRAM Tank
VAR
    Level : LREAL;
END_VAR
VAR RETAIN
    LevelHigh : LimitAlarm;
    LevelHighHigh : LimitAlarm;
    LevelLow : LimitAlarm;
END_VAR
    LevelHigh(Value := Level, Limit := 85.0, Deadband := 2.0,
        Message := 'Tank T-101 level high', Priority := AlarmPriority#Medium);
    LevelHighHigh(Value := Level, Limit := 95.0, Deadband := 2.0, OnDelay := T#2s,
        Message := 'Tank T-101 level high-high', Priority := AlarmPriority#High);
    LevelLow(Value := Level, Limit := 10.0, Deadband := 2.0, Direction := LimitDirection#Low,
        Message := 'Tank T-101 level low');
END_PROGRAM
```

A pump that does not start within 5 s of its command, with a horn and a local acknowledge button:

```iecst
USING Alarm;

PROGRAM Pumps
VAR
    StartCommand AT %QX0.0 : BOOL;
    RunningFeedback AT %IX0.0 : BOOL;
    AcknowledgeButton AT %IX0.1 : BOOL;
    Horn AT %QX0.1 : BOOL;
END_VAR
VAR RETAIN
    FailedToStart : DiscrepancyAlarm;
END_VAR
    FailedToStart(Command := StartCommand, Feedback := RunningFeedback, OnDelay := T#5s,
        Message := 'Pump P-101 failed to start', Priority := AlarmPriority#High);
    IF AcknowledgeButton THEN
        FailedToStart.Acknowledge();
    END_IF;
    Horn := FailedToStart.Annunciating AND FailedToStart.Unacknowledged;
END_PROGRAM
```

The first cause of a boiler trip: each trip alarm latches and joins one group; the reset button
resets the alarms and starts a new first out:

```iecst
USING Alarm;

PROGRAM Boiler
VAR
    FlameLost AT %IX0.0 : BOOL;
    WaterLow AT %IX0.1 : BOOL;
    PressureHigh AT %IX0.2 : BOOL;
    ResetButton AT %IX0.3 : BOOL;
    Burner AT %QX0.0 : BOOL;
    Trips : FirstOutGroup;
END_VAR
VAR RETAIN
    Flame : DigitalAlarm;
    Water : DigitalAlarm;
    Pressure : DigitalAlarm;
END_VAR
    Flame(Condition := FlameLost, Latching := TRUE, Group := REF(Trips),
        Message := 'Boiler flame lost', Priority := AlarmPriority#High);
    Water(Condition := WaterLow, Latching := TRUE, Group := REF(Trips),
        Message := 'Boiler water low', Priority := AlarmPriority#High);
    Pressure(Condition := PressureHigh, Latching := TRUE, Group := REF(Trips),
        Message := 'Boiler pressure high', Priority := AlarmPriority#High);
    IF ResetButton THEN
        Flame.Reset();
        Water.Reset();
        Pressure.Reset();
        Trips.Reset();
    END_IF;
    Burner := NOT (Flame.Active OR Water.Active OR Pressure.Active);
END_PROGRAM
```

A temperature that strays from its setpoint, suppressed while the heater is off:

```iecst
USING Alarm;

PROGRAM Oven
VAR
    Temperature : LREAL;
    Setpoint : LREAL := 180.0;
    HeaterOn : BOOL;
END_VAR
VAR RETAIN
    TemperatureDeviation : DeviationAlarm;
END_VAR
    TemperatureDeviation(Value := Temperature, Reference := Setpoint, Limit := 10.0,
        Deadband := 2.0, OnDelay := T#1m, Suppressed := NOT HeaterOn,
        Message := 'Oven temperature away from setpoint');
END_PROGRAM
```

Own detection: a pump running dry. The block extends the base and calls `Evaluate` with its
condition; message, priority and delays are the base's inputs:

```iecst
USING Alarm;

FUNCTION_BLOCK DryRunAlarm EXTENDS Alarm.Alarm
VAR_INPUT
    Running : BOOL;
    Flow : LREAL;
    MinimumFlow : LREAL := 1.0;
END_VAR
    Evaluate(Condition := Running AND Flow < MinimumFlow);
END_FUNCTION_BLOCK

PROGRAM Station
VAR
    PumpRunning : BOOL;
    Flow : LREAL;
END_VAR
VAR RETAIN
    DryRun : DryRunAlarm;
END_VAR
    DryRun(Running := PumpRunning, Flow := Flow, OnDelay := T#10s,
        Message := 'Pump P-101 running dry', Priority := AlarmPriority#High);
END_PROGRAM
```

## Validation

```bash
cargo test -p trust-runtime --test alarm_library
cargo test -p trust-runtime --test web_hmi_alarms
cargo test -p trust-runtime --test openot_alarm_records
cargo test -p trust-runtime --test alarm_guide
```

`alarm_library` runs the lifecycle of specification 40 on the blocks, `web_hmi_alarms` the alarm
list and the operator actions of the runtime display, `openot_alarm_records` the history, and
`alarm_guide` compiles and runs every example of this guide.
