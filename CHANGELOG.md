# Changelog

All notable changes to this project will be documented in this file.

The format is based on Keep a Changelog and this project adheres to Semantic Versioning.

## [Unreleased]

Target release: `v0.24.21`

### Added

- trust-twin: robot-cell product bridge now generates `Robot_<Model>` ST
  function blocks from a URDF manifest, exposes a `Robot_P3MinimalArm`
  native FB through the runtime world-arm bridge, and keeps the example PLC
  motion surface on that generated FB instead of direct scene-state writes.
- trust-twin: topology authoring now supports surface/contact placement
  metadata (`attach_to`, named surfaces, mount frames, and bounds), robot-cell
  component kinds, physical-scene doctor fixtures, and LM-tool regression
  coverage for attachment-based robot-cell proposals.
- trust-twin: P6 operator surface parity adds multi-page navigation,
  trend/alarm overlays in the VS Code 3D panel, and runtime-owned HMI
  trend/alarm JSONL persistence with restart-window reload.
- trust-twin: P5 VS Code 3D panel now registers a dedicated webview command,
  reuses the existing HMI JSON-line control transport for schema, values, and
  writes, bundles trust-twin WASM/assets into the extension media payload, and
  adds package-smoke evidence for those assets.
- trust-twin: P4 AI-assisted topology authoring now exposes a
  `trust_twin_topology_propose` typed LM tool that generates reviewable
  `.topology.toml` diffs, validates them through the trust-twin compiler
  dry-run path, and documents the local-first/cloud opt-in privacy boundary.
- trust-twin: P3 operator writes from 3D scene views now compile from
  topology `[[interactions]]` into node interaction descriptors and route
  renderer clicks through the existing `hmi.write` control, RBAC, allowlist,
  and audit path.
- trust-twin: P2 Rapier physics now runs behind `SimulationController` with
  `[physics]` / `[[physics.joints]]` `simulation.toml` parsing, deterministic
  revolute-joint encoder feedback through the existing I/O boundary, duplicate
  target conflict checks, and a P2 gate artifact.
- trust-twin: P1.5 topology authoring compiler and v1 component library landed
  with deterministic `.topology.toml` to `.view.toml` emission, hash drift
  checks, compiler diagnostics, `architecture-doctor` fixtures, and rendered
  topology gate artifact output.
- trust-twin: P1 static 3D proof now builds a parsed `.view.toml` into a
  `scena::Scene`, applies `bind3d` tag values to scene node properties, and
  emits the static-view visual gate artifact.
- trust-twin: P1 `scene3d` page/view descriptors and bounded `bind3d`
  runtime schema contract landed for static 3D view integration.
- trust-twin: P0 IO boundary spike landed with a runtime integration test and
  gate artifact proving Modbus TCP simulator I/O flows through the existing
  `IoDriver` cycle boundary.
- Added fail-closed runtime boundary watch envelopes for `trust-harness` protocol
  v2 and `trust-dev` agent harness snapshots. Unresolved watch paths now carry
  structured `status: "error"` entries instead of silently becoming `NULL`.
- Added a runtime boundary fail-closed architecture gate wired into
  `architecture-doctor --full-map` and CI.
- Added a `trust-dev` developer/workbench CLI binary. The first migrated
  command is `trust-dev commit`, and release runtime archives now ship
  `trust-dev` beside `trust-runtime` and `trust-bundle-gen`.
- `trust-dev agent serve` now owns the external agent JSON-RPC server. The
  legacy `trust-runtime agent serve` entrypoint remains as a deprecated
  forwarding alias during the product/workbench CLI split.
- `trust-dev docs` now owns ST API documentation generation, and
  `trust-runtime docs` remains as a deprecated forwarding alias during the
  product/workbench CLI split.
- `trust-dev test` now owns ST test discovery/execution, and `trust-runtime
  test` remains as a deprecated forwarding alias during the product/workbench
  CLI split.

### Changed

- `trust-harness` now defaults to JSON protocol version 2 and accepts
  `--protocol-version 1` or `TRUST_HARNESS_PROTOCOL_VERSION=1` for the legacy
  watch-value map shape during migration.
- `trust-runtime commit` is now a deprecated compatibility alias that forwards
  to `trust-dev commit`, preserving the existing command while the product
  runtime CLI is split from workbench/dev commands.
- Public docs and terminal capture scripts now point agent automation workflows
  at `trust-dev agent serve`, with `trust-runtime agent serve` documented as a
  compatibility alias.
- Public docs, CI templates, flake probes, and terminal capture scripts now
  point ST test and ST documentation workflows at `trust-dev test` and
  `trust-dev docs`.
- Local Rust test recipes now use the `mold` linker on Linux when it is
  installed, `just test-all` and CI no longer run `complete_program` twice, and
  `just test-hir-fast` provides a focused HIR refactor loop before the final
  full workspace gate.
- VS Code extension tests now build and resolve the required local
  `trust-debug` adapter and reuse the Linux fast-link Rust wrapper for local
  Rust build steps.
- OSCAT OOP now uses the settled `OscatOop` dependency alias,
  concrete types in simple examples, `PidGains` plus `Configure(...)` for PID
  setup, clearer PID/calendar/astronomy names, and example coverage for
  `Snapshot()` and configuration error handling.
- MQTT I/O now supports explicit TLS/mTLS configuration instead of carrying
  unused default TLS dependencies: `io.params.tls = true` requires
  `tls_ca_path`, optional `tls_client_cert_path`/`tls_client_key_path` enable
  client authentication, `mqtts://` and `ssl://` broker schemes imply TLS,
  Linux release builds use vendored OpenSSL for the selected native TLS backend,
  and remote plaintext brokers still require `allow_insecure_remote = true`.
- The default OSCAT OOP example catalog test now keeps the all-example folder,
  README, source-layout, and pattern checks in the normal Rust suite while
  moving the expensive 98-project `trust-runtime test --project` sweep behind
  an explicit ignored gate with per-project progress, child PID/elapsed
  reporting, and timeout diagnostics.
- The runtime VM benchmark gate now records per-fixture metrics, confidence
  spread, optional baseline compare deltas, and `quick-low-noise` /
  `full-low-noise` repeated-run profiles in its artifacts.
- The runtime VM malformed-bytecode fuzz smoke gate now records progress-visible
  logs and summary artifacts for deterministic bytecode mutation coverage.
- The runtime VM default-switch readiness ledger now records production-guard,
  differential, malformed-bytecode fuzz, benchmark, residual-risk, and rollback
  evidence without pretending blocked performance thresholds are ready.
- VM function and method local initialization now populates VM frame slots
  directly for initialized locals, static locals, and function-block local
  member overrides instead of creating a temporary runtime storage frame on the
  native-call path.

### Fixed

- Runtime bytecode execution now supports IEC Table 17 partial
  bit/byte/word/dword access on `BYTE`, `WORD`, `DWORD`, and `LWORD`
  function-block instance fields, matching the existing program-variable
  behavior.
- `architecture-doctor --changed` now follows the current
  `crates/trust-runtime/src/host/...` runtime host layout for initializer and
  evaluation checks.
- Runtime I/O drivers now fail closed for the covered safety paths: MQTT
  disconnected/stale reads and publish/connect failures return structured
  runtime errors, EtherCAT discovery and image-size mismatches fault under every
  policy, Modbus flush/transport failures and Modbus exceptions remain
  distinguishable, and GPIO read/write failures are reflected in driver health.
- Retain persistence now writes through a temp file, flush/fsync, atomic rename,
  and parent directory sync, and retain snapshots now use a length-delimited
  CRC/trailer-protected codec with legacy v1 read support plus explicit
  retain migration/orphan events.
- Runtime initialization, evaluator assignments, and queued debug writes now
  fail closed for the covered safety paths: default materialization failures
  return `InitFailed` instead of becoming `NULL`, undefined evaluator/debug
  targets are rejected instead of creating globals, and queued debug write
  failures fault the cycle visibly.
- Interface-typed runtime variables now materialize as explicit `NULL`
  references during default initialization, and generic `ANY_INT` counter
  slots defer to the call argument type instead of failing runtime startup.
- Runtime bytecode lowering now materializes HIR-allowed contextual widening at
  assignment boundaries using the target runtime type, with proof coverage for
  function inputs/outputs, assignments, initializers, return values, InOut
  rejection, and narrowing rejection.
- Runtime audit/event, mesh, debug, and runtime-cloud safety paths now fail
  closed for the covered cases: closed audit sinks emit `AuditDropped`, closed
  debug event/log streams fall back to in-memory buffers, mesh snapshot timeouts
  are errors instead of empty maps, corrupt runtime-cloud config state becomes
  an explicit error state, corrupt link/rollout state prevents web startup, and
  disabled debug-control requests return structured `feature_disabled`.
- `FULLMAP-RUNTIMESAFE` now runs as a blocking architecture-doctor/CI gate with
  zero findings and zero allowlist entries.
- Harness boundary inputs now fail closed: typoed `set_input` names and
  `bind_direct` targets return structured boundary errors instead of creating
  hidden globals or undeclared I/O bindings.
- `CONFIGURATION` builds now reject declared top-level `PROGRAM`s that are not
  bound by the configuration, unless a test builder explicitly opts in through
  extra program instances.
- MQTT `keep_alive_s` is now applied to the `rumqttc` session options instead
  of being parsed only at validation time.
- MQTT TLS fixture paths are normalized before being embedded in TOML so the
  TLS config tests run on Windows paths as well as Unix paths.
- Dependency hygiene now has explicit `cargo deny`, `cargo audit`, and
  `cargo machete` policy evidence: unused direct workspace dependencies were
  removed, the workspace MSRV was raised from Rust `1.85` to Rust `1.95`,
  `time` was updated to the patched `0.3.47`, `ratatui` was updated to `0.30`,
  `rumqttc` no longer enables unused default TLS dependencies, `qrcode` no
  longer pulls its unused image backend, the optional `opcua-wire` advisory paths
  are documented with owner/rationale/removal metadata, `Cargo.lock` is tracked
  for reproducible CI dependency resolution, runtime modulo checks now use the
  Rust `1.95` `is_multiple_of` API so Clippy stays clean on the new MSRV,
  `third_party/tiverse-mmap` is an intentional workspace exclude, and the
  full-map doctor reports the dependency hygiene status with failing policy
  fixtures.
- External safety follow-up now prunes the Zenoh default-feature dependency
  surface to remove the transitive `rsa` advisory path, removes direct
  `rustls-pemfile` use from runtime TLS PEM parsing, refreshes owned
  `cargo deny` advisory metadata for the remaining OPC UA, tiny_http TLS, and
  Zenoh `paste` paths, and records the remaining external unsafe-scanner gap in
  the architecture follow-up checklist.
- Parser recovery for malformed aggregate/positional initializers is now
  bounded by shared top-level scan helpers, preserves following declarations at
  declaration boundaries, and is guarded by full-map doctor and focused
  mutation evidence.
- HIR validation now rejects non-repeat call expressions used as array defaults
  and validates direct array repetition defaults against the repeated element
  type, closing focused mutation-testing gaps in default initializer analysis.
- Runtime and constant-expression array repetition initializers now shape arrays
  from the expanded value count, so declarations such as `[3(1, 2)]` materialize
  as six elements instead of failing type validation as a one-element array.
- Bytecode validation now rejects duplicate `POU_INDEX` ids and unsupported
  runtime-only opcodes before VM module construction/dispatch instead of
  silently allowing map overwrites or late runtime traps.
- VM instance-owner inference now scans partial-access opcodes with their
  correct operand width, avoiding a silent owner-context drop in bytecode that
  mixes instance references with bit/byte/word/dword access.
- Bytecode validation now rejects POU bytecode that uses a local reference
  outside that POU's declared local-ref range, so malformed local slots fail
  before VM frame execution, while still accepting owned derived local path
  references such as local array elements and struct fields.
- HIR validation now reports `UndefinedVariable` when a `VAR_ACCESS` access
  path points at a missing target instead of silently accepting the declaration.
- HIR constant evaluation now reports `CannotResolve` for ambiguous unqualified
  enum value names instead of picking the first matching enum member from the
  symbol table.
- HIR alias resolution no longer silently stops after sixteen alias hops, so
  deep but valid alias chains resolve to their base type.
- HIR call checking now reports `UndefinedFunction` when a resolved field or
  non-callable expression is used as a callee instead of silently returning an
  unknown type.
- HIR global symbol lookup now uses the same first-writer collision policy for
  normal collector inserts and raw/import inserts, avoiding order-dependent
  duplicate-name lookup drift.
- Project source registration now rejects explicit `FileId` collisions instead
  of silently reallocating a different file identity.
- HIR type-check context construction now classifies missing POU owners/scopes
  and reports `CannotResolve` instead of silently falling back to global scope.
- HIR OOP diagnostics now use a shared extends-chain walker so mixed
  function-block/class inheritance cycles cannot bypass cycle detection.
- HIR diagnostics now distinguish existing symbols used in the wrong semantic
  role: values used as types, types used as values, and callables used as
  variables report primary wrong-kind diagnostics instead of silent unknowns or
  wrong-reason undefined diagnostics.
- HIR expression checking now applies the same wrong-kind diagnostics to
  namespace-qualified project imports, so imported types used as values no
  longer type-check silently through field-expression resolution.
- HIR/parser name handling now treats `GET` and `SET` as contextual names in
  method/member expression positions, so methods such as `Get()` can be
  collected, type checked, and called without fallback to global scope.
- HIR expression inference now suppresses numeric/bit/unary/index/dereference
  and `__DELETE` follow-on type errors when an operand or indexed base already
  has a primary unresolved-name diagnostic.
- HIR validation now reports unresolved or malformed `VAR_CONFIG` targets
  explicitly instead of letting invalid configuration entries disappear during
  collection/validation.
- Cross-project HIR symbol imports now report duplicate imported global names
  instead of silently keeping the first imported binding.
- Cross-project HIR type imports now report cyclic aliases with
  `CyclicDependency` and avoid degrading the cycle into silent unknown-type
  suppression.
- HIR type checking now reports constant-evaluation failures at array bounds,
  subrange assignments, CASE label duplicate tracking, and `SIZEOF` type
  operands instead of collapsing `ConstEvalError` into `None`.
- HIR assignment compatibility no longer substitutes missing array element or
  pointer target `TypeId`s with `Type::Unknown`, preventing malformed type
  identities from being accepted as compatible.
- HIR typed literals now report `UndefinedType` when their type prefix cannot
  be resolved instead of silently inferring an unknown expression type.
- Project source registration now preserves noncanonical fallback paths when
  OS canonicalization fails, preventing lexical paths from silently colliding
  with existing canonical source keys.
- HIR OOP `EXTENDS`/`IMPLEMENTS` resolution no longer treats a missing owner
  scope as global scope when resolving inherited or implemented types.
- HIR interface conformance now resolves inherited interface members through
  the owner's namespace scope instead of falling back to a global bare name.
- HIR call inference now preserves ambiguous `USING` resolution as the primary
  `CannotResolve` diagnostic instead of adding a wrong `UndefinedFunction`.
- HIR value/type expression inference now preserves ambiguous `USING`
  resolution as the primary `CannotResolve` diagnostic for value reads,
  assignment targets, `SIZEOF`, and `NEW` type operands instead of degrading
  into wrong-reason undefined diagnostics.
- HIR standard function type checking now suppresses wrong-reason argument
  diagnostics when an argument already has a primary unresolved-name diagnostic.
- HIR diagnostics now deduplicate exact duplicate diagnostics by full diagnostic
  identity, preventing repeated semantic probes from reporting the same primary
  error multiple times while preserving distinct diagnostics.
- HIR `SymbolTable` broad lookup helpers are no longer public API; external
  callers now use explicit global, qualified-name, or registered-type lookup
  contracts.
- HIR constant precollection now keys constants by full namespace/POU scope
  identity, so same-named POUs in different namespaces cannot share local
  constant values for array, subrange, or string bounds.
- HIR `VAR_CONFIG` validation now treats duplicate bare program instance names
  across resources as ambiguous and reports `CannotResolve` instead of choosing
  one instance and emitting follow-on type/address diagnostics.
