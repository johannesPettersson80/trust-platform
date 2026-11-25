# 48 — Rust in truST projects

- **Status:** 0.1, approved by the owner on 15 November 2026 ("approved"), with the decisions of §21;
  written from the owner's decisions of that day (below) against the compiled runtime of
  specification 44. 0.2, 17 November 2026: how the bundle provides the SDK's procedural macros
  (RU-74, §21 decision 1), decided under the owner's delegation of that day. 0.3, 19 November 2026:
  how `trust-dev service install` puts a service under the computer's service manager (RU-64, §21
  decision 14), decided under the owner's delegation of 18 November 2026. 0.4, 19 November 2026:
  the Windows service's program, a copy of trust-dev in the service's own folder, its log, its
  `service.toml` and its virtual account's access (RU-64). 0.5, 20 November 2026: a service is built
  with the user's own Rust, and the bundle carries only the SDK a program is built with (RU-12,
  RU-61, RU-62, RU-76), as the owner decided that day. 0.6, 21 November 2026: what a sequence's
  `s.all` and `s.race` wait for, by IEC §6.7.5, and the state an `s.all` keeps (RU-71, RU-72),
  decided under the owner's standing delegation ("do whats most professional and best for the user
  and us"). 0.7, 21 November 2026: child sequences, when one runs and where its state is (RU-71,
  RU-72), decided under the same delegation. 0.8, 21 November 2026: how an online change tells that
  a sequence's wait points or locals changed, through its state's `shape_<signature>` (RU-71,
  RU-73). 0.9, 21 November 2026: a child sequence's parameters (RU-71, RU-72, RU-73), decided under
  the same delegation. 0.10, 21 November 2026: child sequences as members of `s.all` and `s.race`
  (RU-72), decided under the same delegation. 0.11, 21 November 2026: `trust-dev rust bench`'s
  cases, its run and its record, what makes a measurement current and what a `production` build
  does with it (RU-48, Appendix B), decided under the same delegation. 0.12, 23 November 2026: what
  a cycle trace holds, its start state and the recorded image's tables among it, how it is recorded
  on demand and by the flight recorder, and how a replay starts another build from it and compares it
  (RU-59, RU-60, Appendix C), decided under the owner's delegation of 22 November 2026 ("do whats
  most professional and best for us and the users"). 0.13, 24 November 2026: the flight recorder's
  start state, the memory and forces as the cycle before its first cycle left them, and where a
  fault's trace starts after a record that did not fit (RU-59), decided under the same delegation.
  0.14, 24 November 2026: one rule that sizes a recording's rooms to the memory a runtime has, and a
  chip serving an ended trace's file part by part from its records (RU-59, "Memory"), decided under
  the same delegation. 0.15, 24 November 2026: the memory a chip can give a recording, its free heap
  less what it keeps beside any program (RU-59, "Memory"), decided under the same delegation. 0.16,
  25 November 2026: a method call made between two cycles keeps the wall clock's readings it made,
  and a replay gives them back to it (RU-59, RU-60, Appendix C.6), decided under the same delegation.
  0.17, 25 November 2026: a chip's flight recorder, its two recordings each sized to half the
  memory a recording would have (RU-59, "Memory"), decided under the same delegation.
- **Owns:** Rust as a language of truST projects: Rust functions, function blocks and programs that
  run in the scan; projects written in Rust whose Structured Text configuration is generated
  (Rust-first projects); Rust services that run beside a runtime; the SDK crates; how the build
  compiles Rust into the program image; the rules every Rust POU keeps; what the tools do for Rust.
- **Builds on:** specification 44 (the build, the program image, the kernel), whose module the Rust
  code compiles into; specification 04 (POU declarations), 18 (configurations, resources, tasks),
  10 and 11 (runtime semantics, the control link), 25 (VS Code), 22 (developer workflows and tests),
  34 (devices). Every behaviour those specifications give a POU holds for a Rust POU unless a section
  below says otherwise.
- **Background:** `docs/internal/design/rust-support-architecture-spec-v1.md` ("v2.4, Rust-first",
  July 2026) was written for the bytecode runtime. Its product case, its three tiers and its two
  doors carry over (§1, §3); its native-module ABI, its WebAssembly form, its marshalling layer and
  its bytecode evidence do not, since a Rust POU now compiles into the same module as the generated
  Structured Text (§3).
- **Standard:** IEC 61131-3:2013, cited as "IEC §x.y". IEC §4.3 allows "a programming language other
  than one of those defined in this standard" for "a function, function block type and methods";
  a Rust PROGRAM goes beyond it (§7, `docs/IEC_DEVIATIONS.md` DEV-048).
- **Crates:** `trust-plc` (the SDK of Rust POUs), `trust-plc-macros` (its attributes),
  `trust-plc-decl` (the reader of Rust POUs that the attributes and the build share, RU-24),
  `trust-service` (the SDK of services), `trust-sim` (machine simulation in `cargo test`), and the
  existing trust-hir, trust-runtime (the build), trust-runtime-core (the kernel), trust-plc-abi,
  trust-plc-lib, trust-lsp, trust-dev, the VS Code extension.

## Owner requirements

The owner's decisions, which this specification carries out:

- "We shall add rust support in the structured text code" (15 November 2026).
- "We shall not have web assembly" (15 November 2026): Rust in the scan is native code only,
  compiled into the program's module. There is no WebAssembly form, no sandbox engine in the kernel.
- "In the spec we shall have all tiers, it MUST be a complete spec" (15 November 2026): Rust
  functions and function blocks in the scan, Rust programs on tasks, and Rust services beside the
  runtime are all specified in full.
- Rust-first projects, "Include now" (15 November 2026): a project written in Rust, without
  hand-written Structured Text, is specified and built with the tiers.
- `unsafe` in Rust that runs in the scan: "Forbidden" (15 November 2026).
- The user's own Rust installation: "Own Rust optional" (15 November 2026): every build uses truST's
  bundled toolchain; a user's own Rust serves only rust-analyzer and `cargo test`, never a build.
- A Rust service is built with the user's own Rust ("agree", 20 November 2026, to the recommendation
  that a service, an ordinary Rust program for the computer, is built like any Rust program): it
  links the computer's standard library with the computer's linker, which the bundle does not carry
  and truST cannot ship (RU-61).
- "remember that we need good documentation and good examples" (22 November 2026), and for every
  feature "A user guide in the docs site, written as steps" and "Each crate's API reference gets
  examples that run as tests" (22 November 2026; RU-89).
- Earlier decisions that hold: "the user shall not have to install rust by himself" (27 October
  2026, specification 44 NX-08); boards work exactly like trust-runtime (16 October 2026): a Rust POU
  runs on every device a program runs on; "we MUST follow the IEC standard" (22 October 2026):
  Rust POUs keep the call model, the data types and the faults of IEC 61131-3 as specification 44
  carries them out.

## 1. Decision and scope

**RU-01. One program, two languages.** A project's POUs may be written in Structured Text or in
Rust. A Rust POU is compiled by the pinned compiler into the same module as the Structured Text
(specification 44 §3), runs on the same kernel, in the same memory areas, under the same scheduler,
faults, retain, online change, debugging and monitoring. There is no second runtime, no dynamic
loading and no interface between modules: the program image is still one module (specification 44
NX-03).

**RU-02. Three tiers.** Rust code reaches a running machine in one of three ways, which the tier of
its call site decides:

| Tier | Rust code is | Called by | Time |
|---|---|---|---|
| 1, `inline` | a `FUNCTION` or a `FUNCTION_BLOCK` (with its methods) | Structured Text or Rust, inside the scan | part of the calling task's activation (§10) |
| 2, `cyclic` | a `PROGRAM` | a task of the configuration, every activation | its own place in the task's program list (§10) |
| 3, `service` | a program of its own, beside a runtime | itself; it reads and writes the runtime's variables over the control link | none in the scan: the scan sees the service's values with their age (§13) |

Tiers 1 and 2 run in the scan and keep every rule of §5. Tier 3 is an ordinary Rust program: none of
the scan's rules applies to it, and it can never stop, slow or break a scan.

**RU-03. Two ways to start a project.** A Structured Text project gains Rust POUs in a crate of its
own (§4, the ST-first door). A Rust-first project is a crate whose Rust POUs and whose manifest,
`trust.toml`, are the project; the build generates its Structured Text configuration (§14). Both are
compiled by one path (§9) and run alike.

**RU-04. Out of scope.** Not part of this specification, and refused where a project tries them:
WebAssembly in any form; Rust code written inside a `.st` file; dynamic loading of Rust code; a
module that is built apart from the program image and loaded beside it; `unsafe` code in the scan
(§5.8); the heap and the Rust standard library in the scan (§5.7); a Rust POU that extends a
Structured Text function block or class, or that Structured Text extends (§6.6).

**RU-05. The standard.** A Rust function, function block or method is a POU of IEC §4.3's "other
programming language": its declaration is Structured Text that the build generates (§6.1), its
interface, call and data types are the standard's, and only its body is Rust. A Rust program's body
is Rust too, which IEC §4.3 rule 4 does not allow; DEV-048 records the deviation (§7).

## 2. Terms

- **Rust POU:** a function, function block, method or program whose body is Rust, declared to
  Structured Text by a generated declaration (§6.1).
- **The project's crate:** the Cargo package that holds a project's Rust POUs (§4). It is compiled
  into the program's module; it is not a module of its own.
- **The scan crate graph:** the project's crate and every crate it depends on, which all run in the
  scan and keep the rules of §5.
- **SDK:** `trust-plc` and `trust-plc-macros`, the crates a Rust POU is written with; `trust-service`
  for services; `trust-sim` for machine tests.
- **Generated declarations:** the Structured Text declarations the build writes for the project's
  Rust POUs, `src/generated/rust_pous.st`, which every Structured Text tool reads (§6.1).
- **Layer:** one of the three groups the build compiles a project in (§9.2): the Structured Text that
  Rust calls, the project's crate, and the rest of the Structured Text.

## 3. Architecture

```
Engineering computer                                            Device
┌──────────────────────────────────────────────────────────┐    ┌──────────────────────────┐
│ ST sources            the project's crate (Rust POUs)    │    │ Kernel (spec 44 §8):     │
│   │                     │                                │    │  the same loader, check, │
│   │   trust-plc-decl: generated declarations ────┐       │    │  scheduler, areas,       │
│   ▼                                             ▼       │    │  faults, retain, online  │
│ trust-syntax, trust-hir: one checked program (ST + decl.) │    │  change, debugging       │
│   ▼                                                      │    │                          │
│ generator (spec 44 §6): layer 1 crate  ─► project's crate│ ─► │ one program image        │
│                          (ST that Rust calls)   ▼        │    │ (spec 44 §5)             │
│                          layer 3 crate (the rest of ST,  │    │                          │
│                          entries, configuration)         │    │                          │
│   ▼                                                      │    │                          │
│ pinned rustc + rust-lld (bundle, §16): one module, LTO   │    └──────────────────────────┘
│ image builder: tables, INIT, symbols, signature          │
└──────────────────────────────────────────────────────────┘
       services (tier 3): own processes ──── control link (spec 11 §6.8) ──── runtime
```

The build of specification 44 §3 gains three steps, numbered here into its six:

1. **Declarations.** Before the semantic model, the build writes the generated declarations from
   the project's crate (§6.1), and in a Rust-first project the configuration from `trust.toml` (§14).
   trust-hir then checks the project, the generated Structured Text with it, as today.
2. **Layers.** The generator splits the project's Structured Text into layer 1, the POUs that Rust
   code calls and what they call, and layer 3, the rest (§9.2), and writes each as a crate.
3. **Compilation.** The pinned compiler compiles layer 1, then the project's crate against it and
   the SDK, then layer 3 against both, and links one module with link-time optimization
   (specification 44 §4.5). The checks of §9.4 run on the linked module before the image is built.

Everything after that is specification 44's: the image, the deployment record, the loader, the
kernel. A device cannot tell a Rust POU from a Structured Text POU, except through the symbol
table's language mark (§6.4).

## 4. The project's crate

### 4.1 Where it lies

**RU-06.** In a Structured Text project the crate lies in the folder `rust/` of the project, a Cargo
package with `rust/Cargo.toml` and its sources in `rust/src/`. In a Rust-first project it is the
project folder itself, `Cargo.toml` and `src/` beside `trust.toml` (§14.1). A project has at most
one such crate; a project without one has no Rust and builds as today.

**RU-07.** `trust.toml` at the project's root names the crate and the project's Rust settings. In a
Structured Text project it is optional and holds only `[rust]` and `[services]`; in a Rust-first
project it is required and holds the configuration too (§14.2). Its schema is Appendix B.

### 4.2 The crate's manifest

**RU-08.** The crate is a library (`[lib]`, crate type `rlib`), edition 2024, without a
`[[bin]]`. It depends on `trust-plc` at exactly the version of the truST release that builds it
(`trust-plc = "=0.42.0"` for truST 0.42.0); `trust-runtime new` and Create Project write that line,
and the build refuses another version with the version it needs. The crate's root carries
`#![cfg_attr(not(test), no_std)]` and `#![forbid(unsafe_code)]`; the build refuses a crate without
them, naming the line to add.

**RU-09. Dependencies.** The crate may depend on other crates, from crates.io or vendored into the
project (`cargo vendor`), with `Cargo.lock` committed. Every crate of the scan crate graph keeps
§5.7 and §5.8: it builds without the standard library and without `alloc`, and it holds no `unsafe`
code. The build refuses a dependency whose sources hold `unsafe` code, naming the crate, the file
and the line (§9.3). A crate of the graph has no build script and no procedural macro: both run
code on the engineering computer while a project builds, which a PLC project's build does not allow
(§17); the build refuses one, naming it. The SDK and the crates it is built from are the exception:
the toolchain bundle carries their sources, reviewed with each release (§16), and the SDK's own
`unsafe` is the interface crate's (specification 44 §6.9). A crate that does not build for one of
the project's targets fails the build for that target (§18).

**RU-10. The lock.** The build uses `Cargo.lock` as it lies (`--locked`): it never changes the lock
and never resolves a version on its own. A build without network uses the vendored sources or the
cargo cache; a build that would need to download and cannot fails, naming the crate and
`cargo vendor`.

### 4.3 The SDK

