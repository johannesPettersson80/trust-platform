# Full Architecture Refactor Program Checklist

Status: Complete for required architecture-program boards and exit criteria; recurring guard rules remain active.
Owner: Architecture/runtime/HIR team
Scope: execution program for the full software-map audit findings, SOLID/KISS cleanup, and zero-silent-bug posture.

This is the umbrella checklist. Individual execution boards own the detailed work. A single runtime-core split is not enough to satisfy the architecture goal.

Navigation guard: use `architecture-workboard-index.md` before resuming after a crash, context reset, or branch switch.

Post-closeout runtime safety follow-up: `runtime-safety-fail-closed-checklist.md` is the active dedicated board for runtime-internal fail-closed inventory and fixes. It is outside the already-complete required architecture-program boards, but it inherits the same doctor-first, tests-first, and source-derived evidence rules.

## Program Rule

Unchecked `ARCHPROG-RULE-*` rows are recurring guardrails for future branches, not open implementation boards.

- [ ] `ARCHPROG-RULE-01` Do not claim "0 silent bugs" from behavior-preserving refactors alone.
- [ ] `ARCHPROG-RULE-02` Do not claim "clean SOLID/KISS" while `trust-runtime` still mixes product runtime, workbench tooling, HMI/web/control/cloud ownership, and unchecked large-file hotspots.
- [ ] `ARCHPROG-RULE-03` Every architecture claim must be backed by source-derived facts, a doctor rule, mutation/fuzz evidence, or a documented manual exception.
- [ ] `ARCHPROG-RULE-04` Each branch must state which audit finding it closes and which findings remain open.
- [ ] `ARCHPROG-RULE-05` Do not merge a refactor branch that weakens an existing behavior lock, doctor rule, or generated-map check.
- [x] `ARCHPROG-RULE-06` Use staged validation cadence: run focused tests and doctor checks during implementation, and reserve `just test-all` for merge/release readiness, board-completion gates, large cross-crate refactors, or rebases that touch shared APIs.
- [x] `ARCHPROG-RULE-07` Do not claim production-ready secure remote MQTT until `DEPHYG-FOLLOW-01` implements explicit MQTT TLS/mTLS. Closed in `v0.24.7`: MQTT TLS/mTLS config and `rumqttc` TLS transport wiring are implemented, and remote plaintext remains gated by `allow_insecure_remote = true`.
- [x] `ARCHPROG-RULE-08` Do not treat the completed focused HIR mutation board as global HIR zero-silent-bug safety; all diagnostic-bearing HIR semantic decisions require the separate HIR zero-silent-bug board. Evidence: closed by the separate completed `hir-zero-silent-bug-refactor-checklist.md`, not by the earlier focused mutation board.

## Validation Cadence

- [x] `ARCHPROG-VAL-01` Every execution board must name its focused implementation-loop checks.
- [x] `ARCHPROG-VAL-02` `just test-all` is required before merge, release/customer-facing readiness claims, or marking a board complete unless the board records an explicit owner-approved waiver.
- [x] `ARCHPROG-VAL-03` Long suites such as OSCAT examples, mutation campaigns, fuzzing, Miri, Valgrind, sanitizer runs, and full benchmark sweeps are milestone/release/nightly gates unless the branch directly touches the covered behavior.
- [x] `ARCHPROG-VAL-04` If a focused gate fails, fix it before escalating to broader gates; do not use a passing full suite to hide a failing focused doctor/test.

## Required Execution Boards

