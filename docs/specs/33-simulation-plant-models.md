# Simulation plant models

This specification defines the truST fake-plant system: deterministic dynamic
process models connected to a running PLC through the normal process image.
The system is intended for controller development, regression testing,
commissioning rehearsal, and fault-response exercises before physical hardware
is available or safe to operate.

The term *plant* means the process controlled by the PLC. A simulated plant is
not claimed to be a perfect digital twin. Its purpose is to expose controller
behavior against declared dynamics, limits, delay, uncertainty, noise, and
faults. A successful simulation does not replace validation on the real plant.

This is a truST product contract. IEC 61131-3 does not define plant simulation,
so this specification creates neither an IEC requirement nor an IEC deviation.
It extends the simulation behavior owned by `simulation.toml` and Section 6.9.4
of `11-runtime-engine.md`.

Normative words `shall`, `shall not`, `should`, and `may` have their usual
requirements meanings. Examples are explanatory unless a conformance oracle
explicitly assigns them expected results.

## 1. Purpose and scope

The fake-plant system shall provide:

- closed-loop execution of real PLC code against simulated process dynamics;
- repeatable nominal and adverse scenarios;
- deliberately imperfect actuators, processes, and sensors;
- deterministic pseudo-random uncertainty when an effective seed is known;
- an explicitly nonrepeatable launch option that reports the chosen seed;
- fixed-step model execution independent of PLC scan jitter;
- multiple independent or connected plant instances;
- observable model state and simulation health; and
- native executable conformance tests at the runtime/process-image boundary.

The first implementation shall support the built-in models and effects defined
by this specification. It shall not execute arbitrary native code supplied by a
project and shall not load an unversioned model plug-in.

The system does not claim:

- physical fidelity beyond the configured equations;
- real hardware timing, safety, reliability, or certification;
- automatic identification of an unknown process;
- automatic controller tuning;
- finite-element, computational-fluid-dynamics, or multiphysics analysis;
- a substitute for hardware-in-the-loop or site acceptance testing; or
- stability of a controller merely because a simulation completed.

## 2. Relationship to existing simulation behavior

Plant models are an additional simulation producer alongside existing
couplings, scripted disturbances, and physics joints. Existing behavior remains
unchanged unless a project declares at least one `[[plants]]` entry.

`simulation.toml` continues to own all simulation configuration. A nonempty
plant list infers simulation activation when `[simulation].enabled` is omitted.
An explicit `enabled = false` disables plants together with couplings,
disturbances, and physics.

Plant feedback uses the same scan-boundary rule as existing simulation effects:

1. due plant feedback is written to the input image before a PLC cycle;
2. the PLC cycle reads those inputs and commits its outputs;
3. plant models read committed PLC outputs after the cycle;
4. the models advance through elapsed simulation time; and
5. newly calculated feedback is queued for a later pre-cycle boundary.

A plant shall never mutate PLC input storage during post-cycle execution.
Zero model latency therefore still produces feedback no earlier than the next
pre-cycle boundary.

## 3. Public configuration surface

### 3.1 Simulation settings

The existing `[simulation]` section gains these optional fields:

| Field | Type | Default | Purpose |
|---|---|---:|---|
| `seed` | unsigned 64-bit integer or `"random"` | `0` | Selects the master stochastic seed. |
| `plant_step_ms` | positive integer | `10` | Default fixed integration step for plants. |
| `max_plant_substeps_per_cycle` | positive integer | `10000` | Bounds catch-up work after a delayed PLC cycle. |
| `plant_trace_capacity` | nonnegative integer | `0` | Retains this many completed substep rows per plant; zero disables collection. |

An integer seed selects a reproducible run. `seed = "random"` selects a fresh
effective unsigned 64-bit seed at activation. Before the first PLC cycle, the
runtime shall expose and log that effective seed. Trace export shall store the
effective integer, never the word `random`, so the run can be replayed.
The fresh seed shall come from the host operating system's randomness source.
If that source is unavailable, activation shall fail rather than substituting a
timestamp, process identifier, or fixed seed.

`plant_step_ms` and `max_plant_substeps_per_cycle` shall be greater than zero
and fit their runtime duration and count representations. Invalid values reject
the complete simulation configuration before activation.
`plant_trace_capacity` shall not exceed `100000`. The bound is per enabled
plant and its storage shall be allocated when the controller is created.

### 3.2 Plant instance

Each `[[plants]]` entry declares one state-owning model. A plant is used to turn
one or more PLC output commands into one or more PLC input measurements.

Every plant defines:

| Field | Type | Required | Purpose |
|---|---|---:|---|
| `id` | string | yes | Stable, case-sensitive identity within the project. |
| `name` | string | no | Human-readable display name; defaults to `id` when omitted. |
| `description` | string | no | Optional project documentation for the simulated process. |
| `model` | closed string enum | yes | Selects a built-in model from Section 7. |
| `command_sources` | array of addresses | yes | Ordered numeric PLC output signals. |
| `feedback_targets` | array of addresses | yes | Ordered numeric PLC input signals. |
| `step_ms` | positive integer | no | Overrides the default fixed integration step. |
| `initial_state` | array of finite numbers | model-dependent | Sets dynamic state at activation. |
| `initial_input` | array of finite numbers | no | Supplies engineering-unit input before the first committed PLC command. |
| `input_scale` | array of finite numbers | no | Scales commands into engineering units. |
| `input_offset` | array of finite numbers | no | Offsets commands into engineering units. |
| `output_scale` | array of finite numbers | no | Scales model outputs toward PLC representation. |
| `output_offset` | array of finite numbers | no | Offsets model outputs toward PLC representation. |
| `enabled` | Boolean | no | Enables this instance; default is `true`. |

