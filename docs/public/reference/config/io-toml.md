# `io.toml`

`io.toml` defines which I/O backend a project uses and which outputs must be
driven to a safe value on fault/watchdog handling.

Unknown fields are rejected.

## Two Supported Shapes

### Single-driver form

```toml
[io]
driver = "loopback"
params = {}
```

### Multi-driver form

```toml
[io]
drivers = [
  { name = "modbus-tcp", params = { address = "127.0.0.1:1502", unit_id = 1, input_start = 0, output_start = 0, timeout_ms = 500, on_error = "fault" } },
  { name = "mqtt", params = { broker = "127.0.0.1:1883", topic_in = "trust/examples/in", topic_out = "trust/examples/out", reconnect_ms = 500, keep_alive_s = 5, allow_insecure_remote = false } }
]
```

Rules:

- use either `io.driver` / `io.params` or `io.drivers`
- do not mix both forms in the same file
- at least one driver must be configured unless you intentionally use `driver = "none"`

## Built-in Driver Names

The shipped canonical driver names are:

- `ethercat`
- `gpio`
- `loopback`
- `modbus-tcp`
- `mqtt`
- `simulated`

Accepted aliases also include:

- `sim`, `noop` -> `simulated`
- `modbus_tcp` -> `modbus-tcp`
- `mqtt-tcp` -> `mqtt`
- `ether-cat`, `ecat` -> `ethercat`

## Driver Patterns

### `simulated`

```toml
[io]
driver = "simulated"
params = {}
```

Use this when you want runtime execution without real hardware.

### `loopback`

```toml
[io]
driver = "loopback"
params = {}
```

Use this for first-project validation when you want outputs reflected back
locally.

### `gpio`

```toml
[io]
driver = "gpio"

[io.params]
backend = "sysfs"
inputs = [{ address = "%IX0.0", line = 17, debounce_ms = 5 }]
outputs = [{ address = "%QX0.0", line = 27, initial = false }]
```

### `modbus-tcp`

```toml
[io]
driver = "modbus-tcp"

[io.params]
address = "127.0.0.1:1502"
unit_id = 1
input_start = 0
output_start = 0
timeout_ms = 500
on_error = "fault"
```

### `mqtt`

```toml
[io]
driver = "mqtt"

[io.params]
broker = "127.0.0.1:1883"
topic_in = "trust/examples/mqtt/in"
topic_out = "trust/examples/mqtt/out"
reconnect_ms = 500
keep_alive_s = 5
allow_insecure_remote = false
```

### `ethercat`

```toml
[io]
driver = "ethercat"

[io.params]
adapter = "mock"
timeout_ms = 250
cycle_warn_ms = 5
on_error = "fault"
mock_inputs = ["01", "00"]

[[io.params.modules]]
model = "EK1100"
slot = 0

[[io.params.modules]]
model = "EL2008"
slot = 1
channels = 8
```

Use `adapter = "mock"` for deterministic local validation. Real adapters need
the appropriate wire feature and supported host platform.

## Safe-state Outputs

Safe-state entries are optional but strongly recommended:

```toml
[[io.safe_state]]
address = "%QX0.0"
value = "FALSE"
```

Rules:

- `address` uses IEC `%Q...` form
- `value` is parsed as a typed string literal for the target output
- invalid safe-state values are rejected during validation

## Validation Rules

- `io.drivers[*].name` must not be empty
- driver `params` must be a TOML table
- `safe_state` values must match the output type

## Related

- [I/O Binding Guide](../../connect/devices-and-fieldbus/io-binding.md)
- [Driver Matrix](../../connect/devices-and-fieldbus/driver-matrix.md)
- [EtherCAT](../../connect/devices-and-fieldbus/ethercat.md)