**RU-11.** `trust-plc` is the only crate a Rust POU needs. It re-exports what specification 44 gives
a module, behind safe types: `Cx` (the activation's context), `Fault`, the IEC data types of §6.2
(`Str<N>`, `WStr<N>`, `Time`, `Date`, `Tod`, `Dt` and the rest), `InOut<T>` and `Ref<T>` for
`VAR_IN_OUT` and `REF_TO` (the value of a Structured Text interface is its binding, RU-39), the standard functions and function blocks of the program library (`trust_plc::std`,
the same code `trust-plc-lib` gives Structured Text), the checked arithmetic of §5.3, and the
attributes of `trust-plc-macros` (§6). It holds no `unsafe` code of its own; the `unsafe` of a module
stays in `trust-plc-abi` (specification 44 §6.9). `Cx` is the SDK's own type: it gives a Rust POU the task's
logical time and the wall clock (RU-16) and records the faults the POU returns, and it keeps the
kernel interface's context (specification 44 §8.2) to itself, so that no Rust code reaches the
statement budget, the call depth, the breakpoints or the kernel's pointers, which the generated code
and the kernel keep. The generated code gives each call of a Rust POU a `Cx` made from the kernel's
context and takes back the record of the fault the call returns and the statement budget that the
Structured Text it calls spent (RU-38), which only the generated bindings run on that context.

**RU-12.** Each truST release publishes `trust-plc`, `trust-plc-macros`, `trust-service` and
`trust-sim` to crates.io at the release's version, with the crates they are built from
(`trust-plc-abi`, `trust-plc-lib`, `trust-plc-decl`, `trust-runtime-client`), once the release is
verified (specification 24, "Publishing the SDK's crates"; `trust-sim` joins when it is built). Its
toolchain bundle (§16) carries the sources of the SDK a program is built with: `trust-plc` and
`trust-plc-macros` with the crates they are built from. The build always compiles the bundle's copy:
the project's `Cargo.toml` names the version, the build patches those crates to the bundle's sources,
and a project builds without crates.io when its other dependencies are vendored. `trust-service`,
`trust-runtime-client` and `trust-sim` are not in the bundle: a service and a machine test are built
with the user's own Rust (RU-61, RU-58), which takes them from crates.io. A user's own `cargo` resolves the same version
from crates.io for rust-analyzer and `cargo test` (§16.3). The SDK's crates name each other at the
release's exact version (`=<version>`, beside their paths in the workspace), which the version bump
moves (`scripts/bump_version.py`), so that `cargo package` writes each crate as crates.io and the
bundle take it.

**RU-89.** The SDK's documentation is its API reference on docs.rs, and its examples are tests. Each
attribute of `trust-plc-macros` and the crate page of `trust-plc` show their use in a whole program,
written as a project writes it (through `trust_plc::prelude`) and called as a unit test calls it
(RU-56), with the values it must give worked out by hand; the crate pages of `trust-service` and
`trust-runtime-client` show how a program reaches a runtime. CI runs the documentation examples of
every crate RU-12 publishes (`cargo test --doc`), so that no example drifts from the API it shows. An
example that needs a running runtime or service is compiled and not run (`no_run`).

## 5. The rules of Rust in the scan

The generated Structured Text keeps these rules by construction (specification 44 §6.1, §7.5,
§8.6); a Rust POU keeps them by the SDK's types and the build's checks (§9.4). Each rule names what
breaks it and what the build or the kernel does then.

### 5.1 Memory

**RU-13.** A Rust POU's state lies in the program's areas: a function block's or a
program's variables in its instance (`P`, and `R` for its retained ones), a function's locals on the
task's stack, exactly as for Structured Text (specification 44 §7.1). A Rust POU keeps no `static`
that changes (a `static` with interior mutability, a `static mut`), allocates nothing and keeps no
reference beyond its call. The build refuses a changing `static` in the scan crate graph, naming it
(§9.4); the heap does not exist (§5.7).

### 5.2 Faults, never panics

**RU-14.** A Rust POU returns `Result<_, Fault>`. `Fault` carries one of
specification 10 §10.2's stable codes, with the detail specification 44 §8.6 gives each, or the
code `RustFault` (Appendix A of specification 44, added by this specification) with a number the POU
chooses and two numbers of detail, which `trust_plc::rust_fault(cx, number, first, second)` makes:
`return Err(rust_fault(cx, 7, level, limit))`. A fault ends the scan as a fault of Structured Text does:
the kernel latches it, applies the output policy and reports it with the Structured Text statement
that called the Rust POU, the Rust POU's name and its fault (§11.3). A Rust POU never panics:
specification 44 §8.6 refuses a module whose code references the panic machinery, and the build
names the Rust function that does (§9.4). Indexing, `unwrap`, `expect`, `/` and `%` on integers,
`<<` past the width and slicing out of range can panic in Rust; the SDK gives each a form that
returns the fault instead (`get`, `ok_or(Fault::...)`, §5.3), and the build's message names the
form to use.

### 5.3 Arithmetic

**RU-15.** Structured Text faults on integer overflow, division by zero and a real
result that is not finite (specification 44 §6.5). Rust wraps or panics instead. A Rust POU that
wants the standard's behaviour uses `trust_plc::checked`: `add`, `sub`, `mul`, `div`, `rem`, `neg`
and `abs` on every integer type are the program library's own operations, which the generated
Structured Text calls, and return its faults: `Overflow` for a result outside the type (the code of
specification 44 §6.5's integer overflow), `DivisionByZero` and `ModuloByZero`; the real operations
return `Overflow` for a result that is not finite. Plain
`+`, `-` and `*` on integers are compiled with Rust's overflow checks off, as the module's flags have
them (specification 44 §4.5), and wrap; the build cannot see a POU's intent, so the choice is the
POU's and the SDK's documentation says what each gives.

### 5.4 Time

**RU-16.** A Rust POU reads time only from `Cx`: the task's logical time
(`cx.now()`, specification 11 §5.2.1), the task's interval (`cx.interval()`: its `INTERVAL`, the
resource's cycle interval for a program without a task, and `T#0s` for a task that only its
`SINGLE` variable starts and outside a task, specification 44 §8.2) and the wall clock through the
kernel function specification 44 §8.2 gives (`cx.wall_clock()`). It never reads a clock of its own, never
waits and never sleeps; `core` gives it none. This keeps the scan's results a function of its inputs
and logical time (specification 44 NX-04), which §12's replay needs.

### 5.5 Determinism

**RU-17.** A Rust POU computes the same results on every device for the same inputs
and logical times (specification 44 NX-04). Integer and IEEE 754 operations give that by
themselves. The elementary functions (`sin`, `exp`, `sqrt` and the rest) come from
`trust_plc::math`, the program library's pure-Rust `libm`, never from a target's own: a call of a
function of the target's library whose results IEEE 754 does not fix (`sin`, `exp`, `log`, `pow` and
the rest) is refused by the build (§9.4), while those it rounds exactly (`sqrt`, `fma`, `floor`,
`fmod` and the rest) give one result everywhere. A Rust POU has no source of
randomness but a seeded generator it keeps in its instance (`trust_plc::Rng`).

### 5.6 No recursion

**RU-18.** A Rust POU's code calls nothing that calls it again: the build finds
the cycles of the linked module's call graph (specification 44 §8.5) and refuses one through a Rust
function, naming the functions of the cycle. A function's direct call of itself, which the assembler
resolves inside the function's own section without a call relocation (specification 44 §6.4), is
read from its call instruction: on x86-64 and RISC-V from the function's instructions read one after
the other from its start, so that no constant or part of another instruction is taken for a call.
Structured Text's bounded recursion (specification 44 §6.4) stays Structured Text's.

### 5.7 No standard library, no heap

**RU-19.** The scan crate graph builds against `core` only, the
part of the standard library the toolchain bundle carries for every module target (specification
44 §4.6). `std` and `alloc` do not exist there: a crate that needs them fails to compile for the
module, naming the crate. A crate's own tests may use `std` (`#![cfg_attr(not(test), no_std)]`).

### 5.8 No `unsafe`

**RU-20.** No crate of the scan crate graph holds `unsafe` code, except the SDK's
interface crate (specification 44 §6.9). The project's crate forbids it at its root (RU-08), and the
build refuses every other crate of the graph whose sources hold it (§9.3). A Rust POU reaches the
program's memory, its references and the kernel only through the SDK's safe types. A crate of the
graph outside the SDK depends on the SDK through the crates RU-12 publishes for projects
(`trust-plc`, `trust-plc-macros`, `trust-service`, `trust-sim`), never on the crates they are built
from (`trust-plc-abi`, `trust-plc-lib`, `trust-plc-mem`, `trust-plc-decl`): their functions serve the
generated code and trust what it gives them, as the interface crate's `run` trusts the kernel's
context it is handed. The build refuses such a dependency, naming both crates (§9.3).

### 5.9 Work per activation

**RU-21.** Specification 44's statement budget (NX-10) counts Structured
Text statements and loop passes; Rust code is not counted. A Rust POU's work is bounded by its
declared budget (§10), checked at build and on the device, and its elapsed time by the watchdog
(specification 44 NX-11) as every task's is. A Rust loop that does not end stops its task until the
watchdog's fault; the build cannot prove that a Rust loop ends, so §10's measured evidence is what a
production deployment relies on.

## 6. Rust functions and function blocks (tier 1)

### 6.1 Declarations

**RU-22.** A Rust POU is a Rust item with an attribute of `trust-plc-macros`:

```rust
use trust_plc::prelude::*;

/// Exponential moving average of one input.
#[trust_fb(name = "FILTER_EMA")]
#[budget("15us")]
pub struct FilterEma {
    #[input] pub x: f64,
    #[input] #[init(0.1)] pub alpha: f64,
    #[output] pub y: f64,
    primed: bool,
}

impl Body for FilterEma {
    fn body(&mut self, _cx: &mut Cx) -> Result<(), Fault> {
        if !self.primed {
            self.y = self.x;
            self.primed = true;
        } else {
            self.y += self.alpha * (self.x - self.y);
        }
        Ok(())
    }
}

/// The larger of two inputs, limited to a maximum.
#[trust_function(name = "MAX_LIMITED")]
#[budget("2us")]
pub fn max_limited(_cx: &mut Cx, a: i32, b: i32, limit: i32) -> Result<i32, Fault> {
    Ok(a.max(b).min(limit))
}
```

`#[trust_fb]` declares a function block, `#[trust_function]` a function, `#[trust_method]` a method
of a Rust function block (§6.5) and `#[trust_program]` a program (§7). `name` is the POU's
Structured Text name, an IEC identifier (IEC §6.1.2), unique in the project with its namespace
(`name = "Motion.AXIS_CONTROLLER"`, IEC §6.9). `#[budget]` (RU-46) stands below or above the
POU's attribute.

The program's generated code calls a Rust POU from another crate with one layout and one
signature, so the attribute refuses, at the part that breaks the rule: an item that is not `pub`;
a function block that is not a struct with named fields, or has generic parameters; a function
that is `const`, `async`, `extern` or generic, whose first parameter is not its context
`&mut Cx`, whose other parameters are not a name with its type, or whose result is not
`Result<T, Fault>` with `T` of §6.2's types, or `()` for a function without a result (IEC
§6.6.2.2 rule 2, Table 19 feature 1a). A variable's Structured Text name is its field's or
parameter's, an IEC identifier; two names that differ only in case are one name in Structured
Text and are refused. An IEC identifier is no reserved word of Structured Text in any case
(specification 01 §3, IEC §6.1.2), so no POU, type, namespace, variable, field, parameter or
enumeration value of a Rust POU is named by one; nor by `NULL`, `ON` or `SET`, which only the
CODESYS profile takes as names (docs/IEC_DEVIATIONS.md DEV-034), since a Rust POU's declarations
hold under every profile. The attribute refuses a reserved word where the name stands (`at` is a
reserved word of Structured Text: rename the variable), so that no declaration it writes fails to
parse or to check. A function block's struct gains `new()`, its starting values (RU-26),
and `call(&mut self, cx)`, its call as the kernel makes it (RU-28), which a unit test uses
(RU-56).

**RU-23. The generated declaration.** For every Rust POU the build writes its Structured Text
declaration into `src/generated/rust_pous.st` (a Rust-first project's into
`src/generated/trust/rust_pous.st`, §14.1): the POU's kind, name, variable sections, types and
initial values, marked by the attribute `{attribute 'rust'}` before its keyword, the CODESYS
form of truST's other declaration attributes (specification 02 §3.1), and without a body. The
variables keep their order in the Rust item, which is their order in the instance (RU-27): a
new section starts where the section or the retention changes. A name with namespaces is
declared inside a `NAMESPACE` for each (IEC §6.9). An initial value is written as a
Structured Text literal of the variable's type (IEC §6.3.2, Table 5: `1.0E300`, `2.0` for a
real given as `2`):

```text
(* GENERATED by trust-runtime from rust/src/filter.rs - do not edit *)
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

trust-hir checks the project with these declarations as with any other: a Structured Text call of a
Rust POU is checked against them, the language server's hover, completion, references and rename
read them (specification 14). A POU marked `{attribute 'rust'}` has no statements, its body being
the Rust item; trust-hir reports a statement in one, and the attribute on a declaration that is not
a function, function block or program, as the error E310. A rename of a Rust POU from the
Structured Text side is refused with the advice to rename it in Rust, where its name lives.

**RU-24. Writing them.** `trust-dev rust declarations` writes the generated declarations from the
crate's sources, without compiling them: it reads the attributed items with the reader the
attributes use (`trust-plc-decl`), so the declarations and the compiled code agree by
construction. It writes the project's other generated Structured Text with them: a Rust-first
project's configuration (RU-67) and the services' heartbeats (RU-63). The VS Code extension runs it
when it opens the project and when a `.rs` file or the `trust.toml` of the project is saved, and
shows a failure with its report; the build runs it first (§3, step 1) and fails when a committed file differs from what
it writes, naming the command, so a project whose generated files are out of date never builds.

### 6.2 Data types

**RU-25.** A variable of a Rust POU has one of the types specification 44 §6.2 maps, written in
Rust as the SDK names them:

| IEC type | Rust type in a Rust POU |
|---|---|
| `BOOL` | `bool` |
| `SINT` … `LINT`, `USINT` … `ULINT` | `i8` … `i64`, `u8` … `u64` |
| `BYTE`, `WORD`, `DWORD`, `LWORD` | `Byte`, `Word`, `Dword`, `Lword` (bit strings, not integers) |
| `REAL`, `LREAL` | `f32`, `f64` |
| `TIME`, `LTIME` | `Time`, `LTime` |
| `DATE`, `LDATE`, `TOD`, `LTOD`, `DT`, `LDT` | `Date`, `LDate`, `Tod`, `LTod`, `Dt`, `Ldt` |
| `CHAR`, `WCHAR` | `Char`, `WChar` |
| `STRING[n]`, `WSTRING[n]` | `Str<n>`, `WStr<n>` |
| an enumeration | a Rust `enum` with `#[trust_enum]` and explicit values |
| `ARRAY[a..b] OF T` | `Array<T, A, N>`: the `N` = b − a + 1 elements, indexed from `A` as Structured Text indexes them |
| a structure | a Rust `struct` with `#[trust_struct]`, every field of a type of this table |
| a structure, an enumeration, a type with named values or an interface of the project's Structured Text | its binding, `st::POINT`, `st::MODE`, `st::I_RUN` (RU-39) |
| a function block instance | a field of a function block's type, Rust or Structured Text (§8) |
| `REF_TO T` | `Ref<T>`; a reference to a function block or class instance of the project's Structured Text, `Ref<st::TON>` (RU-39) |
| `VAR_IN_OUT` of `T` | a function's or a method's parameter `InOut<T>`; a function block's field `#[in_out]` of type `Ref<T>` (RU-29) |

The SDK's `Date`, `Tod` and `Dt` and their long forms hold what the areas store (milliseconds since
1970-01-01, a time of day's since midnight, nanoseconds for a long form), which `from_millis` and
`as_millis`, or `from_nanos` and `as_nanos` for a long form, make and give, and are built, split and
moved by a duration with the program library's code, as `CONCAT_DATE`, `SPLIT_DT`, `ADD_DT_TIME`
and the other functions of IEC §6.6.2.5.12 and §6.6.2.5.13 (Tables 36 and 37) are in Structured
Text, with their faults: `from_ymd`, `ymd` and `day_of_week` of a date, `from_hms` and `hms` of a
time of day, `new`, `from_parts`, `parts`, `date` and `tod` of a date and time, `checked_add`,
`checked_sub` and `since` where Structured Text adds, takes away and subtracts them.

`Str<n>` and `WStr<n>` (`Str` alone is `Str<80>`, as `STRING` is `STRING[80]`) are values the POU
owns, its text and its length, which the generated code reads from the instance's string and writes
back, its length and the 0 after its text as Structured Text's writes leave them (DEV-039). A write
into one (`set`, `from_text`) keeps the longest prefix of whole characters its capacity holds, as
every write of Structured Text does (specification 10 §2.3.1); `as_bytes`, `as_str`, `as_units` and
`chars` read it.

`Array<T, A, N>` names its length rather than its upper bound because Rust's constant generics
cannot compute a length from two bounds (stable Rust refuses `[T; B - A + 1]`); `Array<f64, 1, 10>`
is `ARRAY[1..10] OF LREAL`, and `Array<Array<i16, 1, 3>, 1, 2>` is `ARRAY[1..2] OF ARRAY[1..3] OF
INT`. `get` and `get_mut` give the element at an index or `None` outside the bounds, never a
panic; `as_slice` and `iter` give the elements in order.

An enumeration is a Rust `enum` whose variants hold no data, each with an explicit value within
`INT`, that derives `Clone` and `Copy` and is marked `#[trust_enum(name = "MODE")]`, its name a
Structured Text type's as `#[trust_fb]`'s is a POU's (RU-22). The generated declarations declare it
before the POUs, without `{attribute 'rust'}` since it has no body:

```text
(* GENERATED by trust-runtime from rust/src/modes.rs - do not edit *)
TYPE
    MODE : (Off := 0, Auto := 1, Manual := 5);
END_TYPE
```

a closed enumeration (specification 02 §3.1) whose values are the variants' names and values. A
variable of it holds one of its values, as Structured Text's rules keep it to; a value none of its
variants has, which only a write through a pointer can leave, reads as its first variant, the type's
default (IEC §6.4.4.2). A Rust POU's variable of one of the crate's types names it by its Rust name,
which the declarations write as the type's Structured Text name; a name that no type of the crate
has is refused when the declarations are written.

A structure is a Rust `struct` with named `pub` fields, each of a type of this table and with its
initial value as a variable's (`#[init]`, RU-26), that derives `Clone` and `Copy` and is marked
`#[trust_struct(name = "POINT")]`. The generated declarations declare it before the POUs as a
structure of Structured Text with its fields in their order (IEC §6.4.4.6):

```text
(* GENERATED by trust-runtime from rust/src/geometry.rs - do not edit *)
TYPE
    POINT :
    STRUCT
        x : LREAL;
        y : LREAL := 2.0;
    END_STRUCT;
END_TYPE
```

A variable of it starts at its fields' initial values, the structure's default. The generated code
reads it from the structure as it lies in its area into the Rust value, field by field, an array of
structures element by element, and writes it back so (RU-28); it builds the value by the struct's
path, which is `pub` through its modules or re-exported at the crate's root as a POU's is (RU-22).

A field of another Rust type is refused at the declaration with the table's types. Generic types
(IEC §6.4.3) are not declared by a Rust POU; a Rust function may be generic in Rust only when it is
not a POU.

