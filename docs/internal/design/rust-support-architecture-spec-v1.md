# Rust in truST — Architecture Specification v2.4 (Rust-first)

**Status:** north-star architecture — **big decisions D14–D24 LOCKED**
(2026-07-03 review); v1 core D1–D13 carried and stable; D25–D26 lock the
SDK-distribution and deploy-inventory decisions needed before VS Code
implementation starts; §20 open questions are non-blocking unless a slice
explicitly names them.
**Date:** 2026-07-03 (v2 same day as v1; v2.1 lock edits, v2.2
soundness corrections, and v2.3 sequence-lowering correction same day)
**v2.2/v2.3/v2.4 corrections (technical reviews, locks retained on the
corrected form):** (1) cycle sequences re-specified as a **restricted
`#[trust_sequence]` validated/lowered language** — the v2.1 "sealed
awaitables = type error" claim was unsound (`.await` accepts any
`IntoFuture`); enforcement is build-time validation + runtime deadline
backstop, exchange access is closure-scoped (RS-75..80, F23, D18). v2.3
further specifies that sequences lower to explicit SDK `TrustSequence` state
machines with per-poll `Exchange`/`CycleCtx` access, not stored Rust futures.
(2) timing admission re-specified as **worst-frame/hyperperiod simulation**
of the actual non-preemptive single-thread scheduler — the v2.1 formula
double-counted frame-global costs and read as utilization math
(RS-115..117, D15). Claim-vocabulary sweep: admission "admits-or-refuses"
(never "proves"), absence claims dated and scoped.
v2.4 hardens the implementation package: the IDE/backend requirements
RS-118..126 are binding, VS Code slices are mandatory gates, SDK trait shape
is reconciled with §6.1, committed generated artifacts are target/profile
invariant, and the SDK distribution/deploy inventory decisions are locked.
**Document set:** this file is the **master architecture document**. Derived,
audience-specific extracts (changes land here first, then re-derive):
`rust-plc-product-contract.md` (who it's for, DX contract, differentiators,
demo gates), `rust-plc-runtime-contract.md` (normative implementation
contract: RS index, seams, conformance matrix), and
`rust-plc-implementation-board.md` (execution plan as **vertical slices**,
S1 first), plus implementation companions:
`rust-plc-vscode-workflow-v1.md`,
`rust-plc-complete-implementation-checklist.md`,
`rust-plc-sdk-macro-contract.md`,
`rust-plc-codegen-digest-contract.md`,
`rust-plc-cli-json-contract.md`,
`rust-plc-bootstrap-packaging-contract.md`,
`rust-plc-conformance-fixture-matrix.md`, and
`rust-plc-architecture-ownership-map.md`,
`rust-plc-vscode-acceptance-journeys.md`. Where a derived document
disagrees with this file, this file governs.
**Owner:** Johannes
**Hand to:** runtime + compiler + tooling implementers. Build against this contract; review against it.
**Supersedes:** v1 (archived beside this file as
`rust-support-architecture-spec-v1-archived-2026-07-03.md`; filename kept for
link stability). v1's runtime core — tiers, module model, ABI, fault model —
is carried forward nearly intact. What changes is the **product surface**:
v1 answered *"how do we run Rust safely inside the PLC?"*; v2 additionally
answers *"how does a software engineer build a complete machine controller in
Rust, never writing Structured Text?"* — and *why they would choose to*.
**Scope:** Rust as a first-class implementation language **and authoring
surface** for PLC systems in truST: execution tiers, module model, ABI,
Rust-first project model, timing admission, crate admission, cycle
record/replay, testing, services, toolchain, phased delivery.
**Out of scope:** functional-safety certification (SIL), non-Rust guest
languages, marketplace/catalog product design, HMI authoring in Rust (§20 Q10).

Production bar: every phase in §19 ships production-complete or not at all —
no "MVP"/"pre-release" framing. File/line references were re-verified against
the workspace on branch `ads/client` on the date above (two independent code
surveys; corrections over v1 are marked **[v2 correction]**).

---

## Table of contents

