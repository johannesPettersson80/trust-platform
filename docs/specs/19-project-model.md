# Project Model

This specification owns the truST project tree, config-file roles, and the
build/run lifecycle that turns source files into a runnable project.

## 1. Canonical Files

```text
project/
  src/
  trust-lsp.toml
  runtime.toml
  io.toml
  simulation.toml
  hmi/
  program.stbc
```

## 2. File Ownership

| Path | Owns |
|------|------|
| `src/` | project-owned Structured Text sources |
| `trust-lsp.toml` | editor/LSP config, dependencies, vendor profile |
| `runtime.toml` | execution, control, discovery, mesh, runtime-cloud policy |
| `io.toml` | driver selection and safe-state I/O behavior |
| `simulation.toml` | deterministic virtual coupling and fault injection |
| `hmi/` | declarative HMI/operator pages |
| `program.stbc` | compiled bytecode artifact |

## 3. Separation of Concerns

- `trust-lsp.toml` defines authoring and semantic context
- `runtime.toml` defines execution and exposed control surfaces
- `io.toml` defines physical or simulated I/O backends
- `simulation.toml` defines deterministic plant simulation behavior
- `hmi/` defines operator-facing presentation

## 4. Lifecycle

1. Author/edit source files in `src/`
2. Build to `program.stbc`
3. Validate configuration and bundle contents
4. Run or reload the runtime
5. Drive HMI, tests, harness scenarios, or agent workflows against the same project

## 5. Related Specs

- `12-bytecode.md` for `program.stbc`
- `20-agent-api-v1.md` for runtime agent orchestration
- `21-harness-protocol.md` for deterministic harness execution
