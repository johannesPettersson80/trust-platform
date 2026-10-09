# Runtime portability implementation checklist

Behavior authority: [specification 34](../../../specs/34-runtime-portability.md),
[runtime engine](../../../specs/11-runtime-engine.md), and
[STBC format](../../../specs/12-bytecode.md). This file records execution and evidence;
it does not create additional product requirements. Existing closed extraction boards stay closed.

## Current checkpoint

| Field | Recorded state |
|---|---|
| Updated | 9 October 2026 — A1 verified; bookkeeping corrected after closeout review; A1 committed on the implementation branch |
| Plan | Specification 34 v0.10; SHA-256 recorded below |
| Checkout | Implementation: `/home/johannes/projects/trust-platform-portability-a1`, branch `feat/runtime-portability-a1`, base `9a15065725c17da2c912055f1509368d3fd01d6c`. Canonical instructions: `/home/johannes/projects/trust-platform`; all 20 instruction/skill files manually copied and matched before editing. |
| Working tree | The A1 implementation, specifications, evidence and bookkeeping are committed on `feat/runtime-portability-a1` as a commit series on top of the base (`git log 9a1506572..`). The retained manifest identifies the tested snapshot, which is the committed source. Three release-guard script files stay modified and uncommitted: the `remote_hmi` gate copied from the primary checkout's skills, which needs `scripts/hmi_ci_gate.sh` from `feat/hmi-builder`. |
| Authorization | User authorized A1 and separate read-only review; after failed runs 1 and 2, explicitly authorized one additional consolidated run 3. No automatic run 4. On 9 October the user authorized the commit of the reviewed A1 series and one release-guard run; the guard run is not started (see Next action). No push, no A2. |
| Scheduling amendment | SCHED-05 source and native core/host regressions passed run 3, including 25 ms / 10 ms with 40 activations and zero overruns. A4 fixture integration remains later work. |
| Active implementation scope / goal | A1 complete; no subsequent implementation scope authorized. |
| Last accepted implementation gate | A1 run 3: all planned checks passed; frozen local/builder source matched. |
| Next action | Two decisions before the authorized release-guard run: (1) `remote_hmi` requires `scripts/hmi_ci_gate.sh`, which exists only on `feat/hmi-builder`; either revert the three guard-script files in the primary checkout and here, or bring that script and its gate over together; (2) `fix/io-config-auth` already carries the Salsa/rustls update and version 0.24.70, so the merge order decides whether A1 keeps its dependency commit and version 0.24.70. Then run `release_candidate_guard.py prepare` once on the final SHA from a clean builder worktree. Not push-ready until that artifact passes. No A2. |
| Extraction baseline | Rust 1.95.0, edition 2021, resolver 2; required portable direct edges plus the explicitly authorized Salsa/rustls security updates. Broad modernization remains U. |
| Hardware | NUCLEO-F401RE model confirmed by owner. USB discovery on `raspberrypi` (9 October 2026): ST-LINK/V2.1 `0483:374b`; stable serial link `/dev/serial/by-id/usb-STMicroelectronics_STM32_STLink_0671FF575755846687183960-if02` → `/dev/ttyACM0`; OpenOCD installed; current account is in the devices' `plugdev` group. USB enumeration only: SWD/MCU revision, flashing, execution and measurements remain unverified. C6-DevKitC-1 availability unconfirmed. |
| Open execution records | Runs 1/2 retained as failed historical evidence. Run 3 accepted: `/home/johannes/projects/.artifacts/runtime-portability-a1/a1-closeout.md` with raw logs in adjacent run-3/. |

Plan SHA-256: `ca9c4fb008288639bfe813ad53dd28b9ccc02531f27b50414675a4a065774ec6` (the header was reformatted without trailing whitespace after run 3; run 3 tested `9a8912f9e2a23387dc9445362098d5890bd539d5ff8c9b73bf1be89e55ab1356`, as the frozen manifest records).

## How to use this checklist

1. Work on one authorized scope at a time. The four-scope selection changes the old Scope A
   boundary; it does not grant extra runs inside a scope. A1–A4 together close M2A-H.
2. Read the relevant specification and native assertions. Settle ambiguous behavior there before
   coding. Identify the exact observable result, fault/output policy and evidence needed.
3. Implement the complete scope and author its tests. Keep the existing APIs, assertions and
   baseline artifacts. Update the checkpoint and requirement rows as meaningful work finishes.
4. Obtain an independent code/spec review before freezing the implementation. Record reviewer,
   reviewed diff identity, findings and dispositions. A second reading by the author is not an
   independent review. Review need not run builds; no delegation is started by this document.
5. Freeze the implementation and command list, then execute one deduplicated final validation
   batch. Required builds, lint, tests, diagrams and applicable UI/hardware evidence belong there.
   Missing baseline runs use a preserved original snapshot in this same planned batch.