- HIR member and compatibility resolution for raw `EXTENDS`/`IMPLEMENTS`
  references now resolves bare base/interface names from the owning POU scope
  instead of falling back to a global bare-name lookup.
- HIR analysis now exposes a declaration catalog with qualified names, source
  identity, semantic roles, project-import origin, translated type identities,
  and classified OOP reference outcomes; runtime POU registration is driven by
  that catalog and reports catalog/lowering mismatches instead of silently
  skipping accepted declarations.
- VS Code activation no longer writes the default runtime control endpoint into
  workspace folder settings; runtime panels still use the built-in endpoint
  fallback when the setting is empty.
- `Pt1Filter.Reset()` now reinitializes the underlying OSCAT `FT_PT1` state on
  the next update, and `HysteresisSwitch.Reset()` now resets the actual
  hysteresis state instead of only clearing cached wrapper fields.
- `UnitConverter` now delegates Celsius/Kelvin and energy/calorie conversions
  to classic OSCAT functions/function blocks so the component facade cannot
  silently diverge from the parity oracle.
- OSCAT OOP tests now cover reset behavior, multi-scan PT1 parity,
  invalid limit rejection, FIFO ordering, and the OSCAT version through
  `OSCAT_VERSION(IN := FALSE)` instead of a hardcoded value.
- Public docs links for PLCopen/Oscat example READMEs now resolve under MkDocs
  strict mode.
- Runtime compilation now registers namespaced `PROGRAM` declarations and
  resolves sibling namespace types for local OOP `IMPLEMENTS` checks, so
  namespaced runtime programs and same-namespace interfaces are handled
  consistently with functions, function blocks, classes, and type declarations.
- Runtime bytecode generation now preserves POU namespace context for sibling
  function calls, keeps method names owner-local inside namespaced class-like
  POUs, and exposes bare return-variable aliases for namespaced functions.

### Added

- Structured Text initializers now support named aggregate defaults for
  `STRUCT`/`UNION` values at VAR, TYPE, and member-default sites, route those
  defaults through a runtime initializer service, validate unknown/duplicate
  fields with stable diagnostics, allow legal function-block member overrides,
  and expose `trust-runtime bench init` for startup/first-cycle initialization
  benchmark evidence.
- PLCopen Motion now ships a second, object-oriented package at
  `libraries/plcopen_motion/oop`, with ST interfaces and concrete objects for
  `itfAxis`, command objects, and `MC_OopAxis`. The package delegates axis
  behavior to the classic single-axis PLCopen package, includes deterministic
  unsupported command-object returns for unimplemented OOP methods, and is
  covered by Structured Text unit tests plus runtime integration drivers.
- The public docs now include a PLCopen Motion OOP library guide and five
  runnable real-world examples: warehouse shuttle, labeling conveyor,
  pick-and-place lift, indexing table, and feeder axis.
- OSCAT now ships an object-oriented Components companion package at
  `libraries/oscat/oop`, with narrow interfaces for automation context,
  unit conversion, filtering, PI/PID/PWM control, hysteresis switching, signal
  generation, FIFO/stack memory objects, latches/toggles/counters, measuring
  components, calendar/RTC helpers, selected building-control objects, and a
  single-output device driver. The package is covered by Structured Text
  parity tests against classic OSCAT.
- The OSCAT OOP example suite now includes 49 classic/OOP comparison
  pairs under `examples/OSCAT/<example>/{non-oop,oop}`: 27 hand-written
  process-first industrial pattern scenarios, 20 compact component-composition
  showcases, and 2 compact pattern showcases. Each example folder has one
  teaching README explaining the process, the OOP pattern, why it helps, how to
  reuse it, and when classic ST is better. Each project has Structured Text
  application code and Structured Text unit tests; OOP projects with
  communication claims include runtime/IO
  configuration and README integration maps naming `%I/%Q` bindings and exposed
  runtime records. A Rust catalog gate now runs every project and checks that
  the claimed OOP pattern is present in `src/Main.st`, covering Factory,
  Template Method, Strategy, Mediator, Observer, Composite, Iterator-style
  traversal, Decorator, Facade, Chain of Responsibility, State,
  Command/Memento, Adapter, Proxy, polymorphism, and composition. Public docs
  include a library guide plus the new truST Structured Text naming standard.

- The public docs now include a `One Project, Every Surface` concept page that
  positions VS Code, Editor AI tools, Browser IDE, Browser HMI, CLI/CI, Agent
  API, LSP editors, and truST Mesh as live surfaces over the same project, with
  an honest surface capability matrix and AI tooling evidence links. The docs
  homepage, README, workflow chooser, visual-editor docs, and migration path
  now point into the same narrative, and the README/homepage use a new
  one-project surface-tour GIF built from real product captures. Migration
  ecosystem pages now have canonical `migrate/*` URLs, visual-editor pages avoid
  misleading empty captures by using accurate diagrams, and project/support
  pages stay outside the lookup-only Reference route. A full public-doc text
  pass also removed thin wrapper prose, separated HMI authoring from HMI
  operation, routed Learning Paths into Program, clarified Troubleshooting vs
  FAQ, expanded First Project into a real walkthrough, added jargon notes to
  concept pages, tightened the homepage around one route table plus real
  product proof, added richer example/category routing, kept AI Assistance
  grounded in diagram/source evidence, added first-figure proof PNGs for the
  operator HMI overview, daily-check, alarm, and handover pages, and tightened
  README platform/status wording including Linux PREEMPT_RT support. The public docs nav has since
  been collapsed to six user-facing doors (`What Is truST?`, `Install`,
  `Program`, `Run`, `Hardware`, `Reference`), with tutorials/examples,
  communication, I/O, HMI authoring, migration, automation, and AI work routed
  through `Program`; OpenPLC is no longer presented as a standalone truST
  workflow; dense terms blocks and stock "use this guide when" boilerplate were
  removed; and the one-project GIF was regenerated at higher resolution without
  the dark banner overlay.

- `trust-runtime plcopen` now imports official TC6 XML multi-worksheet ST
  bodies in execution order, reconstructs all standard POU interface var
  sections (`VAR`, `VAR_TEMP`, `VAR_INPUT`, `VAR_OUTPUT`, `VAR_IN_OUT`,
  `VAR_EXTERNAL`, `VAR_GLOBAL`, `VAR_ACCESS`), and round-trips CODESYS
  `addData/method` metadata for `FUNCTION_BLOCK` methods with deterministic
  method object IDs/project-structure placement.

- `trust-runtime` now supports a Linux `runtime.realtime` startup profile for
  `PREEMPT_RT` posture, including config-schema support, bundle-template
  defaults, launcher-side kernel/scheduler/affinity/memory-lock verification,
  richer runtime status/config surfaces, application-level cycle `p50` / `p95`
  / `p99` metrics, a dedicated operator deployment guide, a reference
  `systemd` unit template, and a shipped validation script for collecting
  host/kernel/service evidence alongside release-mode benchmark artifacts.
- The public docs now have a workflow-first entry model instead of a mixed
  "one start flow for everyone" structure. Home and Start now route five real
  user journeys directly: Program in VS Code, Program in Browser IDE, Operate
  in Browser HMI, Automate with CLI / CI / agents, and Maintain an Existing
  Project. The site now also includes public `About`, `FAQ`, `Contribute`,
  `Version History`, `Hardware Compatibility`, `API Lifecycle And Deprecation`,
  runbook/operator pages, target-host lifecycle pages, and enterprise-integration
  routing pages so technicians, maintainers, and GitHub visitors can find the
  right docs path without repo spelunking.
- The docs homepage and repo README now surface the published docs site as the
  primary entry point, use the truST logo directly, and show real browser
  captures for Browser IDE and Browser HMI instead of forcing readers through
  repo file paths or VS Code-only framing.
- `trust-runtime agent serve` now provides an initial external agent surface
  over stdio JSON-RPC with `agent.describe`, `workspace.read`,
  `workspace.write`, `workspace.project_info`, `lsp.diagnostics`,
  `lsp.format`, `runtime.build`,
  `runtime.compile_reload`, `runtime.validate`, `runtime.test`,
  `runtime.reload`, `harness.load`, `harness.reload`, `harness.cycle`,
  `harness.set_input`, `harness.get_output`, `harness.advance_time`, and
  `harness.run_until`, plus stable error codes for unknown methods,
  workspace-path escapes, harness state, and bounded `run_until` timeouts.
  The runtime-facing methods now reuse the existing `build --ci`,
  `validate --ci`, JSON test-output path, in-process Web IDE
  diagnostics/formatting services, and control-server `bytecode.reload`
  contract instead of inventing a second reporting surface.
- `trust-runtime agent serve runtime.compile_reload` now returns one
  machine-readable payload for `diagnose -> build -> reload`, including
  diagnostics counts/issues plus `runtimeStatus` / `runtimeMessage` and the
  optional nested build/reload results needed for iterative repair loops.
- `trust-runtime agent serve` now exposes `lsp.ast_canonicalize` and
  `lsp.ast_similarity` for the canonical-AST 5-gram normalization path used by
  benchmark contamination checks, dedup, and self-improvement diversity gates.
  The existing `lsp.diagnostics` payload now also includes end positions and
  zero-based UTF-8 byte spans, so benchmark/datagen tooling can consume stable
  machine-readable diagnostics without brittle text scraping.
- `trust-runtime agent serve` now also exposes a stateless `harness.execute`
  fixture wrapper that loads project or inline sources, runs deterministic
  harness steps, evaluates output/access/direct-I/O assertions, and returns a
  compact pass/fail report with reduced failure context for benchmark/datagen
  callers that need one-shot POU or system-fixture execution.
- `trust-runtime agent serve workspace.project_info` now returns a stable
  orientation payload for agents, including resolved source roots, source
  files, local dependency roots, runtime/io config presence, parsed runtime
  control/web/mesh/discovery summary fields, and `trust-lsp.toml`
  `vendor_profile` when present.
- `trust-runtime` now lowers ST `PROPERTY` implementations into runtime getter
  and setter calls, so OOP interface references can dispatch property reads and
  assignments across source files and package dependencies.
- `trust-harness` is now a real programmable deterministic executor instead of
  a minimal `load` / `cycle` helper. It now supports `reload`, `set_input`,
  `get_output`, `set_access`, `get_access`, `bind_direct`,
  `set_direct_input`, `get_direct_output`, `advance_time`, `run_until`,
  `restart`, and `snapshot`, all over structured JSON-lines with shared typed
  IEC value encoding and protocol docs in
  `docs/guides/TRUST_HARNESS_PROTOCOL.md`.
- A canonical public docs site now lives under `docs/public/` with question-
  driven navigation (`start`, `develop`, `connect`, `operate`, `reference`,
  `concepts`, `examples`), a MkDocs + Material site config at repo root, a
  Pages workflow that builds and deploys the docs site, and a public-doc link
  checker in `scripts/check_public_docs_links.py`.
- The runnable examples catalog is now aligned to the docs IA instead of
  acting like one flat dump: `examples/README.md` now routes by docs category,
  `docs/internal/testing/checklists/example-catalog-audit.md` records keep /
  tweak / merge / archive decisions, `examples/sfc/README.md` and
  `examples/web_ui_complete_project/README.md` were added, and
  `examples/simulate_process/README.md` now marks that family as archive-
  candidate rather than a first-stop public example. Kept public examples now
  also link back to their owning docs category, and
  `scripts/check_example_catalog_links.py` enforces that the curated docs
  catalog points to real runnable example paths.
- The public docs are now materially self-contained instead of mostly wrapper
  pages: editor selection, protocol matrices, runtime-to-runtime transports,
  visual-editor guidance, runtime control/debug/operator docs, benchmark
  reference, vendor-profile example routing, and key protocol pages now render
  substantive in-site content instead of punting readers to raw GitHub pages.
- Public-doc media is now generated through an explicit pipeline:
  `scripts/capture-public-docs-visual-editors.sh` captures the visual-editor
  screenshots automatically, `scripts/generate_public_docs_media.py` syncs
  them into `docs/public/assets/images/`, and the Pages workflow now rebuilds
  when those source screenshots or media-generation scripts change.
- The docs Pages workflow now has stricter quality gates: local search
  regression checks, post-deploy verification of the published site, and
  automated media synchronization all run as part of the docs build path.
- The public docs now cover more real operator/agent search paths directly in
  CI, including `scan cycle`, `project layout`, `visual editor`, `vendor
  profile`, `watchdog`, `fault policy`, `agent serve`, `hot reload`, and test
  output format queries, so discoverability regressions are caught earlier.
- Public-doc screenshots now have a first automated browser-capture lane:
  Playwright regenerates checked-in `/ide` and `/hmi` assets against live
  truST surfaces, code-server provides a truthful VS Code-compatible proof
  capture for the public docs, `docs/public/assets/capture-inventory.json`
  records the checked-in completeness contract, and a dedicated docs-captures
  workflow can refresh those assets on `main` and nightly without relying on
  `ydotoold`.
- Public-doc terminal captures now have a first automated lane as well:
  VHS tapes regenerate checked-in CLI verification GIFs for installation and
  build/validate/test docs, and the same docs-captures workflow refreshes
  those assets alongside the browser/code-server captures.
- The public docs now include a dedicated diagnostics reference page covering
  current `E...`, `W...`, and `I...` codes, their default severities, meanings,
  and first-fix guidance, and docs-search regression coverage now includes code
  queries such as `E001`, `W003`, `implicit conversion`, and `missing else`.

### Fixed

- Enum values loaded from `VAR` initializers now compare equal to the same enum
  variant literal in VM execution, covering unqualified, qualified, and
  literal-on-left equality checks for init-only state-machine selectors. Runtime
  enum construction now canonicalizes alias-backed enum values and retained enum
  state before comparison, preventing stale or case-variant type names from
  silently turning valid enum equality checks into `FALSE`. Runtime compound
  values now also validate struct field identity and array shape at construction
  and retain-apply boundaries, so corrupted retained structs or arrays fail with
  diagnostics instead of being silently coerced or defaulted.
- Runtime ST test compilation now reuses expression type information from the
  HIR analysis pass, avoids repeated full-project symbol-table clones during
  lowering, and skips shared-global task-hazard scans when a project has no
  configured tasks. This keeps the full OSCAT timeout regression below the
  test harness 60-second warning while preserving the runtime timeout budget.
- Public docs IA now removes the duplicate Install/Verify Install route,
  keeps Reference focused on lookup material by moving specifications higher
  and hiding contributor-only pages, deletes dead interoperability stubs and
  orphan/broken image assets, embeds the real one-project surface tour on the
  docs homepage, and expands the IA checker to block generic filler phrases
  such as "use this page", "after reading", and "you should be able".
- Public docs visual chrome now uses a dark/light palette toggle, top-level
  navigation tabs for the six main doors, a shorter GitHub repo label, a
  distinct accent color, Inter/JetBrains Mono typography, and image overflow
  safeguards so screenshots stay inside the content column. The docs homepage
  also switches to a dark-mode wordmark variant so the brand stays legible on
  the slate palette.
- Public docs navigation now exposes the PREEMPT_RT runbook from the Operate
  sidebar, reuses the interoperability pages under Migrate without moving their
  canonical URLs, groups Operate by engineering, target-administration,
  operator/technician, and fleet-delivery personas, surfaces truST Mesh from
  Connect, links README start paths directly, and avoids confirmed
  snippet/list-rendering issues in included public documentation. Concept,
  Reference, Specifications, and Examples indexes now use clearer task and
  mental-model groupings, the docs maintenance guide codifies nav, snippet,
  visual, verification, and proof rules, the SFC profile reference now uses a
  non-numbered slug to avoid a spec-14 collision, visual-editor and operator
  pages now include public visuals, the Migrate page includes a compatibility
  matrix, and the example catalog audit understands PyMdown line-range snippet
  targets used to skip included page titles. A new docs IA lint now enforces
  section-index/nav coverage, snippet H1 safety, and CommonMark list spacing,
  and the internal docs checklist records analytics, versioning, success-metric,
  accessibility, and mobile-review decisions.
