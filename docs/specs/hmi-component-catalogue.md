# HMI component catalogue and FB-backed authoring

> **Current scope, 6 October 2026:** the user authorized necessary runtime integration
> for declared public capability methods and settings providers. Standard IEC retention
> remains the existing runtime responsibility. No separate settings store/startup hook,
> compiler semantics changes or simulator changes are permitted. Source implementation
> is in progress; no validation or visual acceptance is claimed.

Status: agreed target, 5 October 2026. Implementation authorized by the user; validation pending.
Authority: the user's final component-table discussion, including measurement filtering
inside instruments. This document defines what the builder offers and what must be reused,
adapted or created. It does not claim the new workflow is implemented or visually accepted.

Related: [Plant Library](trust-plant-library.md), [HMI Builder](39-hmi-builder.md),
[HMI checklist](../internal/testing/checklists/hmi-builder-implementation-checklist.md),
[Plant Library checklist](../internal/testing/checklists/trust-plant-library-checklist.md).

## 1. Product boundary

The builder exposes recognisable equipment, instruments, controllers, sequences and
operator panels. A flow instrument is labelled **Flow instrument**, not FlowInput.
Internal ST type names remain available when needed, not the primary palette vocabulary.
Reuse the existing P&ID-style symbols and panels. Do not invent a new drawing catalogue.

FB-backed components package a PLC type/contract with its visual definition, settings,
connections and instance/code-generation rules. Customer components use the same public
format and editor as supplied components. PLC libraries own behaviour; the HMI must not
reimplement alarm conditions, filtering, sequence steps, authority or control algorithms.

Existing Control.PID and other control-library implementations remain. Plant Library
replaces the old Equipment library through the documented migration, not a second permanent
library. Historical drawings/FBs are reuse candidates, not proof of completed new contracts.

Simulation is outside this scope. FBs use normal PLC I/O; runtime chooses hardware or the
existing simulator. No simulator interface, mode flag, physical model, HMI-to-simulation
generation, simulator file changes or simulation-example migration is authorized here.

## 2. Inventory interpretation and source owners

Inventory inspected in the feature checkout `feat/hmi-builder`, base a85185968 plus dirty
work, 5 October 2026. “Have” means source definition exists, not tested/accepted for the
new workflow. All existing FB-backed entries need the shared component contract in §8.
A shared panel is deliberate reuse; there need not be a separate panel file per FB type.
“New” means required target work, not a claim that no reusable algorithm exists elsewhere.

Source owners:
- [Plant FBs](../../libraries/plant/src), [component definitions](../../libraries/plant/hmi/pairs),
  [panels](../../libraries/plant/hmi/panels).
- [Control FBs](../../libraries/control/src), [Alarm FBs](../../libraries/alarm/src),
  [communication mapping](../../libraries/comm/hmi/pairs/link.toml).
- [Drawing catalogue](../../crates/trust-runtime/src/hmi/catalogue/components.json).
- [OSCAT OOP control](../../libraries/oscat/oop/src/control.st).

Current built-in shapes may be TypeScript renderers, not editable SVG definitions. An
existing symbol is an asset to preserve visually; migrate it through the common public
FB/SVG authoring path. Do not pretend a legacy renderer already meets customer authoring.

## 3. Equipment palette

Current FB names below use the Equipment namespace. Adapt means implement the Plant
Library capabilities and common mapping, preserve useful behaviour/assets, and migrate
without breaking existing consumers. Each equipment selection exposes only applicable
settings, hardware-connection status/link, FB references and appearance.