6. Record every failure and unrun check. A failed batch leaves the scope unverified. Fixes may be
   prepared within scope, but another run needs explicit user authorization under AGENTS.md.
7. Close a scope only when its implementation, native evidence and review are complete for the
   same source/build identity. Continue already-authorized work; a status question does not stop it.
   Starting another scope follows the authorization already recorded, or a new request if needed.

Use `planned`, `implementing`, `ready for batch`, `failed`, `awaiting evidence`, and `verified`.
A checked task means accepted evidence and resolved review findings, not merely code written.
Keep separate evidence for host tests, cross-target checking, firmware linking and physical boards.
No document, tool, reviewer or test suite guarantees zero bugs. The release target is zero known
correctness/safety defects in the claimed profile and no missing mandatory acceptance evidence;
this work does not establish functional-safety certification.

## Ordered scopes and acceptance work

All tasks start open. The owning specification supplies the complete behavior contract; the
items below organize implementation without replacing that contract.

### A1 — Portable foundations

- [x] RTP-A1-01: Record compiler/features, numeric compatibility expectations and target
  layout/resource limits needed now, with original source/artifact provenance for attribution.
  Freeze the accepted scheduler baseline after the SCHED-05 fix and updated assertions in A1;
  subsequent scopes inherit the corrected contract. Preserve later hardware measurements as
  pending, not guessed values.
- [x] RTP-A1-05: Implement SCHED-05 first as a separately reviewed change, sharing A1's final batch.
  Apply the reviewed readiness/core/case/host test draft from
  `/home/johannes/projects/trust-platform-task-schedule`, branch `fix/task-interval-schedule`,
  base `be8d81a4a7ab16ca7554b8be0f4723161ec1a47b`, excluding stale AGENTS.md and the unsupported
  changelog claim. Update the 35/55 ms sample baseline assertions to 30/50 ms; retain
  backward-time/jump/saturation coverage. Correct
  `periodic_task_keeps_its_interval_when_cycles_start_late` comments to describe injected logical
  time. Add the core 25 ms / 10 ms case: due 25 at sample 30, `last_run=25`, then due 50 at sample
  50 with zero missed intervals. Add the host `tasks.rs` case using `set_current_time` and
  `execute_cycle` at 10 ms samples through 1000 ms: 40 activations, zero overruns. Update the
  CHANGELOG entry when implemented, without the 75-runs claim. No intermediate validation run.
  Non-blocking maintenance alongside these tests adds `verification/spec-gaps.toml` and the new
  mappings in `verification/runtime-anomaly-taxonomy.toml` beside
  `ANOM_MAP_WATCHDOG_REVIEW_C5ABDD0A`, using `scripts/verification` generators for IDs. Close test
  evidence only after the native assertions pass. No metadata/generator work is started by this
  planning amendment; control-course configurations remain outside scope.
- [x] RTP-A1-02: Fix workspace/default feature inheritance and host opt-ins; implement the explicit
  no_std ordered-map hasher and constructors while preserving insertion order and host APIs.
  Add isolated F401/C6 core CI lanes and target `Value` layout assertions.
- [x] RTP-A1-03: Add the locked libm direct edge with the selected features; route core pow/trunc and
  hosted numeric functions through the agreed primitives. Preserve conversions/faults, retain old
  oracles and document the intentional numeric change. Author boundary and threshold assertions.
- [x] RTP-A1-04: Complete review and A1's one batch after all implementation/test authoring.
  Include `cargo +1.95.0 test --locked -p trust-runtime-core` and
  `cargo +1.95.0 test --locked -p trust-runtime --test tasks --test scheduler_resource` plus the
  runtime vertical and remaining A1 gates, deduplicated. Record corrected scheduler, numeric and
  target-graph evidence. A1 proves these foundations; compiler-free integration and boards remain
  later gates. Version/release actions follow the scope's normal rules.

### A2 — Shared loader and validator

- [ ] RTP-A2-01: Relocate the complete container, decoder, validator and metadata materialization
  boundary into core with portable collections and the required locked CRC edge. Decide byte
  serialization ownership without moving compiler/HIR lowering into core; preserve host re-exports.
- [ ] RTP-A2-02: Move the native assertions with their code; preserve 1.x decode/error behavior,
  bounds, CRC and malformed-section cases. Author missing native rejection assertions where the
  owning contract requires them; raw or partly validated data must not become executable.
- [ ] RTP-A2-03: Complete review and A2's batch against the relocated corpus and both target graphs.
  Saved 1.x parsing is not proof of source-free state construction or 2.0 execution.

### A3 — STBC 2.0 format and producer

- [ ] RTP-A3-01: Settle exact section/layout/validation rules in spec 12 before implementation:
  executable initializer bodies, defaults/root construction, ownership/lifetime and version rules.
  Record the P/Q compatibility-window policy; name release versions before P ships.
