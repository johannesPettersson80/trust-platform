# Embedding the runtime

A program runs as a `CompiledRuntime`: the project's sources compiled into a program image for
this computer's CPU (specification 44), admitted against the computer (specification 44 §10.2) and
run scan by scan with the cycle every truST device runs (specification 44 §8.8). The PLC function
set is unchanged. Specification 11 §6.13 defines hosted management behavior.

The bytecode engine and its Rust API are removed: `Runtime`, `Runtime::from_bytes`,
`CompileSession::build_runtime` and the other `build_runtime*` and `build_bytecode*` methods,
`RuntimeBuilder::build`, `apply_bytecode_module`, `storage()` and the STBC formats. Code that used
them builds a program image and loads it, as below.

## Build and load a program

`trust_runtime::native::build` compiles sources into an image with the compiler toolchain
(specification 44 §4.6). `Toolchain::find` uses the folder that `TRUST_TOOLCHAIN` names, else the
installed bundle; `trust-runtime toolchain install` installs it. `CompiledRuntime::load` admits
the image and leaves the program unstarted: a restart starts it, cold, or warm from its retained
values.

```rust,no_run
use trust_runtime::harness::SourceFile;
use trust_runtime::native::{build, Options, Target, Toolchain};
use trust_runtime::value::{Duration, Value};
use trust_runtime::{CompiledRuntime, RestartMode};

const SOURCE: &str = "
PROGRAM Main
VAR
    count : INT := 1;
END_VAR
count := count + 1;
END_PROGRAM

CONFIGURATION Plant
RESOURCE Cpu ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
";

let toolchain = Toolchain::find()?;
let target = Target::this_cpu().expect("a CPU that runs program images");
let work = std::env::temp_dir().join("trust-embedding");
let built = build(
    &[SourceFile::with_path("src/main.st", SOURCE)],
    &Options { target, hooks: true },
    &toolchain,
    &work,
)?;
let mut runtime = CompiledRuntime::load(built.image, "Cpu")?;
runtime.restart(RestartMode::Cold)?;
runtime.reset_task_timing(Duration::ZERO);
// The 10 ms task is first due at 10 ms (specification 10 §4.3).
runtime.set_current_time(Duration::from_millis(10));
runtime.execute_cycle()?;
assert_eq!(runtime.read_variable("P.count")?, Value::Int(2));
# Ok::<(), Box<dyn std::error::Error>>(())
```

`Options::hooks` builds a debug image, whose statements the debugger can stop at; without the
hooks the build is a release image (specification 44 §13.2). `trust-runtime build` builds the same
image, signs it (specification 44 §5.11) and writes it as `program.tpi`.

## Variables and the process image

| Operation | Use |
|---|---|
| `read_variable(name)` | The value as the last scan left it. A name is a global, a program instance's variable (`P.count`), or a program variable by its name alone when exactly one program declares it. |
| `write_variable(name, &value)` | Writes between scans; an untyped integer takes the variable's type (specification 44 §13.3). |
| `find_variable(name)` | Resolves a name as `read_variable` does, without reading. |
| `read_io(&address)` / `write_io(&address, value)` | The process image at an address such as `IoAddress::parse("%QX0.0")?`; a write lands between scans, as a simulation writes its inputs. |
| `snapshot()` | A copy of the program's state as the last scan left it, for other threads: `value(name)`, `variables()`, `programs()`. |

## Online change and restarts

`apply_online_change_bytes(&image)` changes the running program to the program of another image
(specification 44 §12): the new program takes the running program's values and task schedule, the
drivers and the retain file. A refusal leaves the running program as it was.

`restart(RestartMode::Cold)` gives every variable its starting value; `restart(RestartMode::Warm)`
keeps the retained values the program holds (IEC 61131-3 §6.5.6.1).

## Retained values

