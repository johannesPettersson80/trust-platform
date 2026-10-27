# Automatic PLC Startup with StateCharts

This guide shows how to configure `trust-runtime` so it starts automatically at Linux boot and runs ST programs plus UML StateCharts with EtherCAT.

## Goal

Set up a Linux host as a PLC runtime that:

1. Starts automatically with the OS
2. Runs ST programs and UML StateCharts
3. Starts EtherCAT master services
4. Restarts automatically on failure
5. Writes logs to `journald`

## Prerequisites

```bash
# 1. Build and install trust-runtime
cd <repo-root>
cargo build --release
sudo install -m 0755 target/release/trust-runtime /usr/local/bin/trust-runtime

# 2. Configure host I/O once per device
sudo trust-runtime setup --force
```

## Recommended PLC Project Layout

```text
/opt/trust/production/
├── src/                          # ST source files
│   ├── main.st
│   ├── machine.statechart.json   # a statechart; saving it writes the next two files
│   ├── machine.st                # its companion: FUNCTION_BLOCK FB_machine_STATECHART
│   └── machine.visual.runtime.st # its runtime entry: runs the companion every 20 ms
├── program.tpi                   # compiled program image
├── runtime.toml                  # runtime configuration
├── io.toml                       # EtherCAT / I/O configuration
└── trust-lsp.toml                # project configuration
```

### Example `runtime.toml`

```toml
[bundle]
version = 1

[resource]
name = "MainResource"
cycle_interval_ms = 10

[runtime.control]
mode = "production"
debug_enabled = false

[runtime.watchdog]
enabled = true
timeout_ms = 5000
action = "SafeHalt"

[runtime.retain]
mode = "file"
path = "/opt/trust/retain.bin"
save_ms = 1000
```

### Example `io.toml`

```toml
[io]
driver = "ethercat"

[io.params]
adapter = "enp111s0"
timeout_ms = 250
cycle_warn_ms = 5
on_error = "fault"

[[io.params.modules]]
model = "EK1100"
slot = 0

[[io.params.modules]]
model = "EL2008"
slot = 1
channels = 8

[[io.params.modules]]
model = "EL1008"
slot = 2
channels = 8

[[io.safe_state]]
address = "%QX0.0"
value = "FALSE"

[[io.safe_state]]
address = "%QX0.1"
value = "FALSE"
```

## Build the Project

```bash
cd /opt/trust/production
sudo trust-runtime build --project .
ls -lh program.tpi
```

## Configure a systemd Service

### 1. Create the service file

```bash
sudo nano /etc/systemd/system/trust-plc.service
```

### 2. Service content

```ini
[Unit]
Description=truST PLC Runtime with StateCharts
Documentation=https://github.com/trust-platform
After=network.target
Wants=network.target

[Service]
Type=simple
User=root
Group=root
WorkingDirectory=/opt/trust/production
ExecStart=/usr/local/bin/trust-runtime --project /opt/trust/production
Restart=always
RestartSec=5

StandardOutput=journal
StandardError=journal
SyslogIdentifier=trust-plc

LimitNOFILE=65536
LimitNPROC=4096

[Install]
WantedBy=multi-user.target
```

### 3. Enable and start

```bash
sudo systemctl daemon-reload
sudo systemctl enable trust-plc.service
sudo systemctl start trust-plc.service
sudo systemctl status trust-plc.service
```

## Monitoring and Control

### Live logs

```bash
sudo journalctl -u trust-plc.service -f
sudo journalctl -u trust-plc.service -b
sudo journalctl -u trust-plc.service --since "10 minutes ago"
```

### Runtime control commands

The service runs as root, so its endpoint file and its default control socket belong to root:
run `ctl` with `sudo`.

```bash
sudo trust-runtime ctl --project /opt/trust/production status
sudo trust-runtime ctl --project /opt/trust/production shutdown
sudo systemctl restart trust-plc.service
sudo systemctl stop trust-plc.service
sudo trust-runtime ctl --project /opt/trust/production config-get
```

## ST Integration Notes

A statechart runs as Structured Text inside the program image: its companion
`FUNCTION_BLOCK FB_<name>_STATECHART` has a `BOOL` input `EV_<EVENT>` for each
event of an untimed transition and the outputs `STATE_INDEX` and `STATE_NAME`
(`docs/specs/17-visual-editors-runtime-unification.md` §3). Its runtime entry
calls it every 20 ms; your own program can call it instead and drive its
events, for example for a chart with the events `START` and `TICK`:

```st
PROGRAM Main
VAR
    machine : FB_machine_STATECHART;  (* the companion of machine.statechart.json *)
    start_button AT %IX0.0 : BOOL;
    tick : TON;
END_VAR

tick(IN := NOT tick.Q, PT := T#200ms);
machine(EV_START := start_button, EV_TICK := tick.Q);
(* machine.STATE_NAME holds the active state *)
END_PROGRAM
```

The companion writes the direct addresses of its action mappings through
`VAR_EXTERNAL` globals `ld_io_<address>`; a configuration that runs it declares
them `AT` the addresses, as the runtime entry does.

## Production Recommendations

- Keep `debug_enabled = false` in production.
- Define safe output states in `io.toml`.
- Enable watchdog with a strict timeout and `SafeHalt` action.
- Restrict control endpoint access (socket ownership/permissions or auth token).
- Keep deployment artifacts versioned and use rollback workflows.

## Failure Recovery

### Service restart policy

`Restart=always` with `RestartSec=5` will restart the runtime after unexpected exits.

### Emergency checks

```bash
sudo systemctl status trust-plc.service
sudo journalctl -u trust-plc.service -n 100 --no-pager
sudo trust-runtime ctl --project /opt/trust/production status
```

## Optional: Remote access over SSH tunnel

```bash
ssh -L 9000:127.0.0.1:9000 user@plc-ip
trust-runtime ctl --endpoint tcp://127.0.0.1:9000 status
```

## Pre-production testing

```bash
# Use mock adapter first
# io.toml -> [io.params] adapter = "mock"

sudo systemctl restart trust-plc.service
sudo systemctl stop trust-plc.service
cd /opt/trust/production
sudo trust-runtime --project .
# Ctrl+C to stop, then:
sudo systemctl start trust-plc.service
```

## Summary

With this setup, Linux boots directly into your PLC runtime, the service self-recovers on faults, and each statechart runs in the PLC cycle as its companion function block.
