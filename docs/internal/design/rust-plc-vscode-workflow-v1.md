# Rust-first PLC in VS Code — Product Workflow & Integration Plan (v1.1)

**Status:** design memory, v1.1 (2026-07-03). **Master:** `rust-support-architecture-spec-v1.md`
(spec v2.4, D14–D26 locked) — this document extends the master's §15.2 into a full VS Code
workflow contract; where they disagree, the master governs *on runtime/CLI semantics* and this
document governs *on VS Code surface behavior* until merged back (doc patches in §9).
**Companions:** `rust-plc-product-contract.md`, `rust-plc-runtime-contract.md`,
`rust-plc-implementation-board.md`, `vscode-ux-overhaul-plan.md` (v5 — the shell contract this
design builds on, NOT a replacement for it).
**Audience:** product, extension implementers, runtime/CLI implementers (backend contracts), reviewers.
**Purpose:** keep the Rust PLC VS Code workflow and implementation intent intact for later work.
**Rule inherited from the UX plan §0:** every flow below must be reachable from a **visible
surface** — no command-palette-only, no terminal-only. Claim vocabulary per master §2.4 in every
user-visible string.

**v1.1 note:** adds implementation-facing user stories (§2A) and a memory checkpoint (§15). No
runtime or extension behavior is changed by this document.

---

## 1. Verdict — what is missing from the current plan

The four Rust PLC docs specify a state-of-the-art **terminal** product and *assert* IDE
integration as a by-product ("every existing tool works by construction", RS-60; §15.2 is one
paragraph of surface names). The VS Code UX overhaul plan specifies a complete **ST** IDE and
never mentions Rust. Nobody has designed the intersection, and the implementation board cannot
currently fail a slice for skipping it:

1. **The product contract's DX contract (§3) is terminal-only.** "First contact: `cargo install
   trust` → `trust new rust-plc` → `cargo test` → `trust sim`" directly violates the UX plan's
   non-negotiable §0 rule (every core action on a visible surface, no docs, no terminal). The
   primary persona — a software engineer in VS Code — has no specified path from *install
   extension* to *running machine*.
2. **Project-kind is undefined in the shell.** The extension detects a project by
   `**/trust-lsp.toml` (`trustHomeView.ts:1113-1124`) and scaffolds only ST projects
   (`newProject.ts:20-131`). A `trust.toml + Cargo.toml` project renders as "No truST project".
3. **Check/Run semantics are unmapped.** The sidebar's Compile button shells
   `trust-runtime check --project <root> --json` (`checkProgram.ts:80`) — ST only. `trust check`
   (RS-67: codegen + cargo build + ST compile + digest + admission dry-run) has no JSON contract,
   no Problems mapping, no button state. Run/Start launches the `structured-text` debug adapter
   from a `launch.json` pointing at an ST program — there is no pre-build hook for a module crate.
4. **Live Values is address-based only.** `stIoState` returns flat inputs/outputs/memory rows
   (`runtimeLifecycle.ts:560-615`); there is no instance dimension, so `_STATE/_FAULT_CODE/
   _OVERRUNS`, sequence wait points, budgets/evidence, and fault messages (RS-36, RS-79, RS-93 —
   all promised to "live values" by the master) have no defined payload or rendering.
5. **Admission has no UI.** RS-90's worst-frame report is specified as CLI text. No Problems
   mapping, no report surface, no fix actions, no drift rendering.
6. **The board has no VS Code slices and no gates.** S1's exit criteria mention live values, but
   Demo G0 is entirely CLI. Given this repo's demonstrated failure mode (trust-twin: proof became
   the product; substitution recurring until mechanically gated), the VS Code workflow **will** be
   skipped unless slices + acceptance gates make skipping impossible.
7. **The bootstrap story is missing entirely** (see §12) — scaffolded projects depend on
   `trust-plc`, a pinned toolchain, and rust-analyzer, none of which the VSIX/packaging plan
   provides or verifies.

Everything below is the design that closes these. Structure per section:
**[current]** verified behavior · **[target]** product behavior · **[work]** implementation ·
**[docs]** doc changes · **[proof]** tests/evidence.

---

## 2. The ideal VS Code user workflow (first run, clean machine)

**[target]** — the complete journey, every visible step. Time budget mirrors product-contract
§3.1: **under 10 minutes from extension install to green simulated machine, zero terminal, zero
docs.**

1. **Install.** Marketplace "truST" (platform VSIX; bundles `trust-lsp`, `trust-debug`,
   `trust-runtime`, the `trust` umbrella CLI, project templates, examples). On first activation
   with a Rust-kind project (or during Rust scaffold), the extension recommends `rust-analyzer`
   via `extensions.json` + a one-time prompt — never silently degraded Rust editing.
2. **Open the truST activity-bar icon.** Sidebar shows the existing no-project welcome
   (`trustHomeView.ts:848-866`): `▦ Start from example` · `+ Create project` · `Open project`.
   Nothing else. (Unchanged.)
3. **Create project → kind picker.** `+ Create project` now asks one new question first:
   - **Rust** — "Machine logic in Rust. Tests with cargo, generated IEC 61131-3 compatibility."
   - **Structured Text** — "IEC 61131-3 ST. The classic PLC language."
   Then the existing parent-folder + name prompts. Rust kind runs the bundled
   `trust new rust-plc <name>` (one scaffold truth — the CLI's, never a TS re-implementation),
   plus VS Code extras (§3). Pre-flight: if `cargo`/rustup are missing, the picker's Rust entry is
   **enabled but gated** — choosing it explains what's missing with an "Install Rust…" help
   action and does not write a broken folder (§12).
4. **Project opens.** Folder opens; `src/main.rs` is focused on a seeded, working **motor latch**
   program (Demo G0 shape: 2 inputs, 1 output, 1 `#[retain]` counter, 1 cycle sequence, 1 machine
   test). The sidebar shows: project name + a small kind chip (`Rust`), `Target: Simulator (this
   computer)`, the four-button action row **Compile · Run · Debug · Deploy**, nav **Devices &
   Connections · Libraries · Live Values · HMI**. A "Get started with truST" **walkthrough**
   (new; `contributes.walkthroughs` is empty today) offers kind-aware steps: *Meet your program →
   Run the simulation → Watch live values → Trip a fault → Read the generated ST → Does it fit?*
