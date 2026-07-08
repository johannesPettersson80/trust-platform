# Rust-first PLC - Complete Implementation Checklist

**Status:** implementation planning checklist, v1 (2026-07-03).
**Master:** `rust-support-architecture-spec-v1.md`.
**Companions:** `rust-plc-product-contract.md`, `rust-plc-runtime-contract.md`,
`rust-plc-implementation-board.md`, `rust-plc-vscode-workflow-v1.md`,
`rust-plc-sdk-macro-contract.md`, `rust-plc-codegen-digest-contract.md`,
`rust-plc-cli-json-contract.md`, `rust-plc-bootstrap-packaging-contract.md`,
`rust-plc-conformance-fixture-matrix.md`,
`rust-plc-architecture-ownership-map.md`,
`rust-plc-vscode-acceptance-journeys.md`.

This checklist is the implementation memory for the complete Rust PLC package.
It is intentionally broader than S1. Do not treat an unchecked item as optional
unless a later lead review explicitly removes it and records the reason.

Stable item IDs: each checkbox is addressable as
`RPLC-<section-number>-<ordinal>` in this v1 order, for example
`RPLC-9-011` is the eleventh checkbox under section 9. Do not reorder existing
checkboxes. New items append at the end of the relevant section or use an
alphabetic suffix such as `RPLC-9-011A`.

## 0. Standing Rules

- [ ] No implementation starts unless the active slice names its owning
      contract, test plan, conformance rows, and VS Code surface if visible.
- [ ] New runtime/SDK/CLI/IDE ownership follows
      `rust-plc-architecture-ownership-map.md`; no new god-files.
- [ ] ABI/SDK crates have full-map edge rules before landing.
- [ ] User-facing strings use the master claim vocabulary: proven, measured,
      classified, admitted, declared, validated. Do not use certified unless
      an external certification process exists.
- [ ] Generated artifacts are byte-deterministic and digest-pinned.
- [ ] Every CLI output consumed by VS Code is versioned JSON, never scraped
      text.
- [ ] Every visible VS Code Rust PLC workflow is reachable without command
      palette or terminal.
- [ ] New VS Code surfaces use the shared truST webview theme tokens and have
      light/dark/high-contrast evidence.
- [ ] Every failure-mode row F1-F23 has at least one positive fixture, one
      negative fixture where meaningful, and a proof surface.
- [ ] Runtime changes run vertical tests before completion:
      `api_smoke`, `debug_control`, `complete_program`,
      `runtime_reliability`.
- [ ] VS Code changes run extension lint, compile, and registered extension
      tests; interactive gaps require screenshot or recording evidence.
- [ ] Broad gates run on `trust-builder` before declaring a milestone done.
- [ ] Any ownership/execution-flow change updates diagrams and architecture
      checklist entries before release readiness.
- [ ] Before S1 implementation starts, resolve the runtime determinism leak at
      `crates/trust-runtime/src/stdlib/time.rs` (`SystemTime::now()`): either
      route through the runtime clock/deterministic time source or document a
      host-only boundary that Rust POU execution cannot call.

## 1. Contract Hardening Before Code

- [ ] Verify master spec section 15.2 contains RS-118..RS-126 from
      `rust-plc-vscode-workflow-v1.md`.
- [ ] Verify product contract section 3 contains the IDE first-contact flow.
- [ ] Verify runtime contract contains BC-1..BC-6 backend seams.
- [ ] Verify implementation board contains S13-S17 VS Code slices and G0-in-IDE.
- [ ] Verify VS Code acceptance journeys J-R1..J-R6 exist in
      `rust-plc-vscode-acceptance-journeys.md` and are mirrored into the
      broader UI/UX acceptance board when implementation starts.
- [ ] Confirm D25 bootstrap SDK distribution is v1 crates.io exact pins; any
      bundled/offline SDK source is a later explicit decision.
- [ ] Confirm D26 deploy target inventory ownership is fleet-level inventory;
      projects reference target names only.
- [ ] Freeze the first version of the `trust check --json` envelope.
- [ ] Freeze the first version of the instance snapshot envelope.
- [ ] Freeze the first version of generated artifact manifest/digest metadata.
- [ ] Reconcile active-slice SDK trait shape against master section 6.1
      before implementing macros or public traits.
