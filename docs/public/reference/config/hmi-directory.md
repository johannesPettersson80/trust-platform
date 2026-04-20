# `hmi/`

The `hmi/` directory holds declarative HMI pages, SVG process boards, trends,
alarms, and write policy.

## Typical Layout

```text
hmi/
  _config.toml
  overview.toml
  trends.toml
  alarms.toml
  plant.toml
  plant.svg
```

## Important Files

| File | Purpose |
| --- | --- |
| `_config.toml` | global HMI settings, refresh, theme, write policy |
| `overview.toml` | dashboard-style operator page |
| `trends.toml` | trend widgets and time-series views |
| `alarms.toml` | alarm list / acknowledgement view |
| `<page>.toml` | page definition with widgets or process bindings |
| `<page>.svg` | process artwork referenced from `kind = "process"` pages |

## Write Policy

| policy_mode | `enabled` | `default_role` | `allowlist` | Example |
| --- | --- | --- | --- | --- |
| read-only | `false` | `viewer` | `[]` | alarms/trends only |
| controlled writes | `true` | `operator` | explicit symbol list | start/stop buttons, setpoints |

Minimal read-only policy:

```toml
[write]
enabled = false
default_role = "viewer"
allowlist = []
```

Controlled write policy:

```toml
[write]
enabled = true
default_role = "operator"
allowlist = [
  "PROGRAM PumpStation.PumpSpeed",
  "GLOBAL Control.StartButton",
]
```

## Process Pages

Process pages bind live symbols to SVG selectors:

```toml
title = "Plant"
kind = "process"
svg = "plant.svg"

[[bind]]
selector = "#pump_state"
attribute = "class"
source = "PROGRAM PumpStation.Run"
map = { "true" = "running", "false" = "stopped" }
```

## Lifecycle Commands

Use:

- `trust-runtime hmi init`
- `trust-runtime hmi update`
- `trust-runtime hmi reset`

## Related

- [HMI Authoring](../../develop/hmi-authoring.md)
- [HMI And Web UI](../../operate/hmi-and-web-ui.md)
