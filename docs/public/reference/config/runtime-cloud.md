# Runtime Cloud Profiles

Runtime-cloud config keys for `runtime.toml` and the shipped example profile
files under `examples/runtime_cloud/`.

## Core Section

```toml
[runtime.cloud]
profile = "dev"
```

### `[runtime.cloud]`

| Key | Type | Required | Notes |
| --- | --- | --- | --- |
| `profile` | string | yes | `dev`, `plant`, or `wan`. |
| `site` | string | no | The runtime's site: 1 to 63 letters, digits, `-` and `_`, beginning with a letter or digit. Without it, or with `default-site`, the runtime is in the default site. |

## Sites and Runtime IDs

Every runtime belongs to one site. A runtime in the default site is named by its resource name,
for example `runtime_a`; a runtime in any other site by `<site>/<resource>`, for example
`site-b/runtime_b`. This cloud ID names the runtime in action requests (`connected_via`,
`target_runtimes`), in the Runtime Cloud state and in the WAN allowlist's targets. Discovery
announces the site, so the runtimes that discover a runtime know its cloud ID.

```toml
[runtime.cloud]
profile = "wan"
site = "site-b"
```

A target is cross-site when its site differs from the site of the runtime the request is
connected through. The site is read when the runtime starts; `config.get` reports it as
`runtime_cloud.site`, and changing it takes an edit of `runtime.toml` and a restart.

Accepted profiles:

| Profile | Meaning |
| --- | --- |
| `dev` | local development / flexible local trust assumptions |
| `plant` | secure plant-floor or site deployment |
| `wan` | cross-site / federation mode with explicit write allowlists |

## WAN Allowlist Rules

Cross-site writes in `wan` mode are default-deny and must be allowlisted:

```toml
[runtime.cloud.wan]
allow_write = [
  { action = "cfg_apply", target = "site-b/*" }
]
```

### `[[runtime.cloud.wan.allow_write]]`

| Key | Type | Required | Notes |
| --- | --- | --- | --- |
| `action` | string | yes | e.g. `cfg_apply` |
| `target` | string | yes | runtime or site selector |

## Link Preference Rules

You can express preferred transports between runtimes:

```toml
[runtime.cloud.links]
transports = [
  { source = "runtime-a", target = "runtime-b", transport = "zenoh" }
]
```

### `[[runtime.cloud.links.transports]]`

| Key | Type | Required | Notes |
| --- | --- | --- | --- |
| `source` | string | yes | source runtime/resource id |
| `target` | string | yes | destination runtime/resource id |
| `transport` | string | yes | preferred transport for this pair |

Allowed transport values:

| Value | Meaning |
| --- | --- |
| `realtime` | low-latency shared-memory / realtime link |
| `zenoh` | zenoh-based mesh |
| `mesh` | generic mesh transport |
| `mqtt` | MQTT broker path |
| `modbus-tcp` | Modbus TCP path |
| `opcua` | OPC UA path |
| `discovery` | discovery/control path |
| `web` | web/control proxy path |

## Shipped Example Profiles

The repository includes ready-to-read examples:

- `examples/runtime_cloud/runtime-a-dev.toml`
- `examples/runtime_cloud/runtime-b-dev.toml`
- `examples/runtime_cloud/runtime-plant.toml`
- `examples/runtime_cloud/runtime-wan.toml`

Use the example pack when you want complete runnable files instead of overlay
snippets.

## Related

- [Runtime Cloud](../../operate/runtime-cloud.md)
- [Runtime Cloud Federation](../../connect/runtime-to-runtime/runtime-cloud-federation.md)