| ID | Customer component | Current FB | Have: symbol / operator panel | Required work |
|---|---|---|---|---|
| EQ-01 | Pump | Pump | pump / pump; pump-card | Adapt |
| EQ-02 | Variable-speed pump | VariableSpeedPump | pump / variable-speed-pump | Adapt |
| EQ-03 | Dosing pump | DosingPump | dosing-pump / vfd | Adapt |
| EQ-04 | Motor | Motor | motor / motor; motor-card | Adapt |
| EQ-05 | Variable-speed motor | VFDMotor | motor / vfd; VFD asset also exists | Adapt to variable-speed motor contract |
| EQ-06 | Reversing motor | ReversingMotor | motor / reversing-motor | Adapt; define clear direction indication |
| EQ-07 | Two-speed motor | TwoSpeedMotor | motor / two-speed-motor | Adapt; define slow/fast indication |
| EQ-08 | Fan | Fan | fan / motor | Adapt |
| EQ-09 | Variable-speed fan | VariableSpeedFan | fan / vfd | Adapt |
| EQ-10 | Blower | Blower | blower / motor | Adapt |
| EQ-11 | Agitator | Agitator | agitator / motor | Adapt |
| EQ-12 | Conveyor | Conveyor | conveyor / motor | Adapt |
| EQ-13 | On/off valve | OnOffValve | valve / valve; valve-card | Adapt |
| EQ-14 | Motor-operated valve | MotorValve | motor-valve / motor-valve | Adapt |
| EQ-15 | Control valve | ControlValve | control-valve / control-valve | Adapt |
| EQ-16 | Damper | Damper | damper / valve | Adapt |
| EQ-17 | Modulating damper | ModulatingDamper | damper / control-valve | Adapt |
| EQ-18 | Cylinder | Cylinder | cylinder / cylinder | Adapt |
| EQ-19 | Heater | Heater | heater / heater | Adapt |
| EQ-20 | Tank | Tank | tank / tank; tank-card | Retain in Plant Library; select existing level measurement |

Pump/motor panels: commands, mode, confirmed feedback, blocking reasons and maintenance.
Variable-speed panels add requested/actual speed. Valve/cylinder panels show commands,
movement, endpoint/position feedback and blocking reasons. Heater shows enable, requested
power, status and protection. Tank shows its selected level, limits and associated alarms.

Tank measurement is a reference to an instrument, not another scaling/filter chain. When
tank and instrument expose alarms for the same measured quantity, each alarm has one
explicit owner. Tank displays associated instrument alarms; distinct process alarms may
belong to the tank. Migration reconciles existing Tank/AnalogInput limit-alarm overlap;
do not silently create duplicate high/low alarms or discard existing identities/settings.
The tank FB is equipment behaviour/metadata, not a simulated physical tank model.

## 4. Instrument palette

An instrument provides input selection, scaling, engineering units, optional filtering,
quality and explicitly configured associated alarms. Numeric filters reuse control/signal
helpers internally. Existing AnalogInput already has FilterTime and a filter recurrence;
when adapting to Control.LowPass, prove equivalent intended response/timing or document a
migration. Do not replace the equation merely to satisfy a naming preference.

Input addresses are selected in Devices & Connections, reached through the instrument's
Connect hardware action. The HMI inspector does not duplicate its channel picker. Digital instrument settings include
inversion and debounce. No instrument knows whether runtime I/O comes from simulation.

| ID | Customer component | Current FB | Have: symbol / panel | Required work |
|---|---|---|---|---|
| IN-01 | Flow instrument | FlowInput | flow-sensor / flow-input | Adapt input/filter/alarm/totalising settings |
| IN-02 | Pressure instrument | PressureInput | pressure-sensor / analog-input | Adapt |
| IN-03 | Temperature instrument | TemperatureInput | temperature-sensor / analog-input | Adapt |
| IN-04 | Level instrument | LevelInput | level-sensor / analog-input | Adapt |
| IN-05 | Analyzer instrument | AnalyzerInput | analyzer / analog-input | Adapt quantity/units and alarms |
| IN-06 | Weigh scale | WeighScale | weigh-scale / weigh-scale | Adapt filtering, tare and stability |
| IN-07 | Level switch | LevelSwitch | level-switch / digital-input | Adapt |
| IN-08 | Pressure switch | PressureSwitch | pressure-switch / digital-input | Adapt |
| IN-09 | Other numeric instrument | AnalogInput foundation | value / analog-input | Configurable instrument definition |
| IN-10 | Other switch/sensor | DigitalInput foundation | status / digital-input | Configurable digital instrument definition |
| IN-11 | Counter | Counter | value / counter | Adapt counted signal and reset permissions |
| IN-12 | Totalizer | FlowInput contains total/batch-total behaviour | flow total display | Reusable totalizer FB and component for other quantities |

