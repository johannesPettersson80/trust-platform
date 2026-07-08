# Rust-first PLC — Runtime & Implementation Contract

**Status:** derived document, v1 (2026-07-03). **Master:**
`rust-support-architecture-spec-v1.md` (spec v2.4) — the master governs;
this file is the implementation-normative extract: what to build, against
which requirement IDs, at which code seams. RS/F/D identifiers are the
master's; read the master for rationale.
**Audience:** runtime, compiler, and tooling implementers; reviewers.
**Execution order:** `rust-plc-implementation-board.md` (vertical slices).

Conventions: MUST/SHOULD/MAY per RFC 2119; claim vocabulary per master §2.4
(proven / validated / measured / classified / admitted / declared;
*certified* is reserved). Paths under `crates/trust-runtime/src/` unless
noted. v2.4 note: sequences are enforced by **validation + explicit
state-machine lowering + runtime backstop**, never by trait types or stored
Rust futures; timing admission is **worst-frame/hyperperiod simulation**,
never per-task-in-isolation or utilization math.

---

## 1. New workspace crates

| Crate | Charter | Constraints |
|-------|---------|-------------|
| `trust-module-abi` | The ONLY code shared by runtime and modules: `repr(C)` descriptors, vtables, status enums (master App. B) | `no_std`; size-first evolvable structs; additive minors; registered unsafe per repo policy; MUST NOT depend on host crates (xtask edge rule) — RS-84 |
| `trust-plc` | User-facing SDK: macros (`#[trust_program]`, `#[trust_fb]`, `#[trust_function]`, `#[trust_sequence]` validator/lowerer, field attrs), traits (`PlcProgram`, `PlcFb`, `TrustSequence`), `Exchange`, `InitCtx`, `CycleCtx`, `SequencePoll`/`Seq` lowering context (closure-scoped access), `TestBench`, tripwire allocator | All unsafety inside; user modules are 100 % safe Rust (P7); D21 |
| `trust-plc-macros` if split | Proc macros and validators for the SDK surface | No runtime-host dependency; emits span-preserving diagnostics; split is allowed but not required |
| module host (in `trust-runtime` host layer) | Discovery, handshake, lifecycle, marshalling plans, deadlines, faults, telemetry | New module(s), never appended to an existing god-file; registered unsafe sites |
| `trust-sim` | Public machine-sim harness over `host/harness/harness.rs` `TestHarness` | RS-109..111; Linux dev hosts at parity; D20 |
| `trust-service-sdk` | Tier-3 typed client over the control plane | RS-48..50; JSON-line protocol (NOT JSON-RPC), no subscribe RPC — poll-shaped under the hood (RS-99) |

Runtime crates keep `#![forbid(unsafe_code)]` (`trust-runtime/src/lib.rs:3`,
`trust-runtime-core/src/lib.rs:12`) — do not weaken them.

## 2. Integration seams (verified 2026-07-03, branch `ads/client`)

