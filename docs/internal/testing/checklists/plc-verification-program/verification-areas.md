# Verification Areas and Initial Seeds

This document owns area boundaries, invariant classes, harness expectations, and
initial high-risk seeds. It is intentionally separate from implementation rows
so area ownership can evolve without rewriting the board.

## Compiler and IEC Frontend

Machine area: `compiler_iec`.

Owns:

- `trust-syntax`
- `trust-hir`
- IEC standard decisions/deviations
- diagnostics and type checking contracts

Required invariant classes:

- Lexing/parsing never silently drops valid/invalid constructs without a
  diagnostic.
- Parser recovery cannot loop forever.
- HIR error-severity diagnostics block lowering.
- IEC type rules match spec or documented deviations.
- Implicit conversions are materialized or rejected before runtime.
- Initializers, retain declarations, arrays, structs, enums, references, OOP,
  and standard functions have conformance cases.

Harnesses:

- parser snapshot tests,
- HIR semantic tests,
- IEC conformance cases,
- parser/source fuzz,
- mutation tests for type rules and diagnostics.

## HIR to Bytecode to VM Seam

Machine area: `bytecode_vm`.

Owns:

- lowering,
- bytecode encoder,
- bytecode validator,
- stack VM,
- register IR and tier paths,
- runtime value semantics,
- `crates/trust-runtime/src/bytecode/**`,
- `crates/trust-runtime/src/runtime/vm/**`,
- `crates/trust-runtime-core/src/bytecode/**`,
- `crates/trust-runtime-core/src/vm/**`,
- `crates/trust-runtime-core/src/value/**`.

Required invariant classes:

- Declared types and runtime tags cannot diverge silently.
- Reference lifetimes cannot escape valid owner/frame.
- Bytecode accepted by `apply_bytecode_bytes` satisfies VM assumptions.
- Unencodable source fails closed.
- Backend parity is not used as the sole correctness oracle.
- Malformed bytecode fails to load, not later at arbitrary read/write.

Harnesses:

- crafted bytecode negative tests,
- ST expected-value cases with IEC/spec oracle,
- engine parity tests as secondary guards,
- malformed bytecode fuzz,
- mutation shards for validator modules.

This is the first implementation pilot after specification-source inventory.

## Runtime Safety

Machine area: `runtime_safety`.

Owns:

- scan-cycle execution,
- scheduler and deadlines,
- watchdog,
- panic containment,
- safe-state,
- retain/restart,
- process image,
- runtime lifecycle,
- runtime force/write/release semantics.

Required invariant classes:

- A panic cannot silently kill the scan loop while status remains healthy.
- Deadline/watchdog behavior is armed and visible.
- Stop/fault safe-state behavior is applied or reported as not-safe.
- `on_error = warn/ignore/fault` matches config and never blocks the scan
  thread.
- Retain save/load/restart behavior is deterministic and fail-closed.
- Process image bounds are enforced.
- Startup and shutdown are bounded and observable.
- Force/write/release lifecycle is bounded, authorized, visible, and specified
  across disconnect, stop, restart, and debug pause.

Harnesses:

- runtime vertical tests,
- slow-device and worker-queue tests,
- signal smoke tests,
- retain corruption/restart tests,
- panic/deadline/IO fault injection,
- debug control and force/write/release tests,
- long soak.

## Control, Security, and Debug Authority

Machine area: `control_security`.

Owns:

- runtime control API and control-socket authorization,
- RBAC roles and token behavior,
- HMI write authorization boundaries,
- debug write/force/release authorization,
- secret handling and audit/status visibility.

Required invariant classes:

- Viewer/default-level access cannot perform control writes, debug writes, or
  force outputs.
- Authorized writes are visible, bounded, and auditable.
- Force/write/release lifecycle has specified stop, disconnect, restart, and
  debug-pause behavior.
- Control and debug surfaces expose stable error/status identifiers.
- Secrets and tokens are never printed in diagnostics, logs, or public reports.

Harnesses:

- API/RBAC negative and allowed-path tests,
- runtime debug control tests,
- VS Code debug journey evidence when user-visible,
- HMI write-authz tests,
- security/release gate checks for secret leakage.

## Protocol and Connectivity

Machine area: `protocols`.

Owns:

- Modbus,
- MQTT,
- EtherCAT,
- ADS,
- OPC UA,
- GPIO,
- connector status/discovery surfaces.

Required invariant classes:

- Discovery labels do not overstate protocol truth.
- Stale data is marked stale/degraded and cannot appear fresh.
- Reconnect loops are bounded and observable.
- Scan cycle does not wait on slow field devices.
- Output handoff semantics are explicit: level/latest-value vs edge/pulse.
- Real hardware claims have lab proof.

Harnesses:

- protocol unit tests,
- loopback integration tests,
- runtime comms conformance gate,
- protocol fuzz smoke,
- device-in-loop env-gated tests,
- hardware lab release suite.

## Editor and Source Transformation Safety

Machine area: `editor_safety`.

Owns:

- `trust-ide`,
- `trust-lsp`,
- `trust-debug` DAP protocol surface,
- VS Code extension tests where behavior crosses the protocol boundary.

Required invariant classes:

- Rename cannot silently change symbol binding.
- Source edits use correct LSP position encoding.
- Cancellation cannot publish false empty diagnostics.
- Closed dirty buffers revert to disk truth.
- Eviction cannot remove semantic sources from project analysis.
- Import/export cannot synthesize invalid ST from unsupported bodies.
- Debug adapter requests cannot bypass runtime authorization or hide forced
  output state.

Harnesses:

- IDE unit/integration tests,
- LSP protocol tests,
- VS Code extension tests,
- Unicode/incremental-edit fuzz or property tests,
- workspace-size performance tests,
- debug adapter/session tests.

## PLCopen Import and Developer Tooling

Machine area: `plcopen_devtools`.

Owns:

- `trust-plcopen`,
- PLCopen import/export fixtures,
- `trust-dev`,
- developer/workbench CLI helper flows.

Required invariant classes:

- PLCopen import rejects unsupported executable non-ST bodies loudly.
- Benign vendor metadata does not cause valid ST to be skipped unless
  documented.
- Import/export never synthesizes garbled ST from arbitrary XML text.
- Developer test discovery cannot silently skip supported source files.
- Developer helper commands do not hide failing tests or commit unintended
  staged work.

Harnesses:

- PLCopen import/export tests,
- vendor-export corpus checks,
- malformed XML tests,
- `trust-dev` CLI tests,
- command fixture tests.

## HMI, Web, and UI Acceptance

Machine area: `hmi_ui`.

Owns:

- HMI web UI,
- VS Code webviews,
- Devices and Connections,
- Live Values,
- journey evidence.

Required invariant classes:

- UI state matches runtime truth.
- Status vocabulary is consistent across surfaces.
- Writes/force/release are authorized and visible.
- Browser-visible changes have screenshot or Playwright/VS Code proof.
- User journeys cannot be accepted from stale screenshots.

Harnesses:

- runtime API tests,
- browser Playwright captures for web UI,
- VS Code extension tests,
- VS Code acceptance journey runners,
- PNG hygiene and structural acceptance audit.

## Security, Supply Chain, and Platform Integrity

Machine area: `supply_chain_platform`.

Owns:

- dependency policy and vulnerability/license gates,
- npm/cargo lockfile integrity,
- release artifact identity,
- packaged binary/VSIX provenance,
- platform matrix and path behavior.

Required invariant classes:

- Release artifacts correspond to the tested commit.
- Dependency exceptions have owner, reason, and expiry.
- Lockfiles are present and intentionally changed.
- Public security/platform claims match gates.
- Platform-specific path/socket behavior is either supported and tested or
  explicitly unsupported.

Harnesses:

- cargo/npm audit or deny-style gates where configured,
- license/provenance checks,
- package smoke tests,
- version-release guard,
- cross-platform path hygiene tests,
- release artifact checksum/provenance checks.

## Release and Public Claims

Machine area: `release`.

Owns:

- changelog/version/tag/release proof,
- public docs,
- conformance public summary,
- hardware guide labels.

Required invariant classes:

- Changelog/version/VSIX versions stay synchronized.
- Public docs do not claim unverified hardware support.
- Release artifacts correspond to tested commit.
- GitHub latest release matches shipped version.
- Conformance reports are generated from the suite, not hand-written.

Harnesses:

- release preflight scripts,
- version-release guard,
- public docs IA/search checks,
- release workflow artifact checks.

## Required Specification Matrix Seed Scope

`VERIF-P1A-010` turns this scope into `verification/spec-matrix.toml`. The
matrix is keyed by the machine areas above and by coverage tags, not by these
human headings.

Initial required spec tags by area:

- `compiler_iec`: ST syntax/parser recovery, operator precedence, IEC
  decisions/deviations, type system, implicit conversions, subranges, strings,
  arrays/structs/enums, references, OOP/method dispatch, `VAR_IN_OUT`,
  standard functions/FBs, timers/counters, retain/init/reset semantics, and
  external IEC source availability.
