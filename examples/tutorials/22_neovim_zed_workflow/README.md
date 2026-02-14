# Tutorial 22: Neovim/Zed Workflow (Non-VS-Code)

This tutorial covers a practical daily flow when using `trust-lsp` in Neovim or
Zed, plus runtime/test operations from terminal.

## Why this tutorial exists

Some teams do not use VS Code as their primary editor. They still need a clear,
supported workflow for diagnostics, navigation, formatting, build, and runtime
checks.

## Scope reminder

Per current platform scope, Neovim/Zed support includes core LSP features
(diagnostics, hover, completion, formatting, definition). VS Code runtime panel
and VS Code debug command UX are out of scope for this track.

## What you will learn

- how to load official Neovim/Zed configs
- how to verify core LSP workflows
- how to pair non-VS-Code editing with terminal runtime commands

## Prerequisites

- `trust-lsp` and `trust-runtime` on `PATH`
- Neovim and/or Zed installed
- repository cloned locally

## Step 1: Pick a starter source file

Why: use a known-good tutorial file so editor setup problems are obvious.

Recommended file:
- `examples/tutorials/01_hello_counter.st`

## Step 2: Set up Neovim profile

Why: official reference config avoids ad hoc LSP setup drift.

Reference files:
- `editors/neovim/lspconfig.lua`
- `editors/neovim/README.md`

Minimal integration pattern:

```lua
local trust_lsp = require("trust_lsp")
trust_lsp.setup()
```

Expected result:
- opening `.st` file attaches `trust-lsp`

## Step 3: Set up Zed profile

Why: consistent team setup requires shared language-server mapping.

Reference files:
- `editors/zed/settings.json`
- `editors/zed/README.md`

Copy settings to workspace `.zed/settings.json`.

Expected result:
- `.st` files receive diagnostics/completion/formatting via `trust-lsp`

## Step 4: Validate LSP behaviors in editor

Why: this confirms editor wiring before runtime work.

For both editors, verify:
- diagnostic appears after intentional syntax error
- hover works on a symbol
- completion works for `TON`
- go-to-definition works on symbol references
- format operation produces stable style

## Step 5: Run project build/test from terminal

Why: runtime, tests, and CI workflows stay editor-agnostic.

```bash
trust-runtime build --project examples/tutorials/10_unit_testing_101 --sources examples/tutorials/10_unit_testing_101/sources
trust-runtime validate --project examples/tutorials/10_unit_testing_101
trust-runtime test --project examples/tutorials/10_unit_testing_101 --output json
```

Expected result:
- same build/test confidence as VS Code users

## Step 6: Run runtime and inspect via Web UI

Why: non-VS-Code users still need operational visibility.

```bash
trust-runtime run --project examples/tutorials/12_hmi_pid_process_dashboard
```

Open:
- `http://127.0.0.1:18082`
- `http://127.0.0.1:18082/hmi`

Expected result:
- runtime status and HMI pages are available independent of editor choice

## Step 7: Use official smoke gate

Why: this gate is the supported contract for editor integration quality.

```bash
scripts/check_editor_integration_smoke.sh
```

Expected result:
- required config keys and core LSP coverage pass

## Practical workflow pattern

Use this loop daily:
1. edit in Neovim or Zed
2. fix diagnostics immediately
3. run `build` and `validate`
4. run `test` before commit
5. run runtime/Web UI checks when behavior changes

## Common mistakes

- copying partial config instead of full reference profile
- expecting VS Code-specific runtime panel/debug UX in non-VS-Code editors
- skipping terminal validation commands because diagnostics look clean

## Completion checklist

- [ ] Neovim and/or Zed connected to `trust-lsp`
- [ ] core LSP features verified
- [ ] terminal build/validate/test loop verified
- [ ] runtime web/HMI verification completed
- [ ] editor smoke gate executed
