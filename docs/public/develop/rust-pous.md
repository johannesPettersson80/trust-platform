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
trust-plc = "=0.44.1" # exactly your truST version
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
  `cx.wall_clock()` the wall clock. A Rust POU never waits or sleeps inside a scan; a sequence waits
  from one activation to the next ([Sequences](#sequences)).
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
"production"` makes these errors, and a production build also needs a current measurement of each
Rust POU its tasks run (see [Measure a Rust POU](#measure-a-rust-pou)): it counts the larger of the
budget and the measured time. Structured Text's statement budget does not count Rust code; the
watchdog still ends a scan that overruns its task.

A debug image also times each call of a Rust POU while it runs, on a computer and on a board. A
call longer than the POU's budget is the event `rust_budget_exceeded`, with the POU's `name`, the
call's time `call_ns` and its `budget_ns`. The event is recorded for the POU's first call over its
budget and then only for a call longer than every call before it, so a POU that is over its budget
in every scan does not fill the event list. `trust-runtime ctl events` shows the events, and
`trust-runtime ctl stats` prints a line for each Rust POU after the tasks, such as
`rust_pou=SPIN budget_ns=1000 longest_ns=52000 over_budget=7`: its budget, its longest call and its
calls over the budget since the program started (`rust_pous` in the `tasks.stats` reply). A release image (`[build] debug = false`) times
nothing.

### Measure a Rust POU

`trust-dev rust bench` measures how long each Rust function, function block, method and program takes
on this computer, and writes the longest call of each into `trust.toml`:

```bash
trust-dev rust bench --project .
```

The bench builds the program with a test program for each case, and calls each case once a scan for
1,000 scans (`--scans` sets another count) on one instance, so that a function block keeps its state
from one call to the next as it does on the PLC. A Rust program runs on its instances, those your
configuration declares, its case's inputs written into each before every scan. A case gives the
inputs that make the POU work hardest; inputs it does not name keep their declared initial values,
and a POU without a case is measured at its initial values:

```rust
#[trust_fb(name = "FILTER_EMA")]
#[budget("15us")]
#[bench_case(x = 1.0e6, alpha = 0.001)]
#[bench_case(x = -1.0e6)]
pub struct FilterEma {
    // ...
}
```

The values are written as `#[init(...)]` writes them. The attribute is `bench_case`, not `bench`:
`bench` is Rust's own benchmark attribute.

Each measured POU gets an entry for the computer's target in `[timing.measured]`: its longest call
in nanoseconds, `code`, the hash of the files the program is built from, and `device`:

```toml
[timing.measured]
"FILTER_EMA" = { "x86_64-unknown-linux-gnu" = { longest = 4180, code = "…", device = "computer" } }
```

To measure on a board, name the device and its target as `trust-dev test` takes them; the bench
installs its program there and runs each case by stepped scans:

```bash
trust-dev rust bench --project . --device /dev/ttyACM0 --target esp32c6
```

The entry is then the board's target's, and its `device` the runtime ID the board reports. A Rust
program's case on a board gives values of the kinds a step's inputs carry: `BOOL`, integers, bit
strings, reals, `TIME`, `LTIME` and `STRING`.

The command names each POU whose longest call is over its budget, and each case that faulted,
which measures nothing for its POU. A change to any Structured Text or Rust file of the project gives
the program a new `code`, so measure again after it.

## Sequences

Sequential logic, which Structured Text writes as an SFC or a `CASE` machine, can be written in Rust
as straight-line code that waits. The POU names its sequence, and `#[trust_sequence]` on its `impl`
block turns that `async fn` into a state machine: each activation runs the sequence from the wait
point where it waits to its next one, and returns. No future is kept, and nothing waits inside a
scan.

```rust
#[trust_program(name = "PALLETIZER", sequence = "run")]
#[budget("80us")]
pub struct Palletizer {
    #[input] pub part_present: bool,
    #[input] pub gripper_closed: bool,
    #[output] pub close_gripper: bool,
    #[output] pub active_step: Str<16>,
}

#[trust_sequence]
impl Palletizer {
    async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
        loop {
            s.step(&mut self.active_step, "WAIT_PART");
            s.until(|me: &Self| me.part_present).await;
            s.step(&mut self.active_step, "GRIP");
            self.close_gripper = true;
            s.until(|me: &Self| me.gripper_closed)
                .within(Time::from_millis(s, 2_000)?)
                .await?;
            s.wait(Time::from_millis(s, 500)?).await;
            self.close_gripper = false;
        }
    }
}
```

The POU has no `impl Body` of its own: each activation, a program's or a block's call, runs the
sequence. The wait points:

| Wait point | Completes |
|---|---|
| `s.next_cycle().await;` | at the next activation |
| `s.until(pred).await;` | at the first later activation at whose start `pred` holds for the instance |
| `s.wait(t).await;` | at the first activation whose logical time is at least `t` after the one that began the wait |
| `s.all(w1, w2, ..).await;` | when each of its waits has completed, each at an activation of its own, as an SFC's parallel branches converge |
| `let won: u16 = s.race(w1, w2, ..).await;` | when the first of its waits completes; `won` is that wait's number, from 1, and the earlier one in the list wins a tie |
| `self.fill(s, ..).await?;` | when the child sequence `fill` has returned |

A wait never completes at the activation that began it. `.within(t)`, last before `.await`, faults
with `RustFault` when the wait lasts longer than `t`: the fault's number is the wait point's, its
place among the body's wait points from 1, and its two numbers are `t` and the time waited, in
milliseconds. `s.step(&mut output, "NAME")` writes the step's name to a text output, so the HMI and
Live Values show where the sequence is.

**Locals.** A local of the sequence names its type, `let count: i16 = 0;`: it is a variable of the
sequence's state and keeps its value from one activation to the next. A wait point stands as a
statement of its own, and every way round a loop reaches one, or the scan would never end. The
attribute refuses anything else at its place and says what to write instead: a `for` loop or an
`if let` around a wait point, an `.await` inside an expression, a loop that can go round without
waiting.

**Child sequences.** Another `async fn` of the same block is a child sequence. It takes `s` and,
after it, parameters of the types a local takes, `async fn move_to(&mut self, s: &mut Seq, target:
f64)`. Awaited with `self.move_to(s, 25.0).await?`, it runs as a call would, from the activation that
reaches it, so moving part of a sequence into a child changes nothing about when anything happens.
Its parameters are variables of its state: the arguments set them when it starts, and they keep
their values across its waits. A child can also be a member of `s.all` or `s.race`: `s.all(self.left(s),
self.right(s))` runs two children side by side, as an SFC's parallel branches, and
`s.race(self.fill(s), s.until(|me: &Self| me.stop))` abandons the filling where it waits when the stop
comes first, so the sequence closes what the filling left open. The POU's sequence is the one `async
fn` of the block that no other awaits, and it takes no parameter.

**The state.** The POU keeps where its sequence waits in its last variable, `_seq`: `point`, 0 at
the top and then the wait point's number, `began`, the time its wait began, the locals, and each
child's state in a field named as the child. The debugger's Variables view shows it, and
`variables.read` reads `<instance>._seq.point`.

**Restarts and online changes.** A cold or warm restart starts every sequence at its top. An online
change keeps a sequence where it waits when its wait points, named by their steps or by their order,
its parameters and its locals are the same, its children's included. Otherwise the sequence starts at
its top at its next activation, its children with it, and the change's report lists the state's new
member `shape_<signature>`.

A crate's own tests run a sequence activation by activation: `trust_plc::test::task(interval)` gives
a context, `trust_plc::test::advance(&mut cx)` moves it on by one interval, and each `call` is one
activation.

## Call Structured Text from Rust

A Rust POU can call the project's Structured Text functions and function blocks through the
bindings that the build generates in the project's `build/trust_st/`. Name them in the crate's
`Cargo.toml` (`path = "build/trust_st"` in a Rust-first project, whose crate is the project):

```toml
[dependencies]
trust-plc = "=0.44.1"
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

The example `examples/rust_first_cell` is a whole Rust-first project: a press cell whose program runs
a sequence, bound to its button, sensors and valves in `trust.toml`, with machine tests in its own
`tests/` that run the program and its simulated clamp and press under `cargo test`.

## Heartbeats of services

A service is a program of its own beside the runtime, which reads and writes the runtime's variables
over the control link. Declare it in `trust.toml`:

```toml
[services.recipe]
```

truST then declares the global `recipe_heartbeat : UDINT` in `src/generated/services.st`. While the
service runs it sends its heartbeat, `service.heartbeat`, with its service token (below), at the
period its `heartbeat` setting states; each heartbeat adds one to the global at the next cycle's
start. A program gates on it with `SERVICE_FRESH`:

```iecst
fresh(HEARTBEAT := recipe_heartbeat, TIMEOUT := T#1s, FRESH => ok);
```

A Structured Text project may hold `trust.toml` with `[services]` alone, without a crate.

A service writes only the variables its `writes` lists, by their paths:

```toml
[services.recipe]
heartbeat = "500ms"
writes = ["Recipe.target", "Recipe.speed"]
```

The build checks each path against the program and refuses one that names no variable, a
constant, an instance, a structure or an array, or a variable that a second service also lists, so
that each variable has one service that writes it. **Compile** (`trust-runtime check`) checks the
paths the same way, so a wrong path shows before you start the project. The list goes into the signed program image, so
a board holds the service to it as a computer does. The service writes with the control request
`variables.write`, which needs a service token with the Engineer role (`--role engineer`, below).
The next cycle's start writes the whole set together, as the program's input and never as a force,
and a set with a path outside `writes`, or a value its variable's type does not take, writes
nothing. To write as the service does, for example while you commission it:

```text
trust-runtime ctl --project . --token "$(cat secrets/recipe.token)" write Recipe.target=21.5
```

A service signs in to the runtime with a service token of its own. Create it with the runtime's
Admin access, and name the file the service reads it from:

```text
trust-runtime ctl --project . service-token recipe --out secrets/recipe.token
```

The token is written into the file, which only you can read, and is never printed. It is a Viewer,
which reads, unless you give it another role with `--role operator` or `--role engineer`. A service
token does not expire, so a service that runs unattended keeps its connection. To replace it, run the
command again: the earlier token stops working at once. `pair.list` lists it with the service's name,
and `pair.revoke` with its id, `service-recipe`, revokes it. Keep the file out of version control.

## Write a service

A service is an ordinary Rust program, and you build it with your own Rust: truST's toolchain
builds programs, not services. Install Rust with [rustup](https://rustup.rs), which also names the
linker your computer needs (Xcode's command-line tools on macOS, a C compiler on Linux, Visual
Studio's build tools on Windows).

Create the service with `cargo new --bin recipe-service` and make it depend on `trust-service` at
the version of the truST release its runtime runs (`cargo add trust-service@=<version>`); the
example [Rust service](../examples/rust.md#rust-service) is a whole service with its program. It
connects by its name with the settings of its `[services.<name>]`:

```toml
[services.recipe]
credentials = "secrets/recipe.token"   # its service token, never in version control
heartbeat = "500ms"
```

```rust
use std::time::Duration;
use trust_service::Service;

fn main() -> Result<(), trust_service::ServiceError> {
    let service = Service::connect("recipe")?; // [services.recipe] of trust.toml
    let _heartbeat = service.start_heartbeat()?; // every 500 ms, until dropped
    for read in service.subscribe(&["P1.level"], Duration::from_secs(1)) {
        let level: f64 = read?.get("P1.level")?;
        service.write(&[("Recipe.target", (level * 0.9).into())])?;
    }
    Ok(())
}
```

`Service::connect` reads `trust.toml` in the current folder, or in the folder `TRUST_PROJECT` names.
It connects to the endpoint that the project's running runtime publishes in
`.trust-runtime/control.json`, as `trust-runtime ctl` finds it, so the runtime keeps its default
endpoint and no token is written into the project. To reach a runtime that does not publish into
this project folder, name its endpoint: `endpoint = "tcp://192.168.1.20:47800"`.
Each read is one completed scan's values, with the scan's number, its time and whether the program
runs; `get` reads a value as the Rust type of its declared type (`bool`, the integers, `f32` from a
`REAL`, `f64`, `String`, `Duration` from a `TIME`). The heartbeat runs on a connection and a thread
of its own, so the service's own work never delays it.

With bindings, the compiler checks the service's paths and types against the program. Build the
project, then write the bindings into the service's crate:

```text
trust-dev service bindings recipe --out ../recipe-service/src/plant.rs
```

They name each variable of the program that holds one value, with its Rust type, and only the
variables `writes` lists can be written:

```rust
mod plant;

let level: f64 = service.get(plant::P1::level)?;
service.set(plant::P1::target, &(level * 0.9))?; // P1.target is in writes
```

Run the command again when the program changes; a variable it no longer has is then a compiler
error instead of a refusal at run time.

## Run a service

Build the service, then put it under the computer's service manager, so that it starts at every
boot and again 2 s after it ends:

```text
cargo build --release --manifest-path recipe-service/Cargo.toml
trust-dev service install recipe
```

`install` takes root, or an administrator on Windows. Run without those rights, it checks the
service, changes nothing and prints the command to run, with the binary's path. The service is
called `trust-service-recipe`:

- on Linux, a systemd unit, whose output `journalctl -u trust-service-recipe` shows;
- on macOS, a launchd daemon, which writes `/Library/Logs/trust-service-recipe.log`;
- on Windows, a Windows service, whose output goes to
  `%ProgramData%\truST\services\recipe\logs\recipe.log`.

It runs in the project folder, so `Service::connect` finds `trust.toml` and the token there: on
Linux and macOS as the user who owns the folder, on Windows as its own virtual account,
`NT SERVICE\trust-service-recipe`. After a new build, install it again: that restarts it.
`trust-dev service uninstall recipe` stops and removes it. The runtime never starts a service
itself.

## Build, run and debug

A project with Rust POUs is worked like any project: **Compile**, **Start** and **Debug** in the truST
view, `trust-runtime build`, `trust-dev test` and `trust-runtime deploy`. The build compiles the crate
with the toolchain's own cargo and compiler, in a cargo home of its own: your own Rust, its settings
and its caches play no part. The first build of a project compiles the SDK, so it takes longer than
the next. **Compile** (`trust-runtime check`) checks the crate with the same cargo, without building
the program, and lists the crate's errors and warnings in Problems where they are.

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
