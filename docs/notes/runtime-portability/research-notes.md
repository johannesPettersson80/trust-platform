# Runtime portability research and review notes

These are dated, non-normative research records. The current product plan is
[specification 34](../../specs/34-runtime-portability.md); it supersedes decisions
in older review records. Historical artifact hashes establish provenance, not
runtime qualification or a build dependency on a personal Downloads directory.
No historical document-check result is relabeled by this archive move.

The registry snapshot is retained unchanged beside these notes. Refresh its
candidates when the separately authorized modernization scope begins; it is not
a continuously current dependency policy.

## Design inputs and earlier provenance

**D01 — Prior proposed specification.** `trust-runtime-portability-spec-v0.2.md`, dated 8 October 2026. SHA-256: `2190f69e2aec870c00b2811b0fc2a657799c1ecc0ea13384a7f429f2e4ecf5e6`.

**D02 — Research review.** `trust-runtime-architecture-research-review-2026-10-08.md`, dated 8 October 2026. SHA-256: `4adec9138e8e572b9c13c43d845c6c77f87639bb64b075c217b36952239fd19d`. Its research identifiers are mapped in Appendix C; its R01/R02/R03 source numbers are local to that review, not replacements for this document's repository register.

**D03 — Immediate input and user decisions.** `trust-runtime-portability-spec-v0.3.md`, dated 8 October 2026. SHA-256: `86aeda81d201a66777457e9a8bf7d011c7a4960b445d777608a5a11090f7e3ce`; retained unchanged alongside this revision. The 9 October conversational review supplied five implementation recommendations, and the user then requested their incorporation and confirmed possession of a NUCLEO-F401RE. This is a reported board identity, not probe-read device evidence or a physical qualification.

**D04 — v0.5 input and scope decision.** `trust-runtime-portability-spec-v0.4.md`, dated 9 October 2026. SHA-256: `b6b6dee94e088198dab2b84f24a5331091c82e2a2b07a05833d28c98c128aecf`; retained unchanged. The user requested current Rust/packages and researched design patterns, explicitly selected “Update the specification first,” and supplied the Rust Design Patterns URL. This does not authorize repository dependency installation or implementation. The separate v0.4 review-notes file retains its original document-check result; this revision does not overwrite or relabel that evidence.

**D05 — Direct-dependency research snapshot.** [trust-runtime-modernization-inventory-2026-10-09.json](trust-runtime-modernization-inventory-2026-10-09.json), SHA-256: `6a088873d343cced6c536442fdd32e63cf0687017c64bf62ad12cea4d1e9f6d0`. Read-only manifest/lockfile research and registry metadata for 74 distinct direct Cargo registry crates and 30 distinct direct npm packages, plus five pinned Git dependency declarations, from 21 nonignored first-party package manifests. Every registry entry records its queried crates.io/npm URL; Cargo metadata includes reported minimum Rust, features, source, and release date, and npm metadata includes engines/peers. Root Cargo lock versions can include older transitive versions of the same package; they are not all direct selections. This snapshot performs no dependency resolution, installation, advisory scan, build, or compatibility verification.

**D06 — Immediate input and independent review.** v0.5 SHA-256: `aa5ac776175f33256a0524a72977ff4cdbae85771509ca15032ef7853f8052e2`, retained in Downloads together with its original validation notes. The user supplied a 17-finding review of v0.4 (F1–F17), reporting source/toolchain/document inspection and no builds or hardware. This revision rechecked the cited decision-critical code, current product specifications, and E56–E62. Appendix G distinguishes adopted changes from corrections; it does not convert the reviewer's tool observations or checklist history into newly executed validation. The previous D13/REV-MODEL-05 document-scanner results remain in their original notes and are not overwritten by this revision.

## Revision history through v0.6