- The shipped `examples/oscat_smoke` consumer now matches the current OSCAT
  named-parameter surface (`DEG`, `DAYS`, and `DATA`), so the example builds
  cleanly again instead of failing on stale `D := ...` calls.
- `trust-lsp` push-diagnostics mode now refreshes dependent open documents
  after `didOpen`, `didChange`, and `didSave` events, so cross-file call
  diagnostics clear and update immediately instead of staying stale until a
  reopen/save cycle.
- Method-call completion and signature help now resolve `FUNCTION_BLOCK`
  methods with `VAR_INPUT` parameters correctly, so formal parameter names are
  offered for `fb.Method(...)` calls instead of falling back to the owning
  instance call shape.

- Linux `PREEMPT_RT` support no longer breaks non-Linux cargo builds and
  warning gates: Linux-only helpers/imports in `trust-runtime` are now
  target-scoped so macOS and Windows builds keep compiling cleanly while the
  RT posture feature remains Linux-only.
- The Linux cyclic runtime no longer rebuilds ready-task and background-program
  scratch vectors every scan cycle; those scratch buffers are now runtime-owned
  and reused so the `PREEMPT_RT` path reduces avoidable hot-path allocation
  churn while preserving existing Linux behavior.
- Linux `PREEMPT_RT` posture verification now reads the scheduler thread's
  user-space realtime priority from `/proc/thread-self/stat` instead of the
  kernel-internal `/proc/.../sched` priority field, so correctly-configured
  `SCHED_FIFO` / `SCHED_RR` deployments no longer report false priority
  mismatches. The shipped validation script now also exposes whether a target
  threshold was declared, records the approximate soak window, warns when a
  `PREEMPT_RT` run falls back to the cycle budget, and fails gated RT evidence
  runs when `cyclictest` or an explicit `TRUST_RT_CYCLE_P95_MAX_US` threshold
  is missing.
- The public docs and repo landing surfaces now lead with the strongest truST
  story instead of burying it behind browser-first proof shots: the homepage,
  `Program In VS Code`, `Debugging And Runtime Panel`, and `README.md` now show
  real desktop VS Code screenshots for the runtime panel, IEC-aware diagnostics,
  paused debugging, and cross-file rename, while browser IDE/HMI surfaces are
  clearly presented as supporting views of the same project.
- The docs site now ships the truST wordmark, a dedicated favicon, and a teal
  Material theme instead of the default MkDocs branding, and the public pages
  were rewritten to remove boilerplate "use this page when..." framing and
  similar filler so the rendered docs read as product documentation instead of
  internal notes.
- The public docs landing/start/reference/operator pages now describe truST in
  direct product language, ship real social metadata for sharing, and replace
  thin operator checklists with concrete tables, screenshots, and escalation
  placeholders instead of stub prose.
- The checked-in Browser IDE and code-server public-doc captures now prove a
  successful truST state more clearly: `/ide` waits for a real build to finish
  before capture, and the VS Code-compatible proof opens the Structured Text
  Runtime panel instead of showing only a generic workspace shell.
- The docs search regression gate now routes `install mac`, `first project`,
  `ladder`, and `retain` queries back to the intended public docs pages after
  the docs IA/spec split, including a restored `Start -> First Project` page
  and stronger canonical ranking for `runtime.toml` retain/watchdog/fault
  queries.
- The docs-capture code-server launcher now keeps mutable user-data and
  extension state inside container-local writable directories, avoiding the
  GitHub Actions permission failure that previously blocked the code-server
  capture lane on repo-mounted `.cache` paths.
- The public docs Pages pipeline no longer hard-fails on runners without
  `ffmpeg` when all committed screenshot assets are already present, and the
  cross-platform `trust-runtime agent serve` contract tests now normalize
  OS-specific path spellings (`/private` aliases, `\\?\\` prefixes, native
  separators) instead of failing on macOS/Windows-only path formatting
  differences.
- Warm restart and live `bytecode.reload` now preserve compiled instance-backed
  runtime references across the restart boundary, so instance-backed I/O
  bindings and the new `trust-runtime agent serve` `runtime.reload` flow no
  longer trip `null reference dereference` faults immediately after a reload.

- VS Code language-model tool discovery is now consistent again: the extension
  registers the same linked-editing, on-type-formatting, call-hierarchy, and
  type-hierarchy tool names that `editors/vscode/package.json` declares and
  activates, and the extension test suite now fails if manifest declarations,
  activation events, and `lm.registerTool(...)` registrations drift apart.
- The public docs workflow now rebuilds on guide/spec/conformance/media-source
  edits instead of only direct `docs/public/**` changes, closing the stale-site
  gap for `--8<--`-included content and generated public assets.
- The code-server screenshot lane now waits for real editor content instead of
  only a tab label, dismisses first-run/sign-in chrome, closes the unrelated
  chat side panel before capture, and renames the Browser IDE screenshot to the
  truthful "tutorial loaded" state instead of implying a welcome screen that
  was not what the image actually showed. The installation docs also no longer
  tell users to run the nonexistent `cargo build -p trust-harness` package
  command; they now document the real package/build surface.
- The automated public-doc capture lanes now produce stable dark-mode browser
  and code-server screenshots from real app/theme controls, wait for the HMI
  schema/export surface before capturing `/hmi`, and install/load the truST
  VS Code extension through one consistent code-server profile so the
  command-palette capture stops regressing on fresh CI boots.
- Unqualified enum variant names now also resolve in three contexts outside
  `CASE` labels: `VAR` initializers (`state : Phase := IDLE`), the right-hand
  side of assignments (`state := RUNNING`), and operands of binary
  comparisons (`IF state = RUNNING THEN ...`). Before this change the first
  form failed PROGRAM init with `undefined variable 'IDLE'`, and the other
  two compiled but silently lowered the bare variant name as an unresolved
  `Expr::Name`, so the target either kept its previous value or the
  comparison never matched at runtime. The fix introduces a small
  harness-lowering helper next to the existing `enum_literal_value` used by
  `CASE` labels and applies it at the VAR-declaration, assignment, and
  binary-expression call sites, mirroring the pattern introduced by the
  unqualified-CASE-label fix. HIR type-check was already accepting all
  three forms, so this is a lowering-layer alignment only; bare names still
  follow normal symbol resolution, which means local variables/constants
  continue to shadow same-named enum members, and qualified `Phase#X` forms
  plus `CASE` labels behave as before.
- The ST compiler/runtime build path now closes the remaining open regression
  cases around control-flow and declaration lowering: unqualified enum members
  now work as `CASE` labels in end-to-end runtime/bytecode builds, aggregate
  array declaration initializers now parse and execute for explicit, partial,
  and repetition-count forms, struct-field access through indexed arrays stays
  buildable on the CLI path, and bare `RETURN;` now works after assigning the
  implicit function/method result on that path because the HIR checker tracks
  definite assignment and the bytecode VM lowers `RETURN` instead of rejecting
  it as a legacy C5 edge case.
- The OSCAT runtime/compiler path now handles the full shipped OSCAT library
  more reliably: bytecode lowering resolves local instance names before global
  callable symbols, normalizes OSCAT-style identifier lookup consistently
  across locals/reference lowering, and fixes remaining Chapter 19/20/23/24
  regressions such as `AOUT`/`AOUT1` scaling, `MULTI_IN` mode `0`,
  `ACTUATOR_COIL`, `CTRL_IN`, `HYST*`, `PARSET`/`PARSET2`, and first-scan
  `TEMP_EXT` sampling. The full OSCAT core fixture now runs green again.
- Runtime VM string access, native-call dispatch, and hot-path execution are
  now more consistent and predictable: signed unary `NEG` on minimum signed
  integers now reports overflow instead of panicking/wrapping, pure-`DINT`
  `MOD 0` now matches the generic `ModuloByZero` contract, array offset math
  and runtime date construction reject invalid edge cases deterministically,
  runtime `STRING` / `WSTRING` indexing and string stdlib helpers now follow a
  single documented element contract for non-ASCII text, `CALL_NATIVE` now
  caches statically resolvable function targets instead of uppercasing and
  hashing every call, and the VM hot path now reuses register frame buffers,
  drops redundant register-file zeroing, batches stack deadline checks, and
  avoids cloning the debug hook on each statement. The release benchmark path
  also restores the earlier VM latency envelope by caching per-POU local-init
  plans and trimming dynamic-ref hot-path overhead instead of rebuilding that
  work every scan.
- The OSCAT BASIC `CALENDAR` port keeps truST strict about IEC reserved
  keywords instead of relaxing the parser, so the shipped Chapter 3 data type
  exposes compliant local-time field names `LOCAL_DT`, `LOCAL_DATE`, and
  `LOCAL_TOD` in place of upstream `LDT`, `LDATE`, and `LTOD`.
- `trust-hir` now resolves cross-file user-defined types for root `VAR_GLOBAL`
  declarations during project-aware symbol collection instead of depending on a
  late repair pass, so split projects like OSCAT BASIC can keep shared carrier
  types in one file/chapter and their globals in another while diagnostics,
  analysis, runtime builds, and VM execution still resolve `MATH` / `PHYS` /
  `LANGUAGE` correctly. The same project-type catalog path now also preserves
  imported function block/class/interface identity, honors `USING` + namespaced
  simple type references, and attaches every declaration inside multi-entry
  `TYPE ... END_TYPE` blocks so cross-file POU member access surfaces like
  `FB_Accumulator`, `ST_PumpCommand`, and `ST_PumpStatus` analyze correctly.
- Typed conversion calls no longer misclassify an outer positional argument as a named/formal argument when the nested inner call uses `IN := ...`, so expressions like `UDINT_TO_REAL(DWORD_TO_UDINT(IN := x))` type-check correctly again.
- `SIZEOF(...)` now resolves the static type of an explicit type name or
  storage operand (`var`, field/index access, dereference, `THIS.field`) during
  analysis/lowering instead of evaluating a runtime value, so `SIZEOF(var)` now
  works in normal expressions and constant contexts like
  `ARRAY[0..SIZEOF(packet)-1]`, bare-name lookup follows CODESYS-style
  variable-before-type shadowing even when a top-level type shares the same
  bare name, pointer/reference operands now report and const-fold to platform
  pointer word size instead of truST's internal handle layout, and invalid
  forms such as `SIZEOF(call)`, arithmetic expressions, open arrays, unsized strings, and whole
  FB/class/interface instances fail deterministically during build instead of
  lowering to the legacy `SIZEOF_VALUE` path.

### Added

- `libraries/oscat` now ships the full manual-aligned OSCAT chapter set
  (`03_data_types` through `26_list_processing`) as the primary OSCAT package,
  including the OSCAT_BUILDING Chapter 23 control/environmental surface, a
  full ST conformance project under `fixtures/oscat/core` that now passes
  `126` tests with `0` failures and `0` errors, plus a narrowed
  `negative_public_surface` sentinel that exists only to pin the deliberate
  upstream `OVERRIDE` -> truST `OVERRIDE_3` name deviation.
- The OSCAT user documentation now matches the library’s current shipped
  surface: `docs/guides/OSCAT_LIBRARY_GUIDE.md`, `libraries/oscat/README.md`,
  `docs/README.md`, and `examples/oscat_smoke/README.md` now describe the full
  package layout, validation evidence, public carrier/data-type surface, and
  the major function-block families instead of the earlier smaller OSCAT BASIC
  slice.
- `libraries/oscat` now also ships the full Chapter 6 `Arrays` surface:
  `_ARRAY_ABS`, `_ARRAY_ADD`, `_ARRAY_INIT`, `_ARRAY_MEDIAN`, `_ARRAY_MUL`,
  `_ARRAY_SHUFFLE`, `_ARRAY_SORT`, `ARRAY_AVG`, `ARRAY_GAV`, `ARRAY_HAV`,
  `ARRAY_MAX`, `ARRAY_MIN`, `ARRAY_SDV`, `ARRAY_SPR`, `ARRAY_SUM`,
  `ARRAY_TREND`, `ARRAY_VAR`, and `IS_SORTED`.
- `libraries/oscat` now also ships the full Chapter 4 `Other Functions`
  slice: `STATUS_TO_ESR`, `OSCAT_VERSION`, `ESR_COLLECT`, `ESR_MON_B8`,
  `ESR_MON_R4`, and `ESR_MON_X8`.
- `libraries/oscat` now also ships the full Chapter 5 `Mathematics`
  surface, including the remaining error/distribution helpers (`ERF`, `ERFC`,
  `GAUSS`, `GAUSSCD`), integer/sequence/fraction helpers (`EXPN`, `FACT`,
  `FIB`, `GCD`, `REAL_TO_FRAC`), transcendental helpers (`GDF`, `GOLD`,
  `LAMBERT_W`, `LANGEVIN`, `SIGMOID`, `SINC`, `SQRTN`, `TANC`), rounding and
  window helpers (`RND`, `ROUND`, `WINDOW`, `WINDOW2`), and the OSCAT random
  helpers (`RDM`, `RDM2`, `RDMDW`).
- `libraries/oscat` now also ships the first Chapter 3 data-type slice:
  `CALENDAR`, `COMPLEX`, `CONSTANTS_LOCATION`, `CONSTANTS_SETUP`, `ESR_DATA`,
  `FRACTION`, `HOLIDAY_DATA`, `REAL2`, `SDT`, `TIMER_EVENT`, and `VECTOR_3`,
  plus the preloaded `MATH.FACTS[...]` factorial table used by later OSCAT
  helpers.
