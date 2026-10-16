# Runtime portability implementation checklist

Behavior authority: [specification 34](../../../specs/34-runtime-portability.md),
[runtime engine](../../../specs/11-runtime-engine.md), and
[STBC format](../../../specs/12-bytecode.md). This file records execution and evidence;
it does not create additional product requirements. Existing closed extraction boards stay closed.

## Current checkpoint

**15 October, evening: ESP32-C6 M2E bring-up passes on the owned board (owner: "do it").**
Branch `feat/runtime-portability-c6`: the C6 commits on H's head plus the supervisor test fix
(`62bda0a2c`), for one push to PR #137 together with H (owner decision).

- [x] M0 on the owned ESP32-C6-DevKitC-1 (8 MB flash): chip revision v0.2, eFuse block revision
  v0.3, 40 MHz crystal, chip ID 13, MAC 20:6e:f1:17:04:c0, secure boot and flash encryption off.
  Factory flash backed up (SHA-256 `b9e9e489…72d0d1`). Two probes measured the design inputs before
  any firmware was written: the USB-Serial/JTAG port survives MWDT0 system resets with no lost
  record; the watchdog reset reports 0x07; the RTC-fast cookie survives; PCCR counts 160 cycles
  per µs from boot; `misa` 0x40903105 (no F/D); GPIO9 idle high; GPIO18 pad readback; esp-storage
  flash reads; 1 kHz SYSTIMER interrupt.
- [x] Spec 34 D.2 "C6 reference composition (M2E)", §3.2 `trust-board-fixture`, and the post-M0U
  toolchain note in D.1.
- [x] Code, commits `64175c998..b2cea72c6`: the shared fixture crate with the F401 moved onto it
  (F401 code +256 B, 4,256 B free; sector 0 640 B free), the C6 adapter and composition, `xtask
  portability` board profiles with the C6 inspect and verify rules, unsafe and concurrency
  registrations, CI target checks for both adapters, the C6 lock graph in the supply-chain gate,
  and the execution diagram.
- [x] Focused checks 1–5 (`c6-focused-N`) and batches 1–4 (`c6-batch-N`) on the committed SHAs.
- [x] C6 guard 1 (`e29f01a29`) failed only the external AST unsafe gate: its patterns missed
  `pub unsafe fn` (now covered) and it rejects `unsafe extern` blocks (the C6 stack window is now a
  constant that inspection pins, as on F401), fixed in `b2cea72c6`.
- [x] C6 guard 2 (`881cd505e`): the full test run failed one test,
  `bound_socket_is_owner_only_from_creation`, with `Permission denied`. Round 8 narrowed the
  process-wide umask around `bind`, so a directory another thread created in that moment had no
  search permission (a production hazard for directories created at startup). `f2f6c237c` binds,
  restricts to 0600 and only then listens, never touching the umask; a regression test creates
  directories in a second thread during 400 binds.
- [x] H CI on `5c2963c00`: Linux and Windows passed; macOS failed
  `many_short_cycles_inside_their_timeout_are_never_declared_hung` (a 2 ms pace against a 4 ms
  watchdog leaves no room for OS jitter). `62bda0a2c` gives both supervisor race tests a 100 ms
  watchdog and keeps the accumulation check (40 cycles of 5 ms exceed it).
- [x] F401 board run 1 (`f401-board-1`) on the refactored firmware (`da307998…`, byte-identical in
  batches 1–4): verify passed, 101 rows, stack peak 11,672 B, slowest main scan 3,770 µs.
- [x] C6 board runs, all retained:
  - run 1 failed before any flash write: espflash 4.6.0 panicked on a custom data subtype; the
    partition table now uses custom types 0x40 and 0x41 (`1aa9ea76c`), pinned by an xtask test;
  - run 2: `B1,FAIL,main-cycle,runtime_execution_timeout` at the first task activation.
    Diagnostic builds (deadline raised, then instructions counted) showed 188,838 instructions in
    about 1.77 M cycles from flash; the scan path now executes from HP SRAM (`05672b0ef`, 304,796 B
    of RAM code, 52,580 B stack region), and inspection requires the dispatch loop there;
  - run 3 (`c6-board-3`, firmware `eb49c2a1…` at `05672b0ef`): verify passed, 101 rows, stack
    peak 10,856 B, slowest main scan 1,966 µs, numeric 2,291 µs (366,426 cycles), 33 banner lines,
    reset reasons 0x15 then 0x07, checkpoint partitions byte-identical;
  - run 4 (`c6-board-4`, firmware `7cea5f23…` at `8dad879e5`, whose firmware inputs equal the
    final head's): verify passed,
    101 rows, stack peak 10,856 B, slowest main scan 1,972 µs, numeric 2,277 µs (364,114 cycles),
    33 banner lines, checkpoint partitions byte-identical.
- [ ] Open: QIO flash mode did not boot with espflash's bundled bootloader (the board stays on
  DIO); that bootloader is ESP-IDF v6.1-beta1 and M5 needs a released one; supervision during a
  debug halt is not evaluated; optical LED and manual button transitions stay unverified; board
  selection and USB discovery in the hardware canvas (owner request) is a separate slice.

Evidence: `~/projects/.artifacts/runtime-portability-c6/` on the Pi and
`/mnt/HC_Volume_107089260/builder-storage/runtime-portability-c6/` on the builder.

**15 October, late morning: the remaining owner questions decided, then the full assurance
campaign (owner: "do what's most professional and best and supports the IEC standard if
applicable. Then move on to all the tests. We need a perfect runtime that's bug-free").**

- [x] The four questions left open by fix-all, decided on the merits (focused check 3,
  `fixall-focused-3`, 19 of 19 steps: workspace 9,706 passed, 0 failed, 30 ignored, with the 14
  new MQTT path tests, the event buffer and event store tests, the Windows-aware examples test,
  Clippy and format):
  - MQTT stale input: input-path conditions (stale or missing input, incoming overflow) stay in
    the reported health but leave the output path alone, and reconnect backoff follows link
    failures only. The old behaviour let stale input start backoff, which blocked output
    publication, so a stale-input fault could leave the safe-state outputs unpublished and the
    safe stop unconfirmed (spec 32 §8, spec 11 safe-state confirmation). The worker also no
    longer waits up to 100 ms for fresh input (that wait held the 40 ms output handoff), the
    reported health combines the input and output paths instead of flipping with the last
    operation, the output path splits a link condition (cleared when any operation sees the
    link up) from a publish condition (cleared only by a successful publish), and a disabled
    direction is always healthy.
  - Runtime events: the control server's event store keeps 1,000 events, the largest accepted
    `limit` (it kept 200); the debug control's own buffer keeps 1,024 and counts drops (spec 11).
  - `VariableStorage::create_instance` (panicking) is test support only; production
    construction uses `try_create_instance`.
  - Public-API baselines: not regenerable with current tooling. `cargo-public-api` 0.52.0 (the
    newest release) reads rustdoc JSON format 57 (rustdoc-types 0.57, nightlies of about
    2025-11 to 2026-03, rustc 1.95), while the workspace requires rustc 1.99, whose rustdoc
    emits format 61. A nightly install cannot close that gap; the reason-checked compile-fail
    tests guard the critical boundaries meanwhile.
- [ ] Found by native Windows CI on `a446a0c91` (Test windows-latest failed both attempts on
  `every_runnable_example_builds_and_validates`): the project template
  (`bundle_template::render_runtime_toml`) and all 35 runnable examples name `unix://` control
  endpoints, which Windows rejects ("unsupported endpoint"), so a generated project or a shipped
  example cannot validate or start on Windows. Spec 11 scopes the `unix://` default to Unix-like
  platforms. The examples test now gives its Windows copy a loopback TCP endpoint with a token
  (as the generated-project tests already did); the product fix (a Windows default of loopback
  TCP with a generated token, and the examples' Windows instructions) belongs to A19.
  Open, design agreed by the lead (15 October), implementation after round 7: spec 11 defines the
  default endpoint only for Unix-like platforms and is silent on Windows. Findings: the project
  template writes the same `unix:///tmp/trust-runtime.sock` for every generated project (two
  projects on one machine collide), ten shipped examples share that path, and eight OpenOT examples
  bind `tcp://127.0.0.1:0` with a committed placeholder token but nothing tells a client which port
  was chosen. Design: (1) when the runtime binds its control endpoint it writes the actual endpoint
  to `<project>/.trust-runtime/control.json` (owner-only permissions), and `ctl --project` uses it
  when present; (2) an omitted `[runtime.control] endpoint` defaults to a project-local socket on
  Unix (`<project>/.trust-runtime/control.sock`, falling back to a hashed path under the runtime
  directory when the path is too long) and to `tcp://127.0.0.1:0` on Windows, with a random
  per-start token written only to the owner-only discovery file; (3) the template and the examples
  omit `endpoint` and `auth_token`, so every platform uses its default and no token is committed.
  Spec 11 is updated first.
- [x] Guard 17 on `59b2f90ea` failed `remote_test_all` in
  `opcua_interop_reads_exposed_scalars_with_reference_client`: two OPC UA fixture servers in
  parallel were handed the same released port, one server thread panicked on bind, and
  `start_wire_server` still reported success because its readiness check reached the other
  server. Fixed at the root (`93e396558`): startup refuses a listen address in use and fails if
  its server thread has stopped; the test's port reservation never repeats a port; a new test
  pins the refusal. Clippy and five consecutive runs of the OPC UA suite pass.
- [x] Guard 18 on `fcf2499f2` passed; pushed `a446a0c91..fcf2499f2` to PR #137. Native CI is
  green: Test passes on Linux, macOS and Windows (the Windows examples run included), and the
  Release Gate Report passes; 23 checks succeeded and the 3 schedule-only jobs (capture refresh,
  Miri nightly, fuzz extended nightly) were skipped (`ci-fcf2499f2.log`).
- [ ] A13/A27 audit (`A13/AUDIT-REPORT.md`, read-only, expected values computed independently
  from IEC 61131-3 and IEC 60559): 251 boundary rows across 13 standard-function families; 152
  covered, 7 covered weakly, 70 missing, 22 needing a decision. Decided on the merits (specs 07,
  08, 10, 34 and DEV-023):
  - `MUL_TIME`/`DIV_TIME` are the named forms of `*`/`/` (IEC Table 35) and now compute as the
    operator does (exact integer factors, real results truncated toward zero); the helpers had
    rounded ties-to-even in binary64, which also lost nanoseconds on integer factors above 2^53.
  - `LIMIT` follows IEC Table 32 (`MIN(MAX(IN, MN), MX)`; inverted limits give `MX`).
  - `MOD` = `IN1 - (IN1/IN2)*IN2` (IEC Table 29); `MOD` by zero keeps faulting, recorded as
    DEV-023 because IEC defines it as `0`.
  - REAL and LREAL: one finite-result rule, a zero divisor (either sign, `0.0/0.0` too) is
    `DivisionByZero`, IEC 60559 signed zeros and subnormals are kept, `EXPT(0,0) = 1`, other
    domain errors are `Overflow`; `ULINT` arithmetic above `i64::MAX` is valid.
  - `CONCAT_TOD`/`DT` accepted out-of-range components (`CONCAT_TOD(0, 75, 0, 0)` gave 01:15);
    every component must now be in range. Numeric text follows the IEC single-underscore rule.
    Shifts of non-bit inputs are `TypeMismatch`; counter edge memory follows the input on reset
    calls; `TP` with `PT <= 0` keeps `Q` false; `ASSERT_NEAR` documents its real tolerance;
    `INSERT`'s example in spec 07 was wrong; `FIND` of an empty pattern is 1.
  - Implementation and tests in progress (agents N1 for the changes, N2 for the oracle tests).
- [ ] A03 finding: the FOR loop's zero-step guard (`HALT`, opcode `0x01`) had no register
  lowering, so every POU with a FOR loop fell back whole to the shared stack path (fallback
  reason `unsupported_opcode_0x01`). Register IR also leaves 23 more opcodes to the stack path
  (shifts and rotates, `EXPT`, partial access, method and virtual calls, debug marks). HOST-04
  now requires the HALT lowering (in progress); the other opcodes are optimization coverage,
  measured by A03 once it runs.
- [ ] A03 (`property_executor_differential.rs`, proptest 1.12 with a fixed seed): generated
  well-typed programs (BOOL/INT/DINT/LINT/REAL/LREAL, functions, IF/CASE/FOR/WHILE, arrays,
  division and overflow on purpose) run on the stack, register and Tier-1 arms with per-cycle
  comparison of results, faults, outputs and variables. 48 cases pass in CI; a 3,000-case campaign
  passed (`A03-campaign-1`). The first run found only a test defect (NaN/infinity written to a
  REAL input, which the runtime correctly refuses with `NonFiniteValue`).
- [ ] A04 (`property_metamorphic.rs`): positional arguments, POU order, artifact round trip and
  repeated preparation preserve per-cycle behaviour; 48 cases each pass.
- [ ] A10/A15/A11 audit (`A10/AUDIT-REPORT.md`): 72 requirements, 48 asserted exactly, 10
  weakly, 14 missing. Decisions: generation tokens (MEM-07, UPDATE-04), RETAIN-01 identity and
  sequence, the installation journal and the bounded persistence worker stay in C, MCU or M3 as
  spec 34 §4.3.1 and §10 assign them. Non-returning callbacks (TIME-03): first taken to be covered
  by the runtime watchdog, which the A10 tests disproved (see below); independent hosted hang
  supervision is a separate H slice. The missing H tests and the RT_* trace-runner fix (the
  runners ignored the cases' declared `expect.*` and `no_partial_apply`) are below.
- [ ] A06/A07: a pinned nightly (`nightly-2026-10-14`, rustc 1.101) is installed on the builder
  for cargo-fuzz and Miri. Two cargo-fuzz targets are added: `bytecode_admission` (CRC repaired,
  full preparation under small explicit limits, register and stack executors agree for three
  cycles) and `bytecode_reprepare` (repeated preparation agrees; decode→encode fixed point). They
  and the A03/A04/A08 property tests are registered in the fuzz-program registry
  (`verification/fuzz-program.toml` and its closed contract); its campaign record still needs a
  run at the committed source. The `ci.yml` execution-source digests had drifted on this branch
  (19f3b6787, 0f066c9fb, 887b84fd2 only added commands) and are re-pinned.
- [ ] A08 (`stateful_lifecycle_sequences.rs`, model in `support/lifecycle_model.rs`): 1–24-step
  sequences of scan, checked and queued writes, forces, pause/resume, warm/cold restart, retain
  save and online change (accepted, malformed, incompatible) against an explicit spec model,
  comparing every observable after each step. Findings:
  - Decided: the hosted `Runtime::clear_fault` is the tooling reset of the fault holder (spec 11
    "Fault-state clear lifecycle", sentence added); no product path calls it, and product
    recovery always restarts (STATE-06).
  - Defect: `Runtime::clear_debug_control` (no caller since the initial release) dropped active
    forces, against its own doc and spec 11 §6.9; removed.
  - Defect (found reviewing A08): control `pause`/`resume` replied `paused`/`running` on a faulted
    or stopped resource whose scheduler thread had exited; spec 11 §6.9 now requires a stable
    error naming the state, and recovery only through `restart`.
  - Spec gaps recorded for a spec pass: values after a scan that faults after the scan-boundary
    writes; two queued writes to one target; writes and forces while faulted; operations while
    the debugger holds the resource; direct-address initializers at restart.
- [ ] A10/A15/A11 tests: retain checkpoint truncation at every length and corrupt headers, the
  deadline swept over every poll point, failed periodic save, candidate refusal on a live
  runtime, allocation refusal in retained apply and graph transfer, store faults in warm restart,
  read-side input failure, failed warm restart keeping live state; the RT_* trace runners now
  enforce every declared `expect` key and reject unknown ones. Findings:
  - TIME-03 is not met on hosted builds: the watchdog checks elapsed time only after the cycle
    returns, so a driver read that never returns stalls the resource with no fault (spec 34 R33
    already names the same-thread watchdog). Fixed in round 6 (below).
- [ ] Round 6 — faulted resource lifecycle and hosted hang supervision (spec 11 §6.6, both blocks
  written 15 October). Found while reviewing A08/R4: after an ordinary halting fault the runner
  loop ended its thread, the binary then shut the servers down and exited with status 0, and a
  control `restart` was acknowledged ("restart queued") but never applied, so a faulted PLC could
  not be recovered without restarting the process, and every resource command (debugger and web
  snapshots, status reports, online change) failed after a fault. Now the faulted resource waits in
  `scheduler/faulted.rs` (no tasks or outputs; serves reads and configuration; `restart` or online
  change resumes after the configured initialization; `stop` ends it), and the process exits with
  status 1 when it ends faulted. A per-resource supervisor (`scheduler/supervisor.rs`) reads the
  published cycle deadline and faults a scan that does not return (TIME-03);
  `runtime.watchdog.hang_action = "abort"` exits with status 70. The hung-driver test is un-ignored.
- [x] Rounds 4–6 validated at `cf5ce62e5` (15 October). Focused check 4: 19 of 19 steps pass;
  workspace 10,008 passed, 0 failed, 30 ignored; core all-features, portable and register lanes,
  doc tests (with checked error codes), runtime-only lanes, the real-binary journeys in three
  repetitions, i686, full composition, Clippy and the fuzz build pass. Diagram render and drift
  check pass (`runtime-execution.svg` regenerated). Batch 9 (`H-cost-model-9`): canonical rule files,
  F401 firmware link, inspection and package pass; PERF-01 passes (call_binding median ratio
  1.040–1.049, pooled p99 1.119; refs_sizeof 0.847–0.866; string_stdlib 0.967–0.984). F401 replay
  (`H-cost-model-board-9`): verifier passed, 101 trace samples, MSP peak 11,616 B (unchanged),
  maximum main scan 3,761 µs, 4,480 B firmware code free (256 B less than batch 8: stdlib growth).
- [ ] Round 7 (A16 concurrency, A18 allocation and stack, A17 audit) and round 8 (A27 function-block
  oracles, A28 bounded exhaustive checks, security fixes), to be validated together with one final
  batch, guard and push (owner, 15 October: "fix everything and then push"). Findings fixed:
  - A16: queued restarts coalesced and a later warm request replaced a pending cold one (cold now
    takes precedence); a reload that timed out was still applied later (now withdrawn unless the
    resource already took it); the resource command queue was unbounded (now 1,024, `resource_busy`);
    `config.set` sent three separate scheduler commands and could leave a partial update (now one
    command, and every fallible lock precedes the first commit). One A16 test used overlapping
    memory words (`%MWn` is a byte offset, spec 10) and was corrected.
  - A18: construction, steady scans (no growth over 1,000 cycles), 100 restarts and online changes,
    drop, faults and refusals do not leak; maximum call and recursion depth run on a 512 KiB stack.
  - A17: no first-party unsafe in core or runtime sources; three test allocators were missing from the
    unsafe register (added). Miri over core (strict provenance, symbolic alignment) is running; tests
    that reach libm's x86 `sqrtsd` inline assembly cannot run under Miri and are recorded as unrun.
  - A27: TP reset ET on the expiry call when IN was low, against spec 08 §5 (fixed: ET = PT on that call,
    zero on the next low call); SR/RS long input names (IEC Table 43 rows 1b/2b) compiled but faulted at
    run time (now normalized to S1/R and S/R1 at compile time; spec 08 updated).
  - A28: 27 exhaustive tests over task readiness, process-image windows, SIZEOF arithmetic, identity
    allocation, engine identity containers and the reload claim; HOST-02 now states the allocator hands
    out identities 0 through u32::MAX - 1.
  - Security (A19 audit): the Unix control socket was bound with default permissions and narrowed
    afterwards (a local user could connect in that window and get the token-less Unix Admin role; now
    owner-only from creation, and a non-socket file at the path is refused); `trust-dev ctl` and
    `trust-runtime ui` dropped the project token with `--endpoint`; trust-lsp sent the editor's token to
    an endpoint chosen by repository configuration. The rest of A19 (defaults, discovery, examples
    without committed tokens, extension) is designed (`A19/DECISIONS.md`) for its own slice.
  - CI: the Linux test runner ran out of disk building the larger test set; CI now builds with
    `CARGO_INCREMENTAL=0` and line-table debug info (backtraces keep file:line). The Windows run showed
    the two new fault-and-restart journeys failing in the test harness only: it matched an LF pattern
    in a CRLF checkout of the example source (the harness now normalizes line endings).
  - A06/A07 fuzz campaign 1: 34 seeds (24 current-format artifacts built from the examples), 30 minutes
    per target, debug assertions on: container 97.0 M executions, admission 7.39 M (coverage 16,525
    through validation, preparation and both executors), re-preparation 4.79 M; no crash, timeout or
    out-of-memory input. The fuzz-program registry's campaign record (all 19 targets through
    `scripts/run_fuzz_campaign.py` at the committed source) remains to be recorded.
  - A17 Miri campaign 1 over trust-runtime-core: no undefined behaviour; groups reaching libm's x86_64
    inline assembly are rerun on s390x.
- [ ] Round 4 focused checks: check 1 stopped on its free-space guard (the assurance cargo target
  had grown to 230 GB; removed). Check 2: 10 workspace tests failed from two causes (the new
  initializer range check passed a subrange type to a base-type test; an A08 atomicity check
  treated a faulting scan as a refusal) and Clippy found two `explicit_auto_deref` items. Check 3:
  everything else green; the remaining failures were a real gap (subrange bounds beyond the DINT
  range were never declarable in the harness, because bounds were lowered without the base type), a
  250 ms wait in an OpenOT persistence test on a loaded builder, and a ctl-journey race: the test
  harness released reserved ports before the runtimes bound them and shared one token, so one
  journey's client could reach another journey's runtime (fixed: per-copy tokens, ports never handed
  out twice).
  - Spec 11 §6.9.2 said every restart clears debug mutations, while spec 10 §4.8 keeps them after a
    failed preparation; the clearing boundary is the commit (wording fixed).
  - `RT_RELOAD_001` releases its force before any scan, so it never showed a force surviving a
    failed reload; `failure_injection_warm_restart_state.rs` now does.
- [ ] A12 (`temporal_layout_*.rs`, 15 tests) and A14 (`scheduling_*.rs`, 19 tests). Defects:
  - Timer elapsed spans wider than the TIME range panicked in debug builds and wrapped in release
    (spec 08 §5: saturates at PT).
  - LTIME, LDATE, LTOD and LDT were refused at `%IL`/`%QL` addresses although spec 10 maps 64-bit
    families to long words; they now map to LWORD with their exact 64-bit value (IEC 61131-3 Table
    10); DATE, TOD and DT stay refused (implementation-defined size), now stated in spec 10.
  - `Duration::from_millis/micros/secs` multiplied unchecked, so very large configuration,
    control, simulation or conformance values wrapped to negative durations in release builds;
    untrusted inputs now use checked constructors and are refused.
  - Not defects: reference SIZEOF follows the executing target (spec 29 §1); TIME images count
    whole milliseconds (spec 07 §2.8); OPC UA exposes no temporal types. SCHED-04 timing admission
    has no H seam (spec 34 defers it to M3/M4).
- [ ] Assurance campaign order, by expected defect yield; each slice is audit, tests, one
  batch, fixes, then commit, guard, push and CI:
  1. A05 mutation testing on the core VM, register IR, Tier-1, decoder/validator and the
     engine's admission and write paths (the shard map first moves to the H file layout).
  2. A24 line/branch coverage of the core and runtime suites; uncovered failure and cleanup
     paths in changed code get tests.
  3. A03, A04 and A08: property, metamorphic and stateful-sequence tests with fixed seeds and
     shrinking; executors cross-checked against each other and against independent properties.
  4. A06 and A07: container and structured validator/admission fuzzing on a pinned nightly.
  5. A13 and A27: numeric and standard-library corpus against independent IEC 61131-3 and
     IEEE 754 oracles.
  6. A12, A14, A15, A10 and A11: temporal ABI, scheduling and clocks, retain and restart,
     failure injection, ownership and aliasing stress.
  7. A16 and A17: concurrency (deterministic models of the real synchronization units), Miri
     and sanitizers.
  8. A18, A19, A26, A20, A21, A22, A23 and A28: allocation and stack, composition matrix,
     source-to-artifact, remaining service surfaces, performance, hardware, soak and formal
     feasibility.

**15 October, morning: the open owner decisions, implemented ("fix all"; Claude lead).** The
owner approved all 20 recommendations listed overnight. Commits `87055c389..34badc0f1` on the H
branch, specifications first. Evidence: external `runtime-portability-h/fix-all/`, builder
`runtime-portability-h/fixall-targeted-1/`, `fixall-focused-2/`, `H-cost-model-8/`.

- [x] Specifications (`87055c389`). Spec 11: `validate` selects the configured resource as
  launch does; `ctl events` and `ctl faults`; `set` and `var.force` give an untyped integer the
  target's declared type; pending writes, debugger expression writes and forces are bounded at
  1,024 with `request queue full`; safe-state confirmation uses output-path health. Spec 34:
  instance identities never wrap or panic (HOST-02); a debugger attached mid-run takes effect at
  the next activation start, Tier-1 hotness survives eviction, interpreted block entries are
  counted and Tier-1 has no deoptimization counters (HOST-04); §4.3.1 queue bound, the core
  owners as the embedding API, private debugger storage; §11.1 MQTT overflow. Spec 12: the
  container bound precedes every decode charge; `SIZEOF` evaluation depth 128.
- [x] Examples and `validate` (`d958b7cbd`). 83 examples now declare the resource their
  `runtime.toml` names: 75 OSCAT test projects, seven communication examples and
  `network_canvas_demo`; the ADS server example is `truST_ADS`, EtherCAT `EtherCAT_demo`. Five
  PLCopen motion examples got a simulated `io.toml`. `validate` calls launch's own rule
  (`harness::configured_resource`, shared with `select_resource`).
  - The first version of the all-examples test ran 95 minutes and failed 76 of 111 projects
    (`fixall-targeted-1`): every OSCAT project declares `TEST_PROGRAM`s that no configuration
    binds, which ordinary runtime construction rejects (spec 22), so they are ST test projects
    run with `trust-runtime test`, not `play`; `openot_multi_program` depends on an OpenOT
    checkout outside the repository (its `Main.st` is built by `openot_telemetry.rs` with the
    vendored library). The final test builds and validates the 35 runnable examples on four
    workers, parses every example for a declared resource, and checks that only OSCAT examples
    declare test programs.
- [x] Control surface (`3b5a9ab1e`): the literal rule for `set`/`var.force`
  (`vm/engine/literal_admission.rs`, shared with I/O admission); rejected forces leave the force
  set after one `write_rejected`; the five request queues bounded at 1,024 with refusals from
  the control handlers, the HMI and ADS write ports (ADS `NoMemory`) and the debugger (queue
  first, then write; a failed write restores the previous force; `setExpression` writes check
  the bound and update the paused snapshot under one lock); `ctl events`/`ctl faults`.
- [x] MQTT overflow count and degraded detail, `IoDriver::output_health` for safe-state
  confirmation, hosted instance creation returning `Overflow` (`71f9ea8e8`).
- [x] Container bound before the decode charge and "REF_TABLE entries" (`4e4f45bfe`); Tier-1
  deopt counters removed, interpreted block entries counted, HOST-04 tests (`6c408df8f`).
- [x] API boundaries (`887b84fd2`): compile-fail doc tests with exact error codes, each with a
  compiling control; CI runs doc tests with `RUSTC_BOOTSTRAP=1`, which makes stable rustdoc
  check the codes, and builds the runtime-only test set; `DebugRuntimeContext` storage private.
  Two recommendations changed during implementation:
  - `ReferenceContext` stays unsealed: `trust-runtime` implements it for the legacy `Runtime`,
    in another crate. The compile-fail cases import the trait and show that no owner
    implements it.
  - The core owners' input image, time and fault setters and `reset_task_timing` stay public as
    the documented embedding API (spec 34 §4.3.1) instead of a cycle-phase gate: they take
    `&mut self` between cycles, so no cycle is running, and the installed controller calls
    `reset_task_timing` from another crate.
- [x] CI collector waits for registered checks (`80f02cdc3`); changelog (`34badc0f1`); bisect
  worktrees and the `trust-portability-h-bisect-v0/v1` targets removed (binaries and logs kept in
  `H-perf-diag-6/bisect`).
- [x] Targeted run on the pre-review source (`fixall-targeted-1`): runtime library (MQTT, I/O
  subsystem, instances, control), decode bounds, call-path, register parity, debug control,
  ctl journeys, tutorial commands, runtime-only doc tests (checked codes) and trust-debug pass.
  Failures: one compile-fail control with a lifetime error (fixed) and the examples test above.
  Review then found the unbounded `setExpression` queue (fixed, with a test) and two debugger
  files past 1,000 lines (tests and the symbolic-force helpers moved to their own modules).
- [x] Run 1 of the focused check stopped at the workspace compile: two `String`/`SmolStr`
  mismatches in the debugger's setVariable code, which the targeted run had not compiled. Fixed;
  Clippy on all targets and features and the format check then passed, and the architecture
  full map (FULLMAP-CHECK-10 included) passes on `34badc0f1`.
- [x] Focused check 2 (`fixall-focused-2`, the committed source): 19 of 19 steps pass. Workspace
  9,693 passed, 0 failed, 30 ignored (the runnable-examples test takes 24 s); core all-features
  903, portable 734, register 794; i686 793; runtime doc tests 27 and runtime-only 12; doc tests
  with checked error codes 53 and runtime-only 12; runtime-only tests 15; full-composition tests
  10; the A20 journeys 20 of 20 in three repetitions; Clippy, format, F401 and C6 core checks.
- [x] Batch 8 firmware on `34badc0f1` (`H-cost-model-8`): canonical rule files, link,
  inspection and package pass. The link leaves 4,736 B upper flash (batch 7: 4,960; fix-all
  costs 224 B) and 704 B sector-0 free.
- [x] F401 replay of batch 8's firmware (`H-cost-model-board-8`; ELF `38583bd3…`, application
  `71cb8c62…`): `xtask portability verify` passes 101 samples, GPIO, call depths 1 to 4 and the
  numeric bits; MSP peak 11,616 B, longest main scan 3,760 µs. Button, LED and board markings
  stay bench-only.
- [x] Batch 8 PERF-01 (`H-cost-model-8`, 10 of 10 steps pass, source identical at start and
  end): `call_binding` medians 1.039 to 1.069 in all five rounds, pooled p99 0.960 (round 3's
  per-round p99 of 1.253 is recorded only, under the pooled rule); `refs_sizeof` medians 0.824 to
  0.857, pooled p99 0.481; `string_stdlib` medians 0.988 to 1.026 (round 1 disturbed, repeated
  steady), pooled p99 0.972.
  - H's `call_binding` median moved from batch 7's 5,528 ns to 5,618 ns (U 5,369 and 5,339 ns).
    Diagnostic, not evidence (`H-perf-diag-8`): callgrind counts 238.01 M instructions for batch
    8's benchmark binary against 238.19 M for batch 7's, with identical Tier-1 block counts, so
    the shift is code placement, not added work.
- [x] Guard 16 passed on `a446a0c91` (planner and catalog staleness advisory; test-all 14 min);
  pushed `3d9638ca5..a446a0c91` to PR #137.
- [ ] Native CI on `a446a0c91`: 24 of 26 checks pass, including Test on Ubuntu and macOS. Test
  (windows-latest) failed both attempts in `every_runnable_example_builds_and_validates`
  (Unix-socket control endpoints, recorded at the top of this checkpoint), and the Release Gate
  Report reports that failure. The next push carries the test's Windows endpoint handling.
- [ ] Not done: the public-API baselines (`docs/internal/architecture/public-api/*.txt`) are not
  regenerated. `cargo public-api` 0.52 needs a nightly toolchain, which the builder lacks;
  installing one is the owner's decision. No gate compares them.
- [ ] New open questions:
  - MQTT input-freshness states still count as degraded connection health, so they start
    reconnect backoff and unconfirm a safe-state handoff (spec 34 §11.1 keeps that behaviour);
    whether stale input should affect the output path is undecided.
  - The runtime event buffer is unbounded while no event sender is attached, and a refused
    request from a caller without an error channel adds a `write_rejected` event to it.
  - `VariableStorage::create_instance` (panicking) remains public for tests; production
    construction uses `try_create_instance`.

**15 October, night: publication of A01, the call-path regression fix and A20 slice 1 (Claude
lead; the owner authorized fixing all errors in one go and testing again overnight).**
Evidence: external `runtime-portability-h/` (`H-perf-diag/`, `A20/`, `guard-10.log`,
`ci-77fba46dc.log`) and builder `runtime-portability-h/H-perf-diag-6/`, `A20-focused-9/`,
`H-cost-model-7/`.

- [x] Guard 10 passed on `77fba46dc` (planner and catalog staleness advisory); the pre-push
  hook accepted the push `be569f21e..77fba46dc` to PR #137. Native CI complete: 23 checks
  pass, none fail (3 nightly-only skipped), including Test on macOS (29 min), Ubuntu (40 min)
  and Windows (55 min), so A01 and the #135/#136 fixes pass on all three native platforms.
- [x] `call_binding` regression root cause, measured without timing noise: callgrind
  instruction counts on the PERF-01 workload, per benchmark build. Alternating timings alone
  could not attribute 2%, because LTO code layout moved one neutral build by 4%.
  - A01's register/tier-1 step charge (`charge_instruction_cost`) stayed out of line: about
    1,100 extra instructions per cycle. A01's stdlib text charges built an overflow error and
    charged zero bytes on every call: about 230 per cycle.
  - `ea0193246` inlines the step charge (the work-limit refusal is out of line and cold) and
    the text helpers, which skip calls without text and build the overflow error lazily.
  - Diagnostic builds: pre-A01 `be569f21e` 238.15 M instructions, A01 + fix 238.19 M
    (+0.016%); timing 5,501 ns against pre-A01 5,524 ns and U 5,308 ns (ratio 1.036).
- [x] A20 slice 1 (`80ede29f5`): integration tests start the real `trust-runtime play` on a
  private copy of an example and drive it through `ctl`, which the old tests only did against
  a fake server.
  - `runtime_ctl_journeys.rs`: integer and boolean I/O writes, forces, an out-of-range write
    (no fault), production and debug pause and step (#135, #136), the §6.8.1 status, health,
    stats and `config-get` renderings, cold restart and shutdown.
  - `tutorial_ctl_commands.rs`: each documented `ctl` sequence as written; the memory marker
    write is proven by arithmetic on the counter offset, without time windows; a scan of the
    guides and every example README runs each documented `ctl` command line through the CLI,
    with a self-check that the scan rejects `restart --mode cold`.
  - Fixed by the findings: `restart cold` and `config-get` in the operator guide, safety
    tutorial and cloud guides; the operator guide's `status` and "recent events" text; the
    memory marker readings (`Int(41)`); the OPC UA example's RESOURCE name.
  - Focused check 9 (builder, before the Clippy fix in the harness): workspace 9,573 passed,
    0 failed, 30 ignored; A20 18 of 18 in three repetitions; core lanes, doc tests and
    compositions pass. Clippy then passed on the workspace after two collapsed `if`s.
- [x] F401 replay of batch 7's firmware (`H-cost-model-board-7`; ELF `d15ecf0b…`, application
  `71cb8c62…`): `xtask portability verify` passes 101 samples, GPIO, call depths, watchdog
  reset and DONE; MSP peak 11,616 B, longest main scan 3,749 µs. The link leaves 4,960 B
  upper flash (batch 6: 5,152; the inlined text helpers cost 192 B) and 704 B sector-0 free.
- [x] Batch 7 on `80ede29f5` (`H-cost-model-7`, source identical at start and end): 12 of 12
  steps pass, including canonical rule files, i686, firmware link and inspection, the
  application tests (work cost model) and PERF-01. PERF-01 `call_binding` medians 1.020 to
  1.042 in all five rounds (batch 6: up to 1.103), pooled p99 1.016; `refs_sizeof` medians
  0.840 to 0.878, pooled p99 0.524; `string_stdlib` medians 0.962 to 0.990, pooled p99 0.931.
- [x] Guard 11 passed on `a42059511`; pushed `77fba46dc..a42059511` to PR #137.
- [x] A02 executor differential (`9a6960df9`). A read-only audit mapped every existing
  differential test against the A02 scenarios and executor pairs and found 14 gaps; the ten
  that need no product decision are closed by tests that run the same program and per-cycle
  inputs on each executor and compare values, outputs, faults and task state each cycle:
  - default-threshold boundary: every block's 64th entry falls on cycle 64, and Tier-1 then
    runs each block once per cycle (G3); interpreter against Tier-1 with per-cycle inputs and
    an injected `ModuloByZero` (G4); both outcomes of IF and CASE (G5); warm and cold restart
    with retained and plain counters, Tier-1 observations surviving restart (G6); a periodic
    task's state (G8); interpreted versus Tier-1 entries derived from block hits (G11 test
    part); the harness differential now proves its register arm ran with Tier-1 off (G12);
  - leaf-call journeys run Tier-1 from the first entry and recover after the fault with
    `clear_fault`, comparing cycles 22 to 24 across stack, pumped and in-place (G1, G9);
    recursion has stack and Tier-1 arms and asserts the two `nested_scratch` fallbacks (G2);
    the cold-restart cost test re-checks its answer, nested calls refuse at the same
    `ExecutionLimit` on all three executors, and the construction parity tests gained stack
    arms (G10).
  - Builder run: all 44 tests in the seven changed files pass. Two new tests first failed
    because they assumed one block per cycle; the program has three, now derived from the
    block profile. Clippy and format pass.