- [ ] Freeze the v1 sequence macro allowlist; initial allowlist is `iec!(...)`
      only.
- [ ] Freeze the fault-code namespace in SDK, generated declarations, and
      Live Values rendering.
- [ ] Decide how each active implementation worktree receives this package:
      tracked repo files are preferred; copy-only handoff is not sufficient.

## 2. Workspace And Crate Skeleton

- [ ] Add `trust-module-abi` crate.
- [ ] Add `trust-plc` crate.
- [ ] Add `trust-plc-macros` crate if proc macros are split from `trust-plc`;
      otherwise document why a split is unnecessary.
- [ ] Add `trust-sim` crate.
- [ ] Add `trust-service-sdk` crate when S9 begins, not before.
- [ ] Register allowed workspace dependency edges.
- [ ] Ensure ABI crate is `no_std` where practical and has no host dependency.
- [ ] Ensure SDK crate does not depend on `trust-runtime`, `trust-hir`,
      `trust-lsp`, VS Code, or web code.
- [ ] Add API snapshot or semver guard for public SDK/ABI surfaces.
- [ ] Add unsafe-site register for ABI/module host boundary code.
- [ ] Add deny/audit policy for new crates.
- [ ] Add crate-level docs that state safe user-code boundary and TCB boundary.

## 3. S1 - First Light

Goal: one Rust `PROGRAM`, one task, generated ST, static module host, panic
boundary, timing telemetry, one machine test, minimal VS Code visibility.

- [ ] Implement minimal `trust-module-abi` descriptor and call vtable.
- [ ] Implement minimal `trust-plc` traits: `PlcProgram`, `Exchange`,
      `CycleCtx`.
- [ ] Implement field attributes: `#[input]`, `#[output]`, `#[retain]`,
      `#[safe_default]`.
- [ ] Implement `#[trust_program]` only for the S1 shape.
- [ ] Add static built-in module registration path.
- [ ] Add runtime module-host subsystem as a new module.
- [ ] Add panic boundary with no panic crossing ABI.
- [ ] Wire module calls into the existing task/program execution slot.
- [ ] Record module invocation timing through the existing metrics path.
- [ ] Apply module fault reaction through existing fault/safe-output machinery.
- [ ] Generate the minimal ST declaration/configuration needed for the
      compile path.
- [ ] Generate deterministic runtime/io artifacts needed by the S1 fixture.
- [ ] Add digest metadata and F22 drift check for generated artifacts.
- [ ] Add minimal `trust-sim` wrapper over the existing harness.
- [ ] Add motor-latch S1 fixture: start/stop inputs, motor output, retained
      counter.
- [ ] Add machine test proving start/stop, retained warm restart, and scan
      continuation after panic.
- [ ] Add fault-injection test for F1: panic -> FAULTED, safe defaults,
      scan continues.
- [ ] Add digest drift test for F22.
- [ ] Add thin Live Values instance snapshot for name, state, fault code,
      overruns, and safe-output state.
- [ ] Add S1 evidence transcript and screenshot if VS Code surface is used.
- [ ] Run targeted runtime tests and remote full gate before closing S1.

## 4. S2 - Builtin Migration And Conformance Rig

- [ ] Migrate first builtin POU to the SDK path.
- [ ] Migrate all builtin FBs including `ROBOT_P3MINIMALARM`.
- [ ] Add differential tests proving behavior unchanged from the old path.
- [ ] Add unchanged legacy ST corpus acceptance: byte-identical outputs over
      N cycles on the migrated runtime.
- [ ] Add warm restart from a pre-migration retain file.
- [ ] Add cycle-time parity benchmark for migrated builtin dispatch on the
      target/remote builder; statistically significant regression blocks S2.
- [ ] Add failure injection fixtures for F1, F2, F7, F10, F11.
- [ ] Add retain module-blob codec section with version bump.
- [ ] Add retain migration and mismatch tests.
- [ ] Add conformance runner that can execute module fixtures uniformly.
- [ ] Add CI grouping for module conformance.
- [ ] Remove duplicate native call paths or mark deprecated forwarding paths.