```rust,no_run
use trust_runtime::retain::FileRetainStore;
use trust_runtime::RestartMode;
# fn demo(runtime: &mut trust_runtime::CompiledRuntime) -> Result<(), Box<dyn std::error::Error>> {
runtime.set_retain_store(Some(Box::new(FileRetainStore::new("retain.bin"))), None)?;
runtime.save_retain_store()?;
// A warm restart from the store restores the saved values; a cold one starts every variable.
runtime.restart_with_retain_store(RestartMode::Warm)?;
# Ok(())
# }
```

A computer writes the retain file in the format of specification 11 §6.7, the format the bytecode
engine wrote, so values that the old runtime saved restore (specification 44 §8.8, §15).

## Tests

`trust_runtime::harness::TestHarness` builds sources with the same toolchain and drives the
program for tests: `from_source` and `from_sources`, `set_input` and `get_output` by name (a
field `.f` and an element `[i]` within a variable too), `cycle` and `run_cycles`, `advance_time`,
`restart`, `reload_source` for an online change, `bind_direct` with `set_direct_input` and
`get_direct_output` for process-image addresses, and `get_access` and `set_access` for
`VAR_ACCESS` declarations (specification 10 §4.1.2). `runtime()` and `runtime_mut()` give the
`CompiledRuntime`.

## Deployment records

A deployment record (specification 34 §5.1.1; specification 44 §10.1) names the image a device
runs. `trust_runtime::deployment::image_record(project, &image)` gives the encoded record of an
image with the configuration identity of the project's `runtime.toml` and `io.toml`, as
`trust-runtime build` writes it to `deployment-record.bin`. `trust_runtime_core::deployment`
holds the record (`DeploymentRecord::decode`, `encode`), the device descriptor
(`DeviceDescriptor::hosted()` for a computer) and `admit`, which checks a record against an image
and a descriptor before any state exists:

```rust,no_run
use trust_runtime::runtime_core::deployment::{admit, DeploymentRecord, DeviceDescriptor};
# fn demo(project: &std::path::Path, image: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
let bytes = trust_runtime::deployment::image_record(project, image)?;
admit(&DeploymentRecord::decode(&bytes)?, image, &DeviceDescriptor::hosted())?;
# Ok(())
# }
```

A malformed record fails with `RuntimeError::DeploymentRecordInvalid` and a refused deployment
with `RuntimeError::DeploymentRefused`.

## Strings

A `STRING` or `WSTRING` without a length is `STRING[80]` or `WSTRING[80]`, and a declared length
counts UTF-8 bytes or UTF-16 code units (`docs/IEC_DECISIONS.md` 2026-10-17). In the HIR,
`TypeId::STRING` is `Type::String { max_len: Some(80) }`, and
`TypeRegistry::register_string_with_length(80)` returns `TypeId::STRING` itself.

| Item | Use |
|---|---|
| `trust_runtime_core::value::string_capacity_cut(text, wide, capacity)` | The byte length of the longest prefix of whole characters within `capacity` UTF-8 bytes, or UTF-16 code units when `wide`; `None` when the text fits. |
| `size_of_value`, `size_of_type` | Count the UTF-8 bytes of a STRING and two bytes per UTF-16 code unit of a WSTRING. `size_of_type(TypeId::STRING, ..)` is 80 and `size_of_type(TypeId::WSTRING, ..)` is 160. |
| `RuntimeError::StringCapacityExceeded` | A string function or conversion whose result would be longer than 65,535 bytes or code units. Its stable code is `runtime_string_capacity_exceeded`. |
| `trust_runtime_core::value::{check_string_result, DEFAULT_STRING_CAPACITY, MAX_STRING_CAPACITY}` | The check of a text result against the maximum, and the two capacities. |
| `trust_hir::types::{checked_string_length, StringLengthError, DEFAULT_STRING_LENGTH, MAX_STRING_LENGTH}`, `SymbolTable::register_string_type` | Check a declared length, 1 to 65,535, and register the type of `STRING[n]` or `WSTRING[n]`, which is the built-in type for 80. |

## Dates and times

`TIME`, `LTIME`, `DATE`, `LDATE`, `TOD`, `LTOD`, `DT` and `LDT` take eight bytes on every target
(specification 44 §6.2).