The scaling equations are, in declared channel order:

```text
model_input[i] = input_scale[i] * plc_command[i] + input_offset[i]
plc_feedback[j] = output_scale[j] * measured_output[j] + output_offset[j]
```

Missing scale arrays mean all factors are one. Missing offset arrays mean all
offsets are zero. A supplied array length shall equal its associated channel
count. A missing `initial_input` means a zero vector; a supplied vector length
shall equal the command count. Plant identifiers shall match
`[A-Za-z][A-Za-z0-9_-]*` and be unique.

When present, `name` shall contain non-whitespace text. `description` may be
empty and has no execution semantics. Neither field participates in plant
identity, execution order, deterministic random streams, or I/O ownership.

A disabled plant owns no input target, reads no command, advances no state,
consumes no random samples, and publishes no feedback. Enabling or disabling a
plant while the runtime is active is outside the first version of this
contract.

### 3.3 Model parameters

Each plant has one `[plants.parameters]` table. The permitted fields depend on
the selected model. Unknown fields and fields belonging only to another model
shall reject configuration rather than being ignored.

Every numeric parameter shall be finite. Positivity, dimensions, and ordering
requirements are specified with each model in Section 7.

### 3.4 Actuator

An optional `[plants.actuator]` table describes the nonideal device between the
PLC command and the process. It is used to model valves, drives, heaters, and
other final control elements without changing the controller program.

| Field | Type | Default | Purpose |
|---|---|---:|---|
| `minimum` | finite number | unbounded | Lower applied-command limit. |
| `maximum` | finite number | unbounded | Upper applied-command limit. |
| `rate_down_per_s` | nonnegative number | unbounded | Maximum falling slew rate. |
| `rate_up_per_s` | nonnegative number | unbounded | Maximum rising slew rate. |
| `deadband` | nonnegative number | `0` | Command movement ignored around the last accepted command. |
| `lag_s` | nonnegative number | `0` | First-order actuator lag time constant. |
| `initial_applied` | finite number | plant `initial_input[0]` | Initial actuator output. |

The first version supports this actuator only for scalar-input models. Minimum
shall not exceed maximum. A zero rate means no movement in that direction; an
omitted rate is unbounded. A zero lag means no lag.
Supplying an actuator table for a model with more than one command source shall
reject configuration.

### 3.5 Uncertainty effect

Each `[[plants.uncertainties]]` entry declares one deliberate deviation from the
nominal model. An uncertainty effect is used to make tests realistic without
making them irreproducible or unobservable.

Every uncertainty defines:

| Field | Type | Required | Purpose |
|---|---|---:|---|
| `id` | string | yes | Stable identity within the plant. |
| `target` | closed string enum | yes | Location changed by the effect. |
| `mode` | closed string enum | yes | How the generated value is applied. |
| `generator` | closed string enum | yes | Value-generation rule from Section 10. |
| `channel` | nonnegative integer | where applicable | Zero-based input, state, output, or measurement channel. |
| `parameter` | string | for parameter targets | Named model parameter to vary. |
| `start_s` | nonnegative number | no | First active simulation time; default `0`. |
| `end_s` | nonnegative number | no | First inactive time; default is no end. |

Uncertainty IDs shall match `[A-Za-z][A-Za-z0-9_-]*` and be unique within their
plant. `end_s` shall not precede `start_s`. An inactive uncertainty produces no
value and consumes no random sample.

`channel` is required for `command`, `process_input`, `state`,
`process_output`, and `measurement`, even when the selected vector has only one
element. It shall be in range. `parameter` is required only for a parameter
target; `channel` shall then be absent.

The supported targets are:

| Target | Meaning |
|---|---|
| `command` | Transformed PLC command before actuator imperfections. |
| `process_input` | Applied actuator signal before the process equation. |
| `state` | Selected dynamic state after nominal integration. |
| `process_output` | Selected ideal model output. |
| `measurement` | Selected sensor value before PLC output scaling. |
| `parameter` | Selected model parameter for the current substep. |

The supported modes are:

| Mode | Result for nominal value `v` and generated value `g` |
|---|---|
| `add` | `v + g` |
| `multiply` | `v * g` |
| `replace` | `g` |
| `hold` | Last value accepted before the effect became active. |

`hold` does not use a generated numeric value and is valid only for
`process_input`, `process_output`, or `measurement`. Effects at the same target
and channel execute in declaration order. Each effect receives the result of
the preceding effect.

### 3.6 Scheduled plant event

Each `[[plant_events]]` entry performs an explicit scenario action at a stated
simulation time. Plant events complement stochastic uncertainty and existing
I/O disturbances.

| Field | Type | Required | Purpose |
|---|---|---:|---|
| `at_ms` | nonnegative integer | yes | Due simulation time. |
| `plant` | string | yes | Target plant identity. |
| `kind` | closed string enum | yes | Event action. |
| `channel` | nonnegative integer | kind-dependent | Target channel or state. |
| `parameter` | string | kind-dependent | Target parameter. |
| `value` | finite number | kind-dependent | New value or impulse magnitude. |

The supported kinds are:

- `set_state`;
- `add_state`;
- `set_parameter`;
- `set_sensor_bias`;
- `stick_actuator`;
- `release_actuator`; and
- `reset_plant`.

