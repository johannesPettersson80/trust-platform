# Runtime portability implementation checklist

Behavior authority: [specification 34](../../../specs/34-runtime-portability.md),
[runtime engine](../../../specs/11-runtime-engine.md), and
[STBC format](../../../specs/12-bytecode.md). This file records execution and evidence;
it does not create additional product requirements. Existing closed extraction boards stay closed.

## Current checkpoint

| Field | Recorded state |
|---|---|
| Updated | 9 October 2026 — A3 review corrections scope-verified and locally committed; A4 newly authorized |
| Checkout | `/home/johannes/projects/trust-platform-portability-a3`, branch `feat/runtime-portability-a3`, base `9f62fd09181c222ae6f8802128600708e3a304ea` (verified A2). |
| Bootstrap | Canonical AGENTS.md and full .codex/skills manually copied from `/home/johannes/projects/trust-platform`; 21 files matched locally and in the isolated builder checkout at A2 base. Reviewers verified independently. |
| Authorization | User subsequently authorized committing verified A3 locally and starting A4 on a separate branch with independent review and one consolidated validation batch. No A4 push or hardware work. A3 follow-up-run approval does not silently authorize A4 retries. |
| Scope | A3: exact STBC 2.0 layouts and producer/lowering, dual-major byte reader/validator, disassembly, opt-in emission and legacy/source-free version-boundary tests. |
| Status | A3 verified through A3-R5/R6. Run 5 passes production/core/i686/MCU/full-host/architecture/rendering; its three source-fixture failures are preserved and corrected. Run 6 passes all 31 authoring tests, four authoring-boundary tests and every planned lint/warning/format/metadata step. No release qualification. |
| Source identity | A3-R5 frozen manifest SHA-256 `21c8b6ab9fbfb6f7e67490882342ab6730ff2392ad2a094a1a47de1bc0ef062e`; A3-R6 307-record manifest `2faf6f16710ae55a60e227c52b9c2c5e965be541be335d88bdea14a403700717`. Only two Rust test files changed between those runs. Fixture: 6,164 bytes, SHA-256 `0a09a7190f26bbe3481ca7ae5b7f16a130ae4dcb2ee21937b456dd8581a87e5a`. |
| Limitations | A3 proves 2.0 authoring/byte validation only. No source-free execution, firmware link, board/WCET/MCU peak-memory evidence or native macOS/Windows execution. No A3 release guard/push; no A4 work. |
| Integration | #129 merged as `ecbcb08ddd`; its release agent continues verification. A separate agent owns reviewed A1/A2 integration onto main; A1 candidate `9e5a5896f` has started its integration batch. This A3 worktree remains based on committed A2 `9f62fd091`. |
| Next action | Preserve verified A3 code and complete retained evidence while agents finish the A1/A2 integration/release chain. A3 implementation is locally committed as `7e6938a75`; its evidence closeout follows. Create the A4 branch from this completed series. No A3 push or hardware work. |

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
| ARCH-01 | A1–A4, B/E/H/C; QH/QF/QE | A1 partial: shared numeric/control and scheduler assertions pass (R3-N, R3-S); full artifact/platform parity remains open. |
| ARCH-02 | A1–A4, B/E/H/C; QH/QF/QE | A1 partial: existing core extended without a platform fork (R3-B); shared loader is now qualified in A2-R6/R7/R8; shared executor and adapters remain open. |
| ARCH-03 | A1–A4, B/E/H/C; QH/QF/QE | A1 partial: core dependency boundary compiles on both targets (R3-B); complete execution-boundary extraction remains open. |
| ARCH-04 | A1–A4, B/E/H/C; QH/QF/QE | A1 foundations only (R3-B/N); complete profile and platform claims remain open. |
| ARCH-05 | C; QH/QF/QE | pending — open |
| ARCH-06 | A2/A3/A4/C; QH/QF/QE | A2 partial: raw container and borrowed validated token are distinct types (A2-R6); preparation/admission and mutable execution-state separation remain A3/A4/C. |
| ARCH-07 | B/E; QF/QE | pending — open |
| ARCH-08 | A1/A4/H; QH | A1 partial: selected hosted regression corpus passes (R3-N/S); full hosted migration remains open. |
| ARCH-09 | H; QH | pending — open |
| ARCH-10 | B/E; QF/QE | pending — open |
| MODEL-01 | A2/A3/A4/H/C; QH/QF/QE | A3 artifact ownership/layout and explicit version selection pass A3-R5/R6; pointer-independent wire fields are specified. Prepared executable/mutable-state ownership remains A4. |
| MODEL-02 | A2/A3/A4/H/C; QH/QF/QE | A3 serializes ordered initialization actions and callable defaults, with visibility/identity checks (A3-R5/R6). Pre-RUN executable preparation and cache removal remain A4. |
| MODEL-03 | A2/A3/A4/H/C; QH/QF/QE | pending — open |
| MODEL-04 | A2/A3/A4/H/C; QH/QF/QE | pending — open |
| BUILD-01 | A1/A2/A4/H/U; QH/QF/QE | A1/A2 no-default core compilation passes on F401/C6 (R3-B, A2-R6). Source-free engine construction remains A4. |
| BUILD-02 | A1/A2/A4/H/U; QH/QF/QE | Separate F401/C6 library checks and no-dev feature graphs pass (A2-R6); host procedural-macro features are distinct from MCU library dependencies. |
| BUILD-03 | A1/A2; U on modernization | A1 feature/map split preserved; locked portable CRC edge qualified in A2-R6. Toolchain/edition modernization remains U. |
| BUILD-04 | A1/A2/A4/H/U; QH/QF/QE | A1 assembly and A2 library checks cover core target compilation only. Firmware link/startup/board execution remain B/E. |
| BUILD-05 | A1/A2/A4/H/U; QH/QF/QE | Complete byte-oriented implementation and full validator are shared; both MCU graphs pass (A2-R6). Shared executor remains A4/H. |
| BUILD-06 | A1/A2/A4/H/U; QH/QF/QE | Hosted API/format/runtime corpus and unit tests pass (A2-R6/R7/R8); compiler lowering remains hosted and product compositions remain H. |
| BUILD-07 | A4/H; QH | pending — open |
| BUILD-08 | H/U; QH | pending — open |
| BUILD-09 | A1/A2/A3/A4; C for optional no-CAS | F401/C6 no-default core builds pass A3-R5/R6 with fresh production-core checks in run 5. No C3/no-CAS or atomic-retirement qualification claimed. |
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
| LOAD-01 | A2/A3/A4/C; QH/QF/QE | A3-R5/R6 decode and validate saved 2.0 without HIR and reject 1.x in the source-free consumer. Fresh executable state remains A4. |
| LOAD-02 | A2/A3/A4/C; QH/QF/QE | A3 construction/root/initializer/access records, mandatory sections and source producer pass A3-R5/R6. Executable construction and complete target admission remain A4/C. |
| LOAD-03 | A2/A3/A4/C; QH/QF/QE | A3-R5/R6 adds 2.0 version/descriptor/range/private-result rejection cases; A3-R5/R6 adds forged frame/static/alias/image cases. Profile capability and RUN admission remain A4/C. |
| LOAD-04 | A2/A3/A4/C; QH/QF/QE | A2 analysis budget/index/CFG changes pass native exhaustion, peak-memory and maximum-instruction/reference fixtures (A2-R6). MCU preparation budgets and measurements remain C/QF/QE. |
| LOAD-05 | A2/A3/A4/C; QH/QF/QE | pending — open |
| LOAD-06 | A2/A3/A4/C; QH/QF/QE | pending — open |
| LOAD-07 | A2/A3/A4/C; QH/QF/QE | Borrowed validated-token invariants retained (A3-R5/R6); hosted 2.0 application is rejected before replacement and legacy execution continues (A3-R5/R6). Prepared engine/profile admission remains A4. |
| LOAD-08 | A2/A3/A4/C; QH/QF/QE | pending — open |
| LOAD-09 | A2/A3/A4/C; QH/QF/QE | A3-R5/R6 pins failed 2.0 application preserving legacy state/execution through both hosted APIs. Full transactional preparation, bounded overlap and update admission remain A4/C. |
| LOAD-10 | A3/A4; B/E/QH/QF/QE | A3 format/producer portion verified (A3-R5/R6): complete representative 2.0 artifact, explicit producer, legacy/source-free version boundary. Construction and execution through the common dispatcher remain A4. |
| LOAD-11 | A3/A4; B/E/QH/QF/QE | A3 emits ordered executable initialization and type/member-default recipes with declared contexts/visibility (A3-R5/R6). Per-call execution, shared budgets and retirement of the Expr runtime path remain A4. |
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
| COMPAT-01 | A1/A3/A4/H/C; QH/QF/QE | Existing 1.x/runtime behavior suites pass A3-R5/R6; native source diagnostics are preserved. 2.0 execution parity remains A4 and platform qualification. |
| COMPAT-02 | C; QH/QF/QE | pending — open |
| COMPAT-03 | A1/A3/A4/H/C; QH/QF/QE | A3-R5/R6 preserves hosted OOP/interface regressions and emits a 2.0 TON/array/interface/reference-initializer fixture. Cross-engine/platform execution parity remains A4/B/E/H. |
| COMPAT-04 | A1/A3/A4/H/C; QH/QF/QE | A1 host public ordered-map type retained and tested (R3-B); remaining hosted interfaces and platforms remain open. |
| COMPAT-05 | A1/A3/A4/H/C; QH/QF/QE | A1 intentional libm migration has old/new finite reference output and passing threshold/fault assertions (R3-N); broader qualification remains open. |
| COMPAT-06 | A1/A3/A4/H/C; QH/QF/QE | A3-R5/R6 saves and byte-compares the 2.0 fixture. The 25 ms/10 ms logical execution oracle and board replay remain A4/B/E; no execution trace is claimed. |
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
| TEST-01 | A2/A3/A4/B/E/H/C; QH/QF/QE | A3-R5/R6 cover malformed initializer records/ranges, forged frame/static/alias/image metadata and source/version boundaries. Execution, disturbance and deployment qualification remain open. |
| TEST-02 | B/E/H/U/C; QH/QF/QE | A3 source/build/artifact identities, commands, environments, reviews and all failure ledgers retained in A3-R1–R4. Physical memory/timing/board qualification remains later work. |
| TEST-03 | QH/QF/QE | pending — open |
| TEST-04 | QH/QF/QE | pending — open |
| TEST-05 | A2/A3/C; QH/QF/QE | A3-R5/R6 add systematic malformed-construction mutations and failed hosted activation assertions. Broad fuzz/deployment/authorization and platform qualification remain open. |
| MOD-01 | U; QH/QF/QE for changed tuples | pending — open |
| MOD-02 | U; QH/QF/QE for changed tuples | pending — open |
| MOD-03 | U; QH/QF/QE for changed tuples | pending — open |
| MOD-04 | U; QH/QF/QE for changed tuples | pending — open |
| MOD-05 | U; QH/QF/QE for changed tuples | pending — open |
| MOD-06 | U; QH/QF/QE for changed tuples | pending — open |
| DESIGN-01 | A1–A4/B/E/U/H/C; QH/QF/QE | A1 source reviewed for simple shared collections/numeric/operand-context changes; no unresolved findings (R3-R). Later designs remain open. |
| DESIGN-02 | A1–A4/B/E/U/H/C; QH/QF/QE | A1 preserves host type identity, coercion and fault assertions (R3-B/N); admission/generation/bounded-resource APIs remain open. |
| DESIGN-03 | A1–A4/B/E/U/H/C; QH/QF/QE | A1 independent review, architecture and diagram checks pass (R3-R); later shared-engine/platform extraction remains open. |

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