- [ ] RTP-A3-02: Implement 2.0 producer/lowering, loader/validator/disassembler/format handling and
  explicit version selection together. Retain the legacy fixture and add a separately named 2.0
  fixture. Keep current hosted execution usable; candidate 2.0 emission is opt-in until A4.
- [ ] RTP-A3-03: Complete review and A3's batch: legacy regressions, valid 2.0 round trips,
  malformed initialization/construction/version cases, and source-free rejection of 1.x.
  No claim that a serialized initializer has been executed until A4.

### A4 — Shared-engine integration

- [ ] RTP-A4-01: Extract the real dispatcher/state/context and timers needed by the common fixture;
  construct roots/defaults from TYPE_TABLE and execute initialization with that same dispatcher.
  Remove HIR/Expr/harness initialization and host thread-local/time assumptions from this path.
- [ ] RTP-A4-02: Execute saved STBC in a fresh headless consumer: Boolean/integer logic, TON, bounded
  array, FB through an interface, nonzero defaults, changing-input local initialization and the
  numeric/control fixture. Include SCHED-05's 25 ms periodic counter task with exact 10 ms logical
  samples from a 0 ms registration baseline: activations 30/50/80/100 ms, 40 through 1000 ms,
  zero overruns; save due times, samples, state and missed counts for COMPAT-06 on every host and
  both boards. Preserve native call-depth/budget/fault behavior and host/tier APIs; record remaining
  bring-up allocations and profile limits.
- [ ] RTP-A4-03: Complete review and A4's integration batch, including compiler-free loading,
  initialization/state/fault oracles, inherited scheduler assertions and affected host/format
  regressions. Require A1's completed SCHED-05 correction and native evidence before A4; verify
  its compiler-free integration and freeze the saved artifact/trace for B/E. Close aggregate A only
  now; M2A still requires the physical F401 result.

### Remaining program — all open

| Scope | Dependency and complete deliverable | Acceptance evidence |
|---|---|---|
| B — F401 bring-up (M2A-B) | A4; same engine/artifact on NUCLEO-F401RE, frozen 1.95 tuple, timer, process images, UART, IWDG and instrumented bring-up heap. | Physical trace agreement; reset/fault outputs; linked sections, preparation/RUN/heap peaks, native stack paint/MSP/IRQ headroom, numeric timing. B plus A closes M2A. |
| E — C6 bring-up (M2E) | A4 and an available C6 board; reuse the common corpus promptly to expose second-architecture assumptions. | Actual board execution, I/O/time/watchdog, software-float cost and memory evidence. No board means this gate stays open. |
| U — Modernization (M0U) | Default after A+B, before H; E need not block it. Refresh all-project dependency candidates and implement justified upgrades/retentions with Rust/edition/resolver/MSRV/Node changes. | MOD-01 ledger and MOD-06 batch, old/new numeric and performance comparisons, affected WASM/extension/protocol/native/MCU evidence. Requalify changed build tuples. |
| H — Full hosted adoption (M2B) | A+B, normally U; finish shared execution/library coverage, internal register tier and runtime-only/full/development compositions. | Saved-artifact and existing configuration/API/debug/HMI/protocol/restart/update workflows; native Linux/macOS/Windows evidence. |
| C — Bounded operation (M3) | Early hardware findings plus relevant H work; compound storage/atomic retirement, import limits, fixed fault records, bounded persistence and real generation lifetimes/admission/deployment. | Allocation instrumentation including first-use/error/destruction; capacity rejection; lifecycle/update/readers and persistence fault injection. Preserve independent PLC value copies. |
| QH — Hosted qualification (M4H) | H+C+U for each declared build. | Spec 34 §14 on each claimed native OS/architecture, bounded operation and preserved workflows, timing/load/performance records. |
| QF — F401 qualification (M4) | B+C+U for the final build. | §14 on the actual F401: completion-level timing, memory/stack, power-cut retain/install recovery, real faults/watchdog/I/O and declared soak/disturbances. |
| QE — ESP32 qualification (M5) | E+C+U for the final build. | Same applicable §14 contract on the named C6 board, independently of F401 results. |

Define each later scope's exact implementation and single batch before starting it; splitting or
combining an authorized scope requires an explicit change. QH/QF may proceed independently when
their prerequisites hold. Unavailable ESP32 hardware does not stop independent hosted/F401 work.
Completion of A1, A, B or U alone is not completion of the full program. Commit/push/merge/release
actions remain separately requested under AGENTS.md; qualification is not publication.

## Validation allocation

Before a scope's freeze, record the exact commands and assertions, features, target triples,
environment and expected nonzero test counts in its batch record. Use `--locked` and the selected
toolchain. All cargo/just/npm work runs on trust-builder with its target lease, disk preflight,
isolated source copy and canonical agent-file bootstrap. Native macOS/Windows and physical-board
evidence run on their actual platforms; a Linux builder cannot substitute for them.

