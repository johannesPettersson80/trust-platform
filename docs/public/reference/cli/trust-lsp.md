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

It also exposes a small `workspace/executeCommand` API for project info,
namespace moves, and the process-image and control symbol catalog used by
Control Studio and the network canvas.

## Command-line check

`trust-lsp check` prints the diagnostics the editor shows for every source file of a
project, without an LSP client, for CI jobs and pre-commit hooks:

```bash
trust-lsp check --project <project-folder>                  # path:line:col: severity[code]: message
trust-lsp check --project <project-folder> --format json    # one JSON object
trust-lsp check --project <project-folder> --deny-warnings  # warnings fail too
trust-lsp check --project <project-folder> --file src/Main.st
```

The project is loaded from its `trust-lsp.toml` as the server loads a workspace folder.
`--file` (repeatable) limits the report while the whole project is still analyzed; a file
that is not one of the project's source files is a usage error. Exit status: `0` no
errors, `1` errors (or warnings with `--deny-warnings`), `2` usage or I/O problem.

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
