# truST PLC Quick Start (1 Page)

This guide gets a first‑time operator from **zero** to a running PLC in minutes.
No IDE required.

## 0) Start the PLC (first‑run setup)

From an **empty folder**:
```
trust-runtime
```

If no project exists, `trust-runtime` opens the setup wizard to create one
and starts the PLC automatically.

Example output:
```
 _            ___ _____
| |_ _ _ _  _/ __|_   _|
|  _| '_| || \__ \ | |
 \__|_|  \_,_|___/ |_|

Your PLC is running.
Open: http://localhost:8080
Press Ctrl+C to stop.
```

## 1) Configure (optional)

Use the Web UI **Setup** button to change PLC name, cycle time, and driver.
Use **I/O → I/O configuration** to map GPIO or Modbus without editing files.

## 2) Open the Web UI (recommended)

Use the URL printed by `trust-runtime`:
```
http://<device-ip>:8080
```

Optional terminal UI (TUI) still works for headless setups:
```
trust-runtime ui --project .
```

## 3) Stop the PLC

```
trust-runtime ctl --project . shutdown
```

## 4) Raspberry Pi GPIO (one‑time setup)

```
sudo trust-runtime setup
```

Then re‑run `trust-runtime --project .`.

## Optional: VS Code Runtime Panel

If you use the VS Code extension, the **Structured Text Runtime** panel has two modes:

- **Local**: starts a local runtime for quick testing and debugging.
- **External**: connects to a running runtime via its control endpoint.

External mode is for real devices (PLC or remote runtime). Local mode is for development only.

Set the External runtime control endpoint in `trust-lsp.toml` or in VS Code (Runtime panel → gear icon).

## Troubleshooting

- **Missing runtime.toml / io.toml**: Run `trust-runtime` to auto‑create a project folder, then use the Setup wizard in the Web UI.
- **Missing program.tpi**: Run `trust-runtime build --project <project-folder>` to build the program image.
- **No GPIO on laptop**: Use the `loopback` driver in the wizard.
- **Control token required**: If the endpoint is TCP, set `runtime.control.auth_token` in `runtime.toml`.
