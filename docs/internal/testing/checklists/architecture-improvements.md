# Architecture Improvements Checklist

- [ ] Explicit HMI generation correction (6 October): normal editor reads HIR declarations;
  Create PLC code uses the existing generation rules and protected regions; Create HMI
  preserves existing views. No runtime or PLC execution during authoring. Separate helpers
  own discovery, configuration, hardware mapping and source transactions. SOLID/KISS/DRY
  review and native/extension/real-browser evidence are tracked by EG-01..09; validation pending.

- [x] HMI repair pass: read-only LSP command work runs on blocking workers; normal transport and file parsing remain responsive. No PLC execution semantics change. Native command, browser, extension and diagram checks pass in hmi-repair-20261006; all five remaining capture checks now pass in hmi-final-captures-20261006 / -r2 / -r3.

- [ ] Scoped Plant HMI runtime integration (authorized 6 October): shared declaration validation,
  bounded Hmi.Display handoff, existing VM method execution, named-user replay isolation and
  existing audit. Standard ST retention replaces the rejected settings store. Final timing/vertical/browser proof remains open.
  See [contract](../../../specs/plant-hmi-runtime-integration.md); diagram source is
  `docs/diagrams/architecture/plant-hmi-runtime-integration.puml`. SOLID/KISS/DRY review and
  rendered diagram/drift checks belong to the one final batch.
  Page and popup DOM updates now share surfacePatch/ValuePaintQueue; unchanged controls retain
  identity during live updates. No second renderer or PLC execution behavior is introduced.

- [ ] HMI catalogue and generated PLC ownership (source prepared, 6 October): implement
  [CAT-01..13](hmi-builder-implementation-checklist.md) against
  [the exact component specification](../../../specs/hmi-component-catalogue.md). One project
  instance may have many views; preserve customer ST and one execution/settings/alarm owner.
  Reuse current instrument assets and internal filters; no simulator work. Builder-owned
  equipment copying reuses the project identity allocator and the page/source WorkspaceEdit;
  settings copy while I/O and relationships clear. See `docs/diagrams/architecture/plant-library-authoring.puml`.
  Bounded LSP compilation reuse is owned by hmi_compilation_cache, keyed by exact sources;
  mutable access discards the entry. No runtime/compiler implementation is changed.
  Diagram and implementation evidence remain pending under the final validation rule.

- [ ] Plant Library replacement (source prepared, 6 October 2026): implement
  [PL-01..28](trust-plant-library-checklist.md) against
  [the target specification](../../../specs/trust-plant-library.md). Reuse existing
  runtime/alarm/PID/history owners, replace the old Equipment library, add validated
  settings and source-independent normal PLC I/O. Leave simulator code, configuration
  and control examples untouched; no simulation-specific library API. SOLID/KISS/DRY and PlantUML
  implementation evidence remains open pending the one final batch; no release is authorized.

- [ ] Agent HMI authoring (specification 43): shared native-tool/MCP adapters, document snapshots,
  persistent drafts, semantic edits, rendering and editor transactions are implemented in the
  feature checkout with focused red/green proof. Full review/recovery/client acceptance remains
  open. Post-commit cancellation retains the applied receipt; isolated project symbol scopes
  prevent preview registry leakage.
  Keep native schema/binding ownership and shared drawing; see HA-03..09 and
  `docs/diagrams/architecture/hmi-agent-authoring.puml`. Proposal review now uses a read-only
  virtual document in the existing builder, shared theme tokens and renderer-produced images;
  Apply/Discard return to the same revision/transaction owner with a fresh request context.
  The normal editor's P&ID sync, deployment and Live handlers are never instantiated for it.
  Layout constraints now have a separate geometry owner; pinned collision checks reuse the
  builder's rotated bounding-box helper. Versioned composition definitions/references reuse
  normal page objects and shared drawing. Image loading reuses the native HMI bundle reader
  and editor snapshots, with review hashes; no new runtime API or alternate rendering engine.

- [x] Alarm equipment navigation (4 October 2026): `equipmentNavigation` reuses the existing
  read-only page API and instance bindings; alarm actions keep acknowledgement separate from
  navigation, and one display owner handles the temporary focus. Native rendered interaction
  tests red then green. Runtime-display diagram also reconciled with the existing `HmiExchange`
  and program-driven `Hmi.Display` service boundary, replacing obsolete HmiRead command arrows.
  SOLID/KISS/DRY: no new binding registry, runtime endpoint, polling loop or write authority.
  Diagram rendered and drift checked on the builder; 29 focused display checks and real-runtime
  dark/light navigation journeys pass. Evidence: `2026-10-04-hmi-equipment-navigation/`.

- [x] HMI My Symbols visual editor (4 October 2026): a separate host and webview own
  symbol editing; runtime `my_symbols` owns parsing, SVG cleaning and canonical writing;
  the preview reuses `customSymbol` and `applyRules`, and shell styles reuse truST tokens.
  Native format and rendered interaction red/green recorded in HB-5-02. Diagram rendered and
  drift checked; real-extension captures self-reviewed in three themes with native rule-edit
  and Undo proof (`2026-10-04-hmi-symbol-editor`). Independent final UX acceptance remains
  open. SOLID/KISS/DRY: no second parser, renderer,
  theme or persistence owner; edits use the existing VS Code text document.

- [x] Control-course comparison presentation repair (15 September): reuse one presentation-only `modelStructureEvidence` table owner in existing and three-page recorded-fit views; host fit, immutable recordings and structure authorization remain unchanged. Rendered expected red to15 focused green checks; real native choice/export/reopen and29 individual image reviews retained in `control-course-repair-20260915`. SOLID/KISS/DRY slice satisfied; overall architecture/release gates remain open.

- [x] Shared linear chart tick precision now considers the visible value span around the operating point. Rendered expected red (3 distinct labels for8 ticks) reaches25 chart checks green; native heater baseline images show distinct ticks. SOLID/KISS/DRY: existing formatter reused, no raw-sample changes or new chart owner. Evidence: `control-course-repair-20260915/lesson20-relay-final`.

- [ ] Control Studio acquisition/recording display repairs (15 September): reuse the
  existing monitor stream, bound experiment owner, navigation storage and result
  exporter. Live/recorded projections share one binding catalogue; reopening and
  export preserve read-only evidence without replaying runtime actions. SOLID/KISS/DRY:
  no additional acquisition service, persistence store or parameter transaction.
  Seventy focused real-extension checks pass; native Lesson10/14 and imported
  Lesson15 evidence is retained in `control-course-repair-20260915`. Full screenshot
  review and final broad gates remain; diagrams have been rendered and drift checked.

- [x] Test presentation retains ownership of reviewed/running/last Test status;
  generic model and automatic-tuning results cannot certify or overwrite Test.
  SOLID/KISS/DRY: existing experiment renderer, stage projector, recording and
  runtime owners remain; no parallel recorder or controller. Expected focused
  and shipped-bundle reds reach green, the actual Control.PID recovery journey
  passes in dark/light/high contrast, and all24 Test originals were personally
  and independently reviewed. See release `prior-test-*` and
  `ui-clarity-matrix/` evidence. Diagrams rendered and drift checked. This is
  bounded Test-status acceptance, not full recovery/platform qualification.

- [x] Lesson detail keeps the existing gallery, metadata and Create/Open owners.
  One lesson heading and early project action lead to native expandable details;
  exact duplicate prose is omitted without deleting teaching content. Native
  Enter/Space/pointer interactions pass and the title/redundancy assertions have
  expected reds and greens. SOLID/KISS/DRY: no new lesson catalog, project copy
  workflow or custom keyboard controller. Authored Markdown/MathML uses a shared
  formatter; one scoped immutable content provider owns read-only bundled source
  previews. Native tests prove all three authored file types and return to the
  retained lesson tab. Paragraph/capture behavior reds reach green; all 55 current
  screenshots have personal and independent verdicts, including actual narrow
  and zoomed windows. See release `gallery-formatting-acceptance.md`. This closes
  bounded gallery presentation/ownership, not whole-course or release acceptance.

- [x] Manual PI tuning reuses the existing Control.PID complete record, native
  heater, runtime parameter transaction and recording owners. Numeric fields
  project the same JSON values, with separate review/edit views and confirmed
  rollback; measured Tune uses the shared chart contract. Native13-case group,
  target/load UI76.318s/76.497s, focused623 Studio cases, current parameter UI and
  diagrams pass. SOLID/KISS/DRY: one state/transaction owner, no copied controller
  equations or parallel plant model. See release `lesson11-*` and `parameter-*`
  evidence. Whole-lesson application qualification remains open.


- [x] Lesson10 reuses the actual PID and one native heater state with two sensor
  outputs. Native channel0 bias preserves the reference; preceding request is
  paired with current applied feedback. Three expected owner/input/bias failures
  reach green, all11 native intro-engineering cases pass, and the real biased
  Test/save/open journey passes74.462s. Constant-response axis roundoff was fixed
  in the shared chart owner with exact raw data retained. See `lesson10-*` and
  `constant-response-*` release evidence; whole-lesson qualification stays open.

- [x] Recording-only Validate reuses workflow action availability, shared stage
  status vocabulary and the existing chart workspace. Saved revision restore
  replaces displayed evidence before restoring that revision's chart session;
  stored original pins/results remain preserved. Expected rendered failures,
  actual native lesson9 runs, 618 focused Studio greens and diagram drift are
  recorded in the release artifact `recording-validation-*` evidence. Full
  candidate/axis clarity and whole-platform release gates remain open.

- [x] Workflow 1 execution/UX follow-through: direct Main model import, current-loop
  session ownership, requirement-based recorded-channel roles, completed receipt
  identity matching, exact scalar sampled-model analysis and complete relay-cycle
  qualification. See [the execution and usability record](control-studio-workflow1-acceptance-20260910.md).

- [x] Connected Main workflow source ownership: controller adapters compose the
  existing numerical, native search, transfer/parameter and recording owners;
  initialization formulas, scenario policies and automatic draft storage are
  separate modules. Source completion is tracked in
  [the connected workflow checklist](control-studio-connected-workflow-implementation.md).
  No build, test, rendered or architecture-gate acceptance was performed.
- [x] Connected-workflow source integration: project-controller tuning reuses the
  isolated verification client; controller initialization shares one coordinate
  mapper; live switching assessment reuses the observed-requirement evaluator;
  existing workspace persistence owns serialized isolated-operation journals.
  Source changes only under the user's no-tests instruction. Generated diagrams
  remain unchanged and no execution or rendered acceptance is claimed.
- [ ] `ARCH-CONTROL-TRANSFER-01` Keep the guarded mode/reference/state transaction
  in `host/debug/control/transfer.rs`, transport in `control/transfer_handlers.rs`,
  and pre-task/post-ST/output-commit hooks in `runtime/cycle.rs`. Reuse compiled
  ownership, instance resolution, parameter-session identity and recording.
  Native transfer16tests pass, including replay, JSON numeric semantics and
  task-boundary tracking from fresh PV and committed output. Local P Auto entry,
  native response plotting and Manual rollback pass rendered journeys; saved
  commissioning remains historical. Remaining recovery cases and final gates
  remain incomplete. Native vertical28passes; lost-gain ACK recovery, bounded
  recording inspection retry and manual transfer tolerance reuse the existing
  native contracts. Reopened recovery uses fresh clients; historical results
  never recreate Apply authority. No physical readback claim from driver ACK.
- [ ] `ARCH-CONTROL-WORKFLOW-01` Share selected supplied/identified models through small model/session adapters; retain the actual controller and reject incompatible conversions. Connect isolated actual-ST Verify through typed inline simulation override. The selected model now carries validated absolute `operatingPoint` coordinates into model-deviation simulation through physical input/output offsets; supplied nonzero rendered actual-ST proof passed in `/tmp/control-operating-point-rendered.log`, and the independently fitted nonzero Find -> Tune -> Verify journey passed in `/tmp/control-find-point-rendered-green3.log` with rendered evidence inspected in `/tmp/control-find-operating-point.png`; the fitted path also proves wrong supplied coefficients are ignored. Scoped first-order P integration, manual P pin/history, shared recorded-response chart controls, terminal and cancellation-unconfirmed outcome retention, native bounded worker prefix retention on cancellation, wall-budget expiry, abnormal child exit and byte/output-budget faults, and historical engineer study Save/reopen through the existing workspace-state boundary are proven. Native checkpoint/API/runtime evidence is retained in `control-checkpoint-header-green.log`, `isolated-final.log`, `wall-budget.log`, `byte-budget.log`, `search-suite.log`, and the five runtime vertical logs; rendered cancellation Save/reopen passed in `/tmp/control-cancel-prefix-rendered-green2.log` (1 passing, 9s) with controller updates retained. The received-frame count reports complete rows transferred by the supervisor; external OS-kill rendered proof remains unproven. Pin content identity invalidation moved from the expected red in `/tmp/control-pin-content-red.log` to green in `/tmp/control-pin-content-green.log` (1 passing). The normal-window guided flow exercised the compiled law through Save -> Find -> Tune -> actual-ST Verify in `/tmp/control-guided-external-current-dark.log` and `/tmp/control-guided-external-current-light.log`; active Find/Verify study restore now rejects before model/history mutation (`/tmp/control-study-active-open-green.log`), with rendered rejection green in `/tmp/control-study-open-rendered-green2.log`. One symbolic `control.signals.inspect` snapshot now supplies SP/PV/MV and mode through the shared monitor (`/tmp/control-symbolic-mode-rendered-red.log`, `/tmp/control-symbolic-mode-rendered-green.log`, `/tmp/control-symbolic-current-green.log`); resumed current-scan and predicted-versus-verified-unapplied headers are covered by `/tmp/control-study-current-state-{red,green}.log` and `/tmp/control-engineer-candidate-header-{red,green}.log`. The identification session injects coherent MV/PV/Manual baseline acquisition into the campaign, with deterministic qualification owned by `identificationBaseline`. External Supply/import is guarded by `GeneralModelSession` through `EngineerWorkflowSession.assertIdleForReplacement` before mutation; accepted internal Find handoff remains allowed. Manual gain pin compatibility compares the full selected model content plus revision, loop, parameter symbol/unit, bounds and REAL precision, so reused revision strings cannot preserve a stale pin. `RuntimeRecordingSession` first pins a coherent symbolic `control.signals.inspect` identity, then owns the typed start/stop/release handoff; native `control.recording` owns bounded scan capture, program identity and per-binding quality, and the session projects validated acquisition evidence into the saved result. Local P Auto commissioning and guided setup have bounded rendered proof; broader Auto recovery, other workflow persistence, full connected workflow and final architecture acceptance remain incomplete. PlantUML source and targeted immutable-image remote SVG are updated; local drift check passes. Host-owned current Engineer evidence now reuses the shared stage vocabulary through a small webview projector; the compatible P header has one Tune action. Actual Find and supplied manual/failure/reopen journeys pass; broader workflow acceptance remains open.
- [x] `ARCH-CONTROL-SATURATION-01` Retired 8 October 2026: the saturation-and-windup assessment had no reader and is removed with `saturationWindup.ts`; `testTerminalEvidence.ts` keeps the terminal Test record only (specification 35 no longer describes the assessment).
- [x] `ARCH-CONTROL-ANTIWINDUP-01` Retired 8 October 2026: the automatic matched anti-windup comparison was unwired and unused and is removed (the `antiWindup*.ts` modules); back-calculation remains a declared PID law field tuned with the gains (specification 35 Part C).
- [x] `ARCH-CONTROL-RELAY-01` Retired 8 October 2026: `RelayTuningSession` and its unread terminal payload are removed; cycle qualification and the Relay rule stay in `relayTuning.ts` and `relayCycleEvidence.ts`, reached through Tune's Relay rule (numerical annex N5.R).
- [ ] `ARCH-CONTROL-GENERAL-01` Guided setup owns explicit binding selection and exclusive config persistence in configureLoop.ts; reuse the LSP compiled catalog and existing opening command. Multi-loop discovery remains in projectLoop.ts; stage evidence is independent of excitation ownership. Focused red/green passes; full panel decomposition and final diagram gates remain open.
- [ ] `ARCH-CONTROL-RECORDING-ACK-01` Keep native recording-ID ownership in
  `RuntimeRecordingSession` and manual pending Start/Stop ownership in
  `RecordingCoordinator`; the webview only projects `Starting`, acknowledged,
  unconfirmed and retained-evidence states. Focused expected red and 5-case green
  are retained in `control-manual-recording-{red,green}.log`. Actual-ST manual
  lost-ACK/Stop-retry passes in dark, light and high-contrast journeys; retained
  samples remain unqualified after an unconfirmed Start. Program-boundary and
  mismatched-owner recovery are still open. Extension-host recovery now persists
  a credential-free exact owner before Start, restores it as unconfirmed after a
  normal-window reload, and reconstructs only its retained Stop after the same
  runtime reconnects; its matching-runtime red/green and rendered proof are in
  `control-recording-retry-{unavailable,restore}-{red,green}.log` and
  `control-managed-recording-normal-{current.log,recovery.png}`. A normal-window
  runtime restart visibly retains an unconfirmed missing-buffer owner with Start
  disabled (`control-managed-recording-normal-restart.{log,png}`); mismatched
  identity/package acceptance remain open.
