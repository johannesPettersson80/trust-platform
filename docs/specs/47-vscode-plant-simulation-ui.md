# VS Code plant-simulation UI

Shared purpose and completion claims: [Control Platform Purpose and Acceptance Contract](control-platform-purpose.md).
This document retains its detailed technical or teaching authority.

Status: normative truST product specification for configuring and observing
simulated plants in the shipped VS Code extension.

This specification defines how a user creates, edits, understands, and
observes the plant models defined by
[`46-simulation-plant-models.md`](46-simulation-plant-models.md) in
**Devices & Connections**. It is a truST product specification, not an IEC
61131-3 semantic rule.

A simulated plant represents the process controlled by the PLC. The PLC
program continues to read ordinary `%I` process-image inputs and write
ordinary `%Q` process-image outputs. The editor configures the simulated
actuator, process, sensor, uncertainty, and scenario behavior behind those
addresses. It shall not require simulation-specific logic in the controller
program.

The words MUST, MUST NOT, SHOULD, and MAY are normative.

## 1. Purpose and scope

The plant-simulation UI shall let a user:

- add a simulated plant to a runtime from Devices & Connections;
- bind PLC output commands to plant inputs and plant feedback to PLC inputs;
- configure every built-in plant model and effect defined by Specification 46;
- begin with a simple process template or configure the model directly;
- save valid plant configuration while the runtime is stopped or running;
- understand when a saved change requires a restart;
- distinguish configured, active, disabled, waiting, and faulted plants;
- inspect live plant state and replay information without changing the run; and
- export or clear bounded trace data when trace collection is enabled.

The UI shall describe a plant as a development and verification model. It
shall not call a configured plant a digital twin, claim physical fidelity,
declare a controller stable, or imply that simulation replaces real-plant,
hardware-in-the-loop, or commissioning validation.

## 2. Ownership and placement

### 2.1 Devices & Connections

Devices & Connections is the only VS Code surface for adding and configuring
simulated plants. The extension shall not add a separate plant editor command,
simulation topology canvas, or second Start/Stop surface.

A plant belongs to exactly one runtime and appears inside that runtime's graph
boundary. A plant is not a communication protocol, fieldbus device, I/O
driver, runtime, or external system. It shall use the existing runtime-child
brick presentation with the explicit role `SIMULATED PLANT`; it shall not be
presented as `Simulated I/O`.

When loading project-scoped saved plants before explicit per-plant placement is
available, the extension shall resolve their owning project runtime
deterministically: the explicit local project runtime first, otherwise the
selected run target, otherwise the sole runtime in the project graph. If more
than one runtime remains eligible, it shall not guess; the plant topology shall
remain unavailable until ownership is resolved.

`Simulated I/O` remains the nonphysical I/O-driver endpoint defined by the
communication contract. A plant may operate through the process image without
the user adding another Simulated I/O card. The UI shall not imply that the
driver and the plant are the same object.

### 2.2 Eligible runtimes

A runtime is eligible to receive a new plant when the selected project can be
configured and the runtime advertises support for Specification 46 plant
models. A stopped local simulator remains eligible because plant configuration
is project configuration and does not require a running process.

A visible runtime that cannot accept a plant shall remain selectable. The add
action shall be disabled with a short reason, such as read-only project,
unsupported runtime version, unavailable project configuration, or target not
permitted to run simulation. The UI shall not hide an otherwise valid runtime
merely because plant editing is unavailable.

## 3. Add picker

The `+ Add` action shall open a picker titled
`Add device, connection, or simulated plant` for the selected runtime. The
picker shall contain a **Simulation** group with a **Simulated plant** item.
Its purpose text shall explain that it models a process behind normal PLC I/O.

The Simulated plant item shall support both:

- activation by mouse, keyboard, or assistive technology; and
- dragging onto an eligible runtime in the graph.

Choosing the item without dragging shall target the selected runtime. If no
runtime is selected, it shall target the same deterministic default runtime as
the existing `+ Add` action. If no eligible runtime exists, the item shall be
disabled and shall state why.

Opening the plant flow shall clear stale save, validation, and fault messages
from a previous add or edit flow. It shall not alter project files.

## 4. Drag-and-drop behavior

### 4.1 Drag feedback

While a Simulated plant item is dragged:

- every eligible runtime shall receive a visible drop-target treatment;
- the runtime currently under the pointer shall receive a stronger focus
  treatment and the label `Add simulated plant`;
- ineligible runtimes shall remain neutral and expose their reason when
  focused or hovered;
- the rest of the graph shall remain readable; and
- the pointer treatment shall communicate that the operation creates a new
  plant rather than moving an existing node.

Drop-target state shall be expressed by text or shape as well as colour. It
shall remain visible in dark, light, high-contrast, and forced-colour themes.

### 4.2 Drop result

Dropping on an eligible runtime shall open a new plant draft in the right-side
drawer and associate the draft with that runtime. The graph shall show one
draft plant brick inside the runtime. The draft shall be exposed as `Draft` by
the shared brick status semantics and
shall not display running, connected, live, or healthy state.

Dropping outside an eligible runtime shall create no draft, change no
selection, and write no configuration. The UI shall provide a short
`Drop the plant onto a runtime` explanation when the failed drop would
otherwise be ambiguous.

Closing or cancelling the drawer shall remove only the unsaved draft without
changing the project. If the node selected before the add picker opened still
exists, the canvas shall restore that node's selection and keyboard focus. If
it no longer exists, the owning runtime shall receive selection and keyboard
focus when it still exists; otherwise the canvas shall clear selection and
return focus to its `+ Add` action. Saving successfully shall replace the draft
with the configured plant and select and focus that plant.

### 4.3 Drag payload contract

The picker shall publish plant drags only as
`application/vnd.trust.simulated-plant+json`. The JSON object shall be closed
and contain exactly:

- `schema_version`: integer `1`;
- `kind`: string `simulated_plant`; and
- `drag_session_id`: a non-empty identifier for the active in-memory drag.

A drop shall be accepted only when the payload parses as that exact schema and
its `drag_session_id` matches the currently active picker drag. Missing or
malformed payloads, unknown fields, unsupported schema versions or kinds, and
session identifiers from an earlier drag shall create no draft and shall write
no configuration. A stale session shall explain that the drag is no longer
active and that the user can start again from `Simulated plant`; malformed or
unsupported data shall be described as an unsupported plant drag without
showing raw payload content.

## 5. Simulated-plant brick

A simulated plant shall use the same runtime-child brick, layout, selection,
focus, persistence refresh, and inspector routing as every other item contained
by a runtime. It shall not introduce a second canvas-node system or a parallel
communication lifecycle. The shared brick receives plant-specific identity,
state, I/O mapping, and model parameters as data.

### 5.1 Identity and content

A configured plant shall render as an ordinary child brick inside its owning
runtime. The brick shall show only the same compact identity information as
another runtime child:

- the plant ID in user-readable form;
- a permanent `SIMULATED PLANT` role band; and
- the current state through the shared brick status treatment.

The selected model, complete I/O mapping, exact state, and model parameters
belong in the shared inspector. They shall not make the canvas brick larger or
create a plant-specific card layout.

