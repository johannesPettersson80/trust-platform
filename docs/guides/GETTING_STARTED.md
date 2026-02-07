# Getting Started (trueST Runtime)

This guide walks you from installation to a running PLC, then shows how to confirm it is healthy.

## 1) Install the runtime

Follow the install guide: `docs/deploy/INSTALL.md`.

Expected outcome:
- `trust-runtime` is on your PATH, or you can run `/path/to/trust-runtime`.

## 2) Start the PLC (first‑run setup)

From your project folder:

```bash
trust-runtime
```

Expected outcome:
- If no project exists, the setup wizard starts and asks for your project folder.
- After setup, the CLI prints a Web UI URL (for example `http://localhost:8080`).

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

If running in a terminal, the **hybrid console** opens automatically.
Type `/help` to see available commands.

## 3) Open the Web UI

Open the URL printed by the CLI. You should see the Overview page.

![Runtime overview](../assets/ui-overview.png)

Expected outcome:
- State shows **running**.
- Uptime increments.
- Tasks and I/O panels show data (or empty states).

Open the read-only HMI dashboard:

```
http://<device-ip>:8080/hmi
```

Expected outcome:
- Connection state shows **Connected** (or **Stale** while waiting for first cycle).
- Freshness updates every poll.
- Auto-discovered widgets render for program/global variables.

## 4) Run the setup wizard (first time)

Use the Setup button in the top bar or the “Finish setup” banner.

![Setup wizard](../assets/ui-setup.png)

Recommended first‑run values:
- PLC name: meaningful device name (example: `line_a_plc`)
- Cycle time: 50–100 ms
- I/O driver: `auto` (or `loopback` for simulation)

Expected outcome:
- “Setup complete” message.
- Project files updated in your project folder.

## 5) Verify I/O and tasks

Open the I/O page to confirm inputs/outputs are visible.

![I/O page](../assets/ui-io.png)

Expected outcome:
- Inputs/Outputs are listed.
- Driver health shows **ok**.

## 6) Configure I/O in the Web UI (optional)

Open **I/O → I/O configuration**.

For Modbus/TCP:
- Select `modbus-tcp`
- Enter address + unit ID
- Click **Test connection**
- Save I/O config, then restart the runtime

For GPIO:
- Select `gpio`
- Add inputs/outputs with IEC addresses and GPIO pins
- Configure safe‑state outputs
- Save I/O config, then restart the runtime

## 7) Watch variables + trends

- **Program → Variable watch**: add a variable name, see its live value, and (in debug mode) force values.
- **Overview → Trends**: view cycle‑time and watched variable trends over time.

## Troubleshooting

### Web UI doesn’t load
- Confirm the runtime is running.
- Check the CLI output for the Web UI address.
- On headless devices, open the URL from another device on the same LAN.

### “No tasks configured”
- Ensure your project has a `config.st` with a task definition.
- Rebuild the project if sources were changed: `trust-runtime build`.

### “No inputs/outputs mapped”
- Check `io.toml` in your project folder.
- If using system I/O: verify `/etc/trust/io.toml` exists.
- Or open **I/O → I/O configuration** and save a driver config.

### Control requests fail (auth required)
- Pair from the Network page or provide the token via CLI:
  - `trust-runtime ui --token <token>`

## What’s next

- Operator usage: `docs/guides/PLC_OPERATOR_GUIDE.md`
- Multi‑PLC setup: `docs/guides/PLC_MULTI_NODE.md`
- Networking + ports: `docs/guides/PLC_NETWORKING.md`
- Simulation-first workflow: `docs/guides/PLC_SIMULATION_WORKFLOW.md`
