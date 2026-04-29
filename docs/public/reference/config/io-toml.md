# `io.toml`

`io.toml` defines which I/O backend a project uses and which outputs must be
driven to a safe value on fault/watchdog handling.

Unknown fields are rejected.

## Two Supported Shapes

### `[io]`

| Key | Type | Required | Notes |
| --- | --- | --- | --- |
| `driver` | string | single-driver form | Use one built-in driver name plus `[io.params]`. |
| `params` | table | single-driver form | Driver-specific configuration for `driver`. |
| `drivers` | array of tables | multi-driver form | Compose multiple named drivers in one runtime. |

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

Selection rules:

| Condition | Requirement |
| --- | --- |
| single-driver file | use `io.driver` plus `io.params` |
| multi-driver file | use `io.drivers` only |
| mixed forms | invalid |
| empty config | invalid unless you intentionally choose `driver = "none"` |

## Built-in Driver Names

| Driver | Purpose |
| --- | --- |
| `ethercat` | EtherCAT master / mock adapter workflows |
| `gpio` | direct GPIO-backed I/O |
| `loopback` | feed outputs back into inputs locally |
| `modbus-tcp` | Modbus TCP client I/O |
| `mqtt` | MQTT topic-backed I/O |
| `simulated` | runtime without physical hardware |

Accepted aliases also include:

| Alias | Canonical driver |
| --- | --- |
| `sim`, `noop` | `simulated` |
| `modbus_tcp` | `modbus-tcp` |
| `mqtt-tcp` | `mqtt` |
| `ether-cat`, `ecat` | `ethercat` |

## Driver Patterns

### `simulated`

```toml
[io]
driver = "simulated"
params = {}
```

Choose this driver when you want runtime execution without physical hardware.

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

Remote plaintext brokers are rejected unless `allow_insecure_remote = true` is
set explicitly for a test or development exception. Production remote brokers
should use TLS:

```toml
[io]
driver = "mqtt"

[io.params]
broker = "mqtts://mqtt.example.test:8883"
topic_in = "trust/site-a/in"
topic_out = "trust/site-a/out"
reconnect_ms = 500
keep_alive_s = 5
tls = true
tls_ca_path = "/etc/trust/certs/mqtt-ca.pem"
tls_client_cert_path = "/etc/trust/certs/runtime-client.pem"
tls_client_key_path = "/etc/trust/private/runtime-client-key.pem"
tls_alpn = ["mqtt"]
```

MQTT TLS uses the broker host name as the TLS server name/SNI value. Use a DNS
name in `broker`; do not rely on a raw IP address matching a self-signed
certificate. `tls_client_cert_path` and `tls_client_key_path` must be provided
together when mutual TLS is required.

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

### `[[io.safe_state]]`

| Key | Type | Required | Notes |
| --- | --- | --- | --- |
| `address` | IEC output address | yes | Uses `%Q...` addressing. |
| `value` | string | yes | Parsed as a typed value for the target output. |

## Validation Rules

| Condition | Requirement |
| --- | --- |
| `io.drivers[*].name` | must not be empty |
| driver `params` | must be a TOML table |
| `io.safe_state[*].value` | must match the output type |

## Related

- [I/O Binding Guide](../../connect/devices-and-fieldbus/io-binding.md)
- [Driver Matrix](../../connect/devices-and-fieldbus/driver-matrix.md)
- [EtherCAT](../../connect/devices-and-fieldbus/ethercat.md)