5. **Run.** One press. The button runs the compile gate (existing pattern,
   `trustHomeView.ts:453-467`): for Rust kind that is `trust check --json` — codegen, cargo
   build, ST compile, digest chain, admission dry-run — with progress on the button and honest
   failure (Problems + one-line summary + "Show output"). On success the simulator launches
   (trust-debug session over the freshly generated bundle) and **Live Values auto-reveals**
   (existing behavior) — now showing the `motor_latch` instance, `_STATE: RUNNING`, its fields,
   and the sequence's current wait point.
6. **Interact.** In Live Values: write `start := TRUE` → motor output flips; the retain counter
   increments; force/unforce work as today (write/force/release verbs preserved per UX plan
   §0.5.5). The status bar shows `truST: Simulator running`.
7. **Trip a fault** (walkthrough step). The template ships a `fault_test` input wired to a
   deliberate panic. Setting it: instance goes `FAULTED` with **message + file:line** in Live
   Values, outputs land on `#[safe_default]` values (tagged "safe default" in the value column),
   the scan continues, a `Reset fault` action appears on the instance (Engineer role). This is F1
   demonstrated *in the product surface*, against today's baseline where a panic kills the
   process.
8. **Edit → update.** Save a `.rs` change while running: the sidebar's **Update running
   simulation** appears (existing sim-only affordance). For Rust kind it is honest per master
   RS-86: module body change within reload parity → rebuild + `bytecode.reload`; interface/digest
   change → the button relabels **Restart required** and does that instead. Never a fake apply.
9. **Read the generated ST.** CodeLens on `#[trust_program]` → "View generated declaration";
   nav/explorer → `src/generated/trust/configuration.st` opens with a "GENERATED — edit
   trust.toml / Rust instead" banner; edits are refused by the digest chain (F22) with the regen
   command named. After any interface change, Compile's result offers **Review generated ST
   changes** → a native VS Code diff (committed vs regenerated) — the PR-diff experience, inside
   the IDE.
10. **Does it fit?** The Compile badge always carries the admission dry-run one-liner:
    `Fits · worst frame 683/1000 µs · declared`. Clicking it opens the **Admission report** panel
    (§7): worst frame first, coincident tasks named, itemized contributors with evidence chips.
    The walkthrough step deliberately lowers a task period to show a **refusal**: build fails,
    Problems gains `F16 worst frame t≡0 (fast+main): 1140 µs > 1000 µs` on the `trust.toml
    [tasks]` line, the report shows the two biggest contributors and the remedies.
11. **Prepare deploy.** Devices & Connections → add the Pi host → `Set up runtime…` (existing
    wizard). The runtime node's inspector **Deploy** section shows the Rust gate checklist
    honestly: `bundle signed ✓ · timing admission: declared-grade — production requires measured
    (run Measure on target) · crate report ✓`. Until the deploy backend (UX plan phase 13)
    exists, `Send to PLC` is absent/disabled-with-reason exactly per §0.6.12 — never a dead
    button. When it exists, a missing/stale `.trusttime` refuses the deploy naming the artifact
    and the fixing command (F20) — in the UI, not just the CLI.

**Brownfield variant** (guarded persona): an ST project is opened; the user adds a Rust FB via
Libraries → "New Rust function block…" (S3+); `trust module declare` regenerates the declaration;
the FB completes/hovers in ST like any POU; its instance appears in Live Values with
`_STATE/_FAULT_CODE/_OVERRUNS`. **Zero Rust-first features** — this is Demo B0, and it must also
be executable entirely from visible surfaces.

---

## 2A. User stories for later implementation

These stories are not public copy. They are the implementation memory for the future VS Code
Rust PLC work. A story is not accepted unless the user can complete it through visible VS Code
surfaces, with no command palette and no terminal, unless the story explicitly says "CLI/CI".

### RVS-001 — First Rust PLC project

**As a software engineer new to PLCs**, I want to click the truST icon, create a Rust PLC project,
and run the seeded simulator in under ten minutes, so that I can see a real machine result before
learning every truST concept.

**Acceptance:** from a clean VS Code profile, the user installs truST, opens the Activity Bar,
chooses `Create project` → `Rust PLC`, sees the generated motor-latch Rust source, presses
`Run`, and sees a RUNNING instance in Live Values. No terminal, no command palette, no hand-edited
configuration files. The run is timed and recorded for G0-in-IDE.

### RVS-002 — Toolchain preflight

**As a first-time user on a clean computer**, I want the Rust toolchain, SDK, bundled `trust`
binary, and rust-analyzer status checked before files are written, so that missing prerequisites
do not turn the first project into a broken folder.

**Acceptance:** the Rust project option remains visible but gated when cargo/rustup/SDK resolution
is missing. The UI names the missing item, offers an honest install/fix action, and leaves the
filesystem unchanged if the user cancels. Version skew between VSIX, `trust`, `trust-plc`, and the
pinned toolchain is refused with a concrete fix.

### RVS-003 — Professional Rust authoring

**As a Rust developer writing PLC logic**, I want normal Rust editing plus PLC-specific feedback,
so that I can use Rust's type system without losing machine context.

**Acceptance:** rust-analyzer provides Rust diagnostics/completion/hover; `#[trust_sequence]`
validator errors appear with preserved spans; exported programs/FBs get one concise CodeLens with
task, period, budget, evidence grade, and generated-declaration link; `[io]` fields show address
chips after Check. The extension does not duplicate cargo/rustc diagnostics.

### RVS-004 — Brownfield Rust FB inside ST

**As a PLC engineer maintaining an existing ST project**, I want to add one Rust function block
from Libraries and use it from ST, so that I can adopt Rust without migrating the whole controller.

**Acceptance:** in an ST workspace, Libraries exposes `New Rust function block...`; the generated
declaration completes in ST; hover shows the Rust docs; an induced Rust panic produces a FAULTED
instance in Live Values while the scan continues. No Rust-first project features are required for
this story.

### RVS-005 — Check means the whole machine check

**As an automation engineer**, I want the sidebar Check/Compile action to check Rust, generated
ST, digests, crate policy, and timing admission together, so that I know whether the project is
fit to run before pressing Run.

**Acceptance:** for `rust-plc`, the existing Compile slot dispatches `trust check --json`.
Problems receive F16/F17/F22/F23-style findings with file/line anchors and fix actions; the
button badge shows clean, dirty, failed, or refused state. A clean result includes the admission
one-liner.

### RVS-006 — Run simulation from the same shell

