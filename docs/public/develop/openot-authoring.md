# OpenOT Attribute Authoring

truST supports the OpenOT authoring layer as an experimental compiler/runtime
integration. The engineer marks ordinary Structured Text declarations with
`{attribute 'oot' := ...}`. The compiler validates those attributes, lowers them
to a hidden OpenOT producer function-block call path, and the runtime can publish
the encoded records to the OpenOT shared-memory ring.

The OpenOT vocabulary and wire/definition/document contracts live in the
`open-ot-ref` workbench. This page documents the truST integration points: what
the editor offers, what the compiler accepts, what gets generated, and how the
runtime publishes it.

## Authoring Surface

Write normal ST logic. Put OpenOT meaning on the declaration, not at each logging
site:

```iecst
TYPE E_ReactorStep : (Idle := 0, Filling := 1, Mixing := 2, Draining := 3, Done := 4) END_TYPE

PROGRAM Main
VAR
    BatchStarted : BOOL {attribute 'oot' := 'message', 'template' := 'batch started'};
    Step         : E_ReactorStep {attribute 'oot' := 'state', 'category' := 'process'} := Idle;
    Level        : REAL {attribute 'oot' := 'value', 'unit' := 'L', 'deadband' := '0.5'};
    BatchCount   : DINT {attribute 'oot' := 'value'};
    HighPhAlarm  : BOOL {attribute 'oot' := 'alarm', 'class' := 'alarm', 'severity' := '900'};
END_VAR
```

There should be no `Op :=`, `Execute :=`, or `OOT_Log*` calls in the user
program. Those are internal lowering details.

## Kinds

| Kind | Declaration type | Emits | Trigger |
| --- | --- | --- | --- |
| `value` | `BOOL`, integer widths, `REAL`, `LREAL`, or bounded `STRING` | `ValueChanged`; with `audit := 'true'`, `ParameterChange` | value changes, respecting sampling/deadband for supported `REAL` policies |
| `state` | enum type | `StateTransition` | enum value changes |
| `alarm` | `BOOL` | `ConditionActive` / `ConditionCleared` | FALSE->TRUE / TRUE->FALSE |
| `message` | `BOOL` | `Message` | FALSE->TRUE |
| `condition` | `BOOL` | condition lifecycle events | FALSE->TRUE |
| `batch` | enum type | `BatchEvent` | enum value changes |
| `recipe-loaded`, `recipe-approved`, `material-addition` | `BOOL` | recipe/batch/material events | FALSE->TRUE |
| `operator-action`, `operator-login`, `operator-logout`, `security-failure`, `e-signature` | `BOOL` | operator/regulated/e-signature events | FALSE->TRUE |

Unsupported or unbounded value types are compile errors; truST does not silently
coerce them to `DINT`. Audited string values and audited `actor`/`reason`
bindings must use explicit `STRING[n]` widths so the compiler can prove the
record fits the producer buffer.

## Regulated and Lifecycle Attributes

Condition lifecycle commands are companion `BOOL` variables that reference a
parent alarm:

```iecst
OperatorName : STRING[32] := 'operator-a';
HighPhAlarm  : BOOL {attribute 'oot' := 'alarm', 'class' := 'alarm', 'severity' := '900'};
AckHighPh    : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'acknowledge', 'by' := OperatorName};
```

Activation-scoped commands such as `acknowledge`, `confirm`, `shelve`,
`unshelve`, `comment`, and `reset` use the producer's live alarm correlation id
and fail closed if no activation is live. Condition-scoped commands such as
`suppress`, `unsuppress`, `out-of-service`, `in-service`, and
`priority-changed` emit without a correlation id.

Batch/recipe/operator events bind typed fields from normal declarations:

```iecst
ActionId : UDINT := UDINT#6001;
OperatorName : STRING[32] := 'operator-a';
Action : BOOL {attribute 'oot' := 'operator-action', 'action' := ActionId, 'actor' := OperatorName, 'auth' := 'Granted'};
```

`e-signature` attests a deterministic single-event OpenOT variable in the same
source. The compiler assigns hidden attestable ids and the producer writes the
attested event's emitted sequence into `signedEventSeq`:

```iecst
SignAction : BOOL {attribute 'oot' := 'e-signature', 'action' := ActionId, 'actor' := OperatorName, 'meaning' := 'Approved', 'attests' := Action};
```

The signature emits after other generated OpenOT calls in the scan, so it can
attest a same-scan target. It fails closed if the target has not emitted in the
current run/epoch.

## State Categories

Use `category := 'process'` for machine-local equipment or process states. This
is also the compiler default when `category` is omitted from a `state`
attribute.

Use `category := 'mode'` for machine-local operating modes.

Use `category := 'procedural'` only with a named procedural model:

```iecst
Step : E_S88Step {attribute 'oot' := 'state', 'category' := 'procedural', 'model' := 'ISA-88'};
```

`model` without `category := 'procedural'` is rejected. `category :=
'procedural'` without a `model` is also rejected.

## Defaults

| Kind | Minimal valid attribute | Default behavior |
| --- | --- | --- |
| `value` | `{attribute 'oot' := 'value'}` | emit on change; no unit; no deadband |
| `state` | `{attribute 'oot' := 'state'}` | `category := 'process'` |
| `alarm` | `{attribute 'oot' := 'alarm'}` | `class := 'alarm'`, `severity := '800'` |
| `message` | `{attribute 'oot' := 'message'}` | template text defaults to the variable name |

The VS Code code action inserts these minimal valid attributes. Attribute
completions offer optional keys such as `unit`, `deadband`, `severity`, and
`template`. Inlay hints show the record that a tagged variable emits.

## Validation

OpenOT attributes are validated by HIR diagnostics and therefore appear in the
same editor/compiler diagnostic flow as other ST issues.

Rejected cases include:

- unknown OpenOT kinds, keys, categories, classes, or models;
- invalid condition/recipe/operator/e-signature event values or field bindings;
- `model` used without `category := 'procedural'`;
- `category := 'procedural'` without a model;
- severity outside `1..=1000`;
- empty `unit` or `template`;
- non-numeric `deadband`;
- `deadband` on non-`REAL` values;
- `value` on unsupported types;
- `e-signature` attesting an alarm, condition command, another signature,
  cross-source target, unknown variable, or more than 32 distinct targets in one
  producer instance.

## Generated Runtime Path

For attributed programs, truST instruments the source before bytecode generation.
The generated program contains:

- a hidden `OotProducer : OPENOT_Producer` instance;
- hidden source-time inputs used by the runtime to stamp records with Unix
  nanoseconds;
- per-variable previous-value state for change/edge detection;
- calls into the OpenOT producer FB with the internal operation code and typed
  payload fields.

The generated producer is the runtime telemetry source. Configure
`runtime.openot` with `source = "st-fb"` and point `producer_instance` at the
generated instance:

```toml
[runtime.openot]
enabled = true
path = "openot.shm"
capacity = 4096
fence_mode = "fenced"
source = "st-fb"
producer_instance = "Main.OotProducer"
```

See [runtime.toml](../reference/config/runtime-toml.md#runtimeopenot) for all
OpenOT runtime options, including the proof-only unfenced mode.

## Definition File And Resolution

The compiler-side OpenOT model also generates the definition file that maps
numeric ids back to meaning:

- value ids to names, data types, units, and deadbands;
- state-machine ids to enum names and enum members;
- condition ids to names, classes, and severities;
- message-template ids to template text.
Batch/recipe/material/operator/e-signature records currently carry raw ids and
strings for their bound fields; the reserved definition tables are populated by
future vocabulary slices.

The shared-memory ring carries encoded OpenOT records. Consumers resolve those
records with the generated definition file into the document-format JSON used by
the OpenOT workbench.

## Reference Example

The canonical live example is in the sibling `open-ot-ref` checkout:

- `examples/reactor/Reactor.st`
- `examples/reactor/openot-definition.json`
- `examples/reactor/batch-log.json`
- `examples/reactor/batch-log.txt`

The trust-platform gate that compiles and runs that example is:

```sh
cargo test -p trust-runtime --test openot_telemetry openot_telemetry_authoring_showcase_renders_typed_audit_log -- --nocapture
```

On small ARM development hosts, run this gate on the remote builder rather than
locally.