**RU-26. Initial values.** A variable's initial value is given by `#[init(...)]` with a literal of its
type, which the generated declaration carries as the variable's Structured Text initializer, so the
build computes it into the starting memory (specification 44 §7.4) as for Structured Text. A variable
without `#[init]` starts at the type's default (IEC §6.4.2, Table 10). The literal of a bit string
is an integer within its bits (`#[init(0x0F)]`, declared `16#0F`), of a `CHAR` an ASCII character or
a byte (`'A'`, `b'\xE9'`), of a `WCHAR` a character of one UTF-16 code unit, of a `TIME` or an
`LTIME` an IEC duration in a string (`#[init("T#1.5s")]`, declared `T#1s500ms`; IEC §6.3.4), of a
`STRING[n]` or a `WSTRING[n]` a text whose UTF-8 bytes, or UTF-16 code units, `n` holds
(`#[init("pump 1")]`, declared `'pump 1'` with `$` and the quote as `$$` and `$'`, a control
character as `$` and its code, and every other character as itself; IEC §6.3.3, Table 7), of an
array a list of its leading elements, each an initial value of its element's type, the others at
their default (`#[init([1, 2, 3])]`, declared `[1, 2, 3]`; specification 10 §2.3), of an
enumeration one of its variants (`#[init(Mode::Auto)]`, declared `Auto`), and of
a date, a time of day or both, or a long form of one, an IEC literal of its kind in a string, its
prefix short or long (`#[init("D#2024-02-29")]`, `#[init("TOD#12:30:15.25")]`,
`#[init("DT#2024-02-29-12:30:15")]`; IEC §6.3.5, Table 9), naming a date and a time of day that
exist, in whole milliseconds or, for a long form, nanoseconds, and declared with its type's own
prefix (`DATE#2024-02-29`, `LTOD#12:00:00.000000001`). A Rust
POU has no code that runs at a restart: its starting values are its declared values, as the
standard's are.

### 6.3 The instance and the call

**RU-27. Where an instance lies.** A Rust function block's instance is laid out by specification 44
§7.2 like a Structured Text instance: the variables in declaration order, with the alignment of
their types, its retained variables (`#[retain]`) in `R`, the rest in `P`. The generated
declaration is the layout's source, so the symbol table (specification 44 §5.7) names every variable
of a Rust instance, its type and its offset: Live Values, the debugger's variables, the HMI, the
services, retain and online change see a Rust instance as they see any other (§11).

**RU-28. The call.** A Structured Text call of a Rust function block is the standard's call (IEC
§6.6.3.4): the caller writes the inputs, the block's body runs, the caller reads the outputs. The
body sees its instance as the Rust struct it declared: the SDK copies the instance's variables from
the area into the struct before the body and copies the struct's variables back after it returns
`Ok`. On `Err` the variables the body changed stay as the area held them before the call, and the
fault ends the scan (RU-14). A function is called with its inputs by value and its in-outs as
`InOut<T>`, and returns its result; it has no instance.

**RU-29. In-outs and references.** A function's or a method's in-out is the caller's variable itself
(IEC §6.6.1.2.2, specification 44 §7.3), which may be a variable of the caller's frame: an
`InOut<'_, T>` for the length of the call, whose `get()` reads the variable and `set(value)` writes it.
A function block's in-out keeps the reference of its last call (specification 44 §7.3), as a
Structured Text block's does: a field `#[in_out]` of type `Ref<T>`, which the call sets and the body
reads but never rebinds. A `Ref<T>`, of such an in-out or of a `REF_TO` variable or input, reads and
writes the variable it names through the SDK, `get(cx)` and `set(cx, value)`, which make the checks
specification 44 §6.5 makes: null (`NullReference`), the area, the offset and the extent and the
generation (`ReferenceLifetime`), and the type the reference names (`TypeMismatch`). `Ref::NULL` is
`NULL`; a reference is copied and compared as Structured Text's is (specification 44 §7.3). A write
through an in-out or a reference is made at once, as Structured Text's is, so a fault after it leaves
it written, while the instance's own variables stay as they were (RU-28). A function's or a method's
in-out of a string is the caller's string, which the CODESYS profile lets be of any capacity of its
family, as it lets a Structured Text function's (`docs/IEC_DEVIATIONS.md` DEV-035): `get()` reads its
text as an assignment to the in-out's own type does and `set(value)` writes as an assignment to the
caller's string does, each cut to the longest prefix of whole characters the target holds
(specification 10 §2.3.1). A Rust POU takes no
reference of its own variables, which lie in its struct while it runs, not where a reference names
them.

**RU-30. EN and ENO.** A Rust function and function block have `EN` and `ENO` as specification 04
§EN/ENO gives every POU: `EN` = `FALSE` skips the body, and the body sets `ENO` through
`cx.set_eno(false)`; a body that returns a fault ends the scan whatever `ENO` says. A POU offers
them by declaring them (IEC Table 18's explicit form), so the generated declaration gives each Rust
POU the input `EN : BOOL := TRUE` before its own variables and the output `ENO : BOOL` after them;
the generated code moves the Rust item's variables by name, so its layout of them is the
declaration's (RU-27). An enabled call starts the context's `ENO` `TRUE`, and the generated code
writes it into the output when the body returns `Ok`, with the POU's result and outputs. A unit test
reads it with `cx.eno()`.

### 6.4 In the symbol table

**RU-31.** The symbol table marks each POU's language, Structured Text or Rust, by its
declaration's `{attribute 'rust'}` (RU-23), and the generated declaration names a Rust POU's source
file. A tool that shows a POU's source opens the Rust file for a Rust POU, at the item where the
build's reader of the project's crate finds it (§11.1).

### 6.5 Methods and interfaces

**RU-32. Methods.** A Rust function block has methods (`#[trust_method]` on a function of its `impl`
block), which IEC §4.3 lets another language program: inputs by value, in-outs as `InOut<T>`,
outputs as the method's `&mut` parameters, a result, access specifiers `PUBLIC`, `PRIVATE`,
`PROTECTED`, `INTERNAL` (IEC §6.6.5.9) as an attribute, `PUBLIC` when none is given. The generated
declaration carries each method's declaration without a body.

A method is a `pub fn` of an `impl` block of the block's struct, in the project's crate, with
`#[trust_method]` and its `#[budget]` (RU-46), named in Structured Text by its Rust name (an IEC
identifier, RU-22). Its parameters are `&mut self`, the instance, then the context `cx: &mut Cx`,
then its inputs by value, its in-outs as `InOut<'_, T>` (RU-29) and its outputs as `&mut T`, each
a name with its type, in the order of its declaration; its result is `Result<T, Fault>` with `T` of
§6.2's types, or `Result<(), Fault>` for a method without a result. `#[trust_method(private)]`,
`(protected)` and `(internal)` give its access specifier. A call of the method runs as the block's
body does (RU-28): on a copy of the instance's variables, which replaces them when it returns
`Ok`, as its outputs and its result reach the caller then; its outputs start at their types'
defaults, as a method's do at each call; on `Err` the instance stays as it was and the fault ends
the scan. The generated declaration carries each method after the block's variables, in the order
of the crate's `impl` blocks and their functions.

**RU-33. Interfaces.** A Rust function block may implement Structured Text interfaces
(`#[trust_fb(implements = "I_RUN, I_RESET")]`, IEC §6.6.6): the build checks that it has a method
of each prototype with its signature, and a call through an interface reaches the Rust method as it
reaches a Structured Text one (specification 44 §6.4, the dispatch by class id). A Rust POU holds
a value of an interface as a variable of the interface's binding, `#[input] pub runner: st::I_RUN`,
and calls its methods through it, `self.runner.START(cx)?`, as a call through the interface does
(RU-39).

`implements` names the interfaces separated by commas, each a Structured Text name with its
namespaces (IEC §6.1.2, §6.9) and each once; `#[trust_fb]` alone takes it, as a function implements
no interface. The generated declaration carries them in their order, `FUNCTION_BLOCK COUNTER
IMPLEMENTS I_COUNTER`, so the build checks the block as it checks a Structured Text block: a
prototype without the block's method, a method of another signature and one that is neither
`PUBLIC` nor `INTERNAL` are error E208 at the generated declaration, and so is an interface with a
property, which a Rust block cannot have (RU-34). An instance of the block is then assigned to a
variable of the interface, and the admission counts a call through the interface as the most Rust
work among the methods it can reach (RU-47).

### 6.6 What a Rust POU is not

**RU-34.** A Rust function block does not extend a function block or class (`EXTENDS`), and no
Structured Text function block or class extends a Rust one: an instance of a derived type lies as its
base's instance first (specification 44 §7.2), with bodies and methods that the generator binds and
dispatches across both, which a body in another language cannot join. The build refuses either with
this reason. A Rust POU has no `ACTION`s and no `PROPERTY`s (specification 04); a method gives the
same.

## 7. Rust programs (tier 2)

**RU-35.** A Rust program is a struct with `#[trust_program]` and `impl Body` as a function block
has, with `#[input]`, `#[output]`, internal and `#[retain]` variables. Its generated declaration is
a `PROGRAM` marked `{attribute 'rust'}` without a body. A configuration runs it like any program:
an ST-first project names it in its `CONFIGURATION` (`PROGRAM Axis WITH Fast : AXIS_CONTROLLER;`,
IEC §6.8.1, Table 62), a Rust-first project in `trust.toml` (§14.2). Its instance lies in the
instance area, retained variables in `R`, and its program connections (IEC Table 62 features 8 and
9) bind its inputs and outputs to directly represented or global variables as for a Structured Text
program. It has no in-outs, which its connections do not bind here, and no `EN` or `ENO`, which no
program has (RU-30); the generated code reads its variables from where the instance and the retained
area hold them before its body and writes them back when the body returns `Ok`, as a block's
(RU-28), and its budget counts once for each activation of its task (RU-47).

**RU-36. The deviation.** IEC §4.3 rule 4 requires a program's declaration to "use the mechanisms
defined for the IL, ST, LD, or FBD language"; a Rust program's declaration does (it is the generated
Structured Text declaration), but its body is Rust, which the rule does not allow for a program as
it allows it for functions, function blocks and methods. DEV-048 in `docs/IEC_DEVIATIONS.md` records
this, by the owner's decision of 15 November 2026 that Rust programs are part of truST.

**RU-37.** A Rust program has no methods of a program (specification 04 §4) and no `VAR_TEMP`; its
body's locals are Rust locals on the task's stack.

## 8. Calling Structured Text from Rust

**RU-38.** A Rust POU calls Structured Text functions and function blocks of the project and of its
libraries, and holds Structured Text function block and class instances as fields:

```rust
#[trust_fb(name = "TANK_GUARD")]
pub struct TankGuard {
    #[input] pub level: f64,
    #[output] pub alarm: bool,
    hysteresis: st::HYSTERESIS,          // a Structured Text function block of the project
}

impl Body for TankGuard {
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
        self.hysteresis.input.IN = self.level;
        self.hysteresis.call(cx)?;
        self.alarm = self.hysteresis.output.Q;
        Ok(())
    }
}
```

**RU-39. The bindings.** The build generates the crate `trust_st` (layer 1, §9.2), a library named
`st`, with a Rust function for each Structured Text function the crate calls and a Rust struct for
each Structured Text function block type it holds, whose fields are the block's inputs and outputs
and whose `call(&mut self, cx)` is the standard's call, as RU-38's example makes it. It lies in the
project's `build/trust_st/`, outside version control (the folder's own `.gitignore` says so):
`trust-dev rust declarations` and every build write it, and the extension writes it when it opens the project and when its sources change, so
rust-analyzer and `cargo test` find it (§16.3); its code is the same for every target and image: the
debugger's statement hooks (specification 44 §13) are its cargo feature `hooks`, which the build turns
on for a debug image, and a `SIZEOF` of an address follows the pointer width of the target it is
compiled for. A crate that
calls Structured Text names it in `[dependencies]`, `trust_st = { path = "../build/trust_st" }`
(`build/trust_st` in a Rust-first project), and the build refuses a crate that writes `st::` without
it, naming the line, as it names RU-08's. The crate names each Structured Text POU it calls,
`st::SCALE(cx, x)` or `use st::{SCALE, HYSTERESIS};`, which tells the build what layer 1 holds;
`use st::*`, which hides them, is refused. A POU or a data type in a namespace (IEC §6.9), as a
library's are, has its binding in a module of the namespace's name, nested for a namespace in
another: `st::Motion::MOVE_BY(cx, axis, d)`, `st::Motion::AXIS`, `use st::Motion::{MOVE_BY, AXIS};`.
A path names the longest of its prefixes that names a POU or a data type, so `st::MODE::RUN` names
the enumeration `MODE`. A crate's own `st`, a `mod st` or an `… as st`, is what
its `st::` names, as Rust resolves it, so such a crate calls no Structured Text; one that also names
the bindings is refused at its own `st`, which Rust could not tell from them. A call runs the Structured Text on the kernel's context
that the SDK's `Cx` keeps, so its statements count against the activation's budget and stop at
their breakpoints (RU-52); no Rust code reaches that context (RU-11, RU-20). A Structured Text function
block held by a Rust instance lies inside the Rust instance in the area, as a nested instance lies
in a Structured Text one (specification 44 §6.2), so the symbol table, Live Values and online change
see it. It is the Rust POU's own variable, a field `name: st::X` (`st::Motion::X` for a block in a
namespace) of a function block or a program without `#[input]`, `#[output]` or `#[init]`, not an array of blocks (a reference to one is a `Ref<st::X>`, below); a
function, a method and a structure hold none; rustc refuses one elsewhere, as a held block's or
class's binding is no `trust_plc::Value`, which the binding of a structure, an enumeration, a type
with named values or an interface is. Each call of the Rust POU reads the block's inputs
and outputs from its instance into `input` and `output`; `call` gives the instance `input`, runs
its body and takes its outputs into `output`; when the Rust POU returns, `input` goes back to the
instance, as Structured Text's assignment of an instance's input does, and `output`, which only the
block writes, does not.

A binding passes each value as the Rust type of RU-25's table that a Rust POU's variable of its
type has, an array of several dimensions, `ARRAY[1..2, 1..3] OF INT`, as the array of arrays of
them, `Array<Array<i16, 1, 3>, 1, 2>`, which lies as specification 44 §6.2 lays it out. A
function's parameters follow `cx` in the order the function declares them: its inputs
by value, its in-outs as `InOut<'_, T>` (RU-29), which the function's writes reach as Structured
Text's do, also when it faults, and its outputs as `&mut T`, which take its outputs when it
returns; its result is the binding's `Result<T, Fault>`. A held block's inputs and outputs are the
fields of `input` and `output`, by their Structured Text names, and its in-outs fields of `input` of
type `Ref<T>` (RU-29), which `call` gives the instance as Structured Text's call binds an in-out
(specification 44 §7.3); one left `NULL` ends the scan where the block reads it, as Structured
Text's does. A held block's `PUBLIC` methods (IEC §6.6.5.4; Table 48 features 5a to 5d: a method
is `PROTECTED` unless it says otherwise), its own and those it inherits, are methods of its binding,
`self.meter.ADD(cx, 1.5)?`, whose parameters and result pass as a function's do: such a call gives
the instance `input`, runs the method on the instance as Structured Text's call of it does (IEC
§6.6.5.4.4), and takes the instance's outputs into `output`; layer 1 holds each of them. A class
instance (IEC §6.6.5) is held as a block's is, its binding without `call`, as a class has no body:
its `PUBLIC` methods are what the Rust POU calls. A structure, an enumeration or a
type with named values of the project's Structured Text has a binding of its own in layer 1 when a
call or a held block passes it or the crate names it: `st::POINT`, a Rust struct of its fields, each
`pub`, named as Structured Text declares it and of the type this table gives it; `st::MODE`, a Rust
enum of the enumeration's values, each with its value; and `st::LIGHT`, a Rust struct of one value of
the base's type in this table, since a type with named values holds every value of its base
(IEC §6.4.4.3.1, specification 02 §3.2), with each named value as its constant, `st::LIGHT::RED`;
an enumeration that gives two of its names one value, which a Rust enum cannot hold, has this form
too.
Each derives `Clone`, `Copy`, `Debug` and `PartialEq`, an enumeration and a type with named values
also `Eq`, so that a `match` names their values, and its `Default`, which is also its
`trust_plc::Declared` default, is the value a variable of the type starts at: the declared initial
values of a structure's fields and the declared initial value of an enumeration or a type with named
values, else its first value (IEC §6.4.4.2.2, §6.4.4.3.2, §6.4.4.6.2, Table 11; specification 44
§6.7). The crate makes and reads their values to pass them, `st::POINT { X: 1.0, Y: 2.0 }`,
`st::MODE::RUN` or `st::LIGHT::RED`; a value of an enumeration that none of its values has, which
only a write through a pointer can leave, reads as its default, as one of the crate's enumerations
does (RU-25). A structure or an enumeration that the crate declares (RU-25) and a call or a held
block passes has such a binding too, `st::POINT` beside the crate's own `Point`, since layer 1,
compiled before the crate, cannot name the crate's type: the crate converts its values into the
binding's and back, a structure field by field and an enumeration value by value.

