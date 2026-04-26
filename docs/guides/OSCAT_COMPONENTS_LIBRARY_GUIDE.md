# OSCAT Components Library Guide

OSCAT Components is the object-oriented companion to the classic
`libraries/oscat` package. Classic OSCAT remains the behavior source of truth.
The component package adds small object-shaped wrappers for workflows where
state, identity, narrow interfaces, and readable scan code matter.

Use classic OSCAT when you need the upstream manual-aligned function or FB
surface directly. Use OSCAT Components when application code benefits from a
named object with configuration methods, read-only status properties, and a
single scan method.

## Package Setup

```toml
[project]
include_paths = ["src"]
stdlib = "iec"

[dependencies]
AutomationComponents = { path = "../../libraries/oscat/components", version = "0.1.0" }
```

The package depends on classic OSCAT internally:

```toml
[dependencies]
OSCAT = { path = "..", version = "0.1.0" }
```

Application projects normally depend on `AutomationComponents` only. Import
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
| `PidController` | `IPidController` | `CTRL_PID` | PID control loops |
| `HysteresisSwitch` | `IHysteresisSwitch` | `HYST` | threshold/alarm switching |
| `PulseGenerator` | `IPulseGenerator` | `GEN_PULSE` | duty-cycle and actuator pulses |
| `DwordFifo16` | `IDwordQueue` | `FIFO_16` | FIFO event/order queues |
| `DwordStack16` | `IDwordStack` | `STACK_16` | LIFO recipe/work stacks |
| `CalendarClock` | `ICalendarClock` | `CALENDAR_CALC`, `SUN_POS`, `SUN_TIME` | local calendar and sun state |

## Common Types

- `ComponentStatus`: `Ready`, `Error`, `ErrorId`, `Status`.
- `RealRange`: `Low`, `High`.
- `PidGains`: `Kp`, `Tn`, `Tv`.
- `SunPosition`: `Azimuth`, `Height`, `RefractedHeight`.
- `SunTimes`: `Midday`, `Rise`, `Sunset`, `Declination`.

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

`PidController`:

```st
METHOD SetKp(Kp : REAL)
METHOD SetIntegralTime(Tn : REAL)
METHOD SetDerivativeTime(Tv : REAL)
METHOD SetNoiseBand(SupervisionBand : REAL)
METHOD SetOffset(Offset : REAL)
METHOD SetLimits(Limits : RealRange)
METHOD SetManual(Manual : BOOL, ManualInput : REAL)
METHOD ApplyGains(Gains : PidGains)
METHOD Update(Actual : REAL, Target : REAL) : REAL
PROPERTY Output : REAL
PROPERTY Difference : REAL
PROPERTY Limited : BOOL
PROPERTY Actual : REAL
PROPERTY TargetValue : REAL
PROPERTY Kp : REAL
PROPERTY Tn : REAL
PROPERTY Tv : REAL
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
PROPERTY SunRise : TOD
PROPERTY SunSet : TOD
PROPERTY WorkWeek : INT
```

## Example

```st
PROGRAM Main
VAR
    Controller : PidController;
    ControllerObject : IPidController;
    Limits : RealRange;
    CommandPercent : REAL;
END_VAR

Limits.Low := REAL#0.0;
Limits.High := REAL#100.0;

ControllerObject := Controller;
ControllerObject.Initialize();
ControllerObject.SetKp(Kp := REAL#2.0);
ControllerObject.SetIntegralTime(Tn := REAL#1.0);
ControllerObject.SetDerivativeTime(Tv := REAL#0.0);
ControllerObject.SetLimits(Limits := Limits);

CommandPercent := ControllerObject.Update(
    Actual := REAL#42.0,
    Target := REAL#75.0
);
END_PROGRAM
```

## Examples

The example catalog includes 20 comparison scenarios, each as one classic
OSCAT project and one OOP Components project. Every project has `src/Tests.st`
and a README:

- tank level PID
- greenhouse temperature
- conveyor pulse scheduler
- production queue
- maintenance stack
- ventilation filter
- solar lighting clock
- pump pressure
- energy normalization
- wind speed alarm
- cold storage alarm
- wastewater aeration
- packaging reject pulse
- recipe batch stack
- shift order queue
- irrigation sun clock
- compressor pressure filter
- chiller temperature PID
- boiler feedwater alarm queue
- weather station conversion

Run the complete example gate:

```bash
cargo test -p trust-runtime --test oscat_components_examples
```

## Validation

Core library parity:

```bash
cargo test -p trust-runtime --test oscat_components_library
```

Example parity:

```bash
cargo test -p trust-runtime --test oscat_components_examples
```

The core fixture lives at:

- `crates/trust-runtime/tests/fixtures/oscat/components_core`

The example projects live under:

- `examples/oscat_components_*_classic`
- `examples/oscat_components_*_components`
