# truST Runtime Portability Specification

**Version:** 0.10 — scheduling correction establishes the A1 baseline  
**Date:** 9 October 2026  
**Repository:** `johannesPettersson80/trust-platform`  
**Reviewed baseline:** `main` at `be8d81a4a7ab16ca7554b8be0f4723161ec1a47b`  
**Baseline commit date:** 3 September 2026  
**Source recheck:** `main` at `9a15065725c17da2c912055f1509368d3fd01d6c`, 9 October 2026; the reviewed runtime/core sources, workspace manifests/lockfile, specifications, and native CI workflow are unchanged from the original baseline.  
**Scope:** A shared Rust execution engine for Linux, Windows, macOS, the NUCLEO-F401RE reference board, and one required ESP32 target, with separately scoped dependency/toolchain modernization and target qualification.

**Revision basis:** This version moves the dependency-free periodic-task phase correction to the start of A1 as a separately reviewed source change sharing A1's existing batch. A1 establishes the corrected scheduling baseline; A4 integrates it into the compiler-free fixture and saved oracle. Specifications 10/11 and the scheduling arithmetic remain unchanged; current code still uses sampled-time baselines until implementation. The four-scope cadence, separate modernization, loader, STBC and numeric decisions remain. No implementation, dependency upgrade, build, or hardware qualification is asserted by this plan. [D01–D07]

**Requirement status:** This is the canonical implementation plan. “Shall” defines required behavior for the stated future profile, not behavior already implemented. Existing identifiers are retained. Specifications 11/12 distinguish current hosted/STBC 1.1 behavior from planned migration. Scope authorization and validation follow AGENTS.md; a milestone description is not authorization or evidence that it passed.

## 1. Decision and scope

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
| ARCH-02 | MCU firmware shall not depend on `trust-runtime`, compiler/HIR/IDE crates, or hosted service implementations. |
| ARCH-03 | `trust-runtime-core` shall retain its `forbid(unsafe_code)` boundary. Hardware-specific unsafe operations, where required, shall remain outside it and follow the repository's review policy. |
| ARCH-04 | Platform adapters shall provide mechanisms; the common engine shall own IEC behavior, scheduling decisions, memory rules, and fault decisions. |
| ARCH-05 | Supported bounded controller profiles, hosted and MCU, shall execute without general-purpose allocation, reallocation, or deallocation in their control-cycle paths, including fault paths. |
| ARCH-06 | Unsupported instructions, imports, types, capabilities, or resource demands shall cause an explicit admission failure, never silent fallback or changed semantics. |
| ARCH-07 | The controller shall operate independently of the development computer after a valid application has been installed and started. |
| ARCH-08 | Hosted products shall retain their existing supported features and interfaces while adopting the shared core; MCU limits shall not become universal desktop limits. |
| ARCH-09 | Hosted adoption shall be verified through native execution tests on Linux, Windows, and macOS, compiler-free loading tests, and baseline-relative resource and performance evidence. |
| ARCH-10 | The planned platform-support outcome requires one named STM32 board and one named ESP32 board to qualify with the same engine. An STM32 release may be delivered first, but shall not close the ESP32 deliverable. A different MCU family shall not silently substitute for ESP32. |

The research supports this separation of responsibilities, not a universal claim that interpretation is smaller or faster than native compilation. The first implementation shall retain STBC and the existing Rust executor. Native compilation, replacing STBC with WebAssembly, or a generalized multi-backend framework shall require a separate, measurement-led decision; none is a prerequisite for this migration. Existing validated internal execution optimizations may remain within the shared semantic implementation. [D02, E05–E09]

### 1.2 Scope boundaries

This proposal starts a runtime-wide execution improvement workstream and a new embedded implementation phase. The existing closed core/host extraction checklist explicitly excluded STM32 bring-up, ESP32 support, and an embedded product promise. Its closure therefore does not mean that MCU support is already implemented. Those old exclusions remain accurate descriptions of that earlier work. [R13]

The project's first MCU reference board is **NUCLEO-F401RE**. Its STM32F401RE provides a Cortex-M4 with single-precision FPU, a maximum CPU frequency of 84 MHz, 512 KiB flash, and 96 KiB SRAM. These are device capacities, not bytes available exclusively to truST. The board makes compactness an early constraint; complete-runtime fit and timing remain unmeasured. Record the physical board/MCU revision, memory reservations, pins, clock source, toolchain, and HAL before board implementation. [E24–E27]

One **ESP32 target is required**. The revised provisional reference is **ESP32-C6-DevKitC-1**, using `riscv32imac-unknown-none-elf`, `esp-hal`, and initially disabled radios. No ESP32 board possession or purchase is asserted. C6 supplies the A extension needed by the existing atomic-reference-counted values during bring-up; it does not make those values bounded. M0 confirms board availability and freezes the build. M1 checks its isolated core graph, M2E executes the common fixture on actual hardware, and M5 qualifies the profile. C3/IMC becomes a later optional no-CAS profile after storage/atomic retirement, rather than contradicting the early-storage sequencing. RISC-V/Xtensa and bare-metal/ESP-IDF choices remain distinct. [E56, E57]

An STM32H7 board remains an optional later target if measured resource needs justify it. It is not a prerequisite to start on the available F401. Failure to fit the F401 shall retain that failure and lead to an explicit storage/profile or target decision; a larger board shall not be reported as a passed F401 gate. Native ST compilation, an assembly rewrite, FPGA execution, embedded EtherCAT, new MCU support for online application changes while controlling live outputs, and complete MCU parity with every hosted protocol are outside this implementation. Existing supported hosted online-change workflows remain subject to the compatibility requirements.

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
          target admission / installation
                    |
      +-------------+--------------------+
      |                                  |
Hosted product runtime              MCU firmware
trust-runtime                       board entry point
Linux / Windows / macOS                   |
      |                                  |
OS-specific adapters                STM32 / ESP32 adapter
      |                                  |
      +-----------+----------------------+
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

  trust-platform-stm32f4/          NEW; F401 platform adapter implementation
    src/
      clock.rs
      io.rs
      watchdog.rs
      storage.rs

  trust-platform-esp32/            NEW; selected ESP32 adapter, initially C6

firmware/
  trust-nucleo-f401re/             NEW; confirmed board composition
    src/main.rs
    src/board.rs
    memory.x                      or the selected BSP's equivalent

  trust-esp32c6-reference/         NEW; proposed C6-DevKitC-1 composition
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
| Transport workers, web/HMI servers, databases, discovery | Optional services outside the core |
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

Relocation also adds core's direct `crc32fast` edge: disable its workspace default std feature and forward std only from hosted consumers/core's std feature. Add the direct libm edge selected by NUM-05 at its existing lockfile version. Account for alloc imports/formatting in bounded preparation. These portability changes are part of A/M1; Rust/MSRV/edition/Node and broad package upgrades are exclusively M0U. Isolated F401/C6 checks, not a host no-default check, establish the graph. [R03, R25, R36, R37, E45]

**BUILD-04.** Bare-metal checks shall be followed by a fully linked firmware build. The firmware shall provide startup, memory layout, interrupt vectors, panic behavior, and allocator support when applicable. A library `cargo check` is not a complete firmware test.

**BUILD-05.** Assembly shall remain limited to established architecture support or a reviewed, measured optimization. There shall be no processor-specific fork of the bytecode semantics.