An ID containing hyphens or underscores may be humanised for the node title,
but the exact persisted ID shall be available in the inspector and hover
detail. Humanisation shall not change identity or configuration.

The complete ordered mapping shall be shown in the inspector.

### 5.2 Plant states

The visible plant state shall be derived from configuration and runtime
evidence and shall use these product labels:

| State | Meaning |
| --- | --- |
| `Draft` | The add flow is open and nothing has been saved. |
| `Configured` | The plant is saved but no active runtime evidence exists. |
| `Disabled` | The saved plant has `enabled = false` and owns no active I/O. |
| `Waiting for samples` | The active plant exists but no completed live sample is available yet. |
| `Running` | Current runtime evidence reports completed plant execution. |
| `Restart required` | Saved configuration differs from the active runtime revision. |
| `Faulted` | Runtime evidence reports a plant or simulation fault. |
| `Status unavailable` | The plant is configured but live status cannot be obtained. |

`Configured`, `Disabled`, `Waiting for samples`, and `Status unavailable`
shall be neutral states. `Running` may use the shared success treatment only
when current runtime evidence exists. `Restart required` shall use the shared
warning treatment. `Faulted` shall use the shared error treatment.

When several conditions are simultaneously true, the node shall choose one
primary state in this order: `Draft`, `Faulted`, `Restart required`, `Disabled`,
`Status unavailable`, `Waiting for samples`, `Running`, then `Configured`.
Every other true condition shall remain available as a secondary inspector
detail. In particular, a current fault shall not be hidden by a saved/active
revision mismatch, and saving `enabled = false` while an older enabled revision
is still active shall show `Restart required`, not `Disabled`.

Saving a change while an older revision remains active shall not make the new
values appear live. The node shall show `Restart required`, while the inspector
shall distinguish **Saved configuration** from **Running configuration**.

### 5.3 Live presentation

When current samples exist, the node may show the latest transformed command
and published feedback. A compact history or motion treatment may be shown
only from real retained samples. When samples are absent, the UI shall say
`No live samples` or `Waiting for samples`; it shall not draw invented values
or decorative activity.

## 6. Plant setup editor

### 6.1 Shared editor behavior

Adding a plant or choosing Configure plant shall open a focused setup surface
in the main canvas content area, not a narrow settings drawer. The graph shall
not reserve a second inspector-width region for that full-area editor. Reuse
the shared section, field, input, button, message, and focus roles and VS Code
theme tokens. Wide layouts may place model controls beside connection and
starting-condition controls; narrow layouts shall stack them without page-level
horizontal scrolling. This is direct model setup, not a mandatory wizard.

The add breadcrumb shall be
`Devices & Connections / Add simulated plant`. The edit breadcrumb shall be
`Devices & Connections / Edit <plant ID>`.

The editor shall preserve unsaved field values across identical status,
topology, and schema refreshes. Selecting another node, choosing another add
item, or closing the editor shall require confirmation before discarding a
changed draft.

The editor's rules are the runtime's (`plant.edit`, Section 20.1); the
extension keeps no copy of them. The editor holds the draft and shows each
typed value as typed. It sends the draft for checking after a short pause,
never blocking input, and shows only the issues of the answer for the draft as
it now is. An edit that follows a rule is asked of `plant.edit` and applied from
its answer:

- opening a draft;
- choosing or applying a starting point;
- binding a port;
- resizing or pasting into state-space matrices;
- adding or moving curve points;
- adding, duplicating, moving or removing effects and events;
- changing an effect's preset, target or generator;
- changing an event's action;
- entering a temperature-example value;
- typing into a Dynamics field; and
- confirming the example's units.

Labels, help, defaults and choices come from the editor tables of
`plant.capabilities`.

### 6.2 Sections

The initial setup shall expose these named tasks in reading order:

1. **Model & behavior**: model dropdown, one explanation, model diagram and
   only the selected model's response parameters;
2. **Connections**: named PLC output/input selectors with direction explained;
3. **Starting conditions**: initial model state and effective step summary;
4. **Optional realism and advanced settings**: actuator, conversion,
   imperfections and scenario controls behind progressive disclosure.

Optional settings shall not expand as one combined engineering form. They shall
provide separate, plainly named task destinations for **Actuator & command**,
**Signal conversions**, **Realism** (process and sensor imperfections), and
**Scenarios**. Each destination shall keep a concise configured-state summary
visible while its editor is closed, including inactive/default states rather
than implying that hidden values do not exist. Opening one destination shall
not open the others. Switching or closing destinations shall preserve the same
canonical draft, invalid raw text, validation state, and persisted ordering.
When validation navigation targets one of these settings, it shall open the
owning destination before focusing the exact field or row.

Scenario scheduling shall show **Time (s)** with explicit 0.001 s precision.
The persisted contract remains integer `at_ms`: 1.001 seconds shall save and
reopen as exactly 1001 milliseconds. Empty, non-finite, negative, unsafe or
sub-millisecond values shall remain visibly invalid and block Save, without
silently rounding, substituting zero or restoring the previous valid value.
The raw edit and validation shall survive closing and reopening Scenarios.
Changing time units shall not change event order, action, channel, Include
state, or equal-time ordering semantics.

State and sensor event targets shall be named quantity selectors, not bare
channel numbers. Names come from the selected model's existing state and output
descriptors (without the starting-condition prefix for a scheduled change).
For example, the robot joint exposes Angle (rad) and Angular speed (rad/s).
Unnamed dimensions use State 1 / Output 1 and subsequent one-based labels.
Labels are UI context only; selection retains the same zero-based numeric
`channel`, action, Include state and declaration order on the runtime wire.

Realism shall show configured effect cards in canonical execution order, with
Command, Process or Sensor boundary labels and explicit excluded states.
An Add effect chooser shall use the existing preset types before creation.
Only the selected effect's applicable controls shall be exposed; exact runtime
mapping remains an advanced disclosure. Category labels shall not reorder the
underlying effects or imply a different execution order. Validation navigation
shall open the owning effect before focusing its erroneous field. Collapsing
cards or Realism shall retain invalid edits and must not unblock Save.

Technical identity and description shall be initially collapsed under Plant
details rather than interrupting model selection and connection setup. Required
errors shall open their owning section before moving focus. Effective configured
effects shall remain summarized while collapsed. Basic explanations and units
shall be inline, not available only through question-mark tooltips. In FOPDT,
the scalar starting value belongs to Starting conditions, not an unrelated
engineering-effects disclosure.

For the robot-joint model, Starting conditions shall expose separate finite
numeric fields `Starting angle (rad)` and `Starting angular speed (rad/s)`.
They edit the existing initial-state vector at indices 0 and 1 respectively;
editing either shall preserve the other value. Radians are explicit, with no
implicit degree conversion. The Dynamics section shall not duplicate these
values as a raw array. Invalid raw text shall remain visible and block Save
across optional-section toggles until corrected. Saving and reopening shall
preserve both states exactly, including negative angles and nonzero speeds.

The same labelled Starting conditions pattern shall replace raw state arrays
for these models, preserving canonical state indices without introducing a
physical identity for a generic model:

