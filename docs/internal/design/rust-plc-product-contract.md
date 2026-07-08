# Rust-first PLC — Product Contract

**Status:** derived document, v1 (2026-07-03). **Master:**
`rust-support-architecture-spec-v1.md` (spec v2.4) — where this file and the
master disagree, the master governs; changes land there first, then are
re-derived here.
**Audience:** product, docs, marketing, design partners, demo builders.
**Companions:** `rust-plc-runtime-contract.md` (implementation contract),
`rust-plc-implementation-board.md` (execution plan).

---

## 1. Who this is for

**Primary persona — the software engineer entering automation.** Knows Rust
(or will happily learn it), lives in git/CI/code review, has never opened a
vendor PLC IDE and does not want to. Their alternative to us is not TwinCAT
— it is a hand-rolled Rust binary on an industrial PC, which is why the
product must answer "why not just write a loop?" with the machine layer
(master §1.1), not with language advocacy.

**Guarded persona — the existing truST/ST user.** Must lose nothing: ST
remains a fully supported authoring surface; Rust arrives in their world as
digest-pinned FBs they can call, supervise, and see in live values like any
other POU. The brownfield acceptance gate (Demo B0, §5 below) is *their*
acceptance test, and it blocks the roadmap until it passes.

## 2. The promise and the approved language

> **Rust gives you the language. truST gives you the machine.**

The product phrase (normative): **Rust-first PLC development with generated
IEC compatibility.** Authored in Rust + `trust.toml`; executed through
truST's PLC runtime model; audited through generated ST/IEC artifacts.

