# 48 — Rust in truST projects

- **Status:** 0.1, approved by the owner on 15 November 2026 ("approved"), with the decisions of §21;
  written from the owner's decisions of that day (below) against the compiled runtime of
  specification 44.
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
(`Str<N>`, `WStr<N>`, `Time`, `Date`, `Tod`, `Dt` and the rest), `Ref<T>` for `VAR_IN_OUT`, `REF_TO`
and interfaces, the standard functions and function blocks of the program library (`trust_plc::std`,
the same code `trust-plc-lib` gives Structured Text), the checked arithmetic of §5.3, and the
attributes of `trust-plc-macros` (§6). It holds no `unsafe` code of its own; the `unsafe` of a module
stays in `trust-plc-abi` (specification 44 §6.9).

**RU-12.** Each truST release publishes `trust-plc`, `trust-plc-macros`, `trust-service` and
`trust-sim` to crates.io at the release's version, with the crates they are built from
(`trust-plc-abi`, `trust-plc-lib`, `trust-plc-decl`), and carries their sources in its toolchain
bundle (§16). The build always compiles the bundle's copy: the project's `Cargo.toml` names the
version, the build patches the four crates to the bundle's sources, and a project builds without
crates.io when its other dependencies are vendored. A user's own `cargo` resolves the same version
from crates.io for rust-analyzer and `cargo test` (§16.3).

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
(`cx.now()`, specification 11 §5.2.1), the task's interval and the wall clock through the kernel
function specification 44 §8.2 gives (`cx.wall_clock()`). It never reads a clock of its own, never
waits and never sleeps; `core` gives it none. This keeps the scan's results a function of its inputs
and logical time (specification 44 NX-04), which §12's replay needs.

### 5.5 Determinism

**RU-17.** A Rust POU computes the same results on every device for the same inputs
and logical times (specification 44 NX-04). Integer and IEEE 754 operations give that by
themselves. The elementary functions (`sin`, `exp`, `sqrt` and the rest) come from
`trust_plc::math`, the program library's pure-Rust `libm`, never from a target's own; `core`'s float
methods that call the target's library are refused by the build (§9.4). A Rust POU has no source of
randomness but a seeded generator it keeps in its instance (`trust_plc::Rng`).

### 5.6 No recursion

**RU-18.** A Rust POU's code calls nothing that calls it again: the build finds
the cycles of the linked module's call graph (specification 44 §8.5) and refuses one through a Rust
function, naming the functions of the cycle. Structured Text's bounded recursion (specification 44
§6.4) stays Structured Text's.

### 5.7 No standard library, no heap

**RU-19.** The scan crate graph builds against `core` only, the
part of the standard library the toolchain bundle carries for every module target (specification
44 §4.6). `std` and `alloc` do not exist there: a crate that needs them fails to compile for the
module, naming the crate. A crate's own tests may use `std` (`#![cfg_attr(not(test), no_std)]`).

### 5.8 No `unsafe`

**RU-20.** No crate of the scan crate graph holds `unsafe` code, except the SDK's
interface crate (specification 44 §6.9). The project's crate forbids it at its root (RU-08), and the
build refuses every other crate of the graph whose sources hold it (§9.3). A Rust POU reaches the
program's memory, its references and the kernel only through the SDK's safe types.

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
Text and are refused. A function block's struct gains `new()`, its starting values (RU-26),
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
    x : LREAL;
    alpha : LREAL := 0.1;
END_VAR
VAR_OUTPUT
    y : LREAL;
END_VAR
VAR
    primed : BOOL;
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
construction. The VS Code extension runs it when a `.rs` file of the crate is saved; the build runs
it first (§3, step 1) and fails when the committed file differs from what it writes, naming the
command, so a project whose declarations are out of date never builds.

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
| `ARRAY[a..b] OF T` | `Array<T, A, B>`, indexed from `A` as Structured Text indexes it |
| a structure | a Rust `struct` with `#[trust_struct]`, every field of a type of this table |
| a function block instance | a field of a function block's type, Rust or Structured Text (§8) |
| `REF_TO T`, an interface | `Ref<T>`, `IfaceRef<I>` |
| `VAR_IN_OUT` of `T` | `InOut<T>` |

A field of another Rust type is refused at the declaration with the table's types. Generic types
(IEC §6.4.3) are not declared by a Rust POU; a Rust function may be generic in Rust only when it is
not a POU.

