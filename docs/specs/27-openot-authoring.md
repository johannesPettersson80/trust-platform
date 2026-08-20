# OpenOT Authoring Product Contract

Specification ID: `SPEC_OPENOT_AUTHORING_001`

Status: normative truST product specification.

This document defines the OpenOT attribute vocabulary accepted by the truST
Structured Text authoring layer. It is a truST product contract for OpenOT
integration. OpenOT authoring attributes are outside the scope of IEC
61131-3, so this document creates neither an IEC requirement nor an IEC
deviation.

The words MUST, MUST NOT, and MAY are normative.

## 1. Diagnostic boundary

An OpenOT declaration is a variable declaration with an `oot` pragma
attribute. Attribute keys and the `oot` kind value are matched
case-insensitively. Invalid OpenOT authoring input MUST produce
`InvalidOpenOtAttribute`; it MUST NOT be accepted merely because the
underlying Structured Text declaration is otherwise valid.

The supported kind values are:

- `value`
- `state`
- `alarm`
- `message`
- `condition`
- `batch`
- `recipe-loaded`
- `recipe-approved`
- `material-addition`
- `operator-action`
- `operator-login`
- `operator-logout`
- `security-failure`
- `e-signature`

Any other kind MUST be rejected as an unknown OpenOT kind. A key that is not
allowed for the selected kind MUST be rejected as an unknown key for that
kind.

The internal identity keys are `id`, `sourceid`, `valueid`, `machineid`,
`statemachineid`, and `conditionid`. When a kind permits one of these keys,
its value MUST parse as an unsigned 32-bit integer. Event kinds that use bound
field identities MUST reject every internal identity key except `sourceid`.

## 2. Value attributes

### 2.1 Value types and units

A `value` declaration MUST have one of these types: `BOOL`; a signed or
unsigned integer type; `REAL`; `LREAL`; `STRING`; or `STRING[n]`. Other types,
including `WSTRING`, MUST be rejected.

The accepted value keys are `unit`, `deadband`, `quality`, `semanticrole`,
`previous`, `sampling`, `interval`, `audit`, `actor`, `reason`, and `auth`.
The accepted unit values are `1`, `L`, `degC`, `bar`, `rpm`, `s`, `ms`, `kg`,
`m`, and `%`, matched case-insensitively. An empty or unknown unit MUST be
rejected.

`deadband` MUST be a numeric string and is valid only on a `REAL` value.
`quality` accepts `good`, `uncertain`, `bad`, `unknown`, or the numeric codes
0 through 3. `semanticrole` accepts `actual`, `setpoint`, `command`, `count`,
`position`, `status`, or the numeric codes 0 through 5. `previous` accepts
`true`, `false`, `yes`, or `no`.

### 2.2 Sampling

The accepted sampling policies are `on-change`, `deadband`, `periodic`, and
`hysteresis`.

- `periodic` MUST include `interval`, expressed as a positive integer number
  of milliseconds.
- `deadband` and `hysteresis` MUST include a valid `REAL` `deadband`.
- `interval` MUST NOT appear unless sampling is `periodic`.

An unknown sampling policy, a missing policy-dependent key, a zero or
non-numeric interval, or an interval attached to another policy MUST be
rejected.

### 2.3 Audited values

`audit` accepts only `true` or `false`. When `audit := 'true'`:

- `actor` and `reason` are required and MUST reference local `STRING[n]`
  variables where `n <= 96`;
- `auth`, when present, MUST be an authentication-result symbol, a code in
  0 through 4, or a local `UINT` variable;
- `deadband`, `quality`, `previous`, `sampling`, and `interval` MUST NOT
  appear;
- an audited `STRING` value MUST use an explicit `STRING[n]` width where
  `n <= 96`;
- the worst-case record MUST fit the 256-byte producer buffer.

For the buffer calculation, an encoded slot occupies
`4 + payload + padding-to-four-bytes`. The worst-case audited record size is
`52 + 2 * value_slot + actor_slot + reason_slot`, plus 8 bytes when `auth` is
present. A record larger than 256 bytes MUST be rejected.

`actor`, `reason`, or `auth` MUST NOT appear when audit is not enabled.
Pinned `id` or `valueid` values for `value` declarations MUST be unique within
one producer instance.