- [x] `ARCHPROG-BOARD-01` Full-map architecture doctor: `architecture-doctor-full-map-execution-checklist.md`.
- [x] `ARCHPROG-BOARD-02` HIR mutation hardening: `hir-mutation-hardening-execution-checklist.md`.
- [x] `ARCHPROG-BOARD-03` Parser recovery hardening: `parser-recovery-hardening-execution-checklist.md`.
- [x] `ARCHPROG-BOARD-04` Runtime CLI product/workbench split: `runtime-cli-product-workbench-split-checklist.md`. Evidence: the detail board closes `RTCLI-EXIT-01` through `RTCLI-EXIT-06`; post-closeout follow-up promoted `trust-dev` into its own Cargo package owning `agent`, `commit`, `docs`, and `test`; `trust-runtime` retains deprecated forwarding aliases; shared helpers have explicit infrastructure rationales; and `FULLMAP-CHECK-06` enforces command/module/action classification plus migration policy.
- [x] `ARCHPROG-BOARD-05` Runtime host surface ownership: `runtime-host-surface-ownership-checklist.md`. Evidence: the detail board is complete with `RTHOST-EXIT-01` through `RTHOST-EXIT-05` checked, `FULLMAP-CHECK-07` active with approved ports, and final local full-map validation passing.
- [x] `ARCHPROG-BOARD-06` Runtime core/Linux host split: `runtime-core-host-split-execution-checklist.md`. Evidence: the detail board is closed with `RTSPLIT-EXIT-001` through `RTSPLIT-EXIT-010` checked, runtime-core split final workspace/release-readiness recorded in `ARCH-RTCORE-43`, and embedded support still explicitly deferred.
- [x] `ARCHPROG-BOARD-07` Dependency hygiene: `dependency-hygiene-execution-checklist.md`.
- [x] `ARCHPROG-BOARD-08` Runtime large-file split: `runtime-large-file-split-execution-checklist.md`. Evidence: the detail board is complete for its measured hotspot set, and `FULLMAP-CHECK-10` blocks unregistered runtime large-file regressions. Current remaining runtime `src`/`tests` files over 1,000 lines are tracked through `kiss.large_file_allowlist` owner/split metadata rather than claimed as eliminated.
- [x] `ARCHPROG-BOARD-09` Diagram semantic enforcement is added before diagrams are trusted as acceptance evidence.
- [x] `ARCHPROG-BOARD-10` Runtime VM mutation hardening: `runtime-vm-mutation-hardening-execution-checklist.md`. Evidence: the detail board is complete with `RTVMMUT-EXIT-01` through `RTVMMUT-EXIT-03` checked; selected call/register-IR/tier1 mutation shards have zero missed/timeout mutants; `FULLMAP-RUNTIMEVM-MUT` is reported by full-map doctor; and GitHub CI run `25253932423` passed for `df90e38e6`.
- [x] `ARCHPROG-BOARD-11` Unsafe/concurrency hardening: `unsafe-concurrency-hardening-execution-checklist.md`. Evidence: detail board is complete; `FULLMAP-CHECK-09` reports 61 production unsafe occurrences, 300 production panic-like occurrences, and 528 concurrency boundary occurrences with all production hotspots registered/classified; Miri, sanitizer, Valgrind, and deterministic poison-recovery gates pass; `cargo geiger` is advisory-partial with an exact workspace/tool blocker.
- [x] `ARCHPROG-BOARD-12` HIR zero-silent-bug refactor: `hir-zero-silent-bug-refactor-checklist.md`.

## Recommended Order

### Phase A - Automation First, But Only The Needed Automation

- [x] `ARCHPROG-A-01` Implement `architecture-doctor --full-map` enough to enforce branch-relevant boundaries.
- [x] `ARCHPROG-A-02` Add generated/report artifact output so reviewers can inspect the software map.
- [x] `ARCHPROG-A-03` Add forbidden-edge and forbidden-import policy loading.
- [x] `ARCHPROG-A-04` Add size/API trend reporting with blocking thresholds for configured KISS gates; public API growth is advisory only until its baseline snapshot exists.
- [x] `ARCHPROG-A-05` Do not spend this phase building dashboards that do not block a real architecture risk.

### Phase B - Silent-Bug Hardening Before Large Runtime Movement