**As a user who already understands the truST sidebar**, I want Rust PLC simulation to use the
same Target and Run controls as ST, so that Rust does not feel like a separate product.

**Acceptance:** `Target: Simulator` plus `Run` builds/checks the Rust PLC bundle, launches the
simulator/debug session, and auto-opens Live Values. Stop, Update running simulation, and status
bar behavior match today's shell. If the saved edit cannot be hot-updated, the UI says
`Restart required` instead of pretending to apply it.

### RVS-007 — Named Live Values

**As a controls engineer commissioning a machine**, I want Live Values to show named Rust
instances, states, faults, timing, sequence wait points, I/O fields, retain values, and safe-state
tags, so that I can reason about the running machine without reading raw addresses.

**Acceptance:** Live Values keeps the existing I/O map and adds an Instances tab. The tab shows
`_STATE`, `_FAULT_CODE`, `_OVERRUNS`, p99 vs budget, evidence grade, active sequence wait point
with file:line, field groups, forced state, stale/fresh quality, and safe-default tags. FAULTED
rows expand to message, source link, reset action, and flight-recorder link when available.

### RVS-008 — Fault proof in the UI

**As a safety-minded reviewer**, I want the seeded demo to prove a Rust panic becomes a managed
PLC fault in VS Code, so that the safety claim is visible and not only a terminal log.

**Acceptance:** the G0 demo includes a visible fault trigger. Activating it leaves the scan
running, sets the instance to FAULTED, applies safe-default outputs, increments/report overruns
as appropriate, and shows source location. The evidence includes screenshots/recording from the
VS Code capture harness.

### RVS-009 — Generated ST review

**As a brownfield PLC engineer or auditor**, I want to inspect the generated IEC artifacts and
diff them after Rust changes, so that Rust-first authoring remains compatible with IEC review
practices.

**Acceptance:** generated ST opens read-only with a generated banner; a CodeLens/action from the
Rust POU opens the generated declaration; Check can offer `Review generated ST changes` as a
native VS Code diff; direct edits to generated files trigger the F22 digest/refuse/regenerate
flow.

### RVS-010 — Admission as a report, not a log

**As an automation lead deciding whether a task set fits**, I want the timing/crate admission
result shown as a report with the worst frame first, so that I can see bursts, coincident tasks,
and evidence grade instead of average utilization.

**Acceptance:** a Check result opens an Admission report panel using the shared theme. It shows
ADMITTED/REFUSED, profile, evidence floor, digest, worst frame, coincident tasks, contributor
bar/table, crate verdicts, and remedies. F16 anchors on `trust.toml`; F17 anchors on `Cargo.toml`.
Verdict strings use the master claim vocabulary.

### RVS-011 — Machine tests in VS Code Testing

**As a test-focused Rust developer**, I want machine tests, replay tests, and unit tests to appear
in VS Code's native Testing view, so that controller logic can be run and debugged like normal
Rust tests while still producing machine traces.

**Acceptance:** cargo/libtest discovery feeds Testing; machine tests are grouped separately; a
failing simulation test can attach a shrunk `.trusttrace`; `Replay trace` reruns against the
current build and shows divergence in a report panel.

### RVS-012 — Deploy gates are honest

**As a commissioning engineer**, I want deploy readiness shown on the selected runtime/device
node, so that I know exactly which evidence is missing before sending code to a PLC.

**Acceptance:** the sidebar Deploy button focuses the Devices & Connections deploy section when
available. Until the backend exists, it is disabled or absent with a reason. When available, the
section checks signed bundle, fresh `.trusttime`, required evidence grade, crate report, and
target capability; F20 names stale artifacts and fix commands.

### RVS-013 — Runtime drift explanation

**As a maintainer responding to a slow machine**, I want drift and overrun diagnostics linked to
the admitted budget and source, so that I can explain why a previously accepted task is now
risky.

**Acceptance:** runtime drift raises chips in Live Values and the selected runtime node; Explain
opens a timeline/report naming p99, admitted budget, offender, evidence grade, and source
location.

### RVS-014 — One shell, one theme

**As an extension maintainer**, I want Rust PLC surfaces to reuse the existing truST Home shell
and shared webview theme, so that the extension does not split into a Rust product and an ST
product.

**Acceptance:** no new activity container; no second Run surface; no private theme file or
per-surface palette. New panels use `theme.css` tokens and pass light/dark/high-contrast visual
smoke. Commands are palette-hidden unless they are also reachable from visible surfaces.

### RVS-015 — CLI/CI parity without CLI-first UX

**As a team lead**, I want every VS Code action to have an equivalent CLI/CI spelling, so that
the nice UI does not become a manual-only workflow.

**Acceptance:** Create, Check, Test, Build/Admit, Replay, and Deploy all map to versioned CLI
commands with JSON where the extension consumes output. The VS Code path is primary for humans;
the CLI path remains primary for CI. Neither path invents a separate contract.

---

## 3. Professional Rust PLC project layout on disk

**[current]** `trust new rust-plc` layout is specified in master §5.1. The ST scaffold writes
`src/Main.st`, `src/config.st`, `trust-lsp.toml`, `runtime.toml`, `io.toml`,
`.vscode/launch.json` (`newProject.ts:210-240`).

**[target]** — master §5.1 verbatim, plus the VS Code deltas (marked ←):

```
latch/
├── Cargo.toml                  # cdylib+rlib; deps: trust-plc (pinned, §12), trust-sim (dev)
├── trust.toml                  # topology + policy only (App. C); protective defaults (RS-68)
├── rust-toolchain.toml         # pinned toolchain (RS-55)
├── trust-lsp.toml              # ← LSP roots: ["src/generated", "src/st"] — trust-lsp indexes
│                               #   generated + any hand-written ST; kind detection does NOT
│                               #   key on this file (it keys on trust.toml + Cargo.toml)
├── .vscode/
│   ├── launch.json             # ← type: structured-text, kind-aware (pre-build via trust check)
│   └── extensions.json         # ← recommends rust-analyzer
├── src/
│   ├── main.rs (or lib.rs)     # #[trust_program] MotorLatch + one #[trust_sequence]
│   └── generated/trust/        # GENERATED — committed, digest-pinned, read-only in the IDE
│       ├── declarations.st
│       ├── configuration.st
│       └── devices_<n>.st|rs   # from `trust io import` (optional)
├── services/<name>/            # tier-3 binary crates (optional)
├── tests/
│   ├── machine.rs              # trust-sim machine tests (§13.2)
│   └── replay_commissioning.rs # replay gate over traces/ (RS-113)
├── traces/
│   └── commissioning.trusttrace  # committed evidence (recorded later; folder scaffolded)
├── io.toml / runtime.toml      # GENERATED from trust.toml — committed, reviewable
└── target/trust/               # NOT committed: bundle staging, .trusttime, .trustcrate,
                                # admission + crate reports (JSON), replay reports
```