## 5. S3 - Brownfield Rust FB

- [ ] Implement `{external module := ..., digest := ...}` pragma semantics.
- [ ] Add parser/HIR support using the existing attribute micro-syntax.
- [ ] Implement `trust module declare`.
- [ ] Generate ST declarations for Rust FB exports.
- [ ] Implement tier-1 EXTERNAL call path.
- [ ] Add B0 fixture: existing ST project gains one Rust FB.
- [ ] Prove ST LSP completion and hover for the declared Rust FB.
- [ ] Prove induced Rust fault is supervised by ST and visible in Live Values.
- [ ] Prove no Rust-first greenfield features are required for B0.
- [ ] Add B0 VS Code evidence rows and screenshots.

## 6. S4 - Cycle Sequences

- [ ] Implement `#[trust_sequence]` parser/validator/lowerer.
- [ ] Reject every non-admitted `.await` construct with F23 diagnostics.
- [ ] Reject async blocks, async closures, executor APIs, hand futures,
      unknown macros, and wait-free loops.
- [ ] Implement wait points: `next_cycle`, `wait`, `wait_until`,
      deadline-decorated wait, `all`, `race`, child sequence.
- [ ] Implement admitted macro allowlist with `iec!(...)` as the only v1
      admitted macro; add positive fixture.
- [ ] Implement closure-scoped `Seq` accessors:
      `read`, `write`, `retain_read`, `retain_write`.
- [ ] Ensure exchange/retain borrows cannot cross wait points.
- [ ] Store only explicit sequence state.
- [ ] Restart sequences from top on warm/cold restart.
- [ ] Add F21 sequence deadline fault with source location and fault code.
- [ ] Add runtime stuck-wait backstop.
- [ ] Add sequence TestBench support.
- [ ] Add F23 rejection matrix fixtures.
- [ ] Add rust-analyzer-friendly span requirements and tests where practical.
- [ ] Add VS Code wait-point rendering contract for later S14.

## 7. S5 - Dynamic Native Modules And Toolchain

- [ ] Implement cdylib loader behind a new module-host boundary.
- [ ] Implement handshake order: symbol, size, abi major, fingerprint,
      digest, limits.
- [ ] Implement F8/F9/F12 rejection paths with expected-vs-found diagnostics.
- [ ] Implement `trust module new`.
- [ ] Implement `trust module check`.
- [ ] Implement `trust module build`.
- [ ] Implement `trust module test`.
- [ ] Implement `trust module package`.
- [ ] Implement module signing/deploy-key handling.
- [ ] Implement `modules/` bundle discovery.
- [ ] Implement reload parity for body-compatible module changes.
- [ ] Add skew matrix tests.
- [ ] Add out-of-tree module fixture.
- [ ] Add package artifact tests for supported target triples.

## 8. S6 - Rust-first Greenfield Surface And Demo G0

- [ ] Implement `trust new rust-plc`.
- [ ] Scaffold the full project layout from the codegen contract.
- [ ] Include protective defaults.
- [ ] Implement `trust.toml` parsing and validation.
- [ ] Implement full Rust-first codegen for declarations, configuration,
      runtime/io artifacts, and generated ST.
- [ ] Implement `trust check` pipeline for Rust PLC.
- [ ] Implement `cargo trust` as the D24 shim for the `trust` umbrella.
- [ ] Productize `trust-sim` for cargo tests.
- [ ] Add proptest strategy support.
- [ ] Add G0 motor-latch fixture with one task, one program, two inputs, one
      output, one retain, one sequence, and one machine test.
- [ ] Print development-grade admission report even before measured admission.
- [ ] Add generated-ST diff artifact to G0 evidence.
- [ ] Add G0 CLI transcript.
- [ ] Add G0-in-IDE run through VS Code visible surfaces.
- [ ] Time the IDE flow under 10 minutes or revise the claim.

## 9. S7 - Timing Admission

- [ ] Implement `.trusttime` admission record format.
- [ ] Implement base-frame/hyperperiod simulation for current scheduler.
- [ ] Enforce periods as integer multiples of base scan interval unless a
      later scheduler contract replaces this rule.
