# OSCAT Component Library Investigation And Design

Date: 2026-04-26

Status: v1.0 component surface implemented in the working tree. Core library
ST tests and 49 classic/components comparison pairs are part of the acceptance
surface: 27 industrial pattern pairs, 20 compact component-composition
showcases, and 2 compact pattern showcases.

Verdict: build a complete object-oriented component library as a staged,
optional facade over the classic OSCAT package. Do not put `OOP`, `Object`,
`Wrapper`, or `OSCAT` into the public type names. Public names should describe
the automation role: `PidController`, `Pt1Filter`, `UnitConverter`,
`DwordFifo16`, `CalendarClock`.

## Position

The right end state is complete object-oriented coverage for the OSCAT parts
that benefit from object identity. The wrong implementation is a large first
release with broad union interfaces and method wrappers for every classic
function.

The complete package should:

- keep `libraries/oscat` as the source of truth
- expose object identity where there is retained state, configuration, status,
  or a domain concept
- keep pure scalar helpers available through the classic package unless a
  service object has a real ergonomic benefit
- ship each domain as a narrow vertical slice with ST tests first
- avoid speculative interfaces that force unrelated FBs into one shape

The design review is accepted on the important points:

- v0.1 must be narrow
- union interfaces are out
- user-facing `REFERENCE TO` is out
- hidden `Last...` service caches are out
- naming must follow truST package conventions and common software naming
- properties and setters need one unambiguous rule
- the scan model must be consistent

## Sources Reviewed

Local sources:

- `docs/guides/PLC_DEVELOPER_GUIDE.md`
- `docs/specs/01-lexical-elements.md`
- `docs/specs/04-pou-declarations.md`
- `docs/PLCOPEN_DEVIATIONS.md`
- `libraries/oscat/README.md`
- `docs/guides/OSCAT_LIBRARY_GUIDE.md`
- `docs/public/develop/libraries/oscat.md`
- `examples/oscat_smoke/README.md`
- `libraries/oscat/src/**`
- `crates/trust-runtime/tests/fixtures/oscat/core/src/**`
- `docs/guides/PLCOPEN_MOTION_OOP_LIBRARY_GUIDE.md`
- `libraries/plcopen_motion/oop/**`

Upstream and external sources:

- OSCAT BASIC 3.33 manual:
  `https://www.oscat.de/images/OSCATBasic/oscat_basic333_en.pdf`
- OSCAT downloads page:
  `https://www.oscat.de/de/component/content/article/7-news/69-downloads.html`
- Eclipse OSCAT project page:
  `https://projects.eclipse.org/projects/iot.oscat`
- CODESYS Store OSCAT BASIC page:
  `https://us.store.codesys.com/oscat-basic.html`
- PLCopen OOP guideline announcement:
  `https://www.plcopen.org/news/plcopen-releases-guidelines-for-object-orientation/`

## Evidence

OSCAT's own objective is portability: the manual describes OSCAT as an open
IEC 61131-3 library intended to reduce vendor-specific dependencies and keep
source code available for inspection and adaptation. That is exactly the value
our current classic package preserves.

The current truST OSCAT package is already a broad manual-aligned classic
surface:

- `libraries/oscat/README.md` records the shipped scope as manual chapters
  `03_data_types` through `26_list_processing`.
- `docs/guides/OSCAT_LIBRARY_GUIDE.md` records `563` local shipped symbols,
  `126` ST conformance tests, and one deliberate upstream public-name rename:
  upstream `OVERRIDE` is shipped as `OVERRIDE_3` because `OVERRIDE` is a truST
  OOP keyword.
- Source inspection shows many stateful function blocks where a component
  facade is justified: controllers, filters, generators, latches, memory
  modules, calendar/sun helpers, measuring modules, and building-control
  blocks.
- Source inspection also shows many pure scalar functions where OOP adds little
  value: mathematics, geometry, complex arithmetic, string parsing, time/date
  constructors, vector helpers, and simple unit conversions.

PLCopen's OOP guidance is relevant but not directly binding for OSCAT.
PLCopen explicitly says classical programming should be able to cooperate with
OOP programming in parallel. The motion OOP wrapper follows that approach: it
keeps the classic motion package as the compliance kernel and adds a second
object-oriented facade. OSCAT should follow the same coexistence rule.

## Naming Standard

This section defines the default naming standard for the OSCAT OOP
library surface and its examples. It is also suitable as the default style for
new truST-authored ST, but it does not rewrite inherited PLCopen, OSCAT, or
vendor profile symbols.

The local truST library convention is:

- reusable libraries live under `libraries/<name>/`
- dependency aliases are semantic PascalCase names, for example
  `MyMotionLib` in `docs/guides/PLC_DEVELOPER_GUIDE.md`
- ST identifiers are case-insensitive, so names must not rely on case alone
- current truST house style prefers readable PascalCase field/type names over
  preserving every upstream example spelling when the upstream name is not a
  required public standard spelling

Common software design rules applied here:

- names describe domain responsibility, not implementation technology
- avoid redundant module/package prefixes in type names
- avoid Hungarian-style prefixes except where the platform or standard already
  uses them
