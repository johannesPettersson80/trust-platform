# Runtime Connector State Mapping Review

Date: 2026-07-04
Implementation under review:

- `crates/trust-runtime/src/connectors/contract.rs`
- `crates/trust-runtime/src/connectors/mapping.rs`
- `crates/trust-runtime/tests/connectors_status.rs`

Checklist rows covered by this evidence:

- `RTCONN-P1-013`
- `RTCONN-P1-014`
- `RTCONN-P1-015`

## Target Contract

Connector state values:

- `disabled`
- `configured`
- `starting`
- `ready`
- `degraded`
- `reconnecting`
- `stale`
- `not_ready`
- `faulted`

Connector health values:

- `ok`
- `degraded`
- `faulted`
- `unknown`

`stale` is intentionally present in two places:

- Connector state `stale`: the whole connector is no longer fresh.
- Point quality `stale`: one point is old while the connector may still be
  partially usable.

## Mapping Table

### IoDriver

| Source | Error policy | Connector state | Health |
| --- | --- | --- | --- |
| `IoDriverHealth::Ok` | any | `ready` | `ok` |
| `IoDriverHealth::Degraded` | any | `degraded` | `degraded` |
| `IoDriverHealth::Faulted` | `fault` | `faulted` | `faulted` |
| `IoDriverHealth::Faulted` | `warn` | `degraded` | `degraded` |
| `IoDriverHealth::Faulted` | `ignore` | `degraded` | `degraded` |

Rationale: status remains truthful about degraded transport/device condition
even when the configured runtime policy does not fault the scan.

Phase 1 control-route note: `connectors.status` projects the current
`ControlState.io_health` snapshot with conservative `IoDriverErrorPolicy::Fault`
semantics because `IoDriverStatus` currently carries name and health only, not
the configured `io.on_error` policy. This is an additive status projection, not
a new config schema or an `IoDriver` trait change.

### ADS Worker State

| Source | Connector state | Health |
| --- | --- | --- |
| `AdsConnectionState::Disconnected` | `stale` | `degraded` |
| `AdsConnectionState::Connecting` | `starting` | `unknown` |
| `AdsConnectionState::Connected` | `ready` | `ok` |
| `AdsConnectionState::Reconnecting` | `reconnecting` | `degraded` |
| `AdsConnectionState::Faulted` | `faulted` | `faulted` |

### ADS Report State

| Source | Degraded points | Connector state | Health |
| --- | --- | --- | --- |
| `Connected` | `0` | `ready` | `ok` |
| `Connected` | `>0` | `degraded` | `degraded` |
| `Reconnecting` | any | `reconnecting` | `degraded` |
| `NotReady` | any | `not_ready` | `unknown` |
| `Faulted` | any | `faulted` | `faulted` |
| `Stale` | any | `stale` | `degraded` |
| `Disabled` | any | `disabled` | `unknown` |
| `Unknown` | any | `not_ready` | `unknown` |

### OPC UA Client

| Source | Degraded points | Connector state | Health |
| --- | --- | --- | --- |
| `Disabled` | any | `disabled` | `unknown` |
| `Configured` | any | `configured` | `unknown` |
| `Connected` | `0` | `ready` | `ok` |
| `Connected` | `>0` | `degraded` | `degraded` |
| `Reconnecting` | any | `reconnecting` | `degraded` |
| `Stale` | any | `stale` | `degraded` |
| `Faulted` | any | `faulted` | `faulted` |

### OPC UA Server Snapshot State

| Source | Connector state | Health |
| --- | --- | --- |
| Disabled | `disabled` | `unknown` |
| Starting | `starting` | `unknown` |
| No snapshot | `not_ready` | `unknown` |
| Snapshot ready | `ready` | `ok` |
| Faulted | `faulted` | `faulted` |

### MQTT

| Source | Connector state | Health |
| --- | --- | --- |
| Disabled | `disabled` | `unknown` |
| Disconnected | `stale` | `degraded` |
| Connecting | `starting` | `unknown` |
| Connected fresh | `ready` | `ok` |
| Connected stale | `stale` | `degraded` |
| Faulted | `faulted` | `faulted` |

### Modbus

| Source | Connector state | Health |
| --- | --- | --- |
| Disabled | `disabled` | `unknown` |
| Ready | `ready` | `ok` |
| Timeout | `degraded` | `degraded` |
| Protocol error | `degraded` | `degraded` |
| Faulted | `faulted` | `faulted` |

### EtherCAT

| Source | Connector state | Health |
| --- | --- | --- |
| Disabled | `disabled` | `unknown` |
| Operational | `ready` | `ok` |
| Degraded | `degraded` | `degraded` |
| Reconnecting | `reconnecting` | `degraded` |
| Faulted | `faulted` | `faulted` |

## Review Decision

Accepted for Phase 1 scaffold. The mapping is intentionally conservative:
unknown and no-snapshot states do not become healthy, transport loss is at least
degraded, and configured-but-unproven states remain `unknown` health until a
protocol-specific adapter has live evidence.
