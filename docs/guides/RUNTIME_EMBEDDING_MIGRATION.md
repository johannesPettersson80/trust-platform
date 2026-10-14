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

```rust
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

Task/resource/image/binding updates in the full controller use a staged,
resource-only transaction and preserve PLC values and instance identities.
Code or declared-schema replacement follows the existing warm-restart and
retain policy. Failure to prepare or bind a candidate leaves the old controller
installed. Resolve live references again after replacement rather than guessing
instance IDs from artifact indexes.

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