| Batch | Required native/build coverage |
|---|---|
| A1 | Separate `thumbv7em-none-eabihf` and `riscv32imac-unknown-none-elf` core checks/feature graphs; core tests including corrected readiness/case assertions and target layout assertions; `tasks` (draft late-sample and new 25 ms / 10 ms case), `scheduler_resource`; numeric/control/fault assertions; `runtime_core_behavior_lock`, `bytecode_vm_core`, `bytecode_vm_differential`; required runtime vertical. SCHED-05 shares this one final batch. |
| A2 | Both isolated core targets; core/moved unit tests; `bytecode_container`, `bytecode_metadata`, `bytecode_decode_resource_bounds`, `bytecode_sections`, `bytecode_helpers`, `process_image`, `bytecode_validation`, `bytecode_optional_sections`; affected round-trip/VM behavior and runtime vertical. |
| A3 | Both isolated core targets; core/format/version tests; A2 format corpus as affected plus `bytecode_encoder`, `bytecode_roundtrip`, `bytecode_verification_cases`; affected compiler/disassembler assertions and runtime vertical. |
| A4 | Both isolated core targets, core tests, `runtime_core_compiler_free_load` (new, including the 25 ms / 10 ms trace), complete affected format corpus, `runtime_core_behavior_lock`, `bytecode_vm_core`, `bytecode_vm_differential`, `runtime_restart`, inherited `tasks` / `scheduler_resource` regressions, `vars_retain`, numeric/initializer cases and runtime vertical. |
| Every changed runtime boundary | Runtime vertical = `api_smoke`, `debug_control`, `complete_program`, `runtime_reliability`; add `simulation_workflow` if simulation changes. Include architecture-doctor `--full-map`, affected diagrams/render/drift, formatting and lint. Deduplicate suites included by a required full gate. |
| B/E | Firmware link and map, controlled boot and physical fixture traces, real output/watchdog behavior, preparation/heap/native stack and numeric measurements; retain raw data and firmware/artifact identities. |
| U/H/C/QH/QF/QE | The owning milestone plus all applicable spec §14 acceptance. Add risk-relevant property/fuzz/mutation/concurrency checks in the owning batch; assertions, budgets/seeds and raw results are required, not merely a tool name. |

CI/release rules still apply when requested: `just fmt`, `just clippy`, `just test-all`, applicable
area/hardware evidence and exact-SHA guards before a push. Integrate these into the authorized
batch when that delivery is in scope; do not silently launch another run afterwards.
If a prerequisite fails, record dependent commands as unrun; collect independent failures in the
planned batch where safe. Do not restart failed commands automatically or count zero selected tests
as acceptance. Code changed after validation requires another explicitly authorized batch before
claiming that new code verified.

## Requirement-to-evidence ledger

There is one row for each requirement ID defined in spec 34. The route names implementation and
qualification scopes, not a claim that each requirement needs a separate new test. Read the native
assertions before adding work. At the relevant scope, replace `pending` with exact code/test or
review locations, the batch result and artifact identity. Record partial scope evidence without
closing a cross-platform requirement prematurely. Unavailable evidence remains open; an exclusion
needs the owning spec's profile rule, not a convenient skip. Mapping completeness is not correctness
proof and this bookkeeping cannot invent product work or block it by itself.