| Seam | Location | Change |
|------|----------|--------|
| Tier-2 dispatch | `execute_program_by_name`, `runtime/cycle.rs:236-248` | Consult external-program registry before VM POU lookup |
| Tier-1 dispatch | `execute_native_call`, `runtime/vm/call.rs:71-173`; builtin branch `call.rs:205-247`, param bind `call.rs:414-459` | Add EXTERNAL kind beside builtin/stdlib |
| Builtin FBs to migrate | `stdlib/fbs/registry.rs:5-18` (11 kinds incl. `RobotP3MinimalArm`); shared with eval path `host/eval/calls.rs:292-294` | Reimplement on SDK traits; observable behavior unchanged (differential suites) |
| Per-invocation timing | `RuntimeMetrics::record_call`, `host/metrics.rs:266-280`, reached from call-site wrappers such as `record_profile_call` (only `kind="program"` wired today, `runtime/cycle.rs:236-248`) | New kinds `module`, `module_fb`; ranked contributors already exist |
| Wall-clock deadline | dormant `Runtime::set_execution_deadline`, `runtime/core/lifecycle.rs:148-155`; checked every 32 instrs, `runtime/vm/dispatch.rs:36,271-275,650-652` | Arm per task tick (§8.2 master) |
| Faults | `record_fault`/`apply_fault`, `runtime/core/lifecycle.rs:203-234` | Module faults funnel here; F-catalogue reactions |
| Safe state | `IoSafeState`, `io/addressing.rs:152-168`; applied via `runtime/io_subsystem.rs:85-92` **only** under SafeHalt | Templates default protective (RS-68); stop applies none — do not claim otherwise |
| Retain | codec `retain/codec.rs` (STRN v2, CRC32); typed migration `runtime/restart.rs:169-384` | Add module-blob section (version bump); interface retain rides existing path (RS-81/82) |
| Declarations codegen precedent | `host/ads/generate.rs:106-155` → `src/generated/ads_generated.st`, name-bound | Same pattern for declarations/configuration/devices (RS-60..66) |
| Pragma parsing precedent | `{attribute 'k' := 'v'}` in `trust-hir/src/openot_authoring.rs:264-337` | `{external module := …, digest := …}` family reuses the micro-syntax |
| Bundle discovery | `RuntimeBundle::load`, `config/loaders.rs:2-67` | Gains `modules/` artifact class |
| Reload baseline | `apply_online_change_bytes`, `runtime/online_change.rs:1-25`; `bytecode.reload` RPC | RS-86 parity only; body-swaps for known POUs; refuse on handshake failure; no generation-swap claims |
| Machine sim | `TestHarness`, `host/harness/harness.rs:1-277` (`cycle`, `run_until`, `advance_time`, `set_direct_input`); JSON automation `bin/trust-harness.rs` | Wrap as `trust-sim` (RS-109) |
| Trace transport precedent | Open-OT `SharedRecordPublisher`, `runtime/openot_telemetry.rs` (per-cycle, `runtime/cycle.rs:428-429`) | Flight-recorder publication (RS-102) |
| Events | `RuntimeEvent`, `host/debug/types.rs:206-266` | Trace sections + drift/fault diagnostics ride this stream |
| Live values / debug | DAP `trust-debug` custom requests `stIoWrite/stIoForce/stIoRelease` → control `io.write/force/unforce` | Module status surfaces through this pipeline (RS-57); no parallel one |

## 2.1 Backend contracts for IDE and automation

These seams are binding before their consuming VS Code slice starts. They are
specified in detail in the companion contracts; implementations may refine
schema fields but not replace the seam with text scraping or duplicate logic.

| ID | Contract | Owner | Consumed by |
|----|----------|-------|-------------|
| BC-1 | `trust check --json`: generation, cargo/ST compile, digest, development admission, crate policy, diagnostics, artifacts | `trust` CLI + codegen/admission engines | VS Code Check badge, Problems, admission panel, CI |
| BC-2 | Rust PLC launch/build handshake for simulator/debug sessions | `trust-debug`, runtime bundle loader, `trust` build path | VS Code Run/Debug |
| BC-3 | `trust timing measure` orchestration and `.trusttime` result path | timing admission engine + runtime measurement harness | production admission, deploy gate, admission report |
| BC-4 | deploy/send-to-PLC readiness and refusal payloads | deploy/runtime target subsystem | Devices & Connections deploy section |
| BC-5 | `trust replay --json`: trace identity, verdict, divergence, overrun explanation | replay engine | VS Code Testing/replay report, CI |
| BC-6 | instance snapshot over the existing debug/control pipeline | runtime debug/control pipeline | Live Values Instances tab, fault reset, drift rendering |

## 3. Requirement index (binding; one line each)

### Module model & ABI
- RS-01..06: module = unit of package/version/trust; exports FUNCTION/FB/
  PROGRAM; load validates before any user code; per-target artifacts; no
  arbitrary-path loading; generated declarations never hand-edited.
- RS-07..09: handshake order (symbol → size → abi_major → fingerprint →
  digest → limits); `repr(C)` fixed-size pointer-free ABI; no panic across
  FFI (`catch_unwind` in every SDK wrapper; `panic="abort"` refused).
- RS-83: source-built = exact fingerprint; precompiled packages = ABI-line
  compatibility later, explicit, gated (D23).
- RS-85: BLAKE3 digest chain covers exports + configuration inputs + device
  imports; embedded in binary, generated ST, `.stbc` binding, bundle.

### Marshalling & memory
- RS-10..14a: runtime-owned interface vars; generated `repr(C)` exchange
  blocks; copy-in → call → copy-out; no retained pointers (structural);
  opaque instance state via create/destroy; fixed-capacity strings/arrays;
  heap types in interface position = build error; load-time marshalling
  plans (sequential copies, no map lookups per cycle).