**RU-26. Initial values.** A variable's initial value is given by `#[init(...)]` with a literal of its
type, which the generated declaration carries as the variable's Structured Text initializer, so the
build computes it into the starting memory (specification 44 §7.4) as for Structured Text. A variable
without `#[init]` starts at the type's default (IEC §6.4.2, Table 10). The literal of a bit string
is an integer within its bits (`#[init(0x0F)]`, declared `16#0F`), of a `CHAR` an ASCII character or
a byte (`'A'`, `b'\xE9'`), of a `WCHAR` a character of one UTF-16 code unit, and of a `TIME` or an
`LTIME` an IEC duration in a string (`#[init("T#1.5s")]`, declared `T#1s500ms`; IEC §6.3.4). A Rust
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

**RU-29. In-outs and references.** An `InOut<T>` and a `Ref<T>` read and write the variable they name
through the SDK, which makes the checks specification 44 §6.5 makes: null (`NullReferenceDereference`),
the area, the offset, the extent and the generation (`ReferenceLifetime`). A function block's in-out
keeps the reference of its last call (specification 44 §7.3).

**RU-30. EN and ENO.** A Rust function and function block have `EN` and `ENO` as specification 04
§EN/ENO gives every POU: `EN` = `FALSE` skips the body, and the body sets `ENO` through
`cx.set_eno(false)`; a body that returns a fault ends the scan whatever `ENO` says.

### 6.4 In the symbol table

**RU-31.** The symbol table marks each POU's language, Structured Text or Rust, and names a Rust
POU's source file (§11.1). A tool that shows a POU's source opens the Rust file for a Rust POU.

### 6.5 Methods and interfaces

**RU-32. Methods.** A Rust function block has methods (`#[trust_method]` on a function of its `impl`
block), which IEC §4.3 lets another language program: inputs by value, in-outs as `InOut<T>`,
outputs as the method's `&mut` parameters, a result, access specifiers `PUBLIC`, `PRIVATE`,
`PROTECTED`, `INTERNAL` (IEC §6.6.5.9) as an attribute, `PUBLIC` when none is given. The generated
declaration carries each method's declaration without a body.

**RU-33. Interfaces.** A Rust function block may implement Structured Text interfaces
(`#[trust_fb(implements = "I_RUN, I_RESET")]`, IEC §6.6.6): the build checks that it has a method
of each prototype with its signature, and a call through an interface reaches the Rust method as it
reaches a Structured Text one (specification 44 §6.4, the dispatch by class id). A Rust POU calls a
method through an interface it holds with `IfaceRef<I>::call`.

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
program.

**RU-36. The deviation.** IEC §4.3 rule 4 requires a program's declaration to "use the mechanisms
defined for the IL, ST, LD, or FBD language"; a Rust program's declaration does (it is the generated
Structured Text declaration), but its body is Rust, which the rule does not allow for a program as
it allows it for functions, function blocks and methods. DEV-048 in `docs/IEC_DEVIATIONS.md` records
this, by the owner's decision of 15 November 2026 that Rust programs are part of truST.

**RU-37.** A Rust program has no methods of a program (specification 04 §4) and no `VAR_TEMP`; its
body's locals are Rust locals on the task's stack.

## 8. Calling Structured Text from Rust

**RU-38.** A Rust POU calls Structured Text functions and function blocks of the project and of its
libraries, and holds Structured Text function block instances as fields:

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

**RU-39. The bindings.** The build generates a Rust module `st` (`trust_st` crate, layer 1, §9.2)
with a Rust function for each Structured Text function the crate calls and a Rust struct for each
Structured Text function block type it holds, whose fields are the block's inputs and outputs and
whose `call(&mut self, cx)` is the standard's call, as RU-38's example makes it. A Structured Text function
block held by a Rust instance lies inside the Rust instance in the area, as a nested instance lies
in a Structured Text one (specification 44 §6.2), so the symbol table, Live Values and online change
see it.

**RU-40. What can be called.** The build puts every Structured Text POU that a Rust POU calls or
holds, with every POU those call, into layer 1, compiled before the project's crate (§9.2). A POU of
layer 1 that calls a Rust POU, directly or through others, would make the layers a cycle; the build
refuses it, naming the chain (`TANK_GUARD` (Rust) calls `HYSTERESIS`, which calls `FILTER_EMA`
(Rust)). Composition across the two languages in both directions goes through a program's order on
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
warning does not fail the build; a `-D warnings` of the crate's own does.

