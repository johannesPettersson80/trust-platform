# Runtime Model

The runtime model covers build artifacts, execution, process image, control
endpoint behavior, hot reload, and web/runtime-cloud surfaces.

| Term | Meaning |
| --- | --- |
| `program.stbc` | Compiled bytecode artifact loaded by `trust-runtime`. |
| Control endpoint | Local or configured API used for status, reload, and runtime control. |
| Hot reload | Guarded bytecode replacement after diagnostics/build checks. |
| `HMI` | Runtime-hosted browser operator surface. |

## Build Artifact

The executable runtime bundle centers on `program.stbc`, which is produced by:

```bash
trust-runtime build --project ./my-plc --sources src
```

## Runtime Inputs

The runtime combines:

- bytecode from `program.stbc`
- runtime config from `runtime.toml`
- I/O config from `io.toml`
- optional simulation config from `simulation.toml`
- optional HMI descriptors from `hmi/`

## Control Options

The same underlying runtime can be driven through:

- `trust-runtime ctl`
- the runtime UI / web UI
- the VS Code Runtime Panel
- `trust-dev agent serve`

## Reload Path

Hot reload works by rebuilding bytecode and sending `bytecode.reload` to the
control endpoint. The agent method `runtime.compile_reload` wraps:

1. diagnostics
2. build
3. bytecode read
4. runtime reload

into one machine-readable response.

## Fault / Retain / Watchdog

These behaviors are configured in `runtime.toml` and shape how the runtime
reacts to deadline overruns, runtime faults, warm/cold restart, and safe-state
output handling.

## One Runtime, Multiple Operating Modes

The runtime can be used in several different ways without changing the project
model:

- local CLI-driven development
- editor-driven compile/reload loops
- agent-driven diagnostics, build, and reload
- browser-hosted runtime/UI workflows
- simulation-backed commissioning
- runtime-cloud-connected deployment

The docs are organized so those are operating modes of one runtime, not
different products.

## What The Runtime Owns

The runtime owns:

- loading and executing bytecode
- maintaining process image and task scheduling
- applying runtime and I/O configuration
- exposing control/status endpoints
- coordinating safe reload and lifecycle transitions

It does not replace the language server or the harness; those are adjacent
tools built on the same project semantics.

## Related

- [runtime.toml](../reference/config/runtime-toml.md)
- [Compile, Validate, Reload](../operate/compile-validate-reload.md)
- [Runtime UI And Control](../operate/runtime-ui-and-control.md)