- [x] `ARCH-CONTROL-REMEDIATION-03` Resolve omitted plant starting state using existing model descriptors and custom A dimensions; carry omission provenance through one canonical draft to the existing serializer. Field metadata owns scalar optional/default behavior, avoiding a second validator exception list. Native and rendered red-green pass in `cpfix006-omitted-starting-state.md`; diagrams regenerated on the builder with zero drift (`cpfix003-diagram-render-drift-green.log`). This closes the bounded ownership change, not full cross-model UX acceptance or final integrated gates.
- [ ] `ARCH-CONTROL-REMEDIATION-02` Keep optional physical temperature mapping and indexed port bindings in small pure helpers editing the existing canonical draft; reuse SignalSelect and runtime persistence. Shared project landing constants/helper have one owner without introducing a circular dependency. SOLID/KISS/DRY checks: no browser physics, second model catalog, palette, or persistence; port edits preserve neighboring indices. Focused native/rendered tests pass; diagram source updated, final regeneration/drift remains open.
- [ ] `ARCH-CONTROL-REMEDIATION-01` Keep no-argument Control Studio command discovery in `openCommand.ts`, separate from the existing bound-panel lifecycle. Reuse the project loop resolver and selected-runtime store; preserve explicit handoffs. Diagram source updated; focused Extension Host proof and final diagram regeneration/drift remain required.
- [ ] `ARCH-H-CALLSTACK` Keep admitted stack/register/tier-1 user calls in one shared dispatch continuation loop, preserving distinct eligible register calls and occupied-POU stack fallback. H's depth-33 regression exposed synchronous register recursion and 50,584-byte native interpreter frames. Close with nested-depth, work parity and failure-unwind tests plus the execution-flow diagram; ownership remains in the shared VM. Execution/evidence authority: `runtime-portability-implementation-checklist.md`, H-CURRENT-STACK.
- [x] `ARCH-OPENOT-DB-01` Add product-owned durable database persistence after OpenOT document resolution, keeping shared-memory carriage, loss accounting, definition resolution, durable checkpointing, database writes, and status projection in separate owners and entirely outside the PLC scan path. Select the supported database through `[runtime.openot.persistence] backend = "..."` and dispatch through backend-owned adapters without fallback. Execute through `openot-database-persistence-implementation-checklist.md`; close only after specification-first native red-green tests, per-backend outage/recovery proof, refreshed diagrams, remote full gates, and release evidence pass. Evidence includes the split `openot_persistence::{source,consumer,worker,projection,factory,service}` owners and six backend adapters, exact seven-product canonical retrieval, real outage/restart and scoped teardown, retained checksummed artifacts, generated `openot-database-persistence` and system diagrams, green diagram drift/all-scope architecture doctor, and the frozen exact-candidate full/release gate contract.
- [x] `ARCH-OPENOT-DB-02` Replace canonical-JSON-as-query-model with one backend-neutral `LoggingProjector` and a stable descriptive typed database read model. Keep canonical OpenOT documents in internal authority tables; atomically write relational domain projections and checkpoints; use explicit SQLite public views, typed TimescaleDB hypertables, and homogeneous InfluxDB measurements with per-part spool reconciliation. The initial release uses one schema generation, 1, across all adapters; development-only migration/rebuild paths are superseded by fail-closed compatibility checks. Evidence: non-JSON typed queries pass across the complete all-domain workload; exact full-range values and canonical recovery bytes match in all seven real products; the Influx selected-sink contract reports all reconciled parts; the refreshed ownership/data-flow diagram passes drift validation.
- [x] `ARCH-OPENOT-DB-04` Remove unreleased v1/v2/v3/v4 schema histories. Initialize the final generation-1 shape directly on every backend, reject markerless/differently marked/incomplete pre-release state without mutation, and prove the same contract with real products and the real PLC workload. Execute through `openot-database-initial-schema-corrective-checklist.md`.
- [ ] `ARCH-OPENOT-DB-03` Make persistence-open failure ownership explicit: synchronous startup validation owns selected adapter/environment/local artifacts; only typed remote-reachability failures consume the reconnect budget; deterministic schema, migration, capacity, identity, projection, and storage failures fault without repeated open attempts. Close after focused red-green lifecycle tests, real PostgreSQL outage/reconnect proof, all-feature host and Windows warning-deny checks, refreshed OpenOT diagrams, and final remote gates pass.
- [x] `ARCH-VERIF-SCOPE-01` Retire the post-closure per-function code/spec and test-routing control planes. Preserve every real product specification, native behavior test, required fixture, and product fix. Current product work follows the direct written-specification -> native-executable-test contract; historical metadata cannot create requirements.
- [ ] `ARCH-RELEASE-CLEAN-01` Keep post-merge cleanup in a dedicated release-guard module that audits exact candidate ancestry, refs, and worktree dirtiness; reports explicit targets without deleting automatically; is registered in the Supply Chain CI job; and is reflected in the release-evidence diagram. Close after focused tests, diagram regeneration/drift proof, and final remote gates pass.
- [ ] `ARCH-CAPTURE-LIFECYCLE-01` Keep docs-capture process ownership in a dedicated session wrapper, reap its complete owned process session and named code-server container on success, failure, and interruption, preserve the child status, and leave reused external servers untouched. Close after focused lifecycle tests, a real capture run, diagram regeneration/drift proof, and final remote gates pass.
- [ ] `ARCH-WEBIDE-HW-01` Coalesce repeated Hardware canvas relayout requests through one owned animation frame, cancel that frame and its delayed timer when superseded or deactivated, and retain only the latest post-hydration resize/fit pass. Close after the existing rendered deep-link regressions pass, a full browser capture passes, diagram regeneration/drift proof passes, and final remote gates pass.
- [x] `ARCH-VSCODE-FLEET-01` Give Devices & Connections one pure fleet-topology identity boundary: `networkCanvas/fleetTopologyIdentity.ts` clones each schema-v2/v3/v4 response and losslessly scopes container, runtime, endpoint, configured-mesh-external, link, and shared `used_by` references by owner tuple; ambiguous references fail closed without hiding unrelated topology. Keep fetch/merge orchestration in `fleetTopology.ts`, and route singleton live, offline/live overlay, and peer display paths through `mergeFleetTopologies` so normalization is idempotent and independent of response count. SOLID/KISS/DRY acceptance: the identity module has no VS Code/runtime transport or rendering imports, the panel contains no identity-rewrite logic, and all response counts use the one merge ingress. Evidence: the registered remote `network-canvas-fleet-identity.test.ts` suite moved from 1 passing/5 expected behavior failures to 6/6 green; real Extension Dev Host/CDP proof `DC-fleet-identity-scoped.png` and `fleet-identity-scoped-proof.json` visibly preserve both same-named runtimes, peers, and health-bearing links; `system-architecture.puml` and its generated SVG/manifest were refreshed and the remote diagram drift check passed.

Status: In progress - zero-silent-error follow-ups are open

- [x] `ARCH-VERIF-MUT-01` Keep focused source mutation execution in small verification-owned modules: `mutation_execution.py` owns side-effect-contained process and selector primitives, `focused_mutation_runner.py` owns the source-only orchestration, and `focused_mutation_artifact.py` owns the closed at-rest contract. The generic Phase 10 report consumes canonical artifacts without changing runtime ownership or treating mutation associations as proof. Evidence: `docs/diagrams/architecture/verification-mutation-execution.puml`.
- [x] `ARCH-RUNTIMEBOUND-01` Add the runtime boundary fail-closed contract and gate after issue #77 exposed harness watch/input/direct-bind surfaces that could return `ok=true` for unresolved names. Evidence: `docs/internal/architecture/runtime-boundary-fail-closed-contract.md`, `scripts/runtime_boundary_fail_closed_ast_grep_gate.sh`, and `FULLMAP-RUNTIMEBOUND`.
- [x] `ARCH-RUNTIMEBOUND-02` Replace harness boundary missing-name fallbacks with `BoundaryError` / `BoundaryEntry` so typoed inputs, direct bindings, watch paths, ambiguous names, and unbound `CONFIGURATION` programs fail closed. Evidence: `cargo test -p trust-runtime --test boundary_resolver --test harness_fail_closed --test build_unbound_program --test protocol_envelope` passed.
- [x] `ARCH-RUNTIMEBOUND-03` Finish debug/control and web/HMI boundary cleanup from `runtime-boundary-fail-closed-safety-checklist.md` Phase 5/6 before claiming all external observation/control surfaces are covered. Evidence: debug/control fail-closed tests pass, HMI WebSocket control failures now emit structured error frames, runtime-cloud/HMI export serialization fallbacks return explicit errors, and `FULLMAP-RUNTIMEBOUND` scans the expanded surface set.
- [x] `ARCH-HMI-PERSIST-01` Keep operator-parity HMI persistence runtime-owned instead of webview-owned: `HmiPersistenceService` lives under `trust-runtime::hmi`, control handlers persist via the existing HMI endpoints, restart reload is config-gated by `runtime.hmi_persistence`, and diagrams show the JSONL append/load-window data flow.
- [x] `ARCH-RUNTIMESAFE-01` Add the runtime-internal safety fail-closed board and warn-only doctor inventory for I/O driver policy handling, retain durability, watchdog/output ordering, safe-state failure, evaluator writes, init defaulting, audit/event durability, persisted runtime-cloud state, mesh snapshots, HMI robustness, feature-disabled debug surfaces, and coercion proof. Evidence: `docs/internal/testing/checklists/runtime-safety-fail-closed-checklist.md`, `docs/internal/architecture/runtime-safety-fail-closed-contract.md`, `scripts/runtime_safety_fail_closed_ast_grep_gate.sh`, and `FULLMAP-RUNTIMESAFE` reporting 82 warn-only findings with zero allowlist entries.
- [x] `ARCH-RUNTIMESAFE-02` Close the runtime-internal fail-closed board and flip `FULLMAP-RUNTIMESAFE` to blocking after fixing the inventory. Evidence: focused red-to-green runtime tests cover I/O, retain, cycle ordering, init/evaluator/debug writes, coercion, audit/event drops, runtime-cloud corrupt state, mesh timeout, and feature-disabled debug responses; `scripts/runtime_safety_fail_closed_ast_grep_gate.sh` reports `phase=fail_class`, `finding_count=0`, and `allowlisted_count=0`; `cargo run -p xtask -- architecture-doctor --full-map` reports `PASS: FULLMAP-RUNTIMESAFE`; debug/runtime-cloud diagrams were refreshed and `python scripts/check_diagram_drift.py` passes.
- [x] `ARCH-RUNTIMESAFE-03` Keep runtime safety hardening launcher-owned where appropriate: panic/deadline guards stay in scheduler code, SIGINT/SIGTERM handling lives under `run/runtime/signals.rs` and enters through `ResourceControl::stop()`, and the runtime execution diagram shows signal shutdown using the same safe-output/retain-save stop path.
- [x] `ARCH-RUNTIMESAFE-04` Keep later runtime safety fixes inside their owning runtime boundaries: ADS symbol-version refresh releases notifications through the ADS worker/transport, OPC UA rejected point writes stay in the client worker as validation failures instead of reconnect policy, bytecode metadata validation happens before process-image mutation, and Modbus/MQTT scan-thread isolation is explicitly deferred to a worker/snapshot architecture slice rather than hidden inside the I/O fault-policy patch. Evidence: `docs/internal/testing/evidence/runtime-safety-hardening/2026-07-05/phase-9-debug-spikes-disposition.md` and `phase-3b-scan-thread-isolation-disposition.md`.
- [x] `ARCH-RUNTIMESAFE-05` Close the Phase 3B Modbus/MQTT scan-thread isolation ownership change with protocol-owned worker modules instead of an `IoDriver` redesign: `io/modbus/worker.rs` and `io/mqtt/worker.rs` own blocking wire/session work, reconnect/backoff, latest input snapshots, and level output handoff while scan-cycle `IoDriver` methods copy bounded in-memory state and project health through `IoDriverHealth`. Evidence: `runtime-safety-hardening-2026-07-05.md` Phase 3B rows, `phase-3b-worker-implementation-summary.md`, and refreshed `runtime-execution.puml`.
- [x] `ARCH-RUNTIMESAFE-06` Close the EtherCAT `PduStorage` follow-up as an EtherCAT-driver-owned lifecycle policy instead of a shared `IoDriver` redesign: missing/unavailable hardware after the ethercrab storage split boundary becomes terminal for the current driver instance and requires runtime driver rebuild, while lab hardware stress remains an ignored device-in-loop artifact path. Evidence: `runtime-ethercat-pdustorage-follow-up-2026-07-05.md`, `ethercat-pdustorage-implementation-summary.md`, and refreshed `runtime-execution.puml`.
- [x] `ARCH-RUNTIMESAFE-07` Close the Phase 10 behavior-preserving runtime ownership splits without changing runtime semantics: OpenOT authoring is split into validation/model/instrumentation/definition owners, fleet topology handling is split by runtime/topology/I/O/protocol/host/DTO owners, ADS diagnostics is split into report/evidence/failure owners, restart remains unsplit because it is below the guardrail, and the world arm domain is split into bridge/types/model/physics/P3 URDF/P4 multi-URDF owners. Evidence: `runtime-phase10-performance-refactor-board-2026-07-05.md`, `runtime-world-arm-domain-refactor-board-2026-07-06.md`, and `p10-refactor-behavior-lock-after-output.txt`.
- [x] `ARCH-RUNTIMESAFE-08` Keep MQTT tag resolution outside the protocol worker: bundle startup applies bytecode and process-image metadata first, `io/mqtt_tag_mapping.rs` atomically resolves configured program-instance scalar tags through the shared runtime boundary and lowers them to typed MQTT points, and only then is the existing scan-isolated MQTT worker constructed. Output-only mappings create no default raw-input subscription. Evidence: issue #111 focused red/green mapping and worker tests, the live remote broker publication trace, and the refreshed `runtime-execution.puml`.
- [x] `ARCH-RUNTIMESAFE-09` Keep MQTT review follow-ups in their existing owners: the driver configuration owns symmetric raw input/output enablement, tag lowering captures canonical enum metadata, and the shared process-image binding boundary performs enum conversion atomically for both read and write paths. The MQTT worker and scheduler remain transport- and scan-focused. Evidence: `mqtt-tag-mapping-review-followups.md`, focused direction/enum red-green tests, and the refreshed `runtime-execution.puml`.
- [x] `ARCH-RTCONN-01` Complete the runtime connector status contract board so process-image drivers and supervisory connectors project into one additive status/confidence/point-quality model without changing `IoDriver` or merging transport loops. Evidence: `docs/internal/testing/checklists/runtime-connectivity-conformance-remediation-checklist.md` has no unchecked rows; `crates/trust-runtime/src/connectors/` owns the additive status contract; `docs/internal/testing/evidence/runtime-connectivity-conformance/state-mapping-review.md` records the mapping review; final closeout records accepted-deferred boundaries for unavailable real CODESYS/TwinCAT export fixtures and release-tag proof from `main`.

- [x] `ARCH-INIT-01` Centralize initializer syntax classification so aggregate initializer nodes are recognized only in initializer-aware paths, with trivia/pragma excluded from expression and statement classifiers.
- [x] `ARCH-INIT-02` Preserve struct/union member initializer handles through HIR/runtime type metadata and route runtime default materialization through a dedicated initializer service instead of ad hoc zero defaults.
- [x] `ARCH-INIT-03` Route aggregate VAR, TYPE-level, field-default, VAR_CONFIG, and function-block member override initialization through the runtime initializer catalog/service, with diagnostics for unknown/duplicate fields and illegal FB targets.
- [x] `ARCH-INIT-04` Add `trust-runtime bench init` so startup/first-cycle initialization cost has a reproducible CLI benchmark surface with JSON/table reports.
- [x] `ARCH-INIT-05` Refresh syntax, HIR, system, runtime execution, and VM helper-flow diagrams after adding aggregate initializer flow, exact TYPE/VAR initializer parser gates, HIR `SymbolTable` source catalog ownership, runtime lowered catalog ownership, and runtime/VM materialization flow.

Issue #51 init benchmark baseline evidence (2026-04-28):

- Command: `cargo run -p trust-runtime --bin trust-runtime -- bench init --project crates/trust-runtime/tests/fixtures/init_bench --samples 1000 --output json`
- Machine: `raspberrypi`, `Linux 6.12.62+rpt-rpi-2712 aarch64`, best-effort local run, not CPU-pinned.
- Fixture: `crates/trust-runtime/tests/fixtures/init_bench`, resource `InitBench`, execution backend `vm`, warmup cycles `0`.
- Current supported-feature baseline medians: init-only `19974.007 us`, init-plus-first-cycle `19995.964 us`, first-cycle/first-mutation `13.889 us`, retain restart `375.094 us`, steady cycle `4.630 us`.
- Current supported-feature p95/p99: init-only `23108.791 us` / `25647.193 us`, init-plus-first-cycle `23142.997 us` / `25681.064 us`, first-cycle `45.352 us` / `59.185 us`, retain restart `661.410 us` / `846.651 us`, steady cycle `18.556 us` / `33.482 us`.
- Constructor microbench medians, 1000 constructions per sample: `StructValue::new` `313689.932 us`, untyped constructor proxy `68846.835 us`.
- Report artifact: `docs/internal/testing/evidence/issue-51-init-benchmark-2026-04-28.md`; the generated JSON from the run is local under `docs/internal/architecture/generated/reports/issue51-init-bench-current.json`.
- Pre-implementation same-fixture baseline status: unsupported/no measurement. `origin/main` has no `BenchAction::Init` / `bench init` CLI and does not contain `crates/trust-runtime/tests/fixtures/init_bench`; its declaration parser still sends `:=` initializers through `parse_expression()`, so the representative aggregate initializer fixture is not a valid pre-change workload. This makes a same-fixture 10% regression comparison non-applicable for Issue #51 itself. The numbers above are the first enforceable supported-feature baseline for future regressions on this fixture.
- Diagram evidence: Issue #51 PlantUML now distinguishes the HIR source/default `InitializerCatalog` owned by `SymbolTable` from the runtime `program_model::InitializerCatalog` of lowered `Expr` records, and the syntax diagram locks aggregate initializer parsing to the six code-derived initializer-aware call sites rather than ordinary expression parsing.