Flow instrument may expose an internal totalizer. A standalone totalizer can reference a
measurement; neither placement should accidentally instantiate a second accumulation for
an existing total. Define units/time basis, invalid-quality handling and reset permissions.

Analog/digital output FBs stay available to programmers and actuator implementations.
They are not required palette steps beside every pump or valve. Basic status widgets can
observe output members; authorised controls use the equipment command path, not raw-output
writes bypassing its interlocks. Preserve existing AnalogOutput consumers during migration.

## 5. Controllers and their internal helpers

| ID | Customer component | Current implementation | Have | Required work |
|---|---|---|---|---|
| CT-01 | PID controller | Equipment.PIDLoop executes Control.PID | controller symbol, pid-loop panel/card | Adapt wrapper; select measurement and actuator/demand connection |
| CT-02 | Cascade controller | Equipment.CascadeLoop contains two PIDLoops | controller symbol, cascade panel | Select outer/inner measurements and final actuator; reusable definition |
| CT-03 | On/off controller | OSCAT OOP HysteresisSwitch is reuse candidate | Generic value/state assets | Review/reuse behaviour; define Plant contract and panel |

A controller output connection uses explicit units/range, authority and demand semantics;
a wire does not permit a raw write that bypasses the actuator arbiter. Preserve actual
Control.PID execution/configuration/tuning. Do not instantiate duplicate inner loops when
placing another view of a cascade controller.

| Existing helper | Normal visual presentation | Work |
|---|---|---|
| Control.LowPass | Filter settings within its measurement | Reuse helper; no standalone palette picture required |
| Control.PIDGainScheduler | Controller configuration and active schedule status | Define optional integration and settings presentation |
| Control.SmithPredictorFOPDT | Configuration/status in the arrangement using it | Define optional integration; do not enable by default |
| Control.StaticDecoupler2x2 | Configuration/status of a two-loop arrangement | Define integration, affected loops and settings presentation |

Helpers remain callable from ST. A customer may create a custom view if desired. They
must not become mandatory advanced settings in every basic PID or measurement panel.
No new control algorithm is authorized solely because an HMI picture is missing.

## 6. Groups, sequences and shared panels

| ID | Component | Current FB/service | Existing HMI | Required work |
|---|---|---|---|---|
| GR-01 | Duty/standby group | Equipment.DutyGroup | duty panel; generic status placement | Registered-member contract, group visual and mapping |
| GR-02 | Lead/lag group | DutyGroup foundation only | No dedicated panel | New group behaviour/panel |
| GR-03 | Staged equipment group | No dedicated target-contract FB identified | No dedicated panel | New FB/panel |
| SQ-01 | Sequence instance | Equipment.Sequence foundation | sequence panel; current pair uses text | ISequence/lifecycle, status card, typed dependencies/settings |
| SQ-02 | Sequence overview | No SequenceManager FB yet | Individual sequence panel only | Manager and registered-sequence list |
| SQ-03 | Customer equipment/sequence | Customer FB | Standard compatible panel or customer SVG/panel | Common public definition workflow, no editor special cases |
| SV-01 | Alarm list | Alarm FBs and runtime alarm service | alarm-list, details, actions | AlarmManager contract and binding; reuse state engine |
| SV-02 | Alarm indicator | Existing alarm FBs | status/alarm indication | Select existing alarm or summary |
| SV-03 | Event list | Existing event infrastructure | event-log | EventManager/sink mapping |
| SV-04 | Maintenance overview | Device counters | Individual maintenance controls | Manager and collection panel |
| SV-05 | Trend | Existing history/recording services | trend | Select registered channels; preserve existing history owner |
| SV-06 | Equipment settings | Planned ISettingsProvider | Some individual settings controls | Generated typed form/Apply/readback/persistence status |
| SV-07 | Communication status | Comm.Link | network-link / link panel | Definition mapping, preserve communication owner |