An interface of the project's Structured Text (IEC §6.6.6) has a binding of its own in layer 1 when
the crate names it or a call, a held block or another binding passes it: `st::I_RUN`, the value of
a variable of the interface, which is a reference to an instance that implements the interface, or
`NULL` (IEC §6.6.6.5.1), and lies as a reference (specification 44 §6.4, §7.3). It derives `Clone`,
`Copy` and `Debug`; its `Default`, also its `trust_plc::Declared` default, is `st::I_RUN::NULL`, the
value such a variable starts at; `is_null()` tells `NULL`; and two of its values are equal when they
name the same instance or are both `NULL`, as Structured Text compares them (IEC §6.6.6.5.1). It is
a `trust_plc::Value`, so a Rust POU holds it in any of its variables, `#[input] pub runner:
st::I_RUN` among them, which a Structured Text caller gives an instance (`runner := PUMP_1`), and
a call or a held block passes it as any value. A block or class instance that the Rust POU holds or
names through a reference is a value of each interface its block or class implements:
`st::I_RUN::from(&self.pump)` is the reference to it (IEC §6.6.6.5.1). Each method of the interface and of the interfaces
it extends (IEC §6.6.6.6) is a method of the binding, `self.runner.START(cx, 10)?`, whose parameters
and result pass as a function's: the call reaches the method that the class of the instance it names
has, as Structured Text's call through the interface does (IEC §6.6.8; specification 44 §6.4), and a
`NULL` value or one that names no instance ends the scan with the fault Structured Text's call has
(IEC §6.6.6.5.2; specification 44 §6.5). Layer 1 holds each method that such a call can reach
(RU-40), so the build refuses the binding of an interface that a Rust function block implements
(RU-33), naming the chain, since layer 1 is compiled before the crate.

A reference to a function block or class instance of the project's Structured Text, `REF_TO TON`
(IEC §6.4.4.10.1), is a `Ref<st::TON>` (RU-25) in any variable of a Rust POU and in what a call or a
held block passes; a Structured Text caller gives it `REF(timer)`. Its `get(cx)` gives the block's
binding, which names the instance the reference names, with its inputs and outputs as they are;
that binding's `call` and its `PUBLIC` methods run on the instance as Structured Text's call through
the reference does, the body or the method of the instance's own class when a block derived from
the one referenced has its own (IEC §6.6.8; specification 44 §6.4), and what the Rust POU writes
into `input` reaches the instance when `call` or a method gives it. `get` of `NULL`, or of a
reference that names no instance, ends the scan with the fault of Structured Text's dereference
(specification 44 §6.5). Such a reference has no `set`, as an instance is no `trust_plc::Value`.
Layer 1 holds the block's body, or its body's dispatcher, and the methods its binding calls (RU-40).

**RU-40. What can be called.** `st::` names the project's Structured Text functions, function
blocks and classes, its interfaces and its structures, enumerations and types with named values, those the crate declares
among them (RU-39): a name of none of them, or of a Rust POU, is refused where the crate writes it. The build puts every
Structured Text POU that a Rust POU calls or holds, with every POU those call, into layer 1, compiled
before the project's crate (§9.2); a call through an interface or a reference reaches every method
that can be there (specification 44 §6.4). A POU of layer 1 that calls a Rust POU, directly or
through others, would make the layers a cycle; the build refuses it, naming the chain from the call
(`st::HYSTERESIS at rust/src/tank.rs:14 calls HYSTERESIS, which calls FILTER_EMA (Rust)`). Composition across the two languages in both directions goes through a program's order on
its task and through variables, which the cycle's rules keep consistent (specification 10 §4).

## 9. The build

### 9.1 Steps

**RU-41.** `trust-runtime build` (and every command that builds: `play`, `run`, `trust-dev test`, the
extension's Compile) builds a project with a crate in these steps, before specification 44's image:

1. **Declarations** (RU-24) and, in a Rust-first project, the configuration (§14.3): written, and
   compared with the committed files; a difference fails the build, naming the command.
2. **Check:** trust-hir checks the whole project, the generated Structured Text with it.
3. **Layers** (§9.2): the generator writes layer 1 and layer 3 as crates in the build folder.
4. **Compilation** (§9.3): the bundle's `cargo`, with the project's `Cargo.lock`, compiles layer 1,
   the project's crate and layer 3 for each target of the deployment, with specification 44 §4.5's
   flags, and links the module.
5. **Checks** on the linked module (§9.4), then specification 44's image.

### 9.2 Layers

**RU-42.** Layer 1 is every Structured Text POU, data type and global a Rust POU reaches (RU-40),
with everything those reach; the project's crate depends on it as the crate `trust_st`. Layer 3 is
the rest: the Structured Text POUs that call Rust POUs or nothing of Rust, the programs, the
configuration, the entry points and the areas' types; it depends on the project's crate and on
layer 1. A project in which Rust calls no Structured Text has an empty layer 1. Every global and
every area type lies in layer 1 when a Rust POU names one (§11.2), so that both layers see one
definition of each.

### 9.3 Compilation

**RU-43.** The build compiles the scan crate graph with the bundle's `cargo` and `rustc`, offline
when the sources are vendored or cached (RU-10), every crate with specification 44 §4.5's flags.
Before it compiles, the build checks the project's crate (RU-08) and the graph as cargo resolves it
for the module's target: a crate outside the SDK and the crates the SDK is built from that has a
build script or is a procedural macro (RU-09), or whose sources hold the token `unsafe` (RU-20), is
refused, naming the crate, and for `unsafe` the file and the line. The check reads the sources as
the compiler reads them, so a comment or a string that says "unsafe" is none; since no procedural
macro of the graph writes code, a crate's code is its source text, so the check sees every `unsafe`
a crate could compile. (A compiler flag cannot do this: cargo caps the lint levels of every crate
from a registry.) The user's own `cargo` and toolchain are never used (§16). The generated crate of layer 3
is a cargo package that depends on the project's crate, with the SDK's crates patched to the
toolchain's sources whatever version the project's manifest names (RU-12), and the build starts
from the project's `Cargo.lock`. The package and cargo's target folder lie in the build's folder of
the target, `build/native/<target>/` in the project, which `.gitignore` lists, so a second build
compiles only what changed.

**RU-44. Diagnostics.** A Rust compiler error or warning of the project's crate is reported by the
build with its `.rs` file, line, column and message (`rustc`'s JSON diagnostics), and the VS Code
extension shows it in the Problems panel at that place, with the source `truST (rustc)`. A crate
warning does not fail the build; a `-D warnings` of the crate's own does. The build reports no
warning of the crates.io crates that the bundle vendors (RU-74), as cargo reports none of a crate it
takes from crates.io.

### 9.4 Checks on the linked module

**RU-45.** After the link and before the image, the build checks the module and fails with a message
that names the Rust function or item when one breaks a rule:

| Check | How | Message names |
|---|---|---|
| No panic (RU-14) | specification 44 §8.6's check of the panic machinery, then the call graph back from it, through the functions of `core` and `alloc` that call it, to the first function outside them; the generated function of a Rust POU is never inlined, so code of the POU inlined into it is named by the POU | each Rust POU (its file and line) or other function that can panic, what panics, and the form that returns a fault instead (`trust_plc::checked::div`, `get`, `ok_or(...)?`) |
| No changing `static` (RU-13) | the module's writable data sections outside the areas, by symbol | the `static` |
| No recursion through Rust (RU-18) | the call graph's cycles (specification 44 §8.5), with each function's direct call of itself read from its call instruction | the cycle's functions |
| No target library math (RU-17) | calls of the target library's inexact functions (`sin`, `exp`, …) outside `trust-plc-lib` | the function and `trust_plc::math` |
| The stack bound (specification 44 §8.5) | the frames of the Rust functions count as the generated ones do | as today |

## 10. Time

**RU-46. Budgets.** A Rust function, function block or program declares `#[budget("<duration>")]`,
the longest it may take for one call on the slowest of its project's targets. `trust.toml` may lower a
budget for one target or deployment, never raise it. A Rust POU without a budget is refused by the
build: Rust code is not counted by the statement budget (RU-21), so its declared time is the only
bound the build can check.

**RU-47. Admission at build.** For each task the build adds up the declared budgets of the Rust
programs on it and, for each Rust function or function block call in its Structured Text, the
budget times the most calls the scan can make of it: one for a call outside a loop, the bound of a
`FOR` loop with constant bounds times the calls in its body, and no bound for a call inside another
loop. A task whose Rust work, so counted, exceeds its interval, or that calls a Rust POU in a loop
without a bound, gets a warning at build that names the task and the calls; the deployment's
profile (`trust.toml` `[timing] profile`) makes it an error in `production`, a warning in
`development` (the default). The calls are counted through the Structured Text that makes them:
its programs, functions and function blocks, their methods, actions and properties, and the
function block instances the task runs on their own (IEC §6.8.2), each counted as often as the
scan can reach it. A call that can run one of several bodies (a method that a derived type
overrides, a method of an interface, the body of a function block that another extends) counts the
most Rust work among them, since the build knows every type of the program; a call on a variable
that holds an instance of the type itself (one the POU declares, not an input or an in-out, which
a caller fills) or through `SUPER^` runs that type's body. `trust-runtime build` prints each
warning.

**RU-48. Evidence.** `trust-dev rust bench` runs each Rust POU of the project on a device or the
simulator with the inputs of its `#[bench_case]` cases (or the declared initial values) and records
the longest time of each into `trust.toml` `[timing.measured]`, with the device's identity and the
image's hash. A `production` profile requires a measurement no older than the image for every Rust
POU on the deployment's device type, and uses the larger of declared and measured in RU-47's sum.
In detail:

- **Cases.** A Rust function, function block, method or program takes `#[bench_case(...)]`, one
  attribute for each case, below or above its POU's attribute as `#[budget]` is:
  `#[bench_case(x = 1.0e6, alpha = 0.001)]`. The attribute is not called `bench`, as this
  specification first had it: `bench` is Rust's own benchmark attribute, which the prelude's glob
  import cannot shadow. A case gives values to inputs of the POU, a function's or a method's
  parameters and a function block's or a program's `#[input]` fields, and, except for a program,
  whose instances the configuration declares, to its in-outs, whose variable the bench provides;
  each value is written as `#[init(...)]` writes it (RU-26). An input or in-out that the
  case does not name takes its declared initial value, and a POU without a case is measured with
  one case of its declared initial values. The build refuses a case that names anything else, a
  name twice, or a value its type cannot take, at the case's line.
- **The run.** `trust-dev rust bench` builds the project's program with its crate as a debug image,
  in which each Rust POU's longest call is recorded (RU-49), and runs it in the simulator, on the
  computer, or with `--device <endpoint> --target <target>` on a device by stepped scans, whose
  replies give the records (specification 11 §6.8.1), as `trust-dev test` runs the project's tests
  (specification 22). On a device, a program's case gives values of the kinds a step's inputs
  carry (a `BOOL`, an integer, a bit string, a real, a `TIME`, an `LTIME` or a `STRING`); a value of
  another kind ends the case as a fault does. Each case calls its POU once a scan for 1,000 scans, or `--scans <n>`, on one
  instance, so that a function block keeps its state from one call to the next as on the PLC: a
  function's or a function block's case from a test program the bench generates, a method's on an
  instance of its block at its declared initial values, and a program's on each of its instances,
  those the project's configuration declares or, in a project without one, its one instance
  (specification 04), with the case's inputs written into the instance before each scan of its
  task. The bench measures the methods that Structured Text can call, `PUBLIC` and
  `INTERNAL` ones; a `PRIVATE` or `PROTECTED` method of a Rust function block is called only by
  its block's Rust code, since no Structured Text extends a Rust block (RU-34), so its time is
  part of its caller's and it has no measurement of its own. A POU's measured time is its longest
  call over all its cases. A case whose call ends with a fault measures nothing: the command names
  the case and the fault and records no time for its POU. The command also names each POU whose
  longest call exceeds its budget for the target (RU-46).
- **The record.** For each POU it measured, the command writes the entry of the target it ran on
  into `[timing.measured]`, keyed as `[timing.budgets]` is (a method by its block's and its own
  name, `"AXIS.JOG"`): `longest`, the measured time in nanoseconds; `code`, the hash of the code it
  measured (below), which identifies the image; and `device`, where it ran: `computer` in the
  simulator, and on a device the runtime ID that its `status` reports. The command keeps every other
  entry and every comment of the file.
- **A current measurement.** `code` is the SHA-256 of what the image is built from: the project's
  Structured Text sources, the files of its crate that cargo reads (its `Cargo.toml`, its
  `Cargo.lock` and the files below `src/`), the version of the bundle, which fixes the SDK, the
  vendored crates and the pinned `rustc` (§16), and the target. The bench's generated test programs
  and the choice of a debug or a release image are no part of it, so that the image the bench
  measures and the image a deployment builds have the same `code`; a change to any of these files
  makes the measurement older than the image. A build computes it from the same files. The
  deployment's device type is the build's target.
- **Production.** In a `production` profile (RU-47) the build refuses the image when a Rust POU or
  method that the tasks' scans can call, one whose budget RU-47's sum counts, has no current
  measurement for the build's target, and names each such POU and the command that measures it,
  `trust-dev rust bench`, with `--device` and `--target` for a device's target. RU-47's sum then
  takes for each of them the larger of its budget for the target (RU-46) and its measured time.
  The images that `trust-dev test` and `trust-dev rust bench` build hold test programs and are
  never deployed, so they need no measurement; their sum takes the budgets. A `development`
  profile neither requires measurements nor uses them.

**RU-49. On the device.** The kernel measures each Rust POU's call time in a debug image and reports a call that exceeds its declared budget as the event
`rust_budget_exceeded` with the POU's name, the time and the budget (specification 11's events),
without ending the scan; the watchdog (NX-11) still ends a scan that overruns the task. The
generated code of a debug image reads the kernel's monotonic clock (specification 44 §8.2,
`clock`) before and after each call of a Rust POU and keeps, in the kernel's record of the POU, its
longest call and its calls over its budget for the image's target (RU-46, with `trust.toml`'s
lowering), which the image's Rust POU table gives (specification 44 §5.13). The generated code
names a POU only by its number in that table, the same in every target's image, so that layer 1
stays one text (RU-39). A release image measures nothing. A POU that runs over its budget in every
scan would displace every other event of the store, as the four events of every scan would
(specification 11, the event store), so the kernel reports a POU's first call over its budget and,
after it, only a call over its budget that is longer than every call of the POU before it;
`tasks.stats` gives each Rust POU's budget, longest call and calls over budget (specification
11).

## 11. Tools

### 11.1 VS Code and the language server

**RU-50.** The language server treats `src/generated/rust_pous.st` as part of the project. The
file is read-only: the extension marks it so for VS Code (`files.readonlyInclude`), which refuses an
edit of it, and its header names the Rust source to change instead. "Go to Definition" of a Rust POU,
or of a method of a Rust function block, opens the Rust item in its `.rs` file, from its attribute
to its name, and "Go to Declaration" its generated declaration. The extension recommends rust-analyzer for a project with a crate and writes
its settings (§16.3); it never needs it.

**RU-51.** Create Project (specification 25 §3.2) offers three starts: a Structured Text project as
today, a Structured Text project with a Rust crate (`rust/` with one example function block and its
call in `Main.st`), and a Rust-first project (§14). `trust-runtime new --rust` and `--rust-first`
make the same.

### 11.2 Debugging and monitoring

**RU-52.** A Rust POU's body is one step for the debugger (specification 44 §13.2): a breakpoint is
set on Structured Text statements only, "step into" a call of a Rust POU steps over it, and a pause
inside a Rust body happens at the next Structured Text statement. The debugger shows a Rust
instance's variables (RU-27) in Variables, Watch and Live Values, where they are written and forced
as any variable is (specification 44 §13.3), with the Rust POU's source as the instance's location.

**RU-53.** A fault in Rust code is reported as specification 44 §8.6 reports a fault, with the
calling statement and, beside it, the Rust POU's name, the fault's code and detail, and for
`RustFault` the number the POU chose. Live Values, the runtime's status and the debugger show it
with these.

**RU-54.** Global variables and directly represented variables reach a Rust POU only through its
interface: inputs, outputs, in-outs, or a program's connections (IEC Table 62). A Rust POU does not
name a global by name: what a Rust POU reads and writes is what its declaration says, which keeps a
Rust POU testable alone (§12) and its effects visible in the generated declaration.

### 11.3 The HMI, services and the control link

**RU-55.** Every variable of a Rust instance is in the symbol table, so HMI bindings (specification
39), the control link's variable reads and writes, the historian and the services (§13) reach it by
its path, as they reach a Structured Text variable.