- [x] `ARCH-PLCOPEN-01` Add a dedicated PLCopen motion architecture diagram describing the shipped ST public profiles, shared kernels, internal carriers, and deferred-feature guard coverage.
- [x] `ARCH-PLCOPEN-02` Regenerate the PlantUML outputs and refresh `docs/diagrams/manifest.json` after adding the PLCopen motion architecture diagram.
- [x] `ARCH-PLCOPEN-03` Extend the PLCopen motion architecture diagram for the shipped OOP facade package and document that it delegates to the classic motion kernels instead of owning duplicate motion state.
- [x] `ARCH-CONTROL-PID-01` Keep the Control library behind two cohesive state owners: `Control.PID` owns the combined positional/incremental controller history and `Control.LowPass` owns independent filter history. SOLID/KISS/DRY acceptance: no duplicate P/PI/PD state machines, no hidden filter inside the controller, complete record-based configuration, and native tests use only the public surface. Evidence: `docs/diagrams/architecture/control-pid-library.puml`, `docs/specs/45-control-pid.md`, and the Section 7 native conformance fixture.
- [x] `ARCH-CONTROL-PID-02` Route typed enum literal type names through the existing scope- and `USING`-aware HIR resolver instead of introducing a Control-specific lookup or a second type-resolution path, while preserving registered type identity when a value has the same name. Evidence: focused cross-file import and same-named-value enum regressions plus refreshed `docs/diagrams/hir/hir-semantics.puml`.
- [x] `ARCH-CONTROL-PID-03` Regenerate PlantUML outputs, refresh `docs/diagrams/manifest.json`, and pass the remote diagram drift check for the Control PID and enum-resolution architecture updates. Evidence: remote `scripts/render_diagrams.sh` and `python scripts/check_diagram_drift.py` passed in the isolated validation checkout.
- [x] `ARCH-VM-01` Add a dedicated bytecode VM execution architecture diagram that separates the production VM path from residual `EvalContext` / legacy-interpreter-only flows.
- [x] `ARCH-VM-02` Regenerate the PlantUML outputs and refresh `docs/diagrams/manifest.json` after adding the detailed bytecode VM execution diagram.
- [x] `ARCH-VM-03` Add a focused interpreter-removal map diagram that classifies VM runtime pieces as keep, extract, rewrite, or delete.
- [x] `ARCH-VM-04` Regenerate the PlantUML outputs and refresh `docs/diagrams/manifest.json` after adding the interpreter-removal map diagram.
- [x] `ARCH-VM-05` Refresh the interpreter-removal map after landing `program_model` / `helper_eval` extraction so the diagram matches the current ownership split.
- [x] `ARCH-VM-06` Refresh the runtime execution/debug/system diagrams after removing the production interpreter backend so they show direct VM dispatch plus `helper_eval` helper flows.
- [x] `ARCH-HIR-01` Refresh the HIR semantics architecture diagram so the Salsa query flow shows `file_type_prelude_query`, `project_type_catalog_query`, and project-aware `file_symbols_query` as the source of cross-file type visibility.
- [x] `ARCH-HIR-02` Regenerate the PlantUML outputs and refresh `docs/diagrams/manifest.json` after the project type catalog refactor.
- [x] `ARCH-VM-07` Refresh the runtime execution diagrams so the shipped `SIZEOF` contract shows compile-time/static-type lowering and no longer presents `SIZEOF_VALUE` as part of normal ST codegen.
- [x] `ARCH-VM-08` Regenerate the PlantUML outputs and refresh `docs/diagrams/manifest.json` after the `SIZEOF` contract update.
- [x] `ARCH-VM-09` Split the `runtime/vm/register_ir` implementation into focused lowering/profile/tier1/test modules and consolidate composite ref-path walking into `value/reference.rs`; audit the existing VM diagrams and confirm no PlantUML refresh is required because execution/data flow and subsystem ownership stay unchanged.
- [x] `ARCH-VM-10` Harden MP-060 CP65 benchmark evidence generation: `runtime_vm_bench_gate.sh` now supports low-noise repeated corpus runs and optional baseline compare input, and `runtime_vm_bench_summary.py --self-test` verifies per-fixture metrics, confidence spread fields, repeated-run medians, and compare deltas without running the expensive benchmark corpus.
- [x] `ARCH-VM-11` Harden MP-060 CP63 bytecode validator invariants: `POU_INDEX` entries must now have unique POU ids before `VmModule` construction, unsupported runtime-only opcodes fail module validation before dispatch, and focused red-first `bytecode_vm_core` / `bytecode_validation` regressions prove duplicate ids and legacy `CALL_METHOD` bytecode fail at the validator boundary.
- [x] `ARCH-VM-12` Harden MP-060 CP63 shared opcode decode invariants: the canonical VM opcode-length helper now includes partial-access opcodes `0x62`/`0x63`, register lowering no longer carries a duplicate length exception for them, and a red-first owner-inference test proves partial-access bytecode no longer silently drops instance-owner context.
- [x] `ARCH-VM-13` Harden MP-060 CP63 POU-local reference invariants: validator now checks each POU local-ref range against `REF_TABLE`, rejects non-local entries inside declared local ranges, and rejects local-reference operands outside the executing POU's range before VM frame execution.
- [x] `ARCH-VM-14` Add MP-060 CP64 malformed-bytecode fuzz-smoke evidence: `vm_malformed_bytecode_fuzz_smoke_budget` applies deterministic duplicate-id, unsupported-opcode, bad const/ref/jump/local-ref, malformed operand, and invalid string-payload mutations, and `runtime_vm_malformed_bytecode_fuzz_gate.sh` captures progress-visible logs plus summary artifacts.
- [x] `ARCH-VM-15` Add MP-060 CP66 default-switch readiness ledger generation: `runtime_vm_default_switch_readiness.py` records production backend guard, register-vs-stack differential, malformed-bytecode fuzz, benchmark threshold, residual-risk, and rollback evidence while reporting blocked readiness when performance threshold evidence is missing or below target.
- [x] `ARCH-VM-16` Close the CP58 function-POU local-init bridge gap: VM local initialization now evaluates local initializers against VM frame slots and runtime globals/instances without pushing temporary runtime storage frames, with focused architecture, struct-initializer, VAR_STAT, bytecode VM core, and register/stack differential tests passing.
- [x] `ARCH-VM-17` Close the CP58 method direct-path parity gate: method POU direct dispatch is default-on without a rollout env flag, and polymorphic receiver/interface dispatch is locked by focused bytecode VM parity tests.
- [x] `ARCH-VM-18` Close the CP58 direct-binding tests-first parity gate: bytecode VM tests now cover function/FB/method named arguments, omitted default inputs, OUT/IN_OUT copy-back, and module-swap default-metadata invalidation on the direct native-call path.
- [x] `ARCH-VM-19` Close the CP58 call-binding performance gate: focused release `call_binding` benchmark passes the median/p99 target with zero register-program fallbacks; artifact path `target/gate-artifacts/runtime-vm-bench-focused-cp58-call-binding-20260430`.
- [x] `ARCH-VM-20` Close the CP59 string-stdlib focused performance gate: focused release `string_stdlib` benchmark passes the stage-1 and stage-2 median targets with zero register-program fallbacks; artifact path `target/gate-artifacts/runtime-vm-bench-focused-cp59-string-stdlib-20260430`.
- [x] `ARCH-VM-21` Start the CP60/Phase 3 ref-chain structural cleanup with borrow-first dynamic reference reads: `dynamic_ref_index` now peeks array prefixes through borrowed path slices instead of cloning the whole `ValueRef`, VM function-block reference execution reads through the borrowed storage API, and tier-1 `RefField` resolves borrowed reference registers, with architecture and nested partial-index behavior locks.
- [x] `ARCH-VM-22` Add the Phase 3 deep ref-chain parity/trap corpus: register and stack VM paths now share focused tests for nested field/index success and index-trap mapping on deep reference chains.
- [x] `ARCH-VM-23` Add the Phase 3 STRING/WSTRING parity/trap corpus: register and stack VM paths now share focused tests for boundary string element reads, non-ASCII character mapping, and runtime string index traps.
- [x] `ARCH-VM-24` Add the Phase 3 fixture-level benchmark regression self-test: low-noise benchmark summaries now classify per-fixture p95/p99/measured-duration regressions against a configurable budget instead of relying on aggregate-only evidence.
- [x] `ARCH-VM-25` Remove register-IR decode operand heap allocation: decoded instructions now store operands inline and only materialize owned operand bytes for explicit VM fallback instructions.
- [x] `ARCH-VM-26` Fuse hot `SELF.field` dynamic reference load/store chains in register IR: lowering now replaces `LOAD_SELF -> REF_FIELD -> LOAD_DYNAMIC/STORE_DYNAMIC` with self-field dynamic instructions, tier-1 accepts the fused operations, and focused full-demo benchmarks improve p50 with zero register-program fallbacks while the aggregate MP-060 gate remains pending.
- [x] `ARCH-VM-27` Cache VM native conversion specs from measured `refs_sizeof` profile evidence: `perf` showed per-call conversion-name parsing as the top runtime hotspot, so native symbol preparse now stores conversion specs and dispatch uses the cached spec; long focused evidence improved `refs_sizeof` p50 `12.334us -> 9.092us`, p95 `14.037us -> 9.963us`, and p99 `20.556us -> 13.222us` with zero register-program fallbacks.
- [x] `ARCH-VM-28` Remove the measured per-cycle `ProgramDef` clone from VM program dispatch: `Runtime::execute_program_by_name` now validates the program name, ensures the VM module is loaded, and enters `vm::dispatch::execute_program_by_name` without cloning the full program body each cycle; same-profile evidence improved `refs_sizeof` p50 `9.092us -> 5.630us`, p95 `9.963us -> 6.000us`, and p99 `13.222us -> 8.611us` with zero register-program fallbacks.
- [x] `ARCH-VM-29` Fast-path cached VM conversion calls before stdlib map lookup: measured follow-up `refs_sizeof` profile showed conversion calls still paying a missed `StandardLibrary::get`, so cached conversions now bind the single `IN` argument directly and skip that lookup; same-profile evidence improved p50 `5.630us -> 4.889us`, p95 `6.000us -> 5.204us`, and p99 `8.611us -> 6.463us` with zero register-program fallbacks.
- [x] `ARCH-VM-30` Keep pooled register files null-filled instead of clearing and refilling each POU call: measured follow-up `refs_sizeof` profile showed `Vec<Value>::extend_with` from register-file refill and `RegisterExecutionBuffers` drop cost in the hot path; pooled register files now reset slots to `Value::Null` before reuse, improving p50 `4.889us -> 4.519us`, p95 `5.204us -> 4.741us`, and p99 `6.463us -> 6.018us` with zero register-program fallbacks.
- [x] `ARCH-VM-31` Fast-path global VM refs before generic runtime access-target routing: measured follow-up `refs_sizeof` profile still showed `store_ref`/`peek_ref` and direct storage-slot access as the top ref cost, so global refs now bypass the frame-dependent runtime-access helper and use global slot access directly; evidence improved p50 `4.519us -> 4.333us`, p95 `4.741us -> 4.481us`, and p99 `6.018us -> 5.519us` with zero register-program fallbacks.
- [x] `ARCH-VM-32` Enforce fixed STBC resource limits before executable materialization and replace loop-edge-only accounting with one shared original-bytecode instruction budget across stack, register, tier-1, and nested-call execution; the VM execution diagram records the validation and budget ownership boundaries, and focused parity tests pin the same fault boundary across all three paths.
- [x] `ARCH-VM-33` Add one closed stable machine-error vocabulary in `trust-runtime-core`, preserve bytecode and VM identifiers through `RuntimeError` conversion without parsing diagnostic text, and return typed HMI admission codes before write queueing; the runtime VM execution diagram records the shared ownership and boundary flow.
- [ ] `ARCH-VM-34` Preserve IEC assignment-attempt semantics through source lowering, `POU_INDEX` class/interface metadata, and VM opcode `REFERENCE_ATTEMPT`: focused `reference_runtime_` evidence is green (25/25); diagram output regeneration and drift verification remain pending for the final frozen milestone.
- [x] `ARCH-RTCORE-01` Add a dedicated runtime-core/native-host split checklist that freezes the core/host ownership decisions, behavior-lock gates, sync capability traits, Linux/PREEMPT_RT host expectations, and the STM32H7/Opta rollout plan.
- [x] `ARCH-RTCORE-02` Add the deferred runtime-core/native-host split to the full-software architecture refactor plan as a narrowed SOLID/KISS cleanup: revive portable `trust-runtime-core` plus Linux host boundaries, dependency fences, capability traits, behavior-lock gates, and doctor rules, while keeping STM32H7, Opta, ESP32, embedded T0, embedded EtherCAT, no_std product promises, and MCU protocol commitments parked until a separate roadmap decision.
- [x] `ARCH-RTCORE-03` Add `runtime-core-host-split-execution-checklist.md` as the concrete execution board for the narrowed split, with stop rules, behavior-lock tests, doctor rules, staged extraction slices, validation gates, release/merge readiness checks, and explicit embedded non-goals.
- [x] `ARCH-RTCORE-04` Close runtime-core split Phase 1 behavior locks and Phase 2 pre-extraction doctor rules before production code movement: VM/cycle/scheduler/retain/watchdog/initializer behavior locks pass, `FULLMAP-CHECK-05` covers the full runtime-core forbidden dependency/import set, `FULLMAP-CHECK-10` now reports function-size advisories and fails oversized `trust-runtime-core` functions, and `cargo run -p xtask -- architecture-doctor --full-map` passes at `62156e7e9`.
- [x] `ARCH-RTCORE-05` Add the initial `trust-runtime-core` scaffold as a workspace member with default `std`, `no_std`-shaped crate attributes, ownership/exclusion docs, one `scaffold` module, one pre-move unit test, and an explicit `trust-runtime::runtime_core` compatibility re-export; `FULLMAP-CHECK-05` passes with the scaffold present and no forbidden dependencies/imports.
- [x] `ARCH-RTCORE-06` Start Phase 4 value-model movement with the standalone date/time primitives in `trust-runtime-core`, preserving the existing `trust-runtime::value` re-export path and validating the slice with `cargo test -p trust-runtime-core -- --nocapture`, `cargo check -p trust-runtime --lib`, and `cargo check -p trust-runtime-core --no-default-features`.
- [x] `ARCH-RTCORE-07` Move portable memory identity types (`MemoryLocation`, `IoArea`, `FrameId`, `InstanceId`) into `trust-runtime-core` while preserving `trust-runtime::memory` re-exports; validate with runtime-core tests, focused runtime memory tests, no-default core check, clippy for touched crates, and the full-map doctor dependency fence.
- [x] `ARCH-RTCORE-08` Move portable date/time calculation helpers into `trust-runtime-core`, retain a private `trust-runtime` compatibility module for internal imports, and update the full-map top-level module policy so the removed physical `datetime` source module is not treated as stale architecture evidence.
- [x] `ARCH-RTCORE-09` Move `RuntimeError` into `trust-runtime-core`, preserve `trust-runtime::error` as the public compatibility path, keep the core no-default check passing, and update the full-map top-level module policy so the removed physical `error` source module is not treated as stale architecture evidence.
- [x] `ARCH-RTCORE-10` Move portable value reference identities, array-offset helpers, and partial-access parsing into `trust-runtime-core`, preserve the `trust-runtime::value::reference` compatibility re-export, and leave `Value`-dependent path walkers in the host crate until the value model ownership boundary is split by accessor and mutation responsibilities.
- [x] `ARCH-RTCORE-11` Add the compound value accessor boundary needed before moving the full value model: runtime production code now reaches `ArrayValue` and `StructValue` through narrow accessors/mutators instead of private field coupling, with focused value-type, helper-eval, and VM dynamic-reference tests passing.
- [x] `ARCH-RTCORE-12` Move the portable runtime `Value` model into `trust-runtime-core` while keeping the existing `trust-runtime::value` compatibility path, feature-gating HIR-backed constructors behind `trust-runtime-core/hir`, preserving the no-default core check, and classifying the optional `trust-runtime-core -> trust-hir` edge in the full-map policy.
- [x] `ARCH-RTCORE-13` Move numeric conversion helpers into `trust-runtime-core`, preserve the existing internal `crate::numeric` path through a private compatibility re-export in `trust-runtime`, and remove the stale runtime top-level `numeric` module decision from the full-map policy.
- [x] `ARCH-RTCORE-14` Move partial-access read/write semantics into `trust-runtime-core`, preserve the existing `trust-runtime::value` re-export surface, and validate the IEC bit/byte/word/dword access path through the `types_bit_access` integration test.
- [x] `ARCH-RTCORE-15` Move string element and IEC string helper semantics into `trust-runtime-core`, preserve the existing crate-internal `trust-runtime::value` compatibility surface for stdlib/eval/helper-eval/reference paths, and validate the slice with focused core string tests, no-default core check, runtime host compile checks, clippy, full-map doctor, and diagram drift.
- [x] `ARCH-RTCORE-16` Move HIR-backed `SIZEOF` value/type sizing into the optional HIR-feature slice of `trust-runtime-core`, preserve the existing `trust-runtime::value` compatibility API, keep pure core no-default builds free of HIR sizing, and validate runtime `SIZEOF`, bytecode, eval, clippy, full-map doctor, and diagram drift gates.
- [x] `ARCH-RTCORE-17` Move HIR-backed runtime default-value construction into the optional HIR-feature slice of `trust-runtime-core`, preserve the existing `trust-runtime::value` compatibility API, keep pure core no-default builds free of HIR defaults, and validate core defaults, runtime value defaults, reference defaults, clippy, full-map doctor, and diagram drift gates.
- [x] `ARCH-RTCORE-18` Start the program-model record movement with the pure static-name helper boundary in `trust-runtime-core`, preserve the existing `trust-runtime::program_model` compatibility API, and validate core utility tests, static-local runtime integration, clippy, full-map doctor, and diagram drift gates.
- [x] `ARCH-RTCORE-19` Move shared program-model operator semantics into `trust-runtime-core`, preserve the existing `trust-runtime::program_model::ops` compatibility API for VM/eval/helper/stdlib call sites, and validate core operator tests, eval operator/time/error tests, clippy, full-map doctor, and diagram drift gates.
- [x] `ARCH-RTCORE-20` Move portable expression, lvalue, and call-argument records into the optional HIR-feature slice of `trust-runtime-core`, preserve the existing `trust-runtime::program_model` compatibility API, and validate core expression tests, eval/helper expression paths, clippy, full-map doctor, and diagram drift gates.
- [x] `ARCH-RTCORE-21` Move the HIR-backed runtime initializer catalog into the optional HIR-feature slice of `trust-runtime-core`, preserve the existing `trust-runtime::program_model::InitializerCatalog` compatibility API, and validate core initializer tests, runtime initializer architecture tests, no-default core build, clippy, full-map doctor, and diagram drift gates.
- [x] `ARCH-RTCORE-22` Start the bytecode container movement with portable task and bytecode metadata records in `trust-runtime-core`, preserve the existing `trust-runtime::task::TaskConfig` and `trust-runtime::bytecode::*Metadata` compatibility paths, and record that full `BytecodeModule` movement must wait for the public host encoder `from_runtime*` inherent API to be split.
- [x] `ARCH-RTCORE-23` Move bytecode error, reader, and alignment helper ownership into `trust-runtime-core`, preserve `trust-runtime::bytecode` compatibility modules for existing host code, and validate core bytecode helper behavior plus bytecode container/section/validation integration tests, clippy, full-map doctor, and diagram drift gates.
- [x] `ARCH-RTCORE-24` Split host runtime-to-bytecode construction into `trust-runtime::bytecode::build_module_from_runtime*` functions, move runtime and harness call sites off the `BytecodeModule::from_runtime*` inherent API, and keep the old methods as temporary compatibility shims before moving the bytecode container type.
- [x] `ARCH-RTCORE-25` Move pure bytecode format records and raw format constants into `trust-runtime-core`, keep `BytecodeModule` host-side until the public compatibility shims are retired, and validate core format discriminants plus runtime bytecode container, metadata, section decode, and validation tests.
- [x] `ARCH-RTCORE-26` Start VM execution movement by classifying VM modules, moving the pure VM trap/error mapping, operand stack, opcode-length helper, and borrowed-value materialization helper into `trust-runtime-core`, preserving runtime compatibility re-export modules, and validating core VM tests plus `bytecode_vm_core` and runtime VM lib filters.
- [x] `ARCH-RTCORE-27` Move VM frame-stack and local-frame state into `trust-runtime-core`, preserve `trust-runtime::runtime::vm::frames` as a compatibility re-export, and validate local-reference bounds/load/store behavior, frame-stack depth enforcement, `bytecode_vm_core`, runtime VM lib filters, clippy, full-map doctor, and diagram drift gates.
- [x] `ARCH-RTCORE-28` Move pure VM dispatch-op helpers into `trust-runtime-core`, narrow the binary-op helper from full `Runtime` access to `DateTimeProfile`, preserve `trust-runtime::runtime::vm::dispatch_ops` as a compatibility re-export, and validate core dispatch-op tests plus bytecode VM and runtime VM filters.
- [x] `ARCH-RTCORE-29` Move bytecode type-table `SIZEOF` evaluation into `trust-runtime-core` without adding a HIR dependency to the pure core path, keep the HIR-gated `SizeOfError` mapper in the runtime compatibility module, and validate focused core/runtime `SIZEOF` gates plus clippy, full-map doctor, and diagram drift.
- [x] `ARCH-RTCORE-30` Move VM const-pool decoding into `trust-runtime-core`, preserve `trust-runtime::runtime::vm::const_pool` as a compatibility re-export, replace `std` dependencies with `core`/`alloc`, and validate primitive/enum/alias/subrange const decoding plus focused bytecode VM const-error gates.
- [x] `ARCH-RTCORE-31` Close Phase 5 VM execution movement by leaving `debug_map`, `dispatch_refs`, `local_init`, `dispatch`, `call`, and `register_ir` host-owned for their debug/source lookup, runtime storage, initializer, instruction-loop, call-dispatch, lowering/profile/tier1, and host-callback dependencies; Phase 5 closeout passes core VM tests, `bytecode_vm_core`, register-IR parity, bytecode VM differential, and full-map doctor gates.
- [x] `ARCH-RTCORE-32` Start Phase 6 model movement by moving `ResourceState` plus watchdog/retain-mode/fault-policy records into `trust-runtime-core`, preserving `trust-runtime::scheduler::ResourceState` and `trust-runtime::watchdog::*` compatibility paths, keeping host-thread scheduler services in `trust-runtime`, and validating core/no-default, scheduler-state, watchdog behavior-lock, clippy, full-map doctor, and diagram drift gates.
- [x] `ARCH-RTCORE-33` Start Phase 6 cycle movement by moving `ReadyTask` and deterministic ready-task ordering into `trust-runtime-core`, preserving the host-owned I/O/process-image `execute_cycle` boundary, and validating cycle-boundary, I/O-driver, task-order, core/no-default, clippy, full-map doctor, and diagram drift gates.
- [x] `ARCH-RTCORE-34` Move retain/restart policy enums into `trust-runtime-core`, preserve `trust-runtime::RetainPolicy` and `trust-runtime::RestartMode` public re-exports, keep file-backed retain persistence and `RetainSnapshot` host-owned pending a compatibility plan for `RetainSnapshot::from_runtime(&Runtime)`, and validate restart/retain, core/no-default, clippy, full-map doctor, and diagram drift gates.
- [x] `ARCH-RTCORE-35` Move pure fault/watchdog state wrappers into `trust-runtime-core`, preserve the private runtime compatibility modules for existing call sites, keep host fault side effects (`Runtime::apply_fault`, I/O safe state, metrics, debug events) in `trust-runtime`, and validate watchdog/fault behavior locks, core no-default, clippy, full-map doctor, and diagram drift gates.
- [x] `ARCH-RTCORE-36` Move the retained snapshot data model into `trust-runtime-core` while keeping `trust-runtime::RetainSnapshot` as the public compatibility wrapper for `RetainSnapshot::from_runtime(&Runtime)`, keep retain persistence/codecs/runtime collection host-owned, and validate retain-store/restart/reliability behavior plus core no-default, clippy, full-map doctor, and diagram drift gates.
- [x] `ARCH-RTCORE-37` Close Phase 6 host-owned realtime classifications: Linux `PREEMPT_RT` setup, `mlockall`, CPU affinity, scheduler-policy observation, T0 SHM transport, page pinning, and T0 cycle exchange-point enforcement stay in `trust-runtime` unless a separate approved T0 contract split is opened; Linux RT and realtime/T0 focused tests pass.
- [x] `ARCH-RTCORE-38` Move portable task readiness state and overrun accounting into `trust-runtime-core`, preserving host ownership of cycle I/O, process-image exchange, debug/metrics side effects, VM dispatch, retain persistence, and fault recording; validate task/cycle/scheduler behavior, core no-default, clippy, full-map doctor, and diagram drift gates.
- [x] `ARCH-RTCORE-39` Close the Phase 7 Linux host rewire: `trust-runtime` consumes `trust-runtime-core` through compatibility reexports and crate-private host helpers, CLI/config/registry/harness smokes pass, public API snapshot differences are reviewed, and `TaskReadiness`/`evaluate_task_readiness` remain internal rather than becoming public runtime API.
- [x] `ARCH-RTCORE-40` Close the Phase 8 product/workbench coordination gate using existing full-map policy: command variants, bin modules, nested action enums, route exceptions, `bundle_builder`, workbench/dev modules, and CLI infrastructure helpers are classified; focused doctor fixtures prove product-to-workbench imports and core host-dependency regressions fail.
- [x] `ARCH-RTCORE-41` Close the runtime-core split map/diagram phase: `architecture-doctor --full-map` regenerates the current source-derived map artifact for `c2f8d43ac`, `FULLMAP-P7` verifies the generated full-software diagram, diagram rendering passes, and diagram drift is clean with no embedded support claim added.
- [x] `ARCH-RTCORE-42` Complete the runtime-core split Phase 10 focused and runtime validation pass: core, VM/bytecode, scheduler/cycle, retain/watchdog, initializer/value, full-map doctor, runtime vertical, fmt, clippy, and fast workspace tests pass; the full workspace gate exposed and fixed a stale software-map schema assertion before the final rerun.
- [x] `ARCH-RTCORE-43` Close the runtime-core split final workspace/release-readiness gate: final `just test-all` passes after the stale software-map schema assertion fix, the runtime array-repetition initializer bug has focused regressions and changelog coverage, workspace and VS Code versions are synchronized at `0.24.9`, and the runtime-core split exit criteria are complete with embedded support still deferred.
- [x] `ARCH-RTCORE-44` Separate portable numeric kind/widening ownership from execution-domain conversion normalization without changing the public `trust_runtime_core::numeric` API or cataloged test identities; both pre/post behavior locks, the no-default core check, SOLID/KISS/DRY review, rendered diagram drift, and the focused Phase 18 metadata route are green.
- [x] `ARCH-RTCORE-45` Separate portable task configuration, scheduling-state initialization, and event/periodic readiness ownership without changing the public `trust_runtime_core::task` API or cataloged test identities; pre/post task locks, the no-default core check, SOLID/KISS/DRY review, rendered diagram drift, and the focused Phase 18 identity/state-authority route are green. Configuration and readiness authority remain explicit Phase 18 debt.
- [x] `ARCH-FULL-01` Add `full-architecture-refactor-program-checklist.md` as the umbrella board that states the runtime-core split is one branch of the full SOLID/KISS and zero-silent-bug program, not the whole program.
- [x] `ARCH-FULL-02` Add `architecture-doctor-full-map-execution-checklist.md` so `architecture-doctor --full-map` becomes a concrete prerequisite for enforcing workspace, runtime-core, CLI, host-surface, KISS, API, and diagram rules.
- [x] `ARCH-FULL-03` Add `hir-mutation-hardening-execution-checklist.md` for the F2 silent-bug gap: `symbol_import`, `type_check::const_eval`, and aggregate initializer validation mutation survivors.
- [x] `ARCH-FULL-04` Add `parser-recovery-hardening-execution-checklist.md` for the F3 parser recovery gap: bounded scanner API, targeted diagnostics, and fuzz/property coverage.
- [x] `ARCH-FULL-05` Add `runtime-cli-product-workbench-split-checklist.md` for the F10 SRP gap: product runtime commands versus agent/git/docs/prompt/workflow/style/CI workbench commands.
- [x] `ARCH-FULL-06` Add `runtime-host-surface-ownership-checklist.md` for the F11 SRP/KISS gap: `web`, `hmi`, `ui`, `control`, and `runtime_cloud` ownership and ports.
- [x] `ARCH-FULL-07` Fix the runtime CLI split taxonomy so command variants and bin modules are classified separately, allowing doctor rules to catch both unclassified enum variants and unclassified `src/bin/trust-runtime/*.rs` modules.
- [x] `ARCH-FULL-08` Rewrite `architecture-doctor-full-map-execution-checklist.md` from a requirements list into an executable phased plan covering data model, policy loader, dependency checks, forbidden-import scanner, command/module checks, KISS/API gates, diagram claim checks, report writer, and failing fixtures.
- [x] `ARCH-FULL-09` Add `dependency-hygiene-execution-checklist.md` for F4 cargo audit/machete/deny policy and `third_party/tiverse-mmap` workspace metadata cleanup.
- [x] `ARCH-FULL-10` Add `runtime-large-file-split-execution-checklist.md` for F8 quantitative KISS gates and owner/split notes for every runtime Rust file over 1,000 lines.
- [x] `ARCH-FULL-11` Add `runtime-vm-mutation-hardening-execution-checklist.md` so zero-silent-bug claims cover high-risk VM execution paths, not only HIR.
- [x] `ARCH-FULL-12` Tighten `runtime-core-host-split-execution-checklist.md` so the full-map doctor MVP is a hard Phase 0 prerequisite or requires an explicit owner-approved waiver.
- [x] `ARCH-FULL-13` Tighten remaining architecture boards: add full-map doctor hard prerequisites to CLI/host-surface checklists, add dependency-hygiene stop rules and failing fixtures, make host-surface Phase 4 named-file driven, lock exact VM mutation command shapes, add nested CLI action classification, add runtime-host module-count cap, and add `unsafe-concurrency-hardening-execution-checklist.md`.
- [x] `ARCH-FULL-14` Advance the runtime CLI product/workbench split by moving the `agent serve` implementation and workflow helper to `trust-dev`, retaining `trust-runtime agent serve` as a deprecated forwarding alias, updating public docs and system architecture diagrams, and validating the full agent JSON-RPC integration suite.
- [x] `ARCH-FULL-15` Close the runtime CLI product/workbench split by moving `docs` and ST `test` implementations to `trust-dev`, retaining deprecated `trust-runtime docs/test` forwarding aliases, preserving stable CI exit-code classification through shared CLI infrastructure, updating public docs/terminal captures/system diagrams, and validating focused docs/test/CI/OSCAT/PLCopen compatibility plus `FULLMAP-CHECK-06`.
- [x] `ARCH-FULLMAP-01` Start `architecture-doctor --full-map` implementation with an explicit non-zero scaffold, `SoftwareMap` Rust data model, stable JSON serialization, and focused xtask unit tests. The command is intentionally not a passing doctor yet.
- [x] `ARCH-FULLMAP-02` Extend `architecture-doctor --full-map` to write the initial source-derived JSON map under `target/gate-artifacts/full-software-map-<commit>/software-map.json`, including workspace packages, targets, direct workspace dependency edges, source file counts, and tool status records while still failing until policy checks are implemented.
- [x] `ARCH-FULLMAP-03` Extend the JSON map with `trust-runtime` top-level modules, runtime CLI command variants, nested `*Action` enums, and runtime bin modules so the later product/workbench and host-surface policies have source-derived inputs.
- [x] `ARCH-FULLMAP-04` Complete `architecture-doctor --full-map` MVP policy enforcement: source-derived module/import/line-count/unsafe/diagram facts, workspace/core/CLI/host-surface/KISS/dependency/public-API checks, JSON and Markdown reports, and known-bad unit tests for each blocking rule.
- [x] `ARCH-FULLMAP-05` Add staged validation cadence to the full-map and umbrella architecture checklists so focused xtask/doctor checks are the implementation loop and `just test-all` is reserved for merge, release, risky rebase, large cross-crate refactor, or board-completion gates.
- [x] `ARCH-FULLMAP-06` Tighten full-map doctor honesty reporting after review: named CLI route metadata replaces ambiguous command-mapping questions, host-surface approved-port enforcement reports `partial` until ports exist, unsafe/concurrency reports a `finding` while hotspots remain, final host-cap activation is called out, and known-bad tests use a searchable naming convention.
- [x] `ARCH-FULLMAP-07` Verify full-map command route policies against source symbols so a stale handler such as `run::run_play` missing from source fails `FULLMAP-CHECK-06` instead of passing through rationale text.
- [x] `ARCH-FULLMAP-08` Close architecture-program umbrella cleanup with enforced module-size, function-size, public API snapshot, and runtime top-level growth gates; source-checked diagram claim evidence; final architecture report; and a dated runtime host module cap waiver naming `architecture/runtime-host-module-collapse`.
- [x] `ARCH-FULLMAP-09` Add post-closeout gap tracking so performance/compile-time/binary-size baselines, host-module collapse, registered runtime large-file leftovers, the oversized `trust-runtime-core/src/value/types.rs` follow-up, `trust-dev` package-boundary wording, PLCopen status, geiger advisory-partial status, and raw audit upgrade work are tracked outside the closed architecture-program umbrella.
- [x] `ARCH-FULLMAP-10` Execute the post-closeout package-boundary cleanup: `trust-dev` is now a separate Cargo package, PLCopen XML import/export is now `trust-plcopen`, public API snapshots include the new package surface, and `trust-runtime` retains compatibility paths for shipped command/API behavior.
- [x] `ARCH-FULLMAP-11` Execute the post-closeout KISS cleanup: host-only runtime modules now live under `crates/trust-runtime/src/host`, the full-map policy enforces the final `trust-runtime` top-level module cap of 18 without a waiver, and remaining runtime/runtime-core files over 1,000 lines were split.
- [x] `ARCH-FULLMAP-12` Add a dedicated external safety/dependency follow-up board for the residual `cargo geiger` advisory-partial state and raw `cargo audit` upgrade paths so those risks are not hidden inside the closed architecture-program umbrella.
- [x] `ARCH-FULLMAP-13` Close the external unsafe scanner reliability residual with a CI-enforced `ast-grep` structural unsafe gate. Evidence: `scripts/architecture_external_safety_ast_grep_gate.sh` emits `architecture-external-safety-*` artifacts and fails on unregistered unsafe constructs, stale exact register entries, or unsupported unsafe trait/extern forms; `.github/workflows/ci.yml` runs the gate in `Architecture Safety` before full-map doctor artifacts are uploaded.
- [x] `ARCH-UNSAFE-01` Close BOARD-11 unsafe/concurrency hardening with source-derived unsafe, panic-like, and concurrency boundary registers in full-map policy; deterministic scheduler/memory poison-recovery tests; focused Miri, sanitizer, and Valgrind gates; advisory geiger evidence with exact workspace/tool blocker; and `FULLMAP-CHECK-09` known-bad fixtures for unregistered unsafe, unclassified panic-like sites, unregistered concurrency boundaries, and missing tool gates.
- [x] `ARCH-DEPHYG-01` Close the dependency hygiene board with project `deny.toml`, explicit audit/deny metadata, workspace-scoped `cargo machete` cleanup, `third_party/tiverse-mmap` workspace exclusion, full-map dependency hygiene reporting, and canned negative fixtures for audit, machete, deny metadata, workspace membership, and failed tool status.
- [x] `ARCH-DEPHYG-02` Close the MQTT TLS follow-up with explicit `rumqttc` native-tls feature selection, `io.params` TLS/mTLS config, remote plaintext gating, MQTT TLS tests, public `io.toml` docs, and refreshed system architecture diagram evidence.
- [x] `ARCH-HIRMUT-01` Close the HIR mutation hardening board with focused `cargo mutants` shards for `symbol_import.rs`, `type_check/const_eval.rs`, and `collector/variables.rs`: final evidence records 185 mutants tested, 178 caught, 7 unviable, 0 missed, and 0 timeouts.
- [x] `ARCH-HIRMUT-02` Add source-derived cross-file import tests for namespace scopes, callable symbol-kind type remapping, raw compound type imports, `TypeId` collision handling, and initializer ID translation.
- [x] `ARCH-HIRMUT-03` Add const-eval and aggregate-default mutation matrices that assert exact diagnostic codes/messages/locations where behavior is locked, including nested required defaults, array repetition, reference `NULL`, FB member legality, and scope-chain constants.
- [x] `ARCH-HIRMUT-04` Fix mutation-discovered HIR validation gaps: direct array repetition defaults now validate repeated elements, non-repeat call expressions used as array defaults now emit `TypeMismatch`, and the unobservable `LINT` range-check arm was removed.
- [x] `ARCH-HIRZERO-01` Close the newly mapped full HIR zero-silent-bug architecture gap with `hir-zero-silent-bug-refactor-checklist.md`; this is separate from the completed HIR mutation board because it covers all diagnostic-bearing HIR semantic decisions, not only `symbol_import`, `const_eval`, and aggregate initializer validation. Evidence: the detailed HIR zero-silent-bug board is complete, the missing semantic-kernel allowlist and allowlist artifact are now tracked, and `python3 scripts/hir_zero_silent_bug_doctor.py --fail` reports no findings.
- [x] `ARCH-HIRZERO-02` Extend full-map doctor coverage so HIR broad lookup, duplicated resolver/type-resolution/import-resolution logic, ambiguous diagnostic suppression, partial semantic identity keys, and runtime semantic bypass paths cannot be added outside the semantic-kernel allowlist. Evidence: `FULLMAP-HIRZSB` runs `python3 scripts/hir_zero_silent_bug_doctor.py --fail` inside `cargo xtask architecture-doctor --full-map`, has known-bad/pass xtask tests, and fails full-map when the HIR doctor reports findings.
- [x] `ARCH-PARSERREC-01` Close the parser recovery hardening board with bounded top-level scan/recovery helpers, declaration-boundary tests for malformed aggregate/positional initializers, `FULLMAP-PARSERREC` doctor enforcement, and copied-tree mutation slices with 38 caught, 2 unviable, and 0 missed mutants across the touched parser recovery functions.
- [x] `ARCH-RTLINUX-01` Add a dedicated PREEMPT_RT Linux checklist and mark the broader native-host / embedded split plan deferred so the active runtime portability direction is explicit.
- [x] `ARCH-RTLINUX-02` Refresh the runtime/system architecture diagrams so Linux `PREEMPT_RT` posture is shown as a launcher-owned scheduler-thread step with runtime.realtime config feeding verification/memlock/affinity behavior.
- [x] `ARCH-RTVALUE-01` Refresh the runtime execution architecture diagram so enum value identity is owned by validated construction, alias resolution, retained-state canonicalization, and explicit equality semantics.
- [x] `ARCH-RTVALUE-02` Refresh the runtime execution architecture diagram so struct and array values own declared identity, field/type validation, shape validation, and retained-state failure diagnostics without a global value factory.
- [x] `ARCH-RTVALUE-03` Regenerate the PlantUML outputs and refresh `docs/diagrams/manifest.json` after the runtime value contract updates.
- [x] `ARCH-ADS-CLIENT-01` Give ADS client identity one shared `host/ads/identity.rs` owner for onboarding canonicalization and strict Doctor/backend authority, and keep client construction behind the small private `connect_with` closure seam rather than another adapter or trait. Evidence: focused backend and onboarding identity/Doctor tests pass on `trust-builder`; `runtime-ads-client.puml` and its generated SVG show the shared owner and private seam; remote `scripts/render_diagrams.sh` plus `python3 scripts/check_diagram_drift.py` pass and the refreshed manifest also passes the local drift check.
- [x] `ARCH-ADS-CLIENT-02` Keep ADS client network authority in the bounded worker owner and generation-tagged local intent in the bounded cache owner: connection candidates publish only after exact handle/subscription correlation and stable symbol-token bracketing, poll/notification results validate before mutation, older write completions or projections cannot overwrite newer intent, and shutdown revokes cached input authority. Evidence: the 12 focused worker/cache contract tests and 36 legacy ADS client locks pass on `trust-builder`; `runtime-ads-client.puml` records the ownership and authority flow.
- [x] `ARCH-ADS-SERVER-01` Refresh ADS architecture diagrams for the ADS server product surface: standalone `trust-ads-server` protocol crate, `[runtime.ads_server]` startup, source-pinned client allowlist, scan-safe snapshot reads, runtime-boundary write enqueue, `ads.server.write` audit, and loopback/pyads/TwinCAT proof separation. Evidence: `runtime-ads-server.puml`, refreshed `runtime-execution.puml`, refreshed `system-architecture.puml`, rendered SVGs, refreshed `docs/diagrams/manifest.json`, and remote scratch `scripts/render_diagrams.sh && python scripts/check_diagram_drift.py` pass.
- [x] `ARCH-ADS-CLIENT-02` Keep Network Canvas ADS browse and lifecycle ownership split: the panel lifecycle shell stays below 1,000 lines; protocol, configuration, fleet, discovery, refresh, browse-session, and webview-host state have focused modules; ADS browse carries one validated AMS Net ID/port through request-scoped browse/import contracts with structured outcomes. Evidence: `runtime-ads-client.puml` and generated SVG/manifest refreshed; the stale full-map large-file waiver for the now-883-line `browse_symbols.rs` was removed; remote scratch diagram render/drift check, full-map doctor, focused runtime browse/import tests, and VS Code session/issue contract tests passed.
- [x] `ARCH-ADS-SERVER-02` Keep ADS server Doctor orchestration separate from its private AMS/TCP loopback client: `doctor.rs` owns live status, step ordering, and evidence truth while `doctor/loopback_client.rs` owns framing, response correlation, handle validation, notification parsing, and socket I/O. Evidence: `doctor.rs` is reduced below the 1k-line guardrail, the focused loopback owner is bounded, all 11 focused Doctor tests, all 3 legacy Doctor locks, and the live-symbol lifecycle lock pass on `trust-builder`; `runtime-ads-server.puml` shows the dependency, and remote rendering plus generated diagram/manifest drift checks pass.
- [x] `ARCH-COMM-PANEL-01` Keep the Communication setup surface split by ownership: runtime `comm.*` handlers own contracts/apply/probe behavior, `comm_handlers/schema.rs` owns response assembly and configured-instance discovery, `comm_handlers/schema/fields.rs` owns field descriptors, and VS Code owns rendering/deep links without protocol defaults. Evidence: `schema.rs` reduced from 1127 lines to handler-sized orchestration, field catalog moved to `schema/fields.rs`, and focused `comm_schema`, `comm_apply`, and `comm_test` runtime tests pass locally after the split.
- [x] `ARCH-RTHOST-01` Keep runtime host-surface evidence current while closing F11: `FULLMAP-CHECK-07` now enforces active approved-port drift checks, HMI runtime read/write access is behind `control/hmi_runtime_ports.rs`, runtime-cloud profile/link/rollout/config/control-proxy/IO-proxy policy is split into `runtime_cloud::*_policy` modules, HMI websocket event semantics are owned by `hmi/runtime_views/events.rs` while `web/hmi_ws.rs` stays transport-only, runtime snapshot/status contracts are locked by `RTHOST-P5-003`, runtime-cloud projection contracts are locked by `RTHOST-P5-004`, route thin-adapter contracts are locked by `RTHOST-P5-005`, browser-visible verification evidence is recorded by `RTHOST-P5-006`, and `RTHOST-EXIT-01` through `RTHOST-EXIT-05` close the board.
- [x] `ARCH-RTHOST-02` Isolate embedded-web request handling behind a bounded dispatcher: `web/server.rs` owns only accept and route orchestration, `web/request_dispatch.rs` owns fixed read/body admission lanes and the stable overload response, incomplete bodies cannot occupy all read workers, and focused socket plus real-browser tests keep HMI responsive under a held partial request.
- [x] `ARCH-RTHOST-03` Give runtime control authorization one internal operation registry for reviewed operation names, fixed minimum roles, and debug-surface classification; keep parameter-sensitive role decisions in `control/policy.rs`, default unclassified operations to Admin, and return the stable `insufficient_role` wire code before dispatch.
- [x] `ARCH-RTHOST-04` Keep response ordering at the runtime control transport boundary: an accepted shutdown carries one deferred stop action, TCP/Unix and HTTP control transports attempt the complete correlated response write first, and direct in-process dispatch retains immediate completion without duplicating shutdown policy.
- [x] `ARCH-ZEROSILENT-01` Add a zero-silent-error observability gate for long-running subprocess-based tests and release gates: every gate must show the active phase, project/test target, child PID or command identity, elapsed time, and timeout reason while it is running; captured-only stdout/stderr is not acceptable for expensive gates because it hides the real failing/stalled component until process exit. Evidence: `python3 scripts/check_gate_observability.py` failed before wiring with missing markers for CI and runtime gate scripts; `scripts/run_with_progress.py` now emits live `gate-progress` events with phase, target, PID, elapsed time, timeout, command, exit status, and timeout reason; `.github/workflows/ci.yml` wraps cargo build, runtime warning checks, full tests, conformance passes, VS Code binary build, and VS Code integration tests; runtime comms/security/TLS/VM determinism/VM bench gates all run expensive child commands through the same wrapper. Validation: `python3 scripts/run_with_progress.py --self-test`; `python3 scripts/check_gate_observability.py --self-test`; `python3 scripts/check_gate_observability.py`; live success smoke through `run_with_progress`; live timeout smoke through `run_with_progress`; `bash -n scripts/runtime_comms_conformance_gate.sh scripts/runtime_comms_fuzz_gate.sh scripts/runtime_comms_bench_gate.sh scripts/runtime_cloud_security_profile_gate.sh scripts/runtime_mesh_tls_stability_gate.sh scripts/runtime_vm_bench_gate.sh scripts/runtime_vm_determinism_reliability_gate.sh`.
- [x] `ARCH-ZEROSILENT-02` Enforce regression-test-first bug fixes: before changing compiler, runtime, parser, HIR, VM, CLI, web, or test-harness behavior for a bug, add or identify a focused failing test that reproduces the bug; only then patch the implementation and rerun that focused test before broader gates. Evidence: `scripts/check_regression_test_first.py` now scans PR/commit text for bug-fix signals, checks changed compiler/runtime/parser/HIR/VM/CLI/web/test-harness paths, accepts changed test files, inline test additions, or an explicit `Regression-test-first: <focused failing test command or existing failing test>` marker, and fails the known-bad self-test case where a bug-fix signal changes HIR behavior with no regression-test evidence. CI runs the checker self-test and the live PR/push range check in the format job after a full-depth checkout. Validation: `python3 scripts/check_regression_test_first.py --self-test`; `python3 scripts/check_gate_observability.py`; PyYAML parse of `.github/workflows/ci.yml`; `git diff --check`.
- [x] `ARCH-ZEROSILENT-03` Apply the zero-silent-error rule to the OSCAT example gate specifically: the default OSCAT catalog gate stays structural/fast, the explicit exhaustive 98-project CLI sweep must print per-project progress and timeout diagnostics, and any one-compile aggregate replacement must first have focused regression tests for the namespace/HIR issues it exposes. Evidence: `oscat_oop_example_st_unit_tests_pass` remains `#[ignore]` as the explicit expensive 98-project CLI sweep; `oscat_examples_use_grouped_oop_non_oop_layout` keeps the catalog at 49 paired examples; the sweep reports each active project before spawning `trust-runtime test --project`; child lines include PID, command, project, elapsed progress, timeout limit, and now explicit `reason=timeout`; the one-compile aggregate path remains the focused `oscat_airport_baggage_namespace_aggregate_trigger_passes` regression fixture from the original HIR/namespace trigger. Validation: `RUSTUP_TOOLCHAIN=1.95 cargo test -p trust-runtime --test oscat_oop_examples oscat_example_child_lines_include_pid_project_and_elapsed_context -- --nocapture`; `RUSTUP_TOOLCHAIN=1.95 cargo test -p trust-runtime --test oscat_oop_examples oscat_example_gate_reports_active_project_before_running_child -- --nocapture`; `RUSTUP_TOOLCHAIN=1.95 cargo test -p trust-runtime --test oscat_oop_examples oscat_examples_use_grouped_oop_non_oop_layout -- --nocapture`.
- [x] `ARCH-ZEROSILENT-04` Keep the runtime communications conformance gate fail-closed: every named Cargo filter must report at least one passing native test, the gateway development case tracks the permissive development profile, and separate WAN unit cases own default-deny and allowlist coverage. Evidence: `python3 -m unittest scripts.tests.test_runtime_comms_conformance_gate -v` records the expected pre-fix zero-test assertion failure and passes after the gate hardening; the complete remote communications conformance gate executes every mapped case.
- [x] `ARCH-RTCONN-02` Remove the tracked repo-root PLC scratch files from the source-derived root surface while keeping ignored local runtime artifacts out of the tracked tree. Evidence: `src/main.st` and `src/config.st` are deleted, `.gitignore` already ignores `/program.stbc`, `/runtime.toml`, and `/io.toml`, `docs/diagrams/architecture/full-software-map-generated.puml` now renders the tracked root PLC source surface as `none`, and Phase 9 evidence records the grep/reference check.
- [x] `ARCH-RTRESOURCE-01` Make active IEC resource identity explicit runtime-owned state: configuration lowering sets the declared single-resource identity, bytecode encoding preserves it in `RESOURCE_META`, named application rejects unknown resources without primary fallback, and the one-entry `RESOURCE` placeholder remains a bounded legacy compatibility path. Evidence: focused `bytecode_encoder`, `process_image`, and guided setup tests plus the refreshed `runtime-execution.puml` flow.
- [x] `ARCH-RTRELOAD-01` Make restart and online-change preparation transactional: `restart.rs` builds the replacement variable/instance image and retained-value migration off to the side, failures preserve live storage/lifecycle/debug state, and `online_change.rs` performs only infallible restart/retain commits after bytecode validation succeeds. Evidence: focused retained-migration rejection and restart-construction failure regressions plus the refreshed `runtime-execution.puml` flow.
- [x] `ARCH-RTRETAIN-02` Keep restart construction, retained-snapshot migration, and retain-store policy in separate bounded owners: `restart.rs` prepares restart storage, `retain_snapshot.rs` captures qualified global/program identities and validates into a cloned candidate storage image, and `retain_store.rs` owns the atomic Warm load transaction plus Cold store bypass. Harness and resource-runner restart surfaces delegate to that transaction. Evidence: the focused reliability red/green regression, restart migration unit tests, runtime vertical tests, and the refreshed `runtime-execution.puml` flow.
- [x] `ARCH-EDIT-01` Document the Phase 12 editor-intelligence request-flow changes: `ServerState` owns call-hierarchy incoming/outgoing caches keyed by document generation, document mutations invalidate those caches, document highlight stays current-file scoped, references and struct-field go-to-definition use text prefilters before semantic scans, and semantic-token classification precomputes declaration/member/type ranges instead of repeating ancestry probes per token. Evidence: `docs/diagrams/lsp/lsp-request-flow.puml`, `docs/diagrams/architecture/system-architecture.puml`, refreshed generated diagrams/manifest, and `docs/internal/testing/evidence/runtime-safety-hardening/2026-07-05/phase-12/edit-impl-008-workspace-cost-green-output.txt`.
- [x] `ARCH-RELEASE-01` Make release evidence generation an explicit fail-closed flow: dependency exceptions and the VS Code lockfile audit gate the workflow, conformance status derives from a successful CI artifact plus current known gaps, provenance binds tag, commit, workflow, and artifact identity, and public claims remain blocked beyond targeted G1 proof until native, published, or device evidence exists. Diagram: `docs/diagrams/architecture/release-evidence-flow.puml`.
- [x] `ARCH-SIMPLANT-01` Keep simulation plant ownership split between bounded
  configuration validation, event, actuator, uncertainty, and orchestration
  modules; preserve the process-image pre/post-cycle boundary and independent
  seeded streams. Evidence: native model, replay, event, sensor, and real
  `Control.PID` closed-loop tests plus `simulation-plant-system.puml`.