| Version | Date | Change |
|---|---|---|
| 0.1 | 6 October 2026 | Initial repository-grounded core/platform architecture, MCU migration, and acceptance specification. |
| 0.2 | 8 October 2026 | Makes Linux/Windows/macOS adoption mandatory; adds hosted profiles, feature-preservation rules, compiler-free loading on every target, shared bounded execution, explicit service/persistence boundaries, native hosted CI and qualification, and measured resource/performance gates. Retains the original reviewed repository baseline. |
| 0.3 | 8 October 2026 | Integrates the research review: versioned cooperative scheduling and blocking/output admission; distinct artifact/prepared-module/state ownership; generation-scoped hosted online change; compactness and first-use bounds; typed clocks and race-safe wake contracts; numeric/control equivalence; validated construction and application authorization; early host/STM32 checkpoint; expanded per-platform disturbance and lifecycle qualification. Retains Rust/STBC, shared hosted/MCU core, OOP, existing hosted capabilities, and the original repository baseline. |
| 0.4 | 9 October 2026 | Selects the user-confirmed NUCLEO-F401RE for initial feasibility; makes ESP32 mandatory with M1/M2E early evidence and M5 qualification; adds compiler-free initialization mapping, storage/atomic decisions, portable math selection, target appendices, and concrete implementation acceptance. Rechecks relevant source at `9a150657`; preserves existing semantic contracts and the user's consolidated validation cadence. No runtime implementation or hardware fit/timing claim. |
| 0.5 | 9 October 2026 | Adds a proposed Rust 1.99/edition 2024/resolver 3 and Node LTS baseline; all-project dependency/tool dispositions, dated registry candidates, migration risks, and modernization acceptance. Integrates Rust Design Patterns/API/Embedded Rust guidance into concrete ownership, construction, port, and state choices. Keeps numerical semantics, required targets, shared execution, full hosted scope, and one-batch validation. Specification/research only; no dependency or runtime changes. |
| 0.6 | 9 October 2026 | Integrates the plan as specification 34 and reconciles specifications 7/11/12; evaluates F1–F17. Proposes C6/IMAC first and C3 later, shared-bytecode initializers under an explicit 2.0 contract, separate host/board scopes, native-stack/profile bounds, shared optional register tier, specified persistence, qualified numerics and f32 optimization conditions. Adds concrete allocation sites and milestone ownership; retains modernization scope. No implementation, dependency upgrade, or target qualification. |

## Earlier research amendment traceability

The research review's proposed `REV-*` identifiers are review labels, not product requirement IDs. The following document requirements supersede that proposal text. “Incorporated” means specified here, not coded or tested. Section 13.2 assigns interface decisions, bring-up work, bounded-profile mechanisms, and release qualification to milestones; it replaces the earlier unassigned P0/P1 terminology.