## 12. Testing a project with Rust

**RU-56. Unit tests in Rust.** The project's crate has Rust tests (`#[test]`), which a user's own
`cargo test` runs on the computer with the standard library (RU-19). `trust_plc::test` gives them a
`Cx` of a simulated task (its logical time, interval and task) and runs a function block's or a
program's call as the kernel does: `let mut fb = FilterEma::new(); fb.x = 1.0; fb.call(&mut cx)?;`. A unit test runs
the POU's Rust code, not the program. `test::task(interval)` is the context of the task 0 at the
logical time 0 with that interval, and `test::advance(&mut cx)` moves its logical time on by the
interval, as the task's next activation does; `test::cx()` is an activation outside a task, at the
logical time 0 with the interval `T#0s`. A function's or a method's in-out takes a variable of the
test's own, a `Cell` of a value of any of its types, a structure's too:
`InOut::new(&Cell::new(point))`, which the call reads and writes as it does the caller's variable
(RU-29). A unit test's context holds no program, whose memory the Structured Text a Rust POU calls
or holds runs on (RU-38, RU-39): such a call ends with the fault `ModuleInternal`, and a test of it
runs the program, a machine test with `trust-sim` (RU-58), as a PLC's unit tests of Structured Text
run in the PLC.

**RU-57. Project tests.** Structured Text test POUs (specification 04) call Rust POUs like any POU and
run with `trust-dev test`, which builds the project with its crate through the bundle and runs the
tests in the simulator (specification 22). A Rust test of the project's crate that needs the whole
program uses `trust-sim` (RU-58).

**RU-58. Machine tests in `cargo test`.** `trust-sim` runs a project's program with its simulated
plant under a Rust test: the test writes and forces values, runs cycles or logical time, reads
outputs and any variable by its path, and sees faults:

```rust
#[test]
fn guard_raises_alarm_above_90() -> trust_sim::Result<()> {
    let mut plant = trust_sim::Project::open("..")?.simulator()?;
    plant.write("P1.level", 95.0)?;
    plant.run_cycles(3)?;
    assert!(plant.read::<bool>("P1.alarm")?);
    Ok(())
}
```

- **The runtime that runs it.** The installed trust-runtime runs the program, in a simulator process
  of the test's own (`trust-runtime simulate`, specification 11 §6.8.1, "Simulator for machine
  tests"): the project's build and its simulation (`simulation.toml`, specification 46) on the
  runtime's own scan loop, as `trust-runtime run --simulation` runs them, with the plants' work
  before and after each scan (specification 46 §2) and the writes, forces, faults and restarts of a
  run. Its logical time moves only when the test asks, so a test gives the same result on every run
  and every computer, and an hour of plant time takes only the time its scans take. `trust-sim`
  holds no runtime code: it is a client of the simulator over the process's standard input and
  output (`trust-runtime-client`), and needs no port, socket or token. Each simulator is a process
  of its own, which ends when the test drops it, so the tests of one `cargo test`, which run in
  parallel, never share one (§21, decision 15).
- **Finding the runtime.** `Project::open` takes the runtime that the environment variable
  `TRUST_RUNTIME` names, else `trust-runtime` on the `PATH`, else the runtime of the newest truST
  extension in the extension folders of VS Code and its editions. It refuses a runtime whose version
  is not `trust-sim`'s own, naming both versions and the runtime's path, since the SDK's crates and
  the runtime are released together (RU-12); a runtime it cannot find is an error that names the
  places it looked. A test may name its runtime instead (`Project::open_with_runtime`), as a CI job
  that builds trust-runtime itself does; that runtime's version is checked alike.
- **The build.** `Project::open` builds the project with `trust-runtime build`, the build the
  extension runs (the generated Structured Text and the project's crate through the bundle), once
  in a test process however many of its tests open the project. A build that fails is the test's
  error, with the build's messages.
- **Time and cycles.** The simulator starts the program cold at the logical time 0. A cycle is one
  cycle of the resource, as `trust-runtime run` runs it: one every `cycle_interval_ms` of logical
  time (`runtime.toml`), with the tasks due then in priority order (specification 10 §4.3) and then
  the programs without a task. `run_cycles(n)` runs `n` cycles; `run_for(t)` runs the cycles due
  within `t` of the current logical time and ends at its end; `now()` is the logical time. The wall clock that `CURRENT_DT` reads is the logical time from
  1 January 1970 UTC, so that it too is the same on every run.
- **Values.** A path names a variable as the debugger and Live Values name it (`P1.level`, a global
  by its name), and a value is a Rust value of the variable's type: `bool`, the integers, `f32` and
  `f64`, text, and `trust_plc`'s times and dates. `write` writes before the next cycle, as the
  debugger's `set` does, and `force` holds a value until `release`, as Live Values' `var.force`
  does, both on the variable's `global:<path>` target (specification 11 §6.8.1, "Variable mutation
  targets"); `read` reads what the last cycle left (`variables.read`). A path that names
  no variable, or a value its variable's type does not take, is an error that names the path, the
  value and the type. `call` calls a method of an instance through an interface its type
  implements, as a screen's command does, and returns the method's result (specification 11
  §6.8.1, `method.call`): a test presses a sequence's Start as the operator does. Its inputs are
  given by name, each a Rust value as `write` takes one (`Inputs::new().with("Setpoint", 21.5)`),
  and an input not given takes its declared initial value; a method without a result returns `()`,
  and a result of another kind than the test asks for is an error that names the method.
- **Faults.** A cycle that faults ends `run_cycles` or `run_for` there, at that cycle's logical
  time, which `now()` then gives, with an error that names the fault and, when the image has them,
  its POU, the statement's file and its line (specification 11, the fault entry), such as `the
  cycle at 5s faulted: division by zero in Main at src/Main.st:17`, and the project's fault policy then acts as in a run (`[runtime.fault]`):
  a halting fault leaves the resource faulted, with its safe state applied for `safe_halt`, until
  the test restarts it warm or cold (a run in the meantime runs no cycle, while `run_for`'s time
  passes), and a test that expects the fault matches the error and reads the values.
- **Documentation and examples.** The crate's API reference shows each of these in a whole example
  that runs as a doc test, and the user guide teaches, step by step, a machine test of a realistic
  example project, whose machine tests CI runs: `examples/room_heating/machine-tests`, a crate of
  the workspace that names trust-sim at the release's version as a user's tests do, which CI's test
  job runs with the trust-runtime its build made named in `TRUST_RUNTIME`. The version bump moves
  that pin with the release, as it moves an example crate's pin of `trust-plc` (RU-08, RU-12).

**RU-59. Cycle traces.** A trace records a resource's cycles so that a replay can run them again
(RU-60). It holds the program's state at its first cycle, and for each cycle everything that came
into the program from outside and what the program put out. The runtime records a trace on demand
(`trust-runtime ctl record`). When `runtime.toml` names a length (`[runtime.trace]
flight_recorder`), it also records always, into a ring that it keeps as the fault's trace when the
resource faults. A trace is a `.trusttrace` file (Appendix C); specification 11 §6.8.1, "Cycle
traces", gives the requests.

- **Its start.** The program's memory at the start of the trace's first cycle, before that cycle's
    writes: the areas of specification 44 §7.1, `%I` to `P`, without the stack. It also holds the
    forces then in effect, and the recorded image's tables that name that memory: its area, task, I/O
    binding, symbol and type tables and its string pool (specification 44 §5.5 to §5.8), with which a
    replay carries the state into another build (RU-60).
- **Each cycle.** A trace holds, for each cycle:
    - the cycle's logical time;
    - each task activation in it, in the order the tasks ran, with the task's logical time
        (`TIME()`, the kernel's `now_ns`, specification 44 §8.2);
    - every change made to the program from outside before the cycle, in the order the runtime
        applied it, with its source. A change made between two cycles, such as a method call or a
        screen's write (specification 39 §18), is the next cycle's; one made before the trace's
        first cycle is in its start state:
        - a variable's write: the control link's `set`, `var.write` and `variables.write`, a
            screen's write, Control Studio's, a service's (RU-63), the mesh's, a protocol server's
            such as OPC UA's, or a plant's, the measurement it gives a `Control.PID` instance
            included (specification 46 §13.2);
        - a force set or released, on a variable or on an address, in the cycle it is set or
            released before, not in each cycle it holds;
        - an address's write: a one-shot write, the control link's `io.write` or Control
            Studio's, or a plant's feedback at an address;
        - a method call, a tool's (`method.call`) or a screen command's (specification 39 §21.2),
            with each input as a Structured Text literal: a tool's as its request gave it, and a
            screen's as IEC 61131-3 writes the value (Tables 5 to 8), an enumeration's value after
            its type's own name and `#` (`Mode#Auto`, §6.4.4.2); and each reading of the wall
            clock (`CURRENT_DT`) that the call made, in order, since a call between two cycles
            reads the clock between them;
    - the input image as the cycle's scan sampled it, and the output image the cycle wrote, `%I`
        and `%Q` whole;
    - each reading of the wall clock (`CURRENT_DT`) in the cycle, in order. The wall clock is the
        one input from the platform that is not memory (specification 44 §8.2), so a replay gives a
        program what it read;
    - the fault that ended the cycle, if one did.
- **Its identity.** The running image's SHA-256, its sources' hash and its target (specification
    44 §5.2, offsets 32 and 20), the device's identity and platform (`runtime_id`), the runtime's
    version, the resource's name and cycle interval, the runtime's count of cycles at the trace's
    first, the number of cycles, and why the trace ended.
- **On demand.** `trust-runtime ctl record (--cycles <n> | --duration <t>) [--task <task>] [--out
    <file>]` records the next `n` cycles of the resource, or those in the next `t` of logical time.
    With `--task` it counts the runs of that task instead, and still records every cycle until
    then, since a replay needs them all. It writes the trace to `<file>`, by default
    `trace-<UTC date and time>.trusttrace` in the current folder, and prints its path, cycles and
    size; Ctrl+C ends the recording and keeps what it holds.
- **When a recording ends early.** A recording ends early when the resource stops, restarts or
    faults (the faulted cycle is the last), when the program changes online, and when the program
    pauses at a breakpoint, a pause or a step. The trace holds the cycles until then and says why it
    ended.
- **The flight recorder.** With `[runtime.trace] flight_recorder = <n>`, the runtime always keeps
    the last `n` to `2n` cycles. It keeps their records, and the program's state at the first of
    them, which it takes every `n` cycles: the memory and the forces as the cycle before that one
    left them, so that a change made between the two cycles, which the start state of a recording
    on demand holds, is the first cycle's change here. A replay applies it before the first cycle
    either way. A record that does not fit ends its `n` cycles early; the fault's trace then starts
    at the next `n`, since it cannot join cycles across the lost one.
    - When the resource faults, it keeps those cycles up to the faulted one as the fault's trace, and
        `trust-runtime ctl fault-trace [--out <file>]` saves it.
    - A runtime with a folder of its files (specification 11 §6.8.0a) also writes the trace there,
        as `traces/fault-<UTC date and time>.trusttrace`.
    - A chip keeps the trace in its memory until a reset.
- **Memory.** A recording, and the flight recorder at the resource's start, reserve their memory
    before they begin. A runtime that cannot give it refuses the recording, or starts without the
    flight recorder, and names the bytes needed and the bytes it has. A device's limit is reported,
    never cut (specification 34). A trace's records go into that memory between the phases of a
    cycle, which allocate nothing (specification 34 MEM-09). A recording whose records outgrow it,
    as many writes of long texts can, ends `memory_full`. One rule sizes its rooms on every
    runtime: a computer's (64 KiB of changes and 256 wall-clock readings in a cycle, 64 KiB of
    forces at the start, 64 KiB for one written value, and 4 MiB more for the whole recording),
    each at most a share of the memory the runtime has, where it knows it: a sixteenth for a
    cycle's changes and for the forces, a sixty-fourth for a cycle's readings, a thirty-second for
    one value and a quarter for the recording. The memory a chip has for a recording is its free
    heap less what it keeps beside any program (specification 34 MEM-12) and less what an online
    change of the program that runs takes beside it, the program's figure less its stack bound
    (specification 44 §12.3), so that a recording, and the trace it keeps when it ends, never takes
    the working memory the chip needs nor an online change's: a recording whose rooms together
    would need more, as one of many cycles can, is refused before it reserves any, with the bytes
    it needs.
    The free heap is the heap's data regions, which a recording's rooms come from however large
    they are; a chip's program region holds programs only (specification 44 §10.3). The flight recorder's two recordings each take
    the shares of half that memory, so that together they take what one recording would, and a
    recording on demand still fits beside them. A chip thereby records within its memory, and a
    cycle that outgrows its room ends the recording `memory_full` there as on a computer. A chip
    keeps an ended trace's records and serves its file part by part from them, the same file a
    computer writes.
- **Every runtime.** A chip records, keeps and serves traces as a computer does (specification 34
    STATE-08).

**RU-60. Replay.** `trust-dev replay <trace> [--project <path>]` builds the project from its
sources as they are now and replays the trace in a simulator (`trust-runtime simulate`, RU-58),
without the simulation's plants.
The replay starts from the trace's state. It runs cycle by cycle with the trace's logical times,
task activations, changes from outside, input images and wall-clock readings, and compares each
cycle's output image with the trace's (specification 11 §6.8.1, `simulator.replay`).

- **The start.** A build whose area, symbol and type tables equal the trace's lays its memory out
    as the recorded program did, and takes the recorded memory byte for byte. Another build takes it
    as an online change carries a running program's state into a new one
    (specification 44 §12.2, the copy plan, worked out between the trace's tables and the build's
    image). Every variable the build declares under the same name, with a type that holds its value,
    starts with it; the rest start with their starting values; references follow their variables.
    The report lists what the build could not carry over, as an online change's
    `online_change_value` events do. The trace's forces start in effect.
- **Each cycle.** The simulator runs the trace's cycle at its logical time:
    - It activates the build's tasks of the same names, at the trace's task times, in the trace's
        order. A task the build lacks is reported once and skipped.
    - It applies the cycle's changes from outside before the cycle, as the runtime applied them. A
        method call reads the wall clock as it read it when it was recorded: its readings in
        order, the last one again when the build's call reads the clock more often; a call that
        read none reads the last reading the replay gave. A variable, address or method the build
        lacks is reported once and skipped.
    - It gives the scan the trace's input image, the bytes both images have at the same offsets,
        and the trace's wall-clock readings in order, the last one again when the build reads the
        clock more often.
    - It compares the output image after the cycle with the trace's, by the trace's located output
        variables at their addresses.
- **The result.**
    - **A replay that matches** every cycle prints the cycles it ran and ends with 0.
    - **At the first cycle whose outputs differ,** it reports that cycle and its logical time; each
        output that differs, with its name, its address, the trace's value and the build's; and how
        many later cycles differ too. It ends with 1.
    - **A trace or build it cannot replay** ends it with 2 and the reason.
    - **A trace that ended with a fault** expects the build's fault at the same cycle. A fault in
        another cycle, or none, is a difference.
    - **What was replayed.** The report names it, from the sources' hash and target of the trace and
        of the build (an image's build time makes every build's SHA-256 its own):
        - the same program: the same sources, built for the same target;
        - the same program for another device: the same sources, another target;
        - a changed program.

        For the first two, NX-04 makes every difference a determinism fault or an input the trace did
        not hold, and the report says so.