**BUILD-06.** Hosted applications may continue using `std`, operating-system threads, filesystems, and networking outside the portable execution boundary. The MCU portability requirement shall not force the entire desktop product to become `no_std`. [E01]

**BUILD-07.** The hosted runtime-only build shall load and execute admitted artifacts without compiler/HIR/IDE dependencies or unselected product services. The full product may retain compilation as an optional authoring service. Core-only and runtime-only feature graphs shall be checked in isolated builds; a full development build shall not conceal accidental dependencies.

**BUILD-08.** Linux, Windows, and macOS shall have native test jobs for the declared supported architectures. Cross-compilation is supplementary evidence, not a substitute for executing the relevant tests on each OS.

**BUILD-09.** M1 shall check separate `thumbv7em-none-eabihf` and `riscv32imac-unknown-none-elf` core graphs for compiler/HIR/host-service leakage, dependency defaults, and synchronization assumptions. Add no new atomic read-modify-write dependency to shared execution. Existing `Arc`/`SmolStr` storage may remain temporarily on these atomic-capable bring-up targets, with allocation/refcount/drop costs recorded; MEM-06/10/11 retires it from bounded value/compound paths before M3 qualification. Optional C3/IMC checks become mandatory only if that additional profile is selected, after this ownership work. Neither Rust 1.99 nor `smol_str` 0.3.6 removes the IMC issue. Do not fabricate target cfgs or introduce an interim replacement-Arc layer solely to claim C3 support. [R09, R24, E30, E31, E46, E56]

For the F401 Cortex-M4F bring-up, `thumbv7em-none-eabihf` is the selected target family. CPU/FPU flags and BSP startup shall match the part's single-precision FPU. The ABI does not provide hardware double-precision arithmetic on this part. Appendix D records the initial HAL/runner direction and decisions to freeze. [E04, E25, E28]

### 4.3 Hosted product requirements

**HOST-01.** Linux, Windows, and macOS shall use the shared loader, execution state, opcode semantics, cycle policy, and fault identities. Host `Runtime` compatibility APIs shall compose or wrap that engine, rather than retain an independent legacy dispatcher as the permanent desktop implementation.

**HOST-02.** Hosted limits shall be selected for the application and deployment, not copied from STM32 defaults. Hosts may allocate backing storage during preparation and offer larger programs, strings, arrays, instance counts, and task configurations while retaining checked execution and admission.

**HOST-03.** Existing supported debugger, HMI, protocol, persistence, configuration, CLI, and embedding APIs shall remain available on their supported platforms. Hosted online change shall follow the generation, restart, and retirement contract in Section 11.3. The M0 feature matrix shall distinguish implemented support, unavailable platform-specific features, and experimental support; it shall not infer identical service/driver support on every OS.

**HOST-04.** Validated execution optimizations shall remain available where supported and shall use the same semantic and numeric conformance corpus. Their plans shall be prepared from admitted artifacts without requiring an ST compiler session. Bounded hosted profiles shall satisfy MODEL-02 for first use, rare branches, and cache replacement; MCU limits shall not force hosted builds onto a slower path without measured evidence and an approved tradeoff.

**Optimized-tier decision.** The existing register-IR/tier-1 execution remains an optional internal execution plan of the shared engine, moved during M2B. Its lowering and caches are prepared before bounded RUN; profiling and OS timing remain host adapters. The initial MCU profile omits the register representation and its storage, while hosted products retain the enabled optimized path and its semantic helpers. Scope A may leave the old hosted tier in place temporarily; that is not completed HOST-01 migration. No permanent independent hosted dispatcher is exempted. Compare against the actual enabled hosted tier, using the relevant ARCH-VM-19/20/27/28/31 benchmark workloads and fallback criteria as historical regression references, not newly passing evidence. [R32]

**HOST-05.** Production control execution shall use prepared state and bounded interfaces on hosted targets as well as MCUs. Network, HMI, logging, compilation, and storage services shall not impose unbounded waits on the control thread. Moving an existing blocking path shall preserve behavior or have a separately approved semantic change and regression tests.

**HOST-06.** The simulator shall execute through the same core APIs, with injected logical time and input traces. It shall support the selected controller's capability and resource profile and explicitly report unsupported application requirements. Accelerated simulation shall not bypass required arithmetic, reference, bounds, or instruction-budget checks.

**HOST-07.** The implementation shall support a small set of documented hosted compositions: runtime-only; full controller with selected services; and development/simulation. They shall not become separate VM products. Dependencies and services omitted from runtime-only shall remain absent from its build graph, rather than merely disabled at startup.

These compositions require M2B feature/ownership changes: current `Runtime` embeds web/HMI/control/debug and other host state, and protocol feature switches alone do not produce a compiler-free runtime-only product. Scope A's headless core consumer is a narrower deliverable, not proof that HOST-07 is already implemented. [R06, R34]

**HOST-08.** Hosted migration shall have regression and performance gates of its own and shall not be declared complete solely because an STM32 demonstration succeeds. A target whose tests have not run shall remain unqualified for the new profile.

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

### 5.2 Compiler-free preparation

**LOAD-01.** Every target, including Linux, Windows, macOS, and the MCU, shall be able to initialize a fresh engine from bytecode and its deployment metadata without parsing ST, constructing a compiler session, or running HIR analysis. Hosted authoring tools may compile before deployment, but the standalone loading/execution path shall not require them.

**LOAD-02.** All executable metadata needed for initialization shall be present in the deployment artifacts or the firmware's versioned built-in library. Missing information shall fail admission. The build pipeline may need an additive metadata extension; it shall not hide the gap by compiling source on the MCU.

**LOAD-03.** The loader shall verify the container version, integrity, section ranges and overlap, checked size arithmetic, instruction boundaries, jump/call destinations, table references, types, imports, and target capabilities before allowing RUN.

**LOAD-04.** Resource counts shall be checked or conservatively recomputed against the actual target representation. A compiler-provided memory estimate is not trusted authority. Validation work and decoder allocations shall themselves be bounded so a small file cannot request unbounded expansion.

**LOAD-05.** Runtime checks that remain necessary for dynamic indexes, null/reference lifetime errors, call depth, execution budgets, and arithmetic faults shall remain active. Admission is not permission to remove runtime memory protections.

**LOAD-06.** Required imports shall resolve to approved, typed implementations with a versioned execution contract covering maximum work, variable-size arguments, blocking, allocation, failure, and allowed profile/state. A callback without a supportable bound shall be staged outside bounded execution or rejected for that profile; a deadline parameter is not proof of cancellation. No arbitrary pointer or symbol name shall become a device address or callable function. Host-only imports shall fail MCU admission; hosted builds may admit them only under the selected platform/profile contract.

Debug/source sections may be omitted when genuinely optional. Required symbol, type, interface, or import information shall not be stripped merely because it resembles debug metadata.

STBC integrity checks and CRCs shall not be described as authentication. Production deployment requires an explicit trust model, such as physical commissioning authorization or authenticated update access; signatures are required where the product's threat model requires them.

### 5.3 Validated construction and admission evidence

**LOAD-07.** A prepared module eligible for execution shall be constructible only after successful artifact validation, capability/resource admission, and preparation. Raw decoded objects shall not be executable. Constructors, deserialization, test helpers, and embedding APIs shall not expose an accidental production bypass of this transition.