- keep interfaces narrow and named by capability
- use stable nouns for types and verb phrases for methods

Therefore:

- package path: `libraries/oscat/oop`
- dependency alias: `OscatOop`
- documentation name: `OSCAT OOP`
- source files: lower_snake package/domain files that match the existing
  library layout, for example `component_types.st`,
  `component_interfaces.st`, `control.st`, `memory.st`, and `calendar.st`
- public ST interfaces: `IComponent`, `IPidController`, `IPt1Filter`,
  `IHysteresisSwitch`, `IPulseGenerator`, `IDwordQueue`, `IDwordStack`,
  `ICalendarClock`
- public ST concrete FBs: `AutomationContext`, `UnitConverter`,
  `Pt1Filter`, `PidController`, `HysteresisSwitch`, `PulseGenerator`,
  `DwordFifo16`, `DwordStack16`, `CalendarClock`
- shared records: `ComponentStatus`, `RealRange`, `PidGains`,
  `SunPosition`, `SunTimes`
- methods: PascalCase verb phrases such as `Update`, `SetKp`, `SetLimits`,
  `Push`, `TryPop`, `Reset`
- method parameters and `VAR_INPUT` / `VAR_OUTPUT` / `VAR_IN_OUT` names:
  PascalCase semantic names such as `Target`, `ProcessValue`, `MeasuredValue`,
  `MinValue`, `MaxValue`, `HighTime`, `LowTime`
- local scratch and state variables inside new truST-authored FBs/classes:
  PascalCase semantic names such as `Error`, `LastValue`,
  `IntegralAccumulator`, `PulseElapsed`
- loop counters: `I`, `J`, `K` for very short local loops; PascalCase names
  such as `RowIndex`, `ChannelIndex`, `SampleIndex` when the value has meaning
  beyond a few lines
- component instances in examples: semantic instance names such as
  `LevelController`, `PressureFilter`, `PulseScheduler`, `ProductionQueue`;
  avoid numeric names unless the number is part of a real device identity
- public constants: domain-role PascalCase names such as `DefaultPidKp`,
  `DefaultPidLimits`, `DefaultPulseHighTime`, `DefaultPulseLowTime`,
  `MaxDwordFifo16Capacity`, `DefaultCalendarOffsetMinutes`,
  `ComponentErrorQueueFull`

Do not use:

- `OSCAT_Oop...`
- `OscatOop...`
- `Oop...`
- `...Wrapper`
- `...Facade`
- `itf...` for new truST-owned interfaces

The exception is standard-owned names. PLCopen Motion keeps `MC_` and `itf...`
because those names come from PLCopen material. The OSCAT component library is
a truST facade over OSCAT, so it should use truST/domain names.

### Constant Names

Constants follow the same domain-role rule as types and methods.

truST-owned public constants:

- use PascalCase domain nouns
- use semantic words such as `Default`, `Min`, `Max`, `Limit`, `Timeout`,
  `Capacity`, `Mask`, `Bit`, or `Error`
- include units in the name when the type alone is ambiguous
- examples: `DefaultPidKp`, `DefaultPidTn`, `DefaultPidTv`,
  `DefaultPidLimits`, `MaxDwordFifo16Capacity`,
  `DefaultCalendarOffsetMinutes`, `StatusWarningMask`, `StatusErrorBit`,
  `ComponentErrorNotReady`, `ComponentErrorQueueFull`

Upstream or profile-owned constants:

- preserve the upstream spelling exactly
- OSCAT classic examples: `STRING_LENGTH`, `LIST_LENGTH`, `MATH`, `PHYS`,
  `LANGUAGE`, `CONSTANTS_MATH`, `CONSTANTS_PHYS`, `CONSTANTS_LANGUAGE`
- PLCopen examples: `mcERR_NotSupported`, `mcERR_InvalidParameter`, `PN_*`,
  `MC_*`

Enum literals:

- use PascalCase literals
- include the enum or domain noun when bare literals would collide or read
  poorly in ST
- examples: `ComponentStateReady`, `ComponentStateError`,
  `QueueResultFull`, `QueueResultEmpty`
- preserve PLCopen or OSCAT literals when the literal is standard-owned

Internal implementation constants and globals:

- use scoped lower_snake global names only for package-scope implementation
  storage that is not public API
- examples: `g_component_constants_loaded`, `g_component_default_context`
- do not expose internal globals through the dependency alias
- keep ordinary local implementation variables PascalCase as defined above

`VAR_INPUT CONSTANT` parameters keep ordinary parameter names. `CONSTANT` is a
storage or parameter qualifier, not a naming prefix.

Constant placement:

- use `VAR CONSTANT` inside the consuming POU when a constant is private to one
  function, function block, class, method, or property implementation
- use `VAR_GLOBAL CONSTANT` only for shared library API constants or shared
  implementation constants that cannot be scoped narrower
- expose only intentional public constants through documentation and examples
- use `VAR_EXTERNAL CONSTANT` only when a consumer must bind a published
  `VAR_GLOBAL CONSTANT`

