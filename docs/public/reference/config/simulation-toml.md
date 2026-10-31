# `simulation.toml`

`simulation.toml` lets you run deterministic dynamic plants, virtual wiring,
and timed fault injection without touching real hardware. Plant models are
deliberately imperfectible: actuator limits, model mismatch, noise, drift,
quantisation, dropout, and scenario events can be combined with a replay seed.

## Example

```toml
[simulation]
enabled = true
seed = 42
time_scale = 8
plant_step_ms = 100
max_plant_substeps_per_cycle = 10000
plant_trace_capacity = 1000

[[plants]]
id = "reference-fopdt"
model = "first_order_dead_time"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
initial_state = [0.0]

[plants.parameters]
gain = 1.0
time_constant_s = 1.0
dead_time_s = 0.5

[plants.actuator]
minimum = 0.0
maximum = 100.0
rate_up_per_s = 10.0
rate_down_per_s = 10.0

[plants.validity]
input_minimum = [0.0]
input_maximum = [100.0]
state_minimum = [-10.0]
state_maximum = [110.0]
output_minimum = [-10.0]
output_maximum = [110.0]
out_of_range = "fault"

[[plants.uncertainties]]
id = "sensor-noise"
target = "measurement"
channel = 0
mode = "add"
generator = "normal_approx_white"
standard_deviation = 0.01

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
| `seed` | integer or `"random"` | `0` | Replay seed. `"random"` resolves to an exposed concrete OS-derived integer. |
| `time_scale` | integer | `1` | Simulation time acceleration factor. |
| `plant_step_ms` | integer | `10` | Default fixed plant integration step. |
| `max_plant_substeps_per_cycle` | integer | `10000` | Catch-up bound before a visible simulation fault. |
| `plant_trace_capacity` | integer | `0` | Bounded completed-substep rows retained per plant; maximum `100000`. |

Simulation startup rejects enabled physical I/O drivers. The nonphysical
`simulated` (`sim`, `noop`) and `loopback` drivers remain available; disabled
drivers do not block startup. The first version has no mixed physical/simulated
output policy.

### `[[plants]]`

Each enabled plant reads numeric PLC outputs after a scan, advances only on its
fixed simulation-time step, and queues numeric PLC inputs for the next
pre-cycle boundary. Supported models are `static_gain`, `first_order`,
`first_order_dead_time`, `integrator`, `gravity_tank`, `piecewise_linear`,
`robot_joint`, `second_order`, and `state_space`. Optional
`[plants.validity]` input, state, and output bounds fault and roll back the
complete substep when a local model leaves its declared engineering region;
they are simulation evidence, not a physical safety interlock.

The complete field tables, model equations, actuator ordering, uncertainty
generators, random algorithm, plant events, validation rules, and numeric
conformance oracles are normative in
[Simulation plant models](https://github.com/johannesPettersson80/trust-platform/blob/main/docs/specs/46-simulation-plant-models.md).

For normal interactive authoring, use **truST: Open Devices & Connections**, add a
**Simulated plant** to a runtime, and edit the same fields in Plant Properties.
See the [simulation-first workflow](https://github.com/johannesPettersson80/trust-platform/blob/main/docs/guides/PLC_SIMULATION_WORKFLOW.md)
for the Tank example, model field summary, restart behavior, and safety limits.

### `[[couplings]]`

Couplings copy or transform one I/O point into another after a delay.

| Key | Type | Required | Notes |
| --- | --- | --- | --- |
| `source` | IEC address | yes | Source I/O address. |
| `target` | IEC address | yes | Target I/O address. |
| `threshold` | float | no | Optional finite decision threshold; NaN and infinities are rejected. |
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
- [Simulation plant-model specification](https://github.com/johannesPettersson80/trust-platform/blob/main/docs/specs/46-simulation-plant-models.md)
- [Create A New Project](../../start/create-new-project.md)
