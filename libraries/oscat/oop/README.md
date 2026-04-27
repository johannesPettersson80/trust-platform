# OSCAT OOP

OSCAT OOP is an object-oriented facade over the classic `libraries/oscat`
package. It keeps classic OSCAT as the behavior source of truth and adds
component-shaped function blocks for stateful or domain-oriented workflows.

Use this package when application code benefits from object identity,
read-only status properties, explicit scan methods, and narrow interfaces.
Use classic OSCAT directly for pure scalar helper functions and for vendor or
legacy code that already targets OSCAT names.

## Package

```toml
[dependencies]
OscatOop = { path = "../../libraries/oscat/oop", version = "0.1.0" }
```

## Naming

New truST-owned API names use readable PascalCase domain names:
`PidController`, `Pt1Filter`, `DwordFifo16`, `DefaultPidKp`.
Inherited OSCAT names remain unchanged in the classic package:
`CTRL_PID`, `FT_PT1`, `FIFO_16`, `STRING_LENGTH`.

## Components

- `AutomationContext`
- `UnitConverter`
- `Pt1Filter`
- `Pt2Filter`
- `DerivativeFilter`
- `IntegratorFilter`
- `DelayLine16`
- `PiController`
- `PidController`
- `PwmController`
- `HysteresisSwitch`
- `PulseGenerator`
- `RandomSignalGenerator`
- `SineSignalGenerator`
- `SquareSignalGenerator`
- `ByteRamp`
- `WordRamp`
- `DwordFifo16`
- `DwordFifo32`
- `DwordStack16`
- `DwordStack32`
- `Latch`
- `ToggleSwitch`
- `DwordCounter`
- `ShiftRegister8`
- `OntimeMeter`
- `CycleTimeMeter`
- `Calibrator`
- `BarGraphMeter`
- `CalendarClock`
- `HolidayCalendar`
- `RtcClock`
- `TankLevelController`
- `HeatCurve`
- `BoilerController`
- `SingleOutputDriver`

Each component is covered by Structured Text tests under
`crates/trust-runtime/tests/fixtures/oscat/oop_core`.

## Documentation And Examples

- User guide: `docs/guides/OSCAT_OOP_LIBRARY_GUIDE.md`
- Public docs page: `docs/public/develop/libraries/oscat-oop.md`
- Comparison examples: `examples/OSCAT/<example>/{non-oop,oop}`

The example suite contains 49 comparison pairs under `examples/OSCAT`:

- 27 hand-written real-world machine/process pattern scenarios;
- 20 compact component-composition showcases;
- 2 compact pattern showcases for polymorphism and composition.

Each item has one teaching README at `examples/OSCAT/<example>/README.md`,
plus a classic `non-oop` project and an OSCAT OOP `oop` project with
application Structured Text and Structured Text tests.

The examples are process-first, not component-first. Calibration, historian
records, diagnostics snapshots, communication boundaries, and signal
conditioning are shown inside complete machines such as a water booster pump
station, multi-product batch reactor, chemical dosing skid, airport baggage
diverter, boiler room heating plant, pasteurizer, cold storage plant, and
district pump network.

The hand-written pattern catalog is documented in:

- `docs/internal/references/OSCAT/oscat_oop_realworld_pattern_catalog.md`
