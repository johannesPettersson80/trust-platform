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

[physics]
enabled = true
backend = "in_tree_rapier"
step_ms = 10
encoder_counts_per_radian = 1000.0

[[physics.joints]]
id = "axis-1"
kind = "revolute"
enable_source = "%QX0.0"
feedback_target = "%IW0"
velocity_rad_per_s = 1.0
lower_rad = 0.0
upper_rad = 1.570796
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

### `[physics]`

Physics runs deterministic virtual plant motion behind `SimulationController`.
P2 supports the in-tree Rapier backend only.

| Key | Type | Default | Notes |
| --- | --- | --- | --- |
| `enabled` | bool | `true` when `[physics]` exists | Enables physics stepping. |
| `backend` | string | `in_tree_rapier` | Only `in_tree_rapier` is supported. |
| `step_ms` | integer | `10` | Fixed physics step duration. |
| `encoder_counts_per_radian` | float | `1000.0` | Default encoder scale for joints. |

### `[[physics.joints]]`

Joints read PLC outputs after a scan and queue encoder feedback for the next
pre-cycle input write. Encoder feedback must use a word input such as `%IW0`;
`%I0.0` is a bit address and is not valid for encoder values.

| Key | Type | Required | Notes |
| --- | --- | --- | --- |
| `id` | string | yes | Stable joint identifier. |
| `kind` | string | yes | P2 supports `revolute`. |
| `enable_source` | IEC address | yes | PLC output bit, for example `%QX0.0`. |
| `feedback_target` | IEC address | yes | PLC input word, for example `%IW0`. |
| `velocity_rad_per_s` | float | no | Constant enabled joint velocity. |
| `lower_rad` | float | no | Lower angle clamp. |
| `upper_rad` | float | no | Upper angle clamp. |
| `encoder_counts_per_radian` | float | no | Per-joint encoder scale override. |

Physics feedback targets cannot conflict with coupling targets or another
physics feedback target. Couplings may still stack on the same target.

## Typical Use Cases

- loop an output back into an input with realistic delay
- inject sensor dropouts or spikes on a schedule
- derive encoder feedback from deterministic physics
- accelerate time for repeated commissioning scenarios
- make CI or tutorial demos reproducible

## Related

- [Simulation Workflow](../../operate/simulation.md)
- [Create A New Project](../../start/create-new-project.md)
