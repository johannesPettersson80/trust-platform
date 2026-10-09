# Crusher lubrication

Two lubrication pumps share an `FB_DutyStandbyGroup`; an `FB_PressureInput` measures the oil
pressure (4-20 mA for 0-10 bar); an `FB_Permissives` holds the pumps while the oil level is low;
and `LubricationSequence`, an application block extending `FB_Sequence`, proves lubrication
before it gives the crusher its permit.

| Signal | Meaning |
|---|---|
| `StartButton`, `StopButton` | Start and stop the lubrication sequence |
| `OilLevelOk`, `OilFlow` | Oil level switch and flow switch |
| `PressureSignal` | Oil pressure transmitter, 4-20 mA |
| `MainPumpRunning`, `StandbyPumpRunning` | Contactor feedback |
| `MainPumpTrip`, `StandbyPumpTrip` | Overload relays |
| `MainPumpRun`, `StandbyPumpRun` | Contactor outputs |
| `CrusherPermit` | Lubrication proven: the crusher may run |

How it behaves:
- Start runs the duty pump. Flow and at least 2 bar must follow within 10 s, else the sequence
  fails; losing either later fails it too and withdraws the permit.
- A pump that trips or fails to start becomes unavailable, and the group starts the other one.
- Stop stops the pumps and ends once they have stopped. Reset returns the sequence to Idle.
- An operator can put a pump in Manual from its panel; the group then uses the other pump.
- `CrusherPermit` is a process permission, not a certified safety output.
