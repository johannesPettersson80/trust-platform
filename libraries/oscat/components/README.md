# OSCAT Components

OSCAT Components is an object-oriented facade over the classic `libraries/oscat`
package. It keeps classic OSCAT as the behavior source of truth and adds
component-shaped function blocks for stateful or domain-oriented workflows.

Use this package when application code benefits from object identity,
read-only status properties, explicit scan methods, and narrow interfaces.
Use classic OSCAT directly for pure scalar helper functions and for vendor or
legacy code that already targets OSCAT names.

## Package

```toml
[dependencies]
AutomationComponents = { path = "../../libraries/oscat/components", version = "0.1.0" }
```

## Naming

New truST-owned API names use readable PascalCase domain names:
`PidController`, `Pt1Filter`, `DwordFifo16`, `DefaultPidKp`.
Inherited OSCAT names remain unchanged in the classic package:
`CTRL_PID`, `FT_PT1`, `FIFO_16`, `STRING_LENGTH`.

## v0.1 Components

- `AutomationContext`
- `UnitConverter`
- `Pt1Filter`
- `PidController`
- `HysteresisSwitch`
- `PulseGenerator`
- `DwordFifo16`
- `DwordStack16`
- `CalendarClock`

Each component is covered by Structured Text tests under
`crates/trust-runtime/tests/fixtures/oscat/components_core`.

## Documentation And Examples

- User guide: `docs/guides/OSCAT_COMPONENTS_LIBRARY_GUIDE.md`
- Public docs page: `docs/public/develop/libraries/oscat-components.md`
- Comparison examples: `examples/oscat_components_*`

The example set contains 20 real-world comparison scenarios, shipped as 40
projects total: one classic OSCAT project and one OSCAT Components project per
scenario. Each project has a README and Structured Text tests.