**LOAD-08.** Timing admission shall follow Sections 4.4 and 8.4. The loader or installation manager shall verify that the selected timing evidence matches the application, configuration, engine/adapter build, and platform envelope. The device need not reproduce an entire off-device analysis, but it shall independently enforce its capability/resource limits, reject mismatched evidence, and refuse a stronger claim than the evidence supports. A memory-fit result shall not be reported as timing admission.

**LOAD-09.** Candidate validation and preparation shall be transactional with respect to the installed generation: failure shall not corrupt or publish a partially prepared replacement. Work, allocations, reference expansion, and failure reporting shall remain bounded by the preparation profile. Excess demand shall fail before switching.

A validated constructor is an implementation invariant, not a substitute for fuzzing or an assertion that the validator is correct. Artifact and deployment-metadata mutation tests are mandatory under Section 14.

### 5.4 Compiler-free construction and executable initialization

**LOAD-10.** Scope A shall implement the fixture's artifact-to-runtime construction map below. Use complete existing type/vtable/task/I/O/reference/constant data and encode missing construction/initialization under specification 12 §11. STBC 2.0 is the selected compatibility decision, not the only theoretically possible format design. It gives a hard rejection boundary for old readers; this plan does not use an optional 1.x section for mandatory initialization. During the defined window, hosted authoring compositions retain 1.1 import with explicit construction context; source-free consumers accept 2.0 and reject 1.1. Do not infer missing state or invoke HIR inside the source-free engine. [R20, R21, R29, R36]

**Complete loader relocation.** Move the entire current decoder and semantic validator, including optional supported sections and malformed-input rules, into `trust-runtime-core`; do not create a reduced MCU validator. Move the byte-only `BytecodeModule` container and metadata materialization with them, reusing the core reader/helpers. Resolve its inherent-method ownership coherently: portable `encode` serialization may reside in a separate optional core module/feature, while host `encoder/` lowering, compiler/HIR integration, disassembly presentation, file I/O, and Runtime application remain host tooling/adapters. Preserve public hosted bytecode entry points through re-exports or thin compatibility wrappers. Firmware need not link encoding/presentation work it does not call. [R36]

Replace the singleton version assumption with explicit reader-supported pairs and an encoder output version; do not blindly change `SUPPORTED_MAJOR_VERSION` to 2 in every test. Preserve legacy fixtures explicitly and add 2.0 cases, rejection matrix, and resource-limit mutations. Audit all version-pair/layout branches, including current comparisons based only on `minor`, CRC defaults, and VM materialization. Six direct version-assumption test surfaces are `bytecode_container`, `bytecode_metadata`, `bytecode_decode_resource_bounds`, `bytecode_sections`, `bytecode_helpers`, and `process_image`; encoder/roundtrip/validation/optional-section and VM tests also remain in scope. Retain the tracked OSCAT 1.1 artifact and generate a separately named reproducible 2.0 fixture rather than replacing compatibility evidence. [R36]

| Required runtime data | Artifact/preparation obligation | Required acceptance case |
|---|---|---|
| Program/POU identities, global layout, and instance roots | Construct from validated POU/type/reference metadata and explicit layout/root information. | A fresh engine registers and starts the saved application without preinstalled host objects. |
| Scalar/compound defaults and declared capacities | Reuse TYPE_TABLE/CONST_POOL and typed reference metadata; encode missing declared defaults/construction roots explicitly. Do not call HIR-gated `default_value_for_type`. | Nonzero defaults, nested arrays/structures, and defaulted parameters survive cold initialization. |
| Executable local/aggregate initializers | Compile to ordinary validated STBC body ranges, indexed by POU/initialization phase/declaration identity and executed through the same dispatcher and work budget. Do not move the Expr/HIR tree walker into core. | Repeated calls with changing inputs/globals evaluate the initializer at the required call boundary; emitted artifacts need no source or initializer catalog. |
| Static locals and persistent FB/class state | Encode ownership, allocation capacity, first-use rules, and applicable restart behavior. | First use, repeated calls, and cold/warm restart match the baseline. |
| Inheritance, method/interface dispatch, and parameter binding | Prepare validated type/member/method tables, defaults, OUT/IN_OUT behavior, and receiver checks. | Two instances retain distinct state while using shared method code and checked interface dispatch. |
| Tasks, event references, process images, and bindings | Construct tasks and bindings only after validating referenced program/instance storage. | Periodic/event tasks and `%I`/`%Q`/`%M` bindings initialize without `CompileSession`. |
| RETAIN and restart initialization | Bind schema, initial state, checkpoint compatibility, and restart actions to the artifact identity. | Cold/warm initialization and rejected incompatible checkpoints preserve the specified contract. |
| Native/standard-library imports | Resolve only typed, versioned implementations admitted by LOAD-06. | Required timer/math imports resolve; unavailable or mismatched imports fail before RUN. |

**LOAD-11.** Prepare immutable bytecode initialization plans and storage before RUN; execute dynamic initializers at their required call/first-use/restart boundary. Scope/frame visibility, declaration order, parameter preservation, static once-state, instance construction, bounds, faults, and value copies follow specification 12 §7.11. Use the same executor and nested work budget as the POU body. Replace module-address cache identity with the admitted module/generation identity; remove the Expr initializer execution path from migrated runtime execution once the corresponding coverage is complete. HIR may remain in the compiler, but neither a serialized AST interpreter nor a first-use lowering cache belongs in the portable engine. [R29]

M2A shall include the construction rows exercised by its representative fixture, including a changing-input local initializer and initialized compound/FB state. The entire map shall be covered by M2B/M3 before broad parity or bounded-profile claims. Removing the `trust-hir` dependency while leaving a hidden harness initializer path does not close this work.

## 6. Memory and value model

### 6.1 Preparation versus execution

The engine shall have a preparation phase that determines its full resource demand and reserves or accepts the required storage. RUN shall only become available after preparation succeeds.

**MEM-01.** The resource model shall separately account for portable program storage, each prepared instruction/metadata representation, constants, mutable PLC globals and FB/class instances, compound backing regions, process images, operand/frame/local/temporary storage, task-ready storage, I/O staging, diagnostics, service queues, and retain staging. CPU stacks and interrupt allowances shall be recorded separately from VM stacks.

Record target `size_of`/alignment for `Value`, references, frames, and relevant compound headers, then enforce their admitted size ceilings with target-compiled assertions. A host size result does not establish a 32-bit layout. These are resource bounds, not a permanent Rust ABI or a request to pack structs unsafely. [R30]

**MEM-02.** Define per-profile maxima for encoded artifact bytes, decoded instructions/references/types/locals, operand values, frames, native-call arguments, initialization nesting, and `max_call_depth`. Apply decode/allocation limits before consuming attacker-controlled counts. Do not reserve the current host-scale 16,384 operands, 1,024 call levels, or 64 MiB container allowance on F401. Report preparation, RUN, checkpoint, and generation/reader peaks per usable region. Tie admitted depth to native call-stack use when execution remains recursive, including interrupt and fault headroom. [R30]