- [x] `ARCHPROG-B-01` Close HIR mutation gap for `symbol_import`.
- [x] `ARCHPROG-B-02` Close HIR mutation gap for `type_check::const_eval`.
- [x] `ARCHPROG-B-03` Close HIR mutation gap for aggregate initializer validation.
- [x] `ARCHPROG-B-04` Add mutation gate with zero unexplained survivors for the focused HIR shard.
- [x] `ARCHPROG-B-05` Fix parser recovery bounded-scanner and fuzz/property tests.
- [x] `ARCHPROG-B-06` Close the full HIR zero-silent-bug architecture gap with semantic kernel, explicit resolver/validation outcomes, full semantic identity keys, and runtime/HIR declaration parity. Evidence: `hir-zero-silent-bug-refactor-checklist.md` is complete with final merge gate passed.

### Phase C - Runtime Boundary Policy Before Runtime Extraction

- [x] `ARCHPROG-C-01` Classify runtime binary commands as product, UI product, conformance/benchmark, or workbench/dev.
- [x] `ARCHPROG-C-02` Classify `web`, `hmi`, `ui`, `control`, and `runtime_cloud` ownership. Evidence: `runtime-host-surface-ownership-checklist.md` records owner categories in `xtask/config/full_map_policy.json`.
- [x] `ARCHPROG-C-03` Add doctor rules for product/workbench command boundaries.
- [x] `ARCHPROG-C-04` Add doctor rules for host-surface forbidden imports and approved ports. Evidence: `FULLMAP-CHECK-07` now enforces forbidden host-surface imports, owner categories, and direct web runtime-state bypass checks when approved ports are active.
- [x] `ARCHPROG-C-05` Freeze "no new top-level runtime module without subsystem decision note". Evidence: `FULLMAP-CHECK-10` now loads `kiss.runtime_top_level_module_decisions`, requires every current `crates/trust-runtime/src` top-level module to have a subsystem/owner/rationale/review-date/decision-note entry, and fails any new unclassified top-level runtime module.

### Phase D - Runtime Core/Linux Host Split

- [x] `ARCHPROG-D-01` Run `runtime-core-host-split-execution-checklist.md` only after Phases A-C have the required rules or explicit waivers. Evidence: Phase A-C rules are active through `FULLMAP-CHECK-01`, `FULLMAP-CHECK-02`, `FULLMAP-CHECK-05`, `FULLMAP-CHECK-06`, `FULLMAP-CHECK-07`, and the `ARCHPROG-C-05` top-level runtime module decision freeze; runtime-core split Phase 0 is now captured in `runtime-core-host-split-execution-checklist.md`.
- [x] `ARCHPROG-D-02` Treat this phase as behavior-preserving; no embedded product support claims. Evidence: runtime-core split Phase 0 note states the active scope is Linux behavior-preserving extraction and explicitly excludes STM32H7, Arduino Opta, ESP32, embedded T0, embedded EtherCAT, `no_std` product support, MCU protocol commitments, and embedded support claims.
- [x] `ARCHPROG-D-03` Keep behavior-lock tests ahead of code movement. Evidence: `runtime-core-host-split-execution-checklist.md` Phase 1 exit gates are complete before any production runtime code movement; VM/bytecode, cycle-boundary, scheduler, retain, watchdog/fault, and initializer/value behavior-lock rows are complete, with only explicitly conditional future movement rows left open.
- [x] `ARCHPROG-D-04` Keep host crate responsibility shrinkage visible, not hidden behind re-exports. Evidence: `runtime-core-host-split-execution-checklist.md` final exit criteria state the portable execution concerns owned by `trust-runtime-core`, the Linux-host concerns still owned by `trust-runtime`, and the remaining host/core compromises with owner/reason.

### Phase E - Remaining Runtime Host Cleanup