- `bytecode_vm`: HIR diagnostic gate, lowering/fail-closed contract, bytecode
  format, bytecode validator semantic contract, VM value/tag semantics,
  reference and owner semantics, stable trap/error-code model, deterministic
  execution, and resource limits.
- `runtime_safety`: scan-cycle lifecycle, scheduler/deadline/watchdog,
  panic/fault policy, stop/signal behavior, safe-state output handoff,
  retain/restart/corruption, online change/hot reload, time model,
  scan-critical allocation/resource policy, runtime/project config behavior,
  and scan-cycle/VM performance budgets.
- `protocols`: unified status/quality/confidence vocabulary, process-image I/O
  contract, `on_error`, worker/snapshot architecture, discovery truth model,
  Modbus, MQTT, ADS, OPC UA, EtherCAT, GPIO, protocol config schemas, and
  hardware-lab proof matrix.
- `control_security`: control API, ctl/JSON-line protocol, RBAC/authz roles,
  HMI write authorization, debug write/force/release lifecycle, audit/logging,
  secret handling, and default-safe permissions.
- `editor_safety`: LSP position/sync contract, diagnostics cancellation,
  rename/source-transform safety, cross-file indexing/staleness, completion,
  hover, references, call hierarchy, workspace performance budgets, and VS Code
  extension protocol boundaries.
- `plcopen_devtools`: PLCopen import/export, vendor metadata, non-ST body
  rejection, XML malformed-input behavior, vendor corpus/provenance, `trust-dev`
  test discovery, developer CLI behavior, and commit-helper scope.
- `hmi_ui`: HMI API/UI contract, widget mapping, trends, alarms, writes,
  Devices and Connections, Live Values, HMI config, webview/browser behavior,
  and accepted user journeys.
- `release`: conformance suite contract, public known-gaps contract, public
  claim registry, release evidence, version/changelog/tag/latest proof,
  platform support claims, source-build/install workflows, and tutorial/public
  workflow promises.
- `supply_chain_platform`: dependency/provenance policy, optional dependency
  behavior, license/security gates, VSIX/package artifact identity, platform
  path/socket behavior, build scripts, and packaged binary provenance.
- `verification`: TestHarness cycle/time semantics, simulation-mode semantics,
  verification metadata schemas, case-file/prover contracts, mutation/fuzz
  report contracts, and CI/report artifact semantics.

Phase 10 mutation shards remain owned by their product area while the generic
manifest/report contract belongs to `verification`. Exact invariant and test
associations establish review scope only. They do not promote an invariant,
close a specification gap, prove execution, or identify which test caught a
mutant. Coverage percentages are never release-safety proof, and a delivered
binary must be identified before a packaged-path mutation result is measured.

The bytecode/VM tags are the only tags that must be complete enough for Phase
1B. Other areas may start with open spec gaps and owners.

## Initial High-Risk Invariant Seeds

These are seed records for the future `verification/invariants/**` files. They
are not complete metadata yet.

### Runtime Safety

- [x] `RT_SAFE_PANIC_001` A scan-cycle panic cannot silently kill execution while
  surfaces report healthy.
- [x] `RT_SAFE_DEADLINE_001` Execution deadline is armed and expiry is visible.
- [x] `RT_SAFE_STOP_001` Deliberate stop applies safe outputs or reports
  not-safe.
- [x] `RT_SAFE_IO_001` Slow Modbus/MQTT device work cannot block the scan cycle.
- [x] `RT_SAFE_RETAIN_001` Retain load/save failure is visible and fail-closed.
- [x] `RT_SAFE_RESTART_001` Warm/cold/fault restart labels map to deterministic
  retain behavior.
- [x] `RT_SAFE_FORCE_001` Force/write/release lifetime is bounded and visible;
  disconnect, stop, restart, and debug-pause behavior is specified or marked
  `spec_gap`.
- [x] `RT_SAFE_NAN_001` NaN/Inf ingress from IO/comms cannot silently create
  unsafe REAL behavior; accepted behavior is specified and tested.
- [x] `RT_RELOAD_001` Online change/hot reload cannot leave live runtime state
  half-applied or inconsistent with status.

### HIR/VM Seam

- [x] `VM_SEAM_TYPE_001` Declared REAL storage cannot later execute as integer
  arithmetic because of a stale runtime value tag.
