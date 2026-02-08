# truST PLC I/O Binding Guide

This guide explains how to map hardware I/O to Structured Text variables using the Web UI
or `io.toml` and IEC addresses (`%IX`, `%QX`, `%MX`).

Tip: The Web UI supports driver selection, GPIO pin mapping, Modbus/TCP settings,
and safe‑state outputs under **I/O → I/O configuration** (no manual file editing needed).

## 1) Addressing Basics

Use IEC-style addresses in ST:
```
VAR_GLOBAL
  InSignal AT %IX0.0 : BOOL;
  OutSignal AT %QX0.0 : BOOL;
END_VAR
```

- `%I` = input, `%Q` = output, `%M` = memory
- `X` = bit address (use for GPIO and discrete I/O)

## 2) io.toml Structure (v1)

Single-driver form (legacy + still supported):
```
[io]
driver = "simulated"
params = {}
```

Multi-driver form (composed drivers, executed in order):
```
[io]
drivers = [
  { name = "modbus-tcp", params = { address = "192.168.0.10:502", unit_id = 1, input_start = 0, output_start = 0, timeout_ms = 500, on_error = "fault" } },
  { name = "mqtt", params = { broker = "192.168.0.20:1883", topic_in = "line/in", topic_out = "line/out", reconnect_ms = 500, keep_alive_s = 5, allow_insecure_remote = true } }
]
```

Rule:
- Use either `io.driver` + `io.params` or `io.drivers` (do not mix both in one file).

Optional safe state outputs:
```
[[io.safe_state]]
address = "%QX0.0"
value = "FALSE"
```

If `io.toml` is missing, the runtime uses system IO config:
- Linux/macOS: `/etc/trust/io.toml`
- Windows: `C:\\ProgramData\\truST\\io.toml`

## 3) GPIO Example (Raspberry Pi)

```
[io]
driver = "gpio"

[io.params]
backend = "sysfs"
inputs = [
  { address = "%IX0.0", line = 17, debounce_ms = 5 }
]
outputs = [
  { address = "%QX0.0", line = 27, initial = false }
]

[[io.safe_state]]
address = "%QX0.0"
value = "FALSE"
```

## 4) Loopback (Local Testing)

```
[io]
driver = "loopback"
params = {}
```

This copies outputs to inputs for local testing without hardware.

## 5) Modbus/TCP Example

```
[io]
driver = "modbus-tcp"

[io.params]
address = "192.168.0.10:502"
unit_id = 1
input_start = 0
output_start = 0
timeout_ms = 500
on_error = "fault"
```

## 6) Validate + Inspect

Validate a project folder:
```
trust-runtime validate --project <project-folder>
```

Read current I/O snapshot:
```
trust-runtime ctl --project <project-folder> io-read
```

Write output (for testing):
```
trust-runtime ctl --project <project-folder> io-write %QX0.0 TRUE
```