`set_state` and `add_state` require an in-range state `channel` and `value`.
`set_parameter` requires `parameter` and `value`. `set_sensor_bias` requires an
in-range output `channel` and `value`. `stick_actuator`, `release_actuator`, and
`reset_plant` accept none of those three fields. An unknown field or a field not
owned by the selected kind rejects configuration. An event targeting a disabled
plant also rejects configuration.

Events are stably sorted by `(at_ms, declaration_order)`. All events due at an
internal step boundary execute in that order before uncertainty generation and
nominal integration for that step. `reset_plant` restores configured initial
state, actuator state, sensor state, delay buffers, uncertainty generator state,
and random-stream state for that plant. It does not rewind global simulation
time, analytic generator phase, active-time windows, processed events, or
another plant.

## 4. Complete configuration example

This example connects the paper's first-order-plus-dead-time process to an
`LREAL` PLC output and input, adds actuator limits, plant-gain mismatch,
measurement noise, slow sensor-bias drift, quantisation, and a scheduled load
disturbance.

```toml
[simulation]
enabled = true
seed = 42
time_scale = 8
plant_step_ms = 10
max_plant_substeps_per_cycle = 10000

[[plants]]
id = "paper-fopdt"
model = "first_order_dead_time"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
step_ms = 10
initial_state = [0.0]

[plants.parameters]
gain = 1.0
time_constant_s = 1.0
dead_time_s = 0.5
input_bias = 0.0
output_bias = 0.0

[plants.actuator]
minimum = 0.0
maximum = 100.0
rate_down_per_s = 10.0
rate_up_per_s = 10.0
deadband = 0.001
lag_s = 0.05
initial_applied = 0.0

[[plants.uncertainties]]
id = "gain-mismatch"
target = "parameter"
parameter = "gain"
mode = "multiply"
generator = "constant"
value = 1.10

[[plants.uncertainties]]
id = "sensor-white-noise"
target = "measurement"
channel = 0
mode = "add"
generator = "normal_approx_white"
standard_deviation = 0.01

[[plants.uncertainties]]
id = "sensor-bias-drift"
target = "measurement"
channel = 0
mode = "add"
generator = "random_walk"
diffusion_per_sqrt_s = 0.001
minimum = -0.05
maximum = 0.05

[[plants.uncertainties]]
id = "sensor-quantisation"
target = "measurement"
channel = 0
mode = "replace"
generator = "quantise"
quantum = 0.001

[[plant_events]]
at_ms = 10000
plant = "paper-fopdt"
kind = "add_state"
channel = 0
value = -0.2
```

## 5. Activation and state lifecycle

### 5.1 Validation before activation

The runtime shall parse and validate the complete simulation configuration
before creating any plant or changing any runtime I/O. Failure in one plant,
uncertainty, event, address, or cross-reference rejects the complete model.
Partial activation is forbidden.

Validation shall resolve:

- unique identities and references;
- model-specific dimensions;
- numeric finiteness and ranges;
- source and target process-image metadata;
- state, matrix, scale, and offset lengths;
- actuator and uncertainty compatibility;
- event targets and parameter names;
- feedback-target ownership conflicts; and
- internal-step duration and catch-up bounds.

### 5.2 Initial state

Each model defines the required state-vector length. When `initial_state` is
omitted, every dynamic state is zero except where a model explicitly defines a
different default. Initial actuator, delay, sensor, and uncertainty states are
derived from their configuration.

At activation, the plant shall calculate an initial ideal output and initial
measurement from configured state and `initial_input`. It shall queue that
initial feedback for the first pre-cycle boundary without first reading an
uncommitted PLC output.
Deterministic measurement effects such as constant bias, clipping, and
quantisation apply to this initial measurement. Stochastic generators use their
declared initial state and consume no sample before the first completed plant
substep. A dropout is initially inactive.

### 5.3 Restart

A cold runtime start and a cold restart recreate every plant from configuration
and reset all plant-local random streams. Given the same effective seed and
input history, both produce the same trace.

A warm PLC restart also resets simulated plants in the first version. Plant
state is not retained through the PLC retain store. Runtime status shall expose
that policy so a warm restart cannot be mistaken for continuity of a physical
process.

Hot reload of plant configuration while a resource is running is outside the
first version. A changed plant configuration requires a controlled runtime
restart.

## 6. Time and scan semantics

### 6.1 Simulation time

Plant time is the scheduler's monotonic simulation time after applying the
configured time scale. Wall-clock time and CPU execution duration do not enter
plant equations.

Time scaling changes how quickly simulation time advances relative to wall
time; it shall not change model results at equal simulation timestamps.

### 6.2 Fixed internal step

Every plant advances on its configured positive fixed step `h`. PLC scan time
may be shorter, equal to, or longer than `h`.

Each plant owns an elapsed-time accumulator. After a PLC cycle, elapsed
simulation time is added to the accumulator. While the accumulator is at least
`h`, the plant executes one complete substep and subtracts `h`. A remainder
smaller than `h` is retained for the next PLC cycle. The plant output is held
between completed substeps.

PLC commands use zero-order hold. All substeps triggered after one PLC cycle use
the command committed by that cycle unless an internal delayed-command boundary
selects an older held command.

This rule makes results independent of how the same elapsed simulation interval
is partitioned among PLC scans, provided the command history at plant step
boundaries is identical.

### 6.3 Catch-up limit

The number of plant substeps executed after one PLC cycle shall not exceed
`max_plant_substeps_per_cycle`. Requiring more substeps is a simulation fault.
The runtime shall not silently discard elapsed time, enlarge the fixed step, or
publish a partially advanced plant.