| Review amendment | Integrated specification requirements/sections | Required evidence |
|---|---|---|
| REV-SCHED-01 | Section 4.4, SCHED-01, CYCLE-01/02/03 | Existing scheduler/I/O behavior locks on the named cooperative profile. |
| REV-SCHED-02 | SCHED-02, TIME-01/02, Section 8.4 | Blocking-aware application/platform admission record. |
| REV-SCHED-03 | SCHED-03, PORT-04, CYCLE-02, TIME-03 | Separate computation, batch, transfer/feedback response observations. |
| REV-SCHED-04 | SCHED-04, LOAD-08 | Rejection of incompatible claims; explicit measured versus analytical classification. |
| REV-MODEL-01 | Section 3.4, MODEL-01, LOAD-01/07 | Same admitted STBC, separately owned module/state, compiler-free loading. |
| REV-MODEL-02 | MODEL-02, HOST-04, MEM-09 | First-use/rare-target execution has no late preparation on bounded paths. |
| REV-MODEL-03 | MODEL-03, MEM-01/02 | Per-representation storage and preparation/RUN peaks. |
| REV-MODEL-04 | MODEL-04, Section 6.4 | Borrowed/owned lifetime and selected-board memory availability evidence. |
| REV-MODEL-05 | MODEL-01/04, ARCH-03, LOAD-03/05 | Checked decoding and portable handles; no native-layout or unsafe bypass. |
| REV-MEM-01 | MEM-01/02, PERF-02, TEST-02 | Per-region program/metadata/state/stack/update footprint. |
| REV-MEM-02 | MEM-06/07/08 | Compound copy, aliasing, OOP, and reference-lifetime tests. |
| REV-MEM-03 | MEM-06, TIME-02, PORT-02 | Variable-cost opcode/import capacity and work bounds. |
| REV-MEM-04 | MEM-04/09, TEST-01 | First use, maximum sizes, rare/error branches, last-owner destruction. |
| Section 3.4 of review — online-change replacement | MEM-07, HOST-03, SERVICE-02, UPDATE-01 through UPDATE-08 | Boundary/warm-restart parity; generation handles, active readers, overlap exhaustion, and off-path retirement. |
| REV-TIME-01 | PORT-01, Section 7.3, TIME-07 | Domain separation, wrap/reset/discontinuity, injected logical-time tests. |
| REV-TIME-02 | LOAD-06, PORT-02, TIME-02/03 | Callback bounds and blocked-import/driver response. |
| REV-TIME-03 | PORT-02, TIME-03/04 | Supervision when code does not return to a VM check. |
| REV-TIME-04 | PORT-03, Section 7.3, TEST-01 | Wake-token races, expired waits, idle STOP response. |
| REV-NUM-01 | NUM-01, COMPAT-05 | Ordinary/exceptional values, conversions, hardware/import modes. |
| REV-NUM-02 | NUM-02, COMPAT-04/06 | Exact observations under matching numeric/environmental contracts. |
| REV-NUM-03 | NUM-03/04, COMPAT-05/06 | Numeric error plus threshold/Boolean/alarm/state-transition outcomes. |
| REV-TEST-01 | TEST-01, Section 14.1/14.2 | Mixed-rate, rare/limit, I/O, time, storage, update, and overload corpus. |
| REV-TEST-02 | TEST-02, PERF-02/03 | Raw memory/timing evidence with precise completion labels. |
| REV-TEST-03 | TEST-03, TIME-05, PERF-04 | Extended soak/disturbance plan; no duration-to-WCET or unsupported failure-rate inference. |
| REV-TEST-04 | TEST-04, BUILD-08, Section 14.5 | Separate native OS/architecture and physical board qualification. |
| REV-PLAN-01 | PLAN-01, M2A/M2B | Representative saved artifact on fresh host and real STM32 before broad cleanup. |
| REV-LOAD-01 | LOAD-07/09 | No raw/partial-candidate executable construction or production bypass. |
| REV-LOAD-02 | TEST-05, LOAD-03/04 | Fuzz/mutation coverage of bytecode and bound metadata. |
| REV-LOAD-03 | DEPLOY-06, LOAD-03/06, TEST-05 | Application authorization/identity independent of firmware integrity. |

The research did not justify replacing Rust/STBC, removing statically allocated OOP, splitting desktop and MCU semantics, dropping hosted features, or making preemptive scheduling/native compilation/live MCU updates prerequisites. Those scope boundaries remain in Sections 1 and 15.

The 9 October recommendations are integrated as follows:

| Recommendation | Requirements and plan location | Required evidence |
|---|---|---|
| Required, early ESP32 target | ARCH-10, BUILD-09, PLAN-02, M1/M2E/M5, Appendix D.2 | Isolated target build, common saved-artifact execution, and separate physical qualification. |
| Compiler-free executable initialization | LOAD-10/11, M2A/M2B, Appendix E | Fresh-engine construction; dynamic local/default/compound/static/FB initialization and restart cases. |
| Storage ownership and transitive atomics | MEM-10/11, BUILD-09, Appendix E | Both target graphs, bounded storage/copy/drop behavior, preserved host ownership guarantees. |
| Concrete board/stack choices | Section 1.2, M0, Appendix D | Recorded physical revisions, pinned stack, per-region budgets, I/O/supervision/deployment evidence. |
| Concrete numeric implementation | NUM-05, M0/M2A/M2E, Appendix E | Pinned math implementation, numeric/control oracle, helper size and observed execution cost. |
| Current Rust and packages | MOD-01 through MOD-06, BUILD-03/09, M0U, Sections 14/16, Appendix F | Complete dependency/tool dispositions, reproducible versions/features, affected native/MCU/WASM/extension evidence, explicit retained constraints. |
| Useful modern features without numerical drift | MOD-03/05, NUM-01 through NUM-05, PERF-01 | Reviewed edition/API changes, preserved saved PLC observations, bounded formatting/storage, separately attributed compiler and engine effects. |
| Clear Rust design from the start | DESIGN-01 through DESIGN-03, Section 17 | Validated construction, typed clocks/handles, explicit ownership and transitions, small adapter interfaces, relevant native assertions and architecture checks. |