## PLCopen Motion OOP Comparison

Follow these PLCopen Motion OOP choices:

- second package, not a replacement
- classic package remains the behavior source
- concrete component FBs delegate to classic FB kernels
- interface-first design where polymorphism is useful
- ST fixture tests validate the public behavior
- unsupported or deferred behavior is explicit

Deliberately deviate in these areas:

- OSCAT is not command-oriented, so it should not copy the motion
  command-object model.
- Motion has a strong central domain object, `itfAxis`; OSCAT has many
  unrelated utility domains, so it needs smaller interfaces.
- Motion methods can return command objects because commands carry execution
  state; OSCAT configuration methods should be ordinary setters with no return.
- Motion keeps command objects as reusable FB state. OSCAT pure helper methods
  should return values directly or result records that the caller owns, and
  avoid `Last...` service caches.
- Motion v0.1 can cover a coherent axis domain. OSCAT v0.1 must be a vertical
  kernel and then grow domain by domain.

## Non-Goals

These are explicit non-goals for v0.1:

- no one-to-one wrapper around every OSCAT function
- no `OOP` or `OSCAT` prefix in public component type names
- no building-control wrappers: `BOILER`, `BURNER`, `HEAT_TEMP`,
  `LEGIONELLA`, `TANK_LEVEL`, and `TEMP_EXT` need a separate design pass
- no unified generator interface that mixes pulse, waveform, ramp, random, and
  PWM behavior
- no unified meter interface
- no OOP shell around Chapter 25 buffer helpers
- no OOP shell around Chapter 26 list helpers
- no math or text service objects in v0.1
- no `REFERENCE TO` in the user-facing API
- no service-object `Last...` result caches
- no `BOOL` return from setters unless the setter can reject invalid input

Long-term non-goal:

- The classic OSCAT package remains public and supported. The component
  package is a second facade, not a migration that hides the classic API.

## Package Shape

Proposed directory:

```text
libraries/oscat/oop/
  trust-lsp.toml
  README.md
  src/
    component_types.st
    component_interfaces.st
    automation_context.st
    unit_converter.st
    filters.st
    control.st
    generators.st
    memory.st
    calendar.st
```

Proposed manifest:

```toml
[package]
version = "0.1.0"

[project]
include_paths = ["src"]

[dependencies]
OSCAT = { path = "..", version = "0.1.0" }
```

Consumer dependency:

```toml
[dependencies]
OscatOop = { path = "../../libraries/oscat/oop", version = "0.1.0" }
```

Version policy:

- The component package is release-notable and should track the shipped OSCAT
  package version unless we add package-version indirection later.
- If `libraries/oscat/trust-lsp.toml` changes version, keep
  `libraries/oscat/oop/trust-lsp.toml` and the dependency examples in
  lockstep.
- Do not use `version = "*"` in checked-in examples; exact local versions make
  examples and release evidence deterministic.

Proposed test fixture:

```text
crates/trust-runtime/tests/fixtures/oscat/oop_core/
  trust-lsp.toml
  src/
    Configuration.st
    tests.st
```

Proposed Rust test driver:

```text
crates/trust-runtime/tests/oscat_oop_library.rs
```

The Rust driver should follow the same real shape as
`crates/trust-runtime/tests/plcopen_motion_oop_library.rs`: construct the
fixture path and invoke `env!("CARGO_BIN_EXE_trust-runtime") test --project`.
Behavioral assertions should live in Structured Text.

## Runtime Model

There is one scan rule:

- stateful objects expose one domain-specific scan method
- the scan method is the only scan entry point for that object
- there is no inherited `Step()` method on `IComponent`

Examples:

- filters: `Update(Sample : REAL) : REAL`
- controllers: `Update(Actual : REAL, Target : REAL) : REAL`
- hysteresis: `Update(MeasuredValue : REAL) : BOOL`
- pulse generator: `Update() : BOOL`
- FIFO/stack: `Push`, `TryPop`, `Reset`
- calendar: `Update(Utc : DT)`

Properties are read-only snapshots:

- Public properties expose current state and outputs.
- Configuration writes go through methods.
- A property and a setter may exist for the same value only when the property
  is read-only and the setter is the only write path.
- No v0.1 component exposes a writable configuration property.

Configuration methods:

- return no value when they only assign fields
- return a result only when invalid input can be rejected
- use explicit names when a method replaces a full configuration record

Service objects:

- are allowed only when they group a real domain
- return values or caller-owned result records directly
- do not retain `Last...` outputs for caller convenience

Error/status convention:

- OSCAT uses `STATUS`, `ERROR`, `FAIL`, and similar outputs inconsistently by
  module family. Components normalize these only where the underlying module
  has real status.
- `ErrorId` is `WORD`.
- When wrapping a classic `BYTE` status, use `BYTE_TO_WORD(Status)` for errors
  that come from OSCAT.
- `Status` remains `BYTE` when the underlying FB has a native status byte.

Reset convention:

- `Reset()` is allowed only when the wrapper can map it to an existing classic
  reset input or to clearing wrapper-owned status.