| Model | Fields in canonical state order | Meaning |
| --- | --- | --- |
| First order | Starting output | Absolute ideal output, in the configured model's output units |
| Integrator | Starting accumulated value | Stored state before output bias; explain that output equals this value plus output bias |
| Gravity-drained tank | Starting level (m) | Tank liquid level in metres |
| Second order | Starting displacement; Starting rate of change | Output displacement before output bias, then its time derivative in output units per second |

Each field shall preserve its siblings, retain invalid raw text, and block Save
until corrected. Dynamics shall not duplicate the state vector as a raw array.
These display labels shall not change runtime dynamics or add unit conversion.

The editor is a configuration surface. It shall not contain live values,
history charts, runtime state vectors, or trace rows. When the selected runtime
can be monitored, the editor may provide one quiet `Open in Control Studio`
action. That action shall open Control Studio with the current runtime and
plant selected; it shall not expand monitoring content inside the editor.

## 7. Plant identity and model selection

Plant details and the corresponding task sections shall retain:

- the plant ID;
- display name;
- optional description;
- owning runtime placement;
- enabled state;
- the selected model; and
- plant step override when used.

The plant ID shall be validated against the identity rules in Specification
33 and shall be unique. The UI may suggest a valid ID from user-entered text,
but the exact ID to be saved shall be visible before Save.

Display name and description are project metadata and shall serialize as the
optional `name` and `description` fields from Specification 46. Runtime
placement identifies the owning runtime selected by the add flow; it is shown
for orientation and is not an editable free-form identity field.

The first decision in Add and Model & behavior shall be one labelled **Plant
model** dropdown with exactly these ordered names and stable runtime kinds:

| Name | Runtime kind |
| --- | --- |
| Static gain | `static_gain` |
| First order | `first_order` |
| First order with dead time | `first_order_dead_time` |
| Integrator | `integrator` |
| Second order | `second_order` |
| Piecewise linear | `piecewise_linear` |
| State space | `state_space` |
| Gravity-drained tank | `gravity_tank` |
| Gravity-loaded robot joint | `robot_joint` |

All nine shall be selectable without equipment/category cards, an expert-mode
gate, a compiled PLC or a manual. Selection alone shall write nothing. Saved
models shall reopen with their actual model kind selected. State space may have
advanced parameter editing without hiding the model itself from this list.

Optional teaching proposals may include:

- Tank or accumulated level — `integrator`;
- Temperature or slow process — `first_order_dead_time`;
- Motor speed or first-order process — `first_order`;
- Mechanical position or resonant process — `second_order`;
- Static scaling — `static_gain`;
- Nonlinear characteristic — `piecewise_linear`; and
- Custom state-space model — `state_space`.

A starting point shall populate explicit editable model values. It shall be
described as a starting point that the user must adapt to the real process. It
shall not claim that the chosen values identify or reproduce a physical plant.

The six familiar starting points shall use these deterministic initial
proposals. Biases and initial states not shown separately are zero. These
values are unit-neutral simulation seeds, not identified engineering values.

| Starting point | Model | Initial proposal |
| --- | --- | --- |
| Tank or accumulated level | `integrator` | gain `1`; initial state `0`; no optional state limits |
| Temperature or slow process | `first_order_dead_time` | gain `1`; time constant `10 s`; dead time `1 s`; initial delayed input `0` |
| Motor speed or first-order process | `first_order` | gain `1`; time constant `1 s` |
| Mechanical position or resonant process | `second_order` | gain `1`; natural frequency `1 rad/s`; damping ratio `0.7`; both initial states `0` |
| Static scaling | `static_gain` | gain `1` |
| Nonlinear characteristic | `piecewise_linear` | input points `[-1, 0, 1]`; output points `[-1, 0, 1]` |

These optional proposals are not the primary selector or separate model kinds.
Choosing State space directly proposes a one-state, one-input, one-output model
with `A=[[0]]`, `B=[[1]]`, `C=[[1]]`, `D=[[0]]`, and `initial_state=[0]`.
Its matrix fields remain in the model editor; identity controls shall not
duplicate them.

Changing starting point may replace model-owned parameters and initial state
immediately only while those values still equal the untouched proposal. Once
the user has edited any model-owned value, or the draft was loaded from a saved
plant configuration, selecting another model/starting point
shall open a confirmation that names the current and requested starting points
and states that current dynamics values will be replaced. Cancel shall preserve
the complete draft and return focus to the starting-point control. Confirm
shall replace only model identity, model-owned parameters, and initial state;
plant identity, placement, enabled state, signal mappings, and all unrelated
sections shall remain unchanged.

### 7.1 Optional temperature teaching example

After choosing First order with dead time, `Use temperature example` shall be
available without an equipment chooser. The generic model remains unit-neutral.
This explicit example uses a percent heater command and a Celsius output:
resting temperature 20 °C, full-power rise 100 °C, time constant 30 s, delay 2 s,
and starting temperature 20 °C. The exact runtime mapping is `gain=rise/100`,
`input_bias=0`, `output_bias=resting temperature`, `initial_state=[starting
temperature]`, `initial_delayed_input=0`, and actuator minimum/maximum 0/100
with `initial_applied=0`. The state is absolute temperature, not a deviation.
A negative rise is permitted and describes cooling; response time must be
positive and delay nonnegative. Starting away from the resting temperature is
permitted and shall be explained as a non-equilibrium start.

Applying this example shall use the existing replacement confirmation, explicitly
listing replacement of model values, starting state, actuator settings and signal
conversions. Confirm sets initial input to zero and identity scale/offset for
each existing port. Keep current values and Escape shall change nothing.
Identity, runtime, bindings, validity, scenario events and uncertainties are
preserved; retained effects may change the nominal behavior. Selecting a physical
example does not infer units from symbol names. Show `PLC 0–100 = heater 0–100%`
and `PLC feedback = temperature in °C` as explicit assumptions, with confirmation
required before saving. The confirmation covers the PLC signals and conversions
it was given for: changing a binding or a conversion, or applying the example
again, asks for it again. Raw-count signals need setup in Signal conversions.

This view edits the canonical draft directly, not a second parameter store.
Changing a display field uses the exact inverse mapping above. A Model details
view exposes the stored values without resetting them. Imported configurations
remain generic unless the user explicitly applies the example; no equipment
identity is inferred from matching coefficients. Presentation identity is not
serialized into the runtime model schema. Show `Example settings — not calibrated
to your equipment`. Saving never starts the simulator or commands real equipment.

The nominal 0-to-50% step has equilibrium 70 °C and reaches
`20 + 50*(1-exp(-1))`, about 51.6 °C, at 32 s. This is a native runtime oracle,
not permission to add an independent browser simulator or label a formula as a
measured response. Prediction rendering remains governed by its separate contract.

## 8. PLC signal mapping

The PLC signals section is used to connect the controller to the plant through
the normal process image.

Command rows shall select ordered numeric `%Q` sources. Feedback rows shall
select ordered numeric `%I` targets. Each selectable signal shall show, when
available:

- Structured Text symbol;
- direct address;
- declared numeric type; and
- current value when live evidence exists.