### 6.4 Substep order

For each plant substep at time `t`, observable processing order is:

1. apply plant events due at `t`;
2. sample the held PLC commands;
3. apply command uncertainty effects;
4. apply actuator deadband, limits, slew limits, lag, and stuck state;
5. apply process-input uncertainty effects;
6. evaluate parameter uncertainty for this substep;
7. advance the nominal dynamic model by exactly `h`;
8. apply state uncertainty effects;
9. calculate the nominal output;
10. apply process-output uncertainty effects;
11. apply measurement uncertainty and sensor effects;
12. scale the measurement toward the PLC representation; and
13. replace the plant's queued feedback with the completed substep result.

No partial result from a failed substep may become visible.

`set_state` and `add_state` change the selected state before integration.
`set_parameter` establishes a persistent plant-local parameter override to
which later uncertainty effects apply. `set_sensor_bias` establishes a
persistent additive bias for the selected measurement channel before declared
measurement uncertainties. `stick_actuator` captures the current applied value;
`release_actuator` resumes normal actuator processing from that value. Processed
events are not replayed by `reset_plant`.

## 7. Built-in nominal plant models

All built-in dynamics use `LREAL`-equivalent binary64 arithmetic internally.
Inputs, states, parameters, and outputs shall remain finite. `h` is the fixed
step in seconds.

### 7.1 `static_gain`

`static_gain` represents an instantaneous proportional process or scaling
stage. It has one input, one output, and no dynamic state.

Parameters are finite `gain`, `input_bias`, and `output_bias`, with
zero defaults for both biases and a default gain of one.

```text
y = output_bias + gain * (u - input_bias)
```

`initial_state` shall be absent or empty.

### 7.2 `first_order`

`first_order` represents a stable scalar first-order process. It has one input,
one output, and one state equal to ideal process output.

Parameters are:

| Parameter | Requirement | Default |
|---|---|---:|
| `gain` | finite | `1` |
| `time_constant_s` | finite and greater than zero | required |
| `input_bias` | finite | `0` |
| `output_bias` | finite | `0` |

For a held input over one substep, the exact zero-order-hold update is:

```text
target = output_bias + gain * (u - input_bias)
a = exp(-h / time_constant_s)
x_next = target + (x - target) * a
y = x_next
```

### 7.3 `first_order_dead_time`

`first_order_dead_time` adds command transport delay to the `first_order`
model. It has one input, one output, and one process state.

It accepts all `first_order` parameters plus nonnegative finite
`dead_time_s`. Delayed input uses the most recent command sample whose timestamp
is less than or equal to `t - dead_time_s`. Before such a sample exists, the
configured `initial_delayed_input`, defaulting to `input_bias`, is used.

Delay is applied to the process input after actuator behavior and process-input
uncertainty. It is represented by timestamped held samples and shall not be rounded to an
integer number of plant steps. Samples no longer needed to interpolate the
zero-order-held delayed value may be discarded.

The delayed command replaces `u` in the `first_order` equation. When a delayed
command transition falls strictly inside one fixed plant substep, that substep
is divided at the transition timestamp and the exact first-order update is
applied to each segment in chronological order. This preserves fractional dead
time without changing the configured outer plant step.

### 7.4 `integrator`

`integrator` represents inventory, accumulated position, or another scalar
integrating process. It has one input, one output, and one state.

Parameters are finite `gain`, `input_bias`, and `output_bias`, with defaults
one, zero, and zero respectively.

```text
x_next = x + h * gain * (u - input_bias)
y = output_bias + x_next
```

Optional finite `state_minimum` and `state_maximum` clamp the completed state;
the minimum shall not exceed the maximum. Omitted bounds are unbounded.

### 7.5 `second_order`

`second_order` represents an underdamped, critically damped, or overdamped
scalar process. It has one input, one output, and two states: output displacement
`x1` and output velocity `x2`.

Parameters are:

| Parameter | Requirement | Default |
|---|---|---:|
| `gain` | finite | `1` |
| `natural_frequency_rad_s` | finite and greater than zero | required |
| `damping_ratio` | finite and nonnegative | required |
| `input_bias` | finite | `0` |
| `output_bias` | finite | `0` |

The continuous equation is:

```text
dx1/dt = x2
dx2/dt = gain * wn^2 * (u - input_bias)
         - 2 * damping_ratio * wn * x2
         - wn^2 * x1
y = output_bias + x1
```

The state advances by classical fourth-order Runge-Kutta using held input and
parameters. The four derivative evaluations and weighted sum shall follow the
standard RK4 order without adaptive step changes.

### 7.6 `state_space`

`state_space` represents a finite-dimensional continuous linear time-invariant
model and is the general model for multi-input, multi-output linear plants.

Configuration supplies matrices `a`, `b`, `c`, and `d` as row-major nested
arrays and an `initial_state` vector. If there are `n` states, `m` inputs, and
`p` outputs, dimensions shall be:

```text
A: n by n
B: n by m
C: p by n
D: p by m
initial_state: n
command_sources: m
feedback_targets: p
```

All matrix entries shall be finite, and `n`, `m`, and `p` shall be greater than
zero. The equations are:

```text
dx/dt = A*x + B*u
y = C*x + D*u
```

State advances by classical fourth-order Runge-Kutta using held input and
parameters. Output is evaluated from the completed state and current held
input. Matrix-vector products iterate rows and columns in increasing index
order so arithmetic ordering is stable.