- [x] `ARCH-LESSON01-01` Keep the manual lesson's sequence in ST and its thermal
  calculation in the existing simulator. Both refrigeration and door timing
  use experiment enablement; independent thermal inputs preserve the scalar
  balance. No new runtime API or duplicate physical model is introduced.
  SOLID/KISS/DRY review: one sequence owner, one thermal model, existing typed
  enable/restore path; source-selection tests live in a bounded test module.
  Native clock/startup/source-selection red-green evidence is retained under
  `control-remediation/2026-09-05/lesson1-*`. Diagram:
  `control-lesson01-sequence.puml`; remote `scripts/render_diagrams.sh` and
  `python3 scripts/check_diagram_drift.py` passed. This closes the ownership
  correction only; individual lesson acceptance remains in the rebuild ledger.
- [x] `ARCH-SIMPLANT-02` Keep model equations separate from fixed-step
  orchestration, typed numeric I/O, status/trace, and transactional rollback;
  preallocate execution workspaces and reject physical I/O drivers before a
  simulation runtime starts. Evidence: native plant, failed-substep, atomic
  publication, bounded-history, and launcher safety tests plus the refreshed
  `simulation-plant-system.puml`.
- [x] `ARCH-SIMPLANT-03` Keep local-model validity in one bounded owner between
  actuator/state/output calculation and the existing transactional checkpoint;
  out-of-range input, state, or ideal output faults without clamping or partial
  publication. Evidence: the focused validity assertion red and paired green,
  full 41-test plant suite, Specification 33 Section 3.7, and the refreshed
  `simulation-plant-system.puml` source. Generated-diagram and manifest proof is
  deferred to the frozen curriculum milestone gate.
