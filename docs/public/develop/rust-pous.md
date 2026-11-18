# Rust POUs

A project's functions, function blocks and programs can be written in Rust as well as in Structured
Text. Structured Text calls a Rust function block as one of its own: truST declares it to Structured
Text, compiles it into the same program image, runs it in the same scan and debugs, monitors and
deploys it like every other POU. There is no second runtime and nothing loaded beside the program.

You need no Rust of your own: the truST toolchain carries cargo, the `trust-plc` SDK and its macros,
and builds the Rust with the program.

## Start a project with Rust

In VS Code, **+ Create project** asks what the project starts as
([Create A New Project](../start/create-new-project.md)):

- **Structured Text with Rust**: a Structured Text project with a Rust crate in `rust/`, whose
  function block `FILTER_EMA` `src/Main.st` calls.
- **Rust-first**: the crate is the project; its Rust programs run on the tasks of `trust.toml`.

From a terminal, `trust-runtime new --parent <folder> --name <name> --rust` or `--rust-first` writes
the same projects ([trust-runtime](../reference/cli/trust-runtime.md)).

To add Rust to an existing Structured Text project, create the crate yourself in `rust/`:

```toml
# rust/Cargo.toml
[package]
name = "my-plc"
version = "0.1.0"
edition = "2024"

[dependencies]
trust-plc = "=0.42.0" # exactly your truST version
```

```rust
// rust/src/lib.rs
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;
```

The build refuses a crate whose root lacks those two lines, or that names another `trust-plc`
version, and its message names the line to write.

## Write a function block

```rust
/// Exponential moving average of one input: at each call, `y` moves `alpha` of the way to `x`.
#[trust_fb(name = "FILTER_EMA")]
#[budget("15us")]
pub struct FilterEma {
    #[input]
    pub x: f64,
    #[input]
    #[init(0.1)]
    pub alpha: f64,
    #[output]
    pub y: f64,
    primed: bool,
}

impl Body for FilterEma {
    fn body(&mut self, _cx: &mut Cx) -> Result<(), Fault> {
        if self.primed {
            self.y += self.alpha * (self.x - self.y);
        } else {
            self.y = self.x;
            self.primed = true;
        }
        Ok(())
    }
}
```

- `#[trust_fb(name = "...")]` makes the struct a function block with that Structured Text name.
- `#[input]` and `#[output]` fields are its inputs and outputs; `#[init(...)]` gives a starting
  value. A field without them is an internal variable, which Live Values and the debugger still show.
- `body` is its call. It returns `Ok(())`, or a `Fault` that ends the scan as a Structured Text
  fault does.
