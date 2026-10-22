# 44 — Native execution: compiled programs and the kernel

- **Status:** 0.2 (0.1 approved by the owner on 22 October 2026, "implement the checklist"; 0.2 makes
  exact what the implementation of milestone M1 decided within that design, Appendix B), with the six
  decisions of its report: `NEW` and `__DELETE` outside this profile (§1), Ed25519 with unsigned
  images accepted on an empty trust list (§5.11), interface calls by `match` (§6.4), the
  non-preemptive scheduler in v1 (§16), sources embedded on computers only (§5.3), and the request
  name `bytecode.reload` kept (§12.1).
- **Owns:** how a project is compiled on the engineering computer into native code for the device it
  goes to; the program image that carries it; the kernel that runs it on every device; the memory,
  fault, retain, online-change, debugging and monitoring contracts of that kernel; the toolchain the
  tools carry; the numbers each milestone must reach.
- **Replaces:** specification 12 (the STBC bytecode) and the loading, preparation and virtual-machine
  parts of specification 34 (§5.2 compiler-free preparation, §5.4, §6.1, MEM-14, the four execution
  paths of PERF-05). Specification 34's platform, scheduling, fault, retain, deployment and online
  change behaviour (§7 to §11) holds unchanged unless a section below says otherwise. Specifications
  10, 11, 13 and 18 keep their behaviour; their references to bytecode read as references to the
  program image.
- **Standard:** IEC 61131-3:2013, cited as "IEC §x.y". Every behaviour of a program is the standard's;
  this specification decides only how the standard is carried out. Restart behaviour follows IEC
  61131-1 as IEC §6.5.6 refers to it.
- **Crates:** trust-hir (unchanged), trust-runtime (the build), trust-runtime-core (the kernel),
  trust-runtime-services, the platform crates and firmware, trust-debug, the VS Code extension.

## Owner requirements

The owner's decisions of 22 October 2026, which this specification carries out:

- "What we want is a plc that can run plc programs": the reference workload is a real plant program,
  `examples/vat_pasteurizer`, and every number in this specification is measured on it.
- "I want the state of the art": programs are compiled to native code on the engineering computer, as
  CODESYS, TwinCAT and RuSTy do; the runtime kernel is one Rust kernel on every device.
- "We must be better than RuSTy": the generated code is as fast (the same LLVM), smaller in RAM, and
  the product is complete: kernel, online change, retain, debugging over the control link, IDE.
- "We shall not have any spike, we shall implement this directly"; "we must specify everything
  exactly so we don't run around in circles": this document is exact where exactness decides
  behaviour, and every milestone is accepted by numbers.
- "Can they get the Rust toolchain automatically": yes, §4.6.
- "Remember, we MUST follow the IEC standard": §6 maps every construct to the standard's section.
- The hardware stays what the owner has: the ESP32-C6 and the STM32U5A9, and the owner's computers.
- Earlier decisions that still hold: boards work exactly like trust-runtime (16 October); no read-out
  protection, the program stays readable on the device (22 October); online change keeps every value,
  "flickering outputs is shit" (19 October); variables lie at their declared size in flat blocks
  (19 October); Stop and Start on every runtime (16 October).

## 1. Decision and scope

**NX-01. Where the work is done.** The engineering computer compiles a project into machine code for
the CPU of the device it is deployed to, resolves every name to an address, lays out every variable,
computes every initial value it can, builds the symbol table, and signs the result. The device checks
the result and runs it from where it lies. The device never decodes, resolves or lays out a program.

**NX-02. One kernel.** The kernel, `trust-runtime-core` with the shared services, is one Rust crate
set that runs on every device: the ESP32-C6 and the STM32U5A9 firmware, trust-runtime on Linux,
Windows and macOS, and the simulator in VS Code. Every behaviour below holds on every device. A
device differs only in its platform adapter (specification 34 §7) and its sizes.

**NX-03. One program image.** A build produces one program image per resource and target CPU. The
image is the only thing a device loads. It carries the code, the constants, the starting memory, the
sizes of the memory areas, the tasks, the I/O bindings, the symbol and type tables, the statement
table, the retain map and the signature (§5).

**NX-04. Same results everywhere.** A program computes bit-identical results on every device for the
same inputs and logical times (§9).

**NX-05. Scope.** Structured Text as specifications 01 to 09 define it, with truST's extensions named
there (POINTER TO, UNION, STRUCT OVERLAP, PROPERTY, ACTION, edge-qualified inputs, test POUs). Ladder
and the visual editors compile through ST as specification 17 says. SFC stays as `sfc-profile.md`
says. Not part of this profile: `NEW` and `__DELETE` (specification 29), since a program owns no heap
(§7.1); a project that uses them is refused at build with a diagnostic.

**NX-06. What is kept and what goes.** Kept: trust-syntax, trust-hir and the LSP; the language
specifications; the standard functions and function blocks (their semantics and their Rust
implementations, moved into the program library, §6.9); the flat memory areas and their layout rules
(§7); the I/O drivers and process image; the retain store and checkpoint log; the program store and
its slots; the control link and protocol; Live Values, the debugger and VS Code; the ST test corpus.
Removed after parity (§15): the STBC format and encoder, the device's decoder, validator and
preparer, the virtual machine and its compact, kinded and register paths, the legacy formats
(checklist C8), and the engine-internal tests of those parts.

## 2. Terms

| Term | Meaning |
|---|---|
| Project | The ST sources with `runtime.toml`, `io.toml` and `trust-lsp.toml` (specification 19). |
| Build | `trust-runtime build`, or Build in VS Code: project → program image. |
| Program image, image | The file a device loads (§5). One per resource and target. |
| Module | The native code inside the image: the compiled program and its library. |
| Kernel | The runtime without any program: scheduler, I/O, retain, store, link, services. |
| Kernel interface | The fixed set of functions and records through which a module and the kernel talk (§8.2). Versioned. |
| Area | One flat block of program memory: inputs, outputs, markers, globals, retain, instances, stack (§7.1). |
| Symbol table | The list of every named variable with its type, area and offset (§5.7). |
| Statement table | Every statement's file, lines and POU, by statement id (§5.9). |
| Copy plan | What an online change copies from the running program to the new one (§12.2). |
| Target | A CPU and its Rust target triple (§4.4). |
| Toolchain bundle | The pinned Rust compiler, linker and target libraries the tools carry (§4.6). |
| Reference workload | `examples/vat_pasteurizer` (§14.1). |

## 3. Architecture

```
Engineering computer                               Device (every platform)
┌─────────────────────────────────────────┐        ┌──────────────────────────────────────┐
│ ST sources                               │ image  │ Program store (flash slots A/B)       │
│  → trust-syntax, trust-hir (unchanged)   │ ─────► │   the image, read where it lies       │
│  → program model (unchanged)             │        │ Kernel                                │
│  → generator: typed Rust source (§6)     │        │   loader and checker (§10)            │
│  → rustc + rust-lld, pinned (§4)         │        │   scheduler, cycle, process image     │
│  → image builder: tables, init memory,   │        │   retain, faults, watchdog            │
│    symbol table, signature (§5)          │        │   online change (§12)                 │
│ Tools: deploy, debugger, Live Values,    │        │   debugging and monitoring (§13)      │
│   online change plan (§12.2)             │◄─────► │   control link and services           │
└─────────────────────────────────────────┘        │ RAM: the areas and the stack (§7)     │
                                                   └──────────────────────────────────────┘
```

The build's steps and their outputs:

1. **Semantic model.** trust-hir checks the project as today. A project with an error does not build.
   The generator reads each POU's statements from the syntax tree with what trust-hir's type checker
   recorded of them: the type of every expression and the symbol every name, member and call resolved
   to (`FileAnalysis::resolved_symbols`), so that it resolves no name itself. The declarations (types,
   POUs, variables, configuration, tasks) come from the harness's lowering as today; the program model
   carries no expression types and is not the generator's input.
2. **Layout.** The generator lays out every variable in its area by the rules of §7.2 and assigns
   every statement an id in source order.
3. **Generation.** The generator writes one Rust crate: the areas as `#[repr(C)]` types, every POU as
   a function, every check the standard requires (§6). It also writes, for every symbol, a
   compile-time assertion that the Rust layout puts the variable at the offset the symbol table
   names (§6.3), so the two cannot disagree.
4. **Compilation.** The pinned rustc compiles the crate together with the program library
   (§6.9) for the target, with the fixed flags of §4.5, into a position-independent module.
5. **Image.** The image builder extracts the module's code and read-only data, computes the
   starting memory (§7.4), writes the tables, computes the memory figure (§10.4) and signs the
   image (§5.11).
6. **Record.** The deployment record of specification 34 §5.1.1 is written as today, naming the
   image's hash instead of the bytecode's.

## 4. The build and the toolchain

### 4.1 Inputs and outputs

`trust-runtime build --project <dir> --target <device or triple>` reads the project and writes into
the project's build directory: `program.<target>.tpi` (the image), `deployment-record.bin`,
`symbols.json` (the symbol, type and statement tables as JSON, for tools that do not read the
image), `build.json` (the memory figure and the numbers of §10.4) and, when `--emit-source` is
given, the generated Rust crate for inspection. The target is a device from Devices & Connections
(whose capabilities give its triple and kernel interface version, specification 34 COMPAT-02) or a
triple of §4.4. Without `--target`, the build targets the computer it runs on.

### 4.2 Determinism of the build

**NX-07.** The same project, target and toolchain bundle produce the same image bytes, apart from
the build time in the header and the signature. The generator sorts everything it emits by source
order or by name; the compiler runs with one code unit and fixed flags; the image builder writes
tables in defined orders. CI builds the reference workload twice and compares.

### 4.3 What the build checks

Beyond trust-hir's diagnostics, the build refuses with a diagnostic naming the construct and line:

- a variable, string, array or instance whose size exceeds an area's maximum (§7.6);
- an area total that exceeds the target's declared program memory (§10.4), with both numbers;
- a static call chain whose stack bound exceeds the target's task stack (§8.5), with both numbers;
- a task set the kernel cannot schedule (specification 18 §3);
- `NEW`, `__DELETE`, or a construct the generator does not map (§6.1), as a build error, never as a
  silent fallback.

### 4.4 Targets

| Device | CPU | Rust target | Floating point | Code model |
|---|---|---|---|---|
| ESP32-C6 | RV32IMAC, 160 MHz | `riscv32imac-unknown-none-elf` | software (`REAL` and `LREAL`) | position-independent, medium-any |
| STM32U5A9 | Cortex-M33, 160 MHz | `thumbv8m.main-none-eabihf` | hardware single (`REAL`), software `LREAL` | position-independent, Thumb |
| Linux, Windows x86-64 | x86-64 | `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc` | SSE2 | position-independent |
| Linux, macOS ARM64 | AArch64 | `aarch64-unknown-linux-gnu`, `aarch64-apple-darwin` | hardware | position-independent |

On the computer targets the module is built `#![no_std]`, with `panic = "abort"`, without the
operating system's C library, and linked by rust-lld into a freestanding position-independent
module, the same form as on the chips (§5.3). The kernel maps it into executable memory itself
(§10.3). No system linker, Visual Studio or Xcode is needed on any computer.

One module serves every computer of a CPU: the x86-64 module is built for
`x86_64-unknown-linux-gnu` and the AArch64 module for `aarch64-unknown-linux-gnu` on Linux, Windows
and macOS alike, since a freestanding module calls nothing of the operating system. The interface
names the calling convention instead of taking the operating system's: `extern "sysv64"` on x86-64
and `extern "C"` elsewhere (AAPCS64 and Apple's arm64 convention agree for the interface's
signatures, which pass everything in registers). The AArch64 module reserves `x18`, which macOS
reserves.

### 4.5 Compiler flags

Fixed for every target, in the toolchain bundle's manifest, and recorded in the image header:

```
-C opt-level=2 -C codegen-units=1 -C lto=fat -C panic=abort -C relocation-model=pic
-C overflow-checks=off (the generator emits the standard's checks itself, §6.5)
-C debuginfo=0 -C remark=prolog-epilog (each function's frame, §8.5)
-C target-cpu=<fixed per target> -C target-feature=<fixed per target, §9>
-C linker=rust-lld -C linker-flavor=ld.lld, linked with --pie --no-dynamic-linker -nostdlib
--emit-relocs and a linker script that puts CODE at address 0 and RODATA after it
--cfg trust_module
```

No `-Z` flag is used: the build runs on stable rustc only. The module is linked as a static
position-independent executable, so that an absolute address anywhere in it would be a dynamic
relocation, which the image builder refuses (§5.3); a module that links has none. The program
library and the interface crates are compiled for each target with the same relocation model, abort
on panic and embedded bitcode, so that link-time optimization takes in what the program calls. The
ELF's symbols are kept for the image builder, which reads the entry points' offsets from them; the
image carries none of them. (`-C llvm-args=-stack-size-section`, which 0.1 named, is accepted by
stable rustc and has no effect: rustc sets that option itself only for `-Z emit-stack-sizes`.)

### 4.6 The toolchain bundle

**NX-08.** The tools carry the compiler. The VS Code extension and `trust-runtime` download, on
the first build that needs it, one bundle per computer platform from truST's release server:
rustc and rust-lld of the pinned version, `rust-std` for the targets of §4.4, the program library
and interface crates (§6.9, §8.2) of the kernel version, and a manifest with every hash. The bundle
is installed into the extension's private folder (the user's own Rust installation, if any, is
never used or touched), verified against its manifest before use, and needs no administrator
rights. The download is about 250 MB once; the installed bundle about 600 MB. For sites without
internet, the same bundle is a file that the extension's "Install toolchain from file" command
installs. The bundle's version is the kernel's: a kernel update that changes the interface version
(§8.2) comes with a new bundle, and the tools say "rebuild needed" for every project (§10.2).

The bundle is built from the official Rust release of the pinned version. The pinned version is
recorded in `rust-toolchain.toml` of the repository and in each image's header; CI builds the
bundle and the kernel from the same version.

## 5. The program image

### 5.1 General

The image is one file, `TRSTI001`, little-endian, every section aligned to 8 bytes, at most the
size of a program slot on the device it is built for. Its layout is fixed so that a device reads it
where it lies in flash (the C6 through its MMU window, the U5A9 directly, a computer from a mapped
file) without copying: every table is an array of fixed-size records, every string is in the string
pool at an offset, every offset is relative to the start of the image.

### 5.2 Header (128 bytes)

| Offset | Bytes | Field |
|---|---|---|
| 0 | 8 | Magic `TRSTI001` |
| 8 | 4 | CRC-32 (ISO-HDLC) of bytes 12 to the end of the section table |
| 12 | 4 | Total length L of the image |
| 16 | 2 | Image format version, 1 |
| 18 | 2 | Kernel interface version (§8.2) the module was built against |
| 20 | 4 | Target id (§4.4: 1 rv32imac, 2 thumbv8m.main-hf, 3 x86-64, 4 aarch64) |
| 24 | 8 | Build time, Unix seconds (informational, excluded from NX-07) |
| 32 | 32 | SHA-256 of the project sources and settings as the deployment record computes it |
| 64 | 32 | Rust compiler version string, NUL-padded (for example `1.95.0`) |
| 96 | 16 | Program library version and flags hash (first 16 bytes of its SHA-256) |
| 112 | 4 | Section count |
| 116 | 4 | Offset of the section table |
| 120 | 8 | Reserved, zero |

