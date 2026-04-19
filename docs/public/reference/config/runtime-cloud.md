# Runtime Cloud Profiles

This page documents the runtime-cloud portion of `runtime.toml` and the shipped
example profile files under `examples/runtime_cloud/`.

## Core Section

```toml
[runtime.cloud]
profile = "dev"
```

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

Each rule needs:

- `action`
- `target`

## Link Preference Rules

You can express preferred transports between runtimes:

```toml
[runtime.cloud.links]
transports = [
  { source = "runtime-a", target = "runtime-b", transport = "zenoh" }
]
```

Allowed transport values:

- `realtime`
- `zenoh`
- `mesh`
- `mqtt`
- `modbus-tcp`
- `opcua`
- `discovery`
- `web`

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