- v0.1 does not expose `ResetIntegral()` on `PidController`; the classic parity
  surface is the `RST` input behavior, and the wrapper should drive that
  through `Reset()`.

## Shared Types

These types should live in `component_types.st`.

```iecst
TYPE ComponentStatus :
STRUCT
    Ready : BOOL;
    Error : BOOL;
    ErrorId : WORD;
    Status : BYTE;
END_STRUCT
END_TYPE

TYPE RealRange :
STRUCT
    Low : REAL;
    High : REAL;
END_STRUCT
END_TYPE

TYPE PidGains :
STRUCT
    Kp : REAL := 1.0;
    Tn : REAL := 1.0;
    Tv : REAL := 1.0;
END_STRUCT
END_TYPE

TYPE SunPosition :
STRUCT
    Azimuth : REAL;
    Elevation : REAL;
    RefractedElevation : REAL;
END_STRUCT
END_TYPE

TYPE SunTimes :
STRUCT
    SolarNoon : TOD;
    Sunrise : TOD;
    Sunset : TOD;
    Declination : REAL;
END_STRUCT
END_TYPE

VAR_GLOBAL CONSTANT
    ComponentErrorNone : WORD := WORD#16#0000;
    ComponentErrorInvalidConfiguration : WORD := WORD#16#1000;
    ComponentErrorNotReady : WORD := WORD#16#1001;
    ComponentErrorQueueFull : WORD := WORD#16#1002;
    ComponentErrorQueueEmpty : WORD := WORD#16#1003;

    DefaultPidKp : REAL := REAL#1.0;
    DefaultPidTn : REAL := REAL#1.0;
    DefaultPidTv : REAL := REAL#1.0;
    DefaultPidLowLimit : REAL := REAL#-1000.0;
    DefaultPidHighLimit : REAL := REAL#1000.0;
    DefaultPidSupervisionBand : REAL := REAL#0.0;
    DefaultPidOffset : REAL := REAL#0.0;

    DefaultPt1Gain : REAL := REAL#1.0;
    DefaultPt1TimeConstant : TIME := T#1s;
    DefaultPulseHighTime : TIME := T#1s;
    DefaultPulseLowTime : TIME := T#1s;
    DefaultCalendarOffsetMinutes : INT := INT#0;
    MaxDwordFifo16Capacity : UINT := UINT#16;
    MaxDwordStack16Capacity : UINT := UINT#16;
END_VAR
```

`RealRange` is the only generic real low/high range record in v0.1. Do not add
specialized duplicate range records such as `PidLimits` unless a later domain
requires different fields.

## v0.1 Interfaces

### `IComponent`

Common lifecycle and status contract.

Properties:

- `Ready : BOOL`
- `Error : BOOL`
- `ErrorId : WORD`
- `Status : BYTE`

Methods:

- `Initialize()`
- `Reset()`
- `ClearError()`
- `Snapshot() : ComponentStatus`

### `IAutomationContext`

Object wrapper for `OSCAT_BASIC_Constants()` and global carrier access.

Extends:

- `IComponent`

Properties:

- `ConstantsLoaded : BOOL`
- `Version : DWORD`
- `Math : CONSTANTS_MATH`
- `Phys : CONSTANTS_PHYS`
- `Language : CONSTANTS_LANGUAGE`
- `DefaultLanguage : INT`

Methods:

- `LoadConstants() : BOOL`
- `SetDefaultLanguage(LanguageIndex : INT)`
- `DirectionName(Degrees : REAL) : STRING[3]`
- `DirectionDegrees(Name : STRING[3]) : INT`

Concrete FB:

- `AutomationContext`

Classic mapping:

- `OSCAT_BASIC_Constants`
- `OSCAT_VERSION`
- `DEG_TO_DIR`
- `DIR_TO_DEG`
- global carriers `MATH`, `PHYS`, `LANGUAGE`

### `IUnitConverter`

Service facade for selected Chapter 22 conversion FBs and scalar conversion
functions. This is included in v0.1 only because it creates clear result
records and useful examples; it is not a precedent for wrapping every pure
function.

Extends:

- `IComponent`

Methods:

- `KelvinFromCelsius(Celsius : REAL) : REAL`
- `FahrenheitFromCelsius(Celsius : REAL) : REAL`
- `CelsiusFromKelvin(Kelvin : REAL) : REAL`
- `CelsiusFromFahrenheit(Fahrenheit : REAL) : REAL`
- `MetersPerSecondFromKilometersPerHour(Kmh : REAL) : REAL`
- `KilometersPerHourFromMetersPerSecond(Mps : REAL) : REAL`
- `BeaufortFromMetersPerSecond(Mps : REAL) : INT`
- `AngularFrequencyFromHertz(Frequency : REAL) : REAL`
- `HertzFromAngularFrequency(AngularFrequency : REAL) : REAL`
- `PeriodFromHertz(Frequency : REAL) : TIME`
- `HertzFromPeriod(Period : TIME) : REAL`
- `WattHoursFromJoules(Joule : REAL) : REAL`
- `JoulesFromWattHours(WattHour : REAL) : REAL`
- `JoulesFromCalories(Calorie : REAL) : REAL`
- `CaloriesFromJoules(Joule : REAL) : REAL`