### 9.4 Checks on the linked module

**RU-45.** After the link and before the image, the build checks the module and fails with a message
that names the Rust function or item when one breaks a rule:

| Check | How | Message names |
|---|---|---|
| No panic (RU-14) | specification 44 §8.6's check of the panic machinery, then the call graph back from it, through the functions of `core` and `alloc` that call it, to the first function outside them; the generated function of a Rust POU is never inlined, so code of the POU inlined into it is named by the POU | each Rust POU (its file and line) or other function that can panic, what panics, and the form that returns a fault instead (`trust_plc::checked::div`, `get`, `ok_or(...)?`) |
| No changing `static` (RU-13) | the module's writable data sections outside the areas, by symbol | the `static` |
| No recursion through Rust (RU-18) | the call graph's cycles (specification 44 §8.5) | the cycle's functions |
| No target library math (RU-17) | calls of the target's `libm` symbols outside `trust-plc-lib` | the function and `trust_plc::math` |
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
simulator with the inputs of its `#[bench]` cases (or the declared initial values) and records the
longest time of each into `trust.toml` `[timing.measured]`, with the device's identity and the
image's hash. A `production` profile requires a measurement no older than the image for every Rust
POU on the deployment's device type, and uses the larger of declared and measured in RU-47's sum.

**RU-49. On the device.** The kernel measures each Rust POU's call time in a debug image and reports a call that exceeds its declared budget as the event
`rust_budget_exceeded` with the POU's name, the time and the budget (specification 11's events),
without ending the scan; the watchdog (NX-11) still ends a scan that overruns the task.

## 11. Tools

### 11.1 VS Code and the language server

**RU-50.** The language server treats `src/generated/rust_pous.st` as part of the project
(read-only: an edit of it is refused with the advice to change the Rust source), and "Go to
Definition" of a Rust POU opens the Rust item in its `.rs` file at the attribute. The extension
recommends rust-analyzer for a project with a crate and writes its settings (§16.3); it never
needs it.

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
`Cx` of a simulated task (its logical time, interval and task) and runs a function block's call as
the kernel does: `let mut fb = FilterEma::new(); fb.x = 1.0; fb.call(&mut cx)?;`. A unit test runs
the POU's Rust code, not the program.

**RU-57. Project tests.** Structured Text test POUs (specification 04) call Rust POUs like any POU and
run with `trust-dev test`, which builds the project with its crate through the bundle and runs the
tests in the simulator (specification 22). A Rust test of the project's crate that needs the whole
program uses `trust-sim` (RU-58).

**RU-58. Machine tests in `cargo test`.** `trust-sim` builds the project through the installed
trust-runtime and its bundle and runs it in the simulator's kernel inside the test, under a Rust
API: set inputs, run cycles or logical time, read outputs and any variable by path, force, see
faults:

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

`trust-sim` needs trust-runtime (the VS Code extension's or one installed on its own) and finds it as
the extension does; a user's own `cargo` runs the test, truST's bundle builds the program.

**RU-59. Cycle traces.** The runtime records a cycle trace on demand (`trust-runtime ctl record
--task <task> --cycles <n>` or `--duration`) and, when `runtime.toml` `[trace] flight_recorder`
names a length, always into a ring of that many cycles that it writes out on a fault. A trace holds
for each cycle of the task: the logical time, the input image, the writes and forces the control
link applied before the cycle, and the output image after it, with the image's hash and the device's
identity; its format is a sectioned file with a checksum per section, `.trusttrace`, which Appendix C
gives.

**RU-60. Replay.** `trust-dev replay <trace>` runs the project's current build in the simulator from
the trace's first cycle with its inputs, writes and logical times, and compares the outputs cycle by
cycle; a difference is reported with the cycle, the variable, the trace's value and the build's.
Specification 44 NX-04 makes a replay of an unchanged image bit-identical on every device;
`trust_sim::Replay` does the same in `cargo test`, so a trace from commissioning becomes a test of
the next change.

## 13. Rust services (tier 3)

**RU-61.** A service is an ordinary Rust program (`[[bin]]`, the standard library, any crate,
threads, `async`), built with the user's own Rust or the bundle's `cargo`, which runs on a computer
beside a runtime and reaches it only over the control link (specification 11 §6.8). None of §5's
rules applies to it; it can never stop or slow a scan.

**RU-62. The SDK.** `trust-service` gives a service a connection to a runtime through the control
client trust-runtime uses (`trust-runtime-client`, the shared client of specification 11 §6.8), with
the runtime's authentication, and typed access to the program's variables by their paths: read,
write, subscribe (a read at a period the service chooses, of a set of variables in one cycle's
snapshot), with every value's cycle, logical time and quality. The bindings are generated from the
symbol table of the deployed image (`trust-dev service bindings`), so a service names
`plant::P1::level` and gets an `f64`.

**RU-63. Freshness.** Every service in `trust.toml` `[services.<name>]` has a heartbeat variable that
the build declares as a global (`<name>_heartbeat : UDINT`) and the SDK increments at the period the
service states (`heartbeat = "500ms"`). The program library's function block `SERVICE_FRESH` (its
inputs the heartbeat and a timeout, its outputs `FRESH : BOOL` and `AGE : TIME`) is the standard way
a program gates on a service's values; a program decides itself what it does when a service is not
fresh. A value a service writes is never more than the program's input: the scan never waits for a
service.