- [x] Owner decisions from the A02 audit (decided 15 October, implemented in the fix-all
  commits above): G7, when a debugger attached mid-run takes effect
  (activation start only, `activation.rs:109`; spec 34 HOST-04 does not say); G11, a counter
  that proves the interpreter ran (product change); G13, Tier-1 `deopt_count` and
  `deopt_reasons` are never written, so five assertions on them pass vacuously
  (`tier1/state.rs:142-143`); G14, an evicted Tier-1 block is reselected on its next entry
  without re-warming (`tier1/state.rs:212-220`).
- [x] Guard 12 passed on `787bc6d54`; pushed `a42059511..787bc6d54` after `a42059511`'s native
  CI completed green (23 pass; Test macOS 36 min, Ubuntu 34, Windows 30), which includes the
  A20 real-binary journeys on all three operating systems.
- [x] A09 resource boundaries, test-only gaps. A read-only audit listed every declared limit
  (spec 12 §4.6, spec 34 TIME-08/09, LOAD-04/09, MEM-02/03/06) with its enforcing code and
  found exact-limit acceptance missing for many and `is_err()`-only refusals in the named
  suites. New tests assert the exact error at 0/1, the exact limit and limit + 1:
  - preparation: `max_call_depth` 1,024/1,025, zero work allowances refused as
    `ProfileUnsupported` on both routes, `max_process_image_bytes` N/N-1 as `PreparationLimit`,
    exact `PreparationLimit` for the cumulative budget, and no live allocation after a refused
    preparation (counting allocator);
  - decode and validation: native-call arguments 1,024/1,025, construction records
    65,536/65,537 and nodes 1,000,000/1,000,001, REF_TABLE, POU locals and parameters at the
    decoder, the 1,000,001st instruction refused before its jump is checked, first-fault order
    (REF_TABLE before POU locals; instruction limit before operand stack), constant nesting
    64/65, and a 2^32-byte array sized exactly on 32-bit as on 64-bit (sizes are `u64`);
  - runtime: every `vm_resource_limit_cases` scenario with its exact error and exact-N
    acceptance (the locals fixture had tripped the REF_TABLE check first), an oversized
    container that leaves the installed program running, execution-frame capacity, measured
    construction and scan allowances N/N-1 with `used + requested == N`, array upper bound,
    HMI string capacity, reference-path depth 64/65 (`TypeMismatch`), and a malformed retain
    checkpoint path refused (`Overflow`; RETAIN-02 fixes no error kind, MEM-06's
    `TypeMismatch` governs value copies).
- [x] A09 owner decisions from the audit (decided 15 October, implemented above): MQTT incoming queue drops the oldest of 256 silently
  (`io/mqtt/session.rs:63-64`; spec 34 §11.1 requires explicit overflow); `SIZEOF` nesting 128
  exists only in code (`vm/dispatch_sizeof.rs:8`; spec 12 §4.6 has 64 elsewhere); with
  `max_artifact_bytes` above 64 MiB the decode work charge refuses before the container bound
  (`decode/module_decode.rs:17-21`); no declared debugger/control queue capacity; no declared
  instance-count capacity (legacy `create_instance` panics on exhaustion). Also: spec 12 §4.6.2
  names "REF_TABLE entries" while the decoder reported "REF_TABLE count" (now "REF_TABLE entries").
- [x] A25 audit (read-only) and positive examples. The installed `Runtime` has no public
  mutable access to storage, registry, I/O or access map in either composition. Spec 34
  HOST-03's missing migration examples are now compiled and run from the embedding guide: an
  alias write, a process-image write (a bound output is republished from its variable each
  cycle; writing an input the program reads drives the output), and a schema replacement
  (retained `count` kept by name, `transient` restarted, `added` initialized, logical time
  kept). A runtime-only example runs the F401 fixture, and an unclosed doc fence is fixed. Doc
  tests: full 16 and runtime-only 4 pass.
- [x] A25 owner decisions (decided 15 October, implemented above; see the two changes):
  - stable rustdoc does not check the error code of a `compile_fail,E0xxx` doc test (verified
    with rustc 1.99.0: a test declaring E0308 passes on E0425), so the 18 compile-fail doc
    tests prove only that a snippet does not compile. A nightly doc-test lane, `trybuild` (a
    new dev-dependency) or a harness that checks rustc's JSON diagnostics would prove the
    reason. The audit's new compile-fail cases (escaping borrows E0502/E0515, private tokens
    E0451/E0616, internal traits E0277/E0603, runtime-only E0596/E0599) wait for that choice;
  - the public core owners reachable through `trust_runtime::runtime_core`
    (`OwnedRuntimeState`, `RuntimeState`) return an unchecked `&mut [u8]` input image and
    expose `reset_task_timing`, time and fault setters that the installed controller keeps
    crate-private (`vm/engine/owned.rs:172, 220-222, 240-248`; `vm/engine/api.rs:162, 207-209`);
  - the debug context has a public `&mut VariableStorage` field (`host/debug/hook.rs:13-16`)
    and `ReferenceContext` is an unsealed public trait under `full` (`vm/context.rs:9`);
  - CI's `cargo test --all-targets` runs no doc tests, no gate builds the runtime-only
    composition, and the public-API baselines are stale and not compared by any gate.
- [ ] Release guard on the A09 commit, the push to PR #137 and native CI. Guard 13
  (`aedf192b8`) failed FULLMAP-CHECK-10: A09 had grown `runtime_core_compiler_free_load.rs`
  past the 1,000-line module threshold. That file is restored, and its zero-allowance
  assertions live in core `preparation_profile.rs` (exact `ProfileUnsupported` for zero work
  allowances, and the zero construction-byte refusal at instantiation).
- [x] Owner decisions found by A20 (decided 15 October, implemented above):
  - 89 other example projects name a `[resource]` in `runtime.toml` that their ST does not
    declare (every other communication example, `network_canvas_demo`, all 80 OSCAT
    projects), so `play` refuses them, as specifications 10 and 11 require. This is
    pre-existing on `main` `603278202`.
  - `trust-runtime validate` falls back to the primary resource (`run/commands.rs:107`),
    contrary to specifications 10 and 11, so the documented build and validate loop passes
    projects that `play` refuses.
  - `ads_server_basic` names its resource `truST-ADS`, which no IEC RESOURCE identifier can
    match.
  - `set` and `var.force` still reject untyped integers for `INT` targets; #135's literal rule
    covers I/O mutations only (spec 11).
  - No `ctl` command shows `write_rejected` or recent events: `events.tail` and `faults` are
    control operations without CLI subcommands.

**14 October, evening: A01 native contract coverage (Claude lead).** The audit, owner
decisions, product corrections and regressions below are on the H branch after PR #137's
first native runs. Evidence: external `runtime-portability-h/A01-audit/`.

- [x] Audit: six read-only reviewers matched every changed H behaviour (spec 34 HOST-01 to
  HOST-09, TIME-08/09, §4.3.1, the temporal ABI, spec 32 §5, spec 10 §7 binding, references,
  faults, restart and replacement) to native assertions in run 5's logs: 773 behaviour rows,
  519 covered, 130 partly asserted, 107 without an assertion, 16 not run in run 5
  (`report-G1.md` to `report-G6.md`).
- [x] Owner decisions 14 October: the STBC 3.0 CRC32 covers the whole file with its checksum
  bytes as zero (spec 12 §4.1), so a relabelled version or flag fails; writable connections
  that coincide only at run time commit in formal declaration order and the last declared
  formal wins (spec 10 §7.5, `IEC_DECISIONS.md`); standard-library text work is charged, as
  TIME-02 and TIME-08 already required.
- [x] Spec 34 v0.20: TIME-08 states every text rate in UTF-8 bytes, reference growth, `SIZEOF`
  and the `STRUCT OVERLAP` write cost; a zero work limit is unsupported; the local-initializer
  acceptance case agrees with specification 03 (E202).
- [x] Product corrections the audit exposed:
  - standard-library and conversion calls charge 1 unit per UTF-8 byte of their text
    arguments before the body and their text result as a new value, so a loop doubling an
    unbounded `STRING` stops with the work limit instead of exhausting host memory;
  - reference growth charges per added segment and subscript, independent of capacity;
  - a call's rollback journal is no longer charged to the allocation limit;
  - register and tier-1 refusals of a fused step report the stack's boundary (`used` equal to
    the limit, one unit requested); no fused group has an effect before its last instruction;
  - the STBC 3.0 whole-file checksum;
  - health publication never blocks the control cycle (HOST-05);
  - debugger I/O views keep declared enumeration type names, also over remote sessions;
  - an unknown `VAR_ACCESS` write reports `runtime_undefined_variable` (spec 10).
- [x] Regressions: about 270 new native tests in new files (prefixes `installed_write_`,
  `checked_admission_`, `hosted_observation_`, `call_path_`, `work_allowance_`,
  `resource_only_`, `mqtt_install_`, `temporal_abi_`, `call_transfer_`, `restart_generation_`,
  `hosted_platform_`, `stbc_header_`, `stdlib_text_work`, `text_work_`); compile-fail doc tests
  for the installed `Runtime` and `RuntimeBuilder`; the embedding migration guide's examples as
  doc tests; runtime-only and full composition runs.
- [x] Not changed after review: an explicit tier-1 reset clears the state's compile counters as
  designed; the immutable preparation diagnostics live in `preparation_snapshot()`. The audit's
  reading was wrong and the first correction was reverted.
- [x] Focused check 6 on the A01 source (builder; not acceptance): workspace 9,546 passed, 0
  failed, 30 ignored; core all-features 858; portable 708; register 768; runtime doc tests 12;
  runtime-only doc 3 and tests 15; full-composition tests 10; Clippy and format pass; F401 and
  C6 core checks pass.
- [x] GitHub issues against v0.24.74, owner decision 14 October: fix inside H only.
  - #135: `ctl io-write`/`io-force` of an integer no longer fails on `INT` destinations. When the
    request is applied, an untyped integer takes the binding's declared integer or bit-string
    type if it fits (IEC 61131-3 §6.3.2), else `write_rejected` without a fault; spec 11 states
    the rule for I/O mutations. `set`/`var.force` keep their declared-type check (follow-up).
  - #136: a step is accepted only while the debugger holds the scan, so a production `pause`
    then `step-over` is refused instead of stalling; an attached debugger still steps from a
    breakpoint; `resume` always releases a debugger hold; `status` reports the hold.
  - #134 (HMI modules served from the build machine's path) is fixed on the HMI branch.
  - Focused check 7 with both fixes (not acceptance): workspace 9,555 passed, 0 failed, 30
    ignored; core lanes, doc tests, compositions, Clippy and format pass.
- [ ] Deferred to scope C (ignored with reason): a handle taken before a warm restart or body
  replacement can resolve into the new generation (MEM-07, UPDATE-04). `ValueRef` carries no
  generation in U or H; spec 34 assigns generation tokens to UPDATE-03 to UPDATE-08.
- [ ] Open, not blocking A01: debugger scratch slots read Null at a `FOR` body stop (needs a
  probe); HOST-02 deployment-level limit record (spec: proposed); the M0 feature matrix (A19);
  RETAIN-01 checkpoint sequence and completion fields (C); test seams for projection and capture
  failures (the debugger write queues are bounded since 15 October);
  the F401 profile limits are not host-testable.
- [x] A01 batch 6 on `482503bfe` (builder, `H-cost-model-6`, the trimmed run-5 harness
  `batch-a01.sh`): 50 of 54 steps pass, including core lanes, i686, firmware link and
  inspection, compositions, applications, real databases, VS Code 1.109.0 and stable, rustdoc
  and docs. Failures:
  - `environment` and `canonical` are harness defects: `df` on a target directory cargo had not
    created yet, and a checksum list recorded before the 14 October `AGENTS.md` update that also
    listed `.pyc` caches. A direct comparison shows H's `AGENTS.md`, `CLAUDE.md`, skills and
    purpose file identical to the primary checkout.
  - `architecture-fullmap` (FULLMAP-CHECK-10): A01's guide doc-test harness was a new top-level
    `trust-runtime` module without a subsystem decision note. `561afed39` moves it under `runtime`.
  - PERF-01: `call_binding` round 1 median ratio 1.1032 (limit 1.10); rounds 2 to 5 pass; the
    pooled p99 passes for all three workloads (0.909, 0.456, 1.122).
- [x] F401 replay of batch 6's firmware (`H-cost-model-board-6`; ELF `f39504f9…`, application
  `71cb8c62…`): `xtask portability verify` passes 101 samples, GPIO, call depths 1 to 4,
  watchdog reset and DONE; MSP peak 11,616 B, longest main scan 3,753 µs. The link leaves
  5,152 B upper flash (run 5: 6,208) and 704 B sector-0 free (run 5: 736). Button, LED and
  board markings stay bench-only.
- [x] PERF-01 diagnosis (diagnostic, not evidence; `H-perf-diag-6/ab`): six rotating rounds on a
  quiet builder give U 5,328 ns, run 5's H 5,523 ns and batch 6's H 5,628 ns (median of round
  medians). Batch 6's H is 1.9% slower than run 5's (ratio to U 1.056 against 1.037) while U is
  unchanged, so the code moved; the failing round is that shift plus disturbance (H 5,904 ns).
  The repeats of rounds 3 and 5 were disturbed for U (about 7,700 ns) and still count as final
  under the disturbed-round rule, which skews those rounds in H's favour.
- [ ] Follow-up (owner decision 14 October: rerun now and track the regression): find the 1.9%
  `call_binding` slowdown between run 5 and batch 6 (A01 or #135/#136; the builder has no
  profiler, so bisect with rebuilt benchmark binaries) and remove it if it is avoidable.
- [x] Owner-authorized rerun of the two failed checks on `561afed39` (`H-cost-model-6-rerun`,
  source identical at start and end): the architecture full map passes; PERF-01 passes, with
  `call_binding` medians 1.042 to 1.060 in all five rounds and pooled p99 ratios 1.115
  (`call_binding`), 0.556 (`refs_sizeof`) and 0.894 (`string_stdlib`). Round 3's per-round
  `call_binding` p99 of 1.336 is recorded only, under the pooled rule.
- [ ] Release guard, the push to PR #137 and native CI. Attempt 8 (`36a46fa33`) stopped at
  `remote_exact_head` because the lead had not staged the candidate on the builder. Attempt 9
  passed every step through `remote_test_all` (format, Clippy, cross-target warnings, supply
  chain, architecture safety, VS Code, test-all in 13.5 minutes) and failed
  `remote_mp001_parity`: A01's six `call_transfer_overlap_messages` tests were missing from the
  MP-001 HIR test-discovery baseline, which the next commit adds.

**14 October, later: owner chose to fix the call path; Claude now leads the remaining scopes.**
The owner retired Codex and rejected accepting the `call_binding` trade-off ("we need a
professional runtime"). The validated run-4 source is backed up externally
(`claude-validated-run4-20261014/`, tracked patch SHA-256 `9f109ada…`, untracked
`72c108fb…`). Call-path work since then, all behavior-preserving (spec 34 v0.18 HOST-04
"Work done in place"):

- [x] Analysis (cycle-only Callgrind with cache simulation, external `h-prof4/`): per call H
  executed about 7,500 instructions and 450 instruction-cache misses; the code executed at
  least four times per cycle was 668 cache lines against U's 554, both above the 512-line
  L1 instruction cache. Moving data (an activation arena, tried and reverted) did not help;
  executing less distinct code per call did.
- [x] Direct frame defaults: frame-owned literal and intrinsic scalar defaults without an
  executable body are written straight into the new frame with the declaration action's
  checks and charges (unit tests compare both paths, including the type-stack entry charge
  found by those tests).
- [x] Fx hashing for the module's POU-id maps (SipHash cost about 470 instructions per call).
- [x] Tier-1 block selection and register activations borrow prepared data instead of cloning
  four `Arc`s per call.
- [x] In-place leaf calls: a register callee whose prepared plan makes no user call runs inside
  the caller's call instruction with the pump's checks, charges, completion and failure
  cleanup; nested calls, debugging and occupied callees keep the pump. `complete_call`'s core
  is shared as `finish_child_call`.
- [x] Result (diagnostic copy, not acceptance): instructions 75.46M against U's 78.47M, cache
  misses 3.65M against 2.66M, per-call code 622 lines. Timing (`h-prof4/timing-v4`, 5 rounds):
  `call_binding` medians 0.978–1.009 and p99 0.84–1.07 in four rounds; round 5 was a
  disturbed run (H p50 7.84 µs against 5.4 µs otherwise). `refs_sizeof` 0.83–0.85 and
  `string_stdlib` 0.94–0.97 pass every round.
- [x] Measurement noise: a U-against-U run (`h-prof4/timing-aa`) failed one `string_stdlib`
  round (median 1.39, p99 1.42) with identical binaries. Whole-run slowdowns of 40–45% and a
  second latency mode 2–3 µs above the base occur on the shared builder; p99 over 1,024
  samples mostly measures that environment. Recorded for the owner; thresholds unchanged.
- [x] Focused correctness check of v4 (diagnostic copy): core all-features 767, portable
  register 679, portable stack 626, runtime library 3,957, 13 runtime call suites 123
  (register parity, differential, depth-33, tier-1 resumption, deferred FB failure), runtime
  vertical 28 — all pass. The same call suites also pass with in-place calls disabled.
- [x] New native tests: `frame_defaults/tests.rs` (direct path equals the declaration action
  for intrinsic and literal defaults, depth refusal and every declined case),
  `register_ir/prepared/leaf_tests.rs` (leaf flag), `failed_leaf_function_withholds_in_out_copyback`.
- [x] Independent review (fresh-context Opus reviewer, delta SHA-256 `e1fe4afb…` plus the
  `leaf_call.rs` single-step edit and rustfmt reflows): no blocker or major finding; every
  in-place path, frame default and the `complete_call` refactor traced equal to the general
  path. Minor findings and dispositions:
  - Accounting follows layouts: handler continuations shrink with the activation (fewer
    bytes charged on growth) and each prepared plan grows 8 bytes; recorded in HOST-04.
  - Couplings now pinned: `native_call_may_yield` shared by the leaf flag with a test that a
    stdlib call completes immediately; an exhaustive `lifetime_check_is_trivial` replaces the
    literal exclusion list; frame-default parity for every eligible primitive and at every
    limit (work, type-stack bytes, constructed values, a refused text copy).
  - Pump/in-place parity: a test-support switch and counter; `in_place_leaf_calls_match_the_pump`
    (success through tier-1 warmup, faults in a leaf function and a leaf function block,
    depth refusal) compares results, faults, values and profile/tier-1 counters, and
    `in_place_leaf_calls_charge_the_pump_work` compares minimum scan work.
  - The embedded profile also runs the frame-default path and the shared `finish_child_call`:
    run 5's F401 link, inspection and board replay must show size and stack within margins.
  - Spec and CHANGELOG wording corrected (guarantee placed under HOST-04; "faster" not
    claimed before PERF-01 passes); comments corrected.
- [x] Version 5 (exact source before review fixes, diagnostic): `call_binding` medians
  0.976–1.017 under a load average near 6 from another session's VS Code journeys; one p99
  round 1.53 with U's own p99 ranging 8.9–14.1 µs. Run 5's timing step now waits up to 90
  minutes for a load average below 1.5 and records the wait; criteria unchanged.
- [x] Run 5 (`H-cost-model-5`, 11:04:36–11:48:28 UTC; tracked diff with full object ids
  `a78fddd1…` on both machines, the abbreviated identity differs only by git version):
  61 of 62 steps pass, source identity stable. Full workspace 9,278 passed, 0 failed (28
  ignored); core all-features 775, portable 633, register 687, i686 686; applications 91;
  runtime vertical 38; lint, architecture, diagrams, docs, supply chain, compositions, MCU
  lanes; real-database gate; browser captures 10; VS Code 1.109.0 and stable 519 each.
- [ ] PERF-01 in run 5 (consumer `0f1b6996…`; quiet wait 0 s, load 1.41): medians pass every
  round (`call_binding` 1.028–1.051, `refs_sizeof` 0.597–0.835, `string_stdlib`
  0.950–0.990). The owner-approved disturbed-round rule (spec 34 PERF-01, harness installed
  11:22:36 UTC before any timing) re-measured `refs_sizeof` round 1 (U +48%, repeat disturbed
  again, counted, passes) and `string_stdlib` round 5 (H +50%, repeat steady, passes). Two
  comparisons fail p99 with steady medians: `call_binding` round 2 (1.335) and
  `string_stdlib` round 3 (1.425). Pooled over the five rounds (5,120 samples per build) the
  p99 ratios are 1.028, 1.029 and 0.469; the U-against-U calibration gives per-round p99
  ratios 0.75–1.42 for identical binaries and pooled 0.971/0.936. Under the current rule
  PERF-01 fails; the owner decides whether p99 is evaluated over the pooled rounds.
- [x] Run 5 firmware and F401 replay (`H-cost-model-board-5`; ELF `cc444e74…`, application
  `71cb8c62…`): link leaves 6,208 B upper flash (run 3: 7,232) and 736 B sector-0 free;
  `xtask portability verify` passes 101 samples, numeric bits, GPIO, call depths 1–4,
  watchdog reset and DONE; MSP peak 11,616 B (4,768 B left), heap peak 23,496 B, longest
  main scan 3,754 µs. Button, LED and board markings remain unverified (bench).

- [x] Owner approvals 14 October: local commit, the disturbed-round rule, a branch push for
  native CI. H committed locally as `59e582eee` (690 files), then rebased onto published
  `main` `603278202` (v0.24.74) as `d3a3508ba`; backup branch `backup/h-before-main-integration`.
  Five conflicts: CHANGELOG (both entry sets; H's copy of the real-database entry dropped as a
  duplicate), the two OpenOT test support files (H's diagnostic hooks on top of main's scan
  isolation), `xtask/src/browser_command.rs` (H's version: main's file plus Windows job-object
  cleanup, dependency already in H's xtask manifest) and this checklist (H's checkpoint kept;
  main's U publication checkpoint preserved as history).
- [x] Guard attempt 1 on `d3a3508ba` (version 0.24.75 differs from main, so the push is
  release-sensitive; bundle `candidate-d3a3508ba.bundle`, SHA-256 `d69137ec…`, clean builder
  worktree `trust-platform-h-release`, target `trust-portability-h-release`) failed at
  `strict_gate` before any builder step: the direct change contract requires
  `docs/specs/32-mqtt-io.md` with MQTT production paths. H moved the MQTT binding
  installation to `Runtime::configure_io_bindings` (one resource-only reconfiguration);
  spec 32 still named `io/interface.rs`. Behaviour is preserved (typed and enum codecs,
  reuse of existing bindings, atomic refusal; the traffic-light test binds variables no
  code uses). Console: `.artifacts/runtime-portability-h/h-publication/prepare-console.txt`.
- [x] Owner decision 14 October: PERF-01 p99 is evaluated over each build's five final
  rounds pooled; per-round medians and both thresholds unchanged (spec 34 v0.19). Harness
  `perf01_verdict.py` decides the gate; on run 5 it passes: pooled p99 ratios
  `call_binding` 1.0282, `refs_sizeof` 0.4694, `string_stdlib` 1.0290, all medians pass.
- [x] Correction commit `de0fea9b4`: spec 32 owner list, installation sentence and proof map;
  spec 34 v0.19; this checklist. The strict gate passes locally (direct change contract empty).
- [x] No timing round is needed for the integrated source: `crates/`, `xtask/`, `Cargo.toml`,
  `Cargo.lock`, `rust-toolchain.toml` and `.cargo` of `de0fea9b4` are byte-identical to the
  tree run 5 built and timed (2,586 files; the builder copy is unchanged since run 5). Main's
  code changes since H's base were already in that tree. Proof: external
  `h-publication/timing-identity/`.
- [x] Guard attempt 2 on `de0fea9b4` (13:29–13:42 UTC): strict gate, VS Code, format and
  cross-target warnings pass; `remote_supply_chain` fails on four new advisories against
  `rustls` (RUSTSEC-2026-0346 to 0349; `main` has the same versions). Later steps unrun.
- [x] Owner decision 14 October: fix on this branch. `rustls` 0.23.45 → 0.23.46 (lockfile
  only, three dependents) fixes every client path; `tiny_http` 0.12's HTTPS server keeps
  `rustls` 0.20.9 (no fixed release; server-only use, and 0.20 has no post-quantum group to
  downgrade from) under an owned `deny.toml` exception expiring 18 November 2026 with the
  other `tiny_http` exceptions. `scripts/supply_chain_gate.sh` passes with the fix on a
  builder scratch copy. Run 5's PERF-01 binaries linked `rustls` 0.23.45; the timed workloads
  never execute TLS code, so the timing evidence stands.
- [x] Guard attempt 3 on `ae1dacf50` stopped at the rule-file bootstrap: another session had
  changed the primary checkout's `AGENTS.md` at 13:31 UTC; `ee1f7dfc4` carries the canonical
  copy. Guard attempt 4 on `ee1f7dfc4` passed (13:58-14:21 UTC; workspace 9,278 passed, 0
  failed, 28 ignored). Branch pushed; draft PR #137 (validation only, do not merge). `main`
  needs the same `rustls` fix separately.
- [x] PR #137 CI: format, Clippy, supply chain, architecture, MSRV, VS Code, conformance, fuzz
  smoke, documentation and both portable-core targets pass. The Test job failed on every OS
  on a test H had changed, and stopped there (`cargo test` fails fast: 72 of about 337 test
  binaries ran on Linux and macOS, 62 on Windows):
  - Linux and macOS: the traffic-light MQTT test now runs a cycle (HOST-09 removed raw image
    access), which hands outputs to the MQTT worker; without a broker the default fault
    policy fails the cycle. The builder's local broker hid this. The test now sets
    `on_error = "warn"`, like the other broker-free MQTT tests; product behaviour unchanged.
  - Windows: the host clock test stepped 1 ns before the epoch, which a 100 ns Windows clock
    rounds to the epoch itself. It now steps 1 µs.
- [x] Guard attempt 5 on `96211712f` passed (workspace 9,278 passed, 0 failed); pushed. PR #137
  rerun: the Test job passes on Linux and macOS (the first complete native macOS run of H).
  Windows stopped at two H `xtask` browser-command tests: `<nul set /p=...` exits with
  ERRORLEVEL 1, and Rust's argument quoting is not what `cmd.exe` parses, so the
  descendant-cleanup command never ran as written. Both tests now end with `exit 0` and the
  cleanup test passes its command line verbatim (`raw_arg`).
- [x] Guard attempt 6 on `5a8da210d` passed; pushed. Windows now runs both tests to the end
  (the leader exits at once, no timeout); their output carried the space that `set /p`
  prints before `&`. The commands no longer have that space.
- [x] Guard attempt 7 on `be569f21e` passed; pushed. PR #137 is green on every check: the
  native Test job passes on Windows (31 min), macOS (29 min) and Linux (36 min), including the
  Windows browser-command descendant cleanup. These are H's first complete native Windows and
  macOS runs (HOST-01, HOST-08).

**Earlier on 14 October (Claude review and correction): validated except PERF-01 `call_binding`.**
The owner asked for a review and fix of H. Root causes found:

1. Hosted execution moved onto the portable engine, whose bookkeeping was charged to
   the work allowance (1 unit per scalar clone, normalization pass and lifetime check,
   plus index comparisons). The OpenOT changed scan used 219,129 instruction units and
   about 1.2M bookkeeping units.
2. Construction ran under the 1M per-scan allowance instead of the preparation profile.
3. The engine gave a whole scan one allowance; U and specification 10 give each
   top-level program invocation its own. A six-program resource got a sixth of U's.
4. VAR_IN_OUT arrays were snapshot and charged per element for work and cumulative
   allocation (1,038 snapshots of a 1,024-element buffer in one OpenOT scan), although
   IEC passes them by reference.
5. Firmware growth after the last fitting image came mostly from work-motivated marker
   paging (about 4 KB). Codex's proposed typed-array storage targeted (4) only.

- [x] Spec 34 v0.17: TIME-08 cost model (instructions plus value-size work; bookkeeping,
  including VAR_IN_OUT and rollback snapshots, never charged) and TIME-09 allowances
  (construction under `max_construction_work`, hosted default 64 Mi, F401 50,000; one
  `max_work` per top-level invocation). Specs 10 and 12 follow. The ~200-line H
  implementation block after HOST-04 is reduced to its behavioral guarantees.
- [x] Core: bookkeeping charges removed; COPY, TEXT, OVERLAP, LIFETIME, CONSTRUCT,
  NATIVE and INSTR charges kept; character reads keep TEXT via `charge_text_work`;
  lifecycle markers restored to the sorted-vector form with the plan/reserve/publish
  API; per-invocation work reset in the cycle.
- [x] Native tests: `crates/trust-runtime/tests/work_cost_model.rs` (lookup-size
  invariance, per-element array copy, IN_OUT binding without copy cost, construction
  allowance), core budget test, debugger observation adds no work.
- [x] Run 1 (`H-cost-model-1`, superseded source): core lanes 1 test-double failure,
  workspace 3 failures, OSCAT/PLCopen/work-cost tests pass, clippy, MCU lanes,
  architecture, diagrams, docs, browser and VS Code 1.109.0/stable pass. Supply-chain and
  CLI-ordering failures were harness errors, corrected. F401 link 4,704 B over.
- [x] Diagnostic copy (not acceptance): all 12 OpenOT database-example scans pass at
  default limits; F401 links with 544 B upper free plus 768 B sector-0 free.
- [x] Run 2 (`H-cost-model-2`, identity diff `48a004b7…`): 56 of 62 steps pass, including
  firmware link/inspect/pack, MCU and composition lanes, runtime vertical, workspace (apart
  from the item below), lint, supply chain, architecture, diagrams, docs, browser and VS Code
  1.109.0/stable. Failures: two core test expectations (test-only, corrected: the rollback
  test now needs journal growth after the first four entries; the IN_OUT `snapshot()` keeps
  copy attribution) across the core, portable, register, i686 and workspace lanes; PERF-01
  `call_binding` median 1.285–1.321 (references 0.94–0.96 and strings 0.71–1.04 pass).
  F401 replay (`H-cost-model-board-2`) stopped at `stack-headroom`: instantiation MSP
  15,104 B, leaving 1,280 B of the required 2,048.
- [x] Stack correction: the 544-byte engine record is boxed and built in its own short frame
  instead of staying live under every nested construction initializer; legacy child setup
  stays out of the dispatch loop. Diagnostic replay `H-cost-model-board-stack2` passes
  `xtask portability verify`: MSP peak 12,464 B (3,920 B left), heap peak 23,496 B; the
  link leaves 1,664 B upper flash and 768 B sector-0 free.
- [x] Call-path performance analysis (Callgrind with cache and branch simulation, external
  `h-prof2/`, `h-prof3/`): H executed 1.13× U's instructions but 1.70× its instruction-cache
  misses. Its per-call code path touches about 450 cache lines against U's 340 (dispatch
  loop, call completion, register activation, frame initialization), so each of the four
  calls per cycle refetches it. The estimate instructions + 10 × cache misses + 15 ×
  mispredictions gave 1.27 against the measured 1.29–1.32. Error values built eagerly by
  `ok_or(...)` on success paths cost 2.8M instructions in drops alone; 215 sites in 27
  hot-path files now build them lazily. Result: instruction ratio 1.059, estimate 1.208.
- [x] Run 3 (`H-cost-model-3`, identity diff `1945ca6f…`): 60 of 62 steps pass, including
  all core, portable, register and i686 lanes, the full workspace, applications, runtime
  vertical, compositions, MCU lanes, firmware, supply chain, architecture, diagrams, docs,
  browser and VS Code 1.109.0/stable. Failures: (1) workspace lint: Clippy's
  `unnecessary_lazy_evaluations` rejects the 212 lazy error constructors (it treats building
  a variant as free). The crate root now allows that lint with the measured reason; the
  batch's exact Clippy command then passes on a diagnostic copy. The rewrite had been
  compile-checked but not Clippy-checked before freezing. (2) PERF-01 `call_binding`:
  medians 1.129–1.155 (run 2: 1.285–1.321), p99 above 1.25 in rounds 1 and 5 (1.395,
  1.324). References 0.52–0.84 and strings 0.62–0.96 pass every round.
- [x] Run 4 (focused, `H-cost-model-4`, identity diff `78571ffa…`, script `focused.sh`): the
  lint correction only. All 19 steps pass: source identity and stability, format, workspace
  lint, core all-features (578) and portable tests, F401 core check, firmware link, inspect
  and pack, release consumer build, architecture. The firmware (ELF `0acacd2f…`, application
  `71cb8c62…`, report `caf866a3…`) and the measured benchmark consumer are byte-identical to
  run 3's, so run 3's compiled-code evidence (tests, board replay, performance) applies to
  this source. H stays uncommitted; commit and push wait for the owner.
- [x] F401 replay of run 3's firmware (`H-cost-model-board-3`; ELF SHA-256 `0acacd2f…`,
  application `71cb8c62…`): `xtask portability verify` passes 101 trace samples, numeric
  bits, GPIO, task depths, watchdog reset and DONE. MSP peak 11,616 B (4,768 B left of
  16,384; 2,048 required), heap peak 23,496 B of 73,728, longest main scan 3,773 µs. Link:
  7,232 B upper flash and 800 B sector-0 free. Button transitions, LED observation and
  board markings stay unverified (they need a person at the bench).
- [x] PERF-01 `call_binding` stayed above 1.10 in run 3 (medians 1.129–1.155). Owner decision
  14 October: fix the call path, no exception (see the call-path work above).
- [x] Core unit tests: Codex's last run had 586, the corrected source 578. Of 58 changed
  names, 50 are the same scenarios rewritten to assert that bookkeeping draws no work (their
  result, refusal, rollback and allocation assertions kept). The other 8 covered code that
  no longer exists: marker page directories and their diagnostic recorder, plan validation
  against those directories, and the lookup-work formula. Marker order, retirement, plan,
  reservation and single publication keep tests in `engine/keys/tests.rs`.
- [x] Codex's open items H-CURRENT-TYPED-ARRAY-DECISION and H-CURRENT-CONSTRUCTOR-PROOF are
  superseded: TIME-08 makes IN_OUT snapshots bookkeeping, and TIME-09 gives construction its
  own allowance. Neither storage redesign nor constructor proof is needed for the limits.

Pre-fix snapshot of the H tree: external `claude-fix-baseline-20261014/`.

### U publication checkpoint on `main` at `603278202` (historical)

**U checkpoint `19f3b6787` is scope-verified; PR #133 is held on a retry-rescued macOS native assertion.**
Candidate `63cab14e3` passed its exact-SHA guard (8,719 native passes,
24 ignored; 519 default editor passes), all 17 supplement stages and the corrected
real-database boundary. Independent root reviews reconcile those receipts.
Every fifth-wave GitHub job completed: 23 passed and three expected skips. The
macOS native suite failed its first attempt on OpenOT scan timing (775,524,504 ns
per disabled scan versus 1,441,218,600 ns per enabled scan), then passed a full
retry. That real failure remains retained and prevents merge readiness. New bot
reviews were refused for quota; current-head independent review is separate.
The reviewed database corrections are verified. Independent review accepted the
timing fixture correction; focused run 2 passed all ten steps, including the real
native timing assertion (ratio 0.995), Clippy and complete source identities.
Focused run 1's compile/import failures remain retained; no behavior assertion
executed in that run. Fresh macOS proof remains required. It isolates the same real ST/SQLite workload in a
native binary, pairs scans in alternating order and retains the existing limit.
No runtime production logic, feature set or overflow policy changes. A fresh exact-SHA guard and deduplicated supplement precede another push. No merge or release is claimed.
Historical source-identity corrections and their receipts follow.

Candidate `a5b61b633` passed its exact-SHA guard (8,713 native passes, 24 ignored;
519 default editor passes) and all 16 successor supplement stages. Locked-browser
stable/minimum SDK suites each passed 519 tests; strict isolated documentation and
external source checking passed. Independent source/software reconciliation
accepted the complete receipts. All five third-wave GitHub workflows succeeded;
Linux/macOS/Windows passed 8,712/8,705/8,563 native tests, with zero failures and
first-attempt build/test success. Both automatic reviews completed; security found
no issue, while code review added P2 comment 4241745208: a byte-only source identity
misses executable-bit changes. The owning Rust correction now records file kinds,
link targets and actual Unix executable bits, with missing-mode absence explicit
on other hosts. Native permission, kind, link and byte-only-snapshot regressions
are authored but unrun. Complete logs/reviews remain external; independent review,
one preparation and a clean successor exact-SHA guard precede another push.
No merge or release is claimed. The preceding correction history follows.