- [ ] `ARCH-PSUI-01` Split the oversized Devices & Connections React and host
  owners behind current behavior locks: graph shell, add workflow, drawer
  routing, refresh state, message routing, runtime requests, offline project
  access, and topology assembly each retain one reason to change. Evidence
  requires the registered Extension Host locks and the rendered
  Edit/add/cancel/focus/filter journey before and after every move.
- [ ] `ARCH-PSUI-02` Keep the plant contract, seven-model descriptor catalog,
  validation, project repository, runtime client, topology projection, forms,
  and typed Control Studio handoff in the dependency direction documented by
  `control-engineering-ownership.md` and `simulation-plant-system.puml`.
- [x] `ARCH-PSUI-RUNTIME-01` Route live and stopped-project plant operations
  through one shared handler and `PlantControlService`, while keeping scheduler
  status/trace live-only and JSON-lines framing/correlation in adapters.
  Evidence: 16 focused runtime contract tests, the stopped-project command
  integration test, 9 registered Extension Development Host contract tests,
  and the refreshed `simulation-plant-system.puml`.
- [x] `ARCH-PSUI-RUNTIME-02` Reuse the existing canvas brick removal
  interaction while routing saved simulated-plant removal through the same
  stopped-project plant session and shared revision/idempotency service. The
  plant-specific mutation atomically removes one `[[plants]]` table and only
  its owned `[[plant_events]]` tables. Evidence: focused runtime and registered
  Extension Host red-green tests plus the real VS Code Remove/Cancel/Confirm
  journey and exact `simulation.toml` inspection under
  `vscode-plant-simulation-ui/full-interaction-validation-2026-08-25/remove/`.
- [ ] `ARCH-CONTROL-REMEDIATION-01` Keep plant starting-state descriptors separate
  from the shared indexed scalar editor and runtime physics. Keep pending project
  landing persistence/dispatch separate from activation notifications; starter and
  example producers share one authority rather than independent activation races.
  Folder switching/retry now has its own small adapter, shared by both producers;
  retry cannot invoke copy/scaffold. Lesson-copy locations have a separate
  immutable registry and retained-project reopening owner: copying awaits both
  the lesson-path record and structured landing intent before folder opening,
  while reopening accepts only an explicitly selected persisted path and reports
  a missing path without recreating it. Created and existing destinations share
  one internal folder-opening retry loop with truthful context-specific wording.
  Gallery message handling and HTML rendering are separate modules, with
  canonical lesson detail projection reused rather than a duplicate syllabus.
  The landing and simulation diagrams include these owners and the
  optional-settings summary adapter; outputs were regenerated on the builder and
  the refreshed manifest passes local and remote drift checks.
  SOLID/KISS/DRY review: one descriptor table, one scalar editor and one landing
  dispatcher; no parallel simulation engine or lesson catalog introduced.
  Focused native and rendered red/green evidence is under
  `control-remediation/2026-09-05`; changed plant, landing and existing Control
  Studio diagrams were regenerated on the builder and drift checks pass locally
  and remotely. Final integrated gates remain required before closing this row.