### 7.7 `piecewise_linear`

`piecewise_linear` represents a memoryless nonlinear characteristic such as a
valve curve or static sensor map. It has one input, one output, and no state.

Configuration supplies equal-length arrays `input_points` and `output_points`
with at least two finite entries. Input points shall be strictly increasing.
Inside the table, output uses linear interpolation between adjacent points.
Below or above the table, output holds the nearest endpoint; extrapolation is
not performed.

`initial_state` shall be absent or empty.

## 8. Actuator behavior

The actuator operates once per plant substep. It owns the last accepted command
and last applied value.

For scalar command `u`, the order is:

1. If stuck, retain the applied value captured by `stick_actuator`.
2. Otherwise, compare `u` with the last accepted command. If the absolute
   difference is less than or equal to `deadband`, retain the last accepted
   command; otherwise accept `u`.
3. Clamp the accepted command to `[minimum, maximum]` when bounds exist.
4. Limit movement from the previous applied value to
   `[-rate_down_per_s*h, rate_up_per_s*h]` when rates exist.
5. If `lag_s` is zero, use the slew-limited value. Otherwise apply the exact
   first-order update with coefficient `exp(-h/lag_s)`.

The actuator's applied output, saturation state, slew-limited state, deadband
state, and stuck state shall be observable in simulation status.

## 9. Sensor behavior

Measurement uncertainty effects execute in declaration order. The following
generators provide common sensor imperfections without requiring a perfect
physical sensor model:

- additive white or colored noise;
- calibration bias;
- slow random drift;
- quantisation;
- clipping;
- intermittent dropout;
- frozen/held measurement; and
- a scheduled replacement or spike.

Quantisation rounds to the nearest integer multiple of positive finite
`quantum`; exact half values round to the nearest even multiple. Clipping clamps
to configured finite bounds. A dropout shall declare one of these policies:

| Policy | Published measurement |
|---|---|
| `hold_last` | Last successfully published measurement. |
| `replace` | Configured finite replacement value. |
| `fault` | No value; the runtime enters simulation fault. |

Dropout state and the last valid measurement are plant-local. A dropout shall
never manufacture a zero unless zero is the configured replacement.

## 10. Uncertainty generators

### 10.1 General rules

Generators execute once per completed plant substep while active. Configuration
shall use engineering units after input scaling and before output scaling.
Generated values and completed results shall be finite.
`start_s`, `end_s`, and analytic generator time `t` use absolute monotonic
simulation time; a plant reset does not restart those clocks.

The first version supports these generators:

| Generator | Required parameters | Generated value |
|---|---|---|
| `constant` | `value` | Constant `value`. |
| `pulse` | `value`, positive `duration_s` | Constant for its duration, otherwise inactive. |
| `sine` | `amplitude`, nonnegative `frequency_hz`, `phase_rad`, optional `bias` | `bias + amplitude*sin(2*pi*frequency_hz*t + phase_rad)`. |
| `uniform_white` | `minimum`, `maximum` | Independent uniform sample in `[minimum, maximum)`. |
| `normal_approx_white` | nonnegative `standard_deviation`, optional `mean` | Approximate-normal sample with requested mean and deviation. |
| `colored_noise` | nonnegative `standard_deviation`, positive `correlation_time_s` | First-order correlated approximate-normal noise. |
| `random_walk` | nonnegative `diffusion_per_sqrt_s`, optional bounds | Accumulated approximate-normal drift. |
| `quantise` | positive `quantum` | Quantised incoming value; valid only with `replace`. |
| `clip` | `minimum`, `maximum` | Clipped incoming value; valid only with `replace`. |
| `dropout` | nonnegative `rate_per_s`, duration fields, `dropout_policy`, and policy-dependent `replacement_value` | Stochastic sensor dropout. |

For `quantise` and `clip`, the generator consumes the incoming value instead of
producing an independent value. For all other generators, `mode` combines the
generated value with the incoming value.

`dropout` is valid only for a `measurement` target and requires mode `replace`;
its policy determines whether replacement, holding, or faulting occurs.
`dropout_policy` is one of `hold_last`, `replace`, or `fault`. Policy `replace`
requires finite `replacement_value`; the other policies forbid it. A fixed
duration uses positive `duration_s`. A ranged duration omits `duration_s` and
uses positive `duration_min_s` and `duration_max_s`.

### 10.2 White noise

`uniform_white` maps one uniform random sample to the configured interval.
Minimum shall be strictly less than maximum.

`normal_approx_white` uses the deterministic approximate-normal value `z` from
Section 11.4:

```text
g = mean + standard_deviation * z
```

A zero standard deviation is valid and produces the mean without consuming a
random value.

### 10.3 Colored noise

Colored noise owns one state `q`, initially zero. With
`tau = correlation_time_s`:

```text
a = exp(-h / tau)
q_next = a*q + standard_deviation*sqrt(1 - a*a)*z
g = q_next
```

This gives an explicitly band-limited stochastic disturbance rather than
assuming every real sensor has independent sample-to-sample noise.

### 10.4 Random walk

Random walk owns one state `q`, initially zero:

```text
q_next = q + diffusion_per_sqrt_s * sqrt(h) * z
```

Optional minimum and maximum bounds clamp `q_next`. Random walk is suitable for
slow calibration drift and model-parameter drift; it is not stationary noise.

### 10.5 Dropout

At each substep outside an active dropout, a new dropout begins when:

```text
uniform_0_1 < 1 - exp(-rate_per_s * h)
```