Concrete FB:

- `UnitConverter`

Classic mapping:

- `C_TO_K`
- `C_TO_F`
- `K_TO_C`
- `F_TO_C`
- `KMH_TO_MS`
- `MS_TO_KMH`
- `MS_TO_BFT`
- `F_TO_OM`
- `OM_TO_F`
- `F_TO_PT`
- `PT_TO_F`
- direct scalar joule, watt-hour, and calorie formula parity

### `IPt1Filter`

Narrow interface for `FT_PT1`.

Extends:

- `IComponent`

Read-only properties:

- `Output : REAL`
- `TimeConstant : TIME`
- `Gain : REAL`

Methods:

- `Configure(TimeConstant : TIME, Gain : REAL)`
- `Update(Sample : REAL) : REAL`

Concrete FB:

- `Pt1Filter`

Classic mapping:

- `FT_PT1`

### `IPidController`

Narrow interface for `CTRL_PID`.

Extends:

- `IComponent`

Read-only properties:

- `Actual : REAL`
- `TargetValue : REAL`
- `SupervisionBand : REAL`
- `Manual : BOOL`
- `ManualInput : REAL`
- `Offset : REAL`
- `Output : REAL`
- `ControlError : REAL`
- `Limited : BOOL`
- `LowLimit : REAL`
- `HighLimit : REAL`
- `Kp : REAL`
- `Tn : REAL`
- `Tv : REAL`

Methods:

- `SetKp(Kp : REAL)`
- `SetIntegralTime(Tn : REAL)`
- `SetDerivativeTime(Tv : REAL)`
- `SetSupervisionBand(SupervisionBand : REAL)`
- `SetOffset(Offset : REAL)`
- `SetLimits(Limits : RealRange)`
- `SetManual(Manual : BOOL, ManualInput : REAL)`
- `ApplyGains(Gains : PidGains)`
- `Update(Actual : REAL, Target : REAL) : REAL`

Concrete FB:

- `PidController`

Classic mapping:

- `CTRL_PID`

### `IHysteresisSwitch`

Narrow interface for `HYST`.

Extends:

- `IComponent`

Read-only properties:

- `MeasuredValue : REAL`
- `LowLimit : REAL`
- `HighLimit : REAL`
- `Q : BOOL`
- `Window : BOOL`

Methods:

- `SetLimits(Limits : RealRange)`
- `Update(MeasuredValue : REAL) : BOOL`

Concrete FB:

- `HysteresisSwitch`

Classic mapping:

- `HYST`

### `IPulseGenerator`

Narrow interface for `GEN_PULSE`.

Extends:

- `IComponent`

Read-only properties:

- `Enabled : BOOL`
- `Output : BOOL`
- `HighTime : TIME`
- `LowTime : TIME`

Methods:

- `Configure(HighTime : TIME, LowTime : TIME)`
- `SetEnabled(Enabled : BOOL)`
- `Update() : BOOL`
- `Reset()`

Concrete FB:

- `PulseGenerator`

Classic mapping:

- `GEN_PULSE`

### `IDwordQueue`

Narrow interface for DWORD FIFO wrappers.

Extends:

- `IComponent`

Read-only properties:

- `Empty : BOOL`
- `Full : BOOL`
- `Value : DWORD`
- `Capacity : UINT`

Methods:

- `Push(Value : DWORD) : BOOL`
- `TryPop() : BOOL`
- `Reset()`

Concrete FB:

- `DwordFifo16`

Classic mapping:

- `FIFO_16`

### `IDwordStack`

Narrow interface for DWORD stack wrappers.

Extends:

- `IComponent`

Read-only properties:

- `Empty : BOOL`
- `Full : BOOL`
- `Value : DWORD`
- `Capacity : UINT`

Methods:

- `Push(Value : DWORD) : BOOL`
- `TryPop() : BOOL`
- `Reset()`

Concrete FB:

- `DwordStack16`

Classic mapping:

- `STACK_16`

### `ICalendarClock`

Calendar and sun facade.

Extends:

- `IComponent`

Read-only properties:

- `Utc : DT`
- `LocalDateTime : DT`
- `LocalDate : DATE`
- `LocalTimeOfDay : TOD`
- `Year : INT`
- `Month : INT`
- `Day : INT`
- `Weekday : INT`
- `OffsetMinutes : INT`
- `DstEnabled : BOOL`
- `DstOn : BOOL`
- `LanguageIndex : INT`
- `Longitude : REAL`
- `Latitude : REAL`
- `Sunrise : TOD`
- `Sunset : TOD`
- `SolarNoon : TOD`
- `SolarElevation : REAL`
- `SolarHorizontalProjection : REAL`
- `SolarVerticalProjection : REAL`
- `Night : BOOL`
- `Holiday : BOOL`
- `HolidayName : STRING[30]`
- `WorkWeek : INT`

Methods:

- `Configure(LocationLatitude : REAL, LocationLongitude : REAL, OffsetMinutes : INT, DstEnabled : BOOL, LanguageIndex : INT)`
- `Update(Utc : DT)`
- `CalculateSunPosition(Utc : DT, Latitude : REAL, Longitude : REAL) : SunPosition`
- `CalculateSunTime(UtcDate : DATE, Latitude : REAL, Longitude : REAL, Horizon : REAL) : SunTimes`

Concrete FB:

- `CalendarClock`

Classic mapping:

- `CALENDAR_CALC`
- `SUN_POS`
- `SUN_TIME`
- `WORK_WEEK`
- `DST`
- `UTC_TO_LTIME`
- `LTIME_TO_UTC`

Holiday-table support should not use `REFERENCE TO`. If holiday mutation is
needed in v0.1, expose it through a separate concrete FB with explicit
`VAR_IN_OUT` methods and no shared interface.

## v0.1 Concrete Object Catalog

Phase 1 includes the smallest useful vertical slice:

| Object | Interface | Classic source | Reason |
| --- | --- | --- | --- |
| `AutomationContext` | `IAutomationContext` | `OSCAT_BASIC_Constants`, globals | Safe startup pattern and carrier access |
| `UnitConverter` | `IUnitConverter` | Selected Chapter 22 helpers | Readable scalar conversions and examples |
| `Pt1Filter` | `IPt1Filter` | `FT_PT1` | Stateful filter with scan behavior |
| `PidController` | `IPidController` | `CTRL_PID` | Common real-world control object |
| `HysteresisSwitch` | `IHysteresisSwitch` | `HYST` | Clear stateful switch object |
| `PulseGenerator` | `IPulseGenerator` | `GEN_PULSE` | One generator family without a union interface |
| `DwordFifo16` | `IDwordQueue` | `FIFO_16` | Natural object abstraction |
| `DwordStack16` | `IDwordStack` | `STACK_16` | Natural object abstraction |
| `CalendarClock` | `ICalendarClock` | `CALENDAR_CALC`, `SUN_POS`, `SUN_TIME` | Owns calendar carrier and derived fields |

## Complete Roadmap

Completeness should be built in slices. Every slice needs ST tests against the
classic OSCAT package before implementation.

### v0.2 Control And Signal Expansion

Add:

- `PiController` over `CTRL_PI`
- `PwmController` over `CTRL_PWM`
- `DerivativeFilter` over `FT_DERIV`
- `Integrator` over `FT_INT` / `INTEGRATE`
- `Pt2Filter` over `FT_PT2`
- `MovingAverageDword` over `FILTER_MAV_DW`
- `SampleHold` over `SH`

Do not add a broad `IController` until two or more concrete controllers have
identical operation in practice.

### v0.3 Generator Families

Add separate narrow interfaces:

- `IRandomGenerator`
- `IWaveGenerator`
- `IPwmGenerator`
- `IByteRamp`
- `IWordRamp`

Concrete mappings:

- `GEN_RDM`
- `GEN_RDT`
- `GEN_RMP`
- `GEN_SIN`
- `GEN_SQR`
- `PWM_DC`
- `PWM_PW`
- `RMP_B`
- `RMP_W`
- `_RMP_NEXT`

Do not add a single broad generator interface.

### v0.4 Memory And Counters

Add:

- `DwordFifo32`
- `DwordStack32`
- `OntimeMeter`
- `CycleTimerMs`
- `CycleTimerUs`
- `CycleTimerSeconds`

Mappings:

- `FIFO_32`
- `STACK_32`
- `ONTIME`
- `TC_MS`
- `TC_US`
- `TC_S`

### v0.5 Logic, Latches, And Sensors Classification

Add narrow objects only where state or configuration justifies them:

- `Latch`
- `ToggleSwitch`
- `RisingCounter`
- `FallingCounter`
- `ShiftRegister4`
- `ShiftRegister8`

Mappings:

- `LTCH`
- `TOGGLE`
- `COUNT_BR`
- `COUNT_DR`
- `SHR_4E`
- `SHR_8PLE`

Classify Chapter 20 sensor helpers as classic-only for v1.0 unless a later
user workflow needs a stateful calibrated sensor object. Current Chapter 20
surface is pure functions such as `TEMP_PT`, `TEMP_NI`, `TEMP_NTC`, and
`SENSOR_INT`; those should stay in the classic OSCAT package unless we add a
real object that owns calibration/configuration.

### v0.6 Measuring Objects

Add narrow concrete objects first:

- `FlowMeter`
- `Meter`
- `MeterStats`
- `HeatMeter`
- `Calibrator`
- `BarGraph`

Do not add a general `IMeter` unless the first implementation shows a real
common contract.

### v0.7 Calendar And Holiday Expansion

Add:

- `HolidayCalendar`
- `ScheduledEvents`

Mappings:

- `HOLIDAY`
- `EVENTS`

### v0.8 Device Drivers And Hardware-Time Sources

Add only after a separate design pass:

- `RtcMs`
- `Rtc2`
- `Dcf77Receiver`
- selected driver/manual/interlock objects if they have a clear component role

Mappings:

- `RTC_MS`
- `RTC_2`
- `DCF77`
- `DRIVER_1`
- `DRIVER_4`
- `MANUAL_1`
- `MANUAL_2`
- `MANUAL_4`
- `INTERLOCK`
- `INTERLOCK_4`

This is separate from the calendar slice because RTC/DCF77 are device/time
source components, not calendar-domain calculators.

### v0.9 Building-Control Design Pass

Design and implement building-control objects only after the core wrapper style
has shipped and stabilized. This phase must not use a union interface. Each
object gets a narrow domain interface or a concrete-only public surface.

Candidate objects:

- `BoilerController`
- `BurnerController`
- `HeatCurve`
- `LegionellaCycle`
- `TankLevelController`
- `OutdoorTemperature`

Mappings:

- `BOILER`
- `BURNER`
- `HEAT_TEMP`
- `LEGIONELLA`
- `TANK_LEVEL`
- `TEMP_EXT`

## Chapter Coverage Classification

| OSCAT chapter | v1.0 classification |
| --- | --- |
| `03_data_types` | shared record types reused directly; no wrapper |
| `04_other_functions` | context/status collectors where useful; ESR collectors need separate design |
| `05_mathematics` | classic-only unless a user workflow needs a domain object |
| `06_arrays` | classic-only |
| `07_complex_mathematics` | classic-only |
| `08_arithmetics_with_double_precision` | classic-only |
| `09_arithmetic_functions` | filters/ramp FBs wrapped; pure functions classic-only |
| `10_geometric_functions` | classic-only |
| `11_vector_mathematics` | classic-only unless a vector object is requested |
| `12_time_and_date` | calendar, holiday, and RTC objects wrapped where stateful |
| `13_string_functions` | classic-only in v1.0 unless a text workflow proves object value |
| `14_memory_modules` | FIFO/stack objects wrapped |
| `15_pulse_generators` | separate generator-family objects |
| `16_logic_modules` | pure bit functions classic-only; stateful logic is covered through chapter 17 objects |
| `17_latches_flip_flop_and_shift_register` | latch/toggle/counter/shift-register objects |
| `18_signal_generators` | separate generator-family objects |
| `19_signal_processing` | classic-only unless a future workflow needs an owning processing object |
| `20_sensors` | classic-only by default; stateful calibrated sensor object only if justified |
| `21_measuring_modules` | ontime, cycle-time, calibration, and bar-graph objects wrapped |
| `22_calculations` | selected result-record service methods only |
| `23_control_modules` | controller/filter/building-control objects |
| `24_device_driver` | selected narrow device-driver objects wrapped; complex protocol/profile objects classic-only |
| `25_buffer_management` | classic-only unless a true owning buffer object is added |
| `26_list_processing` | classic-only unless a true owning list object is added |

### v1.0 Completion Criteria

The component library can be called complete when:

- every stateful OSCAT FB family has either a component wrapper or an explicit
  documented reason not to wrap it
- every pure function family is classified as classic-only, result-record
  service, or out-of-scope with rationale
- each wrapper is tested against the classic OSCAT behavior in ST
- every public component has documentation and at least one realistic example
  or is covered by a larger workflow example
- classic OSCAT remains public and documented as the parity source of truth

## Example API Shape

### PID controller

```iecst
PROGRAM Main
VAR
    Controller : PidController;
    Control : IPidController;
    Limits : RealRange;
    Output : REAL;
END_VAR

Control := Controller;

Limits.Low := REAL#0.0;
Limits.High := REAL#100.0;

Control.SetKp(Kp := REAL#2.0);
Control.SetIntegralTime(Tn := REAL#8.0);
Control.SetDerivativeTime(Tv := REAL#0.5);
Control.SetLimits(Limits := Limits);

Output := Control.Update(Actual := REAL#44.2, Target := REAL#50.0);
END_PROGRAM
```

### Calendar

```iecst
PROGRAM Main
VAR
    Calendar : CalendarClock;
    Clock : ICalendarClock;
END_VAR

Clock := Calendar;
Clock.Configure(
    LocationLatitude := REAL#59.3293,
    LocationLongitude := REAL#18.0686,
    OffsetMinutes := INT#60,
    DstEnabled := TRUE,
    LanguageIndex := INT#1
);
Clock.Update(Utc := DT#2026-04-26-10:00:00);
END_PROGRAM
```

### FIFO

```iecst
PROGRAM Main
VAR
    Queue : DwordFifo16;
    QueueObject : IDwordQueue;
    Value : DWORD;
END_VAR

QueueObject := Queue;
IF QueueObject.Push(Value := DWORD#16#12345678) THEN
    IF QueueObject.TryPop() THEN
        Value := QueueObject.Value;
    END_IF;
END_IF;
END_PROGRAM
```

## Tests First

Add the failing ST fixture before writing wrapper code.

Minimum v0.1 ST tests:

- `AutomationContext` loads constants and returns the same `OSCAT_VERSION()`
  as classic OSCAT.
- `UnitConverter` methods match classic `TEMPERATURE`, `SPEED`, `ENERGY`, and
  scalar conversion helpers.