**MEM-03.** Operand/frame/local storage shall have capacity established before bounded RUN. A push/call uses existing storage or faults before crossing its admitted limit. Existing FB/method calls can recursively re-enter the Rust executor, so a frame-count check alone does not bound the CPU stack. Scope B records F401 MSP high-water through stack painting or equivalent instrumentation under nested calls, interrupts, and fault unwinding/abort paths; record PSP separately if used. Measure representative per-level growth and corroborate worst-path headroom with code/stack analysis. Keep a conservative finite call limit; an explicit iterative frame loop is an option if measured recursion prevents admission, not an automatic rewrite prerequisite. [R30]

**MEM-04.** The allocation audit shall include cloning/dropping strings and arrays, nested values, local initializers, Arc/Box ownership, destruction of the last owner, formatting errors, task sorting, imports/library calls, tracing, and checkpoints. Tests shall exercise first invocation, rare branches, maximum admitted sizes, fault/limit paths, and relevant generation retirement. A warmed-up success loop or reserved operand stack alone is insufficient.

**MEM-05.** Core fault records shall be fixed-size codes with bounded context on hosted and MCU targets. Rich names and messages shall be rendered outside the core control path. Existing stable fault identities shall be preserved; compatibility wrappers may retain hosted messages without making fault capture allocate or block.

**MEM-06.** Compound data shall have known capacity and execution-cost bounds. The first implementation may use preallocated typed storage before a full packed representation, but bounded execution shall not rely on recursive heap-allocating Value clones. Array/string operations, nested initialization, and reference traversal shall be bounded even when implemented by one opcode. Replacing value copies with aliases is prohibited unless the existing language contract permits that aliasing; reference lifetimes and copy semantics shall be preserved.

### 6.2 Recommended storage approach

Retain the current value behavior as the reference contract. Introduce an execution-storage boundary that can access fixed scalar slots and bounded compound regions using validated handles/offsets.

Strings shall have explicit capacities and lengths; arrays shall retain their declared bounds; structures shall use precomputed member access information; FB/class instances shall occupy pre-created storage. Copies and temporary operations shall have admitted capacity and bounded cost.

The shared VM shall use the same checked access and arithmetic semantics regardless of whether the host owns backing storage in allocated containers or firmware supplies fixed buffers. Do not introduce separate opcode implementations named “desktop” and “embedded.”

A complete packed representation is not the first extraction step. First separate ownership, then replace allocation-producing paths under behavior tests. This keeps memory redesign and module relocation independently reviewable.

**MEM-10.** Use exclusive ownership of mutable execution state and an explicit borrow of immutable prepared metadata as the initial portable boundary. Prepare dense program/type/member/slot identities so bounded execution does not depend on lazy name-map insertion or OS `RwLock` caches. Host code may own or pin generations with `Arc` outside this boundary where its threading model requires it. Do not replace every host `Arc` with `Rc`, weaken host thread-safety, or duplicate instruction semantics to make one target compile. Audit transitive string/container ownership as well as direct core imports. [R22, R24]

**MEM-11.** Document scalar slots, compound regions, frame/local/temporary reuse, initialization scratch, and copy/drop ownership. Select one bounded representation for hosts and MCUs with application-sized capacities. Scope A/B may retain `SmolStr`/`IndexMap`/`Arc` values under a bounded bring-up heap on F401/C6, recording every remaining scan allocation and refcount/destruction cost; this cannot qualify ARCH-05. Implement measured narrow storage improvements before first hardware evidence where needed. Complete atomic-reference-count retirement and bounded compound copying in M3; only then require optional C3/IMC support. Avoid a throwaway value model solely to pass M1. Retain failed fit evidence and the mandatory fixture. [R24, R30, R31]

### 6.3 References and interfaces

**MEM-07.** Executable code and validated metadata shall remain immutable within an installed execution generation. A supported hosted update may replace the generation only under Section 11.3, without invalidating borrowed storage while readers still use it. PLC references shall use validated identities/handles, not serialized native addresses. Bounds, ownership, type, generation, and lifetime checks shall remain independent of CPU pointer size; a stale handle shall not silently resolve to state in a replacement generation.

**MEM-08.** Statically allocated FB/class instances, methods, interfaces, and checked interface dispatch shall remain supported for the admitted profile. Prepare member/method/interface tables and storage requirements before RUN. Dynamic receiver selection shall retain the necessary runtime type/generation checks; shared executable method bodies shall not require duplication for each instance.

Recursive calls and indirect call targets require a conservative bound or an explicit profile restriction. Call and instruction budgets must cover the whole nested execution, not restart on every nested call. Unsupported dynamic allocation or unbounded constructs shall produce a clear admission diagnostic.

**MEM-09.** Hosted backing storage may be allocated during preparation. Capacity growth, first-use allocation/lowering, and allocation-producing destruction or cache replacement shall not occur in bounded control paths. Instrument the control thread and its synchronous callbacks, including rare/failure paths; attribute unrelated service allocations separately. Shared allocator contention and other process-wide interference shall still be exercised in stress tests. Retired-generation reclamation shall satisfy Section 11.3.

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

The current `task/readiness.rs` assigns `last_run = now`. With exact 10 ms resource samples, a 25 ms task therefore has a 30 ms period; the corrected activations are 30, 50, 80 and 100 ms, with nominal deadlines 25, 50, 75 and 100 ms. Fixed hardware-timer sampling makes non-multiple task intervals an ordinary F401/C6 case, so the same rule must be carried into both boards. This change does not redesign the resource beat or promise a one-beat bound on OS scheduling or loaded task completion. It has no dependency on portable collections, decoder relocation, STBC 2.0 or executor extraction. Implementation is pending A1; its native tests join A1's one consolidated batch, with no fifth scope or separate bug-fix run.

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

Hardware forcing and PLC breakpoints shall be disabled in the initial production MCU profile. Commissioning support may allow them only after an explicit controlled-output transition and authorization. A CPU-debugger halt must not be assumed to preserve watchdog or output supervision.

## 10. Retain and persistence

The core shall continue to decide which state is retained and how cold/warm restart behaves. The storage adapter shall decide how bytes are reliably stored on the chosen medium.

**RETAIN-01.** Checkpoints shall be coherent snapshots with a format version, application/schema identity, checkpoint sequence/generation, integrity check, and completion indication. Record the applicable execution-generation identity where needed for compatibility, and do not confuse checkpoint sequence numbers with execution-generation lifetime tokens. Native pointers or raw Rust struct layout shall not be the durable format.

**RETAIN-02.** A restart shall load only a complete, valid, compatible checkpoint. An interrupted write shall not replace the last known-good checkpoint with a partially written one.

**RETAIN-03.** The product shall document its persistence guarantee: periodic checkpoint, stop checkpoint, or a stronger hardware-backed mechanism. “RETAIN supported” shall not imply that every last scan survives arbitrary power loss.

**RETAIN-04.** Flash erase/programming and filesystem/database work shall not introduce unbounded blocking in MCU or hosted bounded control cycles. Generate or schedule a bounded checkpoint and persist outside the critical path, or require STOP when the selected storage mechanism cannot provide the needed bound. Durable-acknowledgment requirements shall be preserved or explicitly changed through the policy review below.

**RETAIN-05.** Snapshot copying, background persistence, retry work, and queue capacity shall be bounded. Some platforms can stall code execution during flash operations; those operations shall remain prohibited in RUN until verified for the selected memory layout.