Independent review accepted the complete mode/type contract, including refusal
of symlink instruction roots and ancestors. Preparation passed 5/5. Final eight-
path identity `a3e837ee…` was reconciled against the accepted source archive;
only Rust wrapping and the expected SVG/manifest changed. The final diagram was
viewed directly. This paragraph and the matching execution status follow that
prepared identity. Next: clean successor commit, exact-SHA guard and fresh
16-stage supplement; the three new native regressions remain unrun.

Candidate `0f066c9fb` passed its exact-SHA guard (8,710 native passes, 24 ignored;
519 default editor passes) and all 13 correction supplement stages. Full locked
browser stable/minimum SDK suites each passed 519 tests. All five GitHub workflows
in the complete second wave succeeded; native Linux/macOS/Windows passed
8,709/8,702/8,561 tests with no failed tests, and all three native jobs and the
editor job passed on their first attempts. Completed current-head automatic
code/security review found one new P2: a source-freeze manifest inside the
checkout inventories itself after creation. Its owning Rust correction now
requires external manifests and refuses replacement, without adding a source
exemption. Native round-trip, refusal and alias cases are authored but unrun.
Independent correction review, final preparation, a clean successor commit and
exact-SHA guard precede another push. No merge or release is claimed.
Independent source review accepted the complete correction and command wiring.
Preparation passed 5/5; visual review found and fixed only overlapping diagram
labels, with its render/drift suffix passing 4/4. Final eight-path prepared identity
`a22dff53…` was independently reconciled, including direct image inspection.
Only this status update and the execution-note closeout follow that identity.
Next is the clean successor commit and exact-SHA guard; new native cases remain
unrun, and prior CI cannot certify the successor.

The earlier d823 firmware tuple and physical replay remain qualified by identical
core, adapter, firmware, compiler, Cargo and saved-artifact bytes; this tooling
correction is not new hardware execution. All original preparation, guard, first
CI failure, SDK sync and corrective validation receipts remain externally retained.
The owner explicitly authorized necessary fixes and retests without another
permission question; required checks, functions and hardware margins stay intact.

### Previous checkpoint (Codex, superseded)

**14 October: sparse-owner correction verified; H awaits the storage architecture decision.**
`H-runtime-sparse-owner-recovery/` ran 23:46:05–23:47:20 UTC on 13 October.
Frozen manifest: `51b08fcc712c97ebdcda2d0eea22d25ac2bf1528f453998a813445769fd889b6`.
All required software recovery checks pass: core all-features **586**, portable **470**,
i686/register **522**, Clippy, formatting and source/harness identity. Its only crate
changes from the preceding freeze are three test files; 2,590 remaining code records
match. The preceding 58 hosted tests, both MCU core checks, architecture/diagrams and
same-artifact cost captures remain applicable. No full workspace/UI rerun occurred.

The completed default F401 link **fails by 8,960 bytes** (maximum FLASH alignment
extent; .rodata exceeds by 8,944). This is 96 bytes more than the prior measured
8,864-byte overflow. Map SHA-256:
`f1076489694dd846e4b415c58c89b37bb4f5307e8065ca977f469feec1a15ff7`.
No current ELF inspection, board test or physical timing proof exists.

- [x] H-CURRENT-SPARSE-OWNER-CACHE — Full-key-checked count-bounded cache and real
  collision/corruption/refusal tests verified. Original artifact changed-scan work
  falls exactly 106,296 to **1,415,377**, with unchanged copy/slot/root behavior and
  zero diagnostic loss. The unchanged 1M limit still fails; this is not H acceptance.
- [x] H-CURRENT-RECORDER-RECOVERY — Native compile, drain/retry, scope restoration
  and lint verified. The old failed ledgers are retained.
- [ ] H-CURRENT-TYPED-ARRAY-DECISION — Owner question pending: bring forward the
  planned MEM-06/MEM-11 primitive-array storage slice into H, using one shared typed
  storage/call interface and private snapshots while preserving full functions,
  whole-value copyback, rollback, aliases and both execution paths. Public Value
  compatibility and compound fallback remain. Independent design review is recorded
  in `H-runtime-borrowed-metadata/analysis/inout-array-copy-review.md`, pinning design
  SHA `1a7cb7d55c2495fcbf3cb38f995ded003ddff91aeeb136e3182053e8df6860da`.
  No storage implementation is authorized by this checklist entry; no gain or fit
  is promised. This is the architectural decision reserved by the user's H goal.
- [ ] H-CURRENT-CONSTRUCTOR-PROOF — Design review requires actual canonical shell
  naming and composed nesting-depth proof before removing parent walks. Successful
  clone-normalize-and-discard does not prove the original value canonical. No proof
  bypass is implemented; database/PLCopen deficits remain **713,843 / 138,605** units.

H remains uncommitted and not ready for commit. Full functionality, overflow checks,
original work limits and physical memory bounds remain unchanged. No further test
or hardware job is running. Resume from the owner architecture decision, then write
its specification/invariants and native assertions before implementation/review;
retain targeted correctness/size measurement before any full H validation.

### Previous measured checkpoint

**14 October: sparse-owner measurement confirms the full projected reduction.**
`H-runtime-sparse-owner/` is terminal, source unchanged; frozen manifest
`2926b4e2ba3b4804f8f27575a8f41852a65ff3c17bdae4617df3106a472d8ba7`.
The identical 382,596-byte parameter artifact (`fae19f718198b1f20f3e3cafec23c248f5b5e776d1786310a2534c10bfe37d9d`)
now completes the observed changed scan in **1,415,377** work units, down exactly
106,296. Seed scan uses 221,086; construction 299,347 and install 333,008 are unchanged.
Every exported complete snapshot has zero lost records. The original 1M test still
fails; this is no default-limit acceptance or elapsed-time performance claim.

Hosted affected suites (58 tests), MCU core checks, architecture, diagram render/drift
and source/harness stability pass. Core all-features has 583 passes / 3 failed existing
charge/geometry expectations; portable and i686 each fail the same high-ID fixture
assumption. The new cache/drain cases pass. Those fixtures assumed POU7 always used
owner binary search; the folded cache now correctly hits it directly. Correct them
and add a real collided owner, preserving exact refusal and comparison assertions.
Clippy also flags an unnecessary immediately-called closure in a diagnostic test.

The planned firmware measurement was **unrun**, not a link overflow: the harness
incorrectly pre-created xtask's output report and its no-overwrite guard refused it.
Previous 8,864-byte overflow remains the last measured size, not this source's proof.
Focused recovery (`H-runtime-sparse-owner-recovery-setup/PLAN.md`) covers test-only
corrections, lint and the missed default link using a fresh report destination.
Do not rerun passed hosted/application/MCU suites for test-only edits.

The constructor proof proposal remains design-only: review found missing shell-name
and composed-depth premises. The narrow safe primitive-array subset cannot close
constructor deficits. The mutable IN_OUT copy cause has a design candidate: move the planned
MEM-06/MEM-11 primitive-array storage slice forward, with typed execution slots and
private typed call snapshots, retaining public Value compatibility and whole-value
transactional copyback. The same helpers serve stack and register paths; other
aggregates retain their checked path. No functions or target support are removed.
Architecture selection is pending the owner response, as the H goal requires.
External design: `H-runtime-borrowed-metadata/analysis/inout-array-copy-design.md`.
No typed-storage code, savings, fit or acceptance is claimed. Independent review and
invariant/test design precede any implementation. The smaller test/lint/measurement
recovery continues independently; no direct alias or check-skipping workaround is selected.

### Previous attribution checkpoint

**14 October: remaining scan costs now have exact consumers.**
`H-runtime-consumer-attribution/` completed 23:15:02–23:16:41 UTC on 13 October.
All nine complete-capture snapshots have zero drops and exactly unchanged coarse
work/copy/index records, phase results and artifact hashes. Saved artifact bytes
and decoded identity tables are retained. 529 of 541 target-array copies are mutable
IN_OUT payloads. `OPENOT_WriteByte.Buffer` accounts for 412 whole-array copies
(105,472 children, 4,261,728 admitted bytes) to perform indexed byte updates. They
cannot simply become borrowed reads: the existing POU transfer decision requires
private execution state and atomic copyback on success.

POU IDs are sparse: 0 and 34–100 across 68 admitted POUs. The current direct cache
misses high IDs despite available buckets. CRC alone incurs 8,732 owner fallback
queries / 69,856 comparisons; the total is 13,287 / 106,296. The bounded, full-key-
checked cache design in spec 34 now folds raw identities into the existing table,
keeps the first ascending owner on collisions and retains charged binary fallback.
For this trace, the projected 106,296 avoided comparisons still leave the changed
scan at 1,415,377 work units, above 1M; this is a projection, not a passing run.

The diagnostic batch passes the runtime vertical, portable check, architecture and
source/harness stability. Core tests were unrun because one existing test Context
literal omitted the new optional owner field; lint consequently remains unrun.
That fixture correction is prepared. Review also found repeated finish() drained
only attribution vectors while retaining coarse counts; the prepared drain fix and
native regression cover all event families and failure counters together. Successful
first-finish application captures remain recorded without claiming unit-test proof.

- [ ] H-CURRENT-SPARSE-OWNER-CACHE — Implement and independently review the bounded
  folded cache, preserving first-owner precedence, full identities, empty/collision/
  corruption fallback, actual work charges and admitted storage bounds. Test sparse
  identities and u32::MAX; replay the original changed-scan artifact before claiming
  savings. Combine this with the diagnostic unit/lint recovery in one focused batch.
- [ ] H-CURRENT-INOUT-DESIGN — Compare the measured mutable-array copying with the
  written call transaction contract and prior code before selecting a representation
  change. No aliasing shortcut, check removal, work-limit increase or function loss.

The folded cache and four regressions are now authored; independent source and
harness review is in progress. Focused plan: external
`H-runtime-sparse-owner-setup/PLAN.md`. This also covers recorder compile/drain
recovery. No new passing evidence is claimed before the frozen batch.

Next: complete independent review, then focused core/native/i686,
original changed-scan capture, runtime vertical, MCU compilation and default firmware
size. No full workspace/UI batch. Constructor proof remains a reviewed design
proposal only; it is not implemented or credited with savings. H is not ready to commit.

### Previous diagnostic checkpoint

**14 October: metadata/marker correction verified; H remains blocked on measured costs and flash.**
`H-runtime-borrowed-metadata/` ran 22:49:40–22:55:32 UTC on 13 October, with
source and harness unchanged. Frozen manifest SHA-256:
`cbd5d84c625143af2feb48889883e4992ccc40187442dd0a05719d2666fea383`.
Core library suites pass: all-features 575, HIR without register 509, portable 466,
portable-register 519 and i686 518. All 64 selected hosted regressions pass, as do
Clippy, both MCU core checks, architecture, diagram render/drift and the three
32-bit allocation-profile assertions (largest peak 30,361 bytes, zero after retire).
These are focused tests; no full workspace/UI rerun or hardware claim follows.

Complete observations using the identical saved artifacts measure database
construction at **1,713,843**, PLCopen construction at **1,138,605**, and the changed
scan at **1,521,673** work units. Their original default-limit failures remain;
the diagnostic-only 10M allowance is not product acceptance. Default F401 link
fails by **8,864 bytes**, 480 fewer than the preceding map. Performance and ELF
inspection remain unrun because their respective prerequisites failed.

- [x] H-CURRENT-METADATA-BORROW — Prepared type metadata stays borrowed across
  mutable constructor and retain callbacks. Array result dimensions retain their
  actual pre-recipe copy/admission. Native exact-charge and existing recipe/error
  coverage pass in the feature suites above.
- [x] H-CURRENT-MARKER-READ — Each necessary marker directory is searched once,
  after its own admitted bound; row access is charged separately. Missing-directory
  and each refusal-stage cases pass; insertion/publication safety stays unchanged.
- [ ] H-CURRENT-CONSUMER-ATTRIBUTION — The remaining 541 target-array copies and
  13,287 owner fallbacks lack exact consumer/POU attribution. Add bounded numeric
  identities to existing test-support logging, retain the same artifact bytes for
  offline identity resolution, and verify zero drops and reconciliation to unchanged
  coarse accounting. No copy elimination or cache choice is justified from totals alone.

Next: finish diagnostic-only attribution and independent review, then run its small
native-recorder and original-case capture batch. Do not repeat production feature
matrices, firmware or UI for logging alone. Continue map analysis independently.
Any broader typed-value ownership/proof change first needs a concrete design against
all writers, mutation/lifetime boundaries and the spec; no skipped checks are selected.
H remains uncommitted and unverified. Complete functions, overflow checks and the
product work allowance remain unchanged.

### Prior correction checkpoint

**14 October: focused recovery passes; H application and flash acceptance remain open.**
`H-runtime-reuse-failed-checks/` completed 22:32:59–22:34:12 UTC on 13 October.
All 11 ledger steps pass: ordinal and marker regressions, owned-call paths,
i686 ordinals/allocation, affected Clippy, formatting and source/harness stability.
Frozen manifest SHA-256: `76ccb6441402f8911c7d7b6ad0f1f91ee6ac71642fcc67d187fa6dcbfa5a84dd`.
The three i686 allocation cases retire to zero live bytes; their largest observed
peak is 30,361 bytes. This is a 32-bit host estimate, not board RAM proof.

The preceding `H-runtime-reuse-test-recovery/` retains its five feature-lane
failures (the same new ordinal test expected eight units instead of the actual
five zero-fill units plus one lookup), a register-only unused-wrapper compile
failure, and Clippy findings. Corrected assertions also pin work refusal before
capacity growth; the product budget was not raised. No failed ledger is replaced.

`H-runtime-reuse/` completed 22:08:56–22:14:42 UTC. Its zero-drop complete-operation
records measure database construction at **1,817,843**, PLCopen construction at
**1,304,109**, and the changed-input scan at **1,521,673** work units. All three
original cases still fail the unchanged 1,000,000-unit allowance. Diagnostic-only
10M captures explain the whole operation; they do not qualify the application.
Runtime reuse removes measured work but does not close these failures.
The default F401 link occupies 468,096 bytes against 458,752 bytes in upper flash:
**9,344 bytes over**. No current ELF inspection, board replay or timing pass exists.

- [x] H-CURRENT-REUSE-REGRESSIONS — Four reviewed corrections are implemented:
  marker-position reuse with freshness/transaction checks, bounded owner/root
  ordinals, active type-operation masks, and private owned-local transfer. The
  feature-lane native coverage and focused recovery above support these mechanisms;
  this item does not assert application acceptance or performance improvement.
- [ ] H-CURRENT-METADATA-BORROW — Remove the measured repeated TYPE_TABLE metadata
  clones by borrowing the admitted immutable tables across mutable construction
  callbacks. Preserve recursive recipes, type/lifetime checks, refusal order for
  remaining operations, and actual allocation/copy/hash charges. Update all
  construction and retain-migration callers and native assertions before review.
- [ ] H-CURRENT-RESIDUAL-RCA — Inspect remaining owner fallback searches and array
  target reads against their consumers; select only source-grounded changes that
  preserve value, lifetime, alias and rollback semantics. Review current firmware
  symbol growth before selecting a size correction. No claimed savings before maps.

Next: finish this bounded source correction and independent review, then one focused
batch covering its affected native/feature/retain paths, original application
captures, allocation and source-matched firmware size. Measure speed when correctness
passes. Do not repeat the full workspace/UI suite while known application failures
remain. Keep all functions and overflow checks; no H commit or push.

### Earlier checkpoints (historical results, superseded by the status above)

- [x] H-CURRENT-COMPLETE-CAPTURE — Complete operation records retained in
  external `H-runtime-complete-work/analysis/work-charge-RCA.md`. Identical saved
  bytes were observed at default 1M and diagnostic-only 10M; all 15 snapshots
  reconcile with zero dropped records. The unchanged default acceptance cases
  still fail. Complete database construction costs 1,985,373 units, PLCopen
  construction 1,477,986, and the parameter-change scan 1,714,680. These are
  measurements, not permission to increase the product allowance.

**Latest diagnostic batch — complete costs established; H remains unverified.**
`H-runtime-complete-work/` ran 21:25:18–21:27:36 UTC (138 seconds).
Three complete-operation observations and 28 runtime vertical tests pass. Core
passes 553 tests; one new recorder-overflow test incorrectly used two caller
locations and fails before demonstrating overflow. Clippy reports a read-only
Vec parameter, a complex test-only capacity tuple, and an ignored test Result.
The four small corrections are prepared, not yet tested. Architecture, diagram
drift and frozen source/harness checks pass. No timing, firmware or hardware run.

- [ ] H-CURRENT-REDUNDANT-WORK — Implementing four corrections from the complete
  records: retain marker insertion positions through admitted publication; direct
  owner/root indirection bounded by admitted record counts with sparse fallback;
  constant-time active `(type, operation)` membership; transfer private owned call
  locals after selection while keeping borrowed-copy APIs. No lifetime, normalization
  or staging check is removed. Independent review precedes external
  `H-runtime-reuse-setup/PLAN.md`: focused correctness and cost captures, speed if
  correctness passes, default F401 size and allocation measurements. Full H validation
  waits for these. The recorded costs do not promise that this set closes every gap.

**Latest focused result — correct feature lanes, remaining application and flash failures.**
`H-runtime-page-index-stack-r2/` ran 20:59:19–21:05:48 UTC (389 seconds).
HIR/no-register488, portable611, portable-register658, i686657, hosted register104
and isolated stack/register consumers4 each pass. Selected hosted integrations
pass95 and retain11 OpenOT failures; OSCAT passes and PLCopen retains a Work refusal.
All-feature test compilation finds two unconstrained diagnostic test closures;
lint/timing remain unrun. MCU core, architecture/drift and source/harness identity pass.
Complete zero-drop captures show construction340,838/install374,499/seed285,967
for the parameter-change application; its change scan still exhausts1,000,000.
Default F401 link overflows6,624 bytes. No new hardware proof. i686 allocation
measurements remain estimates rather than board evidence. Fix the two test closures,
then use full work/copy and symbol-level map evidence to select further corrections.
The default allowance, complete function set and overflow checks remain unchanged.

**Focused correction batch failed at compilation; small corrections prepared.**
`H-runtime-page-index-stack/` ran 20:53:28–20:54:43 UTC (75 seconds). The record
retains the diagnostic range-type error, private continuation-field accesses and
isolated-feature unused helpers. Portable register library passed 497 tests; its
one failing new test assumed a retained global absent from the chosen artifact.
The corrected fixture uses a real GPIO global and distinct source/target values.
Architecture/render/drift and source identity pass. Host application diagnostics,
timing and firmware measurements remain unverified. Complete this correction set
and independently review before `H-runtime-page-index-stack-r2/`; no full suite.

**Current implementation checkpoint — authored, under independent review.**
The three corrections below now have code and native assertions. Review identified
additional concrete accounting gaps in dynamic reference-candidate groups, visited
instance membership and the method-owner/inheritance fallback; these are corrected
at their owning lookup, without increasing the allowance. Marker-page review also
checks all repeated directory searches and retained publication admission.
Next batch: external `H-runtime-page-index-stack-setup/PLAN.md`. It includes focused
feature/native/i686/application proof, cost recapture, source-matched F401 size and
allocation measurements, and timing only if correctness passes. No new result is
claimed until that frozen batch executes. Full workspace/UI and hardware remain pending.

**Next correction wave: measured marker, lookup and stack-only orchestration costs.**
`H-runtime-charge-components/` finished 20:03:17–20:05:52 UTC. Core 539,
i686 485 and runtime vertical 28 assertions pass; lint, MCU checks, architecture,
drift and source/harness identity pass. Six complete zero-drop snapshots reproduce
R4's original Work results and all 215 work/152 copy rows. PLCopen marker cost is
547,847 = 440,930 shifts + 102,866 searches + 2,044 relocation + 2,007 mutation.
The database constructor separately shifts 99,507 entries for first declarations;
a sparse outer declaration map would retain that cause.

- [ ] H-CURRENT-MARKER-PAGES — Sparse 64-declaration by 64-owner pages; preserve
  keys/order, combined admission before publication, actual page/iteration demand,
  full-width identities, retain/rollback and allocation-failure guarantees. Measure
  sparse-case and F401 RAM; do not assume a memory saving.
- [ ] H-CURRENT-INDEX-CHARGES — One entry unit plus the actual branch's index bounds
  before each search, preserving dense paths and sparse/missing fallback. Recorded
  change-scan prefix projects 61,826 fewer units; completion is not yet demonstrated.
- [ ] H-CURRENT-STACK-ONLY — Keep native stack calls in the SAME opcode loop only
  without register-ir. Keep hosted register pump and call/cleanup semantics unchanged.
  Review external H-stack-segment-continuation-plan.md. No predicted size credit.

Complete these bounded corrections, native assertions and independent review before
focused correctness/diagnostic replay, timing and default F401 link/inspection.
Only then consider full H validation and source-matched physical replay. No commit,
function removal, overflow-check change, allowance increase or margin waiver.

**Historical R5 capture plan (completed above): `H-runtime-charge-components/`.** Existing test-support
logging now separates lifecycle insertion components and actual declaration/root
lookup paths without changing charges or results. Review and freeze before the
three original application captures. No storage/accounting redesign, full suite,
performance run or hardware launch in this diagnostic batch. The last fitting
image and R4 failed map are compared in external `R4/analysis/flash-RCA.md`.

**13 October: R4 targeted corrections pass except one marker-test invariant;
remaining work and flash failures are measured.**
`H-runtime-root-corrections-r4/` ran 19:40:06–19:43:34 UTC with stable source/harness.
All 56 selected hosted assertions pass, including depth-33, tier-1, recursive stack
fallback, aggregate output transfer and the runtime vertical. Initializer allocation
preflight now passes. i686 and both MCU graphs compile warning-free; Clippy passes.
The sole core assertion failure conflates legitimate 16-byte lifetime-check scratch
with forbidden marker publication. Correct it to observe marker count/content/capacity
without reordering lifetime checks or removing the value/overflow assertions.
The test-only CLI recorder dependency needs one ordinary full-map edge classification.
OpenOT construction, audited scan and PLCopen construction captures retain their
original failures, every work/copy row, and valid zero-drop snapshots. They are not
behavior passes. PLCopen marker insertion alone consumes 547,847 work units; further
attribution precedes any container/accounting change. Default F401 linking fails:
3,080-byte data overflow, 3,104 bytes including final section alignment. No image was
installed. Compare the retained map with the last fitting default image before fixes.
Timing/full-suite/hardware acceptance remain open; no budget increase or function
removal is authorized by these results. H remains uncommitted.

**13 October: R3 reached native execution; remaining failures are classified.**
`H-runtime-root-corrections-r3/` ran 19:25:22–19:32:01 UTC with stable frozen
source and harness. Admitted depth-33 calls and tier-1 resumption now pass, as do
104 hosted register compatibility tests, OSCAT and external stack/register consumers.
Four core assertions fail: one real missing root frame allocation preflight and
three exact accounting fixture expectations. Restore the preflight from the prior
dispatcher before moving lexical locals; retain its failing assertion. Correct the
three fixtures against unchanged-before/after counters and the measured lookup bound.
Two new ST fixtures failed compilation before runtime assertions: static writable
aliasing is forbidden by spec 10 §7.5, and a bare recursive function name collides
with its return slot in the encoder. Use legal distinct outputs and a supported
qualified call; do not change compiler policy to make these fixtures pass.
Eleven OpenOT tests and PLCopen still refuse Work at the default allowance. Capture
complete charges on this candidate before selecting further runtime fixes. No budget
increase, function removal, performance waiver or full-suite retry. Warning-only
MCU/i686 failures identify unused compatibility wrappers; narrow their feature gates.
Architecture, diagram drift and metadata pass; performance and firmware remain
unrun because prerequisites failed. H is not ready for commit.

**13 October: R2 compiler failure retained; focused R3 correction prepared.**
`H-runtime-root-corrections-r2/` ran from 19:14:11 to 19:16:18 UTC.
Its sole compiler error is E0382: an error-mapping closure captures the suspended
parent activation before the success path returns it. An explicit error branch
preserves ownership without cloning or changing execution semantics. Native tests,
performance and firmware remain unverified. Architecture, diagram rendering/drift,
formatting and frozen-source/harness checks pass; rendered outputs are copied back.
The next focused batch checks compile prerequisites before dependent executions,
while retaining independent feature/MCU lanes. Independent review precedes its
freeze. No full workspace/UI retry, H commit or push.

**13 October: runtime correction batch blocked at compilation; fixes underway.**
`H-runtime-root-corrections/` ran for 130 seconds (18:59:03–19:01:13 UTC).
Source/harness identity passes. Module-split imports/visibility, feature-specific
frame APIs, one mutable binding, one counter type and an obsolete import prevent
native execution. Source review missed these compiler defects. Full-map also
requires its two existing interpreter-body owner records to follow their renamed
slice functions. No behavior assertions, performance comparison, firmware link or
hardware run passed in this batch. Complete failures and frozen source are retained.
The complete known correction set is being fixed and independently reviewed before
another focused run under the existing authorization; no full workspace/UI retry.

**13 October: focused corrections pass; measured construction/scan and native-stack fixes in progress.**
The retained full `H-next-full/` batch has 51 passes, eight failures and one
explicitly open performance gate across 60 rows. Its original ledger is unchanged.
Both VS Code versions passed 519 tests each and all ten browser cases passed;
images were inspected. This is not H acceptance.

`H-post-full-focused/` corrected the producer, typed fault expectations and external
consumer locks. Its one remaining seed assertion was wrong: the preserved fixture
ends in opcode 0x01, whose specified trap is ForStepZero. `H-post-full-attribution/`
passes both seed tests, all 21 correctly selected I/O unit tests, formatting and
runtime/core Clippy. The earlier zero-test I/O selection remains unverified in its
own ledger. All five retained performance files now copy with their pinned hashes;
this is evidence recovery, not a new performance pass.

- [x] H-CURRENT-PRODUCER — Legacy builtin selection respects the supplied POU
  index; absolute versus relative structure addresses retain the correct domains.
  Existing sparse seed, legacy TON and struct-field cases and new direct encoder
  rejection assertions pass in the focused batches.
- [x] H-CURRENT-FAULTS — Stable timeout code retained with typed Work diagnostics;
  native tests distinguish work exhaustion from physical deadlines and separately
  prove bounded string-prefix allocation and genuine refusal with unchanged state.
- [x] H-CURRENT-WORK-CAPTURE — Complete construction and audited second-scan
  work/copy records retained. All snapshots have zero dropped/busy records. Caller
  attribution isolates 390,413 of 1,000,000 constructor work units in lifecycle-mark
  sorted insertion; its 32 work rows reconcile exactly to the original total.
- [ ] H-CURRENT-WORK — Implement admitted sparse lifecycle marks with unchanged
  identity/order, truthful binary-search/name comparison bounds, and transfer of
  completed function output payloads instead of cloning discarded originals.
  Preserve allocation/work admission, write/lifetime checks and transactional
  alias/copyback behavior. No allowance increase or function removal.
- [ ] H-CURRENT-STACK — Replace synchronous register-call recursion with shared
  suspended continuations, preserving nested distinct register execution, occupied
  recursion fallback, tier-1 and fault cleanup. The failing binary reserves 50,584
  bytes per register activation versus 39,240 in Run15; the admitted depth-33 test
  aborts. No stack-size workaround, depth reduction or disabling register execution.
- [x] H-CURRENT-HARNESS — Formatting, consumer locks and retained evidence transfer
  corrected. Preserve prior failures and the zero-selection log.
- [ ] H-CURRENT-PERFORMANCE — Current elapsed-time acceptance remains open.
  Instruction counts or fewer charged work units do not waive timing limits.

Next: complete these measured corrections, independent review, then one focused
native/portable/i686/runtime-vertical batch including the previously failing
OpenOT, OSCAT, PLCopen and depth cases. Do not repeat the full suite while known
causes remain. Measure performance and F401 size/RAM against the corrected source;
old board results do not qualify changed core bytes. H remains uncommitted.
Evidence: external `H-post-full-focused/analysis/work-charge-RCA.md`,
`H-post-full-attribution/`, and `H-next-full-register-call-pump-plan.md`.

**13 October: diagnostic complete; continue independent H software/UI validation.**
The renewed request to continue advances independent correctness checks; it does
not grant a latency exception. Six exact-binary Callgrind captures preserve
traces/counters/inputs and show fewer executed instructions than C21 (calls
-1,316,786; references -880,585). No new source-proven redundant operation
justifies another patch. This is not a latency pass. The existing ordinary timing
failures and current candidate-specific acceptance decision remain open.

Prepare and run the deduplicated full software/UI check set under the standing H
test authorization. This supersedes the preparation note's requirement to delay
all independent checks until performance acceptance. Keep a separate unresolved
performance row, and do not issue SOFTWARE_APPROVED, H-complete or commit-ready
receipts from software passes alone. Required tests and numeric/functional bounds
are unchanged; use source-bound hardware/supply evidence only for unchanged inputs.
No runtime source change, function removal, performance waiver, commit or push.
Evidence: external `H-current-performance-investigation/RESULTS.md` and
`reconciliation.json`; full-run harness is being prepared in `H-next-full-setup/`.


**13 October: resume the remaining performance investigation.**
The owner requested continuing the checklist. No new acceptance waiver is inferred.
Use the exact retained U, C21 and current hosted binaries for six bounded Callgrind
captures (call-binding and references, three binaries), with identical artifact
and runtime settings and collection restricted to execute-cycle. Compare native
execution traces/counters and per-function executed instructions; instrumented
latency is not a performance pass. No speculative source edit, rebuild, capability
change or firmware write is included. Plan and raw evidence: external
`H-current-performance-investigation/`. Full H acceptance remains open.


**13 October: current F401 replay passes after the measured compiler-config correction.**
`H-default-outliner/` passes 41 tooling tests, formatting, xtask lint/build,
firmware link/inspection and source stability. The extra outliner rerun is removed;
all runtime functions, overflow checks and safe ICF remain. ELF SHA256
`4bf713a57305453da8ac5acf026b6e04ce8e326a2916e27853cd6aa33889375c`.
Upper span 456,576 bytes; upper free 2,176; sector-0 free 832. The accepted
informational 16 KiB growth target reports false; actual partition bounds pass.

On the connected F401, backup/install verification and checkpoint/firmware
preservation pass. The unchanged Rust verifier passes the 101-sample trace,
numerical bits, GPIO/readback/safe-state behavior, task and depth fixtures, and
watchdog reset/DONE. Recorded heap peak 22,916 bytes; MSP/IRQ peak 14,184 bytes,
leaving 2,200 bytes (required minimum 2,048). Maximum sampled main scan 4,081 us.
Manual button transitions, optical observation and printed markings stay unverified;
C6 hardware and later qualification remain open. Keep the first failed HardFault
capture and its confirmed LR-loss diagnosis; do not rewrite the failure ledger.

The remaining focused batch confirms 425 portable-core tests, the owned-entry
regression on host and i686, LSP compilation/lint, architecture, regenerated
diagrams/drift and strict documentation. Its HMI selector mistakenly selected zero
library tests; this is not a pass. The corrected binary suite in
`H-lsp-owned-tests/` passes all nine expected HMI tests and source stability.
Preserve the raw zero-selection log. Evidence:
`H-remaining-focused/RESULTS.md` and `evidence.sha256`.

Next within H: finish the remaining risk-targeted assurance and the current
performance acceptance decision, then complete the deduplicated broad software/UI acceptance. Current
hosted latency failures remain recorded; the old C21 exception is not a blanket
waiver. No full H closure, commit or push. Evidence: external
`H-default-outliner/RESULTS.md`, `evidence.sha256`, `board/report.json`.


**13 October: bounded H assurance passes; dependency exception approved.**
`H-remaining-assurance-correction/` passes five admission tests on each of
x86_64 and i686, with identical stage outcomes for 768 fixed-seed mutations per
platform, formatting, Clippy and source stability. The first campaign retains
its unused-mut/formatting failures; 18 existing malformed-input, initializer-link
and executor/construction differential cases passed there and were not repeated.
No production assertion or behavior was changed to obtain the correction pass.

`H-critical-mutations/` passes the 517-test unmutated baseline; all four selected
function-removal mutants build and fail native assertions. No timeout or build
failure is counted as a caught mutant. Rollback, lifetime and staging refusals are
observed directly. Removing declaration-write checking is detected through
program-root protection and error precedence; this does not independently cover
every constant-write branch. Source-byte restoration passes. These are partial
A05/A07 results, not coverage-guided fuzzing, generated state-model coverage or
full H acceptance. Hardware-input comparison matches 329 relevant recorded
inputs; no new firmware behavior or reflash is introduced by these tests.

The owner approved the reviewed RUSTSEC-2024-0449 exception through 11 January
2027. `deny.toml` records ownership, rationale, review and removal criteria;
no parser implementation, protocol capability or lockfile is changed. The
root/firmware supply-chain gate and source stability pass in
`H-supply-disposition/`; preserve the advisory as maintenance risk, not a patched
dependency.

- [ ] RTP-MAINT-YAML — Separate dependency maintenance, owner runtime/opcua+mesh:
  before 11 January 2027, migrate both transitive YAML consumers to a maintained
  parser through reviewed upstream changes or a separately reviewed adapter.
  Preserve OPC UA configuration serialization/roundtrip/error behavior and Zenoh
  typed/include/endpoint configuration behavior with native compatibility tests.
  Prove both locked graphs no longer reach unsafe-libyaml and remove the exception;
  no automatic renewal. Decision/RCA: external `H-unsafe-libyaml/`.

**13 October: remaining hosted performance decision (pending).**
Source comparison confirms 1,340 runtime/core/config records still match the
selected frame-plan benchmark snapshot. Current executable SHA256
`434a11d17910a69b29cb991ad233072a05a1747a7ce10e97eaf1ac4d95738c03`.
The current call-binding paired median H/U ratio is 1.222887; H/C21 is 1.030083.
Four of five call-binding H/U median rounds fail 1.10. Reference workload H/U
round 2 also fails median at 1.447475. Keep every failed round, not only the
five-round summary. PLC trace/counter agreement and zero overruns do not waive
latency acceptance. C21's prior exception does not apply automatically to this
candidate. The owner has been asked whether to accept the measured current
trade-off or retain performance as a blocker. No new exception is applied pending
that choice. Read-only reconciliation: external `H-current-performance-decision.md`
and `H-current-performance-input-parity.json`. Broader H validation has a prepared
plan at external `H-next-full-setup/PLAN.md`; it is not launched under an assumed
waiver. Reuse passing evidence only when its relevant source/configuration/artifact
inputs match; preserve the full workspace, feature, protocol and UI acceptance
obligations. No commit or push.

**13 October: current board replay exposes an extra-outliner return-address defect.**
The approved margin-policy preflight passes 40 tooling tests, lint, formatting,
exact-ELF inspection and frozen-source identity. Firmware/application installation
and checkpoint/firmware byte-preservation checks pass on ST-Link
0671FF575755846687183960. The UART run fails after BOOT with HardFault; the Rust
trace verifier rejects it. This is retained failed hardware evidence, not H closure.

Register/SRAM/GDB captures identify INVSTATE and a corrupted return into heap.
The exact linked `OUTLINED_FUNCTION_135` calls a second-pass helper with BL
without preserving LR, then tail-branches to `read_exact::<1>`. The reader returns
to that same branch; a hardware breakpoint confirms LR 0x080723a5 and the second
entry's mutated length 255. Independent disassembly review confirms the lost
return address. Keep the Rust reader and diagnostic logic unchanged. Remove only
`-machine-outliner-reruns=1`, reject the override in the reference builder, retain
safe ICF/full functions/checks, then review, test the build policy, rebuild, inspect
and replay on the board. Native hosted execution is unchanged; MCU code generation
must be requalified. Existing test/rebuild/retry authorization applies. No commit.
Evidence: external `H-flash-margin-policy/board/`, `H-default-outliner/`.


**13 October: owner accepts informational flash growth margin; advance to hardware.**
The owner explicitly accepted that 16 KiB unused flash is optional code-growth
space and shall not block hardware bring-up, then requested moving on. Spec 34
and the ELF inspector now retain actual executable-region, sector/load, heap and
MSP checks while reporting the 16 KiB growth target without rejecting a fitting
image. This is an approved acceptance change, not a passing result under the old
rule. The selected frame-only correction passes 145 focused native executions,
formatting, both core lint modes, C6 and source identity. Upper span is 455,264;
upper free is 3,488; sector-0 free is 832 bytes. Its prior margin rejection and
hosted timing failures remain retained. Independent policy/harness review,
focused tooling tests, inspection of that exact ELF and real F401 replay are next.
The board preflight receipt proves focused hardware prerequisites only; broader H
software/performance/release gates remain open. No functions removed or commits.
Evidence: external `H-frame-plan-construction/`, `H-flash-margin-policy/`.