The selectable catalog shall come from a successful compilation of the
current project sources. It shall contain only direct process-image bindings
whose declared type is `SINT`, `INT`, `DINT`, `LINT`, `USINT`, `UINT`,
`UDINT`, `ULINT`, `REAL`, or `LREAL`. Bit-string, Boolean, time, date, string,
enumeration, aggregate, untyped, wildcard, and unresolved bindings shall not
be offered. A `%M` memory binding is offered as a command source only, as
specification 46 §13.1 allows a memory command; it is never a feedback target.
Within each direction, candidates shall be ordered by direct address and then
symbol so the stopped and running views remain stable.

When compilation fails, the picker shall offer no candidates and shall show
the compile failure without exposing partial or source-scanned guesses. A
stopped runtime shall still show compiled symbol, address, and type. Current
values are an optional live overlay matched by canonical direct address; a
missing or stale live entry shall not remove the compiled candidate and shall
not be presented as current.

Manual address entry may be offered, but shall use the same validation and
shall not bypass compiled process-image metadata requirements.

Picker selection and manual entry shall report failures at the affected row.
An address is invalid when it is absent from the compiled catalog, belongs to
the opposite process-image direction, duplicates another feedback target, or
cannot be used because an enabled physical I/O driver is configured. Because
Specification 46 defines no mixed-mode policy in version 1, any enabled
physical I/O driver blocks plant signal ownership; the message shall name the
driver and direct the user to Devices & Connections. A driver is physical unless
Specification 46 §6.1 item 5 names it as touching no real I/O (`simulated`,
`sim`, `noop`, `loopback` or `none`). Disabled physical drivers
do not block editing or validation. Multiple plants may share a command source,
as Specification 46 permits.

Each command row shall expose input scale, input offset, and initial input.
Each feedback row shall expose output scale and output offset. Default scale
and offset values shall be shown explicitly or clearly described.

Rows shall be addable, removable, and reorderable for models that support
multiple channels. Reordering shall change the persisted channel order. A
single-input/single-output model shall require exactly one command and one
feedback row. The robot-joint model shall expose one torque command (N·m),
joint-angle feedback (rad), and angular-speed feedback (rad/s), in that order.
Clearing or changing a binding shall preserve every other port's index and
engineering conversion; an unbound port shall remain visible and block Save.
A
state-space model shall keep command, feedback, matrix, and initial-state
dimensions visibly consistent.

The UI shall report ownership conflicts with another plant, coupling, or
physics target before Save completes. It shall not offer a control that
silently reassigns or removes the conflicting owner.

## 9. Dynamics

The Dynamics section shall show the selected model's product name, short
purpose, state count, input count, and output count. It shall show only fields
owned by that model.

The model-specific fields are:

| Model | Visible dynamics fields |
| --- | --- |
| Static gain | gain, input bias, output bias |
| First order | gain, time constant, input bias, output bias |
| First order with dead time | first-order fields, dead time, initial delayed input |
| Integrator | gain, input bias, output bias, optional state minimum and maximum |
| Second order | gain, natural frequency, damping ratio, input bias, output bias |
| State space | A, B, C, and D matrices, initial-state vector, and per-input transport delays in seconds |
| Piecewise linear | ordered input points and output points |
| Gravity-drained tank | tank area and outlet coefficient |
| Gravity-loaded robot joint | joint inertia, gravity torque, viscous friction, Coulomb friction and friction smoothing |

Initial states for first order, first order with dead time, integrator, second
order, gravity-drained tank and gravity-loaded robot joint belong in the
separate Starting conditions section defined in section 6.2. State space keeps
its initial-state vector beside the matrices because its dimensions are part of
the same advanced model editor.

Labels shall use engineering language and units, such as `Time constant (s)`,
while preserving the exact configuration identity in field help. Required
values, defaults, finite-number requirements, positive/nonnegative domains,
matrix dimensions, point ordering, and bounds shall match Specification 46.

Saved models with omitted `initial_state` shall remain editable. Display the
model-sized zero vector defined by Specification 46 section 5.2, preserving
the omission during no-op or identity-only edits. Starting-state changes may
materialize the edited vector; never replace existing parameters or dimensions
with a teaching template when resolving an omitted state.

Visible fields shall not make runtime-defaultable scalars mandatory. Omitted
gain resolves to 1 for static gain, first order, first order with dead time,
integrator and second order; omitted FOPDT dead time resolves to 0. The editor
shall explain these effective defaults without materializing them in the saved
candidate. Explicit zero remains distinct from omission. Required parameters
remain required under Specification 46 §7.

The state-space editor shall present matrices as editable grids with visible
row and column indexes. Changing state, input, or output dimensions shall
resize dependent matrices only after confirmation when existing values would
be removed. A change names only the dimension it changes; the other dimensions
are the draft's when the change is applied, so that dimensions changed one
after the other all apply. Newly created cells shall receive explicit defaults. The editor
shall identify the exact invalid cell when a matrix value or dimension is
invalid.

Continuous state space shall show a compact input-delay row aligned with the
B/D input columns. Each delay is in seconds and follows Specification 46
sections 7.6.1 and 5.1.1. Omission displays effective zeros without writing a
vector during unrelated edits; explicit zero remains explicit. A reset action
may restore the omitted zero default. Other model editors shall not show this
continuous state-space field.

State/output dimension changes shall preserve input delays. Growing the input
dimension shall preserve existing explicit delays and append zeros; shrinking
shall align the vector and explicitly identify removal of nonzero delays in
the existing destructive-resize confirmation. Cancel shall preserve the draft.
Delay edits shall survive Save and reopen through the existing plant contract.
Negative, nonfinite, wrong-shaped and wrong-length vectors shall block Save,
with the affected input identified. When the plant override or project default
step is known, the editor shall apply the native aggregate history limit using
that effective step. It shall not persist editor-only default-step metadata or
invent an effective step when the project setting is unavailable.

The piecewise-linear editor shall present paired input/output rows. Rows shall
be reorderable only when the resulting input points remain strictly
increasing, and the editor shall identify the first invalid or duplicate input
point.

## 10. Actuator

The Actuator section configures the simulated final control element between
the PLC command and the process. It shall be available only for models with one
command source.

The section shall expose minimum, maximum, rising rate, falling rate,
deadband, lag, and initial applied value. Omitted unbounded values shall not be
displayed as zero. The section shall explain that a zero rate prevents movement
in that direction, while an omitted rate is unbounded.

For an active plant, retain the applied actuator value and saturation,
slew-limit, deadband and stuck indicators in the runtime plant evidence defined
by Specification 46. They are not live fields in the Actuator configuration
section. Control Studio uses that evidence under its three page specifications.

## 11. Imperfections

The Imperfections section is used to make the plant deliberately less ideal.
It shall manage an ordered list of uncertainty effects. Each row shall show:

- effect ID;
- target;
- target channel or parameter;
- mode;
- generator;
- active interval; and
- a concise parameter summary.

The user shall be able to add, edit, remove, duplicate, and reorder effects.
The persisted order shall match the displayed order because effect ordering is
behavioral.

The target, mode, generator, and parameter controls shall expose only valid
combinations from Specification 46. In particular, hold, quantisation,
clipping, and dropout shall be constrained to their permitted targets and
modes. A parameter target shall select only a parameter that exists for the
current model, including indexed state-space matrix elements.