**Selected persistence profiles.** The initial F401 profile checkpoints only after outputs enter the controlled STOP/maintenance state; no internal-flash erase/program runs during RUN. Its power-loss guarantee is recovery of the last successfully completed compatible stop checkpoint, not the most recent scan. A stronger RUN-time retention guarantee requires a separately designed medium/power-fail mechanism and qualified timing. Ordinary stop checkpoints do not inherently require external storage. [E61]

For hosted bounded operation, capture a coherent checkpoint in admitted preallocated storage, publish it without waiting for disk, and persist on the storage worker. Use a bounded latest-pending-snapshot mailbox: a pending snapshot may be superseded before worker ownership, while an in-progress/durable snapshot is never mutated. Define capacity/ownership transfer before implementation. Staging exhaustion faults before that cycle's normal output commit; a worker persistence failure is latched and faults at the next control boundary under the normal fault/output policy. Neither condition blocks the scan on storage. Expose the last durably completed sequence and failure state. A later persistence failure cannot retroactively cancel already published outputs. The durability guarantee is the last completed flush; queued/coalesced scans are not durable. This is the selected M3 bounded-profile behavior change, with restart, failure, output-order, and overload tests. Keep legacy synchronous behavior in the compatibility profile until that migration is explicitly released. [R12, R33]

The reviewed hosted cycle can call `maybe_save_retain_store()` before normal output writes. The initial extraction preserves this behavior in hosted compatibility compositions. The bounded profile above deliberately changes failure/acknowledgment timing and needs its own release notes and tests; it is not a behavior-preserving file move. A compatibility build retaining unbounded persistence shall not claim bounded-profile compliance. [R12]

## 11. Services and deployment

### 11.1 Non-control services

Hosted and MCU networking and management are adapters/services, not runtime-core dependencies. They may run on the same device without belonging in the VM. The full hosted product shall retain its supported services; a future MCU Modbus, CAN, or MQTT implementation shall use the same ownership boundary.

Workers shall exchange bounded messages or process-image snapshots with the control path. Polling budgets, queue overflow, stale-data limits, and critical versus noncritical failures shall be explicit. A network outage shall not make PLC execution wait indefinitely.

Rich symbol lookup, source rendering, HMI, DAP presentation, compilation, and deployment orchestration remain on the development computer or hosted product. The MCU needs only the bounded device-side hooks required by the selected features.

**SERVICE-01.** Crate boundaries shall be treated as dependency boundaries, not process-fault isolation. Services may remain in the same process where their scheduling and communication contracts satisfy the selected profile. Stronger isolation requires an explicit thread/process design and its own failure tests; this migration shall not require a process-per-service architecture.

**SERVICE-02.** Rich hosted debugging, tracing, and diagnostic rendering shall remain outside bounded callbacks. Supported pauses, forcing, and online change shall have explicit state/boundary behavior and regression tests. Debugger/export reads shall pin or snapshot the appropriate generation under Section 11.3 and shall not keep unaccounted retired storage alive indefinitely. Initial MCU restrictions shall not remove existing hosted workflows.

### 11.2 Stopped installation workflow

This is the initial MCU installation path and any applicable stopped hosted installation path. Existing hosted online changes use Section 11.3; this procedure shall not silently require STOP for all previously supported hosted updates.

```text
1. Discover/read the device's capability and resource descriptor.
2. Compile the application and build its deployment metadata on the computer.
3. Check compatibility before transfer.
4. Put the controller into the required stopped/commissioning state.
5. Transfer/stage the artifacts using the selected management transport.
6. Independently validate and prepare them on the device.
7. Mark the new installation complete using an interruption-safe procedure.
8. Report the installed application identity and readiness.
9. Start only through the configured authorized start policy.
```

**DEPLOY-01.** MCU firmware and PLC application shall be separate concepts. Firmware is compiled for the CPU/board; STBC is the loadable PLC artifact. Changing only the PLC application should not require a new platform implementation.

**DEPLOY-02.** The first MCU release shall install applications while stopped. It shall not promise live online changes. A probe-flashed, embedded STBC fixture is acceptable for bring-up, but does not satisfy the release deployment gate.

**DEPLOY-03.** A failed transfer or power loss shall never select a partially installed application as valid. Dual slots are one option, not a universal requirement; devices without spare storage shall have a documented stopped/recovery installation procedure.

**DEPLOY-04.** The device shall reject a mismatched configuration/application pair, incompatible retain schema, unsupported required import, or insufficient memory without starting the candidate application.

**DEPLOY-05.** The development environment shall offer the MCU's feature and limit profile in simulation. The simulated process model may differ, but the compiled PLC logic and runtime semantics shall not depend on changing the application for the simulator.

**DEPLOY-06.** Firmware verification shall not automatically establish authorization or authenticity of a separately downloaded PLC application. Define the application's identity, authorized deployment principals/physical commissioning policy, integrity/authentication mechanism, key/trust handling where applicable, and any rollback policy independently. Bind bytecode, configuration, and declared contracts; enforce the selected policy at installation and activation. A bootloader may support firmware recovery without implementing these application-management obligations. [E23]

### 11.3 Hosted online change and execution generations

This subsection preserves the existing supported hosted online-change contract. The reviewed implementation documents a scheduler-cycle-boundary replacement with a warm restart at the entry point and retain handling; it does not establish a no-restart state migration. The first MCU release remains stopped-installation only. [R19]

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

The boundary is the existing semantic transition point, not a claim that warm initialization or snapshot migration takes zero time.

**UPDATE-01.** Installed module metadata/code shall be immutable per generation under MEM-07. Candidate construction shall happen outside bounded execution and shall not mutate the running generation. Failed validation, preparation, or update-memory admission shall leave the prior installed generation intact; a failed candidate shall never become executable.

**UPDATE-02.** Publication shall occur only at the defined quiescent scheduler boundary, with no in-flight instruction stream continuing in replaced code. Preserve the supported warm-restart/retain behavior and existing output policy. Do not introduce CODESYS-style no-restart migration, mid-scan replacement, or automatic full-state rollback as an incidental refactor. [R19, E18]

**UPDATE-03.** Account for preparation, coherent state/retain snapshot or copying, restart, publication, and output transition separately. Timing and state-transfer costs at the switch shall fit the admitted maintenance/update window or use an explicitly approved pause/STOP workflow. Preserve existing hosted functionality in the compatibility profile while resolving any bounded-profile conflict; do not describe an unbounded switch as a bounded scan.

**UPDATE-04.** PLC handles, debugger/export references, cached lookups, and service bindings shall carry or be validated against the applicable generation/lifetime. A stale reference shall fail explicitly unless a supported, checked rebinding operation intentionally replaces it. Define token wrap/reuse and restart/identity behavior; a repeated counter value shall not let an old handle alias an unrelated generation.

**UPDATE-05.** Retired module and state storage shall remain valid until all permitted execution, debugging, and export readers release it. Reclamation, last-owner destruction, and deallocation shall not run on a bounded control path. Use an appropriate ownership/pinning or snapshot mechanism; the design does not mandate a lock-free algorithm or a separate process.

**UPDATE-06.** Admission shall cover overlapping installed/candidate/retired generations and checkpoint/export buffers. Bound retained-reader lifetime or retained-generation capacity through explicit policy; a slow debugger shall not cause indefinite memory growth. Reject/defer additional candidates safely when capacity is exhausted rather than reclaim live storage.