**RU-64. Running.** A service runs as its own process under the computer's service manager:
`trust-dev service install <name>` writes a systemd unit on Linux, a launchd job on macOS and a
Windows service, from `trust.toml` (the binary, the runtime's endpoint, the credentials' file), and
the runtime never starts a service itself. Devices & Connections shows a connected service as a
client of its runtime, with its name and the age of its last heartbeat. A service reaches a chip's
runtime over the network as any client does; a service does not run on a chip.

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
    generated/trust/  GENERATED, committed: rust_pous.st, configuration.st
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
review sees exactly the configuration that runs. Every Structured Text tool reads it: the language
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
#[trust_program(name = "PALLETIZER")]
#[budget("80us")]
pub struct Palletizer {
    #[input] pub part_present: bool,
    #[input] pub gripper_closed: bool,
    #[output] pub close_gripper: bool,
    #[output] pub step: Str<16>,
}

impl Palletizer {
    #[trust_sequence]
    async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
        loop {
            s.step(&mut self.step, "WAIT_PART");
            s.until(|me: &Self| me.part_present).await;
            s.step(&mut self.step, "GRIP");
            self.close_gripper = true;
            s.until(|me: &Self| me.gripper_closed)
                .within(Time::from_secs(2))
                .await?;
            s.wait(Time::from_millis(500)).await;
            self.close_gripper = false;
        }
    }
}
```

**RU-71. What it is.** `#[trust_sequence]` is a restricted sequence language with Rust's syntax, not
Rust's `async`: the attribute validates the body and lowers it to a state machine whose state, the
wait point it stands at and the locals that live across a wait point, are variables of the instance
(in the area, in the symbol table, shown by Live Values as `<instance>.__seq.point` and its locals).
No Rust future is stored, no executor runs, and nothing waits inside a scan: each activation runs the
sequence from its wait point to its next one and returns.