- [ ] Include runtime overhead, I/O, retain, recorder, service exchange, and
      jitter margin in frame work.
- [ ] Report worst frame first with coincident tasks named.
- [ ] Add conservative fallback when hyperperiod is too large.
- [ ] Add event-task conservative modeling and min-interarrival support.
- [ ] Add target measurement harness.
- [ ] Add deploy gate for missing/stale `.trusttime`.
- [ ] Add runtime admission drift diagnostic.
- [ ] Add F18 runtime fixture in S7; VS Code rendering can wait for S15 but
      the runtime diagnostic cannot.
- [ ] Add regression fixture that passes per-task in isolation but fails on
      coincidence frame.
- [ ] Add VS Code Admission report proof.
- [ ] Add Problems anchors for F16.

## 10. S8 - Crate Admission

- [ ] Implement crate knowledge base location and governance.
- [ ] Implement static dependency graph classification.
- [ ] Implement symbol/path scans for known blocking APIs.
- [ ] Implement allocation tripwire integration.
- [ ] Implement syscall/audit integration where available.
- [ ] Implement `.trustcrate` record format.
- [ ] Implement policy gates for unknown, service-only, deny-listed, unsafe,
      license, and advisory outcomes.
- [ ] Anchor F17 diagnostics to `Cargo.toml` dependency lines.
- [ ] Add `reqwest` in scan path refusal fixture.
- [ ] Add admitted math crate fixture with evidence grade.
- [ ] Add VS Code crate verdict rendering proof.

## 11. S9 - Services And Typed I/O

- [ ] Implement `trust-service-sdk`.
- [ ] Implement `[services.*]` codegen and supervisor template.
- [ ] Implement `ServiceLink` freshness/value/age APIs.
- [ ] Implement service liveness diagnostics.
- [ ] Implement ADS typed I/O import for Rust structs.
- [ ] Add EtherCAT variant only when the runtime driver supports it.
- [ ] Keep one I/O binding truth; do not create a Rust-only I/O runtime path.
- [ ] Add stale service fallback fixture.
- [ ] Add service garbage/write validation fixture.
- [ ] Add VS Code freshness rendering in Live Values.

## 12. S10 - Record And Replay

- [ ] Implement `.trusttrace` recorder in window mode.
- [ ] Implement flight-recorder dump on fault.
- [ ] Use bounded overhead and record it against budget.
- [ ] Implement shared-memory publication if required by existing telemetry
      path.
- [ ] Implement `trust replay --json`.
- [ ] Implement `trust diff` or equivalent replay diff output.
- [ ] Implement `trust explain-overrun`.
- [ ] Add replay assertions in `trust-sim`.
- [ ] Add commissioning trace replay test.
- [ ] Add old-vs-new divergence gate.
- [ ] Add overrun explanation fixture naming module and cycle.
- [ ] Add VS Code Testing and Replay Trace proof.

## 13. S11 - Wasm Form

- [ ] Implement wasmtime host boundary.
- [ ] Implement epoch/fuel interruption.
- [ ] Implement `--form wasm`.
- [ ] Implement `--form both`.
- [ ] Prove both forms conform for one module source.
- [ ] Add F4 hard-loop trap fixture.
- [ ] Add F6 memory violation fixture.
- [ ] Add deterministic fuel replay fixture.
- [ ] Document performance overhead and evidence grade.

## 14. S13 - VS Code Rust-first Shell

- [ ] Implement `projectKind` model.
- [ ] Detect Rust PLC by `trust.toml` and `Cargo.toml`, with precedence over
      `trust-lsp.toml`.
- [ ] Add create-project kind picker.
- [ ] Shell Rust scaffold through bundled `trust new rust-plc`; no TypeScript
      scaffold twin.
- [ ] Wire Compile/Check to `trust check --json`.
- [ ] Wire Run to Rust PLC simulator/debug launch handshake.
- [ ] Add generated-ST read-only and diff affordances.
- [ ] Add Rust starter examples.
- [ ] Add Rust walkthrough.
- [ ] Add extension tests for single-root, multi-root, cancel, and conflict.
- [ ] Add no-command-palette acceptance proof.

