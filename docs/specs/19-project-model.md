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
  program.tpi
  deployment-record.bin
```

## 2. File Ownership

| Path | Owns |
|------|------|
| `src/` | project-owned Structured Text sources |
| `trust-lsp.toml` | editor/LSP config, dependencies, vendor profile |
| `runtime.toml` | execution, control, discovery, mesh, runtime-cloud policy |
| `io.toml` | driver selection and safe-state I/O behavior |
| `simulation.toml` | deterministic virtual coupling and fault injection |
| `hmi/` | HMI pages and screen layout, specified in `docs/specs/39-hmi-builder.md` |
| `program.tpi` | the program image this computer runs, written by `trust-runtime build` (specification 44 §4.1, NX-15) |
| `deployment-record.bin` | the program image's deployment record (specification 34 §5.1.1) |

## 3. Separation of Concerns

- `trust-lsp.toml` defines authoring and semantic context; its `[project]
  vendor_profile` also selects the conversion rules with which every tool that
  runs the project executes it (`trust-runtime build`/`check`/`run`,
  `trust-dev test`, debug sessions, `trust-harness --project`; see
  `docs/specs/07-standard-functions.md` section 2.10)
- `runtime.toml` defines execution and exposed control surfaces
- `io.toml` defines physical or simulated I/O backends
- `simulation.toml` defines deterministic plant simulation behavior
- `hmi/` defines operator-facing presentation

### 3.1 Simulation coupling thresholds

For file-backed `simulation.toml`, each optional
`[[couplings]].threshold` must be finite. The loader rejects NaN and positive
or negative infinity before returning a configuration; it does not normalize,
clamp, or substitute a value. This requirement covers TOML loading only and
does not define programmatic `SignalCouplingRule` construction or source-value
admission.

## 4. Lifecycle

1. Author/edit source files in `src/`
2. Build to `program.tpi` with `trust-runtime build`
3. Validate configuration and bundle contents
4. Run or reload the runtime
5. Drive HMI, tests, harness scenarios, or agent workflows against the same project

## 5. Related Specs

- `44-native-execution.md` for `program.tpi`
- `20-agent-api-v1.md` for runtime agent orchestration
- `21-harness-protocol.md` for deterministic harness execution
