# Simulation-First PLC Workflow

This guide covers deterministic simulation runs for PLC projects without physical hardware.

## 1) Enable simulation mode

Create `simulation.toml` in your project folder:

```toml
[simulation]
enabled = true
seed = 42
time_scale = 8

[[couplings]]
source = "%QW0"
target = "%IX0.0"
threshold = 500.0
delay_ms = 100
on_true = "TRUE"
on_false = "FALSE"

[[disturbances]]
at_ms = 250
kind = "set"
target = "%IX0.1"
value = "TRUE"

[[disturbances]]
at_ms = 1250
kind = "fault"
message = "simulated sensor dropout"
```

What this does:
- `couplings`: output-to-input wiring rules for simulation.
- `delay_ms`: delayed effect timing.
- `disturbances`: scripted input changes and fault injection.

## 2) Run with explicit simulation branding

```bash
trust-runtime play --project <project-folder> --simulation --time-scale 8
```

- `--simulation` forces simulation mode even if `simulation.toml` is absent.
- `--time-scale` accelerates simulation time (`>= 1`).

## 3) Validate behavior safely

Recommended checks before touching hardware:

```bash
trust-runtime build --project <project-folder>
trust-runtime validate --project <project-folder>
trust-runtime test --project <project-folder> --output junit
```

## 4) Understand mode indicators

- CLI banner shows `Simulation mode` and a safety warning.
- TUI status panel shows mode and time scale.
- Web status panel shows mode and warning.

Simulation mode is for development/test only. Do not use simulation wiring for live safety-critical outputs.