- `libraries/oscat` now also ships the stateful helper function blocks `DELAY` and `FT_AVG`.
- `libraries/oscat` now also ships the first linear/polynomial/ramp math helpers: `F_LIN`, `F_LIN2`, `F_POLY`, `F_POWER`, `F_QUAD`, and `FRMP_B`.
- Added the first shipped `libraries/oscat` package with the initial OSCAT compatibility slice (shared math/physics constants plus core conversion helpers), alongside `examples/oscat_smoke` as the first consumer project wired through `[dependencies]`.
- Added OSCAT compatibility extensions `TIME_TO_DWORD`, `DWORD_TO_TIME`, `T_PLC_MS`, and `T_PLC_US`, plus restored `DIR_TO_DEG`, `F_TO_PT`, `PT_TO_F`, and the live `LANGUAGE.DIRS[...]` surface in `libraries/oscat`.
- `libraries/oscat` now also ships a larger date/time slice including `DAY_TO_TIME`, `HOUR_TO_TIME`, `MINUTE_TO_TIME`, `SECOND_TO_TIME`, `DAY_OF_DATE`, `DAYS_DELTA`, `DAYS_IN_MONTH`, `DAYS_IN_YEAR`, `DATE_ADD`, `EASTER`, `WORK_WEEK`, `HOUR_OF_DT`, `MINUTE_OF_DT`, `SECOND_OF_DT`, `MONTH_BEGIN`, `MONTH_END`, `YEAR_BEGIN`, `YEAR_END`, and related leap/year helpers.
- `libraries/oscat` now also ships the first larger string and logic slices, including `MONTH_TO_STRING`, `WEEKDAY_TO_STRING`, `DT_TO_STRF`, `TO_LOWER`, `TO_UPPER`, `LOWERCASE`, `UPPERCASE`, the `ISC_*` / `IS_*` predicate helpers, the `FIND_CHAR` / `FIND_CTRL` / `FIND_NONUM` / `FIND_NUM` / `FINDB*` / `FINDP` search helpers, plus `CAPITALIZE`, `CLEAN`, `COUNT_CHAR`, `COUNT_SUBSTRING`, `CODE`, `DEL_CHARS`, `TO_UML`, `DEC_TO_BYTE`, `DEC_TO_DWORD`, `DEC_TO_INT`, `BYTE_TO_STRB`, `BYTE_TO_STRH`, `DWORD_TO_STRB`, `DWORD_TO_STRH`, `BIN_TO_BYTE`, `BIN_TO_DWORD`, `HEX_TO_BYTE`, `HEX_TO_DWORD`, `OCT_TO_BYTE`, `OCT_TO_DWORD`, `MIRROR`, `REPLACE_ALL`, `REPLACE_CHARS`, `REPLACE_UML`, `CHARCODE`, `CHARNAME`, `TICKER`, `LTCH`, `LTCH_4`, `STORE_8`, `COUNT_BR`, `COUNT_DR`, `TOGGLE`, `FF_D2E`, `FF_D4E`, and `FF_DRE`.
- `libraries/oscat` now also ships the rest of the current logic surface: `FF_JKE`, `FF_RSE`, `SELECT_8`, `SHR_4E`, `SHR_4UDE`, `SHR_8PLE`, `SHR_8UDE`, the full current gate-logic helper slice (`DEC_*`, `MUX_*`, `BIT_*`, `BYTE_*`, `WORD_*`, `DWORD_*`, `SHL1`, `SHR1`, `SWAP_*`, `REAL_TO_DW`, `DW_TO_REAL`, `CHK_REAL`, `REFLECT`, `REVERSE`), plus generator trigger FBs `A_TRIG`, `B_TRIG`, and `D_TRIG`.
- `libraries/oscat` now also ships the full current `/Logic/generators` and `/Logic/memory` slices: `CLICK_CNT`, `CLICK_DEC`, `CLK_DIV`, `CLK_N`, `CLK_PULSE`, `CYCLE_4`, `GEN_BIT`, `GEN_SQ`, `SCHEDULER`, `SCHEDULER_2`, `SEQUENCE_4`, `SEQUENCE_8`, `TONOF`, `TP_X`, `FIFO_16`, `FIFO_32`, `STACK_16`, and `STACK_32`.
- `libraries/oscat` now also ships the `/Logic/Others`, `/Buffer Management`, `/List Processing`, `/Mathematical/Geometry`, and `/Mathematical/Double Precision` slices: `CRC_GEN`, `MATRIX`, `PIN_CODE`, `_BUFFER_*`, `BUFFER_*`, `LIST_*`, `REAL2`, `R2_*`, `CIRCLE_*`, `CONE_V`, `ELLIPSE_*`, `SPHERE_V`, and `TRIANGLE_A`.
- Added `docs/guides/OSCAT_LIBRARY_GUIDE.md` plus an expanded `examples/oscat_smoke` walkthrough so the shipped OSCAT package now has a user-facing reference and a concrete consumer guide comparable to the motion-library docs.
- Runtime benchmark scripts now share a documented host-codegen policy via `TRUST_RUNTIME_HOST_CODEGEN=auto|generic|native`, defaulting to host-native builds only on Raspberry Pi benchmark hosts while keeping portable/generic builds explicit for shared comparisons and release artifacts.
- User-facing build docs now also cover the full host-native and PGO runtime build workflow, including the required `llvm-tools-preview` setup, corpus-training commands, and current Raspberry Pi 5 benchmark evidence (`full_demo` about `433.501 us -> 404.353 us` with `native`, then about `292.298 us` with `native+PGO`).
- Added `docs/diagrams/architecture/runtime-bytecode-vm-execution.puml` and corrected the high-level runtime architecture notes so the official diagrams now distinguish production bytecode-VM execution from residual `EvalContext` / `legacy-interpreter` helper paths.
- `trust-runtime bench project` now benchmarks a real project folder with watched-global capture, cycle-budget reporting, per-cycle latency summaries, and budget-overrun counts, so reusable Structured Text libraries can be validated through user-facing example projects instead of only synthetic fixtures.
- Added `examples/plcopen_motion_single_axis_demo` as the reference consumer for `libraries/plcopen_motion/single_axis_core`, plus `scripts/runtime_motion_example_bench_gate.sh` for focused end-to-end semantic + cycle-budget validation of the shipped PLCopen motion surface.
- Added `examples/plcopen_motion_single_axis_benchmarks` plus `scripts/runtime_motion_benchmark_breakdown.sh` to isolate runtime floor, `MC_Constants()`, status/readback, inactive-command, single-command, and constants-once motion costs on the same release benchmark path.
- `scripts/runtime_motion_benchmark_breakdown.sh` now forces a fresh `cargo build --release -p trust-runtime --bin trust-runtime` before benchmarking so release measurements cannot accidentally reuse a stale `target/release/trust-runtime` binary.
- `trust-runtime bench project` now emits VM register-profile details for VM-backed workloads, including hot blocks, fallback reasons, and lowering-cache counters, so project benchmarks can explain where scan time is going instead of only reporting latency totals.
- `trust-runtime bench project` now emits `vm_profile.ref_ops` and `vm_profile.call_ops` in JSON/table output, and `scripts/runtime_motion_benchmark_breakdown.sh` now summarizes those counters per workload so VM performance investigation runs show ref-path and FB/frame activity directly instead of hiding it in raw runtime state.
- `trust-runtime bench project` now also emits `vm_profile.value_ops` in JSON/table output, and `scripts/runtime_motion_benchmark_breakdown.sh` now summarizes value-clone/move attribution (const loads, register reads, read-side value clones, binding expr clones, output copy-back clones) so runtime speed work can distinguish read-path churn from register-file churn.
- `trust-runtime bench project` now also emits `vm_profile.tier1_specialized_executor` in JSON/table output, and the experimental tier-1 compiler now supports `RefField`, `RefIndex`, `LoadDynamic`, and `StoreDynamic` so ref-heavy FUNCTION_BLOCK blocks show up in benchmark compile/deopt/execution evidence instead of staying invisible behind `null` tier-1 stats.
- `VariableStorage` now caches instance-field resolution per instance, with invalidation on new instance-field insert, so repeated FUNCTION_BLOCK self-field access and parameter binding can skip repeated field-name lookup without assuming that all same-type instances always share the exact same runtime layout.
- VM FUNCTION_BLOCK argument binding now uses a direct declared-parameter offset fast path with recursive fallback for inherited or unusual fields, trimming repeated binding overhead on ref-heavy release benchmarks without changing call semantics.
- VM output copy-back and empty-path storage access now use direct-slot `VmWriteTarget` / `VariableStorage` fast paths for locals, globals, and instances, reducing generic `ValueRef` resolution on the hot benchmark path without changing runtime semantics.
- VM `IN` arguments that arrive as empty-path target references now reuse the same direct-slot `VmWriteTarget` read path for locals, globals, and instances, trimming generic reference reads on ref-heavy hot paths without changing call semantics.
- VM dynamic reference shape inspection now borrows storage/local values for `REF_FIELD` and `REF_INDEX` traversal instead of cloning them first, which materially improves ref-heavy release motion workloads without changing runtime semantics.
- VM hot paths now also borrow output target type inspection for `write_output_int()` and use borrowed DINT fast paths for fused ref+const/register guards in `register_ir`, trimming unnecessary clone/materialization work on ref-heavy micro-benchmarks without changing runtime semantics.
- VM FUNCTION_BLOCK binders now skip field resolution when `OUT`/`IN_OUT` parameters are omitted, use an ordered named-argument fast path for the common declaration-order case, and decode native call payloads in pop order instead of reversing/removing a payload vector on every call.
- Runtime `Value::Struct` now uses shared-on-clone copy-on-write storage, which removes the remaining deep-clone hot path for PLCopen-style `AXIS_REF` `VAR_IN_OUT` transfers; on the rebuilt motion demo gate this cuts `read_value_clones` from `6467` to `227`, eliminates `output_value_clones`, and improves release latency to about `p50 424 us` / `p95 649 us` with clean semantics.
- `trust-runtime bench project` now keeps VM profiling enabled by default while leaving the experimental tier-1 specialized executor opt-in via `--tier1`, so normal speed baselines stay on the primary VM path and diagnostic tier-1 cache/compile/deopt evidence is still available when explicitly requested.
- `trust-runtime bench project` and `scripts/runtime_motion_benchmark_breakdown.sh` now also surface tier-1 compile-failure reasons alongside deopt reasons, so remaining specialization misses can be attributed to exact unsupported instruction/op buckets instead of only a generic `compile_failures` count.
- The experimental tier-1 VM path now also compiles `LoadRefAddr` and `CallNative`, and widens binary specialization to the generic non-DINT fallback path (including `AND`/`OR`), so the motion benchmark matrix now reaches `compile_failures = 0` / `deopts = 0` in tier-1 profiling runs while preserving clean motion semantics.
- VM ref-binary execution now reuses the first borrowed ref/const read on non-DINT fallback in both the interpreted register executor and the tier-1 executor, removing duplicate storage reads and bogus clone accounting on BOOL/REAL/UINT/WORD-style ref binaries; the rebuilt motion breakdown improves over the tier-1-complete state (`dynamic_refs 34.834 us`, `full_demo 478.836 us`) while preserving clean motion semantics.
- The register VM and experimental tier-1 executor now lower and execute `LOAD_SUPER (0x24)`, so method receiver / `SUPER` call paths no longer fall back through `unsupported_opcode_0x24` on the current-code benchmark surface.

### Changed

- `trust-runtime test` now prepares one runtime per project and reuses it across discovered cases instead of recompiling the whole ST project for every case, which removes false timeout growth on larger OSCAT fixtures.
- truST now accepts direct `CHAR_TO_BYTE` / `WCHAR_TO_WORD` conversion-style helpers, and the shipped OSCAT `CODE()` helper now uses that fast path instead of a brute-force byte scan; the OSCAT core fixture is back to passing at the default `--timeout 5`.
- `trust-runtime` is now VM-only in production: the `legacy-interpreter` feature, interpreter backend dispatch seam, and interpreter runtime paths are gone; CLI/config startup selection still accepts `vm` and now rejects `interpreter` explicitly.
- Removed `trust-runtime bench execution-backend` and the interpreter differential gate flow; VM-only syntax-corpus, determinism/reliability, and production-backend guard scripts now carry the runtime evidence path.
- Hardened `scripts/runtime_vm_syntax_corpus.sh` so the default corpus skips stale missing fixture folders instead of aborting halfway through a benchmark run.
- Runtime docs/diagrams/specs now describe `program_model` plus bounded `helper_eval` helpers as the surviving support surface around the VM, and executor internals under `src/eval/**` are confined to test-only coverage.
- Rewrote the user-facing PLCopen motion documentation so the single-axis demo README now explains the example itself and the main library guide now serves as a real reference manual with package wiring, datatype purpose tables, and per-function-block input/output coverage.

- Structured Text `FUNCTION_BLOCK` calls now reuse the previously stored `VAR_INPUT` value when an input argument is omitted, with first-call fallback still using the declared initializer or IEC default; this closes the motion-library command-update semantics gap without inventing separate runtime rules.
- Structured Text indexing now supports nested `field[index]` / `index.field` chains and character indexing on `STRING` / `WSTRING`, so OSCAT-style `LANGUAGE.DIRS[ly, i]` access and IEC string element forms compile and execute without fallback workarounds.
- Web IDE build/deploy behavior now matches CLI semantics in unified shell flows: build tasks use project-root `--project` resolution, deploy normalizes `src/`-prefixed source paths to avoid nested `src/src` writes, and online connection defaults now seed same-origin host/port for faster standalone startup.
- Web IDE header controls now use a compact primary toolbar (`Open`, `Save`, `Build`, `Deploy`) with overflow menu actions for lower visual clutter while preserving the full command set.
- VS Code HMI widget-navigation source scanning fallback was merged on top of current mainline traversal logic so declaration resolution stays deterministic for `.st`/`.pou` projects after main-branch merge.
- MP-060 production backend policy now enforces VM-only startup for `trust-runtime run`/`play`: `--execution-backend` accepts `vm` only, `runtime.execution_backend='interpreter'` is rejected by runtime config validation, omitted backend config defaults to `vm`, and run/play startup now fails fast when a legacy interpreter backend is present in runtime bundle settings.
- MP-060 setup/wizard runtime templates now emit `runtime.execution_backend = "vm"` by default so newly generated `runtime.toml` files validate and start under VM-only production policy.
- MP-060 no-bundle and source-compiled runtime startup now always applies compiled bytecode before execution backend selection so VM-only startup has loaded bytecode metadata in both project and IDE-shell startup flows.
- MP-060 interpreter internals are now explicitly feature-gated behind `legacy-interpreter` for parity-only workflows (differential/benchmark/debug parity tests), while default production builds remain VM-only.
- MP-060 runtime/harness defaults now execute through VM paths with automatic bytecode materialization from runtime metadata when VM bytecode has not been preloaded, preventing implicit interpreter execution in default runtime/test flows.
- MP-060 CI now enforces a production backend leak guard (`scripts/runtime_vm_production_backend_guard.sh`) to fail if run/config/control/template/runtime-default surfaces regress to interpreter references.
- MP-060 migration/docs/checklists updated for restart execution: added detailed recovery board `docs/internal/testing/checklists/mp-060-vm-restart-recovery-checklist.md` and updated backend migration/spec/master-plan text to reflect interpreter test-oracle-only policy.
- MP-060 post-C1 recovery pass implemented for register VM hot path (`P0 -> P2 -> P1`): boxed large runtime value variants (`Array`/`Struct`/`Enum` with unboxed `Reference`), added extended register-op fusion (`BinaryRefToRef`, `BinaryRefConstToRef`, `BinaryConstRefToRef`, `CmpRefConstJumpIf`) with tier-1 specialized-executor support, and added consume-aware per-block register read paths; refreshed locked `mp-060-corpus-v3` 3-run benchmark evidence and comparison artifacts in `target/gate-artifacts/runtime-vm-bench-v3-post-p0-p2-p1-run1/`.
- MP-060 post-hotpath correction pass: `execution-backend` benchmark corpus upgraded to `mp-060-corpus-v4` with per-cycle loop-state reset in `loop-arith`, VM profile guardrails now assert loop-body execution during measured cycles, benchmark comparison now uses 3-run median-of-runs decision metrics (`scripts/runtime_vm_bench_compare.sh`) with aggregate median derived from per-fixture medians (instead of pooled cross-fixture sample p50), and register-IR `CALL_NATIVE` now reuses a program-level pooled operand stack (removing per-call stack allocation churn) alongside cached per-program read metadata and direct block-id indexing in the register executor.
- Runtime specs/docs synchronized to VM-default production backend policy: `docs/specs/10-runtime.md` and `docs/specs/README.md` now describe bytecode VM execution as the primary runtime path, with interpreter execution documented as legacy `legacy-interpreter` parity/test-oracle flow only.
- `trust-runtime plcopen import` now defaults to native CODESYS/TwinCAT-style global-list materialization: file-scope GVLs stay as `VAR_GLOBAL` files, `qualified_only` lists import as namespaced GVLs, and mandatory `VAR_EXTERNAL` injection is no longer the default import shape. A strict adapter mode remains available for wrapper + injected-`VAR_EXTERNAL` reshaping when external consumers need it.
- The canonical PLCopen motion demo now initializes `MC_Constants()` once during startup instead of republishing its outputs every scan, so the reference benchmark reflects the cheaper steady-state usage pattern already documented by the library guide.
- Register-IR hotpath coverage now includes `LOAD_SELF (0x23)`, complex local-ref execution, and CASE block-entry stack modeling, project-bench VM hot-block reports resolve readable POU names, and lowering-cache fallbacks now preserve the failing POU name plus the original lowering error message. On the PLCopen motion demo this removes the old `unsupported_opcode_0x23`, `complex_local_ref_path`, and `Main` lowering-error buckets and keeps the release gate comfortably under the 10 ms cycle budget with `0` VM fallbacks.

### Fixed