## 15. S14 - Live Values Instances

- [ ] Implement instance snapshot backend contract.
- [ ] Add Instances tab without expanding `ioPanel.ts` as a god-file.
- [ ] Render task, instance, type, state, fault code, overruns.
- [ ] Render timing p99 vs budget and evidence grade.
- [ ] Render sequence wait point with file/line and deadline elapsed.
- [ ] Render inputs, outputs, retain, variable fields, units, ranges,
      addresses, quality, stale state, and safe defaults.
- [ ] Render fault expansion with source link, reset action, and recorder link.
- [ ] Preserve existing I/O map behavior and force/write/release semantics.
- [ ] Add capability matrix tests and screenshots.

## 16. S15 - Admission In The IDE

- [ ] Implement Admission report model.
- [ ] Implement Admission report webview using shared theme tokens.
- [ ] Render worst frame, hyperperiod strip, contributors, evidence chips,
      crate verdicts, remedies, and profile.
- [ ] Map F16/F17 to Problems entries and report actions.
- [ ] Render drift chip in Live Values and Devices & Connections.
- [ ] Add light/dark/high-contrast screenshot evidence.
- [ ] Add claim-vocabulary string test.

## 17. S16 - Machine Tests And Traces In The IDE

- [ ] Discover Rust PLC tests through cargo/libtest JSON or an equivalent
      versioned contract.
- [ ] Group Unit, Machine, and Replay tests in VS Code Testing.
- [ ] Attach shrunk traces to failing machine tests.
- [ ] Add Replay Trace action.
- [ ] Render divergence report.
- [ ] Add registered extension tests for discovery, run, failure, replay.

## 18. S17 - Deploy Gates In The IDE

- [ ] Render Rust deploy gate checklist in selected runtime/device node.
- [ ] Keep sidebar Deploy as launcher only.
- [ ] Preserve disabled-with-reason behavior until backend exists.
- [ ] Gate on signed bundle, fresh `.trusttime`, fresh `.trustcrate`,
      evidence floor, and target capability.
- [ ] Render F20 stale/missing record refusal with artifact and fix command.
- [ ] Add no-dead-button proof.

## 19. S12 - Productization And Public Documentation

- [ ] Add public Rust PLC develop guide.
- [ ] Add reference guide for SDK attributes and sequence syntax.
- [ ] Add module author guide.
- [ ] Add service author guide.
- [ ] Add operator guide.
- [ ] Add migration guide from ST to Rust FBs.
- [ ] Add example gallery entries for Rust starters.
- [ ] Add VS Code workflow guide.
- [ ] Add troubleshooting pages for F1-F23.
- [ ] Ensure public docs do not exceed evidence grade.
- [ ] Update changelog/version when behavior ships.
- [ ] Run public docs build and screenshot capture where applicable.
- [ ] Add CI workflow rows for Rust PLC fast/slice/full fixture gates.
- [ ] Add release workflow rows for VSIX `trust` binary, templates, SDK pin
      verification, version manifest, and schema fixtures.
- [ ] Add release evidence checklist separating crates.io SDK version, VSIX
      version, `trust` version, schema version, and GitHub release state.

## 20. Final Package Readiness

- [ ] All S1-S17 slice gates closed or explicitly deferred with lead review.
- [ ] All F1-F23 fixtures green.
- [ ] All BC-1..BC-6 schemas frozen and covered by fixtures.
- [ ] All generated artifacts are byte-stable across repeated builds.
- [ ] All Rust PLC templates pass `trust check`, cargo tests, and replay tests.
- [ ] CLI flows pass in CI without VS Code.
- [ ] VS Code flows pass without terminal or command palette.
- [ ] Remote builder broad gates green: fmt, clippy, test-all.
- [ ] Runtime vertical tests green.
- [ ] VS Code lint/compile/npm test green if extension touched.
- [ ] Browser/webview screenshot evidence attached for visible surfaces.
- [ ] Diagrams and architecture ownership map are current.
- [ ] Product contract claims match shipped evidence.
- [ ] Release hygiene complete if user-visible behavior ships.
