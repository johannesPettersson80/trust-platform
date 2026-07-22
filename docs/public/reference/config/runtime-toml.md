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

All millisecond values converted to the runtime's signed-nanosecond duration
must also be no greater than `9223372036854` (`i64::MAX / 1_000_000`); larger
TOML integers are rejected rather than overflowing or panicking.

Documented defaults apply only when an optional field is omitted. An explicit
path, listen/connect endpoint, interface, producer path, symbol pattern, or
version entry must remain nonempty after trimming. Lists and maps reject blank
entries rather than silently dropping them, and an explicit blank scalar is
not replaced by its default.

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
| `level` | string | yes | `error`, `warn` (`warning`), `info`, `debug`, or `trace`, case-insensitively. Empty and unknown values are rejected. |

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

### `[runtime.ads]`

This optional section enables the Beckhoff ADS client runtime path. The ADS
point grammar lives in [`ads.toml`](ads-toml.md); `runtime.toml` only controls
whether that project source file is loaded and how often the background worker
wakes.

Defaults when omitted:

```toml
[runtime.ads]
enabled = false
config_path = "ads.toml"
worker_tick_interval_ms = 20
```

Accepted keys:

| Key | Type | Default | Notes |
| --- | --- | --- | --- |
| `enabled` | bool | `false` | Loads `ads.toml` and starts ADS workers at runtime startup. |
| `config_path` | string | `ads.toml` | Path to the ADS client config. Relative paths resolve against the runtime bundle root. |
| `worker_tick_interval_ms` | integer | `20` | Background ADS worker wake interval. Must be `>= 1`. The scan cycle does not do socket I/O. |

Validation constraints:

| Condition | Requirement | Example |
| --- | --- | --- |
| `enabled = true` | `config_path` must point at a readable ADS config | `config_path = "ads.toml"` |
| `enabled = true` at startup | runtime binary must be built with `ads-wire` | feature-enabled runtime package |
| always | `worker_tick_interval_ms >= 1` | `worker_tick_interval_ms = 20` |

### `[runtime.ads_server]`

This optional section exposes selected truST runtime globals as Beckhoff ADS
symbols. It is the ADS-server direction: external ADS clients connect to truST.
The ADS client import path above uses `[runtime.ads]` and `ads.toml`; server
mode is configured entirely here.

Defaults when omitted:

```toml
[runtime.ads_server]
enabled = false
ads_port = 851
insecure_transport = false
writes_enabled = false
symbol_namespace = ""
allow_unpinned_clients = false
unsafe_allow_public_bind = false
max_symbols = 256
max_clients = 8
max_subscriptions_per_client = 64
max_total_subscriptions = 256
max_frame_bytes = 65536
max_sumup_items = 512
max_write_bytes = 8192
max_string_bytes = 4096
read_timeout_ms = 5000
idle_timeout_ms = 60000
min_notification_cycle_ms = 50
expose = []
writable = []
allow_clients = []
```

Minimal enabled example:

```toml
[runtime.ads_server]
enabled = true
listen = "192.168.77.10"
ams_net_id = "192.168.77.10.1.1"
ads_port = 851
insecure_transport = true
writes_enabled = false
expose = ["global.TankLevel", "global.PumpRunning", "global.StatusWord"]
writable = []

[[runtime.ads_server.clients]]
ams_net_id = "192.168.77.20.1.1"
source_ip = "192.168.77.20"
```

Accepted keys:

| Key | Type | Default | Notes |
| --- | --- | --- | --- |
| `enabled` | bool | `false` | Starts the ADS server listener when the runtime binary is built with feature `ads-server`. |
| `listen` | IP string | none | Required when enabled. Must be one concrete local IP address; `0.0.0.0` and `::` are rejected. |
| `ams_net_id` | string | derived from IPv4 `listen` | Six-byte AMS Net ID exposed by truST. Required when `listen` is not IPv4. |
| `ads_port` | integer | `851` | Logical AMS target port served by truST. The TCP listener still uses ADS router port `48898`. |
| `insecure_transport` | bool | `false` | Required acknowledgement for classic plain ADS. |
| `writes_enabled` | bool | `false` | Global ADS write-back gate. |
| `symbol_namespace` | string | `""` | Optional prefix for exposed ADS symbol names. |
| `expose` | string array | `[]` | Glob allowlist of runtime globals to publish. Empty means publish nothing. |
| `writable` | string array | `[]` | Glob allowlist of published symbols that ADS clients may write when `writes_enabled = true`. |
| `allow_unpinned_clients` | bool | `false` | Lab/loopback escape hatch for AMS-Net-ID-only clients. Does not satisfy production-ready proof. |
| `unsafe_allow_public_bind` | bool | `false` | Explicit override for public/NAT-suspect listen addresses. |
| `max_symbols` | integer | `256` | Maximum exposed symbols. |
| `max_clients` | integer | `8` | Maximum concurrent ADS clients. |
| `max_subscriptions_per_client` | integer | `64` | Per-client notification subscription cap. |
| `max_total_subscriptions` | integer | `256` | Global notification subscription cap. |
| `max_frame_bytes` | integer | `65536` | Maximum accepted AMS/TCP frame size. |
| `max_sumup_items` | integer | `512` | Maximum sum-up request items. |
| `max_write_bytes` | integer | `8192` | Maximum single write payload. |
| `max_string_bytes` | integer | `4096` | Maximum string payload exposed or accepted. |
| `read_timeout_ms` | integer | `5000` | Socket read timeout. |
| `idle_timeout_ms` | integer | `60000` | Idle client timeout. |
| `min_notification_cycle_ms` | integer | `50` | Lower bound for accepted notification cycle requests. |