- [ ] `ARCH-CSTUDIO-01` Keep Control Studio loop discovery, sample acquisition,
  experiment safety, analysis, artifacts, controller apply, Automatic Tune,
  runtime transport, editor host, webview state, plots, and export as focused
  owners with transport-independent domain types and one qualified numerical
  adapter. Evidence requires `control-studio.puml`, focused contract tests, and
  diagram drift checks.
  - Feedback-relay correction (2026-08-29): the extension-owned experiment
    planner now sends a typed PV threshold contract rather than manufacturing a
    timed square-wave trajectory. The existing runtime experiment owner reads
    the reviewed process-image binding, performs hysteresis switching under the
    same lock as abort, and reuses the ordinary timeout/restoration state
    machine. Focused extension and runtime red-green evidence is recorded in
    curriculum row `CELP-053`; the row remains open for the complete Control
    Studio ownership audit and regenerated diagram drift evidence.
  - Open-loop curriculum correction (2026-09-01): the same loop-definition and
    monitor owners now distinguish closed-loop SP/PV/MV from open-loop
    input/output evidence. No setpoint, error metric, transport, or monitor is
    synthesized for open-loop lessons. Focused loop-definition and monitor
    red-green tests cover the boundary; rendered proof remains open.
  - Project-scripted lesson activation (2026-09-01): the existing runtime
    experiment service now grants and restores one typed Boolean lesson
    authority. The lesson program remains the sole owner of its synchronized
    scenario, so Control Studio does not inject a parallel excitation.
  - Empty terminal verification display (2026-09-08): the recorded-response
    owner routes both retained actual-ST rows and an explicit no-sample recovery
    state through the same workspace. The webview never substitutes a chart or
    table for an empty terminal result; its one direct `Retry Verify` action
    creates a fresh attempt while retaining the failed record, and Apply stays
    disabled until a later verified response has samples. Focused real VS Code red/green and
    light/dark/high-contrast evidence are retained under
    `control-studio-workflow/2026-09-07/resume/control-fault-empty-*`; native
    generation of that empty result remains an open runtime contract case.
  - Main-workflow repair (2026-09-09): `EngineerWorkflowSession` delegates
    supplied-model compatibility, automatic controller recommendation and
    cause-to-next-stage routing to focused modules. The Control Studio diagram
    now maps those owners and their model/task/controller/failure flows.
    `control-studio.svg` and the manifest were regenerated locally with the
    current PlantUML jar and passed `check_diagram_drift.py`; remote regeneration
    is pending because the available builder validation checkout is dirty.
  - Test-owned acquisition (2026-09-09): `ExperimentRecordingOwner` is separate
    from Manual Record and starts the same `RuntimeRecordingSession` before a
    reviewed Test excitation. Only complete contiguous runtime scans reach the
    immutable Test-to-Identify ledger; input selection follows the reviewed
    setpoint, disturbance or MV Test signal. The real Routh ST journey passes in
    dark/light/high contrast with retained response evidence. Test-owner
    normal-window same-runtime and runtime-loss recovery now preserve a stop-only Test owner; program/binding recovery remains open.
  - Automatic Test planning (2026-09-09): the experiment owner resolves a
    normal Automatic request into a reviewed bounded step from current mode and
    headroom, writes an explicit Manual command address/type when applicable,
    and preserves actual MV/PV evidence for fitting. The held-step fitter owns
    restoration exclusion and Test record IDs establish independent deterministic
    trials. Focused red/green and actual light/dark/high-contrast evidence are
    retained in `control-studio-workflow/2026-09-07/resume/test-identification-acceptance`.
  - Declared PI Auto boundary (2026-09-09): `pidInitialization` owns the
    explicit retained integral/derivative state contract; `PidCommissioning`
    composes it with the existing parameter transaction, isolated actual-ST
    verification, recording and transfer client. The runtime transfer owner
    validates `parallel-pid-v1`, aligns only the Manual-to-Auto entry, confirms
    the declared one-scan state evolution, and preserves retained state for the
    later target response and Manual recovery. No state is inferred from ST
    names and no PI math is put into ST as an analysis substitute. Focused
    runtime and rendered actual-ST red/green evidence is recorded in the
    Control Studio ledger; cancellation/close/reopen persistence, PD/PID,
    broader models and package acceptance remain open.
  `scripts/render_diagrams.sh` and `check_diagram_drift.py` passed on
  `trust-builder` for this revision. `cargo xtask architecture-doctor
  --changed` generated its map but failed only the pre-existing runtime
  initializer-service funnel findings in
  `crates/trust-runtime/src/host/harness/coerce_contract_tests.rs`; that
  unrelated failure is not used as PI Auto acceptance evidence.
  - Three-page ownership correction (2026-09-13): `control-studio.puml` now
    separates the Identify → Tune → Validate webview pages, exact scoped
    binding discovery/selection, model prediction, configured-ST validation,
    and runtime Apply owners, including declared PID groups and compiled member
    compatibility. Remote regeneration and drift verification passed in
    `three-page-final-diagrams.log`. Rendered ownership checks and native
    single/group PID execution are recorded in the three-page checklist;
    combined extension verification passes (1,482 tests and 30 rendered cases).
    Repository `test-all` is blocked by builder disk capacity; user UI review
    remains pending and work is paused at that checkpoint.
- [ ] `ARCH-CSTUDIO-02` Keep experiment safety and controller apply as separate
  authority/state-machine owners; Automatic Tune only orchestrates the same
  Test, Identify, Tune, Validate, artifact, and apply services used manually.
- [x] `ARCH-MQTT-03` Keep MQTT direction selection, enum process-image identity, and snapshot metadata in separate bounded owners: configuration presence selects raw versus mapped mode, `IoBindingCodec` owns enum conversion metadata, and `IoSnapshotEntry` carries the declared enum name to control serialization without teaching the MQTT worker about runtime types. The strict PR report delegates direct spec-and-native-test companion checks to a dedicated validator while planner and catalog metadata remain advisory. Evidence: focused mapping/snapshot/verification red-green tests and refreshed `runtime-execution.puml` plus `release-evidence-flow.puml`.
- [x] `ARCH-MQTT-04` Keep real-broker interoperability proof in a dedicated executable script owned by the runtime communications conformance gate: the shipped traffic-light example runs against Mosquitto and `mosquitto_sub`, while unit transport doubles remain focused deterministic tests. The direct companion validator covers executable workflow, crate, VS Code, script, xtask, and root build-control paths without treating path presence as semantic proof. Evidence: focused gate-contract red-green tests, the live Mosquitto PASS artifact, and refreshed `release-evidence-flow.puml`.
- [x] `ARCH-MQTT-05` Give MQTT one authoritative protocol specification instead of extending the runtime-engine catch-all: `32-mqtt-io.md` owns broker configuration, mappings, topics, payloads, security, sessions, discovery, and interoperability; `11-runtime-engine.md` retains only atomic tag lowering, process-image binding, bounded worker handoff, and fault projection. The direct-change validator now rejects an MQTT implementation change paired only with the generic runtime-engine spec. Evidence: `specification-architecture-audit.md`, `communication-solid-spec-ownership-audit.md`, and the focused expected-red/green owning-spec governance test.
- [x] `ARCH-MQTT-06` Keep symbolic MQTT tag resolution in the shared runtime-owned resolver and invoke it from both project assembly boundaries before driver construction. The VS Code simulator now lowers project `io.toml` mappings after compiling program metadata, matching `trust-runtime` without moving symbol knowledge into the MQTT worker. Evidence: Issue #119 focused `trust-debug` assertion red/green and refreshed `runtime-execution.puml`.
- [ ] `ARCH-SPEC-01` Execute the staged specification-architecture split for the remaining mixed authorities in `10-runtime-semantics.md`, `11-runtime-engine.md`, `14-lsp.md`, `22-developer-workflows.md`, and `25-vscode-product-contract.md`. Preserve behavior authority and stable references with native owner-route/link tests before moving prose. Evidence owner: `specification-architecture-audit.md`.
- [x] `ARCH-RELEASE-02` Keep post-merge cleanup read-only and classify prunable candidate registrations before filesystem status inspection; capture lifecycle tests allow the owned TERM-to-KILL window to finish without changing launcher ownership. Evidence: focused assertion-red/green tests and the refreshed `release-evidence-flow.puml`.
 - [x] Extract the UTF-16 incremental document-change applicator into the
   reusable `trust-lsp::document_text` library module so the shipped handler,
   focused tests, and fuzz target share one implementation. The binary handler
   remains responsible for LSP type adaptation and document-state updates.
- [x] `ARCH-DEBUG-01` Keep cyclic breakpoint-stop suppression owned by
  `DebugControl`: `Runtime::execute_cycle` opens the scan boundary,
  `DebugState` retains exact breakpoint identities already stopped in that
  scan, and `matches_breakpoint` suppresses only repeat stops for the same
  identity while re-arming it on the next scan. Evidence: the focused
  identity-scope unit regression and the six debugger lifecycle integration
  tests pass on `trust-builder`; `debug-architecture.puml` and
  `debug-step-sequence.puml` record the execution flow.

### Lesson 1 compiled-binding source correction (2026-09-06)

- [x] User authorized the shared LSP loader fix after rendered plant connections failed.
- [x] Specification 38 requires runtime-ignored lesson tests to stay out of the compiled binding catalog.
- [x] Native assertion red, minimal loader correction, paired green and real plant-editor connection verification.
- [x] Diagram output/manifest and drift check; focused binding/HMI-init checks pass. Course-wide integration gates remain scheduled at final integration.
- SOLID/KISS/DRY: source eligibility stays in load_hmi_sources, shared by HMI setup and binding discovery; no UI workaround, schema change or special-cased lesson identity. Preserve compile errors from nonignored sources.

## Generic control parameter transactions (G08)

- [x] Keep control protocol parsing in `control/parameter_handlers.rs` and typed
  transaction ownership in `host/debug/control/parameters.rs`.
- [x] Runtime task composition calls one guarded atomic commit before execution
  and observes the complete set afterward; no per-parameter write queue reuse.
- [x] Driver provenance defaults to physical/unknown; only explicit in-process
  simulator drivers waive physical review.
- [ ] Full runtime vertical and broad milestone gates remain tracked by the
  general Control Studio implementation checklist.
- [x] `ARCH-CSTUDIO-MANUAL-CLOSE-01` Keep a closing Manual apply/rollback
  separate from the disposed engineer session: retain only operation and
  program/binding identity with its terminal native outcome, then let the
  reopened session query current runtime status before it reports Applied or
  Rejected. Do not retain a client or review token, and block a changed/missing
  identity until a fresh review. Evidence: focused Extension Host red/green plus
  real-ST dark/light/high-contrast close/reopen journeys in
  `control-manual-rollback-close-*-settled` evidence. The changed-program branch
  now has real ST source edit → runtime restart → fresh revised status evidence in
  `control-manual-rollback-program-change-{light,dark,contrast}`; it remains
  unresolved with no replay or reused confirmation. The opaque runtime
  `program_revision` already includes its per-debug-session identity, and an
  unchanged-source actual debug-session restart is covered by
  `control-manual-rollback-debug-session-{light,dark,contrast}`. Process/host
  restart and controller-family recovery remain open.
  - Extension Host lifetime correction (2026-09-08): the Manual owner must be
    written through workspace state *before* the native transaction dispatches,
    because a close-time asynchronous write dies with a reloaded host. The owner
    contains only exact loop scope, operation and runtime identity; it has no
    client, review, token, sample or terminal value. A retryable unavailable
    status keeps it durable and exposes one fresh-status Retry recovery action;
    only Applied or Rejected clears it. The normal managed-runtime dark/light/
    high-contrast journey holds one real rollback reply across Developer Reload
    Window, then reads actual `Main.Kp=1` with no replay or old Confirm. Evidence:
    `control-managed-manual-{normal,light,high-contrast}-recovery` under the
    Control Studio workflow resume directory. Runtime-loss recovery remains open.

## Finite actual-ST search (G07)

- [x] Separate protocol composition, bounded contracts, source snapshot, process
  supervision, OS resource limits and actual runtime execution into focused modules.
- [x] Reuse compiled symbolic resolution and SimulationController pre/post-cycle
  execution; do not introduce a substitute PID simulation for arbitrary ST.
- [x] One disposable process per trial resets controller/plant state; supervisor
  bounds concurrency, wall time and retained result payload.
- [ ] Add equivalent native process-memory-limit adapters beyond Linux.

### Focused Routh–Hurwitz analysis (2026-09-07)

- [x] Keep declared polynomial/table/interval mathematics in a pure module; a scoped session owns selected values.
- [x] Reuse Tune, declared loop models and existing parameter application; no runtime analysis math or new execution service.
- [x] Focused rendered/native Routh lesson workflow and remote diagram generation/drift check passed; see control-studio-routh evidence. Final repository-wide gates remain pending.

- [ ] Control Studio workflow-1 guided layout: shared theme resource, existing manual workspace disclosure and next-stage actions are wired. Whole-screen/keyboard/recovery review and diagram regeneration remain pending. Integrating P formulas have a separate calculation owner; full connected native acceptance remains pending.

- [ ] Control Studio failed comparison ownership: automatic selection emits typed rejection evidence, engineer session owns study retention, and comparison presentation renders reasons without application authority. Focused and host-state tests pass; rendered/reopen/package and diagram regeneration remain pending.

- [ ] Control Studio DOM ownership corrected: navigation and comparison rendering live under webview; DOM-backed tests import them explicitly. Combined remote host/webview type checks and 46 targeted tests pass. Rendered/package acceptance and generated diagram refresh remain open.


### Connected Control Studio workflow extension

- [x] Runtime parameter and transfer bindings share one `ValueRef` path owner
  across nested FB instances and STRUCT scalar leaves; focused red/green
  evidence is recorded in `/home/johannes/projects/.artifacts/control-platform-release-20260911/nested-bindings.md`.
  Full connected workflow and whole-platform acceptance remain open.

- [ ] Finish integration through existing numerical, runtime, recording and
  commissioning owners. Relay transitions and transfer-function realization
  now share implementations; the overall SOLID/KISS/DRY review remains open.
- [x] Connect ordered actuator configuration to the native plant parser.
- [ ] Native execution and rendered workflow acceptance for this extension.
- Tests are excluded by the user's explicit instruction for this implementation.

- [x] Control Studio navigation: isolate persisted view selection from workflow data and runtime authority; reuse the same stage actions and shared chart.

- [x] Control Studio visual hierarchy: isolate notice presentation from action authority; retain one status node and one live-status footer, using shared theme tokens.
- [x] Control Studio plot and tuning boundaries: selected-plot routing and scoped native export bridge retain one host write owner; Manual/Imported input validation shares the PID record contract with the host and preserves SIMC/AMIGO dispatch. Independent fit adequacy checks the final response before choosing by complexity. Focused regression and unchanged-default rendered journey evidence are recorded in the guided controls audit.

- [ ] Control Studio complete view review (exhaustive interaction audit remains open): separate stage inspector, scenario editor, evidence presenter and simulator-start routing; preserve one instance of each bound control and operation authority. Current execution evidence is tracked in `control-studio-complete-view-review.md`.

- [x] Control Studio comparison/recording ownership: the host attaches model/baseline comparison context to predicted plots; cached pin selection cannot issue tuning writes; persistent loop scope stays separate from editable-history context. Commissioning projection provides explicit SP/PV/MV roles without inventing verification requirements. Native tuned-prediction interactions and recording-role regression evidence are retained in the current UI audit.

- [x] Control Studio opening-state restoration yields to a newer stage selection;
  unavailable chart tracks stay within their own layout area. Existing navigation
  and chart owners implement the changes without duplicate host state. Expected
  reds: `navigation-late-restore-red.log`, `empty-plot-overlap-red.log`; green:
  `final-all-control-studio-current.log`, `empty-plot-overlap-green.log`, and
  `final-required-goals-green.log`. SOLID/KISS/DRY: no new operation owner or
  parallel workflow state store. Diagram: `architecture/control-studio.puml`.

- [x] Primary actual Control.PID commissioning reuses declared-state transactions, compiled nested ValueRef bindings and isolated actual-ST execution. Seven retained states, parameter records, first output and monitoring are observed in the real PI journey. Selected general-model fit predictions remain owned by existing fitters and carry independent recording identity into shared chart presentation. SOLID/KISS/DRY: no second controller, simulator, numerical fitter or runtime write owner.
- [x] Promote the reactor capstone's existing GetState observation to the shared Control.PID library, with native value-copy/nonmutation proof. P/PI heater lessons delegate output to the shared block; diagnostics read its state. The canonical original update equations remain unchanged. Diagram sources rendered and manifest drift checked on trust-builder on11September2026; evidence under control-platform-release-20260911. Whole course/production simulation adaptation remains open.
- [x] Current release repair diagram refresh: reuse the existing input-history/RK4 owners for independently delayed state-space inputs and their bounded rollback history; reuse the workflow input-edit barrier and chart router for ordered actions and recording-only result inspection. Native delay/history tests, actual heater I/O and focused routing tests pass. Diagram rendering and manifest drift verification passed on trust-builder; current-diagram-render.log and current-diagram-drift.log retain the results. SOLID/KISS/DRY: no browser simulator, second recording owner or duplicate controller state.

- [x] Historical Control Studio Signals: share the opened recording source with plot/value strip; use matching live metadata only for labels. Four expected DOM reds and the actual heater save/open red pass after repair; full native/UI lessons7/8 pass with all recorded rows checked and both reopened Signals images personally and independently reviewed. Evidence: control-platform-release-20260911/historical-signals-*.log and historical-signals-independent-review.md. Diagram render/drift passed on trust-builder on12September2026. SOLID/KISS/DRY: one signal-row projection, no recorder or transport duplication.
- [x] Active Test presentation: the existing experiment identity controls the single shared-header Abort action, Test progress and manual recording controls. Chart observations reuse recordingControls projection; late manual notices cannot take ownership. Expected native/DOM reds and restored actual-MV proof pass; current605 focused Studio tests pass. Diagram render/drift refreshed on trust-builder (test-recording-owner-diagrams.log). SOLID/KISS/DRY: no new host/runtime state owner or duplicate write handler.

- [x] Control Studio view-aware chart controls and accessible Samples reuse chartWorkspace/liveChart. Existing retained readings supply selected values/units/quality; native diagnostic force/release and current screenshots pass. No new recording/runtime owner. Diagram note updated; render/drift and current focused milestone recorded in samples-toolbar-diagrams.log and samples-toolbar-regression.log. SOLID/KISS/DRY: one visibility projection, one table toggle/projection, shared theme.

- [x] Native Control Studio setup separates per-session answer history from VS Code prompt presentation and the existing loop/controller/channel construction owners. Back rebuilds the in-memory draft using cached discovery, validates changed choices, retains compatible adapter selection and preserves one final exclusive save. Expected native Back/input-race reds,20focused setup behavior locks, current611focusedStudio tests and65.178s complete actual Control.PID native journey pass.43currentimages have personal/independent review; event-marker/list-label polish remains separate. SOLID/KISS/DRY: no new webview, runtime transport, controller or parameter-write owner. Diagram render/drift: guided-navigation-diagrams.log.