**13 October: grouped-plan candidate passes focused checks but grows flash; isolate it.**
The completed `H-index-plan-construction/` batch passes 145 native executions,
formatting, both core lint modes, C6 and frozen-source stability. The firmware
links at upper 456,640, an 864-byte regression from selected 455,776; reserve
inspection still fails. Paired speed records retain call-binding median failures
against U; no new waiver. The declaration merge adds emitted cursor helpers;
retain the candidate sources, maps, symbol delta and failed ledger, and isolate
only the direct frame-plan construction for the next focused measurement under
existing owner authorization. No selected footprint gain, hardware, full suite,
commit or push. The previously selected source remains the fallback.


**13 October: owner resumes correction; simplify cache construction, retain runtime speed.**
The explicit renewed instruction is to fix the regression from the former fitting
image. Continue within full-function constraints; the previous architecture-stop
paragraph is historical. Source comparison identifies frame/declaration cache
builders that rebuild owner lists and search already completed grouped indexes.
Derive the same cached ranges directly from those groups, preserving runtime
representation/lookups and all language behavior. No predicted savings. Complete
implementation and tests, independent review, focused validation, matched speed,
allocation and F401 size measurement before broader gates. The currently verified
candidate remains upper 455,776 / free 2,976 / reserve deficit 13,408; no hardware,
commit or push. Plan/baseline sources: external `H-index-plan-construction/`.


**13 October: architecture boundary after bounded compiler diagnosis.**
Extra outliner depths 2 and 3 each recover only 32 aligned bytes (upper 455,744),
still failing the reserve. Both diagnostics retain their failures; canonical
reruns=1 configuration and frozen source were restored and verified. The retained
source candidate remains upper 455,776 / free 2,976 / reserve deficit 13,408.
No task-owned run remains active. No full suite, hardware, commit or push.
The read-only audits found required physical-layout behavior, not a further
substantial duplicated emitted codec. A compact physical-layout representation
would cross into the MEM-06/10/11 storage design; the H goal explicitly stops
for architecture problems. Preserve the candidate and existing successful checks.
The concrete design/acceptance boundary is recorded externally in
`H-footprint-architecture-boundary.md`; it claims neither fit nor savings.
H is NOT complete. All function, validation, overflow and reserve constraints
remain. Do not resume a broad representation rewrite as a routine H fix.
Evidence: `H-split-output-shapes/`, `H-outliner-depth/`.


**13 October: full-function firmware links; reserve inspection still fails.**
Exact SPLIT output shapes and half-integer parity pass 63 focused native executions,
formatting, core lint, C6 and frozen-source stability. Upper span is 455,776 bytes,
a measured reduction of 4,192. Physical free space is 2,976 bytes, leaving a
13,408-byte deficit against the unchanged 16 KiB reserve. The ELF exists, but
inspection fails that reserve; no board run. Speed comparisons preserve PLC
traces/counters but retain latency failures. Do not substitute a physical link
for acceptance. Evidence: `H-split-output-shapes/RESULTS.md`, full failure ledger,
source identity and independent eight-file review. No function cuts or commits.
Continue measured investigation from this candidate; broad tests/hardware await
prerequisites. Required overlap/temporal semantics remain in place.


**13 October: immutable table packing recovers 3,200 upper bytes; fit remains open.**
All 68 targeted native executions, formatting, core/xtask lint, tooling build,
C6 and source stability pass. All 89 descriptors and eight families pass the
1,488-byte linker guard. Upper span is 459,968 bytes; lower read-only free space
is 864 bytes. Physical excess is 1,216 bytes; deficit with the reserve is 17,600.
Timing traces match, but individual latency gates still fail. No ELF or board
claim. Retain the full function set and reserve. Next remove the measured repeated
split-output indexing checks using exact safe slice patterns after the existing
binding validation, preserving first errors and one write per declared output.
Implement/review before focused tests and both speed/size assessment. No commit
or push; full H remains open. Evidence: `H-immutable-table-packing/RESULTS.md`.

**13 October: exact firmware predicates recover 2,720 bytes; immutable packing prepared.**
Nine focused native error cases, formatting and source stability pass. Upper flash
is 463,168 bytes: 4,416 bytes physical excess, 20,800 bytes including the reserve.
Link still fails; no ELF/hardware proof. Keep both exact deadline/latch predicates.
Next, preserve all 89 registry entries as named static arrays, shorten only source
root prefixes with distinct recorded namespaces, and select the complete descriptor,
decimal and compiler lookup tables for sector-0 placement. All relative filenames,
functions, bounds and overflow checks remain. Independently review, then run focused
registry/numeric/tooling tests, speed comparison and one size measurement. No saving
or fit is claimed from source alone. Full H remains unverified; no commit or push.
Evidence: `H-firmware-deadline-check/`; next plan: `H-immutable-table-packing/`.

**13 October: native payload drain verified; equivalent firmware assertions next.**
All 1,412 focused native executions pass, including exactly-one-allocation evidence.
The i686 main/numeric scan counts decrease from 1,162 / 1,320 to 1,002 / 880; GPIO stays 404.
Formatting, both core Clippy graphs, C6 and source stability pass. Upper flash is
465,888 bytes (+32); this is an allocation improvement, not a footprint win.
Timing comparisons retain failures and no new acceptance exception is claimed.
Reader-only object relocations identify 1,050 bytes of RuntimeError equality plus 18 bytes of Option
comparison code used solely by two board deadline assertions. Replace generic
comparison with exact RuntimeError::ExecutionTimeout patterns on both returned
and latched errors; preserve the full predicate and UART output. Pin the exact
conversion natively, correct spec 12's stale stable-code spelling to spec 10/current
contract, review, then run focused error tests and measure the firmware. Production
hosted execution is unchanged; previous timing failures remain open. Full H, board
qualification, commit and push remain open. Evidence: `H-native-payload-drain/`.

**13 October: remove measured native-call payload staging.**
Source and retained ARM code show two argument containers: temporary Vec<Value>
followed by Vec<VmNativeArg>. Drain the existing active-frame operand suffix into
only the final argument vector. Preserve receiver/descriptor order, first errors,
complete payload consumption and caller isolation; tests pin these behaviors.
No claimed size saving before measurement. Independent review precedes focused
native/portable/i686 and runtime vertical tests, allocation counters, same-candidate
release speed comparisons and F401 map measurement. Full H acceptance remains open.
Plan/evidence: `H-native-payload-drain/`. No broad suite, commit or push.

**13 October: controlled C10 diagnosis identifies and removes the growing change.**
Two isolated links held all other production input, compiler and target flags fixed.
C9 upper span: 465,888 bytes; checked reader alone: 465,856; validator checked-entry
reuse alone: 466,624; both (C10): 466,560. Thus the validator change adds 736 bytes
alone; the reader saves 32, and their interaction is -32. Both diagnostic links
still fail physical FLASH capacity; no ELF/hardware acceptance is claimed.
Restore the C9 validator production code, retain both new error-order tests and
the checked reader. Independent review confirms no unrelated changes are removed.
This matches the measured reader-only production variant; its reserve deficit is
23,488 bytes. The newly registered tests and selected combination still need focused
validation and same-candidate speed evidence; C10's timing failures remain open.
Full source checks passed before/after the diagnostic and inputs are retained in
`H-c10-size-isolation/`. The untested C11 proposal remains held externally.
No function cuts, overflow changes, lowered reserve, commit or push.

**13 October: C10 measured; stop speculative implementation and isolate the size regression.**
C10 passes 332 targeted native executions, lint and C6 compilation. Corrected
formatter invocation passes without source edits or repeated native tests.
Speed and size were measured on the same frozen candidate. Whole-image size
is 466,560 bytes, 672 bytes larger than C9; retain this failure. All PLC traces
match, but call/U median and some individual tail-latency comparisons fail.
The existing C21 exception is not relabeled as acceptance of C10.
The untested C11 proposal is held externally; active product source is restored
to the exact measured C10 candidate. Before selecting another edit, complete
two compiler-only component measurements (reader-only, entries-only) against
the retained C9/C10 controls. No assumed additive savings. Preserve each map,
freeze source per variant and restore C10 on exit. No broad tests or hardware.
Plans/results: `H-c10-size-isolation/PLAN.md`, `H-footprint-correction-10/RESULTS.md`.
The best measured image remains C9 at 465,888 bytes: 23,520 bytes short of the
16 KiB reserve requirement. Fit remains unproven; no function/check/reserve cuts,
commit or push. A later selected correction needs fresh speed and size assessment.

**13 October: C10 adds same-candidate speed assessment before size measurement.**
Object-backed decoder/validator reductions reuse checked slices and table entries;
new tests pin oversized-read cursor preservation and index-before-kind rejection.
Complete implementation and independent review precede the focused batch. Keep the
accepted scan benchmark source unchanged and replay U/C21/current on the three
frozen workloads; separately cover all eight float/text conversions. A small Rust
consumer measures source-free preparation/instantiation on identical STBC 2.0
artifacts. Run timing without concurrent builds, pin CPU/configuration, retain raw
samples and exact traces, and compare using the existing Rust gate. The prior
specific C21 exception is not a blanket waiver for the new source. Measure F401
size from the same frozen candidate regardless of a speed-comparison regression,
provided correctness passes. No physical speed/stack claim before ELF inspection
and board execution. C9 remains the last measured size: upper 465,888 bytes.
Plans/evidence: `H-footprint-correction-10/PLAN.md` and `PERFORMANCE-PLAN.md`.
No full workspace suite, commit or push.

**13 October: C9 focused checks pass and recover another 1,312 bytes.**
All 216 targeted native executions pass across hosted, portable x86_64, i686
and the runtime vertical. Formatting, both core library Clippy graphs, C6
compilation and source stability pass. Upper image span is 465,888 bytes:
7,136 bytes physical overflow and 23,520 bytes deficit with the required reserve.
The linker still rejects the image; inspection and hardware remain unrun.
Retain the reviewed borrowed-record correction and extra outliner pass, with
all functions, checks and reservations unchanged. Continue investigating emitted
redundant bounds checks using object/source attribution. No H commit or push.
Evidence: `H-footprint-correction-9/RESULTS.md`, frozen source, map and ledger.

**13 October: extra outliner pass saves 1,280 bytes; C9 removes decoder row copies.**
The reviewed extra pass reduces upper span from 468,480 to 467,200 bytes. Retain
it in the default firmware configuration, subject to hardware qualification.
The image is still 8,448 bytes over physical capacity, or 24,832 bytes over the
required reserve threshold. No hardware ran. Preserve `H-size-outliner-1/`.
C9 changes only the private fixed-record decoder: borrow each checked input row
instead of copying 8–60 bytes to an array before extracting fields. Existing
scalar-oracle tests pin values, errors, cursor and resource accounting. Complete
independent review, then the focused decoder/transport/vertical/MCU batch; use
the resulting map to measure impact. No predicted saving, function cuts or
weakened checks. Full H acceptance remains open; no commit or push.

**13 October: one additional compiler outliner pass prepared for measurement.**
The stable Rust 1.99 compiler exposes LLVM's `machine-outliner-reruns`; its default
is zero additional passes. Keep C8's fat-LTO source and all accepted settings,
and compare one extra pass after independent review. This is a firmware-only
compiler experiment, with no IEC function, admission check, overflow policy,
reservation or hardware-test change. Revert if it fails to reduce the linked span.
Retain all maps and require later hardware timing/stack qualification for any
accepted compiler change. The remaining BTree code occupies only 5,896 distinct
linked bytes; wholesale container replacement cannot close the 26,112-byte gap.
External plan: `H-size-outliner-1/PLAN.md`; no new size result yet.

**13 October: thin-LTO comparison rejected; source reduction continues.**
The independently reviewed compiler-only comparison grows the upper image from
468,480 to 503,936 bytes and crosses the sector-0 application boundary by 96 bytes.
Both linker assertions fail. Preserve the map and failed ledger; restore fat LTO
byte-for-byte on both machines. No hardware ran. C8 remains the accepted source
candidate: 9,728 bytes physical overflow, or 26,112 bytes including the required
16 KiB reserve. Full functionality, validation and overflow checks remain.
Continue investigating concrete remaining storage/decoder representation costs;
do not stop at this rejected experiment or treat speculative savings as evidence.
External evidence: `H-size-thin-lto/RESULTS.md` and `retry-1/ledger.tsv`.
No commit or push; full H acceptance remains open.

**13 October: C8 reviewed and focused-verified; controlled LTO comparison next.**
Owned validated-table transfer passes 1,441 core test executions across hosted,
portable 64-bit and i686 builds, both core Clippy checks, C6 compilation,
architecture, rendering and diagram drift. The runtime vertical's 28 tests pass.
Four older integration assertions were corrected against the existing contract:
structured budget causes with the stable timeout code, and explicit STBC 2.0
production for the retained 2.0 golden artifact. Both affected suites (52 tests)
and targeted Clippy pass on retry. Original failures and fixture bytes are retained.
C8's upper flash span is 468,480 bytes, saving 704 bytes. The remaining excess is
9,728 bytes physically, or 26,112 bytes including the required 16 KiB reserve.
Measured 32-bit preparation peaks are 21,101 / 18,277 / 10,736 bytes for main,
numeric and GPIO artifacts; this is neither physical RAM qualification nor a
measured causal comparison. No ELF or hardware proof exists for this candidate.
One controlled fat-versus-thin LTO comparison keeps all other source and build
settings fixed. Retain only an actual improvement. All functions, validation,
overflow checks and the reserve remain. No commit or push; full H acceptance is open.


**13 October: C7 measured; C8 owned preparation transfer implemented for review.**
C7 passes1,524 focused executions, bothClippy graphs/C6 and source stability.
Measured upper469184 B saves224 B; physical excess10432, reserve deficit26816.
The remaining copying redundancy is ordinary from_bytes cloning decoded table
payloads and then dropping their owner. C8 uses a private sealed validated source:
owned constructor performs full validation, borrowed token preserves existing
APIs, and one shared materializer transfers or clones payloads at the existing
ownership boundaries. No placeholder VM, public mutable token or unsafe code.
All preparation charges/checks/errors stay. Artifact-retaining hosted entrypoint
keeps its decoded sections/order. Add route parity, buffer transfer, exact fault/
usage tests and32-bit allocation measurements; independent review before focused
validation and the next firmware measurement. No promised saving or feature cuts,
no commit/push; continue to verified fit or owner-level architecture blocker.


**13 October: C7 fixed-record decoder authored for review.**
The C6 object has 3,420 B of construction decoding inside the 12,220 B section
parser. Fixed-width count checks prove complete rows before reservation, yet
scalar reads repeat EOF propagation per field. Read each construction/I/O/debug/
variable/retain/section-table row once into a fixed array and extract constant
wire offsets with compile-time bounds. Preserve count/byte/work checks, enum
order, optional sentinels, reserved fields and trailing-byte behavior. Frozen
scalar-reader test oracles compare exact values, faults, budget usage and cursor
where exposed, with every byte mutation, truncation and deterministic mixed data.
Review complete formatted sources, run focused native/lint/MCU checks then same
firmware measurement. No byte saving or fit promised before its map; full features,
validation, overflow checks and16KiB reserve unchanged. Continue until verified fit
or an owner-level architecture decision. No commit/push.


**13 October: initializer ownership gate closed.**
Move diagnostic-only metadata assembly out of executable initializer orchestration,
under the same std+test-support gate. Independent review and focused initializer
checks, all-feature Clippy, architecture-doctor --full-map, canonical diagram
render and drift checks pass. Copy back generated evidence. FULLMAP-CHECK-10 now
passes without exemption; existing advisory/partial doctor findings remain visible.
No MCU compiled code changes or new firmware measurement from this extraction.
Footprint reduction continues from C6; full H/hardware acceptance remains open.


**13 October: C6 passes focused checks; continue larger reductions.**
Checked fixed-width decoding and borrowed transient I/O metadata pass 1,477
native executions across hosted/portable64/i686/runtime vertical, both core
Clippy graphs and C6 compilation. Original formatter failure retained; the only
follow-up source delta wraps an import, independently reviewed before formatting
and linked measurement without duplicate native tests. Upper span469408 B:
672 B recovered, 10656 B physical overflow, 27040 B deficit including the required
16 KiB reserve. All functions, validation and overflow checks remain. No ELF or
hardware. External H-footprint-correction-6/RESULTS.md and both ledgers retain
source/map identity. Continue object-backed reductions; no commit/push.


**13 October: continue C6; optimizer comparison rejected.**
The opt-level=s experiment increased upper span from 470,080 to 575,104 bytes;
opt-level=z is restored byte-for-byte on both machines. No functionality changed.
C6 removes repeated per-byte reader bounds branches by converting each checked
slice to its fixed-size array, and borrows the raw I/O bindings during preparation
instead of cloning and immediately discarding them. All admission checks, budget
charges, errors and public ownership contracts remain. New reader tests cover all
widths, signed values and each truncated cursor boundary. Existing preparation,
source-free I/O and allocation tests pin the borrowing change. Independent review,
focused hosted/portable/i686 tests, Clippy, C6 target and one firmware measurement
follow; hardware requires successful link and inspection. Continue beyond this
checkpoint under the owner's existing retest authorization. No commit/push.


**13 October: C5 placement measured; one compiler size-policy comparison next.**
C5+placement upper470080 B, physical overflow11328 B, reserve deficit27712 B.
All functions and checks retained. Compare stable release opt-level s against z
once on the same source; all other profile flags/reservations stay fixed. This
current-H measurement does not rewrite historical B-R2 z-only evidence. Keep only
an actual size improvement and retain hardware/timing as unverified until tested.
In parallel, inspect emitted panic sites before selecting any source correction.
Do not stop at a size checkpoint; continue to verified fit or an owner-level
architecture blocker. No commit/push.


**13 October: C5 compatibility correction passes; continue to fit.**
Initial differential tests found Ryu ties-to-even versus Rust shortest-display
half-up spelling. Exact integer midpoint detection now changes only the lower
emitted decimal neighbor; upper-even ties remain untouched. Both failures,
adjacent values and both signs are pinned. Focused retry passes all five text
suites on hosted/portable/i686 plus Clippy and C6. Earlier runtime conversion/
assertion suites pass. No weakened oracle. Supply chain still reports the known
unsafe-libyaml advisory; no new-codec exception or hidden waiver.
First C5 map upper 481,760 B (1,184 B below C4). Old float table occupied sector0;
its removal leaves 12,160 B unused there. Repack measured immutable merged strings
(11,681 B) into that same permitted region, retaining all physical bounds/asserts.
This is placement recovery after a real total-image reduction, not a capability
cut. Review then link/inspect; no repeated native tests for linker placement.
Keep working beyond this checkpoint; fit/16KiB reserve, hardware and H remain open.


**13 October: owner requires continued execution until fit; C5 authored.**
Do not end the work after each small size result. C4 remains the measured baseline
(upper 482,944 B; 40,576 B reserve deficit). C5 targets the measured float/text
family without removing any conversion: lossless lexical compact parsing and
Ryu small-table formatting, with decimal-spelling compatibility on every platform.
No lossy mode or profile restriction. Both locks resolve only three new lexical
packages and the existing Ryu version on firmware. New native differential tests
cover all exponent boundaries, deterministic f64/f32 samples, syntax/long-input
rounding, all eight conversion names and assertion diagnostics. Review exact
formatted bytes before focused checks and same-configuration linked measurement.
Retain failures and continue root-cause work under existing retry authorization;
no H commit/push or hardware until its inspection prerequisites pass.



**13 October: footprint correction 4 passes focused checks and saves 1,408 B.**
Share the portable instruction decoder between ordinary POU and initializer
operand callbacks; hosted specialization remains. Exact checks/order preserved.
Independent review precedes one batch, no retry: hosted core 551, portable64 458,
i686 portable 457 and runtime vertical 28 passed (1,494 executions). Hosted and
portable Clippy, C6 warning-denied core check, formatting and 8,599-record source
stability pass. F401 link still fails: upper span 482,944 B, physical excess
24,192 B, reserve-preserving deficit 40,576 B. Total recovered since Run22:
19,520 B. All functions, admission, overflow checks and the 16 KiB reserve stay.
No ELF/hardware, full workspace suite, commit or completed-H claim. Existing
architecture/diagram and other H acceptance gaps remain open. External evidence:
H-footprint-correction-4/RESULTS.md, REVIEW.md, ledger.tsv and firmware/firmware.map.
Map SHA256 6ff5e768c23b79a451207c09d09833eba6fafb0b85391ece7168854871b49c02.
Runtime-error packing was investigated but not selected without layout evidence;
no error allocation, context loss or broad representation rewrite was introduced.


**13 October: footprint correction 4 implemented, awaiting review and measurement.**
C3 object proves two inline instruction-decoder specializations for POU and
initializer operand callbacks. Share the private no_std decoder through a
borrowed callback; retain hosted specialization. Framing, work/count charges,
operand-before-append checks and final jump checks stay in order. New native
tests pin refusal at each callback, earlier rejection precedence, truncation
and work cutoffs. No functions, checks, diagnostics or ABI removed. No byte
saving inferred from overlapping DWARF ranges; measure after focused validation.
External H-footprint-correction-4 records the plan, review and failure ledger.
Runtime-error packing remains unselected: retained objects have no type-layout
DIEs, so no layout or savings claim supports a change. Existing H acceptance,
architecture and hardware gaps remain open; no full workspace suite or commit.


**13 October: footprint correction 3 measured; fit remains open.**
Portable SIZEOF now shares its private recursive traversal; hosted specialization,
all functions, both ABIs, accounting and overflow checks remain. Focused evidence:
11 hosted and 11 portable size/temporal tests, 38 runtime vertical/temporal tests,
and 453 i686 register tests pass across the reviewed original/retry snapshots.
Portable/register Clippy and C6 core check pass. Corrected the owning C2 predicate
mistake (test-only map queries leaked into the register production graph), plus
one formatting difference. Retain both ledgers. The first measurement invocation
was rejected before compilation because its output path already existed; the
fresh-path link reached the linker and failed only the remaining size bound.
Upper span 484,352 B: another 1,056 B recovered, 18,112 B below Run22. Physical
excess 25,600 B; reserve-preserving deficit 41,984 B. No functions removed, no
hardware, full workspace suite or commit. Final 8,598-record source matches.
Map SHA256 fa1a0d1977f203801043a720cc6d99a4d9f835716ba4fc435ff26f230c3ed93c.
Evidence: external H-footprint-correction-3/RESULTS.md, original/retry ledgers,
reviews and measurement/. Current small duplication corrections have not closed
the gap. Further substantial representation work needs architecture design;
no safe large saving is established by the remaining map investigation.


**13 October: footprint correction 3 in implementation.**
Continue reduction from C2 (upper 485,408 B; 43,040 B above the reserve ceiling).
The linked map contains two 980-byte SIZEOF recursive walkers specialized on
accounting callbacks. Share the private portable traversal; retain hosted
specialization. Preserve every callback charge, traversal order, ABI and error.
Add exact located BOOL-alias/temporal visit traces and refusal at every callback
boundary for both ABIs. Reader-sharing was rejected: the retained object already
shares primitive decoder readers. No saving assumed, no capabilities removed.
Independent review precedes focused core/temporal/register and runtime vertical
checks, lint/cross check and same-configuration F401 measurement. Existing full
architecture/diagram and H acceptance failures remain open. No full workspace
suite, commit, push or hardware launch.


**13 October: footprint correction 2 reduced flash again; full-function fit remains open.**
Four portable POU tables share sparse-ID/ordinal indexes plus reserved payloads;
logical capacity, replacement, sorted iteration and clone/equality behavior are
pinned. All runtime functions, validation, overflow checks and hosted APIs remain.
Independent review caught allocator-capacity versus logical-limit confusion before
execution; it was fixed with native overcapacity/clone tests. The first batch then
caught unused query methods in the default portable feature graph. Exact test/
register predicates fixed the owning compile boundary; targeted retry passes.
Candidate evidence:540 hosted core,447 portable64,446 i686 portable,442 i686
register,38 hosted temporal/runtime vertical and3 allocator tests pass (1,916
executions across configurations). Hosted/portable Clippy and C6 core check pass;
8,598 frozen source records match. Full architecture still fails the pre-existing
203-line initializer owner/split check; diagram refresh remains unrun.
Final upper flash span485,408 B, a further4,992 B saved and17,056 B below Run22.
It still exceeds physical capacity by26,656 B and the margin ceiling by43,040 B.
Matched i686 preparation peaks drop by488/552/360 B for main/numeric/GPIO;
preparation allocations rise by4/2/2, while scan/instantiation counts are unchanged.
These are requested payloads, not MCU RAM/stack/timing proof. No full workspace
suite, firmware ELF, hardware execution, commit or push. Keep every failed ledger.
Evidence: external `H-footprint-correction-2/RESULTS.md`, independent reviews and
`rerun-1/` logs/map/source manifest. Map SHA256
`1846a1454f1f698cd4ab91daec1ac792e4d920d5b897a7d5ba9d6d5aee27afee`.


**13 October: footprint correction 2 authorized and in implementation.**
Owner says "continue reduce" after the architecture checkpoint. Four portable
POU metadata indexes share an ID/ordinal tree representation with reserved typed
payload vectors, instead of specializing tree nodes on each large value type.
Sparse IDs never index vectors directly. Preserve ID-order iteration, replacement,
semantic equality, hosted HashMap APIs and every runtime function/check. The
portable read-only POU lookup facade is an intentional internal Rust surface
change, recorded in spec34; no artifact/PLC contract changes. Account index and
payload storage plus the class-count pass before allocation, and check native
32-bit allocation demand. Required focused batch: hosted/portable core tests,
i686 portable/register execution, runtime vertical and temporal compatibility,
core lint, full architecture and dependent diagram refresh, same-config F401
link/inspection. No full workspace suite, hardware or commits. Keep correction1
as measured baseline: upper span490,400 B; reserve-preserving target442,368 B.
No predicted saving or fit claim. Independent review precedes the batch.



**13 October: footprint correction 1 implemented/reviewed; focused tests pass, fit still fails.**
Shared dynamic accounting callbacks remove duplicated compiled layout traversal;
validated POU materialization no longer builds a redundant temporary ID/name map.
All functions, retained runtime indexes, ABI checks, error propagation and firmware
overflow checks remain. Only the removed temporary allocation's budget reservation
was removed. New native tests cover every callback failure boundary and validated
POU identity/name materialization. Existing portable test feature predicate fixed.
Independent review and one focused builder batch: core hosted 512 passed, core
portable 416 passed, hosted temporal/runtime vertical 38 passed; core Clippy and
8,597-record source stability pass. F401 link fails; no ELF/hardware/full suite.
Upper span falls from 502,464 to 490,400 bytes: 12,064 bytes recovered. Remaining
physical overflow is 31,648 bytes; 48,032 bytes must still be recovered to preserve
the mandatory 16 KiB margin. This is progress, not fit or H closeout. No commit/push.
External evidence: `H-footprint-correction-1/RESULTS.md`, `REVIEW.md`, logs, map,
frozen source and failure ledger under the H artifact root. Map SHA256
`4ed9addba80d0178fff4f628f1ca31c266604fad24cbbae96fa052f966f423c2`.
The next representation change requires architecture design/review; a blanket
BTree replacement is not supported by the measured remaining family sizes.


**13 October: owner authorized correction of the measured H footprint regression.**
Preserve all overlap/temporal semantics, functions, validation, overflow checks and
16 KiB free upper flash. The measured layout walk is emitted separately for
validation and materialization callback types; first correction shares that
implementation through borrowed dynamic accounting callbacks, preserving every
charge and failure. Review materialization indexes against actual consumers before
changing storage. No blanket replacements or speculative feature restrictions.
Prepare the complete selected correction and independent review before focused
correctness, lint and linked-size evidence; defer broad acceptance until these
pass. Earlier owner authorization permits necessary rebuilds/retests, and requires
root-cause analysis of each failure. No H commit/push. Measured baseline is
H22 upper span 502,464 bytes; target with reserve is at most 442,368 bytes.



**12 October: owner-requested footprint investigation measured the missing H16 checkpoint.**
One isolated size-only builder link (22 seconds; exit 101 due flash overflow) used
frozen H16 plus three recorded compile-only corrections. All 8,569 source records
matched before/after. H15 upper span 439,904 bytes; corrected H16 495,936; H22
502,464. Thus 56,032 bytes of the 62,560-byte regression precede the later speed
work; all subsequent changes add a net 6,528 bytes. H16 already exceeded physical
capacity by 37,184 bytes. The main new footprint is physical overlap/layout and
module/bytecode materialization; maps contain no temporary profiling symbols.
Same compiler, flags, firmware source and normalized dependency/features graph.
This diagnoses the growth; it does not validate H16 semantics or fix H22. No
active runtime code changed, no full tests or hardware, no commit/push. Preserve
full functionality, overflow checks and the 16 KiB margin. A performance-only
revert cannot recover fit. Detailed evidence and hashes:
`/home/johannes/projects/.artifacts/runtime-portability-h/H-footprint-investigation/RESULTS.md`.
Current H still needs 60,096 bytes recovered to satisfy the existing margin.
The earlier paragraphs below are historical checkpoints superseded by this
intermediate measurement where they say the split was unknown.


**12 October: corrected size-regression baseline after owner challenge.**
H itself already fitted in Run15: upper span 439,904 bytes, 18,848 bytes free,
passing the 16 KiB floor; ELF `8c3b907709760221fba5191ed8bb3f4c294b42e95d9f2691fd1881ad0622dfa4`.
Run22 needs 62,560 bytes (61.09375 KiB) more upper flash than that H build. The
58-package/features graph, target/profile and Rust flags match. Use this fitting
H checkpoint as the primary regression baseline, not only the older U image.
Later work contains both correctness/layout changes (new overlap representation
already present in the Run16 snapshot) and subsequent speed optimizations.
No linked intermediate measurement yet attributes their separate contributions.
Investigate that delta before further optimization/redesign; preserve full
functionality and the achieved flash budget. No new build or code change was
performed for this source/map history correction; Run22 remains failed.


**12 October: Run22 full acceptance failed; F401 footprint is the architectural blocker.**
The C21 hosted call-performance trade-off is approved and remains narrowly recorded
in §14.4; it does not waive MCU size/timing/correctness. Run22 finished on frozen
source `0c2061e030cea8f84ff6800ce9167ce8f5dbe3658885772c00ff1cb88be0f123`
(8,597 records). Required ledger: 40 passed, 8 failed, 12 dependent unrun;
advisory metadata passed. Source/harness stability passed. No hardware was flashed,
no commit/push, and no post-batch test/build has been launched.

Six owning findings explain the failed rows:
1. Three LSP `DebugSnapshot` literals omitted the new `storage_abi` field; this
   prevents workspace/product compilation and lint. Prepared correction takes the
   ABI from the already-available metadata at each source-built snapshot.
2. One owned-binding unit test calls `execute_sync` without its required
   HIR/register feature predicate. Prepared correction uses that exact predicate;
   assertions and production code are unchanged. Register/i686 lanes passed.
3. F401 link exceeds the upper region by 43,712 bytes (42.6875 KiB). Compared with
   fitting U at the same Rust 1.99/features/flags/layout, code grows 63,584 bytes
   and constants grow 3,644 bytes, totaling 67,228 bytes (65.6523 KiB). Recovering
   the unchanged 16 KiB margin requires 60,096 bytes (58.6875 KiB) reduction.
   Static RAM and sector-0 use are unchanged. No ELF or physical proof exists.
4. Supply chain rejects unmaintained `unsafe-libyaml` (RUSTSEC-2024-0449), reached
   through serde_yaml from OPC UA and Zenoh. No new exception or dependency patch
   has been made; maintenance disposition remains open.
5. Full architecture scan requires an owner/split disposition for the 203-line
   `evaluate_initializer` coordinator. No waiver or source split has been made.
6. The new spec link to an internal note breaks when MkDocs relocates the spec.
   Prepared correction retains a plain repository-path reference.

The three snapshot, test-gate and documentation corrections have independent
source review; they are not yet compiled/tested. All other failures remain open.
No further footprint redesign or validation launches precede the required
architectural decision/review. Existing H retest authorization is retained; this
stop is the goal's architecture condition, not a request for routine retry approval.

The MCU core artifact has `features: []`; std/test-support diagnostic logging is
excluded and its symbols are absent from the map. Firmware UART/panic/watchdog
trace source is unchanged from the fitting implementation. New physical overlap/
layout handling occupies 23,944 mapped bytes; materialization/engine families
also grow. Family ranges overlap/inlining differs: do not turn them into promised
savings or remove functionality. Exact analysis and failed raw evidence:
external `run-22-full/analysis/firmware-footprint.md`, `analysis/lsp-snapshot.md`,
`software/ledger.tsv`, and the frozen source archive. C6 and native Windows/macOS
proof remain explicitly unrun. H is not ready for commit.


**12 October: C21 hosted performance trade-off approved; full H acceptance authorized.**
The owner approved the recommendation to accept this specific measured call
regression and proceed. Spec 34 §14.4 and the H execution record identify C21's
candidate/analysis hashes and exact exception: calls median 1.164359 remains a
failed general 1.10 target, accepted for this candidate/workload; no future blanket
waiver. All functions, safety checks, p99/trace requirements, firmware margin,
physical timing and hardware resource checks stay unchanged.
H21's Linux comparison is dispositioned by that exception. H/H22 remain open.

Prepare and independently review `run-22-complete-setup`, then run the required
full H software acceptance and conditional connected-F401 replay. Reuse the
frozen C21 comparison only with recorded production equivalence; do not repeat
optimization or relabel its performance result as a pass. Fix the known portable
lib-test helper cfg at its HIR-only compatibility boundary before freezing.
Keep separate Cargo targets for external consumers and firmware. Reconcile
current source, instructions, firmware/build configuration and UI prerequisites
before the batch. Existing H authorization permits necessary root-cause fixes and
retests. No commit or push. Native OS/C6 limitations remain explicit.


**Historical C21 result before the approved trade-off: correctness passes, call performance failed.**
The single activation registry is implemented with reservation/activation, owner,
suspended-local, monotonic identity, debugger and failure-cleanup invariants intact.
Independent source review is complete. C20's two bookkeeping failures are corrected
and retained: a new fixture had four registry records but expected work for three;
a now-unused private std method needed a no_std API gate. C21 core tests pass:
490 all-feature, 438 portable-register, 437 i686. Four core feature lints, host lint,
native/Windows warning checks and all frozen source identities pass. Unchanged
C20 lifecycle (32), hosted VM (206), services (4), vertical (33), F401/C6 compilation, architecture
and canonical diagrams/drift pass. Record size: 48 bytes host, 32 bytes i686.

Five alternating release rounds preserve exact traces, VM counters, artifacts and
zero overruns. Paired C21/U median/p99: calls 1.164359/1.096911 (median FAIL),
references 0.968249/0.580123 PASS, strings 1.024509/1.151306 PASS. Limits stay 1.10/1.25.
Calls improve versus C19 in each paired round (median ratio 0.989317), but this does
not close H21. Exact-binary instruction count falls 87,928,761 → 86,980,917 (1.078%).
No functions or safety checks removed. No full suite, commit or push. H is not
ready for commit. Do not launch broad acceptance while focused performance fails.
Source manifest `0634546b24c21178587259f7465dadc7ca5e81ff99a033dbebe010b142d1862f`;
candidate `390bfda03a584e7320b4aba0b7c69b2b69f5afd5afd52f655c80f9f4de592704`.
Evidence: external `H-root-correction-20/` failed ledger and
`H-root-correction-21/RESULTS.md`, `REVIEW.md`, `POST-REVIEW.md`, `validation/`.
The current task's redesign review and focused testing are complete; remaining
call-path design work must use the exact profile before any further optimization.

**12 October: C20 focused batch retained; two corrections before measurement.**
Independent review found no runtime source blocker. C20 exercised the unified
registry on native/portable/i686 builds. One new test incorrectly expected a
non-tail retirement charge of 3 for 4 reserved records (3 active); preserve the
3-active assertion and correct storage charge to 4. Clippy/warnings found the
now-unused std-only private `LiveEntries::contains_key`; preserve that API on
its public no_std `InstanceMap` alias and omit the unreachable std method.
Core lifecycle, F401/C6 compilation, hosted VM/services/vertical, portable lints,
architecture and diagram render/drift pass. Release timing was correctly skipped.
Record sizes are 48 bytes x86_64 and 32 bytes i686. No runtime production behavior
failed an assertion. C20 raw ledger remains in external `H-root-correction-20/`.
C21 validates the corrected assertion and feature boundary, then measures the
unchanged production redesign against U/C19; existing retest authorization applies.
No full suite, commit or push.

**12 October: owner authorized redesign review and testing; C20 implementation.**
Authorization: "review the redesign and test it". C19 remains the comparison
baseline; the 1.10/1.25 limits and full functionality remain unchanged.
- [x] H21-RCA-15: consolidate activation liveness, ownership and suspended values
  in one execution-frame registry; preserve reserved versus active distinction,
  ownerless initializer/temp scopes, monotonic IDs, legacy frames and cleanup.
- [x] H21-RCA-16: migrate lifetime/read/write/debug/return consumers to that one
  authority, with exact stale-reference and nested ancestor regressions.