Sequence placement selects a particular reusable FB type, not merely an empty panel.
The component definition declares its required equipment references/settings. A reusable
sequence must already contain its behaviour. No automatic invention of fill/clean/transfer
logic from an HMI drawing; new behaviours are customer/supplied sequence FBs authored once.
Run/valve/position/wait/delay/parallel/call-step helpers belong inside sequence authoring,
not mandatory separate operator-page objects. Simple CASE authoring remains supported.

Manager views select an existing service instance/scope. Where absent, offer one explicit
create action; subsequent views reuse it. Deleting a list does not delete a manager or its
alarms. A settings view references an existing provider, never another equipment instance.
A trend is a view of registered channels; no new Trend FB or historian is required merely
to draw it. Static text and navigation are likewise deliberate exceptions to FB creation.

## 7. Included equipment details and basic page elements

| Included function | Where shown | Required shared work |
|---|---|---|
| Permissives | Cannot-start reasons in equipment panel | PermissiveSet and reusable condition list |
| Interlocks | Active conditions/reasons in equipment panel | Adapt InterlockGroup to registered sets |
| Protection | Trip cause and permitted reset | ProtectionSet and reason presentation |
| Ownership | Current controlling source/owner | Shared arbiter and rejection/owner display |
| Mode | Equipment mode selector/status | Mapping to authorized mode command |
| Maintenance | Hours, starts, cycles | Typed metric mapping |
| Settings | Provider-generated form | Shared definitions/validation, no duplicate HMI rules |

These remain independent reusable PLC contracts where appropriate, but do not require
separate pictures to build a working pump. Optional summary panels reference the same
instances when a customer needs an overview. Do not remove existing interlock assets.

| Basic element | Connects to | Existing asset / work |
|---|---|---|
| Indicator | BOOL or FB member | status; simple compatible picker |
| Value/gauge/bar | Numeric member/measurement | Existing assets; units/range mapping |
| Button | Declared authorized FB command | Existing button assets; new command mapping |
| Setpoint entry | Declared demand/settings contract | Existing input assets; typed mapping |
| Mode selector | FB mode command/status | Existing asset; mapping |
| Text/image | Static content or supported member | Existing assets; preserve editing |
| Navigation button | Page/panel | Existing page-button; target mapping |
| Pipe | Drawing connection between ports | Existing pipe; direct manual routing improvement |
| Sidebar/header | Shared panel definition | Existing primitives; agreed assignment/space reservation |

Other existing drawings (vessel, heat exchanger, filter, crusher, cyclone, screen,
mechanical/motion and process illustrations) are preserved. They are not advertised as
code-generating equipment until an explicit FB definition exists. Do not invent those
FBs or delete assets to make the catalogue appear complete. Map future motion components
to actual existing motion contracts. Decoration creates no hidden FB.

## 8. Common component definition and explicit generation

Each definition has a stable ID/version, customer label/category, FB type and compatible
contract version, symbol/panel references, optional compact view, relative read/command
mappings, typed settings metadata references, required FB dependencies and normal-I/O
terminals. Distinguish drawing ports from signal/dependency terminals. Include execution
and registration ownership metadata; not arbitrary code snippets bypassing native types.
Compiler metadata owns PLC member/type information. Settings provider owns validation.
No second handwritten signal/type catalogue. Reuse existing native parser, binding service,
document transactions and shared renderer. Definitions travel with their library/project.

