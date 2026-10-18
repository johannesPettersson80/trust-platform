# truST Runtime Portability Specification

- **Version:** 0.36 — the client of the byte-stream control link (PORT-05) and the `serial://` endpoint and `ctl install` of specification 11 §6.8.1 (checklist L1a); after 0.35 — a computer keeps trust-runtime up in the Stopped state (owner, 18 October 2026, decision Q11): STATE-08 and §11.2; after 0.34 — the owner's four debugging decisions (checklist S3, debugging on chips): the debug service resolves breakpoints and stepping from the module's optional line table; after 0.33 — the STM32U5A9 as the STM32 chip (owner, 17 October 2026): ARCH-10, §1.2, Appendix D.4 and D.5, and Appendix H; after 0.32 — the C6's 24 KiB main-stack window (D.2) and the due-boundary rule for requests (STATE-08), after 0.31's call scratch that binds a call, the device runner's use of the checkpoint persister (§10), after 0.29's control link on a byte stream
- **Date:** 19 October 2026
- **Repository:** `johannesPettersson80/trust-platform`
- **Reviewed baseline:** `main` at `be8d81a4a7ab16ca7554b8be0f4723161ec1a47b`
- **Baseline commit date:** 3 September 2026
- **Source recheck:** `main` at `9a15065725c17da2c912055f1509368d3fd01d6c`, 9 October 2026; the reviewed runtime/core sources, workspace manifests/lockfile, specifications, and native CI workflow are unchanged from the original baseline.
- **Scope:** One truST runtime for Linux, Windows, macOS, the STM32F407 and the ESP32-C6, with separately scoped dependency/toolchain modernization and target qualification.

**Revision basis:** Retains the A3/A4 construction and shared-engine contracts, B-R1's uniform PLC function set and B-R2's sector-0 placement with a reported 16 KiB upper-region growth target. Selects H's construction-only raw mutation, checked installed-state writes and isolated candidate schema changes in §4.3.1. Adds the uniform eight-byte temporal storage contract and STBC 3.0 compatibility boundary in §4.3.1. Adds the TIME-08 work-unit cost model and the TIME-09 construction allowance, replacing H's per-structure charge rules, and HOST-04's in-place call guarantee. Hosted review corrections and temporal implementation are authored but remain unverified until their consolidated acceptance batch; this specification is not validation evidence. Current source, evidence and authorization remain in the portability checklist; the main-branch baselines above are historical. [D01–D07]

**Requirement status:** This is the canonical implementation plan. “Shall” defines required behavior for the stated future profile, not behavior already implemented. Existing identifiers are retained. Specifications 11/12 distinguish the legacy STBC 1.1/2.0 contracts from the STBC 3.0 migration. Scope authorization and validation follow AGENTS.md; a milestone description is not authorization or evidence that it passed.

## Owner requirements

These are the owner's own words. Every section of this specification carries them out. A section
that conflicts with them is wrong: raise it with the owner, never build it.

- One runtime everywhere: "i would want a universal small runtime that can run on stm32 also, is
  this possible? i dont want many different ports of truST" (5 October 2026).
- No reduced functions: "i never want reduced functions, it will make it impossible with different
  features for different platforms" and "we shall not make anything that reduce functionality of
  the runtime." (10 October 2026).
- The boards in the hardware canvas: "both the esp and stm shall be able to be selectable in the
  hardware canvas" (15 October 2026).
- Debugging from VS Code: "you shall be able to debug from vs code etc." (16 October 2026).
- The same as trust-runtime: "it shall work exactly the same as for the trust runtime on linux, mac
  and windows. thats the whole idea" (16 October 2026).
- Any link: "we shall not only have usb to these boards, we shall be able to connect via wifi, eth
  or whaever" (16 October 2026).
- No limits: "i dont want limit" (16 October 2026).
- Hardware is the owner's decision: "if the runtime does not fit the stm32 i will give you another
  one" (16 October 2026).
- The chip, not the board: "we shall develop for the chip, not for the pheripels. if we detect a
  stm32 on ethernet we shall be able to use it." (16 October 2026).

## 1. Decision and scope

**A1 source status:** The A1 branch implements nominal-deadline readiness, explicit std/no_std collection features, shared libm primitives and target value-slot guards. Its checklist records review and validation separately; none of this establishes compiler-free loading, firmware fit or qualified hardware execution. The checklist records A2/A3 source work and evidence; shared-engine execution and hardware qualification remain later scopes.

truST shall retain one shared Rust implementation of its PLC execution semantics and its STBC bytecode instruction set. Different processors shall receive different compiled firmware binaries, not independently maintained VMs.

The implementation shall extend the existing `trust-runtime-core` extraction. It shall not create a competing core or rename the existing Linux/product runtime simply to accommodate the first MCU.

**This is a runtime-wide improvement, not an embedded-only fork.** Linux, Windows, and macOS products shall consume the same extracted core as MCU firmware. Hosted migration and qualification are required deliverables, not optional follow-up work.

The intended separation is:

> Shared Rust execution engine + validated prepared modules + bounded instance storage + explicit scheduling/numeric contracts + platform and board adapters.

A HAL is necessary, but is not the whole migration. The reviewed code still connects VM execution to the host `Runtime`, compiler-oriented type information, dynamically allocated values, and hosted scheduling facilities. Those dependencies must be separated before an MCU adapter can execute a complete application. [R02–R08]

### 1.1 Intended outcomes

| ID | Requirement |
|---|---|
| ARCH-01 | The same admitted STBC application shall satisfy its declared PLC semantic, scheduling, numeric, import, and logical I/O contract on every claimed target. Exact-trace claims apply only to operations and observations qualified for equality under Section 12; platform math is not assumed bit-identical, and tolerance-qualified numerics do not imply identical threshold decisions. |
| ARCH-02 | MCU firmware shall not depend on compiler/HIR/IDE crates or on operating-system services. It uses the same runtime service code as the hosted product (ARCH-11): the parts of today's `trust-runtime` crate that those services need become shared code that builds without an operating system. |
| ARCH-03 | `trust-runtime-core` shall retain its `forbid(unsafe_code)` boundary. Hardware-specific unsafe operations, where required, shall remain outside it and follow the repository's review policy. |
| ARCH-04 | Platform adapters shall provide mechanisms; the common engine shall own IEC behavior, scheduling decisions, memory rules, and fault decisions. |
| ARCH-05 | Supported bounded controller profiles, hosted and MCU, shall execute without general-purpose allocation, reallocation, or deallocation in their control-cycle paths, including fault paths. |
| ARCH-06 | Unsupported instructions, imports, types, capabilities, or resource demands shall cause an explicit admission failure, never silent fallback or changed semantics. |
| ARCH-07 | The controller shall operate independently of the development computer after a valid application has been installed and started. |
| ARCH-08 | Hosted products shall retain their existing supported features and interfaces while adopting the shared core; MCU limits shall not become universal desktop limits. |
| ARCH-09 | Hosted adoption shall be verified through native execution tests on Linux, Windows, and macOS, compiler-free loading tests, and baseline-relative resource and performance evidence. |
| ARCH-10 | The planned platform-support outcome requires one STM32 chip and one ESP32 chip to qualify with the same runtime: the STM32U5A9 (first on the owner's Riverdi RVT50HQSNWC00-B, a 5-inch STM32 Embedded Display) and the ESP32-C6 (on the ESP32-C6-DevKitC-1). Firmware is developed for the chip and every interface of the chip that the runtime services use, for example the U5's USB OTG and the C6's Wi-Fi and USB; a board's configuration only names which of them are wired, its pins and its Ethernet PHY, so any board with the chip runs the same firmware with its own configuration. An STM32 release may be delivered first, but shall not close the ESP32 deliverable. A different MCU family shall not silently substitute for ESP32. |
| ARCH-11 | Every target runs the same runtime, not only the same engine. The runtime services of trust-runtime behave on a board as on Linux, macOS and Windows: the control connection and its protocol, status, start, stop and restart, the resource states and fault handling, Live Values with read, write, force and release, debugging with breakpoints, pause, continue, stepping, call stack and variables (specification 13), loading a program including online change, access rights, discovery, the communication services, including Bluetooth devices where the chip has Bluetooth, and the web pages and HMI. They are shared code, not a separate implementation per target. VS Code and the other tools reach a board exactly as they reach any trust-runtime; nothing in them is specific to boards. |
| ARCH-12 | A runtime is reached over whatever link its hardware has: a local socket or TCP on a computer, TCP over Ethernet or Wi-Fi and Bluetooth on a board that has them, and USB on a board on a cable. The control protocol is the same on every link; the platform adapter supplies the link. A runtime on the network, a board included, is found by the same discovery as any trust-runtime and can be used at once. |
| ARCH-13 | No target has limits of its own: functionality is never reduced to fit a board. When a board lacks the memory or the hardware for a service, for example a network port, this is reported, and the owner decides on the board. |
| ARCH-15 | Several runtimes on every host (owner, 17 October 2026: "every host shall be able to have several runtimes"). A computer runs several trust-runtime instances, each with its own project, as today. A chip's firmware runs several runtimes in the same way: each has its own installed project, resource state, control connection, retain and diagnostics, and Stop, Start and faults of one do not change another; they share the chip's processor, memory and I/O, and each I/O point belongs to one runtime only. How many fit is measured per chip and reported (ARCH-13); the tools show each runtime of a host as its own runtime. |
| ARCH-14 | Chips and components (owner, 16 October 2026). The truST runtime is a set of components: the engine (`trust-runtime-core`) and the runtime service components above it (ARCH-11), the same code on every target. Linux, macOS and Windows run every component. Each supported chip gets one firmware, released ready-made, that holds every component the chip can hold, as measured; its chip file lists those components (TARGET-03). The tools read the list, offer what the device has, and refuse a project that needs a component the device lacks, naming the chips that have it (ARCH-13). Development boards are examples, for the bench and the documentation; they are neither components nor files of truST. |

The research supports this separation of responsibilities, not a universal claim that interpretation is smaller or faster than native compilation. The first implementation shall retain STBC and the existing Rust executor. Native compilation, replacing STBC with WebAssembly, or a generalized multi-backend framework shall require a separate, measurement-led decision; none is a prerequisite for this migration. Existing validated internal execution optimizations may remain within the shared semantic implementation. [D02, E05–E09]

### 1.2 Scope boundaries

This proposal starts a runtime-wide execution improvement workstream and a new embedded implementation phase. The existing closed core/host extraction checklist explicitly excluded STM32 bring-up, ESP32 support, and an embedded product promise. Its closure therefore does not mean that MCU support is already implemented. Those old exclusions remain accurate descriptions of that earlier work. [R13]

The STM32 target is the **STM32U5A9** (owner, 17 October 2026): a Cortex-M33 with TrustZone and a single-precision FPU at up to 160 MHz, 4 MB flash in two banks, 2.5 MB SRAM, an Ethernet-less 216-ball package with USB OTG high speed and an on-chip PHY. It is first used on the owner's **Riverdi RVT50HQSNWC00-B**; the display of that module is not part of this track. The STM32F407 track of 16 October 2026 is **paused**, and its chip file, adapter, firmware and evidence stay in the tree (Appendix D.4 and D.5). The STM32F407 record that follows stands as the paused track's: a Cortex-M4 with single-precision FPU at up to 168 MHz, 1 MiB flash, 192 KiB SRAM including 64 KiB of core-coupled memory, an Ethernet controller and USB OTG. Its RAM limits and the RAM questions raised against them (O5) are the paused track's; the STM32U5A9 carries the same firmware with megabytes of flash and SRAM to spare (Appendix D.5). It was first used on the owner's **STM32F407G-DISC1**, whose board has USB to the chip but no Ethernet PHY; Ethernet needs a board, or an add-on, with a PHY (ARCH-10). The NUCLEO-F401RE was the first reference board, and its scope B evidence remains; it is no longer the release target. Its firmware was retired 17 October 2026 (owner). Its STM32F401RE provides a Cortex-M4 with single-precision FPU, a maximum CPU frequency of 84 MHz, 512 KiB flash, and 96 KiB SRAM. These are device capacities, not bytes available exclusively to truST. The board makes compactness an early constraint; complete-runtime fit and timing remain unmeasured. Record the physical board/MCU revision, memory reservations, pins, clock source, toolchain, and HAL before board implementation. [E24–E27]

One **ESP32 target is required**. The revised provisional reference is **ESP32-C6-DevKitC-1**, using `riscv32imac-unknown-none-elf` and `esp-hal`, with its Wi-Fi as a network link of the runtime (ARCH-12). No ESP32 board possession or purchase is asserted. C6 supplies the A extension needed by the existing atomic-reference-counted values during bring-up; it does not make those values bounded. M0 confirms board availability and freezes the build. M1 checks its isolated core graph, M2E executes the common fixture on actual hardware, and M5 qualifies the profile. C3/IMC becomes a later optional no-CAS profile after storage/atomic retirement, rather than contradicting the early-storage sequencing. RISC-V/Xtensa and bare-metal/ESP-IDF choices remain distinct. [E56, E57]

An STM32H7 board remains an optional later target. It is not a prerequisite to start on the available F401. When the runtime does not fit the F401, or the F401 lacks hardware that a service needs, for example a network port, that is reported and the owner selects the STM32 board (ARCH-13); a larger board shall not be reported as a passed F401 gate. Native ST compilation, an assembly rewrite and FPGA execution are outside this implementation. Every runtime service of trust-runtime, including online change and the communication protocols, is in scope on the boards (ARCH-11). Existing supported hosted online-change workflows remain subject to the compatibility requirements.

This specification defines an operational control runtime. It does not establish a certified functional-safety runtime or replace independent machine safety measures.

## 2. Repository findings

This is a source and architecture review of the pinned baseline, not a measured firmware assessment. Builds, tests, linker sizes, or hardware timing were not executed during this review. Existing checklist results are repository-reported evidence, not newly reproduced results.

| Reviewed area | What exists | Consequence for this design |
|---|---|---|
| `crates/trust-runtime-core/src/lib.rs` | Conditional `no_std`, `extern crate alloc`, `forbid(unsafe_code)`, and portable value, task, VM-helper, bytecode, and fault modules. | Extend this crate rather than adding another core. `no_std` does not establish allocation-free execution. [R02] |
| Core and workspace manifests | The core defaults to `std`; `hir` also enables `std`. Core dependencies inherit workspace defaults. | Audit the resolved dependency graph for the actual MCU target, not just the crate-level attribute. [R01, R03] |
| `crates/trust-runtime/Cargo.toml` | Compiler/IDE, UI, web, network, physics, and other host dependencies; several protocol/database defaults. | Do not make the small MCU firmware a feature-reduced build of this host crate. [R04] |
| `runtime/core.rs` and `runtime/vm/mod.rs` | The VM dispatcher operates on the host `Runtime`. That state includes `TypeRegistry`, services, debug data, hosted deadlines, and VM caches. | A portable execution-state boundary still has to be extracted. Moving helpers is not equivalent to having a standalone engine. [R06, R07] |
| `trust-runtime-core/src/vm/stack.rs` | The operand stack stores `Vec<Value>` and pushes values during execution. | A count limit is not a reservation strategy; storage must be reserved or provided before RUN. [R08] |
| `trust-runtime-core/src/value/types.rs` | Arrays use vectors; structures contain named `IndexMap` fields; the model uses allocated ownership types. | Audit compound values, copying, temporary values, and reference ownership, not only the top-level VM stack. [R09] |
| `io/driver.rs` | `IoDriver` already exchanges whole input/output images and reports health. | Preserve this PLC-oriented boundary; do not replace it with per-instruction GPIO calls. [R10] |
| `scheduler/clock.rs` | `Clock` includes `now`, `sleep_until`, and `wake`, with hosted implementations. | Separate time observation from platform waiting and scheduling mechanisms. [R11] |
| `runtime/cycle.rs` | Inputs precede execution; ready tasks and background work precede output writes; retain work is on this path too. | Preserve the established cycle semantics and explicitly classify any change to persistence/fault ordering. [R12] |
| Bytecode container and validation ownership | Core owns records, BytecodeReader/helpers, errors and stable codes. Host `bytecode/` still owns BytecodeModule, full decoding/validation, byte serialization, and metadata materialization; `encoder/` separately lowers runtime/compiler data into bytecode. | Scope A relocates the complete byte-oriented loader/validator boundary into core and preserves host adapters. No existing core decode API is assumed. [R14, R36] |
| Behavior-lock tests | Existing source fixtures, cycle boundary checks, typed value checks, scheduler and restart references. | Reuse them, and add a genuinely compiler-free loader test plus frozen pre-migration artifacts. [R13, R15] |
| `runtime/online_change.rs` | Documents replacement at a scheduler-cycle boundary, warm restart at the entry point, and retain handling. | Scope executable immutability to an installed generation; preserve this hosted update contract rather than introducing no-restart migration or MCU live update. [R19] |
| `runtime/bytecode.rs` | Applying bytecode validates task names and FB references against already-registered programs and storage. | A complete loader must construct that state from artifacts, not require a host-prebuilt runtime. [R20] |
| `runtime/vm/local_init.rs` and `local_init/expr.rs` | Local initialization uses HIR types/runtime definitions and calls a harness initializer helper. | Extract executable initialization together with loading; serializing constant defaults alone is insufficient. [R21] |
| `memory.rs`, core values, and locked `smol_str` | Variable storage has named maps and `RwLock` caches; values and `smol_str` 0.2.2 use `Arc`. | Resolve storage ownership and transitive atomic requirements before fixing the MCU-facing API. [R09, R22, R24] |
| `stdlib/numeric.rs` | Several `REAL` standard functions use `unary_real`, which computes through `f64`; math calls use hosted floating-point methods. | Select a portable math implementation and measure double-precision work without silently changing results to single-precision evaluation. [R23] |

The reviewed `runtime_core_behavior_lock.rs` constructs its fixture runtime through `CompileSession` before applying bytecode. It is useful regression evidence, but it does not prove that a standalone MCU runtime can initialize itself from STBC without a compiler. That requires an additional test. [R15]

## 3. Architecture and ownership

### 3.1 Dependency structure

```text
Development computer
  ST sources + libraries + project configuration
                    |
          existing compilation/build path
                    |
       program.stbc + deployment metadata
                    |
   control connection: the same protocol on every link
   (local socket, TCP over Ethernet or Wi-Fi, USB)
                    |
      +-------------+--------------------+
      |                                  |
Hosted product runtime              MCU firmware
trust-runtime                       board entry point
Linux / Windows / macOS                   |
      |                                  |
OS-specific adapters                STM32 / ESP32 adapter
(OS, files, network)                (pins, timer, flash,
      |                              Wi-Fi/Ethernet, USB)
      +-----------+----------------------+
                  |
        shared runtime services (ARCH-11)
        - control connection and protocol
        - states, start/stop/restart, faults
        - Live Values: read, write, force, release
        - debugging: breakpoints, pause, step, variables
        - program loading and online change
        - access rights, discovery, communication, web/HMI
                  |
        trust-runtime-core
        - validated executable model
        - bytecode execution
        - task/cycle policy
        - PLC state and process image rules
        - portable standard-library semantics
        - limits and fault decisions
        - small platform interface definitions
```

The arrows above indicate dependency/composition, not a requirement that the core knows the concrete adapters. All hosted products and MCU firmware shall supply adapters to the same core APIs.

### 3.2 Proposed repository organization

```text
crates/
  trust-runtime-core/              EXISTING; extend
    src/
      bytecode/                   portable reading/validation and records
      vm/                         shared executor and instruction semantics
      memory/                     bounded execution storage contracts
      program_model/              executable metadata without HIR dependency
      cycle/                      common cycle orchestration
      scheduler/                  readiness and ordering policy
      stdlib/                     portable primitives needed by execution
      ports/                      small platform contracts
      ...                         existing value, error, retain, watchdog modules

  trust-runtime/                  EXISTING; hosted product and compatibility APIs
    src/host/platform/            proposed Linux/Windows/macOS adapters
    ...                           existing services, configuration and tooling

  trust-platform-stm32f4/          NEW; STM32F4 platform adapter, one Cargo feature
                                  per chip (the F407); chip differences are data
    src/
      clock.rs
      io.rs
      watchdog.rs
      storage.rs

  trust-platform-esp32/            NEW; selected ESP32 adapter, initially C6

  trust-board-fixture/             NEW; the M2E fixture shared by the F407 and C6
                                  compositions: runner, records, bundle decode,
                                  heap and stack measurement

  trust-runtime-services/          NEW; the shared runtime components (ARCH-11,
                                  ARCH-14), no_std + alloc: control connection
                                  and access rights, start/stop, Live Values,
                                  debugging, deployment, drivers, network
                                  services; used by trust-runtime and firmware

  trust-chips/                     NEW; reads and checks the chip files and embeds
                                  them as the catalog of supported chips
                                  (Appendix H); host only, no firmware dependency

chips/                             NEW; one file per supported chip (TARGET-03)
                                  with its memory layout, interfaces and the
                                  components its firmware holds

firmware/
  trust-stm32f407/                 NEW; the STM32F407 chip firmware (ARCH-10); the
                                  bench board's wiring is its configuration
                                  (src/board.rs), not a firmware of its own

  trust-esp32c6/                   NEW; the ESP32-C6 chip firmware (ARCH-10)
```

Names inside the core are proposed ownership areas, not instructions to rename every existing module at once. Existing public host APIs shall remain available through wrappers or re-exports during migration.

Do not introduce a crate for every trait. Define the portable interfaces in `trust-runtime-core::ports`. Add shared embedded-service or driver crates only when there is a concrete consumer and a clear dependency boundary.

### 3.3 Responsibilities

| Responsibility | Owner |
|---|---|
| Opcode meaning, integer conversion, reference checks, timer behavior | Common core |
| Ready-task ordering, missed-period policy, cycle progression | Common core |
| Application state, frame bounds, process-image binding semantics | Common core |
| Source parsing, HIR, compile-time optimization, rich diagnostics and symbol lookup | Development/host tooling |
| Prepared execution plans and validated VM execution optimizations | Shared preparation/execution code; immutable per installed generation, with the same semantic contract |
| Candidate preparation, installed-generation publication, and handle retirement | Core lifetime/state-transition contracts; host orchestration and reclamation outside bounded execution |
| Control connection, start and stop, Live Values, debugging, deployment, drivers, transport workers, web/HMI servers, databases, discovery | Shared runtime components above the engine (ARCH-11, ARCH-14), the same code on every target |
| Monotonic hardware time, wait/wake, interrupt configuration | Platform adapter/runner |
| Pin choices, external ADC/DAC wiring, polarity, memory reservations | Board support/configuration |
| RETAIN interpretation and restart semantics | Common core |
| Durable storage access and power-loss recovery mechanics | Storage adapter/service |
| Fault decision | Common policy |
| Actual fault-output actuation and reset/watchdog mechanism | Platform/board |

Application-level libraries—alarm managers, motor/valve blocks, plant sequences, and equipment models—shall remain PLC/application libraries, not mandatory runtime components. Only portable primitives genuinely required by VM execution or admitted library imports belong in the runtime dependency set. Optional implementations shall be linked only when needed.

The architecture layer supplies processor-level mechanisms, the MCU HAL handles peripherals, and the BSP describes the actual board. “Cortex-M4” alone does not describe an STM32 timer, and “STM32F401” alone does not describe a controller's external I/O wiring. H7-specific memory/cache mechanisms shall not become assumptions of the shared engine or F401 adapter.

Use an existing MCU HAL and compatible `embedded-hal` drivers where suitable. `embedded-hal` is a peripheral interface foundation, not a complete PLC platform API; its standard traits do not replace process-image, restart, admission, or fault-policy contracts. [E02]

### 3.4 Artifact, prepared module, and execution state

Three distinct ownership concepts shall be explicit. They may be implemented within existing crates; this is not a requirement for three new libraries.

```text
Portable deployment artifact
  program.stbc + configuration/admission metadata + identities
                         |
              validate, admit, prepare
                         |
Immutable prepared module / execution plan
  instructions, constants, resolved IDs, type/member/import tables
                         |
           bind to prepared instance storage
                         |
Mutable execution state
  PLC globals, FB/class instances, frames, locals, temporaries,
  task state, process images, bounded diagnostics
```

An installed execution generation binds a validated prepared module, its configuration/contracts, and the compatible instance state. Immutability applies to the module within that generation; it does not prohibit a supported hosted generation replacement at a defined boundary. Section 11.3 governs replacement and retirement.

**MODEL-01.** Deployment bytes, prepared executable metadata, and mutable state shall have separate ownership and lifetime contracts. STBC compatibility shall not depend on the internal executor representation, CPU pointer width, or native Rust struct layout. An internal lowering or specialization shall preserve the selected observable semantic/numeric contract and the common conformance corpus.

**MODEL-02.** All execution plans, local-initialization plans, import bindings, and execution-required caches reachable under a bounded profile shall be ready before RUN, including rarely called and indirect targets. First-use compilation/lowering, allocation-producing cache fill or replacement, and lazy metadata construction shall not occur in a bounded control path. Optional plans shall not force every MCU to retain their memory cost.

**MODEL-03.** The resource contract shall not require MCU RAM to retain two complete instruction representations. Where an implementation retains multiple forms, their purpose, region placement, and peaks shall be explicit and fit admission. Unneeded preparation data shall be releasable before RUN. Executable method code should be shared across FB instances rather than copied per instance merely to avoid indirection.

**MODEL-04.** The storage interface shall permit owned host backing storage and borrowed read-only storage with an enforced lifetime. Memory-mapped flash is an option only where the board's access, alignment, availability, and erase/write interference satisfy the profile; it is not a mandatory zero-copy design. Checked decoding and validated access shall not be bypassed to obtain a smaller footprint, and the core unsafe-code boundary shall remain intact.

The reviewed VM owns bytecode and decoded lookup tables and exposes internal execution-plan state. The design above separates these concerns without asserting their measured cost. Other portable runtimes demonstrate separation of deployment and internal instruction representations; those designs are rationale, not truST benchmark results. [R07, D02, E09, E17]

## 4. Execution profiles and build boundaries

### 4.1 Profiles

| Profile | Purpose | Memory policy | Support status |
|---|---|---|---|
| Hosted compatibility | Migrate existing Linux/Windows/macOS products without silently changing behavior | Existing behavior recorded; unresolved allocation or blocking paths explicitly tracked | Transitional only; not evidence of bounded-profile compliance |
| Hosted bounded | Shared-core controller on Linux, Windows, or macOS | Host-provided, application-sized storage prepared before RUN; no general-purpose allocator activity in control cycles, including fault paths | Qualified separately per supported OS/architecture; not automatically a hard-real-time claim |
| Hosted development/simulation | Debugging, simulation, testing, and development tooling around the same engine | Tooling may allocate outside the core control path; simulation can enforce the selected controller profile's resource limits | Existing supported workflows preserved and regression-tested |
| MCU bring-up | Establish compiler-free execution on one board | Explicit bounded heap/arena may be used during preparation; scan allocations must be measured | Engineering milestone only |
| MCU bounded | First supported MCU runtime profile | Pre-admitted storage; no general-purpose allocator activity in control cycles, including fault paths | Released only after all acceptance gates |
| Static-only | Later smaller-memory variant of the same engine | No general-purpose heap at loading or execution | Optional later milestone, not required to prove the first board |

An `alloc`-using build can still have heap-free scans. Conversely, using a `no_std` crate does not remove heap use. The strict static-only profile shall not be advertised until initialization and loading also satisfy that policy. Preparation and timed execution remain separate phases. [E01, E10]

The table describes resource/service profiles, not separate VMs and not alternative scheduling algorithms. A deployment shall additionally identify its scheduling/I/O profile, numeric contract, and timing-evidence classification. “Hosted bounded” or “MCU bounded” establishes neither preemption nor an analytical deadline guarantee by itself.

A feature supported today shall not disappear from the hosted product simply because it has not yet qualified for the bounded controller profile. Such a gap shall be tracked explicitly and resolved, or handled through a separately approved compatibility decision, before claiming full hosted migration.

### 4.2 Dependency requirements

**BUILD-01.** MCU builds shall select `trust-runtime-core` without its hosted defaults or `hir` feature. The portable loader, executable model, standard-library subset, and executor shall not need `trust-hir` to initialize or execute an admitted application.

**BUILD-02.** The dependency audit shall include inherited defaults and target feature unification. Disabling a package's defaults does not automatically disable defaults of its dependencies. MCU builds shall use their own Cargo invocation rather than a combined hosted workspace build as the portability proof. [E03]

**BUILD-03.** Scope A retains Rust 1.95.0, edition 2021, resolver 2, and existing dependency versions except required direct portable edges. Disable default features on the shared `indexmap`, `rustc-hash`, `smol_str`, and `thiserror` declarations; audit every consumer and forward required std features explicitly. This includes source changes: no_std IndexMap has no default hasher, and `rustc_hash::FxHashMap/FxHashSet` are std-only aliases. Introduce a core ordered-map alias/default builder with explicit `FxBuildHasher` for no_std, and use `Default`/`with_capacity_and_hasher` rather than std-only constructors. Migrate value/retain/constant materialization and applicable tests. Preserve insertion/replacement/serialization order; hashing is not the iteration contract. [R35]

For existing std-exposed value/retain map APIs, retain the current std default-builder type through the alias's std definition, so changing internal spelling does not silently break public concrete return/parameter types. The no_std definition supplies `FxBuildHasher`; both invoke the same value/retain implementation. Internal validator sets/maps use explicit portable containers. Preserve stable acceptance/errors, and bound preparation work and counts rather than treating a fast unkeyed hasher as protection against adversarial input. No new hashbrown dependency is required solely for the existing ordered maps.

Relocation initially added core's direct `crc32fast` edge with explicit std opt-ins. B-R1 makes that edge optional under core's std feature; no_std builds use the shared compact IEEE CRC-32 implementation with the same wire result. Add the direct libm edge selected by NUM-05 at its existing lockfile version. Account for alloc imports/formatting in bounded preparation. These portability changes are part of A/M1; Rust/MSRV/edition/Node and broad package upgrades are exclusively M0U. Isolated F401/C6 checks, not a host no-default check, establish the graph. [R03, R25, R36, R37, E45]

**BUILD-04.** Bare-metal checks shall be followed by a fully linked firmware build. The firmware shall provide startup, memory layout, interrupt vectors, panic behavior, and allocator support when applicable. A library `cargo check` is not a complete firmware test.

**BUILD-05.** Assembly shall remain limited to established architecture support or a reviewed, measured optimization. There shall be no processor-specific fork of the bytecode semantics.

**BUILD-06.** Hosted applications may continue using `std`, operating-system threads, filesystems, and networking outside the portable execution boundary. The MCU portability requirement shall not force the entire desktop product to become `no_std`. [E01]

**BUILD-07.** The hosted runtime-only build shall load and execute admitted artifacts without compiler/HIR/IDE dependencies or unselected product services. The full product may retain compilation as an optional authoring service. Core-only and runtime-only feature graphs shall be checked in isolated builds; a full development build shall not conceal accidental dependencies.

**BUILD-08.** Linux, Windows, and macOS shall have native test jobs for the declared supported architectures. Cross-compilation is supplementary evidence, not a substitute for executing the relevant tests on each OS.

**BUILD-09.** M1 shall check separate `thumbv7em-none-eabihf` and `riscv32imac-unknown-none-elf` core graphs for compiler/HIR/host-service leakage, dependency defaults, and synchronization assumptions. Add no new atomic read-modify-write dependency to shared execution. Existing `Arc`/`SmolStr` storage may remain temporarily on these atomic-capable bring-up targets, with allocation/refcount/drop costs recorded; MEM-06/10/11 retires it from bounded value/compound paths before M3 qualification. Optional C3/IMC checks become mandatory only if that additional profile is selected, after this ownership work. Neither Rust 1.99 nor `smol_str` 0.3.6 removes the IMC issue. Do not fabricate target cfgs or introduce an interim replacement-Arc layer solely to claim C3 support. [R09, R24, E30, E31, E46, E56]

For the F401 Cortex-M4F bring-up, `thumbv7em-none-eabihf` is the selected target family. CPU/FPU flags and BSP startup shall match the part's single-precision FPU. The ABI does not provide hardware double-precision arithmetic on this part. Appendix D records the initial HAL/runner direction and decisions to freeze. [E04, E25, E28]

### 4.3 Hosted product requirements

**HOST-01.** Linux, Windows, and macOS shall use the shared loader, execution state, opcode semantics, cycle policy, and fault identities. Host `Runtime` compatibility APIs shall compose or wrap that engine, rather than retain an independent legacy dispatcher as the permanent desktop implementation.

**HOST-02.** Hosted limits shall be selected for the application and deployment, not copied from STM32 defaults. Hosts may allocate backing storage during preparation and offer larger programs, strings, arrays, instance counts, and task configurations while retaining checked execution and admission. The hosted product selects them for each deployment from the deployment record next to `program.stbc` (§5.1.1): it admits the record against the computer's descriptor and prepares the program with the record's limits, and a project without a record keeps the hosted defaults. Instance identities are 32-bit: the allocator hands out 0 through `u32::MAX - 1` and never `u32::MAX`, and creating an instance beyond that fails with `Overflow` and never wraps an identity or panics, in hosted and portable construction alike. A live instance's identity is never issued again. When a top-level execution entry (§4.3.1) begins, the identities issued since the previous entry began are issued again if none of their instances is still live, as is the case for function-block and class instances declared in a function or method. If any of them is still live, persistent instances included, none of those identities is issued again until a restart or replacement constructs new storage.

**HOST-03.** Existing supported debugger, HMI, protocol, persistence, configuration, CLI, and embedding workflows shall remain available on their supported platforms. Preserve existing APIs except for the explicitly selected H raw-mutation migration in §4.3.1: unrestricted mutable storage and executable type-registry access shall be available only during non-executable construction; installed-state integrations shall use checked writes or candidate replacement. Document replacement APIs and migrate all repository callers; do not silently claim source compatibility for the old live mutable-reference signatures. Hosted online change shall follow the generation, restart, and retirement contract in Section 11.3. The M0 feature matrix shall distinguish implemented support, unavailable platform-specific features, and experimental support; it shall not infer identical service/driver support on every OS.

**HOST-04.** Validated execution optimizations shall remain available where supported and shall use the same semantic and numeric conformance corpus. Their plans shall be prepared from admitted artifacts without requiring an ST compiler session. Bounded hosted profiles shall satisfy MODEL-02 for first use, rare branches, and cache replacement; MCU limits shall not force hosted builds onto a slower path without measured evidence and an approved tradeoff. Register execution scratch shall be reserved during controller construction for each eligible prepared POU and counted against the construction allocation limits. Scratch cleanup shall preserve success, fault and failed-selection semantics. Default values keep their type, depth and lifetime checks whichever constructor path produces them. Calls to a distinct eligible POU shall retain register execution; recursive re-entry into an occupied POU may use the shared stack fallback without resetting the instruction budget. Explicit tier-1 reset clears selected blocks and counters while retaining immutable plans and reserved storage; profile-only reset preserves the selected blocks. Immutable preparation diagnostics retain lowering and specialized-compilation counts and their specific failure reasons after RUN profiling resets. CLI and embedding reports distinguish those preparation results from RUN hits and zero RUN compilation counters. A debugger attached or detached while the resource runs takes effect at the next activation start: an activation already running finishes on the executor it started on. A Tier-1 block's hotness counts every entry over the block's lifetime; evicting a selected block from the bounded cache does not reset that count, so the evicted block is selected again at its next entry. The register profile counts interpreted block entries separately from Tier-1 block executions. Tier-1 has no deoptimization path (a guard miss continues inside the block with generic arithmetic), so it reports no deoptimization counters. Register lowering covers the opcodes the source encoder emits for supported statements: the `HALT` of the FOR zero-step guard (specification 12 §7.3) lowers to a register trap that raises `ForStepZero` exactly as the shared stack path does, so a POU with a FOR loop keeps register execution; Tier-1 may leave that trap's block to the register interpreter.

H implementation choices that are not product behavior stay out of this
specification. The following guarantees bind them:

- Calls. Compositions without register execution may run native stack-to-stack
  calls inside the shared dispatcher's iterative frame loop. Register-enabled
  compositions keep the mixed stack/register activation handler. Either way,
  opcode semantics, call binding, initialization, copy-back, fault cleanup,
  call-depth limits and the shared budget stay identical.
- Work done in place. A register call to a POU whose prepared plan makes no user
  call (a leaf) may complete inside the calling instruction instead of yielding to
  the activation handler; calls that can nest further, calls while debugging and
  callees whose scratch is in use keep the handler. A frame default without an
  executable body that is either a literal holding no reference or nested storage,
  or the zero value of a non-string scalar, may be written straight into the new
  frame. Both keep the general path's results, faults and charges, in the same
  order. A leaf call reserves no handler storage, so that storage is neither
  charged nor a point of refusal; prepared plans and handler records are charged
  at their actual sizes.
- Value transfer. An invocation-owned argument, a completed function output or a
  prepared local payload may move into its destination instead of being copied.
  Moving never changes observable binding: named and positional binding,
  explicit NULL presence, specification 10 §7.5's rejection of overlapping
  writable connections, output aliases and first-fault order are unchanged.
  Values read from storage, borrowed arguments and default constants are copied.
- Output publication. All outputs of one call publish atomically; a later
  failure restores every earlier destination of that call. A failed candidate
  leaves the installed generation untouched.
- Lifecycle markers. Initialized-declaration and once markers keep their
  `(declaration, optional instance identity)` keys and ascending iteration
  order. `None` differs from `Some(InstanceId(0))`, and every `u32` instance
  identity, including `u32::MAX`, is representable. Storage is reserved before
  publication, so a refused insertion leaves markers unchanged. Frame-slot
  initialization publishes no persistent marker.
- Prepared indexes and caches. Frame phase plans, declaration-owner ranges,
  root-owner and sparse POU indexes may be prepared once and reused. Their size
  follows admitted records, never the largest wire identity, and their
  preparation counts against the preparation limits. A cache hit verifies the
  complete identity. Reusing them never waives constant, visibility, staging,
  type or lifetime checks or changes refusal order.
- Typed construction keeps exact `(type, operation)` cycle detection and the
  structural nesting bound, and borrows admitted TYPE_TABLE metadata rather than
  copying it per visit.
- Activations. The execution-frame registry is the single authority for
  reserved identity, activation membership, optional POU/instance owner and
  suspended local payload. Identities increase within one top-level execution
  entry: one task program or function-block invocation, or one engineering
  operation. A retired identity is not issued again within its entry, so identity
  order is ancestry order among live activations, and exhausting one entry's
  identities fails with `Overflow` instead of wrapping. After construction, which
  keeps one increasing sequence, each entry begins with nothing reserved and
  starts again at identity 0. The lifetime checks confine every reference to an
  activation, and every instance it owns, to its entry, so a reissued identity
  cannot name an earlier entry's state, and every checked operation refuses a
  reference into a retired activation. Retirement
  removes liveness, owner and payload together and never abandons owned-instance
  cleanup on a work or deadline failure. Debugger stack views list owned, active,
  suspended ancestors in activation order, followed by the current frame. Their
  frame identities hold for the pause that reported them, as DAP frame
  references do; a later entry may report the same numbers for other activations.
- Edge-qualified inputs bind by admitted POU identity; compatibility callers
  that do not supply it keep name-based resolution.

Their work is bookkeeping under TIME-08 and is not charged, so none of them
changes a program's charged work.

**Optimized-tier decision.** The existing register-IR/tier-1 execution remains an optional internal execution plan of the shared engine, moved during M2B. Its lowering and caches are prepared before bounded RUN; profiling and OS timing remain host adapters. The initial MCU profile omits the register representation and its storage, while hosted products retain the enabled optimized path and its semantic helpers. Scope A may leave the old hosted tier in place temporarily; that is not completed HOST-01 migration. No permanent independent hosted dispatcher is exempted. Compare against the actual enabled hosted tier, using the relevant ARCH-VM-19/20/27/28/31 benchmark workloads and fallback criteria as historical regression references, not newly passing evidence. [R32]

H shall report lowering and specialized-compilation attempts/results as candidate
preparation observations, separately from RUN plan hits, opcode counters and
precomputed fallback reasons. Register profiling resets clear instrumentation only,
without changing tier selection. The existing explicit tier-1 cache reset clears
active selections, LRU state, hit counts and counters, so inactive blocks must meet
the configured hot threshold again. Both resets retain immutable prepared code and
reserved observation records; neither permits first-use rebuilding. Lowering-cache
reset also retains prepared plans: published counters shall distinguish prepared
entries from the old lazily populated cache rather than claiming that legacy cache
occupancy semantics were preserved. Existing hosted cache-capacity/threshold controls may
select an explicitly prepared optimization policy; they shall not authorize lazy
lowering or threshold-triggered compilation in bounded RUN. Preserve the PLC
behavior assertions and publish the changed timing/meaning of these diagnostics;
a preparation count shall not be presented as a first-use execution measurement.
Preserve original bytecode instruction costs across stack/register/specialized
execution, including fixed-budget failure boundaries. Constant-time dispatch and
optimization-selection bookkeeping is included in that instruction accounting;
selection shall use prepared fixed indices rather than scan/shift work that grows
with the program or configured cache. TIME-08 defines the charged work. Do not add
a parallel instruction/work budget merely to hide an optimization's unbounded
administrative cost.

**HOST-05.** Production control execution shall use prepared state and bounded interfaces on hosted targets as well as MCUs. Network, HMI, logging, compilation, and storage services shall not impose unbounded waits on the control thread. Moving an existing blocking path shall preserve behavior or have a separately approved semantic change and regression tests.

**HOST-06.** The simulator shall execute through the same core APIs, with injected logical time and input traces. It shall support the selected controller's capability and resource profile and explicitly report unsupported application requirements. Accelerated simulation shall not bypass required arithmetic, reference, bounds, or instruction-budget checks.

**HOST-07.** The implementation shall support a small set of documented hosted compositions: runtime-only; full controller with selected services; and development/simulation. They shall not become separate VM products. Dependencies and services omitted from runtime-only shall remain absent from its build graph, rather than merely disabled at startup.

These compositions require M2B feature/ownership changes: current `Runtime` embeds web/HMI/control/debug and other host state, and protocol feature switches alone do not produce a compiler-free runtime-only product. Scope A's headless core consumer is a narrower deliverable, not proof that HOST-07 is already implemented. [R06, R34]

**HOST-08.** Hosted migration shall have regression and performance gates of its own and shall not be declared complete solely because an STM32 demonstration succeeds. A target whose tests have not run shall remain unqualified for the new profile.

### 4.3.1 Construction and installed-state mutation

**HOST-09.** H/M2B shall enforce the following lifecycle in the shared core and
hosted embedding surface. This is a truST product API decision, not an IEC
deviation. It preserves the full PLC function set on every platform.

| Phase or operation | Required behavior |
|---|---|
| Non-executable authoring/construction | Raw storage and authoring-registry edits may be exposed here. The object cannot execute. Installation transfers ownership only after LOAD-07/09 admission and construction checks; no mutable authoring borrow may remain against the installed state. |
| Installed controller, including RUN, debugger pause and STOP | Declared storage layout/schema, executable types and code remain admitted and immutable for that installation. Existing supported value and instance operations remain permitted under admitted capacities and lifetime rules. No public API or adapter-trait call may return unrestricted mutable storage or installed TYPE_TABLE access. STOP or a debugger pause alone does not turn installed state into an authoring object. Ordinary PLC value updates remain permitted through checked operations. |
| Live value writes and forces | Use the single engine owner and existing cycle/debugger write boundaries. Preserve authorization, one-time write versus persistent-force behavior, and existing input/output image timing under CYCLE-05/06. Ordinary supported writes shall not require STOP, restart or re-admission. Synchronous owner-local calls may commit at an existing permitted boundary; remote requests use bounded staging/queues where the profile requires them. |
| Code, declarations, types or storage layout changes | Build and validate an isolated candidate outside bounded RUN. Replace only through §11.3's hosted boundary and warm-restart/retain policy, or §11.2's stopped MCU installation. A failed candidate leaves the installed state executable and unchanged. No implicit first-use preparation or recompilation may repair a live mutation. |
| Resource-only task, image extent or direct-I/O binding reconfiguration | Admit a candidate with unchanged executable code, constants, reference identities, types, declarations, construction/default recipes and access policy. Stage scheduler metadata, reset task readiness to the current logical time and sample each SINGLE input as its initial event baseline when replacing task/resource metadata (binding-only edits preserve readiness), and resize flat images with intersection preservation and zero-filled growth. Commit only after binding and safe-output validation. Keep the same PLC storage and installation generation (§11.3), so every reference stays valid, the same instance IDs, non-retained and retained values, hierarchical images, fault, logical time, cycle counters, register observations, stable debugger thread identities and pending debugger requests. Do not rerun initializers, replace the PLC graph, or clone the full store. Failure preserves the installed resource and images. This is the existing hosted resource-update behavior, distinct from code/schema replacement. |
| Debug expression parsing | Use separate authoring/scratch types and immutable installed metadata for lookup. Parsing an expression shall not extend or alter the installed executable type table. Any resulting write goes through the same checked live-write path. Compiler-dependent expression authoring remains in the selected development service, not the runtime-only graph. A runtime evaluates the compiled form of specification 11 §6.13.2 and parses no expression text, so this row's parsing happens only in the tool. |

Retained checkpoint capture is also a checked owner operation. The hosted
`Runtime::retain_snapshot` and `RetainSnapshot::from_runtime` APIs therefore take
an exclusive owner borrow and return a fallible result. Callers shall propagate
allocation/work/deadline failures; an empty checkpoint is never an error fallback.
This explicitly replaces their former infallible shared-reference signatures.

Hosted read-only `VAR_ACCESS` rejection preserves the shared `RuntimeInvalidAlias`
stable identity and adds the contextual message `VAR_ACCESS '<name>' is read-only`.
The hosted adapter may add this context only after the shared checked write rejects
an immutable admitted read-only alias. Unknown aliases and other faults retain their
original identity; rejected writes do not mutate PLC state or latch a scan fault.
This is product diagnostic compatibility, not an IEC deviation.

Checked writes shall validate the destination's admitted type, range, mutability,
applicable caller access policy, reference lifetime and current binding before
committing. Preserve existing visibility/authorization rules, including supported
privileged debugger access; ST lexical visibility shall not automatically replace
the engineering caller's access policy. Preserve
existing conversion and force rules; adding this boundary does not authorize new
coercions or public writes to previously inaccessible members. A rejected request
shall leave its destination, process images, installed identity and task readiness
unchanged by that request. An independent scan may still advance normally.
Validate before commit, including the existing engineering-write deadline rule;
do not report a fallible post-commit check as a rejected write. Rejection of an
invalid management request is not itself a PLC execution fault. Faults originating
in PLC execution continue to follow the existing cycle/safe-state policy. Enabled watchdog policy may select its existing action without erasing the structured deterministic resource-limit cause; physical deadline expiry remains distinguishable.

Checked engineering `write_global`, `write_instance`, `write_reference` and
`write_access` operations use the finite floating-point admission policy in
specification 11: reject REAL/LREAL NaN and infinity recursively through arrays
and structures before commit, with `runtime_non_finite_value`. The policy belongs
to these management boundaries; it does not alter internal IEC arithmetic,
`IS_VALID`, signed zero or subnormal behavior. Construction and management
coercion share the applicable declared-domain rules without removing deliberate
differences in source conversion or initialization semantics. Management preflight also validates
values reconstructed through physical aliases: an integer bit write that would
introduce an aliased REAL/LREAL NaN or infinity rejects before mutation. Internal
PLC bit operations retain their specified IEEE behavior. Strict physical copies
compare floating encodings, preserving NaN payloads and signed zero, rather than
using numerical equality to certify backing consistency.


Required shared-global synchronization is part of the resource transaction.
Admit existing source/destination names, checked assignment compatibility, initial
values and write permissions before starting the resource. Compatible declared
domains need not be identical; each later transfer remains checked against the
destination range. A later operational synchronization failure must enter the same owning
fault policy as a cycle failure, including configured safe outputs. The resource
handle and runtime owner must agree on the resulting fault state.

Cycle-service ports retain structured rejection causes. Destination/type/access/
lifetime rejection of an engineering request remains point-local under the
existing protocol policy; exhaustion of the enclosing cycle's work, copy,
allocation or physical-deadline allowance propagates through cycle fault handling.
Do not discard those operational errors as a Boolean result. Resource-limit
diagnostics retain `runtime_execution_timeout` compatibility while identifying
the bounded cause and used/requested/limit quantities; physical deadline expiry
is distinguishable from deterministic resource exhaustion.

Queue acceptance is distinct from application of a write. A staged request shall
be checked against the active destination and binding again at its commit
boundary; a request targeting a replaced or retired destination shall be rejected
unless an existing supported checked rebinding operation applies. A request that
names a destination by name is bound to that name at its boundary. A request that
names an instance by its identity keeps the installation generation (§11.3) in
which that identity was resolved, and its boundary rejects it with
`ReferenceLifetime` when another generation is installed, so a restart or
replacement in between cannot redirect it to the instance that now has that
identity. Queue exhaustion
shall return an explicit failure without applying the request; the hosted control
surface bounds its pending writes and forces at 1,024 entries each and refuses
further requests with `request queue full` (specification 11, runtime control I/O
and variable mutation contract). Preserve existing
protocol acknowledgements while distinguishing accepted from applied wherever the
protocol exposes completion. References to debug scratch storage shall not escape
into PLC state; installed references returned through scratch parsing retain their
existing checked lifetime rules. Scratch-only type IDs shall not become installed
identities; checked writes resolve their values against admitted destination types.

The portable core owners (`OwnedRuntimeState`, `RuntimeState`) are the embedding
API for drivers, firmware and hosts, and they change state only between cycles,
through an exclusive borrow. Their raw input image is the I/O driver boundary: a
driver writes sampled input bytes there before the next cycle, as the scan's own
input sampling does, and no other storage is reachable as raw bytes. Variables
change only through checked writes. Setting the logical time (HOST-06 simulation),
latching a host-detected fault, clearing a fault and resetting task timing after a
restart are host responsibilities exposed for that use. The installed hosted
controller keeps task-timing resets inside restart and replacement. Debugger
evaluation contexts hold PLC storage privately, and the installed controller does
not implement the execution-adapter traits, so neither offers a path around
checked mutation.

Expose non-executable construction and admitted execution through distinct Rust
types or facades with private installed state. Prefer ordinary ownership and
checked methods over a new framework. Construction-only compatibility methods
may retain familiar names; their old availability on a live `Runtime` is the
explicit source-compatibility change. Document migration examples for globals,
members, aliases and process-image writes, and for schema replacement. The hosted
migration guide is `docs/guides/RUNTIME_EMBEDDING_MIGRATION.md`. Do not
retain a mutable bypass through internal adapter traits, shadow storage, unsafe
self-references, leaked allocations, or panic-based getter restrictions. Existing
host adapter traits may serve implementation/construction needs but shall not
expose raw mutation of the public installed controller. [E49, E68]

H acceptance shall include compile-fail assertions denying raw mutable installed
access in each hosted composition and native regressions for checked live writes
without reinitialization; rejected constant/type/range/access/lifetime/stale
binding writes without side effects; debugger scratch isolation; existing
mesh/service/debug write and force timing; and unsuccessful candidate replacement
preserving the old state. Exercise both supplied legacy 1.1 instruction bodies
with explicit construction context and source-free 2.0 loading. No production
caller may retain a second mutable storage authority. Preparation and bounded
write paths shall satisfy their profile capacities and MODEL-02/MEM-04.

Hosted instruction observers shall project the current invocation and live
suspended callers from read-only shared-engine state, preserving their actual
activation identities and local-slot ordering. Named debugger rows and expression
evaluation belong to detached development snapshots; they shall not insert
presentation frames into PLC storage. Compiler scratch slots remain resolvable
by their physical offsets but are omitted from user-facing variable lists.
Conditions and watches specified by spec 13 shall receive that detached evaluation
context. This corrects the previous hosted VM observer's absent evaluation
context; it is not permission to mutate installed state during a pause. An
internal observer/projection failure shall propagate through normal dispatcher
cleanup and cycle fault handling rather than disappear as a successful
observation. Installation and isolated restart construction shall not publish
candidate observations into the active debugger. Native acceptance shall cover
the actual nested call stack, ancestor retirement, context-dependent watches and
conditions, detached edits and fault cleanup, alongside old hook delegation.

A successful source/schema reload invalidates compiled watch expressions and
their scratch registries together. The adapter clears its compiled-expression
cache at the same successful boundary; subsequent normal evaluation rebuilds
against current metadata. Retired frame locations, observation snapshots, step/stop
state and queued mutations are invalidated at this successful schema boundary;
DAP variable handles cannot be rebound to a different installation. UI watch
definitions remain available. Failed reload
preserves the installed watch set and cache. Detached observation projects one
owned snapshot and transfers or shares that completed immutable projection;
statement refresh must not immediately clone the complete projection again.

Hosted adapters shall borrow the shared engine's authoritative process images;
construction-time image buffers shall not become a second live authority.
Inspection snapshots may clone their selected values outside RUN, while cycle
callbacks use the enclosing cycle work/deadline allowance. Normal driver failure
stops in driver order; the established safe-state policy still attempts every
driver and reports the first failure. An expired execution deadline shall not
prevent that separate safe-output owner operation, and the owning PLC fault
shall remain latched. Failed queued/forced management writes shall report a
`write_rejected` runtime event with destination, error and cycle time rather than
claim application or fault the PLC solely for an invalid request; physical
deadline expiry still faults execution. Retain saving remains before output
sampling. Telemetry and its final hosted output-deadline check remain after the
old flat-Q publication rollback boundary; a late telemetry failure is not proof
that physical outputs were undone.
The common cycle shall distinguish provisional output preparation from physical
driver submission. A preparation/deadline failure before submission, or a driver
error during submission, restores the touched flat-Q windows as before. Once all
drivers return success, those windows are committed; a subsequent deadline or
observation failure still faults the resource but shall not restore that already
submitted logical image. This preserves the hosted publication boundary without
claiming rollback of physical effects, including effects of earlier drivers when
a later driver fails.

This contract is selected for H before changing ownership. It does not pull the
additional M3/M4H reader-retirement, generation-wrap or capacity mechanisms in
UPDATE-03 through UPDATE-08 into H. H must use valid current bindings and preserve
existing stale-reference checks without inventing a speculative reader service.

Hosted bytecode export shall copy the installation's immutable deployment
artifact, not rebuild startup declarations from mutable PLC values or execute
initializers again. Retained/current values remain separate checkpoint data.
The existing plain export omits optional source-debug sections; source-aware
export preserves the admitted instruction-to-source map and may relabel source
paths. Supplied source text shall agree with an available, original installation
source snapshot before attaching its debug map. A source-free installation shall
not certify unrelated supplied text from a `file_N` label alone; unverifiable
source association is rejected. A source-aware export also rejects an installation
without existing nonempty instruction/source mappings rather than silently returning
a plain artifact. Execution tables and construction
records shall be unchanged by these presentation-only exports.
Rust's construction/borrowing guidance supports the separation; vendor write and
online-change documentation illustrates why mutation and rebinding need defined
boundaries. Neither source defines truST's cycle order or changes its warm-restart
contract. [E49, E68–E70]

#### Uniform temporal storage ABI

The owner authorized the H review correction to preserve the established
64-bit temporal value domain and every platform's full function set. New
applications use STBC 3.0: eight-byte temporal storage, consistent `SIZEOF`,
array strides, relative extents and construction demand. This is an explicit
incompatible artifact ABI change under specification 12 §4.1, not a reinterpretation
of 2.0. Readers preserve legacy 1.1/2.0 widths and reject unknown majors before
state publication. The 3.0 checksum covers the header, so a relabelled version or
changed flag fails instead of selecting other widths (specification 12 §4.1). No temporal function or MCU capability is removed. Existing
explicit I/O/protocol codecs remain checked conversions with their existing widths.
Acceptance includes old-artifact identity/size preservation, rejection by old
readers, negative and greater-than-32-bit temporal round trips, nested relative
array extents, and atomic refusal at legacy physical boundaries. Retain/update
must compare the selected ABI before carrying physical layout forward.

### 4.4 Scheduling/I/O profile and timing claims

The existing nonpreemptive, single-owner resource scan shall be named **`cooperative-resource-v1`** in the deployment contract. This is a proposed profile identifier, not an assertion that the baseline already emits it. It preserves latched inputs, the established ready-task ordering, admitted background work, and one normal output-publication phase after required ready work. PLC-task preemption and per-task physical output commits are not introduced by the MCU port.

**SCHED-01.** The engine shall implement the selected versioned scheduling/I/O profile identically on admitted targets. Migration under `cooperative-resource-v1` shall preserve priority, equal-priority/due ordering, sampled event edges, shared-state access, and process-image behavior. Periodic scheduling shall preserve nominal deadlines under SCHED-05 and specifications 10 §4.3 / 11 §6.2: late samples shall not shift later deadlines, and missed activations remain dropped, counted, and never replayed. The current sampled-time baseline is a defect to correct, not the profile's compatibility oracle. OS/interrupt preemption of the engine thread shall not be confused with PLC-task preemption inside the engine.

**SCHED-02.** Timing admission shall account for release/wakeup jitter, input acquisition and freshness requirements, admitted execution costs, blocking by already-running work, earlier ready work, admitted background work, synchronous imports, bookkeeping, and output completion. Average utilization and instruction counts alone shall not establish schedulability.

**SCHED-03.** Each task or output group with a timing requirement shall distinguish computation completion from required output publication/transfer/feedback completion. Admission shall consider the whole ready batch and the required output evidence, not just the duration of the task that computed an output.

**SCHED-04.** A configuration known to violate a required deadline under the selected profile shall be rejected for that timing claim. A configuration lacking adequate evidence may run only in an explicitly non-guaranteed compatibility/development mode when product policy allows it; this shall not waive memory/type validation, fault handling, or supervision. An application requiring stronger timing evidence shall not be silently downgraded.

Timing evidence shall be classified separately from the resource profile:

| Classification | Permitted claim |
|---|---|
| Unqualified | Functional or bring-up execution only; no supported deadline claim. |
| Measured-qualified | Specified workloads met their deadlines under a recorded platform/load envelope. Observed maxima are not proven upper bounds. |
| Analytically supported | A documented scheduling argument uses justified bounds and stated environmental assumptions. Measured costs with an arbitrary margin alone do not qualify as proof. |

These are proposed descriptor labels. The last classification is not a functional-safety certification or a universal OS guarantee. Section 8.4 defines the admission record and Section 14 defines evidence gates. Qualification applies only to the bound application/configuration and platform envelope.

A future preemptive or independently committed multi-rate profile would need separate state-ownership, I/O-writer, consistency, synchronization, and compatibility rules. It shall not be introduced incidentally through a HAL or RTOS selection. Other PLC runtimes make different scheduling and bus-cycle choices; they do not define truST's compatibility contract. [D02, E08, E11, E12]

## 5. Program, bytecode, and admission contract

### 5.1 Application artifacts

Continue using `program.stbc`. The deployment shall also supply a compact, versioned configuration/admission record generated from the existing project configuration on the development computer.

This record is a new proposed deployment contract, not an assertion that the current repository already emits it. It may be a sidecar or a compatible STBC extension following the existing versioning rules. Do not change opcode meanings to implement it.

The record shall identify the bytecode and configuration hashes; required instruction/semantic features; required native imports; type and library contract versions; scheduling/I/O and numeric contracts; task configuration; computation and output-response requirements; process-image sizes and bindings; requested storage limits; retain-schema identity; and required timing-evidence classification. Target-specific qualification records shall be bound to this identity and the actual deployment envelope, without inserting board pin numbers or native addresses into STBC.

Hardware pin mappings belong to board configuration, not the STBC instruction set. The same STBC bytes can be used on different devices only when both devices support the required features and the application's logical I/O contract. A board-specific firmware image or different peripheral mapping does not invalidate that bytecode portability.

**The installed application container.** A device with an `application` region keeps the installed program and its record in that region as one container, so a chip reads one bounded artifact at boot. Version 2 has the magic `TRSTA002` and carries an install sequence, which orders the two slots of the program store (below). Version 1, with the magic `TRSTA001`, is the same container without the sequence; a chip still reads it until its firmware switches to the program store (S4-6).

| Offset | Bytes | Field |
|---|---|---|
| 0 | 8 | Magic `TRSTA002`: its first four bytes are written before the read-back, its last four bytes last, at the commit point |
| 8 | 4 | CRC-32 of bytes 12 to the end of the entry table |
| 12 | 4 | The total length L of the container in bytes |
| 16 | 8 | The install sequence: 1 for the first commit, then one more than the newest committed sequence at each commit; the device writes it, and the transfer's value is replaced |
| 24 | 4 | The entry count, 2 |
| 28 | 16 each | The entries: for each, its kind (4: 1 `program.stbc`, 2 the deployment record), the payload's offset (4) from the start of the container, its length (4) and its payload's CRC-32 (4) |
| 60 | L − 60 | The payloads: `program.stbc`, then the record |

Version 1 has the same fields without the sequence: its entry count is at offset 16, its entries start at 20 and its payloads at 52. Integers are little-endian and CRC-32 is CRC-32/ISO-HDLC as in STBC. A reader (`trust-runtime-services::installation`, shared and without an operating system) exposes no payload before it has checked, in this order, the magic; the header CRC; the total length against the region; the entry count; that each kind appears once; that each entry's range lies inside the total length, outside the header and overlaps no other entry; and each payload's CRC. A region whose first eight bytes are neither magic, an erased region of `0xFF` in particular, holds no installed program.

A writer writes the container in one order: the payloads, then the header fields and the entry table, and the magic last. A write that stops at any byte then reads as no installed program (DEPLOY-03), because the magic is absent until the container is whole. On the bench, `cargo xtask portability pack-project <program.stbc> <deployment-record.bin> <app.bin>` writes a version-1 container, after checking that the record reads and describes exactly the program it packs.

**The program store.** A chip's program store holds two containers, one in each of two application slots of equal size, which the chip file names. An install writes the complete container to the slot that does not hold the newest committed container, or to slot A when neither slot holds one. The writes run in steps of at most one erase unit, or at most one step budget of programmed or read-back bytes, so that a loop pass serves the link between them (STATE-08). The order is: erase the units the container needs, starting with the unit that holds the magic; program the payloads, then the header fields after the magic, then the first four bytes of the magic; read back every byte that was programmed; and program the last four bytes of the magic. That last program is the commit point: a container is committed only when its magic reads `TRSTA002` in full, and it then holds the sequence the device gave it. An install that stops at any step, or whose read-back fails, leaves the newest committed container as it was, so the previous program stays selected (DEPLOY-03). At boot the store reads the header of each slot. A slot is committed when its magic, its header CRC-32 and its total length are good; any other slot holds no committed container. An erase stopped part-way through a slot's first unit can leave the magic over a damaged header, whose sequence cannot then be read, so such a slot is not committed; a damaged payload or a refused admission under a good header is a damaged committed container. The committed container with the highest sequence is selected, and its payloads are checked and its admission is granted (§5.1.1). If either check fails, the boot is refused with `no installed program: <reason>`, and the other slot is not selected in its place. Two committed containers with the same sequence also refuse the boot, and the reason names the sequence. A rollback needs an explicit activation with its own check (DEPLOY-06); none is defined here.

### 5.1.1 The deployment record

The record of §5.1 is one binary file, `deployment-record.bin`, next to `program.stbc`: a sidecar,
so STBC is unchanged. It is the same record on every target. It describes one STBC 2.0 or 3.0
program. Integers are little-endian, CRC-32 is CRC-32/ISO-HDLC as in STBC, and SHA-256 is that of
FIPS 180-4. `trust-runtime build` writes it (specification 11 §6.8.1d), and so does every tool that
writes a program into a project: the setup wizard and the web deployment write the record of the
new program, and `trust-runtime deploy` copies it with the program.

| Offset | Bytes | Field |
|---|---|---|
| 0 | 4 | Magic `TRDR` |
| 4 | 2 | Record version, 1 |
| 6 | 2 | Section count |
| 8 | 4 | Record length L in bytes, header and CRC included, at most 65,536 |
| 12 | L − 16 | The sections, in ascending order of their identifiers |
| L − 4 | 4 | CRC-32 of bytes 0 to L − 5 |

A section is a 2-byte identifier, 2 reserved bytes (0), a 4-byte payload length and the payload.
Version 1 has these seven sections, each once:

| Section | Payload |
|---|---|
| 1 Application | The program's SHA-256 (32 bytes) and length (4); its STBC major and minor version (2 + 2) and header flags (4); the configuration identity (32); the retain-schema identity (32) |
| 2 Contracts | The scheduling/I/O profile (2): 1 is `cooperative-resource-v1` (§4.4). The numeric contract (2): 1 is the contract of §12.1. The required timing evidence (1): 0 unqualified, 1 measured-qualified, 2 analytically supported (§4.4). Reserved (1), 0. The required capabilities (2): bit 0 is the platform wall clock that CURRENT_DT reads (§5.2) |
| 3 Limits | The requested preparation limits, 8 bytes each, in this order: artifact bytes, preparation bytes, preparation work, validation scratch bytes, validation work, construction values, construction bytes, process-image bytes, work, construction work, call depth |
| 4 Process image | The input, output and marker image sizes in bytes and the number of I/O bindings (4 each) |
| 5 Tasks | The task count (2) and 2 reserved bytes, then for each task in declaration order: its interval in nanoseconds (8, signed), its priority (4) and its flags (4), of which bit 0 marks an event task (`SINGLE`) and the others are 0 |
| 6 Imports | The import count (2) and 2 reserved bytes, then each built-in import of the program once, ordered by kind and then by name: the kind (1), 1 for a function and 2 for a function block; the name's length (1); the contract version (2, §5.2.1); the canonical uppercase name |
| 7 Configuration | The settings a device without an operating system needs from `runtime.toml` and `io.toml` (below): the resource name, the cycle interval, control, the log level, retain, the watchdog, the fault policy, the event-store capacity, the task overrides, the I/O drivers and the safe state |

Section 7 carries what a device without an operating system needs from the project's `runtime.toml`
and `io.toml`: a computer reads those two files themselves at each start, and a chip never parses
them. Its payload is, in this order, every integer little-endian and every text a 1-byte length and
its UTF-8 bytes, at most 255 bytes:

| Field | Bytes |
|---|---|
| Resource name | a text; empty in a record whose build did not fill this section |
| Cycle interval | 8, unsigned, nanoseconds |
| Control: debug enabled, then mode | 1 (0 or 1), then 1 (1 `production`, 2 `debug`) |
| Log level | 1 (1 `error`, 2 `warn`, 3 `info`, 4 `debug`, 5 `trace`: the names the runtime's configuration parser accepts, where `warning` is `warn`) |
| Event-store capacity | 4, unsigned; 0 is the device's default, otherwise the newest events the store keeps (specification 11, `runtime.events.capacity`). The decoder accepts every four-byte value; admission refuses one above the device's maximum (step 7 below) |
| Retain: mode, then save interval | 1 (0 `none`, 1 `file`), then 8, nanoseconds |
| Watchdog: enabled, timeout, action, hang action | 1 (0 or 1), then 8, nanoseconds, then 1 (1 `halt`, 2 `safe_halt`, 3 `restart`), then 1 (1 `fault`, 2 `abort`) |
| Fault policy | 1 (1 `halt`, 2 `safe_halt`, 3 `restart`) |
| Task overrides: presence | 1 (0 none, 1 present) |
| The task overrides, when present: count | 2 |
| Each task override: name, interval, priority, single input, program count, then its programs | a text, never empty; 8, nanoseconds; 1; a text, empty for none; 2; then each program name, a text never empty |
| I/O drivers: count | 2 |
| Each driver: name, enabled | a text, never empty; 1 (0 or 1) |
| The `gpio` driver's inputs: count | 2 |
| Each GPIO input: address, line, invert, debounce | 7; a text, never empty; 1 (0 or 1); 4, milliseconds |
| The `gpio` driver's outputs: count | 2 |
| Each GPIO output: address, line, invert, initial | 7; a text, never empty; 1 (0 or 1); 1 (0 or 1) |
| Safe state: count | 2 |
| Each entry: address, value type, then value | 7; 1 (1 `BOOL`, 2 `SINT`, 3 `INT`, 4 `DINT`, 5 `LINT`, 6 `USINT`, 7 `UINT`, 8 `UDINT`, 9 `ULINT`, 10 `BYTE`, 11 `WORD`, 12 `DWORD`, 13 `LWORD`, 14 `REAL`, 15 `LREAL`); 8, little-endian, zero-extended above the type's width |

An address is 7 bytes: the area (1: 1 `%I`, 2 `%Q`, 3 `%M`), the size (1: 1 X, 2 B, 3 W, 4 D, 5 L),
the byte offset (4) and the bit (1, 0 unless the size is X). A line is a text exactly as `io.toml`
writes it: a number's decimal digits, such as `17`, or a pin name, such as `PD12`. A driver whose
name is not `gpio` carries no lines. `trust-runtime build` fills this section from the project's
`runtime.toml` and `io.toml` with the runtime's own parsers and checks, so every value has the
meaning and the defaults it has on a computer, and refuses a value the section cannot hold with the
reason and without writing a record.

The identities:

- **Program identity.** The SHA-256 and the length of the exact bytes of `program.stbc`.
- **Configuration identity.** The SHA-256 over the project's `runtime.toml` and then its
  `io.toml`, each as its file name, a zero byte, and then either a byte 1, its length (8) and its
  bytes, or a byte 0 when the project has no such file.
- **Retain-schema identity.** The SHA-256 over the declarations a checkpoint keeps, in the order
  it keeps them: every retained global, then the retained variables of each program instance.
  Each counts with its checkpoint name (the global's name, or `@program/<instance>/<name>`), its
  retention policy and its type. TYPE_TABLE follows, with every name spelled out. The shared core
  computes it (`deployment::retain_schema_identity`), and the checkpoint log keeps its first 16
  bytes (§10.1).

The build requests the hosted default of every limit. A program that the runtime cannot prepare
under them gets no record; the build removes an older record and says why. A runtime prepares an
admitted program with exactly the requested limits, so the program meets the same limits on every
target (ARCH-01).

**Reading a record.** A runtime reads every record version it knows. A version-1 record has
exactly sections 1 to 7, in that order, each payload exactly as long as its content. A runtime
refuses with `RuntimeError::DeploymentRecordInvalid` and the stable code
`runtime_deployment_record_invalid`, stating the reason, a record that is shorter than its
lengths; that has another magic, an unknown version, a length other than the file's or above
65,536 bytes, or a wrong CRC; that has an unknown, repeated, out-of-order or missing section, a
section count other than its sections, or a payload of another length; that has a reserved field
other than 0, an unknown import kind or task flag, an empty or non-UTF-8 name, or imports out of
order or repeated; or that the device's memory cannot hold. Section 7 refuses in the same way: a
code outside its list, a flag other than 0 or 1, a bit above 7 or a bit with a size other than X, an
empty name where one is required, a text over 255 bytes or not UTF-8, a line under a driver that is
not `gpio`, a count its payload does not hold, a payload of another length, and a safe-state value
with a byte above its type's width that is not 0. A later version adds sections with new
identifiers after those of version 1 and leaves the sections of version 1 as they are.

**Admission (DEPLOY-04, LOAD-07, LOAD-09).** Before any state exists, a runtime checks the record
against its device descriptor in this order. The first check that fails refuses the deployment,
with `RuntimeError::DeploymentRefused`, the stable code `runtime_deployment_refused` and the
numbers involved; nothing is constructed:

1. The program has the length and the SHA-256 of the record.
2. The device runs its STBC version.
3. The device implements the scheduling/I/O profile and the numeric contract.
4. The required timing evidence is not stronger than the device's (LOAD-08).
5. The device supplies every required capability.
6. Every import has a contract of the same version on the device (§5.2.1). Otherwise the
   refusal is `runtime_import_unsupported`, naming the import.
7. No requested limit is above the device's maximum for it, for example "the record requests
   max_call_depth 2048, the device allows at most 1024"; nor is the record's section-7 event-store
   capacity, when it is not 0, above the device's maximum, refused as "the record requests an
   event store of 500 events, the device keeps at most 200" (0 is the device's default).
8. On a device with an application slot, the program and its record fit in it, for example "the
   program and its record need 20480 bytes of the application slot, the device has 16384".

A chip admits its installed container with `installation::admit` before any state exists.

Preparation then validates the program under the requested limits (LOAD-03). Version 1 does not
compare the configuration identity: a computer reads `runtime.toml` and `io.toml` at each start,
so an edited configuration takes effect at the next start, as it does today, and a chip receives
its configuration inside the record, in section 7. Whether retained values fit a changed schema is
decided at the warm start, by name (specification 11 §6.7).

**Device descriptor (COMPAT-02).** Admission reads from the device's descriptor the STBC versions
it runs, its scheduling/I/O profiles, numeric contracts, timing-evidence class and capabilities,
its maximum for each preparation limit, the event store's default capacity and maximum, and the
size of its application slot. A computer runs STBC 2.0 and 3.0, `cooperative-resource-v1` and
numeric contract 1, has unqualified timing evidence and the wall clock, allows at most a 64 MiB
artifact and 1,024 call levels, keeps the newest 1,000 events by default and admits at most
1,000,000, and has no maximum for the other limits and no application slot (HOST-02). A chip's
descriptor comes from its chip file (TARGET-03); Appendix H gains the keys for its maxima with the
chip's firmware, and its `[events]` table gives the chip's event-store default and maximum. The
control connection serves the descriptor with the `capabilities` operation (specification 11), the
same operation on every runtime.

### 5.2 Compiler-free preparation

**LOAD-01.** Every target, including Linux, Windows, macOS, and the MCU, shall be able to initialize a fresh engine from bytecode and its deployment metadata without parsing ST, constructing a compiler session, or running HIR analysis. Hosted authoring tools may compile before deployment, but the standalone loading/execution path shall not require them.

**LOAD-02.** All executable metadata needed for initialization shall be present in the deployment artifacts or the firmware's versioned built-in library. Missing information shall fail admission. The build pipeline may need an additive metadata extension; it shall not hide the gap by compiling source on the MCU.

**LOAD-03.** The loader shall verify the container version, integrity, section ranges and overlap, checked size arithmetic, instruction boundaries, jump/call destinations, table references, types, imports, and target capabilities before allowing RUN.

**LOAD-04.** Resource counts shall be checked or conservatively recomputed against the actual target representation. A compiler-provided memory estimate is not trusted authority. Validation work and decoder allocations shall themselves be bounded so a small file cannot request unbounded expansion.

**LOAD-05.** Runtime checks that remain necessary for dynamic indexes, null/reference lifetime errors, call depth, execution budgets, and arithmetic faults shall remain active. Admission is not permission to remove runtime memory protections.

**LOAD-06.** Required imports shall resolve to approved, typed implementations with a versioned execution contract covering maximum work, variable-size arguments, blocking, allocation, failure, and allowed profile/state. A callback without a supportable bound shall be staged outside bounded execution or rejected for that profile; a deadline parameter is not proof of cancellation. No arbitrary pointer or symbol name shall become a device address or callable function. Host-only imports shall fail MCU admission; hosted builds may admit them only under the selected platform/profile contract.

Debug/source sections may be omitted when genuinely optional. Required symbol, type, interface, or import information shall not be stripped merely because it resembles debug metadata. The line table of specification 12 §6.13 is optional: a target without it still debugs from statement starts, and a target never needs the ST text to resolve breakpoints or steps (owner, 18 October 2026).

STBC integrity checks and CRCs shall not be described as authentication. Deployment uses the access rights of the control connection on every target, as trust-runtime does (specification 11, runtime control authorization); signatures are required where the product's threat model requires them.

The initial source-free execution profile rejects raw `REF_TABLE` storage domains
`Retain` and `Io` during preparation, before any initializer or POU executes.
These domains have no shared executable storage implementation in Scope A4.
This restriction does not reject `RETAIN` declaration flags on ordinary global
or instance storage, or `IO_MAP` addresses bound to that storage. Process images
and hierarchical direct addresses retain their separate codec and cycle-boundary
synchronization. Future raw-domain support requires an explicit consumer contract;
a structurally valid wire record alone is not execution-profile admission.

The A4 cooperative source-free composition admits exactly one resource. The
2.0 storage/root tables do not encode cross-resource ownership; accepting a
second resource would incorrectly turn its unscheduled program roots into
background work. Resource index zero is the only admitted instance selection.
Hosted 1.x resource handling is unchanged.

Ordinary byte-input preparation may transfer the decoded container's owned
payloads into immutable execution metadata after complete validation. The
private consuming source shall be constructible only through successful validation;
it must not forge a borrowed validation token or publish a partially populated
VM. Borrowed decoded inputs and hosted artifact-retaining entry points keep their
original sections and serialized order. Both ownership routes use the same
materializer and retain fault order, profile admission and conservative logical
preparation charges; eliminated temporary copies do not constitute hardware
memory or timing evidence. Typed transfer occurs only after the corresponding
borrowed conversion/inspection work completes.

Preparation has separate artifact, validation-scratch, logical preparation and
runtime construction limits. The byte entry point enforces the artifact bound against the supplied byte length
before decoding; it shall not serialize the decoded object again. The decoded-object
entry point uses bounded serialization to check representability and encoded size.
Both entry points retain the complete validator and execution-profile admission. Byte-input decoding
charges concrete section and nested-table reservations and payload copies before
allocation, including zero-length string records; its demand seeds the same
cumulative preparation budget. A caller-owned decoded object is excluded from
that initial demand. Preparation accounting
charges serialization scratch only on the decoded-object route, and concrete table records using target
`size_of`, owned payload lengths and nested vector elements. Map entries charge
their key/value payload plus four pointer-sized link/control words; actual bucket
capacity and allocator node occupancy are excluded. Temporary instance-owner
sets are bounded by the number of reference instructions in each actual POU
body. Expanded constants are additionally charged before vector/map reservations
and scalar/string copies. The statement index that a LINE_TABLE builds for debugging (specification 12
§6.13; SERVICE-06) is charged before it is reserved, at 104 bytes per statement and 76 bytes per POU that owns
one, as upper bounds for the 64-bit layout. Work charges serialized bytes, actual metadata/body
visits, lookup comparisons and expanded constant visits. These cumulative logical charges are deliberately
conservative admission metrics, not allocator peak, RSS, elapsed time or WCET;
allocator overhead, error diagnostics and native stack still require measured
profile headroom in Scope B. Validation retains its independent scratch/work
limits. Executable native imports must resolve to the shared standard-library or
POU implementation, and every built-in import must match its contract (§5.2.1),
a built-in FB's signature and hidden state included. CURRENT_DT additionally requires an explicitly supplied wall-clock
service before instance construction; a logical-only composition rejects it.

### 5.2.1 Built-in import contracts

LOAD-06 and PORT-02 are met for the built-in imports by one contract table in
`trust-runtime-core` (`stdlib::contracts`), the same on every target; no built-in
import is host-only. The standard-library registration stays the one list of
registered functions: each registered family names its contract there. Conversions,
date/time splits, the runtime clocks and the standard function blocks keep their
recognizers, and each kind they recognize has one contract. A native test compares
the table with the registration entry by entry.

Every contract in the table is version 1; a change to any rule of a contract issues
a new version. A contract states:

- **Signature.** A registered function takes the parameters of its registration: a
  fixed list, or fixed parameters followed by a numbered variadic suffix with a
  minimum count (specification 07). A conversion takes one input `IN`. A split takes
  `IN` and then its outputs, each a writable target. A clock takes no parameter. A
  standard function block declares its IEC parameters, each with its direction and
  type, and the hidden native state of specification 12 §11.5.5.
- **Work.** What TIME-08 charges beyond the call's instructions. A registered function
  or a conversion charges its argument binding and 1 unit per byte of its STRING and
  WSTRING arguments before the body runs; a clock, a split or a function block charges
  its argument binding only. A call has at most 1,024 native arguments (specification
  12 §4.6), and a text argument is bounded by its declared capacity or by the
  allocation allowance.
- **Allocation.** Whether the call can return a STRING or WSTRING result, which
  TIME-08 charges as a new value: 1 unit and its bytes of allocation.
- **Blocking.** None. Every built-in import returns without waiting for a device, a
  lock or a service other than the clock it is given.
- **Failure.** The stable codes its implementation can return for a call that matches
  its signature. The engine's binding and reference checks (specification 12 §7.7 and
  §7.8) apply to every call besides.
- **Profile.** CURRENT_DT requires the platform wall clock (§5.2). No other built-in
  import requires a capability.

| Family | Imports | IEC 61131-3 Ed.3 | Text | Failures |
|---|---|---|---|---|
| Numeric | ABS, SQRT, LN, LOG, EXP, SIN, COS, TAN, ASIN, ACOS, ATAN, ATAN2, ADD, SUB, MUL, DIV, MOD, EXPT, MOVE | Tables 28 and 29; ADD, SUB, MUL and DIV of durations and dates: Table 35 | arguments charged; MOVE returns text | type mismatch, overflow, division by zero, modulo by zero, date/time range, argument count (ADD and MUL with a time operand take exactly two inputs) |
| Bit | SHL, SHR, ROL, ROR, AND, OR, XOR, NOT | Tables 30 and 31 | arguments charged | type mismatch |
| Selection | SEL, MAX, MIN, LIMIT, MUX | Table 32; enumerations: Table 38 | arguments charged; text results | type mismatch, overflow, index out of bounds |
| Comparison | GT, GE, EQ, LE, LT, NE | Table 33; enumerations: Table 38 | arguments charged | type mismatch |
| String | LEN, LEFT, RIGHT, MID, CONCAT, INSERT, DELETE, REPLACE, FIND; the assignment helper `__TRUST_LIMIT_STRING` (specification 07 §8.1) | Table 34 | arguments charged; text results | type mismatch, overflow, string capacity exceeded (a result above 65,535 bytes or code units) |
| Time | ADD_TIME, ADD_LTIME, ADD_TOD_TIME, ADD_LTOD_LTIME, ADD_DT_TIME, ADD_LDT_LTIME, the twelve SUB_ functions, MUL_TIME, MUL_LTIME, DIV_TIME, DIV_LTIME; CONCAT_DATE, CONCAT_DATE_TOD, CONCAT_DATE_LTOD, CONCAT_TOD, CONCAT_LTOD, CONCAT_DT, CONCAT_LDT, DAY_OF_WEEK | Tables 35 and 36 | arguments charged | type mismatch, overflow, division by zero, date/time range |
| Validate | IS_VALID, IS_VALID_BCD | Table 39 | arguments charged | type mismatch |
| Assertions | ASSERT_TRUE, ASSERT_FALSE, ASSERT_EQUAL, ASSERT_NOT_EQUAL, ASSERT_GREATER, ASSERT_LESS, ASSERT_GREATER_OR_EQUAL, ASSERT_LESS_OR_EQUAL, ASSERT_NEAR (specification 07, a truST test extension) | none | arguments charged | assertion failed, type mismatch, overflow |
| Conversion | every name of a Table 22 conversion form | Tables 22 to 27 | arguments charged; text results | type mismatch, overflow, string capacity exceeded (a STRING or WSTRING result above 65,535 bytes or code units) |
| Split | SPLIT_DATE, SPLIT_TOD, SPLIT_LTOD, SPLIT_DT, SPLIT_LDT | Table 36 | binding only | type mismatch, overflow |
| Clock | TIME, CURRENT_DT (specification 07, truST clock sources) | none | binding only | none for TIME; overflow for CURRENT_DT |
| Bistable | RS, SR | Table 43 | binding only | type mismatch |
| Edge detection | R_TRIG, F_TRIG, and their other names DIFU and DIFD | Table 44 | binding only | type mismatch |
| Counters | CTU, CTD, CTUD and their _INT, _DINT, _LINT, _UDINT and _ULINT variants | Table 45 | binding only | type mismatch |
| Timers | TP, TON, TOF and their _TIME and _LTIME variants | Table 46 | binding only | type mismatch |

The failures are the stable codes `runtime_type_mismatch`, `runtime_overflow`,
`runtime_division_by_zero`, `runtime_modulo_by_zero`, `runtime_date_time_range`,
`runtime_invalid_argument_count`, `runtime_index_out_of_bounds`,
`runtime_string_capacity_exceeded` and `runtime_assertion_failed`.

**Admission.** A standard-library descriptor resolves in the order the call
dispatcher runs it: a clock, a split, a conversion, then a registered function.
Before any state exists, preparation refuses the artifact (LOAD-03, ARCH-06,
DEPLOY-04) when:

- a standard-library descriptor names no contract;
- its argument count does not fit the signature (a fixed list takes exactly its
  parameters, a variadic one at least its minimum), or its argument names do not fit
  the parameters: an unknown or repeated name, a gap in a variadic suffix, or named
  and positional arguments in one call (specification 12 §7.7);
- a split output is not a writable target;
- a function-block template that carries a standard function block's name declares
  other parameters than its contract, in number, name, direction or type, or other
  hidden native state.

The refusal is `RuntimeError::ImportUnsupported` with the stable code
`runtime_import_unsupported`. It names the import by its canonical uppercase name and
states which part of the contract it fails, for example `unsupported import 'TON': a
parameter's type or direction differs from the contract`. A call of the artifact's
own function, function block or method resolves to that POU and has no built-in
contract. When a deployment record (§5.1) names the contract version of a required
import, admission refuses a version other than the runtime's.

### 5.3 Validated construction and admission evidence

**LOAD-07.** A prepared module eligible for execution shall be constructible only after successful artifact validation, capability/resource admission, and preparation. Raw decoded objects shall not be executable. Constructors, deserialization, test helpers, and embedding APIs shall not expose an accidental production bypass of this transition.

**LOAD-08.** Timing admission shall follow Sections 4.4 and 8.4. The loader or installation manager shall verify that the selected timing evidence matches the application, configuration, engine/adapter build, and platform envelope. The device need not reproduce an entire off-device analysis, but it shall independently enforce its capability/resource limits, reject mismatched evidence, and refuse a stronger claim than the evidence supports. A memory-fit result shall not be reported as timing admission.

**LOAD-09.** Candidate validation and preparation shall be transactional with respect to the installed generation: failure shall not corrupt or publish a partially prepared replacement. Work, allocations, reference expansion, and failure reporting shall remain bounded by the preparation profile, including construction under TIME-09. Excess demand shall fail before switching.

The public source-free state surface does not implement the internal mutable
storage/dispatcher adapter traits. Its private execution state is reached only
through admitted construction, typed engineering access, cycle and restart methods;
a trait-qualified call must not expose an untyped storage mutation bypass. The
shared host adapter traits remain available for implementation and construction
compatibility compositions, subject to HOST-09: they shall not expose unrestricted
mutation of a public installed controller.

A validated constructor is an implementation invariant, not a substitute for fuzzing or an assertion that the validator is correct. Artifact and deployment-metadata mutation tests are mandatory under Section 14.

### 5.4 Compiler-free construction and executable initialization

**LOAD-10.** Scope A shall implement the fixture's artifact-to-runtime construction map below. Use complete existing type/vtable/task/I/O/reference/constant data and encode missing construction/initialization under specification 12 §11. STBC 2.0 is the selected compatibility decision, not the only theoretically possible format design. It gives a hard rejection boundary for old readers; this plan does not use an optional 1.x section for mandatory initialization. During the defined window, hosted authoring compositions retain 1.1 import with explicit construction context; source-free consumers accept 2.0 and reject 1.1. Do not infer missing state or invoke HIR inside the source-free engine. [R20, R21, R29, R36]

**Complete loader relocation.** Move the entire current decoder and semantic validator, including optional supported sections and malformed-input rules, into `trust-runtime-core`; do not create a reduced MCU validator. Move the byte-only `BytecodeModule` container and metadata materialization with them, reusing the core reader/helpers. Resolve its inherent-method ownership coherently: portable `encode` serialization may reside in a separate optional core module/feature, while host `encoder/` lowering, compiler/HIR integration, disassembly presentation, file I/O, and Runtime application remain host tooling/adapters. Preserve public hosted bytecode entry points through re-exports or thin compatibility wrappers. Firmware need not link encoding/presentation work it does not call. [R36]

**A2 review expansion (9 October 2026).** The owner explicitly expands A2 to include the external review's correctness and architecture recommendations. Convert bytecode include fragments to normal modules; decode each POU instruction stream once and share it across semantic passes through one validation context; use structured section rejection reasons with unchanged stable error codes/display text; expose a borrowed validated-container marker whose private constructor is reached only by successful validation. The marker does not certify target capability/resource admission or executable preparation. Public hosted fields/constructors remain compatible through borrowed views and owned conversions.

LOAD-04 correction is now in this expanded scope: validation must not retain an operand-stack vector per linear instruction. Retain merge states only at basic-block entries, and enforce explicit preparation scratch/work limits before expanding analysis state. A caller may tighten the default hosted limits; an MCU profile must choose measured limits before admitting bytecode. Memory evidence must distinguish accounted analysis storage from allocator overhead and the decoded artifact itself. No validation of the earlier relocation establishes these new claims.

**A2 compatibility and serialization decision.** Core owns the allocator-backed `BytecodeModule`, its full 1.x decoder/validator, metadata materialization and byte serialization. Serialization stays available without an additional feature; compiler/HIR lowering remains hosted and unused serialization need not be linked into firmware. A borrowed `BytecodeModuleView` shares those read-only operations with the thin hosted container adapter, preserving existing public fields, struct literals and `from_runtime*` constructors without cloning section data. Neither a decoded container nor its view represents executable admission. Semantic analysis uses budgeted portable vectors and borrowed names; framing validation retains a bounded known-section set. Existing input traversal and first-error ordering are preserved except the explicitly added malformed-layout, raw-discriminant and analysis-limit rejections. A2 used crc32fast 1.5.0 with explicit std opt-ins; B-R1 retains its hosted acceleration and replaces its embedded use with the shared compact implementation described in Appendix D.1. A2 retains the STBC 1.x format, with the reviewed admission hardening above; version changes and prepared execution remain A3/A4.

Replace the singleton version assumption with explicit reader-supported pairs and an encoder output version; do not blindly change `SUPPORTED_MAJOR_VERSION` to 2 in every test. Preserve legacy fixtures explicitly and add 2.0 cases, rejection matrix, and resource-limit mutations. Audit all version-pair/layout branches, including current comparisons based only on `minor`, CRC defaults, and VM materialization. Six direct version-assumption test surfaces are `bytecode_container`, `bytecode_metadata`, `bytecode_decode_resource_bounds`, `bytecode_sections`, `bytecode_helpers`, and `process_image`; encoder/roundtrip/validation/optional-section and VM tests also remain in scope. Retain the tracked OSCAT 1.1 artifact and generate a separately named reproducible 2.0 fixture rather than replacing compatibility evidence. [R36]

| Required runtime data | Artifact/preparation obligation | Required acceptance case |
|---|---|---|
| Program/POU identities, global layout, and instance roots | Construct from validated POU/type/reference metadata and explicit layout/root information. | A fresh engine registers and starts the saved application without preinstalled host objects. |
| Scalar/compound defaults and declared capacities | Reuse TYPE_TABLE/CONST_POOL and typed reference metadata; encode missing declared defaults/construction roots explicitly. Do not call HIR-gated `default_value_for_type`. | Nonzero defaults, nested arrays/structures, and defaulted parameters survive cold initialization. |
| Executable local/aggregate initializers | Compile to ordinary validated STBC body ranges, indexed by POU/initialization phase/declaration identity and executed through the same dispatcher and work budget. Do not move the Expr/HIR tree walker into core. | Repeated calls with changing inputs/globals re-run each automatic local's initializer at every call boundary, so no local keeps a value from the previous call (specification 03 limits source initializers to literals and constant expressions, E202); emitted artifacts need no source or initializer catalog. |
| Static locals and persistent FB/class state | Encode ownership, allocation capacity, first-use rules, and applicable restart behavior. | First use, repeated calls, and cold/warm restart match the baseline. |
| Inheritance, method/interface dispatch, and parameter binding | Prepare validated type/member/method tables, defaults, OUT/IN_OUT behavior, and receiver checks. | Two instances retain distinct state while using shared method code and checked interface dispatch. |
| Tasks, event references, process images, and bindings | Construct tasks and bindings only after validating referenced program/instance storage. | Periodic/event tasks and `%I`/`%Q`/`%M` bindings initialize without `CompileSession`. |
| RETAIN and restart initialization | Bind schema, initial state, checkpoint compatibility, and restart actions to the artifact identity. | Cold/warm initialization and rejected incompatible checkpoints preserve the specified contract. |
| Native/standard-library imports | Resolve only typed, versioned implementations admitted by LOAD-06. | Required timer/math imports resolve; unavailable or mismatched imports fail before RUN. |

**LOAD-11.** Prepare immutable bytecode initialization plans and storage before RUN; execute dynamic initializers at their required call/first-use/restart boundary. Scope/frame visibility, declaration order, parameter preservation, static once-state, instance construction, bounds, faults, and value copies follow specification 12 §7.11. Use the same executor and nested work budget as the POU body. Replace module-address cache identity with the admitted module/generation identity; remove the Expr initializer execution path from migrated runtime execution once the corresponding coverage is complete. HIR may remain in the compiler, but neither a serialized AST interpreter nor a first-use lowering cache belongs in the portable engine. [R29]

Source-free native output copy-back is one transaction: all output conversions and
writes either complete or leave all caller destinations unchanged, including on
budget exhaustion or a physical deadline during commit. This covers split-date/time
native outputs as well as user POU and function-block outputs. It does not roll back
the callee's preceding execution or unrelated PLC state mutations. The legacy hosted
composition keeps its existing compatibility policy until its migration.


M2A shall include the construction rows exercised by its representative fixture, including a local initializer exercised by repeated calls with changing inputs, and initialized compound/FB state. The entire map shall be covered by M2B/M3 before broad parity or bounded-profile claims. Removing the `trust-hir` dependency while leaving a hidden harness initializer path does not close this work.

A2 additionally records the LOAD_NULL owner-inference correction in specification 12. Its native
regression must execute a NULL assignment and per-instance field reads through two FB instances.
The large-program admission regression covers indexed metadata lookup and compact decoded
instructions; previous run-4 evidence does not validate these later changes.


## 6. Memory and value model

### 6.1 Preparation versus execution

The engine shall have a preparation phase that determines its full resource demand and reserves or accepts the required storage. RUN shall only become available after preparation succeeds.

**MEM-01.** The resource model shall separately account for portable program storage, each prepared instruction/metadata representation, constants, mutable PLC globals and FB/class instances, compound backing regions, process images, operand/frame/local/temporary storage, task-ready storage, I/O staging, diagnostics, service queues, and retain staging. CPU stacks and interrupt allowances shall be recorded separately from VM stacks.

Record target `size_of`/alignment for `Value`, references, frames, and relevant compound headers, then enforce their admitted size ceilings with target-compiled assertions. A host size result does not establish a 32-bit layout. These are resource bounds, not a permanent Rust ABI or a request to pack structs unsafely. [R30]

**MEM-02.** Define per-profile maxima for encoded artifact bytes, decoded instructions/references/types/locals, operand values, frames, native-call arguments, initialization nesting, and `max_call_depth`. Apply decode/allocation limits before consuming attacker-controlled counts. Do not reserve the current host-scale 16,384 operands, 1,024 call levels, or 64 MiB container allowance on F401. Size each per-call reservation to the program's static call depth, capped at `max_call_depth`; `max_call_depth` remains the admission ceiling and the runtime depth check is unchanged (MEM-09). Report preparation, RUN, checkpoint, and generation/reader peaks per usable region. Tie admitted depth to native call-stack use when execution remains recursive, including interrupt and fault headroom. [R30]

**MEM-03.** Operand/frame/local storage shall have capacity established before bounded RUN. A push/call uses existing storage or faults before crossing its admitted limit. Existing FB/method calls can recursively re-enter the Rust executor, so a frame-count check alone does not bound the CPU stack. Scope B records F401 MSP high-water through stack painting or equivalent instrumentation under nested calls, interrupts, and fault unwinding/abort paths; record PSP separately if used. Measure representative per-level growth and corroborate worst-path headroom with code/stack analysis. Keep a conservative finite call limit; an explicit iterative frame loop is an option if measured recursion prevents admission, not an automatic rewrite prerequisite. [R30]

**MEM-04.** The allocation audit shall include cloning/dropping strings and arrays, nested values, local initializers, Arc/Box ownership, destruction of the last owner, formatting errors, task sorting, imports/library calls, tracing, and checkpoints. Tests shall exercise first invocation, rare branches, maximum admitted sizes, fault/limit paths, and relevant generation retirement. A warmed-up success loop or reserved operand stack alone is insufficient.

Float/text footprint reduction shall retain both conversion directions and the
same function set on hosts, F401 and C6. The shared implementation may use
lossless compact decimal algorithms (lexical-parse-float and Ryu), provided native
differential evidence preserves specification 07 §2.7–2.8, stable errors and
existing diagnostic spelling. No lossy mode, profile rejection or overflow-check
change is permitted. The complete linked image, not table-size estimates,
determines acceptance; conversion execution cost remains part of timing evidence.

**MEM-05.** Core fault records shall be fixed-size codes with bounded context on hosted and MCU targets. Rich names and messages shall be rendered outside the core control path. Existing stable fault identities shall be preserved; compatibility wrappers may retain hosted messages without making fault capture allocate or block. A VM fault raised during a scan shall be a structured, fixed-size value built without allocation, whose rendering is today's text and whose stable code is unchanged; a `FaultRecord` shall hold that fault with the first failing instruction's site, the task and the logical time. The record stays at most 56 bytes on every target, so its logical time is the scan's whole seconds since the resource's logical start in a 32-bit field, unambiguous for 136 years of uptime, beside the millisecond within that second in a 16-bit field (0 through 999), the millisecond precision the host's fault log reports today. The engine latches the record by value, without cloning the error and without allocating, records the innermost failing instruction's site while the scan unwinds, and names the task that ran, which is `u16::MAX` when no task ran. A host reads the latched record as often as it asks, `fault()` still returns the error, and a restart or a fault-clearing recovery clears the record with the fault. The event store and the hosted fault holder keep that record as the fault, and `faults` and `events.tail` render its text when they reply, so a device records a fault without allocating.

**MEM-06.** Compound data shall have known capacity and execution-cost bounds. The first implementation may use preallocated typed storage before a full packed representation, but bounded execution shall not rely on recursive heap-allocating Value clones. Array/string operations, nested initialization, and reference traversal shall be bounded even when implemented by one opcode. Replacing value copies with aliases is prohibited unless the existing language contract permits that aliasing; reference lifetimes and copy semantics shall be preserved.

H applies the artifact type-nesting bound to recursive value-copy accounting,
independently of the profile's POU call-depth bound. A value at the admitted
structural depth remains copyable when the profile admits fewer call frames;
exceeding the structural bound reports `TypeMismatch`. Copy work follows
TIME-08, and backing allocation is charged before copying. This corrects the former use of the call-frame limit
for aggregate traversal; it does not increase admitted call depth.

### 6.2 Recommended storage approach

Retain the current value behavior as the reference contract. Introduce an execution-storage boundary that can access fixed scalar slots and bounded compound regions using validated handles/offsets.

Strings shall have explicit capacities and lengths; arrays shall retain their declared bounds; structures shall use precomputed member access information; FB/class instances shall occupy pre-created storage. Copies and temporary operations shall have admitted capacity and bounded cost.

The shared VM shall use the same checked access and arithmetic semantics regardless of whether the host owns backing storage in allocated containers or firmware supplies fixed buffers. Do not introduce separate opcode implementations named “desktop” and “embedded.”

A complete packed representation is not the first extraction step. First separate ownership, then replace allocation-producing paths under behavior tests. This keeps memory redesign and module relocation independently reviewable.

**MEM-10.** Use exclusive ownership of mutable execution state and an explicit borrow of immutable prepared metadata as the initial portable boundary. Prepare dense program/type/member/slot identities so bounded execution does not depend on lazy name-map insertion or OS `RwLock` caches. Host code may own or pin generations with `Arc` outside this boundary where its threading model requires it. Do not replace every host `Arc` with `Rc`, weaken host thread-safety, or duplicate instruction semantics to make one target compile. Audit transitive string/container ownership as well as direct core imports. [R22, R24]

**Prepared cell layouts.** At preparation the shared engine computes, for every TYPE_TABLE entry, one
layout of the cells that will hold its values: the cell count (a scalar is one cell), a structure's
member ordinal to cell offset mapping with its nested compounds inline, an array's element stride and
dimensions, its text leaves with their byte capacity, its reference and instance leaves, the length
of a `STRUCT OVERLAP`'s byte backing, and whether the type can hold a reference or an instance so a
later lifetime check skips scalar-only layouts. A `STRING[n]` leaf reserves `n` bytes and a
`WSTRING[n]` leaf `2n` bytes of UTF-16 code units; a `STRING` or `WSTRING` declared without a length
reserves 80 and 160 bytes (D-C1-1, D-C1-2), which supersedes the earlier `4 × n` unit. Globals, each
persistent root (one range per root, so two instances of one type get two ranges; a derived
function block's template holds its own cells and a link to its parent's template, and a derived
root's range covers its whole inheritance chain: its own cells, then its parent's cells and so on up,
the parent instance that construction creates for it lying inside that range and reserving no cells of
its own; C1-9c), each POU's locals and each initializer result map to a cell range of the persistent or the activation region,
and a `REF_FIELD` member name resolves to a member ordinal at preparation. The prepared layout tables
and the work of computing them are charged once at preparation, under the admitted profile's limits.
A region's cells are only recorded at preparation, not charged: construction charges them when it
reserves the region, so no storage is counted twice and the refusal points of large programs do not
move. A program whose layout tables and other preparation demand exceed those limits is refused at
preparation, never at run time. Nothing reads the layouts at run time yet, so stored values, outputs
and faults are unchanged.

**Cells and the cell store.** The bounded value store's unit is a 16-byte cell, `Copy` and without
`Drop`. One cell holds a scalar of the value model inline: `BOOL`, the eight integers, `REAL` and
`LREAL`, the four bit strings, `CHAR` and `WCHAR`, and each of the eight temporal types as its
64-bit tick or nanosecond count (`core/value/datetime.rs`). It also holds an enumeration (its
TYPE_TABLE type and its value), an instance identity, a text slot, a reference slot, an empty cell
and an empty reference; a structure or array is a contiguous cell range of its prepared layout with
its nested compounds inline. A region owns one cell store: its cells and one reference record per
reference leaf the region holds, where a record is the flat reference address a reference leaf
points at — the region, the cell index of its target, the character index of a character reference,
the target type and the installation generation (MEM-07). The store reserves both exactly once from
the prepared layouts, through the same checked reservation that refuses an allocation failure as the
other construction reservations do, and never grows: every accessor answers `None` outside the
reserved region and allocates nothing. The persistent region holds its globals' and its persistent roots'
cells and reference leaves, one range per root, counted from the prepared layouts rather than by hand. The
bytes a store takes are its cells at 16 bytes each, its reference records, and its reference
table, which holds one entry per reference leaf and the `ValueRef` of each non-null reference until
C1-11 (O9), so construction charges them before reserving. Nothing reads the store at run time yet, so stored values, outputs and
faults are unchanged. The text store holds a region's text buffers — one fixed buffer per text leaf
at its leaf's byte capacity, reserved once and never grown, with its length in use — and its text
sink cuts written text exactly where assignment normalization cuts it, so a `Cell::Text` leaf
converts a `STRING` or `WSTRING` value through its buffer while a non-null reference stays refused.

**Value conversion.** An engineering write converts a `Value` of a prepared type into its layout's
cells — checking the whole value before it writes a cell and allocating nothing, keeping every
scalar's exact bits, every enumeration's TYPE_TABLE type and value, and every structure member, array
element and nested compound at its layout offset — and refuses, leaving the cells as they were, a
value whose kind, members, bounds, element count or slice length does not match the type, a non-null
reference (C1-9) and a `STRUCT OVERLAP` or union (C1-15), while a read materializes
the same `Value` again and a member-and-index path resolves to its cell offset through the same
layout (C1-10); nothing calls the conversion at run time yet, so stored values, outputs and faults
are unchanged.

**MEM-11.** Document scalar slots, compound regions, frame/local/temporary reuse, initialization scratch, and copy/drop ownership. Select one bounded representation for hosts and MCUs with application-sized capacities. Scope A/B may retain `SmolStr`/`IndexMap`/`Arc` values under a bounded bring-up heap on F401/C6, recording every remaining scan allocation and refcount/destruction cost; this cannot qualify ARCH-05. Implement measured narrow storage improvements before first hardware evidence where needed. Complete atomic-reference-count retirement and bounded compound copying in M3; only then require optional C3/IMC support. Avoid a throwaway value model solely to pass M1. Retain failed fit evidence and the mandatory fixture. [R24, R30, R31]

### 6.2.1 B-R3 live storage and footprint refinement

B-R3 preserves the complete IEC/truST function set and all stable faults, value-copy,
reference, rollback and ordering contracts. It changes storage and code generation,
not bytecode semantics. This is a measured bring-up slice of MEM-03/06/10/11, not
completion of M3's allocation-free compound-value contract.

- Frame/instance identities retain every bit of their existing u32 domain. Within
  a top-level entry they increase and removed identities stay invalid; §4.3.1 and
  HOST-02 state when a later entry issues them again. Capacity follows live
  entries, not the largest identity ever issued. ID exhaustion remains a checked
  fault. Sorted live entries use binary search; reserve appends in identity order.
  Arbitrary public removals remain supported even where ordinary frame retirement
  is LIFO.
- Ordinary activation depth and initializer staging storage are distinct. A POU
  activation remains live while its declaration initializer reserves a result slot
  at the same logical depth. Do not equate storage entries with max_call_depth.
  A conservative storage reservation covers ordinary and initializer stacks each
  bounded by the admitted depth (checked twice-depth capacity); tests exercise an
  initializer at the deepest admitted call. This does not increase admitted call depth.
- Instance retirement processes an owner batch with a single storage retain pass;
  promoted instances survive retirement of their former owner. Side metadata and
  cached offsets must be invalidated consistently. Terminal cleanup must still finish
  when execution fuel is spent.
- Sorted collections retain key order, first/last-match and alias deduplication behavior.
  Journals keep the complete (MemoryLocation, usize slot) key and restore in the existing
  sorted order. Snapshot only affected destinations; never clone whole storage.
  Bulk input snapshots resolve references in their original order, then sort and
  deduplicate destination identities before copying values. Input writes retain
  their original order. An append must not shift the whole collection, and arbitrary
  input order must not turn the admitted input-binding transaction into quadratic
  snapshot construction. This bookkeeping is not charged (TIME-08).
- Once the physical deadline expires in an execution operation, retain that result
  through unwinding and terminal cleanup. Do not sample the clock again to rediscover
  an already observed expiry. Reset the latch only at the next operation's existing
  budget-reset boundary; retain fuel charging and complete terminal cleanup.
- Portable name lookup may use the existing insertion-ordered variable map's hashed
  name-to-slot index instead of maintaining duplicate string-tuple caches. Recursive
  lookup retains parent traversal and shadowing; unknown names stay misses without
  inserting entries. Hosted cache/public-map compatibility remains behind the shared
  storage API. This uses existing indexed identities and adds no linear name-table scan.
- Fallible vector growth and moved-entry work are accounted explicitly. Do not reserve
  host-scale construction maxima in the MCU heap. Preallocate bounded frame metadata;
  instance/value storage retains bring-up allocation under existing construction charges
  and measured allocator capacity. No new claim of a global live-instance bound or
  allocation-free RUN follows from replacing trees. Record live peaks and residual growth.

Numeric reductions retain full signed/unsigned input domains, finite checks, rounding,
error precedence and exact results. Calendar era/remainder calculations may use i64
with a wide final expression where cancellation requires it. Signed/unsigned TIME scaling
must handle i64::MIN and ULINT_MAX without narrowing unsigned factors. Wide text parsing
and its TypeMismatch/Overflow distinction stay. Error constructors preserve exact hosted
rendering and stable codes. A validator inline attribute is an explicitly measured layout
experiment, not credited savings. No blanket panic removal or reduced validation is allowed.

All selected source changes may share one measurement wave. Keep the default stable
1.95.0 configuration, safe ICF and overflow checks. Report the 16 KiB upper-region
growth target, but do not make it a hardware-bring-up fit gate. The hard flash check is
that loadable content stays within the executable region and preserves sector-0 bounds
and reserved erase regions. Whole-section sector-0 repacking must preserve vector/data
alignment. Symbol deltas guide interpretation but do not guarantee independently additive
savings. Candidate trace simplification must preserve the complete bounded UART record
transaction and byte protocol. Only final maps/ELF and physical measurements establish fit.

### 6.2.2 B-R4 native-stack correction

The F401 physical run exceeded its reserved native stack although its logical call
count was admitted. B-R4 corrects that shared execution path without changing IEC
functionality, the admitted call depth, the 72 KiB heap / 16 KiB MSP partition, the
reported 16 KiB upper-flash growth target, or the 2 KiB measured stack-headroom
criterion. This is
an implementation/resource correction; IEC execution and binding semantics are unchanged.

- Compiler-produced function, function-block, method and interface calls on the stack
  backend shall execute through the existing dispatcher loop and explicit VM frames.
  Do not hold another Rust dispatcher invocation for each such PLC activation.
  Typed continuations retain the caller's pending return and output/edge state.
- Shared prepare/complete call helpers remain the single semantic owner. Hosted
  register tiers may use their synchronous adapter and existing optimization hooks;
  they shall not grow a second implementation of argument binding or copyback.
- Preserve argument evaluation order, supplied-versus-omitted parameters (including
  supplied NULL), defaults, receiver/interface resolution, return typing, output and
  IN_OUT alias ordering, atomic failed copyback, edge-input restoration, references
  to suspended locals, statement/call hooks and existing failure identities.
- Each deferred callee has isolated operand state. It cannot consume suspended caller
  operands; successful completion and all failure paths discard only the callee's
  temporary operands, then restore/retire frames in the established order.
- Continuation capacity follows admitted live depth, never issued identity values.
  Reserve fallibly and account new allocation and work before publishing pending state.
  All nested work shares the enclosing fuel/deadline allowance. Error unwinding must
  retire every active identity and restore temporary bindings even after fuel expiry.
- Initializer execution remains on the shared dispatcher and retains staging/lifetime
  protection. Any remaining synchronous construction/initializer nesting is explicit
  in the evidence; measured preparation/instantiation stack headroom is mandatory.
  Passing the fixture is not a whole-profile worst-case native-stack proof or M3 closure.
- The F401 bring-up composition shall place its long-lived `PreparedModule` owner
  in the existing measured heap before instantiation, rather than retain its large
  control object on the limited MSP throughout a fixture. This is one fixed firmware
  ownership allocation, included in PREP-phase actual heap observations; core PREP
  logical charges remain separately identified. State borrows the owner and is dropped
  before it. Standard allocation failure follows the existing terminal safe-output
  OOM boundary; this is not M3 recoverable/admission or allocation-free qualification.
  The 72 KiB heap, 16 KiB MSP and measured stack-headroom criterion remain unchanged.
  The 16 KiB upper-flash growth target is reported but is not a hardware-bring-up fit gate.
- Preserve independent firmware phase boundaries so unrelated fixture state is not
  inlined into a persistent outer runner frame. This is measured code generation,
  not credited savings or a new execution implementation.
- The hardware harness must stop with a failed record when a reported stack sample
  misses the existing headroom threshold, before advancing to the next phase/depth.
  It must not repaint away a failed high-water result or downgrade it into a pass.
  The unchanged depth-four fixture and final watchdog/safe-output requirements remain.

Complete source/test authoring and independent review precede one authorized consolidated
software/link/inspection/hardware batch. Preserve previous failed captures. Hardware is
conditional on passing software and ELF gates; no automatic retry follows a failure.

### 6.3 References and interfaces

**MEM-07.** Executable code and validated metadata shall remain immutable within an installed execution generation. A supported hosted update may replace the generation only under Section 11.3, without invalidating borrowed storage while readers still use it. PLC references shall use validated identities/handles, not serialized native addresses. Bounds, ownership, type, generation, and lifetime checks shall remain independent of CPU pointer size; a stale handle shall not silently resolve to state in a replacement generation.

**MEM-08.** Statically allocated FB/class instances, methods, interfaces, and checked interface dispatch shall remain supported for the admitted profile. Prepare member/method/interface tables and storage requirements before RUN. Dynamic receiver selection shall retain the necessary runtime type/generation checks; shared executable method bodies shall not require duplication for each instance.

Recursive calls and indirect call targets require a conservative bound or an explicit profile restriction. Call and instruction budgets must cover the whole nested execution, not restart on every nested call. Unsupported dynamic allocation or unbounded constructs shall produce a clear admission diagnostic.

**MEM-09.** Hosted backing storage may be allocated during preparation. Capacity growth, first-use allocation/lowering, and allocation-producing destruction or cache replacement shall not occur in bounded control paths. Instrument the control thread and its synchronous callbacks, including rare/failure paths; attribute unrelated service allocations separately. Shared allocator contention and other process-wide interference shall still be exercised in stress tests. Retired-generation reclamation shall satisfy Section 11.3.

**Execution scratch reserved at construction.** Construction reserves the storage that a scan's
stack execution and process-image staging use, and charges all of it against the construction
allocation allowance before reserving any of it (TIME-09), as HOST-04 does for register scratch:

- the operand stack, as deep as the deepest POU body plus, for every further reserved call
  level, the most operands that a body keeps below a call of a user function, function block or
  method while the callee runs. The validator records both numbers. With no such call there is
  one call level;
- the frame and pending-call stacks of those call levels;
- one local-slot vector as large as the largest POU's locals, for the program activation;
- the initializer, construction-frame and type-operation stacks;
- the output copy-back journal, with room for the most parameters of any POU;
- the staged input values, their destination snapshot and the output windows of every
  process-image binding.

A scan takes this storage and gives it back empty. RUN charges and refusals do not change: an
activation is charged the frame and pending-call growth that storage growing on demand would
need, and staging, snapshots and journals are charged as before (TIME-08).

Initializer evaluation, construction and the lifetime check reuse the same reserved bookkeeping:
an evaluated initializer's result slot, its seed's pending, visited and backup lists and its
owned-instance snapshot are taken per call depth and returned on every path, a construction frame
is moved rather than cloned, the lifetime check's visited set is reserved for the live instance
count and cleared per check, and each type's `SIZEOF` walks a fixed-depth stack; every result,
charge and refusal is unchanged (TIME-08).

Construction sizes these call levels to the program's static call depth, not to `max_call_depth`.
The validator walks every call target from every entry and, conservatively, of every POU in the
module, so a target reached only by method or interface dispatch still counts: a `CALL` to a
function or function block, a method call and an interface call over every POU that can implement
the method, and a nested instance declaration as the initialization it runs. The engine reserves
the lesser of that depth and `max_call_depth`. A call graph with a cycle, which IEC 61131-3 forbids
but the engine still executes, has no static depth and reserves for `max_call_depth`, exactly as
before. `max_call_depth` remains the admission ceiling and the runtime depth check is unchanged
(MEM-02).

**Call plans.** Preparation resolves every `CALL_NATIVE` site once, before RUN: the formal index of
each actual (named and positional resolution), the explicit-presence pattern, the output
descriptors and the callee — a function's prepared POU, a function block's POU, a method through
the prepared owner and method table, and a built-in function block's kind and parameters. A scan
reads the plan and resolves no argument names and builds no uppercase type-name key, and an
interface attempt compares prepared POUs while still charging the type-name bytes TIME-08
prescribes; every result, charge and refusal of a call is unchanged (TIME-08).

**Frame payloads.** Construction reserves one frame payload per call depth — that depth's local
slots and parameter-presence bits, sized for the largest POU — and an activation takes the payload
of its depth, binds its arguments into it and reads its edge inputs and their restores from
reserved lists, returning the payload when its frame is popped on every path (return, trap, fault
and abort); a scan allocates no locals, presence, edge-input or restore vector, and every result,
charge and refusal is unchanged (MEM-02, MEM-03, MEM-09; TIME-08).

**Call scratch.** Construction also reserves, per call depth, the call scratch that binds a call —
its evaluated argument list, parameter flags, output bindings, preflighted targets and preflighted
outputs, sized for the largest call site — and a call takes its depth's scratch and returns it on
every path (completion, trap, refusal and abort), so a scan allocates no argument, flag,
output-binding, target or prepared-output vector, and every charge is unchanged (MEM-09;
TIME-08). A standard function's arguments and a split function's outputs are bound into the call
depth's reserved values, and a named argument is matched without building an uppercase name, with
every result, count, charge and refusal unchanged (MEM-09; TIME-08).

### 6.4 Physical memory

The BSP shall identify memory regions available to the engine, CPU stack, DMA, communication, and boot/update storage. Capacity shall be checked per usable region, not only as a sum of all SRAM on the chip. Cache maintenance, DMA ownership, alignment, and interrupt-stack reservations are platform obligations.

Portable core code shall not assume a particular atomic width, multicore capability, or interrupt-masking mechanism. The selected dependency graph must be checked for these assumptions before claiming a new architecture is supported.

## 7. Platform interfaces

Interfaces shall be small, fallible where failure is possible, and free of MCU register types. The control path shall not require an async executor, a filesystem, an RTOS, `Send + Sync` on every object, or dynamically allocated trait objects.

| Interface | Required contract | Not its responsibility |
|---|---|---|
| Monotonic clock | Supply a nondecreasing timestamp in the engine's established time representation; declare resolution and wrap behavior | Sleeping, TON logic, or scheduling priorities |
| Runner wait/wake | Wake at the next relevant deadline or service event | Choosing which PLC task is logically ready |
| Cyclic I/O | Acquire a complete logical input image and publish an output image with bounded, reported health | Running user logic or changing images midway through a scan |
| Fault-output actuation | Attempt the configured fault outputs and report unconfirmed/failed actuation | Claiming that a successful API call proves the physical machine is safe |
| Hardware watchdog | Configure the selected supervision mechanism and service it under supervisor policy | Feeding itself on every opcode or from an unconditional timer ISR |
| Program storage | Bounded read/stage/activate operations with interruption recovery | Compiling ST or interpreting configuration semantics |
| Retain storage | Persist and recover a versioned, coherent checkpoint | Deciding which variables are RETAIN or changing restart semantics |
| Diagnostics sink | Record fixed-size events to a bounded sink, with overflow accounting | JSON formatting, web serving, or blocking log output |
| Management transport | Move bounded request/response frames | Mutating live engine state concurrently |

### 7.1 Clock domains, callbacks, and waiting

Keep the existing signed IEC TIME/LTIME and `Duration`/nanosecond semantics. Introducing stronger interface types shall not change timer precision, rounding, or arithmetic. A nonnegative elapsed budget is a different concept from an IEC value that can legitimately be negative.

**PORT-01.** Public execution interfaces shall distinguish logical instants, physical monotonic execution instants, and elapsed spans using separate wrapper types or an equivalently enforceable contract. A logical simulation timestamp shall not be implicitly usable as an execution/watchdog deadline. Declare resolution, counter-extension/wrap behavior, reset epoch, checked deadline arithmetic, and discontinuity handling; timestamps from unrelated epochs shall not be compared as if they share a clock.

Logical time drives deterministic scheduling/timer behavior under a supplied trace. Physical monotonic time measures whether execution and I/O finish within the deployment's actual budget. Real deployment may derive logical time from its monotonic source, but accelerated simulation shall inject logical time without accelerating physical watchdog supervision. Wall-clock/calendar services remain separate optional capabilities. Clock discontinuities shall trigger the selected resynchronization/fault policy, not silently produce negative elapsed control time or an unbounded catch-up loop.

**PORT-02.** Every synchronous callback reachable from a bounded path shall declare maximum work (including size-dependent work), allocation policy, blocking/cancellation behavior, and failure result. Its cost shall be included in admission. A routine without a supportable bound shall use staged service work or be rejected for that profile. Neither a bytecode instruction budget, an async annotation, nor a passed deadline can interrupt arbitrary native code that does not return. [E13, E16]

**PORT-03.** The runner wait mechanism shall support timer expiry, management/service wakeups, and cancellation with a missed-wakeup prevention protocol. A runner shall capture a wake token, check pending work/STOP state, and then atomically arm or conditionally enter the wait only if the token is still current and the deadline remains in the future. A concurrent wake shall make the wait return or avoid sleeping; token wrap/reuse shall not lose pending work. STOP shall not wait for a long idle timer deadline, but waking an idle runner does not preempt an executing PLC batch or a hung callback. Maximum STOP response and independent hang recovery shall be declared separately.

The current hosted `StdClock` uses `thread::sleep` and inherits a no-op `wake`; changing this is a STOP-latency fix, not merely extraction. Implement it with a native idle-STOP/wakeup-race regression and a CHANGELOG entry in the scope that changes the runner. Retain independent supervision for non-returning callbacks. [R33]

**SCHED-05.** Scope A shall correct periodic readiness in the shared core at the start of A1, as a separately reviewed source change sharing A1's one final batch. Correct the core/case/host assertions before that batch and freeze the corrected scheduler contract for A2 onward. A4 shall integrate the same rule into its compiler-free fixture and saved host/MCU oracle. Registration initializes `last_run` to registration time. For positive `interval`, `SINGLE` FALSE and `n = saturating_sub(now, last_run) / interval >= 1`, emit one activation due at the first pending nominal deadline, count `n - 1` missed intervals, and advance `last_run` by `n` whole intervals using saturating multiplication/addition. Thus `last_run` is the latest nominal deadline accounted for, not the sample time; late sampling shall not move later deadlines. Preserve event gating, priority/due/index ordering, backward-time behavior, saturating counts and the no-replay rule. This is a hosted behavior fix carried with the portability implementation, with native core/host regressions and a CHANGELOG entry, not an adapter-specific policy. (IEC 61131-3 Ed.3 §6.8.2(b); specifications 10 §4.3 and 11 §6.2.)

The reviewed pre-A1 `task/readiness.rs` assigns `last_run = now`. With exact 10 ms resource samples, a 25 ms task therefore has a 30 ms period; the corrected activations are 30, 50, 80 and 100 ms, with nominal deadlines 25, 50, 75 and 100 ms. Fixed hardware-timer sampling makes non-multiple task intervals an ordinary F401/C6 case, so the same rule must be carried into both boards. This change does not redesign the resource beat or promise a one-beat bound on OS scheduling or loaded task completion. It has no dependency on portable collections, decoder relocation, STBC 2.0 or executor extraction. A1 implements the correction; its native tests join A1's one consolidated batch, with no fifth scope or separate bug-fix run.

The underlying mechanism may be an OS event, interrupt, RTOS primitive, or suitable executor integration. The core shall not require one framework. A CPU-bound scan shall not rely on cooperative async scheduling to supply preemption it does not implement. [E16]

### 7.2 Process image, not individual GPIO calls

The VM shall read PLC inputs from the latched process image and write intended outputs into the engine's output image. It shall not call a GPIO driver for every `LOAD` or `STORE` opcode.

Board adapters may use GPIO, ADC/DAC, SPI/I2C devices, DMA, or remote-I/O workers below this boundary. Each input channel/group shall have an explicit stale/invalid policy. Drivers shall not silently substitute zero on read failure.

A healthy sample is a coherent logical snapshot. Multiple buses need not sample at the same physical nanosecond; timestamps/quality and the accepted coherence model must be documented. DMA/interrupt producers shall use staging buffers so they cannot modify the image while PLC logic is executing.

**PORT-04.** Each output adapter shall identify what a successful completion report establishes: driver acceptance, completed bus transfer, or device feedback. Timestamps shall use the physical execution clock. Driver acceptance shall not be labeled physical actuator completion. Staged normal outputs shall have explicit age and cancellation/invalidation rules on STOP, FAULT, or generation replacement, so obsolete queued writes cannot silently override the required fault/new-generation output policy. Partial failures and unconfirmed actuation shall remain reportable in bounded diagnostics.

### 7.3 API direction

The following is a proposed interface sketch, not an existing repository API or a complete implementation. Wrapper construction/conversion shall enforce PORT-01; payloads are private to prevent accidental cross-domain conversion. The existing `Duration` supplies the representation, not permission to interchange the three concepts.

```rust
use trust_runtime_core::value::Duration;

#[derive(Clone, Copy, Debug)]
pub struct LogicalInstant(Duration);

#[derive(Clone, Copy, Debug)]
pub struct ExecutionInstant(Duration);

// Construction must reject a negative elapsed budget without changing IEC TIME.
#[derive(Clone, Copy, Debug)]
pub struct TimeSpan(Duration);

#[derive(Clone, Copy, Debug)]
pub struct PortError {
    pub code: u16,
    pub device: u16,
}

#[derive(Clone, Copy, Debug)]
pub enum InputQuality {
    Valid,
    Stale,
    Invalid,
}

#[derive(Clone, Copy, Debug)]
pub enum OutputEvidence {
    DriverAccepted,
    BusTransferCompleted,
    DeviceFeedbackObserved,
}

#[derive(Clone, Copy, Debug)]
pub struct OutputReceipt {
    pub observed_at: ExecutionInstant,
    pub evidence: OutputEvidence,
}

pub trait ExecutionClock {
    fn now(&self) -> ExecutionInstant;
}

pub trait LogicalClock {
    fn now(&self) -> LogicalInstant;
}

pub trait CyclicIo {
    // A deadline is a bound to enforce, not automatic cancellation.
    fn sample_inputs(
        &mut self,
        image: &mut [u8],
        quality: &mut [InputQuality],
        deadline: ExecutionInstant,
    ) -> Result<(), PortError>;

    fn commit_outputs(
        &mut self,
        image: &[u8],
        deadline: ExecutionInstant,
    ) -> Result<OutputReceipt, PortError>;

    fn apply_fault_outputs(
        &mut self,
        image: &[u8],
        deadline: ExecutionInstant,
    ) -> Result<OutputReceipt, PortError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WakeToken(u64);

#[derive(Clone, Copy, Debug)]
pub enum WakeReason {
    Deadline,
    Event,
    Cancelled,
}

// Used by the runner; never by the TON/TOF/TP implementations.
pub trait RunnerWait {
    fn wake_token(&self) -> WakeToken;

    // Implement the check-and-wait contract of PORT-03.
    fn wait_until(
        &mut self,
        deadline: ExecutionInstant,
        observed: WakeToken,
    ) -> Result<WakeReason, PortError>;
}

// The signaling handle shares the wait primitive, not mutable PLC state.
pub trait WakeSignal {
    fn wake(&self);
}
```

The concrete API shall also expose the admitted callback bounds and input/output quality required by the selected driver. The receipt above describes one adapter's available observation; a multi-driver aggregate shall preserve partial failures and shall not promote weaker evidence into a stronger completion claim. The token width in the sketch is not a requirement for native 64-bit atomics on the MCU.

The concrete implementation may adapt the existing `IoDriver` API rather than replacing it immediately. API naming shall not justify breaking existing host integrations.

Use static composition where convenient. Borrowed trait objects are also acceptable where they keep the design simple; using a trait object does not by itself require heap allocation. Avoid propagating platform generics through every value and instruction handler.

### 7.4 The control link on a byte stream

**PORT-05.** A runtime whose control link is a byte stream, such as a chip's USB serial port, carries the control protocol of specification 11 §6.8.1 as trust-runtime's control socket carries it, so every client works over it unchanged (ARCH-12). The framing, the frame bound, the reply handling and the counters below are shared code (ARCH-11); a chip's adapter only moves bytes between its hardware and that code.

- **Line framing.** A request is the bytes before a line feed (`\n`), with one carriage return (`\r`) directly before the line feed removed, as trust-runtime reads its socket. A reply is one JSON object followed by `\n`. An empty line is a request of zero bytes and gets trust-runtime's answer to an empty line, `invalid request: EOF while parsing a value at line 1 column 0` with id 0.
- **Frame size.** A link declares the longest frame it moves, in bytes and without the line end, the same in both directions. Every chip's USB link moves frames of up to 8,192 bytes. trust-runtime's own sockets have no such bound.
- **Oversized request.** A request longer than the frame is dropped together with every byte up to its line feed. It is answered once, in its place among the replies, with `invalid request: request longer than <frame> bytes`, id 0 and no audit record; the requests after it are served as usual. A request too large for one frame travels in parts, as set out in the bullets below.
- **Oversized reply.** No runtime cuts a reply. A request or reply longer than a link's frame shall travel in parts on every runtime, each part within the frame, with the transfer of program loading (§11.2 step 5, DEPLOY-02). A reply that fits the frame is byte for byte the reply trust-runtime writes; a longer reply is held whole, as set out below.
- **Transfer ids.** A transfer id is 16 hexadecimal digits: the boot number in the high 32 bits and a sequence within the boot in the low 32 bits. An id never repeats within a boot or across boots. A runtime holds one open upload at a time: a `transfer.begin` while another is open is refused with `transfer busy: <id>`.
- **Transfer begin.** `transfer.begin {bytes, sha256}` opens an upload of `bytes` bytes whose SHA-256 is the 64-digit `sha256`. A size above the largest artifact the device's descriptor allows is refused before anything is allocated, with `transfer too large: <bytes>, the device takes at most <max>`. Begin reserves the upload's buffer and a part buffer of 4,096 bytes; if the device cannot reserve them, begin is refused with `no room for a transfer of <bytes> bytes`, never aborted. The reply is `{transfer, part_bytes: 4096, next_offset: 0}`.
- **Transfer part.** `transfer.part {transfer, offset, data, crc32}` carries the raw bytes of one part as standard base64 (RFC 4648, padded, with canonical trailing bits) in `data`, and the CRC-32 of those bytes as 8 hexadecimal digits in `crc32`. `offset` shall equal the next offset. Every part is 4,096 bytes except the last, and a full part's 5,464 base64 characters fit the frame. A part at another offset is refused and nothing is stored. A repeat of a received part with the same bytes is acknowledged again; a repeat with other bytes is refused, and the received bytes stay. A part whose CRC-32 does not match its bytes is refused. Data that is not canonical base64, or that decodes to more than a part, is refused. These refusals leave the next offset unchanged. The reply is `{next_offset}`.
- **Transfer completion.** When the last byte arrives, the SHA-256 of the whole upload is compared with the one `transfer.begin` gave. If they differ, the upload is dropped with `transfer sha256 does not match begin's`, and the client begins again. A complete upload stays open until an operation that uses it, or `transfer.abort`, ends it.
- **Transfer status and abort.** `transfer.status {transfer}` replies `{state, next_offset, bytes}`, where `state` is `receiving` or `complete`. `transfer.abort {transfer}` frees the buffer and the id of an open upload or of a held reply. A new host session keeps an open upload at its next offset. A reset ends every transfer, and a later use of its id is refused with `unknown transfer`.
- **Reply head and get.** A reply longer than the frame is held whole in a reply buffer of the capacity the runtime gives it. The request is answered with `{reply_transfer, bytes, sha256, part_bytes: 4096}` as its result, with the request's id and audit record id. The client reads the reply with `transfer.get {transfer, offset, len}` (`len` at most 4,096), which replies `{offset, data, crc32}`, and checks the SHA-256 of the whole reply. A reply above the buffer's capacity is refused with `reply longer than <capacity> bytes`, never cut: the request was served and is audited as served, and the work after the reply, such as the stop after `shutdown`, still takes place. A newer held reply replaces the older one, whose id is then `unknown transfer`.
- **Transfer audit.** Each transfer operation is audited like any request, and its reply carries the audit id.
- **One reply in flight.** The link sends one reply at a time. While a reply is being sent the link gives the control loop no further request, so the loop ends its pass and waits until the link wakes it, and while the link still holds reply bytes it has not handed to its hardware the loop waits no longer than a millisecond, so a reply in flight is driven to its end within a millisecond per packet without depending on the link's interrupts; a busy link never delays a due cycle (STATE-08). Bytes that arrive while the receive buffer is full stay with the hardware, which stops accepting more (USB flow control); no byte is lost.
- **A host that stops reading.** A reply that has made no progress for at least 1 second, as the loop sees when it polls the link, is dropped and counted, and the link takes requests again. Bytes of it that the hardware already holds cannot be withdrawn, so the next reply starts with an extra line feed: the stale fragment then ends as a line that is not a JSON reply, and the next reply is whole.
- **A new host session**, such as a USB bus reset, drops the reply being sent and any partly received request, so a new session never receives an old reply. A reply dropped part-way is repaired as above.
- **Counters.** The link counts the requests it handed to the loop, the oversized requests, the replies sent, the replies dropped by the stall rule and by a new session, the replies it refused, the times its receive buffer was full and the host sessions. The operation that reports them is decided with the runtime descriptor (COMPAT-02).

The bullets below are the rules of the client, the host side of this link. The shared control client (`trust-runtime-client`) follows them for a `serial://` endpoint, so `trust-runtime ctl`, `ads` and `discover` and the tools built on it reach a chip as they reach a trust-runtime (specification 11 §6.8.1).

- **The client: opening the port.** A client opens a chip's serial port at 115,200 baud with 8 data bits, no parity, one stop bit and no flow control; a USB serial port ignores these values. Opening a port can reset a chip: the ESP32-C6's USB-Serial/JTAG controller resets the chip on the sequence of DTR and RTS levels that its boot loader's flash tool uses (Appendix D.2). Right after the open the client sets DTR on and then RTS off, as the bench does, and it changes no modem line after that and sends no break. Then it discards the bytes that the operating system holds for the port from before the open, which belong to an earlier session. A setting that fails is reported, never ignored.
- **The client: the frame.** A request is one line: the JSON object, then a line feed. The client never writes a request longer than the link's frame, which is 8,192 bytes on a chip's USB link: it refuses the request before it writes any byte, with the text that the chip itself gives for it, `invalid request: request longer than 8192 bytes`. A request that carries more data goes in parts (`transfer.part`). A reply line longer than the frame is malformed; the client reads no more than the frame and its line end for a reply line.
- **The client: one request in flight.** The client sends the next request only after it has read the reply to the previous one. Of the lines it reads, it takes the first that is a JSON object as the reply and skips every other, so that text on the port does not end a request: an empty line (the line-feed repair after a dropped reply), the boot loader's lines after a reset and the stale fragment of a reply that was dropped part-way. A line that holds `{"id":` after other text is read from that point, because the boot loader's last line can lack its line end. A client that has skipped 65,536 bytes this way without finding a reply reports that the port does not speak the control protocol. The reply's `id` must be the request's, or absent, as on a socket.
- **The client: the wait.** The command line (specification 11 §6.8.1) waits at most 5 seconds in each read for a reply, and at most 120 seconds for the reply to `program.install`, which the chip sends after it has committed the container (§5.1). A wait that runs out is reported as a failure of the link that names the wait, never as a reply. After it the client may send `transfer.part` again, because the repeat of a received part is acknowledged; it sends no other request again, because a wait that ran out does not say whether the chip served the request.
- **The client: a reply longer than the frame.** A reply whose result is an object with a `reply_transfer` member is a reply head. The client reads the held reply with `transfer.get {transfer, offset, len}` from offset 0 on, `len` 4,096 (the last part is the rest), and checks each part: its `offset` is the one asked for, its `data` is canonical base64 of exactly the `len` bytes asked for (the reply ends at the head's `bytes`), and its `crc32` is the CRC-32 of its bytes. It then checks that the parts add up to the head's `bytes` and that their SHA-256 is the head's `sha256`. When all of it holds, the whole reply is the answer, handed on as if it had come in one line, and its `id` must be the request's. A head is followed once: a reply that a head names is never itself a head. When anything fails (a refused `transfer.get`, a part that is short, long or at another offset, a CRC-32 or SHA-256 that differs, a head whose `bytes` are above the caller's limit for a reply line), the client sends `transfer.abort {transfer}` to free the reply buffer, ignores that abort's own failure, and reports the first failure. It never hands on a part of a reply, or a reply whose SHA-256 differs.
- **The client: a link that goes away.** A read or write on the port that fails for any reason but the wait running out, and a read that returns no bytes, mean that the port went away: the chip reset (a watchdog, a panic or the reset button), lost power or was unplugged, and its USB device is enumerated again, often under another port name. The client ends the request with `serial port <name> went away (<the operating system's reason>): the chip reset, lost power or was unplugged; its port returns when the chip has started again, so run the command again`. It neither opens the port again nor sends the request again.

## 8. Scan-cycle and scheduling contract

### 8.1 Common cycle phases

The common cycle controller shall define and enforce the phase order. Adapters shall not each invent their own PLC scan algorithm.

```text
apply authorized commands at the permitted boundary
        |
observe logical resource time / determine boundary
        |
read all configured inputs into staging; assess validity
        |
latch input image and synchronize bindings
        |
collect ready tasks; preserve priority and tie-breaking rules
        |
execute ready programs and admitted background work
        |
complete bounded bookkeeping; verify execution/commit budgets
        |
publish output/marker bindings and perform output commit phase
        |
record cycle outcome and supervisor progress
        |
run bounded non-control work / wait for next deadline
```

**CYCLE-01.** Within each cooperative resource batch, all participating input reads shall precede submission of that batch's normal outputs. The input snapshot shall remain stable during execution. Previously queued hardware transfers shall follow the adapter's admitted I/O contract and shall not be confused with a new logical output publication.

**CYCLE-02.** The batch's normal output image shall be submitted only after all required ready work completes successfully and output-publication policy permits it. A failed execution shall not publish partially computed normal outputs. A fast task finishing its computation does not authorize an early independent output commit in cooperative-resource-v1; batch delay and actual adapter completion shall satisfy SCHED-03.

**CYCLE-03.** Priority ordering, equal-priority/due-time ordering, event-edge treatment and background execution shall preserve the existing regression contract under cooperative-resource-v1. Periodic work shall use SCHED-05's nominal-deadline baseline: a late sample does not move later deadlines, and a jump over `n` intervals emits one activation, counts `n - 1` missed activations and never replays them. Ready work is collected at the defined boundary; new releases during a nonpreemptive batch follow these next-readiness/missed-period rules. They shall not be silently preempted, replayed, or given a per-task commit by a platform adapter.

**CYCLE-04.** The selected MCU profile shall not require a different implementation of TON/TOF/TP, arithmetic, or reference semantics. Different timer/peripheral mechanisms are allowed below the clock interface.

**CYCLE-05.** `%I`, `%Q`, and `%M` binding behavior shall be preserved. Any intentional change in when memory bindings, forces, or debugger writes are applied shall be separately specified and tested.

**CYCLE-06.** No execution thread, interrupt handler, communications worker, or debugger shall concurrently mutate engine-owned PLC state. External producers shall use bounded queues or staging images applied at defined boundaries.

The existing code already pins lower numeric priority first and dropping missed periodic intervals rather than replaying all missed executions. These remain regression requirements. **Separate semantic decision, 9 October 2026:** SCHED-05 replaces the sampled-time periodic baseline with nominal-deadline advancement, following IEC 61131-3 Ed.3 §6.8.2(b) and the amended specifications 10/11. Preserve the previous behavior only as a documented defect baseline; it shall not be frozen into `cooperative-resource-v1` or its host/F401/C6 oracle. No other ordering, event, output-publication or missed-activation policy changes with this correction. [R13]

### 8.2 Fault and transaction boundaries

“Output commit” means the engine's logical publication phase. It does not imply physically simultaneous writes across independent drivers or buses.

If the second of several output drivers fails, the first may already have acted. The supervisor shall report partial/unconfirmed actuation, attempt the configured fault response for all applicable drivers, and not claim that outputs were atomically rolled back.

Likewise, aborting a scan does not automatically restore all mutated PLC variables or FB state. The first MCU profile shall enter a faulted state and require the defined restart procedure rather than resuming a partly executed scan. Hosted fault/recovery behavior shall remain subject to its existing compatibility contract and any separately approved policy changes. Full application-state rollback is not part of this design.

### 8.3 Time and work limits

**TIME-01.** Each admitted task/resource batch shall have bounded work, computation/output requirements, and an elapsed-time supervision policy. Instruction budgets cover VM work but are not execution-time bounds or a scheduling proof. Admission shall apply SCHED-01 through SCHED-04 to the full workload and selected evidence classification. [R16]

**TIME-02.** Budgets shall cover the whole nested execution and variable-cost operations, including string work, array copies, local/instance initialization, reference traversal, and native imports. A single opcode shall not perform unaccounted unbounded work. The cost model shall state maximum admitted sizes, enforcement/check frequency, and the basis of any time bound; empirical costs shall not be relabeled proven bounds.

**TIME-03.** Supervision shall cover task computation, the cooperative output batch, and the required adapter completion level. Declare the maximum interval between checks and the maximum STOP/fault response, including callback blocking. If an import/driver fails to return, VM checks cannot supply cancellation; the platform shall provide the admitted independent supervision/recovery mechanism or reject that requirement. Detecting and faulting an overrun is a protection result, not successful completion of the missed control deadline.

The reviewed stack dispatcher checks its elapsed deadline at entry and each 32 original instructions. This is a baseline implementation detail, not a time bound or a bound on a native import. Preserve or explicitly revise the stride, include initializer work, and separately inventory the optimized tier's checks. The current runner's same-thread software watchdog cannot recover a hang in that thread. [R33]

**TIME-04.** The MCU controller shall provide the required hardware-watchdog recovery path for hangs. Hosted deployments shall declare available supervision and enforce the application's required capabilities; a software monitor shall not be represented as an independent hardware watchdog. Hardware-watchdog servicing shall depend on supervisor progress and permitted controller state, not unconditional ISR activity. Healthy STOP/idle operation needs its own liveness policy so stopping the PLC does not accidentally cause a reset loop.

**TIME-05.** Measured maximum execution time shall be labeled as observed timing. It shall not be presented as a mathematical worst-case execution-time guarantee.

**TIME-06.** A bounded core alone shall not qualify Linux, Windows, macOS, or an MCU configuration as hard real time. Qualification shall distinguish release/wakeup lateness, task/batch execution, logical output publication, driver acceptance, bus transfer, and any feedback deadline. OS scheduling, memory residency, CPU/cache/memory interference, interrupt load, and I/O remain part of the target envelope. Only report completion levels actually observed. [E19–E21]

**TIME-07.** Logical-time conformance tests shall use explicit injected logical instants rather than wall-clock sleep precision. Physical-time tests shall use the separate execution clock and the deployment's timing classification, including late wakeups, overruns, timer wrap/discontinuity, and service-event wake races. Correct faulting shall be distinguishable from semantic failure and from meeting the original deadline.

**TIME-08. Work-unit cost model.** The work allowance measures what the PLC
program does, not how the engine does it. It implements TIME-02 as follows:

Text sizes for work are counted in UTF-8 bytes (B, the stored length of the whole text);
a byte count is an upper bound of the character count. The allocation of a copy is counted
by the copy allocation rule below the table.

| Work | Charge |
|---|---|
| Executed original bytecode instruction | 1 unit, identical in the stack, register and tier-1 executors |
| Copying an array | one rule for arrays and structures: 1 unit for each array and each structure in the copied value, plus 1 for each of its elements or fields, at every nesting level; each leaf then pays its own row's charge below; allocation as in the copy allocation rule |
| Copying a reference value | 1 unit plus 1 per path segment and 1 per subscript; creating a reference from a static address charges only its segments and subscripts (its instruction is the base unit); allocation as in the copy allocation rule |
| Extending a reference by a dynamic index or field | 1 unit and one cell (16 bytes) of allocation per added segment and per added subscript (a new index segment with its subscript adds 2 units and 32 bytes), whatever the capacity of the path's storage |
| Copying a WSTRING or an enumeration value | 1 unit; allocation as in the copy allocation rule |
| Copying a STRING value | nothing: its bytes are charged where it is stored (normalization), and its allocation by the copy allocation rule |
| Copying a null reference or an instance identity | nothing, and no allocation: both are fixed-size |
| Binding a VAR_IN_OUT argument | nothing beyond the call's instructions: IEC passes it by reference, and the engine's private snapshot and copy-back are call-transaction bookkeeping |
| Copying a structure | the same rule as copying an array: 1 unit for each array and each structure in the copied value, plus 1 for each of its fields or elements, at every nesting level; each leaf then pays its own row's charge below; allocation as in the copy allocation rule, which counts each copied leaf field. A copy-on-write detach of shared storage charges nothing, because a field write copies no structure in the program |
| Each entry of a retained-value map (a snapshot entry keyed by its name, or a copied instance variable) | the copy of its value, the bytes of its name where it has one, and four cells (64 bytes) of entry allocation |
| Normalizing a STRING/WSTRING as it is stored | 2 units per byte of the value, truncated or not; every STRING/WSTRING is bounded, 80 without a declared length; a cut allocates its kept prefix by the copy allocation rule (its UTF-8 bytes for a STRING, two bytes per UTF-16 code unit for a WSTRING), and a rebuilt structure charges nothing beyond the text rows, its copy having been charged at the load |
| Checking that a bounded text value is canonical (construction and retain checks, a structure's canonical pass) | 1 unit per byte |
| Reading one character, `s[i]` | in a program, 2 units per byte of the whole text after the parent value's copy; through a host cycle access, 1 unit per byte |
| Creating a reference to a dynamic character index | 1 unit per byte of the text |
| Writing one character | 3 units per byte of the whole text plus 12, and 16 allocation bytes per byte plus 64, before the commit |
| Coercing a supplied initializer value | 1 unit per constructed node, plus 1 per byte of a STRING/WSTRING and 4 for a CHAR/WCHAR; a bounded text value then pays its canonical check |
| Copying a type name for an interface attempt | 1 unit per byte of the name |
| Standard-library or conversion call with STRING/WSTRING arguments | each argument is copied by its own row first; then 1 unit per UTF-8 byte of those arguments before the body runs; a text result is then charged as a new value, 1 unit and its allocation by the copy allocation rule (its UTF-8 bytes for a STRING, two bytes per UTF-16 code unit for a WSTRING) |
| Write through a `STRUCT OVERLAP` physical layout | the layout write cost below, before any backing byte changes |
| Lifetime check of a value that can hold references or instances | 1 unit per node visited |
| Constructing a value during construction or initialization | 1 unit per constructed node |
| `SIZEOF` of a value | 1 unit per value node visited, plus 1 per byte of text and 1 per array dimension; an array visits its first element only |
| `SIZEOF` of a type | 1 unit plus its nesting depth per type node visited, an alias and its target each being a node, plus 1 per array dimension |
| Other native and standard-library calls | their argument binding as above; a body without text charges nothing beyond the call |
| Any other value operation on a scalar, including copying one | nothing beyond its instruction, and no allocation |

**Copy allocation.** A copy's allocation depends only on the value copied, never on how the
engine shares or stores it, so a copy is charged the same whether its storage is shared or not.
A cell is 16 bytes (MEM-10). The rule is:

- a scalar value, an enumeration value, a null reference or an instance identity allocates
  nothing;
- a STRING allocates 1 byte per UTF-8 byte of its text; a WSTRING allocates 2 bytes per UTF-16
  code unit of its text, which is its stored form;
- a reference allocates one cell per path segment and one cell per subscript;
- a structure or an array allocates one cell for each copied leaf field or element, counted
  through nested structures and arrays, where a leaf is a field or element that is neither a
  structure nor an array. A leaf also allocates its own copy as above, so a STRING leaf allocates
  one cell and its bytes.

**Worked example.** The structure STRUCT{a: INT; b: STRUCT{c: DINT; d: BOOL}; e: ARRAY[1..3] OF
INT} copies three structures and arrays (the outer structure, b and e) and eight fields and
elements (a, b, e, c, d and the three elements of e): 3 + 8 = 11 units of work. Its leaves are a,
c, d and the three elements of e, six leaves, so it allocates 6 x 16 = 96 bytes. The array
ARRAY[1..2] OF ARRAY[1..3] OF INT costs 3 + 8 = 11 units by the same rule. A copy of an INT costs
nothing and allocates nothing.

**`STRUCT OVERLAP` write cost.** Let w be a path's work (segments, plus the bytes of
its field names, plus its subscripts), P the bytes spanned by the selected bit
range, L the bit length of the larger of the layout's leaf and child counts (at
least 1), and M the larger of the layout's maximum path work and w of the written
path. A write charges 8·L·M + P units; plus, for each selected leaf, 192 + 4·w of
that leaf's own path (and 4 per enumeration member); for each affected leaf, every
leaf whose bits intersect the write including aliases, 192 + its w (and 2 per
enumeration member); for each selected child, its bit count plus 2·its w; for each
affected child, its overlapping bit count plus its w. A write of the whole structure, or of children,
also charges the selected bit span, and the whole structure once more with its
field count. Its allocation is P bytes plus one cell (16 bytes) per affected leaf entry
(and, per affected enumeration leaf, one more cell for the enumeration value, plus the bytes of
its longest member and type names). A write that selects no leaf or child, or a whole-structure
assignment from a value sharing the same layout, charges no layout cost. These
are upper bounds of the writer's work, so a write never runs uncharged.

Engine bookkeeping is part of the instruction's unit and is never charged
separately: index, name and member lookups; owner, root and reference-candidate
resolution; path-segment resolution; writability, staging and visibility checks;
lifecycle markers; copy-on-write detach of shared structure storage; call-transaction snapshots (VAR_IN_OUT copy-in and rollback
journal values); assignment normalization passes other than the text rows above;
canonical field matching; activation-registry records; type-structure comparisons;
retained-value migration indexes, journals and event vectors, and retained identity and root
maps; process-image sampling and publication (a sampled bounded STRING/WSTRING value still
pays its normalization); and scheduler ordering. Each is bounded by admitted metadata, by
the admitted type of the value it handles, or by a value already charged when it
was produced, so its cost per instruction stays bounded. A call-transaction
snapshot lives only until its call commits or rolls back, so its memory is
bounded by call depth and the admitted parameter types; it is added neither to
the work allowance nor to the cumulative allocation charge. Scope C
(MEM-06/MEM-11) bounds compound snapshots further with typed storage. A lifetime
check of a scalar, or of an array whose elements are primitive scalars, visits
nothing that can hold a reference and is skipped.

The charged total therefore depends only on the instructions executed and the
sizes of the values they copy or traverse. Changing a cache, index, journal or
other internal structure shall not change a program's charged work or the point
at which it is refused, and the stack, register and tier-1 executors charge the
same total for the same execution.

**TIME-09. Construction allowance.** Program construction, cold and warm restart
construction, and candidate construction are preparation steps under LOAD-09.
Each runs as one execution entry under `PreparationLimits::max_construction_work`
(hosted default 64 Mi units, equal to the default preparation work bound). Every
top-level program or task function-block invocation in a scan, and every
engineering write, is one entry under `max_work` (hosted default 1,000,000 units),
as specification 10 states for a top-level VM invocation. A profile whose
`max_work` or `max_construction_work` is zero is refused at preparation as
unsupported. Nested calls and
call-time initializers share their invocation's allowance; process-image sampling
and publication around the invocations share the scan's own allowance. An MCU
profile states both values explicitly.
Construction work is therefore bounded by the preparation profile rather than by
the per-scan allowance; raising `max_work` remains no substitute for removing
whole-state work during RUN.

### 8.4 Blocking-aware admission and output response

A scheduling/resource admission record shall bind the following to the artifact/configuration identity: resource periods and release phases; priorities and tie rules; event-arrival/burst assumptions consistent with the sampled-edge contract; background work; input freshness/coherence requirements; per-task and per-batch costs; import/driver bounds; computation deadlines; output-group producer/writer rules; required completion evidence; and the engine/adapter/OS/board/load envelope. Shared output writers shall retain the existing ordered-write semantics; timing analysis shall not assume an earlier writer's value survives later ready work.

The report shall distinguish at least these observations:

| Quantity | Meaning |
|---|---|
| Release lateness | Time from scheduled release to the runner's opportunity to service it. |
| Task execution duration | Time spent executing the task, with nested calls and synchronous work accounted for. |
| Computation response | Time from required release to completed computation, including waiting/blocking. |
| Batch publication response | Time from the applicable release to submission of the completed resource output image. |
| Transfer/feedback response | Time to the declared bus-transfer or device-feedback observation, when available and required. |

Waiting caused by the currently running task/batch and later work in the same output batch shall be included. IRQ/OS interference and admitted I/O delays shall not be excluded merely because they are outside VM instructions. A completion deadline expressed at device feedback cannot be qualified by measuring only driver acceptance.

**Illustrative rejection case, not a truST measurement:** a 1 ms-period fast task taking 0.1 ms and a 100 ms-period slow task taking 6 ms use 16% average computation capacity (`0.1/1 + 6/100`). Nevertheless, the slow task can block a new fast release for several milliseconds. When both are in one batch, executing fast first can still delay its output submission until approximately 6.1 ms plus other overhead. This configuration cannot support a 1 ms fast-output response requirement under this cooperative profile merely because utilization is low.

Acceptance shall record the analysis method or measured envelope, assumptions, raw evidence, and margin policy. A violated or unsupported required bound shall reject that claim under SCHED-04. Timing-affecting changes to code, preparation strategy, task configuration, imports, toolchain, clock/cache settings, platform, or load envelope shall invalidate the applicable evidence until reassessed. This evidence binding does not require an on-device scheduling solver, but the installer shall not reuse evidence for a mismatched deployment.

An analytical argument is optional unless required by the deployment. Where only workload measurements are available, mark the result measured-qualified and retain supervision; do not manufacture analytical assurance from a long test or an arbitrary percentage margin.

## 9. Faults, state transitions, and output supervision

Use a state machine mapped to the repository's existing runtime/resource states. The following names are conceptual; do not break existing public state names just to adopt them.

```text
BOOT -> INITIALIZING -> STOPPED/NO_PROGRAM
                             |
                        valid prepare
                             v
                         READY/STOPPED
                             |
                      authorized START
                             v
                            RUN
                       /           \
                  STOP request     fault
                     |               |
                  STOPPED          FAULTED
                                     |
                          acknowledged restart/recovery
```

**STATE-01.** The BSP shall establish configured startup output behavior before entering ordinary runtime execution. Start shall be rejected until the program is admitted, storage prepared, mandatory I/O ready, and the required supervision available.

**STATE-02.** STOP and FAULT shall have explicit output policies. The MCU release shall not silently treat an empty fault-output configuration as a verified safe condition. Unmapped or uncontrolled outputs must be explicitly identified before START.

**STATE-03.** VM traps, critical input failures, missed deadlines, invalid mandatory state, and output failures shall follow explicit profile policy. The default MCU control policy shall fault on failures that invalidate control, rather than silently continue with invented data.

**STATE-04.** A fault shall latch a bounded diagnostic record, suppress further normal output commits, request the configured fault outputs, and report whether actuation was confirmed by the adapter's available evidence. All applicable drivers shall be attempted even if an earlier one fails, preserving the intent of the existing I/O fault path. [R17]

**STATE-05.** Panic/reset behavior shall be defined by the firmware and board. Merely configuring `panic = "abort"` is not an output-safety mechanism. Reset pin states, watchdog behavior, and external output enables must be considered together.

**STATE-06.** Recovery shall not automatically resume an interrupted instruction stream. The configured cold/warm initialization and retain policy shall run before control resumes. Automatic restart, where offered, shall be a deliberate product setting.

**STATE-07.** Writing, forcing, breakpoints, pause and stepping behave on every target as in trust-runtime (specification 11, debug and resource pause interaction; specification 13, run control), with the same access rights. A pause suspends the current cycle: the outputs keep their last committed values, and the pause does not count against the cycle watchdog. A hardware debug probe (JTAG/SWD) halts the whole processor; it is not a PLC breakpoint, and it must not be assumed to preserve watchdog or output supervision.

These behaviours need no ST source text and no compiler on the target. Breakpoints and stepping resolve from the module's `LINE_TABLE` (specification 12 §6.13; specification 13, "Source Mapping"), so trust-runtime and a chip return the same answers for the same artifact. A debugger hold lasts until a continue or a stop on every target (specification 11 §6.9). `eval`, `debug.stack`, `debug.variables`, watches, breakpoint conditions, logpoints and `debug.evaluate` answer as trust-runtime answers them, from the published debug state and through the compiled expression form (specification 11 §6.13.1 and §6.13.2), which keeps the ST parser, the HIR and expression authoring off the target (owner, 18 October 2026).

**STATE-08.** Stop and Start on every runtime (owner, 16 October 2026: "on a chip, Stop and Start are the existing `shutdown` and `restart`"; owner, 18 October 2026, decision Q11: a computer behaves like a chip when it is stopped, as CODESYS and TwinCAT keep their runtime up while the application is stopped). `shutdown` stops the program as trust-runtime stops it (outputs to their stop policy, STATE-02, and the durable retain save), and the runtime stays reachable in the Stopped state: a chip serves the control connection, and trust-runtime on a computer keeps its control server, its web UI and its other services. `restart` with `cold` or `warm` then starts the installed program as a new trust-runtime process would: its logical time and uptime start at zero, and its events, metrics and task statistics start empty. The process of trust-runtime on a computer ends only when its service is stopped (SIGINT or SIGTERM, specification 11 §6.8), by the explicit exit, `shutdown` with `{"exit": true}`, or by the hang action `abort` (specification 11 §6.6). The explicit exit is the Admin operation `shutdown`: it stops the program as `shutdown` does, writes the reply `{"status":"stopping"}` first, and then ends the process, with status 0 when the resource ends Stopped and status 1 when it ends Faulted (specification 11 §6.6). `exit` must be a Boolean, and `false` is plain `shutdown`; any other value is refused with `invalid params: exit must be a boolean` and stops nothing. A chip's firmware never ends, so the program takes the place of the process, and a chip answers the explicit exit as `shutdown`: it stops the program, replies `stopping` and stays reachable in the Stopped state. A `warm` start gives the RETAIN variables the values they had when the program stopped (IEC 61131-3 Ed.3 §6.5.6.1: "the values the variables had when the resource or configuration was stopped"): a chip still holds them in its memory, and trust-runtime on a computer restores them from its retain store (specification 11 §6.7), because it builds the program anew. Power-on and reset start the installed program as a warm restart when its project keeps retained values (`[runtime.retain] mode = "file"`), so the retained values are restored (IEC 61131-3 Ed.3 §6.5.6.1, "the values the variables had when the resource or configuration was stopped"), as a PLC starts after a power loss and as a computer's service starts trust-runtime with `--restart warm`; with `mode = "none"` the start is cold. Its logical time starts when the program starts, not at the chip's reset. Every other request answers as trust-runtime answers it in the same state; a `restart` of a program that runs, is paused or is faulted restarts it in place as trust-runtime does, and its records stay. A stop drops the restart and the pause that were queued before it, and a `restart` after it starts the program. A `shutdown` of a resource that is already stopped, or that a fault has stopped, stops nothing more and replies `stopping`: the faulted resource stays Faulted, as a chip's does. On a computer, a `restart` that finds the resource Stopped loads the installed program again, as a new process loads it: the bundle that the project folder names, which is the bundle `current` names when the folder is a store's `current` link and otherwise the folder's own files (§11.2). The resource's own configuration is read with it: the I/O configuration and the safe state, the retain, watchdog and fault settings, the cycle interval, the connection configuration of the ADS and OPC UA clients and `simulation.toml`. The program that stopped, with its I/O drivers, is dropped before the new one is built, so that a driver that holds a device alone, such as a serial port or a GPIO line, is released first. The services that belong to the process keep the configuration they started with until the process starts again: the control endpoint and its credential, the web, discovery and mesh listeners, the OPC UA and ADS servers (an ADS server keeps publishing the symbols of the program loaded at launch), the historian, the debugger's source registry and the HMI descriptor. A program that cannot start, because its bundle cannot be read or one of its drivers cannot be built, leaves the resource Faulted with the reason, as a chip's program that cannot start does; the next `restart` loads it again. A program that an online change (§11.3) gave the resource and that `program.install` did not install is not the installed program, so a stop and a `restart` replace it with the installed one. One control loop serves the link between cycles and while Stopped; it is shared code (ARCH-11), and only its wait and wake come from the platform adapter. A request that arrives while the program's next boundary is due waits for that boundary, so a due cycle waits at most for the request already being answered, and a boundary that is due again as soon as it is taken (a scan longer than its interval) lets one request through per boundary, so the link is never starved.

Requests take effect where trust-runtime's scan thread takes them. The loop answers each request at once; a `pause`, `resume`, `restart` or `shutdown` then waits for the program's next boundary, as trust-runtime's scan thread takes it at its next pass: while the program runs, when its next cycle is due, one cycle interval after the last cycle started; while it is paused, one cycle interval after the last boundary; while it is faulted, at the end of each 10 ms poll of the faulted wait. At a boundary the stop is taken first, then the queued pauses and resumes in order, then a pending restart, and then the cycle; at a poll of the faulted wait a pending restart is taken before the stop. Pauses and resumes wait in a queue of trust-runtime's size, and one beyond it is refused with `resource_busy`. Only these differences remain: the device serves requests between cycles, never during one, so a due cycle can wait for the one request being answered; trust-runtime starts each cycle later than it is due by its operating system's wake-up latency; and a `restart` of a stopped program takes effect at once on the device, where trust-runtime's scan thread takes it at the end of its next poll of the stopped wait, within 10 ms, the poll of the faulted wait. The loop feeds the platform's hardware watchdog once in every pass and waits no longer than the watchdog's feed interval, in every state, so a stopped, paused or faulted chip is never reset by it (TIME-04).

A chip whose application region holds no program, or holds one that the chip refuses (DEPLOY-04; for example a program that calls `CURRENT_DT` on a chip without a wall clock, §5.2), runs the same loop without a program: it serves the control connection, `status` reports `stopped` without a fault, and every request answers as in the Stopped state, except that `restart` is refused with the stable code `resource_not_running` and the message `no installed program: <reason>`, which names why the chip has none. A program deployed to it (§11.2) is then started by `restart` as above.

## 10. Retain and persistence

The core shall continue to decide which state is retained and how cold/warm restart behaves. The storage adapter shall decide how bytes are reliably stored on the chosen medium.

**RETAIN-01.** Checkpoints shall be coherent snapshots with a format version, application/schema identity, checkpoint sequence/generation, integrity check, and completion indication. Record the applicable execution-generation identity where needed for compatibility, and do not confuse checkpoint sequence numbers with execution-generation lifetime tokens. Native pointers or raw Rust struct layout shall not be the durable format.

**RETAIN-02.** A restart shall load only a complete, valid, compatible checkpoint. An interrupted write shall not replace the last known-good checkpoint with a partially written one.

An in-process restart preserves the sampled logical resource time before executing
replacement initializers; it does not expose a temporary zero-time epoch. Task
registration uses that same sample as its new nominal-deadline baseline. A failed
replacement leaves the installed state and its clock unchanged. This is a product
lifecycle contract, distinct from restoring time after a power cycle.
Both warm and cold in-process restart stage the previous flat and hierarchical
process images before executing replacement initialization. Ordered configuration
actions then overwrite only their selected targets, including direct `%I`, `%Q`
and `%M` addresses. Unconfigured bytes, bits and hierarchical keys retain their
previous values; repeated or overlapping actions follow the initializer table's
order. A variable located at `%M` then publishes its initial or retained value
into exactly the bits and bytes it covers, so where an action and such a variable
cover the same byte, the variable's value is the one the first scan reads
(specification 11 §6.7). Image carry-over must not overwrite an initializer's
result after construction.
An initialization or budget failure leaves the complete installed state, images
and logical time unchanged. This specifies the portable image lifecycle alongside
the variable-retention rules of IEC 61131-3 §6.5.6; it does not change hosted 1.1
restart behavior or authorize input writes during PLC execution.
Warm in-process restart transfers retained global object graphs into replacement
storage, preserving aliasing and cycles while remapping instance and global
references to replacement identities. Owning POU-typed links retain object state; interface-typed links (including
aggregate members) are nonowning bindings, like references. A reference alone to a non-retained artifact root preserves its binding but
observes that root's reset state. Retained resource-variable globals are visible to replacement
initializers. Function statics retain their distinct AfterRestart initialization
policy; RETAIN does not turn a static into a retained resource-global root. Program-local retained scalar/aggregate values and edge phases
keep the existing hosted restoration order and retainability policy. Live
activation-local references cannot cross restart. Graph transfer consumes the
same admitted construction/work limits and commits only with successful full
replacement; this is not a durable serialization promise for live handles.

**RETAIN-03.** The product shall document its persistence guarantee: periodic checkpoint, stop checkpoint, or a stronger hardware-backed mechanism. “RETAIN supported” shall not imply that every last scan survives arbitrary power loss.

**RETAIN-04.** Flash erase/programming and filesystem/database work shall not introduce unbounded blocking in MCU or hosted bounded control cycles. Generate or schedule a bounded checkpoint and persist outside the critical path, or require STOP when the selected storage mechanism cannot provide the needed bound. Durable-acknowledgment requirements shall be preserved or explicitly changed through the policy review below.

**RETAIN-05.** Snapshot copying, background persistence, retry work, and queue capacity shall be bounded. Some platforms can stall code execution during flash operations; those operations shall remain prohibited in RUN until verified for the selected memory layout.

**Selected persistence profiles.** The initial F401 profile checkpoints only after outputs enter the controlled STOP/maintenance state; no internal-flash erase/program runs during RUN. Its power-loss guarantee is recovery of the last successfully completed compatible stop checkpoint, not the most recent scan. A stronger RUN-time retention guarantee requires a separately designed medium/power-fail mechanism and qualified timing. Ordinary stop checkpoints do not inherently require external storage. [E61]

For hosted bounded operation, capture a coherent checkpoint in admitted preallocated storage, publish it without waiting for disk, and persist on the storage worker. One shared capture serves a chip and a computer (`trust-runtime-core::retain::capture::RetainCapture`): it holds the save interval, the dirty mark and the unchanged-image skip that decide when a checkpoint is taken, and the staging image a platform refills in place at the established pre-output-sample checkpoint is reserved once, so a steady-state capture allocates nothing. Use a bounded latest-pending-snapshot mailbox: a pending snapshot may be superseded before worker ownership, while an in-progress/durable snapshot is never mutated. Define capacity/ownership transfer before implementation. Staging exhaustion faults before that cycle's normal output commit; a worker persistence failure is latched and faults at the next control boundary under the normal fault/output policy. Neither condition blocks the scan on storage. Expose the last durably completed sequence and failure state. A later persistence failure cannot retroactively cancel already published outputs. The durability guarantee is the last completed flush; queued/coalesced scans are not durable. This is the selected M3 bounded-profile behavior change, with restart, failure, output-order, and overload tests. Keep legacy synchronous behavior in the compatibility profile until that migration is explicitly released. [R12, R33]

The reviewed hosted cycle can call `maybe_save_retain_store()` before normal output writes. The initial extraction preserves this behavior in hosted compatibility compositions. The bounded profile above deliberately changes failure/acknowledgment timing and needs its own release notes and tests; it is not a behavior-preserving file move. A compatibility build retaining unbounded persistence shall not claim bounded-profile compliance. [R12]

**Between scans.** A chip drives the shared capture and the checkpoint log through a persister,
`retain::checkpoint::CheckpointPersister` in `trust-runtime-core`. At the pre-output-sample capture
checkpoint it starts a checkpoint when the shared save rule says a save is due (specification 11
§6.7) and the staged image differs from the image it last wrote; the capture's reserved staging
buffer is refilled in place and the persister copies the staged image into a payload buffer reserved
once for the admitted retain image, so starting a save allocates nothing. A checkpoint that is still
being written is never started twice: a newer due image waits until the pending checkpoint commits
and is then offered again, so no save is lost. The persister advances a checkpoint by one log step
at a time — one erase unit, or at most the number of program bytes the caller's idle time allows —
between scans, at most one step per call. A program step is sized to the idle time; an erase unit
cannot be split, so on a flash whose erase takes longer than the time left before the next release
(a C6's external flash erases a 4 KiB sector in up to about 0.4 s with interrupts off) the next scan
starts late, and that lateness is reported like that of any late scan. A step that fails ends
that checkpoint without committing it and latches the failure; the caller takes it once as
`RuntimeError::RetainStore` and fails its next scan with it before physical outputs, while the newest
committed checkpoint stays readable. A stop or shutdown completes the pending checkpoint and then
writes the newest staged image if it was not written, all steps at once, so the save completes before
the runtime is Stopped.

**The device runner.** The installed program's cycle holds this persister over the checkpoint store
its platform supplies: at each scan's pre-output-sample checkpoint it captures the retained values
and offers them, the resource loop advances one step between scans with the store's per-step program
budget, and a stop or shutdown completes the pending checkpoint and the newest image before the
resource reports Stopped. No step runs inside a scan, and a failure a step or a start latched fails
the next scan with `RuntimeError::RetainStore` before physical outputs, as a computer's failed retain
save does. A warm start decodes the newest committed checkpoint with the shared codec and applies it
by name (specification 11 §6.7); an empty log or a `mode = "none"` project starts the retained values
with their declared initial values.

### 10.1 Checkpoint log in flash

A chip keeps its retain checkpoints in the two checkpoint slots of its chip file, `checkpoint_a`
and `checkpoint_b` (Appendix H). This section defines how they are written and read: the chips'
form of RETAIN-01 and RETAIN-02. Both are the same whichever persistence policy decides when a
chip writes a checkpoint. The shared core implements them (`retain::checkpoint` in
`trust-runtime-core`); the platform adapter only reads, programs and erases its flash.

**Flash.** A slot is one or more whole erase units: one 16 KiB sector on the STM32F407 (sectors 2
and 3) and four 4 KiB sectors on the ESP32-C6 (`memory.spi_flash.erase_kib`). Erasing sets every
byte to 0xFF. A program writes whole program units at multiples of the unit, 4 bytes on both
chips: the F407 programs 32-bit words, the parallelism of its 2.7 to 3.6 V supply (RM0090), and
`esp-storage` 0.10.0 writes 4-byte words. A program only clears bits; it never sets a bit back
to 1. The log programs only units that read as erased, so it programs each unit at most once
between two erases.

**Slot header.** A slot holds records only after its erase has completed and its header has been
programmed. The header is the slot's first 16 bytes:

| Offset | Bytes | Field |
|---|---|---|
| 0 | 4 | Magic `TRCS` |
| 4 | 2 | Format version, 1 |
| 6 | 2 | Reserved, 0 |
| 8 | 4 | Slot epoch: one more than the highest epoch of either slot when this one was formatted |
| 12 | 4 | CRC-32 of bytes 0 to 11 |

**Record.** The records follow the header, each starting at the next multiple of the program unit:

| Offset | Bytes | Field |
|---|---|---|
| 0 | 4 | Magic `TRCK` |
| 4 | 2 | Format version, 1 |
| 6 | 2 | Flags, 0 |
| 8 | 8 | Checkpoint sequence: one more than the highest sequence in either slot |
| 16 | 16 | Application identity: the first 16 bytes of the bytecode hash in the deployment record (§5.1) |
| 32 | 16 | Retain-schema identity: the first 16 bytes of the retain-schema identity in the deployment record (§5.1) |
| 48 | 4 | Payload length R, in bytes |
| 52 | 4 | CRC-32 of the payload |
| 56 | 4 | Reserved, 0 |
| 60 | 4 | CRC-32 of bytes 0 to 59 |
| 64 | C | Commit marker: `DONE`, repeated to fill C, the larger of 4 bytes and the program unit |
| 64 + C | R | Payload: the retained values as the retain codec encodes them (specification 11 §6.7) |
| 64 + C + R | | 0xFF up to the next multiple of the program unit |

Integers are little-endian. CRC-32 is CRC-32/ISO-HDLC, as in STBC and the retain file on a
computer. The commit marker has a fixed place after the header, so whether a record was committed
can be read even when its header is damaged. With 4-byte units a record takes 68 + R bytes, R
rounded up to a multiple of 4: a 16 KiB slot takes records of up to 16,300 payload bytes, and
⌊16,368 / (68 + R)⌋ records before it is erased again. With the STM32U5A9's 16-byte units the
commit marker is 16 bytes, so a record takes 80 + R bytes, R rounded up to a multiple of 16: a
16 KiB slot takes records of up to 16,288 payload bytes, and ⌊16,368 / (80 + R)⌋ records. The
sequence counts checkpoints; it is not an execution generation (RETAIN-01).

**Writing.** A checkpoint is written in this order: the record header, the payload, a read-back of
everything the write programmed, and the commit marker last. It goes after the last record of the
slot with the highest epoch, when that slot's log is not ended by a failing header and the record
fits in the erased space after it. Otherwise the log erases a slot unit by unit, programs its
header with the next epoch and writes the record there. It erases the slot that does not hold the
newest committed record; when neither slot holds one, the slot without the highest epoch, or
`checkpoint_a` when neither slot has a header. The slot that holds the newest committed record is
thus never erased, and no write programs over another record, so a write or an erase interrupted
at any byte leaves the previous checkpoint readable. Each erase of a unit and each program is a
step of its own, so a write can be spread over the time between cycles.

**Recovery.** At start the log reads each slot's records in order, from the first:

- A record is committed when its commit marker reads exactly as written, and valid when its header
  and its payload also pass their checks.
- A record whose header is valid but whose commit marker is not is absent: its write was
  interrupted, and the slot's log continues after it.
- A record whose header fails its check ends the slot's log.
- The newest committed record is the last committed record of the slot with the highest epoch that
  has one.

A warm start loads the newest committed record when it is valid. When its header or its payload
fails its check, the warm start fails, as it does for a damaged retain file on a computer
(specification 11 §6.7); an older record is not loaded in its place. A slot whose header fails its
check holds no log, but its records are read as well: when one of them is a valid committed record
with a higher sequence than the newest committed record, or there is no newest committed record,
the warm start fails too, because that slot's header is damaged. Otherwise, without a newest
committed record, the store is empty, as a missing retain file is. A commit marker damaged after
its write cannot be told from one whose write was interrupted, so its record is absent and the
record before it is loaded. The sequence and the identities come back with the payload; the
retained values are matched by name, as for the retain file (specification 11 §6.7).

## 11. Services and deployment

### 11.1 Non-control services

Hosted and MCU networking and management are adapters/services, not runtime-core dependencies. They may run on the same device without belonging in the VM. The full hosted product shall retain its supported services; a future MCU Modbus, CAN, or MQTT implementation shall use the same ownership boundary.

Workers shall exchange bounded messages or process-image snapshots with the control path. Polling budgets, queue overflow, stale-data limits, and critical versus noncritical failures shall be explicit. A network outage shall not make PLC execution wait indefinitely. The MQTT driver keeps at most 256 received messages for the control path; when a new message arrives at that bound it drops the oldest, counts every dropped message, and reports the driver as `degraded` with the stable detail `incoming queue overflow: <n> messages dropped` (the count since the driver started) from the first drop until the driver restarts. A degraded driver does not by itself fault the PLC or make aggregate health false, and because the overflow concerns received messages only, it does not unconfirm a safe-state handoff, which depends on the driver's output path (specification 11, safe-state handoff confirmation).

Compilation stays on the development computer. The runtime services of ARCH-11 run on every target as shared code, and the development tools present them the same way for every runtime.

**SERVICE-01.** Crate boundaries shall be treated as dependency boundaries, not process-fault isolation. Services may remain in the same process where their scheduling and communication contracts satisfy the selected profile. Stronger isolation requires an explicit thread/process design and its own failure tests; this migration shall not require a process-per-service architecture.

**SERVICE-02.** Rich hosted debugging, tracing, and diagnostic rendering shall remain outside bounded callbacks. Supported pauses, forcing, and online change shall have explicit state/boundary behavior and regression tests. Debugger/export reads shall pin or snapshot the appropriate generation under Section 11.3 and shall not keep unaccounted retired storage alive indefinitely. The same behavior and tests apply on every target (ARCH-11).

**SERVICE-03.** OPC UA (owner, 16 October 2026): the OPC UA server and client of trust-runtime
(specification 11) are one shared implementation in Rust that runs with and without an operating
system, on every target. It replaces the `opcua` library on Linux, macOS and Windows and keeps
their present behavior, which their existing tests pin; the chips get the same server and client
over their network links (ARCH-11, ARCH-12).

**SERVICE-04.** OpenOT telemetry (owner, 16 October 2026): the runtime produces the same OpenOT
records on every target (`[runtime.openot]`). On Linux, macOS and Windows it publishes them to the
shared-memory ring as today; a chip, which runs no other program, publishes them over its network
link.

**SERVICE-05.** Simulation (owner, 16 October 2026): `simulation.toml` (disturbances, couplings
and injected faults) works on every target, chips included, as it does in trust-runtime.

**SERVICE-06.** Debugging on a chip (owner, 18 October 2026) is the shared `debug` component of
ARCH-11 and specification 13: breakpoints, pause, continue, stepping, call stack, variables, watches,
conditional breakpoints, logpoints and `debug.evaluate`, with the access rights of specification 11
§6.9.5. The target needs no ST source text, no compiler, no HIR, no expression parser and no build
tooling for it. Statement spans and line tables come from the module's `LINE_TABLE` (specification 12
§6.13), and every expression it evaluates arrives compiled by the tool (specification 11 §6.13.2).
Its answers are trust-runtime's answers for the same artifact and state: the same resolution of
breakpoints and steps, the same freshness of read values (specification 11 §6.13.1), and a hold that
lasts until a continue or a stop (specification 11 §6.9). While a scan waits in a hold, the service keeps
serving the link and the platform feeds the hardware watchdog, so a long hold neither resets the chip
nor loses a queued request (TIME-04); only the wait and wake come from the platform adapter
(STATE-08).

### 11.2 Installation workflow

This is the installation of a program into a stopped runtime on every target. A running runtime also takes a new program by online change (Section 11.3) on every target, as trust-runtime does; this procedure shall not silently require STOP for any update that online change supports.

```text
1. Discover/read the device's capability and resource descriptor.
2. Compile the application and build its deployment metadata on the computer.
3. Check compatibility before transfer.
4. Put the controller into the stopped state, or use online change (Section 11.3).
5. Transfer/stage the artifacts over the control connection.
6. Independently validate and prepare them on the device.
7. Mark the new installation complete using an interruption-safe procedure.
8. Report the installed application identity and readiness.
9. Start through the configured start policy and the access rights of the control connection.
```

**DEPLOY-01.** MCU firmware and PLC application shall be separate concepts. Firmware is compiled for the CPU/board; STBC is the loadable PLC artifact. Changing only the PLC application should not require a new platform implementation.

**DEPLOY-02.** A board loads programs as trust-runtime does: over the control connection, stopped or by online change, with the same checks and access rights. A probe-flashed, embedded STBC fixture is acceptable for bring-up, but does not satisfy the release deployment gate.

**DEPLOY-03.** A failed transfer or power loss shall never select a partially installed application as valid. Dual slots are one option, not a universal requirement; devices without spare storage shall have a documented stopped/recovery installation procedure.

**DEPLOY-04.** The device shall reject a mismatched configuration/application pair, incompatible retain schema, unsupported required import, or insufficient memory without starting the candidate application.

**DEPLOY-05.** The development environment shall offer the board's resource sizes (memory and timing) in simulation; the board has the same features as every other target (ARCH-11). The simulated process model may differ, but the compiled PLC logic and runtime semantics shall not depend on changing the application for the simulator.

**DEPLOY-06.** Firmware verification shall not automatically establish authorization or authenticity of a separately downloaded PLC application. Define the application's identity, authorized deployment principals (the access rights of the control connection), integrity/authentication mechanism, key/trust handling where applicable, and any rollback policy independently. Bind bytecode, configuration, and declared contracts; enforce the selected policy at installation and activation. A bootloader may support firmware recovery without implementing these application-management obligations. [E23]

**Program identities and parameters.** `program.active` (Viewer) replies `{active, previous}`. Each member is `null` when the store holds no such program, and otherwise `{sequence, program_sha256, bytes}`: the install sequence, the SHA-256 of the program as its record names it, and the program's length in bytes. A runtime without a program store refuses `program.active` with `unsupported request`. The transfer operations of PORT-05 and `bytecode.reload` read their parameters with one rule on every runtime, whatever the platform's JSON library: a number that is not a non-negative whole number is refused when it is the whole of `params` (`invalid params: parameters must be an object`), a JSON number in a member read as text is refused with `invalid params: <member> must be a string`, and a number that is not a non-negative whole number in a member read as a whole number is refused with `invalid params: <member> must be a whole number`. `bytecode.reload` takes either `bytes` or `transfer`, and a request that gives both is refused with `invalid params: give either bytes or transfer, not both`.

**Program install (`program.install`).** `program.install {transfer}` (Admin) installs a complete `TRSTA002` container, taken from a finished transfer (PORT-05), as the program the runtime starts next. It is the only persistent install. `bytecode.reload` changes the running program only (§11.3), and it stays allowed while the resource runs. The install is refused unless the resource is Stopped, on every runtime (decision Q11), and a computer that was stopped by `shutdown` is Stopped (STATE-08). The refusal is `resource_not_running` with the resource state in its message, as `pause`, `restart` and `bytecode.reload` refuse a resource that cannot take them; a hung scan thread is refused as `bytecode.reload` refuses it. The deployment of the web interface (`POST /api/deploy`, Admin) installs into the same project folder and follows the same rule, with the same refusal in its `error` and `error_code` members and nothing written (specification 11 §6.8.2); so does its rollback (`POST /api/rollback`, Admin), which changes which program starts next, and a refused rollback changes no link. A platform without a program store answers `unsupported request` at this point and takes nothing. The request then takes its complete transfer. An upload that is not yet complete is refused with the session's own text and stays open. A refusal after the transfer is taken uses it up, as `bytecode.reload {transfer}` does, so the client uploads again. The checks then run in this order, and the first failure is the reply, with nothing written: the container checks; the record and admission (DEPLOY-04); the retained-value check of specification 11 §6.7.1 (decision G13), which runs the new program's warm start with the retained values of the newest checkpoint on a chip, or of the retain file on a computer, and whose refusal is the warm start's own error, which names the retained value and its reason (for example `retain migration error 'invalid retained value for 'r': value 100 outside declared subrange 0..10'`); and the dry-run preparation with the admitted limits, refused as `insufficient memory` with its numbers when the program needs more than those limits, and with its own error otherwise. The commit then writes the program. On a computer it is a deployment of the program alone through the store of the next paragraph, and the reply carries the sequence that store gives it. On a chip it runs in Stopped, one erase unit per loop pass, with the commit point last (§5.1; STATE-08). The reply is `{"status":"installed","sequence":n,"program_sha256":"<64 hex digits>","bytes":n}`, the identity `program.active` reports for the program. The program runs from the next start (step 9 above): after a `restart` of the stopped runtime on every target (STATE-08), and on a computer also when its process starts again. A committed program that fails its checks at boot refuses the start with its reason and is not replaced by the other slot (Q12).

**The computer's program store.** A computer keeps each deployed project as a bundle, `bundles/<label>`. The link `current` names the bundle that runs and `previous` the one before it; both are symbolic links into `bundles`. A deployment copies the running bundle into a new `bundles/<label>`, writes its payload over the copy, and writes the record of the new program there. The program is admitted before anything the runtime starts from changes: its record, its admission against the hosted device, and a dry-run preparation with the limits the record grants. A refused program leaves every bundle and link as it was. Each file is written through a temporary file that is synced, then renamed, and the folder is then synced, as the retain file is (specification 11 §6.7; G17). The commit is one rename of `current`; `previous` then takes the bundle that `current` named before, by a second rename. A stop before the commit leaves `current` on the bundle it named, and a stop after it runs the new bundle. A rollback changes the same two links in the same order, one rename each. A stop between its two renames leaves `current` on the bundle the rollback selected, which the next start runs; `previous` names that bundle too until the second rename, and no start reads it. A folder `bundles/<label>` that neither link names is an interrupted or refused stage: it blocks no label, and a deployment under that label removes it first. After a deployment every bundle except `current` and `previous` is removed. A bundle's install sequence is the decimal number in its `install-sequence` file, and a bundle without the file has sequence 0; a new bundle takes one more than the newer of the `current` and `previous` bundles. A project folder that is not a `current` link has no store. Its files are replaced in place, after its program is admitted, and `program.active` reports its own `program.stbc` as the active program, with sequence 0, its SHA-256 and length, and no previous program. If its deployment record, when it has one, does not admit that program, `program.active` reports no active program.

### 11.3 Hosted online change and execution generations

This subsection preserves the existing supported hosted online-change contract. The reviewed implementation documents a scheduler-cycle-boundary replacement with a warm restart at the entry point and retain handling; it does not establish a no-restart state migration. [R19]

```text
Prepare and validate candidate outside bounded execution
                         |
Check capability, memory/update peak, timing, and retain compatibility
                         |
Reach the permitted quiescent scheduler boundary
                         |
Perform the supported restart/retain transition and publish new generation
                         |
Invalidate or explicitly rebind permitted external handles
                         |
Retire previous generation after its last permitted reader; reclaim off path
```

H distinguishes executable-body replacement from schema-changing source reload.
An isolated candidate may preserve live retained object/reference graphs only
when the admitted storage declarations, construction roots, executable types,
POU identities and inheritance/interface mappings are congruent. Ordinary code
bytes and initializer bodies may change without changing that identity graph.
Otherwise, serializable retained scalars and aggregates migrate by symbolic
name through the shared TYPE_TABLE/default-recipe policy. An incompatible
retained live-object graph is rejected without replacing the installed owner;
H does not silently discard it or invent general object-schema migration.
Process-image intersections carry into candidate construction before its
configuration initialization; new image capacity starts at zero. Host driver,
protocol-worker and persistence objects survive replacement, while their PLC
bindings are prepared against the candidate and committed with it. Source-free
1.1 loading remains rejected; the legacy hosted path supplies explicit
non-executable construction context to the importer and preserves supplied
ordinary instruction bytes.

The boundary is the existing semantic transition point, not a claim that warm initialization or snapshot migration takes zero time.

**UPDATE-01.** Installed module metadata/code shall be immutable per generation under MEM-07. Candidate construction shall happen outside bounded execution and shall not mutate the running generation. Failed validation, preparation, or update-memory admission shall leave the prior installed generation intact; a failed candidate shall never become executable.

**UPDATE-02.** Publication shall occur only at the defined quiescent scheduler boundary, with no in-flight instruction stream continuing in replaced code. Preserve the supported warm-restart/retain behavior and existing output policy. Do not introduce CODESYS-style no-restart migration, mid-scan replacement, or automatic full-state rollback as an incidental refactor. [R19, E18]

**UPDATE-03.** Account for preparation, coherent state/retain snapshot or copying, restart, publication, and output transition separately. Timing and state-transfer costs at the switch shall fit the admitted maintenance/update window or use an explicitly approved pause/STOP workflow. Preserve existing hosted functionality in the compatibility profile while resolving any bounded-profile conflict; do not describe an unbounded switch as a bounded scan.

**UPDATE-04.** PLC handles, debugger/export references, cached lookups, and service bindings shall carry or be validated against the applicable generation/lifetime. A stale reference shall fail explicitly unless a supported, checked rebinding operation intentionally replaces it. Define token wrap/reuse and restart/identity behavior; a repeated counter value shall not let an old handle alias an unrelated generation.

**Installation generations (MEM-07, UPDATE-04).** Every value reference carries the installation generation of the storage it was taken in. Instantiating an application starts its first generation. Each committed warm or cold restart and each committed replacement of the installed application constructs the next generation, on every target. A resource-only reconfiguration (§4.3.1) keeps the storage, the generation and every reference. A refused restart or candidate changes no generation. The generation is a 32-bit counter of one installation that never wraps and is never reused: once it is exhausted, a further restart or replacement is refused with `Overflow` before anything changes, which takes 2^32 − 1 restarts and replacements of one installation. A reference of another generation names no state. A storage read through it returns nothing; a checked read, write or function-block execution refuses it with `ReferenceLifetime` before any change; and no checked write or PLC assignment stores it, so every reference the PLC holds belongs to the installed generation. The warm-restart transfer of a retained reference (§11.3 above) binds it to the new generation, which is the checked rebinding this requirement allows; a copy of the same reference held outside the PLC stays stale. A host binds the references it holds, such as engineering aliases, task function blocks, I/O binding records and protocol client points, to the same declarations again when a restart or replacement commits and before the next cycle. If a declaration of the restarted application no longer resolves, the controller faults (UPDATE-07). A staged request that names an instance by identity carries its generation (§4.3.1). The check is one 32-bit comparison, independent of pointer width.

**UPDATE-05.** Retired module and state storage shall remain valid until all permitted execution, debugging, and export readers release it. Reclamation, last-owner destruction, and deallocation shall not run on a bounded control path. Use an appropriate ownership/pinning or snapshot mechanism; the design does not mandate a lock-free algorithm or a separate process.

**UPDATE-06.** Admission shall cover overlapping installed/candidate/retired generations and checkpoint/export buffers. Bound retained-reader lifetime or retained-generation capacity through explicit policy; a slow debugger shall not cause indefinite memory growth. Reject/defer additional candidates safely when capacity is exhausted rather than reclaim live storage.

**UPDATE-07.** A fault during or after the committed switch shall follow the documented restart/output recovery policy. Preparation rollback does not imply transactional rollback of already mutated application state or physical outputs. The next active identity, retain result, and readiness/fault state shall be observable and tested.

**UPDATE-08.** Qualification shall exercise updates with nested calls completed at the boundary, active debugger/export readers, queued management/I/O work, stale handles, exhausted overlap capacity, invalid candidates, and generation wrap/reuse simulation. Verify output behavior, old-storage lifetime, off-path reclamation, and the preserved restart/retain contract. These tests do not qualify live MCU update support.

Preserve UPDATE-01/02's existing boundary/warm-restart behavior as the hosted code is migrated. The extra bounded generation, reader-capacity, reclamation, and wrap/reuse machinery in UPDATE-03 through UPDATE-08 belongs to M3/M4H, where actual readers and lifetimes are inventoried. Scope A/B needs valid module ownership and no address-as-identity cache; it does not need a speculative multi-generation reclamation service. Use existing ownership where it satisfies the contract and record a reader class as absent when no actual surface can hold it.

### 11.4 Projects, device settings and hardware descriptions

A board is a runtime like trust-runtime on Linux, macOS and Windows (ARCH-11). Three kinds of
settings keep it that way:

| What | Files | Where they live | On Linux, macOS and Windows |
|---|---|---|---|
| What the PLC does | `runtime.toml`, `io.toml`, the program | the project | the same files with the same meaning |
| The device itself | `device.toml` | written onto the device | not used; the operating system holds these settings |
| The chip | `chips/<chip>.toml` | inside truST | not used |

**TARGET-01 — Same project.** A project for a board is the same as for any trust-runtime:
`runtime.toml`, `io.toml` and the program, with the same keys and meaning (specification 11 and the
configuration reference). I/O uses the same `io.toml` drivers; on a chip, the `gpio` driver's `line`
names a pin of the chip, for example `PD12` on the STM32F407 or a GPIO number on the ESP32-C6, and
the chip's other I/O (analog, PWM, counters, CAN) are drivers of the same file. A setting that needs
hardware the device lacks is refused when the project is deployed, with the reason (ARCH-13).

**TARGET-02 — Device settings.** A device without an operating system keeps the settings that the
operating system holds on Linux in `device.toml`, stored on the device and not in the project, so
that no password reaches the project: the device name; each network interface (Ethernet, Wi-Fi)
with DHCP or a fixed address; the Wi-Fi network name and password; Bluetooth; and the time server.
It also holds the few wiring values of the device that are not pins and that the firmware cannot
detect itself: the firmware measures the crystal frequency against the chip's internal clock and
identifies the Ethernet PHY over its management interface, and `device.toml` names them only when
detection is not possible. Pins are named in `io.toml`, as on Linux. It is written over the control
connection, the first time over USB, from `trust-runtime` and from the hardware canvas, with the
access rights of the control connection. Runtimes on Linux, macOS
and Windows do not use it.

**TARGET-03 — Chip descriptions.** truST describes each supported chip in `chips/<chip>.toml`:
processor and instruction set, flash and RAM sizes and layout, clock limits, its interfaces, and the
runtime components its firmware holds (ARCH-14).
truST builds one firmware per chip from it; there is no firmware, and no file, per board. Any board
with a supported chip runs that firmware: its pins are named in `io.toml` and its other wiring is
detected or named in `device.toml` (TARGET-02). A chip of a supported family needs only its chip
file. A new chip family
needs a platform adapter, written once: the drivers for its timer, watchdog, flash, pins, USB and
network links. Everything above the adapter is shared (ARCH-11). Appendix H defines the file.

**TARGET-04 — Installing truST on a device.** `trust-runtime` and the hardware canvas install the
truST firmware on a supported chip, over the chip's own USB boot loader or a debug probe, and
update it later over the control connection. After that, a project is deployed to the board as to
any trust-runtime. A chip on USB is found by the USB source of the one search of specification 11:
a passive listing that reads USB descriptors and never resets or writes a board (owner, 15 October
2026). A chip that already runs truST is listed as a truST runtime, reachable over USB and, when it
has a network, found on the network like any runtime (ARCH-12).

**TARGET-05 — Versions.** The same version rules apply to every runtime: the tools read the
runtime's version over the control connection and, when the versions do not match, say which
update is needed.

**TARGET-06 — Time.** Date and time functions use the device clock. A device without a battery-held
clock sets its clock from the time server of `device.toml` when it has a network, and reports the
time as not set until then. A reply or event value that carries the wall-clock time, such as the
`time_ns` of a `feature_disabled` event, is `null` while the time is not set, never an invented 0;
trust-runtime on Linux, macOS and Windows always has the time, so its replies stay as they are. A
device reports no `audit_dropped`: trust-runtime reports it when its audit logger thread has ended
and the channel to it refuses a record, and a device has no such thread or channel, since its
control loop hands each audit record to the platform's audit store in the pass that serves the
request.

**TARGET-07 — Diagnostics.** The same fault log, events and log levels as trust-runtime
(`[runtime.log]`), read over the control connection.

**TARGET-08 — Outputs on a fault.** As on Linux: `io.toml` `[[io.safe_state]]` gives each
output's safe value, and `runtime.toml` `[runtime.fault]` and `[runtime.watchdog]` choose `halt`,
`safe_halt` or `restart`. A disconnected computer changes nothing; the runtime keeps running
(ARCH-07). An output's safe value is its `[[io.safe_state]]` entry when that is a Boolean, and
false, a de-energized output, otherwise. A chip's terminal fault path, a panic that ends the boot
(STATE-05), drives each configured output to that safe value, with the line's `invert`, before it
resets, as the stop and the fault paths do; it touches no pin that is not a configured output.

**TARGET-09 — Releases.** Each truST release includes the firmware for every supported chip, built
from the same version as trust-runtime, with SHA-256 sums (owner, 16 October 2026:
"the whole point was to release builds for stm32 and esp32").

**TARGET-10 — No read-out protection.** The owner wants to see the code on a device: "we dont
want read out protection, i want to be able to see the code" (16 October 2026). truST does not
enable read-out protection, flash encryption or secure boot on any chip; a debug probe can read the
firmware and the installed program.

## 12. Compatibility and numeric behavior

**COMPAT-01.** Extracted instruction semantics shall be behavior-preserving. A smaller MCU shall not silently narrow DINT/LINT, reinterpret signedness, change array bounds, remove reference checks, or change string truncation behavior.

**COMPAT-02.** Each hosted runtime or firmware shall expose a descriptor covering bytecode versions; semantic/library, scheduling/I/O, and numeric contracts; admitted imports; type features; resource maxima; physical clock resolution; required output evidence; timing-evidence classification; and deployment/update capabilities. The installed application identity and applicable qualification record shall be queryable.

**COMPAT-03.** OOP method/interface calls, function-block state, and applicable standard-library functions shall be included in parity testing, not only arithmetic and a blinking LED.

**COMPAT-04.** Exact comparison tests shall cover integer/Boolean operations, defined byte encodings, stable faults, scheduler decisions, and observable state/output traces where matching initial state, logical input/time traces, imports, and numeric/environmental contracts require equality. An exact whole-trace claim includes all upstream numerical decisions; Boolean output bytes alone do not make a trace independent of floating-point differences. Physical timing observations remain separate platform evidence.

**COMPAT-05.** Floating-point behavior shall use the explicit contract in Section 12.1. Sharing Rust source shall not imply arbitrary bit identity for all CPUs, functions, NaN payloads, or hardware modes. Freeze intended semantics, ordinary and exceptional cases, and permitted function-specific tolerances. A tolerance-qualified result shall not imply bit-identical output traces; changes to the observable contract require explicit approval.

**COMPAT-06.** Cross-platform tests shall execute saved STBC under identical initial state and logical input/time traces on each claimed Linux, Windows, macOS, and MCU configuration for the common supported contract. Record strict versus tolerance-qualified expectations before comparison, including resulting control decisions. Host-only features require additional native regressions; neither removing them nor silently skipping cases establishes parity.

Hardware timing, electrical behavior, available memory, and protocol support remain target-specific. Semantic portability is not a guarantee of identical scan duration or identical physical pin behavior.

### 12.1 Numeric contracts and observable control decisions

**NUM-01.** The numeric contract shall specify relevant ordinary/exceptional behavior, including rounding/conversions, NaNs and any observable payload policy, signed zero, subnormal handling, floating-point hardware modes, and library/import versions. Existing intended IEC behavior remains the migration reference. An unsupported required numeric mode shall fail admission, not silently select host defaults. Canonicalization or an alternative implementation that changes existing observable behavior requires a compatibility decision. [E14, E15]

The initial qualification record has the following closed set of premises. Missing evidence leaves the affected claim unqualified; additions require a contract revision rather than an open-ended demand for unspecified proof.

| Premise | Required record/evidence |
|---|---|
| Inputs and operation semantics | Operand types and bit patterns, promotion/narrowing points, operation/evaluation order, exact versus tolerance-qualified domain, selected function/import, and fault behavior. |
| Compiler/code generation | Exact rustc/LLVM, target/CPU features, optimization/LTO and flags. No fast-math/algebraic reassociation or implicit FMA contraction of separately specified arithmetic; inspect representative generated kernels. Explicit fused operations used by a pinned math algorithm remain explicit and are qualified as fused. |
| Floating environment | Round-to-nearest ties-to-even; gradual underflow. Verify F401 FPSCR flush-to-zero is off and the corresponding hosted FTZ/DAZ controls are off where applicable. Record startup configuration and prevent callbacks/FFI from changing the admitted environment. |
| Math library | libm 0.2.16, resolved features and architecture-selected implementation; no nightly intrinsic/float features. Record actual feature unification for each graph, not just one manifest declaration. |
| Software arithmetic helpers | compiler_builtins/soft-float source version or commit and the target rustlib artifact hash tied to the exact Rust distribution. These sysroot helpers are not assumed to be pinned by the application Cargo.lock. |
| Observable results | Signed-zero, subnormal, NaN/non-finite/fault policy; per-operation accuracy reference/criterion and threshold/control outcomes. Run the saved ordinary, exceptional and boundary corpus on each claimed tuple. |

These premises bound the evidence task; they are not a substitute for an operation-specific result guarantee or numerical qualification, and passing a finite corpus is not an exhaustive proof for every input. Deterministic stateless behavior is claimed within the qualified domain/contract. Rust's unspecified-precision functions do not acquire a universal per-build bit-identity guarantee merely by being stateless. [E59, E63]

IEC 61131-3:2013 §6.6.2.5.8 requires the implementation to express numerical accuracy through implementation-specific dependencies and identifies range overflow/division by zero as errors. Table 28 covers numerical functions; Table 29 covers arithmetic functions including exponentiation. The standard does not prescribe a universal ULP tolerance. Document the chosen accuracy dependencies here; no IEC deviation is created merely by selecting a math library or platform profile. [E62]

**NUM-02.** Strict trace-equivalence claims shall require matching numeric and environmental contracts. Use common deterministic implementations, defined canonicalization, or another verified mechanism where the required observable behavior demands exactness. This does not require identical internal memory layouts or identical physical scan duration.

**NUM-03.** A tolerance-qualified contract shall state the function/operation, valid input domain, error criterion, exceptional cases, and permitted observable outcomes. Tests shall separately verify numeric error and the resulting Boolean decisions, alarms, and state transitions near thresholds. An unspecified numeric tolerance shall not excuse an arbitrary output mismatch or missed fault.

**NUM-04.** Expected traces shall label which observations require exact identity and which, if any, admit documented alternatives. Where the application requires an exact threshold decision and the implementation cannot guarantee it under the selected contract, reject that claim or supply an approved strict implementation; do not silently alter the application with a deadband or different comparison. Tests and reports shall not call tolerated control divergence bit-identical replay.

For example, `HeaterEnable := CalculatedTemperature < Setpoint;` can produce different Boolean outputs from two temperature results on opposite sides of the setpoint even if both satisfy a numeric tolerance. Exact output comparison therefore depends on the upstream numeric contract, not only the output's data type.

One well-defined numeric contract is sufficient for the first implementation. These requirements do not mandate several numerical backends. Rust and runtime documentation establish why exceptional-value/import behavior needs explicit treatment; they do not imply that ordinary arithmetic is arbitrary. [E14, E15]

**NUM-05.** Use one portable numeric implementation across adopting hosts/MCUs. Select `libm` 0.2.16 with its default `arch` feature enabled and no unstable features. This version is already locked through glam, num-traits and simba; Scope A adds a direct core edge, not a package-version upgrade. Its reviewed architecture module has no ARM32/RISC-V specialization, while retaining useful hosted sqrt/FMA paths. Disable or override a feature only for a demonstrated requirement, recorded and requalified under NUM-01. Inventory and route core exponentiation/TIME truncation and hosted numerical functions through the selected primitives. Preserve promotion, finite/narrowing checks, faults and conversions. Host adoption remains an explicit numeric compatibility migration with retained old oracles and reviewed new observations. [R23, R28, R37, E58, E59]

The initial contract distinguishes exact checked integer/Boolean behavior and individually qualified basic floating operations from tolerance-qualified exponentiation/transcendentals. Rust documents unspecified precision for `powf`, `sin`, `cos`, `tan`, `exp`, `ln`, `log10`, and `atan2`, among others; current hosted source therefore supplies no universal bit-equality guarantee. For each admitted non-exact function, record the input domain, accuracy criterion/reference, exceptional behavior, and allowed control observations before its fixture or profile is qualified. Reject an unsupported exact-trace claim. Using the same `libm` source is useful but not proof of bit identity: its feature-selected implementation, compiler, rounding/subnormal modes, and exceptional-value handling remain part of the evidence. Do not conflate `force-soft-floats` with a complete software emulation of all Rust arithmetic. [E58, E59]

An equivalent native `f32` implementation of REAL `+`, `-`, `*`, `/`, or `SQRT` is permitted; it is not automatically a behavior change. Roux's double-rounding result supports such equivalence under its binary-format and rounding hypotheses. Establish those hypotheses for the actual operand/promotion path, preserve fault and conversion behavior, and add differential boundary/subnormal/overflow/signed-zero tests. Do not generalize this result to mixed operands without checking conversions, to `**`, or to transcendental functions. No fast-math/algebraic reassociation is introduced. [R28, E60]

Scope A prepares a small numeric fixture with REAL/LREAL, exponentiation, TIME×REAL truncation, a selected nontrivial math import, and a threshold-controlled Boolean output. Freeze exact/tolerance expectations and the old/new oracle identities before A1's numeric batch; A4 integrates that contract with source-free execution. Scope B/M2E measures helper/code size and execution cost. F401 accelerates single precision only; the selected C6 target and the optional C3 target have no hardware floating-point extension, so both widths incur software arithmetic costs. These facts constrain workloads, not IEC type widths. [E25, E56, E57]

#### A1 numeric and layout acceptance record

The A1 finite numerical corpus, under IEC §6.6.2.5.8's implementation-specific
accuracy requirement, is defined below. It qualifies these inputs only; it is not
an all-input accuracy bound. The Rust constants and decimal references are fixed
independently of the new primitive calls in `portable_numeric_contract.rs`.

| Function | Input | Reference |
|---|---|---|
| SQRT | 2 | sqrt(2), Rust `SQRT_2` |
| LN | 2 | ln(2), Rust `LN_2` |
| LOG | 10 | 1 |
| EXP | 1 | e, Rust `E` |
| SIN | 0.5 | 0.479425538604203 |
| COS | 0.5 | 0.8775825618903728 |
| TAN | 0.5 | 0.5463024898437905 |
| ASIN | 0.5 | pi/6 |
| ACOS | 0.5 | pi/3 |
| ATAN | 1 | pi/4 |

For these LREAL cases, absolute error shall be at most
`8 * f64::EPSILON * max(1, abs(reference))`; REAL uses the reference rounded to
binary32 and `4 * f32::EPSILON * max(1, abs(reference))`. Lower/upper control
thresholds at reference minus/plus twice that tolerance have fixed greater/less
outcomes. This specifies test thresholds, not a deadband added to PLC programs.
The old std math calls remain test-only compatibility references, with old/new
bit patterns recorded in A1's log; no prior execution is implied by their source.

Exact cases cover SQRT(9) and its equality boundary in actual ST, 2**3, signed
zero, a retained REAL subnormal, basic REAL rounding, and duration truncation.
TIME scaling truncates toward zero on the operator path and, since 15
October, in the named helpers too (specification 07 §9, IEC 61131-3 Table 35). Domain/overflow/divide-by-zero
faults retain their existing call-site policy. The ST fixture also exercises
fractional exponentiation, SIN, TIME scaling and Boolean outputs. ATAN2 retains
its binary32 same-width path and binary64 mixed-width path. No f32 fast path,
fast-math or contraction flag is introduced.

Value-slot build guards are at most 32 bytes on 32-bit targets and 48 bytes on
64-bit hosts, with alignment at most 8. These are explicit A1 slot budgets, not
measured whole-runtime footprints or capacity admission for an application.
The native test records actual host size/alignment; target checks enforce their
compile-time bounds. Board preparation/heap/stack/FPSCR evidence remains B/E.
The evidence record identifies compiler/LLVM, features, target sysroot/helper
hashes and flags; missing platform floating-environment evidence stays unqualified.

## 13. Migration plan

| Step | Work | Exit gate |
|---|---|---|
| M0 — Freeze baseline and contracts | Record F401 identity and the provisional C6 build; freeze Scope A's compiler/dependency tuple, profile limits, math contract/fixtures, initializer format, and ownership interfaces. Confirm the register-tier and persistence decisions above, with target measurements still open. Record the wider modernization ledger. | Scope A has an explicit construction/format contract, oracle provenance, dependency graph, and acceptance list. Board availability and unmeasured resources remain honestly open. |
| M0U — Separate modernization scope | Implement Section 16's compiler/MSRV/edition/resolver, Node and dependency upgrades in a separately authorized scope and batch. Default order: Scope A, then Scope B on the frozen 1.95 baseline, then M0U before broad M2B migration. M0U is not an M1/M2A prerequisite. | Record extraction-before-modernization snapshots and compare like-for-like workloads. Earlier M0U ordering requires an explicit scope/baseline revision; it never silently joins A's batch. ESP32 hardware availability need not block modernization. |
| M1 — Isolated portable core builds | On Rust 1.95.0/edition 2021/resolver 2, fix dependency defaults, ordered-map hasher/constructors and exposed API compatibility; add portable math and CRC edges, target layout assertions and F401/C6 core CI checks. | Both core graphs check without host/HIR leakage. Account for source changes as well as manifests. Firmware linking/boot remains Scope B/M2E evidence. |
| M2A-H — Scope A: host compiler-free fixture | Relocate the complete byte-container decode/validate/materialization boundary and preserve host wrappers. Implement the 2.0 producer/version matrix/format tests, TYPE_TABLE-driven construction, shared-bytecode initializers, timers, executor/state and explicit context needed by the fixture. | Fresh headless loading excludes CompileSession/HIR/Expr. Complete legacy-format and 2.0 validation, initialization/state/fault and numeric oracles pass; hosted APIs and optimized tier remain intact. Source checks do not claim hardware success. |
| M2A-B — Scope B: F401 fixture execution | Link the same engine/artifact into NUCLEO-F401RE firmware with timer, PC13/PA5 process images, serial trace, controlled outputs, IWDG, and an instrumented bounded bring-up heap. | Real board traces match the Scope A contract; report flash/loadable sections, `.data`/`.bss`, preparation/RUN/heap peaks, target value sizes, MSP/IRQ headroom and numeric timing. Remaining scan allocations are explicit. Only M2A-H plus M2A-B closes M2A. |
| M2E — Early ESP32 execution checkpoint | Immediately reuse M2A artifacts and numeric cases on the selected physical ESP32, before waiting for M4H/M4 or freezing common interfaces around STM32 assumptions. | The same shared engine loads and executes the common corpus; record ESP32 memory, initialization, numeric, clock, and ownership results. Hardware absence is an open gate; continue independent hosted/F401 work. |
| M2B — Expand shared execution and hosted migration | Expand construction, dispatch, storage, task/cycle policy, and libraries; migrate the optional register tier with preparation-time lowering and host profiling adapters. Implement actual runtime-only/full/development feature compositions. | All claimed hosted targets adopt the common engine and preserve supported workflows, including optimized-tier regressions. The larger source-free corpus passes on its declared targets; runtime-only graphs genuinely exclude authoring/unselected services. |
| M3 — Bounded control and lifecycle paths | Complete compound storage/atomic retirement, fixed diagnostics, import bounds, persistence mailbox/stop checkpoints, actual generation lifetimes, and admission/capability/deployment records. Add no-CAS target checks here only if C3 is selected as an additional profile. | Normal, first-use, fault, limit, and lifecycle paths satisfy the applicable bounded contracts. Larger hosted limits remain; timing/descriptors/authentication and remaining release evidence feed M4H/M4/M5. |
| M4H — Hosted qualification | Qualify runtime-only/full/development builds, all claimed native OS/architecture adapters and supported workflows, numerical trace contracts, generation changes, wake/overload behavior, and baseline-relative performance. | Each claimed hosted target passes Section 14, with explained/approved regressions and its own timing classification. |
| M4 — F401 qualification | Complete real I/O, time, watchdog/fault handling, stopped application deployment, retain policy, disturbed/mixed-rate workloads, and extended tests. | F401 acceptance passes within its actual flash/RAM/stack and storage reservations with declared completion-level timing evidence. H7 results cannot substitute for this gate; M4H remains separate. |
| M5 — Required ESP32 qualification | Complete the adapter first exercised in M2E, then qualify the declared ESP32 profile with I/O, supervision, deployment, persistence, disturbances, and the common corpus. | One named ESP32 board/build passes its applicable Section 14 gates with the shared engine. Another MCU family or a compiler-only check cannot close this deliverable. |

**PLAN-01.** M2A shall occur before broad service migration or a full packed-value redesign. A linked firmware, a probe-loaded Boolean-only demo, or two host simulations shall not close this checkpoint. A bounded preparation heap and explicitly measured remaining scan allocations are permitted at bring-up, but the result shall not be labeled MCU-bounded qualification. This checkpoint proves the same execution architecture at both ends early; it is not a feature-complete MCU release.

Scope A and B use the same saved artifact/metadata and frozen numeric contract. Required initialization semantics use the versioned format contract from LOAD-10, not an optional record an older reader can skip. Expected behavior remains anchored in pre-migration fixtures and recorded compatibility changes; independently reimplementing the tested subset to obtain a passing board demo is prohibited.

M4H and M4 may proceed in parallel once their prerequisites are met. Existing tests shall remain registered with their native assertions during extraction; their execution follows the cadence below. Full native hosted qualification is required at completion even if the first narrow checkpoint uses one host. Do not block early MCU resource evidence on rewriting every hosted service, and do not drop hosted workflows to accelerate the MCU proof.

**PLAN-02.** ESP32 portability shall influence M0/M1 and receive the M2E physical checkpoint. M2E is a bring-up result, not M5 release qualification. Optional H7 work shall neither delay use of the available F401 nor replace the required ESP32 deliverable. Core adoption, bounded execution, and platform release qualification shall remain separately reportable so a difficult hosted service does not conceal progress or erase a remaining gate.

**Execution cadence.** Scope A is the aggregate of four bounded scopes, A1–A4 below, following the explicit 9 October 2026 planning decision. Each has one final validation batch; B and M0U remain separate. Finish the entire authorized scope and necessary test authoring before its batch. Preserve failures and obtain explicit authorization for another run; do not silently subdivide a scope to manufacture extra batches. Selecting these boundaries does not itself start implementation. Milestone labels do not create extra batches, and unrun gates remain unverified. The [implementation checklist](https://github.com/johannesPettersson80/trust-platform/blob/main/docs/internal/testing/checklists/runtime-portability-implementation-checklist.md) records the current checkpoint, scope authorization, requirement coverage, and evidence; this specification remains the behavior authority.

Extraction, value-layout optimization, and intentional behavioral changes shall remain separate reviewable slices with the repository's dependency/architecture checks. The separation of compilation from execution is established in other runtime designs, but their published sizes are not truST targets or estimates. [E22]

Main carries Salsa 0.28.5 and rustls 0.23.45, including the required transitive dependencies, from the security update that resolved RUSTSEC-2026-0308 and RUSTSEC-2026-0285. A1 pins those exact versions, preserves the extraction compiler/edition/resolver and the existing query semantics, and ran the affected analysis and TLS regressions in its batch. This exception does not authorize broad modernization.

M0U uses the same cadence in its own scope. Scope A selects and freezes the existing compiler and the minimum portable dependency graph; it does not raise MSRV, migrate editions/resolvers, update Node/npm, or run MOD-06's modernization matrix. Broad package updates remain required work for the combined plan. Keep their ledger visible without making them hidden M2A prerequisites.

### 13.1 First implementation scopes

**Scope A — host source-free construction on Rust 1.95.0.** Keep edition 2021, resolver 2, and workspace MSRV 1.95. Includes M0 extraction decisions, M1 source/feature fixes, complete decoder/validator relocation, STBC 2.0 producer/container/version-policy/format work, SCHED-05's periodic scheduling correction, and M2A-H. Add only required direct portable dependencies at the existing locked versions. Excludes M0U, edition migration, broad dependency upgrades and MOD-06's WASM/npm/extension modernization batch. The fixture includes Boolean/integer logic, TON, an array, FB/interface state, nonzero defaults, changing-input initializers, NUM-05 math cases, and a 25 ms periodic counter task on exact 10 ms logical resource samples. Preserve host APIs/tier behavior and existing semantics except the recorded corrections, and author fresh-loader, initializer, scheduler, numeric and target-layout assertions. Proposed native test: `runtime_core_compiler_free_load`.

The periodic fixture registers at logical 0 ms with `SINGLE` FALSE and a zero counter. Samples at 10, 20, …, 1000 ms shall produce 40 activations and zero overruns; there is no activation at registration. The saved oracle records nominal due times, activation samples, counter/state and missed-interval counts, including the initial 30, 50, 80, 100 ms activation sequence. Under COMPAT-06, replay this same logical trace on every claimed native host and both F401/C6 boards. These are expected observations, not results of a run; physical clock, jitter and completion measurements remain separate.

Execute A1 → A2 → A3 → A4. Each is a complete implementation scope with its own frozen diff, review, and one consolidated validation batch. Scope A closes only when all four are verified and the integrated M2A-H fixture passes.

| Scope | Complete implementation slice | Required evidence in its final batch |
|---|---|---|
| A1 — Portable foundations | SCHED-05 readiness correction and native regressions first, as a separately reviewed change; workspace feature defaults; explicit portable ordered-map hasher/constructors with host API compatibility; direct locked libm edge and numeric routing/oracles; target value-layout assertions; isolated F401/C6 core CI lanes. | Both MCU core graphs, core tests including corrected readiness/case assertions, host `tasks` / `scheduler_resource`, numeric/control boundary assertions, host compatibility and required runtime vertical. Freeze the corrected scheduling contract and existing compiler tuple; no edition or broad dependency upgrade. |
| A2 — Shared loader and validator | Full container/decode/validate/materialization boundary in core, portable collections and CRC edge, host re-exports, and byte serialization ownership. Preserve the 1.x format and compatible diagnostics; include the authorized review expansion in §5.4. | Relocated unit tests and the complete existing container/metadata/resource-bound/section/helper/image/validation/optional-section corpus; malformed input and failed preparation remain rejected. Both isolated MCU graphs still check. |
| A3 — STBC 2.0 format and producer | Settle exact layouts before code; add executable initializer and construction metadata, compiler lowering, dual-major compatibility policy, producer/version selection and separate 2.0 fixtures. | 1.x regression and 2.0 encode/decode/validate/round-trip/negative cases. Keep existing hosted execution usable; candidate 2.0 emission is explicit until A4 integrates execution. No compiler-free execution claim yet. |
| A4 — Shared-engine integration | TYPE_TABLE construction, shared-dispatcher initializers, timers, execution state/context, headless source-free loading and the representative fixture, including A1's corrected periodic scheduling rule. Preserve hosted APIs and tier behavior. | `runtime_core_compiler_free_load`, integrated numeric/state/fault/oracle cases, inherited scheduler regressions, both MCU core graphs and the complete affected format/VM suites. Freeze the integrated saved-artifact oracle here. This closes M2A-H only; hardware remains B/M2E. |

The changing-input local initializer in the shared fixture is a reference to a
persistent array element selected by a changing method input, as specified in
specification 12 §11.5.10 and IEC 61131-3 Ed.3 §6.4.4.10.2–3. The producer preserves
the existing source constant-expression diagnostics. This fixture must demonstrate
per-call initializer evaluation in A4; acceptance of mutable scalar initializer
expressions is not introduced by the portability work.

**A3 format acceptance before A4.**

Before A4 depends on STBC 2.0, A3 must statically reject initializer writes whose
destination is not proven staging across every reachable path, share immutable
recipes only between equivalent visibility contexts, omit unused standard block
templates and prove edge/retain/partial/configuration/class authoring with native
tests. Spec 12 §11.5.8 and §11.5.11 own these rules. The revised saved fixture is
admission evidence until A4 executes it; runtime checks of mutable reference
contents remain mandatory.

At the start of A1, apply the reviewed draft's readiness arithmetic and core/case/host test changes, excluding its stale AGENTS.md and unsupported hosted-runner timing claim. Correct the late-sample test comments to describe injected logical time. Add the non-multiple host case in `tests/tasks.rs`: use `set_current_time` plus `execute_cycle` for exact 10 ms samples through 1 s, expecting 40 runs and zero overruns (33 runs follows from the old rule). Add a core case asserting due time 25 ms at sample 30 ms, `last_run = 25 ms`, then due time 50 ms at sample 50 ms with zero missed intervals. Retain jump/backward/saturation coverage. Review this small fix separately, finish the rest of A1 and run its one batch; do not add an intermediate run. A1's accepted baseline shall contain the correction and updated assertions. A4 only adds compiler-free fixture integration and the saved-artifact oracle for this requirement.

Include `cargo +1.95.0 test --locked -p trust-runtime-core` and `cargo +1.95.0 test --locked -p trust-runtime --test tasks --test scheduler_resource` in A1's consolidated builder batch, together with the required runtime vertical and remaining A1 gates; deduplicate overlapping suites. No standalone run is authorized for this correction. Non-blocking verification maintenance alongside A1's native tests adds the `verification/spec-gaps.toml` record and new-test mappings in `verification/runtime-anomaly-taxonomy.toml` beside `ANOM_MAP_WATCHDOG_REVIEW_C5ABDD0A`, using `scripts/verification` generators for IDs. Close implementation/test evidence only after the native assertions pass; metadata is not acceptance evidence itself and does not create another scope or batch.

All batches use the selected 1.95.0 toolchain on the builder. Include the required runtime vertical for runtime changes and architecture-doctor with `--full-map`, applicable diagram, format and lint checks for the affected boundaries. The retained corpus includes `runtime_core_behavior_lock`, `bytecode_vm_core`, `bytecode_vm_differential`, `runtime_restart`, `scheduler_resource`, `vars_retain`, `bytecode_container`, `bytecode_metadata`, `bytecode_decode_resource_bounds`, `bytecode_sections`, `bytecode_helpers`, `process_image`, `bytecode_validation`, `bytecode_optional_sections`, `bytecode_encoder`, `bytecode_roundtrip`, and `bytecode_verification_cases`, plus moved unit tests. The checklist assigns these to scopes; preserve native assertions and deduplicate overlapping commands within each batch. Later scopes rerun affected regressions because the implementation changed. Baseline executions, if missing, belong in the applicable planned batch against a preserved baseline snapshot. No hardware or firmware-link result is implied.

**Scope B — F401 execution and resource evidence.** Implements M2A-B using Scope A's artifact, expected observations, engine revision and Rust 1.95.0 build baseline. Include linked firmware, controlled boot/output/watchdog, physical trace comparison and flash/heap/MSP evidence in its batch. If earlier modernization is explicitly selected, update the build identity and comparison plan before B; do not mix A's compiler evidence with a different firmware tuple. Bring-up allocations remain measured, and zero-allocation qualification remains M3/M4. Preserve failed fit evidence. M2E repeats on C6 when available; any changed toolchain/profile needs its own declared evidence.

### 13.2 Contract and implementation timing

This table replaces the earlier unassigned P0/P1 labels. It assigns work to milestones without adding another priority-tag system.

| Work | Required before or within |
|---|---|
| Extraction compiler/features, loader/initializer representation, storage/limits, time/fault interfaces, numeric premises, register-tier destination | M0/Scope A on 1.95.0/2021/resolver 2. M0U is separately authorized and does not supply A's prerequisites. Physical measurements remain Scope B/M2E. |
| Fixture loading, construction/initialization, timers, shared dispatch, checked depths and fixed capacities selected for bring-up | Scope A/B; preserve stable fault and value/reference behavior. |
| SCHED-05 nominal-deadline correction and non-multiple task oracle | Implement first in A1, with updated native assertions in A1's one batch. A2 onward inherits the corrected baseline; A4 integrates the compiler-free fixture/oracle, then B/E and QH/QF/QE replay it. No additional scope or batch. |
| Broad hosted adoption and true runtime-only compositions; preserved update boundary/warm restart (UPDATE-01/02) | M2B, then native hosted acceptance. |
| Construction-only raw access, checked installed-state writes, separate debug scratch registry and explicit embedding API migration (HOST-09) | Contract selected before H ownership changes; implement and test in H/M2B. Deeper UPDATE-03 through UPDATE-08 machinery remains M3/M4H. |
| Bounded storage/import work; actual generation/reader capacity and retirement (UPDATE-03 through UPDATE-08); selected bounded persistence | M3, qualified by M4H/M4/M5 as applicable. No speculative reader subsystem in Scope A. |
| Full timing-admission record (§8.4/SCHED-02 through SCHED-04), release descriptor (COMPAT-02), production deployment authorization (DEPLOY-06), extended disturbances | Design the needed interfaces early; implement/qualify at M3/M4H/M4/M5 before a corresponding release claim. Scope A/B still validates all bytes, capacities, and outputs for its controlled fixture. |
| Full modernization ledger, platform acceptance, update/deployment/retain recovery, and exact or tolerance-qualified published claims | Required before closing the combined plan; independent UI/protocol upgrades need not block first-board feasibility. |

Implementation of ownership/data-flow changes updates `docs/diagrams/architecture/runtime-execution.puml` and `runtime-bytecode-vm-execution.puml` where affected; render/drift checks belong to that scope's batch. New crates/large-file policies follow architecture-doctor and recorded waivers where required. CHANGELOG and release-version rules apply when corresponding behavior is implemented/released, including STOP latency, numeric baseline, and persistence changes. A planning document is not a shipped feature or version bump.

## 14. Verification and acceptance

### 14.1 Required evidence

| Gate | Required result |
|---|---|
| Portable build | Separate actual STM32F407 and selected ESP32 target checks plus linked firmware; no hosted/HIR leak or unresolved dependency/atomic assumption hidden by a combined host build. |
| Standalone loading | Fresh engines on every claimed hosted target and both selected MCU targets initialize from saved STBC and deployment metadata, including LOAD-10/11 initialization, without `CompileSession`, source parsing, harness initialization, or host-built runtime state. |
| Regression | Existing runtime behavior locks, scheduler/restart tests, reference checks, and relevant runtime vertical tests pass. |
| Frozen-artifact parity | Saved bytecode and baseline traces preserve the selected scheduling/numeric/import contracts, with exact or explicitly qualified expected observations. Two newly compiled identical implementations are not a sufficient oracle. |
| Memory and first-use behavior | Preparation, RUN, compound storage, checkpoints, and applicable candidate/retired-generation peaks fit usable regions. CPU/ISR stacks are separate. No bounded control-path allocator activity, including first use, rare branches, maximum-size operations, faults, and last-owner destruction. |
| Validated construction and negative admission | Raw decoded candidates cannot execute. Mutated bytecode/metadata, expanding counts, invalid jumps/references/imports, resource overflows, invalid generation identities, unauthorized/mismatched artifacts, and unsupported timing/numeric requirements are rejected without candidate output activation. |
| Timing and mixed-rate admission | Representative admitted workloads meet computation and required output-completion deadlines under the recorded envelope. Staggered/coincident fast/slow releases expose blocking and batch publication delay; incompatible cases are rejected or explicitly unqualified under SCHED-04. Overrun/hang recovery is tested separately from deadline success. |
| Clock and wake contract | Logical and physical clock domains cannot be accidentally interchanged. Wrap/discontinuity, expired waits, concurrent wake/token races, STOP during long idle waits, and hung synchronous work follow the stated policies. |
| I/O failure and completion evidence | Stale input, delayed transfer, failed/partial commit, and failed fault actuation report the actual available evidence. Obsolete queued normal outputs cannot silently override STOP/FAULT/replacement output policy. Driver acceptance is not reported as physical actuator completion. |
| Stopped persistence/deployment | Power interruption during checkpoints and MCU installation cannot select incomplete state/artifacts; identity, authorization, rollback policy, and restart behavior match the selected deployment contract. |
| Hosted online change | Invalid candidates leave the prior generation intact before switching. Valid changes preserve boundary/warm-restart/retain/output behavior; stale handles fail, active readers stay valid, overlap capacity is bounded, and old storage is reclaimed off the control path. Post-switch failures follow explicit recovery rather than an invented rollback guarantee. |
| OOP/application | Statically allocated FBs, methods, interface dispatch, timers, bounded strings/arrays, value copies, and reference lifetimes work under the same admitted contract on host and hardware. |
| Numeric/control outcomes | Ordinary/exceptional values, conversions/modes, near-threshold branches, alarms, and state transitions obey NUM-01 through NUM-05. The portable math implementation and promoted-double costs are identified. Exact parity and tolerance-qualified alternatives are reported separately. |
| Extended disturbance qualification | Platform-appropriate longer runs and interference cases meet predeclared criteria; duration alone is not a proof of WCET or a statistical failure-rate claim. |
| Hosted platform matrix | Native execution tests pass on Linux, Windows, and macOS for each claimed OS/architecture; unavailable runners or failing jobs are not counted as support evidence. |
| Hosted compatibility | Existing supported configuration, CLI/API, debugging, HMI, communications, persistence/restart, and applicable online-change workflows pass their platform-specific regressions. |
| Hosted bounded execution | Control-thread/callback allocation instrumentation reports zero allocator operations in admitted normal and fault paths; non-control service allocations are attributed separately. |
| Hosted compositions | Runtime-only excludes compiler/HIR/IDE and unselected services; full/development builds retain the documented capabilities and shared engine. |
| Hosted resource and timing evidence | Before/after results satisfy M0 thresholds or have an explicit reviewed waiver; measurements separate core execution from OS wakeup and I/O/commit lateness. |
| Modernization | The MOD-01 inventory has a disposition for every in-scope dependency/tool; selected upgrades have locked resolutions, migration notes, and affected native/WASM/extension/protocol evidence under MOD-06. Compiler/edition and dependency changes preserve saved PLC observations. Retained constraints and advisory exceptions remain explicit. |
| Rust design boundaries | Reviewed APIs enforce validated construction, distinct clock/handle identities, and one mutable execution owner with borrowed prepared data. Existing malformed-input, aliasing, lifecycle, and fault assertions cover the resulting behavior; pattern names or shorter files are not acceptance evidence. |

An initial **integration gate**, not the complete qualification argument, is a 10 ms periodic task for at least 100,000 cycles: 1,000 seconds, or 16 minutes 40 seconds. Under its declared nominal envelope it shall show zero unexplained faults, zero bounded control-path allocator operations, and no missed configured deadlines. Deliberately injected faults/overruns belong in separately identified tests with explicit expected recovery; they are not successful deadline completions.

This workload is a proposal, not a measured truST result or a claim that 10 ms is suitable for every application. Extended soak and disturbance plans shall be fixed for the actual platform under TEST-03. Passing the integration gate shall not, on its own, qualify long-term reliability, all release phases, or all admitted applications.

Memory limits, acceptable footprint, required completion evidence, and timing headroom shall be frozen at M0 for the actual workload/platform. Do not publish a “64 KB runtime” or similar footprint until a release map, application sizes, per-region peaks, and stack evidence support it. Report runtime-only, application, adapter/helper, and complete-firmware costs separately. File size of an ELF with debug data shall not be substituted for loadable flash sections.

### 14.2 Regression applications

Use three complementary applications: a motor/valve sequence with interlocks and TON/TOF; a tank/pump application with analog values and multiple statically allocated equipment FBs accessed through interfaces; and a bounded stress application covering arrays, strings, nested calls, retain/restart, and fault cases.

These are test workload proposals, not new product/library requirements. Each shall have saved artifacts, fixed input/logical-time traces, and explicit expected outputs, state, and faults under the selected numeric contract.

Include SCHED-05's 25 ms task sampled exactly every 10 ms from a registration baseline of 0 ms, with `SINGLE` FALSE. Its COMPAT-06 oracle includes:

| Nominal due time | Activation sample | `last_run` after activation | Missed intervals |
|---|---|---|---|
| 25 ms | 30 ms | 25 ms | 0 |
| 50 ms | 50 ms | 50 ms | 0 |
| 75 ms | 80 ms | 75 ms | 0 |
| 100 ms | 100 ms | 100 ms | 0 |

Continue the exact 10 ms sample trace through 1000 ms: 40 activations, cumulative overrun count zero. Samples without a due deadline do not advance the periodic baseline or counter. This trace is mandatory in A4's compiler-free fixture and on every claimed host and both boards; the old rule's predicted 33 activations is a defect baseline, not an admitted alternative. With uninterrupted fixed-grid sampling, deadline-to-sample lateness is less than 10 ms. Real scheduling/load and physical completion need their own evidence under §8.4. Keep jump-over-multiple-intervals tests separate: one activation, `n - 1` missed, no replay.

The five control-course lessons with 20 ms tasks on 50/100 ms resource cycles are outside this correction. Their slower sampling still drops activations and counts overruns after the fix; any course configuration change belongs to the course owner.

Extend the corpus with mixed-rate fast/slow tasks at staggered and coincident phases; near-threshold numerical decisions; maximum-size compound operations and nested/indirect calls; malformed programs/imports; first-use and rare failure paths; and hosted online change with active debugger/export readers. The early M2A fixture is a mandatory representative subset, not a replacement for this corpus.

Also pin dynamic local initialization across repeated calls, first-use static state, initialized compound/FB state, and the NUM-05 numeric fixture. Use the same saved artifact/metadata and oracle for M2E. Size the shared workload within declared MCU capacities while retaining required behavior coverage; record larger hosted-only cases separately.

### 14.3 Commands and build gates

The following are proposed validation commands, not commands executed as part of this document update. Plan them in the single consolidated batch for the authorized implementation scope; use the remote builder, isolated target invocations, pinned toolchain/lockfile, and target leases required by the repository. They are not instructions to run cargo on the local editing machine.

```sh
cargo +1.95.0 check --locked -p trust-runtime-core --no-default-features \
  --target thumbv7em-none-eabihf

cargo +1.95.0 tree --locked -p trust-runtime-core --no-default-features \
  --target thumbv7em-none-eabihf -e features

cargo +1.95.0 check --locked -p trust-runtime-core --no-default-features \
  --target riscv32imac-unknown-none-elf

cargo +1.95.0 tree --locked -p trust-runtime-core --no-default-features \
  --target riscv32imac-unknown-none-elf -e features

cargo +1.95.0 test --locked -p trust-runtime --test runtime_core_behavior_lock

cargo +1.95.0 run --locked -p xtask -- architecture-doctor --full-map
```

Add exact linked-firmware commands for `trust-stm32f407` and the selected ESP32 composition, map extraction, and physical runners when those projects exist. Extend the existing Linux/macOS/Windows CI matrix with isolated runtime-only and saved-artifact tests rather than treating existing full-workspace tests as that proof. These checks are not assumed to pass on the reviewed baseline. Core checking alone does not measure size, prove linking, demonstrate bounded execution, or establish hardware readiness.

### 14.4 Hosted resource and performance assessment

**PERF-01.** M0 shall capture per-platform baselines and acceptance thresholds before optimization. Compare equivalent release builds and the same saved artifacts/traces, matching features, checks, scheduling/numeric/import contracts, preparation strategy, toolchain, hardware/OS, clock/cache settings, and I/O/load conditions. A feature-reduced new runtime-only build shall not be reported as an equivalent full-product improvement; report both comparisons separately.

For H's Linux-builder hosted non-regression gate, freeze the following criteria
before measurement; they are new H acceptance criteria, not historical results.
Run `call_binding`, `string_stdlib` and `refs_sizeof` sequentially under the same
Rust 1.99 release profile, full features and enabled register/tier-1 policy,
with 128 warmup cycles and 1,024 measured samples each (1,152 total cycles).
Compile each workload once to saved STBC 1.1 on committed U and load those exact
bytes in both builds. U applies the artifact to its matching source-built context;
H supplies that context only for construction metadata absent from 1.1. Pin the
input artifact and workload/configuration identities. H's internal enrichment
does not make the two internal containers identical. Record and compare every
warmup and measured logical cycle's watched values, task misses and fault state;
matching final values alone is insufficient. Retain all measured latency samples
and compute both quantiles by the same nearest-rank rule. Each workload runs five
paired rounds. A round is disturbed when, for either build, its median latency
differs by more than 10% from that build's median latency across the five rounds;
it is measured once more in the same order and the repeat replaces it. A repeat
that is disturbed again still counts, all samples including the disturbed round
are retained, and the thresholds below are unchanged (owner decision, 14 October
2026). Measure cycle execution
only; collect trace observations outside the timed interval.
Both reports shall show enabled tier-1, positive specialized-block execution and
matching hot thresholds/capacities; required watched values shall be resolved.
Against committed U
`19f3b6787`, every workload must have H/U median latency at most 1.10 in each
final round and H/U p99 latency at most 1.25, where each build's p99 is taken
over the measured samples of its five final rounds pooled; per-round p99 ratios
are recorded but do not decide the gate (owner decision, 14 October 2026). Every
workload must also have identical watched PLC values and cycle counts,
zero budget overruns, positive register execution and zero register-program
fallbacks in both builds. These ratios bound acceptable observed regression;
they do not claim a performance improvement or a real-time guarantee. Reject
missing, non-finite or non-positive latency data. A failed comparison requires
root-cause analysis and correction or an explicitly approved tradeoff; CLI exit
success alone does not satisfy the gate. Retain both raw reports and executable,
workload, compiler and configuration identities. Other native host measurements
remain unqualified until executed on those hosts.

H's private execution lookup caches may be derived directly from the already
validated and prepared grouped indexes. They shall preserve the same owner,
phase/trigger ranges, ordering and sparse identities. Preparation accounting
charges the actual construction algorithm and reserves its complete storage
before allocation; omitting redundant temporary owner lists shall not omit
validation or fault checks. This does not change admitted
language capabilities, hard flash partition bounds, or the reported flash-growth target.

**H acceptance exception, 12 October 2026.** The project accepts the measured
C21 Linux-builder `call_binding` median regression for the candidate and frozen
comparison identified in `docs/notes/runtime-portability/h-execution.md`.
The observed paired H/U median is 1.164359, exceeding the general 1.10 target;
it remains a recorded failure of that target, accepted as a specific trade-off.
The paired p99 is 1.096911 and satisfies 1.25. Reference and string workloads,
exact PLC traces, execution counters and zero overruns satisfy their requirements.
This exception permits H's remaining acceptance work to proceed; it does not
close that work or change the general targets. It grants no waiver for a different
runtime candidate, changed workload/configuration, another platform, physical
I/O completion, deadlines, memory/stack limits or correctness. Non-executable
changes and test-only configuration corrections may inherit this result when
production-source equivalence is recorded. A subsequent runtime change requires
a fresh assessment. Preserve the raw failed comparison and its identities.

Default attribution is extraction first: compare original and extracted code under Rust 1.95.0 with matching features/contracts, separately identifying the explicit portable-math and nominal-deadline scheduling changes. Historical executions of the scheduling defect are not correctness oracles for the corrected contract. Then compare extracted-before-M0U and extracted-after-M0U under their recorded compiler/dependency tuples. If modernization is explicitly scheduled first instead, preserve the modernized-before-extraction baseline. In either order, mixed end-to-end measurements do not identify a single cause, and `rust-version` does not identify a historical build compiler.

**PERF-02.** Report shipped and loadable-code sizes separately; runtime, application, adapter/helper, and service costs; preparation duration/peak memory; per-region steady RUN and compound storage; candidate/update/retirement/checkpoint peaks; CPU/interrupt and VM stacks; control-path allocator events; and core-only/full-workspace build/test cost. Do not confuse bytes stored in flash with extra prepared RAM or temporary loading duplication.

**PERF-03.** Measure release/wakeup lateness, task and batch execution, computation response, logical output submission, driver acceptance, and any observed bus-transfer/device-feedback completion separately. Preserve raw distributions and maxima under declared HMI, network, logging, storage, interrupts, and platform interference. Label unavailable physical observations honestly. Inject blocking, queue overflow, service failure, and delayed output work to verify the defined failure policy.

H diagnostic builds may collect native elapsed time for invocation preparation,
frame initialization, initializer evaluation, execution and cleanup. Collect
fixed-capacity counters during execution and export them afterwards, without
per-operation text formatting or log I/O. Report inclusive and exclusive time,
call counts, collector overflow/unbalanced spans and measured probe overhead.
Instrumented timings locate costs; PERF-01 acceptance uses uninstrumented builds.

**PERF-04.** Smaller artifacts, faster scans, lower jitter, and shorter builds are hypotheses, not consequences guaranteed by extraction. A gate passes only when declared requirements are met and regressions are explained/approved. Record missing evidence as missing. Longer tests, margins on observed maxima, and correct overrun faulting do not establish a mathematical WCET bound, a failure rate, or successful execution within a violated deadline.

### 14.5 Definition of done

**Hosted migration is complete** when Linux, Windows, and macOS products use the shared engine, pass native compiler-free loading and contract-qualified trace tests, retain existing supported workflows including applicable generation changes, qualify their bounded control paths, and provide per-platform baseline-relative memory/timing evidence. Timing classification and unresolved compatibility/runner exceptions shall be stated explicitly; an unresolved exception is not a passed gate.

**The first MCU release is complete** when the STM32F407 boots into its controlled state, accepts an authorized and independently validated application/configuration, prepares it without the compiler, executes the shared VM under admitted resource/scheduling/numeric contracts, meets its required completion level within the recorded timing envelope, handles fault, restart, retain, and interrupted installation as specified, and offers every runtime service of ARCH-11 over its links (ARCH-12), tested from VS Code exactly as a trust-runtime is. Integration, disturbed/extended tests, and per-region resource evidence are all required; the release shall state its timing-evidence classification. A changed first-board selection requires an explicit scope decision and preserves the F401 evidence status.

**The required ESP32 release is complete** when the named ESP32 board/build separately satisfies the same applicable MCU release gates for its declared profile. M1 core checking, firmware linking, or M2E trace parity alone is insufficient.

**The complete platform-support plan is finished only when hosted migration, the selected STM32 release, and the selected ESP32 release are qualified.** The combined v0.5 work also requires completion of Section 16's modernization acceptance. The hosted-plus-F401 delivery may be reported as an intermediate release, with ESP32 and any modernization work explicitly open. An embedded demonstration alone shall not close hosted migration; desktop simulation alone shall not close hardware qualification.

Adding a later board shall primarily require a target/BSP adapter, physical resource configuration, and the same qualification tests—not another VM implementation.

### 14.6 Qualification coverage and evidence package

**TEST-01.** The corpus shall cover mixed-rate nonpreemptive blocking and delayed publication, first invocation/rare branches, maximum admitted strings/arrays and call/reference depth, exceptional arithmetic, input staleness, driver/import failures and hangs, timer wrap/discontinuities, missed-wakeup races, service saturation, interrupted checkpoint/stopped installation, and applicable hosted generation replacement. Each case shall state its profile, expected fault/output behavior, and exact or tolerance-qualified oracle.

**TEST-02.** Every qualification package shall identify the source/build/artifact/configuration hashes, declared contracts, toolchain/flags, board or OS/architecture, clock/cache/memory layout, I/O topology, interference envelope, instrument versions/method, and raw results. Record memory categories from MEM-01/MEM-02 and timing observations from Section 8.4 separately. An observation of driver acceptance shall not be renamed physical output completion.

**TEST-03.** Each platform plan shall define extended soak duration, workloads and release-phase coverage, disturbance levels, allowed maintenance/checkpoint activity, and pass/fail criteria before qualification. Exercise relevant memory/service pressure and update/failure cases, not merely more repetitions of the nominal loop. Explain coverage gaps and residual uncertainty. Duration alone shall not support a mathematical worst-case bound or an unsupported statistical reliability estimate.

**TEST-04.** Qualification shall be separate for every claimed OS/architecture and MCU board/build configuration. Logical replay cannot substitute for actual scheduling, driver, interrupt, reset, watchdog, and physical-I/O evidence. Tests requiring independent supervision shall use actual available mechanisms or mark the target unqualified for that requirement.

**TEST-05.** Device-loader validation shall be fuzzed or systematically mutated across bytecode and bound metadata: section/count expansion, checked-arithmetic overflow, malformed jumps/calls/references/imports, invalid types and generation identities, capacity exhaustion, and application/configuration/profile mismatches. Verify that no unvalidated object becomes executable, no invalid candidate activates outputs, and failed preparation cannot corrupt the installed generation. Also exercise deployment-authorization and rollback policies rather than checking CRCs alone.

The evidence package shall identify whether timing support is measured-qualified or analytically supported, with the actual assumptions and supporting bounds. Defects discovered during extraction shall have their own regression cases and approved behavior changes. No absent runner, unexecuted command, or third-party benchmark shall be counted as truST qualification.

## 15. Decisions still requiring target evidence

The STM32 target is the STM32F407 on the STM32F407G-DISC1 (Appendix D.4); its revision and settings still need recording before hardware work. The NUCLEO-F401RE was the first board. The proposed ESP32-C6 board's availability, exact module/revision, and usable memory remain to be confirmed. Pin HAL/BSP/toolchain versions, actual memory reservations, I/O mapping, management transport, checkpoint medium, interrupt/watchdog policy, and time/space headroom for each build. No measured truST footprint or universal family support is supplied here.

M0 shall also freeze the hosted OS/architecture and service matrix, scheduling/numeric/import contracts, permitted timing-evidence classifications, application authentication/rollback model, and regression/measurement thresholds. Pin the production runtime/adapter build in qualification records; support shall not be inferred from portable Rust source alone.

Scope B determines whether bounded reservation plus prepared metadata fits F401, or whether a targeted shared storage improvement is needed. Account for flash/RAM placement, temporary duplication, native recursion, and IRQ stack headroom. Scope A first establishes LOAD-10's fresh-engine construction and same-dispatcher initialization. Freeze the selected C6 build, portable numeric implementation and explicit qualifier, and remaining build/format details before depending on them; do not infer portability from Cortex-M alone.

The required hosted update window, overlap capacity, generation token/reuse policy, reader lifetime limits, and restart/retain/output behavior shall be frozen before qualifying bounded online change. Preserve legacy supported workflows explicitly while resolving bounded-profile conflicts; do not silently add live MCU changes or no-restart migration.

### 15.1 Deferred alternatives and decision conditions

| Alternative | Position for this phase | Reconsider only when |
|---|---|---|
| Native PLC compilation | Outside the first migration; preserve a reasonable semantic boundary without building a generalized backend framework. | A workload misses an actual requirement and profiling identifies interpretation/dispatch as a significant remaining cost after storage/lookup issues are addressed. Compare total runtime/application/helper footprint, RAM, timing, checks, numerical behavior, and faults. |
| Replace STBC with WebAssembly | Not justified merely for platform portability. | A concrete interoperability, isolation, or maintenance benefit warrants a prototype that passes the same PLC semantic/resource gates. |
| Processor-specific assembly VM | No separate full VM; small reviewed optimizations remain possible under BUILD-05. | A measured hotspot has a useful improvement and retains a portable semantic reference and regression coverage. |
| RTOS or Embassy | A platform integration choice beneath the PLC contracts. | The selected board's drivers, interrupts, networking, waiting, and support requirements justify it; it shall not silently change PLC scheduling. |
| Preemptive or independently committed multi-rate PLC profile | Separate future behavior design, not an MCU-port prerequisite. | Admitted application requirements cannot be met under cooperative-resource-v1, and state/I/O consistency plus migration semantics are explicitly specified. |

These decision conditions reflect the research; alternative projects' footprint or performance results are not estimates for truST. No claim that bytecode or native code is universally smaller/faster follows from the selected architecture. [D02, E05–E09, E16]

**Recommended first implementation:** A1 portable foundations, then A2 loader/validator, A3 STBC 2.0 and A4 shared-engine integration, each on Rust 1.95.0/edition 2021/resolver 2 with one final batch. Together they complete Scope A. Scope B uses that artifact and baseline on NUCLEO-F401RE. Default M0U follows that feasibility checkpoint as its own scope; M2E uses C6 when available. Preserve the wider hosted, bounded-storage, modernization and release gates without combining their validation into A implicitly.

## 16. Modern Rust and dependency modernization

The objective is a current, maintainable, reproducible dependency baseline that supports the declared products and targets. Review every project dependency, then adopt the newest suitable stable release or record the concrete reason for another choice. A larger version number does not demonstrate compatibility, maintenance, security, smaller firmware, or better PLC timing. This section plans future implementation; no repository manifest or lockfile was upgraded during this revision.

### 16.1 Observed baseline and proposed selection

Scope U selects the Rust 1.99.0/edition 2024/resolver 3/MSRV 1.99 and Node
24.21.0 candidates below. The refreshed MOD-01 decisions and pending qualification
are recorded in `docs/notes/runtime-portability/u-dependency-decisions.md` and the
implementation checklist. Baseline observations in this section describe the
pre-modernization source. Selection, installation and lock resolution do not
establish qualification. Every runtime function remains enabled on all existing
platform compositions, with unchanged F401 flash/heap/stack acceptance margins.


The proposed upgrades below apply to M0U only. Scope A/B uses Rust 1.95.0, edition 2021 and resolver 2. M0U is separately authorized, normally after the first host/F401 checkpoint, and cannot silently expand A's validation. Raising MSRV later also requires matching builder/CI/release tools and editor/local analysis toolchains; 1.95 diagnostics or checks no longer validate code requiring 1.99. Build/test execution remains on the builder under repository rules.

The reviewed workspace declares Rust **1.95 as its minimum supported Rust version (MSRV)**, edition 2021, and resolver 2. There is no root toolchain pin. Most native CI jobs select floating `stable`; one MSRV job selects 1.95. These declarations do not establish the compiler that produced a historical binary. CI/release Node jobs currently select Node 20. [R25]

| Area | Proposed modernization baseline | Decision and compatibility consequence |
|---|---|---|
| Rust compiler | Rust **1.99.0**, the stable release published 1 October 2026. | Pin the exact version for development, builder, native CI, and release qualification. Refresh the candidate at implementation start, then freeze it for the evidence batch. [E38] |
| Workspace MSRV | **1.99** as the proposed new common floor. | This is an intentional increase from 1.95, enabling one supported modern baseline. Update package metadata, the MSRV job, contributor documentation, and release compatibility notes together. Do not continue claiming 1.95 support after using newer APIs. |
| Rust edition and Cargo resolver | Edition **2024**, workspace resolver **3**. | Separate source/manifest migration from compiler selection; a compiler upgrade does not change either setting. Set the resolver explicitly in this virtual workspace. Dependency editions need not all match. [E41] |
| MCU build targets | `thumbv7em-none-eabihf` and `riscv32imac-unknown-none-elf`. | Qualify the selected stable compiler against each HAL/startup/linker/dependency graph. C3/IMC is an optional later no-CAS profile; host-only tools and nightly fuzz/Miri instrumentation remain outside the product firmware contract. [E46, E56] |
| Node build tooling | Node **24.21.0**, the observed latest Node 24 LTS patch. | Replace the repository's Node 20 build baseline; the official release table marks 20 EOL and 26 Current. Refresh and pin the supported LTS patch at implementation start. This does not change Node embedded inside supported VS Code releases. [E47] |
| Crates, npm, and build tools | Dated stable candidates in Appendix F and the MOD-01 ledger. | Resolve compatible feature/peer/tool combinations, preserve support requirements, and pin the actual result in lockfiles and tool configuration. A prerelease or floating Git revision needs a concrete requirement. |

**MOD-01.** Maintain a dependency decision table covering all first-party Cargo manifests, all three npm projects and their lockfiles, both fuzz projects, direct and transitive dependencies, Git revisions, vendored/patch overrides, and the tool installations/actions/container bases used by CI, packaging, documentation, and firmware. Each entry or coherent dependency family shall identify its current resolution, candidate, feature/target constraints, migration owner, decision, reason, and acceptance evidence. Use the existing work checklist rather than introducing a management framework. The 74-crate/30-npm inventory is the starting direct-dependency snapshot; it is not an exhaustive audit of tools, transitive advisories, or vendored code. [D05, R25, R26]

Scope includes project-owned tools such as `wasm-pack`, matching `wasm-bindgen` tooling, `cargo-deny`, `cargo-audit`, cross/fuzz/test tools, documentation Python packages, and selected HAL/flash/probe tools. The workflows currently contain both pinned installations and floating installations, including the MkDocs packages. Record these explicitly. Updating unrelated machine packages or user-installed plugins is outside this repository modernization work. Ignored prototype/vendor manifests are not silently converted to first-party product code. [R25]

**MOD-02.** Freeze and record the selected compiler, MSRV, edition, resolver, target triples, linker/SDK/HAL versions, features, and lockfile hashes. Keep the stable product baseline and optional instrumentation toolchains distinct. A selected package or target incompatibility shall lead to an explicit revised candidate or bounded retention decision; do not silently introduce nightly product requirements, lie about target atomics, or lower an existing support guarantee. Publish the MSRV increase with the implementation release, not as an already-completed result of this document.

### 16.2 Edition and dependency migration work

**MOD-03.** Review edition migration as a behavior-sensitive change. In particular, inspect temporary destruction/lock lifetimes, newly unsafe APIs, public interfaces, and edition-affected macros before adopting automatic suggestions. Existing runtime test helpers use process-global `std::env::set_var`/`remove_var`; Rust 2024 makes those calls unsafe. Prefer explicit configuration injection or child-process `Command::env` for the tests' actual need. An automatically inserted unsafe block is not a safety argument, and `forbid(unsafe_code)` boundaries shall remain intact. [R27, E42, E43]

Rust 2024's resolver improves Rust-version-aware selection; it does not prove `no_std`, atomics, deterministic numeric behavior, or firmware fit. Explicit workspace dependency features and separate target invocations remain required by BUILD-03/09. Review WASM link/import behavior too: Rust 1.96 changed undefined-symbol handling for `wasm32-unknown-unknown`. Exercise the actual `trust-wasm-analysis` bundle rather than weakening linker checks to hide migration failures. [E40, E41, E45]

**MOD-04.** Upgrade dependencies in reviewable families, preserving the full requested modernization scope. Allow breaking-version migrations when justified, but include their API/behavior changes and native acceptance work. Cargo-compatible patches and npm peer-compatible updates are candidates, not automatic proof. Review API changes even within `0.x` crate families; preserve exact pins when they encode a still-valid interoperability contract. Resolve transitive problems through maintained parent packages or reviewed replacements before introducing overrides. [E45]

| Dependency family | Work specific to this repository |
|---|---|
| Core collections, strings, errors, and portable math | Select `no_std`/allocation/atomic features explicitly. Updating `smol_str` to 0.3.6 does not remove `Arc`; apply MEM-10/11 ownership work instead of relying on the version bump. Recheck value size, clone/drop behavior, formatting, and numeric helpers on both MCU graphs. [E46] |
| HIR, parser, language protocol, and WASM | Salsa and other API migrations belong in their owning crates. Preserve query invalidation/cancellation, diagnostics, LSP interoperability, generated WASM/JavaScript integration, and the existing syntax/semantic corpus. Keep the compiler out of firmware. |
| Hosted protocols, TLS, I/O, and persistence | Review optional features and real protocol/device compatibility. The existing `deny.toml` lists exceptions involving OPC UA, hosted HTTPS/TLS, and Zenoh dependencies; reassess their actual resolved paths. The observed latest `opcua` is still 0.12.0, so maintenance remediation may require a separately reviewed stack migration. This document neither clears those exceptions nor approves a particular replacement. [R26, D05] |
| Simulation and numeric dependencies | The Rapier upgrade is a simulation behavior migration, including determinism and saved scenarios. Keep it separate from PLC math/executor changes so changed observations have an identifiable cause. |
| VS Code, frontend, and capture tools | Review TypeScript/Vite/language-client major changes, React/editor packages, peers, bundling, and test browser versions together. Match Node API typings to the minimum actual runtime each component supports; the registry's latest `@types/node` is not that runtime. Keep the existing VS Code support contract unless explicitly revised. |
| Git, vendored code, and build/release tools | Review the five OpenOT Git dependency declarations at their pinned revision, vendor patches, CI actions/images, tool versions, and generated assets. Record upstream provenance and local modifications; moving everything to upstream HEAD is not a reproducible update. |

All entries receive an `upgrade`, `replace`, or `retain` decision with a reason. “Already latest” is a version observation, not a security result. Retention is acceptable when the recorded compatibility or maintenance decision warrants it; missing review or failing validation is open work. Preserve advisory expiry/ownership policy and required checks. Do not use `npm audit fix --force`, broad Cargo patches, changed snapshots, or relaxed warnings merely to make the batch green.

### 16.3 Useful Rust features and limits

**MOD-05.** Adopt a language/library feature where it removes a concrete defect risk, clarifies ownership, or replaces necessary custom code. Confirm its minimum compiler, target availability, allocation/work behavior, and semantic effect. Modernization does not require rewriting already-clear code or using every newly stabilized feature. Some useful APIs below already work on 1.95; they are design options, not reasons by themselves to raise the compiler floor.

| Feature or idiom | Concrete use | Bound or review condition |
|---|---|---|
| Rust 2024 `let` chains | Flatten short dependent validation conditions where clearer than nested branches. | Prefer ordinary `match` when the error branches differ; review temporary lifetimes. [E43, E44] |
| Safe `slice::get_disjoint_mut` (stable since 1.86) | Access a small fixed number of distinct slots without a custom unsafe helper. | It checks bounds and overlap with quadratic work in the number of indices. Preserve legal IEC alias behavior using the appropriate sequential/copy path; this API must not redefine the language's aliasing rules. [E55] |
| `core::fmt::NumBuffer` and integer `format_into` (1.98) | Replace custom integer-to-text allocation in a bounded diagnostic or transport renderer when needed. | Retain structured fault codes in the engine; budget buffer/work and avoid adding rendering to the scan merely because formatting can be allocation-free. [E39] |
| `core::assert_matches!` (1.96) | Clear native assertions for admission/fault variants with payload checks. | Apply while authoring relevant tests; avoid unrelated mass rewrites. [E40] |
| Borrowed slices, checked conversions, `Option::take`, and `mem::replace` | Express prepared data, explicit absence, and ownership transfer using existing language/library mechanisms. | Preserve valid state on every return/error path; default values, value copies, and destructor effects still count against MEM-04/06/09. [E53] |

The 1.98 algebraic floating-point methods permit transformations that can change numeric results. They shall not replace PLC arithmetic, math imports, or threshold calculations under the existing numeric contract. An intentional numerical-contract change requires its own decision and Section 12 evidence. New raw-pointer/layout, variadic, or allocation APIs in newer Rust releases do not justify unsafe core code or bypassing bounded storage. [E38, E39]

Use existing standard facilities in the appropriate layer. For example, hosted code already uses `OnceLock`; that is not permission to add lazy first-use work to RUN. Async runtimes, Embassy, and ESP-IDF/FreeRTOS remain platform/service composition choices under Section 15.1. Their availability does not justify making VM opcodes async or changing common PLC scheduling.

### 16.4 Sequence and acceptance

M0U is a separate implementation scope, normally after A/B and before broad M2B. Preserve original, extracted-before-modernization and modernized source/build records with the saved artifacts/oracles. Freeze all M0U changes before its own consolidated batch. This sequence does not require a compiler/edition upgrade before extraction. An explicitly changed ordering still uses separate scopes and equivalent baselines; any new checkout follows the canonical AGENTS/skills bootstrap rule. A successful Rust 1.95 run does not validate 1.99, and neither result transfers automatically to a different firmware build.

**MOD-06.** For the separately authorized M0U scope, finish implementation and necessary tests before one deduplicated builder/platform batch. Include its affected runtime, syntax/HIR/LSP/protocol, isolated MCU graph/link, WASM/browser, extension and dependency/license/advisory checks. This is not Scope A's batch definition. Include physical qualification when making claims for the new tuple; older-board results remain evidence only for their recorded build. Existing release gates apply before a requested push/release. Failed/unavailable checks stay open and another run requires explicit authorization.

Modernization evidence shall freeze the actual tracked and nonignored new source,
deleted paths, and canonical instruction sources. Regenerate tracked assets shipped
inside the runtime or extension before freezing; their bytes remain source inputs.
Reject changed bytes, added source paths, file-kind changes and Unix executable-bit
changes during validation. Freeze regular-file versus symbolic-link identity and
the link target as well as resolved content; retain explicit missing-file records.
Unix executable bits come from the working tree, not only the Git index. Hosts
without Unix mode bits record that absence explicitly. A byte-only historical
snapshot lacks this identity contract and shall not be accepted by the current
checker. Instruction-directory inventory shall reject symbolic-link directories,
including configured roots and their ancestors inside the checkout, rather than
following them. Expected rendered diagrams and public-document
images are separate generated outputs with their own identities. Rust owns this
comparison. Source-freeze manifests shall be stored outside the canonical source
checkout; both creation and checking reject destinations that resolve inside it,
including symlink aliases. Creation shall refuse to replace an existing manifest.
Do not exempt a caller-selected source path from the inventory to accommodate the
manifest. Rust also owns decoding of the existing disposable-database runner's generated
environment file. Treat that file as data, reject malformed/duplicate/unexpected
entries, pass settings only to the gate's child process, and never print credentials.
The required real-database workflow shall invoke that Rust decoder instead of
loading generated credentials into the workflow environment or bypassing it with
the shell gate. Generated settings are authoritative: remove all accepted runner
and database names, including derived CA paths, from the child's inherited
environment before applying decoded values. Missing inputs shall not be satisfied
by stale parent settings. Unrelated build and evidence settings remain inherited.
These tooling contracts do not weaken product tests or establish qualification
from source review alone.

Selected CI tool versions shall replace older cached executables before the gate
uses them. The shared worker-import gate installs its locked frontend dependencies
before importing its build configuration. Browser launch/probe validation has a
bounded lifetime, reports the actual executable/version and startup diagnostics,
and terminates task-owned subprocesses on failure. Rust owns this lifecycle;
Node retains rendered HTML and transport wiring. These prerequisites preserve
every rendered fixture and assertion and do not extend a failing outer timeout.
The browser lifecycle command qualifies Unix-host process groups used by the
builder and Ubuntu editor jobs; unsupported hosts fail before launching a child.
Native Windows runtime CI remains separate from this rendered qualification.

The firmware build command shall retain its newly linked ELF and linker map in
the requested evidence directory and record both identities. A cached Cargo
success does not prove that an undeclared linker-map side effect still exists.
Request the map on the final binary link, require a non-cached binary artifact
and a new nonempty map, and reject occupied evidence destinations. Subsequent
inspection, footprint and installation consume that retained pair rather than
guessing a Cargo target path or accepting a map from an earlier link. These
artifact rules do not change the reviewed firmware flags or overflow policy.

The modernization deliverable is complete when every in-scope dependency/tool has a current disposition, all selected migrations and lockfiles are implemented, the support/MSRV/tool documentation agrees, and the applicable evidence passes. Report retained versions and exceptions plainly; do not label a qualified subset “all packages latest.” Saved PLC traces and NUM-05 outcomes must remain compatible across the selected build changes. Attribute performance changes according to PERF-01 rather than presenting compiler gains as evidence for extraction.

### A4 execution-cost and diagnostic contract

Preparation builds immutable owner/slot/member, initializer, edge-input and root
indexes once, and accounts their memory/work against preparation limits. An ordinary
store or reference lookup must not scan the complete program layout; frame entry
must not rescan the entire initializer table. Lookup work is bounded by the selected
index, inheritance/path depth and the values actually processed. One prepared
standard-library registry is reused for admission, instantiation and restart.

Dynamic-reference type discovery shall use a prepared storage-kind/slot index
when executing the prepared module. Keep every candidate alias in original
reference-table order, then check its path and physical invocation owner;
conflicting selected type metadata remains an error. Legacy or temporary modules
without that index retain the same full discovery semantics. Reject impossible
storage kinds, slots and paths before resolving an instance owner. Index lookup
and candidate/path visits are bookkeeping under TIME-08 and are not charged.

A native output transaction preserves atomic copy-back by staging or journaling
only its actual destination values, including aliases and suspended-frame targets.
Sampling stages only input-bound values. Sampling and output publication do work
proportional to the bindings and data actually encoded, never to unused bytes of
the admitted image; this is bookkeeping under TIME-08 and is not charged, except
the TEXT normalization of a sampled STRING/WSTRING value. A 3,000-
declaration program executing 2,000 scalar stores in a scan, a resource sampling
2,000 distinct scalar input bindings, and a resource with a 1 MiB marker image
and sparse bindings must run under the default work limit;
raising that limit is not a substitute for removing whole-state work.

There is one authoritative work budget per execution entry, shared across nested
calls, initializers and helpers. TIME-08 defines what it charges and TIME-09 which
allowance a construction entry uses. Allocation charges remain a separate quantity.
Physical deadline checks use a bounded work stride and explicit entry/completion
checks; charging each individual work unit does not itself call the physical clock.
For a cycle or typed engineering write, the completion deadline is checked after
fallible staging and immediately before committing outputs or the destination.
A failed deadline check must leave publication/destination state unchanged; there
is no fallible post-publication deadline check that reports failure after commit.
Native callbacks still cannot be forcibly interrupted by these cooperative checks.
Borrowed reference loads/stores must not allocate an owned path merely to run a
policy check. Native allocation assertions cover the hosted field-reference path.

Optional no_std caches are accelerators: a failed interior borrow is a cache miss,
not a runtime panic. Dedicated fault identities in spec 12 §11.5.8 distinguish
storage protections, lifetime/visibility violations and profile/resource rejection.
Runtime state groups lifecycle, work, process-image and construction bookkeeping;
internal VM representation is exposed only through the hosted compatibility facade
and documented portable entry points.

A4 correction evidence includes 2.0 execution of TOF, TP, counters, triggers,
bistables, falling edges, hierarchical I/O, an injected mid-cycle deadline and a
forged dynamic initializer store denied outside staging. Physical board replay
belongs to scopes B/E and does not form part of RTP-A4-02 software closure.

## 17. Rust design patterns for this runtime

A4 execution ownership: each source-free invocation has a checked live activation
identity that no other activation of its top-level entry uses (§4.3.1). A nested
call temporarily transfers its caller's local vector
into the engine's reference-addressable storage, then restores that same vector on
both successful and failed return. No local snapshot is substituted for live aliasing.
References to the current activation resolve against its active frame; references to
suspended ancestors resolve against the transferred storage. Releasing an activation
invalidates its references. Initialization staging has a distinct activation identity,
and recursive aggregate lifetime checks apply before writes, returns and commits.
Legacy hosted sentinel references remain confined to the 1.x compatibility adapter.



The **Rust Design Patterns** book is a useful community reference for tradeoffs and idiomatic implementation. Rust's API Guidelines, Edition Guide, and Embedded Rust Book provide complementary guidance. The choices below are truST design recommendations derived from those sources and the reviewed code; the books do not prescribe a PLC architecture or prove this implementation. [E48–E55]

**DESIGN-01.** Choose a pattern to solve a named problem at an existing boundary. Prefer direct structs, enums, functions, ownership, and small traits; introduce an abstraction only when it clarifies current responsibilities or enforces a required invariant. The shared engine and thin adapters remain the architecture. Clearer code and preserved behavior take priority over line count, pattern count, or speculative future flexibility.

| Pattern / idiom | Where it fits this plan | Keep the implementation small and correct |
|---|---|---|
| Validated construction; a small typestate boundary | `DecodedCandidate` becomes an executable prepared module only through the checked LOAD-07/09 preparation path. Private fields and `Result` constructors prevent a production bypass. | Reuse the real validator. Types distinguish raw and admitted data, while bytecode/resource checks still occur at runtime. No general typestate framework is required. [E52, E54] |
| Newtypes and meaningful enums | Distinct logical/physical time, durations, program/slot/type identities, and generation-aware handles in Sections 6/7/11. | A type alias does not provide this distinction. Validate conversion, width, wrap, and generation at boundaries; preserve the explicit wire encoding. Use a nonzero type only when zero is actually forbidden. [E49] |
| Exclusive ownership with borrowed immutable data | One owner of `ExecutionState` borrows `PreparedModule` metadata; compact IDs address prepared storage under MEM-10/11. | Keep thread synchronization in the hosted owner/adapter where required. Avoid cloning to silence the borrow checker, while retaining PLC value-copy, snapshot, and lifetime semantics. [E53] |
| Small mechanism traits and explicit composition | Implement clock, process-image I/O, persistence, waiting, and supervision ports in host/F401/ESP32 adapters. | Inject concrete implementations when composing the runner. A borrowed `&mut dyn Port` does not require heap allocation; choose static or dynamic dispatch where its actual call/code-size cost matters. Do not parameterize every opcode or add a dependency-injection container. [E50] |
| Enums and exhaustive transitions | Runtime STOP/RUN/FAULT, installation candidates, and generation-switch results. | Use explicit transition functions and the existing state contracts. Runtime events still need runtime checks; compile-time types are most useful at construction boundaries, not as a second controller-state model. [E49, E54] |
| RAII and explicit fallible completion | Scoped hosted resources, installation staging, and off-control-path generation retirement. | Keep required persistence/output commit explicit and error-reporting. `Drop` must not introduce blocking I/O, hidden flushes, allocator work, or unbounded destruction on bounded paths. [E52] |
| Move/take/replace instead of incidental clones | Ownership transfer and reuse of prepared scratch/state where lifetimes allow it. | Account for replacement defaults, error recovery, and dropped old values. Never turn a required independent PLC value into shared mutable storage just to remove a clone. [E53] |
| Plain configuration, with a builder only when useful | Board/engine configuration and preparation outside RUN. | A constructor plus a configuration struct is sufficient for small APIs. Add a builder only when real optional/dependent inputs make it clearer; keep validation centralized and do not add a builder dependency by default. [E51] |

**DESIGN-02.** Type design shall reinforce the existing memory, admission, fault, time, and generation contracts without replacing their runtime checks. Public APIs shall make ownership and fallibility apparent. Keep domain fault/error records independent of human-readable host formatting and transport serialization; required diagnostics stay fixed and bounded. Place malformed-input, aliasing/copy, capacity, and state-transition assertions in the native owning tests when those behaviors change. Do not write tests that merely assert the presence of a pattern or wrapper.

**DESIGN-03.** Review each shared-core/platform change against these boundaries and the repository's architecture checks. Keep preparation separate from bounded execution, transport separate from PLC policy, and one shared implementation of IEC semantics. Use existing modules and crates unless another boundary has a concrete responsibility. Maintain relevant ownership/data-flow diagrams with implementation. General backend registries, generic state-machine engines, visitor frameworks for every opcode, broad `Arc<Mutex<_>>` conversion, and whole-VM async conversion need an actual requirement; none is part of this plan.

Apply the Ponytail simplicity guidance already recorded in AGENTS.md together with these choices. Preserve requested scope, error handling, useful modules, and acceptance evidence. Modern syntax and shorter code are useful only when they make the required behavior easier to understand and maintain.

---

## Appendix A — Source register

Repository references R01–R19 retain commit `be8d81a4a7ab16ca7554b8be0f4723161ec1a47b`. R20–R37 record 9 October source checks at `9a15065725c17da2c912055f1509368d3fd01d6c`, before the corresponding specification edits; dependency-source rows identify locked versions separately. Use the original base URL for R01–R19 and the recorded current HEAD for later repository paths. These are source findings, not a full audit or reproduced execution evidence.

Repository base URL:

```text
https://github.com/johannesPettersson80/trust-platform/blob/be8d81a4a7ab16ca7554b8be0f4723161ec1a47b/
```

| Reference | Repository path / reviewed area |
|---|---|
| R01 | `Cargo.toml` — workspace members, dependencies, edition/MSRV, release settings. |
| R02 | `crates/trust-runtime-core/src/lib.rs` — portability boundary and modules. |
| R03 | `crates/trust-runtime-core/Cargo.toml` — default/std/hir feature structure and dependencies. |
| R04 | `crates/trust-runtime/Cargo.toml` — hosted product dependencies/features. |
| R05 | `crates/trust-runtime/src/lib.rs` — host ownership and compatibility surface. |
| R06 | `crates/trust-runtime/src/runtime/core.rs` and its included submodules — host Runtime state; the reviewed top-level file has 151 lines. |
| R07 | `crates/trust-runtime/src/runtime/vm/mod.rs`, lines 1–240 — dispatch dependency and VM metadata. |
| R08 | `crates/trust-runtime-core/src/vm/stack.rs` — operand stack storage and push behavior. |
| R09 | `crates/trust-runtime-core/src/value/types.rs`, lines 1–230; `value/mod.rs` — allocated compound model and HIR-gated helpers. |
| R10 | `crates/trust-runtime/src/io/driver.rs` — process-image and health contracts. |
| R11 | `crates/trust-runtime/src/scheduler.rs`; `scheduler/clock.rs`, lines 1–180 — hosted runner and clock responsibilities. |
| R12 | `crates/trust-runtime/src/runtime/cycle.rs`, lines 1–280 — cycle order, debug hooks, retain path, task execution. |
| R13 | `docs/internal/testing/checklists/runtime-core-host-split-execution-checklist.md` — opening scope/non-goals and Phase 1 behavior-lock sections. |
| R14 | `crates/trust-runtime-core/src/bytecode/mod.rs`, lines 1–150; `crates/trust-runtime/src/bytecode/mod.rs` — version, reader/errors, existing format ownership. |
| R15 | `crates/trust-runtime/tests/runtime_core_behavior_lock.rs`, lines 1–190 — compiler-built fixture initialization and I/O boundary fixture. |
| R16 | `crates/trust-runtime/src/runtime/vm/budget.rs` — instruction-budget accounting. |
| R17 | `crates/trust-runtime/src/runtime/io_subsystem.rs`, lines 1–220 — fault-output attempts and reporting. |
| R18 | `crates/trust-runtime-core/src/vm/mod.rs`, lines 1–180 — shared helpers and tests. |
| R19 | `crates/trust-runtime/src/runtime/online_change.rs` — pinned implementation/contract examined in the 8 October research review; scheduler-boundary replacement, warm restart, and retain handling. |
| R20 | `crates/trust-runtime/src/runtime/bytecode.rs`, especially `apply_bytecode_module`, `apply_resource_metadata`, and `validate_task` — loading assumes programs/instances already exist. |
| R21 | `crates/trust-runtime/src/runtime/vm/local_init.rs` and `local_init/expr.rs`; `crates/trust-runtime-core/src/bytecode/format/refs_consts.rs` — HIR/harness initialization dependency and constant-only `init_const_idx` metadata field. |
| R22 | `crates/trust-runtime/src/memory.rs` — named storage and `RwLock` lookup caches. |
| R23 | `crates/trust-runtime/src/stdlib/numeric.rs`, especially `unary_real` and `expt` — promoted `f64` evaluation and hosted math functions. |
| R24 | `Cargo.lock`, workspace/core manifests, and locked `smol_str` 0.2.2 `src/lib.rs` — direct/transitive `Arc` dependency; actual target graph remains to be built. |
| R25 | Root/workspace and fuzz Cargo manifests; `editors/vscode`, `crates/trust-runtime/web/ide-frontend`, and `scripts/captures` package manifests/lockfiles; `.github/workflows/{ci,release,docs-captures,demo-pages,salsa-hardening}.yml`; `justfile` — compiler/MSRV/edition/resolver, package and CI tool baselines. D05 lists the 21 inventoried package manifests. |
| R26 | `deny.toml`, Cargo Git declarations, and `[patch]`/vendor configuration — existing advisory exceptions and source policy, not a newly executed audit. |
| R27 | `crates/trust-runtime/src/runtime/vm/register_ir/tests/support.rs`, `runtime/vm/register_ir/tests/tier1/state_deadline_buffers.rs`, hosted OPC UA/OpenOT test helpers — process-global environment mutation relevant to Rust 2024 migration. Hosted security/trace/web sources also already use `OnceLock`. |
| R28 | Core `program_model/ops/numeric_arith.rs`, `ops/time_ops.rs`, `ops.rs` and numeric promotion helpers — floating pow/trunc calls, finite/narrowing checks, and native mixed-type/overflow tests. |
| R29 | Core `program_model/{expr,initializers}.rs`, `value/defaults.rs`, `bytecode/{mod,format/pou,format/types,format/refs_consts}.rs`; host `runtime/vm/local_init*`; specification 12 §§4.2/6.11/7.11/8 — HIR initializer execution, address-keyed cache, existing metadata, and version/unknown-section rules. |
| R30 | Host `runtime/vm/{call,frames,stack}.rs`; core `vm/limits.rs`, `bytecode/limits.rs`, and `value/types.rs` — native recursion, dynamic storage, host-scale limits, and target layout measurement needs. |
| R31 | Host local-init/call/stdlib/error/stack paths and `memory.rs`; core `value/types.rs`, `vm/const_pool.rs`, `error.rs`, `retain.rs` — allocation/refcount/name-lookup sites summarized in Appendix E. |
| R32 | Host `runtime/vm/register_ir.rs` and its lowering/interpreter/tier modules; `docs/internal/testing/checklists/architecture-improvements.md`, ARCH-VM-19/20/27/28/31 — optimized hosted baseline, preparation/profiling split, and historical benchmark criteria. |
| R33 | `runtime/vm/dispatch.rs`, `scheduler/clock.rs`, resource runner/handle, `runtime/cycle.rs`, retain snapshot/store/codec — 32-instruction stack polling, no-op hosted wake, same-thread supervision, synchronous pre-output persistence. |
| R34 | `runtime/core.rs`, host manifest, and specifications 11/12 — unconditional hosted composition, inaccurate clock-only portability sentence, and current versus planned format contracts. |
| R35 | Core `value/types.rs`, `retain.rs`, `vm/const_pool.rs`, and applicable tests; cached `indexmap` 2.14.0 `src/map.rs` and `rustc-hash` 2.1.2 `src/lib.rs` — no_std hasher/constructor requirements, std-only aliases, ordered behavior and exposed map APIs. |
| R36 | Host `bytecode/{format/module,decode,validate,encode,metadata}.rs` and submodules; reader/util compatibility re-exports, runtime apply/VM, six version-assumption integration surfaces and the tracked OSCAT `.stbc` fixture — complete relocation/version scope and crc32fast use. |
| R37 | Cargo.lock and cached `libm` 0.2.16 manifest/architecture sources, `crc32fast` manifest — existing libm resolution through glam/num-traits/simba, default arch selection, and the additional no_std CRC dependency edge. |

External references E01–E04 retain the 6 October provenance, E05–E23 the 8 October research, and E24–E62 the 9 October platform/modernization reviews. E59/E63 were consulted for the follow-up numeric qualification. Older entries are not asserted to be wholly refreshed; none supplies truST execution measurements.

**E01 — Embedded Rust Book, no_std.** Distinguishes the portable core from hosted facilities and optional allocation.

```text
https://doc.rust-lang.org/stable/embedded-book/intro/no-std.html
```

**E02 — embedded-hal 1.0 documentation.** Peripheral driver traits, fallible APIs, scope boundaries, and companion I/O crates.

```text
https://docs.rs/embedded-hal/1.0.0/embedded_hal/
```

**E03 — Cargo features and dependency resolution.** Dependency defaults and feature unification; separate invocations for isolated feature sets.

```text
https://doc.rust-lang.org/cargo/reference/features.html
https://doc.rust-lang.org/cargo/reference/resolver.html
```

**E04 — rustc target documentation.** Cortex-M4/M7 bare-metal target family and CPU/FPU distinctions.

```text
https://doc.rust-lang.org/rustc/platform-support/thumbv7em-none-eabi.html
```


### Design input references

Historical inputs and artifact hashes are preserved in the
[research notes](https://github.com/johannesPettersson80/trust-platform/blob/main/docs/notes/runtime-portability/research-notes.md). They are
non-normative provenance; no build or implementation step depends on files in a
personal directory.

**D01 — Original proposal.** Version 0.2; archived identity in the research notes.

**D02 — Architecture research.** The 8 October research review and its amendment mapping are retained in the notes.

**D03 — Version 0.3 input.** Historical design input; superseded by this specification.

**D04 — Version 0.4 input.** Earlier target/implementation review; superseded decisions are identified in the review archive.

**D05 — Dated dependency snapshot.** [Registry inventory](https://github.com/johannesPettersson80/trust-platform/blob/main/docs/notes/runtime-portability/trust-runtime-modernization-inventory-2026-10-09.json), SHA-256 `6a088873d343cced6c536442fdd32e63cf0687017c64bf62ad12cea4d1e9f6d0`. The 9 October snapshot covers 74 direct registry crates, 30 npm packages and five Git declarations across 21 manifests. Refresh candidates in M0U; this is not a resolved upgrade set or advisory audit.

**D06 — Version 0.5 and earlier review.** Historical modernization input and the 17-finding v0.4 review; archived with their original evidence limitations.

**D07 — Version 0.6 and follow-up review.** The eight-finding review, source checks, dispositions and original v0.6 hash are recorded in the research notes.

The following primary sources were cited in D02 as consulted on 8 October 2026. They support design rationale, not newly reproduced tests, release compatibility, or a certification claim. Historical/archived material is identified. This revision incorporates that supplied research rather than claiming another online verification.

| Reference | Source and relevant scope | Address |
|---|---|---|
| E05 | CODESYS, device-manufacturer overview: modular runtime and native application generation. | `https://www.codesys.com/device-manufacturers/why-codesys/` |
| E06 | WAMR maintainers, project README and platform-porting guide: engine/product/compiler boundaries and execution modes. Component footprints are not truST firmware estimates. | `https://github.com/wasm-micro-runtime/wasm-micro-runtime` ; `https://github.com/wasm-micro-runtime/wasm-micro-runtime/blob/main/doc/port_wamr.md` |
| E07 | Beremiz project overview: compilation through C/native runtime organization. | `https://beremiz.readthedocs.io/en/latest/overview.html` |
| E08 | IronPLC runtime overview: bytecode and cooperative scheduling. Not evidence of industrial maturity. | `https://www.ironplc.com/reference/runtime/overview.html` |
| E09 | Wasmi Labs, execution-engine redesign, 28 May 2024: historical external/internal representation separation. | `https://wasmi-labs.github.io/blog/posts/wasmi-v0.32/` |
| E10 | ROS 2 design guidance, introduction to real-time systems: historical preparation/timed-execution guidance. | `https://design.ros2.org/articles/realtime_background.html` |
| E11 | CODESYS task documentation: priority/scheduling choices. | `https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_f_reference_task.html` |
| E12 | CODESYS bus-cycle documentation: task buffers versus bus transfer and I/O consistency. | `https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_buscycle_task.html` |
| E13 | Wasmtime Config API: fuel/epoch mechanisms and blocking native-call limits. | `https://docs.wasmtime.dev/api/wasmtime/struct.Config.html` |
| E14 | Rust f32 documentation: numeric/NaN behavior. | `https://doc.rust-lang.org/core/primitive.f32.html` |
| E15 | Wasmtime deterministic-execution guidance: numeric/import/environment contracts. | `https://docs.wasmtime.dev/examples-deterministic-wasm-execution.html` |
| E16 | Embassy framework overview: platform/executor choices and cooperative execution. | `https://embassy.dev/` |
| E17 | Wasmtime Pulley interpreter: internal execution representation. | `https://docs.wasmtime.dev/examples-pulley.html` |
| E18 | CODESYS online-change documentation: pointer/relocation cautions and restart semantics different from truST's pinned contract. | `https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_cmd_online_change.html` |
| E19 | Linux kernel real-time hardware guidance: cache/memory/platform interference. | `https://docs.kernel.org/core-api/real-time/hardware.html` |
| E20 | Microsoft driver scheduling guidance: Windows preemption/timing limitations, not qualification of a particular real-time extension. | `https://learn.microsoft.com/en-us/windows-hardware/drivers/kernel/always-preemptible-and-always-interruptible` |
| E21 | Apple Kernel Programming Guide, archived: historical soft-real-time scheduling discussion, not current-release qualification evidence. | `https://developer.apple.com/library/archive/documentation/Darwin/Conceptual/KernelProgramming/scheduler/scheduler.html` |
| E22 | Wasmtime minimal embedding: separation of execution from shipped compilation. Configuration-specific sizes are not transferable. | `https://docs.wasmtime.dev/examples-minimal.html` |
| E23 | MCUboot design: image validation and interrupted updates, not a complete PLC application authorization protocol. | `https://docs.mcuboot.com/design.html` |

Additional primary sources consulted on 9 October 2026:

| Reference | Source and relevant scope | Address |
|---|---|---|
| E24 | ST, NUCLEO-F401RE product page: board model, expansion, onboard ST-LINK. | [NUCLEO-F401RE](https://www.st.com/en/evaluation-tools/nucleo-f401re.html) |
| E25 | ST, STM32F401RE product page and STM32F401xD/xE datasheet: CPU/FPU, flash/SRAM capacities, peripherals. | [Product](https://www.st.com/en/microcontrollers-microprocessors/stm32f401re.html); [datasheet](https://www.st.com/resource/en/datasheet/stm32f401re.pdf) |
| E26 | ST UM1724, Nucleo-64 MB1136: SWD/ST-LINK, board routing, LED/button/UART and solder-bridge configuration. Actual board revision must still be checked. | [UM1724](https://www.st.com/resource/en/user_manual/um1724-stm32-nucleo64-boards-mb1136-stmicroelectronics.pdf) |
| E27 | ST RM0368: STM32F401 memory/flash, timers, watchdog, debug and peripheral behavior. | [RM0368](https://www.st.com/resource/en/reference_manual/rm0368-stm32f401xbc-and-stm32f401xde-advanced-armbased-32bit-mcus-stmicroelectronics.pdf) |
| E28 | stm32-rs maintainers, `stm32f4xx-hal`: `stm32f401` device support; exact HAL/PAC version still to be pinned. | [stm32f4xx-hal](https://github.com/stm32-rs/stm32f4xx-hal) |
| E29 | Espressif, Rust hardware/tooling guidance: RISC-V versus Xtensa and C3 bare-metal target setup. | [Architectures](https://docs.espressif.com/projects/rust/book/introduction/hardware-overview.html); [C3 tooling](https://docs.espressif.com/projects/rust/no_std-training/02_2_software.html) |
| E30 | Rust 1.95.0 RV32IMC target definition: `atomic_cas = false`; source-level constraint, not a truST build result. | [Pinned target source](https://raw.githubusercontent.com/rust-lang/rust/1.95.0/compiler/rustc_target/src/spec/targets/riscv32imc_unknown_none_elf.rs) |
| E31 | `portable-atomic` maintainers: optional CAS/critical-section support and `portable-atomic-util`; an implementation option requiring platform review. | [API and feature guidance](https://docs.rs/portable-atomic/latest/portable_atomic/) |
| E32 | Rust `libm` maintainers: portable math functions for both float widths; no claim of equivalence to truST's current numeric results. | [libm](https://docs.rs/libm/latest/libm/) |
| E33 | Espressif, Rust ecosystem: `esp-hal`, radio runtime requirements, and `esp-rtos` integration. | [Crate overview](https://docs.espressif.com/projects/rust/book/introduction/ancillary-crates.html) |
| E34 | esp-rs maintainers, ESP-IDF Rust bindings/HAL/services: alternative platform integration and dependency boundary. | [ESP-IDF Rust workspace](https://github.com/esp-rs/esp-idf) |
| E35 | Earlier ESP-IDF interrupt guidance for its documented ESP32 configuration; background on flash/cache and IRAM handling, not qualification of the selected C6 bare-metal stack. | [Interrupt allocation](https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/intr_alloc.html) |
| E36 | Earlier ESP32-C3-DevKitM-1 proposal: module/flash, USB-UART, pin/strapping details. The v0.6 first ESP32 proposal is C6; neither board's possession is asserted. | [DevKitM-1](https://docs.espressif.com/projects/esp-dev-kits/en/latest/esp32c3/esp32-c3-devkitm-1/user_guide.html) |
| E37 | ST AN4839: Cortex-M7 cache/DMA coherence for an optional later H7 profile; not the F401 memory model. | [AN4839](https://www.st.com/resource/en/application_note/DM00272913-.pdf) |

Additional modernization and Rust design sources consulted on 9 October 2026:

| Reference | Source and use | Primary reference |
|---|---|---|
| E38 | Rust Release Team: current stable compiler and 1.99 API changes. | [Rust 1.99.0](https://blog.rust-lang.org/2026/10/01/Rust-1.99.0/) |
| E39 | Rust Release Team: integer formatting and algebraic floating-point semantics. | [Rust 1.98.0](https://blog.rust-lang.org/2026/08/20/Rust-1.98.0/) |
| E40 | Rust Release Team: `assert_matches!` and WASM undefined-symbol behavior. | [Rust 1.96.0](https://blog.rust-lang.org/2026/05/28/Rust-1.96.0/) |
| E41 | Rust Edition Guide: edition selection and Rust-version-aware resolver 3, including virtual workspaces. | [Rust 2024](https://doc.rust-lang.org/edition-guide/rust-2024/index.html), [Cargo resolver](https://doc.rust-lang.org/edition-guide/rust-2024/cargo-resolver.html) |
| E42 | Rust Edition Guide: process environment mutation becomes unsafe; automatic migration does not establish safety. | [Newly unsafe functions](https://doc.rust-lang.org/edition-guide/rust-2024/newly-unsafe-functions.html) |
| E43 | Rust Edition Guide: `if let` temporary lifetimes and destructor/guard review. | [Temporary scope](https://doc.rust-lang.org/edition-guide/rust-2024/temporary-if-let-scope.html) |
| E44 | Rust Edition Guide: edition-gated `let` chains. | [Let chains](https://doc.rust-lang.org/edition-guide/rust-2024/let-chains.html) |
| E45 | Cargo maintainers: feature unification/defaults and SemVer compatibility rules. | [Features](https://doc.rust-lang.org/cargo/reference/features.html), [SemVer compatibility](https://doc.rust-lang.org/cargo/reference/semver.html) |
| E46 | Rust 1.99 target source and `smol_str` 0.3.6 implementation: upgrading does not remove the atomic ownership issue. | [RV32IMC target](https://raw.githubusercontent.com/rust-lang/rust/1.99.0/compiler/rustc_target/src/spec/targets/riscv32imc_unknown_none_elf.rs), [smol_str source](https://docs.rs/smol_str/0.3.6/src/smol_str/lib.rs.html) |
| E47 | Node.js maintainers: LTS/Current/EOL status and observed latest LTS patch. | [Node.js releases](https://nodejs.org/en/about/previous-releases) |
| E48 | Rust Design Patterns maintainers: pattern tradeoffs; community implementation guidance. | [Book introduction](https://rust-unofficial.github.io/patterns/), [Pattern selection](https://rust-unofficial.github.io/patterns/patterns/index.html) |
| E49 | Newtypes, meaningful types, and enum API guidance. | [Newtype](https://rust-unofficial.github.io/patterns/patterns/behavioural/newtype.html), [Rust API Guidelines: type safety](https://rust-lang.github.io/api-guidelines/type-safety.html) |
| E50 | Small interchangeable mechanisms and borrowed dynamic dispatch. | [Strategy](https://rust-unofficial.github.io/patterns/patterns/behavioural/strategy.html), [On-stack dynamic dispatch](https://rust-unofficial.github.io/patterns/idioms/on-stack-dyn-dispatch.html) |
| E51 | Builder tradeoffs and staged object construction. | [Builder](https://rust-unofficial.github.io/patterns/patterns/creational/builder.html) |
| E52 | Scoped resource ownership, validated constructors, and dependable destructor behavior. | [RAII](https://rust-unofficial.github.io/patterns/patterns/behavioural/RAII.html), [Rust API Guidelines: dependability](https://rust-lang.github.io/api-guidelines/dependability.html) |
| E53 | Borrowing and explicit ownership transfer; avoid incidental cloning. | [Cloning to satisfy the borrow checker](https://rust-unofficial.github.io/patterns/anti_patterns/borrow_clone.html), [mem::replace](https://rust-unofficial.github.io/patterns/idioms/mem-replace.html) |
| E54 | Embedded Rust Book: compile-time construction/state invariants and typestate. | [Typestate programming](https://docs.rust-embedded.org/book/static-guarantees/typestate-programming.html) |
| E55 | Rust library documentation: safe checked disjoint mutable slice access and its work bounds. | [slice::get_disjoint_mut](https://doc.rust-lang.org/std/primitive.slice.html#method.get_disjoint_mut) |
| E56 | Espressif's documented stable bare-metal C6/H2 target and Rust 1.99 IMAC target source. | [ESP Rust toolchains](https://docs.espressif.com/projects/rust/book/getting-started/toolchain.html), [RV32IMAC source](https://raw.githubusercontent.com/rust-lang/rust/1.99.0/compiler/rustc_target/src/spec/targets/riscv32imac_unknown_none_elf.rs) |
| E57 | Espressif C6 device and proposed DevKitC-1 board; separate HP/LP memory and board/module configuration. | [C6 datasheet v1.5](https://www.espressif.com/sites/default/files/documentation/esp32-c6_datasheet_en.pdf), [DevKitC-1 guide](https://docs.espressif.com/projects/esp-dev-kits/en/latest/esp32c6/esp32-c6-devkitc-1/user_guide.html) |
| E58 | libm maintainers: observed 0.2.16 API and architecture/intrinsic feature selection; not a cross-target accuracy certificate. | [libm documentation](https://docs.rs/libm/latest/libm/), [configuration source](https://docs.rs/crate/libm/latest/source/configure.rs) |
| E59 | Rust floating-point library contract: operation-specific guarantees and unspecified precision of power/transcendental methods. | [f64 documentation](https://doc.rust-lang.org/std/primitive.f64.html) |
| E60 | Pierre Roux, 2014, formal double-rounding results with format/rounding hypotheses, including underflow. | [Innocuous Double Rounding of Basic Arithmetic Operations](https://jfr.unibo.it/article/view/4359) |
| E61 | ST DS10086 Rev 5 §3.16/Table 45 and RM0368 Rev 6 §§3.3/3.5: backup registers, erase-sector layout, operation timing and stalled flash reads. | [Official datasheet](https://www.st.com/resource/en/datasheet/stm32f401re.pdf), [Official reference manual](https://www.st.com/resource/en/reference_manual/rm0368-stm32f401xbc-and-stm32f401xde-advanced-armbased-32bit-mcus-stmicroelectronics.pdf) |
| E62 | IEC 61131-3:2013 §6.6.2.5.8, Tables 28/29; variable/POU initialization context in Tables 14/19/40/47/48. Numerical accuracy dependencies must be stated; no universal ULP bound is prescribed. | Local standard text `docs/internal/standards/iec61131-3.txt`, numerical clause around lines 5762–5890; copyrighted standard is not added to Git. |
| E63 | Rust Reference numeric types, complementing operation-specific library contracts and recorded floating-point premises. | [Numeric types](https://doc.rust-lang.org/reference/types/numeric.html) |
| E64 | Pinned cortex-m-rt 0.7.5 linker script: vector placement, `_stext`, read-only data and `.data` load address constraints. | [Startup linker source](https://docs.rs/crate/cortex-m-rt/0.7.5/source/link.x.in) |
| E65 | LLVM address-significance metadata and LLVM 22.1.2 ELF ICF implementation; section folding is broader than Rust function-pointer comparisons alone. | [Address-significance tables](https://llvm.org/docs/Extensions.html#sht-llvm-addrsig-section-address-significance-table), [LLD ICF source](https://raw.githubusercontent.com/llvm/llvm-project/llvmorg-22.1.2/lld/ELF/ICF.cpp) |
| E66 | Rust documents that function-pointer address equality is not a unique function identity; this does not prove unrestricted whole-image folding safe. | [fn_addr_eq](https://doc.rust-lang.org/std/ptr/fn.fn_addr_eq.html) |
| E67 | Rust 1.95 supports source path-prefix remapping. It is a normalization measure, not by itself proof of reproducible firmware. | [Pinned compiler options](https://raw.githubusercontent.com/rust-lang/rust/1.95.0/src/doc/rustc/src/command-line-arguments.md), [Path remapping](https://doc.rust-lang.org/rustc/remap-source-paths.html) |
| E68 | Rust ownership and builder guidance: separate construction from use and transfer ownership without a surviving mutable borrow. Consulted 11 October 2026; design guidance, not PLC qualification. | [Rust Book: references and borrowing](https://doc.rust-lang.org/book/ch04-02-references-and-borrowing.html), [Rust API Guidelines: builders](https://rust-lang.github.io/api-guidelines/type-safety.html#builders-enable-construction-of-complex-values-c-builder) |
| E69 | CODESYS distinguishes one-time writes and persistent forces with defined execution timing. Its exact task-boundary policy is not adopted by truST. Consulted 11 October 2026. | [Forcing and writing variables](https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_forcing_values.html) |
| E70 | Beckhoff documents address changes during online change and pointer update conditions. This motivates explicit binding validity; truST retains its own warm-restart contract. Consulted 11 October 2026. | [POINTER](https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2529453451.html), [Online change](https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2528041355.html) |

## Appendix B — Revision history

| Version | Date | Change |
|---|---|---|
| 0.36 | 19 October 2026 | §7.4 (PORT-05): the rules of the client, the host side of the byte-stream link (checklist L1a, USB as a way to reach a runtime). Opening a serial port sets DTR on, then RTS off, and nothing else, so the open does not reset the chip; a request longer than the 8,192-byte frame is refused before it is sent, with the chip's own text; text on the port is skipped, 65,536 bytes of it at most; a wait is 5 seconds, and 120 seconds for the reply to `program.install`; a reply head is read whole with `transfer.get`, checked against its CRC-32 and SHA-256, and aborted with `transfer.abort` on any failure; a port that went away (a reset or a power cycle enumerates it again) is reported plainly. Specification 11 §6.8.1 names the `serial://` endpoint and `ctl install`. |
| 0.35 | 18 October 2026 | STATE-08 and §11.2: a computer behaves like a chip when it is stopped (owner, 18 October 2026, decision Q11). `shutdown` stops the program and keeps trust-runtime running in the Stopped state, with its control server, web UI and other services; `restart` starts the installed program again, loading the bundle that the project folder names; the process ends on SIGINT or SIGTERM or by the explicit exit `shutdown` with `{"exit": true}`, which a chip answers as `shutdown`. A computer reports `program_control` `keeps_control`. An install, `program.install` and the web deployment alike, is accepted only while the resource is Stopped. |
| 0.34 | 18 October 2026 | Writes the owner's four debugging decisions (checklist S3, debugging on chips): the debug service (SERVICE-06) resolves breakpoints and stepping from the module's optional line table (specification 12 §6.13) on every target, needs no ST text, no compiler, no HIR and no expression parser, evaluates the compiled expression form (specification 11 §6.13.2), reads values from the published debug state (specification 11 §6.13.1), and keeps a hold until a continue or a stop (specification 11 §6.9, STATE-07). HOST-09's debug-expression row and LOAD-06's optional debug sections record it; no function is reduced on any target. |
| 0.33 | 18 October 2026 | The STM32U5A9 is the STM32 chip (owner, 17 October 2026): ARCH-10 and §1.2 name it. The STM32F407 track of 16 October 2026 is paused, and its chip file, adapter, firmware and evidence are kept (D.4). Appendix D.5 records the chip, the board, the adapter, the clock, the identity and the pins; its link, flash, RAM, console, firmware and evidence rows are left to S1b-8u-2, S1b-8u-3 and S1b-9u. Appendix H names `chips/stm32u5a9.toml` and the family `stm32u5`; H.2, H.5 rules 2, 4, 5, 7 and 8, and H.6 cover it. The chip joins the catalog, `build-firmware` and `inspect` with its firmware crate in S1b-9u. |
| 0.32 | 18 October 2026 | Appendix D.2, the C6 memory bullet: the main-stack window is 24 KiB, not 16 KiB, because the S1b-5b2 bench measured a 15,740 B high-water in the 16 KiB window, leaving 644 B untouched, below the 2 KiB headroom; the linker guard and the build check require the window plus a 4 KiB margin (28 KiB). Appendix D.4 keeps the STM32F407's 16 KiB window. STATE-08: a request that arrives while the next boundary is due waits for that boundary, and a boundary that is due again as soon as it is taken lets one request through per boundary, so a scan longer than its interval cannot starve the link; the list of remaining differences says a due cycle can wait for the one request being answered, where it said eight requests in one pass. |
| 0.31 | 18 October 2026 | §6.3, after MEM-09: construction reserves, per call depth, the call scratch that binds a call — its evaluated argument list, parameter flags, output bindings, preflighted targets and preflighted outputs, sized for the largest call site and the largest POU — and a call takes its depth's scratch and returns it on every path (completion, trap, refusal and abort), so a scan allocates no argument, flag, output-binding, target or prepared-output vector. Every result, charge and refusal is unchanged (TIME-08). |
| 0.30 | 18 October 2026 | §10, "The device runner": the chip's installed program holds the checkpoint persister over the checkpoint store its platform supplies; it captures the retained values and offers them at each scan's pre-output-sample checkpoint, the resource loop advances one step between scans with the store's per-step program budget so no step runs inside a scan, a stop or shutdown completes the save before the resource reports Stopped, and a latched failure fails the next scan with `RuntimeError::RetainStore` before physical outputs. A warm start decodes the newest committed checkpoint with the shared codec and applies it by name (specification 11 §6.7); an empty log or a `mode = "none"` project starts with the declared initial values. |
| 0.29 | 17 October 2026 | §7.4 (PORT-05): the control link on a byte stream, such as a chip's USB serial port: line framing as trust-runtime's socket, frames of up to 8,192 bytes on every chip's USB link, an oversized request dropped to its line feed and answered once with `request longer than <frame> bytes`, a reply longer than the frame replaced by `reply longer than <frame> bytes` until requests and replies travel in parts with program loading's transfer, `events.tail` and `faults` written event by event within the frame, one reply in flight, a reply without progress for 1 second dropped with the line-feed repair, a new host session, and the link's counters. STATE-08: a chip without an installed program, or with one it refused, serves the control connection in the Stopped state and refuses `restart` with `no installed program: <reason>` until a program is installed; a warm start after `shutdown` gives the RETAIN variables the values they had when the program stopped (IEC 61131-3 §6.5.6.1). |
| 0.28 | 18 October 2026 | §6.3, after MEM-09: construction reserves one frame payload per call depth (that depth's local slots and parameter-presence bits, sized for the largest POU) and one edge-input list and restore list, and a user or method call takes the locals of its frame from the reserved payloads; an activation binds its arguments into the payload, reads edge inputs and their restores from the reserved lists and returns the payload when its frame is popped on every path, and a scan allocates no locals, presence, edge-input or restore vector. Construction also reserves the bookkeeping scratch: initializer result slots, seed lists and owned-instance snapshots, the lifetime check's visited set, and `SIZEOF(type)` walks a fixed stack. Every result, charge and refusal is unchanged (TIME-08). §10, "Between scans": a chip's checkpoint persister starts a checkpoint when the save is due and the image changed, advances it one log step at a time between scans, completes it at a stop, and latches a failure as `RuntimeError::RetainStore`. |
| 0.27 | 17 October 2026 | §6.3, after MEM-09: preparation resolves every `CALL_NATIVE` site once into a call plan (each actual's formal index, the presence pattern, the output descriptors, and the callee: a function's prepared POU, a function block's POU, a method through the prepared owner and method table, a built-in function block's kind and parameters); a scan resolves no argument names and builds no uppercase type-name key, and an interface attempt compares prepared POUs while still charging the type-name bytes TIME-08 prescribes. Every result, charge and refusal is unchanged (TIME-08). |
| 0.26 | 17 October 2026 | §6.3 (MEM-02, MEM-09): construction sizes the per-call reservations to the program's static call depth instead of `max_call_depth`, counting every call target from every entry, a method and interface call over every implementing POU, and a nested instance declaration as the initialization it runs; a recursive call graph has no static depth and reserves for `max_call_depth`. `max_call_depth` remains the admission ceiling and the runtime depth check is unchanged. |
| 0.25 | 17 October 2026 | §6.3, after MEM-09: construction reserves the stack executor's operand, frame and pending-call storage from the operand demand that the validator records, the program activation's local slots, the initializer stacks, the output copy-back journal and the process-image staging, and charges them against the construction allocation allowance; RUN charges and refusals are unchanged (TIME-08). |
| 0.24 | 17 October 2026 | §5.1.1: the deployment record, `deployment-record.bin` next to `program.stbc`: its version-1 layout of six sections, the program, configuration and retain-schema identities, how a reader refuses a malformed record (`runtime_deployment_record_invalid`) and how a later version adds sections; the admission order that refuses a deployment before any state exists (`runtime_deployment_refused`); the device descriptor read by admission. HOST-02: the hosted product takes its limits from the record. |
| 0.23 | 17 October 2026 | §10.1: the checkpoint log in the chips' two checkpoint slots: the slot header, the record with its commit marker, the order of a write, and recovery, in which a damaged newest committed record fails the warm start, as a damaged retain file does on a computer. Appendix H: the checkpoint slots refer to §10.1. §5.2.1: the version-1 contract of every built-in function and function block (LOAD-06, PORT-02), with its IEC table, TIME-08 charges, failures and capability; preparation refuses an import without a contract or one that does not match it with `runtime_import_unsupported`, naming the import. §4.3.1 and §11.3 (MEM-07, UPDATE-04): every reference carries the installation generation it was taken in; each committed restart and replacement installs the next one, a resource-only reconfiguration keeps it, and exhaustion refuses the restart with `Overflow`; a reference of another generation names no state, a staged request that names an instance by identity carries its generation, and the host binds its own references again at each restart. |
| 0.22 | 17 October 2026 | ARCH-15: several runtimes on every host (owner, 17 October 2026). STATE-08 states the device runner's boundary rules: `restart` after `shutdown` starts the program as a new trust-runtime process would, a `restart` while running, paused or faulted restarts in place as trust-runtime does, and pause, resume, restart and shutdown take effect at the program's next boundary. TARGET-04: the USB source is a passive listing. TARGET-06: a wall-clock value is `null` while the device time is not set. §10: a variable located at `%M` publishes its initial or retained value over the bytes it covers after the configuration actions (specification 11 §6.7). HOST-02, §4.3.1, §6.2.1 and §17 A4: activation and per-call instance identities are reused per top-level execution entry, which removes the `Overflow` fault after 2^32 − 1 activations; they stay monotonic within an entry. Appendix D.2: the ESP-IDF v5.5.1 QIO bootloader. Appendix D.4: the STM32F407 adapter, the crystal measurement, the console and the evidence profile. Appendix H: the chip file format of TARGET-03. |
| 0.21 | 16 October 2026 | Owner corrections: adds the owner's requirements; the boards run the same runtime services as trust-runtime (ARCH-11) over any link (ARCH-12) without limits of their own (ARCH-13); ARCH-14 makes the runtime a set of components and gives each chip one firmware with every component it can hold, development boards being examples only; STATE-07 replaces the board restriction on forcing and breakpoints; removes the stopped-only installation, the physical commissioning authorization, the disabled radios and the exclusion of online change and protocol parity, which the v0.3 draft carried without the owner's approval; ARCH-02 now requires shared service code instead of excluding it; the STM32 target becomes the STM32F407 chip, firmware is developed for chips rather than boards, and network runtimes are found by the common discovery (ARCH-10, ARCH-12, Appendix D.4); adds Bluetooth as a link and a service, and §11.4: the same project files as Linux, `device.toml` for the settings an operating system holds, chip description files (one firmware per chip, no board files), installation and update of truST, versions, time, diagnostics and safe outputs (TARGET-01 to TARGET-09); SERVICE-03 selects one shared OPC UA implementation for every target; SERVICE-04 and SERVICE-05 put OpenOT telemetry (over the network) and simulation on the chips; TARGET-10 records that chips get no read-out protection. |
| 0.16 | 13 October 2026 | Owner-approved decision makes the 16 KiB F401 upper-flash growth target informational for hardware bring-up; the executable-region bound, sector reservations, and all other memory gates remain enforced. |
| 0.15 | 12 October 2026 | Selects uniform eight-byte temporal storage and SIZEOF in STBC 3.0, explicit legacy-layout imports, checked retained-layout compatibility, and management-write alias validation. Preserves every platform function and the external four-byte TIME codec. H corrections are authored, with review and validation tracked separately. |
| 0.14 | 11 October 2026 | Selects HOST-09's construction-only raw mutation, checked live writes, isolated debug parsing and transactional schema replacement. Explicitly narrows live Rust mutable-reference API compatibility without reducing PLC functions or hosted workflows. H implementation and native acceptance remain pending. |
| 0.13 | 10 October 2026 | Plans B-R2 sector-0 immutable firmware placement, explicit discontiguous load accounting and a 16 KiB upper-region bring-up margin. Preserves all functions, full admission, stable toolchain and overflow/fault handling. Measurements and implementation remain separately authorized. |
| 0.12 | 9 October 2026 | A3 admission proves staging-only writes across control flow; producer prunes unreachable standard blocks and shares recipes only between equivalent contexts. Adds edge/retain/partial/class authoring coverage and preserves the source/wire distinction. Execution remains A4. |
| 0.11 | 9 October 2026 | Clarifies the shared changing-input fixture as a reference initializer selecting persistent array storage under IEC §6.4.4.10.2–3; preserves existing scalar constant-expression source checks. A4 must prove per-call initialization; A3 authoring is not execution evidence. |
| 0.10 | 9 October 2026 | Moves the dependency-free SCHED-05 implementation and native regressions to the start of A1, separately reviewed within its existing batch. Removes deferred-defect baseline acceptance; A4 keeps fixture/oracle integration. Four scopes remain; no implementation or validation is asserted. |
| 0.9 | 9 October 2026 | Folds the periodic scheduling defect into Scope A/A4 as SCHED-05; aligns specifications 10/11, selects nominal deadlines for SCHED-01/CYCLE-03, and adds the 25 ms / 10 ms native and shared-artifact oracle. Implementation, validation, version bump and release remain deferred to the authorized implementation scope. |

Version 0.8 divides aggregate Scope A into A1–A4, with one final batch per explicitly
bounded scope, and adds the implementation checklist/current-checkpoint pointer.
The product requirement identifiers and architecture remain unchanged.

Version 0.7 separates M0U from Scope A/B, specifies portable map/hashers and full
bytecode relocation, bounds legacy-version compatibility, retains libm's default
feature with explicit numeric premises, and separates product requirements from
dated research. The [complete revision history](https://github.com/johannesPettersson80/trust-platform/blob/main/docs/notes/runtime-portability/research-notes.md#revision-history-through-v06)
preserves earlier snapshots and decisions.

Version 0.17 (14 October 2026) replaces H's per-structure charge rules with the
TIME-08 cost model: instructions plus value-size work, with engine bookkeeping
never charged. TIME-09 moves construction to its own preparation-profile
allowance. Specifications 10 §runtime cycle and 12 §4.6 follow.

Version 0.18 (14 October 2026) adds HOST-04's guarantee for in-place leaf calls
and direct frame defaults: the general path's results, faults and charges, with no
handler storage reserved for a leaf call. PERF-01 gains the owner-approved rule for
disturbed rounds; its thresholds are unchanged.

Version 0.19 (14 October 2026) evaluates PERF-01's p99 over each build's five
final rounds pooled, by owner decision; per-round medians and both thresholds
are unchanged. Specification 32 names the hosted binding transaction that
installs MQTT mappings.

Version 0.20 (14 October 2026) records the corrections from the A01 coverage audit.
TIME-08 states each text, reference-growth, `SIZEOF` and `STRUCT OVERLAP` rate,
counting text in UTF-8 bytes; standard-library text work is charged (TIME-02) and
reference growth no longer depends on storage capacity. The STBC 3.0 checksum covers the
header (specification 12 §4.1). A zero work limit is unsupported. The
local-initializer acceptance case is worded to agree with specification 03 (E202).

## Appendix C — Amendment traceability

The [research amendment mapping](https://github.com/johannesPettersson80/trust-platform/blob/main/docs/notes/runtime-portability/research-notes.md#earlier-research-amendment-traceability)
is historical provenance. Current implementation timing is defined by §13.2;
requirement identifiers in this specification remain authoritative. Review labels
such as REV-MODEL-05 are not product requirement identifiers.

## Appendix D — Target decisions and first-board feasibility

### D.1 NUCLEO-F401RE: first target until 16 October 2026

The owner selected the STM32F407 as the STM32 target on 16 October 2026 (Appendix D.4). This section remains as the record of scope B and its evidence. The firmware described here (`firmware/trust-nucleo-f401re`), the adapter's `stm32f401` feature and the `nucleo-f401re` board of `xtask portability` were retired 17 October 2026 (owner) and remain in git history.

NUCLEO-F401RE is the selected first reference board. Its role is to establish compiler-free execution and useful resource evidence on hardware already available. It is not yet a supported truST controller. [E24]

| Area | Initial decision | Evidence or remaining implementation decision |
|---|---|---|
| Board identity | NUCLEO-F401RE / STM32F401RE. | Before flashing, record probe identity, electronic device/revision/UID/flash-size registers, and the confirmed board model. Remote bring-up may proceed with PCB revision and package marking explicitly uninspected; do not represent electronic detection as physical inspection. Record those markings when available for final board qualification. |
| CPU/toolchain | Cortex-M4F; `thumbv7em-none-eabihf`; single-precision hardware floating point. | Pin Rust/LLVM and CPU/FPU flags; record actual clock. Up to 84 MHz is the part limit, not a measured or configured runtime speed. [E25] |
| HAL/runner | Start with `stm32f4xx-hal` and its `stm32f401` feature, architecture startup support, and one bare-metal runner owning the engine. | Pin compatible HAL/PAC/startup versions. No RTOS is required for the initial GPIO/timer/serial composition; any later executor remains outside PLC semantics. [E28] |
| SRAM | 96 KiB total device SRAM. | Deduct `.data`/`.bss`, CPU/IRQ stack, loader/preparation storage, all engine regions, queues, I/O buffers, and declared margin. Account for preparation and RUN peaks separately. Do not reserve desktop VM maxima. [E25] |
| Flash | 512 KiB total device flash. | Partition firmware, saved/deployable application, installation metadata, and durable retain/recovery storage at actual erase-sector boundaries. Count numeric helpers and all loadable sections. [E25, E27] |
| Clock/wake | A hardware timer supplies extended monotonic time; the runner implements PORT-03 wait/wake. | Select timer, prescaler, resolution, rollover extension, interrupt priorities, and reset epoch. Validate clock startup and wake races. |
| Initial physical I/O | Board user button and LED through process images; proposed logical bindings `%IX0.0` and `%QX0.0`. | UM1724 maps the relevant button to PC13 and LED/D13 to PA5 for this board. Confirm bridges/polarity on the actual revision; normalize them in the BSP. A physical smoke check supplements the richer M2A trace fixture. [E26] |
| Programming/management | Onboard ST-LINK/SWD for firmware bring-up; the control connection over USB, through the ST-LINK virtual COM port. The F401 has no network port; a network link needs an STM32 board with Ethernet, which the owner selects (ARCH-13). | Verify ST-LINK virtual COM routing and UART setup; distinguish probe-flashed firmware from the stopped STBC installation required for release. [E24, E26] |
| Fault supervision | Independent watchdog and explicit BOOT/STOP/FAULT output behavior. | Record watchdog clock/tolerance, timeout, debug-freeze behavior, and reset output state; service only under supervisor policy. [E27] |
| Persistence | Internal flash stop checkpoints; last completed compatible checkpoint survives power loss. | No last-scan retention promise. Specify separate valid/new checkpoint storage, interrupted-write recovery, endurance, and watchdog servicing during maintenance. External storage or power-fail hardware is needed only if the stronger selected policy requires it. [E61] |
| Numeric behavior | Preserve `REAL`/`LREAL` widths and NUM-05 evaluation rules. | Budget software double precision on M4F; no silent narrowing to make the fixture fit or run faster. |

**Feasibility decision.** Start with this board. Scope A establishes host construction and cross-target source/layout evidence; Scope B determines whether the shared application and preparation peak fit with stack/interrupt and driver headroom. Measure nested-call MSP high-water, not just the logical VM frame count. The desired result includes timers, bounded compound state, interface dispatch, and correct faults. No fixed footprint or maximum application size is promised before measurement.

**Scope B reference composition.** Freeze Rust 1.95.0, the hard-float Cortex-M4
target, HSI/PLL configured to 84 MHz, and the HAL/PAC/startup versions in the
firmware lockfile. TIM2 supplies 1 MHz monotonic ticks and SysTick supplies a
1 kHz non-allocating interrupt workload. Serial evidence uses USART2/ST-LINK VCP
at 230400 baud. Configured clock frequency is not independently calibrated clock
accuracy. Extend TIM2 rollover and require observations less than one full counter
period apart; record the reset epoch. GPIO polarity belongs to the board adapter.
M0U (11 October 2026) later moved the workspace and this firmware to Rust 1.99.0, edition 2024
and resolver 3. F401 board evidence recorded after M0U, including the 15 October 2026 replays,
uses that tuple, which the C6 composition shares (D.2).

The L2 link layout reserves sector 0 for vectors, sector 1 (16 KiB at
0x08004000) for a separately programmable application bundle, sectors 2 and 3
(two independent 16 KiB erase units at 0x08008000 and 0x0800C000) for future
checkpoint slots, and sectors 4–7 (448 KiB from 0x08010000 through 0x0807FFFF)
for executable firmware. Reserving checkpoint slots does not implement persistence,
checkpoint capacity admission or interrupted-write recovery. L2 replaces the
measured, failed L1 layout; its extra 96 KiB is declared capacity, not code reduction. The linker and host ELF inspector reject overlaps; a linked image may
not consume application or persistence space to conceal a fit failure. RAM
reserves a 72 KiB allocator arena, a 16 KiB MSP region and up to 8 KiB for other
statics. Admission work/allocation counters are cumulative logical charges and
must not be confused with measured live heap or native stack peaks. Report both.

**B-R2 placement contract (qualification pending).** Sector 0 may also hold
immutable firmware data after the complete, aligned vector table, ending before
0x08004000. The vectors remain at 0x08000000 and the reset handler/code remain in
sectors 4–7. This uses otherwise reserved capacity; it is not an engine-size reduction.
The application and two checkpoint erase units remain unchanged. Sector-0 data belongs
to the firmware image and must be installed/verified with its matching vectors/code;
application installation must preserve it. Linker assertions, ELF/load-segment checks
and the footprint report must account for both firmware regions separately, including
alignment and the RAM data load image. No segment or flattened firmware image may
silently program the intervening application/checkpoint sectors. Actual linked ELF
segments establish placement; equal section flags alone do not establish installability.
Use explicitly selected read-only sections with an overflow assertion, not an assumed
linker policy that automatically fills remaining space. [E61, E64]
The canonical size build may additionally normalize the core source root and
individual Cargo registry roots to distinct short namespaces, preserving relative
file names and recording every original-to-normalized mapping in its build report.
Private standard-library descriptor arrays may have named static storage so the
linker can select those complete read-only inputs, decimal-formatting tables and
compiler lookup tables explicitly. All descriptors/functions remain present; these
are representation and placement changes, not capability restrictions. Do not select
anonymous sections by unstable compiler hashes. Retain all sector bounds and prove
the final allocation in the map and ELF; shortened paths alone are not upper-region
code savings.

B-R2's hardware-bring-up flash gate is that loadable firmware stays within the upper
448 KiB executable region after alignment and preserves a valid sector-0 bound and all
reserved application/checkpoint regions. Report total occupied flash and free bytes per
region. Report whether the 16 KiB upper-region growth target is met, but do not reject a
physically in-range bring-up image solely for missing that target. The target does not
promise that future persistence/installation functionality fits and is not release
qualification. Retain stable Rust 1.95.0, all functions, full admission, overflow checks and panic/fault
recording. Safe ICF and normalized build paths may be the single default firmware
configuration after review and measurement; unrestricted ICF, nightly/build-std,
RUSTC_BOOTSTRAP, immediate-abort panics and capability cuts are outside B-R2. [E65–E67]

The reference firmware shall use the pinned compiler's default machine-outliner
pass count. Additional `machine-outliner-reruns` settings are not admitted: the
extra pass produced a Thumb helper that overwrote its incoming return address,
confirmed by the linked instructions and a hardware breakpoint. Build tooling
shall reject that override. Keep safe ICF, all IEC functions, admission checks,
overflow checks and panic/fault records. The 16 KiB growth target remains
informational for bring-up; actual flash bounds stay mandatory. Retain exact
compiler/flag identities and require hardware execution when qualifying future
code-generation changes; a smaller linked image is not execution proof.

The application bundle has magic `TRSTB002`, three little-endian u32 lengths at
offsets 8/12/16 and their CRC32 values at offsets 20/24/28. Its payload starts at
byte 32: the unchanged A4 main artifact, unchanged numeric artifact, then a GPIO
probe artifact. Check every length/range and CRC before preparation. This trusted
bench installation format is not the release authentication/deployment contract.

The additional GPIO artifact samples `%IX0.0` and publishes `%QX0.0`; it also
provides one through four nested POU levels for native stack measurement. Keep
the original A4 artifacts and oracle unchanged. Autonomous tests distinguish
actual PC13 samples from injected input-image values; both can drive real PA5
output readback. Unobserved button presses and optical LED inspection remain
unverified. Repaint only unused stack below the live MSP with interrupts masked,
preserve earlier peaks, and record at least 2 KiB observed stack/IRQ headroom.
These finite measurements do not claim exhaustive worst-case stack bounds.

Run the 25 ms tasks on hardware-timed 10 ms samples through 1000 ms and export
observed logical time, state, nominal deadlines and missed counts. Measure scan
time separately from serial export. Check BOOT/STOP/FAULT output-off readback,
then withhold independent-watchdog service deliberately and confirm its reset
flag and safe boot before declaring the run complete. A partial trace, missing
reset, explicit fault record, capacity failure or missed oracle row fails this
bring-up gate; never replace it with a host simulation result.

If the F401 does not fit, preserve the failed map/peak ledger and identify whether executable metadata, duplicated code, compound storage, initialization scratch, native stack, or linked helpers dominate. Prepare a targeted common-core improvement or an explicit application/target decision under the authorized scope; do not automatically rerun validation. The same failed gate remains open until an authorized run succeeds. An H7 may later provide a larger supported profile while the F401 result remains separately visible.

Internal-flash installation and persistence shall not erase the running firmware or the only recovery path. The reserved layout must preserve independent application installation; firmware and PLC application shall not become inseparable simply to fit the first board. A stopped recovery procedure is permitted under DEPLOY-03, but arbitrary power-loss retention of the last scan is not implied.

RM0368 describes four 16 KiB, one 64 KiB, and three 128 KiB sectors for this density; flash reads can stall during erase/program. DS10086 Table 45 lists erase times on a millisecond-to-second scale, with voltage/parallelism-dependent limits. Budget maintenance/watchdog behavior using the selected conditions. The part offers RTC backup registers, not a general backup-SRAM store. Do not reserve “256 KiB firmware + two 128 KiB sectors” before measuring linked size and proving the application/checkpoint recovery scheme fits; a single erase unit alone does not provide two independently recoverable checkpoint slots. [E61]

**Scope B-R1 footprint contract.** Preserve every currently admitted IEC operation,
standard function, error identity and device-side validation check. Firmware overflow
checks remain enabled. The expanded B-R1 scope includes compact preparation diagnostics,
targeted engine/retained-graph key representation reductions and measurement of safe
identical-code folding. It excludes the full M3 storage redesign and capability cuts. Use one compact CRC-32/ISO-HDLC implementation in
no_std core and firmware; the hosted std composition may retain crc32fast acceleration.
Both routes must produce identical wire checksums. Default standard-library descriptors
shall be immutable shared metadata; preserve custom hosted registration, replacement,
lookup and parameter signatures. Refactor only measured collection/sort sites with an
explicit ordering/capacity/work argument and native equivalence tests. Changes must not
reintroduce quadratic admission work or uncharged input-dependent operations. Dense or
sorted indexes are chosen per owner, not substituted wholesale for ordered containers.
The minimum runtime retains its current IEC capabilities throughout this scope.

Preparation/decode/admission diagnostics may use fixed reason identities and bounded
numeric context instead of allocating formatted text. Preserve rejection conditions,
stable public error codes and the information needed for hosted diagnostic rendering;
record any Rust diagnostic-API adaptation. Hosted messages remain available. Dynamic
application names or values must not be silently discarded when they distinguish a
fault. Narrow engine keys only with checked, injective encodings preserving identity,
ordering, generation/lifetime handling and the existing resource-accounting bounds.
Safe identical-code folding is an explicitly measured build choice, not an assumed
saving or permission to disable checks. Retain unchanged traces and fault behavior.
In the allocator-backed no_std API, section diagnostics retain typed reasons/context
and compare structurally; explicit rendering reproduces the hosted message. Hosted
`BytecodeError` text payloads and runtime error conversion stay compatible. Native
regressions shall assert both stable code and rendered detail across compositions.
`StandardLibrary::get` may return a borrowed function/signature view rather than an
owned-entry reference; registration, replacement, case matching and all signatures
stay identical. These are Rust representation adaptations, not PLC feature differences.
Scheduler sorting is bookkeeping bounded by the admitted task count and is not
charged (TIME-08); preserve priority/due/index ordering.


**Uniform PLC function contract.** Every supported platform composition shall retain
the same currently supported IEC operations, standard functions, conversion directions
and truST assertion operations. Neither a device profile nor `no_std` may remove these
functions to meet footprint limits. This includes REAL/LREAL to and from STRING/WSTRING,
integer conversions and inferred/generic forms. Hardware resource limits and physical
I/O bindings remain explicit platform properties; they do not authorize a reduced PLC
function set. If the full runtime cannot fit a target, report that target as unsupported
or unqualified until shared implementation improvements satisfy its limits. Never make
the existing program corpus smaller to manufacture a successful hardware result.

B-R1 evidence shall retain a release linker map even on failure, compare actual total
and per-family footprint to the first B map, and report allocator peaks separately from
static reservations. Estimated savings are not acceptance evidence. One final build
cannot attribute an isolated causal saving to each overlapping source edit. If linked
firmware passes partition checks, execute the unchanged board corpus in the same batch.
An informational dependency exception requires an owner, explicit rationale/removal
condition and expiry of at most 90 days; audit both root and standalone firmware lock
graphs. No desktop dependency may be hidden by moving the adapter out of the workspace.

### D.2 ESP32: required second architecture

| Area | Proposed first profile | Decision to freeze or evidence required |
|---|---|---|
| Board/chip | ESP32-C6-DevKitC-1 with 8 MB flash, HP RV32IMAC core; owned since 15 October 2026. | M0 record: chip revision v0.2, eFuse block revision v0.3, 40 MHz crystal, chip ID 13, base MAC 20:6e:f1:17:04:c0, secure boot and flash encryption disabled; the factory flash image is retained with its SHA-256. Keep one PLC engine owner on the HP core. Another MCU family cannot satisfy ARCH-10. [E56, E57] |
| Rust/HAL | `riscv32imac-unknown-none-elf` with `esp-hal`; Wi-Fi as a network link (ARCH-12). | Pin compiler/HAL/startup/flash tools and linker layout. This is a stable bare-metal target, not the similarly named ESP-IDF target. [E56] |
| Atomic/ownership boundary | IMAC permits existing atomic-backed values during measured bring-up; no new atomic RMW dependency in core. | Retire them in the shared bounded-storage slice before M3. C3/IMC becomes an additional gate after that work if selected; portable-atomic alone does not rewrite `alloc::sync::Arc` consumers. [R24, E46, E56] |
| Numeric cost | No F/D floating-point extension on the selected target; REAL and LREAL use software arithmetic. | Measure numeric helpers/latency on C6 independently of F401; an ARM single-precision result is not a C6 estimate. Optional C3 also needs software floats. [E56, E57] |
| Memory/flash | Explicit usable internal-memory and linker/partition budget. | Record code/data/stack/driver reservations, preparation peaks, and actual module flash. Do not assume external RAM or treat LP memory as freely interchangeable HP engine SRAM. [E57] |
| Runner and I/O | One engine owner, timer wakeups, staged I/O, the control connection over Wi-Fi and USB. | Interrupts/callbacks notify the runner or fill staging buffers; they do not independently run PLC tasks or mutate its active state. Freeze pin/strapping and reset behavior. |
| Radio/services | Wi-Fi carries the control connection and the other network services (ARCH-11, ARCH-12). | Include the radio scheduler, interrupts, buffers, contention, and load in memory/timing qualification. Espressif documents background runtime requirements for its radio stack. [E33] |
| Alternative ESP-IDF composition | Available as an explicit platform integration choice if required drivers/services justify it. | Bind IDF, Rust wrapper, FreeRTOS/task, and synchronization versions; use the same core and PLC scheduler. Do not pull the desktop `trust-runtime` product into MCU firmware. [E34] |
| Flash interference and supervision | Forbid installation/erase during RUN in the initial profile. | Qualify later persistence against the C6 stack/memory placement. E35's earlier ESP32 IDF guidance is background, not C6 bare-metal proof; review the selected implementation before permitting flash work in RUN. |
| Milestones | M1 isolated core check, M2E linked firmware and saved-artifact execution, M5 qualification. | Missing hardware leaves physical gates open while independent host/F401 work proceeds. |

**C6 reference composition (M2E).** The C6 runs the same M2E fixture as F401: the same
application bundle bytes, oracle, record grammar and phase order, through the shared fixture crate
(Section 3.2). Only the board adapter and the values below differ. They were measured on the owned
board by two M0 probes on 15 October 2026 before any firmware was written.

- **Toolchain.** Rust 1.99.0, edition 2024, resolver 3 and `riscv32imac-unknown-none-elf`, the
  tuple of the F401 firmware, retired 17 October 2026 (owner). `esp-hal` is pinned to 1.2.2 with
  the `esp32c6` and `unstable` features; the timer-group watchdog, SYSTIMER alarm, reset reason,
  persistent RTC memory and USB-Serial/JTAG drivers are unstable API in that release.
  `esp-bootloader-esp-idf` 0.6.0 supplies the application descriptor and `esp-storage` 0.10.0
  the flash reads. The bench host flashes and reads back with `espflash` 4.6.0.
- **Bootloader.** M2E used the second-stage bootloader bundled with `espflash` 4.6.0, which
  reports `ESP-IDF v6.1-beta1-497-g14f663f003e`. Since 17 October 2026 the C6 uses a bootloader
  built from the ESP-IDF v5.5.1 release tag, configured for QIO and 8 MB (sha256
  `f5c0819843648a6692a563a4b2c933c9473fda7e42e7c13627f2acffb34208ce`). As ESP-IDF does for QIO,
  its image header says DIO: the chip's ROM loads the bootloader in DIO, and the bootloader sets
  the flash's quad-enable bit and switches to QIO itself. It is flashed with `espflash flash
  --bootloader <file> --flash-mode dio`; forcing a QIO header (`--flash-mode qio`) makes the ROM
  stop with `ets_loader.c 67`, which was the QIO failure seen with the beta bootloader. The
  M2E firmware ran with it in DIO and in QIO with the same records as with the beta bootloader
  apart from timing values.
- **Clock.** `CpuClock::max()` sets 160 MHz. SYSTIMER (16 MHz, 52 bits) supplies monotonic
  microseconds without a rollover extension. The machine performance counter CSR 0x7E2 counts CPU
  cycles from boot: 16,003,071 cycles in 100,018 µs. A 1 kHz SYSTIMER alarm interrupt only
  increments a counter, as SysTick does on F401 (100 interrupts in 100,011 µs). A 10 ms period is
  1,600,000 cycles.
- **Numeric environment.** `misa` reads 0x40903105: RV32 with I, M, A, C, N, U and X, and without
  F or D. REAL and LREAL therefore both use software floating point. IDENTITY reports `misa`, and
  the verifier requires the F and D bits clear.
- **Trace channel.** The native USB-Serial/JTAG port carries the `B1,` records through a bounded,
  non-blocking writer with the shared 512-byte and 100 ms record budget. A record that misses its
  deadline fails the run. The port stayed enumerated across three MWDT0 system resets with no
  record lost, so one USB connection serves flashing, reset and capture. The ROM and the
  second-stage bootloader print about 30 non-record lines at every boot. The C6 verifier profile
  admits printable non-record lines only before each boot's first IDENTITY record, at most 64 per
  boot, and reports their count. A non-record line anywhere else fails the run. The same port
  carries the control link (specification 34, PORT-05), and a watchdog core reset does not reset
  the USB-Serial/JTAG peripheral, so the firmware clears its CDC-ACM FIFOs and interrupt state as
  it binds the link at boot, and the link answers after any reset the firmware survives.
- **Supervision.** `esp_hal::init` disables the RTC and timer-group watchdogs. The firmware arms
  MWDT0 (TIMG0), stage 0 resetting the system, with F401's 4,000 ms interval. After the deliberately
  withheld feed the chip reports reset reason 0x07 (`CoreMwdt0`; the ROM prints
  `rst:0x7 (TG0_WDT_HPSYS)`). The bench host starts every run with a USB-Serial/JTAG reset, reason
  0x15 (`CoreUsbUart`). A power-on reset reports 0x01. No debugger is attached during M2E, so
  supervision during a debug halt is not evaluated; STATE-06 still applies before qualification.
- **Reset cookie.** F401's two-word cookie codec is stored in persistent RTC fast memory. It
  survives MWDT0 system resets and also USB-Serial/JTAG resets, and the host has no SWD path to
  clear it. The firmware therefore reads and clears it at every boot. With reason 0x07, a watchdog
  cookie completes the run. With reason 0x15 or 0x01, a leftover cookie is discarded and a new run
  starts; that reset is the explicit new boot. Any other reason with a cookie present halts with a
  FAIL record. Beside it, both chips keep the same fixed-size reset record (a magic, a version, a
  CRC-32 and a boot counter, with the cause and a short panic location): the panic path writes it
  before a reset, and the next boot reads and clears it once, so a damaged record reads as no
  record. A watchdog reset runs no code, so when no record exists the next boot takes the cause
  from the chip's reset reason, MWDT0 (0x07), as specification 11 defines it.
- **GPIO.** The physical input is the BOOT button, GPIO9: active low with the internal pull-up. It
  is a strapping pin sampled only at reset, so the firmware reads it only after boot. The output is
  GPIO18, a plain pin with no board load, read back through its own pad input; the probe read back
  1, 0, 1, 0 as commanded. The RGB LED on GPIO8 is a WS2812, not a plain LED, so optical output
  stays unverified, as on F401.
- **Memory.** The fixture keeps F401's 72 KiB allocator arena. The esp-hal stack region spans the
  HP SRAM left after static data and RAM-resident code (about 51 KiB). The C6 budgets a 24 KiB
  window at its top, below esp-hal 1.2.2's `_stack_start` (0x4086E610, a constant that inspection
  pins). The window was 16 KiB, as on F401, but the S1b-5b2 bench measured a 15,740 B high-water in
  it, 644 B untouched, below the 2 KiB minimum headroom, so it is 24 KiB; the STM32F407 keeps
  16 KiB (D.4). Exceeding the window fails the run although the region has room. The window ends
  where static data and RAM-resident code end, so a layout whose region cannot hold it lets the
  boot-time stack paint overwrite the allocator's own state: the linker script asserts
  `_stack_start - _stack_end` is at least the 24 KiB window plus a 4 KiB margin (28 KiB), and
  `build-firmware esp32c6` reads the linked `.stack` region, reports it with its margin above the
  window, and fails below that margin. The linked `.stack` region of the current C6 build is
  39,856 bytes, which leaves 15,280 bytes above the 24 KiB window. Interrupts use the same stack on
  RISC-V, so the window includes them. The allocator serves two heap regions on the C6: the 72 KiB
  arena and the boot loader's reclaimed RAM.
- **Code placement.** Executed from flash through the 32 KB cache, one task activation retired
  188,838 instructions in about 1.77 M cycles (11.07 ms, over the 10 ms period): the core stalled
  on code fetches. Scan-time code therefore executes from HP SRAM. The firmware enables esp-hal's
  RAM-code linker hook (`ESP_HAL_CONFIG_USE_RWTEXT_LD_HOOK`) and selects the core's engine, call,
  dispatch, value, standard-library, process-image, memory, scheduling and numeric modules, the
  fixture loop, the adapter, the allocator and libm (about 303 KB). Load-time code (bytecode
  validation and decoding, preparation, construction) stays in flash. With that placement the same
  activation takes about 264,000 cycles (1.65 ms), and the slowest numeric scan 2.31 ms instead of
  8.50 ms. Inspection fails if the VM dispatch loop is not in HP SRAM. Soft-float and memory
  routines run from the chip's ROM.
- **Flash layout.** The bootloader sits at 0x0 and the partition table at 0x8000, per the ESP-IDF
  image format. Partitions: `factory` application at 0x10000 (1,984 KiB); `stbc` at 0x200000
  (6,112 KiB), the separately installable application bundle, the flash the chip allows up to the
  checkpoint slots at the end of the 8 MB flash; and `ckpt_a` at 0x7F8000 and `ckpt_b` at
  0x7FC000 (16 KiB each), the checkpoint slots (below). `stbc` uses the custom
  partition type 0x40 and the checkpoint slots type 0x41 (subtypes 0 and 1), so the bootloader
  ignores them. The firmware reads the installed bundle, at most the fixture's 16 KiB capacity,
  from the start of `stbc` once at boot into RAM through `esp-storage`; the RAM buffer is that
  capacity, never the partition's size. Apart from the checkpoint slots below, no flash write or
  erase happens during RUN.
- **Checkpoint slots.** `checkpoint_a` and `checkpoint_b` are the chip file's two regions (§10.1),
  16 KiB each, with a 4-byte program unit and a 4 KiB erase unit. They are the only flash the RUN
  loop writes, and this is the one exception to the RUN row of the table above: the persister
  programs them between scans, at most one 256-byte page (`CHECKPOINT_STEP_BYTES`) per step, and
  erases a slot one 4 KiB sector per step. An erase holds the machine interrupts masked for up to
  about 0.4 s, and the bench must measure that time with the scan delay it causes. A record takes at
  most 16,300 payload bytes of a slot. `[runtime.retain] mode = "file"` saves into these slots and
  `path` is not used on a chip. The boot reads the newest committed checkpoint through the same
  flash driver, after the installed application is read.
- **Identity.** IDENTITY carries eight values: the chip ID from the running image header (13); the
  chip revision as major·100 + minor (2); the base MAC as its high 16 and low 32 bits; the flash
  size in KiB from the running image header (8192); the CPU clock in Hz; `misa`; and the reset
  reason. The verifier pins this board's MAC, as the F401 profile pins its UID.
- **Bench topology.** One USB cable connects the Raspberry Pi 5 bench host to the DevKitC-1's
  native USB port (GPIO12/13), addressed by its `/dev/serial/by-id` path, which contains the MAC.
  The F401's ST-LINK stays on its own port, and every command names its port explicitly.

The RISC-V choice exposes a different ISA, soft-float cost, interrupts, and platform stack early while keeping full value-model retirement off the initial dependency gate. A future C3/IMC or S3/Xtensa profile has its own toolchain/ownership and hardware gates; C6 qualification does not support the whole ESP32 family. If the target selection changes to C3, explicitly move atomic/string retirement before its first core build rather than promising both the old M1 timing and deferred storage work.

### D.3 Optional STM32H7 profile

An H7 is a later capacity/performance option, not the starting-board requirement. If selected, add a named board/HAL profile with memory-bank availability, DMA reachability, cache/MPU maintenance, clock/FPU setup, and watchdog/output behavior. ST's Cortex-M7 cache/DMA guidance applies to this profile; do not carry its memory assumptions into F401 or ESP32 code. Larger capacity does not waive the shared fixture or turn F401 failure into success. [E37]

### D.4 STM32F407: the STM32 target from 16 October 2026

Firmware is developed for the chip (ARCH-10). The first board is the owner's STM32F407G-DISC1. This track is paused as of 17 October 2026 (D.5); its file, adapter, firmware and evidence remain.

| Area | Decision | Evidence or remaining work |
|---|---|---|
| Chip | STM32F407 (STM32F407VG on the DISC1): Cortex-M4 with single-precision FPU, up to 168 MHz, 1 MiB flash, 192 KiB SRAM including 64 KiB CCM, Ethernet MAC, USB OTG FS and HS. | Record the device ID, revision and flash size at M0. The CCM is reachable by the CPU only, not by DMA. |
| Board | STM32F407G-DISC1: on-board ST-LINK (USB 0483:3752, serial 0671FF575377524867063440), USB OTG FS on its micro-USB connector, no Ethernet PHY. | 16 Oct M0 record over SWD: DBGMCU_IDCODE 0x100F6413 (device 0x413, STM32F405/407, revision code 0x100F), flash size register 1024 KiB, CPUID 0x410FC241 (Cortex-M4 r0p1), UID 0x0034002D 0x34314715 0x35363630, target supply 2.87 V. |
| Links (ARCH-12) | The control connection over USB (OTG FS to the chip) and over Ethernet when the board has a PHY. | The DISC1 has no PHY; Ethernet is proved on a board or add-on with one. |
| Adapter | `crates/trust-platform-stm32f4`, one Cargo feature per chip: `stm32f407`, the default; the `stm32f401` feature and the F401 composition were retired 17 October 2026 (owner). What differs between the chips is data in `chip.rs`: system clock, flash sectors, RAM, device ID, ports and the pins the firmware keeps. Pins are named at run time; no pin of any board is in the adapter. Ethernet and USB are the next steps. | Authored 17 October 2026, not yet built or run. HAL 0.23.0, PAC 0.16.0, cortex-m 0.7.9 and cortex-m-rt 0.7.7 are the F401's versions; one engine owner as on the F401. |
| Clock | 168 MHz for SYSCLK and HCLK, APB1 42 MHz, APB2 84 MHz, from the PLL. The crystal is measured, not assumed (TARGET-02): after reset the chip runs from the 16 MHz HSI, the firmware starts the crystal, and TIM11 counts HSI cycles over 64 captures, each of eight edges of the crystal clock divided by 31 (RCC_CFGR.RTCPRE; TIM11 option register TI1_RMP = HSE_RTC). The figure is rounded to a whole MHz, between 4 and 26 MHz, when it is at most 1.2 % above or 5 % below one, and configures the PLL; above is tight because a crystal above the figure the PLL is told would run the chip above 168 MHz (for a crystal that is not a whole MHz, with the HSI inside its trim, at most about 0.5 % above). No crystal, no clean capture or an unlikely figure sends the PLL to the HSI. The startup record `B1,CLOCK,<HSE or HSI>,<measured Hz>,<crystal Hz>,<SYSCLK Hz>,<note>` follows each `B1,IDENTITY`, the first boot's and the watchdog boot's, and says which, and why. | The HSI is factory-trimmed to 1 % at 25 degrees (DS8626), so the measurement is no more exact than that. A crystal that is not a whole number of MHz is named in `device.toml` (TARGET-02). The route through TIM11 is unproven until the bench run. |
| Flash | 1 MiB in 12 sectors: 4 x 16 KiB, 1 x 64 KiB and 7 x 128 KiB (RM0090). Vectors in sector 0, firmware in sectors 4 to 9 (704 KiB from 0x08010000), the installed application bundle in sectors 10 and 11 (256 KiB from 0x080C0000), the checkpoint slots in sectors 2 and 3, and sector 1 unused. | The linker script asserts the vector size, the firmware start, and that the firmware's load image ends at or below 0x080C0000. |
| RAM | SRAM1 112 KiB and SRAM2 16 KiB form one block of 128 KiB at 0x20000000; the 64 KiB CCM at 0x10000000 is for the CPU only. The bench firmware keeps the fixture's 72 KiB allocator arena and 16 KiB main stack in the 128 KiB; the CCM is unassigned pending the services runner, whose stack and link buffers leave the rest of the CCM as the allocator's second heap region. | The footprint of the services runner decides what moves into the CCM. |
| Identity | The firmware checks DBGMCU_IDCODE device 0x413 and the flash size register, 1024 KiB, and stops on another chip. The device ID is shared by the F405, F407, F415 and F417; the build selects the chip. | The check is tested on the host against the M0 record above. |
| Pins (TARGET-01) | A line names a pin as `P`, a port letter A to K and a number 0 to 15: `PD12` (small letters are accepted). The chip table says which pins exist (ports A to I; port I has PI0 to PI11) and which the firmware keeps: PA2 and PA3 for the console, PA13 and PA14 for SWD, PH0 and PH1 for the crystal. A refused line says why. An output latches its safe level before it becomes an output, and `emergency_halt` drives every configured output to its safe level. The table check is a `const fn`, so the bench firmware asserts its wiring when it is built. | The DISC1 is configuration of the bench firmware, not of the adapter: LD4 (green) on PD12 (LD3 to LD6 are PD13, PD12, PD14 and PD15) and the user button B1 on PA0, which reads high while pressed. |
| Console | USART2 on PA2 (TX) and PA3 (RX), 230400 baud, 8N1, written at register level because the driver that owns every GPIO port also owns these pins. | ST's UM1472, as an ST community post quotes it, says the ST-LINK/V2-A's virtual COM port lines (ST-LINK_TX and ST-LINK_RX on U2 pins 12 and 13) are not connected to the F407's USART, so the DISC1 may give no console over its USB cable. The manual itself has not been checked for this entry; confirm on the owner's board. Until a link over USB exists the bench console is a 3.3 V USB-UART adapter on PA2 and GND. |
| Firmware | `firmware/trust-stm32f407`, for the chip. It runs the shared `trust-board-fixture` runner until the services runner replaces it. | Build and bench run pending. |
| Evidence | `cargo xtask portability build-firmware stm32f407` builds the chip's firmware from `chips/stm32f407.toml` (Appendix H); `inspect` and `verify` take the bench board's name `stm32f407g-disc1`. Inspection reads the sector map above: the vector table alone in sector 0, nothing loaded in sectors 1 to 3 or in the application's sectors 10 and 11, the firmware and the initial values of `.data` in sectors 4 to 9, the statics and the 72 KiB arena in the 112 KiB of SRAM below the 16 KiB main stack (0x2001C000 to 0x20020000, initial stack pointer 0x20020000), and nothing in the CCM. The verifier pins the chip words of the M0 record (DBGMCU_IDCODE and the three UID words), the 1024 KiB flash size and the 168 MHz clock, and requires exactly one `B1,CLOCK` record right after each of the two `B1,IDENTITY` records. It checks the record's fields: the source is HSE or HSI and goes with the note; SYSCLK equals the profile's 168 MHz; an HSE record has a crystal of 4 to 26 MHz that is the measurement rounded to a whole MHz; an HSI record has no crystal figure. It reads B1 as active high (the pin level equals the pressed flag; the C6's GPIO9 reads low while pressed) and bounds a scan at 10 ms of cycles, 1,680,000. | Authored 17 October 2026 and tested on the host with synthetic records only. The reader of the linked ELF and the form of a real capture are proved by the first build and bench run. |

### D.5 STM32U5A9: the STM32 target from 17 October 2026

Firmware is developed for the chip (ARCH-10). The first board is the owner's Riverdi
RVT50HQSNWC00-B, a 5-inch STM32 Embedded Display (owner, 17 October 2026); its display is not part
of this track. The STM32F407 track of D.4 is paused as of 17 October 2026 (D.5): its file, adapter,
firmware and evidence remain. The first slice of this track, S1b-8u-1, builds the chip, its pins and
its clock with no hardware run; the rows marked S1b-8u-2, S1b-8u-3 and S1b-9u are not built by it.

| Area | Decision | Evidence or remaining work |
|---|---|---|
| Chip | STM32U5A9NJH6Q: Cortex-M33 with TrustZone and a single-precision FPU, up to 160 MHz; 4 MB flash in two banks of 256 pages of 8 KiB, with read-while-write between the banks; 2512 KiB of SRAM: SRAM1 768 KiB, SRAM2 64 KiB, SRAM3 and SRAM5 832 KiB each (one block from 0x2000_0000), SRAM4 16 KiB at 0x2800_0000, and 2 KiB of backup SRAM at 0x4201_6400; one USB OTG_HS with an on-chip high-speed PHY; no Ethernet; TFBGA216. TrustZone is off by default, set by TZEN (bit 31 of FLASH_OPTR). Sources: DS13543 Rev 3 §3.4, §3.5, §3.55, Tables 2 and 171; AN5347 §2.1. | DEV_ID 0x481 (ST's `stm32u5a9xx.h` and the `stm32-data` record) and the factory TZEN value are unverified until the M0 record reads them on the part. |
| Board | Riverdi RVT50HQSNWC00-B (datasheet Rev 1.1): SWD on P3; USB on P6 (D−, D+ and ID, which floats for device mode); power on P10 (6.0 to 48.0 V); the user button S1 on PH3-BOOT0 and the user LED on PE5 (pp. 14 and 15); a CR1220 cell on BT1 for the VBAT domain. The 512 Mbit QSPI flash and the micro-SD slot are not used by this track. The board's CubeMX project sets HSE to 16 MHz in bypass mode on PH0 (`riverdi-50-stm32u5-lvgl`: `RCC.HSE_VALUE`, `PH0-OSC_IN`). | Whether the board's 16 MHz source is an oscillator or a crystal driven in bypass mode is unverified; the firmware checks that HSE is present and reports it (S1b-9u). |
| Links (ARCH-12) | USB on OTG_HS as a CDC-ACM device at full speed (DCFG.DSPD 01) on the chip's on-chip high-speed PHY, with VBUS sensing off through the session-valid override (GCCFG VBVALEXTOEN and VBVALOVAL; O2); no Ethernet. | S1b-8u-2b. The PHY sequence follows ST's sources (`stm32u5xx_ll_usb.c`, `stm32u5xx_hal.c`, and the CLKSEL codes in `stm32u5xx_hal.h`). The codes and register values are host-tested; the speed and the enumeration are unverified until the S1b-9u bench. |
| Adapter | `crates/trust-platform-stm32u5`: the chip table in `chip.rs`, the pin names of the shared STM32 crate `crates/trust-platform-stm32` (which the F407 adapter also uses), and the clock in `clock.rs`. Register-level against the `stm32u5` PAC 0.16.0 with feature `stm32u5a9`; no HAL crate. No board value is compiled in. | Host tests compare `chip.rs` with `chips/stm32u5a9.toml`; the register code is checked for `thumbv8m.main-none-eabihf`; no hardware run. |
| Clock | HSE of 16 MHz in bypass mode through PLL1 (M 4, N 80, R 2, Q 2) to SYSCLK and HCLK of 160 MHz; APB1, APB2 and APB3 undivided; voltage range 1 with the EPOD booster, the SMPS regulator and VDDIO2 valid; 4 flash wait states; the 48 MHz USB kernel clock from HSI48 (ICLKSEL) and the OTG_HS PHY reference from HSE (OTGHSSEL); clock security on while HSE runs. If HSE is not ready within a bounded wait, PLL1 runs from HSI16 and the clock record says so. The sequence follows the board's project (`SystemClock_Config` and `SystemPower_Config`) and OpenOCD's `stm32u5x.cfg`, which reaches the same 160 MHz from MSI. The runner's monotonic clock and wait are TIM2 at 1 MHz (PSC 159 on PCLK1 of 160 MHz), extended to 64 bits, with a CC1 compare armed at the wait's deadline and a WFI sleep (PORT-03; S1b-8u-2b). | The arithmetic is tested on the host (M 4, N 80 and R 2 give 160 MHz; 4 wait states; the 48 MHz choice). The register sequence is unverified until the bench run (S1b-9u). The TIM2 wait is unverified until that bench run; its compare arithmetic is host-tested. |
| Identity | The firmware checks DBGMCU DEV_ID 0x481 and the flash size register, 4096 KiB. The 96-bit UID at 0x0BFA_0700 is the USB serial number (S1b-8u-2). | Read at M0 (S1b-9u). DEV_ID is shared across the U5 family, so the flash size check is made as well. |
| Pins (TARGET-01) | A line names a pin as `P`, a port letter A to J and a pin number, as for the F407. Ports A to I have 16 pins; port J has PJ0 to PJ11; port K does not exist. The firmware keeps PA11 and PA12 (USB), PA13 and PA14 (SWD) and PH0 (the HSE in bypass mode). PC14 and PC15 are not kept in this slice; the slice that gives truST a wall clock adds them with reason `oscillator`. A refused line says why. | Host test: every port A to J round-trips, PJ12 and PK0 are refused, and each kept pin is refused with its reason. |
| Flash (S1b-8u-3) | The chip file's layout: the vectors in the first 8 KiB page of bank 1, the firmware of 2040 KiB from 0x0800_2000, the installed application of 2016 KiB as the rest of bank 2, and two checkpoint slots of 16 KiB (two 8 KiB pages each) at the end of bank 2. | Page erase and quad-word programming are S1b-8u-3. |
| RAM (S1b-8u-3) | Heap region A of 1 MiB: SRAM1, SRAM2 and the first 192 KiB of SRAM3. Statics in the rest of SRAM3. The main stack of 32 KiB at the top of SRAM5 (0x2026_8000 to 0x2027_0000): the C6 measured a 15,740 B high-water in a 16 KiB window (D.2), and this chip has the room. Heap region B of 800 KiB below the stack, in SRAM5. | The map is written in S1b-8u-3, with the shared measurement crate; the bench (S1b-9u) measures the high-water against the 2 KiB headroom rule. |
| Console | None in the product firmware. The F407's USART2 console was a fixture of the paused track. | Decided with S1b-9u. |
| Firmware (S1b-9u) | `firmware/trust-stm32u5a9`, built from `chips/stm32u5a9.toml`. Not part of S1b-8u-1. | Pending. |
| Evidence (S1b-9u) | `build-firmware stm32u5a9` and `inspect riverdi-rvt50hqsnwc00-b`, the M0 record and the bench steps of the slice. `inspect` refuses a linked image whose vector table is not alone at 0x0800_0000 in the 8 KiB vectors page; whose firmware or `.data` load image leaves 0x0800_2000 to 0x0820_0000; that puts a section or a load in the application (0x0820_0000 to 0x083F_8000) or a checkpoint slot (0x083F_8000 to 0x0840_0000); whose statics leave 0x2010_0000 to 0x201A_0000; or whose initial stack pointer is not 0x2027_0000 with the 32 KiB window below it. It names the section and the address. The CI portable lane checks the adapter for `thumbv8m.main-none-eabihf`. | The inspect checks and the lane are tested on the host with synthetic layouts only; the reader of a real linked ELF is proved by the first build and the bench run. Pending: the bench has not run. |

## Appendix E — Implementation deliverables and acceptance ownership

These are future work items, not completed code or newly passing tests. Existing native tests supply behavior references; add missing assertions in the owning crate/physical harness while implementing the authorized scope. Execution follows Section 13's consolidated validation cadence.

| Deliverable | Current source boundary | Required resulting behavior and evidence |
|---|---|---|
| Complete bytecode loading and construction | Host `bytecode/format/module.rs`, `decode*`, `validate*`, `metadata.rs`, byte serialization and core reader/records; host `runtime/bytecode.rs` and `encoder/` | Move the full byte-container/decoder/validator/materialization boundary into core, with host compatibility surfaces and portable optional serialization. Keep compiler/runtime lowering and file/presentation adapters hosted. Implement 2.0 and the version matrix across producer, loader, validator, disassembler and format tests; then LOAD-10/11 preparation initializes a fresh engine without HIR. |
| Portable initialization | `runtime/vm/local_init.rs`, `local_init/expr.rs`, initializer helpers and metadata | Shared initialization plans preserve call-time evaluation, defaults, static ownership, compound copies, instance construction, and restart behavior. Initializer execution is included in work/storage budgets. |
| Shared executor/state | `runtime/core.rs`, `runtime/vm/`, `memory.rs`, core `value`/`vm` modules | Hosts and firmware call one execution implementation over prepared state. Host compatibility APIs remain available; selected optimized execution plans remain subject to parity/performance gates. |
| Bounded storage and ownership | `memory.rs`, operand/frame/local storage, compound values, caches, dependency manifests | Document and implement MEM-10/11; both MCU target graphs build; normal/first-use/fault/copy/drop paths stay within admitted storage and host threading contracts remain intact. |
| Portable standard primitives/math | `stdlib/fbs/`, `stdlib/numeric.rs`, relevant core helpers | Share timers/counters and admitted numeric functions; pin NUM-05 implementation; retain fixed logical-time traces and numeric/control oracles. |
| Platform compositions | New F401/ESP32 adapters and board firmware; existing hosted composition | Implement Appendix D mechanisms without processor-specific opcode behavior. Capture real reset, I/O, wake, watchdog, and stopped-installation evidence for each released board. |
| Native hosted/runtime-only qualification | Existing `.github/workflows/ci.yml` OS matrix and runtime behavior/restart/debug tests | Add isolated compiler-free consumer jobs and saved-artifact cases on each claimed OS/architecture. Preserve supported full-product workflows, then separately qualify bounded operation. |
| Modern compiler/dependency baseline | Root/member manifests and lockfiles, toolchain configuration, CI/release/docs workflows, `deny.toml`, all three npm projects | Separate M0U scope after the initial extraction/board checkpoint by default. Preserve extracted-before-modernization snapshots, implement MOD-01 dispositions and align MSRV/tool/support documentation. MOD-06 is its own batch, not A's. |
| Rust ownership/API design | Prepared loader, execution/storage, time and generation identities, state transitions, board/host composition | Apply Section 17 at those boundaries; preserve runtime checks and existing contracts. Native behavioral assertions and architecture evidence demonstrate the result, not pattern-count or line-count targets. |

The following observed allocation/lookup sites define concrete extraction work, not new independent architecture layers:

| Current site | Named deliverable / acceptance focus |
|---|---|
| `runtime/vm/local_init.rs`, `local_init/expr.rs` | Compile dynamic initializers to STBC; prepare instance/static storage and plans; preserve call-time/first-use semantics with no late HIR walker/cache construction. |
| Core `value/types.rs`, `retain.rs`, `vm/const_pool.rs`; relocated bytecode containers | Explicit no_std map hashers/constructors, std API compatibility, count/work bounds and insertion-order tests. Include portable CRC/alloc dependencies with complete decoder/validator relocation. |
| `runtime/vm/call.rs`, `memory.rs`, `vm/call/stdlib.rs` | **Dense POU/type/member/import identities**: reuse artifact IDs/vtable slots and prepare import signatures, arity, work/allocation/fault metadata. Remove repeated uppercase/name-key lookup from bounded dispatch. |
| Core `value/types.rs`, `vm/const_pool.rs`; host `vm/stack.rs`, `frames.rs` | Replace allocation-producing array/struct/string copies and per-call local Vec growth with admitted storage while preserving independent value semantics; account for existing Arc RMW and last-owner destruction. |
| Core `error.rs`, host `vm/errors.rs` | Fixed fault identity/context; move `SmolStr`/`format!` rendering off the bounded path without changing stable codes. |
| Core retain model; host retain snapshot/store and cycle integration | Replace name-keyed dynamic checkpoint capture on bounded paths; implement the Section 10 mailbox/durability policy and failure ordering. |
| `runtime/vm/register_ir*`, `dispatch.rs`, `scheduler/clock.rs` | Prepare optional optimized plans, move host profiling out, remove thread-local pool/Instant assumptions from shared execution, preserve deadline accounting, and implement wakeable hosted waits separately. |

These sites are grounded in R28–R33. Every touched path retains its native behavioral assertions; a table entry alone is not evidence of bounded execution.

For the early implementation scope, the minimum new acceptance set is: fresh-engine loading; nonzero and changing-input initialization; timer/array/interface state parity; the small numeric fixture; separate F401/ESP32 linking and dependency evidence; and per-region F401 preparation/RUN/stack/flash evidence followed by M2E on actual ESP32 hardware. The later release corpus retains every applicable Section 14 requirement. No test count, source move, or build-only result substitutes for those assertions.

## Appendix F — Dated dependency candidates

The [candidate table](https://github.com/johannesPettersson80/trust-platform/blob/main/docs/notes/runtime-portability/research-notes.md#dated-dependency-candidates)
and [registry snapshot](https://github.com/johannesPettersson80/trust-platform/blob/main/docs/notes/runtime-portability/trust-runtime-modernization-inventory-2026-10-09.json)
are dated research under D05, outside the normative specification directory.
Section 16 governs the separate modernization scope: refresh metadata, record
upgrade/replace/retain decisions, then freeze and qualify the selected graph.

## Appendix G — Design review record

| Review item | Disposition and pending evidence |
|---|---|
| H full review and temporal contract decision, 12 October 2026 | All source-review findings are accepted. The temporal width contradiction is resolved through uniform eight-byte storage and STBC 3.0, preserving explicit 1.1/2.0 ABI interpretation. Engineering writes validate reconstructed aliases before commit; retained physical layouts require compatible encoding and geometry. Native regressions and independent corrective reviews are authored; the checklist retains failed Full15 evidence and marks current corrections unverified. No platform functions are removed. |
| H mutation-lifecycle architecture review, 11 October 2026 | Independent source review identified unrestricted live storage/type-registry access as conflicting with admitted-state ownership and HOST-03. The owner delegated selection of the best behavior. HOST-03/09 now select construction-only raw access, checked live writes, scratch debug types and isolated schema candidates. Preserve live write timing and warm restart; document the Rust API migration. This revision records the contract only, not H implementation or validation. |
| A3 external review, 9 October 2026 | All findings and quality recommendations accepted. Spec 12 §11.5.8/11 define staging-write admission, shared contexts, compact templates, stable text and source/wire coverage. Checklist A3-R5/R6 owns implementation evidence; A4 runtime lifetime checks and actual execution remain mandatory. |
| Periodic-task phase defect, 9 October 2026 | Accepted as SCHED-05 with coordinated spec 10/11 amendments. The initial A4 placement is superseded by the sequencing review below. Reuse the draft readiness/regression changes, add missing non-multiple cases and discard the unsupported 75-runs-in-10-seconds hosted-runner claim. The reviewed pre-A1 code used sampled-time baselines; current implementation and validation status are recorded in the checklist. |
| Scheduling-fix sequencing review, 9 October 2026 | Accepted the review's four-batch alternative: implement and separately review the dependency-free fix first in A1, with its native regressions sharing A1's existing batch. A1 establishes the corrected scheduling contract for subsequent scopes. A4 keeps only fixture/oracle integration for this fix; metadata remains non-blocking maintenance. No new implementation authorization or fifth batch is inferred from the review. |
| A3 source-fixture admission, 9 October 2026 | Source checking correctly rejects mutable scalar initializers. Use `REF(history[delta])` against persistent FB storage for the changing-input case; keep those source diagnostics in both producers. The fixture also avoids the reserved STEP identifier. Source authoring and eventual A4 execution remain separate evidence. |

The [review archive](https://github.com/johannesPettersson80/trust-platform/blob/main/docs/notes/runtime-portability/research-notes.md) retains all
17 v0.4 dispositions and the eight v0.6 follow-up dispositions, including corrected
claims and unexecuted evidence. Current choices are expressed in the requirements
and milestones above; historical approval of a plan is not runtime or hardware
qualification. A1 fixes scheduling and freezes the corrected native and numeric
fixtures; A3 settles exact 2.0 layouts before
format implementation. A4 proves integrated source-free execution, and Scope B/M2E
supplies physical resource evidence.

## Appendix H — Chip description files

TARGET-03 describes each supported chip once, in `chips/<chip>.toml`. This appendix defines that
file. The supported chips are the STM32F407 (`chips/stm32f407.toml`, paused as of 17 October
2026, D.4) and the ESP32-C6 (`chips/esp32c6.toml`). The STM32U5A9 (`chips/stm32u5a9.toml`, D.5)
has its file, which its adapter's tests read and check; it is in the catalog, and `build-firmware`
builds its firmware crate (S1b-9u). The STM32F401 of the NUCLEO-F401RE is no longer a
supported chip (D.1)
and has no file; its firmware was retired 17 October 2026 (owner). A development board has no
file (ARCH-14): its pins are named in `io.toml` and its other wiring is detected or named in
`device.toml` (TARGET-01, TARGET-02).

### H.1 Reading a file

A chip file is TOML. Every key below is required unless the table names it for one family only;
`sources.measured_on` follows rule 10 of H.5. A key the format does not define, a missing key, a
value of the wrong type and a word outside a closed list are refused, and so is a file that breaks
a rule of H.5. Numbers are integers; addresses and IDs may be written in hexadecimal. Sizes are in
KiB.

### H.2 Keys

| Table | Key | Meaning |
|---|---|---|
| `[identity]` | `name` | The chip's name: the file's name without `.toml`, in small letters and digits. `build-firmware` takes it. |
| | `model` | The manufacturer's name for the chip, for example `STM32F407`. |
| | `family` | The platform adapter that drives the chip, which also decides how its pins are named: `stm32f4` (`crates/trust-platform-stm32f4`), `stm32u5` (`crates/trust-platform-stm32u5`) or `esp32` (`crates/trust-platform-esp32`). |
| | `device_id` | The number the chip reports for itself, by which the firmware and a probe recognize it. For the STM32 families (`stm32f4`, `stm32u5`): DEV_ID, bits 11:0 of DBGMCU_IDCODE, which the firmware checks at start and a debug probe reads; the firmware also checks that the flash size register equals `memory.flash.kib` (D.4, D.5). `esp32`: the chip ID that the ROM and espflash report and the firmware image header carries (D.2). |
| | `usb_bootloader` | `{ vid, pid }`: the USB IDs the chip shows while its own boot loader runs, through which truST is installed over USB (TARGET-04). The USB search recognizes a chip by them. |
| `[processor]` | `core` | The processor core. |
| | `isa` | Its instruction set. |
| | `rust_target` | The Rust target the firmware is built for. |
| | `fpu` | `none`, `single` or `double` precision. |
| `[clock]` | `max_hz` | The highest processor clock of the data sheet. |
| | `firmware_hz` | The clock the truST firmware runs the processor at. |
| | `crystal_min_hz`, `crystal_max_hz` | The crystals the chip's oscillator accepts. |
| `[memory.flash]` | `start`, `kib`, `sectors` | `stm32f4` and `stm32u5` only: flash in the chip's address space, from address `start`, of `kib`, with its erase sectors in address order as runs `{ count, kib }`. |
| `[memory.spi_flash]` | `erase_kib` | `esp32` only: flash on the chip's SPI flash interface, in the package or the module. Its size is the device's own and is read when truST is installed; `erase_kib` is its erase unit. |
| `[[memory.ram]]` | `name`, `start`, `kib`, `dma` | One entry per RAM region: its name in the reference manual, its first address, its size, and whether DMA reaches it (the F407's CCM is for the CPU only). |
| `[events]` | `default_capacity`, `max_capacity` | The newest events the chip's event store keeps when the project sets none, and the most a project may set (truST's own choice, H.4). The firmware sizes its event store with them, and admission refuses a project above the maximum (specification 11, the event store; §5.1.1 step 7). |
| `[layout]` | `vectors` | `stm32f4` and `stm32u5` only: the vector table. |
| | `bootloader`, `partition_table` | `esp32` only: the second-stage boot loader and the partition table. |
| | `firmware` | The truST firmware. |
| | `application` | The separately installed application (DEPLOY-01). |
| | `application_a`, `application_b` | The two program slots of the program store (§5.1): optional, and a file names both or neither. Each is `{ start, kib }` of the same size, inside `application`, on erase-unit boundaries and apart from the other slot and the checkpoint slots. |
| | `checkpoint_a`, `checkpoint_b` | The two checkpoint slots of the retain store, so that an interrupted write keeps the last good checkpoint (RETAIN-02); §10.1 defines their contents. |
| `[interfaces]` | `usb`, `ethernet`, `wifi`, `bluetooth`, `uart`, `can`, `spi`, `i2c`, `adc`, `dac`, `timers`, `pwm` | The chip's peripherals of each kind, by the manufacturer's names. An empty list means the chip has none. An Ethernet PHY and other wiring belong to the board. |
| `[gpio]` | `ports` | `stm32f4` and `stm32u5` only: `{ port, pins }` for each GPIO port, its pins counted from 0; a pin is named like `PD12`. |
| | `pins` | `esp32` only: the number of GPIOs, `GPIO0` upwards; a pin is named like `GPIO18`. |
| | `reserved` | `{ pin, reason }` for each pin the firmware keeps for itself, which `io.toml` cannot use; `reason` is `console`, `debug`, `oscillator` or `usb`. |
| `[firmware]` | `crate` | The firmware crate's directory, under `firmware/`. |
| | `bin` | Its binary. |
| | `components` | The runtime components the firmware holds (H.3). |
| `[sources]` | `documents`, `measured`, `measured_on`, `unchecked` | Where the values come from (H.4). |

Layout regions are `{ start, kib }`. Their addresses are bus addresses in `memory.flash` and flash
offsets in `memory.spi_flash`. The pins are those of the chip's largest package; a board's package
may have fewer, and its `io.toml` names only pins it has.

### H.3 Components

The component names are a closed list. Each is one runtime service of ARCH-11, the same code on
every target (ARCH-14):

| Component | What it is |
|---|---|
| `engine` | The engine, `trust-runtime-core`: bytecode execution, tasks and cycles, the process image. |
| `control` | The control connection and its protocol: status, start, stop and restart, resource states and faults, events and log. |
| `live-values` | Live Values: read, write, force and release. |
| `debug` | Debugging (specification 13). |
| `deploy` | Program loading, including online change. |
| `access` | Access rights: tokens and roles, TLS and signed deployment. |
| `discovery` | Discovery and the mesh (ARCH-12). |
| `communication` | The communication services: the `io.toml` drivers, and Bluetooth devices where the chip has Bluetooth. |
| `web-hmi` | The web pages and the HMI. |
| `modbus-rtu` | The `modbus-rtu` driver on the chip's UARTs. |
| `ethercat` | The `ethercat` driver on a chip with an Ethernet controller. |
| `opcua` | The OPC UA server and client (SERVICE-03). |
| `ads` | The Beckhoff ADS client and server. |
| `cloud` | The runtime cloud, the fleet and pairing. |
| `observability` | The historian with its alerts, and the Prometheus metrics. |
| `openot` | OpenOT telemetry (SERVICE-04). |
| `simulation` | Simulation (SERVICE-05). |

A firmware holds `engine` and lists each further component once it holds it, as measured. The
ESP32-C6 firmware holds `engine`, `control` and `live-values`, as its bench measured them on
18 October 2026 (the control requests of S1b-5 and Live Values at the pins); the STM32F407
firmware holds `engine` alone. Several resources in one configuration,
and several runtimes on one chip (ARCH-15), are not components: every firmware runs them, and how
many runtimes fit a chip is measured and reported (ARCH-13). Linux, macOS and Windows run every
component and have no chip file. The tools offer what the device's chip file lists and refuse a
project that needs a component it lacks, naming the chips whose files list it (ARCH-14).

### H.4 Measured and data-sheet values

A value is the manufacturer's, from the documents in `sources.documents`, or truST's own choice
(the layout, the firmware clock, the firmware and its components, and the `[events]` default and
maximum), unless `[sources]` says otherwise. `measured` lists the values that were read on a chip,
and `measured_on` says on which device and when. `unchecked` lists manufacturer values that the
repository's records do not yet confirm; a reviewer checks them against the documents before a tool
relies on them. An entry is a key path such as `identity.device_id`. A path through an array of
tables names that key in every entry (`memory.ram.dma`); a path to a table or an array names all of
it.

### H.5 Rules

The reader refuses a file that breaks one of these rules:

1. `identity.name` is the file's name, of small letters and digits. `model`, `core`, `isa`,
   `bin` and every name in a list are not empty.
2. For `stm32f4` and `stm32u5`, `device_id` is 1 to 0xFFF, the twelve bits of DEV_ID.
3. `firmware_hz` is more than 0 and at most `max_hz`; `crystal_min_hz` is more than 0 and at most
   `crystal_max_hz`.
4. The Rust target is of the family's architecture, `thumb` for `stm32f4` and `stm32u5` and `riscv32` for
   `esp32`, and it agrees with the FPU: an `eabihf` target has an FPU and an `eabi` target none; a
   RISC-V target's letters are the instruction set (`riscv32imac` is `RV32IMAC`), and its F or D
   letter is the FPU.
5. `stm32f4` and `stm32u5` describe their flash in `memory.flash` and `esp32` in `memory.spi_flash`, and neither
   in the other. The sectors add up to `kib`. The flash, every sector run, RAM region and layout
   region, and the erase unit hold at least 1 KiB; there is at least one RAM region; an `esp32`
   chip has at least one GPIO.
6. The RAM regions have different names, lie below 4 GiB and overlap neither one another nor
   `memory.flash`.
7. The layout has the family's regions and no others: `vectors` for `stm32f4` and `stm32u5`;
   `bootloader` and `partition_table` for `esp32`. Each region lies inside `memory.flash` and
   starts and ends on a sector boundary, or, in SPI flash, on a multiple of `erase_kib`. No two
   regions overlap. `vectors` starts at the first byte of the flash, where the processor starts,
   and `bootloader` at offset 0, where the ROM loads it.
8. Each name is listed once in each list of `interfaces`, in the ports, the reserved pins, the
   components and the documents. `stm32f4` and `stm32u5` have `ports`, with letters A to K and
   1 to 16 pins each, and no `pins`; `esp32` has `pins` and no `ports`. A reserved pin is named in
   its family's form and is a pin of the chip.
9. `firmware.crate` is a directory under `firmware/`, and the components include `engine`.
10. `sources.documents` is not empty. Every path in `measured` and `unchecked` names a key outside
    `[sources]`, and no path is listed twice or inside another listed path. `measured_on` is given,
    and not empty, exactly when `measured` is not empty.
11. The event store's capacities form a range: 1 <= `default_capacity` <= `max_capacity` <=
    1,000,000.

### H.6 Where the files are read

The crate `crates/trust-chips` reads and checks chip files and embeds, as the catalog of supported
chips, the files of the chips whose firmware crate is in the repository, in which no two files have
one name. It depends on no firmware or
platform adapter, so `trust-runtime` (its command line, and the USB search and installation of
TARGET-04) and `xtask` can use it; the adapters use it only in their host tests.
`cargo xtask portability build-firmware <chip>` builds the firmware of a chip in the catalog from its
file: its crate, binary and Rust target. `inspect` and `verify` take a bench board: `stm32f407g-disc1`, `esp32c6-devkitc-1` or
`riverdi-rvt50hqsnwc00-b`, whose first bench run is still to come (S1b-9u). A bench profile holds the bench's own values, such as the
button's polarity, the fixture's memory budget and the module's flash, and takes its chip values
from the chip's file. Host tests compare the adapters' chip data with the files: the STM32U5A9 table in its
`chip.rs`, the STM32F407 table in `chip.rs`, its crystal range, and the C6 profile. The xtask tests compare the firmware's
linker scripts and partition table, the inspector's regions and the verifier's pinned chip IDs
with them. A chip value is thus written once, in its file, and checked wherever code repeats it.