Rules the IDE relies on: generated artifacts committed and byte-deterministic (RS-61/64);
`.trusttime`/`.trustcrate` are build outputs under `target/trust/` (digest-bound, regenerated),
never hand-committed; committed traces live in `traces/`. **Deploy-target inventory is
fleet-level, not project-level** — named targets (`trust deploy cell-01`) resolve from the same
inventory Devices & Connections owns (the `fleet.toml` mechanism managed local runtimes already
use, `localRuntimeModel.ts:1-30`), because cells are infrastructure, not project source. This
needs ratifying in the master (§9 doc patches — currently unspecified there).

---

## 4. The Rust authoring experience

**[target]** What the engineer writes is master §6.1/§6.3 (normative shapes). What each tool
shows, and the division of labor:

- **rust-analyzer owns Rust.** Completion/types/hover/refactors/inlays on `Exchange<Self>`
  fields, trait impls, sequences. Two SDK obligations make this true (they are product
  requirements, not niceties): (1) `trust-plc` macros MUST expand to rust-analyzer-legible code
  with preserved spans — `#[trust_sequence]` F23 rejections are emitted as spanned
  `compile_error!`s from the proc macro, so a foreign `.await` or unknown macro squiggles **live
  in the editor**, before any build; (2) SDK docs (`///`) are written for hover-first consumption
  — the trait/method docs are the primary onboarding surface.
- **trust-lsp owns ST** — generated and hand-written. On generated files it should be silent: a
  diagnostic inside `src/generated/**` is by definition a toolchain bug and is labeled as such
  ("generated ST failed to compile — this is a truST bug; report it"), never blamed on the user.
  Brownfield: declared Rust FBs are ordinary LSP symbols (completion, hover with Rust docs,
  go-to-declaration; go-to-definition jumps into module source when source-form — RS-56).
- **The extension adds the machine dimension**, sparingly:
  - One CodeLens per exported POU: `PROGRAM · task main (10 ms) · budget 80 µs (declared) · view
    generated declaration`. One line, no stacking.
  - Gutter chips on `[io]`-bound fields: `%IX0.0` (from trust.toml, resolved by Check) — the
    address truth visible where the field is authored.
  - Problems entries **only** for truST-specific findings via `trust check --json` (§6): manifest
    errors (RS-65/66, both sides quoted), digest drift (F22 + regen command as a quick fix),
    admission (F16), crate policy (F17 on the `Cargo.toml` dependency line). Cargo/rustc errors
    are NOT duplicated — rust-analyzer/cargo own them.
- **Diagnostics the author sees, by source:** editor-live (rust-analyzer: type errors, F23 with
  the construct named) → Check (F22, RS-65/66, F16, F17) → runtime (F1 panic with
  message+location, F21 sequence deadline with wait-point location, F18 admission drift) — every
  one naming artifact, expectation, finding, fixing command (master §15.3).
- **Hidden from beginners** (progressive disclosure, UX plan §0.6.12 applied to language
  surface): tiers/ABI/vtables/`module.toml`/forms terminology; `unsafe` (impossible in user code,
  P7); the 27 `trust-runtime` subcommands; raw control-plane; `runtime.toml`/`io.toml` mechanics
  (generated, foldered); "admission mathematics" beyond the report. The words the UI uses are the
  claim vocabulary (§2.4) and plain PLC verbs — enforced by contract test (§11).

---

## 5. VS Code surface design (Home / Activity Bar states)

**[current]** One activity container + one `trust.home` WebviewView; two states (welcome /
project); Target picker (Simulator / managed local / remote); four-button action row; sim-only
Update; nav ×4; passive status bar; native Testing; examples gallery with hardware badges. All
contract-tested (139 ux-shell tests).

**[target]** **The shell is kind-agnostic; kind changes wiring, not chrome.** No new containers,
no new sidebar sections, no second Start anywhere. One new pure model: `projectKind`.

| Kind | Detection (priority order) | What changes |
|---|---|---|
| `rust-plc` | `trust.toml` **and** `Cargo.toml` at project root | Compile→`trust check`; Run→build+sim pipeline; Tests→cargo machine tests; generated-ST affordances; admission badge |
| `st-modules` | `trust-lsp.toml` and `modules/*/module.toml` | ST wiring + module instances in Live Values (B0) |
| `st` | `trust-lsp.toml` | today's behavior, unchanged |
| none / nonTrust | — | welcome (unchanged; `+ Create project` gains the kind picker) |

Sidebar by situation (the six the design must answer):

1. **No project** — unchanged welcome. Examples gallery gains Rust starters with a `Rust` badge
   beside the hardware badge: *Motor latch (Rust)* — `No hardware`; *Palletizing cell (Rust)* —
   `No hardware` (master §16.1, ships at S6).
2. **Rust PLC project** — project name + `Rust` kind chip; Target picker unchanged (Simulator /
   managed / remote — one inventory); action row: **Compile** (badge = check + admission
   one-liner) · **Run** (Start/Stop per target, existing state machine) · **Debug** · **Deploy**
   (launcher, gated); nav unchanged. HMI adaptive (Create HMI when no descriptor — generated
   variables make HMI binding work as for ST).
3. **Brownfield ST project (with or without Rust modules)** — exactly today's sidebar; Libraries
   gains "New Rust function block…" (S3+); Live Values shows module FB instances when present.
4. **Running simulation** — Run→Stop; Update running simulation on save; for Rust the update
   verb is honest per RS-86 (apply vs **Restart required**, §2 step 8). Live Values populated;
   status bar `truST: Simulator running`.
5. **Failed admission** — Compile badge goes danger: `Doesn't fit · +140 µs (F16)`; Run gated
   with the same shared reason model that gates compile failures today (production profile;
   development profile shows a warning badge but does not gate — mirrors §9.4). Problems carries
   the itemized entries; the badge click opens the report.