- Runtime and VM execution now handle nested field/index lvalue chains, string and WSTRING dynamic indexing, split-local/initializer parity, and debugger force/release on direct instance fields consistently across harness, VM, helper-eval, and debug-adapter paths.
- HIR/syntax now parse and type-check `TIME()` as a zero-argument builtin, allow `STRING`/`WSTRING` indexing in expressions, accept string `CASE` labels and fixed-length string comparisons, and document the shipped implementer conversion extensions (`TIME_TO_DWORD`, `DWORD_TO_TIME`, `CHAR_TO_BYTE`, `WCHAR_TO_WORD`).
- `trust-ide` / `trust-lsp` now surface the shipped pointer/`ARRAY[*]`/IEC `CONSTANT` semantics consistently across completion, hover, semantic tokens, signature help, workspace symbols, inline values, refactor const-analysis, and constant-section diagnostics instead of drifting behind the HIR fixes.
- Project analysis now honors cross-file `VAR_GLOBAL CONSTANT` integer expressions in type-length contexts such as `STRING[MaxLen]`, and the OSCAT BASIC CRC/buffer helpers now use the shipped `ARRAY[*]` / `POINTER TO ARRAY[*]` compatibility forms without giant caller-side ceremony arrays.
- IEC `CONSTANT` qualifier handling now preserves parameter/`VAR_TEMP` identity in HIR, rejects writes through the shared `ConstantModification` path, blocks function block instances in `CONSTANT` sections, and keeps parameter/`VAR_TEMP` declarations out of compile-time constant-expression evaluation.
- Bytecode VM lowering now executes `EXIT` and `CONTINUE` correctly in `FOR`, `WHILE`, and `REPEAT` loops instead of rejecting those statement paths through the generic C5 fallback.
- HIR comparison rules now treat same-family `STRING` and `WSTRING` values as comparable regardless of declared max length, so fixed-length values such as `STRING[1]` and `WSTRING[1]` compare cleanly against compatible literals and variables.
- Pointer support docs and regression coverage now explicitly lock the supported string-pointer path (`ADR(str)` with typed dereference/indexing such as `p^[i]`) while keeping raw byte-array reinterpret casts and pointer arithmetic outside the supported runtime model.
- `CASE` now accepts `STRING` / `WSTRING` selectors and literal labels in runtime/lowering paths, with an explicit `IEC-DEC-024` policy that string subranges remain rejected as an implementer decision for the ambiguous IEC area.
- FUNCTION_BLOCK omitted `VAR_INPUT` calls now follow the documented runtime semantics: instance creation seeds declared input initializers, first omitted calls observe that seeded value, and later omitted calls reuse the stored instance input instead of reevaluating the declaration on every call.
- `trust-runtime bench project` now applies the project's configured runtime execution backend and the motion example bench gate accepts `TRUST_MOTION_MAX_OVERRUNS` / `TRUST_MOTION_P95_MAX_US` overrides for hardware-specific performance gating.
- `trust-runtime test` now executes discovered `TEST_PROGRAM`s even when a project includes a `CONFIGURATION`, while normal configured runtime builds continue to register only configured programs.
- `trust-runtime test --project` now compiles sources from local `[dependencies]` packages in addition to the project's own `src/`, so extracted Structured Text libraries can be consumed from real package roots instead of living inside test fixtures.
- `trust-runtime test` now preloads bytecode before per-case timeout accounting, so filtered project runs do not burn their timeout budget on the first lazy VM-module build.
- Multi-file HIR import now preserves configuration/resource global lookup bindings, so vendor-parity bare global access continues to resolve after splitting projects across source files.
- Runtime assignment writes now preserve declared scalar storage types in both interpreter and VM paths, so loop counters keep their declared `INT`/`UINT`/etc. representation and exact-source conversion calls such as `INT_TO_DINT(...)` no longer fail inside `FOR` and `WHILE` loops.
- Structured Text infix bitwise operators now accept `BOOL`/`ANY_BIT` operands with the same widening behavior as the standard `AND`/`OR`/`XOR`/`NOT` functions, and `&` now type-checks as the `AND` synonym instead of falling through as `UNKNOWN`.
- `VAR_STAT` now executes with documented vendor semantics in the runtime: function statics persist across calls, method statics persist per instance/method, and instance-bearing scopes treat `VAR_STAT` as persistent instance storage.
- VM execution now initializes declared local storage before user bytecode runs, so local declaration initializers and split-style standard-library writes to function-local outputs behave the same way in runtime and VM paths.

### Added

- Shipped the first PLCopen Motion library profile in Structured Text: Part 1 single-axis classic core, Part 1 synchronization cam/gear subset, Part 4 coordinated-motion core subset, and Part 5 homing core subset, with a compliance matrix, user-facing guide, deferred-surface guards, and deterministic ST conformance fixtures.
- truST now accepts namespaced vendor-style GVL declarations (`NAMESPACE ... VAR_GLOBAL ... END_NAMESPACE`) and resolves qualified access such as `GVL.shared` in both HIR and runtime execution.
- truST now records vendor-parity global access explicitly: top-level GVLs, namespaced GVLs, and direct global access without mandatory `VAR_EXTERNAL` are documented and covered by regression tests.
- `trust-harness` now provides a lightweight JSON-line test driver for compiled ST programs, including `cycle.dt_ms` virtual-time advancement and typed `TIME`/`LTIME` watch output for timer-oriented automation.
- LSP/type-checker numeric hazard warnings now flag floating-point equality/inequality comparisons (`W013`) and `DIV`/`MOD` expressions with literal zero divisors (`W014`), with a dedicated `warn_numeric_hazards` diagnostics toggle for vendor/workspace tuning.
- VS Code SFC visual editor integration (IEC 61131-3 style step/transition canvas, runtime panel wiring, and bundled EtherCAT Snake SFC examples) is now included in mainline extension workflows.
- MP-060 Phase A execution-backend controls for `trust-runtime`: new `runtime.execution_backend` config (`interpreter|vm`), new CLI override `--execution-backend=interpreter|vm` for `run`/`play`, startup backend-selection log event, backend mode/source fields in control `status`/`config.get`, and Prometheus backend info metric (`trust_runtime_execution_backend_info`). `vm` selection is intentionally fail-fast until VM execution lands in the next MP-060 phase.
- MP-060 Phase B VM core for `trust-runtime`: `runtime.execution_backend='vm'` now runs a real bytecode executor with deterministic program-counter dispatch, operand and call/frame stacks, trap/deadline/budget enforcement, stable trap-to-`RuntimeError` mapping, and slot/index-based VM hot-path access with symbol/source mapping tables preserved for external/debug name-based surfaces. Interpreter remains the default backend.
- MP-060 Phase C1 call-parity rollout for `trust-runtime`: VM codegen/dispatch now emits and executes `CALL_NATIVE` (`kind/symbol/arg_count`) for function/FB/method/stdlib paths, routes builtin FB and stdlib behavior through the same native-call convention, preserves named/default/IN_OUT parity, and replaces C1-required silent NOP call fallthrough with deterministic compile/validate/runtime failures.
- MP-060 Phase C2 string parity rollout for `trust-runtime`: VM bytecode lowering/execution now supports `STRING`/`WSTRING` literals (including stdlib call arguments and comparison expressions), adds deterministic UTF validation for string const payloads, and removes remaining C2 silent-NOP fallback behavior for string-literal control-flow lowering paths.
- MP-060 Phase C3 OOP parity rollout for `trust-runtime`: VM lowering/runtime now supports explicit `THIS`/`SUPER` receiver expressions (`LOAD_SELF`/`LOAD_SUPER`) for method/native-call paths, adds deterministic `SUPER` trap semantics, and extends differential/opcode conformance coverage for method/interface dispatch including malformed method receiver payload rejection.
- MP-060 Phase C4 reference/deref parity rollout for `trust-runtime`: VM lowering/runtime now supports `REF(...)` lowering, dereference execution (`LOAD_DYNAMIC`/`STORE_DYNAMIC`), and nested field/index chain composition (`REF_FIELD`/`REF_INDEX`) with deterministic validator/runtime failures for invalid field-string indices and malformed dynamic-reference operands.
- MP-060 Phase C5 parity rollout for `trust-runtime`: VM lowering/runtime now supports `SIZEOF(type)`/`SIZEOF(expr)` (`SIZEOF_TYPE`/`SIZEOF_VALUE`), adds deterministic validator/runtime opcode failure coverage for C5 paths, and replaces silent VM-lowering fallthrough for remaining C5 edge statements (`?=`/`JMP`/`RETURN`/`EXIT`/`CONTINUE`) with deterministic compile-time rejection.
- MP-060 Phase C audit follow-up hardening for `trust-runtime`: added per-subphase negative-path interpreter-vs-VM differential coverage (C1..C5), extended C2 string stdlib parity coverage to include both `FIND` found/not-found paths, and added a deterministic `SIZEOF_TYPE` recursion-depth guard (`max depth 128`) for deep non-cyclic type-table graphs.
- MP-060 Phase D debug/observability parity for `trust-runtime`: VM execution now resolves statement stops through bytecode debug-map `(pou_id, pc)` source mappings, preserves breakpoint/step and debug-write workflow parity with interpreter mode, and keeps backend metrics/runtime-event observability contracts stable across interpreter and VM backends.
- MP-060 Phase E online-change groundwork for `trust-runtime`: hot-reload semantics are now centralized in a dedicated runtime online-change contract (`apply_online_change_bytes`) with cycle-boundary swap handling, warm-restart entrypoint invalidation, retain/global/instance migration coverage, deterministic invalid-bytecode diagnostics, and explicit startup-only backend-switch diagnostics for live `config.set` requests.
- MP-060 Phase E rollout-gate closure: added a dedicated backend migration note (`docs/guides/RUNTIME_EXECUTION_BACKEND_MIGRATION.md`) covering CLI/config rollback controls and the two-release interpreter compatibility window policy, plus explicit release-evidence gate documentation for `version-release-guard` artifact aggregation.
- MP-060 benchmark groundwork: added `trust-runtime bench execution-backend` (`mp-060-corpus-v1`) for interpreter-vs-VM cycle latency/throughput comparison, plus CI evidence capture via `scripts/runtime_vm_bench_gate.sh` and published artifact/docs pointers (`gate-artifacts/runtime-vm-bench/**`, `docs/internal/reports/mp-060-runtime-vm-benchmark-corpus.md`).
- MP-060 Phase C hotspot profiling: added runtime-scoped register-VM profile controls/snapshot APIs and extended `trust-runtime bench execution-backend` fixture reports with `vm_profile` details (register execution/fallback counters, fallback reasons, top hot blocks, and profiling-overhead ratio).
- MP-060 Phase D tier-1 specialized register-executor pilot: added an experimental hot-block specialization tier in the register VM (guarded compile subset for arithmetic/compare/branch, deopt-to-register-interpreter fallback, cache capacity/eviction controls, and bytecode-reload invalidation), plus benchmark artifact fields for tier-1 specialized-executor counters/deopt reasons and focused unit coverage for startup/deopt/cache behavior.
- MP-060 register executor now caches lowered register IR per `(module,pou)` with runtime-owned invalidation on bytecode reload, eliminating per-cycle relowering and block-map allocation; benchmark reporting now includes lowering-cache hit/miss/eviction stats and uses expanded `mp-060-corpus-v2` fixtures (`call-binding`, `string-stdlib`, `refs-sizeof`, `loop-arith`).
- IDE settings coverage expanded for standalone/offline authoring: new fields for control endpoint/resource name, runtime cloud profile, extended OPC UA options, and observability options.
- IDE settings discoverability improvements: `/ide/settings` now opens in an `All Settings` view by default and adds filter-by-name/key search so MQTT/PLC/TLS/realtime/debug configuration is immediately accessible.
- IDE settings reliability hardening: `/ide/settings` now includes standalone `simulation.toml` controls (`simulation.enabled`, `simulation.seed`, `simulation.time_scale`), shows active filter summary/clear controls to prevent hidden-field confusion, and limits live `config.set` writes to backend-supported keys to avoid false save failures.
- IDE settings navigation hardening: hardware/quick-action jumps now auto-clear stale settings filters and auto-fallback to the owning category (or `All Settings`) so target fields are always visible when jumping to PLC/MQTT/TLS/realtime/debug settings.
- IDE I/O settings completeness hardening: both Settings and Hardware editors now preserve `use_system_io` from loaded I/O configs instead of silently forcing it to `false` on save.
- Default standalone Web IDE demo project (`examples/web_ui_complete_project`) now includes a multi-driver `io.toml` (Modbus TCP, MQTT, EtherCAT mock, GPIO, Simulated) so Hardware and Settings communication flows are populated immediately.
- IDE communication workflow hardening in the unified shell:
  - Settings now shows inline JSON examples for `runtime_cloud.wan.allow_write_json` and `runtime_cloud.links.transports_json`.
  - Realtime link rule parsing now accepts `source/target`, `from/to`, and `pattern` aliases for faster authoring while still writing valid `runtime.cloud.links.transports` TOML entries.
  - Runtime-cloud link transport support now spans `realtime`, `zenoh`, `mesh`, `mqtt`, `modbus-tcp`, `opcua`, `discovery`, and `web` across runtime schema validation, config APIs, and the Hardware/Settings editors.
  - Hardware `Add Link` flow now uses an in-UI transport picker (all 8 transports) plus guided source/target hinting instead of raw text prompt input.
  - Hardware inspector now deep-links all communication/runtime-cloud modules to their exact Settings fields.
  - Added MQTT connectivity probe route `POST /api/io/mqtt-test` and Hardware inspector `Test Connection` support for MQTT alongside Modbus TCP.
  - Hardware communication cards now also project `runtime.cloud.wan.allow_write` as `Cloud WAN Access`.
- IDE mobile responsiveness hardening for `/ide`: stacked layout now keeps the main workspace visible on narrow screens and avoids sidebar-only view lockups.
- Fixed settings schema mismatch for Modbus `on_error`: UI options now match runtime driver-supported values (`fault`, `warn`, `ignore`).
- Unified Web IDE shell with tab-based navigation (Code | Hardware | Settings | Logs):
  - Root `/` and `/setup` now redirect to `/ide`.
  - Legacy fleet web routes removed from the runtime web server (`/fleet`, `/app.js`, `/styles.css`, `/runtime-cloud-utils.js`, and legacy `/modules/*` fleet assets).
  - Added standalone browser IDE startup via `trust-runtime ide serve --project <path> --listen <addr>` (with deprecated alias `trust-runtime config-ui serve`).
  - Tab deep links: `/ide/code`, `/ide/hardware`, `/ide/settings`, `/ide/logs`.
  - Keyboard navigation: Ctrl+1..4 for tab switching, URL history with pushState.
  - Hardware tab: Cytoscape topology canvas, module palette with drag-and-drop, auto-address allocation, property editor with per-driver forms (Modbus TCP, MQTT, GPIO, EtherCAT), address map table with conflict/unused/used-in-code highlighting, live I/O value polling.
  - Hardware tab now auto-hydrates from workspace `io.toml` via new `/api/ide/io/config` endpoint so configured hardware/communication drivers appear on the canvas immediately (including deep-link `/ide/hardware` loads).
  - Hardware tab now renders a full workspace layout (summary KPI cards, surface controls, driver/runtime communication cards, persistent inspector), fixes tab-shell row sizing so content fills the page (status bar no longer consumes the main viewport), and hydrates runtime communication sections from `runtime.toml` into the hardware communication view.
  - Hardware tab now also hydrates from `runtime.toml` communication sections when `io.toml` is missing, so standalone projects still show communication modules on the canvas instead of an empty hardware view.
  - Settings tab: categorized form (General, Execution, Retention, Communication, Security, Advanced), direct runtime.toml editing, settings export/reset.
  - Logs tab: unified log view with severity/source/text filtering, alarm acknowledgment, CSV export.
  - Logs CSV export filenames now follow the user-story format exactly: `truST-logs-YYYY-MM-DD-HHMMSS.csv`.
  - Connection dialog: mDNS discovery scan, manual connect, recent connections.
  - Deploy flow: error gating, confirmation, sync status indicator (in-sync/modified/not-deployed).
  - Debug integration: breakpoints with gutter decorations, debug toolbar (Continue/Step Over/Step Into/Step Out/Stop), variables/call stack/watch panels, live inline value annotations, I/O force/unforce with warning banner.
  - New IDE modules: `ide-tabs.js`, `ide-hardware.js`, `ide-online.js`, `ide-debug.js`, `ide-settings.js`, `ide-logs.js`.
  - New CSS: `ide-06.css` (tabs), `ide-07.css` (hardware), `ide-08.css` (online/debug), `ide-09.css` (settings/logs).
  - 5 new unified shell contract integration tests in `web_ide_integration_part_09.rs`.
  - IDE session bootstrap now recovers reliably in standalone mode by persisting/reusing session tokens across reloads and evicting the stalest inactive session when the session cap is reached (prevents repeated `/api/ide/session` 429 lockouts during active web-UI development).
  - Hardware canvas labels now compact long address ranges and enable wrapped node text to keep TOML-derived module/address data legible on the graph.
  - Hardware Cytoscape theming now resolves IDE CSS variables to concrete colors and updates on theme changes, eliminating runtime style warnings while preserving light/dark parity.
  - Hardware tab visual redesign now uses a canvas-first layout with a much larger default graph surface, collapsible inspector/driver panels, in-canvas communication legend/filter controls, center/fullscreen actions, and SVG-based component iconography (replacing emoji palette glyphs) for a modernized runtime topology editing workflow.
  - Hardware node art polish pass now upgrades node cards with richer per-protocol gradients/glows, glass label treatment, and higher-fidelity SVG chip icons (including distinct analog/communication module icon rendering) without changing interaction workflow.
  - Hardware node visuals now render generated card-style SVG backgrounds per node (runtime/endpoint/module variants with accent rails + icon chips) and enforce higher minimum zoom/readability so topology labels/icons stay legible at dense graph sizes.
  - Hardware canvas visual hardening now uses Cytoscape-safe SVG icon rendering for node cards (fixes missing icon layers in Chromium canvas rendering) and retunes card dimensions/colors for a cleaner production graph look.
  - Hardware tab chrome simplification removes summary KPI cards, runtime/fabric control strip, and active-link/legend toolbar from the canvas stage so palette + topology become the primary workflow surface.
  - Hardware node context menu now de-duplicates endpoint settings navigation: endpoint nodes expose only one settings jump action while runtime nodes keep separate runtime and communication settings actions.