## 3. State attributes

### 3.1 State category

The accepted state keys are `category` and `model`. The accepted category
values are `process`, `mode`, and `procedural`. Any other category MUST be
rejected.

### 3.2 Procedural model

The accepted procedural models are `ISA-88` and `PackML`.

- `model` MUST NOT appear without `category := 'procedural'`.
- `category := 'procedural'` MUST include `model`.
- A procedural state enum MUST contain only label/value pairs defined by its
  selected model.

The canonical ISA-88 pairs are `Idle := 0`, `Running := 1`,
`Complete := 2`, `Pausing := 3`, `Paused := 4`, `Holding := 5`, `Held := 6`,
`Restarting := 7`, `Stopping := 8`, `Stopped := 9`, `Aborting := 10`, and
`Aborted := 11`.

The canonical PackML pairs are `Idle := 0`, `Starting := 1`, `Execute := 2`,
`Completing := 3`, `Complete := 4`, `Holding := 5`, `Held := 6`,
`Unholding := 7`, `Suspending := 8`, `Suspended := 9`,
`Unsuspending := 10`, `Stopping := 11`, `Stopped := 12`,
`Aborting := 13`, `Aborted := 14`, `Clearing := 15`, and
`Resetting := 16`.

An enum member with a non-canonical label/value pair or a value outside the
unsigned 16-bit range MUST be rejected. A subset containing only canonical
pairs MAY be accepted.

## 4. Alarm and message attributes

### 4.1 Alarm

The accepted alarm keys are `class`, `severity`, and `cause`. `class` accepts
`alarm` or `interlock`. `severity` MUST be an integer from 1 through 1000.
`cause`, when present, MUST name a local variable.

### 4.2 Message

The accepted message keys are `template`, `severity`, and `arg1` through
`arg4`. A present template MUST not be empty. Severity uses the range 1
through 1000. Every present argument MUST name a local variable whose type can
be encoded as an OpenOT value. An unknown argument reference MUST be rejected.

## 5. Condition lifecycle attributes

A `condition` declaration MUST be `BOOL`. Its accepted keys are `of`, `event`,
`by`, `seconds`, `reason`, `comment`, `new-priority`, and
`previous-priority`. It MUST contain `of` and `event`; `of` MUST reference a
local `alarm` declaration.

The event-specific contract is:

| Event | Allowed optional fields | Additionally required fields |
|---|---|---|
| `acknowledge`, `confirm`, `reset`, `out-of-service` | `by` | none |
| `shelve` | `by`, `seconds` | none |
| `suppress` | `reason` | none |
| `unshelve`, `unsuppress`, `in-service` | none | none |
| `comment` | `comment`, `by` | `comment` |
| `priority-changed` | `new-priority`, `previous-priority`, `by` | `new-priority`, `previous-priority` |

`by`, `reason`, and `comment` MUST reference `STRING` or `STRING[n]`;
`seconds` MUST reference `UDINT`; and both priority fields MUST reference
`UINT`. An unknown event, missing required field, field not allowed for the
selected event, unknown reference, or wrong reference type MUST be rejected.

A lifecycle declaration inherits its parent alarm identity. It MUST reject
`id`, `sourceid`, `valueid`, `machineid`, `statemachineid`, and
`conditionid`.

## 6. Batch, recipe, and material events

These event declarations use bound field identities and MUST be `BOOL` trigger
variables, except `batch`, which is the batch-state enum declaration.

| Kind | Required fields | Optional fields and reference types |
|---|---|---|
| `batch` | `batchid: UDINT` | `recipe: UDINT` |
| `recipe-loaded` | `recipe: UDINT`, `version: STRING[n]` where `n <= 96` | `batch: UDINT` |
| `recipe-approved` | `recipe: UDINT`, `version: STRING[n]` where `n <= 96` | `auth: auth result or UINT`, `by: STRING[n]` where `n <= 96` |
| `material-addition` | `batch: UDINT`, `material: UDINT`, `quantity: LREAL` | `unit`: an accepted unit |

The canonical batch-state enum pairs are `Started := 0`, `Completed := 1`,
`Held := 2`, `Resumed := 3`, `Aborted := 4`, and `Paused := 5`. A `batch`
declaration MUST resolve to an enum, and every member MUST be one of these
label/value pairs and fit in an unsigned 16-bit value.