Duration is either fixed positive `duration_s` or uniformly selected between
positive `duration_min_s` and `duration_max_s`. Minimum shall not exceed maximum.
While dropout is active, no new start sample is consumed.

For a ranged duration and uniform value `u`, duration is
`duration_min_s + (duration_max_s - duration_min_s) * u`.

This rate-based rule keeps dropout probability approximately independent of the
chosen plant step.

## 11. Randomness and replay

### 11.1 Reproducibility contract

Given identical validated configuration, effective seed, initial state, event
history, command history at plant-step boundaries, and runtime version, plant
state and published feedback shall be identical.

Randomness shall never depend on wall-clock timing, thread scheduling, hash-map
iteration, memory address, or another plant's execution order.

### 11.2 Independent streams

Every stochastic uncertainty owns an independent random stream derived from:

```text
effective master seed
plant id
uncertainty id
random algorithm version
```

Adding, removing, disabling, or reordering an unrelated plant or uncertainty
shall not change the samples produced by an existing unchanged uncertainty.

### 11.3 `splitmix64-v1`

The first random algorithm is named `splitmix64-v1`. All operations below use
wrapping unsigned 64-bit arithmetic:

```text
state = state + 0x9E3779B97F4A7C15
z = state
z = (z XOR (z >> 30)) * 0xBF58476D1CE4E5B9
z = (z XOR (z >> 27)) * 0x94D049BB133111EB
sample_u64 = z XOR (z >> 31)
```

Plant and uncertainty identity bytes are incorporated through a separately
versioned stable seed-derivation function named `fnv1a64-v1`. Its input bytes
are UTF-8 plant ID, one zero byte, UTF-8 uncertainty ID, one zero byte, and the
ASCII bytes `splitmix64-v1`. Starting from FNV offset basis
`0xCBF29CE484222325`, each byte updates the hash by XOR followed by wrapping
multiplication by `0x00000100000001B3`. The initial SplitMix64 state is the hash
XOR the effective master seed. The random algorithm and seed-derivation version
shall be exposed in status and trace metadata. Changing either is a simulation
compatibility change.

### 11.4 Uniform and approximate-normal values

A uniform binary64 value in `[0,1)` is:

```text
uniform_0_1 = (sample_u64 >> 11) * 2^-53
```

The deterministic approximate-normal value is the ordered sum of twelve
independent uniform values minus six:

```text
z = (u1 + u2 + ... + u12) - 6
```

This bounded approximation is intentional. It avoids claiming exact Gaussian
tails and avoids platform-dependent transcendental transformations. The runtime
shall describe it as `normal_approx`, not an exact normal distribution.

## 12. Parameter uncertainty and model mismatch

A realistic controller test should not use only the exact nominal parameters
used for tuning. The system therefore supports declared parameter mismatch and
drift.

Parameter effects apply to a temporary parameter value for the current substep;
they do not rewrite configuration. Multiple effects use declaration order.
After applying effects, the parameter shall still satisfy its model-specific
domain. A time constant made zero or negative, a negative damping ratio, or any
nonfinite effective parameter is a simulation fault.

Scalar parameter names use their configuration spelling. State-space matrix
elements use `a[row,column]`, `b[row,column]`, `c[row,column]`, or
`d[row,column]`, with zero-based decimal indices and no whitespace. A parameter
effect or event shall name exactly one existing scalar or matrix element.

Recommended verification scenarios include, without making their values
mandatory for every project:

- process gain above and below nominal;
- time constant above and below nominal;
- more dead time than assumed by the controller;
- actuator gain, deadband, lag, and saturation;
- measurement bias, noise, quantisation, and dropout;
- load disturbances and state impulses;
- bounded sampling jitter; and
- simultaneous but independently seeded imperfections.

Each project test shall state its chosen uncertainty envelope and acceptance
metrics. The simulator shall not infer that envelope or declare a controller
stable.

## 13. Process-image binding and conversion

### 13.1 Command sources

Each command source shall be a concrete `%Q` address backed by compiled scalar
numeric process-image metadata. `BOOL`, character, string, aggregate, reference,
and untyped byte-sequence commands are invalid. Integer, bit-string, `REAL`, and
`LREAL` values are converted to binary64 exactly where representable and by the
runtime's checked numeric-conversion rules otherwise.

### 13.2 Feedback targets

Each feedback target shall be a concrete `%I` address backed by compiled scalar
numeric process-image metadata. The completed scaled feedback is converted to
the declared target type using the runtime's checked conversion rules.

Conversion shall reject nonfinite values and numeric overflow. Integer targets
round to nearest with ties to even before range checking. No conversion may
wrap, truncate modulo the target width, or reinterpret floating-point bits as
an integer.

For controller verification, `LREAL` command and feedback bindings should be
used to avoid unrelated quantisation unless quantisation is part of the stated
scenario.

### 13.3 Target ownership

An enabled plant feedback target shall be unique across:

- all enabled plants;
- existing simulation couplings; and
- existing physics feedback targets.

A conflict rejects configuration before activation. Multiple plants may read
the same command source. A command source may also be observed by an existing
coupling.

## 14. Multiple plants and plant networks

Plant instances execute in ascending bytewise order of plant ID. Execution
order shall not change random streams.

The first version connects plants through PLC process-image signals. Direct
plant-to-plant links are not part of this contract. A project may connect a
plant output to another plant by routing it through the PLC program, making the
dependency and scan delay explicit.