| Requirement | Implementation / qualification route | Evidence and state |
|---|---|---|
| ARCH-01 | A1–A4, B/E/H/C; QH/QF/QE | A1 partial: shared numeric/control and scheduler assertions pass (R3-N, R3-S); full artifact/platform parity remains open. |
| ARCH-02 | A1–A4, B/E/H/C; QH/QF/QE | A1 partial: existing core extended without a platform fork (R3-B); shared loader/executor and adapters remain open. |
| ARCH-03 | A1–A4, B/E/H/C; QH/QF/QE | A1 partial: core dependency boundary compiles on both targets (R3-B); complete execution-boundary extraction remains open. |
| ARCH-04 | A1–A4, B/E/H/C; QH/QF/QE | A1 foundations only (R3-B/N); complete profile and platform claims remain open. |
| ARCH-05 | C; QH/QF/QE | pending — open |
| ARCH-06 | A2/A3/A4/C; QH/QF/QE | pending — open |
| ARCH-07 | B/E; QF/QE | pending — open |
| ARCH-08 | A1/A4/H; QH | A1 partial: selected hosted regression corpus passes (R3-N/S); full hosted migration remains open. |
| ARCH-09 | H; QH | pending — open |
| ARCH-10 | B/E; QF/QE | pending — open |
| MODEL-01 | A2/A3/A4/H/C; QH/QF/QE | pending — open |
| MODEL-02 | A2/A3/A4/H/C; QH/QF/QE | pending — open |
| MODEL-03 | A2/A3/A4/H/C; QH/QF/QE | pending — open |
| MODEL-04 | A2/A3/A4/H/C; QH/QF/QE | pending — open |
| BUILD-01 | A1/A2/A4/H/U; QH/QF/QE | A1 core no-default graphs pass (R3-B); compiler-free loader/executor construction remains A2/A4. |
| BUILD-02 | A1/A2/A4/H/U; QH/QF/QE | A1 graph/default-feature audit and independent target invocations pass (R3-B); repeat for later extracted components. |
| BUILD-03 | A1/A2; U on modernization | A1 feature inheritance/host opt-ins and explicit no_std hasher implemented; portable_foundations and MCU checks pass (R3-B). A2 CRC/loader graph remains open. Narrow security exception is recorded in spec 34. |
| BUILD-04 | A1/A2/A4/H/U; QH/QF/QE | Library checks/assembly pass (R3-B); firmware linkage, startup and hardware execution remain open. |
| BUILD-05 | A1/A2/A4/H/U; QH/QF/QE | A1 uses shared libm/core code and inspected emitted assembly (R3-N); no platform semantic fork added. Later platform code remains open. |
| BUILD-06 | A1/A2/A4/H/U; QH/QF/QE | A1 host std composition and selected regressions pass (R3-B/N); service extraction remains later work. |
| BUILD-07 | A4/H; QH | pending — open |
| BUILD-08 | H/U; QH | pending — open |
| BUILD-09 | A1/A2/A3/A4; C for optional no-CAS | A1 F401/C6 graphs and CI lanes pass locally (R3-B); later graph and atomic-retirement gates remain open; no C3 qualification. |
| HOST-01 | A4/H/C; QH | pending — open |
| HOST-02 | A4/H/C; QH | pending — open |
| HOST-03 | A4/H/C; QH | pending — open |
| HOST-04 | A4/H/C; QH | pending — open |
| HOST-05 | A4/H/C; QH | pending — open |
| HOST-06 | A4/H/C; QH | pending — open |
| HOST-07 | H; QH | pending — open |
| HOST-08 | H; QH | pending — open |
| SCHED-01 | A1/A4/H/C; QH/QF/QE | A1 corrected nominal readiness and selected scheduler behavior locks pass (R3-S); shared-engine/platform integration remains open. |
| SCHED-02 | A4/H/C; QH/QF/QE | pending — open |
| SCHED-03 | A4/H/C; QH/QF/QE | pending — open |
| SCHED-04 | A4/H/C; QH/QF/QE | pending — open |
| SCHED-05 | A1 fix/native evidence; A4 fixture; B/E/H/QH/QF/QE parity | A1 implementation and native validation complete (R3-S): 25 ms / 10 ms yields 40 activations and zero overruns. A4 saved fixture and B/E/H parity remain open. |
| LOAD-01 | A2/A3/A4/C; QH/QF/QE | pending — open |
| LOAD-02 | A2/A3/A4/C; QH/QF/QE | pending — open |
| LOAD-03 | A2/A3/A4/C; QH/QF/QE | pending — open |
| LOAD-04 | A2/A3/A4/C; QH/QF/QE | pending — open |
| LOAD-05 | A2/A3/A4/C; QH/QF/QE | pending — open |
| LOAD-06 | A2/A3/A4/C; QH/QF/QE | pending — open |
| LOAD-07 | A2/A3/A4/C; QH/QF/QE | pending — open |
| LOAD-08 | A2/A3/A4/C; QH/QF/QE | pending — open |
| LOAD-09 | A2/A3/A4/C; QH/QF/QE | pending — open |
| LOAD-10 | A3/A4; B/E/QH/QF/QE | pending — open |
| LOAD-11 | A3/A4; B/E/QH/QF/QE | pending — open |
| MEM-01 | A1/A4/B/E/C; QH/QF/QE | A1 Value slot guards and host 40-byte/alignment-8 measurement recorded (R3-M); all other regions, preparation peaks and native stacks remain open. |
| MEM-02 | A1/A4/B/E/C; QH/QF/QE | Deferred beyond A1 foundations: no bounded-storage/admission qualification claimed. R3-M records slot size only; full requirement remains open for the listed later scopes. |
| MEM-03 | A1/A4/B/E/C; QH/QF/QE | Deferred beyond A1 foundations: no bounded-storage/admission qualification claimed. R3-M records slot size only; full requirement remains open for the listed later scopes. |
| MEM-04 | C; QH/QF/QE | pending — open |
| MEM-05 | A1/A4/B/E/C; QH/QF/QE | Deferred beyond A1 foundations: no bounded-storage/admission qualification claimed. R3-M records slot size only; full requirement remains open for the listed later scopes. |
| MEM-06 | A1/A4/B/E/C; QH/QF/QE | Deferred beyond A1 foundations: no bounded-storage/admission qualification claimed. R3-M records slot size only; full requirement remains open for the listed later scopes. |
| MEM-10 | A4/C; QH/QF/QE | pending — open |
| MEM-11 | B/E/C; QH/QF/QE | pending — open |
| MEM-07 | A1/A4/B/E/C; QH/QF/QE | Deferred beyond A1 foundations: no bounded-storage/admission qualification claimed. R3-M records slot size only; full requirement remains open for the listed later scopes. |
| MEM-08 | A1/A4/B/E/C; QH/QF/QE | Deferred beyond A1 foundations: no bounded-storage/admission qualification claimed. R3-M records slot size only; full requirement remains open for the listed later scopes. |
| MEM-09 | A1/A4/B/E/C; QH/QF/QE | Deferred beyond A1 foundations: no bounded-storage/admission qualification claimed. R3-M records slot size only; full requirement remains open for the listed later scopes. |
| PORT-01 | A4/B/E/H/C; QH/QF/QE | pending — open |
| PORT-02 | A4/B/E/H/C; QH/QF/QE | pending — open |
| PORT-03 | A4/B/E/H/C; QH/QF/QE | pending — open |
| PORT-04 | A4/B/E/H/C; QH/QF/QE | pending — open |
| CYCLE-01 | A4/H/C; QH/QF/QE | pending — open |
| CYCLE-02 | A4/H/C; QH/QF/QE | pending — open |
| CYCLE-03 | A1/A4/H/C; QH/QF/QE | A1 dropped/count-missed nominal-deadline rule passes (R3-S); extracted cycle and platform parity remain open. |
| CYCLE-04 | A4/H/C; QH/QF/QE | pending — open |
| CYCLE-05 | A4/H/C; QH/QF/QE | pending — open |
| CYCLE-06 | A4/H/C; QH/QF/QE | pending — open |
| TIME-01 | A4/B/E/H/C; QH/QF/QE | pending — open |
| TIME-02 | A4/B/E/H/C; QH/QF/QE | pending — open |
| TIME-03 | A4/B/E/H/C; QH/QF/QE | pending — open |
| TIME-04 | A4/B/E/H/C; QH/QF/QE | pending — open |
| TIME-05 | A4/B/E/H/C; QH/QF/QE | pending — open |
| TIME-06 | A4/B/E/H/C; QH/QF/QE | pending — open |
| TIME-07 | A4/B/E/H/C; QH/QF/QE | pending — open |
| STATE-01 | A4/B/E/H/C; QH/QF/QE | pending — open |
| STATE-02 | A4/B/E/H/C; QH/QF/QE | pending — open |
| STATE-03 | A4/B/E/H/C; QH/QF/QE | pending — open |
| STATE-04 | A4/B/E/H/C; QH/QF/QE | pending — open |
| STATE-05 | A4/B/E/H/C; QH/QF/QE | pending — open |
| STATE-06 | A4/B/E/H/C; QH/QF/QE | pending — open |
| RETAIN-01 | A4/B/E/H/C; QH/QF/QE | pending — open |
| RETAIN-02 | A4/B/E/H/C; QH/QF/QE | pending — open |
| RETAIN-03 | A4/B/E/H/C; QH/QF/QE | pending — open |
| RETAIN-04 | A4/B/E/H/C; QH/QF/QE | pending — open |
| RETAIN-05 | A4/B/E/H/C; QH/QF/QE | pending — open |
| SERVICE-01 | H/C; QH | pending — open |
| SERVICE-02 | H/C; QH | pending — open |
| DEPLOY-01 | B/E/C; QH/QF/QE | pending — open |
| DEPLOY-02 | B/E/C; QH/QF/QE | pending — open |
| DEPLOY-03 | B/E/C; QH/QF/QE | pending — open |
| DEPLOY-04 | B/E/C; QH/QF/QE | pending — open |
| DEPLOY-05 | B/E/C; QH/QF/QE | pending — open |
| DEPLOY-06 | B/E/C; QH/QF/QE | pending — open |
| UPDATE-01 | H/C; QH | pending — open |
| UPDATE-02 | H/C; QH | pending — open |
| UPDATE-03 | H/C; QH | pending — open |
| UPDATE-04 | H/C; QH | pending — open |
| UPDATE-05 | H/C; QH | pending — open |
| UPDATE-06 | H/C; QH | pending — open |
| UPDATE-07 | H/C; QH | pending — open |
| UPDATE-08 | H/C; QH | pending — open |
| COMPAT-01 | A1/A3/A4/H/C; QH/QF/QE | A1 selected host regression corpus and source identities retained (R3-N/S); full artifact compatibility remains open. |
| COMPAT-02 | C; QH/QF/QE | pending — open |
| COMPAT-03 | A1/A3/A4/H/C; QH/QF/QE | A1 source/review/run identities retained (R3); full cross-platform compatibility claims remain open. |
| COMPAT-04 | A1/A3/A4/H/C; QH/QF/QE | A1 host public ordered-map type retained and tested (R3-B); remaining hosted interfaces and platforms remain open. |
| COMPAT-05 | A1/A3/A4/H/C; QH/QF/QE | A1 intentional libm migration has old/new finite reference output and passing threshold/fault assertions (R3-N); broader qualification remains open. |
| COMPAT-06 | A1/A3/A4/H/C; QH/QF/QE | A1 logical scheduling/numeric observations retained (R3-S/N); compiler-free saved oracle and board replay remain A4/B/E. |
| NUM-01 | A1/A4/B/E/U; QH/QF/QE | A1 compiler/libm/features, source identity and finite numeric evidence recorded (R3-N); board floating-point state and full tuple qualification remain open. |
| NUM-02 | A1/A4/B/E/U; QH/QF/QE | A1 exact/tolerance claims separated in spec 34 and finite tests (R3-N); strict cross-platform replay remains open. |
| NUM-03 | A1/A4/B/E/U; QH/QF/QE | A1 finite domain/tolerance, exceptional values and Boolean threshold tests pass (R3-N); full application corpus remains open. |
| NUM-04 | A1/A4/B/E/U; QH/QF/QE | A1 original ST control fixture and boundary assertions pass (R3-N); saved traces and hardware observations remain open. |
| NUM-05 | A1/A4/B/E/U; QH/QF/QE | A1 shared libm 0.2.16 routing implemented; width/fault and duration-context regressions pass (R3-N). Target runtime qualification remains open. |
| PLAN-01 | A1–A4/B | A1 foundation scope complete only; M2A compiler-free host/board checkpoint remains open. |
| PLAN-02 | A1/E; QE | C6 affects A1 graph/CI now (R3-B); physical ESP32 checkpoint remains open. |
| PERF-01 | A1/A4/B/E/U/H/C; QH/QF/QE | A1 compiler/features and original numeric references frozen (R3); no performance optimization or per-platform timing baseline claimed. Measurements remain open. |
| PERF-02 | A1/A4/B/E/U/H/C; QH/QF/QE | A1 host Value slot and target bounds recorded (R3-M); firmware sizes, stacks, memory regions and preparation/run peaks remain open. |
| PERF-03 | A1/A4/B/E/U/H/C; QH/QF/QE | No physical latency/timing distribution measured in A1; logical-time tests are R3-S, not timing qualification. Requirement remains open. |
| PERF-04 | A1/A4/B/E/U/H/C; QH/QF/QE | A1 reports bounded evidence and no speed/WCET claim (R3); performance acceptance remains open. |
| TEST-01 | A2/A3/A4/B/E/H/C; QH/QF/QE | pending — open |
| TEST-02 | B/E/H/U/C; QH/QF/QE | pending — open |
| TEST-03 | QH/QF/QE | pending — open |
| TEST-04 | QH/QF/QE | pending — open |
| TEST-05 | A2/A3/C; QH/QF/QE | pending — open |
| MOD-01 | U; QH/QF/QE for changed tuples | pending — open |
| MOD-02 | U; QH/QF/QE for changed tuples | pending — open |
| MOD-03 | U; QH/QF/QE for changed tuples | pending — open |
| MOD-04 | U; QH/QF/QE for changed tuples | pending — open |
| MOD-05 | U; QH/QF/QE for changed tuples | pending — open |
| MOD-06 | U; QH/QF/QE for changed tuples | pending — open |
| DESIGN-01 | A1–A4/B/E/U/H/C; QH/QF/QE | A1 source reviewed for simple shared collections/numeric/operand-context changes; no unresolved findings (R3-R). Later designs remain open. |
| DESIGN-02 | A1–A4/B/E/U/H/C; QH/QF/QE | A1 preserves host type identity, coercion and fault assertions (R3-B/N); admission/generation/bounded-resource APIs remain open. |
| DESIGN-03 | A1–A4/B/E/U/H/C; QH/QF/QE | A1 independent review, architecture and diagram checks pass (R3-R); later shared-engine/platform extraction remains open. |

