# `runtime.toml` Retain, Watchdog, And Fault Configuration

`runtime.toml` defines how one truST runtime instance executes, exposes control
surfaces, and participates in discovery, mesh, and runtime-cloud workflows.

Unknown fields are rejected. The file is validated by the same schema path used
by `trust-runtime validate`, the browser IDE, and runtime startup.

This is the main reference for runtime retain policy, watchdog settings, and
runtime fault policy.

## Minimal Example

```toml
[bundle]
version = 1

[resource]
name = "main"
cycle_interval_ms = 100

[runtime.control]
endpoint = "unix:///tmp/trust-runtime.sock"
mode = "production"

[runtime.log]
level = "info"

[runtime.retain]
mode = "none"
save_interval_ms = 1000

[runtime.watchdog]
enabled = false
timeout_ms = 5000
action = "halt"

[runtime.fault]
policy = "halt"
```

## Core Sections

### `[bundle]`

| Key | Type | Required | Notes |
| --- | --- | --- | --- |
| `version` | integer | yes | Must currently be `1`. |

### `[resource]`

| Key | Type | Required | Notes |
| --- | --- | --- | --- |
| `name` | string | yes | Logical resource/runtime name. Must not be empty. |
| `cycle_interval_ms` | integer | yes | Main scan interval in milliseconds. Must be `>= 1`. |

Optional task overrides:

```toml
[[resource.tasks]]
name = "Fast"
interval_ms = 10
priority = 1
programs = ["Main"]
single = "Main"
```

Each task needs:

- `name`
- `interval_ms >= 1`
- `priority`
- at least one entry in `programs`
- optional `single`

## Runtime Sections

### `[runtime]`

| Key | Type | Required | Default | Notes |
| --- | --- | --- | --- | --- |
| `execution_backend` | string | no | `vm` | Only `vm` is accepted. `interpreter` is explicitly rejected. |

### `[runtime.control]`

| Key | Type | Required | Default | Notes |
| --- | --- | --- | --- | --- |
| `endpoint` | string | yes | none | `unix://...` or `tcp://...`. Must not be empty. |
| `auth_token` | string | no | none | Required for `tcp://` endpoints. |
| `mode` | string | no | `production` | `production` or `debug`. |
| `debug_enabled` | bool | no | derived | Defaults to `true` in `debug` mode and `false` in `production`. |

### `[runtime.log]`

| Key | Type | Required | Notes |
| --- | --- | --- | --- |
| `level` | string | yes | Logging level string. Must not be empty. |

### `[runtime.retain]` (retain policy)

| Key | Type | Required | Default | Notes |
| --- | --- | --- | --- | --- |
| `mode` | string | yes | none | `none` or `file`. |
| `path` | string | only for `file` | none | Required when `mode = "file"`. |
| `save_interval_ms` | integer | yes | none | Must be `>= 1`. |

### `[runtime.watchdog]` (watchdog and fault policy)

| Key | Type | Required | Notes |
| --- | --- | --- | --- |
| `enabled` | bool | yes | Enables scan watchdog enforcement. |
| `timeout_ms` | integer | yes | Must be `>= 1`. |
| `action` | string | yes | `halt`, `safe_halt`, or `restart`. |

### `[runtime.fault]` (fault policy)

| Key | Type | Required | Notes |
| --- | --- | --- | --- |
| `policy` | string | yes | `halt`, `safe_halt`, or `restart`. |

## Networked / Optional Interfaces

### `[runtime.web]`

Defaults when omitted:

```toml
[runtime.web]
enabled = true
listen = "0.0.0.0:8080"
auth = "local"
tls = false
```

Accepted keys:

- `enabled`
- `listen`
- `auth = "local" | "token"`
- `tls`

Validation constraints:

| Condition | Requirement | Example |
| --- | --- | --- |
| `auth = "token"` | `runtime.control.auth_token` must be set | `auth = "token"` with `runtime.control.auth_token = "secret"` |
| `tls = true` | `runtime.tls.mode` must not be `"disabled"` | enable `[runtime.tls]` before serving HTTPS |
| remote listen + `runtime.tls.require_remote = true` | `tls` must be `true` | `listen = "0.0.0.0:8080"` requires `tls = true` |

### `[runtime.tls]`

Defaults when omitted:

```toml
[runtime.tls]
mode = "disabled"
require_remote = false
```

Accepted keys:

- `mode = "disabled" | "self-managed" | "provisioned"`
- `cert_path`
- `key_path`
- `ca_path`
- `require_remote`

Validation constraints:

| Condition | Requirement | Example |
| --- | --- | --- |
| `mode != "disabled"` | set both `cert_path` and `key_path` | `mode = "self-managed"` with PEM files |
| `mode = "provisioned"` | set `ca_path` in addition to cert/key | provisioned PKI bundle |

### `[runtime.deploy]`

| Key | Type | Default | Notes |
| --- | --- | --- | --- |
| `require_signed` | bool | `false` | Enforces signed deployment artifacts. |
| `keyring_path` | string | none | Required when `require_signed = true`. |