Changing the model shall identify effects that are no longer valid. The UI
shall not silently delete or retarget them. Save shall remain blocked until the
user fixes or removes every invalid effect.

Runtime plant evidence retains active effects and dropouts. The Imperfections
configuration section does not expose live effect state or generated random
values as configuration defaults. This contract does not add live diagnostics
to Control Studio's pages.

## 12. Scenario events

The Scenario events section is used to schedule explicit changes and faults at
simulation times. It shall manage an ordered list of the event kinds defined
by Specification 46.

Each event shall show its due time, action, target, and value where applicable.
Events with the same due time shall retain displayed declaration order. The
user shall be able to add, edit, remove, duplicate, and reorder equal-time
events.

Controls for state channel, parameter, sensor channel, and value shall appear
only when owned by the selected event kind. Model changes shall not silently
rewrite invalid events. Save shall remain blocked until every event is valid.

Runtime plant evidence retains pending and fired events. The Scenario events
configuration section contains neither live pending state nor trace rows.
Control Studio's current page contracts own the presentation of experiment
results; no pending-event counter is added to them.

## 13. Validation and saving

### 13.1 Validation

The editor shows the checks `plant.edit` answers (Section 20.1): required
fields, numbers, dimensions, addresses, and every section's rules. When those
find nothing, `plant.edit` also judges the complete resulting configuration
with the runtime's plant-configuration validation, which remains
authoritative; its refusal is shown as an issue of the whole plant. Every
issue shown blocks Save.

Validation errors shall:

- identify the owning section and field, row, matrix cell, or list item;
- retain all entered values;
- focus or reveal the first error from the summary;
- explain the valid domain in user-facing language; and
- prevent Save while unresolved.

An invalid save shall not change any part of `simulation.toml`, active runtime
state, graph topology, or the last successfully saved plant.

### 13.2 Save result

Saving shall apply the complete plant change atomically while preserving all
other plants, couplings, disturbances, physics configuration, and unrelated
project content.

Within the selected plant, fields not managed by the current editor (including
model validity) shall survive projection, draft
creation and serialization unchanged. Editable fields remain authoritative:
clearing an optional editable effect must not resurrect its saved value.
Unsupported fields must remain isolated from the editable draft values; the
runtime validates the complete resulting candidate before any write.
This does not add support for new runtime configuration fields. In particular,
the current runtime accepts one scalar actuator configuration, not a serialized
per-channel `actuators` array.

When the runtime is stopped, a successful result shall read `Saved` and the
configured node shall replace the draft. When a runtime is active, a
successful configuration change shall read `Saved · restart required`.
Neither result shall claim that the changed model is already running.

If the project changed after the drawer loaded, Save shall reject the stale
draft, retain all entered values, and offer reload-and-review. It shall not
overwrite the newer project state automatically.

## 14. Control Studio plant context

Opening a plant from Devices & Connections reuses the active editor group and
preserves the selected runtime, plant and loop. Generic raw process-image
inspection remains owned by Live Values. Plant configuration remains owned by
Devices & Connections; it is not copied into another Control Studio form.

The [Identify](control-studio-identify-page.md),
[Tune](control-studio-tune-page.md) and
[Validate](control-studio-validate-page.md) specifications are the only
Control Studio page layouts. Runtime plant status, faults, timing, random/effect
identities, actuator evidence and bounded trace data remain available to the
acquisition/validation services under Specification 46. Keep channel ordering,
units and process-image/engineering-value distinctions intact. No plant-summary
panel, state/counter strip or trace-management toolbar is prescribed here.

### 14.1 Typed Control Studio handoff

Opening Control Studio from a plant shall send one versioned, project-scoped
handoff. The handoff shall contain:

- the handoff schema version and unique handoff identity;
- project identity;
- selected runtime identity and current runtime-control identity when known;
- plant identity and saved plant-configuration revision;
- active plant-configuration digest when current runtime evidence supplies it;
- matching loop identity when one has been selected or discovered;
- explicit `Simulation` source class; and
- the requested Control Studio context, initially `Identify`.

The handoff shall contain identities and revisions, not copied live values,
trace rows, editable plant fields, filesystem paths, authorization decisions,
or UI-only labels. Control Studio shall resolve current state through its own
typed services and shall reject an unsupported version, wrong project,
unknown runtime, unknown plant, or stale identity without guessing. A rejected
handoff shall leave the prior editor state unchanged and show the exact
recoverable reason. Retrying the same handoff identity shall not open duplicate
editors or duplicate a running operation.

## 15. Safety and truthfulness

Every plant node, draft, configuration view, Control Studio plant view, trace
view, and exported visual summary shall identify the plant as simulated.

Adding a plant shall never:

- start a runtime;
- enable a physical I/O driver;
- disable an existing I/O driver;
- rewrite PLC source code;
- create hidden process-image mappings;
- resolve an address conflict automatically; or
- claim that controller stability or plant fidelity has been established.

If enabled physical I/O conflicts with simulation safety rules, the UI shall
show the conflict before start and direct the user to the owning I/O
configuration. It shall not offer an override because Specification 46 defines
no mixed physical/simulated output mode in the first version.

A simulation fault shall remain visible even when search or filtering would
otherwise hide the plant. Fault presentation shall include plant ID, model,
simulation time, and the user-facing failing operation when supplied by the
runtime. The UI shall not replace a failed value with zero or continue to
animate the plant as healthy.

## 16. Search, filtering, and topology summaries

Search shall match plant ID, humanised title, model name, command address,
feedback address, active effect ID, and fault text. A nonmatching plant may be
dimmed for search presentation, but its health shall continue to contribute to
runtime rollup.

The Filter pane shall include a `Simulated plants` choice. Filtering shall
report hidden plant and fault counts and shall not hide a faulted plant.

Topology summaries shall count plants separately from communication endpoints,
for example:

```text
1 host · 1 runtime · 2 devices · 1 simulated plant
```

A runtime with neither endpoints nor plants shall say
`No devices or simulated plants yet`. Its guidance shall keep `+ Add` as the
primary route and Discover as the network-hardware route.

Direct plant-to-plant wires shall not be drawn. Plant dependencies continue to
pass through explicit PLC process-image signals as required by Specification
33.

## 17. Visual, responsive, and accessibility behavior

The plant surface shall use the shared truST webview theme. A simulation role
may extend that shared theme, but it shall derive from VS Code theme variables
and shall not introduce a private palette or hard-coded per-theme colours.

The plant brick shall be distinguishable from communication endpoints through
its `SIMULATED PLANT` role band and wording, not through colour alone.
Status shall always pair colour with text. Focus, selection, draft, drop
target, warning, and fault states shall remain distinct in dark, light,
high-contrast, and forced-colour themes.

The drawer shall remain usable when the editor is narrow. The graph shall
reframe after the drawer opens, closes, or changes width. Essential labels,
errors, Save, and Cancel shall remain visible without horizontal page
scrolling. Large matrices and ordered lists may scroll inside their owning
section.