### A1 evidence keys

All R3 references identify the unchanged **tested source snapshot**, not a new validation of the subsequent bookkeeping correction. See the [execution record](../../../notes/runtime-portability/a1-execution.md) and its relative evidence links.

- **R3**: all 24 steps pass in [run-3 ledger](../../../notes/runtime-portability/a1-evidence/run-3-ledger.tsv); exact commands/environment/frozen identity are beside it.
- **R3-S**: core `src/task/readiness.rs`, core readiness/case tests, host `tests/tasks.rs` and `scheduler_resource`; [native runtime output](../../../notes/runtime-portability/a1-evidence/run-3-runtime-tests.txt) and [core output](../../../notes/runtime-portability/a1-evidence/run-3-core-tests.txt).
- **R3-B**: workspace/core manifests, core `collections.rs`, `tests/portable_foundations.rs`, isolated F401/C6 checks/feature trees and `.github/workflows/ci.yml`.
- **R3-N**: core `numeric/math.rs`, arithmetic/TIME operators, hosted numeric helpers, `tests/portable_numeric_contract.rs`, numeric/VM/vertical suites; finite reference output is in the native runtime log.
- **R3-M**: core `value/layout.rs`; `record_value_slot_layout` in the core output plus both target build bounds.
- **R3-R**: [review messages](../../../notes/runtime-portability/a1-evidence/review-record.md), architecture/diagram/CI check statuses in the run-3 ledger.