Each plant owns its state, accumulator, actuator, delay buffers, sensor state,
events, uncertainty generators, and random streams. No mutable state is shared
between plant instances.

## 15. Fault and safety behavior

The fake-plant system is development and verification infrastructure. It shall
be visibly identified as simulation in CLI, TUI, web, status, and exported
trace surfaces already governed by the runtime simulation contract.

Simulation mode shall not silently activate a physical output driver. If a
project configuration would write both simulated and physical outputs without
an explicitly reviewed mixed-mode policy, activation shall fail closed.
The first version defines no mixed-mode policy. While simulation is enabled,
the runtime shall therefore reject every enabled I/O driver except the
nonphysical `simulated` (`sim`, `noop`) and `loopback` drivers before any driver
is constructed or the PLC is started. Disabled drivers do not participate in
this decision.

These conditions are simulation faults:

- a required process-image source cannot be read;
- feedback cannot be converted or written;
- a calculation produces NaN or infinity;
- an effective parameter leaves its valid domain;
- a model exceeds the configured catch-up limit;
- an event references unavailable state or a parameter;
- a stochastic generator produces an invalid state; or
- an internal model dimension changes after activation.

A simulation fault shall identify plant ID, model kind, simulation timestamp,
substep, and failing operation. The runtime shall retain the last completely
published feedback and shall not publish partial values from the failed step.
It shall not substitute zero, clamp an otherwise invalid value, skip the step,
or continue with a reset plant unless the specification explicitly defines that
operation.

## 16. Observability and trace export

`SimulationController::status()` shall return a `SimulationStatus` snapshot.
`SimulationController::trace_snapshot()` shall return a
`SimulationTraceSnapshot`, and `SimulationController::clear_trace()` shall
discard retained rows and reset the dropped-row count without changing plant or
random-generator state. These calls are observations; calling or not calling
them shall not change subsequent model results.

Runtime simulation status shall expose at least:

- simulation active state and time scale;
- effective seed and random algorithm version;
- each plant ID, model, enabled state, step, and simulated timestamp;
- state vector and latest ideal output;
- transformed command and applied actuator output;
- latest measured and published feedback;
- actuator saturation, slew, deadband, and stuck indicators;
- active uncertainty and dropout identifiers;
- pending plant events and delay-buffer depth;
- accumulated remainder time and last substep count; and
- last plant fault, if any.

Trace export shall use simulation timestamps and preserve enough data to
reproduce and diagnose a run:

- validated configuration digest;
- runtime version;
- effective seed and algorithm versions;
- plant commands at substep boundaries;
- fired events;
- generated uncertainty samples or the information required to regenerate them;
- state, ideal output, measurement, and published feedback; and
- faults and limit-state transitions.

Trace collection shall be optional and bounded. Disabling trace collection
shall not change model calculations or random consumption.
Each completed substep contributes one `SimulationTraceRow` when
`plant_trace_capacity` is nonzero. Once a plant's capacity is full, the oldest
row is replaced and the trace snapshot's `dropped_rows` count increases. A
snapshot orders retained rows by simulation timestamp and then bytewise plant
ID. It exposes the configured per-plant capacity and the total dropped count.

## 17. Runtime and performance requirements

After successful activation, advancing built-in plants shall not allocate heap
memory per substep. Delay buffers and trace buffers may use bounded storage
allocated or reserved during activation.

Configuration shall declare or permit calculation of finite bounds for delay
history, pending events, stochastic state, and trace storage. Exceeding a bound
shall produce a visible simulation fault or a documented bounded trace-drop
indicator; it shall not grow memory without limit.

Plant execution duration is not physical time. Time-scale acceleration may be
limited by available CPU. Failure to keep pace with requested wall-clock
acceleration shall be reported as simulation lag, while model time and fixed
steps remain correct.

Performance benchmarks shall report plant count, model mix, step, PLC cycle,
substeps, trace mode, runtime version, CPU, and effective seed. Builder
benchmarks detect regressions but do not prove deployment-target timing.

## 18. Conformance oracles

Conformance requires native executable tests against public configuration,
runtime scheduling, process-image binding, and status behavior. Testing a model
helper alone is insufficient for a complete plant-system claim.

Numeric equality uses absolute tolerance
`1e-12 * MAX(1, ABS(Expected))` for binary64 model values unless an oracle states
an exact integer or bit result.

### 18.1 First-order exact update

Configure `first_order` with gain `2`, time constant `1`, zero biases, initial
state `0`, step `0.1`, and held command `3`. After one plant substep, state and
ideal output shall equal:

```text
6 * (1 - exp(-0.1))
```

After ten substeps they shall equal:

```text
6 * (1 - exp(-1.0))
```

The second assertion shall be unchanged when elapsed time arrives as ten equal
PLC cycles or one catch-up cycle.

### 18.2 Fractional dead time

Configure `first_order_dead_time` with gain and time constant `1`, dead time
`0.15`, step `0.1`, zero initial state and delayed input, and command changing
from `0` to `1` at time zero. Feedback at `0.1` shall remain zero. The substep
from `0.1` through `0.2` shall use zero for its first `0.05` seconds and one for
its final `0.05` seconds, producing feedback at `0.2` equal to
`1 - exp(-0.05)`. The test shall prove the delay was not rounded to `0.1` or
`0.2` seconds.

### 18.3 Integrator

Configure `integrator` with gain `2`, input bias `1`, output bias `3`, initial
state `4`, step `0.25`, and held command `5`. One substep shall publish `9`:

```text
x_next = 4 + 0.25 * 2 * (5 - 1) = 6
y = 3 + 6 = 9
```

### 18.4 Second-order RK4

Configure a second-order plant with gain `1`, natural frequency `2`, damping
ratio `0.5`, state `[0, 0]`, command `1`, and step `0.1`. A native test shall
compare the completed state and output with an independent direct evaluation of
the four specified RK4 stages, not with values generated by calling the model
implementation twice.

### 18.5 State-space dimensions and execution

A two-state, one-input, one-output state-space model shall reproduce the same
state trace as the equivalent `second_order` model. Malformed A, B, C, or D
dimensions shall reject the entire configuration before I/O mutation.

### 18.6 Scan boundary

A PLC program shall write a command during cycle `n`. The plant shall observe it
only after that cycle, and feedback calculated from it shall become readable by
the PLC no earlier than pre-cycle `n+1`. The native test shall assert the values
on both sides of each boundary.

### 18.7 Actuator ordering

A scalar actuator test shall simultaneously exercise deadband, saturation,
asymmetric slew, and lag. Intermediate status values shall prove the ordering
in Section 8 rather than checking only the final measurement.

### 18.8 Seeded uncertainty

Two runs with the same effective seed and configuration shall produce identical
uncertainty, state, and feedback traces. Runs with different seeds shall use
different stochastic samples. Adding an unrelated stochastic effect to another
plant shall not change the original trace.

The `splitmix64-v1` integer sequence and `[0,1)` conversion shall have exact
known-answer tests. Approximate-normal tests shall assert the ordered sum of the
twelve known uniform samples.

### 18.9 Imperfect sensor

A native closed-loop test shall combine seeded colored noise, bounded random
walk, quantisation, and a `hold_last` dropout. It shall prove each declared
imperfection is visible, the dropout does not invent zero, and replay is exact.

### 18.10 Model mismatch

A nominal first-order plant and an otherwise identical plant with a declared
gain multiplier shall receive the same command. Their outputs shall match their
respective equations and differ after the first active step. Disabling the
effect shall recover the nominal trace without consuming hidden random samples.

### 18.11 Fault atomicity

Force one plant calculation or feedback conversion to become nonfinite or
out-of-range. The runtime shall report the required plant-qualified fault and
retain the complete feedback from the preceding step. No target from the failed
multi-output publication may be partially updated.

### 18.12 Restart and random seed

A cold start and warm restart with a fixed seed shall reproduce the same initial
plant trace. A launch with `seed = "random"` shall expose a concrete seed; a
second launch using that integer shall reproduce the recorded trace.

### 18.13 Closed-loop PID baseline

A native integration test shall connect `Control.PID` to the
`first_order_dead_time` model using the process from Section 6 of *A Practical
Guide to PID Controller Implementation*:

```text
P(s) = exp(-0.5*s) / (1 + s)
Kp = 0.667
Ki = 0.667
plant step = 0.1 s
controller cycle = 0.1 s
```

Initialize the plant state, delayed input, controller histories, and controller
output to zero. Use Auto mode, proportional setpoint weight one, derivative
weight zero, zero derivative gain, no feedforward, and nonbinding output and
rate limits. Apply setpoint one from the PLC cycle at simulation time zero and
hold it through 20 seconds. The cycle at time `t` reads feedback published for
`t`, calculates its command, and the following post-cycle plant substep advances
the interval `[t, t + 0.1]`.

The published process trace shall contain these values:

| Simulation time | Process value |
|---:|---:|
| `1.0` | `0.3464035355654934` |
| `2.0` | `0.8142074414846863` |
| `3.0` | `0.9490966777492305` |
| `5.0` | `0.9924202641525338` |
| `10.0` | `0.9998681292138564` |

Over `[0, 20]`, the process value shall remain in `[0, 1.000000000001]`, enter
and remain within two percent of the setpoint no later than simulation time
`3.9`, end within `1e-7` of the setpoint, and have rectangular-rule integral
absolute error no greater than `1.400`. That integral is the right-endpoint sum
`0.1 * SUM(ABS(1 - y[k]))` for the 200 published samples at `0.1` through
`20.0` seconds. These limits belong to this exact
baseline scenario and do not claim unconditional PID stability. Merely
completing the run or matching a plot by inspection is not a passing oracle.

### 18.14 Resource bounds

A stress test shall run a declared mix of plant models for a declared number of
substeps and prove bounded process memory after activation. A catch-up request
one step above `max_plant_substeps_per_cycle` shall fail visibly without
advancing or partially publishing the plant.

## 19. Separation of verification claims

The following claims are distinct and shall be reported separately:

- equation conformance of a built-in model;
- deterministic replay;
- controller closed-loop performance for a declared plant and scenario;
- robustness across a declared uncertainty envelope;
- runtime computational performance;
- hardware-in-the-loop behavior; and
- fidelity to a particular physical plant.

Passing one claim does not establish another. In particular, a stable result
for one simulated plant does not prove unconditional controller stability, and
a fast accelerated simulation does not prove real-time execution on deployment
hardware.

## 20. First-version boundary

The first version deliberately excludes:

- arbitrary native model plug-ins;
- adaptive-step solvers;
- algebraic-loop solving between plants;
- direct plant-to-plant links;
- hot model replacement;
- retained plant state across warm restart;
- distributed co-simulation across network hosts;
- automatic parameter identification;
- automatic PID tuning;
- fidelity certification; and
- a claim that pseudo-random noise is cryptographically random.

Any later addition requires its own written specification and native executable
test before production implementation.