Structured clients:

```toml
[[runtime.ads_server.clients]]
ams_net_id = "192.168.77.20.1.1"
source_ip = "192.168.77.20"

[[runtime.ads_server.clients]]
ams_net_id = "192.168.77.30.1.1"
source_cidr = "192.168.77.0/24"
```

Legacy unpinned clients are accepted only when explicitly enabled for lab
work:

```toml
[runtime.ads_server]
allow_unpinned_clients = true
allow_clients = ["127.0.0.1.1.100"]
```

Validation constraints:

| Condition | Requirement | Example |
| --- | --- | --- |
| `enabled = true` | `listen` must be set and must not be unspecified | `listen = "192.168.77.10"` |
| `enabled = true` | `insecure_transport = true` is required for v1 plain ADS | explicit acknowledgement |
| `enabled = true` at startup | runtime binary must be built with `ads-server` | feature-enabled runtime package |
| `listen` is public or NAT-suspect | rejected unless `unsafe_allow_public_bind = true` | avoid exposing plain ADS on public networks |
| production client entries | each `[[runtime.ads_server.clients]]` needs `ams_net_id` plus `source_ip` or `source_cidr` | source-pinned allowlist |
| `source_ip` and `source_cidr` | only one may be set per client | no ambiguous source pin |
| `allow_clients` | requires `allow_unpinned_clients = true` | lab/loopback only |
| `writable` | every entry must be covered by `expose` | do not write hidden symbols |
| size/time limits | each numeric limit must be `>= 1` | `max_clients = 8` |

Classic ADS is cleartext and route-based. Keep ADS server mode on a trusted OT
segment, source-pin every production client, and enable writes only for the
smallest safe symbol set. Accepted and rejected ADS writes are audited as
`ads.server.write`.

Secure ADS is not supported in this release; `[runtime.ads_server]` only serves
classic plain ADS and requires `insecure_transport = true` when enabled.

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

### `[runtime.openot]`

This optional section enables OpenOT telemetry publishing to a shared-memory
ring. It is separate from plant I/O drivers: the runtime publishes semantic
audit records after output dispatch and fails the scan if the configured
telemetry append fails.

For the attribute-driven authoring path, see
[OpenOT Attribute Authoring](../../develop/openot-authoring.md). That compiler
path generates `Main.OotProducer` by default; use that qualified instance path
with `source = "st-fb"`.

Defaults when omitted:

```toml
[runtime.openot]
enabled = false
path = ""
capacity = 4096
fence_mode = "fenced"
allow_unfenced_for_proof = false
source = "heartbeat"
```

Accepted keys:

| Key | Type | Default | Notes |
| --- | --- | --- | --- |
| `enabled` | bool | `false` | Enables OpenOT shared-memory telemetry publishing. |
| `path` | string | `""` | Shared-memory backing file. Relative paths resolve against the runtime bundle root. Required when `enabled = true`. |
| `capacity` | integer | `4096` | Ring byte capacity. Must be `>= 1`. |
| `fence_mode` | string | `"fenced"` | `"fenced"` for product use, or `"unfenced"` only for controlled proof runs. |
| `allow_unfenced_for_proof` | bool | `false` | Must be `true` when `fence_mode = "unfenced"`. |
| `source` | string | `"heartbeat"` | `"heartbeat"` publishes a runtime heartbeat record per scan. `"st-fb"` publishes encoded records emitted by configured ST OpenOT producer FB instances. |
| `producer_instance` | string | unset | Back-compatible alias for one ST-FB producer. Qualified path to the producer FB instance, for example `"Main.OotProducer"` for attribute-generated programs or `"Main.Producer"` for a hand-authored producer FB. |
| `producer_instances` | array of strings | unset | Preferred when draining more than one ST-FB producer into one ring. Paths are drained in array order. Do not set this together with `producer_instance`. |

Validation constraints:

| Condition | Requirement | Example |
| --- | --- | --- |
| `enabled = true` | `path` must not be empty | `path = "openot.shm"` |
| always | `capacity >= 1` | `capacity = 4096` |
| `fence_mode = "unfenced"` | set `allow_unfenced_for_proof = true` | proof-only A/B run |
| `source = "st-fb"` | set a qualified `producer_instance` or non-empty `producer_instances` | `producer_instances = ["First.OotProducer", "Second.OotProducer"]` |
| `source = "heartbeat"` | omit `producer_instance` and `producer_instances` | default smoke publisher |

Multi-PROGRAM OpenOT authoring generates one hidden producer per `PROGRAM`.
Configure each generated producer explicitly:

```toml
[runtime.openot]
enabled = true
path = "openot.shm"
source = "st-fb"
producer_instances = ["First.OotProducer", "Second.OotProducer"]
```

The runtime drains the listed instances in order and serializes their records
through one shared-memory writer.

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