**RU-72. Wait points.** The body awaits only the SDK's wait points: `s.next_cycle()`, `s.until(pred)`
(true at the start of a later cycle), `s.wait(t)` (the task's logical time advanced by `t`),
`s.all(...)` and `s.race(...)` of these, a child sequence of the same instance, and `.within(t)`
on any of them, which faults with `RustFault` when the wait outlasts `t` in logical time. A
predicate is a plain function of the instance, without a wait point. The attribute refuses any other
`.await`, an `async` block or closure, a future of another crate and a loop that can run without
reaching a wait point, each with its place in the body.

**RU-73. Restarts and changes.** A cold or warm restart starts every sequence at its top, as an SFC
starts at its initial step (IEC §6.7); a sequence that must resume keeps its progress in retained
variables of its own and reads them at its top. An online change keeps a sequence's state when the
changed code has the same wait points (each named by `s.step` or, unnamed, by its order) and the same
locals across them, and otherwise starts the sequence at its top and says so in the change's report
(specification 44 §12).

## 16. The toolchain and the user's own Rust

### 16.1 The bundle

**RU-74.** The toolchain bundle (specification 44 §4.6) gains, for every computer platform:
`cargo` of the pinned release; the standard library of the computer itself, with which `cargo`
compiles the SDK's procedural macros and build scripts on the computer (never into a module); and
the sources of the SDK crates and of every crate they are built from, vendored, with their hashes in
the bundle's manifest. The module targets keep only `core` and `compiler_builtins`: the scan has no
`std` and no `alloc` (RU-19). The release records the bundle's new size as NX-08 records today's.

**RU-75.** The project's crate and its dependencies are built with the bundle's `cargo` and
compiler only; the build never runs a `cargo` or `rustc` of the user's, and never touches the
user's `rustup`, `CARGO_HOME` or global configuration: it gives its `cargo` a home of its own in
truST's data folder.

### 16.2 Without the user's own Rust

**RU-76.** A user without Rust builds, runs, tests (`trust-dev test`), debugs and deploys a project
with Rust POUs as any project. What needs the user's own Rust is said once, when a project with a
crate is opened: rust-analyzer's help while editing `.rs` files, and `cargo test` (RU-56, RU-58).

### 16.3 With the user's own Rust

**RU-77.** The project's `rust-toolchain.toml`, which Create Project writes, names the pinned Rust
version and the module targets, so a user's `rustup` gives rust-analyzer and `cargo test` the
compiler the build uses. The extension writes the project's rust-analyzer settings: the crate, the
`trust_module` configuration of a module (specification 44 §4.5) off for analysis, and checks with
`cargo check` of the computer's target. A difference between the user's Rust and the pinned version
gives a warning in the extension, never a different build.

## 17. Security

**RU-78. Code that runs while building.** A project's build runs no code from a crate outside the
SDK: the scan crate graph has no build scripts and no procedural macros but the SDK's (RU-09), and
the build reads a crate's sources only with the compiler. A service's crates may have both: a
service is built apart from the program (RU-61).

**RU-79. Supply.** `Cargo.lock` pins every crate with its checksum (RU-10); the deployment record
(specification 34 §5.1.1) lists the scan crate graph, every crate's name, version and checksum, so a
deployed image says which Rust code it holds. `trust-dev rust audit` checks the graph against the
RustSec advisory database when the computer can reach it, and against a copy of it otherwise.

**RU-80. The image.** A program with Rust POUs is one image, signed as every image is (specification
44 §5.11); a device installs and runs it only as specification 44 §10.2 allows any image. Nothing of
the Rust code is loaded apart from the image.

**RU-81. Services.** A service authenticates on the control link with credentials of its own
(specification 11 §6.8), with the role it needs: a service that only reads gets a role that reads.

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
(specification 22, "Docs Captures").

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

1. **The bundle grows** by `cargo` and the computer's own standard library (RU-74), which a build of
   a Rust crate with dependencies needs.
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
    a chip (RU-64).

## Appendix A — Fault and event

| Name | Code | Detail |
|---|---|---|
| `RustFault` | 20 in `trust-plc-abi` (specification 44 Appendix A, kernel interface version 6), stable code `runtime_rust_fault` (specification 10 §10.2), `RTSEXCPT_UNKNOWN` in `__TRY` (specification 06 §15) | the POU's number, then two numbers of its choice; the calling statement names the Rust POU |
| event `rust_budget_exceeded` | specification 11's events | the POU's name, the measured time, the budget |

## Appendix B — `trust.toml`

```toml
[rust]
crate = "rust"                  # the crate's folder; "." in a Rust-first project

[timing]
profile = "development"         # or "production" (RU-47)
[timing.budgets]                # lower a POU's budget for a target (RU-46)
"FILTER_EMA" = { "riscv32imac-unknown-none-elf" = "40us" }
[timing.measured]               # written by `trust-dev rust bench` (RU-48)

[services.recipe]               # tier 3 (§13)
package = "recipe-service"      # the Cargo package of the service
heartbeat = "500ms"
timeout = "2s"
endpoint = "tcp://127.0.0.1:47800"
credentials = "secrets/recipe.token"   # a file with mode 600, never in version control

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

A trace is a container like the program image's (specification 44 §5): a header, a section table and
sections, each with a CRC-32 of its bytes. Its sections are `META` (the image's hash, the device's identity, the task, the first
cycle, the cycle count), `SYMS` (the variables recorded, by path and type), and per cycle `CYCL` (its
logical time, the input image, the control link's writes and forces before it, the output image after
it). Values are little-endian at their declared sizes (specification 44 §7.2).

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
