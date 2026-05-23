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
  drive-cell.toml
  views/
    drive-cell.topology.toml
    drive-cell.view.toml
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
| `views/<name>.topology.toml` | human/AI-authored 3D component topology source |
| `views/<name>.view.toml` | static 3D scene payload referenced from `kind = "scene3d"` pages |

## Write Policy

### `[write]`

| Key | Type | Required | Notes |
| --- | --- | --- | --- |
| `enabled` | bool | yes | Enables runtime-side writes from the HMI. |
| `default_role` | string | yes | Default role for write-capable actions. |
| `allowlist` | string array | yes | Explicit symbol allowlist. |

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

### `[[bind]]`

| Key | Type | Required | Notes |
| --- | --- | --- | --- |
| `selector` | string | yes | SVG/CSS selector to target. |
| `attribute` | string | yes | Attribute to update. |
| `source` | string | yes | Runtime symbol path. |
| `map` | table | no | Value-to-attribute mapping. |

## 3D Scene Pages

Scene pages reference a compiled view payload under `hmi/views/` and keep live
tag bindings in the page descriptor:

```toml
title = "Drive Cell 3D"
kind = "scene3d"
topology = "drive-cell.topology.toml"
view = "drive-cell.view.toml"

[[bind3d]]
node = "motor-1.shaft"
property = "transform.rotation.y"
source = "Main.shaft_angle"
scale = { min = -3.14159265, max = 3.14159265, output_min = -3.14159265, output_max = 3.14159265 }
```

The loader resolves `view = "drive-cell.view.toml"` to
`hmi/views/drive-cell.view.toml`. View payload files are not page descriptors.

### Topology Sources

`hmi/views/<name>.topology.toml` is the normal human and AI authoring source for
3D pages. The topology compiler emits the `.view.toml` file used by the runtime
and writes a generated header containing the topology source hash.

```toml
[[components]]
id = "TK-101"
kind = "tank"
at = { grid = "A1" }

[[components]]
id = "P-101"
kind = "pump"
at = { grid = "A3" }

[[connections]]
id = "line-101"
from = "TK-101.outlet"
to = "P-101.inlet"
medium = "water"
diameter = "DN50"
route = "auto"

[[bindings]]
component = "TK-101"
signal = "level"
source = "Program.TK101.level"
access = "read"

[[interactions]]
component = "P-101"
event = "click"
action = "hmi.write"
id = "resource/RESOURCE/program/Main/field/run"
value = true
required_role = "Engineer"
confirmation = { title = "Start pump", message = "Write Main.run TRUE" }
```

The built-in v1 component library ships with `tank`, `pump`, `valve`, `motor`,
`vfd`, and `transmitter` kinds. Each kind defines ports, domains, default
primitive visuals, and signal-to-`bind3d` mappings. The compiler validates
component kinds, ports, domains, grid uniqueness, raw-coordinate justification,
binding signal names, and write interaction role safety before emitting the
view payload.

### View Payloads

P1 view payloads can define primitive scene nodes, cameras, lights, and
low-level `bind3d` records:

```toml
[[node]]
id = "motor-1.shaft"
primitive = "box"

[node.transform]
position = [0.0, 0.0, 0.0]
rotation = [0.0, 0.0, 0.0]
scale = [1.0, 0.35, 0.35]

[node.material]
base_color = "#3b82f6"

[[node.interaction]]
event = "click"
action = "hmi.write"
id = "resource/RESOURCE/program/Main/field/run"
value = true
required_role = "Engineer"
confirmation = { title = "Start motor", message = "Write Main.run TRUE" }

[[camera]]
id = "main"
position = [0.0, 0.0, 4.0]
target = [0.0, 0.0, 0.0]

[[light]]
id = "key"
kind = "directional"
intensity = 1.0
```

The P1 runtime bridge renders primitive nodes through `scena`. Asset-backed
nodes remain a later trust-twin slice.

### `[[node.interaction]]`

Compiled scene nodes can expose operator interactions. P3 supports `hmi.write`
only, and the runtime/webview route that request through the same control
endpoint, role policy, allowlist, parser, pending-write queue, and audit path
used by 2D HMI writes.

| Key | Type | Required | Notes |
| --- | --- | --- | --- |
| `event` | string | yes | `click`, `touch`, or `toggle`. |
| `action` | string | yes | `hmi.write` in P3. |
| `id` | string | yes | HMI write target id/path passed as `params.id`. |
| `value` | bool/string/number | yes | Value passed as `params.value`. |
| `required_role` | string | yes | Must be `Engineer`; lower roles are rejected by policy. |
| `confirmation` | table | no | `title` and `message` metadata for operator confirmation. |

### `[[bind3d]]`

| Key | Type | Required | Notes |
| --- | --- | --- | --- |
| `node` | string | yes | Scene node id to target. |
| `property` | string | yes | One of the bounded 3D property names. |
| `source` | string | yes | Runtime symbol path, validated like 2D HMI bindings. |
| `map` | table | no | Value-to-property mapping. |
| `scale` | table | no | Numeric input/output scaling. |

Supported P1 properties are `visible`, `transform.position`,
`transform.position.x`, `transform.position.y`, `transform.position.z`,
`transform.rotation.x`, `transform.rotation.y`, `transform.rotation.z`,
`transform.scale`, `transform.scale.x`, `transform.scale.y`,
`transform.scale.z`, `material.base_color`, `material.emissive`,
`material.opacity`, and `text.value`.

## Lifecycle Commands

Use:

- `trust-runtime hmi init`
- `trust-runtime hmi update`
- `trust-runtime hmi reset`

## Related

- [HMI Authoring](../../develop/hmi-authoring.md)
- [HMI And Web UI](../../operate/hmi-and-web-ui.md)