- [x] `ARCHPROG-E-01` Split workbench/dev command implementation after compatibility policy is decided. Evidence: `runtime-cli-product-workbench-split-checklist.md` closes BOARD-04 with `trust-dev` implementations, retained `trust-runtime` compatibility wrappers, public docs, terminal captures, full-map policy, and focused CLI compatibility tests.
- [x] `ARCHPROG-E-02` Split HMI/web/control/cloud surfaces behind ports/adapters. Evidence: `runtime-host-surface-ownership-checklist.md` is complete; HMI runtime access is behind control ports, HMI websocket event semantics are HMI-owned, runtime-cloud policy/projection modules own domain decisions, web routes remain transport adapters, and `FULLMAP-CHECK-07` prevents drift.
- [x] `ARCHPROG-E-03` Add owner/split notes for every runtime Rust file over 1,000 lines. Evidence: `runtime-large-file-split-execution-checklist.md` Phase 1 records the measured 2026-05-01 inventory; the BOARD-08 hotspot rows record completed splits; and `FULLMAP-CHECK-10` now fails unregistered runtime `src`/`tests` large files while reporting registered remaining large files with owner and split-plan metadata.
- [x] `ARCHPROG-E-04` Add KISS gates for module size, function size, public API growth, and top-level module growth. Evidence: `FULLMAP-CHECK-10` now enforces configured workspace module-size allowlists, runtime/core function-size allowlists, and the `trust-runtime` top-level module cap of `18`; `FULLMAP-P6-API` requires tracked public API baselines for `trust-runtime` and `trust-runtime-core`; `scripts/check_public_api_snapshots.sh` also tracks `trust-plcopen` and passes in update and check modes.
- [x] `ARCHPROG-E-05` Add runtime VM mutation gate before claiming zero silent bugs for runtime execution. Evidence: `runtime-vm-mutation-hardening-execution-checklist.md` is complete; manual shard gate command remains `TRUST_VM_MUTANTS_IN_PLACE=1 scripts/runtime_vm_mutation_shards.sh --run <shard>` from a clean tracked tree; and `cargo xtask architecture-doctor --full-map` reports selected runtime VM mutation evidence through `FULLMAP-RUNTIMEVM-MUT`.
- [x] `ARCHPROG-E-06` Add unsafe/concurrency risk register and focused Miri/sanitizer/Loom/Valgrind evidence before claiming memory/concurrency safety. Evidence: `unsafe-concurrency-hardening-execution-checklist.md` is complete; full-map policy owns the unsafe-site, panic-like, concurrency-boundary, and tool-gate registers; focused Miri/sanitizer/Valgrind gates passed and geiger has an exact advisory blocker.
- [x] `ARCHPROG-E-07` Close `DEPHYG-FOLLOW-01` by implementing explicit MQTT TLS/mTLS and security tests before any release note, docs page, or architecture report describes remote MQTT as production-secure.

## Deferred End-Of-Program Modernization

