# OSCAT OOP Library Guide

OSCAT OOP is the object-oriented companion to the classic
`libraries/oscat` package. Classic OSCAT remains the behavior source of truth.
The component package adds small object-shaped wrappers for workflows where
state, identity, narrow interfaces, and readable scan code matter.

Use classic OSCAT when you need the upstream manual-aligned function or FB
surface directly. Use OSCAT OOP when application code benefits from a
named object with configuration methods, read-only status properties, and a
single scan method.

## Package Setup

```toml
[project]
include_paths = ["src"]
stdlib = "iec"

[dependencies]
OscatOop = { path = "../../libraries/oscat/oop", version = "0.1.0" }
```

The package depends on classic OSCAT internally:

```toml
[dependencies]
OSCAT = { path = "..", version = "0.1.0" }
```

Application projects normally depend on `OscatOop` only. Import
classic `OSCAT` as well when the same project intentionally mixes direct OSCAT
calls with component objects.

## Design Rules

- Classic OSCAT functions and FBs are the parity oracle.
- Public component names use the truST PascalCase naming standard.
- Inherited OSCAT names are preserved only in the classic package.
- Properties are read-only snapshots. State changes happen through methods.
- Stateful scan objects use `Update(...)`; service objects expose named methods.
- Setters return no value unless the command can fail.
- Interfaces are narrow and domain-specific.
- One stateful component instance represents one device, signal, or logical
  state machine.

## Components

| Component | Interface | Classic OSCAT source | Primary use |
| --- | --- | --- | --- |
| `AutomationContext` | `IAutomationContext` | `OSCAT_BASIC_Constants`, direction helpers | constants, language, direction lookup |
| `UnitConverter` | `IUnitConverter` | Chapter 22 conversion functions and conversion FB parity | service-style unit conversion |
| `Pt1Filter` | `IPt1Filter` | `FT_PT1` | filtered analog process values |
| `Pt2Filter` | concrete component | `FT_PT2` | second-order process filtering |
| `DerivativeFilter` | concrete component | `FT_DERIV` | derivative signal estimate |
| `IntegratorFilter` | concrete component | `FT_INT` | bounded integration |
| `DelayLine16` | concrete component | `FT_TN16` | 16-sample delay line |
| `PiController` | `IClosedLoopController` | `CTRL_PI` | PI control loops |
| `PidController` | `IPidController` | `CTRL_PID` | PID control loops |
| `PwmController` | concrete component | `CTRL_PWM` | PWM output from control value |
| `HysteresisSwitch` | `IHysteresisSwitch` | `HYST` | threshold/alarm switching |
| `PulseGenerator` | `IPulseGenerator` | `GEN_PULSE` | duty-cycle and actuator pulses |
| `RandomSignalGenerator` | concrete component | `GEN_RDM` | sampled random process signal |
| `SineSignalGenerator` | concrete component | `GEN_SIN` | sine-wave test signal |
| `SquareSignalGenerator` | concrete component | `GEN_SQR` | square-wave test signal |
| `ByteRamp` | concrete component | `RMP_B` | byte ramp output |
| `WordRamp` | concrete component | `RMP_W` | word ramp output |
| `DwordFifo16` | `IDwordQueue` | `FIFO_16` | FIFO event/order queues |
| `DwordFifo32` | `IDwordQueue` | `FIFO_32` | larger FIFO event/order queues |
| `DwordStack16` | `IDwordStack` | `STACK_16` | LIFO recipe/work stacks |
| `DwordStack32` | `IDwordStack` | `STACK_32` | larger LIFO recipe/work stacks |
| `Latch` | concrete component | `LTCH` | retained boolean latch |
| `ToggleSwitch` | concrete component | `TOGGLE` | edge-triggered toggle |
| `DwordCounter` | concrete component | `COUNT_DR` | edge-counted DWORD counter |
| `ShiftRegister8` | concrete component | `SHR_8PLE` | 8-bit shift register |
| `OntimeMeter` | concrete component | `ONTIME` | runtime seconds and start cycles |
| `CycleTimeMeter` | concrete component | `CYCLE_TIME` | cycle-time statistics |
| `Calibrator` | concrete component | `CALIBRATE` | calibrated analog scaling |
| `BarGraphMeter` | concrete component | `BAR_GRAPH` | level/status/alarm bucket |
| `CalendarClock` | `ICalendarClock` | `CALENDAR_CALC`, `SUN_POS`, `SUN_TIME` | local calendar and sun state |
| `HolidayCalendar` | concrete component | `HOLIDAY` | holiday/weekend calendar |
| `RtcClock` | concrete component | `RTC_2` | UTC/local runtime clock |
| `TankLevelController` | concrete component | `TANK_LEVEL` | tank valve/alarm state |
| `HeatCurve` | concrete component | `HEAT_TEMP` | heating flow-temperature curve |
| `BoilerController` | concrete component | `BOILER` | boiler heat/error/status state |
| `SingleOutputDriver` | concrete component | `DRIVER_1` | one-channel device output driver |