### `[runtime.discovery]`

Defaults when omitted:

```toml
[runtime.discovery]
enabled = true
service_name = "truST"
advertise = true
interfaces = []
```

Accepted keys:

- `enabled`
- `service_name`
- `advertise`
- `interfaces = ["eth0", ...]`
- `host_group`

### `[runtime.mesh]`

Defaults when omitted:

```toml
[runtime.mesh]
enabled = false
role = "peer"
listen = "0.0.0.0:5200"
connect = []
tls = false
publish = []
subscribe = {}
zenohd_version = "1.7.2"
plugin_versions = {}
```

Accepted keys:

- `enabled`
- `role = "peer" | "client" | "router"`
- `listen`
- `connect`
- `tls`
- `auth_token`
- `publish`
- `subscribe`
- `zenohd_version`
- `plugin_versions`

Validation constraints:

| Condition | Requirement | Example |
| --- | --- | --- |
| `runtime.mesh.tls = true` | runtime TLS must be enabled | mesh listener using the runtime TLS certificate set |
| remote mesh listen + `runtime.tls.require_remote = true` | mesh TLS must be on | `listen = "0.0.0.0:5200"` with `tls = true` |

### `[runtime.cloud]`

This section shapes runtime-cloud policy inside `runtime.toml`.

| Key | Type | Default | Notes |
| --- | --- | --- | --- |
| `profile` | string | `dev` | `dev`, `plant`, or `wan` |

Optional subsections:

```toml
[runtime.cloud.wan]
allow_write = [
  { action = "cfg_apply", target = "site-b/*" }
]

[runtime.cloud.links]
transports = [
  { source = "runtime-a", target = "runtime-b", transport = "zenoh" }
]
```

Allowed `transport` values:

- `realtime`
- `zenoh`
- `mesh`
- `mqtt`
- `modbus-tcp`
- `opcua`
- `discovery`
- `web`

### `[runtime.observability]`

Defaults when omitted:

```toml
[runtime.observability]
enabled = false
sample_interval_ms = 1000
mode = "all"
include = []
history_path = "history/historian.jsonl"
max_entries = 20000
prometheus_enabled = true
prometheus_path = "/metrics"
alerts = []
```

Validation constraints:

| Condition | Requirement | Example |
| --- | --- | --- |
| always | `sample_interval_ms >= 1` | `sample_interval_ms = 1000` |
| always | `max_entries >= 1` | `max_entries = 20000` |
| always | `mode` is `all` or `allowlist` | `mode = "allowlist"` |
| `mode = "allowlist"` | `include` must not be empty | `include = ["PROGRAM Main.Pressure"]` |
| `prometheus_enabled = true` | `prometheus_path` must start with `/` | `prometheus_path = "/metrics"` |

Alert entries support:

```toml
[[runtime.observability.alerts]]
name = "HighPressure"
variable = "PROGRAM Main.Pressure"
above = 8.5
debounce_samples = 3
hook = "log"
```

Each alert needs:

| Field | Requirement | Example |
| --- | --- | --- |
| `name` | required | `"HighPressure"` |
| `variable` | required | `"PROGRAM Main.Pressure"` |
| `above` / `below` | provide at least one threshold | `above = 8.5` |
| `debounce_samples` | must be `>= 1` | `debounce_samples = 3` |

### `[runtime.opcua]`

Defaults when omitted:

```toml
[runtime.opcua]
enabled = false
listen = "0.0.0.0:4840"
endpoint_path = "/"
namespace_uri = "urn:trust:runtime"
publish_interval_ms = 250
max_nodes = 128
expose = []
security_policy = "basic256sha256"
security_mode = "sign_and_encrypt"
allow_anonymous = false
```

Validation constraints:

| Condition | Requirement | Example |
| --- | --- | --- |
| always | `listen`, `endpoint_path`, and `namespace_uri` must not be empty | `listen = "0.0.0.0:4840"` |
| always | `endpoint_path` must start with `/` | `endpoint_path = "/"` |
| always | `publish_interval_ms >= 1` | `publish_interval_ms = 250` |
| always | `max_nodes >= 1` | `max_nodes = 128` |
| `enabled = true` | allow anonymous access or set both `username` and `password` | authenticated endpoint with user/password |
| `security_policy` | must be `none`, `basic256sha256`, or `aes128sha256rsaoaep` | `security_policy = "basic256sha256"` |
| `security_mode` | must be `none`, `sign`, or `sign_and_encrypt` | `security_mode = "sign_and_encrypt"` |

## Validation Workflow

Use this loop whenever you edit `runtime.toml`:

```bash
trust-runtime build --project ./my-plc --sources src
trust-runtime validate --project ./my-plc
trust-runtime ctl --project ./my-plc status
```

## Related

- [I/O Binding](../../connect/devices-and-fieldbus/io-binding.md)
- [Runtime Cloud](../../operate/runtime-cloud.md)
- [Compile, Validate, Reload](../../operate/compile-validate-reload.md)