- `#[budget(...)]` is the longest the call may take (see [Time budgets](#time-budgets)).

Functions, methods and programs work the same way:

```rust
/// The larger of two inputs, limited to a maximum.
#[trust_function(name = "MAX_LIMITED")]
#[budget("2us")]
pub fn max_limited(_cx: &mut Cx, a: i32, b: i32, limit: i32) -> Result<i32, Fault> {
    Ok(a.max(b).min(limit))
}
```

- `#[trust_function]` makes a function. Its first parameter is the call's context, `&mut Cx`, and
  it returns `Result<T, Fault>`.
- `#[trust_method]` on a `pub fn` in an `impl` of a Rust function block makes a method of the block,
  which Structured Text calls as `Filter.METHOD_NAME(...)`.
- `#[trust_program]` on a struct makes a program, which a configuration runs on a task as it runs a
  Structured Text program.

## The generated declarations

truST writes the Structured Text declaration of every Rust POU into `src/generated/rust_pous.st`
(`src/generated/trust/rust_pous.st` in a Rust-first project), marked `{attribute 'rust'}` and
without a body:

```iecst
{attribute 'rust'}
FUNCTION_BLOCK FILTER_EMA
VAR_INPUT
    EN : BOOL := TRUE;
    x : LREAL;
    alpha : LREAL := 0.1;
END_VAR
VAR_OUTPUT
    y : LREAL;
END_VAR
VAR
    primed : BOOL;
END_VAR
VAR_OUTPUT
    ENO : BOOL;
END_VAR
END_FUNCTION_BLOCK
```

The VS Code extension writes the file when the project opens and when you save a `.rs` file or
`trust.toml`; from a terminal, `trust-dev rust declarations` writes it. Commit it with the crate. The
build refuses a project whose file is not what the crate declares, and names the command. The file is
read-only in the editor: change the Rust, not the declaration.

## Call it from Structured Text

```iecst
PROGRAM Main
VAR
    Raw : LREAL;
    Smoothed : LREAL;
    Filter : FILTER_EMA;
END_VAR
Filter(x := Raw, alpha := 0.1);
Smoothed := Filter.y;
END_PROGRAM
```

Completion, hover and checks work on a Rust POU as on any POU, and **Go to Definition** opens its
Rust item.

## Data types

A variable of a Rust POU has one of these types:

| Structured Text | Rust |
| --- | --- |
| `BOOL` | `bool` |
| `SINT` … `LINT`, `USINT` … `ULINT` | `i8` … `i64`, `u8` … `u64` |
| `BYTE`, `WORD`, `DWORD`, `LWORD` | `Byte`, `Word`, `Dword`, `Lword` |
| `REAL`, `LREAL` | `f32`, `f64` |
| `TIME`, `LTIME` | `Time`, `LTime` |
| `DATE`, `TOD`, `DT` and their long forms | `Date`, `Tod`, `Dt`, `LDate`, `LTod`, `Ldt` |
| `CHAR`, `WCHAR` | `Char`, `WChar` |
| `STRING[n]`, `WSTRING[n]` | `Str<n>`, `WStr<n>` |
| `ARRAY[a..b] OF T` | `Array<T, a, n>`, with `n` = b − a + 1 elements |
| an enumeration | a Rust `enum` with `#[trust_enum(name = "...")]` and explicit values |
| a structure | a Rust `struct` with `#[trust_struct(name = "...")]` |
| `REF_TO T` | `Ref<T>` |
| `VAR_IN_OUT` of `T` | a parameter `InOut<T>`, or a function block field `#[in_out]` of type `Ref<T>` |

A type of the project's Structured Text, such as a structure or a function block, is reached through
its binding, `st::POINT` (see [Call Structured Text from Rust](#call-structured-text-from-rust)).

## The rules of Rust in the scan

A Rust POU runs inside the scan, so it keeps the scan's rules. The build checks each one and names
what breaks it:

- **No standard library, no heap.** The crate builds with `core` only; its own tests may use `std`,
  which `#![cfg_attr(not(test), no_std)]` allows.
- **No `unsafe`.** `#![forbid(unsafe_code)]` holds for the crate, and the build refuses a dependency
  whose sources hold `unsafe`, naming the crate, the file and the line.
- **Faults, never panics.** A Rust POU returns `Result<_, Fault>`. Indexing, `unwrap`, `expect`, and
  `/` or `%` on integers can panic in Rust, so use the forms that return a fault instead: `get`,
  `ok_or(...)`, and `trust_plc::checked` (`checked::add(cx, a, b)`, `div`, `rem` and the rest), which
  fault as Structured Text does on overflow and division by zero. Plain `+`, `-` and `*` on integers
  wrap. `rust_fault(cx, number, first, second)` returns a fault of your own, `RustFault`.
- **Time from `Cx` only.** `cx.now()` is the task's logical time, `cx.interval()` its interval and
  `cx.wall_clock()` the wall clock. A Rust POU never waits or sleeps.
- **The same results on every device.** The elementary functions (`sin`, `exp` and the rest) come
  from `trust_plc::math`, never from a target's own library.
- **No recursion, and no `static` that changes.** A Rust POU's state is its fields, which lie in the
  instance like a Structured Text block's variables.
- **No build scripts or procedural macros** in the crate's dependencies; the SDK's own are the
  exception. The build uses `rust/Cargo.lock` as it lies.

## Time budgets

`#[budget("15us")]` declares the longest one call takes. For each task, the build adds up the budgets
of the Rust calls its scan can make, and `trust-runtime build` warns when they exceed the task's
interval, or when a loop calls a Rust POU with no bound. In `trust.toml`, `[timing] profile =
"production"` makes these errors. Structured Text's statement budget does not count Rust code; the
watchdog still ends a scan that overruns its task.

## Call Structured Text from Rust

A Rust POU can call the project's Structured Text functions and function blocks through the
bindings that the build generates in the project's `build/trust_st/`. Name them in the crate's
`Cargo.toml` (`path = "build/trust_st"` in a Rust-first project, whose crate is the project):

```toml
[dependencies]
trust-plc = "=0.42.0"
trust_st = { path = "../build/trust_st" }
```

Then call each Structured Text POU by its name under `st::`:

```rust
#[trust_function(name = "TWICE")]
#[budget("1us")]
pub fn twice(cx: &mut Cx, x: f64) -> Result<f64, Fault> {
    st::SCALE(cx, x, 2.0)
}
```

A Structured Text function block is a struct with its inputs and outputs as fields and its call as
`call(&mut self, cx)`. The Structured Text that Rust calls counts against the scan's statement budget
and stops at its breakpoints. `build/` is generated, so keep it out of version control.

## Rust-first projects

In a Rust-first project the crate is the project: `Cargo.toml` and `src/lib.rs` lie beside
`trust.toml`, which declares what a `CONFIGURATION` declares:

```toml
[rust]
crate = "."

[resource]
name = "MainRes"

[tasks.MainTask]
interval = "10ms"
priority = 1

[programs.MainInstance]
type = "MAIN"
task = "MainTask"
```

truST writes `src/generated/trust/configuration.st` from it, with the declarations, and the build
refuses a project whose file is out of date, as it does for the declarations. `trust.toml` also takes
`[globals]` and `[bindings]` (`"MainInstance.start" = "%IX0.0"`). `runtime.toml`, `io.toml` and the
tools are the same as in every project.

## Heartbeats of services

A service is a program of its own beside the runtime, which reads and writes the runtime's variables
over the control link. Declare it in `trust.toml`:

```toml
[services.recipe]
```

truST then declares the global `recipe_heartbeat : UDINT` in `src/generated/services.st`, which the
service increases while it runs, and a program gates on it with `SERVICE_FRESH`:

```iecst
fresh(HEARTBEAT := recipe_heartbeat, TIMEOUT := T#1s, FRESH => ok);
```

A Structured Text project may hold `trust.toml` with `[services]` alone, without a crate.

## Build, run and debug

A project with Rust POUs is worked like any project: **Compile**, **Start** and **Debug** in the truST
view, `trust-runtime build`, `trust-dev test` and `trust-runtime deploy`. The build compiles the crate
with the toolchain's own cargo and compiler, in a cargo home of its own: your own Rust, its settings
and its caches play no part. The first build of a project compiles the SDK, so it takes longer than
the next.

Live Values and the debugger show a Rust function block's variables, and a fault in Rust names the
Rust POU and the Structured Text statement that called it.

A Rust POU runs on every device a program runs on: the computers, the ESP32-C6 and the STM32U5A9.
The build compiles the crate for each target of the deployment.

## Check the crates for security advisories

`trust-dev rust audit` checks every crate the program's module is built from, the SDK's and your
crate's dependencies, against the [RustSec advisory database](https://rustsec.org):

```bash
trust-dev rust audit --project .
```

It prints each advisory that names a crate at the version the project builds, with the versions
that fix it. A vulnerability fails the command, so it can stop a pipeline before a deployment; an
advisory that a crate is unmaintained or unsound, or a notice, is a warning. The command downloads
the database's current state each time and keeps a copy in truST's data folder; without a
connection it checks against that copy and says when it was downloaded. On a computer that never
reaches GitHub, give it a copy of the database's repository with `--db <folder>`.

The program image itself lists the same crates, with their versions, where each comes from and the
checksum `Cargo.lock` pins for it, so a deployed program says which Rust code it holds. The list is
signed with the image, travels in its deployment record, and is in `build/build.json` as `crates`.

## Reference

- [Specification 48: Rust POUs](../reference/specifications/48-rust-pous.md)
- [Create A New Project](../start/create-new-project.md)
- [Project Layout](project-layout.md)