- `PidController` and classic `CTRL_PID` produce identical outputs over a scan
  sequence with reset, automatic mode, manual mode, and limit behavior.
- `Pt1Filter` and classic `FT_PT1` produce identical outputs over a scan
  sequence.
- `HysteresisSwitch` and classic `HYST` produce identical `Q` and `WIN`.
- `PulseGenerator` and classic `GEN_PULSE` produce identical `Q`.
- FIFO and stack wrappers preserve push/pop order, empty/full outputs, and
  reset behavior.
- `CalendarClock` updates `CALENDAR` fields consistently with
  `CALENDAR_CALC` for UTC, local date/time, DST flag, sun rise/set, and work
  week.
- Real negative/state-isolation test: one shared `UnitConverter` used by two
  call sites returns independent scalar values and does not retain hidden
  result state.
- Documentation requirement: every stateful example uses one component instance
  per logical device; reusing a single stateful instance for two devices is
  documented as invalid application structure.

Suggested Rust driver shape:

```rust
use std::path::PathBuf;
use std::process::Command;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("oscat")
        .join(name)
}

fn assert_trust_runtime_test_passes(project: PathBuf) {
    let output = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args(["test", "--project"])
        .arg(&project)
        .output()
        .expect("run trust-runtime test");

    assert!(
        output.status.success(),
        "expected ST fixture tests to pass for {}\nstdout:\n{}\nstderr:\n{}",
        project.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn oscat_oop_core_st_unit_tests_pass() {
    assert_trust_runtime_test_passes(fixture_path("oop_core"));
}
```

Use the existing OSCAT core fixture as the comparison source. The wrapper test
program should instantiate both the classic FB and the component wrapper in the
same scan sequence and assert equality after each step.

## Documentation And Examples

If implemented, add:

- `docs/guides/OSCAT_OOP_LIBRARY_GUIDE.md`
- `docs/public/develop/libraries/oscat-oop.md`
- A link from `docs/public/develop/libraries/index.md`
- A link from `docs/public/develop/libraries/oscat.md`
- A link from `docs/public/examples/libraries-and-motion.md`
- `libraries/oscat/oop/README.md`

The catalog ships comparison examples as pairs: one classic OSCAT project and
one OSCAT OOP project for the same machine/process scenario. The
implemented target is 49 comparison pairs: 27 hand-written process-first
industrial pattern scenarios, 20 compact component-composition showcases, and 2
compact pattern showcases. The industrial-pattern acceptance contract lives in
`docs/internal/references/OSCAT/oscat_oop_realworld_pattern_catalog.md`.

The catalog is intentionally not component-first. It demonstrates the OOP
surface inside real machines: batch reactor, AHU, water booster station, tank
farm, refinery signal conditioning, boiler room, pasteurizer, CIP skid,
chemical dosing skid, VFD motor cell, cold storage plant, commissioning mode,
filling line, palletizer, silo loading, tunnel oven, hoist cell, filter
backwash, tunnel washer, battery cabinet, conveyor merge, cleanroom pressure
cascade, cooling tower, kiln dryer, baggage diverter, dairy separator, district
pump network, plus compact polymorphism and composition showcases.

Each example should pass its Structured Text tests:

```bash
trust-runtime test --project examples/<example-name>
```

## Acceptance Gates

Targeted gates:

- `cargo test -p trust-runtime --test oscat_oop_library`
- `cargo test -p trust-runtime --test oscat_oop_examples`
- `scripts/render_diagrams.sh`
- `python scripts/check_diagram_drift.py`

Final gates:

- `just fmt`
- `just clippy`
- `just test-all`

Because this is user-facing library work, release hygiene applies when the
component package is implemented:

- update `CHANGELOG.md`
- bump the workspace version
- keep docs and examples aligned
- if the workspace version changes, keep VS Code package versions in sync
- keep classic OSCAT and component package versions aligned unless a future
  package-version policy explicitly changes that

## Risks And Constraints

- Do not break classic OSCAT parity. The classic package is the compliance and
  upstream-audit surface.
- Do not wrap pure functions too aggressively. It would increase maintenance
  work without improving scan logic or readability.
- Do not create broad interfaces just to make many classic FBs look related.
- Do not use `REFERENCE TO` in public signatures.
- Do not expose `VAR_IN_OUT ARRAY[*]` through an object shell unless the
  component adds real ownership or domain behavior.
- Be explicit where truST keyword differences already exist: `OVERRIDE_3`,
  `LOCAL_DT`, `LOCAL_DATE`, and `LOCAL_TOD`.
- Avoid hidden retained state in service objects.

## Final Decision

Build it, but build it in layers. v0.1 should be a narrow kernel that proves
the API style. Then grow toward a complete component package domain by domain.

The complete target is worthwhile because OSCAT has enough stateful,
configuration-heavy, and status-heavy modules to benefit from object-oriented
use. The first release should not pretend those domains all share one
interface. Small interfaces, domain names, concrete components, ST parity
tests, and classic OSCAT delegation are the rules that keep the complete
library maintainable.
