![truST wordmark](assets/images/brand/trust-logo.svg){ width="260" }

# truST

truST is a free IEC 61131-3 Structured Text IDE and runtime. Program PLCs in
VS Code, run them on a laptop or a Raspberry Pi, debug live, and expose a
browser HMI without buying a vendor IDE.

[Install truST](start/installation.md){ .md-button .md-button--primary }
[Browse Examples](examples/index.md){ .md-button }

![truST runtime workflow hero](assets/images/hero-runtime.png)

*Figure:* The runtime panel inside truST with live values, runtime state, and
verification.

## Capabilities

- author Structured Text in VS Code with diagnostics, navigation, formatting,
  and refactors
- run the same project locally or on a target device with `trust-runtime`
- inspect or operate the same project through `/ide` and `/hmi`
- automate build, validate, test, deploy, rollback, and agent workflows from a shell

## What truST Replaces

truST covers the day-to-day authoring, runtime, and browser workflow that many
teams otherwise split across Siemens TIA Portal, CODESYS, Beckhoff TwinCAT, and
shell tooling.

What truST does not replace:

- safety-certified engineering environments
- vendor-specific ecosystems such as Siemens LBP/LGF, Beckhoff TwinCAT motion
  add-ons, or plant-specific closed libraries
- your target-hardware validation and commissioning process

## See It First

### Browser IDE

![Browser IDE](assets/images/browser/ide-tutorial-loaded.png)

*Figure:* The browser IDE with a tutorial project loaded at `/ide`.

### Browser HMI

![Browser HMI](assets/images/browser/hmi-home.png)

*Figure:* The browser HMI at `/hmi` with a connected runtime and live values.

### Architecture

![How truST fits together](assets/images/architecture/workflow-overview.svg)

*Figure:* Source files move through build and validation into `trust-runtime`,
which then exposes I/O drivers, the browser IDE, and HMI/control pages.

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
- Production warning: evaluate runtime/HMI/browser features against your plant
  requirements before treating them as vendor-drop-in replacements

More detail lives on [About](about.md), [FAQ](faq.md), and
[Changelog](changelog.md).
