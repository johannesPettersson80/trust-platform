# Runtime portability implementation checklist

Behavior authority: [specification 34](../../../specs/34-runtime-portability.md),
[runtime engine](../../../specs/11-runtime-engine.md), and
[STBC format](../../../specs/12-bytecode.md). This file records execution and evidence;
it does not create additional product requirements. Existing closed extraction boards stay closed.

## Current checkpoint

A1 integration update (10 October): PR #130 merged through the guarded merge at
`258fe24b8c3706d566f5f05c25cfcd75e5b297c3` after all 26 checks passed. Native
Linux/macOS/Windows and VS Code passed first attempts with no retry rescues.
v0.24.71 release verification is complete. The integration agent owns that
release and subsequent A2 publication. #129/v0.24.70 has completed release
verification. None of this is A4 scope evidence.

| Field | Recorded state |
|---|---|
| Updated | 10 October 2026 — A4 review corrections scope-verified by run 11 and final independent acceptance; uncommitted. |
| Checkout | `/home/johannes/projects/trust-platform-portability-a4`, branch `feat/runtime-portability-a4`, base `77b91381fcf1b1850611f88b397bbfe8d45e3523` (verified and committed A3). |
| Bootstrap | Canonical AGENTS.md, CLAUDE.md and full .codex/skills copied manually from `/home/johannes/projects/trust-platform`; all 21 files byte-match. Read-only reviewer independently verified its checkout. |
| Authorization | Owner explicitly selected “Commit A3 and start A4”: local A3 commit, separate A4 implementation, independent review and one consolidated validation batch. No A4 commit/push or hardware work. Subsequently, “dont ask you fix it” and “continue a4 until its done” authorize reviewed correction/validation cycles without repeated permission questions; no per-command retries or source edits during a batch. |
| Scope | A4 shared-engine integration: extract the existing dispatcher/calls/storage and timer semantics, execute TYPE_TABLE construction and STBC initializers through that dispatcher, preserve hosted APIs/tier behavior, and freeze the compiler-free scheduler/state oracle. |
| Status | A4 verified: all 29 required and both advisory checks passed. Core all-features/portable/i686: 318/278/199; hosted unit: 3,841; hosted integration: 536. No failed, ignored or filtered assertions. Every external-review finding is closed for A4 software scope. B/E remain unstarted. |
| Source identity | Base `77b91381fcf1b1850611f88b397bbfe8d45e3523`; run-11 manifest `abc68ced75de1b39e1510028c4ce9669dd435ce310fa5be2892d9e2d4599e83b` (643 records). Final 239-path Rust identity `657f202d26f8d15a6b7ab54b706fb21bf974eac428bdcf4cb01e12db2574b4e0`; independent acceptance `683af7f5ecccb5c4dde06a95bbd32f792592ce60f2aac2717e31b8fcdbba435a`. Only evidence/closeout docs and the explicitly reviewed CSV comment changed after validation. No A4 commit. |
| Limitations | Scope verification only. No A4 exact-SHA release guard, push, firmware link, physical board run, measured stack/heap/timing fit, durable retain qualification or full hosted migration. Earlier failed batches are retained. |
| Integration | #129/v0.24.70 release is complete. #130 merged at `258fe24b8c3706d566f5f05c25cfcd75e5b297c3`; v0.24.71 release verification is complete. A2 publication is separate. A4 remains based on its committed A3 series. |
| Next action | A4 is ready for external review and separately authorized local commit/integration. Scope B is the next physical F401 implementation gate and remains unstarted. No further A4 validation, commit/push or hardware work is authorized by this closeout. |

A1 historical evidence remains in `docs/notes/runtime-portability/a1-execution.md`.
A2 evidence and full failure history: [execution record](../../../notes/runtime-portability/a2-execution.md).
Current integration decisions and deduplicated command map: [release integration](../../../notes/runtime-portability/a2-release-integration.md).

Historical pre-run-4 correction review: 116 source/config/spec/diagram paths, SHA-256
`30ec65ad4ae1827fb9cab98e561e737cf711260fe2ff24bbfd6765d482f98008`
(sorted path/NUL/content-or-`<deleted>`/NUL; excludes evolving execution notes/checklist).
That historical review does not certify the current post-run-4 corrections.
Prepared batch command/script is recorded in the execution note. Checkboxes stay open until
reviewed implementation and required evidence agree.

## Expanded A2 review correction scope

The owner explicitly requested all findings and recommendations implemented, including the larger
refactors. This supersedes the initial suggestion to defer them. The expanded corrections are now verified through A2-R6/R7/R8;
run-3 evidence remains historical for the original relocation.

- [x] RTP-A2-R01: Commit-safe evidence, complete manifests, accurate status/format/target claims.
- [x] RTP-A2-R02: Real bytecode modules and documented public API; hosted owned conversions,
  public borrowed view and slice-bound lifetime; canonical shared pure test corpus and common helpers.
- [x] RTP-A2-R03: One decoded instruction representation/context, structured rejections, remove
  no-op/dead passes, preserve semantic checks and error precedence where required.
- [x] RTP-A2-R04: Bound validator analysis storage/work; branch-entry stack states and native
  adversarial-budget assertions; checked serializer/jump arithmetic and header/table exclusion.
- [x] RTP-A2-R05: Validated borrowed marker required at host VM materialization, while later
  profile admission/executable preparation remain A4; native positive and rejection tests.
- [x] RTP-A2-R07: Document and regress LOAD_NULL instance ownership; index tables once, bound
  lookup work, compact decoded instructions, share stack effects, remove validator panic paths,
  and admit the maximum-instruction/reference scale fixture within default limits.
- [x] RTP-A2-R06: Independent review of entire correction, then explicitly authorized consolidated
  batch including 32-bit native execution, both MCU checks, host corpus/runtime vertical,
  lint/format (including remaining touched fragments), architecture/diagrams and provenance checks.

No new validation batch or commit/push is authorized by this scope record.

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

These items closed after reviewed corrections and A2-R6/R7/R8. Runs 1–3 certify only the historical relocation; failed runs 4–7 are retained, with subsequent passing evidence explicitly identified.

- [x] RTP-A2-01: Relocate the complete container, decoder, validator and metadata materialization
  boundary into core with portable collections and the required locked CRC edge. Decide byte
  serialization ownership without moving compiler/HIR lowering into core; preserve host re-exports.
- [x] RTP-A2-02: Move the native assertions with their code; preserve 1.x decode/error behavior,
  bounds, CRC and malformed-section cases. Author missing native rejection assertions where the
  owning contract requires them; raw or partly validated data must not become executable.
- [x] RTP-A2-03: Complete review and A2's batch against the relocated corpus and both target graphs.
  Saved 1.x parsing is not proof of source-free state construction or 2.0 execution.

### A3 — STBC 2.0 format and producer