- Type map: master App. A. `#[unit]`/`#[range]` metadata → declaration
  pragmas; `#[range]` violation on copy-out = protocol fault (RS-72).

### Lifecycle & scheduling
- RS-18..20: init outside cyclic deadline (own deadline); FAULTED is
  sticky until explicit reset; exit/destroy bounded, abandon-and-log on
  hang.
- RS-21..23, 86: module programs are ordinary task-list entries in
  declaration order (no second scheduler); tier-1 calls nest in the
  caller's slot; reload parity per RS-86.
- RS-40: FUNCTION exports are stateless (structural — SDK offers no state
  slot).

### Cycle context & capabilities
- RS-24: no wall-clock/fs/net/env/thread APIs; time/randomness injected
  (P2/P5); lint-flagged in native, absent in wasm.
- RS-25: deferred allocation-free rate-limited logging; structured
  key-value payloads (§6.5).

### Temporal & memory protection
- RS-26..29: per-invocation wall-clock deadline (manifest, downward
  override); reactions log/skip/fault-module/fault-task (default
  fault-module); native = detect + cooperative `checkpoint()`, wasm =
  epoch preempt / optional fuel; per-instance min/mean/max/p99 + overruns
  exported like task stats.
- RS-30..33: native = TCB (documented); wasm hard caps; allocation
  tripwire (strict: fault; release: counted diagnostic); state/scratch
  declared and accounted.
- RS-70: `#[budget]` = default deadline + admission budget; one effective
  value feeds enforcement and admission.

### Faults & outputs
- RS-34..37: fault sources enumerated; output policy `default`(typed
  initials, incl. `#[safe_default]` — RS-71) | `hold-last` (explicit);
  generated status outputs `_STATE/_FAULT_CODE/_OVERRUNS`; panic message +
  location captured allocation-free.

### Retain
- RS-41/42, 81/82: interface retain rides runtime retain (typed migration
  included); opaque retain = explicit versioned save/load hooks, STRN
  section, mismatch policy discard|fault.

### Cycle sequences (SDK; conformance-tested)
- RS-75: a `#[trust_sequence]` body is source syntax lowered to a concrete
  SDK state machine implementing `TrustSequence::poll(&mut self,
  &mut Exchange<P>, &mut CycleCtx)`. The generated state object is constructed
  at `init()` and stores only explicit sequence state (step, timers,
  deadlines, child state, owned values crossing wait points), never
  `Exchange`/`CycleCtx`/process-image references and never a Rust `Future`
  capturing them. No waker/executor machinery; polled ≤ once per cycle on the
  task thread.
- RS-76: **a restricted validated language, not general Rust async** (which
  lives in services). Build-time validation admits only SDK wait points
  (`next_cycle`, `until`/`wait_until` [± `.deadline(…)`], `wait` on cycle
  time, `all`/`race`, child trust sequences) and MUST reject with the
  construct named (F23): any other `.await` operand (plain `.await` accepts
  any `IntoFuture` — no trait can restrict it, hence validation), `async`
  blocks/closures, executor APIs, hand-written futures, and **any macro
  outside the admitted allowlist** (v1 allowlist: `iec!(...)` duration/time
  literals only; macros can hide `.await` — conservative rejection).
  Runtime backstop: a non-progressing wait is bounded by `deadline(…)` +
  program budget/deadline machinery; a stuck sequence parks and faults —
  it can never block the scan.
- RS-77: exchange/retain access only through **closure-scoped accessors**
  (`s.read(|io| …)`, `s.write(|io| …)`, `s.retain_read(|r| …)`,
  `s.retain_write(|r| …)`) and wait-point predicates; closures are sync
  (validated); `&`/`&mut` args cannot escape (HRTB lifetimes) ⇒ no
  reference across a wait point — structural, plus validated.
- RS-78: warm/cold restart ⇒ restart from top; durable progress in
  `#[retain]` fields only.
- RS-79: deadline/Err/panic in a poll ⇒ normal fault machinery + sequence
  location (file:line of the pending wait point) in the diagnostic;
  deterministic under RS-43.
- RS-80: each poll bounded by the program budget; the validator rejects
  loops with no reachable wait point on every path; runtime deadlines
  remain the backstop.

### Determinism & replay
- RS-43/44: conformant modules deterministic given inputs+ctx+state;
  native float determinism per-target; wasm bit-exact cross-platform.
