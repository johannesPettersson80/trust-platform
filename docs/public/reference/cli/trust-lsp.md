# `trust-lsp`

`trust-lsp` is the Language Server Protocol server for IEC 61131-3 Structured
Text.

## Capabilities

Standard LSP capabilities include:

- diagnostics
- hover
- completion
- signature help
- definition / declaration / type definition / implementation
- references
- document highlight
- document and workspace symbols
- code actions
- rename
- semantic tokens
- formatting and on-type formatting
- code lens
- call hierarchy

It also exposes a small `workspace/executeCommand` API for project info and
HMI helper workflows.

## Not covered here

Do not use `trust-lsp` when you need:

- build / validate / test orchestration
- project-file reads and writes
- compile -> build -> reload loops
- deterministic harness control

Use `trust-dev agent serve` for those workflows.

## Config

`trust-lsp` reads:

- `trust-lsp.toml`
- `.trust-lsp.toml`
- `trustlsp.toml`

## Typical users

- VS Code
- Neovim
- Zed
- editor integrators
- external tooling that wants standard LSP semantics

## Related

- [trust-lsp.toml](../config/trust-lsp-toml.md)
- [Agent API overview](../agent-api/overview.md)