- **In `cargo test`.** `trust_sim::Replay::open(path)?.run(&project)?` replays a trace in a
    simulator of the project, as RU-58 runs one. It returns `Ok(())` when the outputs match, and
    otherwise an error with the report. A trace from commissioning thereby becomes a test of the next
    change. `sim.fault_trace()?` gives the flight recorder's trace of the simulation's last fault (the
    project's `[runtime.trace] flight_recorder`): the bytes of its `.trusttrace` file, each part's
    CRC-32 and the whole file's SHA-256 checked as `trust-runtime ctl fault-trace` checks them, or
    `None` before a fault. A test that faults on purpose thereby keeps the cycles before the fault,
    and a replay of them shows the fault again.
- **Documentation and examples.** The user guide's machine-test page (RU-58) teaches recording a
    trace of the example `room_heating` and replaying it. The example keeps that trace with its
    machine tests, which replay it in CI, and the guide says how to record it again after a change
    that is meant to change the outputs.

## 13. Rust services (tier 3)

**RU-61.** A service is an ordinary Rust program (`[[bin]]`, the standard library, any crate,
threads, `async`), built with the user's own Rust, which runs on a computer beside a runtime and
reaches it only over the control link (specification 11 §6.8). None of §5's rules applies to it; it
can never stop or slow a scan. Building it takes the computer's standard library and the computer's
linker (Xcode's command-line tools on macOS, a C compiler on Linux, Visual Studio's build tools on
Windows), as any Rust program does: rustup gives the first and names the linker it needs. The
toolchain bundle builds programs only; it carries neither (RU-74), and truST cannot ship the
linkers (owner, 20 November 2026).

**RU-62. The SDK.** `trust-service` gives a service a connection to a runtime through the control
client trust-runtime uses (`trust-runtime-client`, the shared client of specification 11 §6.8), with
the runtime's authentication, and typed access to the program's variables by their paths: read,
write, subscribe (a read at a period the service chooses, of a set of variables in one cycle's
snapshot), with every value's cycle, logical time and quality: a read is specification 11 §6.8.1's
`variables.read`, one completed scan's values of the set, and a write its `variables.write`, of the
variables the service's `writes` names (RU-63), which the next cycle's start writes together. The bindings are generated from the
symbol table of the deployed image (`trust-dev service bindings`), so a service names
`plant::P1::level` and gets an `f64`.

A service connects by its name, `Service::connect("recipe")`: the SDK reads `[services.<name>]` of
the project's `trust.toml`, in the folder `TRUST_PROJECT` names or else the current folder, and takes
its `endpoint`, the service token in its `credentials` file (a path relative to the project,
specification 11 §6.9.6) and its `heartbeat`; `Service::connect_with` takes the same settings from
the service's own code. `endpoint` is optional. Without it the service connects to the endpoint that
the project's running runtime published in its endpoint file (specification 11 §6.8.0a), as every
client finds it there: every running runtime publishes the endpoint it bound, its platform's default
with the free port chosen at its start included, so a project's service needs no endpoint of its
own, and its runtime needs no TCP endpoint and no token in the project's files. The service still
presents its own service token, never the token its runtime generated. Without `endpoint`, a project
whose runtime has published no endpoint file, as when it is not running or could not write the file
(specification 11 §6.8.0a, `control_endpoint_file_unavailable`), is refused with `[services.<name>]
names no endpoint, and no runtime of the project has published one: no endpoint file at <file>`, and
one whose endpoint file does not read with the reason; a service that names its runtime's endpoint
is never sent elsewhere. A connection answers one request at a time. `Service::read` gives one
completed scan's values with the scan's number, its logical time, the resource's state and the
quality, each value read as a Rust value of its declared type (`bool`, the integers, `f32` and
`f64`, `String` and, from a `TIME` or an `LTIME`, `Duration`); `Service::write` writes a set of
values; `Service::subscribe` reads a set at the period the service chooses, each read one completed
scan; `Service::start_heartbeat` sends `service.heartbeat` at the service's `heartbeat` period, naming
that period (`period_ms`), on a connection of its own until it is dropped, so that the service's
own work never delays its heartbeat (RU-63), and its runtime can tell how old its last heartbeat
may be (RU-64; specification 11 §6.8.1a, "Service clients"). A heartbeat that came late, as a slow runtime answers it, is followed one period
later by the next, and the ones it missed are not sent, as a task does not replay missed activations
(specification 10 §4.3); a period of zero is refused. `trust-service` and `trust-runtime-client` are published on crates.io with every
release, at its version, so that a service, which the user's own Rust builds, gets them there
(RU-61).

`trust-dev service bindings <name> --out <file>` writes the bindings of the service `name`: a Rust
module, generated from the symbol table of the image the project's build writes (`program.tpi`), that
names each variable of the program the service can read, so that the compiler checks a service's
paths and types against the program. A program instance is a module named as the configuration
names the instance (`P1`), a global a constant of the file's top module, a structure's or a block
instance's variable a module of its members, at any depth, and each elementary variable a constant
named as the program declares it (`level`), whose Rust type is its declared type's (`bool`, the
integers, `f32` for a `REAL`, `f64` for an `LREAL`, `String` for a string, a character and an
enumeration, `Duration` for a `TIME`, `i64` for a date or a time of day, its count as
`variables.read` gives it), so that `plant::P1::level` reads an `f64`. An array and a
reference are left out, as they hold no single value; a service reads an element by its path. A
variable the service's `writes` names is a `Writable` handle, and only such a handle can be given to
`Service::set`, so that a service whose code writes another variable does not compile. The file
begins with the image's source hash; a build whose image no longer has a variable the bindings name
makes the service's next read of it the runtime's refusal, and running the command again makes the
compiler name it.

**RU-63. Freshness.** Every service in `trust.toml` `[services.<name>]` has a heartbeat variable that
the build declares as a global (`<name>_heartbeat : UDINT`) and the runtime adds one to each time the
service's SDK sends `service.heartbeat` with the service's token (specification 11 §6.8.1), which it
does at the period the service states (`heartbeat = "500ms"`). The globals lie in the generated file `services.st`
(`src/generated/services.st`, in a Rust-first project `src/generated/trust/services.st`): one
`VAR_GLOBAL` with a variable per service, in `trust.toml`'s order. `trust-dev rust declarations`
writes it, and removes it when `trust.toml` declares no service, and the build refuses a project
whose committed file is not what `trust.toml` declares, naming the command (RU-24), whether the
project has a crate or not (RU-07). A service's name is an identifier, so that its global is one.
A service's `writes` lists the variables it may write (`writes = ["Recipe.target", "Line.speed"]`),
by their paths as `variables.read` names them (specification 11 §6.8.1); every other variable is
the program's own. The build refuses a path that names nothing, a constant, an instance or a value
that is not elementary, and a variable that a service gives twice or that two services give, as a
variable has one service that writes it, naming the service and the path, and the signed
image carries each service's `writes` (specification 44 §5.14), so that a chip holds a service to
them as a computer does. A service without `writes` writes no variable but its heartbeat. A
`[services.<name>]` holds only the settings Appendix B lists, `package`, `heartbeat`, `writes`,
`endpoint` and `credentials`: the build refuses another key, naming the service and the key, as
runtime.toml refuses an unknown key, so that a misspelt setting is never lost. The program library's function block `SERVICE_FRESH` (its
inputs the heartbeat and a timeout, its outputs `FRESH : BOOL` and `AGE : TIME`; specification 08
§5A) is the standard way
a program gates on a service's values; a program decides itself what it does when a service is not
fresh. A value a service writes is never more than the program's input: the scan never waits for a
service.

**RU-64. Running.** A service runs as its own process under the computer's service manager, and the
runtime never starts a service itself. `trust-dev service install <name>` puts the service of
`trust.toml` `[services.<name>]` under the system's service manager and starts it: from then on it
starts at every boot, without anyone logging in, and again 2 s after it ends, as the runtime's own
systemd unit restarts the runtime (`docs/deploy/systemd`). `trust-dev service uninstall <name>`
stops it and removes it. On every computer the service is called `trust-service-<name>`.

- **Rights.** Both commands change the computer's services, which takes root (Linux, macOS) or an
  administrator (Windows): on Windows, the right to create a service at the service manager. Without
  these rights they change nothing and print the command to run with them: `sudo` with trust-dev's
  own path, the project folder and the binary, so that the command needs neither the root account's
  `PATH` nor cargo; on Windows the same command without `sudo`, to run in a terminal opened as
  administrator. `install` checks everything below before it asks for rights.
- **The program.** `install` builds nothing, since under `sudo` a build would leave files of root
  in the project. It runs the binary that cargo's release build of `[services.<name>] package`
  wrote: the project folder holds that package's manifest (`cargo new --bin <package>` in it, or a
  member of the project's own Cargo workspace), and cargo's metadata names the package's one binary
  and its target folder, `<target>/release/<binary>`. It refuses a package the project does not
  hold or holds twice, a package without exactly one binary, and a binary that does not exist,
  naming the build: `cargo build --release --manifest-path <manifest>`.
- **Its settings.** `install` refuses a service whose `credentials` file does not exist, naming
  `trust-runtime ctl service-token <name> --out <file>` (RU-81): it could not connect. The service
  reads its settings itself when it starts: its working folder is the project folder, and its
  environment's `TRUST_PROJECT` names that folder, so that `Service::connect` reads the project's
  `trust.toml`, token and, without `endpoint`, its runtime's endpoint file (RU-62), and a new token
  needs no new install. A service without `endpoint` that starts before its runtime has published
  its endpoint, as at a boot, ends with RU-62's refusal and is started again 2 s later, until its
  runtime runs; a runtime that starts again on another port ends the service's connection, and the
  service finds the new port the same way.
- **Its account.** A service never runs as root or as Windows' LocalSystem. On Linux and macOS it
  runs as the user who owns the project folder, and `install` refuses a project folder of root; on
  Windows it runs as its own virtual account (below).
- **One service of a name.** A computer has one `trust-service-<name>`. `install` refuses it while
  it is installed for another project folder, naming `trust-dev service uninstall <name>`;
  installed for the same folder, it is replaced and restarted, as after a new build.
- **Linux:** a systemd unit, `/etc/systemd/system/trust-service-<name>.service`: `User=` the owner,
  `WorkingDirectory=` and `Environment=TRUST_PROJECT=` the project folder, `ExecStart=` the binary,
  `Restart=always` and `RestartSec=2`, after and wanting `network-online.target`, wanted by
  `multi-user.target`; then `systemctl daemon-reload`, `systemctl enable` and `systemctl restart` of
  the unit. Its output goes to the journal, `journalctl -u trust-service-<name>`. `uninstall` runs
  `systemctl disable --now`, removes the file and reloads.
- **macOS:** a launchd daemon, `/Library/LaunchDaemons/trust-service-<name>.plist`: `Label` the
  service's name, `ProgramArguments` the binary, `UserName` the owner, `WorkingDirectory` and
  `EnvironmentVariables` `TRUST_PROJECT` the project folder, `RunAtLoad`, `KeepAlive`,
  `ThrottleInterval` 2 s, its output and errors appended to `/Library/Logs/trust-service-<name>.log`,
  which belongs to the owner; `launchctl bootout` of an earlier one, `launchctl enable` and
  `launchctl bootstrap system` load it. `uninstall` boots it out and removes the file; the log
  stays.
- **Windows:** a service `trust-service-<name>` that starts automatically and that the service
  manager restarts 2 s after it ends, under its virtual account `NT SERVICE\trust-service-<name>`
  (never LocalSystem). Its program is a copy of trust-dev in the service's own folder,
  `%ProgramData%\truST\services\<name>\`, run as `trust-dev service run <name> --project
  <folder> --binary <binary>`, since an ordinary Rust program does not answer the service manager
  (RU-61). That copy answers the service manager and runs the binary as its child, in the project
  folder and with `TRUST_PROJECT`. It appends the child's output and errors to `logs\<name>.log` in
  the service's folder, stops the child when the service is stopped, and ends with a failure when
  the child ends, so that the service manager restarts it. The copy keeps the service when truST is
  updated, since the extension keeps trust-dev in a folder of each version. The service's folder
  also holds `service.toml` with the project folder and the binary, by which `install` knows the
  folder it was installed for and `uninstall` the access it gave. The virtual account exists while
  its service does: after creating the service, `install` gives it read and execute access to the
  project folder, the binary's folder and the service's folder, and write access only to the
  service's `logs` folder (`icacls`), so that the service cannot replace its own program, and then
  starts it. For a service installed for the same folder it stops the service, replaces the copy and
  the settings, and starts it again. `uninstall` stops the service, takes the access away while the
  account still exists, deletes the service, and removes the copy and `service.toml`; the log stays,
  as on macOS.

Devices & Connections shows a connected service as a client of its runtime, with its name and the
age of its last heartbeat. A service reaches a chip's runtime over the network as any client does;
a service does not run on a chip.

## 14. Rust-first projects

### 14.1 The project

**RU-65.** A Rust-first project is a Cargo package with `trust.toml`, and no hand-written
Structured Text:

```
cell/
  Cargo.toml          the crate (RU-08)
  Cargo.lock
  trust.toml          the configuration: tasks, programs, bindings (§14.2)
  runtime.toml        the runtime, as in every project
  io.toml             the I/O, as in every project
  trust-lsp.toml      the project for the Structured Text tools
  src/
    lib.rs            the Rust POUs
    generated/trust/  GENERATED, committed: rust_pous.st, configuration.st, services.st
  tests/              machine tests (RU-58)
```

`runtime.toml` and `io.toml` are the same files with the same tools as in every project
(specification 11, Devices & Connections): they are not generated, so every tool that edits them
works on a Rust-first project unchanged.

### 14.2 `trust.toml`

**RU-66.** A Rust-first project's `trust.toml` declares what a `CONFIGURATION` declares in Structured
Text (IEC §6.8): the resource, its tasks (`[tasks.<name>]` with `interval` or `event`, and
`priority`), its program instances (`[programs.<name>]` with `type`, the Rust program's name, and
`task`), its global variables (`[globals]`) and the bindings of variables to directly represented
variables (`[bindings]`: `"P1.start" = "%IX0.0"`, IEC's `VAR_CONFIG`). Appendix B gives the schema.

### 14.3 The generated configuration

**RU-67.** The build writes `src/generated/trust/configuration.st` from `trust.toml`: one
`CONFIGURATION` with its resource, `TASK`s, `PROGRAM ... WITH ...` instances, `VAR_GLOBAL` and
`VAR_CONFIG`, in the order `trust.toml` gives them, with the generated-file header. The file is
committed, and the build fails when it differs from what `trust.toml` gives (RU-41, step 1), so a
review sees exactly the configuration that runs. `trust-dev rust declarations` writes it with the
declarations (RU-24). The configuration and its resource bear `[resource] name`, the resource
`ON PLC`; a task is `TASK <name> (INTERVAL := <time>, PRIORITY := <n>)`, its interval a `TIME`
literal in the largest unit that holds it whole, or `TASK <name> (SINGLE := <event>, PRIORITY :=
<n>)` for an `event`, a `BOOL` global; the globals are the configuration's `VAR_GLOBAL`; and each
binding is a `VAR_CONFIG` entry `<instance>.<variable> AT <address> : <type>`, with the type the
Rust program declares. A value that Appendix B's schema does not take, a program that is no
`#[trust_program]` of the crate, a task, a variable or an address that does not exist, is refused at
`trust.toml` with its reason before anything is written. Every Structured Text tool reads it: the language
server, the run targets, Devices & Connections, the HMI Builder and the debugger work on a Rust-first
project as on any project.

**RU-68.** A Rust-first project may hold Structured Text libraries (`trust-lsp.toml`'s dependencies,
specification 19) and call them from Rust (§8), and may hold hand-written `.st` files beside the
generated ones: it is then a project of both languages, built the same way.

### 14.4 The daily loop

**RU-69.** A Rust-first project is worked with the commands of every project: `cargo test` (the
user's own Rust, RU-56, RU-58), `trust-runtime build`, `trust-runtime play` or the extension's Start
on the simulator, `trust-dev test`, `trust-runtime deploy` and the extension's run targets. There is
no separate CLI for Rust-first projects.

## 15. Sequences

**RU-70.** A Rust function block or program may run a sequence: sequential logic, which Structured
Text writes as an SFC or a `CASE` machine, written as straight-line Rust with `.await` at the points
where it waits for a later cycle:

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

**RU-71. What it is.** `#[trust_sequence]` is a restricted sequence language with Rust's syntax, not
Rust's `async`: the attribute validates the body and lowers it to a state machine. No Rust future is
stored, no executor runs, and nothing waits inside a scan: each activation runs the sequence from its
wait point to its next one and returns. The sequence's state is a variable of the instance, `_seq`,
after the POU's own variables: its wait point `point` (0 at the top, then the wait point's number,
RU-72), the logical time `began` of the activation that began the wait, `done` when the body holds
an `s.all` (RU-72), `shape_<signature>` in the POU's state, which tells an online change whether the
sequence or one of its children changed (RU-73), and each local of the body, a variable of the same name and type, so that a local keeps its
value from one activation to the next. A local names its type (`let count: i16 = 0;`),
which the attribute cannot infer, and takes no name of the state's own fields in any case: `point`,
`began`, and `done` in a body that holds an `s.all`. The name
`_seq` takes the one leading underscore that an IEC identifier may have (IEC §6.1.2; two in a row
are not allowed, so `__seq` could not be declared). The state is in the symbol table: the debugger's
Variables view shows it, and `variables.read` and the HMI read it by its path,
`<instance>._seq.point` (Live Values lists only variables with an address, specification 25 §4.3).

How the Rust declares it: an attribute reads only its own item, so the struct's attribute cannot see
an `impl` block, and an attribute on a function inside one cannot declare a type. The struct
therefore names its sequence, `sequence = "run"` (as `implements` names what its methods provide,
RU-33), and `#[trust_sequence]` stands on the `impl` block that holds the `async fn`. The attribute
on the block declares the state's type, `<struct>Sequence`, a structure of `point`, `began` and the
locals; the struct's attribute adds `_seq` of that type as the POU's last variable, refuses a
variable of its own named `_seq` in any case, and implements `Body` by running the sequence, so the
POU has no `impl Body` of its own and each activation, a program's or a block's call alike, runs
the sequence. `s` is the POU's context (`Seq` is `Cx`): plain code passes it where a function or a
block call needs the context. Between two wait points a local is an ordinary variable of the
function: the activation reads it from `_seq` when it begins and writes it back when it reaches its
next wait point, and a fault discards the activation's changes with the rest of the instance's
(RU-28). A body that returns starts again at its top at the next activation, as a program's body
runs at each activation; a sequence that must stop waits at its end (`loop { s.next_cycle().await;
}`).

