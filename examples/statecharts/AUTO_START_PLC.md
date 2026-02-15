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
│   └── statechart_handler.st
├── program.stbc                  # compiled bytecode
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
endpoint = "unix:///tmp/trust-runtime.sock"
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
ls -lh program.stbc
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

```bash
trust-runtime ctl --project /opt/trust/production status
trust-runtime ctl --project /opt/trust/production shutdown
sudo systemctl restart trust-plc.service
sudo systemctl stop trust-plc.service
trust-runtime ctl --project /opt/trust/production config-get
```

## ST Integration Notes

Example skeleton:

```st
PROGRAM Main
VAR
    statechart_running: BOOL;
    statechart_state: STRING;
    statechart_event: STRING;
END_VAR

(* StateCharts integrate through RuntimeClient over control socket *)
statechart_running := %MX1000.0;
statechart_state := 'Running';

IF start_button THEN
    %MW2000 := 1;  (* event trigger: START *)
END_IF;
END_PROGRAM
```

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
trust-runtime ctl --project /opt/trust/production status
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

With this setup, Linux boots directly into your PLC runtime, the service self-recovers on faults, and StateChart-driven control stays available through the runtime control interface.
