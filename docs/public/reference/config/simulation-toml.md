# `simulation.toml`

`simulation.toml` lets you script deterministic virtual wiring and timed fault
injection without touching real hardware.

## Example

```toml
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
at_ms = 1800
kind = "fault"
message = "tutorial simulated input dropout"
```

## Sections

### `[simulation]`

| Key | Type | Default | Notes |
| --- | --- | --- | --- |
| `enabled` | bool | `false` | Enables scripted simulation behavior. |
| `seed` | integer | `0` | Deterministic seed for repeatable scenarios. |
| `time_scale` | integer | `1` | Simulation time acceleration factor. |

### `[[couplings]]`

Couplings copy or transform one I/O point into another after a delay.

| Key | Type | Required | Notes |
| --- | --- | --- | --- |
| `source` | IEC address | yes | Source I/O address. |
| `target` | IEC address | yes | Target I/O address. |
| `threshold` | float | no | Optional decision threshold. |
| `delay_ms` | integer | no | Delay before the effect is applied. |
| `on_true` | string | no | Value written when the condition evaluates true. |
| `on_false` | string | no | Value written when the condition evaluates false. |

### `[[disturbances]]`

Disturbances schedule explicit events on the simulated plant.

| Key | Type | Required | Notes |
| --- | --- | --- | --- |
| `at_ms` | integer | yes | Simulation time when the event fires. |
| `kind` | string | yes | `set` or `fault`. |
| `target` | IEC address | for `set` | I/O target written by the disturbance. |
| `value` | string | for `set` | Typed value to write. |
| `message` | string | for `fault` | Fault text injected into the runtime. |

## Typical Use Cases

- loop an output back into an input with realistic delay
- inject sensor dropouts or spikes on a schedule
- accelerate time for repeated commissioning scenarios
- make CI or tutorial demos reproducible

## Related

- [Simulation Workflow](../../operate/simulation.md)
- [Create A New Project](../../start/create-new-project.md)
