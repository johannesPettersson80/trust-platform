# truST

truST is a free IEC 61131-3 Structured Text IDE and runtime. Program PLCs in
VS Code, run them on a laptop or a Raspberry Pi, debug live, and expose a real
browser HMI without buying a vendor-specific engineering suite.

[Install truST](start/installation.md){ .md-button .md-button--primary }
[Browse Examples](examples/index.md){ .md-button }

![truST runtime workflow hero](assets/images/hero-runtime.png)

*Figure:* The runtime/debug surface inside truST. This is the engineering path
for live values, runtime state, and quick verification.

## What You Can Do With It

- author Structured Text in VS Code with diagnostics, navigation, formatting,
  and refactors
- run the same project locally or on a target device with `trust-runtime`
- inspect or operate the same project through `/ide` and `/hmi`
- automate build, validate, test, deploy, rollback, and agent workflows from a shell

## What truST Replaces

truST covers the day-to-day authoring, runtime, and browser workflow that many
teams otherwise split across TIA Portal, CODESYS, TwinCAT, and ad-hoc shell
tooling. It is honest about its limits: truST is not safety-rated, and vendor-
locked ecosystem features still matter when your plant requires them.

## See It First

### Browser IDE

![Browser IDE](assets/images/browser/ide-tutorial-loaded.png)

*Figure:* The browser IDE with a real tutorial project loaded. Use this when
you want editing and build/validate flows at `/ide`.

### Browser HMI

![Browser HMI](assets/images/browser/hmi-home.png)

*Figure:* The browser HMI with a connected runtime and live values. This is the
operator-facing `/hmi` surface.

### Architecture

![How truST fits together](assets/images/architecture/workflow-overview.svg)

*Figure:* Source files move through build and validation into `trust-runtime`,
which then exposes I/O drivers, browser IDE, and HMI/control surfaces.

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