### 5.3 Sections

The section table has one 16-byte entry per section: id (4), flags (4), offset (4), length (4).
Each id appears at most once; a required section that is absent refuses the image. `CODE` starts at
a 64-byte boundary of the image, the largest alignment a module's code or data needs on a target of
§4.4, and a container keeps the image at such a boundary in its slot.

A table section (`ENTRY`, `AREAS`, `TASKS`, `IOBIND`, `SYMBOLS`, `TYPES`, `STMTS`, `RETAIN`,
`FILES`) starts with its record count (4) and a reserved word (4), then holds that many records of
its fixed size; data its records name follows the records and is named by offsets from the
section's start: the task table's program list (4-byte entry indexes) and the type table's details.
`STRINGS` starts with the empty string, at offset 0.

| Id | Section | Required | Content |
|---|---|---|---|
| 1 | `CODE` | yes | The module's executable code, position-independent. Entered only through the entry table. |
| 2 | `RODATA` | yes | The module's read-only data: constants, lookup tables, string literals. Placed by the linker at a fixed distance after `CODE`, so no relocation exists; the image builder verifies that the module has no relocation records. |
| 3 | `ENTRY` | yes | The entry table (§5.4). |
| 4 | `AREAS` | yes | The area table (§5.5). |
| 5 | `INIT` | yes | The starting memory of the globals, retain and instance areas (§7.4). |
| 6 | `TASKS` | yes | The task table (§5.6). |
| 7 | `IOBIND` | yes | The I/O bindings (§5.6). |
| 8 | `SYMBOLS` | yes | The symbol table (§5.7). |
| 9 | `TYPES` | yes | The type table (§5.8). |
| 10 | `STRINGS` | yes | The string pool: NUL-terminated UTF-8 strings, referenced by offset. |
| 11 | `STMTS` | yes | The statement table (§5.9). |
| 12 | `RETAIN` | yes | The retain map (§5.10). |
| 13 | `FILES` | yes | The file table: for each source file its label (specification 12 §7.5.1.4's label rule, kept) as a string offset. |
| 14 | `SOURCE` | no | The project's ST sources, concatenated with a file table of their own, when the project sets `build.embed_source = true` (default true on computers, false on chips). |
| 15 | `MEMFIG` | yes | The memory figure (§10.4): 8-byte fields as `build.json` names them. |
| 16 | `SIGN` | no | The signature (§5.11). Always the last section. |

### 5.4 Entry table

One 16-byte record per entry point: kind (4: 1 program body, 2 cold-init, 3 warm-init, 4 method for
tools), index (4: the program instance's number in the configuration, or 0), code offset (4,
relative to `CODE`, with the Thumb bit on a Thumb target), reserved (4). A task's record lists the
program bodies it runs, in order, so that the kernel observes each program's call as today's
runtime does. The
kernel calls an entry point by its offset through the calling convention of §8.3 and never jumps
anywhere else in `CODE`.

### 5.5 Area table

One 24-byte record per area, in the fixed order of §7.1: id (4), length in bytes (8), alignment (4),
flags (4: bit 0 retained, bit 1 holds the starting memory from `INIT`, bit 2 read-only to the
program), offset of its starting memory in `INIT` (4). The lengths are what the device must
provide in RAM; the kernel refuses an image whose lengths exceed its program memory (§10.4).

### 5.6 Task table and I/O bindings

The task table carries specification 18's resource metadata in fixed records: one 40-byte record
per task: name (string offset, 4), interval in nanoseconds (8), priority (4), the `SINGLE`
variable's symbol index or `0xFFFF_FFFF` (4), program count (4), offset of its program list (4,
into a list of entry indexes), flags (4), reserved (8). Programs without a task (background
programs, specification 10 §4) form the record with priority `0xFFFF_FFFF`. `IOBIND` carries one
record per `VAR_CONFIG` binding and each `AT %…` declaration: symbol index (4), direct address
as specification 10 §9.2 parses it (area 1, size 1, byte offset 4, bit 1, reserved 1), flags (4).
The kernel builds its process image and task state from these records as it does today from
`RESOURCE_META`.

### 5.7 Symbol table

Sorted by canonical name (IEC §6.1.2: identifiers are case-insensitive; the canonical name is the
name in capital letters, so a lookup is one binary search), one 32-byte record per symbol:

| Bytes | Field |
|---|---|
| 4 | Canonical full name, string offset (`PLANT.COUNTER.VALUE`) |
| 4 | Display name, string offset (`Plant.counter.value`, as declared) |
| 4 | Type index (§5.8) |
| 1 | Area id (§7.1); 0 for a symbol that holds no storage (a program root, a POU) |
| 1 | Flags: bit 0 retained, bit 1 constant, bit 2 input, bit 3 output, bit 4 in-out, bit 5 program root, bit 6 FB or class instance, bit 7 directly addressed |
| 2 | Reserved |
| 4 | Offset in the area |
| 4 | Size in bytes (the whole variable, padding excluded) |
| 4 | Parent symbol index, or `0xFFFF_FFFF` for a global, a program or a POU |
| 4 | POU id for a POU symbol; for a variable the POU that declares it, `0xFFFF_FFFF` for a global |

Every variable the program declares has a symbol, members of structures and instances included,
so that Live Values, the HMI, OPC UA, Modbus and the debugger find any `a.b.c` by one lookup of its
canonical name, and browse by parent. Frame variables (`VAR` of functions and methods, `VAR_TEMP`)
have symbols with area 7 (stack) and the offset in their frame, for the debugger only.

### 5.8 Type table

One 24-byte record per type: kind (4: an elementary type 1 `BOOL`, 2 `SINT`, 3 `INT`, 4 `DINT`, 5
`LINT`, 6 `USINT`, 7 `UINT`, 8 `UDINT`, 9 `ULINT`, 10 `REAL`, 11 `LREAL`, 12 `BYTE`, 13 `WORD`, 14
`DWORD`, 15 `LWORD`, 16 `TIME`, 17 `LTIME`, 18 `DATE`, 19 `LDATE`, 20 `TOD`, 21 `LTOD`, 22 `DT`, 23
`LDT`, 24 `CHAR`, 25 `WCHAR`; or 100 enumeration, 101 subrange, 102 array, 103 structure, 104 union,
105 string, 106 wstring, 107 reference, 108 pointer, 109 FB, 110 class, 111 interface, 112 alias,
113 program), name (string offset, 4,
or 0), size (4), alignment (4), detail offset (4, into a type detail list), count (4). The detail
list holds, per kind: an enumeration's values (name offset, value i64); a subrange's base type and
bounds (i64, i64); an array's element type and dimensions (lower, upper i64 each); a structure's,
union's, FB's or class's members (name offset, type index, byte offset, flags); a string's
capacity; a reference's target type; an alias's base type. The table carries enough for a tool to
format and write any variable without the sources.

### 5.9 Statement table

One 16-byte record per statement id, ids in source order: file index (2), POU id (2), start line
(4), start column (2), end line (4), end column (2). Positions are 1-based as specification 13
reports them. The table is what specification 13's "one resolver on every runtime" reads instead of
`LINE_TABLE`; its answers are unchanged (§13.1).

### 5.10 Retain map