- [x] `ARCHPROG-FOLLOW-01` After the architecture refactor boards and release-blocking dependency/security work are closed, run a separate Rust modernization audit on top of the MSRV/toolchain bump from `1.85` to Rust `1.95.0` latest stable as of 2026-04-29. Evidence: workspace already targets Rust `1.95`, `time 0.3.47`, and `ratatui 0.30`; the audit removed unused direct `trust-runtime` dependency `thiserror`; `cargo audit --ignore RUSTSEC-2024-0421 --ignore RUSTSEC-2025-0009 --ignore RUSTSEC-2023-0071 --ignore RUSTSEC-2024-0336 --ignore RUSTSEC-2024-0388 --ignore RUSTSEC-2024-0436 --ignore RUSTSEC-2025-0010 --ignore RUSTSEC-2025-0134`, `cargo deny check`, `cargo machete --with-metadata crates`, `RUSTUP_TOOLCHAIN=1.95 cargo check --all-targets`, `cargo test -p xtask full_map -- --nocapture`, `cargo run -p xtask -- architecture-doctor --full-map`, `scripts/check_public_api_snapshots.sh`, and `python scripts/check_diagram_drift.py` pass locally; remaining raw-audit advisories are the policy-owned OPC UA/tiny_http/Zenoh transitive blockers documented in `deny.toml` and the final report.
  - Scope boundary: the security-driven MSRV bump belongs to the `v0.24.5` dependency-hygiene release, but broader syntax/API modernization remains intentionally deferred end-of-program work. Do not mix broad modernization into behavior-preserving refactor branches, and do not use modernization as evidence that current SOLID/KISS boards are complete.
  - Version changes performed for `v0.24.5`: update `[workspace.package].rust-version` from `1.85` to `1.95`; update the GitHub CI MSRV job name from `MSRV (1.85)` to `MSRV (1.95)`; update `dtolnay/rust-toolchain@1.85` to `dtolnay/rust-toolchain@1.95`; refresh dependency resolution and remove temporary Rust-1.85 compatibility pins where the newer toolchain permits patched dependencies.
  - Dependency/security reason to remember: `time >=0.3.47` fixes `RUSTSEC-2026-0009` but requires Rust `1.88`; `ratatui 0.30` fixes the transitive `lru` advisory path but requires Rust `1.86`; moving to `1.95` should prefer fixed dependencies over allowlists where compatible.
  - Current-release cleanup already done: runtime modulo checks that Clippy flags on Rust `1.95` were changed to `is_multiple_of` in bytecode decoding, constant-pool decoding, VM deadline checks, register-IR deadline checks, and runtime-cloud routing tests.
  - Validation required before completion: `cargo update` or targeted dependency updates, `cargo audit`, `cargo deny check`, scoped `cargo machete`, `RUSTUP_TOOLCHAIN=1.95 cargo check --all-targets`, `cargo test -p xtask`, `cargo run -p xtask -- architecture-doctor --full-map`, `just fmt`, `just clippy`, and `just test-all`.
  - Rust 1.86 notes to review: trait object upcasting; `slice::get_disjoint_mut` and `HashMap::get_disjoint_mut`; safe `#[target_feature]` functions; debug assertions for null pointer reads/writes/reborrows; `missing_abi` warning for implicit `extern` ABI; APIs such as float `next_up`/`next_down`, `Vec::pop_if`, `OnceLock::wait`, and additional const-stable string/slice helpers.
  - Rust 1.87 notes to review: `std::io::pipe` and process `Stdio` integration; safer architecture intrinsics when target features are enabled; inline `asm!` label operands; precise `impl Trait` capture in trait definitions; APIs such as `Vec::extract_if`, `LinkedList::extract_if`, slice `split_off*`, `String::extend_from_within`, `OsStr::display`, `Box<MaybeUninit<T>>::write`, `TryFrom<Vec<u8>> for String`, and unsigned pointer offset helpers.
  - Rust 1.88 notes to review: Rust 2024-only let chains in `if`/`while`; naked functions via `#[unsafe(naked)]`; `cfg(true)` and `cfg(false)`; Cargo automatic cache cleaning; APIs such as `Cell::update`, `HashMap::extract_if`, `HashSet::extract_if`, `hint::select_unpredictable`, proc-macro span location methods, slice `as_chunks`/`as_rchunks`, and related const-stable pointer/cell operations.
  - Rust 1.89 notes to review: const generic `_` inference in expressions; warn-by-default `mismatched_lifetime_syntaxes`; more x86 target features; cross-compiled doctests; `i128`/`u128` accepted in `extern "C"` where ABI-compatible; `x86_64-apple-darwin` demotion notice; APIs such as `File::lock`, `NonNull::{from_ref,from_mut}`, `Result::flatten`, `OsString::leak`, `PathBuf::leak`, and Linux TCP quickack helpers.
  - Rust 1.90 notes to review: LLD becomes the default linker for `x86_64-unknown-linux-gnu`; `cargo publish --workspace`; `x86_64-apple-darwin` demoted to Tier 2 with host tools; APIs such as unsigned integer signed-subtraction helpers, `IntErrorKind` trait impls, C string equality impls, and const-stable float rounding helpers.
  - Rust 1.91 notes to review: `aarch64-pc-windows-msvc` promoted to Tier 1; warn-by-default lint for raw pointers to locals escaping functions; strict integer arithmetic APIs; `Path::file_prefix`; atomic pointer arithmetic/bitwise APIs; `Duration::from_mins`/`from_hours`; `BTreeMap::extract_if`; `BTreeSet::extract_if`; `str::ceil_char_boundary`; `str::floor_char_boundary`; array `each_ref`/`each_mut` const stabilization.
  - Rust 1.92 notes to review: deny-by-default never-type future-compatibility lints; `unused_must_use` no longer warns for `Result<(), Infallible>`-style cases; Linux unwind tables emitted by default even with `panic=abort`; stricter `#[macro_export]` input validation; APIs such as `RwLockWriteGuard::downgrade`, `Box/Rc/Arc::new_zeroed*`, and BTree entry `insert_entry`.
  - Rust 1.93 notes to review: bundled musl updated to `1.2.5` for musl targets, improving static Linux DNS/networking behavior but carrying compatibility implications; global allocators can use thread-local storage; `cfg` attributes on individual `asm!` lines; APIs such as `String::into_raw_parts`, `Vec::into_raw_parts`, slice `as_array`/`as_mut_array`, `VecDeque::pop_front_if`/`pop_back_if`, `Duration::from_nanos_u128`, `char::MAX_LEN_UTF8`, `char::MAX_LEN_UTF16`, and `std::fmt::from_fn`.
  - Rust 1.94 notes to review: slice `array_windows`; Cargo config `include`; Cargo TOML 1.1 manifest/config parsing; APIs such as `LazyCell::get`/`get_mut`/`force_mut`, `LazyLock::get`/`get_mut`/`force_mut`, `Peekable::next_if_map`, `Peekable::next_if_map_mut`, `element_offset`, and new math constants.
  - Rust 1.95 notes to review: `cfg_select!`; `if let` guards in `match`; removal of stable custom JSON target-spec support; APIs such as `bool::try_from`, atomic `update`/`try_update`, `core::range`, `core::hint::cold_path`, pointer `as_ref_unchecked`/`as_mut_unchecked`, `Vec::push_mut`, `Vec::insert_mut`, `VecDeque` push/insert mutable helpers, `Layout` repeat helpers, and const-stable `fmt::from_fn` plus `ControlFlow` state checks.
  - Repo-specific modernization candidates already identified: keep `.windows(2)` / `.windows(3)` conversions to `array_windows` low priority and cosmetic; consider `get_disjoint_mut` only where it removes real borrow-workaround complexity; use `cfg_select!` only if a future branch introduces substantial platform cfg branching; evaluate Rust 2024 edition and let chains separately from the MSRV bump.
  - Official release-note references: `https://blog.rust-lang.org/2025/04/03/Rust-1.86.0/`, `https://blog.rust-lang.org/2025/05/15/Rust-1.87.0/`, `https://blog.rust-lang.org/2025/06/26/Rust-1.88.0/`, `https://blog.rust-lang.org/2025/08/07/Rust-1.89.0/`, `https://blog.rust-lang.org/2025/09/18/Rust-1.90.0/`, `https://blog.rust-lang.org/2025/10/30/Rust-1.91.0/`, `https://blog.rust-lang.org/2025/12/11/Rust-1.92.0/`, `https://blog.rust-lang.org/2026/01/22/Rust-1.93.0/`, `https://blog.rust-lang.org/2026/03/05/Rust-1.94.0/`, and `https://blog.rust-lang.org/2026/04/16/Rust-1.95.0/`.