### A3 run 1 failed; corrections prepared

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


### A3 continuation authorization

After run 1 corrections and independent review, the user replied “yes, you dont
need to ask” to the additional-batch request. This authorizes run 2 and necessary
A3 follow-up validation after complete fixes/review, without repeating permission
questions. Preserve each frozen batch and full failure ledger; do not retry
unchanged failing commands or weaken gates. A3 commit/push and A4 remain outside
this authorization. Coordinate builder use with the separate #129 release guard.

### A3 run 2 checkpoint

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

### A3 external-review correction batch

A3-R5 completed: all gates passed except three authoring-fixture assertions. A3-R6 subsequently passed every planned step. All review findings and quality recommendations are
implemented. Independent read-only review identified and rechecked the expression-only
type discovery fix, edge-test field correction and canonical-action rejection tests.
The retained 113-path review manifest/archive has identity
`5b44303ab8373e80235ff678ec6a66cfb1337446ec5e94da6be64ad439555f5c`.
At that pre-batch checkpoint no native or MCU result was claimed for the correction tree. The subsequently completed consolidated batch includes
all changed core/host layers; no commit, push, A4 or hardware execution is authorized.

### A3 correction closeout

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

### Local A3 commit and A4 authorization

The owner authorized a local A3 commit and a separate A4 implementation branch.
`19fbdad18` records canonical workflow files (byte-identical to the independently
reviewed A1 integration candidate); `7e6938a75` records the tested A3 implementation,
format specification and fixtures. This evidence/checkpoint commit completes the
local A3 series. No extra test run or A3 publication was performed. A4 has one
authorized consolidated batch after implementation and independent review; it
does not inherit A3's follow-up-run authorization.