One 12-byte record per retained variable, in declaration order: symbol index (4), offset in the
retain area (4), size (4). The checkpoint's keys (specification 11 §6.7: the variable's identity,
`@program/<program>/<variable>` for program variables) are the symbols' display names, so a
checkpoint written by today's runtime restores into the new kernel.

### 5.11 Signature

`SIGN` holds: algorithm (4: 1 Ed25519), the signer's public key (32), the signature (64) over
bytes 0 to the start of `SIGN`. The build signs with the project's key (`trust-lsp.toml`
`[signing] key = <path>`; the tools create one on first use and never print it). A device holds a
list of trusted public keys (`device.toml`, or set over the control link by Admin); it refuses an
image signed by another key or not signed. A device whose list is empty accepts unsigned images and
reports `signature: none` in `status`, so that development needs no setup. Signing is for
authenticity and integrity; it hides nothing, and the symbol table and the optional sources keep the
program readable on the device.

## 6. Code generation

### 6.1 Principle

**NX-09.** The generator maps every construct of specifications 01 to 09 to Rust code whose
behaviour is the standard's, and nothing else. A construct the generator does not map is a build
error. There is no interpreter and no fallback.

The generated crate is `#![no_std]`, has no `unsafe` code of its own (the only `unsafe` is in the
interface crate, reviewed once), uses no heap, and calls nothing outside the program library and
the kernel interface.

### 6.2 Data types (IEC §6.4)

| ST type (IEC §6.4.2, Table 10) | Rust | Bytes | Notes |
|---|---|---|---|
| `BOOL` | `u8` (0 or 1) | 1 | Read as `!= 0`, written as 0 or 1 |
| `SINT`, `INT`, `DINT`, `LINT` | `i8`, `i16`, `i32`, `i64` | 1, 2, 4, 8 | |
| `USINT`, `UINT`, `UDINT`, `ULINT` | `u8`, `u16`, `u32`, `u64` | 1, 2, 4, 8 | |
| `BYTE`, `WORD`, `DWORD`, `LWORD` | `u8`, `u16`, `u32`, `u64` | 1, 2, 4, 8 | bit strings |
| `REAL`, `LREAL` | `f32`, `f64` | 4, 8 | IEEE 754 binary32/64 |
| `TIME`, `LTIME` | `i64` nanoseconds | 8 | specification 10's temporal storage, kept |
| `DATE`, `LDATE`, `TOD`, `LTOD`, `DT`, `LDT` | `i64` as specification 10 encodes them | 8 | |
| `CHAR`, `WCHAR` | `u8`, `u16` | 1, 2 | |
| `STRING[n]` | `Str<n>`: `u16` length, `[u8; n]` UTF-8 | 2 + n, rounded up to even | `STRING` is `STRING[80]` (specification 02 §6); even, so that each length in an array is aligned |
| `WSTRING[n]` | `WStr<n>`: `u16` length, `[u16; n]` | 2 + 2n | |
| Enumeration (§6.4.4.2) | the base integer type | | values checked on assignment from an integer |
| Subrange (§6.4.4.4) | the base type | | bounds checked on every assignment (§6.5) |
| `ARRAY` (§6.4.4.5) | `[T; n]`, nested for several dimensions | n × size | bounds checked (§6.5) |
| `STRUCT` (§6.4.4.6) | `#[repr(C)] struct` with explicit padding fields | §7.2 | |
| `STRUCT OVERLAP` (§6.4.4.7) | a byte-backed `#[repr(C)]` struct with bit accessors | §7.2 | |
| `UNION` (specification 02 §3.7) | a struct holding each variant as a member | | as MEM-12 keeps it |
| `REF_TO T` (§6.4.4.10), interface reference | `Ref` (§7.3) | 16 | |
| `POINTER TO T` (extension) | `Ref` with kind pointer | 16 | |
| FB and class instance | `#[repr(C)] struct` of its variables, base first | §7.2 | |

Generic types (IEC §6.4.3) exist only in declarations of standard and library functions; the
generator monomorphizes every call to its argument types.

### 6.3 Areas and symbols in generated code

The generator declares each area as one `#[repr(C)]` type whose fields are the area's variables in
layout order, and emits for every symbol:

```rust
const _: () = assert!(core::mem::offset_of!(Globals, plant) == 16);
const _: () = assert!(core::mem::size_of::<Globals>() == 6_480);
```

so that a mistake in either the layout rules or the symbol table fails the build.

Every variable is a cell (`core::cell::Cell` of its value, a string a cell of its length and cells
of its bytes): a program reads a variable with `get` and writes it with `set` through shared
references, as Structured Text's aliasing needs (a `VAR_IN_OUT` or a `REF_TO` may name a variable the
program also writes by name), in safe Rust and without `unsafe` in the generated crate. A task body
receives shared references to the areas, which the interface crate makes from the kernel's context
for the duration of the entry call (§8.3), and accesses variables as fields:
`w.p.VAT.LEVEL.VALUE.get()`. LLVM compiles such an access to one load or store at a constant
offset, with the aliasing assumptions of C, as every compiler of Structured Text to native code has
them. A type of an area is declared with the interface's `plain_struct!`, which implements the
interface's `Plain` trait only for a `#[repr(C)]` structure whose every field is `Plain`.

### 6.4 POUs (IEC §6.6)

| Construct | Generated form |
|---|---|
| `PROGRAM` (§6.6.4) | an instance struct in the instance area; a body function `fn prog_<name>(cx: &mut Cx, me: &mut <Prog>) -> R`; its task entry calls the body |
| `FUNCTION` (§6.6.2) | `fn f_<name>(cx: &mut Cx, inputs…, in_outs: &mut …) -> Result<T, Fault>`; `VAR` and `VAR_TEMP` are Rust locals; `EN`/`ENO` as specification 04 §EN/ENO |
| `FUNCTION_BLOCK` (§6.6.3) | an instance struct (`VAR_INPUT`, `VAR_OUTPUT`, `VAR`, `VAR_IN_OUT` as `Ref`, nested instances inline); a body function taking `&mut` instance; a call writes the inputs, calls the body, reads the outputs (IEC §6.6.3.4) |
| `METHOD` (§6.6.5.4, §6.6.7.2) | a function taking `&mut` instance; `THIS` is the instance; `SUPER.m()` is a direct call of the base's method |
| `CLASS` (§6.6.5), inheritance (`EXTENDS`) | the base struct embedded as the first field; `OVERRIDE` (§6.6.5.6) resolved statically where the receiver's class is known |
| `INTERFACE` (§6.6.6), dynamic binding | the build knows every class (closed world): a call through an interface reference is a `match` on the instance's class id, held in the first field of every class and FB instance, to the method of that class; `?=` (Table 52) is a `match` on the class id as well |
| `ACTION` (specification 04 §8) | a method of its POU |
| `PROPERTY` (extension) | its get and set methods |
| `NAMESPACE`, `USING` (§6.9) | resolved by trust-hir; names are mangled `ns__pou` |
| Edge-qualified inputs (`R_EDGE`, `F_EDGE`, Tables 40 and 47) | a hidden previous-value byte in the instance and the edge computed at the call, as specification 03 §3 says |
| Test POUs (specification 04) | compiled only by the test build, never into an image |

Calls are direct Rust calls; there is no call table in the program. IEC 61131-3 has no recursion
(§6.6.2.1, §6.6.3.1); trust-hir refuses it, so every call chain is static and the stack bound
computable (§8.5).

### 6.5 Checks the generated code makes

Each is the standard's requirement or specification 10 §10's error, and each ends the scan with the
named fault (§8.6) by returning `Err(Fault)` up the call chain; no panic occurs.