All add, drag-equivalent, edit, list, matrix, Save, Cancel, remove, trace, and
recovery operations shall be keyboard reachable. Drag-and-drop shall never be
the only way to create a plant. Controls shall have accessible names, errors
shall be associated with their fields, and focus shall move predictably when a
drawer opens, validation fails, Save succeeds, or a confirmation closes.

Animation shall respect reduced-motion preferences. A live pulse, transition,
or compact history shall stop or become static under reduced motion without
removing the underlying state label or value.

## 18. Guided learning and professional use

The plant editor shall use one configuration and one set of runtime values for
all users. It shall not create separate beginner and professional
configurations whose values can diverge.

The default presentation shall show the shortest complete path to a working
plant: choose a familiar process or model, map the PLC signals, enter the
essential dynamics, validate, and save. Optional actuator, imperfection,
scenario, and model-specific engineering fields shall use progressive
disclosure. Collapsing or expanding details shall never change, reset, or
silently default a stored value.

Labels, values, units, validation state, and the current primary action shall
remain visible. Explanatory teaching text shall not occupy the drawer by
default. Each model, section, and editable parameter that needs explanation
shall provide a compact `?` help button beside its heading or label.

Activating `?` shall open a small tooltip or popover that states:

- what physical behavior it represents;
- how increasing or decreasing it normally changes the simulated response;
- its unit or that it is dimensionless; and
- the exact stored configuration name when the destination details are shown.

Hover and keyboard focus may show a short summary. Click, Enter, or Space shall
open the complete explanation. Escape shall close it and return focus to the
same `?` button. The button shall have an accessible name that identifies its
subject, such as `Help for response speed`; the question-mark glyph alone is
not an accessible name. Help shall not open merely because the drawer opens,
and opening one explanation shall not expand every other explanation.

Validation errors, safety conflicts, unavailable state, unsaved changes, and a
required restart shall not be hidden behind `?`. These are actionable current
states and shall remain visible at the place where the user must respond.

Help shall use plain process language before mathematical terminology. Where a
term has a common control-theory name, both shall be shown, for example
`delay (dead time)` and `stored energy (integrating behavior)`. Help may show
the model equation or matrix dimensions on demand, but it shall not require a
formula to complete the common preset workflow.

Preset selection shall provide its reasoning and limitations through the same
compact help behavior. These explanations shall not claim that the selected
model identifies the real plant, proves controller stability, or provides safe
tuning.

Starting-point help shall convey these model-specific facts in plain language:

| Starting point | Why the model is reasonable | Important limitation |
| --- | --- | --- |
| Tank or accumulated level | It accumulates the difference between inflow and outflow. | It cannot settle by itself or reproduce leakage unless those effects are added. |
| Temperature or slow process | One dominant lag and a transport delay approximate many heating processes. | It cannot reproduce resonance, multiple time scales, or nonlinear heating. |
| Motor speed or first-order process | One dominant lag approximates speed response when electrical or mechanical dynamics dominate. | It cannot reproduce resonance, transport delay, saturation, or friction. |
| Mechanical position or resonant process | Two states represent inertia and damping, including oscillatory response. | It cannot reproduce backlash, friction, hard stops, or other nonlinear mechanics. |
| Static scaling | An immediate gain represents a sensor, conversion, or process with negligible dynamics. | It cannot reproduce stored energy, lag, delay, or oscillation. |
| Nonlinear characteristic | Ordered points represent a known static nonlinear input/output curve. | It cannot reproduce dynamics, hysteresis, or rate dependence. |
| Custom state space | Coupled linear states represent multi-state or multi-channel linear dynamics, with explicit per-input transport delays. | It cannot reproduce nonlinear or time-varying behavior. |

Validation messages shall explain both the problem and the next corrective
action. They shall retain the exact field name and value needed by a
professional while remaining understandable without knowledge of the
configuration file.

Professional users shall be able to reach every supported field, exact value,
matrix entry, ordered effect, event, address, and runtime trace without editing
TOML by hand. The UI shall preserve full numerical precision during editing and
saving even when a compact display rounds a value.

Context help may be dismissed, but it shall remain available again from the
same `?` button. Keyboard and screen-reader users shall receive the same
explanations, limitations, units, and recovery guidance as pointer users.

## 19. First-version boundary

The first version of the VS Code plant-simulation UI does not provide:

- automatic process identification;
- automatic PID tuning;
- a claim of controller stability;
- arbitrary model plug-ins;
- direct plant-to-plant graph wiring;
- hot replacement of an active model;
- a graphical physics or 3D twin editor;
- synthetic live values or decorative fake activity; or
- an override for mixed physical and simulated outputs.

## 20. Versioned plant service contract

The runtime control surface, stopped-project command surface, extension host,
and webview shall exchange one logical `trust.plant.v1` contract. A producer
shall emit schema version `1`. A consumer shall reject every other version and
shall not infer omitted fields from an older or newer schema.

The contract shall expose these operations:

| Operation | Minimum role | Behavior |
| --- | --- | --- |
| `plant.capabilities` | Viewer | Return supported schema versions, model kinds, limits, and available operations. |
| `plant.read` | Viewer | Return the saved configuration, its revision, and plant topology without requiring a running runtime. |
| `plant.validate` | Engineer | Validate a complete candidate without writing files or changing runtime state. |
| `plant.save` | Engineer | Atomically replace the requested saved plant configuration after validation and revision checks. |
| `plant.remove` | Engineer | Atomically remove one saved plant and the scenario events owned by it after revision checks. |
| `plant.status` | Viewer | Return current active-runtime evidence or an explicit unavailable result. |
| `plant.trace.read` | Viewer | Return one bounded runtime-provided trace snapshot without changing simulation state. |
| `plant.trace.clear` | Operator | Clear retained trace rows for one active plant without resetting or advancing simulation state. |
| `plant.edit` | Engineer | Apply one plant-editor edit to a draft and check the draft, without writing files or changing runtime state. |
| `plant.signals` | Viewer | Build the PLC signals a plant can bind to from compiled bindings, live values and the communication setup, without writing anything. |

Authentication failure and insufficient role shall be rejected before request
payload processing. A stopped-project command shall apply the same validation,
revision, and result semantics as the corresponding control operation, except
that live status and trace operations return `status_unavailable`.

### 20.1 Common envelope and closed variants

Every success and error payload shall contain:

- `contract = "trust.plant"`;
- `schema_version = 1`;
- `operation` containing the exact requested operation; and
- `project_id`, an opaque `project:sha256:<hex>` identity derived from the
  canonical project root and used to prevent cross-project reuse. The path
  itself shall never be returned.

`plant.capabilities` and `plant.read` establish the producer's project ID; a
caller-supplied project ID is optional for those two operations and, when
present, shall match. Every other operation shall require the exact project ID
returned by the producer.

Every mutating request shall additionally contain a non-empty `operation_id`.
Repeating the same operation ID with byte-equivalent validated input shall
return the original result without applying the mutation again. Reusing it
with different input shall return `idempotency_conflict`.

Requests and responses are closed variants. Unknown operation names, unknown
fields in mutating requests, missing required fields, non-finite numbers,
invalid identifiers, invalid addresses, oversized collections, and values
outside declared capability limits shall be rejected. Rejection shall not
partially update saved configuration, runtime state, trace, or topology.

