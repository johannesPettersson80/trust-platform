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

Marker (`%M`) address variants:

- `%MX<byte>.<bit>` (bit, BOOL), example: `%MX0.7`
- `%MB<byte>` (byte), example: `%MB12`
- `%MW<byte>` (word), example: `%MW50`
- `%MD<byte>` (double word), example: `%MD200`
- `%ML<byte>` (long word), example: `%ML8`
- `%M*` (wildcard, resolved by `VAR_CONFIG`)

Runtime cycle semantics for `%M` bindings:

- Cycle start: `%M` process image is read into bound variables.
- Cycle end: bound variable values are written back to `%M` process image.

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
input_function = "read_input_registers"
output_function = "write_multiple_registers"
timeout_ms = 500
on_error = "fault"
```

`input_function` is optional and defaults to `read_input_registers` (FC04).
Supported explicit input functions are `read_coils` (FC01),
`read_discrete_inputs` (FC02), `read_holding_registers` (FC03), and
`read_input_registers` (FC04). `output_function` is optional and defaults to
`write_multiple_registers` (FC16). Supported explicit output functions are
`write_single_coil` (FC05), `write_single_register` (FC06),
`write_multiple_coils` (FC15), and `write_multiple_registers` (FC16). Coil
payloads use Modbus bit packing with bit 0 in the least-significant bit of the
first process-image byte.

For device maps that are not one contiguous raw block, add explicit point maps:

```toml
[[io.params.input_points]]
image_offset = 0
image_bit = 0
address = 10
function = "read_coils"
data_type = "bool"

[[io.params.input_points]]
image_offset = 2
address = 100
function = "read_holding_registers"
data_type = "u32"
scale = 0.1
offset = -40.0
byte_order = "big"
word_order = "little"

[[io.params.output_points]]
image_offset = 8
address = 200
function = "write_single_register"
data_type = "u16"
scale = 1.0
offset = 0.0
```

Point maps support `bool`, `u16`, `i16`, `u32`, `i32`, and `f32`. `scale` and
`offset` convert input raw values as `engineering = raw * scale + offset`; output
writes invert that formula. `byte_order` and `word_order` describe the Modbus
wire/register layout. Numeric process-image bytes remain little-endian so
`%IW`/`%ID` bindings read them consistently inside the runtime.

## 6) MQTT Example

```
[io]
driver = "mqtt"

[io.params]
broker = "192.168.0.20:1883"
topic_in = "line/in"
topic_out = "line/out"
reconnect_ms = 500
keep_alive_s = 5
allow_insecure_remote = true
```

Without a point map, `topic_in` payload bytes are copied directly into `%I` and
`%Q` bytes are published directly to `topic_out`. For typed point topics, add
explicit MQTT point maps:

MQTT can also resolve named program-instance variables into typed point maps at
runtime startup. This avoids adding a `VAR_CONFIG` address solely for MQTT:

```toml
[[io.params.mappings]]
tag = "MainInstance.Green"
topic = "traffic/north/green"
direction = "write"
```

Mapping direction is relative to the PLC. `write` means PLC to broker, while
`read` means broker to PLC. Output-only mappings do not subscribe to the raw
default `trust/io/in` topic. Tag mapping currently supports direct scalar
variables on configured program instances; use explicit `input_points` or
`output_points` for nested values, arrays, custom payload settings, or
Sparkplug metric metadata.

```toml
[[io.params.input_points]]
topic = "line/in/ready"
image_offset = 0
image_bit = 0
data_type = "bool"
payload_format = "json"

[[io.params.input_points]]
topic = "line/in/temperature"
image_offset = 2
data_type = "i16"
payload_format = "text"
scale = 0.1
offset = -40.0

[[io.params.output_points]]
topic = "line/out/speed"
image_offset = 4
data_type = "u16"
payload_format = "json"
scale = 0.5
offset = 0.0
```

MQTT point maps support `bool`, `u16`, `i16`, `u32`, `i32`, and `f32`.
`payload_format` defaults to `text` and may be `text`, `json`, `binary_le`, or
`binary_be`. Numeric process-image bytes remain little-endian; binary MQTT
payload endianness describes the MQTT payload only.

For Sparkplug B outbound node metrics, keep typed `output_points` and add a
Sparkplug profile:

```toml
[io.params.sparkplug]
enabled = true
namespace = "spBv1.0"
spec_version = "3.0.0"
group_id = "trust-line"
edge_node_id = "runtime-a"

[[io.params.output_points]]
topic = "legacy/out/speed"
metric_name = "drive/speed"
image_offset = 4
data_type = "u16"
payload_format = "json"
scale = 0.5
offset = 0.0
```

This profile publishes NBIRTH on connect, configures NDEATH as the MQTT last
will, and publishes NDATA from typed output points. It is outbound node metrics
only: Sparkplug commands, device-level DBIRTH/DDATA topics, aliases, templates,
and store-and-forward are separate future slices.

## 7) Transport Gating Notes (Critical)

EtherCAT hardware transport (non-`mock` adapter):

- requires build feature `ethercat-wire`
- supported on unix targets only in this build
- `adapter = "mock"` remains valid for deterministic local/CI validation

OPC UA wire server:

- requires build feature `opcua-wire`
- if `[runtime.opcua].enabled = true` without `opcua-wire`, runtime startup fails with a feature-disabled error
- when enabled, configure either:
  - `allow_anonymous = true` (local commissioning only), or
  - `allow_anonymous = false` plus both `username` and `password`

Communication examples with step-by-step commissioning flow:

- `examples/communication/modbus_tcp/README.md`
- `examples/communication/mqtt/README.md`
- `examples/communication/opcua/README.md`
- `examples/communication/ethercat/README.md`
- `examples/communication/ethercat_field_validated_es/README.md`
- `examples/communication/gpio/README.md`
- `examples/communication/multi_driver/README.md`

## 8) Validate + Inspect

EtherCAT backend details (module chain profile, diagnostics, and hardware setup):
`docs/guides/ETHERCAT_BACKEND_V1.md`.

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