- [x] RTP-A3-01: Settle exact section/layout/validation rules in spec 12 before implementation:
  executable initializer bodies, defaults/root construction, ownership/lifetime and version rules.
  Record the P/Q compatibility-window policy; name release versions before P ships.
- [x] RTP-A3-02: Implement 2.0 producer/lowering, loader/validator/disassembler/format handling and
  explicit version selection together. Retain the legacy fixture and add a separately named 2.0
  fixture. Keep current hosted execution usable; candidate 2.0 emission is opt-in until A4.
- [x] RTP-A3-03: Complete review and A3's batch: legacy regressions, valid 2.0 round trips,
  malformed initialization/construction/version cases, and source-free rejection of 1.x.
  No claim that a serialized initializer has been executed until A4.

### A4 — Shared-engine integration

- [x] RTP-A4-01: Extract the real dispatcher/state/context and timers needed by the common fixture;
  construct roots/defaults from TYPE_TABLE and execute initialization with that same dispatcher.
  Remove HIR/Expr/harness initialization and host thread-local/time assumptions from this path.
- [x] RTP-A4-02: Execute saved STBC in a fresh headless consumer: Boolean/integer logic, TON, bounded
  array, FB through an interface, nonzero defaults, changing-input local initialization and the
  numeric/control fixture. Include SCHED-05's 25 ms periodic counter task with exact 10 ms logical
  samples from a 0 ms registration baseline: activations 30/50/80/100 ms, 40 through 1000 ms,
  zero overruns; save due times, samples, state and missed counts for later COMPAT-06 replay on every host and
  both boards in B/E and hosted qualification. Preserve native call-depth/budget/fault behavior and host/tier APIs; record remaining
  bring-up allocations and profile limits.