Placement: create a new instance or use an existing instance. New-instance naming defines
the PLC instance once; no independent equipment-name field. An existing FB may have many
views. Identity/instance data belongs to the PLC/project model; page files store visual
references. Copy/paste defaults to another view of the same instance; creating independent
equipment is explicit and receives a new identity. Copying a whole page does not multiply
the plant. Deleting a view never silently deletes PLC code, I/O, alarms or recordings.

One instance's filter/settings/alarm configuration is shared across all its views. Selecting
an existing incompatible FB refuses with a useful type explanation. Renames update references
through existing project services and preserve history identity.

Build selection exposes instance, relevant settings, required equipment references, hardware
connection status/action, appearance and optional On click. Do not display generic irrelevant decimals/
limits on a pump. Filter settings appear on the instrument that owns the filter. Live PLC
settings changes remain explicit operations, distinct from editing project defaults.
Build stays static; Preview is deliberate and cannot write PLC commands by default.

### Explicit generation — agreed 6 October 2026

This contract supersedes earlier continuous PLC-source synchronization requirements.
Normal drawing, selection, configuration and save operations edit the design only. They
must not generate ST, compile a PLC project, instantiate a runtime or execute PLC scans.
The editor reads declarations and component metadata through the existing language services.
Computed values that cannot be established from declarations remain unknown; never execute
customer code to discover settings, alarms or equipment capabilities.

**Create PLC code** is an explicit project action. It consumes the current design, including
unsaved pages, shared instance configuration and hardware mappings, and generates typed FB
declarations, configuration, dependency assignments, execution calls and service registration.
All pages share one instance identity: two pictures of a pump generate one pump. Generation
reports missing mandatory choices with their locations; it must not invent process rules,
I/O addresses, task periods or sequence behaviour. A missing program/execution target is an
explicit generation choice, not a reason to prevent drawing an unfinished plant.

**Create HMI** is the reverse explicit project action. It reads declared PLC FB instances,
matches installed component definitions and creates editable pictures with instance bindings.
It reuses existing views, preserves hand-arranged pages and never duplicates equipment on a
repeat invocation. Unsupported/custom FBs without a visual definition are reported; their
PLC source is untouched. Physical pipe routing and plant behaviour cannot be inferred from
FB declarations. Customers supply visual definitions through the same mechanism as truST.

Both actions use normal undoable editor transactions, preserve dirty buffers and reject
conflicting changes. Generated-source ownership and stable identities are recorded in the
project manifest. Update only owned regions/files; preserve customer FB bodies and manual
ST. Repeated generation is idempotent. Deleting a picture never deletes PLC code. Invalid
intermediate ST remains editable and does not block unrelated drawing operations.

Build and Deploy remain separate explicit actions. Neither generation button compiles,
starts, deploys or writes to a running PLC. Existing runtime execution, I/O and simulator
behaviour is unchanged. Generated code must pass normal build checks before deployment;
generation alone does not establish commissioning or physical safety.

### 8.1 Connect hardware — agreed editor handoff

In Build mode, an equipment or instrument instance that has physical I/O exposes a compact
Hardware section in its settings pane:

```text
P101 — Pump
Hardware: Not connected
[Connect hardware]
```

Open the existing Devices & Connections editor, focused on that exact PLC instance and
its declared I/O terminals. For a pump these may be Run command, Running feedback and
Protection trip. The hardware editor owns device/channel selection, mapping, type/direction
checks and conflicts. Do not add a second hardware wiring form or address editor to the HMI.

