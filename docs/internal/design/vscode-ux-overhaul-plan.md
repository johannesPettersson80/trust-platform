# truST VS Code Extension — UX Review & Overhaul Plan (v5, complete PLC IDE)

> Status: v5, 2026-06-23 — scope correction. v4 specified the **run / select / connect / see-values
> shell** but still treated runtime **install / deploy / lifecycle** on real targets as "later/deferred."
> That was wrong: the product goal is a **complete PLC IDE** — create a project, simulate, connect or
> install a runtime (this computer / Raspberry Pi / IPC / Docker), select a target, deploy the program,
> inspect + write/force live values, build HMI, and update/uninstall — all **without docs**. v5 keeps the
> v4 model intact (Run bar = select + own-connection; Devices & Connections = inventory + per-node
> lifecycle; Live Values = read + write/force) and adds **§0.6**, the authoritative end-to-end
> install/deploy/lifecycle contract, plus a real milestone plan that marks what "complete IDE" requires.
> **§0.5 and §0.6 are AUTHORITATIVE** and supersede the run-card / sidebar / deploy specifics in
> §4/§6/§8/§10 where they differ.
> For shipped behavior, `docs/specs/25-vscode-product-contract.md` and
> `docs/PRODUCT_DECISIONS.md` supersede every conflicting statement in this
> historical plan, including statements marked authoritative below.
> v5 round 2 (Codex review #6, 2026-06-23): stale §10 "later/DEFER" text stripped; **§0.6.12 node-action
> density / progressive-disclosure** rule added; host empty-state verb = **`Set up runtime…`** (gated
> wizard); remote `Apply changes` locked to the runtime node (Run bar = sim-only); §0.6.11 contract-
> maturity gate added; "tutorials" → "Examples"; **release line v1 = phases 0–10, v2 = 11–15** (§0.5.8).
> v5 round 3 (Codex review #7, 2026-06-23): first-run contradiction fixed (no project → `Create project`,
> never "Start simulating"); **remote-install artifact source DECIDED** = download matching `linux-<arch>`
> from GitHub release + checksum/signature (§0.6.2); v1 wording fixed (v1 = complete local + connect-
> existing, includes Compile + persistent-local backends); "Network Canvas HOST NODE" → "Devices &
> Connections host node"; "three **runtime/control** surfaces + Project/HMI launchers"; **omit (not grey)
> unshipped-backend actions**, hide empty inspector sections; **`Connect` also sets the Run target**;
> `Logs` only when a log backend exists; **local persistent runtime v1 = VS Code-managed background
> process** (OS service later); **remote Windows/macOS install out of scope** (Connect-only; §0.6.2 = Linux/SSH).
> Gitignored design doc — do not commit. Plan phase: agree direction BEFORE building.

## 0. The non-negotiable product goal (read first)

truST is a **complete PLC IDE**, not a run-shell. A first-time user — a controls engineer — must
understand all of this **without docs**:

1. How to **create a new project**.
2. How to **start from an example** (editable starter projects; guided onboarding = the walkthrough).
3. How to **run simulation**.
4. How to **connect or install a runtime** on this computer, a Raspberry Pi, an IPC, or Docker.
5. How to **select the active run target**.
6. How to **start / stop what truST owns** (the simulator / a local runtime).
7. How to **connect / debug / monitor a running remote runtime**.
8. How to **configure devices and communication**.
9. How to **check / build the program**.
10. How to **send / deploy it to the target**.
11. How to **see / write / force / release live values**.
12. How to **open / create HMI**.
13. How to **view logs / status / errors**.
14. How to **update / uninstall a runtime**.

Every one of these must be reachable from a **visible surface — not the command palette** — in an
intuitive place, with **no jargon** and **no fake state**.

**Run-bar honesty subset (still non-negotiable — the original v3/v4 rule):** at a glance the user must
see, in **one place**, (a) which runtime am I controlling, (b) is it stopped / running / connected,
(c) what one button starts / stops / connects it, (d) where to connect a remote runtime or device.

**Bottom line:** Select a run target once. Press one button. Manage a runtime's *whole life* —
add / install / start / stop / restart / deploy / update / logs — where you see it spatially, in
**Devices & Connections**. **No duplicate Start buttons. No "Network Canvas" jargon. No fake remote Stop.
No dead Send/Deploy button. The Run bar never starts or stops a runtime it does not own — it only
connects to / disconnects from the selected target.**

## 0.5 Surface model — AUTHORITATIVE (supersedes run-card/sidebar details in §6/§8; install/deploy/lifecycle in §0.6)

### 0.5.1 The one rule that prevents the recurring drift

There are exactly **three runtime/control surfaces**, with **non-overlapping** jobs (the sidebar also has
**Project** and **HMI** — those are *launchers*, not runtime-control surfaces, so they don't count here):

| Surface | Owns | Verbs |
|---|---|---|
| **Run bar** (top of the truST sidebar) | authoritative Compile, selecting a run target + truST's *own* connection to it | **Compile** · **Start/Stop** (simulator only — truST owns that process) · **Connect/Disconnect** (remote) · **Apply changes** (**sim only** — remote apply/restart/deploy lives on the runtime node, §0.6.6). |
| **Devices & Connections** (the graph; user-facing name for the canvas — internal name "Network Canvas" stays) | the runtime/device **inventory** + each runtime's whole **lifecycle** | per node, **disclosed by state** (one primary + ≤2 secondary + the rest in the inspector — 0.6.12): **Set up runtime…** (empty host) → **Set as run target** · **Start/Stop/Restart** · **Connect/Disconnect** · **Deploy** · **Update/Uninstall** · **Logs** · **Settings**. Full contract: **§0.6** (density rule: **§0.6.12**). |
| **Live Values** | live values **+ controlled write/force** | live state · I/O · memory/globals · **Write value · Force · Unforce/Release** (read by default; write/force are deliberate, gated, visually distinct). **No runtime lifecycle** (no Start/Stop/Connect). |

The error v3 still had (Claude's §6): the Run card showed **Start/Stop for a connected remote**. That
breaks the honesty rule. **Fix:** the Run bar's lifecycle verbs (Start/Stop) apply to the **simulator
only**. For a remote the Run bar only **Connect/Disconnect**s; the remote's process Start/Stop lives
**only** on its node in Devices & Connections.

A runtime is therefore **never Start-able from two places**: the simulator's only Start is the Run bar
(its node in the graph shows status + "Set as run target", no competing Start); a remote/local-
persistent runtime's only Start is its graph node (the Run bar only connects).

### 0.5.2 Three states (never conflate)

1. **Selected target** — the dropdown choice (UI state).
2. **Runtime process state** — is the program running on that box? Simulator: == truST's session.
   Remote: the remote's *own* reported state (owned by Devices & Connections; shown in the Run bar only as
   secondary text, e.g. `Running · not connected`).
3. **Attachment state** — is truST connected to observe/control it? Drives the Run bar's remote verb
   (Connect ↔ Disconnect). Never inferred from the remote's process health.

### 0.5.3 The Run bar (top section of the truST sidebar)

Wording decided by Johannes 2026-06-22: **plain PLC verbs** (Start/Stop/Connect/Disconnect); "debug/
monitor" stays in tooltips, never on the primary button.

```
RUN
 ✓ No errors                              ← passive validity from diagnostics (not a button)
 Run target: [ Simulator (this computer) ▾ ]   ← select-only; no Add/Connect entries
 Status: <state>
 [ <one primary action> ]
 (Apply changes)                          ← only: target = simulator AND source changed
```

Per-state (the only states the primary action recognizes):

- **Simulator stopped** → `Start` (compiles first, then runs). Status `Stopped`.
- **Simulator running** → `Stop`. Status `Running`. `Apply changes` shows if source changed.
- **Remote reachable, not connected** → `Connect`. Status `Running · not connected` (or the remote's
  real state · not connected).
- **Remote connected** → `Disconnect`. Status `… · connected`. Live Values populates.
- **Remote unreachable / stopped** → `Connect` **disabled** + line: *"Not reachable — open Devices &
  Connections to start or diagnose this runtime."*

Label: **`Run target:`** (not "Run on:"/"Runtime:"). Dropdown is **select-only** — lists only existing
inventory (simulator always; local-persistent if real; adopted remotes). **No "Add…/Connect…" entries**
(that revises v3 — drop the `Add runtime…` sentinel). Adding/connecting happens in Devices & Connections.

### 0.5.4 The sidebar (short, stable, nav — not a pile of buttons)

```
truST
┌────────────────────────┐
│ RUN                    │   ← the Run bar (0.5.3); Project welcome when no project open
│ Project                │   ← when empty: Create / Open / Start from example
│ Devices & Connections  │   ← opens the graph (inventory + per-runtime control)
│ Live Values            │   ← reveals live values (read + write/force)
│ HMI                    │   ← Open HMI, or Create HMI when none exists
└────────────────────────┘
```

The sidebar items are **launchers**; the rich surfaces (graph, Live Values, HMI, code) open in the editor/
panel area where they have room. "New project"/"Examples" are **actions under Project**, not permanent
items.

**Two explicit sidebar states (no ambiguity):**
- **No project open:** show **only the Project welcome** — `Create project` · `Open project` ·
  `Start from example`. No Run bar (nothing to run), no Devices/Live Values/HMI.
- **Project open:** **Run bar** (top), then **Project · Devices & Connections · Live Values · HMI**.

**Status bar (passive — already shipped, specify exactly):** a single left item, text only, click =
reveal/focus the Run bar (command `trust.home.focus`), **no Start/Stop**. Exact strings:
`truST: Simulator stopped` · `truST: Simulator running` · `truST: starting…` · `truST: <name> connected`.

**Devices & Connections is the Network Canvas** — implementers MUST build to its existing contract, not a
shallow launcher: per-node setup gear, Browse/Connect, discover, ADS tag browsing, `comm.apply`, offline
+ live topology, and per-node Start/Stop/Connect/Disconnect (+ `Set as run target`, 0.5.11). See
`docs/specs/25-vscode-product-contract.md` section 5 and the reviewed product
decisions in `docs/PRODUCT_DECISIONS.md`.

### 0.5.5 Live Values = read + controlled write/force (NOT read-only)

Name: **Live Values** (decided — "Monitor" implies read-only, but this surface also writes/forces;
"Live Values" is honest and beginner-clear). Today it's the editor-area
Runtime Pane (`trust-lsp.debug.openIoPanel`), and **it already writes and forces values** — verified:
`writeInput`/`forceInput`/`releaseInput` → `trust-lsp.debug.io.{write,force,release}`, with a `forced`
flag. **This capability MUST be preserved and improved — Live Values is NOT read-only. Do not remove
write/force/unforce.**

**Live Values owns:**
- live **runtime state**, **I/O values**, **memory/global values**
- **Write value** (where allowed) — set an input/variable
- **Force** (where allowed) — pin a value
- **Unforce / Release** (where allowed)
- **clear visual indication of forced values** (a value that is forced must always show that state)
- **audit/result feedback for every write/force** (success/failure surfaced, never silent)
- **safe failure** when the target is disconnected / debug disabled / not authorized / write
  unsupported — the control disables with a reason, never errors silently or fakes success

**Read is default; write/force is deliberate, gated, and visually distinct.** Force/unforce must never be
hidden or silently removed. All writes/forces keep today's safety/authz semantics (role + reachability +
capability gating).

**Simulator vs remote write/force — LOCKED behavior (verified against the backend, updated 2026-06-25):**
- **Simulator target:** full **read + write + force + unforce** (today's Runtime Pane capability —
  preserve it).
- **Remote / managed attached target:** **read + write + force + unforce for I/O values**, subject to
  runtime authorization and target capability. Attach-mode arbitrary variable writes remain disabled,
  but Live Values I/O uses the attach-safe debug-adapter custom requests `stIoWrite`, `stIoForce`, and
  `stIoRelease`, which forward to runtime-control `io.write`, `io.force`, and `io.unforce`.
  Never silently drop the tools, never fake a force; surface authorization/capability failures with a
  clear reason.

**Live Values ↔ debug boundary (explicit):** Live Values owns *values* (read + write/force/unforce). **Execution
control** (breakpoints, pause/continue, step) belongs to **VS Code's native debug** (the `trust-debug`
adapter, F5 / the debug toolbar) — not duplicated in Live Values. So: values → Live Values; stepping/breakpoints
→ the standard debug UI. (Open: if stepping/breakpoints aren't yet exposed, that's a debug-adapter scope
item, tracked separately — it does not change Live Values's value-centric ownership.)

**Placement LOCKED by the reviewed 2026-07-26 product decision:** a **Live Values**
sidebar launcher opens/focuses the editor-area view. When launched from Devices
& Connections it reuses the active editor group; other routes use column two.
The narrow sidebar does not own the value table. **Auto-reveal on
Start/Connect**, or a one-click prompt.

### 0.5.6 Decisions

- **Examples** (NOT "tutorials" — we ship editable starters, not stepped tutorials; *guided* onboarding is
  the VS Code **walkthrough**, phase 5) → editable **starter projects** (full contract in **0.5.12**):
  `Start from example` → QuickPick of bundled demos (tagged by hardware requirement) → copy an editable
  working copy to a user-chosen folder → open. Never docs links; the user never hand-edits `.toml` to start.
- **HMI** → **adaptive** (full contract in **0.5.13**): `Open HMI` when present, `Create HMI` when
  absent. Never a dead/disabled button.
- **Deploy** → the reviewed 2026-07-26 product decision supersedes the earlier
  `Send to PLC` label. The fixed sidebar action remains visible but disabled
  with a plain reason until a real backend exists; no palette deploy command is
  contributed before then. The eventual backend still checks/builds first,
  transfers only what changed, reports the applied mode, and confirms before
  changing a remote.
- **Apply changes** (= hot reload, `trust-lsp.debug.reload` exists) → **in the Run bar, simulator-only**;
  **remote apply/restart/deploy lives on the runtime node/inspector** (SPECIFIED in §0.6.6: live-apply if
  supported / restart-required / deploy-required → route to `Deploy` / hidden if unsupported) — never
  a non-working `Apply changes`, never fake success, never a remote `Apply changes` in the Run bar.
- **Compile** → **`Compile`** (not bare "Check"). The landed
  `trust-lsp.checkProgram` route owns authoritative project validation.
  Before a real result, diagnostics-derived state must not claim "build OK";
  successful and failed result wording is governed by
  `docs/specs/25-vscode-product-contract.md`.
- **Naming — DECIDED (Johannes 2026-06-23):** the graph area is **`Devices & Connections`** — it owns
  runtimes + field devices + comms links (ADS/OPC UA/MQTT/EtherCAT), i.e. software connections too, which
  "Hardware" alone undersold. The live-values area is **`Live Values`** (NOT "Monitor" — "Monitor"
  implies read-only, but the surface also writes/forces). All visible names are now decided.
- **Packaging — bundle `trust-runtime` in the VSIX (PREREQUISITE, now a phase — see 0.5.8 / §10).**
  Offline examples/config/device-setup shell `trust-runtime`, which the published VSIX does NOT ship
  today (only `trust-lsp` + `trust-debug`). Without bundling it, fresh-install examples + Devices &
  Connections setup silently degrade. This must land before/with the first-run + device-setup phases.
- **Palette cleanup** (the actual current leak — verified visible in `package.json`): hide from the
  command palette `Communication`, `Open Runtime Panel`, `Start Debugging`, `Attach Debugger`,
  `Hot Reload`, raw `hmi.init/refresh`. Keep registered as escape hatches; keep `debug.start` for F5;
  ADS six already hidden.

### 0.5.7 Cut from the current (v3) implementation

- The `Add runtime…` sentinel in the run-target dropdown (→ select-only).
- The `Connect runtime or device` and `Open HMI` links inside the Run card (→ they become the
  **Devices & Connections** and **HMI** sidebar areas; the Run card is run-only).
- The §6 claim that a connected remote shows Start/Stop in the Run card (the error this v4 fixes).
- Keep: passive status bar, the single Run card, canvas per-node controls (add `Set as run target`),
  idempotent stop.

### 0.5.8 Phases (risk-ordered, grouped into milestones; build is a separate worktree, not this review)

**The "complete PLC IDE" (the §0 goal) requires ALL of Milestones A–D (phases 0–15).** Milestone A alone
is a **usable shell**, NOT the complete IDE — do not market it as done.

**Release line — v1 = phases 0–10 / v2 = phases 11–15 (DECIDED 2026-06-23):**
- **v1 (phases 0–10) = the complete local + connect-existing workflow:** create → simulate → run a
  persistent runtime locally → connect a *running* remote → see / write / force values → HMI. **v1 is NOT
  "existing backends only"** — it includes the landed authoritative `Compile`
  route and persistent-local-runtime launcher; connect-existing reuses the
  runtime status/fleet topology backend.
- **v2 (phases 11–15) = install / deploy / operate *remote* targets:** SSH native install (11) · Docker
  (12) · `Deploy` (13) · update/uninstall/logs (14) · remote force/unforce (15).
- **Until a v2 backend lands, its UI is omitted by default** (preferred — see 0.6.12) **or, when the
  action should exist but is blocked, disabled-with-reason.** v1 does **not**
  get filled with greyed-out v2 buttons: the fixed Deploy slot is the reviewed
  exception and remains disabled with its reason. Install-over-SSH and Docker
  remain visible in runtime setup but disabled with concrete reasons, and
  remote force/release is capability- and authorization-gated rather than
  blanket-hidden. No dead button promises working v2 behavior.

**Milestone A — Shell (run / select / connect / see-values / HMI):**
0. **Packaging prerequisite** — bundle `trust-runtime` into each platform VSIX (§10). Unblocks offline
   examples/config/device setup **and every local-runtime/deploy flow**. Must land before phases 5, 9, 11–14.
1. **Palette cleanup** (removes the real leak; lowest risk).
2. **Run bar refinement** — `Run target:` label, drop the Add sentinel, passive validity line, remove the
   two in-card links, the full plain-verb state machine incl. the error/edge states in 0.5.10.
3. **Sidebar nav** — Project / Devices & Connections / Live Values / HMI launchers + `viewsWelcome`.
4. **Live Values** — launcher + auto-reveal; **preserve write/force/unforce + forced indicator + per-write
   audit + `Release all forces`** per the capability matrix (0.5.5 / 0.5.16).
5. **Project creation + starters** — `Create project` runnable-scaffold (0.5.15) + starter picker →
   editable copy → open (0.5.12); walkthrough.
6. **Apply changes (sim)** — wire `debug.reload`, sim-only, conditional on source-changed.
7. **`Set as run target`** on graph runtime nodes (shared selected-target source of truth, 0.5.11).

**Milestone B — Trust the build + a real local runtime:**
8. **`Compile`** — real authoritative project-compile contract (0.5.6 / 0.5.17) replacing the passive
   line. *(backend: runtime/LSP side)*
9. **Persistent local runtime** — launcher + service lifecycle around the bundled `trust-runtime`
   (start/stop/restart/status/logs), runtime node + `Run target` entry (0.6.1). *(backend: runtime side)*

**Milestone C — Real targets:**
10. **Connect existing remote** — add endpoint → validate via status/`fleet.topology` → token in
    SecretStorage → host + dropdown (0.6.4). *(mostly exists; lowest-risk real-target phase)*
11. **Remote native install over SSH** — `host.detect`, `runtime.install.native`, service lifecycle, clear
    per-failure errors (0.6.2). *(backend: runtime side; gated on §0.6.8 security)*
12. **Docker runtime** — detect Docker, run container, mount/map/token, container lifecycle (0.6.3).
    *(backend: runtime side)*

**Milestone D — Deploy + operate:**
13. **Deploy** — check→build→transfer-what-changed→report applied-live/restart/deploy-
    required→verify; confirm before remote; never a dead button (0.6.5). *(backend: runtime side)*
14. **Update / uninstall + remote logs** — version check, in-place update preserving config/tokens,
    rollback/recovery, uninstall, node logs (0.6.7 / 0.6.9). *(backend: runtime side)*
15. **Remote Live Values force/unforce** — complete: `stIoForce`/`stIoRelease` forward attach-mode I/O
    forcing through the runtime control endpoint, so remote/managed force/unforce cells (0.5.16) enable
    with the same authz (0.5.5). *(backend: runtime side)*

**Scope honesty:** Milestone **A (phases 0–7) is the usable shell** (run/select/connect/configure/see-
values/HMI) — **not the complete IDE.** The §0 complete-PLC-IDE goal is met only at **A+B+C+D (phases
0–15).** Each B–D phase carries a **new backend contract owned by the runtime/LSP side** (0.6.11); the UI
for each ships **absent/disabled-with-reason** until its contract lands — never a dead or lying control.
Don't market A as "done."

### 0.5.9 Open questions — only the truly blocking ones (rest are specified in 0.5.10–0.5.17)

These are **backend/visual** unknowns; the UI phases 1–7 ship without them.

1. **`Compile` contract** — does an authoritative whole-project compile exist or is it
   scheduled? (Blocks phase 8 only. Until then: passive `No known errors` line + `Start`-compiles-first.)
2. **`Apply changes` change-detection** — how is "source changed since loaded" determined for the sim?
   (Confirm it stays sim-only; remotes hidden.)
3. **Live Values placement** — editor tab vs bottom panel (visual pass; not code-blocking).

*(Resolved since last round — no longer open:)* remote/managed I/O write/force is **LOCKED** in 0.5.5
and 0.5.16 (read + write + force + unforce when the runtime authorizes it; memory/global writes remain
limited by attach-mode debug-adapter capability).

(Selected-target persistence, dropdown-switch semantics, examples manifest, HMI detection, and tests/
visual-editor placement are now **specified** below — not open.)

### 0.5.10 Run-card state machine (complete — incl. error/edge states)

The primary action + status line are a pure function of `(target kind, condition)`. Every non-happy
path is explicit: the button **disables with a reason**, never errors silently or fakes success.

| Target | Condition | Status line | Primary | Enabled | Hint / secondary |
|---|---|---|---|---|---|
| Simulator | stopped | `Stopped` | **Start** | yes | — |
| Simulator | starting | `Starting…` | Start | no | — |
| Simulator | running | `Running` | **Stop** | yes | `Apply changes` if source changed |
| Simulator | start/compile failed | `Failed to start` | **Start** | yes | reason + “Show output” |
| Remote | reachable, not connected | `<state> · not connected` | **Connect** | yes | — |
| Remote | connecting | `Connecting…` | Connect | no | — |
| Remote | connected, data flowing | `Connected` | **Disconnect** | yes | Live Values populates |
| Remote | connected, no values yet | `Connected · waiting for data` | **Disconnect** | yes | Live Values: “waiting…” |
| Remote | unreachable / process stopped | `Not reachable` | Connect | **no** | “Open Devices & Connections to start or diagnose.” |
| Remote | auth missing / invalid token | `Sign-in required` | Connect | **no** | “Set credentials in Devices & Connections.” (never collect a token in the Run card) |
| Remote | insufficient role | `Not permitted` | Connect | **no** | “Your role can’t connect to this runtime.” |
| Remote | reachable, no debug capability | `Debugging unavailable` | Connect | **no** | “This runtime doesn’t expose debugging.” |
| any | selected target removed/stale | (falls back to Simulator) | **Start** | yes | one-time notice: “‘`<name>`’ was removed — switched to Simulator.” |

**Dropdown switch while connected:** selecting a *different* target does **not** auto-disconnect the
current one silently. It changes the selection only; if the user then presses the new target’s action,
and another target is still connected, prompt/confirm before switching the active connection. (Never
drop a live connection as a side effect of a dropdown change.)

### 0.5.11 Selected-target source of truth + persistence

- **One store** (small service, or extend `runtimeLifecycleService`): the Run-card dropdown, the graph
  node’s **Set as run target**, **and `Connect`** all read/write the *same* value. No second copy.
- **`Connect` (on a node or the Run bar) ALSO sets that runtime as the active `Run target`** — connecting
  implies controlling it (subject to the §0.5.10 confirm-before-dropping-a-live-connection rule).
  **`Set as run target`** is the distinct "select without connecting" action.
- **Persistence:** `workspaceState` is primary. A workspace-keyed global-state
  value and extension-global-storage file are durable repair fallbacks across
  real VS Code restart.
- **Validation on read:** if the stored target is not in the current inventory (removed/stale), reset to
  **Simulator** and notify once. (This is the pattern the v3 Run card already uses — reuse it.)
- **Multi-root:** the selected target is **window-level**; the *program* that Start runs resolves from
  the active ST file’s workspace folder, falling back to the first folder. (A per-folder target is out of
  scope for v1 — note it, don’t build it.)

### 0.5.12 Examples / starter projects (contract)

- **Manifest** bundled in the VSIX (e.g. `media/examples/manifest.json`): array of
  `{ id, title, description, path (folder within the bundle), hardware: "none" | "twincat" | "raspberrypi" | …, tags[] }`.
- **`Start from example`** → searchable gallery with combinable hardware and
  category filters. Each card shows a hardware-requirement badge:
  `No hardware` · `Requires TwinCAT` · `Requires Raspberry Pi`.
- **On pick:** prompt for a destination folder → **copy the example into an editable working copy** there
  → open it (focus `main.st`). The user **never hand-edits `.toml`** to start; a `No hardware` starter is
  immediately runnable in the Simulator.
- **Bundling:** examples ship inside the VSIX (`.vscodeignore`/`files` must include them) — depends on
  phase 0 (and on `trust-runtime` being bundled so the copied project’s offline config works).
- **After creation:** project opens, first-run walkthrough available, Run card defaults to Simulator.
- **Curated first set (the starters to ship):**
  1. **Empty simulator** — `No hardware`; the `Create project` scaffold, runnable immediately.
  2. **Conveyor demo** — `No hardware`; a small realistic program for "see it run".
  3. **TwinCAT ADS** — `Requires TwinCAT`; Devices & Connections pre-wired for an ADS device.
  4. **Raspberry Pi (EtherCAT / GPIO)** — `Requires Raspberry Pi`; field-IO example.
  5. **HMI starter** — `No hardware`; ships an HMI descriptor so `Open HMI` works out of the box.
  6. **PLCopen Motion single axis** — `No hardware`; portable vendored motion-library starter.

### 0.5.13 HMI (detection + create)

- **Presence detection:** the project has an HMI **descriptor** (the file/folder the existing
  `hmi.openPreview`/`refreshFromDescriptor` consume). Descriptor present → HMI exists.
- **`Open HMI`** (present) → `trust-lsp.hmi.openPreview`.
- **`Create HMI`** (absent) → `trust-lsp.hmi.init` (scaffolds the descriptor + a starter HMI) → then open
  preview. Never a dead/disabled button.
- **Scope:** HMI is **preview** today (descriptor-driven); editing = editing the descriptor/source. A
  visual HMI *editor* is out of scope here (note, don’t build).

### 0.5.14 Tests & visual editors (placement)

- **Tests:** use VS Code’s **native Test Explorer** (the `stTests` integration already registers tests).
  Do **not** build a custom Tests sidebar view. Optionally a `Run tests` action under **Project**.
- **Visual editors (SFC / Statechart / Ladder / Blockly):** they are `customEditors` that open on file
  type — keep that. Discovery = a single **`New diagram…`** entry under **Project** offering the four
  types (the `new*`/`import*` commands already exist). Do **not** put four entries in the sidebar.

### 0.5.15 Create project (contract) — the biggest remaining gap

Today `newProject.ts` writes only `src/Main.st` + `trust-lsp.toml` — **not enough** for a first-timer to
simulate or configure devices without hand-editing TOML. `Create project` must scaffold a **runnable
simulator project**:

- **Writes:** `src/Main.st` (a minimal valid PROGRAM), `trust-lsp.toml`, **`runtime.toml`**, **`io.toml`**
  (the demo project proves these four are the runnable set; `program.stbc` is generated on first run,
  `hmi/` is created later by `Create HMI`).
- **Default run target = `Simulator (this computer)`** (persisted, 0.5.11).
- **Opens the folder** and **focuses `src/Main.st`**.
- **Run bar immediately shows `Start`** (Simulator, Stopped) — runnable with zero further setup.
- **Devices & Connections loads the offline topology immediately** (the bundled `trust-runtime` reads
  `io.toml`/`runtime.toml` offline — depends on phase 0).
- The user **never hand-edits `.toml`** to get a running, configurable project.
- (Same scaffolding underlies the `No hardware` starter in 0.5.12.)

### 0.5.16 Live Values — capability matrix (read/write/force/release by target × value-kind)

Capabilities differ by target **and** value kind; the UI must reflect the real backend, disabling with a
reason rather than failing. (Locks 0.5.5 per value kind.)

| Target | Value kind | Read | Write | Force | Unforce/Release |
|---|---|---|---|---|---|
| Simulator | I/O | ✅ | ✅ | ✅ | ✅ |
| Simulator | memory / globals | ✅ | per debug-adapter capability | per adapter | per adapter |
| Remote (attached) | I/O | ✅ | ✅ | ✅ (backend forwards `io.force` via attach — bbe4dacf2; runtime authorizes by role) | ✅ (`io.unforce`) |
| Remote (attached) | memory / globals | ✅ | ❌ disabled if attach-mode blocks variable writes | ❌ | ❌ |

- **`Release all forces`** — a REQUIRED safety action: whenever **any** value is forced, Live Values shows
  a prominent **Release all forces** control (one click clears every force on the target). Forced values
  always carry a visible indicator (0.5.5).
- Disabled cells show the reason on hover/inline; never silent, never faked. Remote/managed I/O
  force/release use `stIoForce`/`stIoRelease` and still disable with a reason when the runtime denies
  the operation or the target lacks the capability.

### 0.5.17 `Compile` — landed authoritative project validation

- **Command:** `trust-lsp.checkProgram`.
- **Placement:** fixed sidebar action plus command-palette escape hatch.
- **Result:** structured `{ ok, errors, warnings, issues, source_count }` data
  drives exact passed/failed summaries and the four-button state projection.
- **Truth boundary:** diagnostics-derived pre-result state never claims a
  successful build. The current normative wording and project-shell behavior
  are owned by `docs/specs/25-vscode-product-contract.md`.

## 0.6 Runtime installation, deployment & lifecycle — COMPLETE PLC IDE CONTRACT (AUTHORITATIVE)

This is the half v4 left vague. A complete PLC IDE must take a controls engineer from *nothing* to
*program running on real hardware* without docs and without hand-editing config. The flows below are
**specified now** (not deferred as a vague future item); each names its **backend dependency** and
**owning side** honestly. "Specified, build later" ≠ "fake it now": until a flow's backend exists, its
control is **absent or disabled-with-reason**, never a dead/lying button.

### 0.6.0 Where it lives + the honest-state rule
- **Surface** (NOT a visible "Home"): the **Devices & Connections** graph. You run a runtime *on a host*,
  so install/deploy/lifecycle live on the **host node** and the **runtime node** it owns — spatially
  correct, not a palette installer. The **Run bar** only *selects* a target and Start/Stops the sim or
  Connect/Disconnects a remote (0.6.10).
- **Empty-host entry — `Set up runtime…`:** a host with no runtime yet shows ONE primary action,
  **`Set up runtime…`**, which opens a small **wizard** (not "Install" up front). The wizard offers, in
  this order, **only the options the backend can actually do** (capability-gated, 0.6.12):
  1. **Connect existing runtime** (exists today — phase 10)
  2. **Run a persistent runtime on this computer** (bundled binary + launcher — phase 9)
  3. **Install truST runtime** (native, over SSH — phase 11)
  4. **Run in Docker** (phase 12)
  Options whose backend isn't built yet are **shown disabled-with-reason** (or omitted), never as dead
  entries — the wizard obeys the same honesty rule as everything else (no dead buttons *inside* the wizard).
- **Once a runtime exists the primary is state-specific** (NOT "Set up runtime…" forever): installed-but-
  stopped → **Start**; running, not connected → **Connect**; connected → **Open Live Values / Disconnect**
  (full table in 0.6.12). The word "Install" appears **only inside the wizard, when it genuinely installs.**
- **Honest state always:** every node shows only **detected** truth (reachable? OS/arch? runtime present?
  version? running? healthy?). Never a fabricated green, never an unbounded spinner, never "failed" with
  no cause + next action (0.6.9).

### 0.6.1 This computer
- **Start simulator** — the default; one click, no setup (trust-debug, shipped). Run bar `Start`.
- **Run a persistent runtime here** — a real **launcher + lifecycle** around the bundled `trust-runtime`
  (start / stop / restart / status / logs), distinct from the ephemeral debug simulator. **NOT just a
  label** — it is a workflow to build (phase 9).
  - **v1 implementation model (DECIDED): a VS Code-managed background process** — the extension owns the
    child process (cross-platform, no admin rights, dies with VS Code or on explicit Stop). A **true OS
    service** (systemd / launchd / Windows service, survives VS Code exit, auto-starts on boot) is a
    **later** enhancement, not v1. The node must say which it is (don't imply boot-persistence in v1).
  - Choose project/root; choose or auto-assign a **control endpoint** + **web port**; generate + store an
    **auth token** (SecretStorage, 0.6.8); show **logs**; **verify reachable** before claiming running.
  - On success: a **runtime node** appears in Devices & Connections and becomes **selectable in
    `Run target`** (0.6.10).
- **Backend:** binary is bundled (phase 0); the persistent launcher/service control is **new** (phase 9).

### 0.6.2 Raspberry Pi / IPC / remote Linux (native, over SSH)
- **Add host** by IP/hostname → **detect** reachability, OS, architecture, and **whether a runtime is
  already installed** (+ version). Show only detected facts.
- **Artifact source (DECIDED — the gap Codex flagged):** the VSIX bundles only the **host** platform's
  `trust-runtime` (§10), so the **target's** matching `linux-<arch>` artifact (e.g. `aarch64`/`armv7`/`x86_64`)
  is **downloaded on demand from the matching GitHub release**, then **checksum + signature verified
  before transfer** (the §10 "fetch-on-demand" mechanism + §0.6.8 verification). Rejected: bundling every
  arch in every VSIX (GB-scale, almost always unused); requiring a preinstalled runtime (defeats the
  install flow). The detected target arch (above) picks the artifact; an arch with no published artifact →
  the install option is **disabled-with-reason**, never a failed copy.
- **Install truST runtime** (host-node `Set up runtime… → Install truST runtime`): fetch+verify the
  matching-arch artifact (above), copy it over SSH, create/update `runtime.toml` / `io.toml` / project
  files, create + start a **service**, configure **control endpoint / web port / auth token**, then
  **verify reachable** before reporting success.
- **Lifecycle:** start / stop / restart the service; **logs**; settings.
- **Clear errors for every failure mode** (0.6.9): SSH auth/connection failure · no `sudo` · wrong
  architecture · port already in use · service failed to start · runtime unhealthy after install — each
  names the cause **and** the next action. Never a bare spinner or generic "failed."
- On success: runtime node added, selectable in `Run target`.
- **Backend (owning side, NEW):** `host.detect`, `runtime.install.native`, service lifecycle, `logs`.
  **Security:** §0.6.8 (no plaintext SSH passwords; ssh-agent / user terminal). Phase 11.

### 0.6.3 Docker
- **Detect Docker availability** on the chosen host (local or remote).
- **Run runtime container** (host-node `Set up runtime… → Run in Docker`): pull/run the runtime
  image, **mount project/config**, **map control + web ports**, generate + store an **auth token**.
- **Lifecycle:** start / stop / restart / **logs** / **remove** the container; **verify reachable**.
- On success: runtime node added, selectable in `Run target`.
- **Errors** (0.6.9): Docker not installed / not running · image pull failed · port conflict · container
  exited — each with cause + next action.
- **Backend (owning side, NEW):** `runtime.install.docker` + container lifecycle/logs. Phase 12.

### 0.6.4 Connect an existing runtime
- **Add endpoint manually** (a host/runtime already running elsewhere) → **validate** via runtime
  `status` / `fleet.topology`; show **reachable / unreachable / auth-required / insufficient-role**
  honestly.
- **Token** stored in **VS Code SecretStorage**. The reviewed 2026-07-26
  compatibility boundary permits only the explicitly labelled read-only
  `trust.runtime.authTokenFallback`; new or endpoint-specific tokens are never
  written to plaintext settings (0.6.8).
- On success: appears in Devices & Connections **and** the `Run target` dropdown.
- **Backend:** **mostly EXISTS today** (reuses `fleetEndpoints` + runtime status/`fleet.topology`; needs
  no local `trust-runtime`). Work = surface it on the host node + the SecretStorage token flow. Phase 10
  (lowest-risk real-target phase).

### 0.6.5 Deploy
- **User-facing name: `Deploy`**. The reviewed 2026-07-26 product decision
  supersedes the earlier `Send to PLC` wording.
- **Must check/build first** (0.5.17) — never send an unbuildable program.
- **Transfer only what's needed** — the program artifact (`program.stbc`) + changed `*.toml` config /
  runtime files; not a blind full copy.
- **Report the applied mode honestly:** **applied live** / **restart required** / **deploy required**
  (files changed beyond hot-reload) — surfaced to the user, not guessed (0.6.6).
- **Confirm before changing a remote target** (explicit per-target confirmation, 0.6.8).
- **Report success/failure clearly** (0.6.9).
- **Never imply backend success:** until deployment exists, the fixed action is
  visible but disabled with `Deploy is not available for this target yet.` No
  palette action is contributed. Once implemented, capability-dependent
  disabled/hidden states remain honest for the selected target.
- **Backend (owning side, NEW):** a deploy/transfer contract (build → transfer → apply/restart/redeploy →
  verify). Phase 13.

### 0.6.6 Apply changes (simulator + remote — no vague defer)
- **Placement (DECIDED):** the **Run bar's `Apply changes` is SIMULATOR-ONLY.** All **remote** apply /
  restart / deploy lives on the **runtime node / inspector** in Devices & Connections (the inspector's
  **Deploy** section, 0.6.12) — **never in the Run bar** (keeps it clean; matches 0.5.1 / 0.5.3). A
  connected remote shows **no** `Apply changes` in the Run bar.
- **Simulator:** hot reload (`trust-lsp.debug.reload`), shown only when source changed (0.5.6).
- **Remote (on the runtime node / inspector) — SPECIFIED, not "later":** behavior is a pure function of
  the connected runtime's capability —
  - **live-apply** if it supports online change → `Apply changes`;
  - **restart-required** if it can't hot-apply but the artifact is already deployed → `Restart required`;
  - **deploy-required** if files changed beyond what's on the target → route to **Deploy** (0.6.5);
  - **hidden** if the target exposes no apply path at all.
- **Never fake success**, never a non-working `Apply changes`. Today attach-mode blocks remote reload ⇒
  until the live-apply/deploy backend lands, a connected remote shows **restart-required / deploy-required
  routing**, not a dead `Apply changes`.

### 0.6.7 Update / uninstall a runtime
- **Check the installed version** vs the bundled/available version (the host/runtime node shows both).
- **Update** the runtime binary/container in place; **preserve project / config / tokens** across the
  update.
- **On failure: roll back or show a recovery path** — never leave the target half-updated/unhealthy with
  no next step.
- **Uninstall / remove** the service or container on explicit request (confirmed, 0.6.8).
- **Backend (owning side, NEW):** `runtime.update`, `runtime.uninstall`. Phase 14.

### 0.6.8 Security (gates every remote install/deploy/lifecycle flow)
- **Never store or collect plaintext SSH passwords.** Prefer the OS **ssh-agent** or a **user-driven
  terminal**; the extension must not handle SSH passwords itself.
- **Runtime auth tokens → VS Code SecretStorage.** The only plaintext-settings
  exception is the explicitly labelled read-only
  `trust.runtime.authTokenFallback` compatibility input; the extension never
  writes tokens there. A managed runtime's generated bootstrap token MAY be
  read from that runtime project's `runtime.toml` solely for immediate
  SecretStorage import before attach. Example fixtures and logs remain
  token-free.
- **Verify copied/downloaded artifacts by checksum + signature** before any install/update.
- **Explicit confirmation before any remote install / update / deploy / stop** (per host/target).
- **Never expose secrets** in logs, topology, screenshots, or tests — the visual gate + contract tests
  must assert no token/credential leak.

### 0.6.9 Logs, status & errors (no dead ends)
- **Every runtime node exposes `Logs`** (local persistent / remote service / Docker container).
- **Every host/runtime install/lifecycle failure shows a clear cause + the next action** (0.6.2 list).
- **The user is never left with a bare spinner or generic "failed."** Pending states are **bounded**
  ("Verifying reachable… (cancel)"); failures are **actionable**.

### 0.6.10 Relationship to the Run bar (the honesty rule, unchanged)
- **Run bar:** selects the **target** and **Start/Stops the simulator** or **Connect/Disconnects a
  remote** — nothing else. It **never** installs, deploys, restarts, updates, or Start/Stops a remote
  process.
- **Host / runtime nodes (Devices & Connections):** own **install / run-here / start / stop / restart /
  deploy / update / uninstall / logs / settings**.
- **Dropdown stays select-only** (0.5.3): it lists existing inventory; **no Add/Install/Connect entries**
  in the dropdown — those live on the graph. Runtimes created by 0.6.1–0.6.4 simply *appear* in it.

### 0.6.11 Backend contract index + what exists today (honesty ledger)

| Flow | UI phase | Backend status | Owning side |
|---|---|---|---|
| Simulate | shell | **exists** (trust-debug) | — |
| Bundle `trust-runtime` in VSIX | 0 | packaging change (release.yml) | release owner |
| Connect existing remote | 10 | **exists** (`fleetEndpoints` + status/`fleet.topology`) | UI surfaces it |
| Persistent local runtime | 9 | **NEW** launcher/service lifecycle | runtime side |
| Remote native install (SSH) | 11 | **NEW** `host.detect`, `runtime.install.native`, service, logs | runtime side |
| Docker runtime | 12 | **NEW** `runtime.install.docker` + container lifecycle | runtime side |
| Deploy | 13 | **NEW** deploy/transfer + apply-mode contract | runtime side |
| Update / uninstall / remote logs | 14 | **NEW** `runtime.update` / `uninstall` / `logs` | runtime side |
| Remote Live Values force/unforce | 15 | **exists** via attach-safe `stIoForce`/`stIoRelease` forwarding to runtime-control `io.force`/`io.unforce` (0.5.5) | runtime side |
| Real `Compile` | 8 | **NEW** authoritative project compile (0.5.17) | runtime/LSP side |

Until each **NEW** contract lands, its UI control is **absent or disabled-with-reason** (0.6.0). v1
real-target capability ships from the rows that already exist — **Simulate + Connect-existing**; the
install/deploy/update rows are **designed here** and built in their phases (0.5.8 Milestones C–D).

**Contract maturity gate (before any NEW backend phase starts).** A product-level name (e.g.
`runtime.install.native`) is **not** a buildable contract. Before its phase begins, the **owning side**
(runtime/LSP) must specify, per request:
- **request/response shape** (fields + types),
- **idempotency** (safe to retry / re-run? what does a repeated install/deploy do?),
- **required auth role** (who may call it),
- **error taxonomy** (the failure modes in 0.6.2 / 0.6.3 mapped to typed, surfaced errors),
- **audit + security behavior** (what is logged, what is confirmed, what is *never* logged — 0.6.8).

The UI phase for a flow **does not start** until this exists. This doc owns the **UX contract**; the
**wire contract** is the owning side's deliverable — naming a flow here is scope, not a green light.

### 0.6.12 Progressive disclosure / node action density (anti-clutter rule)

Devices & Connections owns many lifecycle actions, but a node must stay **readable**, not become a
toolbar of 12 buttons. This is the rule that keeps the architecturally-correct "nodes own everything"
from becoming the cluttered UI the whole overhaul exists to prevent.

**Rules:**
- A host/runtime node renders **one primary action**, chosen by **state**.
- A node renders **at most two secondary** *visible* actions.
- **All other actions live in the inspector / gear menu.**
- The inspector groups actions into sections: **Setup · Run · Deploy · Logs · Settings**.
- **Empty inspector sections are HIDDEN** — don't render `Deploy`, `Update`, or `Logs` if no backend /
  capability for them exists on this node (e.g. no `Logs` section when there's no log backend yet).
- **Two distinct "can't do it" cases (don't conflate — this prevents a v1 full of grey buttons):**
  - **State/auth-blocked** (the action *should* exist here but is blocked right now): **disabled with a
    short reason** (e.g. "Connect — sign-in required").
  - **Backend not shipped** (the feature isn't built yet — most v2 actions in a v1 build): **OMIT by
    default**, or tuck under an explicit **"More ways to run…"** preview affordance — **not** a greyed
    button on the node. (Applies to the `Set up runtime…` wizard options too, gated by 0.6.11 status.)
- A disabled action **always carries a short reason**.
- **No node card may become a toolbar of buttons.**

**Primary action by state (the only primaries a node shows):**

| Node state | Primary | Typical secondary (≤2) | In the inspector |
|---|---|---|---|
| Host, no runtime | **`Set up runtime…`** (wizard, 0.6.0) | — | host detect/settings |
| Runtime installed, stopped | **Start** | Set as run target | Logs · Settings · Update · Uninstall |
| Runtime running, not connected | **Connect** | Set as run target · Logs* | Deploy · Restart · Settings · Update · Uninstall |
| Runtime connected | **Open Live Values** | Disconnect | Deploy (apply / restart / transfer) · Logs* · Settings · Update · Uninstall |
| Runtime unhealthy / errored | **Logs*** (see the cause) | Restart | Settings · Update · Uninstall |

- **`Connect` also sets that runtime as the active `Run target`** (connecting means "I want to control
  this") — following the §0.5.10 rule (confirm before dropping another live connection). **`Set as run
  target`** is the *separate* "select without connecting" action; it never connects.
- **`Logs*` are shown only when a log backend exists for that node:** local persistent runtime = yes (v1);
  **remote logs = phase 14** — until then the `Logs` action/section is **omitted** (per the hidden-empty-
  section rule), not a dead button. For an *unhealthy* node with no log backend yet, the primary falls back
  to `Restart` / `Settings` + the detected error reason.
- **Update / Uninstall / advanced settings / logs live in the inspector** unless one of them *is* the
  current next action (e.g. unhealthy → `Logs` is primary, when a log backend exists).
- **Remote `Apply changes` / `Restart` / `Deploy` live in the inspector's `Deploy` section** (0.6.6),
  never on the Run bar.

## 0.7 Project kinds - Rust-first shell extension

This section binds the Rust PLC workflow to the same shell. It does not add a
new activity container, Run surface, or command-palette workflow.

Project-kind detection priority:

| Kind | Detection | Shell effect |
|---|---|---|
| `rust-plc` | project root contains `trust.toml` and `Cargo.toml` | same Home shell; Compile/Check dispatches `trust check --json`; Run uses Rust PLC sim/debug handshake; generated-ST and admission affordances appear |
| `st-modules` | `trust-lsp.toml` plus module declarations | today's ST shell plus Rust FB/module status where present |
| `st` | `trust-lsp.toml` | today's shell |
| `none` | no truST markers | welcome surface; Create project asks for Rust PLC vs Structured Text |

Rules:

- A Rust PLC project must never render as "No truST project" because it lacks
  `trust-lsp.toml`.
- `+ Create project` asks the kind first: Rust PLC or Structured Text.
- Rust PLC scaffolding shells the bundled `trust new rust-plc`; VS Code does
  not maintain a second TypeScript scaffold.
- The Run bar stays the single place to select Simulator/local/remote target.
- Compile/Check, Run, Update, Debug, Deploy, Live Values, HMI, Testing, and
  Devices & Connections remain the visible surfaces; command-palette commands
  are escape hatches only.
- Rust starter projects join the examples contract in 0.5.12 and carry the
  same evidence/screenshot expectations as ST starters.
- Rust PLC visible surfaces use the shared truST theme tokens and must have
  acceptance-board evidence before being called complete.

## 1. North Star

truST = the **easiest and best-looking PLC IDE** (beat TIA/CODESYS on discoverability + looks). Open a
project and, without docs: see the program → run/stop → see live state → wire devices → build HMI →
debug, all from visible surfaces.

Principles: (1) Discoverable (every core action on a visible surface, not just the palette);
(2) One front door per concern; (3) Honest run/stop + status (no fabricated green); (4) Project-centric;
(5) self-explanatory > powerful-but-hidden. **(6) NEW — never remove an escape hatch before its
replacement is self-contained and tested.** (Codex's main-risk correction.)

## 2. Current state (audit, corrected per Codex)

Core problem holds: **no product shell** — 0 viewsContainers/sidebar, 0 views, 0 walkthroughs, 0
keybindings, 0 `editor/title` items, 0 status-bar items. 33 commands, mostly "Structured Text:"
category, palette-only; 6 ADS commands hidden (`when:false`). 58 language-model tools (AI, not chrome).

Corrections to v1:
- Visual editors are **real `customEditors`** (`trust-lsp.statechartEditor`, `blockly.editor`,
  `ladder.editor`, `sfc.editor`) — they open on file type, not just palette. Keep; just need a
  discoverable "new diagram" entry.
- **Snippets** are contributed (`structured-text`).
- **The canvas IS now self-contained (UPDATED 2026-06-23):** the `communication.openPanel` deep-links
  were removed (P2 done) — `grep` shows **no** network-canvas source reference left. The Communication
  panel now survives ONLY as its own registration/import (`extension.ts` ~:13/:180) + the ADS bridge.
  ⇒ remaining work is **remove/hide that old command + registration + ADS bridge** (palette-cleanup
  phase 1), NOT "fix canvas bounces."

Everything else as audited: Devices & Connections graph (comms front door, self-contained), Communication
panel (legacy — registration-only, to be hidden/removed in phase 1), 6 hidden ADS commands, HMI
(preview/refresh/init), trust-twin 3D panel, Tests,
Project (newProject/PLCopen/moveNamespace), Runtime/Debug (Start/Attach/Hot Reload/Runtime Panel).

## 3. Problems

Invisible until Ctrl+Shift+P; no run/stop or live-status surface; comms fragmented 3 ways; no "home";
no onboarding; islands; verbose labels.

## 4. Target UX — SUPERSEDED by §0.5

> The original §4 (activity-bar "Home" + **status-bar runtime control** + **ST editor toolbar** +
> Project Health) is **withdrawn** — it described the exact multi-run-surface drift v4 removes. The
> target UX is now **§0.5**: short nav sidebar (Run · Project · Devices & Connections · Live Values · HMI),
> passive status bar, no editor-toolbar Run/Stop, no "Project Health", no visible "Home". Walkthrough →
> phase 5; visual editors → 0.5.14.

## 5. Remove / consolidate — ONLY after parity (revised)

Order matters: the canvas is already self-contained (P2 done — no `communication.openPanel` references).
- **Communication panel** → **now safe to hide/remove** (canvas parity reached): remove/hide its command
  + registration + ADS bridge (`extension.ts` ~:13/:180) in **palette-cleanup phase 1**. Keep shared
  `schemaForm.ts`/`runtimeComm.ts`.
- **6 ADS palette commands** → keep as **hidden internal escapes** (support/debug); all *user-facing*
  ADS actions become Canvas actions (diagnose/route/import). Respect roles: browse/import = Engineer,
  route-add = Admin.
- **trust-twin VS Code panel** → **retired** by the reviewed 2026-07-26 product
  decision. A future digital-twin product may return through a separately
  reviewed surface; this extension does not activate, contribute, or package
  the retired panel.

## 6. Run card — SUPERSEDED by §0.5.3 / §0.5.10

> The original §6 is **withdrawn** to remove two contradictions a builder could trip on:
> 1. it listed **`Add runtime… / Connect runtime…`** in the dropdown — **wrong**; the dropdown is
>    **select-only** (existing inventory), adding happens in Devices & Connections (§0.5.3).
> 2. it put **`Connect runtime or device`** + **`Open HMI`** links **inside the Run card** — **wrong**;
>    those are sidebar areas, the Run card is run-only (§0.5.4, §0.5.7).
> Authoritative Run card = **§0.5.3** (label `Run target:`, plain verbs, select-only dropdown) + the full
> state machine incl. error/auth/role/stale states in **§0.5.10**. The honesty rule (no remote Start/Stop
> in the Run card; remote process lifecycle only on the graph node) is in **§0.5.1**.

## 7. Open decisions — SUPERSEDED by §0.5

> Withdrawn — its #4 ("status bar + editor toolbar FIRST") and #5 ("activity-bar **home**") are the drift
> v4 removes. Still-valid points are folded into §0.5: Communication panel removed (kept hidden until
> canvas parity); ADS six hidden escapes; the VS Code trust-twin panel retired; visual editors via
> `New diagram…` under Project (0.5.14). Sequencing is now §0.5.8 (packaging → palette cleanup → run bar
> → sidebar → monitor → starters → apply-changes → set-as-target → check-program).

## 8. Phasing — SUPERSEDED by §0.5.8 (build status retained below)

> The v3 phasing block is **withdrawn** — its P1 Run-card mockup still showed `Runtime:` + the
> `Connect runtime or device` / `Open HMI` links inside the card (the builder trap §0.5 fixes).
> Authoritative phases = **§0.5.8**.

**Already built this session (factual status — kept so the builder doesn't redo it):**
- **Run card shipped** as a WebviewView (`trust.home`, internal id only — no visible "Home" word):
  select-only dropdown + one plain-verb action. ✅ (Needs v4 tweaks: drop the `Add runtime…` sentinel,
  remove the two in-card links, add the 0.5.10 error states, relabel to `Run target:`.)
- **Status bar is passive** (text only, click reveals the card; no Start/Stop command). ✅
- **ST editor-title Run/Stop removed**; `runtime.start/stop` commands removed. ✅
- **Canvas = comms front door**; no `communication.openPanel` deep-links. ✅
- **Canvas runtime-node controls** (Start/Stop/Connect/Disconnect + logs/settings, honest). ✅
  (Needs: `Set as run target`, shared selected-target store 0.5.11.)
- **`stopRuntime()` idempotent**. ✅

## 9. Hard UX tests (v5 — guardrails for §0.5 + §0.6)

Keep the v3 guards that still hold and ADD the v4 ones. Existing v3 contract tests pass; the starred
items are NEW work for v4.

- **One primary run surface.** Exactly one truST activity container + one view (internal id `trust.home`),
  a **WebviewView** (not a TreeView). ✅ (already enforced)
- **No visible "Home".** ★ No user-facing label reads "Home" anywhere (the internal id is fine).
- **`Run target:` label.** ★ The run card uses `Run target:` (not "Runtime:"/"Run on:").
- **Dropdown is select-only.** ★ The run-target dropdown contains only existing inventory — **no
  `Add runtime…`/`Connect…` entries** (the v3 sentinel is gone).
- **No in-card nav links.** ★ The run card contains **no** `Connect runtime or device` / `Open HMI`
  buttons (those are sidebar areas). Run card = run controls only.
- **Literal verbs per state** (0.5.10): sim stopped→`Start`/running→`Stop`; remote not-connected→
  `Connect`/connected→`Disconnect`. ✅
- **No remote Start/Stop in the Run card.** ★ The run card never renders `Start`/`Stop` for a remote —
  only `Connect`/`Disconnect`. Remote process Start/Stop exists **only** on the graph node.
- **Error/edge states disable with a reason** (0.5.10). ★ unreachable / auth-missing / insufficient-role
  / no-debug-capability / stale-target each disable the action with the specified hint — never a silent
  fail or fake success.
- **Live Values is NOT read-only.** ★ Live Values preserves **write / force / unforce** (`debug.io.write/force/
  release`), shows a **forced** indicator, surfaces **per-write audit feedback**, and **disables
  write/force with a reason** when disconnected/unauthorized/unsupported (never silent, never faked).
- **Status bar passive.** Contributes no Start/Stop command (click reveals the card). ✅
- **No ST editor-toolbar run controls.** `editor/title` has no `trust-lsp.runtime.*`. ✅
- **Canvas runtime-node actions** correct per state + honesty rule; ★ adds `Set as run target` writing
  the shared selected-target store (0.5.11).
- **`Stop` idempotent.** Already-stopped/disappeared session = success. ✅
- **Palette cleanup.** ★ `Communication`, `Open Runtime Panel`, `Start Debugging`, `Attach Debugger`,
  `Hot Reload`, raw `hmi.init/refresh` are hidden from the command palette (still registered as escape
  hatches). ADS six already hidden. No `communication.openPanel` deep-link in the canvas. ✅(canvas)
- **Comms route copy.** "Connect runtime or device" / "Devices & Connections" — never "Network Canvas". ✅
- **★ Visual acceptance gate for the Run bar.** Headless contract tests are not enough — the Run bar must
  *render* as one obvious control strip, not another button pile. Acceptance requires **screenshots at
  normal AND narrow sidebar widths** proving: the `Run target:` dropdown text fits (no truncation of
  `Simulator (this computer)` / hostnames), the primary button is visually obvious and singular (no
  second `Start` anywhere on screen), and the disabled-with-reason states are readable. Same gate for the
  Live Values force/forced-indicator legibility.

**Complete-IDE acceptance (Milestones A–D — the §0 goal):**
- **First-run, no project** shows **Create project · Open project · Start from example** on a visible
  surface (not the palette). ★
- **A new project starts the simulator with zero manual TOML** (0.5.15) — Create project → Start works. ★
- **A Devices & Connections host node can connect *or* install a runtime** via `Set up runtime…` → a
  **capability-gated wizard** (Connect / Run-local / Install / Docker; unbuilt options disabled-with-
  reason, never dead). ★
- **Node action density (0.6.12):** every host/runtime node shows **one primary + ≤2 secondary** visible
  actions, state-filtered; everything else lives in the inspector — **no node renders a toolbar of
  buttons**. ★
- **A v1 build shows no greyed-out v2 buttons:** unshipped-backend actions are **omitted** (or under a
  "More ways to run…" preview), NOT greyed; disabled-with-reason is reserved for state/auth blocks where
  the action should exist. Empty inspector sections (Deploy/Update/Logs) are hidden (0.6.12). ★
- **`Connect` also sets the active `Run target`** (node or Run bar), honoring the confirm-before-dropping-
  a-live-connection rule (0.5.10/0.5.11). ★
- **First run with no project shows `Create project` as primary** (Open / Start-from-example secondary) —
  **never "Start simulating" with nothing to run** (§0.5.4 / §10). ★
- **A runtime node exposes logs / settings / start / stop / deploy where appropriate** — and **only** where
  the backend can actually do it (honest per node type, 0.6.11); `Logs` only when a log backend exists. ★
- **The `Run target` dropdown is populated from the Devices & Connections inventory** (no separate list,
  no Add/Install/Connect entries in the dropdown). ★
- **Deploy stays disabled with a reason until it can do a real transfer** to the selected target (disabled for the
  Simulator; disabled-with-reason when no deploy backend). ★
- **Live Values keeps write / force / unforce / `Release all forces`** (0.5.16) — forced indicator +
  per-write audit feedback intact. ★
- **Remote install / deploy / lifecycle failures show a clear cause + next action** — no bare spinner, no
  generic "failed" (0.6.9). ★
- **No secret leaks:** no SSH password collected/stored; tokens only in SecretStorage; nothing secret in
  logs / topology / screenshots / tests (0.6.8). ★
- **Invariants hold everywhere:** no duplicate Start buttons · no visible "Home" · no "Network Canvas"
  wording in user-facing UI · no fabricated green/connected state. ★

Each phase: build → verify (headless contract tests + the above) → live Ext-Dev-Host pass with the visual
gate → iterate. Landed backend contracts (`trust-runtime` bundling, real `Compile`, persistent
local runtime, remote/managed I/O force/release) must light up their matching UI honestly. Remaining
backend contracts (`host.detect`/install native+Docker, Deploy, update/uninstall/logs) are
tracked with the owning side per 0.6.11; their matching UI ships absent/disabled-with-reason until each
lands.

## 10. Simulation vs Real Runtime — first-run model + host-node deploy (analysis behind §0.6)

> **v5 note:** this section is the **analysis / rationale** (packaging decision + security reasoning +
> first-run copy). The **authoritative** end-to-end install/deploy/lifecycle contract is now **§0.6**, and
> the build sequence is **§0.5.8 (Milestones A–D)**. Where §10 once said "DEFER," read §0.6: those flows
> are **specified and phased**, not vaguely postponed.

This is the single most confusing first-run question ("do I need to install a runtime just to try it?")
and it's central to "no docs needed." Reviewed proposal — ACCEPTED with corrections.

**KEY FACT (CORRECTED 2026-06-22 — Codex caught my error, verified in `.github/workflows/release.yml`):**
the *local dev tree* has all three in `editors/vscode/bin/`, but **the published VSIX ships only
`trust-lsp` + `trust-debug`** (release.yml:107/115). **`trust-runtime` is a SEPARATE download**
(`trust-runtime-<platform>.tar.gz`, release.yml:123/139), NOT in the VSIX. So for a fresh user *today*:
**simulation/debug works (trust-debug), but the canvas's offline `comm` config (schema/topology/apply/
discover/browse) and any persistent local runtime do NOT** — they shell `trust-runtime`, which isn't
installed (the canvas silently degrades to needing a *running* runtime).

**⇒ PREREQUISITE for the "no local install" model (PACKAGING CHANGE).** Until it lands, "simulate +
configure locally with zero install" is **not true for shipped users.** The resolver + tests already
expect a bundled `bin/trust-runtime` (binary.ts:37, stTests.ts:277), so the gap is packaging only.

**PACKAGING DECISION (resolved with Codex 2026-06-22):**
- **(a) PRIMARY — bundle `trust-runtime` into each platform VSIX** alongside trust-lsp + trust-debug
  (add `cp trust-runtime → editors/vscode/bin/` at release.yml:107/115; already built at :97/99).
  Gives the target UX directly: install extension → simulate → offline canvas config works → no manual
  runtime install.
- **(c) FALLBACK — fetch-on-demand**, ONLY if the VSIX size is judged unacceptable: an explicit
  "Install local runtime tools" first-use flow → download the matching platform artifact → verify
  checksum/signature → store under VS Code global storage → use automatically. Second-best (still an
  install step).
- **(b) REJECTED — do NOT put the `comm` CLI inside `trust-debug`** (Codex, agreed): it's the debug
  adapter; mixing offline config/topology/discovery in muddies responsibility AND wouldn't save size
  (it would duplicate the same runtime/config/ADS/discovery logic). If a smaller helper is ever wanted,
  make it an **explicit** binary (e.g. `trust-tools`), not hidden in the debug adapter.
- **DECIDED (user, 2026-06-22): (a) bundle.** Zero-friction first-run wins; a ~500MB platform-specific
  VSIX is acceptable for a pro PLC IDE, and fetch-on-demand's second install moment (right when the user
  expects the canvas to work) is worse than the size. (c) stays a later optimization only if Marketplace
  size becomes a real blocker. Action: add `cp trust-runtime → editors/vscode/bin/` at release.yml:107/115
  (packaging task; whoever owns release.yml).

**Correction to the reviewed proposal:** once trust-runtime is bundled, "This computer → Install native
runtime" is the wrong framing (nothing to install). But **"Run a persistent local runtime here" is NOT
just a label** (Codex): the binary being present ≠ a workflow. It needs a proper **launcher + service
lifecycle** around `trust-runtime` (start/stop/status/logs as a persistent process), distinct from the
ephemeral debug simulator that already exists. So it's a real workflow to build, not free.

**Model — three clearly separated intents (never say "runtime missing"):**
1. **Simulate now** — one click, no setup, runs in VS Code (trust-debug). The default.
2. **Run on a device** — deploy/connect a runtime on a target (Pi / IPC / this computer as a service /
   Docker). For real hardware + comms.
3. **Deploy / update target** — the operational flow (specified §0.6.5 / §0.6.7; phased in §0.5.8, **not**
   "later/vague").

**First-run copy (the no-docs crux) — two distinct states, do NOT conflate:**
- **No project open** (the true first run): primary = **`Create project`**; secondary = **`Start from
  example`** · **`Open project`** (per §0.5.4). **There is no "Start simulating" here — you can't simulate
  nothing.**
- **Project open, choosing where it runs** (this is where the simulate-vs-device copy belongs): the Run
  bar's default target is the **Simulator** — **"▶ Start"** ("Runs your program right here in VS Code. No
  install, no hardware."); the device path is in **Devices & Connections** ("When you're ready for a
  Raspberry Pi, IPC, or to run this as a service.", never an error/"missing" framing).

**Where it lives — the Devices & Connections host node** (internal impl name "Network Canvas"; not a
palette installer; spatially correct — you run a runtime *on* a host). Host actions via `Set up runtime…`
/ gear (density rule 0.6.12):
- **This computer:** Start local simulator (primary) · Run a persistent runtime here (bundled binary —
  not "install") · Connect to a runtime.
- **Raspberry Pi / IPC / remote Linux:** Connect (phase 10) · Install native over SSH (phase 11) ·
  update / uninstall / logs / stop service (phase 14). All via `Set up runtime…` (0.6.0).
- **Windows / macOS target:** **Connect to an existing runtime only** (phase 10). **Remote Windows/macOS
  *install* is OUT OF SCOPE** for v1/v2 — §0.6.2 covers remote **Linux over SSH** only; native remote
  install for other OSes is a later, separately-specified effort (different mechanism: not SSH+systemd).
- **Docker host:** Run in Docker (phase 12).

**Honest host inspector** — show only what we can actually detect AND what's relevant to the offered
action: OS/arch, reachable?, Docker available?, control port, web port, status verified by runtime
`status` / `fleet.topology`. Don't show SSH/Docker prereqs for actions not in v1.

**Build ordering (these are now fully SPECIFIED in §0.6 — phases, not a vague "defer"):**
- **Simulate** — exists (trust-debug, shipped). ✅ (shell)
- **Connect existing runtime** — point at a control endpoint; reuses `fleetEndpoints` + runtime status;
  needs NO local trust-runtime. ✅ exists → **phase 10** (surface on host node + SecretStorage token).
- **(prereq) Bundle trust-runtime in the VSIX** — unblocks offline config + every local/deploy flow →
  **phase 0**.
- **Run a persistent local runtime** — packaging fix + a NEW launcher/service lifecycle around
  trust-runtime → **phase 9** (not free; a real workflow).
- **Higher-risk but REQUIRED for "complete IDE" (designed in §0.6, built in their phases, gated on the
  security work below):** remote SSH native install → **phase 11**; Docker → **phase 12**; Deploy /
  deploy → **phase 13**; update / uninstall / logs → **phase 14**. These are **no longer "deferred as a
  vague future item"** — they are specified contracts with named backend owners (0.6.11).

**Security (dominant for the install/deploy flows — authoritative version is §0.6.8):** SSH creds — never collect/store plaintext; use
the OS ssh-agent / a user-driven terminal (the extension must not handle passwords). Remote install
scripts — signed + checksummed artifacts, explicit per-host confirmation. Docker socket — privileged,
confirm. Downloads — checksum + signature mandatory (avoided in v1: binaries are bundled). Control
tokens — VS Code SecretStorage, with only the reviewed read-only
`trust.runtime.authTokenFallback` compatibility input in plaintext settings.
⇒ v1 (local + connect) touches none of the install/deploy credential flows.

**Terminology:** "Install runtime" → "Run on a device" / "Deploy"; lead with WHERE it runs (This
computer / Raspberry Pi / IPC / Docker), native-vs-Docker is a secondary choice; keep "Host" in-canvas
but say "computer/device" in first-run copy; "Start local simulator" is good.

**Backend contracts:** the rows that ship from existing backends need **no new contracts** (connect =
existing fleetEndpoints + runtime status/fleet.topology; local run = bundled binary + lifecycle). The
install/deploy contracts (`host.detect`, `runtime.install.native`/`docker`, deploy, `update`,
`uninstall`, `logs`) are **specified in §0.6 and gated by the §0.6.11 maturity bar**, built in their
phases (11–14) by the owning side — **not** "design them later, not now."

**Where this lands in the plan:** the host-node model here is the analysis behind **§0.6** (now the
authoritative complete-IDE contract) and **§0.5.8 Milestones C–D**. The deploy/install flows are
**specified in §0.6**, built in **phases 9–14**, gated on the **§0.6.8** security work — they *complete*
the IDE; they are not an optional "someday."
