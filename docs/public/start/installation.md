# Installation

Install truST when you need the local toolchain, not just syntax coloring.
Most users only need `trust-lsp` in the editor and `trust-runtime` for build,
validate, run, test, and runtime control.

## What gets installed

| Binary | Use when | Typical user |
| --- | --- | --- |
| `trust-lsp` | editor diagnostics, formatting, rename, navigation | every developer |
| `trust-runtime` | build, validate, run, test, setup, deploy, agent surface | every project |
| `trust-debug` | debug adapter protocol bridge | VS Code / DAP users |
| `trust-harness` | deterministic JSON-line execution | CI, agents, evaluation |
| `trust-bundle-gen` | low-level bundle packaging | advanced build workflows |

## Platform matrix

| Platform | Recommended path | Notes |
| --- | --- | --- |
| Linux | build from source or use released binaries | best-supported runtime path |
| macOS | build from source for CLI/editor tooling | browser/runtime features depend on local environment |
| Windows | use the VS Code extension plus local binaries | prefer project-local runtime configs over system-wide paths |
| Docker / CI | build from source in image | best for reproducible validation and docs gates |
| Raspberry Pi / edge Linux | build on target or cross-build carefully | verify GPIO/EtherCAT/runtime permissions on host |

## Fast paths

### VS Code first

1. Install the `truST LSP` extension from the Marketplace.
2. Open a folder containing `.st`, `.pou`, or supported visual-editor files.
3. Use `Structured Text: Open Runtime Panel` when you want runtime control,
   live I/O, or debug integration.

### Build from source

```bash
cargo build -p trust-lsp -p trust-runtime -p trust-debug -p trust-harness
```

For a release build:

```bash
cargo build --release -p trust-lsp -p trust-runtime -p trust-debug -p trust-harness
```

## Post-install verification

Run these checks from a shell:

```bash
trust-lsp --version
trust-runtime --version
trust-debug --version
trust-harness --help
```

What success looks like:

- each command resolves on `PATH`
- version output prints without a loader error
- `trust-harness --help` shows the JSON-line harness interface

## Which install path should you choose?

| Situation | Recommended path |
| --- | --- |
| You only want editing + diagnostics | VS Code extension + `trust-lsp` |
| You want to run projects locally | add `trust-runtime` |
| You want debugging | add `trust-debug` |
| You want CI or agent workflows | add `trust-harness` and `trust-runtime agent serve` |

## Common installation failures

### `command not found`

- ensure the built binary directory is on `PATH`
- if you used Cargo, add `~/.cargo/bin` or use the full binary path

### VS Code opens files but truST features are missing

- confirm the `truST LSP` extension is enabled for the workspace
- confirm `trust-lsp` can start without dependency/runtime errors
- open the Problems panel and the extension host logs

### Runtime works in build/validate but hardware access fails

- this is usually a host permission or device-access problem, not an install problem
- see [GPIO](../connect/devices-and-fieldbus/gpio.md),
  [EtherCAT](../connect/devices-and-fieldbus/ethercat.md), or
  [Troubleshooting](../troubleshooting.md)

## Where to go next

If you are new, read these in order:

1. [Choose Your Workflow](choose-your-workflow.md)
2. [First Project](first-project.md)
3. [First Run And Setup](first-run-and-setup.md)

If you want runnable examples immediately, jump to:

- [Examples](../examples/index.md)
- [Tutorials](../examples/tutorials.md)