- [x] RTP-A4-03: Complete review and A4's integration batch, including compiler-free loading,
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
| A2 | Both isolated core targets; core/moved unit tests; `bytecode_container`, `bytecode_metadata`, `bytecode_decode_resource_bounds`, `bytecode_sections`, `process_image`, `bytecode_validation`, `bytecode_optional_sections`; affected round-trip/VM behavior and runtime vertical. |
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
| ARCH-01 | A1–A4, B/E/H/C; QH/QF/QE | A4 partial (A4-E/N/S): saved STBC 2.0 state, scheduling and labelled numeric oracles execute in native consumers. Replay and qualification on every claimed host and board remain B/E/H/QH/QF/QE. |
| ARCH-02 | A1–A4, B/E/H/C; QH/QF/QE | A4 partial (A4-E/B): the no-default core constructs and executes saved artifacts without trust-runtime or HIR; isolated F401/C6 library graphs include the shared engine. Firmware compositions remain B/E. |
| ARCH-03 | A1–A4, B/E/H/C; QH/QF/QE | A4 partial (A4-B/Q): the shared dispatcher, storage and standard library retain core forbid(unsafe_code). Hardware-specific unsafe/BSP review remains B/E. |
| ARCH-04 | A1–A4, B/E/H/C; QH/QF/QE | A4 partial (A4-E/S/Q): common dispatch, calls, references, timers and cooperative policy have thin host clock/debug/profile/tier adapters. Physical platform mechanisms and qualification remain B/E/H. |
| ARCH-05 | C; QH/QF/QE | pending — open |
| ARCH-06 | A2/A3/A4/C; QH/QF/QE | A4 partial (A4-A/R): private prepared/state constructors enforce native import/state/clock capabilities and logical resource limits; unsupported raw Retain/Io and multiple-resource profiles reject explicitly. Timing and deployment admission remain C/qualification. |
| ARCH-07 | B/E; QF/QE | pending — open |
| ARCH-08 | A1/A4/H; QH | A4 partial (A4-H): selected existing hosted API, debug, restart, standard-library and register-tier regressions remain supported. Full hosted compositions, services, performance and native-OS qualification remain H/QH. |
| ARCH-09 | H; QH | pending — open |
| ARCH-10 | B/E; QF/QE | pending — open |
| MODEL-01 | A2/A3/A4/H/C; QH/QF/QE | A4 partial (A4-E/A): serialized bytes, immutable PreparedModule and exclusively owned RuntimeState have separate ownership; saved-artifact construction is compiler-free. Installed-generation replacement remains H/C. |
| MODEL-02 | A2/A3/A4/H/C; QH/QF/QE | A4 partial (A4-E/R): artifact initializer/default bodies and native bindings are prepared before execution; first and repeated calls use the shared dispatcher. Allocation-free RUN, complete hosted plans/caches and bounded first-use qualification remain H/C. |
| MODEL-03 | A2/A3/A4/H/C; QH/QF/QE | pending — open |
| MODEL-04 | A2/A3/A4/H/C; QH/QF/QE | pending — open |
| BUILD-01 | A1/A2/A4/H/U; QH/QF/QE | A4 partial (A4-E/B): no-default core initializes and executes saved artifacts without HIR; F401/C6 core checks include loader, standard library and executor. Firmware and board execution remain B/E. |
| BUILD-02 | A1/A2/A4/H/U; QH/QF/QE | A4 partial (A4-B): separate F401/C6 no-default library invocations and no-dev feature graphs cover the extracted engine dependencies. Firmware-specific feature unification and qualification remain B/E/U. |
| BUILD-03 | A1/A2; U on modernization | A1 feature/map split preserved; locked portable CRC edge qualified in A2-R6. Toolchain/edition modernization remains U. |
| BUILD-04 | A1/A2/A4/H/U; QH/QF/QE | A4 partial (A4-B): both MCU core library checks pass; this is not a firmware link/startup or board-execution result. Those requirements remain B/E. |
| BUILD-05 | A1/A2/A4/H/U; QH/QF/QE | A4 partial (A4-E/B/Q): one Rust dispatcher and shared arithmetic/reference/timer implementation serve the portable and hosted stack paths. MCU firmware integration remains B/E. |
| BUILD-06 | A1/A2/A4/H/U; QH/QF/QE | A4 partial (A4-H): compiler, services, host clocks/debugger/profiling and register-tier selection remain hosted adapters around shared execution. Full product composition work remains H. |
| BUILD-07 | A4/H; QH | A4 partial (A4-E/B): the standalone core consumer loads and executes without compiler/HIR. The hosted runtime-only product composition and its isolated feature graph remain H; this requirement is not closed. |
| BUILD-08 | H/U; QH | pending — open |
| BUILD-09 | A1/A2/A3/A4; C for optional no-CAS | A4 partial (A4-B): independent F401/C6 no-default engine checks and graphs pass. Arc/SmolStr bring-up ownership remains; atomic-retirement, no-CAS/C3 and bounded-compound qualification remain C. |
| HOST-01 | A4/H/C; QH | A4 partial (A4-H/E): the actual hosted stack dispatcher, call/reference operations and native semantics are shared with the source-free engine. Full hosted cycle/service adoption and all native OS qualification remain H/QH. |
| HOST-02 | A4/H/C; QH | A4 partial (A4-A/R): explicit application-selectable preparation, construction, work/depth and image limits are checked. No STM32 memory-fit or measured hosted capacity qualification is claimed. |
| HOST-03 | A4/H/C; QH | A4 partial (A4-H): required runtime vertical, debug, restart, I/O, bytecode and selected compatibility APIs retain native regressions. Complete HMI/protocol/online-change and platform qualification remain H/C/QH. |
| HOST-04 | A4/H/C; QH | A4 partial (A4-H): existing register-tier APIs and stack/tier differential assertions are retained through the host adapter. Compiler-free optimized-plan preparation and bounded cache behavior remain H/C. |
| HOST-05 | A4/H/C; QH | pending — open |
| HOST-06 | A4/H/C; QH | pending — open |
| HOST-07 | H; QH | pending — open |
| HOST-08 | H; QH | pending — open |
| SCHED-01 | A1/A4/H/C; QH/QF/QE | A4 partial (A4-E/S): shared readiness, priority/event/background regressions and compiler-free nominal-deadline trace integrate the A1 correction. Board and full hosted scheduling-profile qualification remain B/E/H. |
| SCHED-02 | A4/H/C; QH/QF/QE | pending — open |
| SCHED-03 | A4/H/C; QH/QF/QE | pending — open |
| SCHED-04 | A4/H/C; QH/QF/QE | pending — open |
| SCHED-05 | A1 fix/native evidence; A4 fixture; B/E/H/QH/QF/QE parity | A1 correction integrated in A4-E/S: saved 25 ms task sampled every 10 ms activates at 30/50/80/100 ms and 40 times through 1000 ms with zero overruns; due-time/state trace retained. Physical and remaining host replay remain B/E/H/Q. |
| LOAD-01 | A2/A3/A4/C; QH/QF/QE | A4 partial (A4-E): a fresh core consumer initializes and executes saved STBC 2.0 without source parsing, compiler session or HIR. Remaining OS/board profiles require B/E/H qualification. |
| LOAD-02 | A2/A3/A4/C; QH/QF/QE | A4 partial (A4-E/A): TYPE_TABLE, construction roots, task/I/O metadata and executable initializers supply fresh state; missing or inconsistent executable metadata fails admission. Complete later-profile coverage remains H/C. |
| LOAD-03 | A2/A3/A4/C; QH/QF/QE | A4 partial (A4-A): full validation is followed by native import/signature/hidden-state and clock-service checks plus resource/domain restrictions before RUN. Timing/deployment profile admission remains C. |
| LOAD-04 | A2/A3/A4/C; QH/QF/QE | A4 partial (A4-A): decoder reservations, empty-string slots, concrete target-sized table/payload demand, expanded constants and byte/decoded artifact bounds have native boundary tests. Logical accounting is not allocator/stack peak measurement; B/C qualification remains open. |
| LOAD-05 | A2/A3/A4/C; QH/QF/QE | A4 partial (A4-R/H): dynamic indexes, reference identity/lifetime, typed assignments, constants, nested call/depth/fuel/deadline and arithmetic faults remain enforced. Broader profile/platform qualification remains open. |
| LOAD-06 | A2/A3/A4/C; QH/QF/QE | A4 partial (A4-A/R): executable imports resolve to shared implementations; native FB state/signatures and CURRENT_DT capability are checked. Physical/import blocking, cancellation and full bounded-callback contracts remain C. |
| LOAD-07 | A2/A3/A4/C; QH/QF/QE | A4 partial (A4-A/R): raw/validated containers are distinct from private PreparedModule admission and RuntimeState construction; public state exposes typed writes, not mutable adapter-trait storage access. Later deployment admission remains C. |
| LOAD-08 | A2/A3/A4/C; QH/QF/QE | pending — open |
| LOAD-09 | A2/A3/A4/C; QH/QF/QE | A4 partial (A4-A/R): bounded construction/preparation and failed restart preserve prior state; native output groups preserve caller destinations on conversion/write/budget failure. Online generation replacement and bounded overlap remain H/C. |
| LOAD-10 | A3/A4; B/E/QH/QF/QE | A3 artifact production now feeds A4-E: fresh roots/defaults, task/I/O and common fixture execution use the shared dispatcher without HIR. Hosted 1.x remains compatible; broader platform/profile construction remains B/E/H/C. |
| LOAD-11 | A3/A4; B/E/QH/QF/QE | A4 partial (A4-E/R): default/alias/member/local/static initializer bodies execute through the ordinary dispatcher with shared budgets and activation lifetimes. Legacy 1.x retains its hosted Expr adapter; full hosted retirement remains H. |
| MEM-01 | A1/A4/B/E/C; QH/QF/QE | A4 partial (A4-A/R): logical preparation, construction, value-copy and image limits supplement A1 slot bounds. Per-region physical peaks, service/retain staging, native stacks and IRQ headroom remain B/E/C qualification. |
| MEM-02 | A1/A4/B/E/C; QH/QF/QE | A4 partial (A4-A/R): artifact/decoder/validation/preparation, construction-node/byte and execution work/depth limits reject excess demand. MCU-specific capacities and measured preparation/RUN/native-stack peaks remain B/E/C. |
| MEM-03 | A1/A4/B/E/C; QH/QF/QE | A4 partial (A4-R): finite frame/call/depth checks and suspended-activation storage are exercised. Fully reserved bounded RUN storage and native CPU-stack/interrupt measurements remain B/E/C. |
| MEM-04 | C; QH/QF/QE | pending — open |
| MEM-05 | A1/A4/B/E/C; QH/QF/QE | A4 partial (A4-R/H): stable runtime fault identities are preserved through shared execution. Rich allocated diagnostic errors remain; fixed-size bounded fault records are still C work. |
| MEM-06 | A1/A4/B/E/C; QH/QF/QE | A4 partial (A4-R): aggregate/string/reference and shared-struct COW copies are charged before mutation; independent value semantics and transactional output groups are tested. Allocation-free bounded compound representation remains C. |
| MEM-10 | A4/C; QH/QF/QE | A4 partial (A4-A/R/Q): exclusive private EngineState borrows immutable PreparedModule; the public wrapper prevents untyped mutable-trait access. Bounded storage/lookup allocation qualification remains C. |
| MEM-11 | B/E/C; QH/QF/QE | pending — open |
| MEM-07 | A1/A4/B/E/C; QH/QF/QE | A4 partial (A4-R): mutable state borrows immutable prepared metadata and checks activation/reference lifetimes. Installed-generation replacement, readers and retirement remain H/C. |
| MEM-08 | A1/A4/B/E/C; QH/QF/QE | A4 partial (A4-E/R): artifact member/method/interface metadata supports FBs, inherited classes, dynamic receiver checks and shared method bodies. Broader admitted profiles and board qualification remain open. |
| MEM-09 | A1/A4/B/E/C; QH/QF/QE | Deferred beyond A1 foundations: no bounded-storage/admission qualification claimed. R3-M records slot size only; full requirement remains open for the listed later scopes. |
| PORT-01 | A4/B/E/H/C; QH/QF/QE | A4 partial (A4-E/R): injected logical Duration is separate from ExecutionServices physical-deadline and UTC callbacks. Complete physical clock resolution/wrap/epoch and watchdog ports remain B/E/H. |
| PORT-02 | A4/B/E/H/C; QH/QF/QE | A4 partial (A4-A/R): explicit services, native capability checks and work/deadline failures are enforced. Returning test callbacks do not prove cancellation, blocking or allocation bounds; platform/import qualification remains C. |
| PORT-03 | A4/B/E/H/C; QH/QF/QE | pending — open |
| PORT-04 | A4/B/E/H/C; QH/QF/QE | pending — open |
| CYCLE-01 | A4/H/C; QH/QF/QE | A4 partial (A4-E/S): stable input images precede participating task/background execution; flat/hierarchical binding and edge cases are exercised. Hardware transfer/completion contracts remain B/E/H. |
| CYCLE-02 | A4/H/C; QH/QF/QE | A4 partial (A4-S/R): normal output publication follows successful batch completion; changed outputs are suppressed on retain failure and native copy-back is transactional. Physical adapter completion remains open. |
| CYCLE-03 | A1/A4/H/C; QH/QF/QE | A4 partial (A4-E/S): inherited readiness/order tests and saved nominal-period trace cover dropped/counted missed activations, sampled edges and task-before-background order. Physical/platform replay remains B/E/H. |
| CYCLE-04 | A4/H/C; QH/QF/QE | A4 partial (A4-E/N/B): shared storage-backed TON/TOF/TP, arithmetic and reference semantics execute natively and compile for both MCU targets. Board timer/clock integration remains B/E. |
| CYCLE-05 | A4/H/C; QH/QF/QE | A4 partial (A4-S/H): shared %I/%Q/%M codecs, partial/configuration/access bindings and existing I/O regressions are retained. Full forces/debugger and platform integration remain H/B/E. |
| CYCLE-06 | A4/H/C; QH/QF/QE | A4 partial (A4-R/Q): exclusive mutable state and typed public access enforce between-cycle engineering ownership; input images are staged through exclusive access. Bounded external service queues remain H/C. |
| TIME-01 | A4/B/E/H/C; QH/QF/QE | A4 partial (A4-A/R): finite nested work/depth limits and deadline-fault tests cover the selected bring-up engine. No schedulability, physical timing admission or successful-deadline guarantee is claimed. |
| TIME-02 | A4/B/E/H/C; QH/QF/QE | A4 partial (A4-R): dispatcher, initializer, helper traversal, allocation/copy and COW paths consume shared limits; budget/deadline errors propagate without mutation bypass. Complete callback bounds and physical cost qualification remain C/B/E/H. |
| TIME-03 | A4/B/E/H/C; QH/QF/QE | pending — open |
| TIME-04 | A4/B/E/H/C; QH/QF/QE | pending — open |
| TIME-05 | A4/B/E/H/C; QH/QF/QE | pending — open |
| TIME-06 | A4/B/E/H/C; QH/QF/QE | pending — open |
| TIME-07 | A4/B/E/H/C; QH/QF/QE | A4 partial (A4-E/S): saved state/numeric traces use explicit injected logical instants, including non-multiple nominal periods and restart time. Physical wakeup/wrap/service-race and timing classification remain B/E/H. |
| STATE-01 | A4/B/E/H/C; QH/QF/QE | pending — open |
| STATE-02 | A4/B/E/H/C; QH/QF/QE | pending — open |
| STATE-03 | A4/B/E/H/C; QH/QF/QE | A4 partial (A4-S/R): execution and retain-service failures latch faults and suppress normal output publication. MCU failure policy, mandatory hardware state and independent supervision remain B/E/C. |
| STATE-04 | A4/B/E/H/C; QH/QF/QE | A4 partial (A4-S/R): latched runtime faults block later normal cycles/output commits. Fixed diagnostics, configured fault-output actuation, all-driver attempts and confirmation remain B/E/H/C. |
| STATE-05 | A4/B/E/H/C; QH/QF/QE | pending — open |
| STATE-06 | A4/B/E/H/C; QH/QF/QE | A4 partial (A4-E/R): explicit warm/cold restart reconstructs state and preserves required retained graphs/aliases; function-static first use and failed restart transactions are tested. Physical reset/recovery policies remain B/E. |
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
| COMPAT-01 | A1/A3/A4/H/C; QH/QF/QE | A4 partial (A4-H/E): existing 1.x regressions and the new saved 2.0 consumer exercise shared semantics. Explicit 2.0-only static-FB initialization support is documented; full hosted/platform qualification remains open. |
| COMPAT-02 | C; QH/QF/QE | pending — open |
| COMPAT-03 | A1/A3/A4/H/C; QH/QF/QE | A4 partial (A4-E/R): saved Boolean/integer/TON/array/interface/changing-reference/default/retain fixture and inherited-class/static tests execute through shared code. Cross-platform and full feature qualification remain B/E/H/C/Q. |
| COMPAT-04 | A1/A3/A4/H/C; QH/QF/QE | A4 partial (A4-E/R/H): exact integer/Boolean, scheduler, typed fault, copy-back and saved state assertions preserve selected observable contracts. Broader cross-platform exact-trace qualification remains open. |
| COMPAT-05 | A1/A3/A4/H/C; QH/QF/QE | A4 partial (A4-N): shared libm/conversion behavior and saved numeric expectations distinguish exact results/control outputs from operation-specific tolerances. No arbitrary cross-CPU bit identity is claimed. |
| COMPAT-06 | A1/A3/A4/H/C; QH/QF/QE | A4 partial (A4-E/N): saved program/numeric artifacts and expected logical trace are frozen; native 64-bit replay plus selected i686 saved-program execution are recorded. Native Windows/macOS and physical F401/C6 replay remain open. |
| NUM-01 | A1/A4/B/E/U; QH/QF/QE | A4 partial (A4-N/B): shared library/toolchain/features and ordinary/exceptional numeric assertions are retained with artifact/source identities. Board floating-point state and full environmental qualification remain B/E/U/Q. |
| NUM-02 | A1/A4/B/E/U; QH/QF/QE | A4 partial (A4-N): strict observations are separated from tolerance-qualified function results in saved-artifact assertions. Matching numeric/environmental contracts on every claimed target remain unqualified. |
| NUM-03 | A1/A4/B/E/U; QH/QF/QE | A4 partial (A4-N): saved numeric replay checks operation-specific reference tolerances and resulting threshold/command decisions; existing exceptional/width/fault suites remain. Full application/platform corpus remains open. |
| NUM-04 | A1/A4/B/E/U; QH/QF/QE | A4 partial (A4-N): numeric-v2 replay labels exact root/power/sum/TIME/Boolean command assertions separately from transcendental tolerances. Hardware and all-platform observations remain open. |
| NUM-05 | A1/A4/B/E/U; QH/QF/QE | A4 partial (A4-N/B): scalar standard library, conversions and timers share portable numeric primitives; native regressions and MCU compilation cover the extracted implementation. Target runtime qualification remains B/E/H/U. |
| PLAN-01 | A1–A4/B | A1–A4 aggregate A/M2A-H scope complete only after the full A4 acceptance batch passes. M2A remains open until Scope B executes the same engine/artifact on physical F401 with required measurements. |
| PLAN-02 | A1/E; QE | A4 supporting evidence (A4-B): the C6 graph includes the common loader/executor. Physical ESP32 bring-up and qualification remain E/QE; F401 evidence cannot close them. |
| PERF-01 | A1/A4/B/E/U/H/C; QH/QF/QE | A1 compiler/features and original numeric references frozen (R3); no performance optimization or per-platform timing baseline claimed. Measurements remain open. |
| PERF-02 | A1/A4/B/E/U/H/C; QH/QF/QE | A1 host Value slot and target bounds recorded (R3-M); firmware sizes, stacks, memory regions and preparation/run peaks remain open. |
| PERF-03 | A1/A4/B/E/U/H/C; QH/QF/QE | No physical latency/timing distribution measured in A1; logical-time tests are R3-S, not timing qualification. Requirement remains open. |
| PERF-04 | A1/A4/B/E/U/H/C; QH/QF/QE | A1 reports bounded evidence and no speed/WCET claim (R3); performance acceptance remains open. |
| TEST-01 | A2/A3/A4/B/E/H/C; QH/QF/QE | A4 partial (A4-A/R/S): first-use/static-restart, hidden references, readonly/type/limit, initializer and output-failure cases extend the native corpus. Full disturbance, hang, service and deployment qualification remains open. |
| TEST-02 | B/E/H/U/C; QH/QF/QE | A4 supporting evidence (A4-Q): source/build/artifact identities, environments, commands, independent reviews and every failed/successful batch ledger are retained. Physical memory/timing/topology and per-platform qualification packages remain open. |
| TEST-03 | QH/QF/QE | pending — open |
| TEST-04 | QH/QF/QE | pending — open |
| TEST-05 | A2/A3/C; QH/QF/QE | A4 partial (A4-A/R): admitted-wire mutations, capability/capacity rejection and failed construction/restart preservation are exercised. This is not a broad fuzz campaign or deployment authorization/rollback qualification. |
| MOD-01 | U; QH/QF/QE for changed tuples | pending — open |
| MOD-02 | U; QH/QF/QE for changed tuples | pending — open |
| MOD-03 | U; QH/QF/QE for changed tuples | pending — open |
| MOD-04 | U; QH/QF/QE for changed tuples | pending — open |
| MOD-05 | U; QH/QF/QE for changed tuples | pending — open |
| MOD-06 | U; QH/QF/QE for changed tuples | pending — open |
| DESIGN-01 | A1–A4/B/E/U/H/C; QH/QF/QE | A4 partial (A4-Q): the existing dispatcher is shared through small context boundaries; preparation, construction, assignment/reference policy and lifecycle remain cohesive modules. Later profile designs remain open. |
| DESIGN-02 | A1–A4/B/E/U/H/C; QH/QF/QE | A4 partial (A4-A/R/Q): private admission, exclusive state, typed fallible writes and runtime lifetime/copy/budget checks reinforce the native contract. Fixed diagnostic/generation/bounded-storage design remains C. |
| DESIGN-03 | A1–A4/B/E/U/H/C; QH/QF/QE | A4 partial (A4-Q): independent correction reviews, architecture checks and ownership/flow diagrams cover the shared execution boundary. Remaining host/platform and bounded-profile extraction stays H/B/E/C. |