- [x] Review/freeze complete change, then focused C20/C21 core/all-feature/portable/
  i686/MCU, hosted call/debug/vertical, feature lint, architecture/diagrams and
  conditional ordinary-release U/C19 comparison. No full suite, commit or push.
Independent design review accepted the record model. Spec34 HOST-04 now defines
actual unified reservation and metadata work charges, including candidate-only
allocator failure ordering; no shadow charges for removed arrays and no safety
check removal. Preparation capacity may grow; report actual sizes. C20 evidence
will be external `H-root-correction-20/`.


**12 October: architectural decision point; H is not ready for commit.**
C19 is the latest measured candidate. Focused correctness and feature checks pass;
call median is 1.191671 times U against the unchanged 1.10 limit. References,
strings and call p99 pass. No functions, guards or acceptance thresholds were
removed. No full suite, commit or push has run.

The independent post-C19 review reconciles the native-call instruction delta:
nested execution +10,314,384; completion +2,088,669; preparation -4,437,975;
other direct work +32,973, totaling +7,998,051. Parent/child inclusive costs are
not added. Frame initialization, tier-1 checked execution and frame-result
retirement/cleanup are the concrete remaining branches. Current evidence does
not prove their entire cost unavoidable, nor promise a safe local fix.

H21-RCA-13 descriptor design was reviewed but not implemented: estimated metadata
slice under 1% of the profile, with added prepared storage/constructor interface.
H21-RCA-14 exact-binary investigation confirms 496 bytes of control-header moves
per POU handoff, but only 171,000 profiled memcpy instructions (under 0.2% total).
A stable-address holder would add preparation allocation for little demonstrated
benefit; not implemented. Required cleanup and check order remain unchanged.
Both investigations are complete; neither closes H21.

Per the H goal's explicit architecture stop condition, stop code/test launches
before redesigning call/frame execution or changing the performance contract.
The owner decision is whether to pursue a separately designed call-lifecycle
optimization preserving all safety checks, or explicitly accept/revise the
measured performance criterion. No permission to weaken it is inferred. A new
architecture must be reviewable before implementation; no speculative tiny
optimization loop. Latest code is verified only to the focused C17/C18/C19 chain;
full H acceptance and H22 remain open. Retained analysis:
external `H-root-correction-19/REMAINING-RCA.md` and `RESULTS.md`.


**Remaining root-cause investigation after C19 (no further runtime edits yet):**
- [x] H21-RCA-13: review the measured direct-scalar initializer path. A prepared
  descriptor could remove metadata lookup, but the reviewed design adds a sidecar
  and constructor hint for less than 1% of current instruction cost. No default
  payload cache or check removal is authorized; implementation is deferred pending
  a justified benefit and simpler shape. DATE/DT defaults remain profile-dependent.
- [x] H21-RCA-14: inspect exact-binary copies of `RegisterExecutionBuffers` on
  nested take/return/recycle. Separate payload clearing (required) from movement
  of the owning control structure. Compare the U representation, constructor-time
  reservation, failure rollback and public compatibility before any change.
  A stable-address representation is only a candidate; no saving is assumed.

**12 October: C19 focused correctness passes; call performance remains open.**
All feature-specific lint and binding checks pass (18 each all-feature/portable/
i686; 57 hosted). C17/C18 retain the unchanged native/vertical, MCU,
native+Windows warning and architecture/drift evidence. Five ordinary release
rounds preserve every watched trace, counter and source/artifact identity, with
zero overruns. Paired median/p99: calls 1.191671/1.047282 (median FAIL), references
0.962044/0.578278 PASS, strings 1.006027/1.084153 PASS. Call medians U 5.348 us,
C15 6.421 us, C19 6.369 us. Source manifest
`7183e27b7cbab1b923c927bd2d572eb42fbb057dab063f36da8f69465df01984`;
candidate `63138fb3308cdf11209ff3b7da01ca5cdc3b90fd8572ee8a1f4e7bc82a6c3699`.
Exact-binary profile records 87,928,761 instructions. Hosted deadline checks
remain 59,909; the absent-deadline path falls from 27 to 11 self instructions
per call. C16's unsuccessful specialization is reverted; its tests remain.
H21-RCA-11/12 pass focused validation; H21 itself does not pass. Further work
must follow measured remaining costs, preserve all safety/phase charges and avoid
another speculative local optimization. Full acceptance and H22 remain unrun.
Evidence: external `H-root-correction-19/RESULTS.md`, `validation/`, `profile/`.
No full suite, commit or push.

**12 October: C18 native and hosted lint pass; portable lint/format corrections prepared.**
VM tests pass in all-feature (257), portable (243) and i686 (243) shapes;
host services (4) and vertical/restart (33) pass. All-feature core and hosted
library Clippy pass. Portable Clippy finds the now-unused borrowed-call adapter:
its only public export is already HIR-gated, while shared production execution
uses the owned binder. Gate the borrowed wrapper/impl/function with `hir || test`,
preserving HIR compatibility and unit assertions without changing MCU behavior.
Formatting found three older diagnostic-expression wraps in `vm/context.rs`;
builder rustfmt corrected them. Independent review confirms both changes.
Retain the C18 failed ledger and C17 passing cross-target/architecture evidence.
C19 reruns affected call-binding tests in hosted/portable/32-bit shapes,
feature-specific lint and formatting, then conditional release U/C15 timing.
No full suite, commit or push; H21 remains open.

**12 October: C17 native/cross-target checks pass; host lint blocks timing.**
Core all-feature/no-default/i686, lifecycle, hosted VM/services/vertical, MCU,
core Clippy, host+Windows warning checks, architecture and drift pass. Host-library
Clippy rejects two redundant tail `return` statements in the non-test-support
branches of `vm/context.rs` and `vm/engine/context.rs`. Earlier all-feature lint
excluded those branches. This is a feature-matrix lint gap, not a PLC assertion
failure. C17 source stayed frozen throughout; release timing did not run.
The failed ledger remains external `H-root-correction-17/validation/`.

**C18 correction:** remove only those tail returns; mutually exclusive cfg blocks
retain identical results and refusal order. Independently reviewed. Reuse C17's
unchanged lifecycle/hosted and target evidence; rerun core VM tests in diagnostic,
portable and i686 shapes, host-service/vertical tests, both core feature lint
shapes and hosted library lint/format, then conditional ordinary release U/C15
comparison. No full suite or commit/push. H21 remains open.

**12 October: C16 correctness passes; measured optimization rejected.**
Call medians U 5.388 us, C15 6.400 us, C16 6.579 us; paired median 1.219376
FAIL, p99 1.220510 PASS. Reference/string comparisons pass but are also slower
than C15. Exact traces/counters hold. All focused gates pass (481 all-feature,
429 portable, 428 i686, 32 lifecycle, 206 hosted VM, 33 vertical tests).
The lower instruction count (88,475,893 versus 89,036,325) is not a latency gain.
H21-RCA-10 is rejected as a performance remedy. Restore its production wrapper
to C15 while retaining the three new regression tests. Evidence:
external `H-root-correction-16/RESULTS.md`, `validation/`, `profile/`.

**C17 measured correction scope:**
- [x] H21-RCA-11: absent hosted deadlines return before unused pause arithmetic;
  retain every explicit and stride deadline check, active deadline pause handling,
  suspend/restore behavior and overflow fallback. C15 measures 59,909 calls with
  27 self instructions each, no clock syscall. This inherited source work became
  more frequent under H's required checks; removing checks is not authorized.
- [x] H21-RCA-12: construct Overflow only on refused checked insertion-demand sum;
  preserve exact growth/search/move charges. Measured success-path temporary drop
  cost: 5,760 calls, 57,600 instructions, in frame initialization.
- [ ] Independent review, then focused core/portable/32-bit/MCU, hosted service,
  VM/vertical, lint/architecture and ordinary release comparison against U/C15.
  No full suite or commit while PERF-01 remains open.

**12 October: C15 focused correctness passes; call median remains open.**
All 21 ledger steps pass, with 478 all-feature core, 427 portable, 426 i686,
32 lifecycle, 206 hosted VM and 33 vertical/restart test executions. Exact saved
traces, counters and identities agree in all five ordinary release rounds.
Paired median/p99 ratios: calls 1.193344/1.139898 (median FAIL), references
0.965389/0.549491 PASS, strings 0.991881/1.029849 PASS. Calls measure U 5.379 us,
C14 6.570 us and C15 6.419 us. Frozen source manifest
`7e507ac47ed297edcde94946a48942381bfaa49c709ea204e1752c7713436b9b`.
C15 exact-binary Callgrind: 89,036,325 instructions. H21-RCA-8/9 are verified;
overall H21 is not. External evidence: `H-root-correction-15/`.

**C16 measured correction scope:**
- [ ] H21-RCA-10: keep top-level shallow value-copy admission local to its caller,
  retaining one work/deadline charge followed by the exact allocation charge.
  C15 still executes 33,033 tier-1 estimator calls (1,420,419 self instructions)
  through an outlined `Result<usize, RuntimeError>` boundary for scalar success.
  Recursive owned-value accounting and depth limits remain unchanged. Diagnostic
  and ordinary builds must execute the same admission logic.
- [ ] Independently review, freeze, then run focused native/portable/32-bit/MCU,
  hosted VM/vertical, Clippy and architecture checks and conditional ordinary
  release comparison. No full suite, commit or push while PERF-01 is open.

**12 October: C14 correctness passes; call median still fails PERF-01.**
All focused core/portable/32-bit, hosted/lifecycle/vertical, MCU, Clippy,
architecture and drift steps pass. Five release rounds retain exact U traces and
counters. Paired median/p99: calls 1.229937/1.098501 (median FAIL), references
0.937589/0.574579 PASS, strings 1.017903/0.801122 PASS. Calls measure U 5.358 us,
C13 6.780 us, C14 6.590 us. Exact binary profile: 91,209,629 instructions.
Frozen manifest `ed79479b92b3b0204acda24b8a45217b846800cdcf8aebb5edf66d4c584be1ac`.
No full suite, commit or push. Evidence: external `H-root-correction-14/`.

**C15 measured correction scope (verified by C15; PERF-01 remains open):**
- [x] H21-RCA-8: construct unit-variant errors only on actual refusal at the
  measured frame reservation/suspension/resumption, typed-entry lookup, frame
  commit, buffer selection and scalar-canonical lookup sites. Keep guards,
  first-error identity/order and all budget/deadline behavior unchanged. Exact
  C13 disassembly and C14 caller counts show eager errors dropped on success.
- [x] H21-RCA-9: reset returned frame locals to NULL in place, retaining length,
  capacity, backing address and destruction order. C14 measures 9,216 generic
  NULL-clone calls from clear-plus-resize; change only this measured reset method.
- [ ] Independent review, then focused native/portable/32-bit/MCU and hosted
  checks, lint/architecture, followed by conditional ordinary release comparison.
  Preserve the failures, original baseline and thresholds; no full suite or commit.

**C14 corrections (verified; overall PERF-01 open):**
- [x] H21-RCA-6: return an already exact non-text primitive from the checked
  assignment boundary without passing it through two additional 40-byte argument
  temporaries. C11/C13 disassembly and executed Callgrind addresses confirm that
  path. Retain shared primitive matching, all charges, text/coercion paths, alias
  depth and subrange rejection. No savings claimed until ordinary measurement.
- [x] H21-RCA-7: reuse the immutable return-slot fact read during argument binding
  in private owned call continuation. Keep the public `BoundVmCall` struct and
  borrowed API unchanged, and retain missing-POU/parameter fault order.
- [ ] Independently review complete code/tests, freeze, then run focused native,
  portable/32-bit/MCU, hosted VM/vertical, lint and architecture checks. Conditional
  ordinary-release U/C13/C14 comparison uses the same artifacts and thresholds.
  No full workspace suite, commit or push.

**12 October: C13 focused checks pass; call performance remains above the limit.**
468 all-feature, 417 portable and 416 i686 core tests pass, plus Clippy/format.
C12's 271 native lifecycle/hosted/vertical and MCU/architecture/diagram passes
remain applicable to identical production bytes. All five release rounds retain
exact U traces/counters and zero budget overruns. Calls: median ratio 1.261753,
p99 ratio 1.282695, both fail. References (0.975105/0.728073) and strings
(1.022163/1.073369) pass. Candidate `41edc8a161948d807bb389240c7f154f1170bc93ddb28ed13b3faafcdc7fd299`;
frozen manifest `05c7e4e0d1725402f6d670f63d9dff0d5a75ad787dd30fe6f4a577f1eb925ec2`.
Exact-binary Callgrind: 92,936,115 instructions; C11: 94,797,767.
Evidence is external `H-root-correction-13/RESULTS.md`, `validation/`, `profile/`.
No full suite, commit or push; H21 remains open.

**12 October: C12 retained test-compilation failure; C13 focused retry ready.**
The new sparse-ID test compared its u32 wire slot to a usize test slot. Core unit
binaries and Clippy stopped at that compile error; no assertion in those binaries
ran. C12 passes all 271 executed lifecycle/hosted/vertical tests, both MCU core
checks, formatting, architecture and diagrams. C13 fixes only the comparison
width, reuses those passes, and reruns blocked native/core/32-bit and Clippy checks.
Release timing is conditional on correctness. No production change, full suite,
commit or push. Failed ledger retained as external `H-root-correction-12/`.

**12 October: C11 focused correctness passes; call-performance criterion remains open.**
C10 retained compilation/Clippy failures in new test scaffolding and a shared-target
artifact collision, not PLC assertion failures. C11 corrects the fixture match,
keeps the old action lookup test-only, and rebuilds only the collided generated
`trust-wasm-analysis` artifacts. External Cargo roots now use separate targets.
C11 passes 462 all-feature core, 411 no-default core, 410 i686, 32 lifecycle,
206 hosted VM and 33 hosted/vertical test executions, plus core Clippy and format.
C10's unchanged-production F401/C6 checks, architecture and diagram proof are
retained. The three H21-RCA corrections below are implemented and tested; they do
not close H21. Five alternating ordinary-release rounds preserve exact U traces
and counters, with no budget overruns. Paired median/p99 ratios: calls
1.286963/1.244972, references 1.014761/0.584717, strings 1.053929/1.169904.
Calls still exceed the 1.10 median limit. Source manifest:
`07ed1118517910042b00940de550c18b8ab5447e4ef92a7dd40f4f29b8010a33`.
Candidate SHA256: `77a2e89f693af67af9b89613192b794e2421ced58189181e4a06419a0f0f2f0d`.
Exact-binary Callgrind records 94,797,767 instructions versus C8's 97,610,596.
Entry-local clone calls fall by 13,824 (12 per invocation), and journal insertions
fall from 2,304 to 1,152. These counts prove those operations disappeared; they do
not assign all latency changes to them. Next: correct measured immutable
owner/declaration lookup overhead and investigate scalar normalization before
selecting any further change. No full suite, commit or push. Evidence: external
`H-root-correction-10/RESULTS.md`, `H-root-correction-11/RESULTS.md`, `validation/`
and `profile/`. Standing focused-retest authorization applies.

**C12 corrections (verified by C12+C13; overall PERF-01 open):**
- [x] H21-RCA-4: prepare declaration owner ranges once, preserving sparse wire IDs,
  slot holes, physical method ownership and runtime fault/charge boundaries. C11
  attributes about 1.95 million self instructions to declaration lookup.
- [x] H21-RCA-5: leaf lifetime checks retain their one-unit charge without building
  a recursive traversal context. C11 attributes about 1.78 million self
  instructions to the recursive lifetime helper across callers. Savings are not
  inferred from these totals; retained recursive behavior is tested against the
  original helper, including budget/deadline refusal and stale references.
- [ ] Review completed implementation and tests, freeze, then run core native,
  no-default/i686, MCU graph checks, hosted VM/vertical, lint and architecture.
  Only after correctness passes, compare ordinary release U/C11/C12 on the same
  saved traces. No full suite until PERF-01 passes. No commit or push.
Scalar normalization investigation found no redundant deep copy: keep its checked
assignment boundaries. Register move accounting is also correct; constant/storage
materialization is a separate operation. Do not delete these admission calls.

**12 October: focused root-cause diagnostic passes; per-call costs are now observed.**
C9 passes two recorder tests, core library Clippy, formatting, diagnostic consumer
build, all source identities and saved-artifact replay with exact original-U
trace equality. For each of 1,024 recorded cycles: 20 initialization phase queries
include 12 empty queries, four parameter queries visiting 12 supplied/skipped
actions, and four return actions; entry copies four NULL and eight INT values.
Register reads are 32 moves and zero clones per cycle, with no fallback or deopt.
All recorder overflow/busy/active/unbalanced/error counts are zero. Independent
raw-profile arithmetic also attributes the completion increase to returned-buffer
recycling, transactional journal/write checks and preflight, offset partly by
reduced destruction/freeing. U's corresponding policy/transaction hooks were
no-ops. Preserve those safety contracts; correct ownership transfer and repeated
immutable lookup work rather than deleting checks. C9 adds diagnostics only;
C8's ordinary release call-performance failure remains open. No full suite,
commit or push. Evidence: external `H-root-cause-9/RESULTS.md`, `validation/`,
and `H-root-correction-8/PROFILE-PARTITION.md`. Source manifest:
`7bd3843fc98a46754e2852bd8f4354813b73cf93abdb2574705b7ebdd71522dd`.

**Corrections justified by C9 and verified by C11 (overall PERF-01 remains open):**
- [x] **H21-RCA-1 — Resolve frame actions during preparation.** Avoid repeated
  immutable owner/phase searches at call entry; retain declaration order,
  argument-presence including explicit NULL, return defaults, static/restart
  behavior, work/deadline fault ordering and preparation-memory admission.
  Native assertions must cover empty phases, supplied/omitted arguments,
  explicit return stages, restart and failed initialization.
- [x] **H21-RCA-2 — Transfer owned call locals at selected execution entry.**
  Keep borrowed APIs checked-copy compatible, return the original owned payload
  intact on every optimized fallback, and preserve cleanup/recycling and
  output/reference lifetime checks. Assert alias independence, explicit NULL,
  defaults, output copyback, failure/unwind and repeated invocation reuse.
- [x] **H21-RCA-3 — Review completion storage movement with rollback preserved.**
  C8 attributes overhead to buffer recycling and temporary-to-active journal
  transfer. This is not permission to remove preflight/commit checks, mutate
  fault order or weaken rollback. Implement only a reviewed ownership/collection
  improvement with specific equivalence evidence; otherwise record it as required
  cost. Any speedup remains unverified until ordinary release measurement.

**12 October: remaining H call regression under instrumented root-cause investigation.**
Owner explicitly resumed investigation after correction 8. Ordinary production
semantics remain unchanged. Retained C8/U profiles have identical workload counts
and 97,610,596 versus 78,473,206 executed instructions; inclusive costs must not
be added as independent phases. Call preparation/binding is already cheaper than
U. Frame initialization, reference policy and register read accounting remain
measured costs. Existing evidence cannot distinguish empty initializer phase
queries from supplied-argument skips, so a bounded, opt-in std+test-support
recorder will capture that distinction. Planned focused diagnostic batch: logger
unit assertions, touched formatting, core library Clippy, source-bound diagnostic
consumer build and one saved call-binding replay with trace comparison. No full
suite or ordinary release remeasurement is needed for count-only attribution.
Keep instrumented elapsed time separate from PERF-01 acceptance. Evidence belongs
outside the tree in `runtime-portability-h/H-root-cause-9/`.

**12 October: correction-8 focused checks pass; call performance still fails.**
The exact marker-refusal test, core all-feature/all-target Clippy, formatting,
frozen-source checks and ordinary release consumer build pass. C7's 450 core
and 15 portable call passes and C6's 266 lifecycle/hosted/vertical passes remain
applicable to unchanged sources. Five alternating rounds retain exact traces and
execution counters with no budget overruns. Paired candidate/U median ratios:
calls 1.293801, references 0.96, strings 1.02; call p99 is about 1.27 and also
exceeds 1.25. References and strings pass this comparison. H21 remains open.
Profile the exact C8 binary before selecting further corrections; no full suite,
commit or push. Evidence: external `H-root-correction-8/RESULTS.md` and
`validation/`; source manifest
`1eda5307f4f50f59841dde36f7d632bcd5fef2f18a02b85516ccbaa48a9d557a`.
The 28-category assurance plan below is planned work, not executed evidence.

**12 October: correction-7 native tests pass; correction-8 focused lint retry ready.**
450 core tests and 15 no_std call tests pass, in addition to correction6's 266
passing lifecycle/hosted/vertical tests. Clippy found only explicit drop(state)
in the marker-refusal test; EngineState has no destructor. Removing that call
uses ordinary non-lexical borrow lifetimes, with every assertion and production
path unchanged. Independent review agrees. C8 runs the exact affected test,
core Clippy and formatting, then release performance if clear; passing suites
and diagrams are reused. The owner's additional assurance request is recorded
as 28 planned categories below; no mutation/fuzz/soak campaign has run here.

**12 October: correction-6 fixture compilation failure retained; correction-7 ready.**
266 native tests passed (32 lifecycle, 206 hosted VM, 28 vertical), as did format,
architecture and diagrams. New edge fixture wrongly assigned String to SmolStr;
no_std compilation caught the same expression. Core unit tests did not execute.
The unrun owned-binding fixture also declared INT while asserting DINT. Clippy
found a non-const TLS initializer in the existing diagnostic logger. All three
are corrected and independently reviewed; no production assertion is relaxed.
Correction-7 reruns blocked core/no_std/Clippy checks, reuses the 266 passes and
diagrams on unchanged ordinary production, then builds/measures release if clear.
Standing owner authorization applies. No full suite, commit or push.

**12 October: owner resumed root-cause correction; correction-6 in progress.**
The prior architecture stop did not establish that the remaining regression was
unavoidable. Continue investigation and focused correction under the owner's
instruction to find and fix it. C5 profile/source comparison proves two uppercase
allocations per root edge transaction (2,304 per 1,152 cycles), a POU-ID/name/ID
round-trip; use the existing ID with compatibility fallback. Private function
argument binding also copies six uniquely owned expression values per fixture
cycle before dropping their payloads. Independent review confirms Frame-only
initialized markers have no semantic consumer; local visibility uses activation
and ordinal rules. Remove that unused persistent bookkeeping, preserving Global/
Instance marker refusal coverage and all lifetime/name/slot checks. The register
read-count setup additionally constructs and drops a discarded error per successful
slot lookup; use a direct checked branch with the same failure. HOST-04 records the
ownership and initialization contracts before implementation. Author native tests,
review complete source/callers, then focused correctness and ordinary release
comparison. No full suite, commit or push; preserve every prior ledger.

**12 October: tested corrections retained; architectural call-performance issue open.**
Root-correction-5 is complete as focused evidence (706 passing tests). H is not
commit-ready: call_binding median H/U remains 1.36650 against 1.10. Reference and
string comparisons pass. Independent call/write reviews distinguish required
constant/staging/lifetime/copy admission from the original hosted no-op hooks;
no safe removal of those checks is established. Next work needs a reviewed shared
call/write plan that reuses admitted facts while preserving dynamic checks and
fault/work ordering. Six owned expression copies per cycle are a smaller proven
ownership opportunity, with no isolated timing saving claimed. No additional
adapter, runtime edit or full-suite run is launched. Preserve this tested source
at the owner's architectural stop boundary; no commit or push. Focused retry
permission remains valid and is not the blocker. Detailed evidence and next design
constraints: external `runtime-portability-h/H-root-correction-5/REMAINING-CALL-PATH.md`.

**12 October: correction-5 passes 706 native tests; call performance remains open.**
All focused correctness, formatting, architecture and portable compilation steps
pass on source manifest `67713aea9fe49bc91efe2c462664bb3688ba5b12af70e5a2d67968c910737020`.
The five-round ordinary release comparison preserves all traces and execution
counters. Candidate/U median ratios: calls 1.36650 (fails), references 1.09549
(passes), strings 1.07167 (passes); all paired p99 median ratios pass. Overall
PERF-01 is still failed. The fresh exact-binary profile records 101,380,562
instructions versus correction-3's 109,181,194. Continue measured call-path
investigation under standing authorization; no full suite, commit or push.
Evidence: external `runtime-portability-h/H-root-correction-5/RESULTS.md`,
`validation/` and `profile/`. The failed correction-4 compile ledger is retained.

**12 October: root-correction-4 failed compilation; correction-5 prepared.**
One declaration lookup retained the old tuple constructor after the packed-key
change. Review missed that consumer. Two test bindings also became unused.
All source hashes matched; formatting and architecture passed; native tests and
performance remain unrun for this change. The three sites are corrected and all
index/type-table callers are being re-reviewed. Repeat the focused batch under
standing authorization; retain correction-4 logs. No full suite or commits.

**12 October: root-correction-4 reviewed; focused batch starting.**
Dense reference-type slots and order-preserving declaration/action keys replace
measured hash and compound-key lookup costs. Source-only independent review found
no blocker; formatting is complete. Tests cover ordinal alignment, absent metadata,
duplicate/bounds rejection, preparation charging and key-order boundary parity.
Run core library/lifecycle, portable compile, hosted VM and runtime vertical,
formatting and architecture, then ordinary release comparison only if all pass.
Standing owner authorization covers focused retests; no full suite or commits.
Evidence: external `runtime-portability-h/H-root-correction-4/`.

**12 October: root-correction-3 correctness passed; performance remains failed.**
The one corrected fixture passed, closing 390 focused tests across corrections
2/3. Full source and consumer identities passed around the release build; all
three PLC traces and exported execution counters match U. Candidate/U paired
median ratios are 1.45/1.15/1.13 (calls/references/strings), above 1.10; call p99
is 1.28, above 1.25. Candidate/previous-H medians are 0.96/1.02/0.95. The comparison
runner succeeded but PERF-01 did not. Evidence is external
`runtime-portability-h/H-root-correction-3/RESULTS.md`, `validation/` and `profile/`.
Current-binary Callgrind recorded 109,181,194 Ir versus previous-H 112,895,749;
instruction counts are not elapsed time. Further authorized source corrections
target measured immutable lookup costs: dense reference-type indexing instead of
hashing REF_TABLE ordinals, and losslessly ordered integer keys for declaration/
action groups. Preparation allocations stay charged, runtime checks and charge
points stay intact, and no new mutable cache or feature reduction is authorized.
Author tests, review, then focused validation/normal-release measurement. No commit
or push. Preserve all prior failures and do not infer full acceptance from this work.

**12 October: root-correction-3 authorized; focused corrections/retests may continue.**
The owner approved the one corrected test and release comparison, then instructed
"approved, dont ask, just fix this shit". Continue evidence-led corrections and
focused retries within this performance task without repeating permission requests;
preserve failed runs and source identities. No functionality removal, weakened
assertions, commit or push. Root-correction-3 is running with manifest
`cac3b2f1e5a3731e92651d51fe03abedb9efe14ee5523ba0d56d18dcb903af1d`.
It reuses correction-2's 389 passing tests and checks on unchanged production,
and runs the one allocation-fixture regression before the release comparison.

**12 October: authorized root-correction-2 completed with one fixture failure.**
230 core unit, 27 lifecycle, 104 hosted register and 28 runtime-vertical tests
passed (389 total); one newly authored allocation-refusal fixture failed because
it did not ensure the already-prepared marker collection needed growth. Portable
core, formatting, rendering/manifest refresh and architecture passed. Source hash
`a765b2ba2cd335c788cb6367958ba5833bfe9633618bc7a3e3447468d70fada8`
matched before/after. No release build/timing ran. Generated diagrams were copied
back. After the batch, only that test was corrected: isolate empty marker storage
and assert positive allocation demand, retaining all refusal/publication assertions.
Production code is unchanged from the passing cases. The fixture correction was
independently reviewed and remains unrun. A one-test retry followed by normal
release comparison is prepared under `H-root-correction-3/`; explicit approval
was requested under the failed-batch rule and is pending. Do not rerun the 389
passing tests or the other passed gates for this fixture-only change. Evidence:
`runtime-portability-h/H-root-correction-2/RESULTS.md` and `validation/ledger.tsv`.
No full suite, commit or push. H21/PERF-01 remains unverified/failed historically.

**12 October: root-correction-1 batch failed at compilation; no timing run.**
The 3,052-file frozen manifest matched before/after execution. Formatting passed.
E0433 exposed a missing ExecutionLimitKind import in the new budget test; E0502
exposed the Tier-1 dynamic-store reference borrow overlapping a mutable counted
value read. No native tests executed. Architecture checks passed except three
pre-existing stale diagram hashes; local/builder diagram bytes match. Performance
was skipped. Ledger, exact logs and source identity are retained externally in
`runtime-portability-h/H-root-correction-1/validation/`, summarized in `RESULTS.md`.
After the batch, the import and dynamic-store sequence were corrected using the
existing interpreter's owned/count-aware reference-then-value reads. Alias/null
regressions and independent correction review are being completed. Those changes
remain unverified; no rerun, full suite, commit or push is claimed. H21 stays open.

**12 October: measured-overhead corrections authorized and in progress.**
The owner instructed implementation of the diagnosed root causes. Scope: the
three corrections in `FIX-DESIGN.md`, plus reuse of prepared last-use read counts
in Tier-1 only after source proof covers fused operands, live-outs and errors.
During implementation, marker-search reuse was deferred: holding a borrowed map
entry conflicts with whole-EngineState charging. The attempted token substitute
added runtime guards/address identity and was removed before validation. Existing
marker storage and accounting remain unchanged; no saving is attributed to them.
No feature reductions, raised limits or relaxed assertions. Root owns zero-unit
accounting; host/register agents own declaration bookkeeping and operand movement;
the integration agent independently reviews the complete correction.
Finish implementation/tests and review before a focused builder batch: core
register/engine/budget assertions and affected source-free lifecycle suites,
portable core check, formatting, required runtime vertical, then normal-release
U/previous-H/candidate comparisons of the same three saved workloads. Verify all
crate/build-input hashes before and after builds, not selective changed-file sync.
Use the mounted-volume Cargo target and TMPDIR; a disk/workload preflight precedes
the batch. Timing runs do not overlap compilation. Full acceptance remains separate
and follows only if focused correctness and performance justify it. No commit or
push is authorized. The previous investigation checkpoint below is historical.

**12 October: source/spec investigation complete; no optimization implemented.**
The caller evidence was reviewed against HOST-04, MEM-06 and the STBC execution
contract. Three narrow corrections have a concrete source justification: skip
zero-unit EngineState charges; reuse the declaration-marker search through
reservation and commit without changing its charge/fault order; and omit the
unused owned-name clone for frame declarations while retaining name-index
validation. Independent read-only review found no design blocker under those
constraints. The external design and focused proof matrix are
`runtime-portability-h/run-17-focused/rootcause-instructions/FIX-DESIGN.md`.
No runtime edits, tests, builds or benchmarks ran in this investigation.
No latency saving or sufficient performance recovery is claimed. Nonzero copy,
type/lifetime and fused source-step checks remain; preparation-cache and Tier-1
last-use proposals require further equivalence justification. H21/PERF-01 stays
failed/open. The next implementation should follow the narrow design, then
focused correctness and normal-release measurements before broad acceptance;
the current investigation has not executed those steps.

**12 October: source-matched caller logging complete; no performance fix applied.**
The corrected v3 diagnostic verified all 2,515 crate/build-input files against the
reviewed local tree before and after compilation (manifest SHA-256
`033295b861db4294e186e955517c519ff4e77d19651ac880e262132e88c5487e`).
Three saved workloads ran for 128 warmup plus 1,024 measured cycles. Their PLC
traces, artifact identities, executed-program/block counts and zero fallbacks
match U. Logger overflow/busy/refusal counts are zero. Warmup compilation-counter
placement differs as required by HOST-04 (U first-use compilation, H preparation).

For `call_binding`, every one of the 344 work-charge calls per measured cycle is
attributed to its source site. Against U's 54 calls, the 290 additional calls are:
96 copy-accounting visits/path charges; 67 assignment/type/lifetime charges;
42 declaration/owner/reference lookups; 40 initialization charges; 30 frame,
cycle and output-transaction charges; and 15 extra charge boundaries where H
preserves individual source-instruction faults inside fused operations. These are
accounting event counts, not exclusive latency shares or proof that the underlying
operation is absent in U. H's 89 copy-admission calls charge 3,560 logical bytes;
their values are scalars/null/empty-path references, not 3,560 bytes of measured
heap allocation. There is one preflight per checked-register copy, not a proven
duplicate preflight. Twenty-two charge calls per cycle request zero units; source
inspection confirms these leave EngineState's allowance/stride unchanged and do
not poll the deadline. Their isolated timing contribution is unmeasured.

The other workload totals reproduce the prior measurements: `refs_sizeof` has
201 work/29 copy calls per cycle, and `string_stdlib` has 238/91. The raw caller
rows and source-matched receipt are external under
`runtime-portability-h/branch-diagnostic-consumer/callsite-accounting-v3/` and
`CALLSITE-ACCOUNTING-V3.md`. The owning source/spec reconciliation is under
`run-17-focused/rootcause-instructions/ROOT-CAUSE.md` and
`CALLSITE-RECONCILIATION.md`; an independent read-only review confirmed the
arithmetic and evidence limits. Two focused logger tests passed; no full
suite or optimization was run in this diagnostic step. H21/PERF-01 remains failed.

**Evidence correction:** selective file synchronization had omitted the caller
wrapper and left four rejected H-P1 files on the builder. The initial caller
diagnostics v1/v2 and H-P2 candidate comparison therefore cannot establish the
claimed isolated local-source effects. They remain retained with invalidation
notices. Root restored the four known task-owned files, then checked the entire
crate input tree before v3. The frozen original U/H binary profiles are unaffected.
Future source receipts must cover the complete build inputs, including reverted
files, rather than only the newest edits.

**12 October: operation-level debugging required; no further optimization.**
The owner rejected selecting remedies from aggregate hotspots. The next evidence
must identify exact callers, value kinds and charged operations, explain why H
performs them compared with U, and distinguish specification-required work from
demonstrated redundant work. Use the retained unchanged-binary profiles first;
add only bounded, count-only diagnostic events for unresolved facts. Account for
the observed work-charge/copy totals and retain exact PLC trace parity. No inline
or other performance candidate, full suite, commit or push is authorized by this
diagnostic step. Record uncertainties explicitly rather than proposing a remedy
to discover whether it helps.

**12 October: checked-read accounting candidate measured and rejected.** The
subsequent full-source audit invalidated this run's isolated-candidate claim:
the builder also contained four rejected H-P1 files (see correction above). The
following numbers describe that mixed-source run, not the inline attribute alone.
The
candidate removed the intended out-of-line helper call, but five alternating
uninstrumented comparisons showed no consistent benefit: candidate/previous-H
median per-round p50 ratios were 1.00 (`call_binding`), 1.06 (`refs_sizeof`) and
1.01 (`string_stdlib`). All 45 reports retained identical PLC traces, profiles
and input identities. Host `.text` grew 736 bytes. Root removed the inline
attribute; production code in `engine/allocation.rs` again matches the prechange
snapshot exactly. Two useful accounting boundary tests remain. Thirteen focused
tests and formatting passed on the candidate; no post-reversion run is claimed.
The runtime vertical did not execute: compilation reported three `Serialize`
trait-identity errors at the web-analysis boundary. Existing derives and a single
locked serde version were confirmed; shared-target artifact identity is under
read-only investigation. No web/dependency code was changed to mask that failure.
The candidate receipt, codegen and raw measurements are retained under external
`runtime-portability-h/H-P2-accounting/RESULTS.md`. H21/PERF-01 remains failed;
this mixed-source run cannot establish the isolated effect of helper-call removal.
No full suite, commit, push or MCU qualification was performed.

**12 October: isolated checked-read accounting candidate authorized.** After the
diagnostic findings, the owner instructed implementation, focused tests and an
uninstrumented comparison. The measured register-read edge calls the general
`value_clone_charge` helper 36,384 times, including 32,973 simple-value cases and
3,411 reference-path cases. The candidate exposes the existing bounded helper to
inlining so the constant root depth can specialize without duplicating the
simple/owned classification. Recursive backing accounting, work/allocation
amounts, deadline cadence, fault precedence and copy semantics stay unchanged.
No last-use/move draft is revived. Author boundary assertions and obtain an
independent review, then run focused tests, inspect generated code and compare
uninstrumented U/previous-H/candidate on the saved workloads. Retain a candidate
only with supporting evidence; do not infer a benefit from the inline attribute.
Evidence is external under `runtime-portability-h/H-P2-accounting/`. The earlier
diagnosis-only checkpoint below is historical. No full suite, commit or push is
part of this isolated step; H21 and full acceptance remain open.

**12 October: diagnosis only until a specific cause is measured; optimization edits stopped.**
The owner clarified: do not guess a solution; log and establish the root cause
first. This supersedes proceeding from a broad hotspot to an optimization
candidate. H-P1 was rejected and reverted. The untested H-P2 draft was archived
and its three files restored to their pre-draft bytes; no optimization candidate
remains applied. No task-owned build or benchmark is running. The immediate work
is read-only analysis of the original/current profiles and targeted diagnostics
that distinguish specific operations and caller counts. Do not introduce another
optimization based only on a slow aggregate phase or a plausible source pattern.