- RS-101..108: `.trusttrace` recorder (window / flight-recorder ring /
  sim); recorder cost admitted when enabled, reference figure gates
  "supported" (≤ 3 % of a 1 ms cycle at defaults, measured); replay =
  re-execution + divergence report with tolerances; `trust diff`;
  `explain-overrun` reconstructs a cycle timeline against admitted
  budgets; fault diagnostics link their trace dumps.

### Timing admission (worst-frame / hyperperiod model)
- RS-87/88: budgets attach to exports and ST programs with evidence grades
  (`declared`/`measured`/`wasm_fuel`/`static_stbc`/`builtin_maintained`);
  ST loop bounds via constants or `{trust_loop_bound}`.
- RS-115: admission simulates the **actual scheduler** — one resource
  thread, base frame `F = cycle_interval`, due tasks collected per frame,
  sorted (priority, due, index), run **non-preemptively** — over the
  hyperperiod `H = lcm(T_i)` (precondition F | T_i per RS-66; bounded
  fallback: one synthetic frame with every task due). Per frame:
  `frame_work = input update + Σ due-task budgets (incl. RS-89 inline call
  counts + marshalling) + background programs + control-write drain +
  service exchange + retain save (when due-able) + trace recorder (when
  enabled) + output commit + overhead + jitter margin`. ADMIT iff
  `frame_work ≤ F` for **every** frame and each task completes in its
  release frame (⇒ deadline = period). Never average utilization; never
  per-task-in-isolation.
- RS-116: event tasks in every frame's due-set (conservative) unless a
  declared `min_interarrival` bounds them — still present in the worst
  frame.
- RS-117: exact for the current non-preemptive scheduler only;
  response-time analysis reserved for a future preemptive scheduler.
- RS-90: report shows the **worst frame first with its coincident tasks**,
  itemized contributors + remedies; build/deploy fails on any frame
  deficit.
- RS-91: measurement harness on named target: corpus + stress +
  property-adversarial inputs; min/mean/p99/p99.9/max; margin; report
  digest-bound to (module, target, runtime version).
- RS-92: same run = crate-evidence probe (allocation tripwire + seccomp
  user-notify syscall audit; init-path vs cycle-path attribution).
- RS-93: production runtime compares p99 vs admitted → **admission
  drift** diagnostic; `.trusttime` ships in the bundle; deploy gate.

### Crate admission
- RS-94: `trust crate check` → verdict + evidence class per finding +
  resolved concrete call path when available, otherwise explicit `unknown`/
  dynamic-dispatch reason, plus remedy for scan-lane rejections.
- RS-95..97: policy in `trust.toml [crates]` (profile defaults: dev warn,
  production deny); `.trustcrate` = SBOM + licenses + advisories +
  unsafe/ffi inventory + verdicts; services exempt from RT classification
  but not from supply-chain reporting; deploy refuses missing/stale/
  violating `.trustcrate`.

### Rust-first codegen
- RS-60: one compile path — generated sources through the existing
  compiler; no second configuration parser or scheduler binding.
- RS-61/85: deterministic generation, "generated" headers, digest-chained;
  hand edits = digest mismatch at build (F22).
- RS-62: mixed projects both ways; Rust→ST in-cycle calls deferred (D22).
- RS-63/64: project crate = the module (cdylib+rlib); `src/generated/**` +
  generated `io.toml`/`runtime.toml` are committed, reviewable artifacts.
- RS-65: trust.toml = topology + policy only; never restates interfaces;
  unknown-export references are build errors.
- RS-66: generation rules — `period` xor `event` per task; instance names
  = snake_case type (override `{instance, type}`, duplicates = error);
  `[io]` types checked against Rust interface; `cycle_interval` = min task
  period, must divide all periods.
- RS-67: `trust check` = codegen + compile + digest + admission dry-run,
  no artifacts written.
- RS-68: protective template defaults (watchdog SafeHalt, fault SafeHalt,
  retain on, safe_state warning for unlisted %Q outputs).
- RS-69: FB libraries = cargo crates compiled into the project module,
  provenance recorded.

### Typed I/O & services
- RS-73/74: `#[trust_device]` structs are typed *views* over name-bound
  generated variables (ADS pattern) with quality access; import provenance
  recorded; symbol-version drift warned at `trust check`.
- RS-48..54, 98..100: service SDK quality metadata mandatory; snapshot
  reads cycle-coherent; writes acked with landing cycle; `[services.*]`
  generates unit + variables + supervisor; polling now, push-ready API;
  mesh transport optional; Devices & Connections liveness from the runtime's registry.