A terminal that no declared PLC signal fits says which declaration would, as an I/O list does:
`No input of type LREAL is declared yet. Declare one with an address in your program, for
example TT101_Raw AT %IL0 : LREAL;, then press Refresh.` The example is named after the
instance's last name and the member. Its address is in the terminal's direction (`%I` for an
input, `%Q` for an output), at the first address of the size the runtime binds the type to
(IEC 61131-3 §6.5.5): `X0.0` for `BOOL`; `B0` for a byte type and for a `STRING`, which is bound
as the byte window at a byte address; `W0` for a word type and `WCHAR`; `D0` for a double word
type, `REAL` and `TIME`; `L0` for a long word type, `LREAL` and the long time types. A type the
runtime binds at no size, such as `DATE` or a structure, gets the advice without an example;
a member whose type is unknown is taken as `BOOL`. The language server words the help with the
terminal (`trust-lsp.hmiEquipment`, `help`).
FB settings still decide whether optional feedback is fitted; hardware mapping uses that
same definition instead of maintaining another list of required signals.

Status derives from the current project mapping, including supported unsaved document state:
- Not connected: no relevant connections assigned while required terminals remain missing.
- Partly connected: some connections exist but required terminals remain unassigned.
- Connected: all required terminals have valid mappings; unused optional terminals do not
  make the instance incomplete. Button becomes Edit hardware connections.
- Connection issue: mappings exist but are invalid, unresolved or conflicting; link to the
  actual diagnostics. Never display Connected from mapping count alone.

If no physical connections apply, omit the action or show No hardware connection required;
do not show an alarm manager or sequence as incomplete equipment. Hardware mapping status
is not live device health: Connected does not assert that a device is online or commissioned.

The handoff preserves the selected instance, page, zoom and unsaved HMI edits. Returning to
the HMI restores selection and refreshes mapping status without requiring a reload. All
views of P101 share its PLC-project mapping. Existing ST wiring is reused where resolvable;
unresolved/manual wiring is identified, not overwritten or presented as an empty new device.
Changing mappings in the hardware editor follows its ordinary save/Undo rules; navigating
between editors does not implicitly save or deploy anything. An accepted mapping edit
synchronizes the project through the shared authoring transaction.

PLC generation reads these project-owned mappings; the HMI document holds no duplicate
hardware assignments. This does not change runtime I/O sourcing or simulation configuration.

### 8.2 Placement and dependency selection — agreed 5 October 2026

No popup setup wizard. Dropping a component from the catalogue defaults to a new instance
and selects it. Its Build settings show one PLC instance selector with a collision-free
suggested name, for example New: P101, and compatible existing instances. The name defines
the PLC instance, not a second HMI label. Dragging an existing project FB directly creates
a view bound to that instance without proposing another FB.

A new-instance placement records recoverable design intent and a stable instance identity.
Its declaration and execution wiring are written only by Create PLC code. Unresolved
references remain visible and editable. Selecting an existing instance reuses that instance
without creating another declaration. Switching an unreferenced pending instance to an
existing FB removes the pending intent; referenced instances require reference handling.
Dropping, selecting and configuring equipment never runs or deploys PLC code.

Example pump selection (On click is a selectable example, not a change to the agreed
Nothing default):

```text
Pump
PLC instance   [New: P101 v]
Hardware: Not connected
[Connect hardware]
On click       [Nothing v]
```

A sequence exposes only the dependencies and settings declared by its reusable FB/component
definition. Example for a filling sequence, once such a sequence FB exists:

```text
Fill sequence
PLC instance   [New: FillTank1 v]
Equipment
  Inlet valve  [Choose valve... v]     [Pick on page]
  Level        [Choose level instrument... v] [Pick on page]
Settings
  Target level [     ] m
  Fill timeout [     ] s
Hardware connection not required
```

This illustration does not invent a shipped FillTank FB or generate its behaviour from
labels. Sequence code owns the steps. Pickers list compatible saved FB instances and pending
instances in the project design; names are shown without requiring typed variable paths.
Pick on page highlights compatible objects; clicking chooses the underlying FB instance,
not that particular visual. Escape cancels unchanged. Incompatible objects explain the
required capability; duplicate views of one instance do not create duplicate candidates.