6. **Deploy-ready target** — the *node inspector* (Devices & Connections) owns deploy per UX plan
   §0.6.5/§0.6.6; the sidebar Deploy button is a **launcher** that focuses that section when a
   real backend exists for the selected target, else stays disabled-with-reason (current
   behavior at `trustHomeView.ts:1324-1337` is already honest — keep it until phase 13). Gate
   checklist rendered per §2 step 11.

```
┌ TRUST ──────────────────────┐
│ Latch  · Rust               │   ← name + kind chip
│ TARGET                      │
│ [ Simulator (this computer)▾]│
│ ┌──────┬──────┬──────┬─────┐│
│ │Compile│ Run  │Debug │Deploy│   ← same 4 slots; Compile badge: "Fits · 683/1000 µs · declared"
│ └──────┴──────┴──────┴─────┘│
│ (Update running simulation) │   ← sim-only; Rust: apply or "Restart required", never fake
│ ▤ Devices & Connections     │
│ ▦ Libraries                 │
│ ◉ Live Values               │
│ ▭ HMI                       │
└─────────────────────────────┘
```

---

## 6. The main actions — exact behavior

**[target]** (each entry: trigger → behavior → failure honesty). Backend contracts marked **BC**
follow the UX plan §0.6.11 maturity rule: the UI ships absent/disabled-with-reason until the
owning side lands the contract.

1. **Create Rust PLC Project.** Welcome/`+ Create project` → kind QuickPick (§2 step 3) → folder
   + name (existing flow, `newProject.ts` arg contract preserved for tests) → shells bundled
   `trust new rust-plc <name>` → writes §3 layout → opens folder, focuses the program source.
   Failure: missing toolchain → §12 flow; scaffold error → nothing half-written (CLI is atomic or
   cleans up), message names the step.
2. **Check (the Compile button).** Rust kind dispatches `trust check --project <root> --json`
   (**BC-1**: versioned JSON schema — sections `generate`, `cargo`, `st_compile`, `digests`,
   `admission` (dry-run, evidence floor per profile), `crates`; per-issue `{file, line, severity,
   code, message, fix_command?}`). Problems shows truST-specific findings only (§4); the badge
   states extend today's model: `unknown / dirty / clean+fits / failed(n) / refused(+Δµs)`.
   Clean shows the admission one-liner. Click on a clean/refused badge → Admission report (§7).
3. **Run Sim.** Compile gate (existing, extended with BC-1) → launch `structured-text` debug
   session for the sim over the checked project (kind-aware `DebugConfigurationProvider` performs
   the module build via the check/build pipeline before launch — **BC-2**: the adapter's launch
   contract for rust-plc bundles; the run/play path already recompiles ST from `src/` per master
   §3.1, so generated sources flow through it) → auto-reveal Live Values. Stop unchanged.
   Failure: launch errors classified as today (`runtimeLifecycle.ts:270-322`), plus "module build
   failed" → Problems + output link.
4. **Test.** Native **Testing** view (no new sidebar button; the row stays 4 slots). The existing
   truST test controller (ST `TEST_PROGRAM`/`TEST_FUNCTION_BLOCK`, `stTests.ts`) gains a rust-plc
   branch: discovery via `cargo test --no-run` + libtest `--list --format json`, execution via
   `cargo test --format json`, items grouped *Machine tests* / *Replay* / *Unit*; click-to-open
   from libtest file:line. A failing trust-sim property test that shrinks to a trace attaches the
   `.trusttrace` path to the TestMessage with a **Replay trace** action (RS-112). rust-analyzer's
   own runner/debug remains available — we don't fight it, we add the machine grouping and trace
   affordances. One honest footer note on machine-test runs: *simulated behavior; timing is
   admission's job* (RS-110).
5. **Admit.** Not a fifth button. Development admission = the dry-run inside every Check (badge +
   report). Production admission = `trust build --profile production` from the Admission report
   header: `Profile: development — production requires measured evidence · [Measure on target…]`
   — enabled only when a non-simulator target is selected and the measurement backend exists
   (**BC-3**: `trust timing measure` orchestration + progress + `.trusttime` result path), else
   disabled-with-reason. Results re-render the report with `measured (target: cell-01)` chips.
6. **View Generated ST Diff.** Three entries, one implementation: (a) Compile result state
   `generated-changed` → **Review generated ST changes** opens a multi-file diff
   (committed vs freshly generated — the check pipeline regenerates to a temp dir); (b) CodeLens
   on exported POUs → view declaration/configuration; (c) explorer context on `src/generated/**`.
   Generated files open with a banner decoration and are digest-protected (F22): an edit attempt
   is met with the regen command and "edit trust.toml / Rust source instead".
7. **Live Values.** Nav unchanged; content per §8.
8. **HMI.** Unchanged adaptive `Open HMI`/`Create HMI` (`trustHomeView.ts:1375-1382`); Rust
   projects bind HMI to generated variables; `#[unit]`/`#[range]` metadata (RS-72) flows into
   display formatting.
9. **Deploy.** Sidebar button = launcher to the selected target's node-inspector **Deploy**
   section (§0.6.5 `Send to PLC` when phase-13 backend lands — **BC-4**). Rust gates rendered
   honestly: signed bundle, `.trusttime` fresh + grade ≥ profile floor, `.trustcrate` clean —
   refusal names the artifact + fixing command (F20) in the inspector, not a toast.
10. **Replay Trace.** `traces/*.trusttrace`: editor/explorer context **Replay against current
    build** → `trust replay --json` (**BC-5**) → result as a Testing run (pass/divergence) + a
    divergence table (per-variable, tolerances) in the report panel family; overrun cycles offer
    **Explain** (RS-107 view). Replay test files in `tests/` run under action 4 automatically.

---

## 7. Admission & deploy workflow in the IDE

**[current]** Nothing. RS-90's report is CLI text; `record_call` ranks contributors; drift
(RS-93) is a runtime diagnostic with no surface.

**[target]**