## Dated dependency candidates

The following are selected entries from D05, consulted **9 October 2026**, against repository HEAD `9a15065725c17da2c912055f1509368d3fd01d6c`. They make the migration scope concrete; they are not an approved compatible upgrade set. “Candidate” means the crates.io latest stable release or npm `latest` tag observed in that snapshot. Refresh metadata before implementation, inspect release/migration notes, then freeze the chosen versions. The JSON records all 74 direct registry crate names and 30 npm package names, including packages not shown here.

| Package | Relevant current locked version | Observed candidate | Main review concern |
|---|---|---|---|
| `indexmap` | 2.14.0 | 2.14.2 | Portable features, allocation, and deterministic iteration requirements. |
| `rustc-hash` | 2.1.2 | 2.1.3 | Collection behavior and `no_std` feature selection. |
| `thiserror` | 2.0.18 | 2.0.21 | Error API/formatting and portable dependency graph. |
| `smol_str` | 0.2.2 | 0.3.6 | Breaking-family migration; `Arc` remains and must not dictate core ownership. |
| `salsa` | 0.26.1 | 0.28.5 | Query API, invalidation, cancellation, diagnostics, and performance. |
| `time` | 0.3.47 | 0.3.55 | Parsing/formatting/features; keep PLC clocks explicitly modeled. |
| `tokio` | 1.52.1 | 1.53.2 | Host service compatibility and feature breadth; absent from the MCU core. |
| `wasm-bindgen` | 0.2.121 | 0.2.129 | Matching build/CLI glue and actual browser analysis behavior. |
| `rapier3d` | 0.32.0 | 0.36.0 | API and simulation/determinism migration, separate from PLC numerics. |
| `zenoh` | 1.7.2 | 1.10.1 | Exact pin, wire/mesh compatibility, transport features, and advisory paths. |
| `opcua` | 0.12.0 | 0.12.0 | Latest version does not resolve the repository's recorded maintenance exceptions. |
| `typescript` | 5.9.3 in both consumers | 7.0.2 | Major compiler/tooling migration; extension and frontend builds/behavior. |
| `vite` | 7.3.1 | 8.3.4 | Major bundler migration, Node requirements, and browser/WASM assets. |
| `react` | 19.2.4 | 19.3.0 | Compatible React DOM/types and actual extension webview workflows. |
| `eslint` | 10.0.2 | 10.12.0 | Node support, config/plugin peers, and reviewed diagnostics. |
| `vscode-languageclient` | 9.0.1 | 10.1.2 | Major client API and extension-host/LSP workflows. |
| `@playwright/test` | 1.59.1 | 1.64.0 | Matching browser installation and capture/test behavior. |
| `@types/node` | 25.3.2 | 26.6.4 | Registry candidate only; select typings for the real minimum runtime, not the highest number. |

The same root Cargo lockfile also contains older transitive lines for some names above, including `indexmap`, `rustc-hash`, and `thiserror`. The table selects the relevant modern direct line, while D05 preserves every matching locked version; neither implies that transitive duplication has been removed. Git and vendored dependencies require revision/provenance decisions rather than a registry-version lookup. No package manager command or advisory test has been run for this snapshot.

## v0.4 review disposition as recorded in v0.6

The supplied review targets v0.4; this revision builds on v0.5, preserving its modernization/design work. “Accepted” below means amended in the specification, not implemented or tested. Product support, exact floating equivalence, physical fit/timing, and package compatibility still require their named evidence.