The generated declarations (RU-23) declare `_seq` as the POU's last variable, an internal one of
the structure `<POU>_SEQ` in the POU's namespace, whose fields are `point : UINT`, `began : TIME`,
`done : WORD` when the body holds an `s.all`, `shape_<signature> : BOOL` in the POU's state, and the
locals with their
types; the program's code builds the state from `<struct>Sequence` and
writes it back as it does any structure of the crate (RU-25), so that type must be reachable as
the POU is: a POU re-exported from a private module re-exports its state's type beside it. Each
child sequence (RU-72) has a state of its own, laid out as the POU's sequence's (`point`, `began`,
`done` when its body holds an `s.all`, and its locals), in a field of `_seq` named as the child,
after the POU's sequence's locals and in the order of the block's `async fn`s, its parameters before
its locals: the debugger shows `<instance>._seq.fill.point`. Its type is `<struct>Sequence<Child>` in Rust, the child's name in
upper camel case (`PalletizerSequenceFill`), and the structure `<POU>_SEQ_<CHILD>` in the generated
declarations, the child's name in capitals (`PALLETIZER_SEQ_FILL`). The declarations' writer
refuses a POU whose `sequence` no `#[trust_sequence]` block of its struct holds or names a child
sequence of it, and a block whose sequence no POU runs, each at its place.

**RU-72. Wait points.** The body awaits only the SDK's wait points: `s.next_cycle()`, `s.until(pred)`
(true at the start of a later cycle), `s.wait(t)` (the task's logical time advanced by `t`),
`s.all(...)` and `s.race(...)` of these, a child sequence of the same instance, and `.within(t)`
on any of them, which faults with `RustFault` when the wait outlasts `t` in logical time. A wait
begins at the activation that reaches its wait point, which then returns, and completes at a later
activation, never at the one that began it: `s.next_cycle()` at the next activation, `s.until(pred)`
at the first later activation at whose start `pred` holds, and `s.wait(t)` at the first activation
whose logical time is at least `t` after the one that began it. A wait point's number is its place
among the body's wait points, from 1.

`s.all(..)` and `s.race(..)` take two to sixteen members, each `s.next_cycle()`, `s.until(pred)`,
`s.wait(t)` or a child sequence with its arguments, `self.fill(s, ..)`, which all begin with the wait
point, and each member completes as it would alone: a plain wait at a later activation, a child when
it returns.
`s.all` completes at the activation at which the last of its members completes, a member that has
completed staying complete, as simultaneous sequences evolve each on its own and converge once each
has reached its end (IEC §6.7.5, Table 61, features 4a and 4b): `s.all(s.until(a), s.until(b))`
waits for `a` and for `b`, each at an activation of its own, where `s.until(|me| me.a && me.b)` waits
for one activation at whose start both hold. `s.race` completes at the first activation at which one
of its members completes, the first of them in the members' order when several do, as a selection of
sequences takes the first of its transitions that holds from left to right (IEC §6.7.5, Table 61,
feature 2a). `let won: u16 = s.race(..).await;` keeps that member's number, from 1, in the local
`won`. The state of a sequence whose body holds an `s.all` has `done : WORD` after `began` (RU-71):
the members of the `s.all` that the sequence waits at that have completed, bit 0 for the first, and 0
at every other wait point.

A child that is a member runs as part of its wait point. The activation that reaches the wait point
starts each child member in the members' order, from its top with its arguments, as a child awaited
alone starts, and each later activation that checks the wait point resumes each child member that has
not returned, in the members' order with the plain members. So `s.all(self.left(s), self.right(s))`
runs two child sequences side by side, as an SFC's simultaneous sequences of several steps each run
(IEC §6.7.5, Table 61, features 4a and 4b), and `s.race(self.fill(s), s.until(|me| me.stop))`
abandons the filling where it waits when the stop comes first: a child that loses a race is not
resumed, keeps what it last wrote, and starts at its top the next time it is awaited. A child member
that returns at the activation that began the wait point has completed there: an `s.all` all of whose
members do completes at once, and an `s.race` completes at once with the first child member in order
that does, starting no member after it. A child is a member of one `s.all` or `s.race` once at most,
as it has one state.

A child sequence is another `async fn` of the same `#[trust_sequence]` block, which takes the context
and, after it, parameters of the types a local takes (`async fn move_to(&mut self, s: &mut Seq,
target: f64) -> Result<(), Fault>`), and which a sequence of the block awaits as a wait point of its
own with an argument for each parameter, `self.move_to(s, 25.0).await?;`: the child's fault is the
fault of the sequence that awaits it. A parameter is a variable of the child's state, as a local is:
the activation that reaches the wait point evaluates the arguments once, before the child starts,
and sets the parameters from them, which keep their values across the child's waits. A child runs as a call would: the activation that reaches the wait
point runs the child from its top to its first wait point, and the sequence that awaits it goes on
at the activation at which the child returns, at once when the child returns without waiting, so
moving part of a sequence into a child changes nothing about when anything happens. While the child
waits, the sequence that awaits it waits at that wait point, which began at the activation that
reached it, for its `.within(t)` (`self.fill(s).within(t).await?`). Each time it is awaited, a child
starts at its top with its state as a restart leaves it (RU-73), its parameters then set. The POU's
sequence, which its struct names (RU-71), is the one `async fn` of the block that no other awaits,
and takes no parameter, as its POU's activation runs it; every other is a child, and no sequence
awaits itself, directly or through other children, as each sequence has one state.

`.within(t)` faults at the first activation whose logical time is more than `t` after the one that
began the wait, a wait of exactly `t` being still in time, with the wait point's number as the
fault's number and `t` and the time waited, in milliseconds, as its two numbers (Appendix A), as an
SFC names the step that overran its time. It stands last before `.await` and limits the whole wait
point, the members of an `s.all` or an `s.race` together. An activation that resumes a wait checks
`.within(t)` first: a wait that would complete more than `t` after it began faults, as it came too
late. It then checks the wait, an `s.all`'s members that have not completed and an `s.race`'s until
one completes, in the members' order. A wait point's arguments, `t` and the predicate, are evaluated
at each activation that checks them. A predicate is a plain function of the instance, without a wait
point. A wait point stands as a statement of its own (`s.wait(t).await;`, `....within(t).await?;`,
`let won: u16 = s.race(..).await;`). The attribute refuses any other `.await`, an `.await` inside
another expression, an `async` block or closure, a future of another crate, a loop that can run
without reaching a wait point (a child sequence counts as one only when every way through it waits
before it returns, an `s.race` only when every child member does, and an `s.all` when one of its
members does), a `for` loop with a wait point in it (its iterator would have to survive the
wait), an `if let` or a `while let` with a wait point in it (its binding would have to live across
the wait), a labelled block with a wait point in it (its `break` would leave the wait behind), a
`break` with a value out of a loop that holds a wait point, an `s.all` or an
`s.race` with fewer than two members or more than sixteen, a member that is not `s.next_cycle()`,
`s.until(..)`, `s.wait(..)` or a child sequence or that has a `.within` of its own, a child that two
members of one `s.all` or `s.race` await, a value from a wait point other than
`s.race`, a local of `s.race`'s member number whose type is not `u16`, a local without its type, two
locals of one name in one sequence, a local that takes a name of the state's own fields (RU-71), a
parameter of the POU's sequence, a child's parameter that is not one name with its type or that
takes the name of a local or of the state's own fields, a child awaited with another number of
arguments than it has parameters, a child awaited without `?`, a call `self.name(s, ..)` that names
no `async fn` of the block, a sequence that awaits itself, directly or
through other children, two `async fn`s of a block that no other awaits, a child whose name is no
IEC identifier, is a reserved word, begins with an underscore or names a field of the state of the
POU's sequence (`point`, `began`, `done`, a local, another child) in any case, and a variable of the
POU named `_seq` in any case, each with its place in the body.

**RU-73. Restarts and changes.** A cold or warm restart starts every sequence at its top, as an SFC
starts at its initial step (IEC §6.7); a sequence that must resume keeps its progress in retained
variables of its own and reads them at its top. An online change keeps a sequence's state when the
changed code has the same wait points (each named by `s.step` or, unnamed, by its order) and the same
locals across them, and otherwise starts the sequence at its top and says so in the change's report
(specification 44 §12).

The state's member `shape_<signature> : BOOL` carries this, with no rule of its own in the change:
`<signature>` is eight hexadecimal digits of the 32-bit FNV-1a hash of the lines of the POU's
sequence and then of each child sequence in the block's order, each child's after a line `child
<name>`. A sequence's lines are a line per wait point, `wait <step>` with its step's name or `wait
#<number>`, a line per parameter of a child, `parameter <name>: <type>`, and a line per local, `local
<name>: <type>`, each name in small letters and each type its IEC type, in the body's order, each line
ending in a line feed.
The member starts `FALSE`, and an activation that finds it `FALSE` starts the sequence at its top,
with the state as a restart leaves it, and sets it. A change carries it by its name, as any variable
(specification 44 §12.2): when the wait points and the locals are the same, so is its name, and the
state goes on as the change carried it; when they changed, the member is new, `FALSE`, which the
change's report lists as a new variable, `<instance>._seq.shape_<signature>`, and the sequence's next
activation starts it at its top, its children's states with it. Only the POU's state has the member:
a change of a child starts the whole sequence at its top, as the sequence that awaits the child
evaluated the child's arguments when it started it and would not evaluate them again.

## 16. The toolchain and the user's own Rust

### 16.1 The bundle

**RU-74.** The toolchain bundle (specification 44 §4.6) gains, for every computer platform:
`cargo` of the pinned release; the SDK's procedural macro library (`trust-plc-macros`), which the
release builds for that platform with the pinned compiler, a Linux platform's against glibc 2.28 so
that it loads on every Linux the packages support (specification 24); the sources of the SDK crates and of
every crate they are built from, vendored, with their hashes in the bundle's manifest; and, for each
module target, the output of the build scripts of those crates (today `libm`'s only), which the
release records when it builds the program library for that target. The build gives `rustc` the
macro library as an external crate of the bundle's copy of `trust-plc`, which takes its macros from
it instead of from a dependency, and gives `cargo` each recorded output in place of its build
script, as cargo's build script override. The bundle's copies differ from crates.io's only in their
manifests: such a crate's has the `links` key that names the output, and every copy's allows the
crate's warnings, which cargo caps for a crate it takes from crates.io but not for a path
dependency, as the copies are to the build (RU-44). So a build compiles and links
nothing for the computer itself and needs no linker of the computer's: by RU-09 the SDK's macros
are the only procedural macros of the scan crate graph and the crates the SDK is built from the
only ones with build scripts, so the computer runs only the bundle's compiler and the macro library
inside it. A user's own `cargo` (RU-56, RU-77) builds the same SDK from crates.io, its macros as an
ordinary dependency and its build scripts run, so both expand the same macro code and configure the
same library code. The module targets keep only `core` and `compiler_builtins`: the scan has no `std` and no
`alloc` (RU-19). The release records the bundle's new size as NX-08 records today's.

**RU-75.** The project's crate and its dependencies are built with the bundle's `cargo` and
compiler only; the build never runs a `cargo` or `rustc` of the user's, and never touches the
user's `rustup`, `CARGO_HOME` or global configuration: it gives its `cargo` a home of its own in
truST's data folder, the folder `cargo-home` of the installed bundle (of the folder
`TRUST_TOOLCHAIN` names, for a development toolchain), and an environment without the user's
cargo settings: without `CARGO` and the `CARGO_` variables, which cargo reads as configuration,
and without `RUSTFLAGS`, `RUSTDOCFLAGS`, `RUSTC_WRAPPER`, `RUSTC_WORKSPACE_WRAPPER` and
`RUSTC_BOOTSTRAP`. The bundle's copies of the SDK and of the
crates.io crates it is built from are all such a build takes from outside the project, so its
`cargo` needs no registry.

### 16.2 Without the user's own Rust

**RU-76.** A user without Rust builds, runs, tests (`trust-dev test`), debugs and deploys a project
with Rust POUs as any project. What needs the user's own Rust is said once, when a project with a
crate is opened: rust-analyzer's help while editing `.rs` files, and `cargo test` (RU-56, RU-58).
Building a service needs it too (RU-61).

### 16.3 With the user's own Rust

**RU-77.** The project's `rust-toolchain.toml`, which Create Project writes, names the pinned Rust
version and the module targets, so a user's `rustup` gives rust-analyzer and `cargo test` the
compiler the build uses. The extension writes the project's rust-analyzer settings: the crate, the
`trust_module` configuration of a module (specification 44 §4.5) off for analysis, and checks with
`cargo check` of the computer's target. A difference between the user's Rust and the pinned version
gives a warning in the extension, never a different build. A crate that calls Structured Text finds
its bindings in `build/trust_st/` (RU-39): a fresh clone needs one `trust-dev rust declarations` or
one build before the user's own `cargo test`, which the extension does when it opens the project.

## 17. Security

**RU-78. Code that runs while building.** A project's build runs no code from a crate outside the
SDK: the scan crate graph has no build scripts and no procedural macros but the SDK's (RU-09), and
the build reads a crate's sources only with the compiler. A service's crates may have both: a
service is built apart from the program (RU-61).

**RU-79. Supply.** `Cargo.lock` pins every crate with its checksum (RU-10); the program image's
crate table (specification 44 §5.12), which its signature covers, and its deployment record
(specification 34 §5.1.1, section 9) list the scan crate graph, every crate's name, version, source
and checksum (none for a path crate, for which `Cargo.lock` pins none), so a deployed image says which
Rust code it holds. The list holds the crates the audit below checks. `trust-dev rust audit` checks the graph against the
RustSec advisory database when the computer can reach it, and against a copy of it otherwise.

The audit reads the scan crate graph as the build reads it, from cargo's metadata of the build's
package (RU-20), for each module target (specification 44 §4.4), and checks every crate of it but the
project's crate and the packages the build generates, by its name and version, against each
advisory of the database for a crate of that name. An advisory names a version that matches none of
its `patched` and none of its `unaffected` requirements, which are semantic versioning's
requirements as cargo reads them; a withdrawn advisory names none. For each advisory that names a
crate of the graph, the command prints the advisory's id, the crate's name and version, the
advisory's title and its patched versions, and, for an informational advisory, its kind
(`unmaintained`, `unsound` or `notice`). It fails when a vulnerability, an advisory that is not
informational, names a crate of the graph; an informational one is a warning. A project without a
crate has nothing to check. The database is the repository `rustsec/advisory-db` on GitHub: the
command downloads its current state and keeps it in `advisory-db/` of truST's data folder
(specification 44 §4.6), in place of the copy it kept before. When the download fails, it checks
against the kept copy and says when that copy was downloaded; without one it fails. `--db <folder>`
checks against a copy of the repository in a folder, as a computer that never reaches GitHub is
given it, and downloads nothing.

**RU-80. The image.** A program with Rust POUs is one image, signed as every image is (specification
44 §5.11); a device installs and runs it only as specification 44 §10.2 allows any image. Nothing of
the Rust code is loaded apart from the image.

**RU-81. Services.** A service authenticates on the control link with credentials of its own, a
service token that an Admin creates for the service's name (`trust-runtime ctl service-token`,
specification 11 §6.9.6), with the role it needs: a service that only reads gets a role that reads,
Viewer when none is asked for. A service token does not expire, so that a service that runs
unattended keeps its connection; creating it again replaces it.

## 18. Devices

**RU-82.** A Rust POU runs on every device a program runs on, as the owner's rule of 16 October 2026
requires: the ESP32-C6, the STM32U5A9 and the computers. The build compiles the scan crate graph
for each target of the deployment (specification 44 §4.4); a crate that does not build for a target
fails that target's build, naming the crate and the target. A Rust POU computes the same results on
each (RU-17); the C6's floating point is the compiler's software arithmetic, with the same IEEE 754
results.

**RU-83.** The memory figure (specification 44 §10.4) counts the Rust code and the Rust instances,
so a device whose program memory the Rust code exceeds refuses the image as it refuses any image
that does not fit. Services run on computers only (RU-64).

## 19. Online change, retain and restarts

**RU-84. Online change.** A change of Rust code is a change of the program's module, which
specification 44 §12 installs as any change: the copy plan carries each variable of a Rust instance
to the new program by its path and type, as it carries a Structured Text variable (§12.2), a
variable that is new starts at its declared value, and a variable whose type changed follows §12.2's
rules. A sequence keeps its state as RU-73 says.

**RU-85. Retain.** A Rust POU's `#[retain]` variables lie in `R` and are retained as Structured Text's
retained variables are (specification 44 §11, specification 11 §6.7).

