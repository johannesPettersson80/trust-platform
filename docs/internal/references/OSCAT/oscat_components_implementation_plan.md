# OSCAT Components Implementation Plan

Date: 2026-04-26

Status: In progress

Scope: ship the first complete OSCAT Components release as an optional OOP
facade over `libraries/oscat`, with a documented v1.0 completion path for all
OSCAT families that benefit from object identity.

## Release Goal

Deliver `libraries/oscat/components` as a tested, documented, example-backed
library package. The package must coexist with classic OSCAT; classic OSCAT
remains the behavior source of truth and the non-OOP comparison surface.

This release is complete for the v0.1 component slice:

- `AutomationContext`
- `UnitConverter`
- `Pt1Filter`
- `PidController`
- `HysteresisSwitch`
- `PulseGenerator`
- `DwordFifo16`
- `DwordStack16`
- `CalendarClock`

The v1.0 completion roadmap remains staged because OSCAT covers unrelated
domains. Future slices must follow the same test-first and documentation gates.

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

- `crates/trust-runtime/tests/fixtures/oscat/components_core/trust-lsp.toml`
- `crates/trust-runtime/tests/fixtures/oscat/components_core/runtime.toml`
- `crates/trust-runtime/tests/fixtures/oscat/components_core/src/tests.st`
- `crates/trust-runtime/tests/oscat_components_library.rs`

ST tests:

- package symbols load through dependency alias `AutomationComponents`
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

Target 20 pairs, 40 total examples:

- tank level PID: classic `CTRL_PID` vs OOP `PidController`
- greenhouse temperature: classic `HYST` / `TEMPERATURE` vs OOP
  `HysteresisSwitch` / `UnitConverter`
- conveyor pulse scheduler: classic `GEN_PULSE` vs OOP `PulseGenerator`
- production FIFO queue: classic `FIFO_16` vs OOP `DwordFifo16`
- maintenance task stack: classic `STACK_16` vs OOP `DwordStack16`
- ventilation filter: classic `FT_PT1` / `SPEED` vs OOP `Pt1Filter` /
  `UnitConverter`
- solar lighting clock: classic `CALENDAR_CALC` / `SUN_TIME` vs OOP
  `CalendarClock`
- pump pressure deadband: classic `HYST` vs OOP `HysteresisSwitch`
- energy normalization: classic `ENERGY` vs OOP `UnitConverter`
- wind alarm conversion: classic `SPEED` / `MS_TO_BFT` vs OOP
  `UnitConverter`
- cold storage alarm: classic `TEMPERATURE` / `HYST` vs OOP
  `UnitConverter` / `HysteresisSwitch`
- wastewater aeration: classic `HYST` / `GEN_PULSE` vs OOP
  `HysteresisSwitch` / `PulseGenerator`
- packaging reject pulse: classic `GEN_PULSE` vs OOP `PulseGenerator`
- recipe batch stack: classic `STACK_16` vs OOP `DwordStack16`
- shift order queue: classic `FIFO_16` vs OOP `DwordFifo16`
- irrigation sun clock: classic `CALENDAR_CALC` vs OOP `CalendarClock`
- compressor pressure filter: classic `FT_PT1` / `HYST` vs OOP
  `Pt1Filter` / `HysteresisSwitch`
- chiller temperature PID: classic `TEMPERATURE` / `CTRL_PID` vs OOP
  `UnitConverter` / `PidController`
- boiler feedwater alarm: classic `HYST` / `FIFO_16` vs OOP
  `HysteresisSwitch` / `DwordFifo16`
- weather station conversion: classic constants/direction helpers vs OOP
  `AutomationContext` / `UnitConverter`

Each pair must include:

- `trust-lsp.toml`
- `src/Main.st`
- `src/Tests.st`
- `README.md`

### 3. Documentation

Internal docs:

- keep `oscat_oop_wrapper_design.md` as the design source
- keep this implementation plan updated while work progresses

Developer docs:

- add `docs/guides/OSCAT_COMPONENTS_LIBRARY_GUIDE.md`
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

- `docs/internal/references/OSCAT/oscat_components_naming_audit.md`

Rules:

- new OSCAT Components examples must comply fully
- generated/vendor/imported examples may preserve inherited names if documented
- legacy truST-authored examples should be updated where low-risk
- remaining exceptions must be recorded with a reason and follow-up

### 5. Validation Cadence

Targeted gates during implementation:

- `cargo test -p trust-runtime --test oscat_components_library`
- `cargo test -p trust-runtime --test oscat_components_examples`
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

Every future slice must repeat the same pattern: write ST parity tests first,
then implement the component wrapper, then add docs and examples.

- v0.2: `PiController`, `PwmController`, `DerivativeFilter`, `Integrator`,
  `Pt2Filter`, `MovingAverageDword`, `SampleHold`
- v0.3: separate random, wave, PWM, byte-ramp, and word-ramp generator
  families
- v0.4: `DwordFifo32`, `DwordStack32`, `OntimeMeter`, cycle timers
- v0.5: latch, toggle, edge counters, shift-register objects; sensor helpers
  remain classic-only unless calibrated state is introduced
- v0.6: measuring objects such as flow, meter, heat meter, calibrator, bar
  graph
- v0.7: holiday calendar and scheduled events
- v0.8: RTC/DCF77 and selected device-driver objects after a separate design
  pass
- v0.9: building-control objects after a separate design pass with narrow
  domain interfaces only

## Status Checklist

- [x] Internal design reviewed and naming standard tightened
- [x] ST library fixture added and observed failing before implementation
- [x] example Rust driver added and observed failing before examples exist
- [x] OSCAT Components package implemented
- [x] ST library fixture passing
- [x] 20 classic/OOP example pairs added
- [x] example ST tests passing
- [x] library docs and public docs updated
- [x] naming-standard docs updated
- [x] example/tutorial naming audit completed
- [x] targeted gates passing
- [x] final release gates passing
- [ ] committed, pushed, merged, and release/version bump verified
