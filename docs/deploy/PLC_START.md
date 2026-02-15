# PLC Start (Production)

This guide describes the recommended production start flow for truST.

## 1) Initialize System IO Config (optional, once per device)

```
trust-runtime setup --force
```

This writes `/etc/trust/io.toml` with the detected driver/backend. Project folders can still
override this by shipping their own `io.toml`.

## 2) Build a Runtime Project

Required project layout:

```
project/
  program.stbc
  runtime.toml
  io.toml
```

If you keep ST sources in `src/`, generate `program.stbc`:

```
trust-runtime build --project /path/to/project
```

## 3) Start the Runtime

```
trust-runtime --project /path/to/project
```

The runtime runs in the foreground and reports the control endpoint.
Use a process supervisor (systemd, init, etc.) for production.

## 4) Monitor / Control

```
trust-runtime ctl --project /path/to/project status
trust-runtime ctl --project /path/to/project shutdown
```

If `runtime.control.auth_token` is set, pass `--token` or export `TRUST_CTL_TOKEN`.

### Auth Token Rotation

Rotate tokens without restarting the runtime:

```
trust-runtime ctl --project /path/to/project config-set control.auth_token NEW_TOKEN
```

To clear a token (Unix socket only):

```
trust-runtime ctl --project /path/to/project config-set control.auth_token null
```

TCP control endpoints require an auth token and cannot be cleared at runtime.

### Remote Control (SSH/Tunnel)

For remote access, keep the runtime on `unix://` or `tcp://127.0.0.1` and tunnel:

```
ssh -L 9000:127.0.0.1:9000 user@plc-host
```

Then point `trust-runtime ctl --endpoint tcp://127.0.0.1:9000`.

## 5) Run at Boot (systemd)

See `docs/deploy/systemd/trust-runtime.service`.

### Log Rotation / Retention (journald)

If you use systemd/journald, configure retention in `/etc/systemd/journald.conf`:

```
SystemMaxUse=200M
SystemMaxFileSize=50M
MaxRetentionSec=1week
```

Reload journald after changes:

```
systemctl restart systemd-journald
```

### Structured Log Example

Example startup log line (JSON):

```
{"ts":1706265600000,"level":"info","event":"runtime_start","data":{"project":"/opt/trust/project","project_version":1,"resource":"Res","restart":"Warm","cycle_interval_ms":100,"io_driver":"simulated","retain_mode":"file","retain_path":"/opt/trust/retain.bin","retain_save_ms":1000,"watchdog_enabled":true,"watchdog_timeout_ms":5000,"watchdog_action":"SafeHalt","fault_policy":"Halt","control_endpoint":"unix:///tmp/trust-runtime.sock","control_auth_token_set":true,"control_auth_token_length":16}}
```