### A3 evidence keys

- **A3-R1**: failed initial compilation/architecture; dependency audit and isolated no-dev feature trees pass. Unchanged dependency files retain that evidence.
- **A3-R2**: [ledger](../../../notes/runtime-portability/a3-evidence/run-2-ledger.tsv), 221 core/70 portable/139 i686 tests and F401/C6 core compilation pass. Core bytes remain unchanged through A3-R4.
- **A3-R3**: [ledger](../../../notes/runtime-portability/a3-evidence/run-3-ledger.tsv), canonical diagrams render successfully; source-authoring failures remain historical, corrected in A3-R4.
- **A3-R4**: [ledger](../../../notes/runtime-portability/a3-evidence/run-4-ledger.tsv), all 13 required/3 advisory steps pass; [execution and evidence reconciliation](../../../notes/runtime-portability/a3-execution.md) records counts, identities and remaining A4/board boundaries.
- **A3-R5**: [ledger](../../../notes/runtime-portability/a3-evidence/run-5-ledger.tsv); 224 core, 70 portable, 142 i686, both MCU checks, 219 integrations and 3,863 units pass. Lint/warnings/architecture/rendering/metadata pass; three authoring-fixture failures are retained.
- **A3-R6**: [ledger](../../../notes/runtime-portability/a3-evidence/run-6-ledger.tsv); all 31 authoring and four boundary tests pass after fixture-only corrections; all eight required and two advisory steps pass. Production source and generated artifact remain covered by run 5.

