# OSCAT OOP Implementation Plan

Date: 2026-04-26

Status: v1.0 completion pass in progress

Scope: ship OSCAT OOP as an optional OOP facade over `libraries/oscat`
for every OSCAT family in this repository that benefits from object identity.
Pure scalar functions and vendor-shaped helper surfaces remain classic-only by
design and are not counted as missing component coverage.

## Release Goal

Deliver `libraries/oscat/oop` as a tested, documented, example-backed
library package. The package must coexist with classic OSCAT; classic OSCAT
remains the behavior source of truth and the non-OOP comparison surface.

This release now includes the v1.0 component surface:

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

The remaining OSCAT symbols are intentionally classic-only when they are pure
functions, generated/helper internals, or device/vendor shapes where an object
surface would hide rather than improve the upstream contract.

## Non-Negotiable Gates

- Every new component starts with Structured Text unit tests before
  implementation.
- Every wrapper test compares against the classic OSCAT symbol where parity is
  meaningful.
- All public component names follow the truST naming standard:
  PascalCase domain names for truST-owned API, exact spelling for inherited
  PLCopen/OSCAT/vendor symbols.
- No new public `OOP`, `OSCAT`, `Wrapper`, or `Facade` type prefix/suffix.
- No user-facing `REFERENCE TO`.
- No broad union interfaces.
- No hidden `Last...` service caches.
- Configuration setters return no value unless they can reject invalid input.
- Examples ship in OOP and non-OOP pairs and each example has ST unit tests.
- Public docs, internal docs, README files, and navigation are updated.
- Release hygiene is complete: changelog, version consistency, checks, commit,
  push, merge, tag/release monitoring.

## Test-First Work Items

### 1. Library ST Fixture

Files:

- `crates/trust-runtime/tests/fixtures/oscat/oop_core/trust-lsp.toml`
- `crates/trust-runtime/tests/fixtures/oscat/oop_core/runtime.toml`
- `crates/trust-runtime/tests/fixtures/oscat/oop_core/src/tests.st`
- `crates/trust-runtime/tests/oscat_oop_library.rs`

ST tests:

- package symbols load through dependency alias `OscatOop`
- `AutomationContext` loads OSCAT constants and direction helpers match
  classic functions
- `UnitConverter` scalar methods match selected classic conversion functions
  and direct unit formulas where OSCAT exposes FB-only conversions
- `Pt1Filter.Update` first-scan output matches `FT_PT1`
- `PidController.Update` output, difference, and limit status match `CTRL_PID`
- `HysteresisSwitch.Update` output and window status match `HYST`
- `PulseGenerator.Update` disabled/enabled behavior matches `GEN_PULSE`
- `DwordFifo16` push/pop/reset behavior matches `FIFO_16`
- `DwordStack16` push/pop/reset behavior matches `STACK_16`
- `CalendarClock.Update` derived local time, calendar fields, sun data, and
  work week match `CALENDAR_CALC`
- interface dispatch works for every public interface
- negative state-sharing test proves two component instances keep independent
  state

### 2. Example ST Fixtures

Add a Rust driver that runs `trust-runtime test --project` for every new
example pair. The ST assertions live inside each example.

Target 49 comparison pairs: 27 industrial pattern pairs, 20 compact
component-composition showcase pairs, and two compact pattern showcase pairs.
The industrial catalog is process-first: examples are named after real
machines/processes, not library features. Calibration, historian records,
diagnostics, communication, and commissioning are included inside complete
applications when they are part of the scenario.

The primary scenarios are documented in
`docs/internal/references/OSCAT/oscat_oop_realworld_pattern_catalog.md` and
include:

- multi-product batch reactor
- HVAC air handling unit
- water booster pump station
- tank farm transfer skid
- refinery temperature conditioning
- boiler room heating plant
- pasteurizer temperature control
- CIP wash skid
- chemical dosing skid
- mixed-vendor VFD motor cell
- cold storage plant
- water booster commissioning mode
- pharmaceutical filling line
- robotic palletizer cell
- silo loading system
- tunnel oven
- crane hoist load cell
- water treatment filter backwash
- tunnel washer laundry line
- battery energy storage cabinet
- warehouse conveyor merge
- cleanroom pressure cascade
- cooling tower cell
- kiln dryer moisture control
- airport baggage diverter
- dairy separator skid
- district pump network

Each pair must include:

- `trust-lsp.toml`
- `runtime.toml` when the scenario exposes OPC UA/runtime state
- `io.toml` when the scenario includes Modbus, MQTT, EtherCAT, or other
  runtime IO boundaries
- `src/Main.st`
- `src/Tests.st`
- detailed `README.md` explaining the process, problem, field signals,
  communication/logging boundary, state machine, alarm model, OOP pattern,
  pattern tradeoffs, and when classic ST is simpler

### 3. Documentation

Internal docs:

- keep `oscat_oop_wrapper_design.md` as the design source
- keep this implementation plan updated while work progresses

Developer docs:

- add `docs/guides/OSCAT_OOP_LIBRARY_GUIDE.md`
- add public docs under `docs/public/develop/libraries/`
- update `docs/public/develop/libraries/index.md`
- update `docs/public/examples/libraries-and-motion.md`
- update `mkdocs.yml`

Naming docs:

- add a normal user-facing naming-standard page
- link it from project layout / library docs
- include constants, parameters, locals, source files, examples, and inherited
  symbol exceptions

### 4. Naming Audit

Audit examples and tutorials after the standard is documented.

Audit record:

- `docs/internal/references/OSCAT/oscat_oop_naming_audit.md`

Rules:

- new OSCAT OOP examples must comply fully
- generated/vendor/imported examples may preserve inherited names if documented
- legacy truST-authored examples should be updated where low-risk
- remaining exceptions must be recorded with a reason and follow-up

### 5. Validation Cadence

Targeted gates during implementation:

- `cargo test -p trust-runtime --test oscat_oop_library`
- `cargo test -p trust-runtime --test oscat_oop_examples`
- `cargo test -p trust-runtime --test tutorial_examples`
- `cargo test -p trust-runtime --test st_test_cli_command`

Final gates before commit:

- `cargo test -p trust-runtime --test api_smoke`
- `cargo test -p trust-runtime --test debug_control`
- `cargo test -p trust-runtime --test complete_program`
- `cargo test -p trust-runtime --test runtime_reliability`
- `scripts/render_diagrams.sh`
- `python scripts/check_diagram_drift.py`
- `just fmt`
- `just clippy`
- `just test-all`

## v1.0 Completion Roadmap

Every implemented slice followed the same pattern: write ST parity tests first,
then implement the component wrapper, then add docs and examples.

- [x] v0.2: `PiController`, `PwmController`, `DerivativeFilter`,
  `IntegratorFilter`, `Pt2Filter`, `DelayLine16`
- [x] v0.3: separate random, sine, square, byte-ramp, and word-ramp generator
  families
- [x] v0.4: `DwordFifo32`, `DwordStack32`, `OntimeMeter`, `CycleTimeMeter`
- [x] v0.5: `Latch`, `ToggleSwitch`, `DwordCounter`, `ShiftRegister8`
- [x] v0.6: `Calibrator`, `BarGraphMeter`
- [x] v0.7: `HolidayCalendar`
- [x] v0.8: `RtcClock`, `SingleOutputDriver`
- [x] v0.9: `TankLevelController`, `HeatCurve`, `BoilerController`

Classic-only decisions:

- Pure scalar chapters, array/list/buffer helpers, math/string/geometric/vector
  helpers, and raw sensor conversion functions stay classic-only.
- DCF77 and complex device-driver profiles stay classic-only for this release
  because their upstream bit/protocol surfaces are already the public contract.
- Building objects with broad or safety-specific process contracts not listed
  above require separate domain design before they get component names.

## Status Checklist

- [x] Internal design reviewed and naming standard tightened
- [x] ST library fixture added and observed failing before implementation
- [x] example Rust driver added and observed failing before examples exist
- [x] OSCAT OOP package implemented
- [x] ST library fixture passing
- [x] 49 classic/OOP comparison pairs implemented: 27 industrial pattern pairs,
  20 compact component-composition showcases, and 2 compact pattern showcases
- [x] example ST tests passing after real-world catalog expansion
- [x] library docs and public docs updated
- [x] naming-standard docs updated
- [x] example/tutorial naming audit completed
- [x] targeted gates passing
- [x] final release gates passing after real-world catalog expansion
- [ ] committed, pushed, merged, and release/version bump verified

## Post-Review Hardening Checklist

Added after the external review on 2026-04-26. No commit, push, tag, or release
action is allowed before this checklist is implemented, validated, and reviewed
with the user.

- [x] Add ST regressions for `Pt1Filter.Reset()` and
  `HysteresisSwitch.Reset()`.
- [x] Add multi-scan parity coverage for `Pt1Filter`.
- [x] Add invalid-range tests for PID and hysteresis limit configuration.
- [x] Add FIFO multi-value ordering coverage.
- [x] Remove hardcoded OSCAT version expectations from ST tests.
- [x] Delegate `UnitConverter` temperature and energy conversions to classic
  OSCAT symbols.
- [x] Replace ambiguous PID, calendar, and astronomy names with the settled
  PascalCase API.
- [x] Remove simple-example interface theater; reserve interface variables for
  real polymorphic seams.
- [x] Replace PID setter explosion in examples with `PidGains` plus
  `Configure(...)`.
- [x] Demonstrate component `Snapshot()` and configuration error handling in
  examples.
- [x] Surface `AutomationContext` constant-load failure through status/error
  fields instead of clearing it.
- [x] Keep `PidController.TargetValue` as the read-only property while using
  `Target` as the `Update(...)` scan parameter.
- [x] Document the deliberate `Pt1Filter.Reset()` snap-to-sample behavior and
  `HysteresisSwitch` inverted-limit rejection.
- [x] Document `Configure(...)` as PID initial setup and individual setters as
  runtime adjustment methods.
- [x] Identify the weather-station pair as the dedicated `AutomationContext`
  comparison example.
- [x] Add architectural comparison examples for polymorphism, composition,
  multi-instance fan-out, and diagnostics snapshots.
- [x] Replace shallow component-demo catalog with process-first real-world
  machine examples that include state machines, alarm handling, logging/
  historian records, communication boundaries, and README pattern explanations.
- [x] Run targeted component and example ST gates after real-world expansion.
- [x] Run docs link/build gates after real-world expansion.
- [ ] User review checkpoint before any commit or push.
