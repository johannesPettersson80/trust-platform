# truST: IEC 61131-3 Structured Text for VS Code

Write, run and debug IEC 61131-3 Structured Text in VS Code, watch and force its I/O while it runs,
design operator screens, and commission control loops, all in one extension. Programs compile to
native code, and the complete compiler toolchain is part of the extension, so there is nothing else
to install.

![Structured Text in VS Code: a hover and completion, Start runs the program on the simulator, Live Values forces a level, and the debugger stops at a breakpoint and steps](https://raw.githubusercontent.com/johannesPettersson80/trust-platform/main/docs/public/assets/images/readme/core-loop.gif)

## Highlights

- **Language server.** Diagnostics as you type that name the IEC 61131-3 rule, completion, hover,
  go to definition, find references, rename across files, formatting and refactorings, for
  Structured Text with classes, interfaces, methods and namespaces.
- **Native execution.** Structured Text compiles through Rust into native code. The extension
  carries the compiler toolchain of its platform (`rustc`, `rust-lld`, the target libraries and the
  PLC program library), and the first build installs it: no download and no Rust installation of
  your own, offline too.
- **Run and debug.** Run on the built-in simulator or on a runtime, with breakpoints, stepping, call
  stack and variables on the compiled program. Live Values reads, writes and forces inputs, outputs
  and memory while the program runs.
- **HMI Builder.** Design operator screens with drag and drop, stored as TOML pages that diff and
  review like code. The runtime serves them at `/hmi`, with trends and alarms.
- **Devices & Connections.** Find and configure Modbus TCP, MQTT with Sparkplug B, OPC UA, Beckhoff
  ADS, EtherCAT and GPIO devices, and the runtimes of a project.
- **Tests and interchange.** Structured Text tests in the Testing view and through CodeLens,
  PLCopen XML import and export, and visual editors for ladder, SFC, Blockly and statecharts that
  keep a Structured Text companion.
- **Libraries and existing code.** OSCAT, PLCopen Motion and truST's control, plant, alarm, HMI and
  communication libraries from **truST: Open Libraries**, and vendor profiles that read the
  Structured Text of CODESYS and TwinCAT, Siemens SCL and Mitsubishi GX Works3.
- **Control Studio.** Identify a plant from experiments, tune and validate a controller against the
  model, apply it to a running loop with a bumpless transfer, and restore the previous settings
  when needed.

## Quick start

1. Run **truST: Create Project** from the Command Palette (`Ctrl+Shift+P`), or press **Start from
   example** in the truST side panel.
2. Press **Start** on the Run card. The first build installs the compiler toolchain, then the
   program runs on the simulator.
3. Open **Live Values** to watch and force the I/O, set a breakpoint, and step through the scan.
4. Open **Control Studio** to tune a loop, or run **truST: New HMI Screen** to design an operator
   screen.

![The debugger stepping through a compiled program, with its locals and call stack](https://raw.githubusercontent.com/johannesPettersson80/trust-platform/main/docs/public/assets/images/readme/debugger.png)

![An operator opens the LIC-101 faceplate of the hmi_equipment example at /hmi and raises its setpoint; the level follows](https://raw.githubusercontent.com/johannesPettersson80/trust-platform/main/docs/public/assets/images/readme/hmi-operation.gif)

## Commands you will use most

| Command | What it does |
| --- | --- |
| **truST: Create Project** | Creates a project folder from a starter or an example. |
| **truST: Compile** | Checks the project's sources and shows the problems. |
| **truST: Start Debugging** / **truST: Attach Debugger** | Runs the program under the debugger, or attaches to a running runtime. |
| **truST: Open Live Values** | Shows and forces the program's I/O and memory. |
| **truST: Open Control Studio** | Identifies, tunes, validates and applies controllers. |
| **truST: Open Devices & Connections** | Finds and configures devices, runtimes and connections. |
| **truST: New HMI Screen** | Adds an operator screen to the project. |
| **truST: Import PLCopen XML** / **truST: Export PLCopen XML** | Exchanges projects with other PLC tools. |
| **truST: Run All Tests** | Runs the project's Structured Text tests. |
| **truST: Install Toolchain from File** | Installs a toolchain bundle file, for example another release's. |

## Build an HMI with your agent

The agent you already use in VS Code can author HMI pages with truST's nine `hmi_*` tools: they
read the project's HMI and Structured Text documents, browse the component library, compose or
refine a draft, and return rendered previews for the agent to inspect. A review opens in the HMI
Builder with before and after images, where you apply or discard it; applying uses VS Code's Undo.
The tools do not deploy the HMI or operate the controller. For an MCP client, run
**truST: HMI: Connect Agent Tools** in the workspace and use the configuration it shows.

## Requirements

- VS Code 1.109 or later on Windows x64, macOS (Intel or Apple silicon) or Linux (x64 or arm64).
- About 650 MB of disk space for the installed compiler toolchain.

## Settings

The extension needs no settings to start. The **truST** section of the Settings editor holds the
options, for example `trust.diagnostics.showIecReferences`, and the executable paths
(`trust.languageServer.executablePath`, `trust.runtime.executablePath`,
`trust.debugAdapter.executablePath`) for using your own builds.

## Links

- [Documentation](https://johannespettersson80.github.io/trust-platform/)
- [Examples and the control course](https://github.com/johannesPettersson80/trust-platform/tree/main/examples)
- [Changelog](https://github.com/johannesPettersson80/trust-platform/blob/main/CHANGELOG.md)
- [Issues](https://github.com/johannesPettersson80/trust-platform/issues)
- [Source code](https://github.com/johannesPettersson80/trust-platform)

Licensed under MIT OR Apache-2.0.