- [x] `VM_SEAM_TYPE_002` Declared-width integer widening cannot trap at the
  narrower stored tag width after assignment.
- [x] `VM_SEAM_REF_001` REF to temporary/local/return-frame data cannot persist
  beyond its valid frame.
- [x] `VM_SEAM_OWNER_001` Bytecode with ambiguous or stale instance ownership is
  rejected before execution.
- [x] `VM_SEAM_VALID_001` Bytecode validator rejects stack shape/type,
  const-use, param-direction, call-target, owner, and reference-escape
  violations.
- [x] `VM_SEAM_ENC_001` Unsupported source constructs fail to compile instead of
  lowering to NOP.

### Compiler and IEC

- [x] `IEC_PARSE_RECOVER_001` Parser recovery always consumes or advances and
  emits diagnostics for missing delimiters.
- [x] `IEC_PREC_001` Operator precedence matches IEC or documented deviation.
- [x] `IEC_STRING_001` `STRING[n]` bounds are enforced through assignment and
  call/FB parameter binding.
- [x] `IEC_SUBRANGE_001` Subrange writes are checked or explicitly documented as
  unsupported/deferred.
- [x] `IEC_TIMER_001` TP/TOF/TON timer semantics, including ET behavior and
  restart/time-base boundaries, match IEC or documented deviation.

### PLCopen and Developer Tooling

- [x] `PLCO_IMPORT_001` Unsupported executable non-ST PLCopen content rejects
  loudly while benign metadata does not skip valid ST.
- [x] `DEV_TEST_DISCOVERY_001` `trust-dev` test discovery does not silently skip
  supported mixed-case source extensions.
- [x] `DEV_COMMIT_SCOPE_001` `trust-dev` commit helpers cannot silently include
  unrelated pre-staged changes without surfacing the scope.

### Protocols

- [x] `PROTO_DISC_001` Discovery confidence vocabulary is consistent and honest:
  confirmed, likely, or port_reachable.
- [x] `PROTO_MODBUS_001` Modbus discovery does not report protocol truth from TCP
  connect alone unless labelled port_reachable.
- [x] `PROTO_MQTT_001` MQTT discovery uses CONNECT/CONNACK or is labelled
  port_reachable; discovery does not leak broker sessions.
- [x] `PROTO_ETHERCAT_001` EtherCAT unavailable hardware paths are bounded in
  memory and visible in status.
- [x] `PROTO_ADS_001` ADS status and route/browse/import surfaces report honest
  connection state.
- [x] `PROTO_OPCUA_001` OPC UA client session/subscription lifecycle is
  persistent, observable, and reconnects without false fresh data.

### Editor and UI

- [x] `EDIT_RENAME_001` Rename refuses shadow capture.
- [x] `EDIT_RENAME_002` Cross-file rename conflict checks use the merged project
  symbol table.
- [x] `EDIT_LSP_POS_001` LSP wire positions use negotiated encoding and handle
  supplementary-plane characters.
- [x] `EDIT_DIAG_CANCEL_001` Cancelled diagnostics do not publish false empty
  success.
- [x] `UI_STATUS_001` Runtime connector status vocabulary is consistent across
  HMI, Devices and Connections, hover, CLI, and reports.
- [x] `DEBUG_AUTH_001` Debug writes and forces require an authorized role; a
  default viewer/control connection cannot force outputs.
- [x] `DEBUG_PAUSE_001` Pause/breakpoint interaction with scan deadline and
  watchdog is specified and tested.

### Security, Supply Chain, and Platform

- [x] `SEC_DEP_AUDIT_001` Dependency vulnerability/license status is visible in
  release proof with owned exceptions.
- [x] `SEC_AUTHZ_001` Runtime control/API/debug authorization boundaries are
  tested for denied and allowed cases, including force/write/release.
- [x] `SEC_ARTIFACT_001` Release artifacts map to the tested commit, tag, and
  generated checksums.
- [x] `PLAT_PATH_001` Supported platform path behavior is tested or unsupported
  behavior fails with a clear diagnostic.
- [x] `PLAT_VSCODE_001` VSIX/package behavior is smoke-tested on the supported
  platform matrix or scoped honestly.

### Release and Docs

- [x] `REL_CLAIM_001` Public hardware docs distinguish loopback/mock proof from
  real device-in-loop proof.
- [x] `REL_CONF_001` Public conformance page is generated from suite result and
  known gaps.
- [x] `REL_VERSION_001` Version, changelog, VSIX version, tag, Release workflow,
  and Latest marker are aligned.
