# Zed LSP Setup (truST)

This directory provides the official Zed reference configuration for `trust-lsp`.
The configuration is validated by `scripts/check_editor_integration_smoke.sh`.

## Prerequisites

- Zed stable channel.
- `trust-lsp` on `PATH`.

## Install

1. Copy `editors/zed/settings.json` into your workspace as `.zed/settings.json`.
2. Open a project containing Structured Text files (`.st`, `.pou`).
3. Reload the workspace in Zed if the language server does not start immediately.

## Validated Workflow Surface

- Diagnostics
- Hover
- Completion
- Formatting via language server
- Go to definition

## Scope

This setup targets core LSP workflows only. Runtime panel and debugger UX remain
VS Code specific.
