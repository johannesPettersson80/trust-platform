# Tutorial 18: Simulation Mode with `simulation.toml` Couplings and Fault Injection

This tutorial adds deterministic simulation behavior to a local project and
shows how to exercise couplings, disturbances, and synthetic fault events.

## Why this tutorial exists

Runtime panel input poking is useful, but repeatable simulation scenarios are
better for debugging and CI. `simulation.toml` gives deterministic, scripted
behavior you can re-run exactly.

## What you will learn

- structure of `simulation.toml`
- difference between couplings and disturbances
- how to force simulation mode from CLI
- how to use simulation before hardware commissioning

## Prerequisites

- complete Tutorial 13 first

## Step 1: Create a simulation sandbox

Why: simulation changes should be isolated from normal runtime configs.

```bash
rm -rf /tmp/trust-simulation
cp -R /tmp/trust-tutorial-13 /tmp/trust-simulation
cd /tmp/trust-simulation
```

## Step 2: Build and validate baseline project

Why: verify base project health before introducing simulation variables.

```bash
trust-runtime build --project . --sources src
trust-runtime validate --project .
```

## Step 3: Add `simulation.toml`

Why: this file defines deterministic virtual wiring and scheduled events.

```bash
cat > simulation.toml <<'TOML'
[simulation]
enabled = true
seed = 42
time_scale = 8

[[couplings]]
source = "%QX0.0"
target = "%IX0.0"
delay_ms = 100
on_true = "TRUE"
on_false = "FALSE"

[[disturbances]]
at_ms = 250
kind = "set"
target = "%IX0.0"
value = "TRUE"

[[disturbances]]
at_ms = 1200
kind = "set"
target = "%IX0.0"
value = "FALSE"

[[disturbances]]
at_ms = 1800
kind = "fault"
message = "tutorial simulated input dropout"
TOML
```

What each section does:
- `[simulation]`: enables mode + deterministic seed/time scaling
- `[[couplings]]`: automatic output-to-input behavior across cycles
- `[[disturbances]]`: timed forced inputs and fault injection events

## Step 4: Run in explicit simulation mode

Why: `--simulation` makes mode unambiguous even if file paths change.

```bash
trust-runtime play --project . --simulation --time-scale 8
```

Expected result:
- runtime banner/status indicates simulation mode
- scripted events occur at deterministic simulation times

## Step 5: Observe effects in UI

Why: simulation is only useful if you can verify expected state transitions.

Open runtime UI and observe `%IX0.0` / `%QX0.0` transitions around scheduled
`disturbances` timings. Confirm alarm/fault message appears for the injected
fault event.

## Step 6: Change one scenario and re-run

Why: quick scenario iteration is the core simulation productivity gain.

Edit `simulation.toml` and adjust one value, for example:
- change first `set` event to `at_ms = 500`
- or change `delay_ms` in couplings

Re-run:

```bash
trust-runtime play --project . --simulation --time-scale 8
```

Expected result:
- behavior changes exactly with your edited schedule

## Step 7: Use simulation as a pre-hardware gate

Why: this catches logic regressions before touching physical outputs.

Recommended pre-hardware command set:

```bash
trust-runtime build --project .
trust-runtime validate --project .
trust-runtime test --project . --output junit
```

## Common mistakes

- forgetting `--simulation` and assuming simulation file was used
- using simulation couplings as if they were production wiring
- not pinning seed/time-scale when comparing two runs

## Completion checklist

- [ ] `simulation.toml` created and understood
- [ ] couplings and disturbances observed in runtime
- [ ] one scenario modified and re-run deterministically
- [ ] simulation used as pre-hardware validation gate