### A2 evidence keys

A2-R1/R2/R3 (historical relocation only): [execution and failure record](../../../notes/runtime-portability/a2-execution.md), frozen manifest and full batch-1 logs. Runs 2 and 3 closed the original relocation gates only; subsequent corrections reopened A2 and are closed by A2-R6/R7/R8.

A2-R4: [run-4 ledger](../../../notes/runtime-portability/a2-evidence/run-4-ledger.tsv) and [execution record](../../../notes/runtime-portability/a2-execution.md). Failed expanded correction batch; post-run-4 changes were unverified at that checkpoint, then qualified in A2-R6/R7/R8.

### A4 evidence keys — verified run-11 source

All keys now identify the passing [run-11 evidence](../../../notes/runtime-portability/a4-evidence/run-11/README.md), its exact commands and frozen source/fixture hashes, and [final independent acceptance](../../../notes/runtime-portability/a4-review/run-11-final-independent-acceptance.md).
The historical run-5 evidence remains intact for its older source. These keys
establish software scope proof; later platform and hardware qualification stays open.

- **A4-E**: Run-11 core-all-features/core-portable and selected i686 logs: core/tests/runtime_core_compiler_free_load.rs saved artifact/expected-trace oracle; host/tests/runtime_core_compiler_free_load.rs source-to-saved-to-fresh-core and legacy scan parity. i686 does not run every hosted or numeric suite. Commands, hashes and saved trace are bound by the linked evidence index.
- **A4-A**: Run-11 preparation_profile and source_free_profile_admission native cases plus core preparation/decoder tests: exact accounting boundaries, expansion/import/native state/capability/domain/multiple-resource limits; private API compile-fail contract.
- **A4-B**: Run-11 isolated thumbv7em-none-eabihf and riscv32imac-unknown-none-elf checks and no-dev feature graphs, Rust 1.95.0 tuple. Library compilation only, not linked firmware or execution.
- **A4-H**: Run-11 runtime-unit and affected runtime-integration logs: actual retained API/format/debug/stdlib/tier/differential/I/O/restart assertions, required api_smoke/debug_control/complete_program/runtime_reliability vertical; runtime cross-warnings supplementary only.
- **A4-S**: Run-11 source_free_cycle, source_free_cycle_contract, tasks/tasks_fb/scheduler_resource and saved trace assertions: nominal periods, sampled edges, ordering, process images and retain-before-output failure behavior.
- **A4-N**: Run-11 generated numeric-v2.stbc and core/tests/source_free_numeric.rs: ten LREAL reference results bounded by 8*f64::EPSILON*max(abs(reference),1), exact root/power/sum/scaled TIME/command expectations; existing portable_numeric_contract and shared stdlib conversion/helper/FB suites. These are finite native observations, not general numerical proof.
- **A4-R**: Run-11 source_free_assignment/source_free_readonly/source_free_restart_graph, compiler-free hosted cases and core engine units: typed/COW/constant/lifetime gates, alias/default frequency, class/static/NULL binding, initializer work/deadline, output transactions and restart graphs.
- **A4-Q**: Run-11 complete ledger, frozen source manifest/archive, generated STBC hashes, commands/environment, independent review manifests, affected lint/cross-warning/supply-chain/architecture/render/drift/format/diff outcomes. Metadata/provenance advisories remain individually classified. No release guard, commit/push or physical qualification implied.