- [x] Control Studio event/readiness projection: retain existing liveChart and engineer navigation owners; use actual chart coordinates for events, with zoom/resize clipping, and align review labels with the existing action state. Expected rendered/DOM/native reds then greens; 613 focused cases pass, 42 opt-in unexecuted, and all 13 native PID images personally/independently reviewed (`control-platform-release-20260911/event-markers-*`). SOLID/KISS/DRY: no new state, acquisition, controller or runtime-write owner.

- [x] Lesson9 heater ownership: existing Control.PID owns output and histories; existing native state-space plant combines delayed heating and immediate extraction into one measured temperature input. No ST plant sum or duplicate PI state remains. Two direct expected native reds then all10 foundation cases green; original target/load response criteria preserved. Diagram source: control-pid-library.puml. Public lesson UI/image acceptance remains pending.

- [x] Control Studio runtime-session recovery and shared signal selection: reuse one ordered native watch across live observation, recording and identification baseline; retain original recorder/recovery/write owners. Confirmed session identity resets only live plot time/samples and scan freshness, preserving native records. SOLID/KISS/DRY: one binding builder, existing transport and chart adapter, no duplicate recording Start path. Focused and native heater recovery evidence is retained under control-platform-release-20260911; 632 focused Studio cases pass (49 opt-in unexecuted), primary actual-PID journey passes50.342s, and all8 recovery/13 primary current images are personally/independently reviewed. Diagram render/drift and lint/compile/registration pass in heater-recovery-final-regression.log and heater-recovery-diagrams-current.log. Whole-platform/release gates remain open.

- [x] Open-loop recording presentation reuses explicit topology, the shared ECharts adapter and signal-row projection. Existing host Test/manual-record messages now carry their retained native samples; plot selection chooses that immutable record or the live window. Saved-result identities and runtime acquisition/write owners remain unchanged. Expected DOM/SVG/native UI reds precede fixes;637 focused Studio cases, the15.171s short open-loop UI journey and23.446s actual-PID recovery pass. All10 open-loop originals have personal and independent review. SOLID/KISS/DRY: no additional recorder, controller or runtime-write path. Diagram render/drift: open-loop-display-diagrams.log. Whole-release gates remain open.

- [x] Recorded channel selection in Validate reuses controlStagePlots, chartWorkspace and the existing retained-sample renderer. Both Response and Signals carry recording source; saved-result ownership stays separate. Paired axis/selected legend projection stays in the shared chart adapter. Actual native Test/Save/Open and navigation pass; diagram render/drift logged in lesson13-diagrams.log. SOLID/KISS/DRY: no new runtime writer, recorder or controller; bounded image evidence in lesson13-self-review.md.

- [x] Experiment-purpose placement reuses the parameter transaction and shared UI; five-second baseline uses existing native timed activation. RuntimeLifecycle owns per-session ordered I/O, leaving consumer freshness/rewind guards in place. Source flow updated in control-studio.puml; builder render/drift pass. SOLID/KISS/DRY: no duplicate plant, recorder, runtime API or write owner. [Native and rendered regression evidence](/home/johannes/projects/.artifacts/control-platform-release-20260911/lesson14-self-review.md).

- [x] Recording import is a separate scoped decoder/selection boundary composing the existing workflow-result, immutable record, qualification and fit owners. Original acquisition provenance survives destination study Save/Open; view-only Monitor navigation resumes the existing live buffer. SOLID/KISS/DRY: no controller, numerical fitter, recorder or runtime-write duplication. Source flow in control-studio.puml is rendered and drift-checked (`lesson15-diagrams.log`); [native and reviewed UI scope](/home/johannes/projects/.artifacts/control-platform-release-20260911/lesson15-self-review.md). Legacy large panel.ts still needs owner extraction before complete architecture acceptance.

- [x] Course Simulator discovery reuses the established runtime-ignore marker; all original test bodies remain in their existing owner. All82 source and packaged projects pass native runtime construction. Test-name filtering keeps discovery/compilation complete and limits only execution/results in the existing CLI/JSON owner. SOLID/KISS/DRY: no compiler relaxation, duplicate controller or test runner. Diagram `st-test-source-selection.puml` rendered/drift checked; [native proof and limits](/home/johannes/projects/.artifacts/control-platform-release-20260911/lesson-source-selection-review.md). MPC timing/fallback qualification remains open.

- [x] Bounded held-step identification keeps numerical window/grid calculation in `stepResponseIdentification`, connects it through `boundedStepReview` to the existing IdentificationWorkflow, and uses shared controls/chart reference lines. SOLID/KISS/DRY: no alternate acquisition, controller, runtime writer or persistence owner. The explicit measurement-consistent marginal ranges are distinct from statistical confidence. Twelve focused cases and native imported preview/refinement pass; lint/compile, 662 focused Studio cases and bounded diagram render/drift pass in `lesson15-step-final-gates.log`. Current 16 images have personal/independent verdicts in `lesson15-step-feedback-self-review.md`; full lesson, native independent acquisition, persistence and remaining UI acceptance are separate open work.
- [x] Finish shared Control Studio session ownership before resuming individual lesson acceptance: atomic identification restoration, selected-model invalidation, durable editable drafts and one navigation/action evidence projection. Reuse the existing numerical, storage, runtime and chart owners; preserve original records and compatible settings. Track focused red/green, native restart, actual Control.PID regression and every image verdict in `control-platform-release-20260911/shared-workflow-refactor.md`. Contradictory restored state, connected invalidation, full draft/restart retention and the rejected-Open boundary passed; see the13September shared-state closure below. Full platform/release acceptance remains open.

- [x] Native identification acquisition retains the original recording ID in the existing evidence envelope and saved-result decoder. The selected native record selects its matching Test mapping regardless of the ledger role; imported estimation can use the next native Test for validation. Current chart/table references use validation measurements while the immutable estimation grid remains separate. SOLID/KISS/DRY: reuse runtimeRecording, Test ledger, IdentificationWorkflow and chart owners; no extra runtime operation or recording store. Expected acquisition/selection/native baseline and rendered-context reds reach green. Native actual-ST lesson15-kGhFBw passes64.811s, 666 focused Studio cases pass,49 opt-in unexecuted. Current image review and remaining full-lesson work: lesson15-progress.md and lesson15-validation-display-self-review.md in control-platform-release-20260911. Diagram render/drift: lesson15-native-validation-final-gates.log, current drift: lesson15-display-final-gates.log.

- [x] Shared Studio draft and model reconciliation milestone: manual command state is isolated from panel routing; the existing workspace writer persists complete drafts and raw field text; GeneralModelSession preserves analysis for equal model content, while changed content invalidates it. Saved input validation precedes cancellation; conflicting supplied models reject before identification adoption. Compile/lint,705 focused Studio tests, actual Control.PID56.762s, native draft/import/reopen and supplied Save/Open/Save pass. Diagram/source evidence and per-image review are in control-platform-release-20260911/EXECUTION.md. SOLID/KISS/DRY: no new acquisition, fitting, storage-key or runtime-write owner. Final rejected-Open metadata/export preservation passes native red/green (3.006s green); metadata commits after successful activation. Shared-state refactor checklist is closed; full release gates remain open.

- [x] Lesson15 adverse held-step calculation stays in the existing numerical owner: signed sustained onset, explicit missing-step error and complete baseline/held-window coverage. Native numerical/workflow23cases and real Identify rejection/edit/fit pass;713 Studio regression cases pass. The ST arithmetic helper retains its scalar API and uses standard ENO for zero-step rejection (3native cases). SOLID/KISS/DRY: no new controller, acquisition, persistence or runtime-write owner; diagram render/drift and226/226registration pass. Evidence: control-platform-release-20260911/lesson15-progress.md; full UI/course/release gates remain open.
- [x] Held-step field correction reuses the same pure numerical settings validator in host and webview; one field/reason result drives named inline errors and focus. Existing stage projection handles rejection/recovery and existing recording controls point opened evidence to Validate without replacing pending/Test status. SOLID/KISS/DRY: no second validation policy, controller, acquisition, persistence or runtime-write owner. All 16 native field/keyboard cases and five clarity/Monitor image reviews pass; see lesson15-fields-images/self-review.md and lesson15-progress.md in control-platform-release-20260911. The layout/chart matrix is now closed by the following row; broader release gates stay open.
- [x] Resolve the remaining held-step responsive/inspection defects in the existing shared styles and chart action owner. Short viewports use one scrollable page with reachable stage navigation; narrow plots preserve reference spacing; window changes clear the old inspection. Expected native layout/tooltip reds pass in the 29.556 s corrected journey; all 46 current images personally and independently reviewed. SOLID/KISS/DRY: no new chart, numerical, acquisition or session owner. See lesson15-responsive-final-images/self-review.md in control-platform-release-20260911; wider lesson/media/release gates remain open.
- [x] The existing copiedLessonReadme adapter resolves descriptive as well as numbered lesson links through the catalogue, keeps authored labels, and preserves external/unresolved links and bundled originals. The real copied-project assertion failed before the repair and passes after it; native Markdown preview passes 1.091 s. SOLID/KISS/DRY: reuse the copy/landing flow and existing metadata sequence; no second course catalogue or project-copy path. The three old 84-lesson/PID-case expectations were reconciled with the approved 82-lesson source map.

- [x] Treat native test POUs as test-runner entry points within the existing unused-symbol collector, reusing scoped pou_context identity. Preserve ordinary production and test-local unused warnings. SOLID/KISS/DRY: no second lint policy in the webview/test runner and no controller/session change. Expected native W009 red reaches 16-case green; rebuilt language server and five ST workflow cases pass; final Lesson15 learner editor has no diagnostics. Updated hir-semantics diagram and manifest; see test-pou-unused-*.log and lesson15-calculations-final/self-review.md in control-platform-release-20260911.

- [x] Lesson16 uses the existing identification/model, worker, recording and ordered-draft owners for comparison and Save/Open. No second recorder, simulator, session store or controller was introduced. Native plant/timing and numerical evidence is retained; the actual published procedure, persistent restart and runnable ST exercise pass. The bounded UI projection uses existing chart and presentation components.759focusedStudio cases pass. The user forbids expanding specifications or adding product features for individual lessons; continue by adapting Lesson17 instructions to existing controls. Detailed evidence and9/10substeps (only user acceptance pending): `control-platform-release-20260911/lesson16-progress.md`.

- [x] Control Studio course capabilities retain the existing three-page host/session, recording, controller mapping and actual-ST owners. Exact Routh uses bounded rational workers; finite LQR preserves its explicit fixed runtime policy. MIMO command handling and periodic recording validation are separate modules. Diagram source/render/drift updated; focused/native/real-editor evidence and SOLID/KISS/DRY checks are recorded in `control-studio-course-capabilities.md`. Release-wide gates remain separate.

- [x] Identify preview repair (14 September 2026): reuse the existing loop monitor envelope and shared response chart. `ThreePageLiveSignals` owns only bounded display projection; the shared checkbox dropdown owns pointer/keyboard selection. No additional transport or runtime owner. See `control-studio-identify-preview-repair.md`.

- [x] Isolated scan preparation (14 September 2026): the existing worker compiler/bytecode loader prepares the executable before scan deadlines are armed. Total child/job supervision still bounds preparation; controller state is not advanced by warm-up scans. Local PID regression evidence is recorded under `control-studio-identify-preview-repair.md`.

## Control Studio model review correction — 14 September 2026

- [x] Reuse AutomaticStudioFlow to end live/recorded identification on Identify; only an explicit Tune request continues to Validate.
- [x] Reuse ThreePageOperation, shared buttons and persisted navigation for Identify again / Continue to Tune. No new session, acquisition or runtime transport.
- [x] Focused host/native rendered red-green (14 passing), actual preview mouse interactions, 18 reviewed screenshots and updated diagram evidence: see identify-review/task.md under the control-pid-preview-20260914 artifact directory.

## Identify action simplification — 14 September 2026

- [x] Remove duplicate Continue to Tune navigation; keep stable Identify plant label.
- [x] Existing chart owns Reset chart as a display-only action, with no host operation or study deletion.
- [x] Three focused rendered/native checks pass; actual preview reset/re-identification/tab navigation and eight image reviews recorded in control-pid-preview-20260914/identify-action/task.md. Diagram regenerated and drift checked.

## Identification experiment display — 14 September 2026

- [x] Reuse validated trial receipts to display applied input and measured outputs before fitting. Publish each completed job once while retaining receipts through the original owner.
- [x] Retain the matching experiment in existing model comparisons; reject malformed stored display coordinates and preserve selected recording channels.
- [x] Existing chart renders aligned input/output time axes, reset stays display-only, no new runtime transport or simulator.
- [x] Focused expected-red/green, five builder checks, local ARM native check and ten reviewed images recorded in control-pid-preview-20260914/experiment-chart/task.md; PlantUML regenerated and drift checked.

- [x] Control Studio Reset: one scoped `StudioStudyReset` owner composes existing model/identification/validation owners; duplicate and busy/recovery guards precede clearing; panel persists the fresh draft before webview refresh. Fresh live acquisition policy stays in `AutomaticStudioFlow`. Focused continuity/reset evidence tracked separately; no runtime/controller algorithms replaced.

## Connected live identification correction — 2026-09-14

- [x] Keep live acquisition in its own owner; reuse native experiment guards, runtime scan recording, controller-transfer transactions and existing independent-data fitters.
- [x] Keep connected acquisition provenance separate from isolated verification; admit it only in identification-campaign persistence.
- [x] Keep chart projection on the existing monitor stream and preserve the selected recorded model comparison.
- [x] Update Control Studio PlantUML and regenerate its diagram/manifest on trust-builder; drift checks passed.

[Bounded native and screenshot evidence](/home/johannes/projects/.artifacts/control-pid-preview-20260914/live-identification/commands.md). Release-level and physical qualification gates remain open.

## Control Studio qualification boundary — 2026-09-14

- [x] Reuse connected acquisition for one Identify recording and a separate
  independent recording requested by Validate; keep frozen prediction and
  qualification in dedicated modules. Preserve the existing controller, tuning,
  scenario, cancellation and coherent application owners.
- [x] SOLID/KISS/DRY review: the workflow session delegates model checking and
  recovery; the existing step fitter and model conversion serve provisional
  fitting; no parallel runtime or controller implementation was introduced.
- [x] Regenerated Control Studio architecture diagram and verified manifest drift.

Evidence: [active task](control-studio-single-identification.md).

## Automatic tuning setup — 2026-09-14

- [x] Keep model-derived criteria in their own pure policy/evidence modules;
  reuse existing model analysis, controller tuning and the workflow study.
- [x] Verify diagram and source/manifest agreement after rendering.

[Active task](control-studio-automatic-tuning-setup.md).

## Bytecode VM execution cost — 2026-09-28/29

- [x] Root cause 1: `dispatch.rs` resolved a debug source location for every executed statement even without debug control, and with debug control copied the variable storage before every statement. The statement location is now resolved only with debug control and cached per (POU, pc) in `Runtime::vm_statement_locations` (cleared on every module change); the debug snapshot is taken at a stop and once per completed cycle (`cycle.rs` step 9a). Behaviour locks: `debug_snapshot_timing.rs`, register-IR location test; spec 13 updated.
- [x] Remaining costs (callgrind): lazy `VmTrap` construction; dense `ref_types` and a `ref_slots` (owner, offset) index in `dynamic_ref_type` instead of a scan of every reference; store fast path for values that already have the declared elementary type (`type_policy.rs`); same-type arithmetic fast path in `apply_binary` (`same_type_binary`); `declared_instance_field_offset` reads the instance's own map instead of a string-keyed type-layout cache (cache removed); `VmModule` and local-init plan maps moved to Fx hashing. Behaviour locks added before each change and green before and after (`same_type_operands_keep_checked_results_errors_and_float_comparisons`, `primitive_policy_keeps_values_of_the_declared_elementary_type_unchanged`) plus core/runtime lib, differential, complete_program, FB-array, debug and lesson suites.
- [x] SOLID/KISS/DRY review: each change stays in its owning module (dispatch, type policy, core ops, memory); no new module boundaries; the fast paths return `None` and defer to the unchanged generic path, so there is one semantic owner per operation.
- [x] Diagram `docs/diagrams/architecture/runtime-execution.puml` updated for the debug-hook gating and cycle snapshot; render and drift check on trust-builder recorded with the final gates.

Release measurements: Lesson 79 scan 2.8 → 1.8 → 0.79 ms; `X := X + 1` loop iteration 1.39 → 0.6 → 0.44 µs; function-block call 2.39 → 1.64 → 1.24 µs (debug control 7.93 → 2.18 → 1.81 µs).

## Control Studio setup, Apply and program-law prediction — 2026-09-29/30

- [x] New setup owner: `controlStudio/loopSetup/*` (compiled catalog decoding, form model, one generator per structure: PID, cascade,
  PID group, ON/OFF, state feedback, identify only) and the setup webview (`webview/loopSetupForm.ts`); the language server adds
  `controlSymbols.links`/`writes` and each configured program instance's declared type.
- [x] Apply: `commissioningStartingPoint.ts` owns the settled-start rule, the bounded cancellable wait and the state-feedback
  Auto-entry allowance; `runtimeDuration.ts` owns observation in runtime time from `control.transfer.status` `runtime_time_ns`.