- Runtime cloud multi-host topology demo pack:
  - Added `examples/runtime_cloud/multi_host_topology_demo/` with `bootstrap.sh`, `start-5.sh`, and `stop-5.sh`.
  - Added per-runtime sample configs for 5 runtimes (`runtime-a`..`runtime-e`) with mixed `mqtt`, `modbus-tcp`, `ethercat`, `simulated`, and `loopback` I/O profiles.
  - Added preloaded manual topology seed (`topology-devices.runtime-a.json`) to exercise host containers, manual devices, and module-slot rendering during UI testing.
  - Changed bootstrap defaults to TOML/API-first: manual topology JSON and legacy link-transport JSON are now opt-in only (`TRUST_TOPOLOGY_DEMO_SEED_MANUAL=1`, `TRUST_TOPOLOGY_DEMO_SEED_LINK_JSON=1`).
  - Added TOML runtime-cloud link preference support via `runtime.cloud.links.transports` and seeded demo link lanes from `runtime-a.toml`.
  - Added TOML discovery host grouping via `runtime.discovery.host_group` and host-group-aware runtime-cloud same-host evaluation for localhost multi-host demos.
- Runtime cloud topology visual map in Web UI:
  - Reworked `Network -> Topology` into a graphical runtime map with box nodes and connection lines.
  - Added at-a-glance status-focused KPIs (`online`, `degraded`, `offline`, link health) for operator use.
  - Moved diagnostic-heavy controls (filters, tables, timeline, rollout/config control surfaces) into a collapsed advanced section to reduce default clutter.
- Runtime cloud edge transport switching in Fleet view:
  - Clicking topology links or edge cards now allows direct `Realtime` / `Zenoh` transport switching.
  - Added `/api/runtime-cloud/links/transport` with same-host validation for `realtime` routing preferences.
  - Fleet topology state now reflects selected link transport (`t0_hard_rt` overlay for realtime links).
  - Promoted topology to primary workflow with in-canvas plane/keyspace/issue filters, contextual node/edge inspector, and batch action bar for selected runtimes.
  - Moved compact timeline controls into topology map while keeping deep diagnostics under the advanced drawer to reduce default UI clutter.
  - Communication lines now render per-lane overlays (Zenoh + optional Realtime), with left-click showing only critical lane status/settings and right-click lane actions for add/remove realtime communication.
- Fleet topology deterministic ownership slots:
  - Fleet topology now renders runtime-owned driver nodes (`mqtt`, `modbus`, `ethercat`, `opcua`, `gpio`) between runtimes and endpoints.
  - Host assignment is enforced for operational endpoints: remote/discovered endpoints attach to deterministic external host containers instead of free-floating.
  - Topology edit flow now requires host placement for manual devices and blocks host removal that would orphan hosted devices.
  - Runtime cards now reserve deterministic in-card driver slot zones and driver cards render as compact protocol chips anchored to those fixed slots.
  - Empty host containers are hidden in normal view when they have no runtime or endpoint children.
  - Runtime/device cards now render a single border layer (removed Cytoscape outer border) to match card design and avoid double-outline artifacts.
  - Runtime card subtitle no longer duplicates health state when the status badge already shows it.
  - Driver slot placeholders now render without static protocol text; labels appear only on occupied slots.
  - Driver nodes are now non-draggable to preserve deterministic runtime slot placement.
  - Device edges are normalized to bind via runtime-owned driver nodes (including manual runtime->endpoint edges), and render as direct source-to-target lines from the driver chip.
  - Runtime cards now render driver summary text in the body while driver chips are laid out as a single deterministic bottom row (no multi-row chip stacks).
  - Driver chips now include compact protocol context subtitles (for example broker/address/adapter/internal) for faster visual inspection.
  - Host compound labels are pinned inside host containers for clearer ownership headings.
  - Carousel priming now guards missing `runtimeCloudState` to prevent early-page-load `ReferenceError` in browser console.
  - Fleet graph position caches are now layout-versioned and auto-relayout when stale pinned positions overlap runtime cards.
  - External-only host groups now use deterministic anchor lanes so remote endpoint hosts do not collapse onto `(0,0)` and overlap each other.
  - Runtime-internal I/O drivers (`simulated`, `loopback`) now render as deterministic runtime-owned driver cards.
  - Comms add-driver flow now includes `loopback` alongside `simulated` for runtime-internal I/O driver configuration.
  - Modbus canonical endpoint normalization now handles `address` values that already include `:port` (prevents malformed duplicate-port canonical keys).
  - Demo topology seed data now keeps manual endpoints hosted (no hostless manual operational devices).
  - Added config-mode runtime topology writeback endpoints: `POST /api/config-ui/runtime/create` and `POST /api/config-ui/runtime/delete`.
  - Topology edit UI now supports runtime create/remove actions in edit mode and routes adapter actions through TOML-backed config flows.
  - Added config-ui live connection manager APIs (`GET/POST /api/config-ui/live/targets`, `POST /api/config-ui/live/targets/remove`, `POST /api/config-ui/live/connect`, `GET /api/config-ui/live/state`) with in-memory target profiles and read-only runtime-cloud polling.
  - Added config-ui runtime lifecycle provider APIs (`GET/POST /api/config-ui/runtime/lifecycle`) for runtime status/probe plus managed start/stop/restart actions.
  - Config-mode `/api/runtime-cloud/state` now supports read-only live overlay of node/edge health from connected runtime-cloud snapshots without changing TOML-derived topology ownership/layout.
- Added config-ui integration tests for runtime create/delete and ST/runtime conflict-safe writeback paths.
- Removed active topology-devices overlay route dependency; runtime/config topology remains TOML/API-driven.
- PLCopen CODESYS global/folder parity:
  - `trust-runtime plcopen import` now imports CODESYS `addData/globalVars` into ST `VAR_GLOBAL` sources (plaintext-first with variable-node synthesis fallback).
  - CODESYS `addData/projectstructure` object trees are now used to place imported POUs/GVLs into mirrored `src/` subfolders (for example `src/Application/...`).
  - `trust-runtime plcopen export` now emits deterministic CODESYS `globalVars` and `projectstructure` metadata for ST POUs + GVL files.
  - Import/export JSON reports now include global-list and project-structure counters (`discovered/imported_global_var_lists`, folder/object-node counts).

### Changed

- `libraries/oscat/src` and the OSCAT core/negative conformance fixtures now follow the OSCAT manual chapter structure (`03_data_types` through `26_list_processing`) instead of a flat source bucket, so continued porting can proceed chapter-by-chapter with tests in the matching chapter first.
- VS Code statechart automated coverage:
  - Added editor lifecycle test coverage to verify running statechart sessions are cleaned up when a custom editor panel is disposed.
  - Added state machine engine behavior tests for awaited hardware action ordering and fail-closed guard evaluation paths.
  - Added runtime client timeout cleanup coverage to ensure request listeners are removed on timeout/error.
- Web IDE project selection flow:
  - Added `/api/ide/project` and `/api/ide/project/open` to query/switch active project root at runtime.
  - Added no-bundle startup support for `trust-runtime run` so `/ide` can start first and open a project folder from the browser.
  - Added integration coverage for no-bundle project-open flow in `crates/trust-runtime/tests/web_ide_integration.rs`.
- Web IDE workspace scope hardening:
  - Changed IDE file/tree scope from fixed `<project>/src` to full active project root with hidden/system path filtering.
  - Added project-aware command execution guard so build/test/validate require an active project selection.
  - Added task status timestamps in `/ide` task panel (`started`/`finished`) to satisfy workflow traceability requirements.

- Web IDE full implementation closure for `/ide`:
  - Added workspace tree + filesystem mutation endpoints and UI flows (`/api/ide/tree`, `/api/ide/fs/create`, `/api/ide/fs/rename`, `/api/ide/fs/move`, `/api/ide/fs/delete`) with conflict-safe errors.
  - Added IDE navigation/search surface: quick open, workspace text search with include/exclude globs, file/workspace symbol search, and command-palette actions.
  - Added project-aware language navigation endpoints and UI wiring for go-to-definition, references, and rename (`/api/ide/definition`, `/api/ide/references`, `/api/ide/rename`) with cross-file analysis context.
  - Added build/test/validate task orchestration endpoints (`/api/ide/build`, `/api/ide/test`, `/api/ide/validate`, `/api/ide/task`) with streaming output, parsed source-location links, and retry UX in the browser IDE.
  - Added browser IDE format endpoint and command flow (`/api/ide/format`) so formatted content can be applied from the editor command surface.
  - Added filesystem mutation audit trail endpoint (`/api/ide/fs/audit`) and health counters (`fs_mutation_events`) for IDE security observability.
  - Added dedicated Web IDE contract/performance/security integration coverage in `crates/trust-runtime/tests/web_ide_integration.rs` and parser/unit coverage in `crates/trust-runtime/src/web.rs` + `crates/trust-runtime/src/web/ide.rs`.
  - Added a standalone static browser demo at `docs/demo/` with all 7 LSP features running fully client-side through WebAssembly, plus GitHub Pages deployment workflow (`.github/workflows/demo-pages.yml`) for client sharing.
  - Added Browser IDE runtime docs for `/ide` in `docs/guides/WEB_IDE_FULL_BROWSER_GUIDE.md` and linked them from `README.md` and `docs/README.md`.
- EtherCAT bring-up examples:
  - Added `examples/ethercat_ek1100_elx008_v2/` for EK1100 + EL2008-only hardware chains with a validated 8-output snake pattern.
  - Added helper run scripts and docs for real-NIC bring-up and deterministic mock-mode fallback.
- HMI Phase 0 scaffold start:
  - Added runtime scaffold engine APIs `scaffold_hmi_dir` and `scaffold_hmi_dir_with_sources` in `crates/trust-runtime/src/hmi.rs` to generate deterministic `hmi/` directory artifacts from source metadata.
  - Added `trust-runtime hmi init` CLI workflow (`crates/trust-runtime/src/bin/trust-runtime/hmi.rs`) with style selection (`industrial|classic|mint`) and deterministic generation summary output.
  - Added `trust-lsp.hmiInit` `workspace/executeCommand` support in `crates/trust-lsp/src/handlers/commands.rs`, reusing runtime scaffold generation from workspace sources and returning structured summary payloads.
  - Added VS Code LM tool `trust_hmi_init` in `editors/vscode/src/lm-tools.ts` and `editors/vscode/package.json`, routing HMI scaffold generation through the new LSP command path.
  - Added Phase 0 runtime scaffold tests for external symbol filtering, widget mapping by writability/type, deterministic output stability, and repeated-instance section grouping.
  - Converted `docs/internal/testing/checklists/hmi-complete-implementation-checklist.md` into a tracked execution board with active Phase 0 lanes and work item IDs.
- HMI Phase 1.6 live descriptor refresh:
  - Added runtime descriptor watcher (`notify`) with debounce for `hmi/*.toml` updates and safe hot-reload without runtime restart.
  - Added `schema_revision` to `hmi.schema.get` payloads and wired in-memory revision bumps on successful descriptor reload.
  - Added fail-safe invalid descriptor handling that retains the last known-good schema and prevents runtime crashes on bad TOML edits.
- HMI Phase 2.1 section layout foundation:
  - Added schema-driven section rendering in web HMI (`page.sections`) with responsive 12-column spans for sections/widgets.
  - Added explicit fallback to legacy group card rendering when section metadata is absent, preserving existing trend/alarm/dashboard behavior.
  - Added widget renderers for `gauge`, `sparkline`, `bar`, `tank`, `indicator`, `toggle`, and `slider` with value updates and existing `hmi.write` integration for writable controls.
  - Added transition/micro-animation behavior for gauge/bar/tank/indicator/toggle updates and dark-mode CSS variable overrides via `prefers-color-scheme: dark`.
  - Added integration coverage for section layout assets, renderer assets, and schema span metadata used by the web renderer.
- HMI Phase 2.4 live transport foundation:
  - Added `/ws/hmi` websocket endpoint in the embedded web server for push updates (`hmi.values.delta`, `hmi.schema.revision`, `hmi.alarms.event`).
  - Added web HMI client websocket transport with reconnect/backoff and automatic fallback to HTTP polling when websocket is unavailable.
  - Added schema live-refresh behavior in the web client to re-fetch full `hmi.schema.get` payloads on `schema_revision` events.
  - Added integration coverage for websocket event flow and export transport metadata (`/ws/hmi` route + `config.ws_route`).
  - Added websocket hardening integration gates for local latency SLO budgets (p95/p99), forced socket-failure polling recovery, and reconnect churn stability.
- HMI Phase 2.5 process-page foundation:
  - Added process-page schema support (`kind = "process"`) with page-level SVG asset and bind metadata (`svg`, `bindings`) in runtime HMI contracts.
  - Added secure SVG asset serving route (`/hmi/assets/<svg>`) with path containment checks and SVG-only file restrictions.
  - Added process bind parsing guardrails for safe selector/attribute usage plus optional `format`, `map`, and `scale` transform contracts.
  - Added web client process renderer path to inline SVG assets and apply live bind updates from HMI value streams.
  - Added integration coverage for process schema metadata, asset serving, and negative filtering of unsafe/missing bind selectors.
- HMI Phase 2 completion hardening:
  - Added renderer-state regression coverage for all new widget kinds (`gauge`, `sparkline`, `bar`, `tank`, `indicator`, `toggle`, `slider`) across null/stale/good value paths.
  - Added responsive breakpoint regression checks for desktop/tablet/mobile class behavior in the web HMI layout engine.
  - Added process asset-pack integrity checks for `hmi/pid-symbols/` licensing/library presence, stable IDs in `hmi/plant.svg` and `hmi/plant-minimal.svg`, and selector alignment in `hmi/plant.bindings.example.toml`.
- HMI Phase 3 core command/tool surface:
  - Added LSP command `trust-lsp.hmiBindings` with workspace-root/file scoping and deterministic bindings catalog payloads (`programs`, `globals`) including qualifier/writable metadata and available constraints (`unit`, `min`, `max`, `enum_values`).
  - Added VS Code LM tools `trust_hmi_get_bindings`, `trust_hmi_get_layout`, and `trust_hmi_apply_patch` with dry-run conflict reporting for descriptor file operations.
  - Added VS Code command registrations `trust-lsp.hmi.init` and `trust-lsp.hmi.refreshFromDescriptor` and integrated refresh invocation from HMI patch application flows.
  - Added VS Code HMI integration coverage for LM tool valid/invalid payload handling and cancellation behavior.
- HMI Phase 3.6/3.7 completion:
  - Added LSP diagnostics for `hmi/*.toml` files with stable codes for parse failures, unknown bind paths, type/widget mismatches, and invalid widget property combinations.
  - Added near-match suggestion hints for unknown binding paths in HMI descriptor diagnostics.
  - Added multi-root LM-tool integration coverage to verify explicit `rootPath` resolution and write routing for `trust_hmi_apply_patch`/`trust_hmi_get_layout`.