| Finding | Disposition and resulting decision |
|---|---|
| F1 — Canonical specification and clock-only contradiction | Accepted. Register specification 34 and correct specification 11 §§5.3/6.1. Numeric accuracy is a product profile under IEC §6.6.2.5.8; the review's claim that IEC has no accuracy requirement needs qualification because that clause requires accuracy dependencies to be stated. Table 28 covers the transcendental functions; Table 29 covers arithmetic. |
| F2 — C3 atomics versus delayed storage redesign | Accepted. Propose C6/IMAC + bare-metal `esp-hal` for early M1/M2E. Retain the required ESP32 deliverable, defer optional C3 until atomic/string retirement, and measure temporary bring-up values. No board purchase/possession is asserted. |
| F3 — Workspace dependency defaults | Accepted and made concrete in BUILD-03. Audit all consumers and core std forwarding, not just the core manifest. Toolchain modernization remains requested but is not a prerequisite for this mechanical feature fix. |
| F4 — Core pow/trunc and platform math | Accepted inventory gap; qualified the alleged ARCH-01 contradiction, since v0.4 already had exact/tolerance rules. NUM-05 selects common libm provisionally and makes host adoption an explicit compatibility migration. Shared source does not establish “bit-identical by construction”; per-operation qualification remains required. |
| F5 — Dynamic initialization representation | Accepted same-dispatcher bytecode and TYPE_TABLE-driven construction, with explicit missing defaults/root records. Select a new 2.0 contract rather than an unexamined minor/optional extension, because current 1.x unknown sections are ignorable. Specification 12 §11 records the required representation and Scope A's detailed-format work before encoder/decoder changes. |
| F6 — M2A size and one-batch cadence | Accepted. Proposed separately authorized Scope A (host/core graph) and B (same fixture on F401); only both close M2A. The review's overlapping focused/full test list is deduplicated, and milestone names authorize no additional runs. |
| F7 — Native recursion and host-scale limits | Accepted. MEM-01/02/03 requires target layout bounds, profile decoder/operand/depth limits, native-stack headroom and F401 MSP evidence. Host tests or logical frame counts cannot substitute for board stack measurement. |
| F8 — Allocation inventory and dense IDs | Accepted. Appendix E names current allocation/refcount/name lookup sites and a dense POU/type/member/import-ID deliverable, including import bound metadata. |
| F9 — Register tier destination | Decided. Optional internal shared-engine plan with preparation-time lowering; host profiling outside. MCU can omit that representation. Preserve hosted tier regressions and actual benchmark baseline; no permanent legacy-host executor exemption. |
| F10 — Idle STOP and watchdog | Accepted. PORT-03 records the real hosted wake fix with native regression/release note; TIME-03 records the current 32-instruction stack polling and same-thread-watchdog limitation. |
| F11 — Retain/storage | Accepted with correction. F401 retains the last completed STOP checkpoint using a qualified internal-flash scheme; live last-scan power-loss guarantees need a stronger mechanism. Select bounded hosted snapshot/off-thread persistence with explicit durability/failure ordering. Verify sector layout from ST; defer partition sizes until linked evidence exists. |
| F12 — f32 basic arithmetic and ESP32 software float | Accepted conditionally. Permit equivalent f32 basic operations with applicable rounding/promotion hypotheses and differential evidence; do not extend the theorem to power/transcendentals or unchecked mixed conversions. Record software floating-point costs on C6 and optional C3. |
| F13 — Unassigned priority tags and late machinery | Accepted problem; replace unused P0/P1 terminology with the milestone ownership table in §13.2 rather than tagging every requirement. Preserve basic validation/ownership early; put full generation, admission, descriptors and deployment qualification in M3/release work. |
| F14 — Diagrams, architecture and release process | Accepted in §13.2. Apply concrete diagram paths, doctor checks/waivers and CHANGELOG/version rules in the implementation scope, not as a claim that planning shipped a feature. |
| F15 — Hosted compositions | Accepted. HOST-07/M2B explicitly requires absent feature/ownership boundaries; the Scope A core consumer does not qualify the full runtime-only host product. |
| F16 — R06 range | Corrected to the actual top-level file and included modules; removed the nonexistent line range. |
| F17 — Host wrapper bounds | Retained. Existing host `IoDriver: Send` and `Clock: Send + Sync + 'static` bounds are host-composition contracts, not compulsory core lifetimes or threading requirements. |