**Diagnostic checkpoint, 12 October:** unchanged U/H release binaries were
profiled at instruction addresses on the same saved `call_binding` artifact.
H executes 112,895,749 instructions against U's 78,473,206; these are instruction
counts, not elapsed-time shares. The H action-key searches account for 3,290,112
instructions (24 search iterations per frame). Matched Tier-1 operand reads
perform 36,384 clones in both versions: the clone subtrees cost approximately
1.28 million instructions each, while H's checked-read wrapper plus clone costs
4,653,741, including 1,847,625 in copy preflight. H register-result completion
also includes activation retirement, return checks and payload recycling that
are absent from U's result-builder boundary. Parent/child inclusive totals must
not be added or presented as like-for-like elapsed-time deltas.

A separately reviewed, count-only diagnostic confirms the actual initializer
branch: over 1,024 measured cycles after 128 warmup cycles, all 4,096 evaluations
are zero-code `INT` return defaults using `direct_scalar_intrinsic_fast_path`.
Initializer 174 accounts for 3,072 calls and 178 for 1,024. No staged initializer
frame or initializer bytecode body is selected. The fixed-capacity log has zero
overflow/busy events; its complete 1,152-cycle PLC trace equals both original
U and prior H traces. Seven focused collector tests pass. The debug diagnostic
build establishes branch/count evidence only; its timings are not acceptance
evidence. No optimization was applied and no full suite was run.

Evidence: external `runtime-portability-h/run-17-focused/rootcause-instructions/`
contains `ANALYSIS.md`, `COPY-COMPLETION-ANALYSIS.md` and
`RETURN-DEFAULT-U-H-COMPARISON.md` with raw profile/source references. External
`runtime-portability-h/branch-diagnostic-consumer/` contains the raw branch
report, test/build logs, frozen source hashes and `RESULTS.md`. These establish
specific additional execution paths; they do not yet attribute every nanosecond
of the whole-cycle regression or demonstrate a successful remedy. H21/PERF-01
remain open. Do not revive the rejected optimization drafts from these findings.

The previously selected sequence below is retained for after diagnosis, not as
permission to implement an unproven remedy.
The owner requested this sequence be recorded and executed using cheaper
subagents. Investigate independently where useful; implement and measure one
owning cause at a time. Focused regression tests and focused reruns are authorized
before the required full acceptance suite. Preserve the full function set, all
checks, phase/fault ordering, PERF-01 limits and the firmware margin. No H commit
or push is authorized.

- [ ] **H-P1 — Frame initialization:** review and implement the smallest safe
  reuse of the existing prepared action index. Preserve all four phase charge
  points, phase/trigger selection, supplied parameters/returns and action order.
  Run focused index/initialization/lifecycle regressions, then compare the
  uninstrumented candidate with the frozen previous H and original U on the same
  saved workloads. Retain the change only if evidence supports its benefit.
  **Attempt 1 rejected and reverted:** the independently reviewed owner-start
  candidate passed 36 focused tests and built without diagnostic probes. Its
  five-round candidate/previous-H median p50 ratios were 1.02 (calls), 0.97
  (references, variable), and 1.02 (strings), with identical traces. No consistent
  improvement was established. Only the four candidate files were restored, with
  hashes matching their saved pre-change bytes. This also removes the candidate's
  unused test-oracle warning. Frame-initialization cost remains open; no larger
  initialization redesign is authorized by this result. Evidence:
  `/home/johannes/projects/.artifacts/runtime-portability-h/H-P1/RESULTS.md`.
- [ ] **H-P2 — Copy accounting:** identify added/repeated calculations in
  register read/materialization paths. Correct the owning duplication, preserving
  allocation/work limits and fault boundaries; run focused regressions and measure.
  **Draft archived and reverted; diagnosis pending:** Tier-1 clones register operands on every read
  although prepared per-block use counts already let the interpreter move a
  last-use value. Forward those same counts into Tier-1; retain clone preflight
  before each actual clone. Review aliasing, live-out values, selection fallback,
  early errors and read order. This removes copies rather than disabling required
  accounting. This proposed change was not run and is not approved for continued
  implementation without the required diagnosis. Benefit is unmeasured. The separate owned-string-element copy
  candidate does not explain these workloads and is not part of this change.
- [ ] **H-P3 — Function completion:** reassess the matched completion cost after
  P1/P2, then correct demonstrated avoidable work with focused proof.
- [ ] **H-P4 — Budget/deadline reassessment:** distinguish consequences of P1/P2
  from independent repeated work. Preserve required checks; do not reduce checks
  or change thresholds to pass performance.
- [ ] **H-P5 — Final acceptance:** once focused correctness and uninstrumented
  PERF-01 comparisons pass, obtain review of the complete final diff and run the
  required full H validation. Report any architectural decision blocker rather
  than silently changing the contract. Failed/unrun gates remain open.

For every item retain source identity, root-cause finding, isolated diff, review,
focused command/results, performance/trace comparison and disposition in the
existing external evidence tree. A failed candidate measurement is evidence,
not permission to relabel the candidate as an improvement. The H-only timing
and paired diagnostic results below remain historical evidence. H is not yet
verified or ready for commit.

**12 October: paired old/new timing diagnostics complete; H21 remains failed.**
U at `19f3b6787ef1e2d8fd434851684c463baab645e9` and H at the same base were
measured with the same diagnostic collector, saved performance-4 STBC1.1
artifacts and CPU 2. Three workloads × six phase modes × five alternating pairs
produced 90 comparisons and 180 valid reports. All pairs preserved identical
artifact/workload identities and exact PLC traces; each report has 128 warmup
and 1,024 measured cycles, zero budget overruns and valid balanced spans. The
root-only `HostedCycle` H−U means were +3.74 µs/cycle for `call_binding`
(range +0.34 to +4.97), +1.04 µs for `refs_sizeof` (−0.31 to +3.09), and
+2.21 µs for `string_stdlib` (+1.33 to +3.09). This control still contains the
collector and root timing span.

Paired phase counts show additional H budget/accounting checks: for
`call_binding`, copy-accounting calls were 12→89, work charges 54→344,
deadline checks 12→43 and assignment normalizations stayed at 29; the
corresponding work-charge counts were 58→201 / deadline 3→24 / normalization
19→19 for `refs_sizeof`, and 61→238 / 4→22 / 16→16 for `string_stdlib`.
Source review attributes CopyAccounting additions to register-IR clone-size
estimation and allocation charging, and WorkCharge additions to scalar/recursive
clone-size estimates, normalization, reference-lifetime checks and stores;
deadline additions come from extra charges and explicit boundary checks. The
phase totals do not identify exact per-callsite contributions.
However, policy-mode time is strongly observer-dominated: selected probes
raise the mean of per-run `call_binding` cycle p50s from 10.596 µs in root-only
control to 44.739 µs. The mean HostedCycle phase span rises from 10.775 to
46.289 µs (+35.513 µs/cycle) with probes. Its timed WorkCharge/DeadlineCheck spans include
per-span clock and collector cost, so their measured time is not a product
regression estimate. Equal-count call-mode scopes show H FrameInitialize
self-time increases of 1.22 µs/cycle for `call_binding` (5 calls each; range
1.18–1.34 µs), 73 ns for `refs_sizeof` (1 call; 63–81 ns) and 60 ns for
`string_stdlib` (1 call; 37–73 ns). `call_binding` CallComplete increases by
351 ns/cycle (4 calls each; 281–434 ns). The references phase recorded zero
reference helper calls in these three programs, so it provides no evidence
about their reference-operation cost. Results and ranges are diagnostic; the
full paired report and machine audit are at
`/home/johannes/projects/.artifacts/runtime-portability-h/paired-timing-consumers/RESULTS.md`
and its `results/` directory. This supersedes the H-only inference about where
the measured difference lies, but it does not pass PERF-01 or H21.

The owner explicitly changed cadence: run targeted small tests first to catch
remaining bugs and integration mistakes, then the required full tests. This
supersedes the one-batch-only restriction for this correction validation sequence.
The focused source corrections and their applicable native regressions now pass.
The runtime unit run had 3,951 passes and three obsolete register-fixture failures;
the corrected register subset then passed all 104 tests. Core all-features passed
409 unit plus 22 source-free tests; the corrected i686 lane passed 369 unit plus
21 source-free tests. RETAIN integrity (13), ADS worker/cycle cases (17), the
affected Clippy checks, and the full-map architecture checks pass. These are
focused results on their recorded source identities, not a green full-H run.

H21 remains failed: the latest same-CPU release comparison preserves all watched
PLC traces but has H/U median ratios 1.541 (calls), 1.175 (strings), and 1.129
(references), above the unchanged 1.10 limit. The call p99 ratio is 1.262 versus
the 1.25 limit. The independent instruction-profile review found no further small
accidental duplication that explains the remaining gap. No broader invocation or
frame-action plan implementation is started. Its benefit is not established by
the current measurements and must not be assumed.

The earlier H-only timing/logging record below is historical and is superseded
by the paired evidence above; it was measured in a diagnostic `std`/`test-support`
build. Its three collector tests and core Clippy pass. All three
workloads completed 1,024 measured cycles with valid spans and unchanged saved PLC
traces. In the call workload, execution accounts for 63.5% of instrumented root
time, call preparation/inline builtins 15.3%, and frame plus initializer work
13.9%. Empty-span overhead is 70–84 ns; instrumented timings are diagnostic and
do not replace PERF-01. The frozen 8,579-record source passed stability checking.
Its H-only phase attribution must not select a redesign. Full H acceptance, current
hardware/browser evidence and commit readiness remain unverified. No full Run17,
commit or push was launched. Evidence: external `run-17-focused/FOCUSED-CLOSEOUT.md`
and `run-17-focused/PERFORMANCE-REVIEW.md`; timing evidence and interpretation are
in `run-17-focused/NATIVE-TIMING-RESULTS.md` and `NATIVE-TIMING-REVIEW.md`.
This checkpoint edit follows the diagnostic source freeze; runtime bytes are
unchanged. The draft full harness retains its
pending source/review markers; it must not be launched from this checkpoint.

## Historical execution checkpoints

The dated checkpoints below describe their own source and evidence only. They
are superseded for current action/status by the checkpoint above.


**12 October: full-review source corrections prepared; validation remains stopped.**
Three GPT-6-luna agents implemented the bounded compiler/tooling/architecture,
hosted compatibility/RETAIN, and register corrections. Root implemented physical
construction and integrated the changes. Source cross-review covered the new
physical checkpoint decoder, register payload ownership and fused charges,
legacy/debug routes and cycle-refusal fixtures. Review-found corrections were
incorporated before this handoff. This is not compilation or test evidence.

H01–H20 have source dispositions in the external
`full-review-20261012/corrections/CORRECTIONS.md`. H15 was withdrawn after tracing
layout selection: the standalone located temporal fixture has no overlay backing;
its assertions remain unchanged. The other implementation findings have prepared
corrections and regressions. The correction manifest separately identifies paths
changed since the original audit and newly included paths.

No correction test, build, formatter, Cargo command, hardware action, commit or
push has run. Run16 remains the latest failed batch and H is not ready for commit.
H21 performance and H22 current firmware/board/native-platform acceptance remain
open; thresholds, the 16 KiB firmware margin and all IEC functions are preserved.
The existing goal remains paused. Next action: authorize the consolidated batch
in `full-review-20261012/corrections/validation-plan.md`; formatting and locked
Windows dependency resolution/audit are still pending in that batch preparation.




**12 October: complete H source review delivered; implementation remains stopped.**
The owner requested a full read-only review after failed Run16. Root and three
independent reviewers covered all changed code hunks and new code files in the
562-path H inventory; the coverage records distinguish surrounding unchanged
code, historical documentation and unexecuted proof. The external report is
`/home/johannes/projects/.artifacts/runtime-portability-h/full-review-20261012/REVIEW.md`,
with exact per-path coverage and a zero-product-drift receipt. It consolidates
H01–H19 implementation/test/documentation corrections, advisory metadata H20,
and unclosed performance/hardware acceptance H21–H22. The findings include
physical constant/initializer/retain lifecycle gaps, swallowed cycle limits,
optimized arithmetic/budget differences and legacy/debug compatibility defects.
No correction, build, test, formatter, hardware action, commit or push was made
as part of this review. Only external review records and this handoff note changed.
The paused H goal is not complete or ready for commit.

**12 October: Run16 failed; H stopped and not ready for commit.**
The separately authorized consolidated batch ran 04:58:06–05:01:19 UTC on
8,569 frozen source records. Its required ledger has 26 passes, 19 failures
and 12 dependent UNRUN rows; metadata failed separately as advisory. Source
and harness stability passed. The retained exact source archive has zero
mismatches. No further test, formatter, product correction, commit or push
follows this conditional stop. The H goal remains paused.

New compiler defects prevented native behavior assertions: nonexistent Salsa
derive; four borrowed temporal-input getters passed as owned values; missing
TypeKind and test Arc imports; an unconstrained sort error type; and an unused
physical-extent reexport under warning-as-error gates. These are six owning
integration mistakes fanned out across shared build graphs, not 19 separately
observed PLC bugs. Cargo.lock is unchanged from Full15; formatting preserved
the preexisting authoring mistakes. The independent source review missed them.

Architecture also rejects the new unregistered core test-support dev edge,
new size/owner thresholds and the stale lowerer function-name entry. These
policy obligations were missed during authoring/review; no check was bypassed.
Native correctness, benchmarks, current firmware fit and both actual VS Code
suites remain unverified. Diagrams and dependent browser/physical phases are
UNRUN; no STM32 was flashed. Tooling24 tests, supply chain, dependency graphs,
formatting, editor lint/compile, docs and stability checks passed independently.

Evidence and root-cause reports: external `run-16-full/software/ledger.tsv`,
`run-16-full/analysis/` and `run-16-full/frozen-source.tar.gz`; source archive
SHA256 `6a80b0f60d586ba187971e124a346ecebd1e6cce0b5d59c9f2d1e95b1dc41397`.
The following implementation/review checkpoint is historical and superseded
by these failed compile/gate results.


**12 October: H correction implementation and independent source review complete; validation unrun.**
The owner requested all findings from the complete H review to be fixed.
Core and hosted corrections are authored in `trust-platform-portability-h`,
branch `feat/runtime-portability-hosted`, base `19f3b6787`. No correction test,
build, formatter or validation batch has started. Full15 remains failed evidence.
The owner subsequently authorized the researched temporal contract. New source
applications use STBC 3.0 with uniform eight-byte temporal storage and SIZEOF.
Legacy 1.1/2.0 retain their old widths and interpretation; checked physical writes
reject loss before mutation. External DWORD TIME milliseconds remain unchanged.
Implementation and native regression authoring are complete. Corrective
review also separates structural type/value traversal from invocation depth,
preserving the existing 64-edge admission bound and actual call-depth limit.
No validation pass or H commit/push is claimed. No functions are removed from any composition.

- [ ] H-TIME-01: Versioned 3.0 writer/reader/admission and preserved 1.1/2.0 ABI.
- [ ] H-TIME-02: Uniform temporal SIZEOF, relative extents and array strides.
- [ ] H-TIME-03: Exact negative/large temporal round trips, legacy atomic refusals,
  old-reader rejection, retain/update ABI compatibility and unchanged I/O codec.
  Preserve saved 2.0 corpora; the conditional H board replay uses a separate 3.0
  GPIO artifact whose existing GPIO/depth oracle depends on full-width physical
  temporal round trips. No binary generation or board execution is claimed yet.
- [ ] H-TIME-04: Independent review and the consolidated H validation evidence.

Independent correction review is closed by source inspection, with exact-path
receipts outside git under `runtime-portability-h/full-audit-20261011/`,
`h-temporal-reviews/` and `physical-writer-corrections/`. The hosted correction
inventory has 63 paths: 34 reviewed by the core peer, 27 by the publication peer,
and two physical-layout paths covered by the separate core review. Core's
24-path depth/allocation/enum addendum and the final 20-path physical writer
have independent host receipts. Root's temporal hosted/board fixture review
and the core peer's HIR/spec review cover the new ABI integration. Author
exclusions, the invalidated narrow writer review and its repaired full-file
review remain explicit in those records. Source review establishes no pass
for the authored native assertions.

The owner authorized the new consolidated batch on12October by replying
“continue” to the explicit fullHbatch request. Run16 is the sole new batch;
no automatic retry, commit orpush.

Next action: authorized formatting preparation, reconcile formatter deltas with
review identities, freeze one source/harness, then one consolidated validation
batch. The plan is `h-temporal-reviews/validation-plan.md` in the external task
artifact directory. Full15 remains failed historical evidence; this is a separately authorized batch. H is not
ready for commit, push or publication. Final size/heap/MSP/performance and the
current 3.0 board replay remain unverified, including worst-case structural
aggregate stack use.


U publication is now complete: PR #133 merged at
`603278202e7e60a918a79c98ec4eee9897ce911a`, and v0.24.74 release verification
passed. The publication receipt is retained outside git at
`/home/johannes/projects/.artifacts/runtime-portability-u-publication/RELEASED.md`.
The following U/H paragraphs retain the historical execution sequence; their
former stop/publication states are superseded by this checkpoint.

**U modernization is scope-verified; publication is authorized and delegated.**
U is isolated on `feat/runtime-portability-modernization`; all seven U acceptance
items are checked after independent review and passing software/F401 evidence.
[U execution](../../../notes/runtime-portability/u-execution.md) and
[dependency decisions](../../../notes/runtime-portability/u-dependency-decisions.md)
own its source identities and retained failure history. On 11 October the owner
requested verification of earlier releases and delegated U commit, release-guard,
push/PR, guarded merge and publication to a subagent. This authorization does not
claim a passing exact-SHA guard or authorize bypassing one.

Live verification confirms A1/#130, A2/#131 and A3/A4/B/#132 merged; v0.24.73 is
GitHub Latest with its Release workflow successful and all five Marketplace
targets at 0.24.73. U is the next unpublished portability scope. Its current
source is committed as the immutable local checkpoint `19f3b6787`. The separate
publication integration retains prior main corrections. Its initial d602 guard
failure and corrected MQTT companion note are historical records in the execution
note. The immutable checkpoint is unchanged. Generated ladder-bundle whitespace
is recorded as publication preparation.

U publication's first PR #133 wave finished before correction. All native
platform lanes passed; browser worker prerequisites, the cached cargo-deny version,
and VS Code browser startup failed. The automatic review also identified a pinned
PATH overridden by a login shell. The publication agent corrected the complete
known set and obtained independent source/evidence reconciliation.

Successor `0f066c9fbeecceae1e7eefc0f2b5084e46e1173a` has a passing exact-SHA
guard: 8,710 native tests passed, zero failed, 24 ignored. All 13 required
supplemental steps passed, including 519 tests on each minimum/current VS Code
version and four browser checks in each qualified browser composition. The
corrective push succeeded. All five successor workflows finished successfully;
current-head reviews found one new source-manifest self-inventory defect. Its
reviewed correction is committed as `a5b61b633`. Its exact-SHA release guard
passed 8,713 native tests, zero failures and 24 ignored tests; all 16 supplemental
steps passed. The corrective push succeeded. Successor `63cab14e3` passed its exact-SHA guard and all 17 supplement steps.
All 26 public checks completed, but raw macOS logs exposed a scan-timing assertion
rescued by a retry. Merge remains held; bot review requests were refused for quota.
The independent-reviewed measurement fixture isolates and pairs the unchanged
ST/SQLite workload without relaxing its threshold. Its U focused native validation
passed, and successor `93294e0d3` is committed locally; it has not been pushed or
qualified by a new guard/CI. Fresh macOS and release proof remain open.

The core/adapter/firmware/compiler/Cargo/STBC source delta from `d823861a2` to
`0f066c9fb` is zero bytes. Its physical evidence is explicitly the retained
`d823861a2` tuple, not a new board execution: firmware
`c010eed884d7d907a22f41d27bd10fce332c40f93a300026fa68672cdbb3abe8`, UART
`37662b3bb9923287bf36bb45d10de8d9aed669ca19392934692c539b79c7bafb`,
101 oracle-matched samples, 40 periodic activations, zero missed activations;
upper flash free 23,520 B, heap peak 28,272 B and MSP peak 13,652 B.
External evidence remains task-owned; planner/catalog findings are disclosed as
advisory. Physical markings/manual button/optical LED remain unverified.
These bytes are U evidence, not acceptance of H's changed core.

**H stopped after failed complete final batch — not ready for commit.**
The owner's 11 October authorization was one completed implementation/review,
one full consolidated batch, then STOP if it failed. Full15 ran from
20:02:33–20:44:17 UTC on branch `feat/runtime-portability-hosted`, base
`19f3b6787ef1e2d8fd434851684c463baab645e9`, against one frozen dirty source.
All 8,537 source records and the harness stayed unchanged throughout the batch.
The ledger contains 50 required passes, four required failures and two dependent
UNRUN steps; verification metadata failed separately as advisory. The complete
workspace reported 8,855 passed, 66 failed and 24 ignored assertions across 332
parent result blocks, excluding nested child stdout from the count.

Required failures: native workspace, unchanged U/H latency acceptance, diagram
rendering and the real-database ST application gate. Portable stack/register,
i686, isolated runtime-only/full/development compositions, F401/C6 library
checks, firmware link/inspection, lint, architecture, supply chain, docs,
browser workers/captures and both VS Code suites passed. Each VS Code lane ran
519 passing tests; browser captures ran ten. The fresh F401 image has 18,848 B
free in the upper code region, above the unchanged 16 KiB floor.

Read-only comparison with specs and old code identifies actual OVERLAP wire-layout
loss, deliberate STOP force/queued-write cleanup regression, checked-live nonfinite
admission inconsistency and default-profile resource compatibility failures.
Enabled return defaults expose an inherited implementation/spec mismatch. Other
legacy format, tier-reset and fixture expectations require individual dispositions;
they are not all new execution defects. The unchanged performance medians exceed
the 1.10 ratio in all three workloads; call_binding p99 also exceeds 1.25.
The diagram fails at line 303; diagram drift is UNRUN. Physical H replay is UNRUN
because required software acceptance failed. No hardware was flashed.

Full15 evidence, frozen source archive and read-only root-cause receipts are retained
outside git at `/home/johannes/projects/.artifacts/runtime-portability-h/run-15-full/`
and `complete-spec-review/`. [H execution](../../../notes/runtime-portability/h-execution.md)
records the source/review identity and evidence limits. Source-preserving closeout
updates are documentation only after the frozen batch. H's goal is paused under
the owner's explicit conditional STOP: no post-failure product corrections,
retests, H commit or push. Independent U/#133 publication continues.

**Historical H correction checkpoint:**
The owner authorized all necessary H tests/rebuilds/retests and all review fixes.
The independent 41-row review found five owning issues: the legacy consumer,
anonymous legacy type bindings, default-clone construction accounting, reset
wording and missing benchmark acceptance comparisons. All five corrections were
implemented and reviewed. Focused10 finished with 20 required passes and three
failed suite rows: full/development compositions and hosted corrections. Core
all-features (480), portable stack (404), portable register (417), i686 (308),
xtask (112) and debug (95) native assertions passed; F401/C6 library checks,
Clippy, cross warnings, architecture and source/harness stability passed.
Counts are per configuration, not distinct tests. Four unique new test failures
are fixture/expectation defects: methods lacked PUBLIC, a qualified function
used undeclared EN, VAR_CONFIG pinned a producer diagnostic instead of the
owning HIR E202, and plain export was compared against opt-in debug sections.
The complete failed 8,535-record snapshot is archived with zero mismatches.
These four fixture-only corrections are prepared without production changes.
The reviewed current U source-identity checker is adopted separately; historical
byte-only snapshots remain historical, not relabeled as its new identity format.
Benchmark acceptance will load the same saved U STBC1.1 bytes in both builds,
compare all 1,152 cycle observations and retain 1,024 raw latency samples.
Focused9 was frozen but never run. Focused10 remains failed. Corrective11 ended with 17 required passes and two
external benchmark-consumer compiler failures: sha2 0.11's digest no longer
implements LowerHex. Both full/development compositions passed 3,797 assertions;
the default hosted selection passed 3,999 and xtask passed 119. All four fixture
corrections passed. Lint, cross warnings, architecture and source/harness stability
also passed. Counts are per configuration. The exact 8,535-record failed source
is archived with zero mismatches. The owning external-runner correction reuses
hex::encode, as current xtask already does; production Rust and PLC assertions
stay unchanged. The consumer-only repair passed all nine required checks, including both release
compile checks and exact source/harness stability. No production Rust or native
assertion changed between corrective11 and that repair. Individually passing
corrective10/11 lanes and the isolated repair form one pinned evidence chain;
the failed ledgers remain failed. Full H acceptance and a new hardware tuple remain open. The tested U measurement
fixture/spec correction is adopted through five non-production owners, preserving
H's existing telemetry API migrations, and requires fresh H native execution.
Next qualify common-artifact performance and linked/inspected F401 fit before
expensive final host/editor/hardware acceptance; failed prerequisites remain open.
Full12 qualification finished with 11 required passes and one failed benchmark
step. Both builds preserved all 1,152 trace rows, tier counters and zero fallbacks.
Call binding failed the unchanged latency bounds (median ratio 2.2602, p99 1.6949),
references/SIZEOF failed (1.9773/1.4051), and string stdlib passed (0.8632/0.8358).
The fresh F401 image linked and passed inspection with 19,232 bytes free in the
upper region, above the unchanged 16 KiB floor. Firmware native/Clippy, application
packing, tuple hashes and source/harness stability passed. Broad host/editor and
physical H qualification remain unrun; Full12 remains failed. Cycle-only
Callgrind on the retained binaries identified repeated dynamic-reference owner
resolution across unrelated reference-table entries as the main reference cost.
The successor correction prepares kind/slot candidate groups, preserves every
alias and conflict check, prefilters impossible paths and reuses already checked
static-reference addresses. Review and focused native/performance proof precede
any broad acceptance rerun; neither bounds nor functionality are reduced.
Focused correction13 is terminal: five required infrastructure/stability checks
passed, and eight compiler/test/lint/MCU lanes failed at the same new E0277.
No behavior assertion executed. The prefilter borrowed a Vec where strip_prefix
requires a slice; explicit as_slice restores the old borrowed-slice contract.
The 8,537-record failed snapshot is archived with zero byte/mode mismatches
(8,522 regular files and 15 missing records). The one-line correction is prepared;
independent review and focused native14 validation remain pending. There is no
performance, fresh successor firmware or hardware acceptance from this failure.
H is isolated on `feat/runtime-portability-hosted` from immutable U checkpoint
`19f3b6787ef1e2d8fd434851684c463baab645e9`; no H commit/push is authorized.
H's first frozen software batch completed on 11 October with 20 required passes,
16 required failures and 13 dependent steps unrun, plus one advisory pass.
The isolated runtime-only stack consumer executed four tests successfully; failed
workspace/core/register/composition lanes stopped at compilation. No failed PLC
behavior assertion was observed. MCU firmware link and hardware remained unrun.
The exact 8,527-record source snapshot is preserved externally and reconciled
without mismatches. Core and host corrections are authored and independently
reviewed; factory, clock and architecture-owner corrections await final review.
Focused validation comes before repeating the complete workspace/editor/hardware
allocation. No H acceptance item closes from an unrun correction.
The subsequent focused batch finished with 18 required passes and 11 failures,
with source/harness stability passing. Native assertions now distinguish two new
fixture error-identity mistakes from the unchanged core timeout contract; numeric
values pass, but an EXPT body legitimately follows the inherited stack fallback.
No-driver composition and supply-chain checks pass. Exact failed source is retained
again. The complete next correction includes positive register proof on the saved
clock artifact, checked/reference-instruction allocation regressions, feature-owned
unit fixtures and responsibility splits under unchanged architecture limits.
Focused run 3 completed at 14:18:04 UTC with 21 required passes, six failures and
no unrun steps. No native assertion failed: core default library 281 passed,
portable-register selection 271 passed, i686 selection 298 passed and the
independent register consumer four passed. F401/C6 core warning checks and the
full architecture map passed. Hosted tests remained compilation-blocked by two
missing imports and SQLite fixtures compiled without their driver. The external
unsafe scanner also detected shifted line registrations for unchanged allocator
code. All 8,530 source records reconcile with the retained failed snapshot.
The four-owner correction restores the imports, gates only driver-specific
fixtures and rebinds all seven existing allocator registrations. Independent
source review accepted it before focused run 4, whose result follows.
Focused run 4 completed at 14:53:58 UTC with 10 required passes and five failures.
Format, unsafe AST, full-map and source/harness stability passed. Hosted native
assertions did not execute: two allocation-test calls passed a borrowed view where
the installed facade requires a reference to ValueRef, while warning-denied unit
builds exposed unused private legacy fixture APIs. Clippy also identified complex
tuple signatures, two nested conditionals and a helper placed after its tests.
The exact 8,530-record failed snapshot is retained with zero mismatches.
The corrective slice changes the two caller arguments, preserves the tuple types
through aliases and removes only unreachable test-only compatibility code; active
production services and all native assertions remain.
Focused run 5 completed at 15:05:35 UTC with 10 required passes and five compiler
failures. The prior legacy-warning and allocation-call roots are resolved; newly
reachable compilation found one fixture import still inherited from its removed
parent API and six unsupported fields in two new DAP request literals. No native
assertion executed. Both test-only corrections are source-reviewed; focused run 6
adds DAP native execution and uses supported Clippy compiler keep-going without
changing test assertions or their no-fail-fast allocation. Earlier core/MCU
receipts remain historical evidence rather than new executions.
Focused run 6 completed at 15:15:56 UTC with nine required passes and six failures.
Native behavior now executed: full/development no-driver compositions each passed
3,755 assertions and failed 35; the hosted selection passed 3,908 and failed 34;
DAP passed 94 and failed one. Across configurations there are 39 unique failing
test names, plus seven lint findings. The existing reference-allocation suite
passed all three tests, including 128 LOAD_REF and 256 STORE_REF observations;
registry isolation passed three and register/stack string parity passed one.
The complete 8,529-record failed snapshot is retained with zero mismatches.
Corrections address the owning FB-frame validator, equivalent anonymous reference
shapes, configuration seed target, legacy STRING I/O width, nested register scratch,
hosted labels/diagnostics and feature-specific protocol expectations. New fixture
mistakes are recorded separately: explicit tier reset, duplicate PROGRAM type,
pre-instruction debug observation and retained-label rejection stage. No production
hook or validation is weakened to fit a test. Focused run 7 is prepared, not run;
changed core, 32-bit and both MCU graphs require fresh evidence. Full workspace,
editor/browser, benchmark, firmware link and physical H acceptance remain open.

[H execution and command allocation](../../../notes/runtime-portability/h-execution.md)
records the owning seams, compatibility audit and validation. Deliver shared source-free hosted construction
and library coverage, the internal register tier, preserved host APIs/workflows,
and real runtime-only/full/development compositions. All IEC/runtime functions
remain available on every platform. E/C6 bring-up stays open until its board is
available; later bounded-operation and production qualification scopes stay open.

On 11 October the owner delegated selection and specification of the mutation
lifecycle. Spec 34 v0.14 §4.3.1 / HOST-09 now selects construction-only raw access,
checked installed-state value writes, separate debug parsing types and isolated
candidate schema changes. HOST-03 explicitly records the live Rust mutable-reference
API migration while preserving PLC functions and hosted workflows. Spec 11 carries
the planned-H cross-reference. The owner resumed the goal after that amendment.
H now separates owned mutable engine data from operation-scoped metadata/service
borrows and adds a std owned-controller facade, preserving the borrowed MCU API.
Ownership/parity/rollback/access tests are authored; independent source review
accepted the ownership slice, with accounting corrections included. Shared
optimized plans and ready-scratch reuse are authored and under source review.
Declaration lowering now owns a non-executable `RuntimeBuilder`; both producers
borrow a read-only authoring view. Debug expression parsing uses scratch registries,
watches retain their scratch types, and dynamic assignment indices are resolved
without extending installed types. Active-frame observers are read-only and suppressed
during isolated candidate construction. All corresponding assertions are authored,
not run. Independent source review accepted these slices after comparing explicit
cache reset and profile reset against old U. Later preparation compilation failed
before any native assertion; its retained results are recorded below.
Shared cycle hooks now borrow host adapters through checked owner-local ports,
including ADS/OPC UA inputs and output capture. The ordinary MCU cycle retains
its no-hook path; hosted post-output rollback journals only touched flat Q
windows and preserves the old marker/hierarchical rollback domain. Full live
caller views and detached named debugger snapshots are authored, with actual
depth-four, ancestor retirement, observer-failure cleanup, watch/pause and
conditional-breakpoint regressions. The host clock adapter preserves UTC and
debug-pause deadline accounting. Source review accepted the caller view and
root reviewed the hosted service implementation; whole-scope review and every
H execution check remain pending. Diagram sources are updated, not rendered.
The public hosted `Runtime` now owns one shared controller and immutable tooling
metadata, with checked value/image APIs and an immutable image view. Ordinary
`CompileSession`/test-harness construction is wired to STBC 2.0 admission and the
same engine; explicit 1.1 production is confined to a non-executable builder.
The 1.1 import bridge preserves supplied table prefixes, ordinary instruction
bytes and local ranges while appending initialization metadata/code. Source-free
tooling types use an explicit artifact-to-HIR mapping, and configured program
templates remain separate from instance names so `USING` scope is not lost.
Hosted cycle adapters are wired for drivers, debug queues/forces, protocols,
retain checkpointing, metrics and safe-output faults. Native constructor,
25-ms scheduling, rejection/force ordering and expired-watchdog delivery tests
are authored, not executed. The scalar cycle-image read accounting gap found
in source review is corrected with native assertions, also unrun.
Manual-builder installation, atomic retained-value migration and candidate
replacement are now authored, including typed aggregate/instance seeds, image
intersections and checked protocol/safe-output rebinding. DAP production callers
use checked installed operations and detached snapshots; retained scopes project
actual declared global/program fields, without a second retain bank. Historical
dispatch/Expr/lifecycle corpora are explicitly private `cfg(test)` fixtures.
Ordinary exports preserve immutable deployment initializers instead of serializing
current PLC values. Native regressions cover these paths and publication rollback
before/after successful driver submission; all remain unrun. Source review found
and corrected unverifiable source-map association, lost character materialization
and retained program-object loss on incompatible replacement.
Resource-only task/image/binding transactions and contextual supplied-artifact
loading are authored. Engineering ADS writes queue the canonical value returned
by the shared writer, preserving exact wire encoding after admitted conversion.
Retained management targets use an ordered metadata index and resolve current
live global/program destinations; single lookups share the current scan fuel.
External integration callers now use admitted builders or checked writes; explicitly
isolated bridge/codec fixtures remain distinct from PLC execution evidence.
Runtime-only/full/development feature compositions and saved-artifact/clock tests
are authored. Historical `cfg(test)` fixture types are explicit, without a
production ingress. The embedding migration guide documents replacement APIs.
U's committed validation prerequisites are integrated without replacing changed
H owners; H reserves version 0.24.75. The first preparation completed: formatting,
frontend/WASM prerequisites, baseline and syntax-corpus generation passed. Fixture
producer compilation failed on two missing imports after relocation, and the
consumer setup failed because the clock fixture was unavailable. These are
retained preparation failures, not PLC assertion failures. The imports and
dependency guards are corrected for independently reviewed recovery preparation,
which reuses successful prerequisite receipts. Native software validation and
fresh H hardware evidence remain unrun. Copy back/review generated outputs and
freeze before that batch. Diagram sources are updated, not rendered; H is not
ready for commit.

Recovery preparation R2 reached hosted compilation and retained nine errors and
eight warnings. Their complete owning corrections preceded R3, whose seven
required steps passed: both fixture producers and the isolated consumer setup
are now generated. Twelve subsequent warnings were traced before freeze; one
revealed missing per-program hosted profiling, now restored with paired optional
common-engine observations and authored success/fault/deadline assertions. The
corrected source is not yet native-verified. Failed attempts and source-review
identities remain in H's execution record; no H acceptance box is closed by
preparation alone.

Final recovery preparation R4 passed all six required steps and its advisory
refresh. Both fixture producers compiled without warnings; generators succeeded
and the independent consumer's clock fixture still matches. The three original
STBC fixtures match the U base exactly; the new clock fixture is separately
pinned. Returned formatted bytes are under independent final review before
freeze. No H PLC execution, consolidated software batch or board phase has run.
U publication continues independently.


Published A1 integration record (10 October): PR #130 merged through the guarded merge at
`258fe24b8c3706d566f5f05c25cfcd75e5b297c3` after all 26 checks passed. Native
Linux/macOS/Windows and VS Code passed first attempts with no retry rescues.
v0.24.71 release verification is complete. The integration agent owns that
release and subsequent A2 publication. #129/v0.24.70 has completed release
verification. None of this is A4 scope evidence.