**UPDATE-07.** A fault during or after the committed switch shall follow the documented restart/output recovery policy. Preparation rollback does not imply transactional rollback of already mutated application state or physical outputs. The next active identity, retain result, and readiness/fault state shall be observable and tested.

**UPDATE-08.** Qualification shall exercise updates with nested calls completed at the boundary, active debugger/export readers, queued management/I/O work, stale handles, exhausted overlap capacity, invalid candidates, and generation wrap/reuse simulation. Verify output behavior, old-storage lifetime, off-path reclamation, and the preserved restart/retain contract. These tests do not qualify live MCU update support.

Preserve UPDATE-01/02's existing boundary/warm-restart behavior as the hosted code is migrated. The extra bounded generation, reader-capacity, reclamation, and wrap/reuse machinery in UPDATE-03 through UPDATE-08 belongs to M3/M4H, where actual readers and lifetimes are inventoried. Scope A/B needs valid module ownership and no address-as-identity cache; it does not need a speculative multi-generation reclamation service. Use existing ownership where it satisfies the contract and record a reader class as absent when no actual surface can hold it.

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

**Execution cadence.** Scope A is the aggregate of four bounded scopes, A1–A4 below, following the explicit 9 October 2026 planning decision. Each has one final validation batch; B and M0U remain separate. Finish the entire authorized scope and necessary test authoring before its batch. Preserve failures and obtain explicit authorization for another run; do not silently subdivide a scope to manufacture extra batches. Selecting these boundaries does not itself start implementation. Milestone labels do not create extra batches, and unrun gates remain unverified. The [implementation checklist](../internal/testing/checklists/runtime-portability-implementation-checklist.md) records the current checkpoint, scope authorization, requirement coverage, and evidence; this specification remains the behavior authority.

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
| A2 — Shared loader and validator | Full container/decode/validate/materialization boundary in core, portable collections and CRC edge, host re-exports, and byte serialization ownership. Preserve existing 1.x behavior. | Relocated unit tests and the complete existing container/metadata/resource-bound/section/helper/image/validation/optional-section corpus; malformed input and failed preparation remain rejected. Both isolated MCU graphs still check. |
| A3 — STBC 2.0 format and producer | Settle exact layouts before code; add executable initializer and construction metadata, compiler lowering, dual-major compatibility policy, producer/version selection and separate 2.0 fixtures. | 1.x regression and 2.0 encode/decode/validate/round-trip/negative cases. Keep existing hosted execution usable; candidate 2.0 emission is explicit until A4 integrates execution. No compiler-free execution claim yet. |
| A4 — Shared-engine integration | TYPE_TABLE construction, shared-dispatcher initializers, timers, execution state/context, headless source-free loading and the representative fixture, including A1's corrected periodic scheduling rule. Preserve hosted APIs and tier behavior. | `runtime_core_compiler_free_load`, integrated numeric/state/fault/oracle cases, inherited scheduler regressions, both MCU core graphs and the complete affected format/VM suites. Freeze the integrated saved-artifact oracle here. This closes M2A-H only; hardware remains B/M2E. |

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
| Bounded storage/import work; actual generation/reader capacity and retirement (UPDATE-03 through UPDATE-08); selected bounded persistence | M3, qualified by M4H/M4/M5 as applicable. No speculative reader subsystem in Scope A. |
| Full timing-admission record (§8.4/SCHED-02 through SCHED-04), release descriptor (COMPAT-02), production deployment authorization (DEPLOY-06), extended disturbances | Design the needed interfaces early; implement/qualify at M3/M4H/M4/M5 before a corresponding release claim. Scope A/B still validates all bytes, capacities, and outputs for its controlled fixture. |
| Full modernization ledger, platform acceptance, update/deployment/retain recovery, and exact or tolerance-qualified published claims | Required before closing the combined plan; independent UI/protocol upgrades need not block first-board feasibility. |

Implementation of ownership/data-flow changes updates `docs/diagrams/architecture/runtime-execution.puml` and `runtime-bytecode-vm-execution.puml` where affected; render/drift checks belong to that scope's batch. New crates/large-file policies follow architecture-doctor and recorded waivers where required. CHANGELOG and release-version rules apply when corresponding behavior is implemented/released, including STOP latency, numeric baseline, and persistence changes. A planning document is not a shipped feature or version bump.

## 14. Verification and acceptance

### 14.1 Required evidence

| Gate | Required result |
|---|---|
| Portable build | Separate actual F401 and selected ESP32 target checks plus linked firmware; no hosted/HIR leak or unresolved dependency/atomic assumption hidden by a combined host build. |
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

Add exact linked-firmware commands for `trust-nucleo-f401re` and the selected ESP32 composition, map extraction, and physical runners when those projects exist. Extend the existing Linux/macOS/Windows CI matrix with isolated runtime-only and saved-artifact tests rather than treating existing full-workspace tests as that proof. These checks are not assumed to pass on the reviewed baseline. Core checking alone does not measure size, prove linking, demonstrate bounded execution, or establish hardware readiness.

### 14.4 Hosted resource and performance assessment

**PERF-01.** M0 shall capture per-platform baselines and acceptance thresholds before optimization. Compare equivalent release builds and the same saved artifacts/traces, matching features, checks, scheduling/numeric/import contracts, preparation strategy, toolchain, hardware/OS, clock/cache settings, and I/O/load conditions. A feature-reduced new runtime-only build shall not be reported as an equivalent full-product improvement; report both comparisons separately.

Default attribution is extraction first: compare original and extracted code under Rust 1.95.0 with matching features/contracts, separately identifying the explicit portable-math and nominal-deadline scheduling changes. Historical executions of the scheduling defect are not correctness oracles for the corrected contract. Then compare extracted-before-M0U and extracted-after-M0U under their recorded compiler/dependency tuples. If modernization is explicitly scheduled first instead, preserve the modernized-before-extraction baseline. In either order, mixed end-to-end measurements do not identify a single cause, and `rust-version` does not identify a historical build compiler.

**PERF-02.** Report shipped and loadable-code sizes separately; runtime, application, adapter/helper, and service costs; preparation duration/peak memory; per-region steady RUN and compound storage; candidate/update/retirement/checkpoint peaks; CPU/interrupt and VM stacks; control-path allocator events; and core-only/full-workspace build/test cost. Do not confuse bytes stored in flash with extra prepared RAM or temporary loading duplication.

**PERF-03.** Measure release/wakeup lateness, task and batch execution, computation response, logical output submission, driver acceptance, and any observed bus-transfer/device-feedback completion separately. Preserve raw distributions and maxima under declared HMI, network, logging, storage, interrupts, and platform interference. Label unavailable physical observations honestly. Inject blocking, queue overflow, service failure, and delayed output work to verify the defined failure policy.

**PERF-04.** Smaller artifacts, faster scans, lower jitter, and shorter builds are hypotheses, not consequences guaranteed by extraction. A gate passes only when declared requirements are met and regressions are explained/approved. Record missing evidence as missing. Longer tests, margins on observed maxima, and correct overrun faulting do not establish a mathematical WCET bound, a failure rate, or successful execution within a violated deadline.