The review's confirmed cooperative cycle/readiness/order, fail-closed I/O, nested shared budgets, debugger boundary writes, single public execution backend, and native OS test matrix remain baseline behavior locks. This amendment does not weaken them or convert the review's source inspection into test evidence. The remaining work is implementation and qualification under the selected future scopes, including exact format layouts, numeric accuracy records, board reservations, and physical measurements.

## v0.6 review input

**D07 — v0.6 specification and follow-up review.** The canonical v0.6 document
has SHA-256 `a5ae759de17ed69811ee09d4b4b26181fff5edbd5baf388ede2fd62a9f34e9e6`.
Its existing Downloads export and original 22-check report remain unchanged.
The follow-up review contains eight findings on modernization scope, no_std map
hashers, bytecode relocation, compatibility lifetime, the leftover C3 composition,
libm features/numeric premises, provenance placement, and stateless functions.
Its evidence is reported source/registry inspection, not an executed build.
The v0.7 dispositions below record the additional current-source checks.

## v0.6 review dispositions

These dispositions describe the v0.7 document changes. They do not report code,
dependency, build, or hardware implementation.

| Finding | Disposition |
|---|---|
| 1 — Modernization scope | Accepted. Scope A and initial Scope B retain Rust 1.95.0, edition 2021 and resolver 2. M0U is separately authorized, normally after the host/F401 checkpoint. Performance attribution follows that order; any earlier modernization requires an explicit baseline/scope change. |
| 2 — Portable maps | Accepted. Name the no_std hasher/constructor migration in BUILD-03, including constant materialization and tests. Preserve std-exposed concrete map types through the alias's std definition; insertion order remains the behavioral contract. |
| 3 — Decoder/validator ownership | Accepted. Relocate the complete validator/decoder, byte-only module and metadata boundary; keep compiler lowering hosted. Include public host wrappers, optional serialization, CRC dependency features, the six version-assumption test surfaces and the wider format corpus. The low-level reader/helper already resides in core. |
| 4 — Version choice and window | Accepted. Keep 2.0 as a deliberate compatibility decision. Specify hosted dual-major release P, its patch cycle, and retirement at the next announced breaking feature release Q. Name actual P/Q versions before P ships. Source-free consumers reject 1.x. Missing 1.1 construction data requires original source/context; byte conversion alone cannot recover it. Retain the old fixture and add a separate 2.0 fixture. |
| 5 — Leftover C3 composition | Corrected to `trust-esp32c6-reference` / C6-DevKitC-1. |
| 6 — libm feature and premises | Accept default `arch` and record that 0.2.16 is already locked. Add explicit compiler/codegen, floating environment, library/features, sysroot soft-float provenance and observable-result premises. No measured host speedup or universal bit identity is inferred from feature selection. |
| 7 — Product-spec hygiene | Accepted. Remove session authorization/personal-board wording; move historical hashes, review mappings, candidate table and unchanged JSON into this non-normative archive, indexed by README. D05 retains its SHA pin in the specification. |
| 8 — Stateless functions | Restore the deterministic-stateless principle with a defined numeric/environmental contract. Qualify the unconditional per-build claim: Rust's unspecified-precision math contract permits variation even within one execution. Statelessness alone does not strengthen that guarantee. |

Additional verified source details: `BytecodeModule` is host-owned; byte readers
and alignment helpers are core re-exports; decode uses `crc32fast` whose default
std feature must be disabled for its new core edge. Version assumptions include
literal pairs as well as shared constants. The single tracked `.stbc` fixture is
the OSCAT fixture; the MQTT example artifact is not a tracked compatibility fixture.

## v0.7 revision record

9 October 2026: applied the eight-finding review, separated modernization,
completed the planned portable collection/loader boundary, specified the
compatibility window and numeric premises, corrected the composition name, and
moved dated research out of `docs/specs`. All earlier requirement identifiers and
artifact hashes remain traceable. Current product decisions are in specification
34; earlier sections in this archive remain historical records.
