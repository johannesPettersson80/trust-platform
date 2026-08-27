# Simulation-First PLC Workflow

This guide covers simulated plants that use ordinary PLC process-image I/O.
The PLC program does not know that a simulator is connected: it writes a `%Q`
command and reads `%I` feedback exactly as it would with an actuator and sensor.

## Create a Tank plant in VS Code

1. Open **truST: Devices & Connections**.
2. Open **Add**, then click or drag **Simulated plant** onto the runtime.
3. Choose **Tank or accumulated level**.
4. Select a compiled numeric `%Q` output as the command and a compiled numeric
   `%I` input as the feedback.
5. Keep the proposed gain and initial state for a first run, then select
   **Save**.

Save changes `simulation.toml`. If an older configuration is running, VS Code
reports **Saved · restart required**; it does not claim that the new plant is
already active. Open **Control Studio** from the saved plant to inspect live
simulation values. Plant Properties remains a configuration surface and does
not duplicate live charts.

The Tank starting point is useful for level, inventory, and accumulated
position because its state accumulates the command relative to its bias. It
does not settle by itself and does not model leakage unless those effects are
added.

## Models and editable values

The drawer preserves the exact entered numbers. Bias and state units follow
the engineering units of the selected PLC signals.

| Model | Editable values | Units and use |
| --- | --- | --- |
| Static gain | `gain`, `input_bias`, `output_bias` | gain: output/input; immediate scaling with no stored state |
| First order | static-gain values, `time_constant_s`, `initial_state` | time constant: seconds; one lagging state |
| First order with dead time | first-order values, `dead_time_s`, `initial_delayed_input` | delay and time constant: seconds |
| Integrator | `gain`, biases, `state_minimum`, `state_maximum`, `initial_state` | gain: output/(input·second); omitted bounds are unbounded |
| Second order | `gain`, `natural_frequency_rad_s`, `damping_ratio`, biases, two initial states | natural frequency: rad/s; damping ratio: dimensionless |
| Piecewise linear | ordered `input_points` and `output_points` | signal engineering units; input points must increase strictly |
| State space | matrices `a`, `b`, `c`, `d`, and `initial_state` | matrix entries map state, input, rate, and output engineering units |

The familiar starting points are editable proposals, not identified models:

- **Temperature or slow process** uses one lag and a delay; it cannot represent
  resonance, several time scales, or nonlinear heating.
- **Motor speed or first-order process** uses one dominant lag; it cannot
  represent resonance, delay, saturation, or friction.
- **Mechanical position or resonant process** represents inertia and damping;
  it cannot represent backlash, friction, or hard stops.
- **Static scaling** has no dynamics.
- **Nonlinear characteristic** represents an ordered static curve, not
  hysteresis or rate dependence.
- **Custom state space** represents coupled linear states, not nonlinear or
  time-varying behavior.

## Actuator, imperfections, and events

For scalar plants, actuator settings expose `minimum`, `maximum`,
`rate_up_per_s`, `rate_down_per_s`, `deadband`, `lag_s`, and
`initial_applied`. Empty limits and rates are unbounded. A rate of zero blocks
movement in that direction; it is not the same as an empty field.

Imperfections can add bias, drift, Gaussian noise, quantisation, sample and
hold, dropout, a stuck sensor, or an outlier. Enabled rows execute in displayed
order. Scenario events schedule deliberate state, parameter, sensor, actuator,
reset, or fault changes at simulation time; equal-time events also execute in
displayed order.

Simulation is not physical-plant evidence. An enabled physical-I/O driver owns
the process image and blocks simulated plant assignment; there is no mixed-I/O
override. Validate limits, failure behavior, and safety again on the reviewed
physical topology before commissioning.

## Configure simulation as text

The VS Code drawer is the normal authoring route. The equivalent file is
`simulation.toml`; direct editing remains available for automation and review.

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

## Run with explicit simulation branding

```bash
trust-runtime play --project <project-folder> --simulation --time-scale 8
```

- `--simulation` forces simulation mode even if `simulation.toml` is absent.
- `--time-scale` accelerates simulation time (`>= 1`).

## Validate behavior safely

Recommended checks before touching hardware:

```bash
trust-runtime build --project <project-folder>
trust-runtime validate --project <project-folder>
trust-dev test --project <project-folder> --output junit
```

## Understand mode indicators

- CLI banner shows `Simulation mode` and a safety warning.
- TUI status panel shows mode and time scale.
- Web status panel shows mode and warning.

Simulation mode is for development/test only. Do not use simulation wiring for live safety-critical outputs.
