# Changelog

All notable changes to this project will be documented in this file.

The format is based on Keep a Changelog and this project adheres to Semantic Versioning.

## [Unreleased]

Target release: `v0.7.12`

### Added

- VS Code PLCopen import workflow:
  - Added command `Structured Text: Import PLCopen XML` (`trust-lsp.plcopen.import`) for CLI-backed PLCopen project import from the editor.
  - Command flow prompts for input XML + target project folder, validates conflict paths, runs `trust-runtime plcopen import --json`, and offers quick access to the generated migration report.
  - Added VS Code integration coverage for success/cancel/conflict/invalid-input paths in `editors/vscode/src/test/suite/plcopen-import.test.ts`.
- OpenPLC Interop v1 deliverable closure:
  - Added dedicated OpenPLC migration guide: `docs/guides/OPENPLC_INTEROP_V1.md`.
  - Added end-to-end OpenPLC sample bundle with native/export/import walkthrough:
    - `examples/openplc_interop_v1/README.md`
    - `examples/openplc_interop_v1/interop/openplc.xml`
    - `examples/openplc_interop_v1/sources/main.st`
  - Added regression coverage to keep the OpenPLC sample bundle executable in CI:
    - `crates/trust-runtime/tests/plcopen_command.rs`
    - `crates/trust-runtime/tests/tutorial_examples.rs`
- truST Browser Analysis Spike (Deliverable 10):
  - Added new browser/WASM analysis adapter crate `crates/trust-wasm-analysis/` exposing deterministic diagnostics, hover, and completion APIs for virtual-document analysis.
  - Added JSON boundary wrapper `WasmAnalysisEngine` for worker/browser transport integration (`applyDocumentsJson`, `diagnosticsJson`, `hoverJson`, `completionJson`, `statusJson`).
  - Added parity + performance regression suite against native analysis in `crates/trust-wasm-analysis/tests/mp010_parity.rs`.
  - Added browser worker host example and build pipeline:
    - `examples/browser_analysis_wasm_spike/`
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
  - `examples/tutorials/unit_testing_101/`
  - `examples/tutorials/unit_testing_102/`
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

- Browser/WASM position mapping now uses UTF-16 column semantics for protocol compatibility in `trust-wasm-analysis` range/position conversions.
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
- `%MW` memory marker force/write synchronization in runtime I/O panel flow.
- Debug adapter force latch behavior and state-lock interaction.
- Debug runner now respects configured task interval pacing.
- Windows CI/test path issues (`PathBuf` import and path hygiene guardrails).
- `Harness::run_until` now has a default cycle guard and explicit `run_until_max` limit to prevent hangs.
- Filtered test runs now clearly report when zero tests match but tests were discovered.
- `version-release-guard` now tolerates short ordering races between `main` and tag pushes by polling for the expected version tag before failing.
