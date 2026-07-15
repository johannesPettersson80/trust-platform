# Troubleshooting

For failed commands, runtime connection issues, stale values, or browser/runtime
surfaces that do not match the expected state. For product questions, use
[FAQ](faq.md).

## First three things to try

1. run build/validate first
2. confirm the config files and transport choice are the ones you think they are
3. reduce the system to the smallest local path that still reproduces the issue

## Problem routing

| If the problem sounds like... | Go to |
| --- | --- |
| runtime-to-runtime communication | [Program / Communication / Runtime To Runtime](connect/runtime-to-runtime/index.md) |
| hardware or fieldbus issues | [Program / I/O And Hardware](connect/devices-and-fieldbus/index.md) |
| editor/runtime panel issues | [Program / PLC Programming / Debugging](operate/debugging-and-runtime-panel.md) |
| HMI issues | [Run / Operator HMI](operate/hmi-and-web-ui.md) |
| runtime-cloud issues | [Run / Runtime Cloud](operate/runtime-cloud.md) |

## Common symptoms

| Symptom | First check | Then go to |
| --- | --- | --- |
| no `Structured Text:` commands in VS Code | extension enabled and command palette opened with `Ctrl/Cmd+Shift+P` | [Installation](start/installation.md) |
| build fails before runtime starts | diagnostics and config files | [Build, Validate, Test](operate/build-validate-test.md) |
| runtime panel cannot connect | control endpoint, local process, and port | [Debugging And Runtime Panel](operate/debugging-and-runtime-panel.md) |
| values stay stale | I/O mapping and driver choice | [I/O Binding](connect/devices-and-fieldbus/io-binding.md) |
| `/hmi` opens but values are wrong | runtime freshness and descriptor bindings | [HMI And Web UI](operate/hmi-and-web-ui.md) |
| remote node cannot pair | discovery, pairing, and firewall boundary | [Discovery And Pairing](connect/runtime-to-runtime/discovery-and-pairing.md) |

## Common first checks

- run diagnostics: [Agent Quickstart](start/agent-quickstart.md) or [Build, Validate, Test](operate/build-validate-test.md)
- inspect config files: [Config reference](reference/config/index.md)
- verify transport/protocol choice: [Protocol Matrix](connect/protocol-matrix.md)
- verify runtime control/reload path: [Compile, Validate, Reload](operate/compile-validate-reload.md)

## Runtime Cloud Troubleshooting

--8<-- "docs/guides/RUNTIME_CLOUD_TROUBLESHOOTING.md:3"