Integer runtime identities and nanosecond durations that can exceed the JSON
safe-integer range shall be encoded as unsigned base-10 strings. In version 1,
this applies to `effective_seed`, every `simulation_time_ns`, every `step_ns`,
every `accumulated_remainder_ns`, and every trace-row `sequence`. Consumers
shall reject signs, whitespace, fractions, exponents, leading zeroes other than
the value `0`, and values outside the source integer's range. Bounded counts,
declared limits, and algorithm versions remain JSON integers.

The version-1 request variants are:

| Operation | Required operation-specific fields | Optional fields |
| --- | --- | --- |
| `plant.capabilities` | none | `project_id` |
| `plant.read` | none | `project_id` |
| `plant.validate` | `project_id`, complete `plant` candidate | none |
| `plant.save` | `project_id`, `operation_id`, `expected_revision`, complete `plant` candidate | none |
| `plant.remove` | `project_id`, `operation_id`, `expected_revision`, `plant_id` | none |
| `plant.status` | `project_id` | `plant_id` |
| `plant.trace.read` | `project_id`, `plant_id` | none |
| `plant.trace.clear` | `project_id`, `operation_id`, `plant_id`, `active_configuration_revision` | none |
| `plant.edit` | `project_id`, `draft` | `action`, `signals` |
| `plant.signals` | `project_id`, `compiled` | `live`, `communication_schema`, `communication_topology` |

Every variant also requires the common `contract` and `schema_version` fields.
An omitted `plant_id` on `plant.status` requests all active plants. An empty
identifier is invalid; omission and an empty string are not equivalent.

The plant editor's requests carry the editor's own draft, not a saved
candidate:

- `plant.edit` takes `draft`, the editor's draft object. The optional
  `action` (`{"kind": ...}`) names one rule-bearing edit of Section 6.1:
  `open`, `choose_starting_point`, `apply_starting_point`,
  `set_temperature_value`, `set_dynamics_text`, `confirm_temperature_units`, `bind_port`,
  `resize_state_space`, `paste_matrix`, `append_point`, `move_point`,
  `add_effect`, `apply_effect_preset`, `duplicate_effect`, `move_effect`,
  `remove_effect`, `set_effect_target`, `set_effect_generator`, `add_event`,
  `duplicate_event`, `move_event`, `remove_event` or `set_event_kind`. The
  optional `signals` is the catalog `plant.signals` answered. The answer
  carries:
  - `draft`, the draft after the edit;
  - `outcome`, present with an action: `applied` with the row it created or
    moved to, `confirm` with the question, or `rejected` with the reason;
  - `issues`: each with `code`, `severity`, `path`, `message`, `summary`, the
    `target` the editor focuses, and `plant_id`;
  - `can_save`, true when there is no issue;
  - `view`, what the editor shows besides the values; and
  - `candidate`, the plant candidate Save sends.

  `set_dynamics_text` with `key` and `text` gives the Dynamics field `key` of
  the draft's model the value its typed text spells. Empty text holds no
  value, and `initial_state` an empty list. A number field holds the number
  that JavaScript's `Number` reads (`0x10` is 16, `1e3` is 1000, surrounding
  spaces are ignored), or `null` when that is not finite, which the checks
  report. A list or matrix field holds the JSON value of the text, else the
  text itself, which the checks report. The draft's `modelValuesTouched`
  becomes true. A key that the model has no field for is `malformed_request`.

  An unknown action or a malformed draft is `malformed_request`.
- `plant.signals` takes the `compiled` answer of `trust-lsp.hmiBindings`, the
  `live` process image, and the communication schema and topology the canvas
  holds. It answers `signals` with:
  - `status`: `ready` or `compile_error`;
  - the `commands` and `feedbacks`, each with `symbol`, `address`, `dataType`
    and an optional `currentValue`;
  - optional `physicalDrivers`; and
  - an optional `error`.
- `plant.capabilities` also answers `editor`, the editor tables:
  - models;
  - actuator fields;
  - starting points;
  - imperfection presets;
  - uncertainty targets, modes, generators, field labels and dropout policies;
    and
  - scenario actions.

An error shall contain one of these stable codes:

`unsupported_schema`, `unsupported_operation`, `malformed_request`,
`missing_auth_token`, `invalid_auth_token`, `insufficient_role`,
`validation_failed`, `revision_conflict`,
`idempotency_conflict`, `plant_not_found`, `status_unavailable`,
`trace_disabled`, `runtime_conflict`, or `io_failure`.

An error may contain technical detail and validation issues, but UI wording is
not part of the service contract. Each validation issue shall contain a stable
code, severity, exact field path, and offending plant ID when known. A field
value that may contain a credential shall never be echoed.

### 20.2 Saved configuration and revision

`plant.read` and every successful `plant.save` or `plant.remove` shall return:

- `configuration_revision`, an opaque digest of the exact saved
  `simulation.toml` content;
- the complete saved simulation settings and ordered plant/event definitions;
- a topology descriptor for every saved plant, including disabled plants; and
- `active_configuration_revision` when a running runtime can prove which saved
  revision it activated.

The topology descriptor shall contain stable plant ID, display name,
description, model kind, enabled state, command/process/feedback mappings, and
saved order. It shall not contain rendered labels, colours, drawer state,
filesystem paths, credentials, or fabricated runtime values.

Every save request shall contain the caller's `expected_revision` and a
complete candidate for the plant being created, replaced, or disabled. If the
current saved revision differs, the mutation shall fail with
`revision_conflict` and return the current revision plus the current saved
plant descriptor when that plant still exists. No bytes shall be written.

The request's plant candidate shall contain `scenario_events`, an ordered
array owned by that plant editor. Each entry shall contain `at_ms`, `kind`, and
only the `channel`, `parameter`, or `value` fields permitted for that kind by
Specification 46. The plant identity is taken from the enclosing candidate and
shall not be repeated or accepted in an event entry. An omitted array and an
empty array both mean that the saved plant has no scenario events.

On validation and save, the service shall qualify every candidate event
against the candidate model shape, parameters, enabled state, and scalar
actuator capability. A successful save shall replace exactly the persisted
`[[plant_events]]` entries whose `plant` equals the candidate plant ID, using
the candidate array's displayed order for equal-time events. It shall preserve
all events owned by other plants. In `simulation.toml`, the service shall write
the enclosing plant ID into each corresponding global `[[plant_events]]`
entry; `scenario_events` is a request representation and shall not be written
as a field inside `[[plants]]`.

`plant.read` shall continue to return the complete global ordered event list in
`saved_configuration`. The extension shall associate those entries with their
owning saved plant when constructing an edit draft. It shall never attach an
event belonging to another plant or silently reorder equal-time entries.

Validation shall complete before the revision-guarded write. A successful save
shall be atomic: readers observe either the complete prior file or the complete
new file. Existing comments, unrelated sections, and ordering shall be
preserved except where the requested plant or its ordered events must change.
An invalid or failed save shall leave the exact prior bytes unchanged.

