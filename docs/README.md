# Documentation Index

This directory contains specifications, guides, and diagrams for truST LSP.

For quick start and runtime inline values, see the root `README.md`.

## Reports

Durable engineering reports and gate baselines are in `docs/reports/`.
See `docs/reports/README.md` for what is kept there vs. what should go to `logs/` or `docs/internal/`.

## Internal Documents

Implementation planning notes and remediation checklists live in `docs/internal/`.

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
include_paths = ["libs"]
vendor_profile = "codesys"

[runtime]
# Required to surface live inline values from a running runtime/debug session.
control_endpoint = "unix:///tmp/trust-runtime.sock"
# Optional auth token (matches runtime control settings).
control_auth_token = "optional-token"
```

Inline values can surface live locals/globals/retain values when the runtime control endpoint is
reachable and `textDocument/inlineValue` requests include a frame id.

If you set the endpoint from the Runtime panel, inline values work without a manual
`trust-lsp.toml`.