- **Admission report panel** — editor-area webview (same family as Libraries/Examples; shared
  `theme.css` tokens; light/dark/HC per the theme contract):
  - Header: verdict **ADMITTED / REFUSED** + profile + evidence floor + digest + target.
  - **Worst frame first** (RS-90): a stacked horizontal bar of contributors against `F` with the
    reserve visible; the coincident tasks named in the title (`t≡0 — fast + main, coincident
    release`); a hyperperiod strip (H/F frames, per-frame fill, worst highlighted) so "bursts,
    not averages" is *visible*.
  - Contributor table: name (→ source), budget, evidence chip (`declared` grey "author-asserted"
    / `measured (target)` / `wasm_fuel` / `static_stbc` / `builtin_maintained` — §2.4 wording
    verbatim), share of frame.
  - Remedies as rows (from RS-90), actionable where honest: "Move `palletizer::Palletizer` to a
    slower task — edit `trust.toml [tasks]`" (opens the line); "Demote to a service" (doc link).
  - Crate section: per-crate verdict chips + evidence class + expandable call path
    (`motion::cycle → reqwest::Client::send`) or explicit `unknown` reason + remedy (RS-94).
- **Problems integration:** REFUSED → one error per frame deficit anchored on the `trust.toml
  [tasks]` entry (`trust(F16)`), info entries for top contributors; crate violations anchored on
  the offending `Cargo.toml` dependency line (`trust(F17)`) via cargo-metadata mapping; each with
  a "Show admission report" action. Fix commands surface as quick-fix-style actions that run the
  named command.
- **Drift (RS-93/F18):** runtime raises admission-drift → Live Values instance row gains a
  warning chip (`p99 212 µs > admitted 150 µs`), the Devices & Connections runtime node badges
  it, and **Explain** opens the RS-107 timeline (same panel, retrospective mode) naming the
  offender against its admitted budget.
- **Deploy gate:** §6.9. The inspector's Deploy section is the single place deploy-readiness is
  judged; the sidebar never claims it.

---

## 8. Live Values for Rust PLC (and modules generally)

**[current]** Address-based tables (inputs/outputs/memory) with name/value/type/forced; write/
force/release/release-all with per-write audit and viewer gating; lifecycle pill; target label;
forced filter (`ioPanel.ts:519-580`, ux-shell tests). No instances, no module status.

**[target]** Two tabs in the same panel, same theme, same verbs:

1. **I/O map** — today's table, unchanged (it is correct for commissioning by address).
2. **Instances** — the machine as the author structured it:

```
TASK main (10 ms)                                   scan #48 231 · fresh
└─ palletizer : PALLETIZER            [RUNNING] · 78 µs p99 / 80 µs budget · declared
   ▶ waiting at until(io.axis_in_position)  palletizer.rs:214 · deadline 3 s · 1.2 s elapsed
   inputs   start TRUE · safety_ok TRUE · axis_in_position FALSE (%IX0.2)
   outputs  gripper_close_cmd FALSE [safe_default] · target_position_mm 812.5 mm (%QL100)
   retain   boxes_placed 7 ⟳
└─ recipe_supervisor : RECIPE_SUPERVISOR  [RUNNING] · svc recipe: fresh (age 2 cycles)
```

- **Status header per instance:** `_STATE` pill (RUNNING/FAULTED/…), `_FAULT_CODE`, `_OVERRUNS`,
  p99 vs budget with evidence chip, drift warning when RS-93 fires.