- HMI Phase 4 panel refresh foundation:
  - Expanded VS Code HMI preview refresh relevance to `hmi/*.toml` and `hmi/*.svg`.
  - Added debounced filesystem watching of HMI descriptor/assets (`**/hmi/*.{toml,svg}`) so open panels refresh on descriptor edits without forced auto-open behavior.
  - Added section-aware panel rendering (`page.sections` + `widget_span`) for dashboard pages.
  - Added process-page panel rendering (`kind = "process"`) with safe binding updates and local `hmi/*.svg` asset hydration in VS Code preview.
- HMI Phase 5 export bundle foundation:
  - Updated `/hmi/export.json` payload contract to `version = 2`.
  - Added `config.descriptor` to export payloads with the resolved live `hmi/` descriptor content when present (or `null` when running legacy/no-descriptor projects).
  - Added export integration coverage for descriptor-backed bundles and renderer-capability assertions in exported `hmi/app.js`.
- HMI Phase 6 intent-to-evidence foundation:
  - Added VS Code LM tool `trust_hmi_plan_intent` to generate/update deterministic `hmi/_intent.toml` artifacts with operator goals, personas, KPI priorities, and constraints.
  - Added VS Code LM tool `trust_hmi_trace_capture` for deterministic scenario-tagged API-level runtime trace capture (`hmi.schema.get`/`hmi.values.get`) into `hmi/_evidence/<run>/trace-<scenario>.json`.
  - Added VS Code LM tool `trust_hmi_generate_candidates` for scaffold-rules-based deterministic candidate generation and ranking by readability/action-latency/alarm-salience metrics with `candidates.json` evidence output.
  - Added VS Code LM tool `trust_hmi_validate` to run machine-readable HMI validation checks, emit deterministic `hmi/_lock.json`, and write evidence runs under `hmi/_evidence/<timestamp>/`.
  - Added VS Code LM tool `trust_hmi_preview_snapshot` to emit deterministic desktop/tablet/mobile snapshot artifacts under `hmi/_evidence/<run>/screenshots/`.
  - Added VS Code LM tool `trust_hmi_run_journey` for API/event-level operator journey execution (no headless browser requirement) with pass/fail timing metrics in `journeys.json`.
  - Added VS Code LM tool `trust_hmi_explain_widget` for widget provenance reporting (canonical ID, symbol/type, write-policy allowlist state, and contract endpoint mapping).
  - Hardened `trust_hmi_run_journey` with tool-side write guardrails (read-only/disabled/allowlist checks) and machine-readable step codes before issuing runtime `hmi.write` requests.
  - Added validation retention support (`prune` + `retain_runs`, default 10) and evidence pruning behavior in `trust_hmi_validate`.
  - Added integration tests covering intent generation, candidate-ranking determinism, trace capture, validation artifact emission, snapshot output, API/event journey execution, unauthorized-write safety handling, evidence retention pruning, lock determinism, and cancellation handling for the full Phase 6 toolset.
  - Added runtime hardening regressions for write-cycle budget checks, websocket slow-consumer/backpressure stability, malformed process-SVG resilience, and rapid descriptor file-churn no-deadlock behavior.
  - Added default gitignore rule for `hmi/_evidence/` artifacts.
- Direct Siemens TIA source handoff via export adapters:
  - `trust-runtime plcopen export --target siemens` now emits a Siemens SCL sidecar bundle (`<output>.scl/*.scl`) alongside PLCopen XML.
  - Siemens-target export reports now include `siemens_scl_bundle_dir` and `siemens_scl_files[]` for automation and CI evidence.
  - Added dedicated Siemens import tutorial with exact TIA path (`External source files` -> `Add new external file` -> `Generate blocks`): `docs/guides/SIEMENS_TIA_SCL_IMPORT_TUTORIAL.md`.
  - Added runtime + CLI regression coverage for Siemens SCL bundle generation and `PROGRAM` to `ORGANIZATION_BLOCK` conversion.
- VS Code PLCopen import workflow:
  - Added command `Structured Text: Import PLCopen XML` (`trust-lsp.plcopen.import`) for CLI-backed PLCopen project import from the editor.
  - Command flow prompts for input XML + target project folder, validates conflict paths, runs `trust-runtime plcopen import --json`, and offers quick access to the generated migration report.
  - Added VS Code integration coverage for success/cancel/conflict/invalid-input paths in `editors/vscode/src/test/suite/plcopen-import.test.ts`.
  - Added explicit import usage docs in `README.md`, `docs/README.md`, `editors/vscode/README.md`, and PLCopen example READMEs.
- Guided example/tutorial expansion:
  - Added `examples/README.md` as a structured tutorial index with recommended learning order and setup checklist.
  - Added `examples/tutorials/12_hmi_pid_process_dashboard/` as a full process-HMI tutorial with a runnable PLC model, descriptor-driven `hmi/` pages, production-style `kind = "process"` SVG bindings, setpoint/deviation/alarm coverage, and step-by-step build/run/customization guidance (including screenshot/GIF capture workflow).
  - Added a full walkthrough for `examples/filling_line/README.md` (run, I/O mapping, expected outcomes, and tuning exercise).
  - Reworked example READMEs (`plant_demo`, `memory_marker_counter`, `siemens_scl_v1`, `mitsubishi_gxworks3_v1`, `ethercat_ek1100_elx008_v1`, `plcopen_xml_st_complete`, `tutorials`) into detailed step-by-step VS Code setup guides.
  - Added `examples/vendor_library_stubs/` as a tutorial for user-provided vendor library stub indexing (`[[libraries]]`) with Siemens-style sample declarations.
  - Removed `examples/openplc_interop_v1/` (content absorbed into `examples/plcopen_xml_st_complete/`).
  - Removed `examples/browser_analysis_wasm_spike/` (prototype assets moved to `docs/internal/prototypes/browser_analysis_wasm_spike/`).
- OpenPLC Interop v1 deliverable closure:
  - Added dedicated OpenPLC migration guide: `docs/guides/OPENPLC_INTEROP_V1.md`.
  - Added OpenPLC fixture and walkthrough coverage inside the PLCopen ST-complete bundle:
    - `examples/plcopen_xml_st_complete/README.md`
    - `examples/plcopen_xml_st_complete/interop/openplc.xml`
  - Added regression coverage to keep the OpenPLC sample bundle executable in CI:
    - `crates/trust-runtime/tests/plcopen_command.rs`
    - `crates/trust-runtime/tests/tutorial_examples.rs`
- truST Browser Analysis Spike (Deliverable 10):
  - Added new browser/WASM analysis adapter crate `crates/trust-wasm-analysis/` exposing deterministic diagnostics, hover, and completion APIs for virtual-document analysis.
  - Added JSON boundary wrapper `WasmAnalysisEngine` for worker/browser transport integration (`applyDocumentsJson`, `diagnosticsJson`, `hoverJson`, `completionJson`, `statusJson`).
  - Added parity + performance regression suite against native analysis in `crates/trust-wasm-analysis/tests/mp010_parity.rs`.
  - Added browser worker host prototype assets and build pipeline:
    - `docs/internal/prototypes/browser_analysis_wasm_spike/`
    - `scripts/build_browser_analysis_wasm_spike.sh`
    - `scripts/check_mp010_browser_analysis.sh`
  - Published scope contract and evidence report:
    - `docs/guides/BROWSER_ANALYSIS_WASM_SPIKE.md`
    - `docs/reports/browser-analysis-wasm-spike-20260212.md`
- EtherCAT Backend v1 (Deliverable 9):
  - Added new runtime I/O driver profile: `io.driver = "ethercat"` with EtherCrab-backed hardware transport (`adapter = "<nic>"`) and deterministic mock transport mode (`adapter = "mock"`).
  - Added module-chain process-image mapping contract for Beckhoff-style digital I/O profiles (`EK1100`, `EL1008`, `EL2008`) with size-check diagnostics.
  - Added startup/discovery diagnostics and cycle-time health telemetry (`ok`/`degraded`/`faulted`) with driver error policy handling.
  - Added EtherCAT deterministic integration coverage and runtime example project:
    - `crates/trust-runtime/tests/ethercat_driver.rs`
    - `examples/ethercat_ek1100_elx008_v1/`
  - Published EtherCAT backend guide with scope boundaries and compliance checkpoint:
    - `docs/guides/ETHERCAT_BACKEND_V1.md`
- Mitsubishi GX Works3 Compatibility v1 (Deliverable 8):
  - Added Mitsubishi vendor profile support in LSP tooling (`vendor_profile = "mitsubishi"` and alias `gxworks3`) for formatting, stdlib selection defaults, and diagnostics rule-pack aliases.
  - Added native `DIFU`/`DIFD` edge-alias support in semantic/runtime builtins (mapped to IEC `R_TRIG`/`F_TRIG` behavior) for normal ST authoring and execution.
  - Added Mitsubishi GX Works3 example project: `examples/mitsubishi_gxworks3_v1/`.
  - Added compatibility guide with supported subset, incompatibilities, and migration guidance: `docs/guides/MITSUBISHI_GXWORKS3_COMPATIBILITY.md`.
  - Added dedicated regression coverage across HIR semantics, runtime edge behavior, LSP formatting/diagnostics, and example compile tests.
- Multi-vendor Export Adapters v1 (Deliverable 7):
  - `trust-runtime plcopen export` now supports `--target <generic|ab|siemens|schneider>` for vendor-targeted interchange artifacts.
  - Export JSON contract now includes target adapter evidence fields: `target`, `adapter_report_path`, `adapter_diagnostics`, `adapter_manual_steps`, and `adapter_limitations`.
  - Vendor-target exports now emit deterministic sidecar adapter reports (`<output>.adapter-report.json`) and embedded `trust.exportAdapter` metadata in `addData`.
  - Published target-specific limitations/manual migration steps guide: `docs/guides/PLCOPEN_EXPORT_ADAPTERS_V1.md`.
- Editor Expansion v1 (Deliverable 6):
  - Official Neovim setup pack published with reference `nvim-lspconfig` profile and workflow keymaps: `editors/neovim/`.
  - Official Zed setup pack published with reference language-server profile: `editors/zed/`.
  - Editor setup/validation guide published: `docs/guides/EDITOR_SETUP_NEOVIM_ZED.md`.
  - New editor smoke gate script `scripts/check_editor_integration_smoke.sh` validates editor config contracts and runs targeted LSP workflow tests for diagnostics/hover/completion/formatting/definition.
- Vendor Library Compatibility Baseline (Deliverable 4):
  - `trust-runtime plcopen import` now applies deterministic vendor-library shim mappings for selected Siemens, Rockwell, Schneider/CODESYS, and Mitsubishi aliases.
  - Import/migration JSON contracts now include `applied_library_shims` with vendor/source/replacement/occurrence metadata.
  - Vendor-library compatibility matrix and shim catalog published in `docs/guides/VENDOR_LIBRARY_COMPATIBILITY.md`.
- Siemens SCL Compatibility v1 (Deliverable 3):
  - Siemens-style `#`-prefixed local references now parse in expression and statement contexts (including `FOR` loop control variables).
  - Siemens SCL compatibility guide published: `docs/guides/SIEMENS_SCL_COMPATIBILITY.md`.
  - Siemens SCL example project added: `examples/siemens_scl_v1/`.
  - Regression coverage added across parser, LSP formatting/diagnostics, and runtime example compile tests.
- PLCopen Interop Hardening (Deliverable 2):
  - expanded migration fixture coverage for major ecosystems (`codesys`, `beckhoff-twincat`, `siemens-tia`, `rockwell-studio5000`, `schneider-ecostruxure`)
  - structured unsupported-node diagnostics in migration reports with code/severity/node/action metadata
  - explicit compatibility coverage summary in import/migration reports (`supported_items`, `partial_items`, `unsupported_items`, `support_percent`, `verdict`)
  - dedicated compatibility/limits guide: `docs/guides/PLCOPEN_INTEROP_COMPATIBILITY.md`
- Conformance Suite MVP shipped (Deliverable 1):
  - deterministic case pack + versioned expected artifacts in `conformance/cases/` and `conformance/expected/`
  - coverage for timers (TON/TOF/TP), edges, scan-cycle ordering, init/reset, arithmetic corner cases, and mapped memory behavior
  - negative/error-path coverage for runtime overflow behavior and unresolved wildcard mapping compile errors
  - external run guide and submission process (`conformance/external-run-guide.md`, `conformance/submissions.md`)
  - explicit known-gaps register (`conformance/known-gaps.md`)
- `trust-runtime conformance` CLI runner mode:
  - deterministic `case_id` ordering
  - machine-readable JSON summary contract (`trust-conformance-v1`)
  - stable failure reason taxonomy (`conformance/failure-taxonomy.md`)
  - `--update-expected` mode for deterministic artifact refresh
- ST unit-testing tutorials:
  - `examples/tutorials/10_unit_testing_101/`
  - `examples/tutorials/11_unit_testing_102/`
- Salsa hardening gates and overnight validation scripts/reports:
  - `scripts/salsa_*_gate.sh`
  - `scripts/salsa_overnight_hardening.sh`
  - `docs/reports/salsa-overnight-hardening-20260209.md`
- Runtime/UI multi-driver coverage and integration tests for Modbus + MQTT.
- New ST assertion functions in runtime/hir:
  - `ASSERT_NOT_EQUAL`
  - `ASSERT_GREATER`
  - `ASSERT_LESS`
  - `ASSERT_GREATER_OR_EQUAL`
  - `ASSERT_LESS_OR_EQUAL`
- `trust-runtime test --list` to discover test names without executing.
- `trust-runtime test --timeout <seconds>` for per-test execution timeout.
- CLI/integration tests for list/filter/timeout behavior and JSON duration fields.

### Changed

- Local developer test workflow acceleration:
  - `just test` now uses `cargo-nextest` for the default local loop (`trust-runtime` library tests), with automatic fallback to `cargo test` if `nextest` is unavailable.
  - Added split test targets in `justfile`: `test-integration`, `test-e2e`, and `test-all` (full previous gate).
  - Enabled faster local linking via `mold` (`.cargo/config.toml`) and enabled `sccache` as Cargo `rustc-wrapper`.
  - Reduced dev/test debug symbol level to `debug = 1` for faster incremental compile/link during local iteration.
- PLCopen CODESYS import hardening:
  - `trust-runtime plcopen import` now maps CODESYS `{attribute 'qualified_only'}` global variable lists into a compiler-valid `TYPE + CONFIGURATION/VAR_GLOBAL` wrapper model instead of emitting unsupported top-level `VAR_GLOBAL` files.
  - Imported POUs that reference qualified lists (for example `GVL.start`) now receive injected `VAR_EXTERNAL` declarations so cross-file global access resolves in trust-lsp/runtime builds.
  - Imported `FUNCTION` POUs without explicit result assignment now get a deterministic fallback self-assignment (for example `MyFunc := MyFunc;`) to prevent `E206 missing return value` on first import.