**RU-86. Restarts.** A cold restart gives a Rust instance its declared values (RU-26), a warm restart
its retained ones; a Rust POU runs no code at a restart, and its sequences start at their tops
(RU-73).

## 20. Verification and milestones

### 20.1 Tests

**RU-87.** Every requirement above has a native test that names it and the wrong behaviour it would
catch (specification 22, `trust-test-authoring`), among them:

- the build refuses, each with its message naming the item: a crate without `no_std` or without
  `forbid(unsafe_code)`; a dependency with `unsafe`, a build script or a procedural macro; a crate
  that needs `std` or `alloc`; a panicking operation; a changing `static`; recursion through Rust;
  the target's math; a Rust POU without a budget; stale declarations or configuration; a cycle
  between the layers; `EXTENDS` across the languages;
- a Structured Text call of a Rust function block writes its inputs, runs its body and reads its
  outputs, and a fault from Rust leaves the instance as it was and ends the scan with the calling
  statement and the Rust POU's name;
- the symbol table names a Rust instance's variables, and Live Values, the debugger and the HMI read
  and write them;
- an online change carries a Rust instance's variables, and a retained variable survives a warm
  restart;
- a Rust POU and its Structured Text twin compute bit-identical outputs on each module target;
- a sequence's lowering, its wait points, `.within`, its refusals and its restart;
- a Rust-first project's generated configuration, its tools and its build;
- a service's typed access, its heartbeat and `SERVICE_FRESH`;
- a trace's recording, its format and its replay.

### 20.2 Examples

**RU-88.** The release ships three runnable examples, each with a README that teaches one thing
(owner, 30 October 2026): `examples/rust_function_block` (a Structured Text project with one Rust
function block), `examples/rust_first_cell` (a Rust-first project with a sequence and machine tests)
and `examples/rust_service` (a service with its heartbeat gate). Their pictures come from captures
(specification 22, "Docs Captures"). `examples/room_heating` gains machine tests of its two loops
against its plant and a cycle trace recorded from a run of it, replayed as a regression test (the
owner, 22 November 2026: "machine tests for room_heating plus a trace recorded from it and
replayed"). A user runs them as the example's README says, and CI runs them too, so that they
cannot go stale.

### 20.3 Milestones

Each milestone is a set of small pull requests (owner, 31 October 2026), accepted by its tests in CI:

| Milestone | Content |
|---|---|
| R1 | Tier 1: the SDK's types, `#[trust_fb]`, `#[trust_function]`, `#[trust_method]`, the generated declarations, the build of the project's crate, §9.4's checks, the symbol table's language mark, budgets and §10's admission, the fault `RustFault`, unit tests, `rust_function_block` |
| R2 | Tier 2: `#[trust_program]`, DEV-048 |
| R3 | Calls of Structured Text from Rust: the layers and the bindings (§8) |
| R4 | Rust-first projects: `trust.toml`, the generated configuration, Create Project, `rust_first_cell` |
| R5 | Services: `trust-service`, the heartbeat, `SERVICE_FRESH`, `trust-dev service`, `rust_service` |
| R6 | Sequences (§15) |
| R7 | `trust-sim`, cycle traces and replay (§12) |
| R8 | The bundle of §16 in the release, `trust-dev rust bench` and `audit`, the documentation and a course lesson |

## 21. Decisions for the owner

The owner's decisions of 15 November 2026 are above. The draft took the decisions below within
them, and the owner approved them with the draft on 15 November 2026 ("approved"):

1. **The bundle grows** by `cargo` and, for each computer platform, the SDK's procedural macro library
   that the release builds (RU-74), which a build of a Rust crate with dependencies needs. The draft
   had the computer's own standard library instead, with which `cargo` would have compiled the
   macros on the computer; that needs the computer's own linker, which a computer without Rust may
   lack (macOS without Xcode's command-line tools has none). The owner delegated the choice on 17
   November 2026 ("do whats most professional and best for the user and us"), and the prebuilt
   library was chosen over the build expanding the attributes itself, which would have pointed the
   compiler's messages into rewritten files instead of the user's own lines. A build script of a
   crate the SDK is built from (`libm`'s) needs that linker too, so under the same delegation the
   bundle carries each script's output for every module target, which cargo takes in its place.
2. **The SDK goes to crates.io** at every release, beside the bundle's copy (RU-12), so that a user's
   own `cargo` finds it for rust-analyzer and `cargo test`; the release workflow publishes it.
3. **`unsafe` is forbidden in every crate of the scan**, dependencies included, not only in the
   project's own code, and no third-party build scripts or procedural macros (RU-09, RU-20, RU-78).
   This rules out crates such as `heapless`, which hold `unsafe` code.
4. **Rust calls Structured Text through layers** (§8): a chain that would call back into Rust is
   refused rather than supported.
5. **The debugger does not stop inside Rust code** (RU-52): Rust bodies are one step; a native
   debugger of Rust code is outside this specification.
6. **No inheritance across the languages, no actions or properties in Rust** (RU-34).
7. **A Rust POU reaches globals only through its interface** (RU-54).
8. **Every Rust POU declares a budget**, and a `production` deployment needs measured times (§10).
9. **A Rust-first project's `trust.toml` holds only the configuration**; `runtime.toml` and `io.toml`
   stay hand- and tool-edited, not generated as the July plan had it (§14).
10. **No `trust` command of its own** (the July plan's D24): the commands go into trust-runtime and
    trust-dev.
11. **Sequences** (the July plan's D18) are part of this specification (§15).
12. **Cycle traces and replay** (the July plan's D17) are part of this specification, and serve every
    project, not only those with Rust (§12).
13. **Rust programs are a recorded IEC deviation**, DEV-048 (§7).
14. **Services run under the computer's service manager**, never started by the runtime, and never on
    a chip (RU-64). How `trust-dev service install` does it was delegated on 18 November 2026 ("do
    whats most professional and best"), compared with the runtime's own systemd unit, which an
    operator installs by hand, and with tools that put a program of the user's under the service
    manager (cloudflared's and ngrok's `service install`, the GitHub Actions runner's `svc.sh
    install` with its Windows service host): the command installs and starts the service at system
    level, so that it runs after a boot without a login, rather than only writing the files and
    printing the commands; it needs root or an administrator and otherwise names the command that
    has them; it builds nothing and takes the release binary; and the service runs as the project
    folder's owner, or a virtual account on Windows, never as root or LocalSystem.

15. **Machine tests run the installed runtime in a simulator process of their own** (RU-58), not
    in the test's own process as the draft had it. A program runs on trust-runtime's scan loop with
    its simulated plants (specification 46), which `trust-runtime` holds and which cannot be
    published to crates.io with its dependencies; a `trust-sim` that ran the program in the test's
    process would run it without its plant, or would need the runtime published. A process of the
    test's own runs the very runtime that runs the project in VS Code, needs nothing of `trust-sim`
    but the control client, and costs the start of a process for each simulator. The owner
    delegated the choice on 22 November 2026 ("do whats most professional and best for us and the
    users").

## Appendix A — Fault and event

| Name | Code | Detail |
|---|---|---|
| `RustFault` | 20 in `trust-plc-abi` (specification 44 Appendix A, kernel interface version 6), stable code `runtime_rust_fault` (specification 10 §10.2), `RTSEXCPT_UNKNOWN` in `__TRY` (specification 06 §15) | the POU's number, then two numbers of its choice; a sequence's `.within` gives the wait point's number, then its limit and the time waited in milliseconds (RU-72); the calling statement names the Rust POU |
| event `rust_budget_exceeded` | specification 11's events | the POU's name, the measured time, the budget |

## Appendix B — `trust.toml`

```toml
[rust]
crate = "rust"                  # the crate's folder inside the project; "." in a Rust-first project

[timing]
profile = "development"         # or "production" (RU-47)
[timing.budgets]                # lower a POU's budget for a target (RU-46)
"FILTER_EMA" = { "riscv32imac-unknown-none-elf" = "40us" }
[timing.measured]               # written by `trust-dev rust bench` (RU-48)
"FILTER_EMA" = { "riscv32imac-unknown-none-elf" = { longest = 31250, code = "9c41…", device = "a3f0c2d9" } }

[services.recipe]               # tier 3 (§13): the global recipe_heartbeat (RU-63)
package = "recipe-service"      # the Cargo package of the service
heartbeat = "500ms"
writes = ["Recipe.target", "Recipe.speed"]   # the variables the service may write (RU-63)
endpoint = "tcp://127.0.0.1:47800"  # optional: without it, the endpoint the project's runtime publishes (RU-62)
credentials = "secrets/recipe.token"   # mode 600, never in version control: `trust-runtime ctl
                                       # service-token recipe --out secrets/recipe.token` (RU-81)

# A Rust-first project only (§14.2):
[resource]
name = "Cell"
[tasks.Fast]
interval = "10ms"
priority = 1
[programs.Pal1]
type = "PALLETIZER"
task = "Fast"
[globals]
LineSpeed = { type = "REAL", init = "1.5" }
[bindings]
"Pal1.part_present" = "%IX0.0"
"Pal1.close_gripper" = "%QX0.0"
```

## Appendix C — `.trusttrace`

A trace (RU-59) is one file, little-endian, laid out as the program image is (specification 44
§5.1, §5.3): a header, a section table, and sections that each start at a multiple of 8 bytes. Each
section has its CRC-32 (ISO-HDLC) in the section table, so a reader names a damaged section by
itself. Values lie at their declared sizes (specification 44 §7.2). A text inside a cycle is its
length in 2 bytes and its UTF-8 bytes; a string of `META` and `SYMS` is an offset into `STRS`.

### C.1 Header (64 bytes)

| Offset | Bytes | Field |
|---|---|---|
| 0 | 8 | Magic `TRSTT001` |
| 8 | 4 | CRC-32 of bytes 12 to the end of the section table |
| 12 | 2 | Format version, 1 |
| 14 | 2 | Reserved, 0 |
| 16 | 8 | Total length of the file |
| 24 | 4 | Section count |
| 28 | 4 | Reserved, 0 |
| 32 | 8 | Offset of the section table |
| 40 | 24 | Reserved, 0 |

### C.2 Sections

The section table has one 32-byte entry per section:

- the id (4);
- the flags (4), 0;
- the offset (8);
- the length (8);
- the CRC-32 of the section's bytes (4);
- reserved (4).

The sections follow the order of the table below, and the cycles their own order. A reader refuses
a trace whose header, section table or required section is damaged, and names which. A damaged
`CYCL` ends the trace before that cycle, and the reader notes it.

| Id | Section | Required | Content |
|---|---|---|---|
| 1 | `META` | yes | The trace's identity (C.3) |
| 2 | `STRS` | yes | The string pool of `META` and `SYMS`: NUL-terminated UTF-8, starting with the empty string |
| 3 | `SYMS` | yes | The located variables (C.4) |
| 4 | `STAT` | yes | The start state (C.5) |
| 104, 106 to 110 | `IAREAS`, `ITASKS`, `IIOBIND`, `ISYMBOLS`, `ITYPES`, `ISTRINGS` | yes | The recorded image's sections whose ids are 100 less (specification 44 §5.3), byte for byte |
| 5 | `CYCL` | no | One cycle (C.6), once for each, in order |

### C.3 `META`

| Offset | Bytes | Field |
|---|---|---|
| 0 | 32 | The running image's SHA-256 |
| 32 | 32 | The image's sources' hash (specification 44 §5.2, offset 32) |
| 64 | 4 | The image's target id (specification 44 §5.2, offset 20) |
| 68 | 4 | Why the trace ended: 1 `complete`, 2 `stopped`, 3 `resource_stopped`, 4 `restarted`, 5 `faulted`, 6 `program_changed`, 7 `paused`, 8 `memory_full` |
| 72 | 8 | The resource's cycle interval, in nanoseconds |
| 80 | 8 | The runtime's count of cycles at the trace's first |
| 88 | 8 | The number of cycles |
| 96 | 8 | The UTC time the recording began, in milliseconds (informational) |
| 104 | 4 | The runtime's version (string) |
| 108 | 4 | The device's identity, `runtime_id` (string) |
| 112 | 4 | The platform (string) |
| 116 | 4 | The resource's name (string) |
| 120 | 4 | The task whose runs the recording counted (string), 0 for none |
| 124 | 4 | Where the trace comes from: 1 a recording on demand, 2 the flight recorder |

### C.4 `SYMS`

A record count (4) and a reserved word (4) come first. Then follow 24-byte records, one for each
located variable of the recorded image, in the order of its I/O binding table:

| Offset | Bytes | Field |
|---|---|---|
| 0 | 4 | The variable's path, as the debugger names it (string) |
| 4 | 4 | Its type's name (string) |
| 8 | 1 | Its area: 1 `%I`, 2 `%Q`, 3 `%M` |
| 9 | 1 | Its bit, 0 to 7, for a `BOOL` of one bit; 255 otherwise |
| 10 | 2 | Reserved, 0 |
| 12 | 4 | Its byte offset in the area |
| 16 | 4 | Its size in bytes |
| 20 | 4 | Reserved, 0 |

### C.5 `STAT`

An area count (4) and a reserved word (4) come first. Then follows each area of specification 44
§7.1 but the stack, by id: its id (4), its length (4), and its bytes, padded to a multiple of 8.
Last come the forces in effect: a count (4), a reserved word (4), and each force as a change of
kind 2 or 5 (C.6).

### C.6 `CYCL`

| Offset | Bytes | Field |
|---|---|---|
| 0 | 8 | The cycle's logical time, in nanoseconds |
| 8 | 4 | The number of activations A |
| 12 | 4 | The number of changes C |
| 16 | 4 | The number of wall-clock readings W |
| 20 | 4 | Flags: bit 0 when the cycle faulted |
| 24 | 16 × A | Each activation: the task's index in the recorded task table (4), reserved (4), the task's logical time in nanoseconds (8) |
| … | 8 × W | Each reading: the UTC time in milliseconds, as `CURRENT_DT` encodes it (8) |
| … | | The C changes. Each has its kind (1), its source (1), reserved (2), the length of its data (4), and its data, padded to a multiple of 8 |
| … | | The input image: its length (4), reserved (4), its bytes, padded to a multiple of 8 |
| … | | The output image: its length (4), reserved (4), its bytes, padded to a multiple of 8 |
| … | 16 | When the cycle faulted: its fault record's code (4, specification 44 §8.6), POU id (4) and statement id (4); reserved (4) |

A change's kind and data:

| Kind | Change | Data |
|---|---|---|
| 1 | a variable's write | its path (text), its type's name (text), its value's bytes at the declared size (a length of 4, then the bytes) |
| 2 | a force set on a variable | as 1 |
| 3 | a force released on a variable | its path (text) |
| 4 | an address's one-shot write | the area (1: 1 `%I`, 2 `%Q`, 3 `%M`), the bit (1, 255 for whole bytes), reserved (2), the byte offset (4), the bytes (a length of 4, then the bytes): the address's bytes as the write left them, or for a bit one byte, 0 or 1 |
| 5 | a force set on an address | as 4 |
| 6 | a force released on an address | the area (1), the bit (1), reserved (2), the byte offset (4), the size (4) |
| 7 | a method call | the instance's path, the interface's qualified name and the method's name (texts), the number of inputs (2), each input's name and Structured Text literal (texts), the number of the wall clock's readings the call made (2), and each reading: the UTC time in milliseconds, as `CURRENT_DT` encodes it (8) |

A change's source: 1 the control link (the debugger, Live Values and `ctl` among its clients), 2 a
screen, 3 Control Studio, 4 a service, 5 the mesh, 6 a protocol server, 7 a plant. A variable's path is
the path the debugger names it by, a member of an instance included, whatever form the request named
it in.

A later version may store a cycle's images as their differences from the cycle before, and record
that in the section's flags, as the image's `INIT` may be run-length coded (specification 44 §7.4).

## Appendix D — The July plan and this specification

| July plan (v2.4) | Here |
|---|---|
| D14 one compile path, two doors | kept; one crate, not one pipeline of modules (§3) |
| D15 timing admission with graded evidence | kept as declared and measured budgets (§10); the `static_stbc` and `wasm_fuel` grades are gone |
| D16 crate admission | kept, stricter: no `unsafe`, no build scripts, no third-party macros (§4.2) |
| D17 record and replay | kept (§12) |
| D18 sequences | kept (§15) |
| D19 typed I/O | `[bindings]` of `trust.toml` and the program connections (§14.2, RU-35) |
| D20 `trust-sim` | kept (RU-58) |
| D21 SDK `trust-plc` | kept |
| D22 no Rust-to-ST calls | replaced: Rust calls Structured Text through layers (§8) |
| D23 precompiled packages | dropped: Rust is built from source with the program |
| D24 a `trust` CLI | dropped (§21, item 10) |
| D25 SDK on crates.io | kept, with the bundle's copy for every build (RU-12) |
| D26 deploy targets | Devices & Connections, as today |
| module ABI, `cdylib`, marshalling, digest chain, WebAssembly | gone: one module, one image, no WebAssembly (owner) |