The following table retains the completed, published Scope B checkpoint.

| Field | Recorded state |
|---|---|
| Updated | 10 October 2026 — B-R4 run 8 software/ELF and physical F401 bring-up pass. Independent source, software and physical reconciliation complete; earlier failures retained. |
| Checkout | `/home/johannes/projects/trust-platform-portability-b`, branch `feat/runtime-portability-b`, base `df427259cc387a7a79fb81132be23e48ea1493d4`. |
| Bootstrap | Canonical AGENTS.md, CLAUDE.md and complete skills manually copied from `/home/johannes/projects/trust-platform`; 21 files byte-match locally and on the isolated builder checkout. |
| Authorization | Owner requested “commit then start next step”; A4 committed locally, Scope B implementation/review and one consolidated builder/hardware batch. Owner is away and explicitly instructed remote tests without requesting board markings. Owner subsequently explicitly requested implementation of all five B-R1 decisions: L2, shared footprint reductions, targeted containers/sorts, preserved checks, reviewed expiring advisory exception and both lock audits. B-R1 has implementation, independent review and one consolidated validation batch. Owner additionally authorized one linked-size measurement before the full batch, with no tests/hardware or automatic retry. Owner subsequently requested implementation of the recommended broader function-preserving set: compact diagnostics, targeted engine/retained-graph keys, lean registry and remaining sort/bookkeeping reductions, with safe ICF evaluation and bounded size-only measurements. Owner explicitly prohibits reduced function sets on any platform; the proposed restricted-profile fallback is withdrawn. Local Scope B commit authorized by the subsequent “commit” instruction; publication was subsequently authorized through the separate release-integration scope. The later explicit order “do as mny tests as required to fix it, dont stop” authorizes the recorded B-R4 corrections and retries without repeated permission requests. |
| Scope | B: NUCLEO-F401RE thin adapter and bare-metal composition, unchanged A4 main/numeric artifacts plus a separately authored GPIO/stack probe, real board traces and memory/timing evidence. |
| Status | Run 8 reviewed candidate passes ELF inspection and corrected validation2 (17 required software gates, one advisory); physical attempt 2 passes all 156 records and the Rust verifier. Upper flash free 18,720 B; peak heap 28,272 B / 73,728 B; peak MSP 13,596 B / 16,384 B, leaving 2,788 B. First metadata precheck and first installation failures remain retained. |
| Source identity | A4 base `df427259cc387a7a79fb81132be23e48ea1493d4`; run-8 frozen manifest `a95bcff98b5d4a51e70acf7e29cf0113b585ac69bcfc39c4cc5fbac6b1e8337c`; ELF `a0e6ed2d7f725fd17746d30c229d02e5b0125c00c494015f3be40322e293ec1e`; UART `9601cceef51d46170961c74e0afe2aba33b44250d3795225f6df60055a315704`. Runtime/firmware/tooling source is unchanged from the tested candidate; closeout documents/evidence and a separate 0.24.73 release-metadata commit follow the freeze. Historical hardware evidence retains its 0.24.71 tuple. |
| Limitations | Electronic board identity available; printed PCB/package markings, physical button actuation and optical LED observation unavailable. No claim of external-plant operation, durable retain, release qualification or worst-case timing proof. |
| Integration | A1/#130/v0.24.71 complete. A2/#131 merged at `3bed89b47b4e0ed8ecc11410b592228fff5f0112`; v0.24.72 release verification is complete (tag, Release workflow, assets/checksums and all five Marketplace targets). A3/A4/B merged through PR #132 at `b7d8ffbf242ed5bbef4c971afd43a179757fc1b6`; v0.24.73 tag, Release, Latest/checksums and all five Marketplace variants verified. |
| Next action | Scope B remote bring-up and independent evidence reconciliation are complete. Retain this result for the next separately authorized scope. Scope B local checkpoint is `f756ddd9a`; the following metadata commit targets 0.24.73. The subsequently authorized publication scope validates the exact integrated SHA, replays its final firmware, and then handles push, guarded merge and release; later implementation scopes remain separate. Printed markings, manual button actuation and optical LED observation remain explicitly unverified; M3 allocation-free/worst-case qualification remains separate. |

Publication completed through PR #132. Final source `c3def9e144235831579ee45c03fc5d875c8607c2` has the same tree as main `b7d8ffbf2`; fresh physical ELF `4761d2bc5153b3892da352ca1f0be1f7041f421457158bfc2a9682ddbbb39a61` and preserved fixture proof accompany v0.24.73. The publication release agent verified all release outputs; exact cleanup is complete.
[Release integration command map](../../../notes/runtime-portability/stm32-release-integration.md)
owns the exact-SHA guard, fresh final-image hardware replay, push, guarded merge and release.
Earlier source checkpoints and hardware evidence remain unchanged.

Scope B execution and command map: [record](../../../notes/runtime-portability/b-execution.md).
B-R1 correction scope: [record](../../../notes/runtime-portability/b-r1-execution.md).
B-R2 result and remaining gates: [record](../../../notes/runtime-portability/b-r2-execution.md).

Run 3 correction evidence: [record](../../../notes/runtime-portability/b-r1-evidence/run-3/README.md).
Software assertions and lint/architecture pass. [Run 4](../../../notes/runtime-portability/b-r1-evidence/run-4-diagrams/README.md) closed diagrams at that checkpoint. Its flash/physical gaps are now closed by the
reviewed B-R4 run-8 successor, without changing the historical B-R1 failures.

### B-R1 — Shared footprint reductions, unchanged IEC functionality

- [x] RTP-BR1-01: L2 specification, linker, installer/ELF inspector, partition regressions and diagram agree; two 16 KiB checkpoint sectors remain unimplemented persistence.
- [x] RTP-BR1-02: Byte preparation avoids serialization while decoded-object checks remain; compact shared CRC matches accelerated CRC for fixtures, boundaries and deterministic randomized inputs.
- [x] RTP-BR1-03: Static default-library descriptors preserve every name, parameter shape and hosted custom/override behavior, with allocation/lookup equivalence assertions.
- [x] RTP-BR1-04: Record selected map/sort owners and preserved key order, tie/error order, lookup cost, capacity and work charging; implement targeted reductions and native regressions. Full M3 storage replacement stays deferred.
- [x] RTP-BR1-05: Overflow checks and complete admission stay enabled; reviewed RUSTSEC-2026-0110 exception expires within 90 days; root and firmware locks both undergo dependency audits.
- [x] RTP-BR1-06: Independent review, frozen source and native/core/32-bit/host, MCU, lint/cross-target, architecture/diagram and footprint evidence. Closed by the explicitly authorized B-R4 successor runs and final conditional board capture; original failed batches retained.

- [x] RTP-BR1-07: Expanded reductions preserve all IEC/assertion operations: remaining shared sorts, compact/shared registry metadata, checked engine/retained identities and reused initialization membership. Native order, identity, restart and retained-graph regressions.
- [x] RTP-BR1-08: Compact preparation diagnostics preserve stable fault identities and bounded useful context, with hosted rendering and native parity assertions; uniform function set required across every platform; no capability-reduction fallback.
- [x] RTP-BR1-09: At most two recorded size-only comparisons for the complete reviewed expanded implementation (ordinary and safe-ICF link); no unchecked-overflow variant, no tests/hardware in measurements. Final validation remains one batch with no automatic retry.

B-R1 software items above use runs 2–4. The checked measurement item records the two
authorized attempts, including the failed ordinary compilation and safe-ICF link; it
does not claim a successful link or isolated ICF savings in B-R1. The successor B-R4
run-8 evidence closes RTP-BR1-06 after the shared storage/stack corrections.

- [x] RTP-B-01: Reviewed thin adapter, separately installed application bundle, bounded firmware and native tooling regressions.
- [x] RTP-B-02: Run-8 linked ELF satisfies flash partitions/margin, static RAM, 72 KiB heap and 16 KiB MSP reservation. This does not establish physical stack usage; further candidates require fresh inspection.
- [x] RTP-B-03: Same A4 main/numeric artifacts replay on the physical F401; recorded GPIO pad state, safe-output transitions and independent watchdog reset.
- [x] RTP-B-04: Preparation/scan heap, call-depth 1–4 stack with IRQ activity, timing and numeric measurements satisfy the declared profile.
- [x] RTP-B-05: Independent evidence reconciliation and limitations retained; no substitution of simulated input for a physical button actuation.

### B-R4 — Shared native-stack correction

**Current authorization supersedes the earlier run limit:** “do as mny tests as required
to fix it, dont stop”. Continue reviewed root-cause fixes and recorded software/link/
hardware attempts until the required gates pass. No feature or acceptance reduction.

Current result: **run 8 software/ELF and physical bring-up pass; independently reconciled closeout complete**.
[Execution record](../../../notes/runtime-portability/b-r4-execution.md) and
[retained evidence](../../../notes/runtime-portability/b-r4-evidence/README.md).

Owner explicitly authorized “fix it and run the hardware tests again”. Historical correction work stayed on
feat/runtime-portability-b before its authorized local commit; no function/depth reduction, RAM repartition,
relaxed margins. The later repeat authorization above supersedes the original single-batch
limit; every failed attempt remains separately recorded, with review before correction runs.

- [x] RTP-BR4-01: Spec34 §6.2.2 records continuation, binding, cleanup, operand,
  budget, initializer and harness invariants before implementation.
- [x] RTP-BR4-02: Shared prepare/complete call semantics and explicit stack-dispatch
  continuations; preserve hosted optimized adapters and all lifecycle behavior.
- [x] RTP-BR4-03: Firmware phase boundaries and stop-on-insufficient-stack harness;
  unchanged fixtures, supported depth, images, heap/MSP and acceptance thresholds.
- [x] RTP-BR4-04: Native continuation/lifecycle/operand/depth/error assertions plus
  applicable existing corpus; independent full-diff review and source freeze.
- [x] RTP-BR4-05: Recorded builder batches and explicit source-parity reuse cover new firmware map/ELF,
  complete native/portable/i686/hosted gates, tooling, audits, diagrams and formatting.
- [x] RTP-BR4-06: Physical run-8 attempt 2 passes stack/heap/depth 1–4, numeric,
  process image, STOP/FAULT and watchdog evidence; prior failures retained.
- [x] RTP-BR4-07: Independent evidence reconciliation and checkpoint update.

### B-R3 — Compact live storage, unchanged runtime functions

Historical attempts below remain as run. Their unresolved physical criterion
RTP-BR3-07 is superseded by passing RTP-BR4-06; it is not a remaining Scope B blocker.

**Additional authorization after the failed batch:** owner requested “test and remeasure
then run the hardware tests”. Run 2 covers reviewed deadline/journal corrections, one
preparation/freeze, the deduplicated software gates and one corrected-image link, followed
by the existing physical procedure only if all prerequisites pass. No automatic retries.
The two earlier measurements and failed batch remain separately retained evidence.

Status: **software verified; flash fit/inspection passed in run 3; physical stack qualification failed**, owner “implement it”, 10 October 2026.
Continue on feat/runtime-portability-b at base df427259c. B-R2 failed measurements
and source identity remain historical evidence, not a passing baseline. Authorization:
complete implementation and independent review; at most two size measurements (M1/M2)
with their one-time source preparation; one consolidated validation batch, conditional
hardware only after complete software/ELF/headroom acceptance. No retries, commit/push,
capability cuts, unchecked arithmetic configuration or toolchain changes.

- [x] RTP-BR3-01: Write spec34 §6.2.1 invariants for IDs, frame/staging capacity,
  batch retirement, ordered journals, indexed name fallback, work/allocation and numerics.
- [x] RTP-BR3-02: Implement compact live frame/instance storage and portable indexed
  name lookup, preserving hosted interfaces; lifecycle, stale-ID, high-ID, miss and
  retirement native regressions. Preallocate frame bookkeeping from the admitted depth;
  do not claim all instance/compound allocations eliminated.
- [x] RTP-BR3-03: Replace selected engine journals/bookkeeping trees with compact
  ordered entries, preserve alias rollback and retained graphs, account movement/work.
- [x] RTP-BR3-04: Complete proved numeric/helper changes, one measured validator
  inlining experiment, exact error construction and sector-0 packing/trace decisions;
  preserve full domains and all functions. Include the five B-R2 Clippy expressions.
- [x] RTP-BR3-05: Independently review all source/test/tooling changes. Freeze complete
  source, then M1 once; M2 only for a reviewed measurement-informed revision. Each
  attempt is consumed even on failure; no extra trial/flag matrix.
- [x] RTP-BR3-06: One deduplicated final batch: B-R2 suites plus new lifecycle/numeric
  assertions, 32-bit allocation, MCU graphs, hosted vertical, tooling/Clippy/architecture,
  both locks, diagrams/drift, format/provenance. Collect all independent failures.
- [ ] RTP-BR3-07: Exact measured ELF leaves >=16 KiB upper headroom and respects
  sector0/app/checkpoints/RAM; then original hardware procedure including backup,
  explicit sector erases, UART/stack/heap/timing/safe outputs/IWDG. No shortened fixture.
- [x] RTP-BR3-08: Reconcile evidence; no board claim without execution. A failed final
  batch stops runs; corrections may be prepared but reruns need explicit authorization.

M1/M2 review and measurement are complete for their frozen identities. The measured
M2 image meets the flash floor; RTP-BR3-07 remains open for physical evidence.
Run 2 closes the corrected storage/journal behavior; run 3 closes final placement and
ELF inspection using the same Rust source. RTP-BR3-07 remains open: physical testing
failed stack headroom. Checked software rows do not claim hardware qualification. [B-R3 execution record](../../../notes/runtime-portability/b-r3-execution.md)
and [retained evidence](../../../notes/runtime-portability/b-r3-evidence/README.md)
identify the failed assertions and passing gates. Neither reruns nor hardware are launched.

### B-R2 — Fit the complete runtime on F401

Status: **implemented and reviewed; fit failed and validation remains incomplete**. The owner's “implement it” authorization covered
B-R2 implementation, independent reviews, at most two size-only measurements with
snapshot preparation, and one consolidated software/conditional hardware batch.
No automatic reruns, commits or push. Every PLC function remains required.

Active source: `trust-platform-portability-b`, branch `feat/runtime-portability-b`,
base `df427259c`. The isolated M1 snapshot is
`trust-platform-portability-br2-m1`, detached at the same base with the full B-R1
working source copied before B-R2 runtime edits. Both manually received matching
canonical AGENTS.md, CLAUDE.md and all skills from the primary checkout.
M1 contains layout/build/tooling changes only; M2 includes the complete selected
source wave. M1 completed with a capacity-only link failure: upper span 474,400 B,
15,648 B over L2; sector 0 has 1,248 B free. M2 also failed flash capacity
(17,504 B over). The final native suites passed; two Clippy gates failed. Prepared
corrections remain unvalidated, and hardware is unrun. Both measurement attempts
and the final batch are consumed.

Checked B-R2 items below mean their implementation/review/measurement or reconciliation
was performed, not that fit passed. RTP-BR2-08 remains open for the two failed Clippy
lanes; RTP-BR2-09/10 remain open for missing installable ELF and physical evidence.
The measured M2 source and post-batch lint corrections have separate identities.

#### Evidence and arithmetic

The retained safe-ICF map has SHA-256
`2b9f0af658b63c9902386c2d3a89db36144e56b5ffe657b18e998a5f0cc7d933`.
This B-R1 baseline predates the run-3 mechanical Rust corrections and the B-R2
measurements above. The planning arithmetic below is retained as historical input,
not the current fit result. Read-only inspection established:

| Quantity | Bytes | Meaning |
|---|---:|---|
| Upper firmware span / capacity | 493,536 / 458,752 | 34,784 B (33.97 KiB) over L2 |
| Vector table / sector 0 | 404 / 16,384 | At most 15,980 B (15.61 KiB) recoverable before extra alignment |
| Residual if the entire tail is usable | 18,804 | Still 18.36 KiB to remove just to link |
| Required additional reduction for a 16 KiB upper-region margin | 35,188 | 34.36 KiB beyond maximal tail use, not merely the residual-to-link |
| Merged string section | 14,306 | 11,210 B are path strings including terminators; physical classification below |
| Outlined function definitions | 1,543 totaling 18,742 B | Confirms outlining exists; does not measure net savings or prove optimization is exhausted |

Moving data into an existing firmware-owned sector changes placement, not total
occupied flash. Alignment, merging and path remapping interact, so savings must not
be added as independent guarantees. The review's 39–52 KiB range does not guarantee
the proposed 16 KiB margin: from the old layout that needs 51,168 B total benefit.
Estimates prioritize work; only the final map/ELF establish fit.

Follow-up inspection reads the saved ELF object, SHA-256
`997d7dee59569c20b34326177b4d2f3751e160b4a289b74ff2acc4c3d896b753`,
without a new link. The safe-ICF log records 92 removed text and 16 removed read-only
sections. Its address-significance table contains 47 entries and no function symbols:
the heap, IRQ counter, five dragon tables and parameter metadata. The review reports
no additional foldable twins. No incremental saving is credited to unrestricted ICF;
keep safe ICF and spend no measurement slot on `all`. This inventory is not an executed
safe-versus-all comparison or a general permission to fold address-significant data.

Splitting the physical `.rodata.str1.1` bytes on NUL terminators gives:

| String group | Count | Bytes including terminators |
|---|---:|---:|
| Worktree paths | 61 | 6,748 |
| Cargo registry paths | 19 | 1,942 |
| Local rustup source paths | 13 | 1,598 |
| `/rustc/<hash>/library/...` paths | 11 | 922 |
| Other names/messages | 93 | 3,096 |
| Total | 197 | 14,306 |

This reconciles exactly; the review's four byte subtotals sum to 14,335 and are not
used as a physical partition. Contrary to its wording, `/rustc/` strings are present.
Normalize the three local prefixes in Phase 1 and measure the result; about 5 KiB is
a working estimate, not guaranteed reclaimed upper-region capacity. Moving/shrinking
the same strings must not be counted twice. Existing precompiled string constants
are not automatically rewritten by changing this crate's source-path flags.

#### Decisions and disposition of the nine proposed levers

| Lever | Plan and limits |
|---|---|
| 1. Sector-0 immutable data | Include first. Select whole read-only input sections, preserve alignment/vectors and assert the 16 KiB end. Do not assume a wildcard automatically packs only what fits. Audit reset/vector symbols, data load addresses, load segments and installer erase ranges. |
| 2. Common BTree representations | Select the numeric owners from the retained map now and author alongside Phase 1. Normalize u32 newtype keys and existing ID-vector payloads without new side vectors. Keep typed wrappers at boundaries, the same tree operations and ordering. Treat packed Value-map keys, optional lifetimes and the three name caches separately; no blanket two-type replacement. |
| 3. Cold error constructors | Include measured shared failure constructors, preserving error identity, useful context and exact hosted rendering. Reuse existing conversion helpers, with cold/no-inline boundaries where appropriate. Do not force the ordinary success path through indirect dispatch. |
| 4. Smaller execution/preparation functions | Prioritize cold error/construction work outside the recursive dispatcher, then natural boundaries in other measured owners. This can remove cold-only slots from every recursive frame. Inspect actual frame layout and full nested hot/cold/IRQ peaks: compiler slot reuse, spills and call overhead prevent a source-only guarantee of the final high-water reduction. Keep one dispatcher, shared budgets and deadlines. |
| 5. Duplicate generic helpers | Include the charged sort error adapter, assertion comparator and evidenced duplicate helpers. A common internal sort failure marker must restore each caller's original error and exact charge/tie behavior. Retain typed public contracts; do not invent a general type-erasure framework. |
| 6. Explicit faults | Apply to touched, reachable malformed-data/capacity paths with a specified fault. Preserve fail-closed cleanup/output behavior and the firmware panic record. Inventory remaining panic/arithmetic/index sites; no claim of a panic-free crate from removing unwraps, and no blanket new lint suppression. A full core/alloc panic-elimination project remains outside this fit scope. Overflow checks stay on. |
| 7. Assertions and 128-bit arithmetic | Share assertion comparison control flow while retaining all names/messages and float formatting. Narrow numeric intermediates only with full-range equivalence, including LINT_MIN, ULINT_MAX, ties-to-even, nonfinite input, signed bit patterns, text radix/sign parsing and error precedence. Keep i128 where proof is absent. Time operators also use i128 division/conversion, so changing stdlib conversions alone cannot be credited with removing those helpers. |
| 8. Build flags | Put the already measured safe ICF choice, address-significance emission and existing linker/CPU settings into one default firmware configuration. Normalize source paths with the pinned compiler's supported remapping; preserve useful file/line context. No unrestricted ICF, optimization-level sweep, overflow-off variant or toolchain change in this scope. |
| 9. Firmware trace writer | Select from the retained map if useful; authoring need not await M1. Use shared integer/hex output helpers only for measured duplication. Preserve the B1 protocol byte-for-byte, error records and separation of timing from serial output. Keep full runtime float formatting; trace optimization is not a capability cut. |

Specific qualifications from source inspection:

- `memory.rs` contains three name-based caches and two other lookup maps, not five
  string-keyed caches. Strings cannot be replaced by a lossy numeric hash. Stable
  prepared IDs may be used only where already available or where their exact
  interning, unknown-name and invalidation behavior is designed and bounded first.
  Cache failures remain misses; scans must not acquire linear name-table work.
- Raw-ID normalization does not require side vectors or add lookup indirection.
  Specifically, `activation_pous` and `instance_templates` can share the existing
  `BTreeMap<u32,u32>` shape used by method owners and retained identity translation;
  `owned_instances` can share `BTreeMap<u32,Vec<u32>>` with retained marks/root indices;
  InstanceId sets can share `BTreeSet<u32>`. Preserve typed operations at the API edge.
  The RAM concern applies to widened/packed keys and any later auxiliary storage,
  not to unchanged u32 representations. Count B-tree node capacity/alignment in
  allocator peaks rather than bounding total RAM by four bytes per live entry alone.
- The Value-map merge needs an explicit key proof. `MemoryLocation` has five enum
  variants, including three I/O areas, and the journal key also contains a usize slot.
  Prove admitted bounds, supported/rejected locations, injectivity and original sort
  order before packing. Existing 65,536-record/local caps may support a checked compact
  encoding; they do not authorize truncating host offsets or changing error precedence.
  `Option<FrameId>` must preserve None versus Some(0) and the full admitted ID range.
- The measured BTree family is an optimization budget, not a minimum saving. An
  average-per-instantiation estimate does not establish a 12 KiB floor: instantiation
  sizes, LTO/inlining and existing folding differ. Existing retained-graph, snapshot,
  cycle and execution-frame suites are the primary regression corpus; rerun them on
  the new representation. Add only missing key-boundary/order/rollback assertions.
- Both safe and unrestricted ICF fold functions. The difference is address-significant
  sections, not whether function folding occurs. Use the saved-object inventory above
  as the practical reason not to spend a build on `all`; safe ICF is already in the
  retained map and adds no new credited saving.
- Path remapping does not rewrite already compiled core/alloc objects. It improves
  normalization, but a two-directory byte-identity build is required before claiming
  full reproducibility; that extra build is not in this two-attempt campaign.
- Assembly or alternate compiler-builtins would broaden numerical/startup evidence.
  Neither is required by this plan. Nightly, build-std, RUSTC_BOOTSTRAP and panic-record
  bypasses are excluded. The reported GCC/assembly saving is not independently verified.

Registry review: `parameters.rs` defines nonzero-sized named statics, and descriptors
borrow them. The `ptr::eq` assertions checking that sharing are valid and should stay;
Rust gives those statics allocation identity. Function-address equality is different:
the `fn_addr_eq` checks do not by themselves prove registry behavior. In B-R2 test
authoring, preserve registration coverage through representative native calls and
signature equality, replacing address-only behavior claims where redundant. Do not
remove the static-sharing/allocation regressions or claim an observed LTO test failure.
There is no identified production dispatch rule based on function address here.

The 128-bit helper caller set must include root `datetime.rs::days_from_civil`,
`time_ops::duration_to_ticks`, `time_ops::scale_duration`, and float/integer conversions.
Source contains wide division in all three time helpers; which operations emit a
particular compiler builtin must be traced in the saved object. Checked narrowing
must preserve extreme signed years, epoch arithmetic, MIN/-1, full ULINT factors,
rounding and existing faults. Do not credit the divider as removed until all linked
callers are accounted for and the next map confirms absence.

#### Phase 1 — Placement baseline; source authoring may proceed concurrently

- [x] RTP-BR2-01: Implement spec 34 §D.1's sector-0 extension in `memory.x`/
  `profile.x`, keeping `_stext=0x08010000`, all vector offsets, application sector 1
  and checkpoint sectors 2/3 intact. Add explicit read-only placement/end symbols
  and assertions. Keep the startup crate/link script pinned; do not fork startup.
- [x] RTP-BR2-02: Update `xtask/src/portability/elf.rs` and `map.rs` together.
  The current inspector accepts sector-0 loads, but needs named data bounds and
  read-only checks. The current map reporter rejects any non-vector load below
  `.text`; it must understand both firmware regions. Report occupied load bytes,
  upper span/free bytes, sector-0 use/free bytes and padding separately. Add native
  cases for valid discontiguous loads, vector overlap, sector-boundary overflow,
  a segment spanning protected sectors, writable data in sector 0, and `.data` LMA.
- [x] RTP-BR2-03: Freeze one ordinary firmware build configuration: stable 1.95.0,
  `opt-level="z"`, overflow checks, safe ICF with address-significance metadata,
  original CPU/link scripts and every function enabled. Integrate normalized path
  mappings without replacing required rustflags or inheriting host CC/CXX overrides.
  Update firmware README and diagram sources; retain the B-R1 candidate flags/map.
- [x] RTP-BR2-04: Review the complete phase, including authored tooling tests,
  linker scripts, flags and installer. Complete one mechanical formatting/provenance
  preparation for this snapshot and reconcile it with review. Then, only if explicitly authorized, perform
  size measurement **M1 once** on the builder. Retain errors, map, flags, identities
  and load-region report. No test or board run. A linker capacity miss with a complete
  map informs Phase 2; a compilation/tooling failure is retained and stops execution.

The first map determines actual tail occupancy and remaining reduction required for
16 KiB upper-region free space. It is not a prerequisite for selecting or authoring
already evidenced source improvements. Keep the M1 baseline free of those runtime
edits by using a frozen, bootstrapped measurement snapshot; do not overwrite concurrent
work or mix identities. A separate snapshot is a source copy, not an extra build.
No agent delegation is started by this plan. M1 does not authorize extra trials.
If M1 already satisfies the floor, skip unnecessary source reductions and M2 and use
that unchanged candidate for final acceptance.

#### Phase 2 — One selected source reduction set

- [x] RTP-BR2-05: Record a per-owner selection from the retained B-R1 map now:
  straightforward raw-ID maps/sets, shared assertion/sort/error helpers and cold
  dispatcher boundaries. Author them concurrently with placement/config/tooling
  work, preserving the M1 snapshot. Use M1 to adjust the remaining size budget,
  not to postpone independent authoring. Trace writing and proven-equivalent numeric
  narrowing follow the same selection. Keep rejected/retained rationale per lever;
  do not promise cumulative savings from overlapping symbols or an average floor.
- [x] RTP-BR2-06: Finish all selected implementation and native test authoring.
  Cover numeric limits and stable faults; ordering/cost exhaustion; instance/frame
  deletion, reuse protection, retained graphs and restart; output alias rollback;
  cache invalidation/misses; unchanged trace bytes and function/parameter registry.
  Preserve valid static-identity and zero-allocation tests; registry dispatch is
  demonstrated by native behavior, not merely function-pointer address equality.
  No per-edit tests, links or lint runs. Existing asserts/oracles are not weakened.
- [x] RTP-BR2-07: Obtain independent review of all changed source, tests, profile,
  tooling and reachable callers, including included Rust fragments. Complete the
  single formatting/provenance preparation and freeze before the final candidate.
  Only if explicitly authorized, perform size measurement **M2 once** of that exact
  default configuration. It is the last measurement attempt, not a flag matrix.
  Record the source/locks/toolchain/ELF/map hashes and map deltas against M1/B-R1.

Two measurement attempts are the explicitly authorized exception to the implementation-first
validation cadence, including each snapshot's one formatting/provenance preparation,
not recurring tests/lint or authorization granted by this document. A failed compile/link
counts as an attempt. There is no automatic retry, ICF-all fallback, opt-level variant
or third link hidden in acceptance. Freeze measured Rust/config bytes for validation;
if later corrections change them, the measurement no longer certifies that candidate.

#### Phase 3 — One consolidated software and conditional hardware batch

The unchecked B-R2 candidate-specific gates below remain historical failures.
Their software/ELF/physical requirements are satisfied by successor B-R3/B-R4
evidence; no claim is made that the failed B-R2 M2 image became qualified.

- [ ] RTP-BR2-08: Before execution, record one deduplicated command ledger using
  the B-R1 run-2 suite allocation plus new native assertions. Include core all-feature
  and portable suites, i686 allocation/behavior evidence, F401/C6 checks, hosted unit
  and affected integration/stdlib/restart suites, the required four runtime-vertical
  binaries, xtask/platform/firmware tooling, Clippy including portable/firmware lanes,
  cross-target warnings, both lock audits, architecture then diagrams/drift, and
  format/diff/provenance checks. No duplicate test-all or release guard in this scope.
- [ ] RTP-BR2-09: Inspect the exact M2 ELF (or unchanged M1 if Phase 2 was skipped).
  Require at least 16,384 B free in the upper region, sector-0 end <=0x08004000,
  safe vector/reset addresses, valid load segments and unchanged 72 KiB heap/16 KiB
  MSP reservations. Reuse its hashes and bytes; do not relink to obtain a passing map.
  A linked image below the headroom floor remains an unqualified candidate.
- [ ] RTP-BR2-10: If link/partition/headroom and required software prerequisites
  pass, use Scope B's existing device procedure in this same authorized batch:
  back up flash, program/verify the firmware sectors and separate application,
  preserve checkpoint sectors, replay the unchanged three artifacts, compare traces,
  check output-off states and IWDG reset, and measure live heap/stack/IRQ/timing.
  Confirm app-only installation preserves firmware-owned sector-0 data. No mass
  erase, option-byte change, fixture reduction or manual board-marking dependency.
- [x] RTP-BR2-11: Reconcile independent review, source and generated artifact identities;
  retain all attempted/failed/unrun steps. Close B hardware items only on real device
  evidence. If fit or hardware fails, collect independent software results in the
  authorized batch, leave dependent steps unrun, and stop without another build.

The 27.4 KiB i686 peak is a useful prior estimate only. Re-measure after container
changes; it excludes ARM layout, allocator metadata/fragmentation and MSP/IRQ stack.
Require actual heap fit and spec D.1's observed 2 KiB stack/IRQ margin, preserve fault
recording, and keep timing measurements separate from serial transmission. A smaller
function or a successful flash link does not establish a stack or timing bound.

No function is removed by this plan. If the two attempts cannot meet the full-function
acceptance floor, report the remaining measured obstacle and prepare a new scope;
do not silently reduce capabilities, hardware acceptance criteria or the toolchain pin.