### 14.5 Definition of done

**Hosted migration is complete** when Linux, Windows, and macOS products use the shared engine, pass native compiler-free loading and contract-qualified trace tests, retain existing supported workflows including applicable generation changes, qualify their bounded control paths, and provide per-platform baseline-relative memory/timing evidence. Timing classification and unresolved compatibility/runner exceptions shall be stated explicitly; an unresolved exception is not a passed gate.

**The first MCU release is complete** when the NUCLEO-F401RE boots into its controlled state, accepts an authorized and independently validated application/configuration, prepares it without the compiler, executes the shared VM under admitted resource/scheduling/numeric contracts, meets its required completion level within the recorded timing envelope, and handles fault, restart, retain, and interrupted stopped installation as specified. Integration, disturbed/extended tests, and per-region resource evidence are all required; the release shall state its timing-evidence classification. A changed first-board selection requires an explicit scope decision and preserves the F401 evidence status.

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

The first board model is confirmed as NUCLEO-F401RE. Its physical revision/marking and Appendix D settings still need recording before hardware work. The proposed ESP32-C6 board's availability, exact module/revision, and usable memory remain to be confirmed. Pin HAL/BSP/toolchain versions, actual memory reservations, I/O mapping, management transport, checkpoint medium, interrupt/watchdog policy, and time/space headroom for each build. No measured truST footprint or universal family support is supplied here.

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

The modernization deliverable is complete when every in-scope dependency/tool has a current disposition, all selected migrations and lockfiles are implemented, the support/MSRV/tool documentation agrees, and the applicable evidence passes. Report retained versions and exceptions plainly; do not label a qualified subset “all packages latest.” Saved PLC traces and NUM-05 outcomes must remain compatible across the selected build changes. Attribute performance changes according to PERF-01 rather than presenting compiler gains as evidence for extraction.

## 17. Rust design patterns for this runtime

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
[research notes](../notes/runtime-portability/research-notes.md). They are
non-normative provenance; no build or implementation step depends on files in a
personal directory.

**D01 — Original proposal.** Version 0.2; archived identity in the research notes.

**D02 — Architecture research.** The 8 October research review and its amendment mapping are retained in the notes.

**D03 — Version 0.3 input.** Historical design input; superseded by this specification.

**D04 — Version 0.4 input.** Earlier target/implementation review; superseded decisions are identified in the review archive.

**D05 — Dated dependency snapshot.** [Registry inventory](../notes/runtime-portability/trust-runtime-modernization-inventory-2026-10-09.json), SHA-256 `6a088873d343cced6c536442fdd32e63cf0687017c64bf62ad12cea4d1e9f6d0`. The 9 October snapshot covers 74 direct registry crates, 30 npm packages and five Git declarations across 21 manifests. Refresh candidates in M0U; this is not a resolved upgrade set or advisory audit.

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

## Appendix B — Revision history

| Version | Date | Change |
|---|---|---|
| 0.10 | 9 October 2026 | Moves the dependency-free SCHED-05 implementation and native regressions to the start of A1, separately reviewed within its existing batch. Removes deferred-defect baseline acceptance; A4 keeps fixture/oracle integration. Four scopes remain; no implementation or validation is asserted. |
| 0.9 | 9 October 2026 | Folds the periodic scheduling defect into Scope A/A4 as SCHED-05; aligns specifications 10/11, selects nominal deadlines for SCHED-01/CYCLE-03, and adds the 25 ms / 10 ms native and shared-artifact oracle. Implementation, validation, version bump and release remain deferred to the authorized implementation scope. |

Version 0.8 divides aggregate Scope A into A1–A4, with one final batch per explicitly
bounded scope, and adds the implementation checklist/current-checkpoint pointer.
The product requirement identifiers and architecture remain unchanged.