**Claim vocabulary** (master §2.4, binding here): *proven* (structural),
*measured* (named target + corpus, digest-bound), *classified* (layered
evidence, class recorded), *validated* (conservative build-time source
analysis, always paired with its runtime backstop), *admitted*
(policy-accepted for a profile and target), *declared* (author-asserted,
dev-only), *certified* (reserved — never used inside this product's scope).

**Forbidden phrasings** — each has a required replacement:

| Never say | Say instead |
|-----------|-------------|
| "pure Rust PLC" / "purely in Rust" | Rust-first PLC development with generated IEC compatibility |
| "nobody has any Rust story" | the dated, scoped market claim below (§6) |
| "WCET-proven" / "guaranteed timing" / "proves schedulability" | admitted-or-refused under *\<grade\>* evidence by worst-frame simulation; enforced by deadlines/watchdog |
| "crash-proof" / "sandboxed" for native modules | native = trusted computing base; sandboxing = the WASM form |
| "any crate, anywhere" | any crate, **admitted where the evidence supports it**; services take the rest |
| "Rust async on the PLC" / "sealed awaitables make blocking a type error" | a restricted, **build-validated** sequence language in Rust's await syntax; foreign waits rejected at build, runtime deadlines as backstop; general async lives in services |

## 3. The developer-experience contract

These flows MUST work as written, and they define "done" for the product
(not the code):

1. **First contact** (clean Linux box → running simulated machine):
   `cargo install trust` (or distro package) → `trust new rust-plc demo` →
   `cargo test` green → `trust sim` running — target: **under 10 minutes**,
   no vendor account, no license wizard, no Windows.
2. **The daily loop:** edit Rust → `cargo test` (unit + machine-sim +
   replay) → `trust check` (codegen + compile + digest + admission dry-run)
   → commit. Generated ST appears in the diff and is reviewable.
3. **To hardware:** `trust build --profile production` (admission + crate
   gates itemized) → `trust deploy <target>` (signed) → live values,
   forcing, HMI, historian work immediately — no extra wiring.
4. **When something breaks:** the diagnostic names the artifact, the
   expectation, the finding, and the fixing command (master §15.3). A
   fault links its flight-recorder trace; an overrun names its module and
   budget. "Error: see log" is a defect.
5. **When they ask "can I use crate X?":** `trust crate check` answers with
   a verdict, the evidence class, and — for rejections — the offending
   call path and the service-tier remedy.
6. **First contact, IDE form** (clean VS Code profile → running simulated
   machine): install the truST extension → click the truST Activity Bar icon
   → `Create project` → `Rust PLC` → seeded motor-latch project opens →
   press Run → Live Values shows a RUNNING Rust instance → trip the seeded
   fault → see FAULTED with safe defaults and source location → review
   generated ST diff → open the admission report. Target: **under 10
   minutes**, zero terminal, zero command palette, zero docs. This is equal
   rank with flow 1, not a later polish item.

## 4. The six differentiators (with their claim grades)

1. **Scan-native Rust POUs** — memory-safe systems language inside the
   scan; cycle coherence *proven* by copy-in/copy-out; containment honest
   per form (native = TCB, wasm = sandboxed). Master §7–8.
2. **Cycle sequences** — sequential machine logic in straight-line await
   syntax: a **restricted, build-validated sequence language** lowered to an
   SDK state machine, deliberately *not* general Rust async (which lives in
   services). Foreign waits, async blocks, and unrecognized macros are
   *rejected by `#[trust_sequence]` validation* at build, with runtime
   deadlines as the backstop;
   deterministic; restart-from-top (SFC semantics). The "better than ST,
   not just different" feature. Master §6.3.
3. **The machine in CI** — `cargo test` drives the real cycle engine
   in-process with virtual time; property tests shrink to replayable
   traces. Behavior is what it demonstrates; **timing is not** (that is
   admission's job — say so whenever this is demoed). Master §13.
4. **Timing admission** — the build simulates the real scheduler's frames
   over the task hyperperiod and refuses task sets whose **worst frame**
   (coincident task releases) doesn't fit, with an itemized account naming
   the coincident tasks; budgets are *admitted under graded evidence*,
   drift is watched in production. Master §9.
5. **Crate admission** — the dependency graph reaching the scan is
   *classified* (curated + static + artifact + measured tripwire/syscall
   evidence) and policy-gated. "Observed under test corpus, not proof"
   stays in every report. Master §10.
6. **Record & replay** — cycle traces as commissioning evidence, CI
   regression gates, black-box fault forensics, and overrun explanations
   that name the culprit. Master §12.

Soft differentiator, stated honestly: AI coding tools know Rust deeply and
ST barely — a Rust-first project inherits that leverage for free.

## 5. Use cases that must work (acceptance demos)

The adoption ladder (master §0.4) is gated by three demos; full checklists
live in the implementation board:

- **Demo B0 — brownfield acceptance** (gates P2): an existing ST project gains
  one Rust FB. Declaration in LSP, ST calls it, live values show status,
  induced fault supervised from ST, timing in telemetry. **Zero Rust-first
  features used.** The same acceptance is repeated from visible VS Code
  surfaces. Demonstrates: PLC users lose nothing.
- **Demo G0 — minimal greenfield acceptance** (gates P3): motor-latch scale.
  1 task, 1 Rust PROGRAM, 2 inputs, 1 output, 1 retain variable, 1 cycle
  sequence, 1 `trust-sim` cargo test, a development-grade admission
  report, a reviewable generated-ST diff. G0 has two equal gates: CLI/CI and
  VS Code visible-surface execution. Demonstrates: the whole toolchain, end
  to end, small enough to actually read.
- **Reference example — the palletizer** (master §16.1): the vision demo —
  sequences, typed I/O, a service with honest staleness, machine tests,
  property tests, replay gate, signed deploy. Used for docs/marketing
  *after* B0 and G0 exist; never the first external evidence.

## 6. The market claim (normative wording)

> As of 2026-07-03, we have not found a shipping PLC platform that combines
> Rust-first PLC authoring, scan-native Rust POUs, timing admission, crate
> admission, cycle replay, generated ST/IEC compatibility, and optional
> sandboxing as one coherent product.

Rules: the claim is dated and scoped ("we have not found"); it is
re-verified before each external use; and it is never compressed to
"nobody has Rust" (PLCnext ships a Rust sample runtime over its C API;
ctrlX has community Rust discussion; RoboPLC exists as a Rust-first
framework). The differentiator is the **product contract**, not the word
Rust. Landscape detail: master §17.

## 7. What we do not claim

- Not a SIL-rated safety PLC; safety interlocks live in safety hardware.
- Native in-process code can crash the runtime — containment is per-form
  and labeled; that is *why* the wasm form and service tier exist.
- Linux-class real-time, not MCU-class determinism; task granularity is
  bounded by the resource base scan.
- Simulation demonstrates behavior, never target timing.
- Crate and timing verdicts are evidence-graded admissions, not proofs.

## 8. Pointers into the master spec

| Topic | Master section |
|-------|----------------|
| Value proposition / DIY table | §1 |
| Claim vocabulary | §2.4 |
| Two doors, one pipeline | §4 |
| Project model & trust.toml | §5 |
| Programming model & sequences | §6 |
| Timing admission | §9 |
| Crate admission | §10 |
| Record/replay | §12 |
| Testing & CI story | §13 |
| Honesty notes & failure catalogue | §14 |
| Worked examples | §16 |
| Landscape & positioning | §17 |
| Locked decisions | §18 (D14–D26 locked 2026-07-03) |