## Batch, review and failure record

Three explicitly authorized A1 batches ran. This table records their outcomes; the field list below remains the template for later scopes.

| Batch | Source / review | Result and disposition |
|---|---|---|
| A1 run 1 | Initial A1 source review; initial frozen source retained | Failed: fixture syntax, MySQL lint and dependency advisories; later host suites unrun. [Ledger](../../../notes/runtime-portability/a1-evidence/run-1-ledger.tsv). |
| A1 run 2 | Security/metadata remediation reviewed; TLS test gap corrected before batch | Failed: duration operand-context lowering and LSP lint; advisory metadata wording failed. [Ledger](../../../notes/runtime-portability/a1-evidence/run-2-ledger.tsv). |
| A1 run 3 | Five-file review hash `64550b9f0909f4292b53cdfcc583c79a469e63418d77a1dbf0c31a26e7abd992`; 73-file frozen manifest retained | Passed: 23 required plus one advisory step; no automatic retry. [Ledger](../../../notes/runtime-portability/a1-evidence/run-3-ledger.tsv), [commands and identities](../../../notes/runtime-portability/a1-execution.md). |
| Post-closeout bookkeeping | Detailed ledger, registry links, gate inventory and portable evidence corrected after review | Documentation/metadata only. No run 4 and no fresh validator result. A1 remains verified, not push-ready. |