Missing required fields, unknown fields, wrong reference types, non-canonical
batch-state members, and forbidden identity keys MUST be rejected.

## 7. Operator and security events

All declarations in this section MUST be `BOOL` trigger variables.

| Kind | Required fields | Optional fields and reference types |
|---|---|---|
| `operator-action` | `action: UDINT`, `actor: STRING[n]` where `n <= 96` | `context1` through `context4`: `UDINT`; `auth`: auth result or `UINT`; `workstation: STRING[n]` where `n <= 96` |
| `operator-login` | `actor: STRING[n]` where `n <= 96`, `auth`: auth result or `UINT` | `workstation: STRING[n]` where `n <= 96`; `role: UINT` |
| `operator-logout` | `actor: STRING[n]` where `n <= 96` | `workstation: STRING[n]` where `n <= 96` |
| `security-failure` | `actor: STRING[n]` where `n <= 96` | `workstation: STRING[n]` and `reason: STRING[n]`, each where `n <= 96` |

Authentication-result symbols are `Granted`, `Denied`, `NotRequired`,
`Pending`, and `Expired`; accepted spellings for `NotRequired` also include
`not-required` and `not_required`. Numeric codes 0 through 4 are accepted.

Missing required fields, unknown fields, wrong reference types, an unknown
kind such as `program-download`, and forbidden identity keys MUST be rejected.

## 8. E-signatures

An `e-signature` declaration MUST be a `BOOL` trigger and MUST contain:

- `action`, referencing `UDINT`;
- `actor`, referencing `STRING[n]` where `n <= 96`;
- `meaning`, using `Authored`, `Reviewed`, `Approved`, `Verified`,
  `Performed`, `Witnessed`, or a numeric code from 0 through 5;
- `attests`, referencing a local deterministic single-event OpenOT variable.

`auth` MAY contain an authentication-result symbol/code or reference `UINT`.
The attested declaration MUST use the same `sourceid` as the signature. An
omitted `sourceid` has the producer-local default value 1 for this comparison.
A signature MUST NOT attest itself, another `e-signature`, an `alarm`, or a
`condition`. No producer instance may have more than 32 distinct attested
variables.

Unknown targets, non-attestable targets, cross-source targets, self-attestation,
invalid meanings, wrong reference types, and excess distinct targets MUST be
rejected.

## 9. Decision partitions

The authority above defines these mapping partitions for the existing
authoring validation suite:

1. unknown state category rejection;
2. documented core attribute acceptance;
3. unknown value unit rejection;
4. value sampling-policy rejection cases;
5. message-argument and alarm-cause acceptance;
6. condition-lifecycle reference acceptance;
7. batch/recipe/material reference acceptance;
8. operator/security reference acceptance;
9. e-signature reference acceptance;
10. audited-value reference and buffer-budget acceptance;
11. invalid batch/recipe/material rejection;
12. invalid audited-value rejection;
13. invalid operator/security rejection;
14. invalid e-signature rejection;
15. non-canonical batch-state rejection;
16. invalid condition-lifecycle rejection;
17. unknown message-argument rejection;
18. unsupported value-type rejection;
19. model-without-procedural-category rejection;
20. procedural-category-without-model rejection;
21. non-canonical procedural enum rejection; and
22. canonical procedural enum acceptance.

These partitions establish specification authority only. Catalog association,
dynamic reachability, assertion-strength review, and the final broad gate
remain separate verification work.

## 10. Runtime authoring translation

### 10.1 Source and value identity allocation

Each attributed `PROGRAM` MUST produce one independently addressable OpenOT
source. An explicit `sourceid` pins that source identity and MUST be unique
across all programs in the compilation. Programs without an explicit
`sourceid` MUST receive distinct positive identities in deterministic program
declaration order, beginning at 1 while skipping identities already pinned in
the same compilation.

An explicit `id` or `valueid` pins the corresponding value identity and MUST
remain stable when declarations are reordered. Unpinned value identities MUST
be allocated deterministically in declaration order, beginning at 2001 while
skipping pinned identities. A collision MUST reject the complete OpenOT
translation; it MUST NOT silently renumber an explicitly pinned identity.

### 10.2 Definition-file projection