- [x] Tune/Validate share `modelPredictionRuns.ts` (the program's ST with current and candidate settings on the model).
- [x] Removed the built-in physical families (`physicalModel.ts`, `physicalIdentification.ts`) and their branches.
- [x] SOLID/KISS review: new behaviour went into new modules with explicit owner interfaces; `engineerWorkflowSession.ts` still grew
  (997 → 1,085 lines) and its 537-line `handle()` needs a per-action split (tracker CPGP-217).
- [x] Diagram `docs/diagrams/architecture/control-studio.puml` updated (setup form and generators, program-law prediction, Apply
  wait and runtime-time observation); rendered with `scripts/render_diagrams.sh` and `scripts/check_diagram_drift.py` clean on
  trust-builder (30 Sep).
- [x] Automatic tuning: `automaticPidResponse.ts` owns the requirement-driven response choice for PI/PD/PID (Spec 35 §9.2); it
  reuses `designPidForTask` per response and `engineerPrediction`, and shares `responseDeadlineSeconds` with the verification plan.
- [x] Session split (tracker CPGP-217): `engineerSessionState.ts` holds the session's working state behind one explicit interface
  (`EngineerSessionState`); `engineerActions/*` own one action each: `find.ts` (Identify), `tune.ts`, `verify.ts` (Validate),
  `application.ts` (Apply review and confirmation), `recovery.ts` (runtime review, retirement and interrupted rollback). They
  reach the session only through `EngineerActionHost` (`host.ts`). `engineerWorkflowSession.ts` keeps request guards, input
  invalidation, persistence and recovery: 1,088 → 654 lines. Behaviour lock: full VS Code suite green before the split (both
  phases, 30 Sep) and the session-level suites green after it; new session-level locks for Validate cancellation confirmation
  and the Manual rollback ticket were green on the code before the split.
- [x] Structure designs share one path module (30 Sep): `pathReduction.ts` owns the settling horizon, the open-loop step
  response, the two-point first-order-plus-delay reduction, the model sampling, the single-loop view of one controller and the
  first-order path model. The PID-group design (`decentralizedPidDesign.ts`), the cascade design (`cascadePidDesign.ts`) and the
  feedforward design (`feedforwardPidDesign.ts`) use it instead of three copies. Behaviour lock: PID-group and cascade suites green
  before and after. Measured disturbances are model inputs the controller only reads: `measuredDisturbanceInput` (loopChannels)
  keeps them out of the controller outputs, the verification output bounds and the transfer's written states, and
  `engineerDisturbanceScenarios.ts` owns their verification case.

## Legacy HMI removal and HMI Builder foundation — 2026-10-01

- [x] Removed the legacy HMI (spec 39, decision 12): the `trust-runtime` `hmi` internals (descriptor,
  scaffold, runtime views, points, layout, catalog, persistence), the `/hmi` routes, assets, export
  and `/ws/hmi` socket, the `hmi.*` control operations and handlers, the `ControlState` HMI fields
  and descriptor watcher, the `hmi` CLI command and the scaffold `run` wrote into projects,
  `[runtime.hmi_persistence]`, `trust-lsp.hmiInit` and the HMI diagnostics, the `trust-debug` HMI
  wiring, and the VS Code HMI panel, language-model tools and launcher. `notify` and
  `tungstenite` are no longer dependencies. Removal tests went red first and are green.
- [x] HMI Builder foundation as new single-responsibility modules under
  `crates/trust-runtime/src/hmi/`: `page_files` (format 1 loader, canonical writer, JSON shape),
  `member_paths` (one walker for bound paths), `variable_tree` (picker tree), `catalogue`
  (`components.json`, shared with the editor) and `binding_checks`; in `trust-lsp`,
  `commands/hmi_files.rs` (parse, format) and `commands/hmi_checks.rs` (checks), which share
  `compile_project` with `trust-lsp.hmiBindings`. Largest file 416 lines (`page_files/load.rs`).
  Network Canvas reads the `hmiBindings` process image directly instead of an HMI normalizer.
- [x] Diagrams: `compiled-binding-sources`, `runtime-execution`, `system-architecture`,
  `runtime-bytecode-vm-execution` and `runtime-ads-client` edited; the software-map generator
  updated (HMI Builder node depending on the runtime core, `Hmi` command gone) and the same three
  lines changed in `full-software-map-generated.puml` (its counts were already stale at HEAD; a
  full regeneration needs the static line-count artifacts). Rendered with
  `scripts/render_diagrams.sh` on trust-builder, manifest refreshed,
  `scripts/check_diagram_drift.py` clean (1 Oct).
- [x] Runtime display (spec 39 §11.1), new single-responsibility modules: `hmi/display_values`
  (value encoding for the display, members in declared order; `read` runs on the resource thread
  through `ResourceCommand::HmiRead`, so no storage copy and no shared lock), and
  `web/hmi_display` split into routes and access (`hmi_display.rs`), project files read on open
  (`files.rs`) and the write-only value stream (`stream.rs`; the page names its own paths). The
  browser side is TypeScript beside the drawing module: `hmiBuilder/display/liveValues.ts`
  (state, staleness, trend samples; no DOM, so Live mode can reuse it), `runtimeDisplay.ts`
  (page loading and redraw with injected browser services) and a thin `webview/` entry; both
  bundles are committed into the runtime with staleness tests. The debug adapter's bridge answers
  the same read. `tungstenite` is a dependency again, for the stream. Largest new file 302 lines
  (`display_values/mod.rs`). Diagram `hmi-runtime-display` added, rendered with
  `scripts/render_diagrams.sh` on trust-builder, manifest refreshed, drift check clean (1 Oct).
- [x] **Communication link states for the program (specification 42, 3 October 2026).** A new
  `runtime/link_subsystem.rs` (`LinkSubsystem`, composed into `Runtime` beside the ADS and OPC UA
  client subsystems) maps driver health and the ADS and OPC UA client reports to one link state
  and writes every `Comm.Link` block after the inputs are read; the runtime cycle gains one call.
  The instance walk the alarm service used moved, unchanged, to `hmi/instance_walk.rs`, shared by
  both (alarm suites green before and after); `driver_to_protocol` moved to `io/link_names.rs`,
  shared with `comm.schema`. Diagram `runtime-execution` updated, rendered on trust-builder,
  manifest refreshed, drift check clean.

- [x] **Named HMI operation in the editor (spec 39 §11, 4 October).** Shared runtime
  `startDisplay`/Operation/SessionControl reused through `LiveDisplayHost`; `LiveOperator`
  owns the saved/deployed gate, `LiveSession` owns a pinned in-memory cookie, and `liveNetwork`
  owns HTTP/WebSocket transport. App notices/value helpers extracted under 113 green locks.
  Diagram `hmi-editor-live` rendered on trust-builder; manifest refreshed and drift check clean.
  Focused authority tests and 608 HMI extension tests pass; runtime/browser milestone tracked
  in the HMI implementation checklist.

- [x] **Shared HMI presentation defaults (spec 39 §5.9, 4 October).** Native
  `hmi::presentation` owns stable per-path format collection. Runtime page/popup responses and
  the language-server project catalogue reuse it; the editor passes unsaved text overrides.
  The shared drawing applies defaults to copies, preserving authoring and command constraints.
  `hmi-runtime-display` diagram regenerated on trust-builder; drift check clean. The same native module supplies live equipment metadata in the value snapshot; metadata-only
  changes participate in stream deltas. Diagram regenerated and drift checked on the builder.

- [ ] **HMI cross-runtime values (spec 39 §7.9, 4 October).** First slice reuses the existing
  member-path resolver for configured mesh publish paths; no second member walker. Native
  expected-red/green snapshot proof is recorded in the HMI checklist. Typed discovery, HMI
  subscription/freshness and authenticated owner-policy writes remain open. Cloud-planes
  diagram updated; regeneration follows this slice.


### HMI authoring completion batch — 4 October 2026

- Implemented, final validation pending: metadata/selection provenance, page/draft inspection,
  native dirty faceplate overlays and pairs, region constraints, multi-page draft commit,
  viewport images, synthetic scenarios, qualitative review records, inherited display formats,
  native incomplete-page deployment refusal, separate PLC proposals, and configured Control Studio
  handoff. Transport, queries, geometry, snapshots, preview and source application remain separate.
- Synthetic exploration reuses the shipped runtime display with snapshot-only fetch/socket owners;
  writes and login are refused without accessing runtime transport. No new model provider.
- SOLID/KISS/DRY inspection: new responsibilities are separate small modules; shared native
  parser, display renderer and Control Studio configuration resolver remain authoritative.
  App.tsx remains below 1,000 lines. Diagram source updated; regeneration is part of the one final batch.
- Tests were authored/registered but not executed during implementation, per the user's order.

- Cross-runtime HMI: qualified paths are parsed by the shared member-path owner; remote data
  cache and named owner sessions are separate from mesh transport and web presentation. Network
  reads run outside the PLC thread; native writes and pulse endings remain at Hmi.Display.
  Owner login uses existing SessionStore, type checks, policy and audit over authenticated TLS;
  neither metadata nor a local user role grants owner-write authority. Native two-peer TLS and
  owner-domain regressions are authored but not yet executed.
- Named simulator journeys reuse the existing LiveSession/selected-target owners with explicit
  local-simulation filtering, deployed-page matching and bounded outcomes. No fallback or new
  provider client. Final screenshots and consolidated test evidence remain pending.

- Final batch 4 October 18:35 UTC: HMI authoring diagram rendered and drift check passed.
  Rust/TypeScript compilation failed; source corrections are prepared but unverified. No second
  validation run. See the HMI checklist's final-batch ledger; integration/acceptance remains open.

- 5 October repair: request existing mesh liveliness tokens and exclude the local runtime from
  peer discovery. Isolate ordinary value-card scheduling in `ValuePaintQueue`, preserving atomic
  value/unit cards and immediate safety-state/control painting. Updated performance probe measures
  complete value frames rather than first mutation. Diagram source updated; one final regeneration
  and validation are pending. These local repairs are not the previously tested remote snapshot.

- 5 October functional repair: preview faceplate/alarm provenance reuses `symbolInstance`;
  private configured-runtime discovery advances to the first due periodic scan. Snapshot
  transport emits separate value/alarm frames, and CSS containment preserves the proposal
  shell. The real VS Code navigation/faceplate/alarm journey passes. `ValuePaintQueue` bounds
  ordinary updates at 96 cards, begins in the drawing turn and retains DOM text nodes; fresh
  stream updates suppress redundant timer redraws while stale/quiet pages keep their timers.
  SOLID/KISS/DRY: no new parser, binding resolver, runtime endpoint, model provider or write
  authority. Strict physical timing and complete independent/client acceptance remain open.

## Plant library / HMI completion review — 6 October 2026

- Shared ST equipment and managers own behavior; normal I/O remains the hardware/simulation
  boundary. No compiler, VM or scheduler change was introduced in this completion pass.
- Generated settings preserve ST defaults and apply only explicit overrides. Provider metadata
  supplies the inspector defaults; the editor does not infer active settings from zero values.
- Source-exact, bounded LSP metadata caches reuse pristine compilations and immutable binding
  responses. Mutable runtime access disqualifies that instance from reuse. Dirty-source and
  cache-isolation cases passed; the full editor journey remains tracked in PL-27.
- Page and popup share the same SVG patch/paint owner. Real browser ownership, start/stop,
  settings Apply and alarm acknowledgement passed in browser-plant-live-6.
- SOLID/KISS/DRY: no duplicate device kernel, historian, settings store or UI paint engine.
  Plant authoring and display diagrams rendered; final drift check passed.
- Overall acceptance is not closed: see trust-plant-library-checklist.md and its final ledger.


### HMI cold discovery compiler work — 6 October 2026

- [x] Reuse project catalog source ranges for imported declaration lookup; prune executable
  trees from declaration precollection. Existing semantic ownership and invalidation remain.
- [x] First-candidate compiler/editor checks executed; one new whitespace assertion failed.
  Cold inspector still took 114.891 s. See hmi-cold-performance-20261006/RESULTS.md.
- [x] Prepare source-revision constant-location reuse to remove remaining repeated scans.
- [ ] Validate prepared follow-up and regenerate its diagram after explicit rerun approval.


### Cold editor follow-up — 6 October 2026

The declaration/configuration indexes and owner-scoped hazard query passed 1,544 native checks;
actual settings still took 71 seconds. A live stack identified three redundant semantic
projects inside OpenOT preprocessing. The no-attribute fast path and shared authoring model
are prepared with tests/specification; validation and regenerated diagram are pending explicit
next-batch approval. No PLC execution subsystem changed.

## Plant library and HMI Builder rebuild — 7 October 2026

Ownership and data flow changed; diagrams `plant-library-authoring.puml` and
`plant-hmi-runtime-integration.puml` rewritten (rendering, manifest and drift check run in the
validation batch on trust-builder).

- [x] Runtime owns the one parser and writer for page files, panels, component definitions and
  docked-panel composition (`hmi/page_files/{pairs,docked,faceplates}.rs`); the language server
  adapts them thinly (`hmi_authoring/documents.rs`, `pairs.rs`); the deploy projection is gone.
- [x] One pair check (`binding_checks/pairs.rs`) serves `hmi check`, the ST-file diagnostics and
  the editor's pair-file check (through `PairProgram for Declarations`).
- [x] Create PLC code is a new module split by responsibility (`hmi_generate/{mod,regions,
  equipment}.rs`); the inspector (`hmi_equipment.rs`) and Create HMI (`hmi_authoring/propose.rs`)
  reuse its definition lookup and region refresh. The old component-sync and explicit-generation
  modules are retired; single-page "Add missing variables" keeps value variables only.
- [x] Settings values have one owner (`hmi_authoring/equipment_settings.rs`) for the inspector
  and the generator; the design file (`hmi_plant_project.rs`, format 2) holds choices only.
- [x] Retain: instance member paths in the snapshot (`runtime/retain_snapshot.rs`,
  `restart_instances.rs`); inherited initialisers resolved in `host/instance.rs`. No VM or
  scheduler change.
- [x] Plant library: behaviour families own their logic once; alarms and controllers come from
  the Alarm and Control libraries.
- [x] Instance walks share one ownership rule (`hmi/instance_holdings.rs`): interface, REFERENCE,
  POINTER, VAR_IN_OUT and VAR_EXTERNAL members are not followed, so alarm, display and link
  discovery find each instance once, where it is declared.
- [x] One binding direction rule (`binding_checks::check_direction`) for the runtime, `hmi check`
  and the editor; the editor's own copy is gone.
- [x] My equipment is a new language-server module (`hmi_own_equipment/{blocks,definition,
  picture}.rs`) writing through the runtime's pair writer; the host writes its files
  (`editor/ownEquipment.ts`), screen-file edits moved to `editor/screenFile.ts`.
- [x] Library HMI files (`LIBRARY_FOLDERS`: panels, pairs, symbols) are read by one bundle reader
  for the runtime and the editor.
- [x] Cross-file function block initialisers are checked after the project's symbols merge
  (`collector::DeferredInitializer`), not in the per-file collector.
- [x] Validation batch, diagram rendering and drift check (final run 9 on trust-builder: native
  set, Clippy, VS Code, browser, diagrams with no drift).
- [x] PID loops as ordinary code (7 October 2026, evening): Create PLC code's loop rendering is its
  own module (`hmi_generate/loops.rs`); the regions gained a retained region in `regions.rs`; pair
  members beside an instance resolve through one helper (`page_files::pair_member_path`, TS
  `memberPath`) at every join site; the runtime's PID measurement override is its own module
  (`runtime/controller_measurement.rs`, `vm/call/measurement.rs`) with one empty check on the
  method-call path; experiment targets by name are `experiment_handlers/target.rs`. Diagrams:
  `plant-library-authoring.puml`, `simulation-plant-system.puml`.
- [ ] Validation batch for the loops change (pending).

## Simulation time, target fact and Control Studio storage — 8 October 2026

- [x] Simulation time (specification 33, §6.1): each executed cycle advances process time by one cycle interval, paced at the shared `time_scale` (1–100); without simulation process time is the wall clock. `config.set simulation.time_scale` changes a running speed from the next cycle; `status` reports the configured and achieved speed. Diagrams: `runtime-execution.puml`, `simulation-plant-system.puml`.
- [x] Target fact: `Runtime::simulated_target()` (simulation enabled, all I/O simulation-only, no ADS or OPC UA client connection) decides live writes. It feeds the task-boundary parameter and transfer commits and the experiment, transfer, parameter and controller handlers. Diagram: `runtime-execution.puml`.
- [x] Control Studio storage: studies live in the project under `control-studio/<loop id>/`; workspace state holds only operation journals. Identification is a process-time campaign (recording first, steady baseline, held step, early finish for single steps, restoration confirmed). Experiments run in the runtime at the loop task's executions; Apply observations are runtime recordings followed on process time. Diagram: `control-studio.puml`.
- [x] Removed from Control Studio: Manual Record, `validation.ts`, `automaticTune.ts`; `identificationBaseline.ts` and `webview/validationPresentation.ts` are deleted too.
- [ ] Open: the `antiWindup*.ts` modules are still in the worktree, and `panel.ts` still attaches an anti-windup availability field to Test payloads; the comparison itself has no panel entry point. `resumeApplyObservation` has no product caller, so an interrupted Apply is recovered by reviewed recovery, not followed to its end.
- [ ] Validation batch for these diagrams (rendering, `manifest.json`, drift check) pending.
- [x] `ARCH-OPENOT-DB-03` Keep OpenOT database startup validation and failure
  classification at explicit boundaries: the factory validates the selected
  TOML backend's required environment before worker creation, adapters emit a
  distinct reachability error for typed transient connection failures, and the
  service retries only that transport category during open, schema
  initialization, commit, or maintenance. Generic commit/storage and
  deterministic projection errors fault immediately. Evidence: focused
  missing-environment, newer-schema, and deterministic-commit assertion
  red/green tests; a forced real PostgreSQL operation disconnect red/green;
  real restart recovery across PostgreSQL, TimescaleDB, MySQL, MariaDB, SQL
  Server, and InfluxDB 3; the complete pinned real-database gate; refreshed
  `openot-database-persistence.puml`, generated SVG, and manifest; remote
  diagram render and drift check.
- [ ] `ARCH-OPENOT-DB-04` Validate generation 1 from each backend's actual
  catalog instead of object names alone: adapters extract native table/view,
  column, key, check, foreign-key, and index definitions, while the shared
  schema-contract owner provides deterministic ordering and SHA-256 identity.
  Preserve one initial schema with no migration or repair path, and reapply
  connection-local SQLite/Influx spool durability settings on every open.
  Close after focused corruption red-green tests, all real database products,
  a real PLC workload, refreshed diagrams, and exact-candidate release gates.
- [ ] `ARCH-OPENOT-DB-05` Bound every SQL Server persistence statement below
  its effective parameter ceiling without splitting the logical transaction;
  preserve every loss and unresolved typed projection in multi-document
  batches. Close after real 2,101-document and repeated-special-document tests,
  the complete real-product PLC gate, refreshed diagrams, and exact-candidate
  release proof.
- [ ] `ARCH-OPENOT-DB-06` Keep shared-memory source observation independent of
  initial database connectivity: a non-consuming control-snapshot owner keeps
  head and durable-cursor lag visible during retry and shutdown, while the
  consuming source remains owned by the connected persistence worker. Keep the
  exact GitHub Architecture Safety gate and release-candidate preparation on
  one shared script. Close after focused red-green outage proof, the exact
  full-map gate, real-product PLC/database proof, and release verification.
- [ ] `ARCH-OPENOT-DB-07` Freeze bounded shutdown from a fresh non-consuming
  producer-head snapshot instead of an older buffered poll, and keep
  release-runner cleanup strictly scoped to exact-labelled Docker resources
  even when local state exists. Close after focused assertion red-green proof,
  repeated shutdown stress, runner-contract coverage, refreshed diagrams,
  runtime verticals, and exact-candidate release gates.