- Demo Pages WASM startup UX now force-updates the `WASM Ready` badge after engine readiness resolves, preventing a stale `Loading WASM...` state when the worker `ready` event arrives before listener registration.
- Demo WASM analysis clients now send an internal bootstrap `status` request on worker spawn so startup no longer depends on user interaction before `WASM Ready` appears.
- Demo Pages now publishes a root `favicon.ico` and links it from `docs/demo/index.html`, avoiding browser default `/favicon.ico` 404s.
- VS Code statechart extension quality gates now lint TSX sources and run explicit webview TypeScript typechecking (`tsconfig.webview.json`) during compile.
- Statechart import command flow now consistently resolves source/target paths and reuses shared URI/path helpers across statechart commands.
- Statechart hardware helper scripts/docs now use repository-relative paths and group-based socket permissions (`660`) instead of world-writable sockets.
- Statechart operator/developer guides and helper script output are now standardized to English for consistent repository documentation language.
- Web IDE visual styling now aligns with the runtime page sidebar theme treatment (surface, spacing, and footer layout) for a consistent `/` and `/ide` experience.
- Web IDE authoring gating no longer depends on runtime `control_mode=debug`; editor capability is now enforced via web auth role and IDE session role (viewer/editor) at API boundaries.
- Web IDE performance gates were tightened to product targets (completion/hover p95 <= 150ms, diagnostics p95 <= 300ms, workspace search p95 <= 400ms) in automated integration checks.
- Standardized example I/O mapping patterns to use `VAR_CONFIG` bindings (instead of direct `%I/%Q` declarations in `VAR`/`VAR_GLOBAL`) across communication, EtherCAT, Siemens, Mitsubishi, and unit-testing tutorial projects.
- Updated example documentation snippets to reflect `PROGRAM` variables plus `CONFIGURATION`-level `VAR_CONFIG` wiring as the recommended deterministic pattern.
- Browser/WASM position mapping now uses UTF-16 column semantics for protocol compatibility in `trust-wasm-analysis` range/position conversions.
- HMI scaffold update behavior now skips regenerating default `process.toml` when curated custom process pages already exist, and skips creating empty `control.toml` when no writable points are discovered.
- HMI auto-schematic Process scaffold now enforces deterministic grid/anchor rules (shared FIT/PT instrument template geometry, value offset one grid row above sensor centerline, stems snapped to process line, connector-anchored routing) and level-fill bindings now update both `y` and `height` for percent-consistent tank visuals.
- Tutorial `12_hmi_pid_process_dashboard` now uses grid-aligned P&ID SVG layouts (hidden `pid-layout-guides` layer) with compact overview inventory widgets and cleaned page set (no duplicate scaffold `Process` page / empty `Control` page).
- Tutorial `12_hmi_pid_process_dashboard` Process/Bypass P&ID pages now use denser operator layout (integrated status rail, calibrated tank scales, stronger typography hierarchy, and ISA-style symbol/line cleanup) with synchronized level-fill scaling.
- Tutorial `12_hmi_pid_process_dashboard` Process/Bypass P&ID pages now remove duplicated PV summary strip and fix tank header text overlap while keeping tank fill scaling aligned to visible geometry.
- Tutorial `12_hmi_pid_process_dashboard` Process/Bypass P&ID pages now render pipe runs without the extra lane background panel.
- Added grouped communication examples under `examples/communication/` for Modbus/TCP, MQTT, OPC UA, EtherCAT, GPIO, and composed multi-driver setups, and documented protocol transport gates (including `ethercat-wire` unix hardware scope and `opcua-wire` requirement) in examples/tutorial indexes and guide docs (`PLC_IO_BINDING_GUIDE.md`, `PLC_NETWORKING.md`, `PLC_DEVELOPER_GUIDE.md`).
- Restored the field-tested EtherCAT `EK1100 + EL2008` profile as `examples/communication/ethercat_field_validated_es/` (Spanish operator-focused commissioning walkthrough) so communication examples now include two EtherCAT tracks.
- Added automated runtime CLI regression coverage for grouped communication examples via `crates/trust-runtime/tests/communication_examples_cli.rs`, verifying `build --sources src` and `validate` for `modbus_tcp`, `mqtt`, `opcua`, `ethercat`, `gpio`, and `multi_driver` in CI/test runs.
- Added advanced operations tutorial `23_observability_historian_prometheus` and expanded `16_secure_remote_access` with explicit TLS commissioning/validation steps so observability and remote-hardening flows are covered end-to-end in the examples track.
- CI release-gate aggregation now includes a dedicated `Editor Expansion Smoke` gate for Neovim/Zed integration coverage.
- PLCopen XML Full ST Project Coverage (Deliverable 5):
  - Profile advanced to `trust-st-complete-v1`.
  - `trust-runtime plcopen import` now supports full ST-project model import for:
    - `types/dataTypes` (`elementary`, `derived`, `array`, `struct`, `enum`, `subrange`)
    - `instances/configurations/resources/tasks/program instances`
  - `trust-runtime plcopen export` now emits supported ST `TYPE` declarations and configuration/resource/task/program-instance model back into PLCopen XML.
  - Import/export JSON contracts now include deterministic ST-project coverage counters:
    - `data_type_count`, `configuration_count`, `resource_count`, `task_count`, `program_instance_count` (export)
    - `imported_data_types`, `discovered_configurations`, `imported_configurations`, `imported_resources`, `imported_tasks`, `imported_program_instances` (import/migration)
  - Added CODESYS ST-complete fixture packs (`small`/`medium`/`large`) with deterministic expected migration artifacts and CI schema-drift parity gate in `crates/trust-runtime/tests/plcopen_st_complete_parity.rs`.
  - Updated PLCopen compatibility/spec docs and added end-to-end import/export example project in `examples/plcopen_xml_st_complete/`.
- Project source layout is now `src/` only across runtime/CLI flows:
  - `trust-runtime build`, `test`, `docs`, and `run --project` now require `<project>/src` as the default source root.
  - Legacy `<project>/sources` fallback resolution was removed from bundle/project source lookup.
  - Setup/wizard-generated projects and shipped tutorials/examples now use `src/` consistently.
  - PLCopen import/export project-root resolution now targets `src/` only.
  - `trust-runtime setup` auto-migrates an existing `<project>/sources` folder to `<project>/src` when preparing a project.
- `trust-runtime build --sources <relative-path>` now resolves relative source overrides from `--project`, avoiding path-resolution mismatches from external working directories.
- `trust-runtime plcopen export` and `trust-runtime plcopen import` now support `--json` for machine-readable report output.
- `trust-runtime plcopen profile` now publishes a compatibility matrix plus round-trip limits/known-gaps contract fields.
- `trust-runtime plcopen import` compatibility scoring now accounts for shimmed vendor-library aliases as partial-coverage items.
- PLCopen ecosystem detection now recognizes Mitsubishi GX Works markers (`mitsubishi-gxworks3`) for migration reporting/shim selection.
- PLCopen ecosystem detection now recognizes OpenPLC markers (`openplc`) for migration reporting/shim selection, with OpenPLC fixture-backed regression coverage.
- PLCopen vendor-library shim matching now also normalizes keyword-style vendor aliases (for example `R_EDGE`) when used in supported type/call positions.
- Migrated `trust-hir` semantic path to Salsa-only backend and upgraded Salsa to `0.26`.
- Enabled VS Code extension integration tests in CI under virtual display (`xvfb`).
- Expanded cancellation checks in workspace-scale LSP operations.
- CI now includes a dedicated conformance gate with repeated-run deterministic comparison.
- VS Code extension marketplace metadata now declares dual-license SPDX (`MIT OR Apache-2.0`) and monorepo repository directory (`editors/vscode`).
- Documentation organization:
  - Public durable reports remain in `docs/reports/`.
  - Working remediation checklists are no longer published in `docs/reports/`.
- `trust-runtime test` output now reports per-test elapsed time and total elapsed time in human output.
- `trust-runtime test --output json` now includes `duration_ms` per test and in summary.
- Tutorial 10/11 docs updated for list/timeout usage and expanded assertion coverage.

### Fixed

- VS Code SFC webview TypeScript message/edge typing regressions that previously failed extension compile gates in CI.
- Standalone Web IDE bootstrap now serves fully composed split module assets (`ide-editor-language`, `ide-editor-pane`, `ide-workspace-tree`, `ide-workspace-files`, `ide-observability`, `ide-commands`) so session startup/open-project flows no longer fail with missing runtime symbols (for example `refreshProjectSelection`).
- Standalone Web IDE bootstrap no longer stalls waiting for WASM analysis readiness when the analysis worker repeatedly restarts; IDE startup now completes and remains usable with analysis disabled.
- Web IDE UI-mode detection now accepts both wrapped (`{ok,result}`) and direct (`{ok,mode}`) API payloads, restoring correct standalone-mode behavior (for example disabling runtime-only controls in `standalone-ide` mode).
- Web IDE Hardware tab now rehydrates after IDE session bootstrap events (including deep-link startup on `/ide/hardware`) and performs delayed canvas relayout/fit passes after tab activation so hardware + communication modules from `io.toml`/`runtime.toml` reliably render without clipped/off-screen graphs.
- Web IDE workspace tabs now publish explicit ARIA tab semantics (`tablist`/`tab`/`tabpanel`, `aria-selected`, and active-tab `tabindex` management) for keyboard/screen-reader correctness.
- Config-UI Fleet topology projection no longer reports planned runtimes/endpoints as online by default; `/api/runtime-cloud/state` in config mode now renders offline/degraded runtime state and failed links until live runtime connectivity exists.
- Config-UI now serves read-only `/api/runtime-cloud/config` and `/api/runtime-cloud/rollouts` snapshots in offline mode to prevent Web UI polling 404 noise during TOML/ST-only editing sessions.
- Runtime cloud 5-host topology demo is now self-contained and deterministic: `start-5.sh` auto-bootstraps missing projects, performs clean restarts, waits for runtime-cloud readiness, and ships explicit mesh/link-transport seed data for mixed `realtime`/`zenoh` topology behavior.
- Fleet/Comms add-driver deep-link handling now preserves unknown preselected driver types instead of silently falling back to `modbus-tcp`.
- Fleet/Comms schema-number handling now preserves valid `0` values (for example Modbus `unit_id = 0`) and enforces min/max validation before save.
- Fleet topology device context menu now resolves touch coordinates reliably (long-press/taphold) and always unregisters its outside-click dismissal listener on close.
- VS Code statechart custom editor packaging now loads the webview template from bundled extension code instead of `src/**` runtime paths excluded by `.vscodeignore`.
- VS Code statechart editor lifecycle now stops active execution sessions when the panel closes, ensuring timers/runtime connections are cleaned up.
- State machine engine transition execution now awaits exit/transition/entry hardware actions before completing transitions.
- Hardware-mode guard handling now fails closed on parse/read/runtime errors instead of allowing unsafe transitions.
- Runtime control client request timeout/error handling now removes per-request listeners reliably to prevent listener leaks.
- Statechart traffic-light example now starts from a valid non-dead initial state (`Red`).
- Web IDE hover behavior now follows VS Code semantics by relying on Monaco hover tooltips; the redundant Output-side Hover panel was removed to prevent duplicate/stale hover text.
- Web IDE hover provider failures are now logged as `[ide] hover failed:` in browser console output to improve diagnostics when hover requests fail.
- Web IDE analysis cache refresh now skips unrelated unreadable/oversized `.st` files so hover/completion keep working for the active source, Monaco hover/suggest widgets now use overflow-safe rendering to avoid hidden popups, and `/ide` assets are served with `Cache-Control: no-store` so browser refreshes pick up the latest IDE fixes.
- Web IDE hover/completion hardening now normalizes Monaco hover content payloads, falls back to local symbol suggestions when completion analysis is unavailable, and debounces hover popup triggering on mouse idle so behavior matches VS Code more closely on Chromium-based runtime kiosks.
- Browser analysis worker-recovery CI gate now includes the Node test fixture file (`docs/internal/prototypes/browser_analysis_wasm_spike/web/analysis-client.test.mjs`) in tracked repository contents, fixing missing-file failures on GitHub runners.
- GitHub Pages demo language features are now strict WASM/LSP-only (no local fallback synthesis), completion request flow is hardened via deduplicated document sync and longer WASM request budgets for slower devices, and FB hover signatures now recover declared member type text from source when semantic type IDs are unresolved.
- Browser WASM completion ranking now prioritizes typed-prefix matches before result limiting, and completion context recovery now keeps in-scope program variables available during parser recovery edits (fixing missing `Status`/`Pump` suggestions in the demo).
- Browser IDE/WASM LSP navigation for structured type fields is now project-wide: `Go to Definition`, `Go to References`, and rename now resolve `STRUCT`/`UNION` members across files (fixing `No definition/references found` on demo symbols like `Enable`, `ActualSpeed`, and `ramp`).
- GitHub Pages demo rename wiring now consumes WASM rename edits directly, restoring `F2` rename in Monaco, and walkthrough instructions now match the exact Monaco UI labels/actions (`Ctrl+Left-click`, `Go to References`, `Fn+F2` note for laptops).
- GitHub Pages demo LSP request handling is now resilient on slower browser hosts: WASM timeouts were raised for diagnostics/navigation/rename/completion, Monaco cursor boundary fallback now retries nearby symbol offsets for strict WASM requests, URI mapping accepts normalized location schemes, and diagnostics walkthrough text now reflects a clean-by-default project (use a temporary typo to demonstrate squiggles).
- GitHub Pages demo strict-WASM LSP cursor anchoring is now delimiter-aware (`.`, `#`, `:` boundaries): completion/definition/references/rename retry nearby identifier edges, context-menu requests set the cursor to the clicked symbol, and rename preflight resolves identifier ranges from the same fallback matrix.
- Browser analysis worker-recovery checks now also run demo cursor-position fallback regressions (`docs/demo/lsp-position-resolver.test.mjs`) and no longer fail hard when the legacy prototype test fixture path is absent.
- GitHub Pages demo walkthrough actions are now deterministic and WASM-only for investor flow: steps 1-7 target the correct symbols/files, `Shift+F12` and `F2` are triggered through Monaco actions, cross-file navigation opens in-editor via a registered Monaco editor opener, and bundled Monaco now includes the required references/rename contributions to keep references/rename/go-to-definition working in Pages builds.
- Added local GitHub Pages replica serving for the demo (`scripts/serve_demo_local_replica.py`, `scripts/run_demo_local_replica.sh`) with `/<repo>/` routing and no-cache headers so local behavior matches deployed Pages URLs.
- IDE symbol-target resolution now anchors token/scope lookup to the resolved identifier range (not raw punctuation cursor token), fixing `Go to Definition`, references, and rename misses when the cursor lands on adjacent punctuation in demo/editor workflows.
- GitHub Pages demo release packaging now rebuilds and ships refreshed `docs/demo/wasm/` artifacts from the current `trust-wasm-analysis` engine so deployed Pages behavior matches committed source fixes.
- VS Code extension project workflows now follow `src/`-based projects consistently: ST test run root detection accepts `src`, PLCopen export integration coverage uses `src`, and new-project scaffolding no longer generates legacy `sources/`.
- CI Windows build no longer fails on missing `wpcap.lib` when `ethercat-wire` is enabled by default; EtherCAT wire dependency wiring is now unix-target gated while preserving mock-driver support cross-platform.
- MP-001 parity baseline updated for newly added Mitsubishi LSP regression tests so discovery parity gate remains deterministic.
- Parser diagnostics now report a targeted error (`expected identifier after '#'`) for malformed Siemens SCL `#` local-reference syntax instead of generic expression errors.
- Schneider EcoStruxure vendor detection is now distinct from generic CODESYS-family heuristics in PLCopen migration reports.
- GitHub license detection no longer reports an extra `Unknown` license entry after removing the non-standard root `LICENSE` stub (dual-license files remain `LICENSE-MIT` and `LICENSE-APACHE`).
- Release packaging metadata:
  - VS Code extension package versions are now aligned to the workspace release version to avoid duplicate publish artifacts from prior extension versions.
- Release workflow hardening:
  - VS Code Marketplace publish now runs per-VSIX with retry/backoff on transient network timeouts and treats already-published artifacts as idempotent success for reruns.
- VS Code Marketplace screenshots now use absolute image URLs from GitHub raw content so images render reliably in extension listing pages.
- `trust-runtime validate` no longer reports false `undefined program` errors for valid bundles; it now validates decoded bytecode/module metadata directly and enforces task-program name integrity at bytecode-validation time.
- Bytecode encoding/validation now handles enum-typed constant payloads correctly during module build/validation.
- `%MW` memory marker force/write synchronization in runtime I/O panel flow.
- Debug adapter force latch behavior and state-lock interaction.
- Debug runner now respects configured task interval pacing.
- Windows CI/test path issues (`PathBuf` import and path hygiene guardrails).
- `Harness::run_until` now has a default cycle guard and explicit `run_until_max` limit to prevent hangs.
- Filtered test runs now clearly report when zero tests match but tests were discovered.
- `version-release-guard` now tolerates short ordering races between `main` and tag pushes by polling for the expected version tag before failing.
