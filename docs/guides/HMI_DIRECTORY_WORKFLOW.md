# HMI Directory Workflow

This guide covers the production HMI descriptor workflow using the `hmi/` directory, including process pages (`plant.svg` / `plant-minimal.svg`) and LM tool invocation patterns.

## 1. Initialize `hmi/`

Generate a deterministic descriptor scaffold from PLC sources:

```bash
trust-runtime hmi init --root <project-root>
```

VS Code command alternative:
- `Structured Text: Initialize HMI Descriptor`

Expected output directory shape:

```text
hmi/
  _config.toml
  overview.toml
  trends.toml      # optional when enough numeric points exist
  alarms.toml      # optional when alarm-worthy points exist
```

## 2. Descriptor layout basics

`hmi/_config.toml` controls theme, refresh, and write policy. Page files (`hmi/*.toml`) define page metadata and widgets.

Minimal write policy example (default-safe):

```toml
[write]
enabled = false
default_role = "viewer"
allowlist = []
```

Controlled write example:

```toml
[write]
enabled = true
default_role = "operator"
allowlist = [
  "PROGRAM PumpStation.PumpSpeed",
  "GLOBAL Control.StartButton",
]
```

Write guardrails remain strict:
- writes are disabled unless explicitly enabled
- target paths must be allowlisted
- runtime authz still applies per request

## 3. Process pages (`kind = "process"`)

Use process pages when binding live values to SVG elements.

Example `hmi/plant.toml`:

```toml
title = "Plant"
kind = "process"
svg = "plant.svg"

[[bind]]
selector = "#tank_level"
attribute = "height"
source = "PROGRAM PumpStation.Level"
format = "%.0f"
scale = { in_min = 0.0, in_max = 100.0, out_min = 20.0, out_max = 180.0 }

[[bind]]
selector = "#pump_state"
attribute = "class"
source = "PROGRAM PumpStation.Run"
map = { "true" = "running", "false" = "stopped" }
```

### `plant.svg` and `plant-minimal.svg` templates

The repository includes production-ready templates:
- `hmi/plant.svg`
- `hmi/plant-minimal.svg`
- `hmi/plant.bindings.example.toml`

Typical usage:
1. Start from `hmi/plant-minimal.svg` for compact dashboards or low-density screens.
2. Start from `hmi/plant.svg` for a richer process board with more symbol anchors.
3. Copy selector IDs into `[[bind]]` entries in your page TOML.
4. Validate with `trust_hmi_validate` (or runtime integration tests) before enabling writes.

You can embed reusable symbols from `hmi/pid-symbols/`:

```svg
<image href="pid-symbols/PP001A.svg" x="500" y="450" width="96" height="96"/>
```

## 4. LM tool invocation patterns

Use deterministic tool order instead of direct free-form file rewrites.

### Scaffold-first flow

1. `trust_hmi_init`
2. `trust_hmi_get_bindings`
3. `trust_hmi_get_layout`
4. `trust_hmi_apply_patch` with `dry_run=true`
5. `trust_hmi_apply_patch` with `dry_run=false`

### Intent-to-evidence flow

1. `trust_hmi_plan_intent`
2. `trust_hmi_trace_capture`
3. `trust_hmi_generate_candidates`
4. `trust_hmi_validate` (optionally with prune/retention args)
5. `trust_hmi_preview_snapshot`
6. `trust_hmi_run_journey`
7. `trust_hmi_explain_widget`

If validation or journey steps fail, iterate and re-run before accepting changes.

## 5. Runtime + panel verification

Runtime verification:

```bash
cargo test -p trust-runtime --test hmi_readonly_integration
```

VS Code extension verification:

```bash
cd editors/vscode && npm run lint
cd editors/vscode && npm run compile
cd editors/vscode && ST_LSP_TEST_SERVER=<path>/trust-lsp npm test
```

Open preview:
- `Structured Text: Open HMI Preview`

Live descriptor refresh is supported for `hmi/*.toml` and `hmi/*.svg` updates.