## Common Types

- `ComponentStatus`: `Ready`, `Error`, `ErrorId`, `Status`.
- `RealRange`: `Low`, `High`.
- `PidGains`: `Kp`, `Tn`, `Tv`.
- `PiGains`: `Kp`, `Ki`.
- `SunPosition`: `Azimuth`, `Elevation`, `RefractedElevation`.
- `SunTimes`: `SolarNoon`, `Sunrise`, `Sunset`, `Declination`.
- `BarGraphSnapshot`: bucket, alarm, and status fields for bar graph style
  HMI binding.

Constants:

- `ComponentErrorNone`
- `ComponentErrorInvalidConfiguration`
- `ComponentErrorNotReady`
- `ComponentErrorQueueFull`
- `ComponentErrorQueueEmpty`
- `DefaultPidKp`
- `DefaultPidTn`
- `DefaultPidTv`
- `DefaultPidLowLimit`
- `DefaultPidHighLimit`
- `DefaultCalendarOffsetMinutes`
- `MaxDwordFifo16Capacity`
- `MaxDwordStack16Capacity`
- `MaxDwordFifo32Capacity`
- `MaxDwordStack32Capacity`

## Interface Summary

All components implement `IComponent`:

```st
PROPERTY Ready : BOOL
PROPERTY Error : BOOL
PROPERTY ErrorId : WORD
PROPERTY Status : BYTE
METHOD Initialize
METHOD Reset
METHOD ClearError
METHOD Snapshot : ComponentStatus
```

`AutomationContext`:

```st
METHOD LoadConstants : BOOL
METHOD SetDefaultLanguage(LanguageIndex : INT)
METHOD DirectionName(Degrees : REAL) : STRING[3]
METHOD DirectionDegrees(Name : STRING[3]) : INT
PROPERTY ConstantsLoaded : BOOL
PROPERTY Version : DWORD
PROPERTY Pi2 : REAL
PROPERTY DefaultLanguage : INT
```

`Initialize()` loads OSCAT constants and reports `ComponentErrorNotReady` if
the constants loader fails. Property reads are snapshots only; they do not
perform lazy global initialization.

`UnitConverter`:

```st
METHOD KelvinFromCelsius(Celsius : REAL) : REAL
METHOD FahrenheitFromCelsius(Celsius : REAL) : REAL
METHOD CelsiusFromKelvin(Kelvin : REAL) : REAL
METHOD CelsiusFromFahrenheit(Fahrenheit : REAL) : REAL
METHOD MetersPerSecondFromKilometersPerHour(Kmh : REAL) : REAL
METHOD KilometersPerHourFromMetersPerSecond(Mps : REAL) : REAL
METHOD BeaufortFromMetersPerSecond(Mps : REAL) : INT
METHOD AngularFrequencyFromHertz(Frequency : REAL) : REAL
METHOD HertzFromAngularFrequency(AngularFrequency : REAL) : REAL
METHOD PeriodFromHertz(Frequency : REAL) : TIME
METHOD HertzFromPeriod(Period : TIME) : REAL
METHOD WattHoursFromJoules(Joule : REAL) : REAL
METHOD JoulesFromWattHours(WattHour : REAL) : REAL
METHOD JoulesFromCalories(Calorie : REAL) : REAL
METHOD CaloriesFromJoules(Joule : REAL) : REAL
```

`Pt1Filter`:

```st
METHOD Configure(TimeConstant : TIME, Gain : REAL)
METHOD Update(Sample : REAL) : REAL
PROPERTY Output : REAL
PROPERTY TimeConstant : TIME
PROPERTY Gain : REAL
```

`Reset()` causes the next `Update()` to behave like the classic `FT_PT1`
first call: the output snaps to `Gain * Sample` and the internal time-delta
tracking restarts.

`PidController`:

```st
METHOD SetKp(Kp : REAL)
METHOD SetIntegralTime(Tn : REAL)
METHOD SetDerivativeTime(Tv : REAL)
METHOD SetSupervisionBand(SupervisionBand : REAL)
METHOD SetOffset(Offset : REAL)
METHOD SetLimits(Limits : RealRange)
METHOD SetManual(Manual : BOOL, ManualInput : REAL)
METHOD ApplyGains(Gains : PidGains)
METHOD Configure(Gains : PidGains, Limits : RealRange, SupervisionBand : REAL, Offset : REAL)
METHOD Update(Actual : REAL, Target : REAL) : REAL
PROPERTY Output : REAL
PROPERTY ControlError : REAL
PROPERTY Limited : BOOL
PROPERTY Actual : REAL
PROPERTY TargetValue : REAL
PROPERTY Kp : REAL
PROPERTY Tn : REAL
PROPERTY Tv : REAL
```

Use `Configure(...)` for initial setup. Use the individual setters only for
runtime adjustments where one parameter changes without rebuilding the full
controller configuration.

`PiController` implements `IClosedLoopController` with the same
`Update(Actual, Target)` scan method as `PidController`. Use
`IClosedLoopController` only where a caller genuinely accepts interchangeable
PI/PID implementations; otherwise keep concrete variables.

`PwmController`:

```st
METHOD Configure(Frequency : REAL)
METHOD SetManual(Manual : BOOL, ManualInput : REAL)
METHOD Update(ControlInput : REAL) : BOOL
PROPERTY Output : BOOL
PROPERTY Frequency : REAL
```

`HysteresisSwitch`:

```st
METHOD SetLimits(Limits : RealRange)
METHOD Update(MeasuredValue : REAL) : BOOL
PROPERTY Q : BOOL
PROPERTY Window : BOOL
PROPERTY LowLimit : REAL
PROPERTY HighLimit : REAL
```

`HysteresisSwitch` rejects inverted limits (`High < Low`). Use classic `HYST`
directly if an application intentionally depends on OSCAT's inverted-limit
behavior.

`PulseGenerator`:

```st
METHOD Configure(HighTime : TIME, LowTime : TIME)
METHOD SetEnabled(Enabled : BOOL)
METHOD Update : BOOL
PROPERTY Output : BOOL
PROPERTY Enabled : BOOL
PROPERTY HighTime : TIME
PROPERTY LowTime : TIME
```

`DwordFifo16` and `DwordStack16`:

```st
METHOD Push(Value : DWORD) : BOOL
METHOD TryPop : BOOL
PROPERTY Value : DWORD
PROPERTY Empty : BOOL
PROPERTY Full : BOOL
PROPERTY Capacity : UINT
```

`DwordFifo32` and `DwordStack32` use the same method/property names with
32-entry capacity constants.

Additional concrete components follow the same narrow object pattern:

```st
DerivativeFilter.Configure(Gain)
IntegratorFilter.Configure(Gain, Limits)
Pt2Filter.Configure(TimeConstant, Damping, Gain)
DelayLine16.Configure(DelayTime)
RandomSignalGenerator.Configure(Period, Amplitude, Offset)
SineSignalGenerator.Configure(Period, Amplitude, Offset, Delay)
SquareSignalGenerator.Configure(Period, Amplitude, Offset, DutyCycle, Delay)
ByteRamp.Update(SetHigh, RampTime, Enabled, Up)
WordRamp.Update(SetHigh, RampTime, Enabled, Up)
Latch.Update(Data, Load)
ToggleSwitch.Update(Clock)
DwordCounter.Configure(StepSize, MaxValue)
ShiftRegister8.Update(DataIn, LoadValue, Clock, Up, Load)
OntimeMeter.Update(Input)
CycleTimeMeter.Update(ResetStatistics)
Calibrator.Configure(OffsetTarget, ScaleTarget)
BarGraphMeter.Configure(TriggerLow, TriggerHigh, AlarmLow, AlarmHigh, LogScale)
```

`CalendarClock`:

```st
METHOD Configure(
    LocationLatitude : REAL,
    LocationLongitude : REAL,
    OffsetMinutes : INT,
    DstEnabled : BOOL,
    LanguageIndex : INT
)
METHOD Update(Utc : DT)
METHOD CalculateSunPosition(Utc : DT, Latitude : REAL, Longitude : REAL) : SunPosition
METHOD CalculateSunTime(UtcDate : DATE, Latitude : REAL, Longitude : REAL, Horizon : REAL) : SunTimes
PROPERTY LocalDateTime : DT
PROPERTY Year : INT
PROPERTY Month : INT
PROPERTY Day : INT
PROPERTY Night : BOOL
PROPERTY Sunrise : TOD
PROPERTY Sunset : TOD
PROPERTY WorkWeek : INT
```

Calendar/device/building domain components:

```st
HolidayCalendar.Configure(LanguageIndex, FridayAsHoliday, SaturdayAsHoliday, SundayAsHoliday)
HolidayCalendar.SetHoliday(Index, Month, Day, Use, Name)
HolidayCalendar.Update(DateIn) : BOOL
RtcClock.SetClock(InitialDateTime, Milliseconds, DstEnabled, OffsetMinutes)
RtcClock.Update()
TankLevelController.Configure(MaxValveTime, LevelDelayTime)
TankLevelController.Update(LevelReached, LeakDetected, AlarmClear) : BOOL
HeatCurve.Configure(MaxFlowTemperature, MinFlowTemperature, ConfigFlowTemperature, ConfigInsideTemperature, ConfigOutsideTemperature, FlowDifference, Curve, Hysteresis)
HeatCurve.Update(OutsideTemperature, InsideTemperature, Offset, RequestedTemperature) : REAL
BoilerController.Update(UpperTemperature, LowerTemperature, PressureOk, Enabled, Request1, Request2, Boost) : BOOL
SingleOutputDriver.Configure(ToggleMode, Timeout)
SingleOutputDriver.Update(SetRequest, Input) : BOOL
```

## Example

```st
PROGRAM Main
VAR
    Controller : PidController;
    Limits : RealRange;
    Gains : PidGains;
    CommandPercent : REAL;
END_VAR

Limits.Low := REAL#0.0;
Limits.High := REAL#100.0;
Gains.Kp := REAL#2.0;
Gains.Tn := REAL#1.0;
Gains.Tv := REAL#0.0;

Controller.Initialize();
Controller.Configure(
    Gains := Gains,
    Limits := Limits,
    SupervisionBand := REAL#0.0,
    Offset := REAL#0.0
);

CommandPercent := Controller.Update(
    Actual := REAL#42.0,
    Target := REAL#75.0
);
END_PROGRAM
```

Use interface-typed variables only when the caller accepts interchangeable
implementations. Simple application code should use the concrete component
directly; that keeps scan code shorter and makes initialization obvious.

## Examples

The example suite now contains 49 classic/OOP comparison pairs:

- 27 hand-written real machine/process pattern scenarios;
- 20 compact component-composition showcases;
- 2 compact pattern showcases for polymorphism and composition.

Each item is shipped as a classic/OOP comparison project pair with a README,
application Structured Text, and `src/Tests.st`. OOP projects with
communication claims include `runtime.toml` and/or `io.toml` backing those
boundaries.

The catalog is intentionally process-first. There are no standalone
"calibrator", "historian", or "diagnostics" examples. Calibration, historian
records, alarm journals, Modbus/MQTT/OPC UA boundaries, and commissioning
signals are demonstrated inside complete machines such as a chemical dosing
skid, airport baggage diverter, water booster station, boiler room,
pasteurizer, cold storage plant, and tank farm.

Patterns covered by the OOP examples include:

- Factory and Template Method
- Strategy
- Mediator
- Observer
- Composite
- Iterator-style child traversal
- Decorator
- Facade
- Chain of Responsibility
- State objects / state-machine ownership
- Command and Memento-style audit records
- Adapter
- Proxy
- Snapshot/read-model objects for SCADA, historian, CSV, database, and HMI use

The hand-written pattern catalog and acceptance rules live at:

- `docs/internal/references/OSCAT/oscat_oop_realworld_pattern_catalog.md`

The README in each project explains the physical process, field signals,
communication boundary, state machine, alarms, OSCAT surface used, the OOP
pattern, how to use that pattern, why it helps, and when the classic version is
still the better choice.

Run the default example catalog gate:

```bash
cargo test -p trust-runtime --test oscat_oop_examples
```

Run the full runtime execution sweep for all 98 paired example projects only
when validating OSCAT changes or release evidence:

```bash
cargo test -p trust-runtime --test oscat_oop_examples oscat_oop_example_st_unit_tests_pass -- --ignored --nocapture
```

## Validation

Core library parity:

```bash
cargo test -p trust-runtime --test oscat_oop_library
```

Example catalog checks:

```bash
cargo test -p trust-runtime --test oscat_oop_examples
```

The core fixture lives at:

- `crates/trust-runtime/tests/fixtures/oscat/oop_core`

The example projects live under:

- `examples/OSCAT/<example>/non-oop`
- `examples/OSCAT/<example>/oop`

Each `examples/OSCAT/<example>/README.md` explains the process, the OOP
pattern, why that pattern fits, how to reuse it, and when the classic
non-OOP version is the better shape.
