![truST wordmark](assets/images/brand/trust-logo.svg){ width="260" }

# truST

truST is a free IEC 61131-3 Structured Text IDE and runtime. Program PLCs in
VS Code, run them on a laptop or a Raspberry Pi, debug live, and expose a
browser HMI without buying a vendor IDE.

[Install truST](start/installation.md){ .md-button .md-button--primary }
[Browse Examples](examples/index.md){ .md-button }

![truST runtime workflow hero](assets/images/hero-runtime.png)

*Figure:* VS Code with the Structured Text Runtime panel showing live I/O,
memory, and compile diagnostics.

## Capabilities

- author Structured Text in VS Code with diagnostics, navigation, formatting,
  and refactors
- run the same project locally or on a target device with `trust-runtime`
- inspect or operate the same project through `/ide` and `/hmi`
- automate build, validate, test, deploy, rollback, and agent workflows from a shell

## What truST Is

- one project for authoring, runtime, debugging, and browser operation
- VS Code, the browser IDE, and CLI/agent workflows all reuse the same project
  semantics
- one runtime binary for Linux, Windows, macOS, and Raspberry Pi
- open licensing, scriptable automation, and browser HMI pages shipped with the
  runtime
- `/ide` and `/hmi` run against the same bundle you build and deploy

## See It First

### Browser IDE

![Browser IDE](assets/images/browser/ide-tutorial-loaded.png)

*Figure:* The browser IDE with a tutorial project loaded at `/ide`.

### Browser HMI

![Browser HMI](assets/images/browser/hmi-home.png)

*Figure:* The browser HMI at `/hmi` with a connected runtime and live values.

### Architecture

![How truST fits together](assets/images/architecture/workflow-overview.svg)

*Figure:* Source files move through Build+Validate into artifacts
(`program.stbc`, `runtime.toml`, `io.toml`, `hmi/`), then into `trust-runtime`,
which exposes I/O drivers, the browser IDE at `/ide`, and HMI/control pages at
`/hmi`.

## Start Here

- [Installation](start/installation.md)
- [Program In VS Code](start/program-in-vscode.md)
- [Program In Browser IDE](start/program-in-browser.md)
- [Operate In Browser HMI](start/operate-in-browser.md)
- [Automate With CLI / CI / agents](start/automate-with-cli.md)

## Project And Support

- Maintainer: Johannes Pettersson
- License: MIT OR Apache-2.0
- Support: community issue tracker and maintainer contact

More detail lives on [About](about.md), [FAQ](faq.md), and
[Changelog](changelog.md).
