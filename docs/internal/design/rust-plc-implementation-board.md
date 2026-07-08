# Rust-first PLC — Implementation Board (vertical slices)

**Status:** execution plan v1 (2026-07-03). **Master:**
`rust-support-architecture-spec-v1.md` (spec v2.4, D14–D26 locked on the corrected form);
requirements/seams in `rust-plc-runtime-contract.md`. The master governs on
any disagreement.

**Execution model:** vertical slices, not subsystems. Every slice cuts a
thin, demoable thread through all layers it touches and lands
production-complete (no "pre-release" framing). Master §19's phases are the
capability inventory; this board is what actually gets built, in order.

**Standing gates (every slice, non-negotiable):**
- `just fmt/clippy/test-all` green on **trust-builder** before "pass"
  (xtask-gate-green ≠ runtime-green).
- Conformance/failure-injection tests for every F-row the slice touches.
- New host subsystems/validators as **new modules** — never appended to an
  existing large file; SRP/file-size/coupling reviewed per slice.
- Generated artifacts byte-deterministic; digest chain intact (F22 test).
- Demo acceptance: reviewer ≠ implementer; evidence (transcript +
  screenshots where UI is involved) attached to the slice row.
- Diagnostics quality bar (master §15.3): every new failure names artifact,
  expectation, finding, fixing command.
- Claim vocabulary (master §2.4) in every user-visible string.
- Any slice with a user-visible surface MUST name the VS Code surface and
  attach capture-harness screenshot evidence. A CLI-only demo does not close
  a visible-surface requirement.
- Active-slice checklist items must be referenced by stable IDs from
  `rust-plc-complete-implementation-checklist.md`; removals require lead
  review and a recorded reason.

**Status values:** `not_started` → `in_progress` → `demo_review` → `pass`.

---

## Slice ledger

| # | Slice | Demonstrates | Draws on | Depends | Status |
|---|-------|--------|----------|---------|--------|
| S1 | First light | The whole thesis, minimally | P1a+P3 (thin) | P0 | not_started |
| S2 | Builtin migration + conformance rig | One native path; failure injection works | P1a | S1 | not_started |
| S3 | Brownfield FB (**Demo B0**) | PLC users lose nothing | P2 (pragma/declare subset) | S2 | not_started |
| S4 | Cycle sequences | The flagship language feature | P1b | S2 | not_started |
| S5 | Dynamic native modules + toolchain | Out-of-tree vendoring, skew safety | P2 (rest) | S3 | not_started |
| S6 | Rust-first surface (**Demo G0**) | Greenfield door end to end | P3 (rest) | S1,S4,S5 | not_started |
| S7 | Timing admission | "Does it fit?" answered at build | P4 (+P4b later) | S6 | not_started |
| S8 | Crate admission | crates.io made honest | P5 | S7 | not_started |
| S9 | Services + typed I/O | Tier 3 + device views | P6 | S6 | not_started |
| S10 | Record & replay | Traces as evidence and CI gates | P7 | S6 | not_started |
| S11 | Wasm form | Sandboxed tier | P8 | S5 | not_started |
| S12 | Productization | Docs/examples/journeys | P9 | S6+ | not_started |
| S13 | VS Code Rust-first shell | Human first-contact path | RS-118..126 | S6 + BC-1/2 | not_started |
| S14 | Live Values instances | Named machine state in IDE | BC-6 | S1,S13 | not_started |
| S15 | Admission in the IDE | Evidence visible, not log-only | BC-1/3 | S7,S13 | not_started |
| S16 | Machine tests + traces in IDE | Native Testing/replay workflow | BC-5 | S10,S13 | not_started |
| S17 | Deploy gates in IDE | F20 honest deploy readiness | BC-4 | S13 + deploy backend | not_started |

Dependency notes: S4 ∥ S3/S5 (SDK-only); S9/S10 ∥ S7/S8 where staffing
allows. Nothing starts before S1 passes.

---

## S1 — First light (normative first slice)

> One Rust PROGRAM, scheduled by one task, compiled through generated ST,
> executed through the module host (static form), with the panic boundary,
> per-instance timing telemetry, and one cargo-test machine simulation.
> Once this is green on trust-builder, the rest of the spec is believable.

**In scope (thin versions):**
- `trust-module-abi` + `trust-plc` minimal: `#[trust_program]`, `#[input]`/
  `#[output]`/`#[retain]`/`#[safe_default]`, `PlcProgram`, `Exchange`,
  `CycleCtx` (cycle/period/dt/log/diagnostic subset), `catch_unwind`
  boundary (F1), static/built-in registration only.
- Module host v0: registry consulted by `execute_program_by_name`
  (`runtime/cycle.rs:236-248`); copy-in/out via a generated exchange block;
  faults → `record_fault/apply_fault`; `record_call(kind="module", …)`.