Primary references checked during planning: [ST RM0368 §3.3/Table 5](https://www.st.com/resource/en/reference_manual/rm0368-stm32f401xbc-and-stm32f401xde-advanced-armbased-32bit-mcus-stmicroelectronics.pdf),
[cortex-m-rt 0.7.5 linker source](https://docs.rs/crate/cortex-m-rt/0.7.5/source/link.x.in),
[LLVM address significance](https://llvm.org/docs/Extensions.html#sht-llvm-addrsig-section-address-significance-table),
[LLD 22.1.2 ICF](https://raw.githubusercontent.com/llvm/llvm-project/llvmorg-22.1.2/lld/ELF/ICF.cpp),
[Rust function-pointer identity](https://doc.rust-lang.org/std/ptr/fn.fn_addr_eq.html),
[Rust static allocation identity](https://doc.rust-lang.org/reference/items/static-items.html),
[LLVM 22.1.2 stack-slot reuse](https://raw.githubusercontent.com/llvm/llvm-project/llvmorg-22.1.2/llvm/lib/CodeGen/StackColoring.cpp),
[Rust 1.95 remapping flag](https://raw.githubusercontent.com/rust-lang/rust/1.95.0/src/doc/rustc/src/command-line-arguments.md).


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

### Program continuation — B published, U verified/publication delegated, later scopes open

| Scope | Dependency and complete deliverable | Acceptance evidence |
|---|---|---|
| B — F401 bring-up (M2A-B), complete | A4; same engine/artifact on NUCLEO-F401RE, frozen 1.95 tuple, timer, process images, UART, IWDG and instrumented bring-up heap. | Physical trace agreement; reset/fault outputs; linked sections, preparation/RUN/heap peaks, native stack paint/MSP/IRQ headroom, numeric timing. B plus A closes M2A. |
| E — C6 bring-up (M2E) | A4 and an available C6 board; reuse the common corpus promptly to expose second-architecture assumptions. | Actual board execution, I/O/time/watchdog, software-float cost and memory evidence. No board means this gate stays open. |
| U — Modernization (M0U), scope-verified; publication authorized | Default after A+B, before H; E need not block it. Selected dependency/toolchain/edition/resolver/MSRV/Node migrations implemented and independently reviewed. | MOD-01 ledger, retained software batches and new physical F401 replay passed for the recorded tuple; final release guard/native CI/publication remain separate. |
| H — Full hosted adoption (M2B), current acceptance open | Shared execution/library coverage and runtime-only/full/development compositions are implemented. The historical C21 performance exception does not qualify subsequent candidates. Constructor/scan work exhaustion is closed by TIME-08/09 (runs 3 and 4) and register-call native stack depth by the admitted depth-33 tests (run 3). Hosted timing: the call-path work in the current checkpoint reaches parity on the diagnostic copy; run 5 on the frozen source, including renewed F401 size, stack and board evidence, is next. No H commit/push yet. | Saved-artifact and existing configuration/API/debug/HMI/protocol/restart/update workflows; native Linux/macOS/Windows evidence. Explicit native-OS limitations if not available before commit. |
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

### H and runtime qualification — expanded assurance plan (12 October)

Owner request: add all applicable test categories for this critical runtime. This
section turns spec 34 HOST-03/04/09, LOAD, MEM, TIME, UPDATE, COMPAT and TEST-01–05
into an explicit execution plan. Specs 10, 11 and 12 remain the behavior authority.
This is a plan, not evidence that a campaign ran or a claim of zero defects.
Unchecked items require a coverage audit and current evidence; they do not mean
that the named existing suites have never run. Reuse existing native assertions
before creating more tests, and retain intentional compatibility decisions.

**Order and scope.** Finish the active focused correctness/performance correction
first. Then audit current coverage and add missing native assertions. Run targeted
properties, fault injection, source mutations and bounded fuzz campaigns before
H's broad acceptance. Freeze source and campaign definitions before each authorized
run; deduplicate suites and reuse unchanged, source-bound evidence. The owner's
standing focused-retest authorization remains in force. Full acceptance/release
rules and later C/QH/QF/QE hardware/qualification obligations are unchanged. New
hardware, publication, platform features and a second execution engine are not
implied by this test plan. Tests of C-only functionality run when C implements it.

| Item and scope | Risk, required assertions and starting coverage | Evidence/pass criteria |
| --- | --- | --- |
| [x] A01 — Native contract coverage (H); done 15 October: batch 6 and rerun, guard 10, native CI green on `77fba46dc` | Audit each changed H behavior against a native assertion: construction versus installed writes, default/local initialization, owned/borrowed arguments, explicit NULL, OUT/IN_OUT aliasing, references, faults, restart and generation replacement. Start with core unit tests, `owned_runtime_state`, `checked_owner_references`, `checked_owner_images`, `source_free_cycle`, `resource_reconfiguration` and the runtime vertical. | Spec clause, actual test selector, feature/target and current source-bound result. Every identified behavior gap gets a native regression. Zero selected tests is not a pass. |
| [x] A02 — Executor differential replay (H); test-only gaps closed 15 October (`9a6960df9`); the four owner decisions implemented the same day (`87055c389`, `6c408df8f`: HOST-04 debugger attach and eviction rules, interpreted block entries, no deopt counters) | Run the same saved artifact and complete input/time trace through shared stack, register interpreter and Tier-1 where eligible. Include nested/recursive calls, first invocation, rare branches, debugger fallback, failure/unwind and state reuse. Start with `bytecode_vm_differential`, `portable_register_parity`, `runtime_core_compiler_free_load` and `portable_construction_parity`. | Compare each cycle's values, outputs, tasks, faults and supported restart state. Compare original U only for unchanged contracts; corrected legacy defects are not oracles. Record intended fallback instead of demanding every program specialize. |
| [ ] A03 — Property-based execution tests (H) | Generate bounded well-typed programs and values: argument permutations, defaults, alias graphs, arrays/strings, references and legal call trees. Cross-check executors and the documented invariants. | Fixed reproducible seeds and case/depth/size limits; shrinking or minimization for failures; independent expected properties rather than copying the implementation into the oracle. |
| [ ] A04 — Metamorphic tests (H) | Semantics-preserving transformations: positional versus equivalent named arguments, omitted versus explicit defaults where equivalent, serialization round trips, harmless POU/name reordering, and repeated preparation of the same artifact. | Same observable behavior under documented transformations; explicitly exclude transformations that change evaluation order, aliasing or numeric rounding. |
| [ ] A05 — Focused source-code mutation (H) | Use the existing mutation runner on changed guards and transitions: constant-write checks, staging restrictions, lifetime rejection, bounds, work/deadline limits, failed-candidate preservation, binding/copy admission and cleanup. Existing entry points are `verification/mutation-program.toml` and `scripts/verification/mutation_program_cli.py`; audit selectors against the current sources before execution. | A passing unmutated baseline, exact source/mutant identities and named tests. Every selected viable non-equivalent mutant must fail the intended assertion. Build failures, timeouts and unviable mutants are separate outcomes, not killed-by-test evidence. Investigate survivors; add missing assertions, remove dead code or document a reviewed equivalence/unreachable rationale. Historical mutation evidence remains immutable. |
| [ ] A06 — Byte-level container fuzzing (H/qualification) | Existing `fuzz/fuzz_targets/bytecode_container.rs` calls decode only, with a 1 MiB input cap. Exercise truncation, lengths, overflow, section order/overlap, checksums and supported-version boundaries. | No panic, hang or uncontrolled resource use within declared limits. Seed corpus, engine/tool version, duration, executions, coverage and minimized failures retained. Decoder survival does not certify validation or execution. |
| [ ] A07 — Structured validator/admission fuzzing (H/C/qualification) | Add or verify a harness that reaches decode → full validator → profile admission/preparation. Seed valid supported STBC versions and mutate instructions, types, references, calls, imports, initialization, storage/image demand and bound configuration. Recompute integrity fields where needed to reach deeper checks. Start from `bytecode_source_free`, `bytecode_validation_budget`, `bytecode_validation_scale`, `source_free_profile_admission` and saved artifacts. | Inputs rejected deterministically within declared work/memory bounds; no unvalidated executable state. A rejected candidate leaves the installed generation and outputs intact. Native deterministic regressions for every discovered defect. Mark the deeper harness missing until its code actually exists. |
| [ ] A08 — Stateful operation-sequence testing (H/C) | Generate valid and invalid sequences of load, install, start, scan, checked write, debug operation, stop, warm/cold restart and candidate replacement. Include stale references/generations, repeated failures and recovery. | Compare against a small explicit state-transition model from the spec. Assert legal transitions, ownership, retained state and failure atomicity after every operation. Bound both sequence length and resource demand. |
| [x] A09 — Resource-boundary and exhaustion tests (H/C); test-only gaps closed 15 October; the owner decisions implemented the same day (MQTT overflow, `SIZEOF` depth in spec 12, container bound first, bounded control queues, instance `Overflow`) | Test zero, one, exact-limit and limit-plus-one for stack/call/reference/type depth, instruction/helper work, strings/arrays, images, instances, validation/preparation memory and service queues with explicitly declared capacities. Include compound-size multiplication/offset overflow on 32-bit. | Correct stable fault and first-fault order; no premature publication, leak, stale handle or partially installed candidate. Cover `vm_resource_limit_cases`, `portable_execution_frames`, `bytecode_decode_resource_bounds` and allocation profiles. |
| [ ] A10 — Failure injection and transactional recovery (H/C) | Inject reservation refusal, clock/deadline expiry at different phases, driver/import failure, copyback failure, retain/checkpoint failure and failed replacement. Include failure after earlier parameters/outputs have been staged. Exercise non-returning driver/import callbacks under external supervision: verify independent recovery or rejection by the applicable capability/profile rule; an in-thread deadline poll cannot cancel a hung callback (PORT-02/TIME-03/TEST-01). | Snapshot pre/post logical state and journal/identity ownership; assert the spec's commit/rollback/safe-state boundary. Physical I/O is qualified separately—do not promise rollback of effects already sent to hardware. Retain the original error while cleanup finishes. |
| [ ] A11 — Ownership, aliasing and lifetime stress (H/C) | Nested arrays/structures, independent PLC copies, COW, reference escape, IN_OUT/OUT aliases, frame retirement, instance batches, retained graphs and re-entry. Repeat successful and failed operations with reusable scratch. Start with `memory_lifetime`, `bounded_value_semantics`, `checked_owner_references` and source-free restart tests. | No cross-instance/state contamination; dead references rejected; no stranded identities or unrecycled scratch after fault. Existing safety checks remain enabled. |
| [ ] A12 — Temporal ABI and physical layout (H) | TIME/DATE/TOD/DT and long variants, SIZEOF, relative field placement, nested/overlapping structures, direct/partial access, I/O encoding and external protocol units. Check 32-bit and 64-bit hosts and supported artifact compatibility boundaries. Start with `temporal_storage_abi`, `sizeof_semantics`, `datetime_range` and `io_hierarchy`. | Exact eight-byte product contract and explicit legacy/external conversions; no silent truncation, field overlap or hidden host-size dependence. Include minimum/maximum, negative and fractional-time cases. |
| [ ] A13 — Numeric and standard-library corpus (H/qualification) | Integer widths/conversions/overflow, REAL/LREAL rounding, subnormals, signed zero, non-finite rejection, division by zero, EXPT/libm range cases; timers, counters, triggers and bistables including edge transitions. Use `source_free_numeric`, `portable_native_functions`, existing IEC trace suites and pinned independent reference vectors. | Exact or tolerance-qualified oracle as NUM specifies, including stable faults and relevant near-threshold PLC decisions. Do not use the function under test to generate its own expected answer. All runtime functions remain enabled. |
| [ ] A14 — Scheduling and clocks (H/qualification) | Non-multiple task intervals (25 ms/10 ms), priority/due/index ties, missed periods, jumps, timer wrap/backward time, cold/warm state, event/single tasks and deadline expiry mid-operation. Include staggered and coincident fast/slow nonpreemptive task batches, delayed output publication and admission against SCHED-04. Start with `task_readiness_cases`, `tasks`, `scheduler_resource` and timer trace suites. | Nominal deadlines do not drift; misses counted and dropped without replay; cycle order and fault/output behavior match the spec. Logical-time evidence is separate from OS/hardware timing. |
| [ ] A15 — Retain, restart and installation (H/C/Q) | Warm/cold restart, retained alias/instance graphs, schema mismatch, corrupt/truncated checkpoints, interrupted journal/install and repeated recovery. Start with `runtime_restart`, `vars_retain`, `retain_integrity`, `source_free_restart_graph` and transaction trace suites. | Explicit last-flushed/committed guarantees, no mixed generations and unchanged active state after rejected candidates. Host fault injection precedes physical power-cut qualification for implemented persistence. |
| [ ] A16 — Concurrency and service boundaries (H/C/QH) | Debugger writes/forces, readers during replacement, cancellation, bounded queue saturation, shutdown and callback ordering, including externally supervised non-returning callback cases from A10. Exercise real host interleavings; use deterministic scheduling/model checking where a small actual synchronization unit permits it. | Single mutable execution owner, no lost/duplicated commands, stale-reader misuse, deadlock or unbounded control-thread wait. A mocked ordering test does not replace real host concurrency evidence. |
| [ ] A17 — Miri and sanitizers (H/C/qualification) | Select compatible core ownership/container/lifecycle tests for Miri. Run supported sanitizer builds on affected unsafe/FFI/native boundaries; audit actual unsafe registrations and dependencies. | Toolchain/flags and selected paths recorded; no UB/memory violation in the exercised scope. Unsupported tests/targets are explicitly unrun, not clean. Keep instrumentation separate from release performance evidence. |
| [ ] A18 — Memory/allocation and stack evidence (H/C/Q) | Counting allocator on construction, scan, first-use, rare errors, copying, restart/replacement and destruction; bound peak/live allocations and detect growth across repeated cycles. Native stack/recursion measurements and actual MCU stack paint/high-water. | Profile-specific bounds and headroom. Zero control-path allocator operations only where C's bounded profile requires and demonstrates it; hosted/admitted allocations attributed honestly. Heap/RSS, logical accounting and stack are separate measurements. |
| [ ] A19 — Composition, features and target matrix (H/qualification) | Full/runtime-only/development builds; std/no_std and register feature combinations; native Linux/Windows/macOS, 32-bit execution and isolated F401/C6 target graphs. Verify compiler/HIR/service exclusions and admitted import descriptors. | Same supported PLC semantics and declared capabilities. A cross-target cargo check is compile evidence only; Linux is not native Windows/macOS proof, and a firmware link is not a board run. |
| [ ] A20 — CLI, API, debugger, HMI and protocols (H); slice 1 (real-binary `ctl` journeys, tutorials, documented commands) done 15 October (`80ede29f5`) | Existing supported project/configuration/build/test-program/deployment workflows, checked engineering writes, authentication, debugger edits/forces/stepping, protocol image access, stale/invalid input age and quality, restart and published state. Assert that queued normal outputs cannot publish after STOP/FAULT/replacement and record fault-actuation completion separately (PORT-04/STATE-04). Include runtime vertical, relevant HMI/protocol/native integration suites, actual browser and minimum/current supported VS Code wiring journeys. | Rust assertions prove logic; live UI tests prove wiring/rendering. Use real supported protocol peers/device cases where required. Record unavailable services/platforms instead of substituting mocks for end-to-end claims. |
| [ ] A21 — Real-time and performance qualification (H/Q) | Finish H21's original-U release comparison first. Later measure wakeup latency, computation, output submission and physical completion under documented CPU/I/O/service interference. Capture first-use and tail latency, not averages only. | Exact artifacts/inputs/features/toolchain; native timing distributions, unchanged thresholds or explicit owner-approved tradeoff. Current corrections do not grant a waiver. Instrumented profiling is diagnosis, not release timing proof. |
| [ ] A22 — Hardware replay and disturbance (H22/B/E/Q) | Replay the same qualified artifacts on the connected F401 and the selected C6 when available; GPIO, timers, UART, watchdog, reset, actual I/O failures and declared interference. Re-measure flash, heap and stack for the changed build. | Reviewed physical topology and source/firmware/artifact identity, real trace agreement, safe outputs and measured resource margins. Keep historical bring-up distinct from current-build qualification. |
| [ ] A23 — Soak, lifecycle cycling and recovery (H/C/QH/QF/QE) | Define realistic long-running workloads, durations, restart/replacement counts, release phases, service/memory pressure and permitted maintenance before launch. For H/QH, start from §14.1's proposed 100,000-cycle integration workload at 10 ms, freezing its suitability and limits before execution. Define extended hosted soak separately under TEST-03; extend by declared risk and platform envelope. | No unexplained faults, drift, leaks or missed configured deadlines inside the declared nominal envelope. Injected failures have their own expected outcomes. Duration alone is not a reliability probability or WCET proof. |
| [ ] A24 — Coverage-guided review and final acceptance (H/Q) | Inspect line/branch coverage on changed failure/cleanup code and uncovered assertions; combine with mutation survivors and independent full-diff/spec review. After focused gates are green, execute the required deduplicated broad acceptance/release batch at its frozen identity. | Explain uncovered critical paths and remaining platform/qualification gaps. Coverage percentage, test count or a tool's exit code alone is not correctness. Commit/push/release still require the existing authorizations and exact-SHA guard. |
| [x] A25 — Public API and type-boundary rejection (H); audit and positive examples done 15 October; compile-fail tests with checked error codes (`RUSTC_BOOTSTRAP=1` doc-test lane in CI) and private debugger storage the same day (`887b84fd2`) | Compile ordinary external consumers and compile-fail cases for forbidden installed-state mutation, invalid validated-token construction, escaping borrows and unavailable internal execution traits. Include supported API migration examples and rustdoc cases. | Valid consumers build under the declared features; forbidden examples fail for the intended error, not an unrelated missing import/dependency. Public installed state cannot bypass checked mutation. |
| [ ] A26 — Source/compiler-to-artifact integration (H) | Existing lexer/parser/HIR/IDE/WASM tests plus real ST projects through compile, saved artifact, load and execution. Include TEST_PROGRAM workflows, configurations, namespaces, OOP/interfaces, alias/default lowering and local/static/edge/retain declarations. Exercise source diagnostics and valid programs, not only hand-built bytecode. | Correct diagnostics and lowering; source-built and compiler-free execution agree where promised. A hand-built fixture does not alone prove the source-authoring route. |
| [ ] A27 — Independent conformance/oracle checks (H/Q) | For critical numerics, timers/counters, scheduling and storage semantics, derive expected vectors directly from the owning IEC/product contract independently of the runtime implementation. Where an already available independent implementation/reference tool matches the contract, compare bounded common cases and document differences. | Independent expected values and documented precision/platform assumptions. Majority agreement is not authority over the spec; unavailable/licensed hardware or tools remain explicit limitations. |
| [ ] A28 — Bounded formal/model checks, feasibility first (C/Q) | Assess small pure high-risk kernels: checked offset/count arithmetic, deadline advancement, identity reuse/lifetime rules and transactional state machines. Use a supported bounded checker or explicit finite-state exploration where its assumptions can be stated. | Record model, bounds, assumptions and correspondence to production code; a bounded proof applies only to those bounds and properties. Tool availability and feasibility are open, and this is not a claim to formally verify the entire runtime. |

**Campaign preparation and evidence.** Before each mutation/fuzz/property/soak run,
record exact commands, toolchain, target/features, source and artifact identities,
seed/corpus identity, input/depth/memory/work limits, time/execution budget, CPU/disk
allocation, required invariants and stop/pass criteria. Start with focused critical
paths and expand after those pass. Do not launch an unbounded whole-workspace
mutation run while the focused behavior or performance gate is failing. Preserve
all failures and minimize reproductions into native tests before rerunning their
owning checks. Keep generated corpora/raw captures outside tracked source under
the existing evidence policy; retain reviewed reproducers and digest-bound reports.

**Current gap/state (13 October):** A06's existing fuzz target is decode-only;
no current coverage-guided fuzz result is claimed. A07 now has five native bounded
admission tests, including 768 fixed-seed integrity-corrected wire mutations on
both 64-bit and 32-bit hosts (`H-remaining-assurance-correction/`). They do not
close a generated execution/state-machine campaign. A05 has four current-source
function-removal mutants caught by native assertions after a 517-test baseline
(`H-critical-mutations/`), with the per-guard limitations recorded above. Historical
mutation report bindings remain advisory maintenance, not substitutes for these
runs. Other categories remain planned or supported only by their named partial
results; no blanket closure is implied. No finite set of passing tests establishes
zero bugs, certification, a mathematical WCET bound or a cross-platform guarantee
without the corresponding evidence.

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
| ARCH-01 | A1–A4, B/E/H/C; QH/QF/QE | A4 native evidence plus B-R4 run 8: unchanged saved artifacts execute on F401 with exact state/scheduling and labeled numeric tolerances. Other hosts/boards and production qualification remain E/H/Q. |
| ARCH-02 | A1–A4, B/E/H/C; QH/QF/QE | A4 compiler-free core is linked and physically executed by the isolated F401 composition in B-R4 run 8. ESP32 composition/hardware remains E. |
| ARCH-03 | A1–A4, B/E/H/C; QH/QF/QE | Core remains forbid(unsafe_code); F401 BSP unsafe registrations/review and architecture pass through B-R4 run 8. Other BSP qualification remains E/Q. |
| ARCH-04 | A1–A4, B/E/H/C; QH/QF/QE | A4 shared execution is used by the physical F401 clock/I/O/watchdog adapter. Further hosted composition and platform qualification remain H/E/Q. |
| ARCH-05 | C; QH/QF/QE | pending — open |
| ARCH-06 | A2/A3/A4/C; QH/QF/QE | A4 partial (A4-A/R): private prepared/state constructors enforce native import/state/clock capabilities and logical resource limits; unsupported raw Retain/Io and multiple-resource profiles reject explicitly. Timing and deployment admission remain C/qualification. |
| ARCH-07 | B/E; QF/QE | B partial: reviewed F401 thin adapter and its real GPIO/clock/watchdog evidence pass B-R4 run 8. Other platform and production obligations remain open. |
| ARCH-08 | A1/A4/H; QH | A4 partial (A4-H): selected existing hosted API, debug, restart, standard-library and register-tier regressions remain supported. Full hosted compositions, services, performance and native-OS qualification remain H/QH. |
| ARCH-09 | H; QH | pending — open |
| ARCH-10 | B/E; QF/QE | B partial: F401 uses the declared stable toolchain/full-function composition. C6 library checks pass; C6 hardware and final per-board qualification remain E/Q. |
| MODEL-01 | A2/A3/A4/H/C; QH/QF/QE | A4 partial (A4-E/A): serialized bytes, immutable PreparedModule and exclusively owned RuntimeState have separate ownership; saved-artifact construction is compiler-free. Installed-generation replacement remains H/C. |
| MODEL-02 | A2/A3/A4/H/C; QH/QF/QE | A4 partial (A4-E/R): artifact initializer/default bodies and native bindings are prepared before execution; first and repeated calls use the shared dispatcher. Allocation-free RUN, complete hosted plans/caches and bounded first-use qualification remain H/C. |
| MODEL-03 | A2/A3/A4/H/C; QH/QF/QE | pending — open |
| MODEL-04 | A2/A3/A4/H/C; QH/QF/QE | pending — open |
| BUILD-01 | A1/A2/A4/H/U; QH/QF/QE | A4 no-HIR consumer and B-R4 run-8 full F401 link/boot/replay pass. C6 firmware and other product graphs remain E/H/U. |
| BUILD-02 | A1/A2/A4/H/U; QH/QF/QE | B-R4 uses isolated locked firmware workspace and recorded full-feature no_std graph; F401 image executes. Broader feature matrix and modernization remain E/H/U. |
| BUILD-03 | A1/A2; U on modernization | A1 feature/map split preserved; locked portable CRC edge qualified in A2-R6. Toolchain/edition modernization remains U. |
| BUILD-04 | A1/A2/A4/H/U; QH/QF/QE | F401 canonical firmware link, section inspection, boot and physical execution pass B-R4 run 8. C6 remains library-check evidence, not physical proof. |
| BUILD-05 | A1/A2/A4/H/U; QH/QF/QE | Same shared Rust engine runs on F401; no platform semantic fork or reduced function set. Stable safe-ICF build remains measured; other target qualification open. |
| BUILD-06 | A1/A2/A4/H/U; QH/QF/QE | A4 partial (A4-H): compiler, services, host clocks/debugger/profiling and register-tier selection remain hosted adapters around shared execution. Full product composition work remains H. |
| BUILD-07 | A4/H; QH | A4 partial (A4-E/B): the standalone core consumer loads and executes without compiler/HIR. The hosted runtime-only product composition and its isolated feature graph remain H; this requirement is not closed. |
| BUILD-08 | H/U; QH | pending — open |
| BUILD-09 | A1/A2/A3/A4; C for optional no-CAS | A4 partial (A4-B): independent F401/C6 no-default engine checks and graphs pass. Arc/SmolStr bring-up ownership remains; atomic-retirement, no-CAS/C3 and bounded-compound qualification remain C. |
| HOST-01 | A4/H/C; QH | A4 partial (A4-H/E): the actual hosted stack dispatcher, call/reference operations and native semantics are shared with the source-free engine. Full hosted cycle/service adoption and all native OS qualification remain H/QH. |
| HOST-02 | A4/H/C; QH | A4 application-selectable logical limits retained; B-R4 run 8 measures the selected F401 fixture fit. This does not establish hosted capacity or arbitrary F401 program admission. |
| HOST-03 | A4/H/C; QH | A4 partial (A4-H): required runtime vertical, debug, restart, I/O, bytecode and selected compatibility APIs retain native regressions. Complete HMI/protocol/online-change and platform qualification remain H/C/QH. |
| HOST-04 | A4/H/C; QH | A4 partial (A4-H): existing register-tier APIs and stack/tier differential assertions are retained through the host adapter. Compiler-free optimized-plan preparation and bounded cache behavior remain H/C. |
| HOST-05 | A4/H/C; QH | pending — open |
| HOST-06 | A4/H/C; QH | pending — open |
| HOST-07 | H; QH | pending — open |
| HOST-08 | H; QH | pending — open |
| HOST-09 | H; QH | Contract selected in spec 34 §4.3.1. Non-executable builder, shared owned state, scratch debug types and candidate-observer isolation are authored with native/compile-fail regressions; all unverified. Installed hosted API and checked live-write caller migration are authored; full-review lifecycle and operational-fault corrections remain unvalidated after Run16. |
| SCHED-01 | A1/A4/H/C; QH/QF/QE | A4 cooperative readiness/order regressions retained; F401 run-8 oracle verifies nominal deadlines and 40 activations with zero misses. Complete host/C6 scheduling-profile qualification remains H/E/Q. |
| SCHED-02 | A4/H/C; QH/QF/QE | pending — open |
| SCHED-03 | A4/H/C; QH/QF/QE | pending — open |
| SCHED-04 | A4/H/C; QH/QF/QE | pending — open. Not applicable to H (A14 round 5): no timing-admission API or seam exists, and spec 34 (~line 1610) defers the full timing-admission record to M3/M4. H writes no accept/reject test for it. |
| SCHED-05 | A1 fix/native evidence; A4 fixture; B/E/H/QH/QF/QE parity | A1 nominal-deadline correction integrates through A4 saved fixture; F401 run 8 confirms 25 ms on 10 ms samples, 40 activations through 1000 ms and zero misses. Other host/C6 replay remains E/H/Q. |
| LOAD-01 | A2/A3/A4/C; QH/QF/QE | Fresh native and F401 run-8 consumers construct/execute saved STBC 2.0 without HIR or source. Other profiles and production admission remain E/H/C/Q. |
| LOAD-02 | A2/A3/A4/C; QH/QF/QE | A4 partial (A4-E/A): TYPE_TABLE, construction roots, task/I/O metadata and executable initializers supply fresh state; missing or inconsistent executable metadata fails admission. Complete later-profile coverage remains H/C. |
| LOAD-03 | A2/A3/A4/C; QH/QF/QE | A4 partial (A4-A): full validation is followed by native import/signature/hidden-state and clock-service checks plus resource/domain restrictions before RUN. Timing/deployment profile admission remains C. |
| LOAD-04 | A2/A3/A4/C; QH/QF/QE | A4 bounded decoder/preparation tests retained; F401 run-8 peak preparation heap is 28,272 B with zero failures. Arbitrary-profile allocator/stack bounds remain C/E/Q. |
| LOAD-05 | A2/A3/A4/C; QH/QF/QE | A4 partial (A4-R/H): dynamic indexes, reference identity/lifetime, typed assignments, constants, nested call/depth/fuel/deadline and arithmetic faults remain enforced. Broader profile/platform qualification remains open. |
| LOAD-06 | A2/A3/A4/C; QH/QF/QE | A4 partial (A4-A/R): executable imports resolve to shared implementations; native FB state/signatures and CURRENT_DT capability are checked. Physical/import blocking, cancellation and full bounded-callback contracts remain C. |
| LOAD-07 | A2/A3/A4/C; QH/QF/QE | A4 partial (A4-A/R): raw/validated containers are distinct from private PreparedModule admission and RuntimeState construction; public state exposes typed writes, not mutable adapter-trait storage access. Later deployment admission remains C. |
| LOAD-08 | A2/A3/A4/C; QH/QF/QE | pending — open |
| LOAD-09 | A2/A3/A4/C; QH/QF/QE | A4 partial (A4-A/R): bounded construction/preparation and failed restart preserve prior state; native output groups preserve caller destinations on conversion/write/budget failure. Online generation replacement and bounded overlap remain H/C. |
| LOAD-10 | A3/A4; B/E/QH/QF/QE | A3 artifacts construct fresh F401 roots/defaults/task/I/O state in run 8 through the shared dispatcher. Broader hosted/platform construction qualification remains E/H/C. |
| LOAD-11 | A3/A4; B/E/QH/QF/QE | A4 partial (A4-E/R): default/alias/member/local/static initializer bodies execute through the ordinary dispatcher with shared budgets and activation lifetimes. Legacy 1.x retains its hosted Expr adapter; full hosted retirement remains H. |
| MEM-01 | A1/A4/B/E/C; QH/QF/QE | F401 run-8 region inspection and physical peaks pass: heap 28,272/73,728 B, MSP 13,596/16,384 B, Value 32 B/alignment 8. Broader profiles, service/retain staging and production bounds remain C/E/Q. |
| MEM-02 | A1/A4/B/E/C; QH/QF/QE | F401 capacities remain fixed and checked; measured preparation/instantiation/RUN fit in run 8. Full worst-case capacity qualification for all admitted programs remains C/Q. |
| MEM-03 | A1/A4/B/E/C; QH/QF/QE | B-R4 compiled user calls use explicit VM continuations; depth 1–4 with SysTick passes on F401, boot peak leaves 2,788 B. Construction recursion still exists; exhaustive native-stack and allocation-free RUN proofs remain C/Q. |
| MEM-04 | C; QH/QF/QE | pending — open |
| MEM-05 | A1/A4/B/E/C; QH/QF/QE | A4 partial (A4-R/H): stable runtime fault identities are preserved through shared execution. Rich allocated diagnostic errors remain; fixed-size bounded fault records are still C work. |
| MEM-06 | A1/A4/B/E/C; QH/QF/QE | A4 partial (A4-R): aggregate/string/reference and shared-struct COW copies are charged before mutation; independent value semantics and transactional output groups are tested. Allocation-free bounded compound representation remains C. |
| MEM-10 | A4/C; QH/QF/QE | A4 partial (A4-A/R/Q): exclusive private EngineState borrows immutable PreparedModule; the public wrapper prevents untyped mutable-trait access. Bounded storage/lookup allocation qualification remains C. |
| MEM-11 | B/E/C; QH/QF/QE | B-R3/B-R4 targeted compact live storage and measured F401 bring-up feasibility are implemented. Complete bounded-compound storage and allocation-free RUN remain C; no broad closure claimed. |
| MEM-07 | A1/A4/B/E/C; QH/QF/QE | A4 partial (A4-R): mutable state borrows immutable prepared metadata and checks activation/reference lifetimes. Installed-generation replacement, readers and retirement remain H/C. |
| MEM-08 | A1/A4/B/E/C; QH/QF/QE | A4 partial (A4-E/R): artifact member/method/interface metadata supports FBs, inherited classes, dynamic receiver checks and shared method bodies. Broader admitted profiles and board qualification remain open. |
| MEM-09 | A1/A4/B/E/C; QH/QF/QE | Deferred beyond A1 foundations: no bounded-storage/admission qualification claimed. R3-M records slot size only; full requirement remains open for the listed later scopes. |
| PORT-01 | A4/B/E/H/C; QH/QF/QE | F401 run 8 exercises hardware timer/logical-time separation, deadline fault and independent watchdog. Clock wrap/epoch and all-platform service qualification remain E/H/C/Q. |
| PORT-02 | A4/B/E/H/C; QH/QF/QE | A4 partial (A4-A/R): explicit services, native capability checks and work/deadline failures are enforced. Returning test callbacks do not prove cancellation, blocking or allocation bounds; platform/import qualification remains C. |
| PORT-03 | A4/B/E/H/C; QH/QF/QE | pending — open |
| PORT-04 | A4/B/E/H/C; QH/QF/QE | pending — open |
| CYCLE-01 | A4/H/C; QH/QF/QE | A4 shared image sequencing retained; F401 run 8 samples PC13 and exercises injected input-image transitions driving real PA5 readback. Full platform transfer/completion contracts remain E/H/Q. |
| CYCLE-02 | A4/H/C; QH/QF/QE | F401 output publication and STOP/FAULT transitions pass real PA5 pad readback in run 8; A4 transactional failure regressions retained. General adapter completion qualification remains E/H/C/Q. |
| CYCLE-03 | A1/A4/H/C; QH/QF/QE | A4 order/miss regressions retained; F401 run 8 matches saved nominal-deadline state trace. Other host/C6 replay remains E/H/Q. |
| CYCLE-04 | A4/H/C; QH/QF/QE | A4 shared TON/TOF/TP tests retained; F401 saved TON fixture executes against the hardware timer in run 8. Complete timer/target qualification remains E/Q. |
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
| STATE-03 | A4/B/E/H/C; QH/QF/QE | F401 deadline fault latches and suppresses normal publication; physical safe-off and IWDG reboot pass run 8. Production configured-failure/supervision qualification remains E/H/C/Q. |
| STATE-04 | A4/B/E/H/C; QH/QF/QE | F401 run 8 turns previously high PA5 output off on STOP and deadline FAULT, with pad readback. General multi-driver confirmation and configured fault outputs remain H/E/C/Q. |
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
| COMPAT-03 | A1/A3/A4/H/C; QH/QF/QE | A4 native feature corpus retained; F401 run 8 replays the unchanged Boolean/integer/TON/array/interface/default/retain fixture. Broader feature/platform qualification remains E/H/C/Q. |
| COMPAT-04 | A1/A3/A4/H/C; QH/QF/QE | A4 partial (A4-E/R/H): exact integer/Boolean, scheduler, typed fault, copy-back and saved state assertions preserve selected observable contracts. Broader cross-platform exact-trace qualification remains open. |
| COMPAT-05 | A1/A3/A4/H/C; QH/QF/QE | A4 partial (A4-N): shared libm/conversion behavior and saved numeric expectations distinguish exact results/control outputs from operation-specific tolerances. No arbitrary cross-CPU bit identity is claimed. |
| COMPAT-06 | A1/A3/A4/H/C; QH/QF/QE | Saved A4 main/numeric bytes are unchanged; F401 run 8 matches all 101 main rows and numeric contract. Native Windows/macOS profile replay and physical C6 replay remain open. |
| NUM-01 | A1/A4/B/E/U; QH/QF/QE | F401 run 8 records 84 MHz and FPSCR 0 with pinned Rust/libm/build identity. Full numeric premises/environment qualification on other profiles remains E/U/Q. |
| NUM-02 | A1/A4/B/E/U; QH/QF/QE | A4 partial (A4-N): strict observations are separated from tolerance-qualified function results in saved-artifact assertions. Matching numeric/environmental contracts on every claimed target remain unqualified. |
| NUM-03 | A1/A4/B/E/U; QH/QF/QE | A4 partial (A4-N): saved numeric replay checks operation-specific reference tolerances and resulting threshold/command decisions; existing exceptional/width/fault suites remain. Full application/platform corpus remains open. |
| NUM-04 | A1/A4/B/E/U; QH/QF/QE | F401 run 8 passes three exact numeric bit checks, TIME/Boolean state and ten tolerance-qualified function results. Broader application/platform observations remain E/H/Q. |
| NUM-05 | A1/A4/B/E/U; QH/QF/QE | Full shared numeric/standard-library functions remain linked; F401 numeric fixture passes with 9,464 us maximum observed scan. Full target runtime/timing qualification remains E/H/U/Q. |
| PLAN-01 | A1–A4/B | A1–A4/M2A-H and physical F401 M2A-B bring-up are complete with B-R4 run-8 fixture evidence. This closes M2A only; later modernization/hosted/bounded-storage/production scopes remain. |
| PLAN-02 | A1/E; QE | A4 supporting evidence (A4-B): the C6 graph includes the common loader/executor. Physical ESP32 bring-up and qualification remain E/QE; F401 evidence cannot close them. |
| PERF-01 | A1/A4/B/E/U/H/C; QH/QF/QE | F401 run-8 observed main/numeric scan maxima are 5,798/9,464 us on the frozen 84 MHz profile. This is fixture timing, not general WCET or all-platform performance qualification. |
| PERF-02 | A1/A4/B/E/U/H/C; QH/QF/QE | B-R4 run-8 map/ELF, region reservations, physical preparation/RUN/heap/MSP and Value-layout records are retained. Other targets and production capacity qualification remain open. |
| PERF-03 | A1/A4/B/E/U/H/C; QH/QF/QE | No physical latency/timing distribution measured in A1; logical-time tests are R3-S, not timing qualification. Requirement remains open. |
| PERF-04 | A1/A4/B/E/U/H/C; QH/QF/QE | A1 reports bounded evidence and no speed/WCET claim (R3); performance acceptance remains open. |
| TEST-01 | A2/A3/A4/B/E/H/C; QH/QF/QE | A4 partial (A4-A/R/S): first-use/static-restart, hidden references, readonly/type/limit, initializer and output-failure cases extend the native corpus. Full disturbance, hang, service and deployment qualification remains open. |
| TEST-02 | B/E/H/U/C; QH/QF/QE | B-R4 run-8 source/ELF/UART identities, full successful physical capture, failed attempts and independent reviews are retained. Manual observations and production/all-platform qualification remain open. |
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

B-R1 run-2 closeout and prepared corrections: [execution record](../../../notes/runtime-portability/b-r1-execution.md#consolidated-software-validation-run-2). No checkboxes are closed from unrun corrected-source gates.


## U — Modernization scope

Current U checkpoint: scope-verified, independently reviewed and uncommitted.
Required Linux software and physical F401 evidence passed. Run 4's full native workspace passed 8,703
tests with no failures. Its five setup/harness failures were retained and closed
by the authorized supplements: run 5 passed the real database gate and browser
geometry; direct existing tests passed 3 build-command and 8 test-command cases,
and the full editor suites passed 519 tests each on VS Code 1.109.0 and 1.141.0.
Existing CI-green fixture commands passed build/validate and both ST tests in
JUnit/JSON. Tutorial 21's inherited instructions remain a documentation mismatch;
the successful CI fixture does not qualify those instructions.

The newly installed ELF `52c52312…` passed physical UART verification
`ba3fe270…`: 101 samples, 40 activations, no misses, numerics, GPIO safe outputs,
call depths 1–4 and IWDG reset. Upper flash headroom is 23,520 bytes; measured
heap/MSP peaks are 28,272/13,652 bytes, leaving 45,456/2,732 bytes. Checkpoint
sectors and firmware bytes around application installation are preserved.
Full earlier source review reconciles all 246 changed Rust owners; the incomplete
239-record manifest remains historical. No runtime/compiler selection change or
new helper was added for the final supplement. U publication is now separately
authorized and delegated; exact-SHA guard and public release remain pending.
Native Windows/macOS, C6 physical execution and production qualification remain
outside this U result. [U execution record](../../../notes/runtime-portability/u-execution.md)
owns source identities, failure history, review and evidence. A3/A4/B publication
completed through PR #132/v0.24.73 and remains the comparison baseline.

- [x] RTP-U-01: MOD-01 complete current dependency/tool/Git/vendor/transitive
  decision ledger with primary provenance and justified upgrade/replace/retain choices.
- [x] RTP-U-02: MOD-02/03 compiler/MSRV/edition/resolver and Node/CI/developer
  baseline aligned; lifetime/macros/unsafe API migration reviewed and implemented.
- [x] RTP-U-03: MOD-04 selected dependency API/feature/peer migrations and locks
  implemented across all first-party/native/WASM/frontend/firmware/fuzz compositions.
- [x] RTP-U-04: MOD-05 useful modern idioms apply only to concrete owning problems,
  with native regressions; no changed PLC numeric semantics or gratuitous rewrites.
- [x] RTP-U-05: Independent full-diff review, completed test authoring and one
  frozen consolidated MOD-06 batch with recorded affected runtime/platform/UI evidence.
- [x] RTP-U-06: New F401 image meets existing flash/heap/MSP margins and real
  trace/numeric/GPIO/watchdog rules; old physical results are not relabeled.
- [x] RTP-U-07: Reconcile source, selected/retained versions and every failed/unrun
  gate honestly; separate scope verification from publication and production qualification.


H focused run 7 finished at 15:43:59 UTC: 23 required steps, 11 passes and
12 compile failures, all caused by one E0308 in initializer seed construction.
The new selected-aggregate branch called an owned materialization API while the
existing branches borrow values. Source review missed that API return ownership.
The owning correction uses the existing borrowed reference read, preserving
visibility, owned-graph discovery, charge-before-clone and one clone. No extra
clone or reduced assertion is introduced. The exact failed 8,531-record snapshot
is archived (8,516 present, 15 deleted, zero mismatches). Independent source review
accepted the one-line correction; compilation and behavior remain unverified.
Focused run 8 is prepared with the same 23 commands; full acceptance stays open.


H focused run 8 completed at 17:03:18 UTC: 18 passes and five failures across
23 required steps. Portable stack (401 tests), portable register (414), i686
selection (305), DAP (95), both MCU core checks and no-dev graphs, cross warnings
and architecture passed. Core all-features passed 473 and failed two new owned
optimization fixtures. Hosted/default and no-driver unit runs exposed eight
unique failing test names across configurations. The raw failed 8,531-record
snapshot reconciles without mismatches. Remaining owning corrections: source-only
external declaration policy and namespace-function receiver classification; new
fixture corrections retain numeric EXPT fallback, clock optimized proof, exact
post-initialization debug frames, valid nonreserved input names and lint style.
Reviewed U database workflow/runner corrections are adopted into H with their
contract; H must execute its own tests and real database gate before acceptance.
No H commit/push, full acceptance or hardware result is claimed.