Successful translation MUST produce an OpenOT definition document that:

- carries a 64-character hexadecimal content hash computed over the canonical
  definition content;
- declares the producer's 256-byte maximum record size;
- projects each program as a source with its qualified name, file/program
  hierarchy, path, and allocated source identity;
- projects supported values, state machines, enum sets, conditions, units, and
  canonical event-type schemas without changing their reviewed numeric
  identities;
- uses the canonical unit registry (`L` = 2, `degC` = 3, `kg` = 8);
- records explicit sampling as `periodic:<milliseconds>`, `deadband`, or
  `hysteresis`, while leaving the field absent for the legacy implicit
  deadband form;
- defaults an uncategorized state to the process category and no procedural
  model; and
- retains the raw-definition tables for recipe, batch, material, and operator
  vocabulary as empty arrays until those vocabularies receive separately
  reviewed definition records.

State-machine definitions MUST reference the declared enum set and preserve
the enum's labels and numeric values. Every emitted event-type definition MUST
equal the canonical OpenOT schema for that event type.

### 10.3 Deterministic ST instrumentation

Successful translation MUST instrument each attributed program before
bytecode generation. The generated declarations MUST include one hidden
`OotProducer : OPENOT_Producer`, a disabled-by-default host source-time input,
and the previous-value or edge state needed by the declaration kinds in
sections 2 through 8.

The generated calls MUST preserve the reviewed internal producer operation
partition:

- message = 0, scalar `REAL` value = 6, alarm = 9;
- fixed-width scalar value = 10 and bounded-string value = 11;
- condition lifecycle = 12, procedure/batch/recipe/material = 13; and
- regulated operator/security event = 14.

The operation number is an internal truST/OpenOT ABI, not user-authored syntax.
Generated fixed-width value calls MUST carry the exact type tag, payload
length, and bit-preserving payload for the declared type. Bounded strings MUST
use the string payload input. State calls MUST preserve the HIR enum type,
initial member, and declared numeric member values instead of substituting
ordinal positions.

Sampling, quality, semantic-role, previous-value suppression, lifecycle,
procedure, regulated-event, authentication, and source-time fields MUST be
projected explicitly when their source attributes require them. Alarm calls
MUST be ordered before lifecycle calls that may refer to the alarm activation.
E-signature calls MUST be ordered after other generated event calls in the
same scan.

### 10.4 Compilation fail closed

Runtime authoring validation MUST agree with the HIR OpenOT validation contract
for shared identity and type rules. If validation or definition generation
fails, compilation MUST return the error and MUST NOT build bytecode from the
uninstrumented source. Unsupported value types, duplicate pinned identities,
or an unrepresentable record shape MUST therefore fail the complete
compilation.

## 11. Structured Text producer execution

### 11.1 Producer scan contract

`OPENOT_Producer` MUST treat a false-to-true `Execute` edge as one operation
request and MUST NOT duplicate that request while `Execute` remains true.
Successful operations in one scan MUST retain invocation order. The producer
MUST expose a bounded scan buffer, the number of committed records, a
source-high-water record when requested, and an explicit error state.

The runtime drain MUST consume only fully committed producer records. Clearing
or acknowledging the scan MUST not mutate already published ring records.

### 11.2 Typed wire encoding and source time

Every producer record MUST use the canonical event type, source identity,
sequence, source time, and typed slot keys required by the requested operation.
Fixed-width values MUST be encoded byte-exactly according to their type tag and
payload width. Strings MUST preserve their bounded UTF-8 content.

When the generated host source-time input is enabled, the producer MUST use the
provided Unix-nanosecond value. Otherwise it MUST use its configured source
clock. Source sequence MUST increase monotonically within one producer epoch.

### 11.3 Fail-closed record construction

An operation whose required fields are absent, whose encoded record exceeds
the producer buffer, or whose lifecycle/attestation precondition is not live
MUST fail without publishing a partial record. The previous-value baseline,
activation state, and attestation state MUST advance only when their
corresponding record is successfully committed.

### 11.4 Runtime value sampling

The producer MUST apply the selected value policy against the last successfully
emitted value for that value identity:

- `on-change` MUST emit the first observed value and each later unequal value,
  and MUST suppress an unchanged value;