| Check | Where | Fault |
|---|---|---|
| Array index within bounds (IEC §6.4.4.5) | every index that is not a constant inside the bounds | `IndexOutOfBounds` |
| Subrange on assignment (IEC §6.4.4.4) | every assignment to a subrange variable from a wider value | `SubrangeViolation` |
| Integer overflow (specification 05 §3: a runtime error) | `+ - * NEG ABS` on integers: `checked_*` | `IntegerOverflow` |
| Division and modulo by zero (IEC §6.6.2.5.8) | `/`, `MOD` | `DivisionByZero` |
| Conversion range (IEC §6.6.2.5.2 to §6.6.2.5.7, specification 07) | every `*_TO_*` that can fail, through the library | the library's error |
| String capacity (IEC §6.4.2.2 note, specification 02 §6) | every string assignment and `VAR_IN_OUT` binding: longer strings are cut to the capacity as specification 02 says | none (cut), `StringCapacityExceeded` for the 65,535 maximum |
| Null dereference (IEC §6.4.4.10.3) | every `^` | `NullReferenceDereference` |
| Reference validity | every `^`: the reference's area, offset, extent and generation (§7.3) | `ReferenceLifetime` |
| Character index of `s[i]` (specification 02 §6) | every character access | `IndexOutOfBounds` |
| `FOR` step zero (specification 10 §10) | every `FOR` with a non-constant step | `ForStepZero` |
| Statement budget (§8.4) | every statement and loop iteration | `InfiniteLoop` |
| Enumeration value on integer-to-enumeration conversion | the library | `TypeMismatch` |

A `BOOL` read from a byte that is neither 0 nor 1 reads as `TRUE`; the generator never writes
another value.

### 6.6 Expressions and statements (IEC §6.6.1, §7.3)

Operators follow specification 05's precedence and the standard's types: integer `/` truncates
toward zero and `MOD` takes the dividend's sign, as Rust's do; `**` through the library; bit-string
operators on the unsigned types; comparisons of strings by the library (byte order); `AND_THEN` and
`OR_ELSE` short-circuit. `IF`, `CASE` (with ranges and enumeration values, as a Rust `match`),
`FOR` (the bound and step evaluated once, IEC §7.3.3.4.2), `WHILE`, `REPEAT`, `EXIT`, `CONTINUE`,
`RETURN` and `JMP` map to Rust control flow. Each statement begins with the statement hook (§13.2)
and the budget count (§8.4). Partial access to `ANY_BIT` (`x.%X3`, IEC §6.6.1.3) is a masked
read-modify-write. Direct addresses (IEC §6.5.5) are offsets into the input, output and marker
areas, computed at build; an address outside the configured process image is a build error.

### 6.7 Initialization (IEC §6.5.1, §6.5.6)

The build computes the starting memory of the globals, retain and instance areas from the
declarations' initial values (IEC §6.5.1, Table 14) and writes it into `INIT`. A value that depends
on a call or on another variable (a `VAR` initialized by a function call, an FB instance whose
initial state its own method sets) is computed by the cold-init entry, generated code that runs
once at a cold restart after `INIT` is copied in. The warm-init entry initializes the non-retained
variables only. Specification 03's rules on which initializers are eligible are unchanged. The
kernel's restart behaviour is IEC 61131-1's as IEC §6.5.6 refers to it:

- **Cold restart:** every area takes `INIT`; the cold-init entry runs; the retain area takes `INIT`
  too (IEC §6.5.6.2).
- **Warm restart:** the retain area is restored from the newest valid checkpoint (specification 11
  §6.7, RETAIN-02) and the rest takes `INIT`; the warm-init entry runs.
- **Online change:** §12.

The build computes the starting memory with the runtime's own initialization of the same program on
the engineering computer (the declared initial values, the types' defaults, the function blocks'
defaults), so that a compiled program starts where an interpreted one started, and writes each value
at the place the program's layout gives it. Until the cold-init entry carries the initializers that
depend on a call (milestone M2), every initializer is evaluated there.

### 6.8 Standard functions and function blocks (IEC §6.6.2.5, §6.6.3.5)

Every standard function of specification 07 and every standard function block of specification 08
is a Rust function of the program library (§6.9), called directly by generated code: `TON` is
`lib::ton(&mut inst.timer, in_, pt, cx.now)` on an instance struct the library defines
(`#[repr(C)]`, laid out by §7.2 and listed in the type table as an FB). The library's behaviour is
the one specification 07, 08 and `docs/IEC_DECISIONS.md` describe today, with the same tests (the
stdlib oracle corpus, the timer trace cases, the counter decisions). Overloaded timers (`TON` on
`TIME` or `LTIME`) are two monomorphic library functions. Text results (`CONCAT`, `MID`) are
written into the destination's capacity by the library; no text is built on a heap.

### 6.9 The program library and the interface crate

- `trust-plc-abi`: the kernel interface (§8.2): `Cx`, `Fault`, `Ref`, the function table. Versioned
  by the kernel interface version. The only crate with `unsafe` in a module.
- `trust-plc-lib`: the standard functions and function blocks, string, date and time operations,
  conversions, and `libm` (the pure-Rust `libm` crate) for `SIN`, `EXP` and the rest. `#![no_std]`,
  no heap, no `unsafe`. Compiled into every module, dead code removed by LTO, so a module carries
  only what it calls. Its version is in the image header.
- Both are published in the toolchain bundle of the kernel version they belong to, and the kernel
  refuses an image built against another interface version (§10.2).

## 7. Memory

### 7.1 Areas

A program's memory is these areas, each one contiguous block of RAM that the kernel owns and gives
the module through `Cx` (§8.3):

| Id | Area | Holds | Starting memory |
|---|---|---|---|
| 1 | `I` inputs | the input process image (`%I`) | zero; the drivers fill it |
| 2 | `Q` outputs | the output process image (`%Q`) | zero, then the configured start values |
| 3 | `M` markers | the marker image (`%M`) | `INIT` |
| 4 | `G` globals | `VAR_GLOBAL` without `RETAIN` | `INIT` |
| 5 | `R` retain | every `RETAIN` variable: globals and program variables (IEC §6.5.6) | the checkpoint at warm restart, `INIT` at cold restart |
| 6 | `P` instances | each program's instance with its variables and nested FB instances, non-retained | `INIT` |
| 7 | stack | function and method frames, `VAR_TEMP`, temporaries; one per task | — |

`RETAIN` members of a program or FB instance lie in `R`, the rest of the instance in `P`: the
generator splits such an instance into two structs, one in each area, and its code reaches each
member in the right one; the symbol table's parent links join them for the tools. `NON_RETAIN` (IEC §6.5.6) is the default; `PERSISTENT` (extension)
behaves as `RETAIN` and is retained across a cold restart only when specification 11 §6.7 says so.

### 7.2 Layout rules

MEM-12's rules of specification 34 hold unchanged for every type: declared sizes and natural
alignment (at most 8), structures padded to a multiple of their alignment, arrays contiguous,
strings as length and bytes, `STRUCT OVERLAP` and `UNION` as there, instances with their own
variables first and their base's after them. New here: the generator emits the layout as Rust types
and asserts it (§6.3), and a `BOOL` in an array or structure takes 1 byte (no packing of bits), as
MEM-12 says. `SIZEOF` is specification 29's.

### 7.3 References

