# Embedding the shared runtime

The hosted `Runtime` owns admitted executable state. `RuntimeBuilder` owns
construction declarations and seeds and cannot execute. The PLC function set is
unchanged; Rust integrations that used mutable installed storage or type-registry
references must use the operations below. Specification 34 §4.3.1 defines the
lifecycle; specification 11 §6.13 defines hosted management behavior.

Use source authoring in the full/development composition:

```rust
use trust_runtime::{harness::CompileSession, value::{RefSegment, Value}};

let mut runtime = CompileSession::from_source(
    "VAR_GLOBAL count : INT := 1; END_VAR \
     PROGRAM Main VAR member : INT := 2; END_VAR END_PROGRAM",
).build_runtime()?;
runtime.write_global("count", Value::Int(7))?;
runtime.write_named_path("Main", &[RefSegment::Field("member".into())], Value::Int(8))?;
assert_eq!(runtime.storage().get_global("count"), Some(&Value::Int(7)));
# Ok::<(), Box<dyn std::error::Error>>(())
```

For manual declarations, use `RuntimeBuilder::register_global` and the program,
function-block, class and interface registration methods. Type definitions,
construction seeds, aliases and image bindings belong to that builder. Consume
it with `build()` only after those inputs are complete. Seeds must fit declared
storage and types; an undeclared raw storage entry does not define a variable.
The resulting controller has no `storage_mut()` or `registry_mut()` getter.

Use saved STBC 3.0 for new compiler-free applications in every hosted composition.
Existing 2.0 artifacts remain supported with their original storage ABI:

```rust,no_run
use trust_runtime::{Runtime, value::{RefSegment, Value}};

let bytes = std::fs::read("application.stbc")?;
let mut runtime = Runtime::from_bytes(&bytes, Default::default())?;
// These destinations must exist in the admitted application's declarations.
runtime.write_global("count", Value::Int(7))?;
runtime.write_named_path("Main", &[RefSegment::Field("member".into())], Value::Int(8))?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

| Previous installed operation | Replacement |
|---|---|
| Raw global assignment | `write_global(name, value)` |
| Raw instance/member assignment | `write_named_path(root, path, value)` or a checked `write_reference` |
| Raw engineering-alias assignment | `write_access(alias, value)`; the alias must be admitted and writable |
| Raw flat/hierarchical process-image mutation | `write_io(address, value)`; extents and bindings remain admitted |
| Raw type/layout/code edit | Prepare an isolated artifact and use the full controller's `apply_bytecode_module`/online-change boundary |
| Live registry mutation for watch parsing | Detached debug parsing types; apply writes through the checked controller |
| Infallible retained snapshot | Exclusive, fallible `retain_snapshot()`; propagate failure rather than save an empty fallback |

Ordinary checked writes do not restart or reinitialize the program. Rejected
requests preserve the destination and do not themselves fault PLC execution.
Remote/debug requests retain their existing queued write or persistent-force
timing and report application rejection separately from queue acceptance.
`retain:` targets resolve retained declarations in their actual globals or
program fields; no second live retain bank is created.

An alias write goes through the same checked owner as a global write. The alias
is declared with `VAR_ACCESS` and must be `READ_WRITE`:

```rust
use trust_runtime::{harness::CompileSession, value::Value};

let mut runtime = CompileSession::from_source(
    "PROGRAM Main VAR member : INT := 2; END_VAR END_PROGRAM \
     CONFIGURATION C PROGRAM P : Main; \
     VAR_ACCESS Engineering : P.member : INT READ_WRITE; END_VAR \
     END_CONFIGURATION",
).build_runtime()?;
runtime.write_access("Engineering", Value::Int(9))?;
assert_eq!(runtime.read_access("Engineering"), Some(Value::Int(9)));
# Ok::<(), Box<dyn std::error::Error>>(())
```

A process-image write takes effect at once, but it is not a persistent override.
Every cycle publishes each bound output again from its PLC variable. Here the
bound `lamp` stays FALSE, so the cycle replaces the written TRUE with FALSE. To
change a bound output for good, change the value the program computes.

```rust
use trust_runtime::{harness::CompileSession, io::IoAddress, value::Value};