- `deadband` MUST emit the first observed `REAL` value and a later value only
  when its absolute movement from the last emitted value is greater than the
  configured deadband;
- `periodic` MUST emit the first observed value, a changed value, or an
  unchanged value whose source time has advanced by at least the configured
  interval since the last emission, and MUST suppress an earlier unchanged
  sample; and
- `hysteresis` MUST emit the first observed `REAL` value, suppress values
  inside the band centered on the last emitted value, and emit and recenter
  only after the value crosses above `center + deadband` or below
  `center - deadband`.

A suppressed sample MUST NOT advance the published-record count, source
sequence, periodic timestamp baseline, or hysteresis center. A successful
sample emission MUST advance those applicable baselines exactly once.

## 12. Runtime telemetry publication

### 12.1 Heartbeat and ST-producer publication

When OpenOT telemetry is enabled with the heartbeat source, each successful
runtime cycle MUST append one CRC-valid `Heartbeat` record using the system
source identity, the current run identity, and the cycle sequence/source-time
value.

When the configured source is `st-fb`, the runtime MUST drain the configured
producer instance or instances after program execution. Every complete
producer record MUST be appended in the configured producer order with its
original source identity, event type, source sequence, source time, and slots.

### 12.2 Publication failure and scan integrity

Failure to create, encode, or append any required heartbeat or producer record
MUST fail the runtime cycle. The runtime MUST NOT report a successful scan
after silently dropping a required record. A failed append MUST preserve the
producer scan state needed to diagnose or retry the uncommitted data; it MUST
not acknowledge publication that did not occur.

### 12.3 Multi-source sequence and transition

Multiple configured ST producer instances MAY publish to one ring. Publication
MUST preserve the configured instance order while retaining an independent
monotonic sequence and source-high-water progression for each `(run_id,
source_id)` pair. Definition or epoch transition records MUST remain ordered
with the producer events that establish the transition.

## 13. Event materialization and definition resolution

### 13.1 Value, state, alarm, and message events

The producer MUST materialize value changes, parameter changes, state
transitions, alarm activation/clear, and messages with the canonical event
type and required typed slots. A parameter change MUST carry both previous and
new values plus its reviewed audit context. A state transition MUST carry the
declared previous and new numeric enum values.

### 13.2 Conditions and attestations

Activation-scoped condition events MUST use the live parent-alarm correlation
and fail when no matching activation exists. Condition-scoped suppress,
unsuppress, out-of-service, in-service, and priority-change events MUST be
allowed without a live activation and MUST omit an invented correlation.

An e-signature MUST identify the attested event's committed sequence. Same-scan
attestation MUST observe the earlier phased event; cross-scan attestation MAY
refer to an event committed earlier in the same epoch. A never-emitted target
or a target from an earlier epoch MUST fail closed.

### 13.3 Procedure and regulated events

Batch, recipe, material, operator, login, logout, and security events MUST use
their canonical event types and typed bound fields. Authentication symbols
MUST lower to the codes defined in section 7. Optional fields MUST be marked
present only when their source binding exists; an absent optional binding MUST
not produce a fabricated zero-valued field.

### 13.4 Definition resolution

A consumer supplied with the matching definition MUST resolve numeric value,
state-machine, condition, message-template, enum, and unit identities to the
definition meaning. Raw procedure and regulated-event bindings remain raw
until their reserved definition tables are populated. Resolution MUST NOT
invent a definition entry for an unknown identity.

## 14. Consumer reconciliation

### 14.1 Fenced loss accounting

In fenced mode, the consumer MUST distinguish delivered records from
overwritten records using the ring's committed sequence and loss evidence.
For every `(run_id, source_id)` high-water claim, the reconciled delivered and
lost counts MUST equal the expected total. A CRC-invalid or incompletely
committed record MUST NOT count as delivered.

Unfenced mode is proof-only and MUST NOT be reported as providing the fenced
reconciliation guarantee.

### 14.2 Epoch and cross-process reconciliation

Producer and consumer processes MUST reconcile the same committed ring state
across the shared-memory boundary. Epoch transitions MUST partition source
sequence and attestation state; stale records from an earlier epoch MUST be
identified rather than attributed to the current run. A reconciled report MUST
retain the source/run identity, expected total, delivered count, loss count,
and stale-observation evidence needed to audit the result.
