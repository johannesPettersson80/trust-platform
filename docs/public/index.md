![truST wordmark](assets/images/brand/trust-logo.svg){ .brand-wordmark .brand-wordmark--light width="260" }
![truST wordmark](assets/images/brand/trust-logo-dark.svg){ .brand-wordmark .brand-wordmark--dark width="260" }

# What Is truST?

truST is an open IEC 61131-3 programming and runtime workspace for PLC-style
control projects.

One project can be edited in VS Code or the Browser IDE, built and validated
from the CLI, run by `trust-runtime`, automated through the Agent API, and
connected through runtime/plant communication surfaces.

## Go To

| Need | Open |
| --- | --- |
| install the tools | [Install](start/index.md) |
| program a PLC project | [Program](develop/index.md) |
| run, deploy, or operate a system | [Run](operate/index.md) |
| check hardware and target support | [Hardware](hardware/index.md) |
| look up commands, config, diagnostics, APIs, or specs | [Reference](reference/index.md) |

## Product Model

| Part | Role |
| --- | --- |
| VS Code extension | ST diagnostics, navigation, formatting, visual editors, Live Values, debugger, AI tools |
| Browser IDE | runtime-hosted editing surface at `/ide` |
| `trust-runtime` | execution engine, CLI workflows, runtime control, web UI |
| Agent API | JSON-RPC automation for scripts, CI, and agent loops |
| truST Mesh / protocols | runtime-to-runtime and plant communication paths |

## Core Idea

truST keeps the important state in reviewable project files:

- ST source
- `runtime.toml`
- `io.toml`
- generated build artifacts such as the program image `program.tpi`
- interchange artifacts such as PLCopen XML

That is the point of [One Project, Every Surface](concepts/one-project.md):
the editor, runtime, automation, AI tools, and communication surfaces do
not become separate project models.

## Proof

![One project across VS Code, diagnostics, debug, and Browser IDE](assets/images/one-project-surface-tour.gif)

*A real-product tour of the same project across desktop VS Code, diagnostics,
debugging, and Browser IDE.*

![Rename across files in VS Code](assets/images/vscode/lsp-rename-across-files.png)

*Rename a Structured Text symbol across files and preview the affected
definition before applying the change.*

![How truST fits together](assets/images/architecture/workflow-overview.svg)

*Source files move through build and validation into runtime artifacts, then
into `trust-runtime` and `/ide`.*

## Project Links

- [About](about.md)
- [FAQ](faq.md)
- [Changelog](changelog.md)