## Program Exit Criteria

- [x] `ARCHPROG-EXIT-01` Full-map doctor runs and blocks known bad dependency/ownership patterns.
- [x] `ARCHPROG-EXIT-02` Focused HIR mutation shard has zero unexplained survivors.
- [x] `ARCHPROG-EXIT-03` Parser recovery has bounded scanner API plus fuzz/property coverage.
- [x] `ARCHPROG-EXIT-04` Product runtime binary no longer owns unclassified workbench/dev commands. Evidence: BOARD-04 is complete; `FULLMAP-CHECK-06` reports all runtime commands, bin modules, and nested actions classified and records the `trust-runtime -> trust-dev` migration policy for every workbench command retained as a compatibility alias.
- [x] `ARCHPROG-EXIT-05` HMI/web/control/cloud ownership is enforced by ports and doctor rules. Evidence: `runtime-host-surface-ownership-checklist.md` closes `RTHOST-EXIT-01` through `RTHOST-EXIT-05`, with approved ports active and `FULLMAP-CHECK-07` reporting zero direct web runtime-state/control-dispatch bypass findings.
- [x] `ARCHPROG-EXIT-06` `trust-runtime-core` owns portable execution and blocks host-only dependencies. Evidence: `runtime-core-host-split-execution-checklist.md` closes `RTSPLIT-EXIT-001` through `RTSPLIT-EXIT-010`; `trust-runtime-core` owns portable execution concerns and full-map doctor dependency/import fences block host-only leakage.
- [x] `ARCHPROG-EXIT-07` Every runtime Rust file over 1,000 lines has an owner/split note; every file over 1,500 lines has an approved split plan, completed split, or dated waiver. Evidence: BOARD-08 closed with `FULLMAP-CHECK-10` owner/split enforcement, and post-closeout follow-up split the remaining runtime/runtime-core files over 1,000 lines while keeping the regression gate active.
- [x] `ARCHPROG-EXIT-08` Diagrams are source-checked, not only render-fresh. Evidence: `FULLMAP-P7` validates selected diagram component aliases and crate dependency claims against source-derived map facts, and `python scripts/check_diagram_drift.py` passes.
- [x] `ARCHPROG-EXIT-09` Final report states what is fixed, what remains risky, and which gates prove each claim. Evidence: `docs/internal/architecture/full-architecture-refactor-final-report-2026-05-02.md`.
- [x] `ARCHPROG-EXIT-10` Runtime VM mutation shard has zero unexplained survivors or a documented equivalent-mutant list. Evidence: `runtime-vm-mutation-hardening-execution-checklist.md` records all selected shards with `0` missed and `0` timeout mutants, no accepted equivalent survivors, full-map doctor `PASS: FULLMAP-RUNTIMEVM-MUT`, and passing GitHub CI for `df90e38e6`.
- [x] `ARCHPROG-EXIT-11` `trust-runtime/src` host top-level module count is at or below the configured full-map cap after CLI, host-surface, and runtime-core boards complete, or a dated waiver names the next extraction branch. Evidence: post-closeout host-module collapse removed the dated waiver; `FULLMAP-CHECK-10` reports current count `18`, current cap `18`, and final host cap `18`.
- [x] `ARCHPROG-EXIT-12` Unsafe/concurrency register is complete and focused Miri/sanitizer/Loom/Valgrind evidence or exact blockers are attached. Evidence: `FULLMAP-CHECK-09` is active and fails unregistered unsafe, unclassified panic-like sites, missing safety tool gates, and failed/not-run tool gates; BOARD-11 artifacts live under `target/gate-artifacts/unsafe-concurrency-*e22773356/`.
- [x] `ARCHPROG-EXIT-13` HIR zero-silent-bug architecture is centralized, mutation-backed, doctor-guarded, and runtime declaration discovery is HIR catalog/parity driven.