A successful remove shall delete exactly the persisted `[[plants]]` entry
whose `id` equals `plant_id` and every persisted `[[plant_events]]` entry whose
`plant` equals that ID. It shall preserve all other TOML content and ordering,
return the removed ID in `affected_plant_ids`, and never change active runtime
state. A missing plant shall return `plant_not_found` without writing bytes. A
stale revision shall return `revision_conflict` with the current revision and
current saved descriptor when present, without writing bytes.

### 20.3 Saved versus active truth

The runtime shall retain the configuration revision activated at startup.
Saving while a runtime is active shall never hot-replace the plant model. If
the saved and active revisions differ, the response shall set
`restart_required = true`, retain the prior `active_configuration_revision`,
and identify affected plant IDs. It shall not report the edited plant as
running under the new configuration.

When no active revision can be proven, `active_configuration_revision` shall
be absent and the status shall not infer it from the current file. A disabled
or removed saved plant may remain active until restart; the response shall
report that mismatch explicitly.

### 20.4 Runtime status

A successful `plant.status` shall identify the active runtime, active
configuration revision, simulation clock, deterministic random identity, and
one status entry per active plant. Each plant status shall contain its stable
ID, model kind, operating state, simulation time, current command, applied
input, ideal output, measured feedback, published feedback, actuator flags,
active uncertainties/dropouts, pending-event count, substep information, and
current fault when present. Numeric vectors shall retain full runtime
precision and declared channel order.

The VS Code simulator answers `plant.status` and `fleet.topology` as the runtime
binary does: its simulation carries the content revision of the project's
`simulation.toml` as its active configuration revision, and its control server
reports the health of the I/O drivers the program loaded, after every reload. A
running simulator's plants and simulated I/O are therefore shown live, not as
configured.

If the runtime, active revision, or requested plant cannot be observed,
`plant.status` shall return `status_unavailable` with a stable reason code. The
unavailable result shall contain no numeric values, samples, sample timestamps,
simulation time, healthy/running state, or cached prior status. The extension
shall clear any previously displayed live values when it receives this result.

### 20.5 Trace snapshot and clear

`plant.trace.read` shall require a plant ID and shall return one immutable
snapshot containing capacity, retained-row count, dropped-row count,
configuration digest, deterministic random identity, runtime version, and
rows ordered by simulation time and runtime sequence. Every row shall retain
the plant ID, model kind, simulation time, state, command, applied input, ideal
output, measured feedback, queued/published feedback, and fired events exactly
as provided by the runtime. Reading or exporting a snapshot shall not alter
the buffer or simulation state.

If trace collection is disabled, the operation shall return `trace_disabled`
and no rows. If status is unavailable, it shall return `status_unavailable`
and no cached rows.

`plant.trace.clear` shall require `operation_id`, plant ID, and the active
configuration revision observed by the caller. A revision mismatch or unknown
plant shall fail without clearing any rows. Success shall clear only that
plant's retained rows and dropped-row counter. It shall not change plant state,
simulation time, event state, random-generator state, other plants' traces, or
saved configuration. The result shall report the cleared plant ID, prior row
count, and prior dropped-row count.

### 20.6 Extension decoding and recovery

The extension shall decode the contract before updating graph or drawer state.
It shall reject unsupported schemas, unknown variants, wrong-project payloads,
missing revisions, malformed values, and status responses whose claimed
availability contradicts their fields. Rejection shall produce `Status
unavailable`, clear stale live data, preserve unsaved edits, and leave the last
confirmed saved configuration intact.

The extension shall use the returned current revision and descriptor after a
revision conflict to offer reload/reconcile behavior. It shall never retry a
save against a newer revision automatically and shall never convert simulation
evidence into physical-plant evidence.

The extension host shall own a typed plant service boundary. It shall obtain
saved topology from `plant.read` even when the project runtime is stopped. When
the owning runtime is reachable, it may additionally request `plant.status`
using the project identity established by the read response. The webview shall
receive only the normalized plant projection, never a runtime socket, command
process, filesystem path, or undecoded service payload.

Normalization shall preserve the saved descriptor and configuration revision
as the node's identity source. Current status may augment a saved plant only
when the project ID and plant ID match. A missing, rejected, malformed, stale,
or wrong-project status shall leave the saved plant visible, clear all prior
live samples, and mark live status unavailable; it shall not remove the saved
node or invent a running state. A runtime status entry without a matching saved
descriptor shall not create a canvas plant node.

### 20.7 Stopped-project command session

The shipped runtime executable shall expose `trust-runtime plant serve
--project <root>`. It is a local, stopped-project JSON-lines session used by the
extension host; it shall not start PLC execution or a listener. Each stdin line
shall be one closed control request containing `id`, `type`, and `params`. Each
stdout line shall be the corresponding standard control response. Diagnostics
shall use stderr so they cannot corrupt the protocol stream.

The process shall own one plant operation service for its lifetime. Therefore
revision guards and `operation_id` replay/conflict semantics apply across all
lines in that session. The extension shall maintain at most one command session
per canonical project and shall not assume a replay cache survives process
exit. File revisions remain authoritative after a new session starts.

The local process owner's filesystem authority is the command session's access
boundary; the session does not accept or mint runtime bearer tokens. It shall
apply the same closed request validation, project identity, saved-file,
revision, atomic-write, error-envelope, and decoder-facing result semantics as
the runtime control surface. `plant.status`, `plant.trace.read`, and
`plant.trace.clear` shall return structured `status_unavailable` without live
values or side effects. EOF shall terminate the session normally. The extension
sends the plant editor's requests (`plant.capabilities`, `plant.edit` and
`plant.signals`) through this session, so the editor works while the runtime is
stopped.

## 21. Saved-brick removal interaction

A persisted simulated-plant brick shall use the same Remove action,
confirmation interaction, Cancel action, danger styling, keyboard behavior,
and focus behavior as the other persisted hardware-canvas bricks. An unsaved
plant draft shall not show Remove.

The first Remove activation shall only reveal confirmation and shall not write
configuration. Cancel shall close confirmation and restore focus to Remove.
Confirm remove shall invoke `plant.remove`. On success, the drawer shall close,
the plant brick shall disappear after the normal canvas refresh, and focus and
selection shall return to its owning runtime brick. On failure, the plant brick
and open drawer shall remain and the error shall be shown. This interaction
shall not add enable, disable, start, stop, or other lifecycle controls.
### 21.1 Robot-joint runtime defaults

For `robot_joint`, `gravity_torque_nm`, `viscous_friction_nm_s_per_rad`, and
`coulomb_friction_nm` are optional runtime parameters and default to `0` when
omitted, as defined by Spec46 §7.9. The editor shall label each blank field
with its `0` default, keep the draft valid when the field is blank, and preserve
the omission when saving unless the user enters a value. `inertia_kg_m2` and
`friction_smoothing_rad_s` remain required.

### Cascaded gravity-tank model

The model selector includes `gravity_tank_cascade` as **Cascaded gravity tanks**.
It exposes one inlet flow command in m3/s, upstream/downstream level feedback
ports in metres, two separate initial-level fields, and the shared area/outlet
parameters from Simulation Specification46 section7.10. The starting point is
area .5m2, discharge coefficient .020 and levels [.25,.25]. Its diagram shows
two freely discharging tanks in cascade. Editor support does not bypass runtime
configuration validation or imply physical commissioning.