A `Ref` is 16 bytes: area id (u8), kind (u8: 0 null, 1 variable, 2 instance, 3 character, 4
pointer), character index (u16: the 1-based index of a character reference, 0 otherwise), offset
(u32: the target's first byte in the area), extent (u32: the bytes the target spans), generation
(u32: the installation generation of specification 34 UPDATE-04). A dereference checks that the area exists,
that offset + extent lies inside it, and that the generation is the current one; an online change
repoints or nulls references by its copy plan (§12.3). A reference to a frame variable (IEC
§6.4.4.10.2 forbids references to temporaries; trust-hir refuses them) never exists.

### 7.4 Starting memory

`INIT` holds the bytes of areas `M`, `G`, `R` and `P` after every initializer the build can compute
(§6.7); a cold-init entry computes the rest. The image builder computes `INIT` by the same layout
rules and writes it once; a device copies it with one `memcpy` per area. Its size is the areas'
size, so a program with large zero-initialized arrays still has an `INIT` of that size; a later
version may add run-length coding, recorded in the section flags.

### 7.5 What a program never does

A program allocates nothing, frees nothing, keeps no pointer to kernel memory beyond the scan, and
touches no memory outside its areas and its task's stack. §6.5 and §8.5 enforce this with checks,
and §10.5 with hardware protection.

### 7.6 Maxima

An area is at most 2^32 − 1 bytes; a variable at most 2^31 − 1; a string capacity at most 65,535
(IEC §6.4.2.2 note, specification 02); an array dimension at most 2^31 − 1 elements. A device's
maxima are its program memory (§10.4).

## 8. The kernel

### 8.1 Components

The kernel is specification 34's runtime without decoding, validation, preparation and a virtual
machine: the loader and checker (§10), the scheduler and cycle (specification 10 §4, 11 §6.2 and
34 §8, unchanged), the process image and drivers (11 §6.4, 6.5), faults and the watchdog (11 §6.6,
34 §9), retain (11 §6.7, 34 §10), the program store (34 §5.1), online change (§12), debugging and
monitoring (§13), the control link and protocol (11 §6.8, 34 PORT-05) and the services (34 §11.1).
A chip's firmware composes it as today (specification 34 Appendix H.3 components).

### 8.2 The kernel interface

The interface is the `trust-plc-abi` crate: what a module sees of the kernel. It is versioned by
one integer, raised whenever a record or function below changes; the image header carries it and
the kernel refuses a mismatch (§10.2).

```rust
#[repr(C)]
pub struct Cx {
    pub version: u32,            // the interface version the kernel implements
    pub task: u32,               // the running task's index, u32::MAX for none
    pub areas: [Area; 7],        // base pointer and length of each area, §7.1
    pub now_ns: i64,             // the task's logical time, TIME() (specification 11 §5.2.1)
    pub budget: u32,             // statements left in this activation, §8.4
    pub stmt: u32,               // the id of the statement being executed, §13
    pub armed: u32,              // not 0 while statements look at their breakpoint bits
    pub breakpoint_bytes: u32,   // the bytes of the breakpoint bits
    pub breakpoints: *const u8,  // one bit per statement id, §13.2
    pub api: *const KernelApi,
    pub kernel: *mut c_void,     // the kernel's own pointer, which only its functions read
    pub fault: FaultRecord,      // the fault that ended the call, written by the module
}
#[repr(C)] pub struct Area { pub base: *mut u8, pub len: u32 }
#[repr(C)]
pub struct KernelApi {
    pub stop: StopFn,          // a breakpoint hit, a pause or a step: returns 0 to continue, else the fault to raise
    pub debug_write: DebugWriteFn, // applies queued Live Values writes to one variable at a safe point
}
#[repr(C)] pub struct FaultRecord { pub code: u32, pub stmt: u32, pub detail: [i64; 3] }
```

`StopFn` is `fn(*mut Cx, u32) -> u32` and `DebugWriteFn` `fn(*mut Cx, u32) -> u32`, in the
convention §4.4 names. A module records a fault in `Cx::fault` (its code, the statement and up to
three numbers: the index and the bounds, the value and the bounds) and returns its code; no kernel
function is called for it. A statement looks at its breakpoint bit only while `armed` is set, so a
statement costs a store of its id, a decrement of the budget and two branches when no debugger is
armed.

Two functions are the whole interface a scan can call. Everything else a program needs (time,
inputs, outputs, markers, retain) is memory the kernel prepared before the scan. The kernel calls
the module only through the entry table.

### 8.3 Calling convention

An entry point is `extern "C" fn(cx: *mut Cx) -> u32`, returning 0 or a fault code (§8.6). The
kernel fills `Cx` before each call. Inside the module, functions use the Rust ABI of the pinned
compiler; the module is one compilation unit with LTO, so nothing of its internals is an interface.
A module reads and writes areas only through `Cx`, which the generated code turns into `&mut`
references of the area types for the duration of the entry call.

### 8.4 The statement budget and time supervision

Specification 34's TIME-01 to TIME-07 hold. TIME-08's work-unit model is replaced by:

**NX-10. Statement budget.** The kernel gives each task activation a budget of statements
(`runtime.toml` `[resource] max_statements_per_scan`, default 1,000,000; a chip's descriptor may
lower the maximum). Each statement and each loop iteration decrements `cx.budget`; at zero the scan
ends with `InfiniteLoop` (specification 10 §10). This bounds a scan's work in every program,
including a `WHILE TRUE DO` loop, at a cost of one decrement and one branch per statement.

**NX-11. Elapsed-time supervision.** The watchdog of specification 11 §6.6 and 34 TIME-04 supervises
each task's wall time as today; a hardware watchdog on the chips as today.

### 8.5 Stacks

Each task runs its activations on the kernel's task stack (one stack in the cooperative scheduler
of v1; one per task when preemption comes, §16). The build computes each task's stack bound: the
compiler's `prolog-epilog` remarks (§4.5) give every function's frame; the call graph of the
linked module (its call relocations, kept by `--emit-relocs`) gives the deepest static chain of each
task's programs; the bound is that chain's sum plus the library's declared maximum plus 25%. Until
the call graph is read, the bound is the sum of every function's frame, which no static chain
exceeds since IEC 61131-3 has no recursion (1,584 bytes for the reference workload on the C6). The image's `MEMFIG` carries the bound; the kernel refuses an image whose
bound exceeds its task stack (specification 34 MEM-03: the C6's main stack window), with both
numbers. The kernel fills the stack with a pattern at start and checks the high-water mark after
every scan; a mark inside the margin is the fault `StackOverflow`. §10.5 adds a hardware guard.

### 8.6 Faults

A fault inside a module is a return value: the generated code returns `Err(Fault)` from every
function up to the entry, which calls `api.fault` with the detail (the index and bounds, the
divisor, the reference) and returns the code. The kernel then does what specification 34 §9
(STATE-03, STATE-04) and 11 §6.6 say: records the fault with the statement id, file and line from
the statement table, latches the fault state, applies the output policy. Fault codes are
specification 10 §10.2's stable codes, numbered in `trust-plc-abi`. A panic in a module cannot
happen by construction (no `unsafe`, no panicking operation in generated code: every slice index is
proven in range or checked, every arithmetic checked); as the last resort the module's panic handler
calls `api.fault` with `ModuleInternal` and spins until the hardware watchdog resets the device
(STATE-05), and the build fails any module whose code references the panic machinery
(`panic_fmt`), which the image builder checks in the module's symbols.

### 8.7 Scheduling and the cycle

Unchanged: specification 10 §4 (tasks by priority then due time, `INTERVAL`, `SINGLE`, background
programs, overruns), 11 §6.2 and 34 §8 (inputs read before the batch, outputs committed after,
CYCLE-01 to CYCLE-06), the logical clock of 11 §5. The scheduler calls a program's entry instead of
interpreting it. SCHED-05's 25 ms trace (specification 34 §14.2) is a mandatory test on every target.

## 9. Determinism and numerics

**NX-12.** For the same image, inputs and logical times, every device computes the same bytes. The
rules that make this hold:

- Integers: two's complement, checked as §6.5, identical on every CPU.
- `REAL` and `LREAL`: IEEE 754 binary32 and binary64, round to nearest even, no fused
  multiply-add contraction (Rust emits none without explicit `mul_add`, which the generator never
  emits), no fast-math (Rust has none). Hardware floating point on the U5A9 (single) and on
  computers (SSE2, NEON) and the software floating point of compiler-builtins on the C6 and for
  `LREAL` on the U5A9 are all IEEE-correct for `+ - * / SQRT` and conversions, so results agree.
- Transcendental functions (`SIN`, `COS`, `EXP`, `LN`, `LOG`, `**`, `ATAN`, …): the `libm` crate's
  pure-Rust implementations in the program library on every target, never the operating system's.
  A device's results are therefore the same bits as the computer's and the simulator's.
- `-C target-feature` is fixed per target (§4.5): on x86-64 SSE2 only and no x87; on the C6 no
  floating-point extension; on the U5A9 `+fp-armv8d16sp` for `REAL` only.
- Conversions between real and integer types and between times: the library's one implementation.
- The conformance corpus (§14.2) is run on every target and its outputs compared byte for byte.

## 10. Loading, installation and the program store

### 10.1 Container and slots

The program store, its two slots, the install steps, the commit point, the boot selection and
"never write the running program's slot" are specification 34 §5.1 unchanged. The container
becomes `TRSTA003`: the same header with entry kinds 3 (the program image) and 2 (the deployment
record); kind 1 (STBC) is refused. `cargo xtask portability pack-project` packs an image.

### 10.2 Checks at install and at boot

Before an image runs, on every device, in this order, and the first failure names its reason:

1. The container (specification 34 §5.1) and the record (§5.1.1).
2. The image header: magic, CRC, length inside the slot, format version 1, target id equal to the
   device's, kernel interface version equal to the kernel's (`kernel interface <n> required, this
   runtime has <m>: rebuild the project`), program library hash known to this kernel.
3. The section table: each required section present, no overlap, all inside the image, 8-byte
   alignment, `SIGN` last.
4. The signature (§5.11) against the device's trusted keys.
5. Every offset in every table inside its section or area; every entry offset inside `CODE`; every
   symbol's offset + size inside its area; every task's programs existing; every I/O binding inside
   the configured process image; every retain record inside `R`.
6. The memory figure (§10.4) against the device's program memory, and the stack bound (§8.5)
   against the task stack.
7. For an install while a program is held: the retained-value check of specification 34 §11.2
   (decision G13), which now compares the new image's retain map with the checkpoint's keys and
   types by name, without building the program.

The checks read the image where it lies and allocate nothing beyond a few records; on the C6 they
take the mapped slot. An image that passes is "admitted"; nothing of it is copied to RAM except
`INIT` into the areas at start.

### 10.3 Mapping the code

The kernel runs `CODE` where the image lies when the platform maps flash as executable memory (the
C6 through its MMU window, specification 34 MEM-14 as 0.52 built it; the U5A9's flash is executable
in place) and otherwise copies `CODE` and `RODATA` into memory it maps executable (computers:
`mmap`/`VirtualAlloc` with execute permission, written before it is made executable; on macOS with
`MAP_JIT` and the `allow-jit` entitlement of the notarized binary; the simulator the same). The
copy's placement keeps `RODATA` at its fixed distance after `CODE` (§5.3). Position independence
makes any placement correct.

### 10.4 The memory figure

The build computes what the program needs in RAM, replacing the device-side figure of specification
34 MEM-12 (C1-21): the areas' lengths (§5.5), the stack bound (§8.5), the kernel's per-program
bookkeeping (tasks, breakpoints: one bit per statement, Live Values queues, as the kernel version
declares in a table of constants per device class), and the retain capture buffer (twice the retain
area, specification 34 MEM-09). It is written to `MEMFIG` and `build.json`, shown by the tools before
deploy, and checked by the device against the program memory it measures at boot (specification 34
Appendix D.2 and D.5 define that measurement; it changes only by no longer subtracting a prepared
program). `status` and `capabilities` report it as today.

### 10.5 Hardware protection (milestone M5)

On the C6 the kernel programs the RISC-V PMP and on the U5A9 the MPU so that, while a module runs,
the CPU can write only the areas and the task's stack, read the image and the areas, and execute
only `CODE`. A violation is the fault `MemoryProtection` and ends the scan; the kernel keeps running.
On computers the task stack has guard pages. Until M5 the checks of §6.5 and §8.5 alone hold.

## 11. Retain

Specification 11 §6.7 and 34 §10 hold: the checkpoint format (STRN v3), its keys, the save
triggers (periodic, stop, due), the checkpoint log in flash with its two slots, RETAIN-01 to
RETAIN-05. What changes: the capture reads the retain area through the retain map (§5.10) instead
of walking declarations, which is one `memcpy` per record and no decoding; the restore writes
through the same map after checking each key's type against the type table; a key the new image
does not declare is reported as `retain_orphan_dropped` as today; a declared key the checkpoint
lacks takes `INIT`.

## 12. Online change

### 12.1 Behaviour

Specification 34 §11.3 holds in full: the request (`bytecode.reload` keeps its name until the
protocol's next version renames it `program.change`), the boundary, the one reply in flight, the
five-second withdrawal on a chip, the lines, the settings, the events, the generation, the
debugger; and above all: every variable the new program declares at the same place and by the
same name keeps its value, retained or not, instances and timers included; a changed declaration
keeps what fits; references name the same variable or become `NULL`; a change never fails because
of a value; the outputs never change at the boundary (decisions OC-1, OC-2).

### 12.2 The copy plan, computed on the computer

**NX-13.** The build computes the copy plan when it knows the running program: the tools keep the
last image deployed to each device in the project (`.trust/deployed/<device>/program.tpi`), and when
they do not, the deploy reads the running image's symbol and type tables from the device
(`program.active` answers them, as specification 34 §11.2 says of the headers). The plan is a table
of records (old area, old offset, new area, new offset, size, conversion id) produced by matching
the two symbol tables by canonical name and type:

- same type: copy the bytes;
- a simple type that the new type holds (`INT` to `DINT`, a value inside a new subrange, an
  enumeration value by name): the named conversion;
- a structure or instance: its members, by this rule, recursively; an array: its elements by index
  up to the shorter length;
- a reference: repoint by the plan when its target exists in the new program, else `NULL`;
- anything else: the new declaration's initial value, and an `online_change_value` event naming the
  variable, as today.

The plan is deterministic and small (one record per variable kept), and it is transferred with the
image in the same upload as a `TRSTA003` entry of kind 4. A plan the device finds inconsistent with
the two images (an offset outside an area, a size mismatch) refuses the change before anything
changes.

### 12.3 Applying the change

At the boundary the kernel: copies `INIT` of the new image into a second set of areas (the device
holds both programs' areas during the change, admitted by §10.2 step 6 against the memory the running
program leaves free, UPDATE-06); applies the plan from the old areas to the new; repoints the
references; swaps the area set and the entry table; retires the old areas. The scan that follows
runs the new program with the old program's values and the task schedule continued (specification
34 0.46, 0.49). Nothing is written to flash at the boundary: the new image was installed into the
other slot before the request, as today.

## 13. Debugging and monitoring

### 13.1 Source positions

Specification 13 holds. Its resolver reads the statement table (§5.9) instead of `LINE_TABLE`; its
answers are the same for the same program, and the parity tests of specification 13 keep proving it.

### 13.2 Breakpoints, pause and stepping

Each statement begins with the generated hook:

```rust
cx.stmt = 1234;
if cx.breakpoints[1234 / 8] & (1 << (1234 % 8)) != 0 { stop!(cx, 1234); }
```

Two loads and a branch when no breakpoint is set. The kernel sets a statement's bit for a
breakpoint, for a step (the bits of every statement that may follow, cleared after the stop) and for
a pause (every bit); conditions, hit counts and logpoints are evaluated by the kernel in `api.stop`
as today. `api.stop` runs the hold of specification 34 STATE-07 and 11 §6.9 (serving the link,
applying writes at the safe point, the production-mode hold rules) and returns. A production image
may be built without hooks (`build.debug = false`): it is not debuggable and says so in its header
flags, as CODESYS's "release" build does.

### 13.3 Variables, Live Values, HMI and services

Every read or write of a variable by name, from the debugger, Live Values, the HMI, OPC UA, Modbus,
ADS, OpenOT and the control protocol, is a symbol-table lookup (one binary search in flash) and a
typed access to the area at the symbol's offset, converted at the boundary as MEM-12 says; a
structure or instance is browsed by the parent links. Writes and forces are queued and applied at
the safe points of specification 13 through `api.debug_write`. `debug.evaluate`, logpoint
expressions and conditions keep specification 13's tool-compiled form, which reads values through
the symbol table. Frame variables of a function or method are read from the task stack while the
task is held inside that function (the symbol's frame offset plus the frame's base, which the hook
records in `Cx` on entry to a function when hooks are on).

## 14. Verification and acceptance

### 14.1 The reference workload

`examples/vat_pasteurizer` is the reference: the batch control with `Control.PID`, the state
machine, twelve alarms, status texts, retain, the plant simulation and the operator, on tasks of
10, 20 and 500 ms. It is run on the C6, the U5A9 and a Linux computer, and three numbers are
reported for every milestone: the memory figure, the worst-case scan time of the control task over
one hour, and the shortest control-task interval with zero missed cycles over one hour.

### 14.2 The conformance corpus

The ST corpus of the repository (the stdlib oracle tests, the timer, counter and edge traces, the
runtime behaviour locks, the A4 fixture and trace, the source-free fixtures rewritten as source,
OSCAT with its tests, PLCopen motion, the IEC examples of specifications 01 to 09, the debugger
corpus of specification 13), built into images and run on every target. Pass: every expected value,
fault and event of the corpus; bit-identical outputs across targets (§9); the debugger parity
tests; SCHED-05's trace.

### 14.3 Against RuSTy

From M2, CI compiles the pasteurizer's control program and the OSCAT corpus with RuSTy and with
truST on x86-64 and reports the scan time of each and the size of the generated code. Acceptance
for M4: truST is not slower than RuSTy on either, within 5%, and its RAM figure is smaller.

### 14.4 Milestones

| Milestone | Weeks | Accepted when |
|---|---|---|
| M0 | 1–2 | this specification approved by the owner; the kernel interface and the image format frozen (interface version 1); the toolchain bundle published; the pasteurizer committed; one branch |
| M1 | 3–5 | the pasteurizer runs from an image on the C6 (mapped slot) and on Linux: batches complete against the simulation; memory figure ≤ 15 KB; worst-case control scan ≤ 0.5 ms on the C6; faults, watchdog and the budget work |
| M2 | 6–8 | the whole conformance corpus passes on Linux and the C6 with identical outputs; retain through the map; the RuSTy benchmark running |
| M3 | 9–11 | online change on the pasteurizer while it runs with outputs unchanged; breakpoints, stepping, Live Values, HMI, OPC UA and Modbus by the symbol table on both chips; the U5A9 at the numbers of §14.1; install checks complete |
| M4 | 12–14 | trust-runtime on Linux, Windows and macOS on the new kernel; the simulator; the extension's toolchain download; the old execution layer deleted (§15); the RuSTy acceptance; the release |
| M5 | after | hardware protection (§10.5); preemptive tasks (§16); Wi-Fi on the C6 with the freed RAM |

A milestone is accepted by its numbers in the weekly report, nothing else. A change to this
specification between milestones needs a written reason and the owner's approval.

## 15. Migration and deletion

1. The new kernel and generator are built beside the old engine in the same crates behind the
   feature `native` until M4; CI runs both against the corpus in M2 and M3 and compares outputs.
2. At M4 the old execution layer is deleted in one change: `trust-runtime-core/src/bytecode`,
   `src/vm` with its compact, kinded and register paths, the preparation tables, the STBC encoder
   in `trust-runtime/src/bytecode`, `LegacyTemporal32` and the legacy loaders (checklist C8),
   the bytecode fixtures, and the tests of those parts. Specification 12 is marked superseded by
   this document; specification 34's superseded sections get a note pointing here.
3. Checkpoints written by the old runtime restore into the new kernel (§11); a `TRSTA002` container
   with STBC is refused at boot with "rebuild the project", never run.
4. The `examples/vat_pasteurizer` project and this specification are committed at M0; nothing of
   the old loading work after `4de1a57df` is continued.

## 16. Later work, outside this version

- Preemptive task scheduling with priorities (IEC §6.8.2 allows preemptive and non-preemptive
  scheduling; v1 is non-preemptive as today), one stack per task, I/O consistency per task.
- Several resources per image and several images per kernel (checklist S17, S18).
- Run-length coded `INIT`; a production image without hooks on by default for releases.
- A `TRSTI` reader in the LSP so that a project opened without its sources can be browsed from the
  image's symbols and embedded sources.

## Appendix A — Fault codes

`trust-plc-abi` numbers the faults a module raises, and the kernel maps each to the runtime error of
specification 10 §10.2 named after it: 1 `DivisionByZero`, 2 `ModuloByZero`, 3 `Overflow`, 4
`IndexOutOfBounds` (index, lower, upper), 5 `SubrangeViolation` (value, lower, upper), 6
`NullReference`, 7 `ForStepZero`, 8 `TypeMismatch`, 9 `DateTimeRange` (reported as `Overflow`), 10
`StringCapacityExceeded`, 11 `ReferenceLifetime`, 12 `NonFiniteValue`, 13 `InfiniteLoop` (reported
as the work limit of TIME-08, `runtime_execution_timeout`), 14 `StackOverflow`, 15
`MemoryProtection`, 16 `ModuleInternal` (the last three reported as an invalid execution state until
the runtime has errors of their own).

## Appendix B — Revision history

| Version | Date | Change |
|---|---|---|
| 0.1 | 22 October 2026 | First complete version, written after the vat pasteurizer did not fit the ESP32-C6 (160 KB needed, 51 KB available) and the owner asked for the state of the art, no spike and an exact specification. Approved by the owner the same day ("implement the checklist"). |
| 0.2 | 22 October 2026 | Exact where milestone M1's implementation decided within the approved design: the generator reads trust-hir's recorded types and resolved symbols (§3); one module per CPU for every computer, its calling convention named (§4.4); the flags as they link (static PIE, `prolog-epilog` remarks, because `-stack-size-section` has no effect on stable rustc, §4.5, §8.5); table sections with a count, `CODE` at 64 bytes (§5.3); an entry per program body (§5.4); the type kinds listed (§5.8); strings rounded to even (§6.2); variables as cells behind shared references so that the generated crate needs no `unsafe` (§6.3); the context with the fault record in it and two kernel functions (§8.2); the fault codes listed (Appendix A). Measured: the reference workload's image for the C6 is 86 KB with 43 KB of code, and its memory figure 4,546 bytes (§10.4). No change of direction. |