- [0. Executive summary](#0-executive-summary)
- [1. The product case — what truST adds to Rust](#1-the-product-case--what-trust-adds-to-rust)
- [2. Goals and non-goals](#2-goals-and-non-goals)
- [3. Current state of truST (grounding survey)](#3-current-state-of-trust-grounding-survey)
- [4. Architecture: one runtime model, two authoring surfaces](#4-architecture-one-runtime-model-two-authoring-surfaces)
- [5. The Rust-first project model](#5-the-rust-first-project-model)
- [6. The programming model (SDK)](#6-the-programming-model-sdk)
- [7. Module model and ABI](#7-module-model-and-abi)
- [8. Execution semantics](#8-execution-semantics)
- [9. Timing admission](#9-timing-admission)
- [10. Crate admission](#10-crate-admission)
- [11. Services outside the cycle](#11-services-outside-the-cycle)
- [12. Observability, record, and replay](#12-observability-record-and-replay)
- [13. Testing strategy](#13-testing-strategy)
- [14. Failure modes, safety, and security](#14-failure-modes-safety-and-security)
- [15. Toolchain and IDE](#15-toolchain-and-ide)
- [16. Worked examples (real-world)](#16-worked-examples-real-world)
- [17. Industry landscape and positioning](#17-industry-landscape-and-positioning)
- [18. Decision log](#18-decision-log)
- [19. Phased implementation plan](#19-phased-implementation-plan)
- [20. Open questions](#20-open-questions)
- [Appendix A — IEC 61131-3 ⇄ Rust type mapping](#appendix-a--iec-61131-3--rust-type-mapping)
- [Appendix B — Native module ABI definition](#appendix-b--native-module-abi-definition)
- [Appendix C — Manifest schemas (trust.toml, module.toml)](#appendix-c--manifest-schemas-trusttoml-moduletoml)
- [Appendix D — Glossary](#appendix-d--glossary)

---

## 0. Executive summary

### 0.1 The thesis

> **Rust gives you the language. truST gives you the machine.**

Plain Rust already gives a software engineer a world-class systems language:
enums and pattern matching, generics, cargo and crates.io, memory safety,
`#[test]`, rustfmt/clippy, refactoring tools, and an AI-tooling ecosystem that
actually knows the language. What plain Rust does **not** give them is a
machine controller: scan cycles with priorities and watchdogs, an addressed
process image, fieldbus drivers, retained state that survives power loss,
safe-output policy, forcing, live values, an HMI, a historian, commissioning
tools, deployment and rollback, and operator-grade diagnostics. That layer is
years of plumbing — and it is exactly the part of a control system that has
nothing to do with *your machine*.

truST already ships that layer (§3 grounds every item in code). This
specification makes Rust a first-class citizen of it, **twice over**:

1. **Rust inside PLC projects** (v1's contract, carried forward): Rust
   FUNCTIONs, FUNCTION_BLOCKs, and PROGRAMs as digest-pinned declared POUs,
   callable from ST, scheduled by tasks — the brownfield door for existing
   PLC users.
2. **PLC projects in Rust** (v2's addition): a project whose sources are Rust
   and one manifest (`trust.toml`), where every ST/IEC artifact is a
   *generated, inspectable compatibility artifact* — the greenfield door for
   software engineers. `trust new rust-plc`, `cargo test`, `trust sim`,
   `trust build`, `trust deploy`. No hand-written ST anywhere in the normal
   workflow.

Both doors open into **one runtime model**: the same compiler, the same
scheduler, the same process image, the same tooling. Rust-first is a frontend
(codegen + SDK + CLI), not a second universe. That is the load-bearing
architectural decision of v2 (D14), and it is what keeps this buildable.

### 0.2 What v2 changes over v1

| # | v1 | v2 |
|---|----|----|
| 1 | ST is the composition root (P1); Rust modules are composed *by* ST | The **compiled configuration** is the composition root; its *source* is either hand-written ST (brownfield) or `trust.toml` + Rust exports generating ST (greenfield). Neither door is privileged (D14) |
| 2 | Generated ST declarations for Rust exports | Same, **plus** generated `CONFIGURATION`/`VAR_CONFIG`, generated `io.toml`/`runtime.toml` — the whole bundle is derivable from Rust + manifest (§5) |
| 3 | Timing = per-invocation deadlines + watchdog (enforcement) | Same enforcement, **plus timing admission**: the build simulates the real scheduler's base frames over the task hyperperiod from evidence-graded budgets and refuses configurations whose worst frame cannot fit, before deploy (§9, D15) |
| 4 | Crate policy = lint list (RS-24) | Full **crate admission**: dependency-graph classification with layered evidence — curated DB, static scans, and *measured* tripwires (allocation, syscalls) in the timing harness (§10, D16) |
| 5 | Replay mentioned as a sandbox debugging story (RS-43) | **Record/replay is a flagship product feature**: cycle traces, replay in `cargo test`, trace diff, overrun explanation, black-box flight recorder (§12, D17) |
| 6 | Module unit tests + conformance suite | Same, **plus the machine in your test suite**: an in-process simulation harness (`trust-sim`, built on the existing `TestHarness`) drives full scan cycles with virtual time from ordinary `cargo test`; property tests and replay tests ride it (§13, D20) |
| 7 | Step-function `cycle()` only | Same foundation, **plus cycle sequences**: a restricted, macro-validated sequence language in Rust's `await` syntax, lowered to SDK `TrustSequence` state machines with per-poll process-image access — sequential automation logic without hand-rolled state enums (§6.3, D18) |
| 8 | I/O binding via hand-written `VAR_CONFIG` | Same mechanism underneath, **plus typed I/O**: addresses in `trust.toml`, and device import (`trust io import`) generating typed Rust bindings — the shipped ADS browse→generate pipeline extended to Rust (§6.2, D19) |
| 9 | — | Explicit product-value and positioning case: what truST adds to Rust, and why this beats "TwinCAT but with Rust" (§1, §17) |

Everything else — the three tiers, one-module model, `repr(C)` exchange
blocks, copy-in/copy-out, digest pinning, native/wasm forms, catch-at-boundary
panic policy, honest TCB statements, the failure catalogue, RBAC/signing — is
carried from v1 with corrections marked inline.

### 0.3 The six differentiators

The state-of-the-art claim, phrased the way we can defend it (§17.3 carries
the normative wording): we have not found a shipping platform — commercial
or open — offering more than fragments of this list as one product. truST's
architecture supports all six with mechanisms, not marketing (each is a
section of this spec):

```
1. Scan-native Rust        Memory-safe systems language as first-class POUs
                           inside the scan, with per-form honest containment.   §7–§8
2. Cycle sequences         restricted await-syntax sequences across scan
                           cycles: sequential machine logic in straight-line
                           code, build-validated, deterministic, SFC-
                           equivalent semantics.                                §6.3
3. The machine in CI       cargo test spins the full controller (tasks, I/O
                           image, retain) in-process with virtual time;
                           property tests and trace replay included.            §13
4. Timing admission        The build admits-or-refuses the task set by
                           simulating worst-case base frames from evidence-
                           graded budgets. We have not found another PLC
                           build system that answers "does this fit?" (§17).    §9
5. Crate admission         crates.io made usable responsibly: dependency
                           classification with measured (not pretended)
                           evidence of scan-safety.                             §10
6. Record & replay         Cycle traces as commissioning evidence, CI
                           regression gates, black-box fault forensics, and
                           overrun explanations that name the culprit.          §12
```

### 0.4 The adoption ladder

Product framing for docs/marketing; each rung is independently useful and
de-risks the next:

```
Rung 0  Rust service beside any truST runtime (tier 3)      zero scan risk
Rung 1  One Rust FB inside an existing ST project (tier 1)  brownfield value
Rung 2  A Rust PROGRAM on a task (tier 2)                   Rust owns a cadence
Rung 3  Rust-first project: no hand-written ST at all       greenfield product
Rung 4  Fleet: crates as shared FB libraries, CI-gated       ecosystem
        deploys, replay-diffed upgrades
```

### 0.5 Key decisions added in v2 (full log in §18)

**D14–D26 were locked at the 2026-07-03 reviews.**

| ID | Decision |
|----|----------|
| D14 | Rust-first authoring surface = `trust.toml` + Rust macros **generating** the ST configuration and bundle files; one compile path, two doors |
| D15 | Timing admission is a build/deploy gate with graded evidence (`declared`/`measured`/`wasm_fuel`/`static_stbc`), computed by worst-frame/hyperperiod simulation of the actual non-preemptive scheduler — not a WCET claim, not utilization math |
| D16 | Crate admission classifies RT reachability with layered evidence; production profiles reject `service_only`/`unknown` crates in scan paths |
| D17 | Cycle record/replay is a core product feature; determinism contract (RS-43/44) is its foundation |
| D18 | Cycle sequences: a restricted `#[trust_sequence]` language in Rust's `await` syntax — build-validated wait points (foreign awaits/async blocks/unknown macros rejected), lowered to an SDK state machine polled once per cycle, restart-from-top (SFC-equivalent), deterministic; general async stays in services |
| D19 | Typed I/O: manifest-declared addresses + device-import codegen (ADS pipeline precedent) generating both ST `VAR_CONFIG` and typed Rust bindings |
| D20 | `trust-sim` public simulation harness wraps the existing `TestHarness`; `cargo test` is a supported way to run the machine |
| D21 | User-facing SDK crate is `trust-plc` (traits `PlcProgram`/`PlcFb`); `trust-module-abi` stays the internal ABI crate |
| D22 | Rust→ST *in-cycle* calls are deferred; composition across languages happens at task order + shared variables (revisit with field evidence) |
| D23 | Source-built modules keep v1's exact-fingerprint pin; precompiled *packages* move to ABI-line compatibility in a later phase, never silently |
| D24 | `trust` umbrella CLI + `cargo trust` front the existing `trust-runtime`/`trust-dev` commands for the Rust-first workflow |
| D25 | SDK distribution v1 uses crates.io exact pins stamped by `trust new rust-plc`; bundled/offline SDK source is deferred |
| D26 | Deploy target inventory is fleet-level infrastructure owned with Devices & Connections; projects reference target names |

---

## 1. The product case — what truST adds to Rust

This section exists because "we support Rust" is not a product. The product
is the answer to two questions a software engineer will actually ask.

### 1.1 "Why not just write a Rust binary on an industrial PC?"

Because then you build the controller *and* the control system. The table
below is the DIY bill of materials for a credible machine controller, against
what truST ships **today** (every claim code-verified; paths under
`crates/trust-runtime/src/` unless noted):

| You need | DIY cost in plain Rust | truST today |
|----------|------------------------|-------------|
| Deterministic scan loop with prioritized periodic + event tasks, overrun accounting | Hand-rolled tokio/timer loops; drift, jitter, no priorities | Resource scan loop + task readiness/priority sort (`scheduler/runner_loop.rs:60-229`, `trust-runtime-core/src/task.rs:58-94`, `cycle.rs:15-26`) |
| Addressed process image with typed access | Invent your own `%I/%Q/%M` equivalent | Byte-addressable I/O images + symbolic bindings copied each cycle (`io/interface.rs:173-364`), `%IX0.0`-style parsing (`io/addressing.rs:11-90`) |
| Fieldbus and device I/O | Integrate ethercrab/modbus crates yourself, per project | Driver registry: `ethercat` (ethercrab master), `modbus-tcp`, `mqtt`, `gpio`, `simulated`, `loopback` (`io/registry.rs:47-66`); ADS client with symbol browse + codegen (`host/ads/`), OPC-UA server *and* client (`host/opcua/`) |
| Retained state across power cycles | serde + fsync discipline + schema migration, hand-rolled | Versioned retain codec (magic `STRN`, CRC32), atomic tmp+rename+dir-fsync writes, typed value migration with events (`retain/codec.rs`, `retain/store.rs:21-58`, `runtime/restart.rs:169-384`) |
| Safe outputs on fault + watchdog | Roll your own; get it wrong once | Output-commit deadline watchdog + `Halt/SafeHalt/Restart` fault policies + configured safe-state writes through every driver (`trust-runtime-core/src/watchdog.rs`, `runtime/io_subsystem.rs:85-92`) |
| Live values, forcing, online writes | Build a debug server | Control plane with `io.read/write/force/unforce`, `eval/set/var.force`, cycle-coherent snapshots, queued cycle-aligned writes (`control/`, `host/debug/control/api/writes_forcing.rs`) |
| Debugger | printf | Full DAP adapter (`crates/trust-debug`): breakpoints, stepping, watches, plus `stIoWrite/stIoForce/stIoRelease` custom requests wired into VS Code |
| RBAC'd remote access with audit | Auth from scratch | Viewer<Operator<Engineer<Admin per-command policy, token/pairing auth, audited requests (`control/auth.rs:27-71`, `control/policy.rs:31-105`) |
| HMI + historian + metrics | A web app + a TSDB | Embedded web HMI with websocket diff-push (`web/hmi_ws.rs`), historian with JSONL + Prometheus export (`host/historian/`), cycle/task/percentile metrics (`host/metrics.rs`) |
| Simulation | A test double, maybe | Headless sim mode with physics couplings/disturbances (rapier3d) and time scaling (`host/simulation.rs:19-38`; `trust-runtime run --simulation --time-scale N`) |
| Deploy, rollback, packaging | rsync and prayer | Bundle deploy with `current`/`previous` symlink rollback (`bin/trust-runtime/deploy/`), signed deploys (`web/deploy.rs`), a package registry (`host/registry/`) |
| Fleet/northbound | Per-project glue | zenoh mesh runtime-to-runtime pub/sub (`host/mesh/`), mDNS discovery/pairing, OPC-UA/ADS/MQTT northbound, Open-OT shared-memory telemetry (`runtime/openot_telemetry.rs`) |
| An IDE someone else can use | — | VS Code extension (Devices & Connections, Live Values, HMI panel), LSP, embedded browser IDE (`editors/vscode/`, `crates/trust-lsp`, `web/ide_routes/`) |

The pitch in one line: **you write `cycle()` and the sequence logic; truST is
everything around it.** A DIY binary re-implements this table badly, or skips
it and pays at commissioning time.

### 1.2 "Why not TwinCAT/CODESYS — they're mature?"

They are — for the workflow they were built for. A software engineer walking
into them hits, in order: a Windows-locked or vendor-locked IDE; a 1970s-era
language with no enums-with-data, no generics, no package manager, no
refactoring, and near-zero AI-tooling knowledge; version control on XML
exports or binary blobs; unit testing as a bolt-on product; libraries as
vendor catalogs, not ecosystems; and vendor C++ as the only escape hatch —
inside the TCB, without admission, sandboxing, or crate ecosystems. §17 does
the platform-by-platform verification. The structural point:

- **Their high-performance escape hatch is C++ in the TCB** (TwinCAT
  TcCOM/PLC++, CODESYS external components, PLCnext C++ components). No
  memory-safe option, no sandbox tier, no dependency admission — "trust the
  vendor module" is the whole story.
- **Their engineering workflow is IDE-first, not repo-first.** Text sources,
  `cargo test`, CI gates, code review diffs, reproducible builds — the things
  software teams consider table stakes — are aftermarket products there, and
  first-class here.
- **None of them can answer "will this fit in the cycle?" at build time**, or
  "which of my dependencies is allowed in the scan?", or "replay last night's
  fault in my test suite". Those are §9, §10, §12.

truST's opportunity is not "PLC vendors, but cheaper" — it is the workflow
generation gap: the next cohort of automation engineers are software
engineers, and we have not found anyone building the platform *they* would
choose (§17).

### 1.3 "Why not Embassy/RTIC — Rust already does real-time?"

Different layer, honestly stated (this also goes in public docs, because Rust
developers will ask): Embassy and RTIC are **MCU firmware frameworks** —
superb for building a *device*. truST is a **machine-controller product** —
scan semantics, fieldbus, HMI, forcing, commissioning, deployment, operators.
You could build a truST-like product *on top of* embedded Rust in a few
years; that is precisely the plumbing table in §1.1. The two compose rather
than compete: a truST controller talks EtherCAT/Modbus to devices whose
firmware might well be Embassy.

### 1.4 What only truST-with-Rust offers (the differentiators, argued)

1. **Scan-native memory-safe logic.** Rust POUs run *inside* the scan with
   copy-in/copy-out cycle coherence, per-invocation deadlines, fault states
   visible to ST/HMI/tooling, and a sandboxed (WASM) form for untrusted logic
   (§7–§8). Vendors offer TCB-C++; DIY offers nothing.
2. **Cycle sequences (§6.3).** IEC's answer to sequential logic is SFC or
   hand-built state machines over `CASE`. truST binds a **restricted,
   build-validated subset of Rust's await syntax** to the scan, then lowers it
   to an SDK-owned state machine: `await` a condition, a timeout, a motion
   completion — each wait point yields to the next cycle; anything else
   (foreign futures, async blocks, executor APIs, unrecognized macros) is
   rejected by the `#[trust_sequence]` validator, with runtime deadlines as
   the backstop. General, unrestricted async lives in services. Linear,
   reviewable, testable sequence logic with no runtime executor (polled once
   per cycle, explicit state allocated/accounted at init, deterministic). This deletes the most
   error-prone genre of PLC code; we have not found it — IEC or Rust-DIY —
   anywhere else (§17).
3. **The machine in CI (§13).** The runtime's own `TestHarness` already
   drives real `execute_cycle()` calls in-process with virtual time
   (`host/harness/harness.rs`). v2 productizes it as `trust-sim`: your
   pick-and-place logic runs 10,000 simulated cycles in milliseconds inside
   `cargo test`, on every PR, with property-based input generation and
   recorded-trace replay. PLC vendors sell test add-ons; this is `cargo test`.
4. **Timing admission (§9).** The build simulates the real scheduler's base
   frames across the task hyperperiod from budget×call-count evidence and
   refuses task sets whose worst frame cannot fit — with an itemized report
   naming the worst frame, its coincident tasks, and the biggest
   contributors. Evidence tiers (declared → measured-on-target → wasm-fuel
   → static) keep the claim honest: this is admission control of the actual
   non-preemptive scheduler, not a WCET fantasy and not utilization math.
5. **Crate admission (§10).** The promise "use any crate" is a trap without
   classification. truST classifies the *reachable-from-`cycle()`* dependency
   graph — curated knowledge base + static scans + **measured** allocation
   and syscall tripwires in the timing harness — and production profiles
   reject `service_only`/`unknown` crates from scan paths with a diagnostic
   that names the offending call path.
6. **Record/replay (§12).** Deterministic modules (time/randomness injected,
   no ambient authority) + cycle-coherent snapshots make traces replayable.
   Commissioning traces become CI regression baselines; a 3 a.m. fault comes
   with its own black-box recording; `trust explain-overrun` names the module
   and the cycle. This turns the platform's honest-status culture into a
   debugging superpower.

And one soft differentiator worth stating plainly: **AI-era leverage.** Every
serious coding assistant knows Rust deeply and ST barely; a Rust-first PLC
project gets working completion, refactoring, and review assistance for free.
For the target developer this may be the single most persuasive line in the
pitch — and it costs the architecture nothing.

### 1.5 What we do not claim

- Not a SIL-rated safety PLC (N1). Safety interlocks live in safety hardware.
- Native Rust in-process can crash the runtime; containment is per-form and
  honestly labeled (§8.5, D8). Anyone claiming otherwise for in-process C++
  or Rust is lying; we say so in docs.
- Linux-class real-time (PREEMPT_RT, mlock, affinity — `host/linux_rt.rs`),
  not MCU-class nanosecond determinism. Task granularity is bounded by the
  resource base scan (§3.2).
- The Rust ecosystem is *admitted*, not blessed: §10 is an evidence system,
  not a proof system, and its report says which.
- Not a "pure Rust PLC" — and we do not use that phrase. The product is
  **Rust-first PLC development with generated IEC compatibility**: authored
  in Rust + `trust.toml`, executed through truST's PLC runtime model,
  audited through generated ST/IEC artifacts. "Pure(ly) Rust" would imply
  the IEC/ST machinery is gone; it is underneath by design, and that is a
  feature (auditability, brownfield interop), not residue. This phrasing
  rule is normative for technical docs and product copy.

---

## 2. Goals and non-goals

### 2.1 Goals

- **G1 — Rust-first projects.** A developer can create, test, simulate,
  build, deploy, and operate a complete controller project whose sources are
  Rust + `trust.toml`. Generated ST/IEC artifacts are inspectable and
  committed, never hand-maintained. *(v2)*
- **G2 — First-class Rust POUs.** Rust FUNCTIONs/FBs/PROGRAMs with the same
  lifecycle behavior ST enjoys: deterministic cyclic execution, retain,
  addressed I/O, live values, diagnostics; indistinguishable at the ST call
  site. *(v1 G1)*
- **G3 — Full Rust inside ordinary modules.** For `cycle()`/FB/FUNCTION
  bodies, there is no Rust syntax subset: traits, generics, iterators, crates.
  The restriction surface is the PLC *boundary* (ABI types) and the *execution
  contract* (no blocking/ambient authority in scan lanes). The one explicit
  exception is opt-in cycle sequences (§6.3): `#[trust_sequence]` is a
  restricted sequence language using Rust's await syntax, by design. *(v2,
  sharpened from v1)*
- **G4 — Deterministic cyclic execution with honest temporal protection.**
  Enforced deadlines and watchdogs per form (detect for native, preempt for
  wasm); admission before trust. *(v1 G2 + v2 §9)*
- **G5 — Safety by architecture.** Every failure mode has a specified
  detection and reaction; no fabricated green. *(v1 G3)*
- **G6 — Evidence-gated dependencies.** Crate use in scan lanes is
  classified, reported, and policy-gated. *(v2)*
- **G7 — The machine is testable and replayable.** In-process simulation in
  `cargo test`; recorded cycle traces replay in tests and CI. *(v2)*
- **G8 — One product, uniform tooling.** Modules and Rust projects are
  visible in the VS Code extension, LSP, Devices & Connections, live values, web
  IDE; one CLI workflow fronts it all. *(v1 G4, widened)*
- **G9 — Production-grade from day one.** Versioned ABI, signed packages,
  conformance + failure-injection tests, documented TCB. *(v1 G5)*
- **G10 — Portable development.** Dev on x86_64/aarch64, cross-compile,
  sandbox form target-independent; `cargo test` simulation runs on the dev
  host. *(v1 G6)*

### 2.2 Non-goals

- **N1 — Functional-safety rating.** "Safety" herein = system robustness,
  not IEC 61508.
- **N2 — Killing ST.** ST remains fully supported, both as an authoring
  surface (brownfield door) and as the generated compatibility layer
  (greenfield door). Mixed projects are a feature. What v2 removes is only
  ST's *monopoly* on composition.
- **N3 — General async/threads/blocking inside scan lanes.** Scan-lane
  modules are step functions. Cycle sequences are still step functions —
  §6.3 is a restricted, build-validated language using await syntax, not a
  runtime executor; **normal Rust async is a service-tier capability**.
- **N4 — Compiling Rust to STBC.** `rustc` compiles Rust; the VM runs ST
  bytecode. Generated ST exists for interface/configuration compatibility,
  never as a Rust lowering target.
- **N5 — Synchronous service calls from scan lanes.** Structurally
  impossible, not merely discouraged (P5, RS-51).
- **N6 — Marketplace/catalog distribution.** Packaging and signing are
  specified; a public module registry is a separate product effort.

### 2.3 Requirements shorthand

Requirement IDs **RS-xx** continue from v1 (RS-01…RS-59 carried; v2 adds
RS-60+). MUST/SHOULD/MAY per RFC 2119. The v1 requirement text remains
normative where this document carries it forward by reference.

### 2.4 Claim vocabulary (normative)

Words about guarantees are load-bearing in this feature area — users will
read "admission" as stronger than intended unless every document uses the
same graded vocabulary. These definitions are **normative across the entire
document set, diagnostics, and user-facing docs**:

| Word | Meaning | May be claimed when |
|------|---------|---------------------|
| **Proven** | Structural property enforced by the type system, the ABI, or a runtime guard | The mechanism makes violation impossible or trapped (e.g. copy-in/out ⇒ no torn reads; closure-scoped exchange access ⇒ no reference across a wait point; wasm memory cap). Note: sequence no-blocking is **not** in this class — it is *rejected by validation* with a runtime backstop (RS-76), and is worded that way |
| **Validated** | Rejected/accepted by a conservative build-time analysis of source (e.g. the `#[trust_sequence]` validator, crate static scans) | The analysis ran and is conservative where it cannot see (unrecognized macros rejected, dynamic dispatch reported unknown); always paired with its runtime backstop when one exists |
| **Measured** | Observed on a **named target** under a **named corpus and configuration**, with the report digest-bound to artifact + target | A `.trusttime`/tripwire/syscall-audit run produced it (RS-91/92) |
| **Classified** | The outcome of layered curated/static/artifact/measured evidence, with the evidence class recorded per finding | Crate admission verdicts (§10) — never stated as proven |
| **Admitted** | Accepted by build/deploy policy for a **specific profile and target** on the strength of the above | Timing (§9) and crate (§10) gates passed; says nothing beyond the evidence grade behind it |
| **Certified** | **Reserved.** Used only for an external certification process (e.g. IEC 61508) | Never, in this spec's scope (N1). The §9.2 evidence grade formerly named `builtin_certified` is **`builtin_maintained`**, respecting this reservation |
| **Declared** | Author-asserted, unverified | Development profile only; always labeled |

Rules: a stronger word MUST never summarize a weaker one ("admitted under
measured evidence" is correct; "proven timing" is not). Every user-visible
verdict carries its evidence class (RS-94). Diagnostics, docs, and marketing
inherit this table verbatim.

---

## 3. Current state of truST (grounding survey)

Everything in this section exists today; paths under
`crates/trust-runtime/src/` unless noted. Two independent code surveys
re-verified this section for v2; **[v2 correction]** marks places where v1 (or
team folklore) was wrong or imprecise. This is the substrate §5–§13 plug
into — every integration point named here is referenced again in §19.

### 3.1 Project form and boot

- **A runnable project ("bundle") is a directory**: `runtime.toml` +
  `program.stbc` + `io.toml` (or system `/etc/trust/io.toml`), optional
  `simulation.toml`/`ads.toml`/`opcua_client.toml`
  (`RuntimeBundle::load`, `config/loaders.rs:2-67`).
- **[v2 correction] `run`/`play` always recompile ST from `<project>/src/`**
  (`bin/trust-runtime/run/runtime/load.rs:1-76` → `resolve_sources_root`,
  `host/bundle_builder/build.rs:78-100`); the on-disk `program.stbc` is then
  re-applied over the fresh runtime but only replaces `vm_module` + tasks +
  image sizing (`runtime/bytecode.rs:74-94`). A bytecode-only bundle without
  sources is **not runnable today**, and source⇄stbc drift is possible. The
  Rust-first pipeline (§5) inherits this happily — it always generates
  fresh ST sources — but the deploy story must state it (§14, §20 Q13).
- **CONFIGURATION/TASK/PROGRAM/VAR_CONFIG are ST-language constructs** lowered
  by the harness compiler (`host/harness/compiler/config/entry.rs:1-77`,
  `tasks_programs.rs:1-60`, `globals_access.rs:110-143`), applied via
  `attach_programs_to_tasks`/`register_task`
  (`host/harness/mod.rs:271-355`). **[v2 correction]** A `RESOURCE` block's
  own name/`ON` clause is parsed and *discarded* — there is no multi-resource
  runtime model; one implicit resource exists (`entry.rs:40-64`).
- `trust-runtime registry` is a **bundle package registry**
  (init/publish/download/verify/list, SHA-256 manifests,
  `host/registry/mod.rs:1-35`) — a distribution mechanism, unrelated to POU
  registration.

### 3.2 Execution core

- **One OS thread per resource; tasks evaluated inside that thread's scan
  loop** (`run_resource_loop_core`, `scheduler/runner_loop.rs:60-229`), ticking
  at `runtime.toml` `cycle_interval` (default 10 ms,
  `run/runtime/entry.rs:131-134`), with `time_scale` support and
  pause/resume/stop commands. Task granularity is bounded by this base scan.
- **Tasks**: `TaskConfig { interval, single, priority, programs,
  fb_instances }` (`trust-runtime-core/src/task.rs:8-23`). Periodic when
  `elapsed >= interval`; **event tasks trigger on the rising edge of a BOOL
  variable** (`single`, `task.rs:58-94`). Ready tasks sort by
  (priority asc, due-time, index) (`trust-runtime-core/src/cycle.rs:15-26`);
  programs not bound to any task run every scan ("background",
  `runtime/cycle.rs:276-300`). **Footgun**: a TASK with neither `INTERVAL` nor
  `SINGLE` never becomes ready — silently. §5's generated configuration makes
  that unrepresentable.
- **Cycle order** (`Runtime::execute_cycle`, `runtime/cycle.rs:17-88`):
  queued debug writes → read inputs (drivers → image → storage; forces
  re-applied; ADS/OPC-UA-client subsystems `apply_inputs`) → ready tasks in
  priority order → background programs → retain mark-dirty + interval-gated
  deduped save → write outputs (storage → image; forces re-applied so they
  win; ADS/OPC-UA `capture_outputs`; **output-commit watchdog deadline**
  checked around driver writes; Open-OT telemetry publish) → metrics, events,
  cycle counter.
- **Overrun/watchdog/fault, today**: task overruns are *telemetry only*
  (`RuntimeEvent::TaskOverrun` + per-task counter, `host/metrics.rs:130-133`);
  the resource-level watchdog (`WatchdogPolicy{enabled, timeout, action}`,
  `trust-runtime-core/src/watchdog.rs:83-91`) enforces the output-commit
  deadline + post-cycle elapsed check with `Halt|SafeHalt|Restart`; any
  cycle error funnels `record_fault → apply_fault`
  (`runtime/core/lifecycle.rs:203-234`).
- **[v2 correction] Safe-state honesty**: `IoSafeState` is an *explicit*
  address→value list from `io.toml` (`io/addressing.rs:152-168`), applied
  through every driver on fault **only when the policy is `SafeHalt`** —
  plain `Halt` freezes outputs, and a **graceful stop applies no safe state
  at all** (stop branch saves retain only, `runner_loop.rs:80-84`). The
  scaffolded template defaults are non-protective (`watchdog enabled=false,
  action="halt"`, `fault policy="halt"`, retain `mode="none"` —
  `host/bundle_template.rs:159`). RS-68 mandates protective defaults for
  Rust-first templates; the stop-gap is §20 Q12.
- **VM budget**: backward-jump budget, default 1 M, shared across the whole
  nested call tree of one top-level execution
  (`runtime/vm/dispatch.rs:236-237,299-328,635-641`); plus a **dormant
  wall-clock deadline API** — `Runtime::set_execution_deadline` checked every
  32 instructions (`dispatch.rs:36,271-275,650-652`;
  `runtime/core/lifecycle.rs:148-155`) — public, tested, and currently
  **unwired in production**. §9.6 wires it. Call-depth guard at 1024
  (`trust-runtime-core/src/vm/frames.rs:9-19`).
- **Panics: no `catch_unwind`, no panic hook, anywhere** in
  `trust-runtime`/`trust-runtime-core` (grep-verified). A panic in a cycle
  kills the scheduler thread **silently** — no fault event, no safe state, no
  retain flush; the main thread notices at `join()`
  (`run/runtime/entry.rs:556-558`) and the process exits. Module hosting
  (§7) introduces the containment boundary that closes this — F1.
- **Both runtime crates are `#![forbid(unsafe_code)]`** at crate level
  (`trust-runtime/src/lib.rs:3`, `trust-runtime-core/src/lib.rs:12`). The
  module host's FFI therefore lives in **new crates** with registered unsafe
  sites per the repo's unsafe-register policy — the forbid stays where it is.
- **Real-time posture**: `[runtime.realtime]` → mlockall, affinity,
  PREEMPT_RT evidence checks, SCHED_FIFO verification (not setting), `strict`
  fail-fast (`host/linux_rt.rs:67-219`).

### 3.3 Native-code precedent

- **Builtin FBs are native Rust behind bodiless declarations** — the shape
  §7 formalizes. Closed enum `BuiltinFbKind` (Rs, Sr, RTrig, FTrig, Ctu, Ctd,
  Ctud, Tp, Ton, Tof, RobotP3MinimalArm; `stdlib/fbs/registry.rs:5-18`),
  dispatched by type-name match at the VM's `CALL_NATIVE` site
  (`runtime/vm/call.rs:205-247`, param bind at `call.rs:414-459`) and shared
  with the tree-walk eval path (`host/eval/calls.rs:292-294`). State is
  stringly-keyed `__ST_*` instance vars rehydrated per call
  (`stdlib/fbs/timers.rs:16-47`). **Not user-extensible** — adding a native
  FB today means editing trust-runtime. The SDK's typed exchange + opaque
  state (§7) is the deliberate upgrade; P1a migrates these builtins onto it.
- **`ROBOT_P3MINIMALARM` demonstrates declared-interface + native body end-to-end
  in production**: its FB declaration ships as user-project ST; the runtime
  intercepts purely by type name (`stdlib/fbs/robot.rs:8-36`).
- **Stdlib functions** are native Rust registered at construction
  (`stdlib/mod.rs:22`); no user registration API. VM opcode `CALL_NATIVE`
  (0x09) already *means* this builtin/stdlib dispatch — hence v1's D12 naming
  rule (**external POU** for user Rust) stands.
- **No dynamic loading exists**: wasmtime absent from the entire lockfile;
  `libloading` only as an unrelated transitive dep; the only `cdylib` is
  `trust-wasm-analysis` (IDE analysis wasm). Loader/handshake/host in §7 are
  new subsystems.
- **Simulation hooks** run arbitrary host Rust with `&mut Runtime` on the
  resource thread pre/post cycle (`host/simulation.rs:19-38,133-263` —
  rapier3d physics couplings, disturbances, seeded, time-scaled): an existing
  in-process extension seam and the basis of §13's sim story.

### 3.4 Values, process image, retain

- Storage: name-keyed `VariableStorage` (globals/retain `IndexMap`, instances
  `FxHashMap`; deep-clone as the snapshot primitive, `memory.rs:43-76`).
  Tagged `Value` enum covers all IEC scalars/strings/arrays/structs/enums;
  **no byte-stable layout exists outside the I/O image and retain codec** —
  precisely why §7 uses generated `repr(C)` exchange blocks.
- Process image: three flat byte buffers (%I/%Q/%M) + symbolic bindings
  copied each cycle + a hierarchical side-table for multi-segment fieldbus
  paths (`io/interface.rs:1-8,173-364,226-291`). `VAR_CONFIG` lowers to
  IoMap bytecode and binds instance vars straight to the image
  (`host/harness/config/config_inits.rs:1-102`).
- **Forcing** is two distinct mechanisms: one-shot queued writes vs.
  persistent forces re-applied twice per cycle so they beat both hardware
  and program writes (`host/debug/control/api/writes_forcing.rs:1-181`,
  `runtime/cycle.rs:90-121`).
- **Retain**: `STRN` v2 codec (tagged name→value, CRC32, tmp+rename+fsync
  atomicity, `retain/codec.rs`, `retain/store.rs:21-58`); interval-gated
  deduped saves; **typed migration on restore** — subrange re-checks, struct
  field add/drop reconciliation, enum canonicalization by variant name, with
  `RetainMigrationApplied`/`RetainOrphanDropped` events
  (`runtime/restart.rs:169-384`). `Reference`/`Instance` values are
  explicitly non-retainable (`codec.rs:261-265`). Module opaque-state retain
  (§6.4/RS-42) extends this codec with a module-blob section.

### 3.5 Control plane and comms

- **[v2 correction] The control protocol is a custom newline-delimited JSON
  envelope, not JSON-RPC**: `ControlRequest{id, type, params, auth,
  request_id}` / `ControlResponse{id, ok, result, error, error_code,
  audit_id}` (`control/types.rs:7-30`) over loopback-TCP or a `0600` Unix
  socket (`control.rs:117-143`, `control/transport.rs:12-86`).
- **RBAC**: `Viewer < Operator < Engineer < Admin`
  (`host/security/mod.rs:19-24`); exact `auth_token` ⇒ Admin, pairing tokens
  carry their role, tokenless Unix peer ⇒ Admin, tokenless TCP ⇒ Viewer
  (`control/auth.rs:27-71`); per-command table: reads at Viewer,
  pause/resume at Operator, writes/forces/`eval`/`comm.apply` at Engineer,
  `shutdown`/`bytecode.reload`/pairing at Admin (`control/policy.rs:31-105`).
- **Method surface** (dispatch chain `control/handlers/mod.rs:12-21`):
  `status/health/tasks.stats/events/faults/config.*/historian.*`;
  `pause/resume/step_*/breakpoints.*/debug.*`;
  `io.list/read/write/force/unforce`; `hmi.*` (schema/values/trends/alarms/
  write/ack); `eval/set/var.force/var.unforce/var.forced`; `ads.*`
  (discover/import_symbols(.apply)/routes/server.*); `comm.capabilities/
  schema/discover/browse_symbols/apply/test`; `fleet.topology`;
  `shutdown/restart/bytecode.reload/pair.*`. **[v2 correction]** The
  release verb is `unforce`; **there is no `subscribe`** — the plane is
  request/response; the HMI websocket is server-side poll+diff-push
  (`web/hmi_ws.rs:56-115`). Tier-3 subscriptions therefore start as
  SDK-side polling over snapshots (RS-48), unchanged from v1.
- Reads are **cycle-coherent by construction** (snapshot answered between
  cycles, `runner_loop.rs:302-308`); writes queue and apply at the next
  cycle boundary.
- **[v2 correction] OPC-UA is server *and* client** (browse/read/write,
  `host/opcua/client.rs:187-610`), both behind the `opcua-wire` feature;
  public docs currently frame server-only. The server's cold-start gap is a
  *silent empty node set* when it starts before the first snapshot
  (`unwrap_or_default()`, `host/opcua/wire.rs:33-36`) — the literal
  "runtime snapshot unavailable" string belongs to the HMI and ADS-server
  paths, not OPC-UA.
- **The ADS generated-declarations pipeline is shipping and is v2's codegen
  precedent**: `ads.discover` → `ads.import_symbols` (upload symbol table)
  → candidate selection → `.apply` → `generate_ads_interface()`
  (`host/ads/generate.rs:106-155`) writes `ads.toml` + a snapshot JSON +
  **`<project>/src/generated/ads_generated.st`** (plain `VAR_GLOBAL`s plus a
  sibling `_quality : ADS_QUALITY` enum per point), compiled by the ordinary
  source path and **bound by name** at runtime (`host/ads/client.rs:19-27`).
  No pragmas involved. Transport: `AdsTransport` trait
  (`host/ads/transport.rs:105-128`) over the birkenfeld `ads` 0.4.4 crate
  (client-only); the ADS *server* wire is hand-rolled in `trust-ads-server`.
- Modbus TCP and MQTT are **client-side only** (`io/modbus.rs`, `io/mqtt/*`
  on rumqttc); zenoh mesh does runtime-to-runtime pub/sub (`host/mesh/`);
  Open-OT publishes per-cycle records into a **shared-memory ring**
  (`runtime/openot_telemetry.rs`, `open_ot_shm::SharedRecordPublisher`) —
  the in-tree precedent for §11's data plane and §12's low-overhead trace
  publication.

### 3.6 Compiler pipeline, declared interfaces, IDE

- Pipeline: ST → `trust-syntax` (logos+rowan) → `trust-hir` (salsa) →
  harness lowering → bytecode encoder → `.stbc` (magic `STBC`,
  `trust-runtime-core/src/bytecode/format/header.rs:1-20`) → `VmModule`.
  `trust-runtime check` compiles without writing artifacts
  (`bin/trust-runtime/check.rs`); `build` writes `program.stbc`; the LSP
  reuses the same `CompileSession` in-memory for diagnostics
  (`crates/trust-lsp/src/handlers/diagnostics.rs:21-24`).
- **[v2 refinement] Pragmas are trivia at the syntax level**
  (`token_kind_variants_part_01.rs:17-21`), **but a working semantic pragma
  family already exists**: `{attribute 'key' := 'value', …}` parsed from CST
  trivia for Open-OT authoring, with its own diagnostics
  (`trust-hir/src/openot_authoring.rs:264-337,2154-2156`). The `{external …}`
  marker (§7.4) reuses this exact micro-syntax precedent — smaller compiler
  work than v1 assumed.
- `VAR_EXTERNAL` remains IEC global linkage only; no external-*body* concept
  exists anywhere (`trust-hir/src/symbols/defs.rs:136-157`).
- **Live values / debug**: the VS Code IO panel and `trust-lsp.debug.io.*`
  commands are extension-side; values flow over DAP (`crates/trust-debug`,
  stdio) with custom requests `stIoState/stIoWrite/stIoForce/stIoRelease`
  mapping to control-plane `io.write/io.force/io.unforce`
  (`trust-debug/src/adapter/`, `protocol.rs:161-169`). Launch mode embeds
  `trust-runtime` in-process with its own control socket; attach mode targets
  a remote runtime — Rust POU status/telemetry must surface through this
  existing pipeline (RS-57), no parallel one.
- **Online change, today, precisely**: `bytecode.reload` (Admin) →
  `apply_online_change_bytes` = swap bytecode + warm restart + retain reload
  (`runtime/online_change.rs:1-25`). Body changes to *existing* POUs only;
  **new POU names/globals are rejected** (`runtime/bytecode.rs:96-124` +
  boot-frozen catalogs); nothing in deploy/IDE currently calls it — deploy is
  filesystem + optional process restart (`bin/trust-runtime/deploy/`).
  v1's RS-21a language ("online change re-validates handshakes") stands as a
  *requirement on the module host*, but §8.4 now states the honest baseline.
- **TestHarness** (`host/harness/harness.rs:1-277`): `from_source(s)`,
  `set_input/get_output` (named), `set_direct_input/get_direct_output`
  (%I/%Q), `cycle() -> CycleResult`, `run_cycles/run_until[_max]`,
  **`advance_time`** (virtual clock), `restart`, `reload_source` preserving
  retain; runs the *production* `execute_cycle` synchronously with no
  threads/drivers. Already the vehicle for in-crate tests (e.g. retain
  migration tests, `runtime/restart.rs:678-921`). A JSON-line automation
  wrapper + standalone binary exist (`trust-harness`,
  `host/harness/protocol.rs:22-24`, `bin/trust-harness.rs`). **[v2
  correction]** there is no `cycle_dispatch_only()` anywhere; team notes
  claiming it conflated a control-message dispatch bench.
- **Timing/telemetry, today**: cycle stats + 512-sample p50/p95/p99 window
  (`host/metrics.rs:12-97,202-358`); per-task stats + overruns; a generic
  per-call profiling mechanism `record_call(kind, name, duration)` with
  ranked `top_contributors` **wired only for `kind="program"`**
  (`runtime/cycle.rs:236-248`) — per-FB/module granularity is a data
  structure away, not a redesign (§9.6, §12.1). `RuntimeEvent` stream
  (CycleStart/End, TaskStart/End, TaskOverrun, Fault, SafeStateFailed,
  Retain*) feeds a logger + last-200 ring (`host/debug/types.rs:206-266`,
  `run/runtime/entry.rs:93-109`). Historian: JSONL + Prometheus.
- **Libraries model**: `trust-lsp.toml [dependencies]` path/git entries with
  a lockfile (`trust-lsp/src/config/deps.rs:26-48`; no registry source kind);
  vendored `libraries/<name>/` convention (OSCAT, PLCopen Motion in-repo).
  Rust changes the equation: **FB libraries become ordinary cargo
  dependencies of the project's module crate** (§5.4).
- **CLI, today**: `trust-runtime` has 27 subcommands (run, play, ui, ctl,
  validate, check, build, hmi, plcopen, ads, comm, fleet, registry, setup,
  ide, wizard/init, commit, deploy, rollback, completions, bench,
  conformance, …; `bin/trust-runtime/cli/commands.rs:19-355`) — the raw
  material §15's `trust` umbrella fronts.

### 3.7 What this means for the design

1. **The seams are already right** (unchanged from v1, now with more
   receipts): tier 2 = one registry lookup in `execute_program_by_name`
   before the VM POU lookup; tier 1 = an EXTERNAL kind beside the builtin
   branch in `execute_native_call`; per-invocation timing = `record_call`
   with a new kind + the dormant deadline API; faults = existing
   `record_fault/apply_fault`; retain = codec section bump; declarations =
   the ADS codegen pattern; sim/testing = `TestHarness`; trace transport =
   the Open-OT shm precedent.
2. **The gaps are exactly the spec's new obligations**: no panic containment
   (F1 — module host introduces it); no dynamic loading (§7); no
   external-declaration semantics (one pragma family, with the Open-OT
   micro-syntax as precedent); no admission of any kind (§9, §10); no trace
   recording (§12); no public sim harness (§13); protective defaults absent
   from templates (RS-68); no `subscribe` (services poll first, RS-48).
3. **Constraints inherited**: single resource thread; task granularity
   bounded by base scan; `forbid(unsafe_code)` in runtime crates (module
   host = new crates with registered unsafe); `trust-runtime-core`
   portability/no_std discipline for the ABI crate; MSRV = workspace
   `rust-version = 1.95`; authoritative validation on trust-builder.

---

## 4. Architecture: one runtime model, two authoring surfaces

### 4.1 The tier model (carried from v1, unchanged)

Three execution lanes; tier = call site, not technology (v1 §3, D2):

| Tier key | Surfaces as | Called by | Timing contract |
|----------|------------|-----------|-----------------|
| `inline_rt` | `FUNCTION`, `FUNCTION_BLOCK` | ST or Rust PLC code, mid-cycle | inherits the calling task's budget; per-call deadline share |
| `cyclic_rt` | `PROGRAM` | task scheduler, every tick | own program slot; per-cycle deadline |
| `service` | external process | itself (tokio, threads, anything) | none — must tolerate and expose staleness |

Isolation is the orthogonal axis (v1 R2): any RT-tier module is deliverable
**native** (`cdylib`, full speed, inside the TCB) or **sandboxed** (WASM via
wasmtime: memory-capped, epoch/fuel-bounded, ~10 % slower); first-party
modules may compile in statically (**built-in** — what the builtin FBs
already are). Services are isolated by the process boundary.

Rules of thumb (user docs): *if it must finish this cycle, it's tier 1 or 2;
if it must survive the cycle not caring, it's tier 3. When in doubt, start in
tier 3 — promotion into the cycle is a deliberate act with a budget attached.*

### 4.2 The two doors, one compile path (new, D14)

```
   BROWNFIELD DOOR (ST-first)                GREENFIELD DOOR (Rust-first)
   ─────────────────────────                 ──────────────────────────
   hand-written .st sources                  src/*.rs  +  trust.toml
   + modules/<name>/ Rust modules                   │
   + generated declarations                          │ trust build / cargo trust build
             │                                       ▼
             │                    ┌─ src/generated/trust/*.st ──────────────┐
             │                    │   declarations.st   (from Rust exports) │
             │                    │   configuration.st  (from trust.toml)   │
             │                    │   devices_*.st      (from io import)    │
             │                    ├─ io.toml, runtime.toml (generated)      │
             │                    └─ <crate>.so / .wasm  (rustc)            │
             ▼                                       │
   ═══════════ ONE pipeline: trust-syntax → trust-hir → harness → ═══════════
                 .stbc  +  module binaries  +  digests  →  bundle
                                     │
                     ┌───────────────┼──────────────────┐
                     ▼               ▼                  ▼
               timing admission   crate admission   conformance     (§9, §10, §13)
                     └───────────────┴──────────────────┘
                                     ▼
                         deployable, signed trust bundle
```

- **RS-60** The Rust-first surface MUST compile through the existing ST
  pipeline via generated sources. There is no second configuration parser, no
  second scheduler binding path, no Rust-only runtime mode. (Consequences:
  the LSP, debugger, live values, HMI, and every existing tool work on
  Rust-first projects by construction; and the generated ST is a faithful,
  reviewable record of exactly what the runtime will do.)
- **RS-61** Generated artifacts MUST be deterministic (same inputs ⇒
  byte-identical outputs), carry a "generated — do not edit" header naming
  the generator and inputs, and be digest-chained to the Rust interface they
  came from (§7.4). Hand edits are detected as digest mismatches at build.
- **RS-62** Mixed projects MUST work in both directions of *authorship*:
  hand-written ST alongside generated ST (both doors feed one sources root),
  ST calling Rust POUs (tier 1), and Rust/ST programs interleaved on one task
  in declared order. In-cycle *calls* from Rust into ST bodies are deferred
  (D22) — cross-language composition happens at task order + bound variables,
  which the cycle-coherence rules already make race-free.

### 4.3 Design principles (v1 P2–P8 carried; P1 revised; P9–P10 new)

- **P1 (revised) — The compiled configuration is the composition root.**
  Task topology, program order, and I/O binding live in exactly one compiled
  artifact, whatever its source (hand ST or `trust.toml`). Rust code never
  self-schedules, self-binds addresses, or instantiates itself at runtime.
- **P2 — The scheduler owns time.** Modules are step functions; time reaches
  them only through the cycle context. No wall clocks, no sleeps. This is
  what makes §12 and §13 possible, so it is enforced, not advised.
- **P3 — Fail loud, land safe.** Defined state machine + configured output
  policy + surfaced diagnostic for every failure; no fabricated green.
- **P4 — Allocate at init, not per cycle.** Tripwired (RS-32), with arenas
  provided so obeying is ergonomic.
- **P5 — No ambient authority in RT tiers.** Capabilities come only from the
  cycle context. Native is policed (lints + §10 evidence), wasm is prevented,
  services are free.
- **P6 — Version skew is refused, not survived.** Handshake + digest chain.
- **P7 — One SDK, boring on the outside.** All unsafety inside
  `trust-plc`/`trust-module-abi`; user modules are 100 % safe Rust by
  default.
- **P8 — Observable by default.** Per-instance stats, states, and fault
  codes flow through the same metrics/live-values/Devices & Connections lenses as
  everything else.
- **P9 (new) — Admission before trust.** Anything entering the scan carries
  evidence: a timing budget with a grade (§9) and a dependency report (§10).
  Production profiles refuse to run on vibes.
- **P10 (new) — Determinism is a contract, not a vibe.** RS-43/44 determinism
  is conformance-tested, because record/replay (§12) and CI simulation (§13)
  — the product's flagship features — are only as real as it is.

---

## 5. The Rust-first project model

### 5.1 Project layout

`trust new rust-plc <name>` scaffolds a cargo-native project:

```
palletizer/
├── Cargo.toml               # ordinary cargo manifest; crate-type cdylib+rlib
├── trust.toml               # PLC topology: tasks, bindings, policies (App. C)
├── trust-lsp.toml           # IDE/ST roots for generated + hand-written ST
├── rust-toolchain.toml      # pinned toolchain (RS-55)
├── .vscode/
│   ├── launch.json          # kind-aware Rust PLC sim/debug launch
│   └── extensions.json      # rust-analyzer recommendation
├── src/
│   ├── lib.rs               # module root: pub mod safety; pub mod motion; …
│   ├── safety.rs            # #[trust_program] SafetyGate
│   ├── motion.rs            # #[trust_program] AxisController
│   ├── palletizer.rs        # #[trust_program] Palletizer (cycle sequence)
│   ├── supervision.rs       # RecipeSupervisor (scaffolded per [services.*], RS-98)
│   └── generated/           # GENERATED — committed, never hand-edited
│       └── trust/
│           ├── declarations.st    # from Rust exports (digest-pinned)
│           ├── configuration.st   # from trust.toml [tasks]/[io]
│           ├── devices_drive1.st  # from `trust io import` (optional)
│           └── generated_manifest.json
├── services/
│   └── recipe/              # tier-3 binary crate (workspace member)
│       ├── Cargo.toml
│       └── src/main.rs
├── tests/
│   ├── machine.rs           # trust-sim: full-machine tests (§13.2)
│   └── replay_commissioning.rs
├── traces/
│   └── commissioning.trusttrace    # recorded evidence (§12)
├── io.toml                  # GENERATED from trust.toml [io]/[outputs]
├── runtime.toml             # GENERATED from trust.toml [runtime]/[timing]
└── target/trust/            # trust bundle staging + .trusttime/.trustcrate
```

Committed generated artifacts (`src/generated/trust/*`, `runtime.toml`,
`io.toml`) are target/profile invariant. Target-, profile-, or
measurement-specific artifacts live under `target/trust/` and do not trip
F22 in a clean checkout on another supported dev host.

- **RS-63** The project's PLC logic compiles as **one module crate**
  (`cdylib` for the runtime + `rlib` for tests/sim). FB libraries are
  ordinary cargo dependencies re-exported through it (§5.4). Additional
  prebuilt modules may still be dropped under `modules/` exactly as in the
  brownfield door (v1 §5.2).
- **RS-64** `src/generated/**` and the generated `io.toml`/`runtime.toml`
  are build outputs *and* committed sources: deterministic (RS-61),
  reviewable in diffs, and consumed by the ordinary compile path (the ADS
  import pipeline already established this convention —
  `src/generated/ads_generated.st`, §3.5).
- The daily loop:

```bash
cargo test              # unit + machine-sim + replay tests, off-target
trust check             # codegen + ST compile + admission dry-run, no artifacts
trust sim               # headless sim (drivers=simulated, physics optional)
trust build             # bundle: stbc + module + digests + admission records
trust deploy cell-01    # signed deploy; rollback retained
```

### 5.2 `trust.toml` — the one manifest

Full schema in Appendix C. The shape:

```toml
[project]
name    = "palletizer"
profile = "production"          # development | production | hard-rt (§9.4)

[runtime]
cycle_interval = "1ms"          # optional; default = min task period (RS-66)
watchdog       = { timeout = "5ms", action = "safe-halt" }   # protective defaults (RS-68)
fault          = { policy = "safe-halt" }
retain         = { mode = "file", save_interval = "500ms" }

[tasks.fast]
period   = "1ms"
priority = 1                    # lower runs first (matches runtime semantics)
programs = ["safety::SafetyGate", "motion::AxisController"]

[tasks.main]
period   = "10ms"
priority = 5
programs = ["palletizer::Palletizer", "supervision::RecipeSupervisor"]
# event tasks: `event = "<bool var>"` instead of `period` (task.rs `single`; §20 Q1)

[io]                            # instance-field ⇄ address bindings → VAR_CONFIG
"safety_gate.e_stop_ok"                = "%IX0.0"
"axis_controller.actual_position_mm"   = "%IL100"
"axis_controller.setpoint_position_mm" = "%QL100"
"axis_controller.drive_enable"         = "%QX0.0"

[io.drivers.ethercat]           # → generated io.toml driver entries
# … driver-specific config, schema-checked against comm.schema

[outputs.safe_state]            # → io.toml safe_state list (RS-68)
"%QX0.0" = false
"%QL100" = 0.0

[timing]                        # §9
require_evidence = "measured"
jitter_margin    = "150us"

[crates]                        # §10
rt.deny_unknown      = true
rt.deny_service_only = true

[services.recipe]               # §11
binary    = "services/recipe"
heartbeat = "svc.recipe_hb"
timeout   = "500ms"
```

- **RS-65** `trust.toml` declares **topology and policy only**. POU
  interfaces are never restated here — they come from macro-extracted
  interface JSON (§7.4). A policy entry referencing an unknown export or
  field is a build error naming both sides. (This kills the drift class the
  v1 manifest review flagged.)
- **RS-66** Generation rules, normative:
  - `[tasks.*]` → `CONFIGURATION` with `TASK` (INTERVAL/SINGLE/PRIORITY) and
    `PROGRAM <instance> WITH <task> : <TYPE>;` in **array order** (RS-22).
    A task entry MUST have `period` xor `event` — the "never-ready task"
    footgun (§3.2) is unrepresentable.
  - Program **instance names** default to the snake_case type name
    (`motion::AxisController` → `axis_controller`); an entry may override
    via `{ instance = "…", type = "…" }`. Duplicate instance names are a
    build error. Instance names are the dotted-path roots used by `[io]`,
    live values, and the control plane.
  - `[io]` → `VAR_CONFIG` entries with types taken from the Rust interface
    (mismatch = build error with both declarations quoted).
  - `[outputs.safe_state]` → `io.toml` `safe_state`; `[runtime]` →
    `runtime.toml`. `cycle_interval` defaults to the minimum task period and
    MUST divide every task period (else a build error explains task
    granularity, §3.2).
- **RS-67** `trust check` MUST validate the full chain — codegen, ST
  compile, digest match, admission dry-run — without writing artifacts
  (fronting the existing `check` path, §3.6).
- **RS-68** Rust-first templates ship **protective defaults**: watchdog
  enabled with `safe-halt`, fault policy `safe-halt`, retain enabled, and a
  build **warning** for any `%Q` output bound in `[io]` but absent from
  `[outputs.safe_state]`. (Today's scaffold defaults are non-protective —
  §3.2; software engineers should not need tribal knowledge to fail safe.)

### 5.3 What gets generated (and what it looks like)

From the manifest above and the Rust exports, `trust build` emits
`src/generated/trust/configuration.st`:

```iecst
(* GENERATED by trust build from trust.toml + Rust interface digest b3:9f41…
   Do not edit. Regenerate with `trust build`. *)
CONFIGURATION Palletizer
    TASK fast (INTERVAL := T#1ms, PRIORITY := 1);
    TASK main (INTERVAL := T#10ms, PRIORITY := 5);

    PROGRAM safety_gate       WITH fast : SAFETY_GATE;
    PROGRAM axis_controller   WITH fast : AXIS_CONTROLLER;
    PROGRAM palletizer        WITH main : PALLETIZER;
    PROGRAM recipe_supervisor WITH main : RECIPE_SUPERVISOR;

    VAR_CONFIG
        safety_gate.E_STOP_OK                AT %IX0.0 : BOOL;
        axis_controller.ACTUAL_POSITION_MM   AT %IL100 : LREAL;
        axis_controller.SETPOINT_POSITION_MM AT %QL100 : LREAL;
        axis_controller.DRIVE_ENABLE         AT %QX0.0 : BOOL;
    END_VAR
END_CONFIGURATION
```

plus `declarations.st` (bodiless external POUs with the `{external …}`
pragma and status outputs — v1 §5.6/RS-15..17 unchanged), plus `io.toml` /
`runtime.toml`. An ST engineer can read every line of what the Rust project
does to the machine; an auditor can diff it release to release. That is the
compatibility promise: **Rust-first, never Rust-opaque.**

### 5.4 Libraries: FBs as crates

- **RS-69** A reusable FB/function library is an ordinary Rust crate
  depending on `trust-plc`, exporting `#[trust_fb]`/`#[trust_function]`
  items. Projects consume it with `cargo add`; the items compile **into the
  project's module** and appear in its generated declarations like local
  exports (with their originating crate+version recorded in interface JSON).
  Distribution = crates.io / git / path — cargo's, not ours.
- Binary module *packages* (vendor-closed source) remain the v1 packaging
  story (v1 §5.2, App. C) with D23's compatibility roadmap. The existing ST
  library conventions (`libraries/<name>/`, `trust-lsp.toml [dependencies]`)
  are untouched — brownfield door.
- This makes the OSCAT-shaped ecosystem question concrete: standard-library
  FB packs become versioned crates with types, tests, and docs.rs pages,
  consumable from *both* doors (ST projects vendor the module binary +
  declarations; Rust projects `cargo add`).

---

## 6. The programming model (SDK)

The user-facing crate is **`trust-plc`** (D21); `trust-module-abi` remains
the internal `no_std` ABI crate (App. B). Everything in v1 §6 (lifecycle
state machine, cycle context capabilities, temporal/memory protection, fault
and output policy, data-exchange rules, determinism, wasm specifics — RS-18
… RS-47) is carried unchanged; this section shows the authoring surface and
adds the v2 features.

### 6.1 Programs, function blocks, functions

```rust
use trust_plc::prelude::*;

/// Axis position controller on the 1 ms task.
#[trust_program(name = "AXIS_CONTROLLER")]
#[budget("150us")]                      // admission input (§9); overridable down in trust.toml
pub struct AxisController {
    #[input]  pub actual_position_mm: f64,
    #[input]  pub target_position_mm: f64,
    #[input]  pub enable: bool,

    #[output] #[safe_default(0.0)]   pub setpoint_position_mm: f64,
    #[output] #[safe_default(false)] pub drive_enable: bool,
    #[output] pub in_position: bool,

    #[retain] pub odometer_mm: f64,     // interface retain (RS-41)

    profile: TrapezoidProfile,          // private; allocated in init (P4)
}

impl PlcProgram for AxisController {
    fn init(&mut self, ctx: &mut InitCtx) -> PlcResult<()> {
        self.profile = TrapezoidProfile::for_period(ctx.period());
        Ok(())
    }

    fn cycle(&mut self, io: &mut Exchange<Self>, ctx: &mut CycleCtx) -> PlcResult<()> {
        if !io.enable {
            io.drive_enable = false;
            io.in_position = false;
            return Ok(());
        }
        let step = self.profile.step(io.actual_position_mm,
                                     io.target_position_mm, ctx.dt())?;
        io.setpoint_position_mm = step.position;
        io.drive_enable = true;
        io.in_position = step.done;
        io.odometer_mm += step.travelled.abs();
        Ok(())
    }
}
```

FBs (`#[trust_fb]`, trait `PlcFb`) and functions (`#[trust_function]`,
instance-less, RS-40) follow v1 §9.1 verbatim with the renamed traits.
Notes:

- **RS-70** `#[budget]` on the item declares the *default* cycle deadline
  and admission budget; `trust.toml`/`module.toml` may override **downward
  only**. Exactly one effective value feeds both enforcement (RS-26) and
  admission (§9).
- **RS-71** `#[safe_default(v)]` sets the declared ST initial value *and*
  the `output_policy = "default"` value (RS-35) — one annotation, one truth.
- **RS-72** `#[unit("mm")]`, `#[doc]` comments, and value ranges
  (`#[range(0.0..=500.0)]`) flow into generated declarations as pragma
  metadata (Open-OT `{attribute}` micro-syntax, §3.6) for HMI/live-values
  display and — for `#[range]` — copy-out validation (out-of-range output =
  protocol-violation fault, extending RS-34).

### 6.2 Typed I/O: addresses, and devices as types

Level 1 — **manifest addresses** (§5.2 `[io]`): field-to-address bindings
generating `VAR_CONFIG`; types checked against the Rust interface (RS-66).

Level 2 — **device import** (D19). The shipped ADS pipeline
(browse → select → generate, §3.5) is extended to emit *Rust* alongside ST:

```bash
trust io import ads://192.168.10.5 --select "MAIN.Axis1.*" --name drive1
```

generates (a) the existing ST globals + quality enums
(`src/generated/trust/devices_drive1.st`), and (b) a typed binding:

```rust
// src/generated/trust/devices_drive1.rs  — GENERATED, digest-pinned
#[trust_device(connection = "drive1")]
pub struct Drive1 {
    #[point("MAIN.Axis1.ActPos")]  pub act_pos: f64,
    #[point("MAIN.Axis1.SetPos")]  pub set_pos: f64,
    #[point("MAIN.Axis1.Enable")]  pub enable: bool,
}
```

- **RS-73** A `#[trust_device]` struct is *sugar over bound variables*: its
  fields marshal through the same generated globals/bindings every other
  client sees (name-bound like `ads.toml` today), each carrying the imported
  quality state (`.act_pos_quality()`). No parallel I/O path exists — a
  device struct is a typed *view*, not a driver.
- **RS-74** Import provenance (device identity, symbol-table version,
  import digest) is recorded in the generated files; `trust check` warns
  when the live device's symbol version no longer matches (the ADS symbol
  version handshake exists — `AdsTransport::symbol_version`, §3.5).
- EtherCAT: the same command shape consumes ESI/scan data to type the
  process-image segments the `ethercat` driver maps (`io/registry.rs`);
  addresses stay the truth underneath (Level 1), the struct is the view.

### 6.3 Cycle sequences — a restricted await-syntax sequence language (D18)

The flagship language feature. Sequential automation logic — the code PLC
programmers write as SFCs or `CASE`-machine boilerplate — written as
straight-line code in Rust's `await` syntax. **This is deliberately not
"general Rust async on a PLC"**: a `#[trust_sequence]` body is a restricted,
macro-**validated and lowered** sequence language that borrows Rust's syntax
but emits an SDK-owned state machine, not a stored Rust `Future`; normal,
unrestricted async belongs in
services (§11), never in scan lanes. (Why validation and not a trait: in
plain Rust, `.await` accepts anything implementing `IntoFuture`, and std
blanket-implements `IntoFuture` for every `Future` — no sealed SDK trait can
restrict what an ordinary `async fn` awaits. Only build-time validation of
the body can, so that is what the design claims. Why lowering and not a stored
future: the sequence must access the current cycle's `Exchange` and `CycleCtx`
on every poll; storing a future created at `init()` would store stale borrows
or require self-referential lifetime tricks. The generated state machine owns
only explicit sequence state; the process image is borrowed only for the
current `poll` call. The example below is macro input: `#[trust_sequence]`
consumes the await-shaped body and emits `TrustSequence` code before ordinary
Rust async legality/type checking would apply.)

```rust
#[trust_program(name = "PALLETIZER")]
#[budget("80us")]
pub struct Palletizer {
    #[input]  pub start: bool,
    #[input]  pub safety_ok: bool,
    #[input]  pub axis_in_position: bool,
    #[input]  pub gripper_closed: bool,

    #[output] #[safe_default(false)] pub gripper_close_cmd: bool,
    #[output] pub target_position_mm: f64,
    #[output] pub move_cmd: bool,
    #[output] pub pallet_complete: bool,

    #[retain] pub boxes_placed: u32,

    seq: PalletizerRun,                  // generated state machine; polled once per cycle
}

impl PlcProgram for Palletizer {
    fn init(&mut self, ctx: &mut InitCtx) -> PlcResult<()> {
        self.seq = PalletizerRun::new(ctx);         // explicit state allocated/accounted at init
        Ok(())
    }
    fn cycle(&mut self, io: &mut Exchange<Self>, ctx: &mut CycleCtx) -> PlcResult<()> {
        self.seq.poll(io, ctx)                      // drives `run` one step
    }
}

impl Palletizer {
    /// The whole machine sequence, linearly. Every wait point = "resume in
    /// a later cycle when the condition holds". The #[trust_sequence]
    /// validator admits ONLY SDK wait points; any other `.await`, async
    /// block/closure, executor API, or unrecognized macro is a build
    /// error (F23).
    #[trust_sequence]
    fn run(s: &mut Seq<Self>) -> PlcResult<()> {
        loop {
            s.until(|io| io.start && io.safety_ok).await?;

            for slot in pallet_grid(s.retain_read(|r| r.boxes_placed)) {
                // command a move, wait for completion with a deadline
                s.write(|io| {
                    io.target_position_mm = slot.pick_mm;
                    io.move_cmd = true;
                });
                s.until(|io| io.axis_in_position)
                    .deadline(iec!(T#3s), FaultCode::MOVE_TIMEOUT).await?;

                s.write(|io| io.gripper_close_cmd = true);
                s.until(|io| io.gripper_closed)
                    .deadline(iec!(T#500ms), FaultCode::GRIP_TIMEOUT).await?;

                s.write(|io| io.target_position_mm = slot.place_mm);
                s.until(|io| io.axis_in_position)
                    .deadline(iec!(T#3s), FaultCode::MOVE_TIMEOUT).await?;

                s.write(|io| io.gripper_close_cmd = false);
                s.wait(iec!(T#100ms)).await?;       // release settle, cycle-time based

                s.retain_write(|r| r.boxes_placed += 1);
            }

            s.write(|io| io.pallet_complete = true);
            s.until(|io| !io.start).await?;         // handshake reset
            s.write(|io| io.pallet_complete = false);
            s.retain_write(|r| r.boxes_placed = 0);
        }
    }
}
```

Compare with the equivalent ST: a `state : INT` enum, a `CASE` with a dozen
branches, hand-managed timers, and the ever-present forgotten-transition
bug class. Here the state machine is generated by the `#[trust_sequence]`
lowerer from a body the validator has restricted to exactly the wait points
the scan can honor.

Normative semantics (these make it a PLC feature, not an executor):

- **RS-75 (lowering, creation & memory).** The sequence body is
  `#[trust_sequence]` source syntax, but the macro MUST lower it to a concrete
  SDK state machine implementing:

```rust
pub trait TrustSequence<P> {
    fn poll(
        &mut self,
        io: &mut Exchange<P>,
        ctx: &mut CycleCtx,
    ) -> PlcResult<SequencePoll>;
}
```

  The generated state object is constructed at `init()` and stores only
  explicit sequence state: current state/step, timers/deadline counters,
  child-sequence state, and owned values that cross wait points. It MUST NOT
  store `Exchange`, `CycleCtx`, process-image references, retain references,
  or a Rust `Future` that captures them. No allocation on `poll` (the tripwire
  RS-32 applies). There is no waker/executor machinery: the runtime calls
  `TrustSequence::poll` exactly **once per `cycle()` call**, always on the
  owning task thread.
- **RS-76 (the restricted language).** Cycle sequences are **not general
  Rust async**. The `#[trust_sequence]` validator/lowerer accepts, as wait
  points, exactly the SDK set — `next_cycle()`, `until(pred)`/
  `wait_until(pred)` (optionally `.deadline(…)`-decorated), `wait(TIME)`
  (cycle-time arithmetic, never wall clock), `all(...)`/`race(...)` over
  admitted wait points, and awaiting a child trust sequence — each yielding
  to the next cycle at most once per poll. It MUST reject at build time,
  with a diagnostic naming the construct and the admitted set (F23): any
  other `.await` operand (plain Rust's `.await` accepts anything
  implementing `IntoFuture`, so no trait bound can restrict it — validation
  is the enforcement, not types), `async` blocks and closures, executor/
  reactor APIs and hand-written futures, and **any macro invocation outside
  the admitted macro allowlist** (a macro can expand to a hidden `.await`;
  unrecognized macros are conservatively rejected). The v1 allowlist contains
  only SDK literal helpers that cannot expand to control flow or `.await`:
  `iec!(...)` duration/time literals used as wait/deadline arguments. Adding
  another macro to the allowlist requires a positive conformance fixture and
  validator review. Runtime backstop: an
  admitted wait that never progresses is bounded by its `deadline(…)` and
  by the program's budget/deadline machinery (RS-26..28) — a stuck sequence
  parks its instance and faults per policy; it can never block the scan.
- **RS-77 (exchange access).** The body accesses exchange and retain data
  only through **closure-scoped accessors** — `s.read(|io| …)`,
  `s.write(|io| { … })`, `s.retain_read(|r| …)`, `s.retain_write(|r| { … })`
  — and the predicate closures of wait points. These closures are
  synchronous (the validator rejects wait points inside them) and their
  `&`/`&mut` arguments cannot escape the call (higher-ranked closure
  lifetimes), so no reference is ever held across a wait point —
  structural, and additionally validated. Cycle coherence (RS-38) is
  preserved exactly: every access acts on the current poll's exchange.
- **RS-78 (restart semantics).** On warm or cold restart the sequence
  restarts **from the top** — SFC-initial-step semantics. Durable progress
  belongs in `#[retain]` fields (the example resumes the grid from
  `boxes_placed`). This is honest: Rust async state machines are not
  serializable, so we do not pretend to persist them (resumable checkpoints
  are §20 Q8).
- **RS-79 (faults & determinism).** `deadline(…)` elapsing, an `Err` return,
  or a panic inside a poll fault the instance through the normal machinery
  (RS-34); the diagnostic includes the *sequence location* (file:line of the
  pending wait point, captured by the lowering). Sequences are
  deterministic under RS-43 by construction — same input trace ⇒ same poll
  path — so they record and replay (§12) like any other module.
- **RS-80 (bounded polls).** Each poll is one bounded step under the
  program's budget. The validator additionally rejects any loop in the
  sequence body with no reachable wait point on every path — the same
  defect class as an unbounded ST loop; the runtime deadline/`checkpoint`
  machinery (RS-26..28) remains the backstop for loops whose wait points
  are conditionally skipped at runtime.

### 6.4 Retain (carried, with the migration story completed)

Interface retain (RS-41) and versioned opaque-state retain (RS-42) stand as
specified in v1 §6.8. v2 adds:

- **RS-81** Opaque retain blobs ride the existing `STRN` codec as a new
  section (format version bump) and inherit the properties verified in §3.4:
  atomic writes, CRC, orphan-drop with events. Version mismatch policy per
  binding: `discard` (default) or `fault`.
- **RS-82** Interface-retain fields participate in the runtime's **typed
  migration** (`runtime/restart.rs:169-384`) exactly like ST retain
  variables — field add/drop reconciliation and range re-validation included
  — because they *are* runtime retain variables (RS-41). The migration
  events name the module.

### 6.5 Diagnostics, logging, units

- `ctx.log(...)` stays deferred, allocation-free, rate-limited (RS-25) —
  drained post-cycle into the `RuntimeEvent`/historian stream. v2 adds
  structured key-value payloads (deferred formatting, defmt-style) so traces
  and the historian index them without scan-path cost.
- `ctx.diagnostic(code, state)` raises/clears operator-visible diagnostics
  (v1 §6.3); generated status outputs `_STATE/_FAULT_CODE/_OVERRUNS`
  (RS-36) remain the ST/HMI-facing supervision surface. Per-invocation
  durations surface via telemetry (§12.1), **not** as exchange outputs —
  keeping exchange blocks lean (deliberate deviation from the reviewed
  draft's `_LAST_DURATION_US` outputs).
- `#[unit]`/`#[range]` metadata: §6.1 RS-72.

### 6.6 Services from the PLC side

The heartbeat/staleness supervision idiom (v1 RS-51, `SVC_SUPERVISOR`) gets
a Rust-native form — a `ServiceLink<T>` SDK type wrapping the bound
exchange variables + heartbeat + freshness window, so Rust programs consume
service data with the same explicitness ST does:

```rust
#[input] recipe: ServiceLink<RecipeAdvice>,   // fresh()/value()/age_cycles()
…
if let Some(advice) = io.recipe.fresh() { self.apply(advice); }
else { self.fallback(); }
```

Same variables, same honesty, better types. Service side: §11.

---

## 7. Module model and ABI

Carried from v1 §5 with three amendments; the normative ABI is Appendix B
(unchanged, `abi_major = 1`).

### 7.1 One module, three integration points (v1 §5.1–5.2, unchanged)

A **module** is the unit of packaging, versioning, loading, and trust; it
exports FUNCTIONs, FBs, PROGRAMs (RS-01..06). In the greenfield door the
project crate *is* the module (RS-63); in the brownfield door modules live
under `<project>/modules/<name>/` with `module.toml`, per-target artifacts,
and generated declarations, discovered by the bundle loader
(`config/loaders.rs:2-66` gains `modules/`).

### 7.2 ABI strategy (v1 §5.3–5.4, unchanged; amendment on packaging)

Native form = `cdylib` + versioned `repr(C)` vtable + strict handshake
(RS-07..09); sandboxed form = wasmtime; built-in form for first-party;
source-built modules pinned to exact toolchain fingerprints (RS-55).
Amendments:

- **RS-83 (D23)** Two compatibility regimes, explicit: **source-built**
  modules (the default DX, including every Rust-first project) keep v1's
  exact-fingerprint match — cheap for us, eliminates silent-UB reports.
  **Precompiled packages** (vendor distribution) target *ABI-line*
  compatibility (abi_major equality + additive minors + SDK compat line) in
  a later phase, gated on abi 1.x field evidence — because demanding a
  vendor rebuild per rustc release would strangle the package ecosystem.
  Never both regimes silently: the manifest says which, the loader enforces
  which.
- **RS-84** The module-host and ABI crates are **new workspace crates**
  (`trust-module-abi` no_std; `trust-module-host` in trust-runtime's host
  layer; `trust-plc` SDK) with registered unsafe sites per repo policy —
  the runtime crates' `#![forbid(unsafe_code)]` (§3.2) is not weakened.
  xtask full-map edges: ABI/SDK crates MUST NOT depend on the host
  (mirroring `trust-runtime-core` direction rules).

### 7.3 Marshalling and memory model (v1 §5.5, unchanged)

Runtime-owned interface variables; generated `repr(C)` exchange blocks;
copy-in → call → copy-out; no retained pointers; fixed-capacity
strings/arrays; compile-time rejection of heap types in interface position;
load-time marshalling plans so per-cycle work is sequential copies
(RS-10..14a). Rationale table in v1 stands (cycle coherence, pointer-free
ABI, torn state impossible; zero-copy is §20 Q5).

### 7.4 Declared interfaces and the digest chain (v1 §5.6, extended)

The Rust declaration is the single source of interface truth; `.st`
declarations are generated with the `{external module := '…', digest :=
'b3:…'}` pragma (RS-15..17) — now explicitly implemented on the Open-OT
`{attribute}` parsing precedent (§3.6), and extended to cover the v2
artifacts:

- **RS-85** The BLAKE3 interface digest covers: exports (names, kinds,
  typed interfaces, defaults, retain/safe-default/unit/range metadata),
  generated configuration inputs (`trust.toml` task/io sections), and the
  device-import provenance (RS-74). It is embedded in: the module binary,
  every generated `.st` file, the compiled `.stbc` binding section, and the
  bundle manifest. Any mismatch anywhere in the chain is a build/load error
  naming the stale artifact and the regenerating command (F9).

---

## 8. Execution semantics

v1 §6 is carried in full: instance lifecycle state machine (RS-18..20),
scheduling integration (RS-21..23 — one registry lookup in
`execute_program_by_name`, an EXTERNAL kind in `execute_native_call`, no
separate module scheduler), the cycle-context capability set, temporal
protection with per-form honesty (RS-26..29 — native: detect + cooperative
checkpoints; wasm: epoch preempt/fuel), memory protection (RS-30..33,
allocation tripwire), fault model and output policy (RS-34..37), data
exchange and consistency (RS-38..40), retain (RS-41..42 → §6.4), determinism
(RS-43..44), and wasm engine config (RS-45..47). v2 adds precision in three
places:

### 8.1 Cycle order with Rust programs (normative restatement)

Within `Runtime::execute_cycle` (§3.2), Rust `cyclic_rt` programs occupy
ordinary slots in each task's ordered program list; `inline_rt` calls nest
inside whichever program slot reaches them; module copy-in happens at slot
entry from current storage, copy-out at slot exit into storage — so an ST
program later in the same task sees the Rust program's outputs this cycle,
exactly as with two ST programs. Background (task-unbound) module programs
run every base scan like background ST programs.

### 8.2 Per-invocation timing (RS-26/29, wired to real seams)

Entry/exit stamps around the vtable call feed `record_call` with new kinds
(`"module"`, `"module_fb"`) — the mechanism exists and ranks contributors
already (§3.6); per-instance overruns extend the task-overrun counter
pattern; the **dormant `set_execution_deadline`** (§3.2) is armed per task
tick so *ST* code in the same task also gains wall-clock protection — one
deadline discipline across languages (this closes a v1 gap where only
modules had wall-clock deadlines).

### 8.3 Overrun and fault escalation (unchanged, restated against code)

`deadline_action`: `log | skip | fault-module | fault-task` (default
`fault-module`); output policy `default | hold-last` (default `default`,
D11). Escalation terminates in the existing watchdog/fault machinery
(`WatchdogPolicy`, `FaultPolicy`, `IoSafeState`) — and §14 carries the
platform-level honesty notes from §3.2 (SafeHalt-only safe-state, stop
applies none) into user-facing documentation requirements.

### 8.4 Warm/cold restart and online change (v1 RS-21a, made honest)

- Module programs participate in warm/cold restart like ST programs
  (retain preserved on warm; `init()` re-runs on cold) — restart machinery
  verified in `runtime/restart.rs`.
- **RS-86** Online change baseline, stated exactly: today's primitive is
  `bytecode.reload` = body-swap + warm restart + retain reload for
  *already-declared* POUs; new top-level names/globals are rejected; no
  tooling invokes it (§3.6). For modules, v2 requires only parity with that
  baseline: a reload whose bundle touches a module binary/digest MUST
  re-run the handshake and re-create instances before the swapped
  configuration goes live, and MUST refuse the swap (not degrade) on
  handshake failure. Generation-based hot swap with state migration — the
  full "online change" of vendor marketing — is future work with its own
  spec (§20 Q13); we will not imply we have it before we do.

### 8.5 Containment honesty (v1 RS-30/D8, restated)

Native modules are inside the TCB: segfault/UB cannot be contained
in-process, and the docs say so; the wasm form and the service tier exist
precisely because of this. The failure catalogue (§14) binds each mode to a
detection + reaction; the conformance suite exercises the reactions (§13.4).

---

## 9. Timing admission

### 9.1 Principle

truST shall not promise compile-time WCET for arbitrary Rust — nobody can,
and pretending is fabricated green. It shall instead implement **admission**:
at build/deploy time, the task set must demonstrably fit the scheduler it
will actually run on — every base frame's worst-case work within the frame,
every task's deadline satisfied — under evidence-graded budgets, or the
build fails with an itemized account naming the worst frame. Runtime
enforcement (deadlines, watchdog — §8) then polices what admission assumed,
and §12.4 explains any divergence.

This is D15. As of 2026-07-03 we have not found a PLC build system —
TwinCAT, CODESYS, PLCnext, ctrlX among those checked (§17) — that computes
task-set admission from component budgets; the claim stays dated and scoped
per §2.4/§17.3.

### 9.2 Evidence levels

Every in-cycle POU (Rust *and* ST) carries a budget with a grade:

| Level | Meaning | Trusted in profile |
|-------|---------|--------------------|
| `declared` | Author-asserted `#[budget]` / manifest value | development |
| `measured` | truST harness measured on the **named target** with stress inputs; margin applied; report digest-bound (§9.5) | production |
| `wasm_fuel` | Deterministic fuel bound + per-target fuel→time calibration | production, hard-rt |
| `static_stbc` | Static worst-path cost model over ST bytecode (opcode costs × bounded loops) | production, hard-rt *(later phase — §19 P4b)* |
| `builtin_maintained` | First-party module with repo-maintained evidence | all |

- **RS-87** Budgets attach to exports (`#[budget]`, RS-70) and to ST
  programs (measured task stats seed them; explicit pragma overrides). The
  effective budget = min(declared, manifest override); its evidence level
  is recorded in the `.trusttime` admission record.
- **RS-88** ST loop bounds: `FOR` with constant bounds are inferred;
  `WHILE`/data-dependent loops require `{trust_loop_bound := N}` (the
  pragma micro-syntax exists, §3.6) to participate in `static_stbc`; absent
  a bound, the enclosing POU is admissible only by measurement.

### 9.3 The admission computation (worst-frame / hyperperiod model)

Admission MUST model the scheduler that actually runs (§3.2): **one**
resource thread ticking at the base frame; each frame collects the tasks
due that frame, sorts them by (priority, due-time, index), and executes
them **non-preemptively to completion inside that frame**, together with
background programs and the frame-global phases. Average utilization and
per-task-in-isolation bounds are therefore meaningless here: a 10 ms task
is not due in every 1 ms frame, but in the frames where it *is* due it
runs back-to-back with the 1 ms task. Admission models those coincidence
frames explicitly — bursts, not averages.

Definitions:

```
F    = runtime.cycle_interval — the base frame
T_i  = period of periodic task i;  RS-66 already requires F | T_i
       (every period an integer multiple of F) — a precondition here
B_i  = admitted budget of task i
     = Σ its program-slot budgets (Rust: module budget; ST: measured/static)
     + Σ inline_rt call budgets × max_call_count   (RS-89)
     + its slots' marshalling bounds               (RS-14a plans)
H    = lcm(T_i) over periodic tasks — the hyperperiod. If H is
       impractically large, the bounded conservative fallback is a single
       synthetic frame in which EVERY task is due (a valid upper bound:
       any real frame's due-set is a subset of it).
```

- **RS-115 (frame simulation & admit condition).** The build simulates
  task releases over the `H / F` frames of one hyperperiod using the
  actual readiness and ordering rules (synchronized release at t = 0;
  priority/due-time/index sort). For every frame it computes:

```
frame_work = input_update_bound                   (measured platform figure)
           + Σ B_i  over tasks due this frame     (scheduler order)
           + Σ background-program budgets         (every frame — §3.2)
           + queued debug/control write drain bound   (bounded queue)
           + service_exchange_bound
           + retain_save_bound                    (frames where a save may fire)
           + trace_recorder_bound                 (when recording enabled, RS-103)
           + output_commit_bound + runtime_overhead_bound
           + jitter_margin                        (profile; trust.toml [timing])

ADMIT  iff  frame_work ≤ F  for EVERY frame in the hyperperiod,
       and every task completes within its release frame — which, given
       non-preemptive frame execution and F | T_i, satisfies each task's
       deadline (= its period).
```

- **RS-116 (event tasks).** Event tasks (`single` rising edge, §3.2) have
  no period. They are included in **every** frame's due-set (conservative
  default). A binding MAY declare a bounded event model
  (`min_interarrival`); the simulation then places the event task at that
  rate — but it is still included in the worst frame, because nothing
  prevents it coinciding with one.
- **RS-117 (scope honesty).** This model is exact for the current
  non-preemptive, single-resource-thread scheduler and MUST be re-derived
  if that changes. Fixed-priority response-time analysis (preemption,
  blocking terms) is reserved for a possible future preemptive scheduler
  and MUST NOT be applied to this one.
- **RS-89 Call-count analysis.** For each `inline_rt` call site in ST:
  outside loops → 1; inside statically-bounded loops → the bound; inside
  unbounded loops → inadmissible without `{trust_loop_bound}`; through
  dynamic dispatch → requires a closed target set, else inadmissible. Calls
  *inside* a Rust module body need no per-call analysis — the module's own
  whole-invocation budget covers them (a deliberate simplification: the
  module is the unit of evidence).
- **RS-90** `trust build` prints the per-frame account — **worst frame
  first, naming its coincident tasks** — and fails on deficit with the
  largest contributors and concrete remedies (move a program to a slower
  task, reduce call count, demote to a service, raise a period, stagger…
  once offsets exist):

```
frame admission over hyperperiod H = 10 ms  (F = 1 ms, 10 frames)
worst frame: t ≡ 0 ms — due: fast + main  (coincident release)       evidence
  input update                              45 µs                    measured (platform)
  fast: safety::SafetyGate                  60 µs                    measured
  fast: motion::AxisController             150 µs                    measured (tt:9f2…)
  fast: FILTER_EMA × 4 (inline)             48 µs                    measured
  main: palletizer::Palletizer              80 µs                    measured
  main: supervision::RecipeSupervisor       25 µs                    measured
  control-write drain + service exchange    20 µs                    measured (platform)
  retain save (due-able this frame)         30 µs                    measured
  output commit + runtime overhead          75 µs                    measured (platform)
  jitter margin                            150 µs                    profile: production
  ─────────────────────────────────────────────
  worst-frame work                         683 µs  ≤ F = 1000 µs  →  PASS (reserve 317 µs)
all 10 frames fit; deadlines satisfied (non-preemptive release-frame completion)
```

### 9.4 Profiles

- **development**: `declared` accepted; unknown crates warn; admission
  report informational.
- **production**: `measured`/`wasm_fuel` (or better) required for scan-lane
  Rust; admission failure blocks build; admission record required at deploy.
- **hard-rt**: adds `static_stbc` (or explicit waiver) for ST, larger
  default jitter margin, stricter crate policy (§10), and `strict`
  real-time posture checks at boot (`host/linux_rt.rs`).

### 9.5 The measurement harness

`trust timing measure --target <name>`:

- **RS-91** Runs on the named target (or trust-builder-managed rig),
  pinning/locking per the RT config; drives each export through the
  conformance input corpus **plus** author-declared stress inputs and
  property-generated adversarial inputs (§13.3); collects
  min/mean/p99/p99.9/max over ≥ a configured sample count; applies the
  profile margin; emits `.trusttime` bound to (module digest, target
  triple, runtime version). Re-measurement is forced by any digest change.
- **RS-92** The same harness run doubles as the **crate-evidence probe**
  (§10.4): the allocation tripwire (RS-32) is armed, and on Linux the run
  executes under a **syscall audit** (seccomp user-notify) recording every
  syscall reached from `cycle()` — the measured half of crate admission.
  Timing evidence and behavior evidence come from one instrumented run,
  honestly labeled *observed under test corpus* (not proof).

### 9.6 Continuous verification in production

- **RS-93** The runtime compares per-instance p99 (RS-29 stats, via the
  `record_call` seam) against admitted budgets and raises an **admission
  drift** diagnostic when sustained p99 exceeds the admitted value —
  before the watchdog ever fires. Drift events carry the module, instance,
  measured vs. admitted values, and land in live values/Devices & Connections/historian
  like any diagnostic (P8). The `.trusttime` admission record ships in the
  bundle so the runtime knows what was admitted.

---

## 10. Crate admission

### 10.1 Principle

"Use any crate" is the headline promise of Rust-first PLC development — and
an unqualified lie if `reqwest` can end up inside a 1 ms task. Crate
admission makes the promise honest: every build classifies the dependency
graph **reachable from scan-lane entry points** and enforces project policy
over it. It is explicitly an *evidence and admission* system, not a proof
system (D16) — and the report says which class each verdict came from.

### 10.2 Classifications

```
rt_ok           evidence supports scan use at the required grade
rt_candidate    statically clean; measurement required before production
service_only    blocking/network/fs/async/thread behavior in reachable paths
forbidden       violates project policy (license, advisory, unsafe budget, …)
unknown         insufficient evidence (denied in scan lanes under production)
```

### 10.3 Layered evidence (what actually exists behind each verdict)

1. **Curated knowledge base.** A versioned, repo-maintained database of
   common crates: `tokio`/`reqwest`/`sqlx` → `service_only`;
   `nalgebra`/`heapless`/`micromath`/`pid` → `rt_candidate` with notes
   (e.g. "compile regex at init only"). Shipped with the toolchain,
   overridable per project, provenance recorded. Honest scope: this is
   curation, and the report labels it as such.
2. **Static source scan** (`trust module check`, extends RS-24): deny-listed
   API families in cycle-reachable code — `std::net`, `std::fs`,
   `std::process`, `std::thread::spawn/sleep/park`, `std::time::SystemTime/
   Instant::now` (time is injected — P2), blocking channel recv, lock
   acquisition on non-`try_` paths, `async` executor entry points; plus
   `unsafe`-site and recursion inventories. Attribution is best-effort
   through monomorphized calls and honest about dynamic dispatch (reported
   `unknown`, not guessed).
3. **Artifact symbol scan.** The built `cdylib`'s undefined-symbol imports
   are checked against a syscall-adjacent deny list (`socket`, `connect`,
   `open*`, `pthread_create`, `epoll_*`, `nanosleep`, …) — link-level truth
   that is cheap, source-independent, and catches what macros hide.
4. **Measured tripwires** (the differentiator, RS-92): the timing-harness
   run arms the allocation tripwire (RS-32) and a seccomp user-notify
   syscall audit; every allocation and every syscall reached from `init()`
   vs. `cycle()` is recorded and attributed. `cycle()`-path findings demote
   the verdict; `init()`-path findings annotate it. Labeled *observed under
   the test corpus* — evidence, not proof, and stronger than anything a
   static-only claim could honestly say.

### 10.4 Requirements

- **RS-94** `trust crate check [--rt-path <export>]` produces the
  classification report: per-crate verdict, evidence class per finding, and
  for scan-lane rejections either the **concrete call path** when attribution
  resolves (`motion::cycle → recipe_client::get → reqwest::Client::send`) or
  the explicit `unknown`/dynamic-dispatch reason when it does not, plus the
  remedy ("move to a service; exchange via freshness-checked variables").
- **RS-95** Policy lives in `trust.toml [crates]` (deny_unknown,
  deny_service_only, per-crate allow/deny, license allow-list, advisory
  severity gate, `unsafe` budget) with profile defaults: development warns,
  production/hard-rt deny. The build report and the bundle both embed the
  resulting `.trustcrate` document (SBOM, license set, advisory scan,
  unsafe/ffi inventory, verdicts + evidence).
- **RS-96** Services are exempt from RT classification by construction but
  still get SBOM/license/advisory reporting — one supply-chain story, two
  strictness levels.
- **RS-97** A production deploy MUST refuse a bundle whose `.trustcrate`
  is missing, stale (digest mismatch), or in violation of the target's
  policy — same gate discipline as `.trusttime` (RS-93).

---

## 11. Services outside the cycle

v1 §7 is carried in full: services are ordinary Rust programs on the
existing control plane (JSON-line protocol, §3.5 — **not** JSON-RPC, the SDK
hides the framing), RBAC'd like every client, reading cycle-coherent
snapshots and issuing cycle-aligned writes; `trust-service-sdk` wraps this
with mandatory quality/staleness metadata (RS-48..50); PLC logic gates on
service data explicitly (RS-51..52; Rust-side sugar `ServiceLink<T>`,
§6.6); deployment is systemd units; the connection registry feeds honest
liveness into Devices & Connections (RS-53..54). v2 additions:

- **RS-98** `trust.toml [services.*]` declares each service (binary,
  heartbeat variable, timeout, on-stale policy) and generates: the systemd
  unit, the heartbeat/exchange variable declarations (into generated ST +
  Rust bindings), and a scaffolded supervisor program (the RS-51 idiom as
  Rust, template-owned so every project supervises the same way).
- **RS-99** Subscriptions remain SDK-side polling over snapshots initially
  (there is no push channel in the plane today — §3.5 [v2 correction]); the
  SDK API is shaped so a future push transport changes no user code. A
  dedicated high-rate shared-memory data plane (vision/vibration/waveform
  workloads) stays a measured v2 addendum (§20 Q6) — with the note that the
  Open-OT `SharedRecordPublisher` (§3.5) is the in-tree precedent it will
  generalize, not a from-scratch invention.
- **RS-100** Fleet reach: services may equally subscribe over the zenoh
  mesh (`host/mesh/`) where deployed — the SDK exposes the same
  quality-carrying read model over either transport. (Fleet-level services
  — one anomaly scorer for ten cells — become an examples-and-docs story,
  not new runtime machinery.)

---

## 12. Observability, record, and replay

### 12.1 Live observability (extends v1 RS-29/RS-36/RS-57)

Per module instance, always on: state, fault code, overrun count, last/p99
duration, evidence grade, admission budget vs. observed p99 (drift, RS-93),
retain version, crate-report status. Surfaced through the existing lenses —
live values (via the DAP pipeline, §3.6), control plane, Devices & Connections
runtime node, historian/Prometheus — using the `record_call` +
`RuntimeEvent` seams (§3.6). No parallel telemetry universe (P8).

### 12.2 Cycle trace recording

- **RS-101** The runtime can record, per task cycle: the input image +
  bound-variable deltas, applied control-plane writes/forces, cycle context
  (counter, dt, rng stream position), per-slot POU timings, module
  states/faults/diagnostics, service exchange values + quality, and output
  image — keyed by bundle/module digests and target identity. Format:
  `.trusttrace`, sectioned+CRC'd like `.stbc`/`STRN` (one container
  discipline; App. C).
- **RS-102** Recording modes: (a) **on-demand window** (`trust record
  --task fast --duration 30s`), (b) **flight recorder** — always-on
  bounded ring holding the last N seconds, dumped automatically on fault
  (the 3 a.m. black box; ring publication rides the Open-OT shm mechanism
  so the scan thread never blocks on trace I/O), (c) **sim recording** —
  free, always available in `trust sim`/`trust-sim`.
- **RS-103** Recording overhead is budgeted and admitted like everything
  else: the recorder's per-cycle cost appears in the §9.3 account when
  enabled, and a measured reference figure (target: ≤ 3 % of a 1 ms cycle
  for a 1 KiB image at defaults) gates the feature's "supported" flag —
  same discipline as RS-46.

### 12.3 Replay

Determinism (RS-43/44, P10) makes a trace a *test*:

- **RS-104** `trust replay <trace>` re-executes recorded cycles against the
  current build in the simulator — same inputs, same context — and reports
  divergences (outputs, states, faults, timings) with per-variable
  tolerances. Module-level replay runs a single program/FB against the
  trace; project-level replay runs the whole task set.
- **RS-105** Replay is a first-class `cargo test` citizen via `trust-sim`
  (§13.2): commissioning traces committed under `traces/` become regression
  gates — *the machine's accepted behavior, in CI*. Float caveat: native
  replay is bit-exact per target (RS-44); cross-target replay uses
  tolerance bands or the wasm form.
- **RS-106** `trust diff a.trusttrace b.trusttrace [--tolerance …]`
  compares two recordings (old vs. new module version, native vs. wasm,
  sim vs. plant) and renders a variable-level divergence report — the
  upgrade-confidence tool.

### 12.4 Explanation tooling

- **RS-107** `trust explain-overrun --trace t --cycle N` (and the same view
  in the IDE on any overrun diagnostic) reconstructs the cycle's timeline
  from per-slot timings and names the contributors against their admitted
  budgets, ending with concrete remedies — the §9.3 account, applied
  retroactively to one real cycle:

```
cycle 1 842 291 exceeded the 1000 µs base frame (measured 1 378 µs)
  frame due-set: fast + main   (coincident release — matches admitted worst frame)
  fast: motion::AxisController      812 µs   admitted 150 µs  ← offender
  fast: safety::SafetyGate           62 µs   admitted  60 µs
  main: palletizer::Palletizer       78 µs   admitted  80 µs
  …
likely cause: planner replan with 384 segments (admitted stress corpus max: 64)
remedies: bound segment count; move replanning to a service; lower task rate
```

- **RS-108** Every fault diagnostic links its flight-recorder dump (when
  enabled); every admission-drift diagnostic (RS-93) links the cycles that
  breached. Nothing says "error" without saying *which cycle, which module,
  which budget*.

---

## 13. Testing strategy

Layered as in v1 §8.3, with the machine-sim layer promoted to a product
feature. All layers run off-target; all are ordinary `cargo test`.

### 13.1 Unit tests (module author's)

`cycle()` is a method; the SDK's `TestBench` drives lifecycle + scripted
cycles + assertions on outputs/diagnostics/faults (v1 layer 1, unchanged).
Sequences test the same way — a `TestBench` poll *is* a cycle, so a whole
pick-place sequence unit-tests in microseconds.

### 13.2 The machine in `cargo test` (`trust-sim`, D20)

- **RS-109** `trust-sim` is a public crate wrapping the existing
  `TestHarness` (§3.6) into a supported API: load the *project* (trust.toml
  + generated ST + module, or any bundle), then drive real
  `execute_cycle()`s in-process with **virtual time**:

```rust
use trust_sim::Machine;

#[test]
fn estop_kills_drive_enable_within_one_cycle() {
    let mut m = Machine::from_project(env!("CARGO_MANIFEST_DIR")).unwrap();
    m.set_input("%IX0.0", false).unwrap();          // open the e-stop chain
    m.cycle();
    assert_eq!(m.output_bool("%QX0.0").unwrap(), false);   // drive enable
}

#[test]
fn full_pallet_completes_and_counts() {
    let mut m = Machine::from_project(env!("CARGO_MANIFEST_DIR")).unwrap()
        .with_axis_model("axis_controller", SimAxis::ideal());      // simulation coupling
    m.set_input("%IX0.0", true).unwrap();
    m.set_named("palletizer.START", true).unwrap();
    m.run_until(|m| m.named_bool("palletizer.PALLET_COMPLETE"), 200_000).unwrap();
    assert_eq!(m.named_u32("palletizer.BOXES_PLACED"), 24);
}
```

  Virtual time (`advance_time`) means 200 000 cycles of a 1 ms task run in
  well under a second on a laptop; the physics coupling hooks
  (`host/simulation.rs`) are exposed for closed-loop plant models. This is
  the harness the runtime's own tests already trust — productized, not
  invented.
- **RS-110** `Machine` uses the same cycle semantics as the target
  (it *is* `execute_cycle`), and the docs state the deltas honestly: no
  real drivers, no thread/jitter effects, virtual clock. Timing is §9's
  job, behavior is this section's.
- **RS-111** Platform: Linux dev hosts at parity; macOS/Windows dev-host
  support is tracked as §20 Q11 (the runtime is a Linux host product; the
  sim path's OS-specific surface is small but not zero).

### 13.3 Property-based and adversarial testing

- **RS-112** The SDK ships proptest integration: strategies for exchange
  types (IEC-range-aware, `#[range]`-aware), harness drivers for FBs and
  for `Machine`, with failure shrinking down to a **replayable input
  trace** (a shrunken counterexample serializes as a `.trusttrace` — a bug
  report you can `trust replay`). The adversarial corpus generated here
  feeds the timing/crate measurement harness (RS-91/92) — one corpus,
  three uses (correctness, timing, behavior-evidence).

### 13.4 Conformance and failure injection (v1 layers 2/4, unchanged)

`trust module test` asserts the contract per form: handshake skew matrix,
panic containment, deadline reactions incl. native hard-loop and wasm trap,
allocation tripwire, retain save/load/version-mismatch, marshalling
round-trips incl. extremes, determinism (same stream twice ⇒ identical),
status outputs (RS-59 gate). CI runs it over deliberately-broken fixture
modules; the suite passes only if every one is rejected-or-contained with
the specified diagnostic.

### 13.5 Replay tests and CI/GitOps

- **RS-113** Replay assertions (§12.3) are part of the standard template:
  `tests/replay_commissioning.rs` runs committed traces on every PR with
  budget + output-tolerance + no-fault assertions.
- The deployment story completes the loop: text sources + deterministic
  codegen + signed bundles + `trust deploy`/`rollback` (existing,
  §1.1 table) mean a Rust-first project does **branch → PR → CI (unit +
  machine-sim + replay + admission + crate policy) → review the generated-ST
  diff → signed deploy → flight-recorder-audited operation** with no
  vendor-specific ALM product in sight. This paragraph is the workflow
  pitch of §1.2 made concrete, and it uses zero new mechanisms — only
  gates this spec already requires.

---

## 14. Failure modes, safety, and security

### 14.1 Failure-mode catalogue

v1 §10's F1–F15 are carried verbatim (panic, cooperative/hard overrun per
form, segfault honesty, allocation tripwire, ABI/digest skew, init/retain
failures, missing artifacts, service staleness/garbage, exit hangs). v2
appends:

| # | Failure | Detected by | Reaction |
|---|---------|-------------|----------|
| F16 | Task set inadmissible (worst frame exceeds F) | §9 frame simulation at build/deploy | build/deploy refused with worst-frame account naming coincident tasks (RS-90/115) |
| F17 | Policy-violating crate in scan path | §10 admission | build/deploy refused; call path named (RS-94) |
| F18 | Admission drift in production (p99 > admitted) | runtime comparison (RS-93) | diagnostic + flight-recorder link; escalation per project config |
| F19 | Replay divergence (old vs new behavior) | `trust replay`/CI (RS-104/113) | test failure with variable-level diff; never deploys |
| F20 | Admission record missing/stale at deploy (`.trusttime`/`.trustcrate`) | deploy gate (RS-93/97) | deploy refused naming the stale artifact |
| F21 | Sequence deadline elapsed (`until().deadline()`) | SDK (RS-79) | instance fault with sequence location + configured fault code |
| F22 | Generated-artifact drift (hand-edited/stale) | digest chain (RS-61/85) | build error naming the regenerating command |
| F23 | Non-conformant sequence body (foreign `.await`, async block/closure, executor API, unrecognized macro, loop without wait point) | `#[trust_sequence]` validation at build (RS-76/80) | build error naming the construct and the admitted wait-point set |

**Fault-code namespace (binding for SDK, generated outputs, and Live
Values):** `_FAULT_CODE` is a 32-bit unsigned value. `0` means no fault.
`1..=999` are truST runtime/SDK reserved faults (panic, deadline, range,
protocol, init/exit). `1000..=9999` are truST toolchain/generated-artifact
faults. `10000..=65535` are reserved for standard library and built-in FB
migrations. `65536..=0x7fff_ffff` are user/module fault codes; user enums
lower into this range by default. `0x8000_0000..=0xffff_ffff` are external
vendor/integration codes and MUST carry a source namespace in diagnostics.
Generated declarations and VS Code render both numeric code and symbolic
name when the source enum is known.

### 14.2 Platform honesty notes (user-docs normative)

Carried and extended from §3.2's verified findings — these appear in
public docs because operators plan around them:

- Native in-process code (any language, any vendor) cannot be contained
  after memory corruption; safe actuation ultimately rests on external
  watchdogs, fieldbus safe-state, and safety hardware (RS-30, F5).
- Safe-state output writes happen on **fault with `SafeHalt`**; plain
  `Halt` freezes outputs; a **graceful stop applies no safe state** today.
  Rust-first templates default to `SafeHalt` + explicit safe_state lists
  (RS-68); the stop-behavior gap is tracked platform-side (§20 Q12).
- Task overruns are accounted and now admitted (§9), but a missed periodic
  activation is not a fault by itself — the watchdog and deadline actions
  are the enforcement line.

### 14.3 Security and trust model (v1 §11 carried)

Trust tiers (built-in > signed native > unsigned native (dev-only) >
sandboxed > service) — unchanged. Signing rides the existing deploy-key
keyring (D13) — unchanged, including the honest note that asymmetric
signing is a platform-wide upgrade, not a modules-only pretense. Control
plane: services and tooling authenticate exactly like every client
(tokenless Unix peer = Admin on the 0600 socket, tokenless TCP = Viewer,
per-command RBAC — verified §3.5); module control RPCs slot into the same
role table (`module.list/status` Viewer, `module.reset` Engineer,
load/unload Admin). Supply chain gains teeth via RS-95/97 (`.trustcrate`
gate) on top of v1 T4's locked builds.

---

## 15. Toolchain and IDE

### 15.1 CLI: the `trust` umbrella (D24)

Today's surface is `trust-runtime` (27 subcommands, §3.6) + `trust-dev` +
`trust-debug` + `trust-lsp`. Telling a Rust developer to run
`trust-runtime wizard` undersells the product; v2 fronts everything with a
single `trust` binary (thin dispatch — no behavior forks) plus a
`cargo-trust` shim so both spellings work:

```
trust new rust-plc <name> | trust new st <name>        # both doors
trust check | build | sim | run | deploy | rollback     # fronting existing verbs
trust io import <url> --select <glob> --name <n>        # §6.2
trust timing measure|check|admit|report               # §9
trust crate check|report                                # §10
trust record|replay|diff|explain-overrun                # §12
trust module new|check|build|declare|test|package|sign  # v1 §8.1, unchanged
trust service new|run|package                           # §11
trust ctl … | trust ui | trust hmi …                    # existing operator verbs
```

- **RS-114** `cargo trust <verb>` == `trust <verb>`; `cargo test` needs no
  wrapper at all (that is the point). Existing `trust-runtime` spellings
  remain supported — operators' muscle memory and scripts are not broken.

### 15.2 IDE integration (VS Code workflow contract, v2.4)

The CLI remains complete and CI-first. The human product is also
VS Code-first: the G0 and brownfield flows MUST be reachable from visible
VS Code surfaces without command-palette or terminal dependence. Detailed
workflow and user stories live in `rust-plc-vscode-workflow-v1.md`; the
requirements below are binding here.

- **RS-118 — Visible-surface parity.** Every product-contract §3 flow MUST
  have a visible VS Code equivalent. The §3.1 ten-minute first-contact target
  applies independently to the VS Code path and is timed in acceptance.
- **RS-119 — Project-kind detection and one shell.** A workspace with
  `trust.toml` and `Cargo.toml` at the project root is a Rust PLC project and
  takes precedence over `trust-lsp.toml` detection. The truST Home/Activity
  Bar shell is kind-agnostic; Rust changes wiring, not chrome. No second Run
  surface, Rust activity container, or palette-only primary workflow.
- **RS-120 — JSON diagnostics/report seams.** `trust check --json`,
  `trust build --json`, and `trust replay --json` are versioned schemas and
  the only diagnostics/admission/replay entry points consumed by IDEs. Human
  text is never parsed. Per-issue fix actions are mandatory where a user can
  act.
- **RS-121 — Instance snapshot seam.** Module and Rust PLC live values extend
  the existing debug/control pipeline with an instance snapshot section
  (task, instance, state, fault, timing, sequence wait point, fields). No
  parallel telemetry pipeline.
- **RS-122 — Admission IDE rendering.** IDE admission views render the
  worst frame first, name coincident tasks, show evidence grades using §2.4
  vocabulary, and anchor F16 on `trust.toml` and F17 on `Cargo.toml`.
- **RS-123 — Generated ST IDE policy.** Generated IEC artifacts are
  read-only in the IDE, digest-protected by F22, navigable from Rust/source
  locations, and reviewable through native regenerated-vs-committed diffs.
- **RS-124 — Machine tests in VS Code Testing.** Rust PLC unit, machine, and
  replay tests appear in VS Code's native Testing view. Shrunk traces and
  replay actions are attached to failures when present.
- **RS-125 — Update honesty.** Updating a running Rust PLC simulation follows
  RS-86: reload-compatible body changes may apply; interface/config/digest
  changes are labeled Restart required. The UI never pretends to hot-apply a
  change the runtime cannot accept.
- **RS-126 — Bootstrap and version skew.** VS Code preflights `trust`,
  cargo/rustup, pinned toolchain, SDK resolution, templates, rust-analyzer
  state, target support, and version compatibility before writing or running
  a Rust PLC project. Version skew between VSIX, `trust`, SDK crates,
  generated artifacts, ABI, and runtime is refused with expected-vs-found
  diagnostics.

Generated declarations make Rust POUs ordinary LSP symbols: completion,
hover, go-to-declaration; go-to-definition jumps into the Rust source for
source-form modules (RS-56). The Devices & Connections runtime node, Live
Values, Problems, Testing, HMI, generated-ST diff, and admission report are
the named VS Code surfaces. Rust-first projects use rust-analyzer for Rust
and trust-lsp for generated/hand-written ST, with cross-navigation between
`VAR_CONFIG`/generated declarations and `#[trust_program]` fields.

### 15.3 Diagnostics quality bar (v1 tone, v2 examples)

Every new failure class ships with a diagnostic that names the artifact,
the expectation, the finding, and the command that fixes it — the F16/F17
examples in §9.3/§10.4 are normative for tone. A diagnostic that says
"error" without a remedy is a review-blocking defect in this feature area.

---

## 16. Worked examples (real-world)

Three examples spanning the adoption ladder (§0.4). APIs are normative for
shape, indicative for exact names. Example 1 is the flagship: a complete
Rust-first machine.

### 16.1 Greenfield: a palletizing cell, Rust-first (rung 3)

**The machine**: one servo axis (EtherCAT), a vacuum gripper on digital
outputs, an e-stop chain, a box-present sensor. Place 24 boxes in a grid,
recipe (grid pattern) supplied by a plant service. Fast interlocks at 1 ms,
sequence at 10 ms.

**The project**: layout and `trust.toml` as §5.1–5.2; the axis controller as
§6.1; the palletizing sequence as §6.3. What completes the picture:

`src/safety.rs` — the 1 ms interlock program, deliberately boring:

```rust
use trust_plc::prelude::*;

#[trust_program(name = "SAFETY_GATE")]
#[budget("20us")]
pub struct SafetyGate {
    #[input]  pub e_stop_ok: bool,
    #[input]  pub axis_fault: bool,
    #[output] #[safe_default(false)] pub power_permit: bool,
    #[output] pub e_stop_chain_broken: bool,       // chain status for HMI/reporting
    latched: bool,
}

impl PlcProgram for SafetyGate {
    fn cycle(&mut self, io: &mut Exchange<Self>, ctx: &mut CycleCtx) -> PlcResult<()> {
        if !io.e_stop_ok || io.axis_fault {
            self.latched = true;
            ctx.diagnostic(DiagCode::SAFETY_TRIP, DiagState::Raised);
        }
        io.e_stop_chain_broken = !io.e_stop_ok;
        io.power_permit = !self.latched && io.e_stop_ok && !io.axis_fault;
        Ok(())
    }
}
```

(And the honest reminder that belongs in every example: `power_permit`
*conditions* the drive; the e-stop itself is wired hardware. N1.)

`services/recipe/src/main.rs` — tier 3, full ecosystem allowed:

```rust
use trust_service_sdk::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let rt = Client::connect_env().await?;          // unix socket / TCP+token
    let mes = mes_client::from_env()?;              // HTTP, sqlx, anything — tier 3
    let mut hb = 0u32;
    loop {
        let recipe = mes.current_pallet_pattern().await?;   // 24 slots
        hb = hb.wrapping_add(1);
        rt.write_many([
            ("svc.recipe_slots", recipe.into_iec_array()),
            ("svc.recipe_hb", hb.into()),
        ]).await?;                                   // acked with landing cycle
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}
```

The PLC consumes it through `ServiceLink` (§6.6) with the generated
supervisor (RS-98): stale recipe ⇒ the sequence finishes the current box and
holds at a safe step with a visible diagnostic — never a mystery stall.

`tests/machine.rs` — the machine in CI (§13.2):

```rust
#[test]
fn estop_mid_move_drops_permit_and_holds_sequence() {
    let mut m = Machine::from_project(env!("CARGO_MANIFEST_DIR")).unwrap()
        .with_axis_model("axis_controller", SimAxis::with_limits(2000.0, 5000.0));
    m.start_sequence();                              // helper: start + safety ok
    m.run_until(|m| m.named_bool("palletizer.MOVE_CMD"), 50_000).unwrap();
    m.set_input("%IX0.0", false).unwrap();           // e-stop mid-move
    m.cycle();
    assert!(!m.output_bool("%QX0.0").unwrap());      // permit dropped, 1 cycle
    let fc = m.named_dword("palletizer._FAULT_CODE");
    assert_eq!(fc, 0, "sequence holds, does not fault, on safety trip");
}

proptest! {                                          // §13.3
    #[test]
    fn any_recipe_grid_never_commands_outside_envelope(g in grid_strategy()) {
        let mut m = machine_with_recipe(g);
        m.run_until(|m| m.named_bool("palletizer.PALLET_COMPLETE"), 500_000)?;
        prop_assert!(m.max_observed("%QL100") <= ENVELOPE_MM);
    }
}
```

**The build** (what the developer sees; §9.3/§10.4 formats):

```
$ trust build --profile production
   Compiling palletizer v0.3.1 (rustc 1.95.0, aarch64-unknown-linux-gnu)
   Generating src/generated/trust/{declarations,configuration}.st   digest b3:9f41…
   Compiling ST → program.stbc                                      OK
   Timing admission (frames over H = 10 ms, F = 1 ms)
     worst frame t≡0 (due: fast+main)   work 683 µs / 1000 µs   PASS   evidence: measured
     all 10 frames fit; deadlines satisfied (release-frame completion)
   Crate admission (production policy)
     nalgebra 0.34   rt_ok (measured: 0 allocs, 0 syscalls in cycle path)
     heapless 0.9    rt_ok
     tokio, reqwest  service_only → services/recipe only            OK
   Bundle target/trust/palletizer.bundle   signed (key: line-3)      OK

$ trust deploy cell-01 && trust ctl restart --mode warm
```

Six months later, a planner upgrade: the PR shows the Rust diff *and* the
generated-ST diff; CI replays `traces/commissioning.trusttrace` and fails on
a 0.4 mm setpoint divergence beyond tolerance — caught before the plant ever
sees it. That is the workflow this architecture exists to enable.

### 16.2 Brownfield: one Rust FB inside an ST project (rung 1)

Carried from v1 §9.1 in abridged form — this door must stay first-class. A
module crate exports:

```rust
#[trust_fb(name = "FILTER_EMA")]
#[budget("15us")]
pub struct FilterEma {
    #[input] enable: bool,
    #[input] raw: f64,
    #[input(default = 0.1)] alpha: f64,
    #[output] filtered: f64,
    #[output] valid: bool,
    prev: Option<f64>,          // private state — invisible to ST
}

impl PlcFb for FilterEma {
    fn cycle(&mut self, io: &mut Exchange<Self>, _ctx: &mut CycleCtx) -> PlcResult<()> {
        if !io.enable { self.prev = None; io.valid = false; io.filtered = 0.0; return Ok(()); }
        let a = io.alpha.clamp(0.0, 1.0);
        let y = self.prev.map_or(io.raw, |p| p + a * (io.raw - p));
        self.prev = Some(y); io.filtered = y; io.valid = true;
        Ok(())
    }
}
```

`trust module declare` regenerates the digest-pinned declaration; ST calls
it like any FB and supervises it with ordinary logic
(`IF flt._STATE <> MOD_RUNNING THEN …`). The PLC programmer's world is
untouched; the algorithm is Rust with tests.

### 16.3 Advisory service with honest staleness (rung 0)

Carried from v1 §9.3 abridged: an anomaly-scoring service (ONNX model, any
crates) subscribes to cycle-coherent snapshots, writes
`GVL_Svc.ANOMALY_SCORE` + heartbeat; the PLC applies advice only while
`sup.FRESH`, falls back locally otherwise, and Devices & Connections shows the service
node's liveness from the runtime's registry — a down service is a visible,
alarmed condition, never silent staleness (RS-49..54). Zero scan risk; the
natural first rung for a Rust team meeting truST.

---

## 17. Industry landscape and positioning

Re-verified 2026-07-03 against primary sources (vendor docs/press, GitHub
orgs); confidence flags noted where a claim rests on absence-of-evidence.

### 17.1 The incumbents, precisely

- **Beckhoff TwinCAT 3.** User code in the RT context = **TwinCAT C++**
  (TC1300, cyclic TcCOM modules); Beckhoff's own framing keeps C++
  subordinate ("the algorithm in C++, the sequence control in a PLC
  language"). **TwinCAT PLC++** (announced 2024-11, expanded 2025-03, GA
  ~end-2025) is *not* C++: it is a next-generation **IEC 61131-3** compiler/
  runtime — faster, plain-text projects, CLI compiler for CI/CD, an AI
  assistant. No first-party Rust found as of 2026-07-03 (no docs, repos, or
  announcements; the "Beckhoff Rust" artifacts we found are third-party ADS
  client crates).
- **CODESYS.** C integration exists only as subordinate modules called from
  an IEC application; the vendor's own docs state the C code "cannot
  interact with the runtime system or the IEC application" beyond passed
  parameters, and there is no C debugger. No hard-RT guarantee documented
  for C modules; no Rust activity found. No CODESYS product lets the
  primary application be a general-purpose language.
- **Phoenix Contact PLCnext.** The most polyglot RT story among the majors:
  ESM schedules IEC, **C++**, C# (eCLR), and Simulink programs in real
  time, mixed in one task, exchanging via GDS. Rust exists only as an
  official *sample-runtime tutorial* — a Linux process beside the PLC using
  the ANSI-C boundary (our tier 3 + C ABI, essentially), not an ESM
  language.
- **Bosch Rexroth ctrlX OS.** Snap-based apps in any language — but
  categories 1/2 (everything in the public SDK) are **non-real-time**;
  RT extension is Category 3, gated behind vendor training/partnership.
  The PLC itself is CODESYS-based. No first-party Rust SDK/examples found
  as of 2026-07-03; a community request was answered with C/C++ bindings
  only (Python/Go "in preparation" at the time), though ecosystem Rust
  discussion exists.
- **Siemens.** ODK 1500S runs user **C++** blocks inside the S7-1500
  software-controller cycle (synchronous mode) — callable *from* an IEC
  program, never the program itself; narrow SKUs, paid license. **SIMATIC
  AX** is the workflow modernization: VS Code, git, package manager, unit
  tests, CI/CD — now covering IEC-language authoring such as ST and XLad
  ladder, not Rust-first scan-native authoring.

**Pattern**: every vendor admits exactly one general-purpose language into
the RT path — C/C++ — always subordinate to an IEC host, always inside the
TCB, never with admission, sandboxing, dependency governance, or replay.
And the two most modern moves in the industry (TwinCAT PLC++, SIMATIC AX)
are *workflow* modernizations around existing automation languages rather
than Rust-first scan-native authoring — independent confirmation of §1.2's
thesis that repo-first workflow is where the market is going, while the
language/runtime contract remains the open gap this design targets.

### 17.2 The Rust-native neighbors

- **Embassy / RTIC**: MCU firmware frameworks (async executors, Cortex-M
  concurrency); real production traction (automotive), **zero PLC
  positioning** — the §1.3 layer distinction, verified.
- **RoboPLC** (Bohemia Automation): the nearest neighbor — a Rust
  code-first control framework for Linux with a Ferrocene/IEC 61508 angle.
  It is a *framework library*, not a PLC platform: no IEC 61131-3
  compatibility in either direction, no process-image/forcing/HMI/
  commissioning stack, no admission/replay story. Watch it; it validates
  demand for exactly the developer truST targets.
- **RuSTy** (PLC-lang, active): ST→LLVM compiler frontend in Rust —
  a toolchain, not a runtime. **IronPLC**: self-declared prototype.
  No open-source Rust IEC runtime with traction exists.
- **OPC UA in Rust**: no production-ready server per specialist assessment
  (2025); the active crate is `async-opcua`. Relevant only as ecosystem
  context for tier-3 services.

### 17.3 The trend line

"Software-defined automation" is now a funded category (Software Defined
Automation GmbH: industrial DevOps, vPLCs, git for PLC code, browser IDEs;
Rockwell marketing the same phrase). The IT-engineer-into-OT migration this
spec bets on is visibly underway — but every player modernizes *around* the
IEC languages.

**The market claim, phrased as we can defend it (normative wording for all
product copy):**

> As of 2026-07-03, we have not found a shipping PLC platform that combines
> Rust-first PLC authoring, scan-native Rust POUs, timing admission, crate
> admission, cycle replay, generated ST/IEC compatibility, and optional
> sandboxing as one coherent product.

Do **not** compress this to "nobody has any Rust story": PLCnext ships an
official Rust sample runtime (an adjacent process over the C API — §17.1),
ctrlX has community Rust discussion, and RoboPLC exists (§17.2). The
differentiator is not the word Rust; it is the **product contract** around
it. Absence claims are dated, scoped ("we have not found"), and re-verified
before external use. (WASM-in-PLC likewise: research interest exists, no
shipping runtime found — truST's sandboxed form would plausibly be first
there too, same phrasing discipline.)

### 17.4 Positioning sentence

For product copy, the claim this architecture can back mechanism-for-
mechanism:

> truST brings **Rust-first PLC development with generated IEC 61131-3
> compatibility**: machine logic authored in Rust, `cargo test` against a
> simulated machine, timing and dependency admission at build, and every
> cycle recordable and replayable — on a production PLC runtime with the
> full operations surface (I/O, HMI, forcing, deploy, fleet) built in.

---

## 18. Decision log

D1–D13 carried from v1 (statuses updated); D14+ new in v2.

| ID | Decision | Status / v2 note |
|----|----------|------------------|
| D1 | Rust POUs via generated, digest-pinned ST declarations | Carried; digest chain widened to configuration + device imports (RS-85) |
| D2 | One module model; tier = call site | Carried unchanged |
| D3 | Native = cdylib + versioned C vtable + exact-fingerprint handshake | Carried; see D23 for the packaging split |
| D4 | Sandbox = wasmtime (epoch + optional fuel) | Carried; §17.3 notes no shipping PLC runtime with this full sandboxed form was found as of 2026-07-03 |
| D5 | Copy-in/copy-out exchange blocks | Carried (zero-copy still Q5) |
| D6 | Panic caught at boundary; `panic="abort"` refused | Carried; §3.2 re-verified the no-catch_unwind baseline it fixes |
| D7 | Wall-clock deadlines, per-form honest enforcement | Carried; now wired through the dormant deadline API + `record_call` seams (§8.2) |
| D8 | Services ride the existing control plane + typed quality SDK | Carried; protocol described precisely (JSON-line, no subscribe — RS-99) |
| D9 | `trust module build` pins toolchain | Carried |
| D10 | Generated status outputs on every external POU | Carried; per-invocation durations stay in telemetry, not exchange (§6.5) |
| D11 | Defaults: `output_policy=default`, `deadline_action=fault-module` | Carried |
| D12 | Runtime-internal naming: **external POU** (never "native") | Carried |
| D13 | Module signing rides the deploy-key keyring | Carried |
| D14 | **Rust-first surface = trust.toml + macros generating ST/bundle files; one compile path, two doors** | Alternatives: a Rust-only runtime config parser (second universe — rejected: forks scheduler/tooling semantics); ST-only composition (v1 P1 — rejected: fails the product goal). Generated-ST keeps every existing tool working and the machine auditable |
| D15 | **Timing admission with graded evidence, as a build/deploy gate — computed by worst-frame/hyperperiod simulation of the actual non-preemptive single-thread scheduler (RS-115..117)** | Alternatives: pretend WCET (dishonest); enforcement-only (v1 — leaves "does it fit?" unanswered until the plant); utilization/`ceil()` scaling (wrong for a non-preemptive frame scheduler — coincidence frames are the binding case, and per-task sums double-count frame-global costs — corrected 2026-07-03 second review); fixed-priority response-time analysis (reserved for a future preemptive scheduler, RS-117) |
| D16 | **Crate admission with layered evidence incl. measured tripwires** | Alternatives: pure static claims (over-promise — undecidable); nothing (the "any crate" trap). Measured alloc+syscall audit in the timing run is the honest differentiator |
| D17 | **Record/replay as core product** | Determinism contract already paid for (P2/P5/RS-43); Open-OT shm precedent bounds recorder cost; flight recorder default-on in sim, opt-in on target |
| D18 | **Cycle sequences: a restricted `#[trust_sequence]` language in Rust's await syntax — build-time validated and lowered to an SDK state machine, polled once per cycle, init-allocated/accounted, restart-from-top** | Alternatives: raw async fn with "sealed awaitables" (**unsound** — `.await` accepts any `IntoFuture`, std blanket-impls it for all futures; no trait can restrict it — corrected 2026-07-03 second review); stored Rust future created at init (**unsound shape** for current-cycle `Exchange`/`CycleCtx` access — would store stale borrows or require self-referential lifetime tricks; corrected 2026-07-03 third review); SFC-in-Rust DSL (a whole new language — rejected); nothing (leaves the worst ST pain unaddressed). Enforcement = validation (foreign awaits/async blocks/unknown macros rejected, F23) + explicit state-machine lowering + runtime deadline backstop; closure-scoped exchange access makes no-borrow-across-wait structural; general async stays in services; non-serializable state honestly excluded from retain (Q8) |
| D19 | **Typed I/O: manifest addresses + device-import codegen (ADS pipeline precedent), Rust structs as views over bound variables** | Alternative: a parallel typed-I/O runtime path — rejected (two I/O truths). Views keep one binding mechanism |
| D20 | **`trust-sim` productizes `TestHarness` for cargo test** | Alternative: bespoke test kernel — rejected; the production `execute_cycle` is the only honest simulator of itself |
| D21 | **User-facing SDK crate `trust-plc`; public traits `PlcProgram`/`PlcFb`; functions are stateless `#[trust_function]` exports** | Naming for the imported audience; ABI crate keeps internal name. No separate public `PlcFunction` trait is locked for v1 |
| D22 | **Rust→ST in-cycle calls deferred** | VM re-entry mid-module-call tangles budgets/reentrancy for a need task-order composition already covers; revisit with field evidence |
| D23 | **Exact fingerprint for source-built; ABI-line compatibility for precompiled packages, later, explicitly** | v1's exact-only stance kept for the default path; ecosystem packaging can't demand vendor rebuilds per rustc — but the relaxation ships as its own gated phase, never silently |
| D24 | **`trust` umbrella CLI + `cargo trust`** | Thin dispatch over existing verbs; no behavior forks; old spellings kept |
| D25 | **SDK distribution v1 = crates.io exact pins stamped by the scaffold** | `trust new rust-plc` writes exact `=x.y.z` pins for `trust-plc`/`trust-sim` matching the bundled `trust` tool; bundled local registry/offline SDK is deferred until a field need justifies the packaging cost. Version skew is refused by RS-126 |
| D26 | **Deploy target inventory is fleet-level infrastructure** | Projects reference target names; runtime hosts, credentials, and deployment inventory live in the same fleet/runtime inventory owned by Devices & Connections, not in committed project TOML |

---

## 19. Phased implementation plan

Phases are deliverable-gated, each shippable and useful alone; production
bar per the header (done = exit criteria green on trust-builder). v1's
P0–P5 ordering discipline is kept — semantics hardened in-tree before
dynamic loading — with the v2 surfaces slotted where their dependencies
land. Each phase names its integration seams (§3.7).

**Execution model — vertical slices, not subsystems.** Work is cut and
tracked in `rust-plc-implementation-board.md`; the phases below are the
capability inventory and exit-criteria framework the slices draw from. The
**first slice (S1) is normative**: *one Rust PROGRAM, scheduled by one
task, compiled through generated ST, executed through the module host
(static form), with the panic boundary, per-instance timing telemetry, and
one cargo-test machine simulation* — thin versions of codegen and
`trust-sim` included. Once S1 is green on trust-builder, the rest of this
plan is believable; until then, nothing else starts.

**P0 — Contract freeze.** This document reviewed; D14–D26 locked
(2026-07-03); ABI structs (App. B), manifest schemas (App. C), RS-118..126,
and BC-1..BC-6 integration seams frozen before their consuming slice starts.
`abi_major = 1`. *Exit:* sign-off; no unresolved decision blocks P1a.

**P1a — Module host + built-in POUs (tiers 1+2, static form).** New crates
`trust-module-abi` (no_std), `trust-plc`, module-host subsystem (registered
unsafe, RS-84). Seams: `execute_program_by_name` registry lookup;
`execute_native_call` EXTERNAL kind; `record_call` new kinds + armed
deadline API (§8.2); faults via `record_fault/apply_fault`; retain codec
module-blob section. Migrate the builtin FBs (incl. `ROBOT_P3MINIMALARM`)
onto the SDK traits — one native path, `catch_unwind` boundary lands (F1).
*Exit:* conformance suite green over built-in fixtures incl. failure
injection (F1/F2/F7/F10/F11); migrated builtins pass existing differential
suites unchanged; per-instance stats in live values.

**P1b — Cycle sequences (SDK layer, on the stable P1a surface).**
The `#[trust_sequence]` validator/lowerer + SDK wait points
(`next_cycle`/`until`/`wait`/`all`/`race`/child sequences), generated
`TrustSequence` state machines with per-poll `Exchange`/`CycleCtx` access,
closure-scoped exchange access (`s.read`/`s.write`/`s.retain_read`/
`s.retain_write`), loop-without-wait-point validation (RS-80), `TestBench`
sequence support, sequence-location diagnostics. Deliberately split from P1a so the
foundation is never hostage to the flagship (may overlap P2 in calendar;
depends only on the P1a SDK surface).
*Exit:* a sequence-based fixture demonstrates RS-75..80 (incl. F21) on sim +
target; the F23 rejection matrix passes (foreign `.await`, async
block/closure, executor API, hand future, unrecognized macro, wait-free
loop — each rejected with the construct named); restart-from-top semantics
demonstrated across warm restart.

**P2 — Native dynamic form + module toolchain.** cdylib ABI, loader +
handshake, `trust module new/check/build/declare/test/package`, signing via
deploy keys, the `{external}` pragma family in trust-hir (Open-OT
micro-syntax precedent), `.stbc` binding section, `RuntimeBundle` discovers
`modules/`; module RPCs in the role table; reload parity per RS-86.
*Exit:* out-of-tree module (§16.2) built, loaded, conformant; full skew
matrix (F8/F9/F12) diagnosed; **Demo B0 (brownfield acceptance) accepted** —
an *existing ST project* gains one Rust FB: generated declaration appears
in the LSP, ST calls it, live values show `_STATE/_FAULT_CODE`, an induced
fault is supervised from ST, per-instance timing appears in telemetry, and
**no Rust-first project feature is used anywhere** (full checklist in the
implementation board). This gate demonstrates we did not break PLC users
while chasing software engineers.

**P3 — Rust-first projects + machine sim.** `trust new rust-plc`,
`trust.toml` → generated configuration/declarations/io.toml/runtime.toml
(RS-60..68), `trust` umbrella + `cargo trust` (D24), **`trust-sim`**
(RS-109..111) with proptest integration (RS-112), protective template
defaults. *Exit:* **Demo G0 (minimal greenfield demo) accepted** — a
deliberately tiny end-to-end acceptance demo (motor-latch scale): 1 task, 1 Rust
PROGRAM, 2 inputs, 1 output, 1 retain variable, 1 cycle sequence, 1
`trust-sim` cargo test, a development-grade admission report (declared
evidence; re-run measured at P4), and a reviewable generated-ST diff —
the whole toolchain visible in one small project (full checklist in the
board). Additionally: §16.1's palletizer builds/tests/sims/deploys with
zero hand-written ST; generated-ST diffs stable under regeneration; mixed
Rust/ST project demonstrated.

**P4 — Timing admission.** `.trusttime`, worst-frame/hyperperiod frame
simulation + itemized worst-frame reports (RS-87..90, RS-115..117),
measurement harness on target (RS-91), deploy gate + drift diagnostics
(RS-93). `static_stbc` is **P4b**, explicitly separate (bytecode cost model
+ loop-bound pragmas, RS-88). *Exit:* an inadmissible task set refused with
the §9.3 worst-frame account naming coincident tasks; a task set that
passes per-task-in-isolation but fails on a coincidence frame is refused
(the regression test for the corrected model); measured evidence bound to
digest+target; drift diagnostic demonstrated against an induced regression.

**P5 — Crate admission.** Knowledge base, static + symbol scans, tripwire/
syscall audit integration with the P4 harness (RS-92), `.trustcrate`,
policy + deploy gate (RS-94..97). *Exit:* `reqwest`-in-RT-path refused with
named call path; math crate admitted via measurement; report in build
output + IDE problems.

**P6 — Service SDK + typed I/O.** `trust-service-sdk` (quality metadata,
polling subscriptions), `[services.*]` codegen + supervisor template
(RS-98), `ServiceLink`, Devices & Connections liveness; `trust io import` for ADS (Rust
bindings beside the existing ST generation) and the EtherCAT variant
(RS-73..74). *Exit:* §16.3 running against a live runtime with staleness
fallback in acceptance evidence; device import round-trip (symbol-version
drift warning) demonstrated.

**P7 — Record & replay.** `.trusttrace` recorder (window + flight-recorder
modes, shm-backed publication), `trust replay/diff/explain-overrun`,
replay assertions in `trust-sim`, trace-linked diagnostics (RS-101..108).
*Exit:* commissioning trace recorded on target replays in `cargo test`;
induced overrun explained naming the module; old-vs-new diff gates a
deliberate regression in CI.

**P8 — Sandboxed form.** wasmtime host (RS-45..47), epoch/fuel, `--form
wasm|both`, perf budget validation, deterministic fuel replay. *Exit:* one
module source conformant in both forms; wasm hard-loop trap contained while
the task continues (F4); fuel-bound replay demonstrated.

**P9 — Productization.** Public docs (`docs/public/develop/` + section
`index.md` links), example gallery (§16 as shipping examples), operator/
module-author/service-author/migration guides, acceptance-board journeys
for the Rust workflows, positioning page (§17.4). *Exit:* an external Rust
developer builds a useful controller from docs alone; acceptance rows
`ux_accepted` per board process.

**Repository obligations (every phase, carried from v1 verbatim in force):**
new crates enter with full hygiene (xtask full-map `allowed_workspace_edges`
— ABI/SDK crates forbidden from depending on the host; `deny.toml`;
public-API snapshots for SDK/ABI; MSRV = workspace 1.95); new validators as
new per-slice modules, never appended to a god-file; execution-flow changes
update `docs/diagrams/**/*.puml` + manifest; public docs added to section
`index.md`, not just nav; nothing is "done" before `just fmt/clippy/
test-all` is green on trust-builder.

Risks (v1 table carried) plus v2-specific:

| Risk | Mitigation |
|------|------------|
| Rust-first codegen drifts from hand-ST semantics | One compile path (RS-60) makes drift structurally impossible; generated-ST diffs are reviewed artifacts |
| Admission theatre (numbers nobody trusts) | Evidence grades visible everywhere; drift diagnostics close the loop against reality (RS-93); hard-loop/overrun tests keep enforcement honest |
| Crate classifier over-claims | Verdicts carry evidence class; static findings say "observed/declared", never "proven"; measured tripwires are the load-bearing tier |
| Sequences become a foot-gun (hidden state machines) | Validated wait-point subset (foreign awaits/async blocks/unknown macros rejected — F23), runtime deadline backstop, restart-from-top, RS-80 validation, sequence location in diagnostics, SFC-style step view in IDE |
| Sequence validator has blind spots (macro opacity, future language changes) | Conservative rejection of anything unrecognized; the F23 rejection matrix in CI; runtime deadline/budget backstop means an escapee parks and faults — it cannot block the scan |
| Scope: two doors double the surface | The doors share one pipeline and one test suite by construction; brownfield examples ship in the same phases as the mechanisms they use |

---

## 20. Open questions

| # | Question | Owner | Default if unresolved |
|---|----------|-------|----------------------|
| Q1 | Event tasks (`SINGLE`) for module programs in v1 scope? Runtime supports them; trust.toml exposes them (§5.2) | product | expose; conformance covers rising-edge semantics |
| Q2 | Minimum native target set (musl? Windows runtime?) | platform | linux-gnu aarch64 + x86_64 |
| Q3 | `FUNCTION` purity: structural only, or lint-verified too? | compiler | structural + lint |
| Q4 | (superseded by D23) | — | — |
| Q5 | Zero-copy exchange for large arrays (vision ROI, point clouds) | runtime | copy until profiles demand; then `#[view]` addendum |
| Q6 | Dedicated shm data plane for high-rate tier-3 exchange | runtime | control plane until a workload exceeds it; generalize the Open-OT publisher then |
| Q7 | Module-contributed `VAR_GLOBAL`s | compiler | not in v1; globals stay configuration-declared |
| Q8 | Resumable sequences (checkpoint labels persisted via retain, resume at label instead of top) | SDK | restart-from-top only; revisit with field demand |
| Q9 | crates.io namespace/curation for PLC FB libraries (`trust-fb-*`?) and knowledge-base governance | product | convention doc in P9; KB in-repo |
| Q10 | HMI authoring from Rust (screens-as-code) | product | out of scope; symbols/units flow to existing HMI |
| Q11 | `trust-sim` on macOS/Windows dev hosts | runtime | Linux-first; audit the sim path's OS surface in P3 |
| Q12 | Should graceful stop optionally apply safe-state outputs (platform gap, §3.2)? | runtime (platform-wide) | raise as its own platform proposal; Rust-first templates already mitigate via SafeHalt defaults |
| Q13 | Full online change (generation swap + state migration) and stbc-only deploys (drop the src/ recompile requirement, §3.1) | runtime (platform-wide) | separate spec; RS-86 baseline until then |
| Q14 | Sequence step visualization: derive an SFC-like diagram from await points for docs/HMI? | tooling | IDE "current step" only (§15.2); diagram export later |

---

## Appendix A — IEC 61131-3 ⇄ Rust type mapping

Unchanged from v1 (normative): `BOOL⇄bool` (validated), signed/unsigned
widths ⇄ `i8..i64`/`u8..u64`, bit-strings ⇄ `Byte/Word/Dword/Lword`
newtypes, `REAL/LREAL⇄f32/f64` (no fast-math profiles, RS-44),
`TIME/LTIME⇄IecTime(i64 ns)`, date family ⇄ `IecDateTime`,
`STRING[N]⇄IecString<N>`, `WSTRING[N]⇄IecWString<N>`,
`ARRAY[a..b] OF T⇄IecArray<T,N>` (row-major, bounds in digest),
`STRUCT⇄#[trust_struct] repr(C)`, enums ⇄ `#[trust_enum] #[repr(i32)]`
(out-of-range copy-in ⇒ protocol fault). Rejected in interface position:
references/pointers, heap types (`String/Vec/Box/Rc/Arc/HashMap`), trait
objects, closures, futures, unbounded slices — build-time errors (RS-14).
v2 adds: `#[unit]`/`#[range]` metadata carried beside types (RS-72);
`ServiceLink<T>` marshals as its underlying bound variables + quality enum
(the ADS `_quality` convention, §3.5).

## Appendix B — Native module ABI definition

Unchanged from v1 (normative): single export
`trust_module_entry_v1() -> *const TrustModuleDescV1`; `repr(C)`,
size-first, additive-minor structs; `ModuleIdent` + `BuildFingerprint`
(SDK ver, rustc ver+hash, triple, panic strategy, profile, lockfile hash);
BLAKE3 `iface_digest`; export table with per-POU vtables
(`create/init/cycle/exit/destroy/save/load`, `TrustCallStatus{Ok,Fault,
Panicked}`, retain_version + reserved space); every wrapper `catch_unwind`;
loader validates symbol → size → abi_major → fingerprint (per D23 regime) →
digest → limits, in order, refusing before any user code runs. The full
struct listing from v1 App. B is authoritative and lives in
`trust-module-abi` once P1a lands; the archived v1 file retains the
long-form listing.

## Appendix C — Manifest schemas (trust.toml, module.toml)

**`trust.toml`** (Rust-first projects; §5.2 shows a worked instance):

```toml
[project]        name, profile = "development|production|hard-rt"
[runtime]        cycle_interval?, watchdog{timeout,action}, fault{policy},
                 retain{mode,path?,save_interval}, realtime{…}   # → runtime.toml
[tasks.<name>]   period XOR event (+ optional min_interarrival for event
                 tasks — RS-116), priority, programs = ["mod::Type", …]
[io]             "<instance>.<field>" = "%<area><size><addr>"     # → VAR_CONFIG
[io.drivers.<d>] driver config (schema-checked)                   # → io.toml
[outputs.safe_state]  "%Q…" = value                               # → io.toml safe_state
[timing]         require_evidence, jitter_margin, per-export overrides (downward)
[crates]         rt.deny_unknown, rt.deny_service_only, allow/deny lists,
                 licenses, advisories, unsafe budget
[services.<n>]   binary, heartbeat, timeout, on_stale
[modules.<n>]    prebuilt module bindings (brownfield-style, optional)
```

**`module.toml`** (v1 App. C carried, one clarification per RS-65): it
attaches *policy* to exports — `[limits]` (cycle_deadline, deadline_action,
init_deadline, state, scratch, memory), `[trust]` (form, conformant,
signature), `[retain]` (on_version_mismatch), per-export `singleton` — and
never restates interfaces; a policy key naming an unknown export is a build
error.

**Containers**: `.trusttime` (timing admission record), `.trustcrate`
(dependency evidence), `.trusttrace` (cycle recording) — all sectioned,
CRC'd, digest-keyed to module/bundle/target identity, following the
`STBC`/`STRN` container discipline (§3.4/§3.6).

## Appendix D — Glossary

v1 glossary carried (module, module host, exchange block, declared
interface, form, tier, TCB, fingerprint, conformance suite, external POU,
`CALL_NATIVE` disambiguation, built-in module). v2 additions:

| Term | Meaning |
|------|---------|
| Door | Authoring surface: brownfield (hand ST + modules) or greenfield (Rust + trust.toml, generated ST). One compile path serves both |
| Cycle sequence | A `#[trust_sequence]` body: a restricted, build-validated sequence language in Rust's await syntax, lowered to an SDK `TrustSequence` state machine with per-poll process-image access (D18). Not general Rust async — that lives in services |
| Wait point | An SDK-admitted suspension in a sequence body (`next_cycle`/`until`/`wait`/`all`/`race`/child sequence) — the only awaits the validator accepts (RS-76) |
| Admission | Build/deploy-time acceptance of a task set (timing, §9) or dependency graph (crates, §10) based on graded evidence |
| Evidence grade | `declared` / `measured` / `wasm_fuel` / `static_stbc` / `builtin_maintained` — how much a budget claim can be trusted |
| Admission drift | Production p99 exceeding the admitted budget; a diagnostic, raised before the watchdog would fire (RS-93) |
| Flight recorder | Always-on bounded ring of recent cycles, dumped on fault (RS-102) |
| Machine test | A `cargo test` driving the in-process simulated controller via `trust-sim` (RS-109) |
| Device view | A `#[trust_device]` struct: typed Rust access over imported, name-bound variables (RS-73) |
| ServiceLink | Typed, freshness-explicit consumption of service-written variables inside PLC logic (§6.6) |

---

*End of specification v2.4. The archived v1 retains the long-form ABI listing
and the original worked examples; where the two documents disagree, v2.4
governs.*