| Field | Required record |
|---|---|
| Scope and authorization | Scope ID, boundaries and instruction; explicit rerun authorization if applicable |
| Frozen identity | Branch/HEAD plus dirty patch and untracked-file hashes or complete source snapshot; spec, lockfile, artifact/config hashes |
| Environment | Canonical/destination agent-file verification, builder/platform path, compiler/flags/features, board/OS identity |
| Review | Independent reviewer, reviewed diff identity, findings, fixes/dispositions and unresolved items |
| Planned commands | Exact commands, target/lease, expected assertions, baselines and evidence locations; no hidden retry policy |
| Result | Every command's exit/status, selected/passed/failed/skipped counts, logs and relevant raw measurements |
| Failure ledger | Each failure, unrun dependency, violated requirement, root cause if known, prepared fix and next authorized action |
| Gate decision | Verified only for evidence actually obtained; remaining scope/platform claims stay open |

Historical document-setup evidence (superseded for A1 execution by the records above): The earlier dated report in the
[research archive](../../../notes/runtime-portability/README.md) records v0.8 document consistency
only. The v0.9 scheduling amendment was reviewed against the current sources, draft diff and
IEC §6.8.2; v0.10 corrects its placement to A1 following the sequencing review. No validation
batch, tests, generators or commits were run for either amendment. A2 and later implementation gates remain open; A1 subsequently passed run 3.

## Resume and goal use

Read AGENTS.md, the current checkpoint and owning spec first. Verify branch/HEAD/diff and preserve
unrelated work. For another checkout, manually copy and verify canonical AGENTS.md and the full
`.codex/skills/` tree before work; preserve the uncommitted plan too. Read the last batch/failure
record before launching anything. Resume the next incomplete authorized task without replaying
passed commands, reopening historical boards or losing a STOP/failure boundary. Update this
checkpoint before handoff, context reset or stopping, including the next concrete action.

Use one goal per authorized scope. Goals preserve an objective across turns; this repository file
holds detailed progress and evidence across sessions/reviewers. A goal cannot waive scope, STOP,
validation or publication rules. See [OpenAI's goal guidance](https://developers.openai.com/cookbook/examples/codex/using_goals_in_codex).

A1 is complete and committed. The next decision is the release-guard run after the two decisions recorded in the checkpoint, or a separately scoped A2 goal. Neither action is activated by this checklist.

A1 accepted evidence: [execution record](../../../notes/runtime-portability/a1-execution.md) in the A1 implementation checkout; full logs/requirement audit are linked there. Three batches were explicitly authorized separately; none was an automatic retry. Checkmarks cover A1 only.
