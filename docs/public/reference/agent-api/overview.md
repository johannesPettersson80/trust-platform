# Agent API Overview

The external agent contract is served by:

```bash
trust-dev agent serve --project ./my-plc
```

## Use cases

- local agents
- CI automation
- shell tooling
- future hosted backends that should reuse the same command handlers

## Capability matrix

| Capability | Pure LSP | Agent contract | VS Code only |
| --- | --- | --- | --- |
| diagnostics | yes | yes | yes |
| formatting preview | yes | yes | yes |
| code actions | yes | not yet mirrored in agent v1 | yes |
| rename preview/apply | yes | not yet mirrored in agent v1 | yes |
| project info/orientation | limited | yes | yes |
| build | no | yes | yes |
| validate | no | yes | yes |
| tests | no | yes | yes |
| compile + reload loop | no | yes | yes |
| deterministic harness control | no | yes | no |

**Design principle:** Use LSP for language-standard editor features. Use the
external agent contract for orchestration, runtime, test, and harness
automation. Keep editor-specific panels and UX in VS Code.

## Current high-value methods

- workspace methods
- diagnostics and formatting
- build/test/reload
- deterministic harness methods

## Exact protocol

- [Agent API v1](v1.md)
