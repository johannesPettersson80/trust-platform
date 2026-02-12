# Neovim LSP Setup (truST)

This directory provides the official Neovim reference configuration for `trust-lsp`.
The configuration is validated by `scripts/check_editor_integration_smoke.sh`.

## Prerequisites

- Neovim 0.10 or newer.
- `trust-lsp` on `PATH`.
- `neovim/nvim-lspconfig` installed.

## Install

1. Copy `editors/neovim/lspconfig.lua` to `~/.config/nvim/lua/trust_lsp.lua`.
2. Call `require("trust_lsp").setup()` from your plugin bootstrap.
3. Open a workspace containing `.st` or `.pou` files.

## Default Keymaps (from reference config)

- `K` -> hover
- `gd` -> go to definition
- `gr` -> references
- `<leader>f` -> format document

Completion is enabled through `omnifunc` (`v:lua.vim.lsp.omnifunc`) and works with
or without completion plugins such as `nvim-cmp`.

## Scope

This setup targets core LSP workflows (diagnostics, hover, completion,
formatting, go-to-definition). Debug adapter and runtime panel UX remain
VS Code specific.
