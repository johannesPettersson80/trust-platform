# truST Runtime Review — 2026-07-05

Branch `ads/client`, working tree as-is. Scope: `trust-runtime` + `trust-runtime-core`.
Method: 5 parallel deep-review agents (VM core, scheduler/lifecycle, IO/drivers,
control/web surfaces, value/memory/stdlib) + a Rust-support-readiness pass, with the
load-bearing claims re-verified by hand against the code.

---

## Overall: 3.0 / 5

The execution core and data model are genuinely strong — a defensively-written VM with a
real load-time validator and no reachable panics, and an unusually rigorous IEC semantic
layer (checked i128 arithmetic, round-half-even conversions, atomic checksummed retain with
typed migration). What drags it down is a consistent pattern at the **process edges**: the
scan-cycle thread has weak fault containment, and the "long-running unattended field device"
story is unfinished. None of it is architecturally broken — it's un-hardened, and most fixes
are cheap and localized. But today I would not run this 24/7 on a machine that can hurt
someone without the fixes below.

---

## The systemic theme: the scan-cycle thread is under-protected

Four independent reviews converged on the same soft spot. Each was verified against the code:

- **A panic in any cycle work silently kills the PLC.** No `catch_unwind` anywhere in
  production, and `Cargo.toml` doesn't set `panic = abort`. A panic in the VM, a driver, or
  stdlib unwinds the resource thread — but `ResourceState` stays `Running` (no drop-guard
  faults it), so control/HMI/monitoring all report a *healthy* resource with a dead thread
  (`scheduler/runner_api.rs:115`, `runner_loop.rs:60`).
- **No effective watchdog against a runaway program.** The only in-loop protection is the
  VM's 1M-backward-jump budget. `execution_deadline` (the every-32-instruction wall-clock
  check) has **zero production callers** — it's dead code — and the runner's wall-clock
  watchdog is evaluated *after* `execute_cycle()` returns (`runner_loop.rs:201` vs the cycle
  at `:166`), so it can't interrupt a stuck cycle. A deep call-fanout (no backward jumps)
  bypasses the budget entirely and hangs the scan thread unbounded.
- **No signal handling at all.** `rg` for `SIGTERM/ctrlc/signal_hook` is empty. `run_runtime`
  just blocks on `join()` and prints "Press Ctrl+C to stop" — Ctrl+C / systemd-stop kills the
  process mid-cycle: retain unsaved since the last interval, outputs frozen live, no safe
  state (`bin/.../run/runtime/entry.rs:556`).
- **One field-device hiccup halts the whole PLC.** Default `FaultPolicy::Halt`
  (`trust-runtime-core/watchdog.rs:218`); a single driver `Err` propagates via `?` out of
  `read_cycle_inputs` and faults the resource. Worse, the advertised `on_error =
  "warn"/"ignore"` knob is **dead** — both branches of Modbus/EtherCAT `handle_error` return
  `Err` (`io/modbus.rs:500`), while the status UI projects those policies as "degraded but
  running." Operators configuring `warn` get a silently-halted PLC.

For a PLC runtime, these four together are the headline: the cycle thread can be stopped,
hung, or silently killed, and in some cases the surfaces will still report green.

---

## 1. Stability

**Critical / major (verified by hand):**

- Panic containment, watchdog, signal handling, one-fault-halts-all — see above.
- **Stop applies no safe state and swallows the retain-save error** — `let _ =
  save_retain_store(); break;` with no `io.apply_safe_state()` (`runner_loop.rs:80`). Faults
  apply safe state; a *deliberate* stop doesn't — backwards from what you want.
- **Fault-policy `Restart` spins with no backoff.** Both restart paths `continue` past the
  `sleep_until`, so a persistent fault → warm-restart + full instance re-creation at 100% CPU
  forever, no counter, no escalation (`runner_loop.rs:184,195`).
- **Process image auto-grows unbounded on a bad address.** `%QD4294967295` (or a debug
  `io.write`) triggers a ~4.3 GB `Vec::resize` → allocation-failure *abort*, not a clean
  fault (`io/interface.rs:402`). No configured-size cap; over-read silently returns 0.
- **Blocking IO on the scan thread.** Modbus does one synchronous TCP round-trip *per point
  per cycle* with reconnect-every-cycle and no backoff; MQTT sleep-polls up to 100 ms on the
  cycle thread waiting for a "fresh" message then faults (`io/mqtt/driver.rs:126`). A 1 Hz
  sensor on a 100 Hz scan = guaranteed overrun.

**Agent-reported, high-confidence, not personally re-verified:** ADS notification leak on
symbol-version refresh; OPC-UA client tears down the whole session on one rejected node write
(infinite churn); EtherCAT leaks ~130 KB `PduStorage` per failed reconnect; corrupt/
unmigratable retain refuses to boot (no quarantine-and-cold-start); failed online-change
leaves half-applied task/image state.

