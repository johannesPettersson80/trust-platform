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

Every runtime belongs to one site. A runtime names the runtimes of its own site, itself among them,
by their resource names, for example `runtime_c`, and a runtime of another site by
`<site>/<resource>`, for example `site-b/runtime_b`; a runtime of the default site is
`default-site/<resource>` to a runtime of another site. These IDs appear in action requests
(`connected_via`, `target_runtimes`), in the Runtime Cloud state and in the WAN allowlist's targets.
Discovery announces each runtime's site, so every runtime knows the site of the runtimes it
discovers.

```toml
[runtime.cloud]
profile = "wan"
site = "site-b"
```

Because the names are relative to the runtime's own site, a project whose runtimes are given a site
keeps the names it already uses for its own runtimes. A target is cross-site when its site differs
from the runtime's own site. The site is read when the runtime starts; `config.get` reports it as
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
