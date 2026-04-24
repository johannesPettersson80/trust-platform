# Troubleshooting

Use this page when something has already failed: a command exits with an error,
the runtime does not connect, values do not move, or a browser/runtime surface
does not match what you expected. Use [FAQ](faq.md) when the question is about
which workflow, platform, or product boundary to choose before you start.

## First three things to try

1. run build/validate first
2. confirm the config files and transport choice are the ones you think they are
3. reduce the system to the smallest local path that still reproduces the issue

## Problem routing

| If the problem sounds like... | Go to |
| --- | --- |
| runtime-to-runtime communication | [Connect -> Runtime To Runtime](connect/runtime-to-runtime/index.md) |
| hardware or fieldbus issues | [Connect -> Devices And Fieldbus](connect/devices-and-fieldbus/index.md) |
| editor/runtime panel issues | [Operate -> Debugging And Runtime Panel](operate/debugging-and-runtime-panel.md) |
| HMI issues | [Operate -> HMI And Web UI](operate/hmi-and-web-ui.md) |
| runtime-cloud issues | [Operate -> Runtime Cloud](operate/runtime-cloud.md) |

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