- Codegen v0: a `trust.toml` subset (`[project]`, one `[tasks.*]`, `[io]`)
  → `src/generated/trust/{declarations,configuration}.st` (deterministic,
  digest-pinned header), compiled by the ordinary pipeline.
- `trust-sim` v0: `Machine::from_project` over `TestHarness` — enough for
  `set_input`/`cycle`/`run_until`/`output_bool`/named reads.
- Demo program: **motor latch** (start/stop inputs → motor output, retained
  run counter) — *no sequence yet* (that is S4).

**Out of scope:** dynamic loading, FBs/functions, sequences, admission,
crate checks, traces, wasm, `trust` umbrella (a `cargo xtask`-style driver
or direct `trust-runtime` invocations are fine for S1).

**Exit criteria:**
1. `cargo test` in the demo project runs a machine test green (latch set,
   latch reset, retain survives `Machine::restart_warm`).
2. The same project runs under `trust-runtime run` on sim **and** on the
   Pi/target; live values show the program instance and its
   `_STATE/_FAULT_CODE/_OVERRUNS` through the thin BC-6 instance snapshot;
   VS Code screenshot evidence is attached if the extension surface is used.
3. Injected panic in `cycle()` → instance FAULTED with message+location
   diagnostic, outputs follow `#[safe_default]`, scan continues (F1 demonstrated
   — against today's baseline where a panic kills the process).
4. Per-instance duration visible in `tasks.stats`/live values
   (`record_call` wired).
5. Generated ST is byte-stable across regeneration; hand-editing it fails
   the build with the F22 diagnostic.
6. All standing gates.

---

## Demo definitions (acceptance checklists)

### Demo B0 — brownfield acceptance (gates S3; product-contract §5)

Existing ST project (a real one from the examples/test corpus, not a
synthetic toy) gains one Rust FB (`FILTER_EMA` shape, master §16.2):

- [ ] Module builds via `trust module build`; declaration regenerated via
      `trust module declare`; digest-pinned.
- [ ] Declaration is an ordinary LSP symbol: completion, hover (Rust docs
      visible), go-to-declaration.
- [ ] ST instantiates and calls the FB; mixed task order with ST programs.
- [ ] Live values show the instance incl. `_STATE/_FAULT_CODE/_OVERRUNS`;
      IO-panel force/write/release works on its bound variables.
- [ ] Induced fault (panic fixture input) supervised **from ST**
      (`IF flt._STATE <> MOD_RUNNING …`) and visible in Live Values and the
      selected Devices & Connections runtime node where supported.
- [ ] Per-instance timing appears in telemetry.
- [ ] **Zero Rust-first features used**: no trust.toml, no generated
      configuration, no trust-sim, no umbrella CLI.
- [ ] Reviewer ≠ implementer; evidence attached.

### Demo G0 — minimal greenfield acceptance (gates S6; product-contract §5)

Motor-latch scale, deliberately tiny — the whole toolchain readable in one
sitting:

- [ ] `trust new rust-plc latch` scaffold with protective defaults (RS-68).
- [ ] 1 task, 1 Rust PROGRAM, 2 inputs, 1 output, 1 `#[retain]` variable.
- [ ] 1 **cycle sequence** (S4 shipped): e.g. `until(start) → on →
      until(stop) → off`, with one `deadline(…)` branch.
- [ ] 1 `trust-sim` cargo test green (incl. retain-across-warm-restart
      assertion).
- [ ] `trust check` clean; **development-grade admission report** printed
      (declared evidence; measured re-run added to the demo at S7).
- [ ] Generated-ST diff shown in a PR and reviewed; regeneration is a
      no-op diff.
- [ ] `trust build` → bundle; `trust deploy` to sim target; live values +
      forcing work.
- [ ] Under 10 minutes from clean checkout to green `cargo test` on a
      Linux box (product contract §3.1), timed and recorded.
- [ ] The same G0 flow is repeated in VS Code from visible surfaces only:
      Create Rust PLC project, Run, Live Values, induced fault, generated-ST
      diff, and admission report. No terminal or command palette.
- [ ] Reviewer ≠ implementer; evidence attached.

---

## S2–S12 scope sketches (details firm up as their turn approaches)

- **S2** — migrate **all** builtin FBs (incl. `ROBOT_P3MINIMALARM`) to
  `trust-plc`; conformance + failure-injection rig (F1/F2/F7/F10/F11)
  over fixture modules; differential suites verify behavior unchanged;
  retain module-blob codec section lands. Adds **legacy brownfield
  acceptance**: an unchanged existing ST corpus project re-runs on the
  migrated runtime with byte-identical outputs over N cycles, warm restart
  from a pre-migration retain file, and statistically equal cycle times on
  the target/remote builder benchmark gate. S2 cannot pass on new Rust
  fixtures alone.
- **S3** — `{external}` pragma semantics in trust-hir (Open-OT micro-syntax
  precedent); `trust module declare`; tier-1 EXTERNAL call path
  (`call.rs`); **Demo B0**.
- **S4** — the `#[trust_sequence]` **validator/lowerer** + SDK wait points
  (`next_cycle`/`wait`/`until`[.deadline]/`all`/`race`/child sequences);
  generated `TrustSequence` state machine shape with per-poll `Exchange`/
  `CycleCtx` access and no stored Rust future; closure-scoped exchange access
  (`s.read`/`s.write`/`s.retain_read`/
  `s.retain_write`); the **F23 rejection matrix** as CI fixtures (foreign
  `.await`, async block, async closure, executor API, hand-written future,
  unrecognized macro, wait-free loop — each rejected naming the construct);
  loop-without-wait-point validation (RS-80); runtime stuck-wait backstop
  demonstrated (deadline fault, scan unaffected); sequence-location
  diagnostics (F21); TestBench sequence tests; restart-from-top across warm
  restart (RS-75..80). Note: sequences are a restricted validated language,
  **not** general Rust async (that stays in services).
- **S5** — cdylib loader + handshake + skew matrix (F8/F9/F12); module CLI
  (new/check/build/test/package); deploy-key signing; `modules/` bundle
  discovery; reload parity (RS-86).
- **S6** — full trust.toml codegen (RS-60..68) incl. generated
  io.toml/runtime.toml; `trust` umbrella + `cargo trust`; `trust-sim`
  productized (+ proptest strategies, RS-112); **Demo G0**; palletizer
  reference example (master §16.1) lands in examples.
- **S7** — `.trusttime`, **worst-frame/hyperperiod frame simulation** of
  the actual non-preemptive scheduler + itemized worst-frame report naming
  coincident tasks (RS-87..90, RS-115..117), target measurement harness
  (RS-91), deploy gate + drift diagnostic (RS-93); regression fixture: a
  task set that passes per-task-in-isolation but fails on a coincidence
  frame MUST be refused; G0 re-run at measured grade. `static_stbc` (P4b)
  deferred to its own slice when justified.
- **S8** — crate knowledge base + static/symbol scans + tripwire/syscall
  audit (RS-92) + `.trustcrate` + policy gates (RS-94..97).
- **S9** — `trust-service-sdk`, `[services.*]` codegen + supervisor,
  `ServiceLink`, Devices & Connections liveness; `trust io import` ADS Rust bindings +
  EtherCAT variant (RS-73/74, 98..100).
- **S10** — `.trusttrace` recorder (window/flight/sim modes, shm-backed),
  replay + diff + explain-overrun, replay tests in template
  (RS-101..108); recorder overhead measured against its budget (RS-103).
- **S11** — wasmtime host, epoch/fuel, `--form wasm|both`, both-forms
  conformance, F4 containment demo (RS-45..47).
- **S12** — public docs (develop + reference + section index.md), example
  gallery, guides (module author / service author / operator / migration),
  acceptance-board journeys, positioning page (product contract §6
  wording).
- **S13** — VS Code Rust-first shell: `projectKind`, create-project kind
  picker, Rust scaffold via `trust new rust-plc`, Check wired to
  `trust check --json`, Run wired to BC-2, generated-ST read-only/diff,
  Rust starters and walkthrough. Gate: G0-in-IDE timed under 10 minutes or
  the claim changes; extension tests cover single-root, multi-root, cancel,
  conflict, and no-palette-only reachability.
- **S14** — Live Values instances: BC-6 instance snapshot, Instances tab,
  `_STATE/_FAULT_CODE/_OVERRUNS`, fault expansion/reset, sequence wait
  point, safe-default tags, timing/evidence chips, stale/fresh quality.
  Gate: B0-in-IDE and F1 demonstrated in-panel with screenshots.
- **S15** — Admission in the IDE: report panel, Problems anchors for F16/F17,
  evidence chips, measure-on-target gating, drift chip + Explain. Gate: the
  refused coincidence-frame fixture renders in IDE and F18 drift has both
  runtime and visible-surface proof.
- **S16** — Machine tests + traces in the IDE: cargo/libtest or equivalent
  discovery, Machine/Replay groups in VS Code Testing, shrunk traces,
  Replay action, divergence table. Gate: F19 regression fails as a Testing
  item with variable diff.
- **S17** — Deploy gates in the IDE: selected runtime/device Deploy section
  renders signed bundle, `.trusttime`, `.trustcrate`, evidence floor, target
  capability, and F20 stale/missing record refusal. Gate: no dead Deploy
  button states; disabled/absent-with-reason proof attached.