let mut runtime = CompileSession::from_source(
    "PROGRAM Main VAR lamp AT %QX0.0 : BOOL := FALSE; END_VAR END_PROGRAM",
).build_runtime()?;
let lamp = IoAddress::parse("%QX0.0")?;
runtime.write_io(&lamp, Value::Bool(true))?;
assert_eq!(runtime.io().read(&lamp)?, Value::Bool(true));
runtime.execute_cycle()?;
assert_eq!(runtime.io().read(&lamp)?, Value::Bool(false));
# Ok::<(), Box<dyn std::error::Error>>(())
```

To drive the program, write an input that it reads. The next cycle executes with
that input, and the program's output follows:

```rust
use trust_runtime::{harness::CompileSession, io::IoAddress, value::Value};

let mut runtime = CompileSession::from_source(
    "PROGRAM Main VAR button AT %IX0.0 : BOOL; lamp AT %QX0.0 : BOOL; END_VAR \
     lamp := button; END_PROGRAM",
).build_runtime()?;
runtime.write_io(&IoAddress::parse("%IX0.0")?, Value::Bool(true))?;
runtime.execute_cycle()?;
assert_eq!(runtime.io().read(&IoAddress::parse("%QX0.0")?)?, Value::Bool(true));
# Ok::<(), Box<dyn std::error::Error>>(())
```

Task/resource/image/binding updates in the full controller use a staged,
resource-only transaction and preserve PLC values and instance identities.
Code or declared-schema replacement follows the existing warm-restart and
retain policy. Failure to prepare or bind a candidate leaves the old controller
installed. Resolve live references again after replacement rather than guessing
instance IDs from artifact indexes.

A declaration change uses the same boundary. The replacement below adds `added`
and changes the code. `count` is RETAIN, so it keeps its value by name.
`transient` is not retained, so it starts again from its declaration. The
logical time is kept, and the replacement code runs in the next cycle.
Specification 34 §11.3 defines which retained graphs migrate. An incompatible
retained object graph is rejected, and the installed controller stays unchanged.

```rust
use trust_runtime::{harness::CompileSession, value::{Duration, Value}};

let mut runtime = CompileSession::from_source(
    "VAR_GLOBAL RETAIN count : DINT := DINT#3; END_VAR \
     VAR_GLOBAL transient : DINT := DINT#4; END_VAR \
     PROGRAM Main count := count + DINT#1; END_PROGRAM",
).build_runtime()?;
runtime.write_global("count", Value::DInt(12))?;
runtime.write_global("transient", Value::DInt(99))?;
runtime.set_current_time(Duration::from_millis(35));
let replacement = CompileSession::from_source(
    "VAR_GLOBAL RETAIN count : DINT := DINT#3; END_VAR \
     VAR_GLOBAL transient : DINT := DINT#4; END_VAR \
     VAR_GLOBAL added : DINT := DINT#7; END_VAR \
     PROGRAM Main count := count + DINT#2; END_PROGRAM",
).build_bytecode_module()?;
runtime.apply_bytecode_module(&replacement, None)?;
assert_eq!(runtime.storage().get_global("count"), Some(&Value::DInt(12)));
assert_eq!(runtime.storage().get_global("transient"), Some(&Value::DInt(4)));
assert_eq!(runtime.storage().get_global("added"), Some(&Value::DInt(7)));
assert_eq!(runtime.current_time(), Duration::from_millis(35));
runtime.execute_cycle()?;
assert_eq!(runtime.storage().get_global("count"), Some(&Value::DInt(14)));
# Ok::<(), Box<dyn std::error::Error>>(())
```

A reference belongs to the installation generation of the storage it was taken
in (specification 34 §11.3). A warm or cold restart and a replacement each
install the next generation; a resource-only update keeps it. A reference of an
earlier generation names no state: `storage().read_by_ref_ref` returns `None`,
and `read_reference`, `write_reference` and `execute_function_block` refuse it
with `RuntimeError::ReferenceLifetime` before anything changes. Resolve a held
reference again after a restart or replacement. The hosted `Runtime` does so for
the references it holds itself: engineering aliases, task function blocks, I/O
bindings and ADS and OPC UA client points.

```rust
use trust_runtime::{error::RuntimeError, harness::CompileSession, value::Value, RestartMode};