### Tooling & tests
- RS-55..59: pinned toolchain builds; LSP/IDE surfaces; sim runs modules
  identically to target; conformance gate for the `conformant` mark.
- RS-109..114: `trust-sim` Machine API over TestHarness (virtual time,
  physics couplings); honesty note (behavior ≠ timing); proptest
  strategies + trace-shrinking; replay tests in the standard template;
  `trust`/`cargo trust` umbrella with old spellings kept.
- RS-118..126: VS Code visible-surface parity; `trust.toml`+`Cargo.toml`
  project-kind detection; versioned JSON schemas for check/build/replay;
  instance snapshots through the existing debug/control pipeline; IDE
  admission rendering; generated-ST read-only/diff policy; machine tests in
  VS Code Testing; honest Update vs Restart-required; bootstrap/version-skew
  preflight.

## 4. Conformance matrix (failure catalogue)

Every row = a mandatory conformance/failure-injection test; "diagnosed" =
visible in live values + control plane + Devices & Connections where
applicable with named module/instance/cycle and a remedy.

| # | Failure | Detection | Reaction |
|---|---------|-----------|----------|
| F1 | Panic in cycle() | catch_unwind / wasm trap | no copy-out; output policy; FAULTED; msg+loc diagnosed |
| F2 | Cooperative deadline overrun | watchdog stamps | per `deadline_action` |
| F3 | Native hard loop | task-level monitor | task overrun reaction; module blamed; documented TCB limit |
| F4 | Wasm hard loop | epoch interruption | trap → FAULTED; task continues |
| F5 | Native segfault/UB | uncontainable | process fatal path; post-mortem names last-entered module; honest docs |
| F6 | Wasm memory violation | sandbox trap | as F4 |
| F7 | Allocation in cycle path | tripwire allocator | strict: fault; release: counted diagnostic |
| F8 | ABI/toolchain skew | handshake fingerprint | REJECTED, expected-vs-found |
| F9 | Interface digest drift | digest chain | compile/load error naming regen command |
| F10 | init failure/deadline | host | FAULTED before first cycle |
| F11 | Retain version mismatch | RETAIN_VERSION | discard (default) or fault |
| F12 | Artifact missing for target | loader | load error; no silent form fallback |
| F13 | Service down/stale | supervision idiom + registry | local fallback; honest liveness |
| F14 | Service writes garbage | RBAC + types + PLC validation | rejected/clamped; PLC authoritative |
| F15 | exit/destroy hang | exit deadline | abandoned + logged; bounded shutdown |
| F16 | Task set inadmissible (worst frame > F) | §9 frame simulation | build/deploy refused; worst frame + coincident tasks named |
| F17 | Policy-violating crate in scan path | §10 gate | refused; call path named |
| F18 | Admission drift | runtime p99 vs admitted | diagnostic + trace link |
| F19 | Replay divergence | replay/CI | test failure with variable diff |
| F20 | Admission record missing/stale at deploy | deploy gate | refused, artifact named |
| F21 | Sequence deadline elapsed | SDK | fault + sequence location + code |
| F22 | Generated-artifact drift | digest chain | build error naming regen command |
| F23 | Non-conformant sequence body (foreign await / async block / executor API / unrecognized macro / wait-free loop) | `#[trust_sequence]` validation | build error naming construct + admitted wait points |

Fault-code namespace: `_FAULT_CODE = 0` means no fault; `1..=999` runtime/
SDK reserved; `1000..=9999` toolchain/generated-artifact; `10000..=65535`
standard library and builtin migrations; `65536..=0x7fff_ffff` user/module
fault enums; `0x8000_0000..=0xffff_ffff` external/vendor namespaces.

## 5. Repository obligations (every slice)

xtask full-map policy entries for new crates (ABI/SDK crates MUST NOT
depend on the host); `deny.toml`; public-API snapshots for `trust-plc`/
`trust-module-abi`; MSRV = workspace `rust-version = 1.95`; new validators
and host subsystems as **new modules**, never appended to a large file;
execution-flow changes update `docs/diagrams/**/*.puml` + manifest; public
docs land in `docs/public/develop/` + `reference/` **and** the section
`index.md`; nothing is called done before `just fmt/clippy/test-all` is
green on **trust-builder** (xtask-gate-green ≠ runtime-green).