- **FAULTED expands** to message + file:line ("panicked at 'index out of bounds',
  motion.rs:88"), *Open source*, *Reset fault* (Engineer; `module.reset`), and the
  flight-recorder dump link when present (RS-108).
- **Sequence wait point** (RS-79's captured location): the active-step line shown above —
  SFC's step view, for free; click opens the source; deadline + elapsed shown when armed.
- **Fields:** grouped in/out/retain; addresses shown where bound (`[io]`); units/ranges from
  RS-72 metadata; retain marker; device-view fields carry quality (ADS `_quality` convention);
  `ServiceLink` values show freshness/age and **dim when stale** — staleness is a rendered state,
  never a silent old number.
- **Forcing:** same verbs and gating as today. Bound fields force by address (existing
  `stIoWrite/stIoForce/stIoRelease` path); non-bound instance variables use the `var.force`
  family where the target authorizes it — per-cell capability gating exactly like the UX plan
  §0.5.16 matrix, disabled-with-reason otherwise. `Release all forces` unchanged and global.
- **Safe state:** instance-level — outputs on defaults tagged `safe_default`; resource-level —
  a banner when the runtime is in SafeHalt with safe state applied ("Safe state applied — outputs
  at configured safe values"), sourced from runtime state, never inferred. Plain Halt/stop shows
  what actually happened (outputs frozen / no safe state) per master §14.2 honesty.
- **Freshness:** the existing scan counter becomes a visible `scan #N · fresh/stale` chip; a
  snapshot older than N scans dims the tab.

**BC-6 (backend contract, owns the slice gate):** extend the DAP/control snapshot with an
`instances` section (served via the existing debug pipeline — RS-57 forbids a parallel one):

```
instances: [{ instance, type, kind: "module"|"st", task,
  state, fault_code, fault?: {message, file, line}, overruns,
  timing: {last_us, p99_us, budget_us, evidence},
  seq?: {label, file, line, deadline_ms?, elapsed_ms?},
  fields: [{name, dir: "in"|"out"|"retain"|"var", value, type, unit?, address?,
            forced, quality?, safe_default?, stale?: {age_cycles}}] }]
```

Viewer role reads; cycle-coherent snapshot semantics as today.

---

## 9. Implementation architecture (mapped onto current code)

**[work]** Reuse the shell; add pure models + thin surfaces; god-file rule enforced (standing
board gate + the SOLID/KISS review memory — `ioPanel.ts` at 2 422 lines and `trustHomeView.ts` at
1 503 are already at the limit; nothing new lands inside them).

| Concern | Where | Notes |
|---|---|---|
| Project kind | **new** `src/projectKind.ts` (pure) + detection in `trustHomeView.ts` replacing the trust-lsp.toml-only check (`:1113-1124`) | ONE store, consumed by home view, check dispatch, debug config provider, test controller — the `selectedRuntime.ts` single-store pattern |
| Scaffold | `newProject.ts` gains the kind picker; Rust branch shells the bundled CLI (**never** a TS scaffold twin) via **new** `src/rustProject/scaffold.ts` | keeps the test-arg contract (`simulateCancelAt` etc.) |
| Check | `checkProgram.ts` stays the command + dispatch; **new** `src/rustProject/checkModel.ts` (pure parse of BC-1 JSON → badge/Problems view-model) | one diagnostics entry point for all truST findings (mirrors RS-60's one pipeline) |
| Admission UI | **new** `src/rustProject/admissionModel.ts` (pure) + `admissionPanel.ts` (webview, `theme.css`) | report + drift + replay-divergence + explain share this panel family |
| Generated ST | **new** `src/rustProject/generatedSt.ts` — banner decorations, CodeLens, diff command | uses VS Code native diff; no bespoke viewer |
| Live Values instances | **new** `src/io-panel/instances.ts` + webview module | `ioPanel.ts` is NOT appended; the tab composes |
| Run pipeline | `src/debug/configuration.ts` gains a kind-aware `DebugConfigurationProvider` (pre-build via check/build) | trust-debug adapter changes live behind BC-2 |
| Tests | `stTests.ts` → controller split: **new** `src/rustProject/machineTests.ts` for the cargo branch | libtest JSON; trace attachments |
| Traces | **new** `src/rustProject/traces.ts` (context actions, replay run) | BC-5 |
| Deploy | Devices & Connections node-inspector Deploy section per UX plan §0.6.6; sidebar button becomes a launcher | BC-4; gate rendering shares `admissionModel` grades |
| package.json | commands: `generatedSt.open/diff`, `admission.show`, `trace.replay` (+ palette-hidden per policy); one walkthrough; examples manifest + Rust starters | no new containers/views |

**Where diagnostics/admission JSON enters VS Code:** exclusively `trust check --json` /
`trust build --json` / `trust replay --json` parsed by the pure models. The extension never
scrapes CLI text, never re-derives admission math, and never adds a third diagnostics source
beside rust-analyzer (Rust) and trust-lsp (ST).

**Backend contracts to specify before their UI slices (owning side: runtime/CLI):** BC-1 check
JSON schema · BC-2 rust-plc launch/build handshake for trust-debug · BC-3 measure orchestration ·
BC-4 deploy/Send-to-PLC (already UX-plan phase 13) · BC-5 replay JSON · BC-6 instances snapshot.
Each needs request/response shape, idempotency, auth role, error taxonomy, audit behavior
(§0.6.11 maturity bar) before its phase starts.

---

## 10. Doc-hardening status (v2.4)

Completed in the v2.4 hardening pass:

1. Master spec §15.2 now contains RS-118..RS-126 as binding requirements.
2. Product contract §3 contains the IDE first-contact flow as an equal-rank
   done criterion.
3. Runtime contract contains BC-1..BC-6 and mirrors RS-118..126.
4. Implementation board contains S13..S17 and G0-in-IDE gates.
5. D25 locks SDK distribution v1 to crates.io exact pins; D26 locks deploy
   target inventory to fleet-level infrastructure.
6. `vscode-ux-overhaul-plan.md` contains §0.7 Project kinds - Rust-first
   shell extension.
7. `rust-plc-vscode-acceptance-journeys.md` carries J-R1..J-R6 for later
   mirroring into the broader UI/UX acceptance board.

---

## 11. Required implementation-board changes (slices + gates)

**Amend standing gates (board header):** add — *"Any slice whose exit criteria include a
user-visible surface MUST name its VS Code surface and attach capture-harness screenshot evidence
(light/dark/HC for core surfaces); a CLI-only demo does not close such a slice."* And: *"Claim
vocabulary (master §2.4) applies to extension strings; the forbidden-word contract test (§12
below) must stay green."*

**Amend existing slices:**
- **S1 exit 2** (live values show the instance + `_STATE/_FAULT_CODE/_OVERRUNS`): append "…in
  the VS Code Live Values panel via the existing debug pipeline (BC-6 thin version), with
  screenshot evidence" — S1 already promises this; make it un-skippable.
- **S3 / Demo B0 checklist:** the live-values and IO-panel rows name the VS Code panel and
  require screenshots; add "declaration completion/hover shown in VS Code".
- **S6 / Demo G0 checklist:** add — *"The entire G0 checklist is repeated inside VS Code from
  visible surfaces only (no terminal, no command palette), timed, under 10 minutes, screen
  recording + screenshots attached (J-R1)."* Two timed runs total: CLI and IDE.
- **S7 exit:** add — *"The refused task set (the passes-in-isolation regression fixture) is shown
  refused in the IDE: Compile badge, Problems F16 entry on trust.toml, report panel naming the
  coincident tasks (J-R4 evidence)."*
- **S12:** public docs gain `docs/public/develop/rust/vscode-workflow` (+ section `index.md`);
  acceptance journeys J-R1..R6 reach `ux_accepted`.

**New slices (appended; numbering stable):**

| # | Slice | Demonstrates | Depends | Gate (exit criteria) |
|---|---|---|---|---|
| S13 | VS Code Rust-first shell | kind detection, kind-picker scaffold via `trust new`, Compile=`trust check --json` (BC-1), Run pipeline (BC-2), generated-ST read-only+diff, walkthrough, Rust starter | S6 (thin parts testable against S1 fixtures) | **Demo G0-in-IDE** (above); no-terminal proof; contract tests green; scaffold tests incl. "check green on a fresh scaffold" |
| S14 | Live Values instances + fault workflow | instance tree, `_STATE/_FAULT_CODE/_OVERRUNS`, fault expand + reset, sequence wait point, safe-state tags, named forcing | S1 (telemetry), S13, BC-6 | B0-in-IDE rows pass; F1 fixture demonstrated in-panel (screenshot); capability matrix honored (disabled-with-reason proofs) |
| S15 | Admission in the IDE | report panel, Problems anchors (F16 trust.toml / F17 Cargo.toml), evidence chips, measure-on-target gating, drift chip + Explain | S7, S13 | refused-set rendered (J-R4); drift fixture surfaces in Live Values + node badge; every verdict string carries its evidence class |
| S16 | Machine tests + traces in the IDE | Testing-view cargo integration, shrunk-trace attachments, replay runs + divergence table, explain-overrun view | S6 (tests), S10 (replay), S13 | a deliberate regression fails as a Testing item with variable diff (F19) and a working Replay action (J-R6) |
| S17 | Deploy gates in the IDE | node-inspector Deploy section rendering F20 gates; Send to PLC honesty | S13, UX-plan phase 13 (BC-4) | missing/stale `.trusttime` refusal rendered naming artifact + fix command; no dead button states (omission/disabled-with-reason proofs) |

Dependency note: S13 model/contract-test work may begin against S1's thin project the moment
BC-1's schema is frozen — schema freeze is therefore part of S1's "generated ST byte-stable"
gate family, not an afterthought.

---

## 12. Bootstrap story

"Create project → Run, under 10 minutes" depends on explicit bootstrap checks:

1. **SDK resolution.** D25 locks v1 to crates.io exact pins. The scaffold's
   `Cargo.toml` writes `trust-plc = "=x.y.z"` and `trust-sim = "=x.y.z"`
   matching the bundled `trust` tool. First build may need network unless the
   crates are cached. Bundled/offline SDK source is deferred until a field
   need justifies the packaging cost.
2. **Toolchain presence.** `rust-toolchain.toml` pins 1.95 — on a machine without rustup, `Run`
   must fail *before* scaffolding with a guided, honest flow ("Rust isn't installed — Install
   Rust… / Use Structured Text instead"), and with rustup present the pinned-toolchain download
   needs consent + progress, not a silent multi-minute hang inside a Compile press.
3. **rust-analyzer.** Recommendation + graceful degradation wording when absent (the machine
   still runs; editing is degraded — say so).

RS-126 and S13 own the pre-flight. Bootstrap is no longer unowned: version
skew is refused, missing tools are explained before writes/runs, and
rust-analyzer absence is an honest degraded-editing state.

---

## 13. Required tests and proof (consolidated)

- **Pure-model unit tests** (no `vscode` import — the `trustHomeModel.ts` pattern):
  `projectKind` precedence; BC-1 parse → badge/Problems view-models (incl. refused ordering,
  evidence chips, fix commands); instances normalization (BC-6, incl. stale/fault/seq variants);
  trace/replay result models; admission remedy mapping.
- **Contract tests** (extend `ux-shell-contract.test.ts` / `runtime-controls-contract.test.ts`):
  kind-aware sidebar states (six situations of §5); Compile dispatch per kind; Run gating incl.
  admission (production) vs warning (development); Update honesty (apply vs Restart required —
  never fake); Deploy honesty preserved; generated-ST read-only affordance; **claim-vocabulary
  lint** — no extension string contains "proves/guaranteed/certified/WCET/crash-proof/pure Rust"
  (greps the bundle, mirrors the master's forbidden-phrasings table); **no-palette-only** — every
  new command reachable from a visible surface or `when:false`.
- **Scaffold tests** (extend `new-project.test.ts`): Rust kind writes the full §3 layout via the
  bundled CLI; cancel at each stage leaves the filesystem unchanged; *a fresh scaffold passes
  `trust check`* (the Rust analog of the existing "generated ST parses cleanly" test at
  `new-project.test.ts:301`); bootstrap pre-flight paths (no cargo → gated, no half-written dir).
- **Backend schema tests** (runtime side): BC-1/BC-5/BC-6 JSON snapshot tests; the S7 regression
  fixture emits the exact worst-frame JSON the panel consumes.
- **Capture-harness evidence** (the saved runners at
  `docs/internal/testing/evidence/vscode-ui-ux-acceptance/2026-06-25/runners/`): command-driven
  runners for sidebar kind states, Live Values instances (needs the S1 module fixture), fault
  workflow; CDP runners for the admission panel DOM and generated-ST diff; theme triplet
  (light/dark/HC) for Live Values instances + admission panel; screenshots filed on the
  acceptance board rows (J-R1..R6), reviewer ≠ implementer.
- **Timed acceptance:** the G0-in-IDE run recorded (screen capture + wall clock) — the ten-minute
  claim gets evidence, or the claim changes.

---

## 14. The professional end-state (why this beats both alternatives)

**Better than classic ST tooling** because the author gets a real language (types, enums-with-
data, cargo, refactoring, AI assistance that actually knows the language), the machine runs in
CI (`cargo test` drives real cycles with virtual time), "does it fit?" is answered at build time
with an itemized, evidence-graded account, and last night's fault replays in a test — none of
which TIA/CODESYS/PLC++-era tooling offers, and all of it visible in the same panel language a
controls engineer already reads (live values, forcing, HMI, topology).

**Better than "Rust plus a terminal"** because the IDE shows the *machine*, not just the code:
named instances with states, budgets, and fault locations; the sequence's current wait point as
a live SFC step; safe-state honestly tagged; the generated IEC artifacts one diff away for the
auditor and the brownfield colleague; admission refusals anchored to the exact `trust.toml` and
`Cargo.toml` lines that caused them. The terminal remains fully supported (CI uses nothing
else) — but no human is ever *required* to leave the editor between "empty folder" and
"program running on the target, with evidence."

The tell that it's done: a software engineer who has never seen a PLC and a PLC engineer who has
never run cargo can sit at the same project — one lives in `src/*.rs`, the other reads
`src/generated/trust/*.st` and Live Values — and neither can find a fabricated state, a dead
button, or a claim stronger than its evidence grade.

---

## 15. Memory checkpoint for future implementation

If only one page of this document survives into the implementation phase, preserve these
decisions:

1. **VS Code-first is a product requirement.** The CLI remains complete, but Rust PLC v2 is not
   done until the G0 flow runs from visible VS Code surfaces.
2. **The existing truST shell is the product shell.** Add `projectKind` and kind-aware wiring;
   do not create a second Rust activity bar, second Run surface, or command-palette workflow.
3. **The minimum first slice is project recognition + scaffold + Check/Run + thin Live Values.**
   The rich panels can mature later, but a Rust project must not render as "No truST project".
4. **Live Values must become instance-aware.** Address tables alone cannot prove Rust PLC safety,
   faults, sequence waits, retain state, or admission drift.
5. **Admission must be visible as evidence.** Worst-frame, crate verdicts, and evidence grades
   belong in Problems and a themed report panel, not only terminal text.
6. **Generated ST is a reviewed artifact, not a user-edit target.** Make it read-only, diffable,
   digest-protected, and navigable from Rust and ST.
7. **Bootstrap is part of the UX.** Toolchain, SDK version, `trust` binary, and rust-analyzer
   readiness are checked before the first project/run can fail confusingly.
8. **Every new backend contract enters VS Code through versioned JSON or the existing debug
   pipeline.** No CLI-text scraping, no parallel telemetry, no duplicated admission math in
   TypeScript.
9. **User stories RVS-001..RVS-015 are the acceptance memory.** Later implementation boards
   should map each story to slices, tests, and screenshot evidence before code starts.

---

*End v1.1. Review with: does any flow here require the palette or a terminal? does any surface
claim more than its evidence? does any slice lack a gate that would catch it being skipped?*