**Genuinely solid (don't touch):** retain file-store atomicity is textbook (temp→write→flush
→fsync→rename→dir-fsync); control commands apply strictly at cycle boundaries — no mid-cycle
mutation races found; task-overrun policy is sane skip-with-counter; the RT posture module
(mlockall/affinity/verify-don't-set) is honestly designed.

---

## 2. Speed

Top wins, ranked:

1. **Cache per-callsite binding plans.** Every ST call is a string-symbol native call that
   re-derives positional/named bindings with multiple Vec allocations *per call* — builtin
   FBs (TON/CTU/R_TRIG, the hottest calls in real PLC code) clone a `Vec<Param>` with
   default-expr `Option<Expr>` every invocation (`vm/call/bindings.rs`, 904 lines largely
   because that resolution loop is written three times). This is the dominant architectural
   cost.
2. **Two always-on debug taxes on the production path.** `play` unconditionally enables
   debug, so dispatch does a full `storage.clone()` snapshot per located statement *and* the
   stack-VM fallback does a debug-map HashMap lookup per instruction *before* checking whether
   a debugger is attached (`dispatch.rs:277`). Mainstream code using `%X`/`%B` bit access
   lands on that fallback because those opcodes have no register-IR lowering.
3. **Per-cycle deep clones on the scan thread.** `write_outputs` clones the entire binding
   table every cycle (`io/interface.rs:197`); retain save deep-compares + fsyncs on the cycle
   thread *before* outputs are written (fatal on SD-card Pi targets — the fsync stall delays
   that same cycle's output commit); each OPC-UA/ADS server publish clones the full
   `VariableStorage`.

---

## 3. Bugs (concrete correctness)

- **TP and TOF report `ET = 0` in their hold state**, where IEC Figure 15 holds ET at PT
  until IN changes (`stdlib/fbs/timers.rs:151` and the TOF `else` branch). TON is correct.
  Programs ported from CODESYS/TwinCAT that gate on ET after Q will misbehave silently.
  *Verified against the code and the standard's diagram.*
- **Declared `STRING[n]` length is never enforced at runtime.** `Value::String` carries no
  capacity and no store path truncates — `s := CONCAT(s,'x')` every cycle grows forever until
  OOM on a 24/7 runtime. *Confirmed by negative grep — no truncation anywhere.*
- **NaN/Inf enter unchecked at IO/HMI** (`from_bits`, `"NaN"` parses) into an arithmetic core
  that faults-on-non-finite — so one glitchy analog input → resource faults *every cycle* with
  a misleading "Overflow" (`io/coercion.rs:91`).
- **Warm restart resets `current_time` to zero while the clock runs on** → thousands of
  phantom overrun events after every restart, and ~99 fake overruns at first boot
  (`runtime/restart.rs:142`). *Verified.*
- **Cold restart restores RETAIN from disk** → plain `RETAIN` survives a cold start; IEC
  cold-start semantics broken (`entry.rs:71`). *Agent-reported, high confidence.*
- **Frame-local sentinel refs can escape and alias another frame's locals** (`REF(localVar)`
  stored into a global), and multi-instance FB correctness rests on an unchecked single-owner
  heuristic with a silent template-instance fallback (`vm/dispatch_refs.rs`, `vm/mod.rs:436`).
  Both need a cooperating frontend to trigger — no current producer found — but the runtime
  has no guard. *Agent-reported.*
- **Modbus raw (no-points) register mode is byte-swapped** against real devices (BE wire vs
  LE image) — only truST↔truST is self-consistent; the mapped-point path is correct.
  *Agent-reported, high confidence.*

---

## 4. Refactoring

Real, but secondary to the stability work. Files over the repo's ~1k-line rule that are
actual mixed-concern god-objects:

- `openot_authoring.rs` (2940 lines, ≥7 responsibilities, and it runs on *every* server-side
  compile with reachable `panic!`/`expect` in the lowering path).
- `fleet_handlers.rs` (2699, but pure/cohesive — lower risk).
- `host/ads/diagnostics.rs` (1920, three clean split seams).
- `restart.rs` (921, mixes lifecycle with retain-migration codec that belongs beside
  `retain/`).
- `run_runtime` is a 615-line god-function; `scheduler.rs`/`run.rs` compose via `include!`
  instead of modules, defeating file ownership.
- **`world/**` (rapier3d twin sim, ~7k LOC) is linked into the production binary** but only
  used by tests and a hardcoded pose-lookup FB — belongs in a separate crate.

What's *not* a problem: the `trust-runtime` vs `trust-runtime-core` split is clean layering
(format/VM primitives in core, host encode/validate in runtime), not duplication — low drift
risk; `register_ir/` is live production tier-0, not dead.

---

## 5. Before you implement Rust support

The key realization: **the hardening the runtime needs anyway is exactly what Rust support
requires.** A misbehaving Rust POU with today's zero panic isolation would silently kill the
resource — the S1 "first light" slice cannot safely land on the current substrate. Do these
first, in order:

1. **Fault containment on the cycle thread** — `catch_unwind` around POU execution →
   `RuntimeError` → existing `FaultPolicy`, plus a drop-guard in the runner so *any* panic
   faults the resource visibly instead of leaving it "Running." This is the #1 blocker for
   Rust POUs and a bug-fix the runtime needs regardless.
2. **Arm the execution deadline** — wire `set_execution_deadline` per-cycle from watchdog
   config. The plumbing all exists; it's just not connected. Gives you a real time-based
   watchdog and bounds any external POU.
3. **Build the external-POU registry seam** — today stdlib FBs are a closed `BuiltinFbKind`
   match (`stdlib/fbs/mod.rs:37`); there is *nothing* for an external Rust POU to plug into.
   Replace it with a stateful trait + typed context (current_time, cycle_counter,
   instance-state handle) and migrate one builtin FB through it as proof. This *is* the S1
   door.
4. **Close the determinism leaks** for record/replay — the choke points exist (per-cycle
   `set_current_time`, IO snapshot, debug tap), but `stdlib/time.rs:401` reads
   `SystemTime::now()` directly and budget/watchdog traps read raw `Instant::now()`; write
   down the replay policy before the first external call.
5. **Signal handling + stop-with-safe-state** — table stakes before anyone deploys a
   Rust-authored program to a field device.
6. **Decide the register-IR vs stack-VM parity policy** for external calls, and freeze the SDK
   on the codegen digest + typed views, *not* on `Value` directly (which leaks host-only
   `Reference`/`Instance`/`Null` variants and has undocumented `Arc`-struct CoW).

---

## 6. Scores

| Subsystem | Score | One-line |
|---|---|---|
| VM execution core | 3.5 | No reachable panics, real validator, CoW values; runaway-protection hole + costly call bindings |
| Value / memory / stdlib | 3.5 | Rigorous IEC core; boundary validation gaps + TP/TOF + STRING[n] bugs |
| Scheduler / lifecycle | 2.5 | Disciplined in-cycle; unfinished lifecycle edges (signals, panic, restart storm) |
| IO / drivers | 2.5 | ADS side is a good reference; process-image drivers block the cycle, dead `on_error` |
| Control / web surfaces | 2.5 | Sound buffering/auth; Viewer arbitrary-file-read + single-threaded HOL blocking |
| Rust-support readiness | — | Substrate decent; external-POU door absent + panic isolation absent = real blockers |
| **Overall** | **3.0** | **Strong core, unfinished production edges; clear cheap path to 4+** |

---

## Security (dual-concern for an industrial runtime)

- **Viewer-role arbitrary host file read.** The IDE `normalize_project_root`
  (`web/ide/utils/paths.rs:75`) accepts any absolute path with no confinement, and
  `set_active_project` gates on `ensure_session` (any session) not `ensure_editor_session`. A
  low-trust Viewer repoints the workspace root to `/`, then `GET /api/ide/file?path=etc/passwd`
  passes the traversal check (which confines against the now-attacker-controlled root).
  Impact lands in the remote pairing/token deployments the product targets. *Chain verified
  end-to-end.* Fix is small: gate root-repoint on Editor + confine to a configured base.
- **Single-threaded web loop → head-of-line blocking.** All HTTP is one `tiny_http` dispatch
  thread (`web/server.rs:115`); heavy routes (256-host discover scan, deploy, io-proxy) run
  inline, so one slow request freezes the whole HTTP plane including HMI page loads. Plus an
  unauthenticated unbounded body read at `/api/pair/claim` (`web/ops_routes.rs:77`).

---

## If you fix only three things this week

1. **Panic containment + visible fault on the cycle thread.**
2. **Arm the execution deadline.**
3. **Make `on_error=warn/ignore` actually gate propagation** so one flaky device stops
   halting the PLC.

Those three retire the worst of the stability column and are prerequisites for Rust support
anyway.

---

## Confidence caveat

I personally re-verified the panic/watchdog/signal/IO-halt/timer/string/web-security findings
against the code. The ADS/OPC-UA/EtherCAT driver-lifecycle leaks and the VM ref-escape /
instance-owner hazards are agent-reported at high confidence but I didn't trace every one
end-to-end — validate those specific ones before scheduling fixes.
