# Installation

Use this page when you need the local truST toolchain. This page is for people
installing developer/runtime tooling, not for operators who were only given an
HMI URL.

## What You Install

- `truST LSP` in VS Code when you want the main editor workflow
- `trust-runtime` when you want to build, validate, run, test, serve `/ide`,
  serve `/hmi`, or use `agent serve`
- `trust-debug` when you want the debug adapter
- `trust-harness` when you want deterministic shell/CI execution

## Fastest Path: VS Code

1. Install VS Code if you do not already have it.
2. Install `truST LSP` from the VS Code Marketplace:
   <https://marketplace.visualstudio.com/items?itemName=trust-platform.trust-lsp>
3. Open Command Palette and type `Structured Text:`.

Command-line install:

```bash
code --install-extension trust-platform.trust-lsp
```

## CLI / Runtime Path

Build the shipped binaries:

```bash
cargo build -p trust-lsp -p trust-runtime -p trust-debug -p trust-harness
```

Or release binaries:

```bash
cargo build --release -p trust-lsp -p trust-runtime -p trust-debug -p trust-harness
```

## How To Know It Worked

In VS Code:

- `Structured Text:` commands appear in Command Palette
- `.st` files get diagnostics and syntax support

In a shell:

```bash
trust-lsp --version
trust-runtime --version
trust-debug --version
trust-harness --help
```

## If It Did Not Work

### No `Structured Text:` commands appear

- restart VS Code once
- confirm the extension is enabled for this workspace
- confirm `trust-lsp` starts without loader/runtime errors

### `command not found`

- add the binary directory to `PATH`
- if you built with Cargo, check `~/.cargo/bin`

### Runtime installs but hardware access fails

- that is usually a permissions/device issue, not an install issue
- go to [GPIO](../connect/devices-and-fieldbus/gpio.md),
  [EtherCAT](../connect/devices-and-fieldbus/ethercat.md), or
  [Install On Target](../operate/install-on-target.md)

## Where To Go Next

- [Choose Your Workflow](choose-your-workflow.md)
- [Program In VS Code](program-in-vscode.md)
- [Program In Browser IDE](program-in-browser.md)
- [Automate With CLI / CI / agents](automate-with-cli.md)
