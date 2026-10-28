# truST Plant library

Ready equipment, instruments, controllers, groups and sequences for truST, written once in
IEC 61131-3 object-oriented ST. Declare an instance, call it once per scan with its normal I/O,
and write only your process logic.

```iecst
VAR
    P101 : Plant.FB_Pump;
END_VAR
    P101(AutoRun := Level > 2.0, RunningFeedback := PumpRunning, Trip := Overload);
    PumpContactor := P101.RunCommand;
```

- **Auto** follows the program's `Auto…` inputs; **Manual** follows the operator's commands
  (`Start`, `Stop`, `Open`, `Close`, `SetSpeed`, `SetPosition`, …); **Out of service** holds the
  device safe. Changing between Auto and Manual never stops or moves anything.
- Interlocks force the safe state, permissives gate starts, trips and supervision failures latch
  until `Reset`, and valves fail to their fail position.
- Alarms are standard `Alarm` library instances inside each block; the runtime finds them itself.
- Settings are public RETAIN variables with their titles, units and limits in `hmi/pairs`; they
  survive power loss.
- The PID loop runs the real `Control.PID`.

Contents: `src/` (the library), `hmi/pairs/` and `hmi/panels/` (HMI component definitions and
operator panels), `examples/` (runnable reference projects).

Specification: [`docs/specs/trust-plant-library.md`](../../docs/specs/trust-plant-library.md).
Manual: [`docs/guides/PLANT_LIBRARY_GUIDE.md`](../../docs/guides/PLANT_LIBRARY_GUIDE.md).
