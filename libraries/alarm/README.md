# truST Alarm Library

The Alarm library provides alarms as function blocks, following ISA-18.2 (IEC 62682) and
EEMUA 191. Every alarm is an instance in the program; the truST runtime lists them, applies
operator actions and records every transition. The full contract is
[specification 40](../../docs/specs/40-alarm-library.md); the manual is the
[Alarm library guide](../../docs/guides/ALARM_LIBRARY_GUIDE.md).

The public namespace contains:

- `Alarm.Alarm`, the base: lifecycle, message, priority, on- and off-delays, latching, shelving,
  suppression by design, out of service and first out, with `Acknowledge` and `Reset` methods;
- `Alarm.DigitalAlarm`, `Alarm.LimitAlarm`, `Alarm.DeviationAlarm` and `Alarm.DiscrepancyAlarm`,
  which extend the base;
- `Alarm.FirstOutGroup`, which marks the first alarm of a group;
- the enums `AlarmPriority` (Low, Medium, High), `AlarmState` and `LimitDirection`.

Add the package to a project's `trust-lsp.toml`:

```toml
[dependencies]
Alarm = { path = "../../libraries/alarm", version = "0.1.0" }
```

Call each alarm every scan with its inputs and read its outputs:

```iecst
USING Alarm;

PROGRAM Tank
VAR
    Level : LREAL;
END_VAR
VAR RETAIN
    LevelHigh : LimitAlarm;
    LevelHighHigh : LimitAlarm;
END_VAR
    LevelHigh(Value := Level, Limit := 85.0, Deadband := 2.0,
        Message := 'Tank level high', Priority := AlarmPriority#Medium);
    LevelHighHigh(Value := Level, Limit := 95.0, Deadband := 2.0, OnDelay := T#2s,
        Message := 'Tank level high-high', Priority := AlarmPriority#High);
END_PROGRAM
```

Declared in a `VAR RETAIN` section, an alarm keeps its state across a warm restart.

Own detection extends the base and calls `Evaluate` with its condition:

```iecst
FUNCTION_BLOCK DryRunAlarm EXTENDS Alarm.Alarm
VAR_INPUT
    Running : BOOL;
    Flow : REAL;
END_VAR
    Evaluate(Condition := Running AND Flow < 1.0);
END_FUNCTION_BLOCK
```
