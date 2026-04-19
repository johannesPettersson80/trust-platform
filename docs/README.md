# Documentation Index

Canonical public docs now live under `docs/public/` and are built with `mkdocs.yml`.

Use these entry points first:

- `docs/public/index.md`
- `docs/public/start/agent-quickstart.md`
- `docs/public/connect/protocol-matrix.md`
- `docs/public/examples/index.md`

This `docs/` directory still contains source guides, specs, diagrams, reports, and internal material used to build or support that public surface.

For quick start and runtime inline values, see the root `README.md`.

## Reports

Durable engineering reports and gate baselines are in `docs/reports/`.
See `docs/reports/README.md` for what is kept there vs. what should go to `logs/` or `docs/internal/`.

## Internal Documents

Implementation planning notes and remediation checklists live in `docs/internal/`.

## Guided Examples

The curated runnable catalog is indexed in:

- `docs/public/examples/index.md`
- `examples/README.md`

## Agent Automation

The current external automation surfaces are documented in:

- `docs/public/reference/agent-api/overview.md`
- `docs/public/reference/harness/protocol.md`
- `docs/guides/AGENT_CONTRACT_V1.md`
- `docs/guides/TRUST_HARNESS_PROTOCOL.md`

## Runtime Cloud Manual

User-facing runtime cloud onboarding docs are here:
- `docs/guides/RUNTIME_CLOUD_MANUAL.md`
- `docs/guides/RUNTIME_CLOUD_QUICKSTART.md`
- `docs/guides/RUNTIME_CLOUD_PROFILE_COOKBOOK.md`
- `docs/guides/RUNTIME_CLOUD_FEDERATION_GUIDE.md`
- `docs/guides/RUNTIME_CLOUD_UI_WALKTHROUGH.md`
- `docs/guides/RUNTIME_CLOUD_TROUBLESHOOTING.md`

Runnable payload/config examples:
- `examples/runtime_cloud/README.md`

## Runtime Execution Backend Migration

Backend mode controls, rollback workflow, compatibility-window policy, and release-evidence expectations for MP-060 are documented in:
`docs/guides/RUNTIME_EXECUTION_BACKEND_MIGRATION.md`.

## Runtime Performance And Benchmark Builds

User-facing runtime benchmark and build-mode guidance is documented in:
- `docs/guides/PLC_DEVELOPER_GUIDE.md`
- `examples/plcopen_motion_single_axis_benchmarks/README.md`

Use `TRUST_RUNTIME_HOST_CODEGEN=generic` for portable/shared binaries and cross-host comparisons. Use `TRUST_RUNTIME_HOST_CODEGEN=native` when you control the deployment hardware and want maximum performance on that machine class. The full host-native + PGO workflow is documented in `docs/guides/PLC_DEVELOPER_GUIDE.md`.

## HMI Directory Workflow

Production `hmi/` descriptor usage (including process SVG pages and LM tool
invocation order) is documented in:
`docs/guides/HMI_DIRECTORY_WORKFLOW.md`.

## Conformance Suite

Conformance scope, naming rules, and summary-contract artifacts are in
`conformance/README.md`.
External comparison guidance is in `conformance/external-run-guide.md`.

## PLCopen Interop

PLCopen compatibility matrix, migration diagnostics contract, round-trip limits,
and known gaps are documented in:
`docs/guides/PLCOPEN_INTEROP_COMPATIBILITY.md`.

## PLCopen Motion Library

The currently shipped PLCopen Motion profile, public data types, per-FB input/output reference, and deferred-scope guard rails are documented in:
- `docs/guides/PLCOPEN_MOTION_LIBRARY_GUIDE.md`
- `docs/specs/coverage/plcopen-motion-coverage.md`

Reference consumer walkthrough:
- `examples/plcopen_motion_single_axis_demo/README.md`

The reusable PLCopen Motion ST packages live under `libraries/plcopen_motion/`.
The fixture projects under
`crates/trust-runtime/tests/fixtures/plcopen_motion/` are conformance
consumers, not the library source.

## OSCAT Library

The currently shipped OSCAT library is the full manual-aligned chapter set
ported in truST, including the shared carriers, signal-processing/control
helpers, OSCAT_BUILDING Chapter 23 surface, and the conformance evidence for
the shipped `fixtures/oscat/core` project. The user-facing package manual lives
in:

- `docs/guides/OSCAT_LIBRARY_GUIDE.md`

The reusable OSCAT package lives under:

- `libraries/oscat/`

Reference consumer walkthrough:

- `examples/oscat_smoke/README.md`

The fixture projects under:

- `crates/trust-runtime/tests/fixtures/oscat/`

are conformance consumers, not the library source.

Upstream reference material and license capture used for the port live under:

- `docs/internal/references/OSCAT/OSCAT_BASIC/`

LD network-body schema v2 interop profile:
`docs/guides/PLCOPEN_LD_INTEROP.md`.

ST-complete import/export walkthrough example:
`examples/plcopen_xml_st_complete/README.md`.

