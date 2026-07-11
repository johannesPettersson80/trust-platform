# `ads.toml`

`ads.toml` defines Beckhoff ADS client connections and the TwinCAT points that
generate ST globals. It is a project source file, not runtime-only state.
The VS Code ADS panel and the `/setup/ads` runtime-host wizard generate and
review this file with the cached symbol snapshot and generated ST file. Enable
it at runtime with `[runtime.ads]` in `runtime.toml`.

Unknown fields are rejected. Use `trust-runtime ads validate --offline` to
prove the checked-in generated ST still matches this config and the committed
symbol snapshot without connecting to a PLC.

## Minimal Example

```toml
[[connections]]
name = "line1"
target_net_id = "5.23.91.12.1.1"
host = "192.168.10.5"
ams_port = 851
transport = "plain"
insecure_transport = true

[[connections.points]]
symbol = "MAIN.Temperature"
var = "line1_temp"
type = "REAL"
mode = "poll"

[[connections.points]]
symbol = "GVL.Setpoint"
var = "line1_setpoint"
type = "REAL"
access = "write"
```

## Connection Entries

Use one `[[connections]]` entry per ADS route.

| Key | Type | Required | Default | Notes |
| --- | --- | --- | --- | --- |
| `name` | string | yes | none | Unique connection name. |
| `target_net_id` | string | yes | none | Remote TwinCAT AMS Net ID. |
| `host` | string | yes | none | Remote host/IP used for the ADS route. |
| `ams_port` | integer `1..=65535` | no | `851` | Logical ADS server and symbol namespace. Common examples are `301` for Additional Task 1, `501` for NC SAF, and `852+` for additional PLC runtimes; availability and Symbol Upload support depend on the TwinCAT project. |
| `local_net_id` | string | no | none | Local AMS Net ID override. Generated onboarding configs pin this to the runtime-host identity proven by the Doctor. |
| `transport` | string | no | `secure` | `secure` or `plain`. Classic ADS uses `plain`. |
| `insecure_transport` | bool | no | `false` | Required acknowledgement for `transport = "plain"`. |
| `auto_add_route` | bool | no | `false` | Reserved for explicit route-writing workflows. |

`transport = "plain"` without `insecure_transport = true` is rejected. Setting
`insecure_transport = true` without `transport = "plain"` is also rejected.

Changing `ams_port` selects a different ADS server on the same AMS Net ID; it
does not perform a global search across ports. Devices & Connections preserves
the selected port through symbol browsing, import, and the generated
`ads.toml`. A reachable port may still return no symbols when that server does
not implement Symbol Upload or when symbol generation is disabled.

## Point Entries

Use nested `[[connections.points]]` entries under the owning connection.

| Key | Type | Required | Default | Notes |
| --- | --- | --- | --- | --- |
| `var` | string | yes | none | Generated ST global name. Each `var` may appear once. |
| `symbol` | string | symbol path | none | TwinCAT symbol name such as `MAIN.Temperature`. |
| `index_group` | integer | index path | none | ADS index group for advanced addressing. |
| `index_offset` | integer | index path | none | ADS index offset for advanced addressing. |
| `size` | integer | index path | none | Byte size for index addressing. Must be `>= 1`. |
| `type` | string | yes | none | IEC scalar type assertion. |
| `string_len` | integer | string types | from `STRING(n)` | Explicit `STRING` capacity. |
| `dimensions` | array | no | `[]` | Scalar array dimensions. |
| `access` | string | no | `read` | `read`, `write`, or `read_write`. |
| `mode` | string | no | `poll` | `poll` or `notify`. |
| `notification_mode` | string | no | `on_change` | `on_change` or `cyclic`, only valid with `mode = "notify"`. |
| `allow_retain_read` | bool | no | `false` | Allows ADS reads into retained globals after explicit review. |

A point must use exactly one address form:

- symbolic: `symbol = "MAIN.Temperature"`
- index: `index_group`, `index_offset`, and `size`

## Supported Types

Scalar points support:

```text
BOOL SINT INT DINT LINT USINT UINT UDINT ULINT
REAL LREAL BYTE WORD DWORD LWORD STRING STRING(n)
```

Bind STRUCTs by leaf member, for example `Axis1.Status.ActualPosition`, rather
than importing a whole struct blob.

Array dimensions use inclusive IEC bounds:

```toml
[[connections.points]]
symbol = "GVL.Temperatures"
var = "line1_temps"
type = "REAL"
dimensions = [{ lower = 1, upper = 8 }]
```

## Generated Output

The generated ST file contains one `ADS_QUALITY` enum and one value/quality
pair per point:

```iecst
TYPE
    ADS_QUALITY : (Stale := 0, Good := 1, Error := 2);
END_TYPE

VAR_GLOBAL
    line1_temp : REAL;
    line1_temp_quality : ADS_QUALITY := Stale;
END_VAR
```

Use a single generated ADS file per project, for example
`src/generated/ads_generated.st`, so `ADS_QUALITY` is defined once.

## Runtime Activation

`ads.toml` is loaded by a project bundle when `[runtime.ads]` is enabled:

```toml
[runtime.ads]
enabled = true
config_path = "ads.toml"
worker_tick_interval_ms = 20
```

At startup, the runtime resolves each ADS point to the declared generated
global, starts one background ADS worker per connection, and applies read
values plus `_quality` values at the scan input phase. Write/read-write points
are captured at the output phase and drained by the worker. Live ADS I/O
requires a runtime built with feature `ads-wire`; without it, enabled ADS fails
startup explicitly.

## Commands

```bash
trust-runtime ads import-symbols \
  --target 192.168.10.5 \
  --target-net-id 5.23.91.12.1.1 \
  --connection line1 \
  --out ads.toml \
  --gen src/generated/ads_generated.st

trust-runtime ads validate --offline \
  --config ads.toml \
  --snapshot ads/snapshots/line1.symbols.json \
  --generated src/generated/ads_generated.st
```

Validate the same generated ST against the live TwinCAT symbol table:

```bash
trust-runtime ads validate --live \
  --config ads.toml \
  --generated src/generated/ads_generated.st
```

Live validation connects to every configured ADS connection, uploads the current
compatible symbol table, and compares the generated ST against those live
symbols. Use it after TwinCAT online changes and before treating a deployed
bundle as production-ready.

The authoring/import front ends use the scriptable command below to preview all
generated files before writing:

```bash
trust-runtime ads import-symbols \
  --target 192.168.10.5 \
  --target-net-id 5.23.91.12.1.1 \
  --connection line1 \
  --out ads.toml \
  --gen src/generated/ads_generated.st \
  --dry-run \
  --json
```

Live discovery, doctor, route-add, and symbol import require a runtime binary
built with feature `ads-wire`. Runtime live I/O is separate from the CLI and is
controlled by `[runtime.ads]` plus the same `ads-wire` build feature.

## Related

- [Beckhoff ADS](../../connect/external-systems/ads.md)
- [`trust-runtime`](../cli/trust-runtime.md)
- [Connectivity Examples](../../examples/connectivity.md)
