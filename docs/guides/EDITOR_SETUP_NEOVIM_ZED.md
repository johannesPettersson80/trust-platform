# Editor Expansion v1: Neovim + Zed

This guide documents the official non-VS-Code editor setup for `trust-lsp`.
It covers the Deliverable 6 baseline:

- Neovim reference configuration
- Zed reference configuration
- Core LSP workflow validation scope

## Scope

Supported in this guide:

- Diagnostics (`textDocument/publishDiagnostics`)
- Hover (`textDocument/hover`)
- Completion (`textDocument/completion`)
- Formatting (`textDocument/formatting`)
- Go to definition (`textDocument/definition`)

Out of scope for this editor expansion phase:

- VS Code runtime panel
- VS Code debugger command UX

## Prerequisites

- `trust-lsp` installed and available on `PATH`.
- Workspace with Structured Text files (`.st` and/or `.pou`).

## Neovim Setup

Reference files:

- `editors/neovim/lspconfig.lua`
- `editors/neovim/README.md`

Copy the reference config to `~/.config/nvim/lua/trust_lsp.lua` and then load it.

Minimal integration pattern:

```lua
local trust_lsp = require("trust_lsp")
trust_lsp.setup()
```

The reference config registers a `trust_lsp` server, enables `omnifunc`
completion, and installs keymaps for hover, definition, references, and
formatting.

## Zed Setup

Reference files:

- `editors/zed/settings.json`
- `editors/zed/README.md`

Install by copying the settings file to `.zed/settings.json` in your workspace.
The profile binds the Structured Text language to `trust-lsp`, enables language
server formatting, and enables format-on-save.

## Validation Contract

The editor-expansion smoke gate is:

```bash
scripts/check_editor_integration_smoke.sh
```

The gate validates:

1. Neovim and Zed reference configs exist and contain required keys.
2. Core LSP workflow test coverage remains green for:
   - diagnostics
   - hover
   - completion
   - formatting
   - definition

This smoke gate runs in CI as the `Editor Expansion Smoke` job.