VS Code command workflow for XML import:
`README.md` and `editors/vscode/README.md` (`Structured Text: Import PLCopen XML`).

OpenPLC ST-focused migration guide and end-to-end sample bundle:
- `docs/guides/OPENPLC_INTEROP_V1.md`
- `examples/plcopen_xml_st_complete/README.md` (OpenPLC fixture: `interop/openplc.xml`)

## Vendor Library Compatibility

Vendor library baseline shim coverage and compatibility matrix are documented in:
`docs/guides/VENDOR_LIBRARY_COMPATIBILITY.md`.

## Siemens SCL Compatibility

Siemens SCL v1 supported subset, known deviations, and regression coverage are
documented in:
`docs/guides/SIEMENS_SCL_COMPATIBILITY.md`.

## Mitsubishi GX Works3 Compatibility

Mitsubishi GX Works3 v1 supported subset, known incompatibilities, and
regression coverage are documented in:
`docs/guides/MITSUBISHI_GXWORKS3_COMPATIBILITY.md`.

## EtherCAT Backend v1

EtherCAT backend v1 driver scope, module-chain mapping profile, startup/health
diagnostics, and hardware setup guidance are documented in:
`docs/guides/ETHERCAT_BACKEND_V1.md`.

## Browser Analysis WASM Spike

Worker-based browser static-analysis spike scope, protocol contract, unsupported
features, and go/no-go decision are documented in:
`docs/guides/BROWSER_ANALYSIS_WASM_SPIKE.md`.

Browser host example and build harness:
- `docs/internal/prototypes/browser_analysis_wasm_spike/`
- `scripts/build_browser_analysis_wasm_spike.sh`
- `scripts/run_browser_analysis_wasm_spike_demo.sh`
- `scripts/check_mp010_browser_analysis.sh`
- `docs/guides/BROWSER_ANALYSIS_WASM_DEMO_SCRIPT.md`
- `docs/guides/BROWSER_ANALYSIS_WASM_INTEGRATION_BRIEF.md`
- `docs/guides/BROWSER_ANALYSIS_WASM_OPENPLC_EVENT_MAPPING.md`
- `docs/guides/BROWSER_ANALYSIS_WASM_PARTNER_ACCEPTANCE_CHECKLIST.md`

GitHub Pages static demo (all 7 LSP features, no server required):
- `docs/demo/`
- `docs/guides/BROWSER_ANALYSIS_WASM_GITHUB_PAGES.md`
- `.github/workflows/demo-pages.yml`
- `scripts/build_demo.sh`
- `scripts/run_demo_local_replica.sh`

## Web IDE (`/ide`)

Runtime-hosted product browser IDE documentation:
- `docs/guides/WEB_IDE_FULL_BROWSER_GUIDE.md`
- `docs/guides/WEB_IDE_ACCESSIBILITY_BASELINE.md`
- `docs/guides/WEB_IDE_COLLABORATION_MODEL.md`

## Editor Expansion (Neovim + Zed)

Official non-VS-Code LSP setup guides and reference configurations are
documented in:
`docs/guides/EDITOR_SETUP_NEOVIM_ZED.md`.

Reference editor config packs:
- `editors/neovim/`
- `editors/zed/`

## Diagram Maintenance

Use the helper scripts to keep PlantUML diagrams in sync:

- `python scripts/update_syntax_pipeline.py` refreshes
  `docs/diagrams/syntax/syntax-pipeline.puml` and
  `docs/diagrams/generated/syntax-stats.md`.
- `scripts/render_diagrams.sh` renders all `docs/diagrams/*.puml` files to
  `docs/diagrams/generated/*.svg` and updates `docs/diagrams/manifest.json`.

Diagrams are also auto-rendered in CI via `.github/workflows/diagrams.yml`.

## Project Config Example

Use `trust-lsp.toml` at the workspace root to configure indexing and runtime-assisted features.
For inline values you can also set the runtime control endpoint from the VS Code
**Structured Text Runtime** panel (gear icon → Runtime Settings). In **External** mode the panel
connects to that endpoint; in **Local** mode it starts a local runtime for debugging and
inline values.

```toml
[project]
include_paths = ["src"]
vendor_profile = "codesys"

[dependencies]
MyMotionLib = { path = "libraries/my_motion_lib", version = "0.1.0" }

[[libraries]]
name = "vendor-stubs"
path = "vendor/siemens"
version = "0.1.0"

[runtime]
# Required to surface live inline values from a running runtime/debug session.
control_endpoint = "unix:///tmp/trust-runtime.sock"
# Optional auth token (matches runtime control settings).
control_auth_token = "optional-token"
```

Use `src/` for project-owned code, `[dependencies]` for reusable truST ST
packages, and `[[libraries]]` for external/index-only stub libraries or vendor
docs packs.

Inline values can surface live locals/globals/retain values when the runtime control endpoint is
reachable and `textDocument/inlineValue` requests include a frame id.

If you set the endpoint from the Runtime panel, inline values work without a manual
`trust-lsp.toml`.