- **A4-COST**: Run-11 source_free_execution_cost, borrowed_reference_allocations and source_free_cycle_contract: 3,000 declarations / 2,000 stores, 2,000 bound inputs, sparse 1 MiB marker publication and actual zero reference-path allocations. No resource limit was raised to conceal cost.
- **A4-CORRECTIONS**: [Finding-by-finding disposition](../../../notes/runtime-portability/a4-execution.md#external-review-disposition-evidence), [complete Rust identity](../../../notes/runtime-portability/a4-review/run-11-final-rust-identity.json), and independent acceptance cover RH1/RM2/RM3/RL1–7/RQ1–5, including generic counters, all requested standard blocks, falling edges, hierarchical I/O, exact diagnostics, deadline boundaries and forged staging writes.

### A1 evidence keys

All R3 references identify the unchanged **tested source snapshot**, not a new validation of the subsequent bookkeeping correction. See the [execution record](../../../notes/runtime-portability/a1-execution.md) and its relative evidence links.

- **R3**: all 24 steps pass in [run-3 ledger](../../../notes/runtime-portability/a1-evidence/run-3-ledger.tsv); exact commands/environment/frozen identity are beside it.
- **R3-S**: core `src/task/readiness.rs`, core readiness/case tests, host `tests/tasks.rs` and `scheduler_resource`; [native runtime output](../../../notes/runtime-portability/a1-evidence/run-3-runtime-tests.txt) and [core output](../../../notes/runtime-portability/a1-evidence/run-3-core-tests.txt).
- **R3-B**: workspace/core manifests, core `collections.rs`, `tests/portable_foundations.rs`, isolated F401/C6 checks/feature trees and `.github/workflows/ci.yml`.
- **R3-N**: core `numeric/math.rs`, arithmetic/TIME operators, hosted numeric helpers, `tests/portable_numeric_contract.rs`, numeric/VM/vertical suites; finite reference output is in the native runtime log.
- **R3-M**: core `value/layout.rs`; `record_value_slot_layout` in the core output plus both target build bounds.
- **R3-R**: [review messages](../../../notes/runtime-portability/a1-evidence/review-record.md), architecture/diagram/CI check statuses in the run-3 ledger.

## Batch, review and failure record

A2 batch 1 is retained as failed: 15 required PASS, 2 FAIL (Clippy, unnecessary architecture exemption), 2 UNRUN (diagrams/drift); advisory metadata failed. All 381 native assertions passed. Independent source review completed before the batch. Separately authorized run 2 closed lint/architecture/diagrams but failed three native artifact pins. Separately authorized run 3 passed 25 affected native tests and format/diff checks; the original relocation snapshot was verified, before the expanded corrections reopened A2. Full failures and reviewed corrections remain in A2-R1/R2/R3.


Historical A1 scope table, before release integration: three explicitly authorized
batches ran. These outcomes do not describe the later merge/release status in the
current checkpoint. The field list below remains the template for later scopes.

| Batch | Source / review | Result and disposition |
|---|---|---|
| A1 run 1 | Initial A1 source review; initial frozen source retained | Failed: fixture syntax, MySQL lint and dependency advisories; later host suites unrun. [Ledger](../../../notes/runtime-portability/a1-evidence/run-1-ledger.tsv). |
| A1 run 2 | Security/metadata remediation reviewed; TLS test gap corrected before batch | Failed: duration operand-context lowering and LSP lint; advisory metadata wording failed. [Ledger](../../../notes/runtime-portability/a1-evidence/run-2-ledger.tsv). |
| A1 run 3 | Five-file review hash `64550b9f0909f4292b53cdfcc583c79a469e63418d77a1dbf0c31a26e7abd992`; 73-file frozen manifest retained | Passed: 23 required plus one advisory step; no automatic retry. [Ledger](../../../notes/runtime-portability/a1-evidence/run-3-ledger.tsv), [commands and identities](../../../notes/runtime-portability/a1-execution.md). |
| Post-closeout bookkeeping | Detailed ledger, registry links, gate inventory and portable evidence corrected after review | Documentation/metadata only. No run 4 and no fresh validator result. A1 remains verified, not push-ready. |

A4 batch history is retained independently of the historical A1 table above:

| Batch | Required result | Evidence |
| --- | --- | --- |
| A4 run 1 | 10 PASS, 12 FAIL, 4 UNRUN | [Ledger](../../../notes/runtime-portability/a4-evidence/run-1/ledger.tsv) |
| A4 run 2 | 16 PASS, 8 FAIL, 2 UNRUN | [Ledger](../../../notes/runtime-portability/a4-evidence/run-2/ledger.tsv) |
| A4 run 3 | 23 PASS, 5 FAIL | [Ledger](../../../notes/runtime-portability/a4-evidence/run-3/ledger.tsv) |
| A4 run 4 | 27 PASS, 1 FAIL | [Ledger](../../../notes/runtime-portability/a4-evidence/run-4/ledger.tsv) |
| A4 run 5 | 28 PASS; both advisories PASS | [Ledger and frozen evidence](../../../notes/runtime-portability/a4-evidence/run-5/README.md) |
| A4 run 6 | 17 PASS, 10 FAIL, 2 UNRUN; both advisories PASS | [Ledger and frozen evidence](../../../notes/runtime-portability/a4-evidence/run-6/README.md) |
| A4 run 7 | 17 PASS, 10 FAIL, 2 UNRUN; both advisories PASS | [Ledger and frozen evidence](../../../notes/runtime-portability/a4-evidence/run-7/README.md) |
| A4 run 8 | 19 PASS, 8 FAIL, 2 UNRUN; both advisories PASS | [Ledger and frozen evidence](../../../notes/runtime-portability/a4-evidence/run-8/README.md) |
| A4 run 9 | 26 PASS, 3 FAIL; both advisories PASS | [Ledger and frozen evidence](../../../notes/runtime-portability/a4-evidence/run-9/README.md) |
| A4 run 10 | 28 PASS, 1 FAIL; both advisories PASS | [Ledger and frozen evidence](../../../notes/runtime-portability/a4-evidence/run-10/README.md) |
| A4 run 11 | 29 PASS; both advisories PASS | [Ledger and frozen evidence](../../../notes/runtime-portability/a4-evidence/run-11/README.md) |

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
batch, tests, generators or commits were run for either amendment. A2 and later implementation gates were open at that document checkpoint; A1 subsequently passed run 3 and A2 closed through A2-R6/R7/R8.

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

A2 expanded corrections are verified through A2-R6/R7/R8. Resume from the current checkpoint; no A3 or A2 commit/push is authorized. A1 publication and #129 integration are separate from A2 implementation.

A1 accepted evidence: [execution record](../../../notes/runtime-portability/a1-execution.md) in the A1 implementation checkout; full logs/requirement audit are linked there. Three batches were explicitly authorized separately; none was an automatic retry. That execution record covers A1 only; A2 checkmarks use the final A2-R6/R7/R8 allocation; earlier A2 records are historical.


### Historical post-run-4 correction review checkpoint

Latest external-review corrections are implemented and incrementally reviewed without running
checks. The explicit [130-path manifest](../../../notes/runtime-portability/a2-evidence/pre-run-5-review-manifest.json)
has SHA-256 `98670d41d8846c0200d4050933c24439d632779667cbb75691d5035f7b6992b9`;
the independent reviewer reproduced its records and complete exclusion list. This is source
review only, not a fresh all-file audit or test result. At that checkpoint A2 items remained open and run 5 awaited authorization. The subsequent
authorizations and executed corrections are recorded below and in the final execution note.

A2-R5: [complete stopped-run ledger](../../../notes/runtime-portability/a2-evidence/run-5-complete-ledger.tsv); source-freeze preparation failed before all native checks. It did not close A2; later A2-R6/R7/R8 did.


### A2 final evidence keys

- A2-R6: [ledger](../../../notes/runtime-portability/a2-evidence/run-6-ledger.tsv): core 204,
  portable 53, i686 167 assertions pass; both MCU checks, no-dev graphs, supply chain,
  architecture and rendered diagrams pass. Other failures preserved.
- A2-R7: [ledger](../../../notes/runtime-portability/a2-evidence/run-7-ledger.tsv): runtime
  unit 3,858 and tooling 102 assertions pass; Clippy/cross-target/metadata pass. Two failures retained.
- A2-R8: [ledger](../../../notes/runtime-portability/a2-evidence/run-8-ledger.tsv): all nine
  required steps pass; VM 44 assertions close the corrected NULL fixture; formatting complete.

[Final execution record](../../../notes/runtime-portability/a2-execution.md) explains the
passing union, exact identities and unrun later scopes. No single earlier failed batch is
relabeled passed. Only evidence and closeout documents changed after the final source copy.

The user subsequently authorized the local A2 commit and starting A3 on a separate branch.
Shared release-guard parity files already owned by #129 are excluded from this A2 commit;
they remain preserved locally and will be inherited through upstream integration.


## Historical A3 implementation checkpoint

A2 commit: `9f62fd09181c222ae6f8802128600708e3a304ea`. Canonical bootstrap copied and
verified from `/home/johannes/projects/trust-platform` to this A3 worktree. The three
shared guard parity files remain inherited local differences pending #129 integration.

Design constraints confirmed by source review: ordinary aggregate expression emission
currently rejects initializer aggregates; POU type records omit private instance layout;
initializer lookup sees only earlier frame locals then outer storage; method/function static
ownership differs; concrete reference owners require explicit construction mappings; the
existing execution entrypoint must reject 2.0 until A4 consumes its mandatory semantics.
Global and type defaults must come from authoring declarations/catalogs, not current values.
At that implementation checkpoint, no A3 build, test, validator, formatter, firmware action or commit had run. Run 1 and its failures are recorded below.


### Historical A3 first-batch preparation

The connected 2.0 producer, portable admission/disassembly and native test corpus
are prepared on `feat/runtime-portability-a3` at A2 base `9f62fd091`. Independent
read-only reviews corrected static visibility, frame metadata, dense storage,
lexical identity and source API issues. All three A3 items remain unchecked until
executable evidence is reconciled. At that checkpoint the first consolidated batch was prepared but
had not started; see [A3 execution record](../../../notes/runtime-portability/a3-execution.md).
The new builder checkout uses the same base and receives the canonical rule files
manually. No A3 commit, push, A4 implementation or board execution is authorized.


### Historical A3 run 1 failed; corrections prepared

The first consolidated batch finished on 9 October 2026 with 12 required failures
(11 compilation-dependent steps plus architecture) and 3 dependent steps unrun.
No native test assertions passed; no MCU compilation/execution claim is available.
Supply chain, formatting and advisory metadata checks passed. All A3 boxes remain
open. Evidence: [run-1 ledger](../../../notes/runtime-portability/a3-evidence/run-1-ledger.tsv)
and [execution/correction record](../../../notes/runtime-portability/a3-execution.md).

Compiler-root corrections and a focused stack-validation function split are
prepared locally and independently reviewed without actionable findings. They are untested.
No second batch, commit, push or A4 work is authorized. Next action: obtain explicit authorization for one consolidated validation batch;
its command plan is prepared, with unchanged dependency evidence reused from run 1.


### Historical A3 continuation authorization

After run 1 corrections and independent review, the user replied “yes, you dont
need to ask” to the additional-batch request. This authorizes run 2 and necessary
A3 follow-up validation after complete fixes/review, without repeating permission
questions. Preserve each frozen batch and full failure ledger; do not retry
unchanged failing commands or weaken gates. A3 commit/push and A4 remain outside
this authorization. Coordinate builder use with the separate #129 release guard.

### Historical A3 run 2 checkpoint

Portable/core tests and both MCU library checks passed; hosted compilation and
diagram rendering failed. Core source remains unchanged by the prepared facade
and diagram fixes. A3 items remain open. See the current checkpoint and A3
execution record for counts, evidence and standing continuation authorization.

The changing-input fixture uses the IEC-permitted reference initialization in
spec 12 §11.5.10: an FB array element selected by the current method input. It
preserves source constant-expression diagnostics and must be evaluated per call
by the A4 oracle.


### Historical A3 run-4 closeout

Historical run-4 closeout: all three A3 items were checked using the source-compatible evidence chain above. They were reopened for the subsequent external-review corrections; those changes are now verified by the separate R5/R6 closeout below.
P/Q compatibility policy is recorded in spec 12; actual product versions remain a
pre-P release gate. Existing behavior tests are retained for A4's shared engine.
The generated 2.0 artifact has not been executed by that engine. No A3 commit,
push, A4 implementation or board work was performed. Only closeout docs/evidence
changed after the successful frozen candidate; Rust and fixture bytes are preserved.

### Historical A3 external-review correction batch

A3-R5 completed: all gates passed except three authoring-fixture assertions. A3-R6 subsequently passed every planned step. All review findings and quality recommendations are
implemented. Independent read-only review identified and rechecked the expression-only
type discovery fix, edge-test field correction and canonical-action rejection tests.
The retained 113-path review manifest/archive has identity
`5b44303ab8373e80235ff678ec6a66cfb1337446ec5e94da6be64ad439555f5c`.
At that pre-batch checkpoint no native or MCU result was claimed for the correction tree. The subsequently completed consolidated batch includes
all changed core/host layers; no commit, push, A4 or hardware execution is authorized.

### Historical A3 correction closeout

All external findings and quality recommendations are implemented and verified:

| Review area | Implemented result | Evidence |
|---|---|---|
| Staging writes | CFG must-proven staging destinations, native target checks, no synthetic owner | R5 core/i686 and R6 forged-global authoring regression |
| Artifact demand | Reachable stdlib templates, expression-only type discovery, canonical recipe sharing | R6 pruning, SIZEOF and canonical-link positive/negative tests; R5 generated 6,164-byte fixture |
| Missing features | Edge, retain, partial alias, configuration, class and lowered partial-write coverage | R6 31 authoring + four boundary tests |
| Diagnostics and tooling | Specific rejection reasons, explicit stable disassembly, strict prefixes | R5 core + R6 authoring; generated text retained |
| Code quality | Named opcodes, shared reference helpers, one partial table and invalid helper, grouped state/typed IDs, prebuilt action indexes | Independent review; R5/R6 lint/compile/native evidence |
| Provenance | README indexed; exact current review input manifests and archives retained; old digests labeled historical | Artifact index and review archives |

Closeout-only documentation changes follow the run-6 freeze; no Rust or fixture bytes
were changed afterwards. Scope verification is not an exact-SHA release artifact,
source-free execution or hardware qualification. No A4 implementation occurred.

### Historical local A3 commit and initial A4 authorization

The owner authorized a local A3 commit and a separate A4 implementation branch.
`19fbdad18` records canonical workflow files (byte-identical to the independently
reviewed A1 integration candidate); `7e6938a75` records the tested A3 implementation,
format specification and fixtures. This evidence/checkpoint commit completes the
local A3 series. No extra test run or A3 publication was performed. A4 has one
authorized consolidated batch after implementation and independent review; it
does not inherit A3's follow-up-run authorization.

### Historical initial A4 execution checkpoint

Source/command map and implementation decisions are in
[the A4 execution record](../../../notes/runtime-portability/a4-execution.md).
A4 has one authorized validation batch after completed implementation and review.
No automatic retry, commit/push, board work or A1/A2 release change is authorized by
that initial development authorization. At that checkpoint all RTP-A4 boxes were open.

### Historical A4 run-5 scope closeout

The current checkpoint and A4 evidence keys supersede that initial authorization
checkpoint: standing reviewed correction cycles were subsequently authorized. Runs
1–4 retained failures; run 5 passed every planned step. See the full [batch history](../../../notes/runtime-portability/a4-evidence/run-5/README.md).
At that historical checkpoint RTP-A4-01..03 were closed. The external review below
reopened them; current corrections remain unverified. M2A remains open until
actual F401 scope B execution.
No A4 commit, push, hardware work or later scope was performed.


### A4 external-review correction scope

The owner requested every finding and recommendation fixed. Run 5 remains historical
proof of the prior source. Run 11 and final independent reconciliation verify every
correction below; A4-CORRECTIONS provides the finding-by-finding evidence.
No limits may be raised to conceal program-size costs, and no failure assertion may
be weakened. Requirement meanings are extended by spec 34's execution-cost contract
and spec 12's dedicated fault identities. All work stays in the A4 worktree.

- [x] A4-RH1: Prepare indexes once; destination-only output transactions and input staging; binding-based image work. Native realistic-size regression.
- [x] A4-RM2: Borrowed reference policy paths; hosted zero-allocation regression.
- [x] A4-RM3: Dedicated fault variants/codes, owning checks and native fault assertions.
- [x] A4-RL1: Preserve raw evidence externally; keep reviewable summaries and hash/location manifests in git; ignore scratch artifacts.
- [x] A4-RL2: Cache borrow failure is a miss, including instance type-cache callers.
- [x] A4-RL3: Bounded deadline polling stride with explicit boundary checks.
- [x] A4-RL4: One execution budget across dispatcher, helpers, initialization and hosted tiers.
- [x] A4-RL5: Remove unreachable construction branches; correct executed trace header.
- [x] A4-RL6: Software checklist closure explicitly excludes later board replay.
- [x] A4-RL7: Source-free TOF/TP/counter/trigger/bistable/falling-edge/hierarchical-I/O/deadline/staging-forgery execution coverage.
- [x] A4-RQ1: Group EngineState lifecycle, work, image and construction state by responsibility.
- [x] A4-RQ2: Named record predicates and one partial-access wire mapping.
- [x] A4-RQ3: One prepared immutable standard-library registry, reused during restart.
- [x] A4-RQ4: Documented portable facade; hide mutable VM internals and remove blanket docs suppressions.
- [x] A4-RQ5: Full dirty-Rust review manifest, verbatim-move equivalence inventory and independent review of each authored slice, plus independent final evidence reconciliation.

Resume status: all external-review changes are implemented and independently
verified against run 11. The final 239-path Rust identity and independent evidence
reconciliation are linked above. All A4 software items are closed; physical board
replay, release qualification and later bounded profiles remain their own scopes.
No A4 commit or push occurred. The A2 integration agent continues separate release
work. No further A4 test run is planned after the passing consolidated batch.