let mut runtime = CompileSession::from_source(
    "VAR_GLOBAL count : INT := 1; END_VAR PROGRAM Main END_PROGRAM",
).build_runtime()?;
let held = runtime.storage().ref_for_global("count").expect("declared global");
runtime.restart(RestartMode::Warm)?;
assert_eq!(runtime.read_reference(&held), Err(RuntimeError::ReferenceLifetime));
let current = runtime.storage().ref_for_global("count").expect("declared global");
assert_eq!(current.generation, runtime.storage().generation());
assert_eq!(runtime.read_reference(&current)?, Value::Int(1));
# Ok::<(), Box<dyn std::error::Error>>(())
```

This changes the embedding API:

| Previous | Replacement |
|---|---|
| `ValueRef { location, offset: usize, path }` | `ValueRef { location, offset: u32, generation, path }`: the slot index is 32 bits on every target, and `generation` comes from the storage the reference addresses (`VariableStorage::generation()`). Prefer the storage's `ref_for_*` methods, which set both. |
| `ValueRefView { location, offset, path }` | The view also carries `generation`. `ValueRefView::to_owned()` returns `Result<ValueRef, RuntimeError>`: a slot beyond 32 bits fails with `Overflow`. |
| `DebugControl::enqueue_instance_write(id, ..)`, `try_enqueue_instance_write`, `force_instance`, `try_force_instance` | The same methods take the generation the instance identity was resolved in first, for example `snapshot.storage.generation()`. A request of an earlier generation is rejected at its scan boundary with `write_rejected`. `DebugControl::generation()` gives the installed generation for a bare identity a client sent. |
| `ForcedVarTarget::Instance(id, name)` | `ForcedVarTarget::Instance(generation, id, name)`; `release_instance(id, name)` releases the force in any generation. |

`ValueRef` keeps its size on 64-bit targets (40 bytes) and `Value` keeps its size
on every target.

STBC 1.1 loading requires the full authoring composition's explicit matching
construction context, `CompileSession::build_runtime_from_bytecode`. It preserves
the supplied instruction bodies. Source positions are attached only when the
artifact/source association is verified; providing source text does not replace
the supplied code or certify unrelated debug locations.

Runtime-only excludes source tooling and product services. Full/development
include those services and use the same execution owner. Optional prepared
register plans select an optimization, without changing the IEC function set.
Native H validation and platform qualification are tracked separately in the
portability implementation checklist; these examples do not claim passing tests.


New source applications use eight-byte TIME/DATE/TOD/DT storage and `SIZEOF`
on every platform. Their logical range and precision are preserved. Relative
field offsets are explicit: a field after a TIME at byte zero must start at
byte eight or later unless shared backing through `STRUCT OVERLAP` is intended.
Do not shift offsets or reinterpret old saved bytes automatically.

Explicit 1.1/2.0 artifacts retain four-byte short temporal sizes and their
historic aggregate SIZEOF. A 1.1 import uses matching source construction context;
its admitted construction archive is 3.0 with a legacy-ABI compatibility flag.
The unchanged original artifact may be exported until resource or code metadata
changes. A successful resource update exports the updated admitted compatibility
container; failed updates preserve the previous archives. Old firmware rejects
major 3 before installation and must be upgraded to consume new artifacts.
This is artifact compatibility on every platform, not a different function set.

The external DWORD TIME process-image codec is still unsigned milliseconds with
checked overflow. It is independent of eight-byte internal temporal storage.

Preparation names a refused built-in import. An artifact whose standard-library
call or standard function-block template does not match its built-in contract
(specification 34 §5.2.1) fails with `RuntimeError::ImportUnsupported { import,
reason }` and the stable code `runtime_import_unsupported`. Before, an unknown name
failed with `runtime_profile_unsupported` and a mismatched call or template with
`vm_bytecode_decode`, neither naming the import. A `match` over `RuntimeError` or
`StableErrorCode` without a wildcard arm needs the new variant. The contracts
themselves are public in `trust_runtime_core::stdlib::contracts`.

A deployment record (specification 34 §5.1.1) selects the preparation limits of
one program. `trust_runtime_core::deployment` holds the record
(`DeploymentRecord::for_program`, `encode`, `decode`), the device descriptor
(`DeviceDescriptor::hosted()` for a computer) and `admit`, which checks a record
against a descriptor before any state exists and returns the `PreparationLimits`
to prepare the program with:

```rust
use trust_runtime::harness::CompileSession;
use trust_runtime::runtime_core::deployment::{
    admit, configuration_identity, DeploymentRecord, DeviceDescriptor,
};
use trust_runtime::runtime_core::vm::{PreparationLimits, PreparedModule};