Version 0.7 separates M0U from Scope A/B, specifies portable map/hashers and full
bytecode relocation, bounds legacy-version compatibility, retains libm's default
feature with explicit numeric premises, and separates product requirements from
dated research. The [complete revision history](../notes/runtime-portability/research-notes.md#revision-history-through-v06)
preserves earlier snapshots and decisions.

## Appendix C — Amendment traceability

The [research amendment mapping](../notes/runtime-portability/research-notes.md#earlier-research-amendment-traceability)
is historical provenance. Current implementation timing is defined by §13.2;
requirement identifiers in this specification remain authoritative. Review labels
such as REV-MODEL-05 are not product requirement identifiers.

## Appendix D — Target decisions and first-board feasibility

### D.1 NUCLEO-F401RE: confirmed first target

NUCLEO-F401RE is the selected first reference board. Its role is to establish compiler-free execution and useful resource evidence on hardware already available. It is not yet a supported truST controller. [E24]

| Area | Initial decision | Evidence or remaining implementation decision |
|---|---|---|
| Board identity | NUCLEO-F401RE / STM32F401RE. | Record PCB and silicon revision, probe identity, and actual MCU marking before flashing. Model confirmation is not a physical inspection. |
| CPU/toolchain | Cortex-M4F; `thumbv7em-none-eabihf`; single-precision hardware floating point. | Pin Rust/LLVM and CPU/FPU flags; record actual clock. Up to 84 MHz is the part limit, not a measured or configured runtime speed. [E25] |
| HAL/runner | Start with `stm32f4xx-hal` and its `stm32f401` feature, architecture startup support, and one bare-metal runner owning the engine. | Pin compatible HAL/PAC/startup versions. No RTOS is required for the initial GPIO/timer/serial composition; any later executor remains outside PLC semantics. [E28] |
| SRAM | 96 KiB total device SRAM. | Deduct `.data`/`.bss`, CPU/IRQ stack, loader/preparation storage, all engine regions, queues, I/O buffers, and declared margin. Account for preparation and RUN peaks separately. Do not reserve desktop VM maxima. [E25] |
| Flash | 512 KiB total device flash. | Partition firmware, saved/deployable application, installation metadata, and durable retain/recovery storage at actual erase-sector boundaries. Count numeric helpers and all loadable sections. [E25, E27] |
| Clock/wake | A hardware timer supplies extended monotonic time; the runner implements PORT-03 wait/wake. | Select timer, prescaler, resolution, rollover extension, interrupt priorities, and reset epoch. Validate clock startup and wake races. |
| Initial physical I/O | Board user button and LED through process images; proposed logical bindings `%IX0.0` and `%QX0.0`. | UM1724 maps the relevant button to PC13 and LED/D13 to PA5 for this board. Confirm bridges/polarity on the actual revision; normalize them in the BSP. A physical smoke check supplements the richer M2A trace fixture. [E26] |
| Programming/management | Onboard ST-LINK/SWD for firmware bring-up; bounded serial management is the initial application-installation direction. | Verify ST-LINK virtual COM routing and UART setup; distinguish probe-flashed firmware from the stopped STBC installation required for release. [E24, E26] |
| Fault supervision | Independent watchdog and explicit BOOT/STOP/FAULT output behavior. | Record watchdog clock/tolerance, timeout, debug-freeze behavior, and reset output state; service only under supervisor policy. [E27] |
| Persistence | Internal flash stop checkpoints; last completed compatible checkpoint survives power loss. | No last-scan retention promise. Specify separate valid/new checkpoint storage, interrupted-write recovery, endurance, and watchdog servicing during maintenance. External storage or power-fail hardware is needed only if the stronger selected policy requires it. [E61] |
| Numeric behavior | Preserve `REAL`/`LREAL` widths and NUM-05 evaluation rules. | Budget software double precision on M4F; no silent narrowing to make the fixture fit or run faster. |

**Feasibility decision.** Start with this board. Scope A establishes host construction and cross-target source/layout evidence; Scope B determines whether the shared application and preparation peak fit with stack/interrupt and driver headroom. Measure nested-call MSP high-water, not just the logical VM frame count. The desired result includes timers, bounded compound state, interface dispatch, and correct faults. No fixed footprint or maximum application size is promised before measurement.

If the F401 does not fit, preserve the failed map/peak ledger and identify whether executable metadata, duplicated code, compound storage, initialization scratch, native stack, or linked helpers dominate. Prepare a targeted common-core improvement or an explicit application/target decision under the authorized scope; do not automatically rerun validation. The same failed gate remains open until an authorized run succeeds. An H7 may later provide a larger supported profile while the F401 result remains separately visible.

Internal-flash installation and persistence shall not erase the running firmware or the only recovery path. The reserved layout must preserve independent application installation; firmware and PLC application shall not become inseparable simply to fit the first board. A stopped recovery procedure is permitted under DEPLOY-03, but arbitrary power-loss retention of the last scan is not implied.

RM0368 describes four 16 KiB, one 64 KiB, and three 128 KiB sectors for this density; flash reads can stall during erase/program. DS10086 Table 45 lists erase times on a millisecond-to-second scale, with voltage/parallelism-dependent limits. Budget maintenance/watchdog behavior using the selected conditions. The part offers RTC backup registers, not a general backup-SRAM store. Do not reserve “256 KiB firmware + two 128 KiB sectors” before measuring linked size and proving the application/checkpoint recovery scheme fits; a single erase unit alone does not provide two independently recoverable checkpoint slots. [E61]

### D.2 ESP32: required second architecture

| Area | Proposed first profile | Decision to freeze or evidence required |
|---|---|---|
| Board/chip | ESP32-C6-DevKitC-1, HP RV32IMAC core; provisional, not owned/ordered by this plan. | Confirm physical availability, module/flash size, and board revision in M0. Keep one PLC engine owner on the HP core. Another MCU family cannot satisfy ARCH-10. [E56, E57] |
| Rust/HAL | `riscv32imac-unknown-none-elf` with `esp-hal`; radios disabled. | Pin compiler/HAL/startup/flash tools and linker layout. This is a stable bare-metal target, not the similarly named ESP-IDF target. [E56] |
| Atomic/ownership boundary | IMAC permits existing atomic-backed values during measured bring-up; no new atomic RMW dependency in core. | Retire them in the shared bounded-storage slice before M3. C3/IMC becomes an additional gate after that work if selected; portable-atomic alone does not rewrite `alloc::sync::Arc` consumers. [R24, E46, E56] |
| Numeric cost | No F/D floating-point extension on the selected target; REAL and LREAL use software arithmetic. | Measure numeric helpers/latency on C6 independently of F401; an ARM single-precision result is not a C6 estimate. Optional C3 also needs software floats. [E56, E57] |
| Memory/flash | Explicit usable internal-memory and linker/partition budget. | Record code/data/stack/driver reservations, preparation peaks, and actual module flash. Do not assume external RAM or treat LP memory as freely interchangeable HP engine SRAM. [E57] |
| Runner and I/O | One engine owner, timer wakeups, staged I/O, bounded serial management. | Interrupts/callbacks notify the runner or fill staging buffers; they do not independently run PLC tasks or mutate its active state. Freeze pin/strapping and reset behavior. |
| Radio/services | Initial parity profile has Wi-Fi/BLE disabled. | If later enabled, include its actual radio scheduler, interrupts, buffers, contention, and load in memory/timing qualification. Espressif documents background runtime requirements for its radio stack. [E33] |
| Alternative ESP-IDF composition | Available as an explicit platform integration choice if required drivers/services justify it. | Bind IDF, Rust wrapper, FreeRTOS/task, and synchronization versions; use the same core and PLC scheduler. Do not pull the desktop `trust-runtime` product into MCU firmware. [E34] |
| Flash interference and supervision | Forbid installation/erase during RUN in the initial profile. | Qualify later persistence against the C6 stack/memory placement. E35's earlier ESP32 IDF guidance is background, not C6 bare-metal proof; review the selected implementation before permitting flash work in RUN. |
| Milestones | M1 isolated core check, M2E linked firmware and saved-artifact execution, M5 qualification. | Missing hardware leaves physical gates open while independent host/F401 work proceeds. |

The RISC-V choice exposes a different ISA, soft-float cost, interrupts, and platform stack early while keeping full value-model retirement off the initial dependency gate. A future C3/IMC or S3/Xtensa profile has its own toolchain/ownership and hardware gates; C6 qualification does not support the whole ESP32 family. If the target selection changes to C3, explicitly move atomic/string retirement before its first core build rather than promising both the old M1 timing and deferred storage work.

### D.3 Optional STM32H7 profile

An H7 is a later capacity/performance option, not the starting-board requirement. If selected, add a named board/HAL profile with memory-bank availability, DMA reachability, cache/MPU maintenance, clock/FPU setup, and watchdog/output behavior. ST's Cortex-M7 cache/DMA guidance applies to this profile; do not carry its memory assumptions into F401 or ESP32 code. Larger capacity does not waive the shared fixture or turn F401 failure into success. [E37]

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

The [candidate table](../notes/runtime-portability/research-notes.md#dated-dependency-candidates)
and [registry snapshot](../notes/runtime-portability/trust-runtime-modernization-inventory-2026-10-09.json)
are dated research under D05, outside the normative specification directory.
Section 16 governs the separate modernization scope: refresh metadata, record
upgrade/replace/retain decisions, then freeze and qualify the selected graph.

## Appendix G — Design review record

| Review item | Disposition and pending evidence |
|---|---|
| Periodic-task phase defect, 9 October 2026 | Accepted as SCHED-05 with coordinated spec 10/11 amendments. The initial A4 placement is superseded by the sequencing review below. Reuse the draft readiness/regression changes, add missing non-multiple cases and discard the unsupported 75-runs-in-10-seconds hosted-runner claim. Current code still uses sampled-time baselines; implementation and validation are pending. |
| Scheduling-fix sequencing review, 9 October 2026 | Accepted the review's four-batch alternative: implement and separately review the dependency-free fix first in A1, with its native regressions sharing A1's existing batch. A1 establishes the corrected scheduling contract for subsequent scopes. A4 keeps only fixture/oracle integration for this fix; metadata remains non-blocking maintenance. No new implementation authorization or fifth batch is inferred from the review. |

The [review archive](../notes/runtime-portability/research-notes.md) retains all
17 v0.4 dispositions and the eight v0.6 follow-up dispositions, including corrected
claims and unexecuted evidence. Current choices are expressed in the requirements
and milestones above; historical approval of a plan is not runtime or hardware
qualification. A1 fixes scheduling and freezes the corrected native and numeric
fixtures; A3 settles exact 2.0 layouts before
format implementation. A4 proves integrated source-free execution, and Scope B/M2E
supplies physical resource evidence.