Equipment groups use the same selection mechanism with a member list. Prevent duplicate
membership, incompatible capabilities and invalid reference cycles. Declaration metadata
owns labels, units, required/optional dependencies and types; no sequence-specific picker
logic in the editor. Missing references remain located diagnostics through generation.

Acceptance: catalogue drop proposes one new instance, project-FB drag reuses one; existing
selection retains wiring/settings; two views generate one declaration; sequence dependencies
resolve by picker or page selection including pending objects; cancel/Undo preserves state;
group members deduplicate; save/reopen restores design intent. Create PLC code updates only
owned source. One Undo restores the complete explicit generation transaction. Execute these with CAT-08/10 in the single final validation batch.

### 8.3 Remote engineering and explicit deployment — agreed 5 October 2026

The engineering computer and runtime are independent. A project may be edited in VS Code
on the Pi while the selected truST runtime runs on another machine. Choose the project runtime
target using existing target services, not a second HMI-specific endpoint registry. If the
workspace itself is remote, its document provider still owns edits; no assumption of Pi-local
paths. Offline editing uses saved project definitions; live observations and deployment need
an authenticated reachable target.

Explicit generation keeps HMI and PLC source in the engineering project consistent.
It never silently changes the running PLC, active HMI bundle or live operating settings.
Save, build and deployment retain their normal separate meaning. Do not start builds/tests
on each UI edit as a consequence of this contract.

Connect hardware focuses the selected runtime's project hardware configuration, not the Pi's
hardware because the editor happens to run there. Authoring connections uses saved metadata;
live connection health remains a separate observation. No automatic target fallback.

Deploy remains explicit and shows the target identity. Layout-only changes may deploy the HMI
bundle alone when compatible. A change creating FBs or changing wiring requires a compatible
program/HMI deployment through the existing deployment contract. Check both sides before
activation; retain the prior working version on failure. Do not label ordinary project sync
as deployment or bypass current authorization/version checks. Reuse spec 39 section 16 rather
than create a second deployment mechanism. The target runtime serves the operator HMI; the Pi
or any authorized panel/browser can display it without running the PLC program locally.

Acceptance covers offline edits, remote workspace URIs, Pi/editor versus target hardware,
loss of target connection, explicit compatible deploy, wrong-target refusal, HMI-only updates
and a new FB requiring program deployment. Routine authoring sends no runtime mutation.

### 8.4 Static settings and execution choices

Declaration metadata MUST include inherited FB input, output and in-out parameters as well
as ordinary fields. Identity, cycle timing and hardware terminals must resolve from the same
HIR declaration model; missing members must not be silently treated as absent capabilities.

Build settings read the typed settings structure selected by the component definition.
Project overrides are editable design values. A library default that has not been overridden
is identified as a declaration default, with its source initializer where available; the
editor must not present a zero-filled type placeholder as the actual default. Computed
provider values remain unknown until Live mode. Choosing an override is explicit.

Authoring limits declaration traversal and settings templates (16 levels / 4096 values),
reporting oversized or recursive configurations before allocating defaults. These editor limits
do not change IEC runtime type or execution semantics.

Authoring checks validate types and structural references without calling Configure or any
settings-provider method. The PLC's existing configuration validation and live settings
provider remain authoritative for operating restrictions. No editor action executes them.

Create PLC code also prepares the HMI display service if needed. It reuses a declared periodic
task; when several are available, the action asks which one to use. It does not invent a task
period or modify an existing period. If a new project has no configuration, the action asks
for the control cycle and creates one task using that explicitly chosen period. Existing
ambiguous or unresolved task assignments remain located diagnostics. Library installation and drawing do not create a Screens program.

## 9. Organisation, implementation and acceptance