let program = CompileSession::from_source("PROGRAM Main END_PROGRAM").build_bytecode_bytes()?;
let record = DeploymentRecord::for_program(
    &program,
    PreparationLimits::default(),
    configuration_identity(None, None),
)?;
let bytes = record.encode()?;
let limits = admit(&DeploymentRecord::decode(&bytes)?, &program, &DeviceDescriptor::hosted())?;
let prepared = PreparedModule::from_bytes(&program, limits)?;
assert_eq!(prepared.limits(), PreparationLimits::default());
# Ok::<(), Box<dyn std::error::Error>>(())
```

This adds to the embedding API:

| Addition | Use |
|---|---|
| `RuntimeError::DeploymentRecordInvalid`, `RuntimeError::DeploymentRefused`; `StableErrorCode::RuntimeDeploymentRecordInvalid`, `RuntimeDeploymentRefused` | A malformed record and a refused deployment. A `match` over `RuntimeError` or `StableErrorCode` without a wildcard arm needs the new variants. |
| `ImportRefusal::ContractVersion { record, runtime }` | A record names another contract version of an import than the runtime has. |
| `RuntimeBundle::preparation_limits` | The limits of the project's admitted record, or the defaults. A `RuntimeBundle { .. }` literal needs the field. |
| `CompileSession::build_runtime_from_bytecode_with_limits` | Install bytecode with those limits; `build_runtime_from_bytecode` keeps the defaults. |
| `BundleBuildReport::deployment_record`, `trust_runtime::deployment` | What the build wrote, and the hosted record file (`deployment-record.bin`). |

A `STRING` or `WSTRING` without a length is `STRING[80]` or `WSTRING[80]`, and a
declared length counts UTF-8 bytes or UTF-16 code units (`docs/IEC_DECISIONS.md`
2026-10-17). In the HIR, `TypeId::STRING` is `Type::String { max_len: Some(80) }`,
and `TypeRegistry::register_string_with_length(80)` returns `TypeId::STRING` itself.
An STBC string type whose `max_length` is 0, as earlier producers wrote it, has the
capacity 80. This changes the embedding API:

| Previous | Replacement |
|---|---|
| `trust_runtime_core::value::truncate_string_elements(text, max_len)`, which kept `max_len` characters | `string_capacity_cut(text, wide, capacity)`: the byte length of the longest prefix of whole characters within `capacity` UTF-8 bytes, or UTF-16 code units when `wide`; `None` when the text fits. |
| `size_of_value` counted the characters of a STRING and two bytes per character of a WSTRING | It counts the UTF-8 bytes of a STRING and two bytes per UTF-16 code unit of a WSTRING. `size_of_type(TypeId::STRING, ..)` is 80 and `size_of_type(TypeId::WSTRING, ..)` is 160. |

This adds to the embedding API:

| Addition | Use |
|---|---|
| `RuntimeError::StringCapacityExceeded` | A string function or conversion whose result would be longer than 65,535 bytes or code units. Its stable code is the existing `runtime_string_capacity_exceeded`. A `match` over `RuntimeError` without a wildcard arm needs the new variant. |
| `trust_runtime_core::value::{string_capacity, check_string_result, DEFAULT_STRING_CAPACITY, MAX_STRING_CAPACITY}` | The capacity of an STBC string type (80 for 0), and the check of a text result against the maximum. |
| `trust_hir::types::{checked_string_length, StringLengthError, DEFAULT_STRING_LENGTH, MAX_STRING_LENGTH}`, `SymbolTable::register_string_type` | Check a declared length, 1 to 65,535, and register the type of `STRING[n]` or `WSTRING[n]`, which is the built-in type for 80. |