Keep hmi/screen.toml and pages/*.toml / panels/*.toml. Explorer handles file organisation;
remove the duplicate internal Pages list. Shared sidebar/panel assignment remains as agreed.
This document refines the Plant Library's HMI scope, not its simulator boundary.

Implement the common definition/instance/generation path once, then register and migrate
the table entries. Adapt the existing assets through the public customer workflow. Every
row requires: resolved FB contract, picture/panel, relevant settings/connections, one-owner
registration/execution, source-generation mapping and public-UI acceptance evidence.

Required cases include:
- Existing P&ID instrument assets retained; labels say Flow/Pressure/Temperature/Level
  instrument, not implementation class names.
- Two views of one pump generate one FB; explicit independent instance generates two.
- Pressure input scaling/filtering exists once inside the instrument; no filter picture
  needed; tank references level without duplicate signal processing or alarm occurrences.
- Actual Control.PID runs through the existing wrapper; no second PID implementation.
- A new customer FB/sequence definition uses the same editor and generation path without
  changes to native managers or hardcoded renderer/editor branches.
- Manager/setting/trend views reuse their providers; deleting/copying a page changes views
  only. Commands remain authorized and acknowledged through existing runtime semantics.
- Generated dependencies and execution are correct; no double execution of contained FBs,
  no overwriting customer code, stale transaction or duplicate registration on regeneration.
- Existing Equipment-dependent examples migrate through compatibility; failed examples block
  old-path retirement. Unrelated control examples and simulator files remain unchanged.
- Real VS Code/runtime images demonstrate coherent panels, usable selection/settings,
  static Build and manually editable connected pipes. Existing asset files alone are not
  visual acceptance; capture and review the actual product.

Author implementation/tests first, then one consolidated final validation batch including
native ST, generator/extension and real rendered journeys. Reuse/deduplicate PL and
FB-SVG/PAGE-PANEL coverage. No per-component test/build/capture loops or automatic retries.
SOLID/KISS/DRY: one instance owner, one type owner, one command/settings owner, shared renderer
and thin definition adapters. No generator-specific control algorithm or parallel library.

## Source completion additions — 6 October

`plant-conditions` presents rows published by registered PermissiveSet, InterlockSet and
ProtectionSet instances. The shared Members editor uses the declared collection capability;
set registration and execution order follow child dependencies. An unconfigured set does not
pretend to be ready. `plant-sequence` presents state, step, waiting/failure reason and the
ISequence commands. A customer pair supplies its concrete sequence FB and status mappings;
the builder does not offer an empty lifecycle as a complete plant sequence. The crusher and
packaging reference projects contain such customer pairs using the same format as stock FBs.
Stock command panels expose explicit Take control and Release control through existing
IControlOwnership. These source additions remain subject to executable and rendered review.

### Bundled library installation

Install immutable library files with initial contents in the same VS Code WorkspaceEdit as
the project dependency edit. Do not open the complete package as dirty text documents: that
triggers a diagnostic refresh for every source and makes New Screen unresponsive. Existing
customer files remain protected by the pre-apply content/version checks. Page and generated
application edits retain their normal editable and undoable document behavior.

### Reusing unchanged PLC metadata

The language server may retain at most two pristine driverless compilations, each with at
most 16 MiB of source text, for HMI checks. A cache key contains the project root plus every
collected source path and its exact edited text; changed or invalid source must never reuse
stale metadata. HMI page geometry alone does not invalidate PLC compilation. A compilation
is borrowed exclusively and returned only if it remained read-only. Any mutable borrow,
including the existing initial-scan alarm inspection, permanently excludes that runtime from
reuse. This is an LSP optimization; it changes no deployed runtime or IEC execution behavior.

The source-bound hmiBindings response has its own two-entry cache (at most 8 MiB per
response). It contains only immutable compile/initial-inspection metadata; no executed
runtime is retained. This prevents repeated context discovery from consuming a pristine
compilation or recompiling unchanged PLC sources during a graphical edit.
