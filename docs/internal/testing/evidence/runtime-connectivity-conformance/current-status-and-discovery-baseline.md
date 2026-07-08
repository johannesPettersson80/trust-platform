# Runtime Connectivity Baseline

Date: 2026-07-04
Branch: `ads/client`
Base commit inspected: `6a7492cae`

Checklist rows covered by this evidence:

- `RTCONN-P0-001`
- `RTCONN-P0-002`
- `RTCONN-P0-003`
- `RTCONN-P0-004`
- `RTCONN-P0-005`
- `RTCONN-P0-012`

## Coordination State

This checkout is already on branch `ads/client`. Phase 2 of the remediation
checklist must not rework ADS status behavior as if ADS client work were a
separate merged baseline. The ADS adapter/status work must either continue on
this branch after rebasing current work, or wait until `ads/client` merges and
then rebase the connector work onto the merged ADS client state.

## Process-Image Drivers

Current process-image drivers live behind `IoDriver` and cycle I/O exchange.

| Driver | Current home | Status source |
| --- | --- | --- |
| simulated | `crates/trust-runtime/src/io/driver.rs` | `IoDriverHealth::Ok` default |
| loopback | `crates/trust-runtime/src/io/loopback.rs` | `IoDriver::health()` |
| GPIO | `crates/trust-runtime/src/io/gpio.rs` | `IoDriver::health()` |
| Modbus TCP | `crates/trust-runtime/src/io/modbus.rs` | `IoDriver::health()` |
| MQTT | `crates/trust-runtime/src/io/mqtt/` | `IoDriver::health()` |
| EtherCAT | `crates/trust-runtime/src/io/ethercat/` | `IoDriver::health()` |

## Supervisory Connectors

Current supervisory connectors use separate subsystem/status models.

| Connector | Current home | Current status model |
| --- | --- | --- |
| ADS client | `crates/trust-runtime/src/runtime/ads_subsystem.rs`, `crates/trust-runtime/src/host/ads/` | `AdsConnectionState`, `AdsConnectionStatusState`, `AdsStatusReport` |
| ADS server | `crates/trust-runtime/src/host/ads/server/` | `AdsStatusReport` with server role |
| OPC UA client | `crates/trust-runtime/src/runtime/opcua_client_subsystem.rs`, `crates/trust-runtime/src/host/opcua/client.rs` | `OpcUaClientConnectionState`, `OpcUaClientStatusReport` |
| OPC UA server | `crates/trust-runtime/src/host/opcua/wire.rs` | server startup/snapshot availability, no shared connector status |

## Status Consumers

Known status consumers before the connector contract:

- Control API status and health handlers in `crates/trust-runtime/src/control/status_handlers.rs`.
- Communication handlers in `crates/trust-runtime/src/control/comm_handlers/`.
- Fleet topology projection in `crates/trust-runtime/src/control/fleet_handlers.rs`.
- Runtime accessors in `crates/trust-runtime/src/runtime/core/accessors.rs`.
- VS Code Network Canvas and communication surfaces under `editors/vscode/src/networkCanvas/`.
- HMI and web status surfaces that consume runtime/control status.
- CLI commands and diagnostics such as `trust-runtime ctl status`, `ads.status`, and ADS doctor routes.

## Known Truth Gaps

- Modbus discovery currently needs a protocol proof or a downgraded
  `port_reachable` label when only TCP connect succeeds.
- MQTT discovery currently needs CONNECT/CONNACK proof or a downgraded
  `port_reachable` label when only TCP connect succeeds.
- ADS server cold start without a runtime snapshot currently needs `not_ready`
  degradation instead of runtime startup failure.
- OPC UA client currently needs a persistent session/subscription worker before
  fleet-scale status can honestly report efficient change-of-state behavior.
- Status names for process-image drivers, ADS, and OPC UA are currently
  separate. The connector contract added in this board is additive and does not
  remove legacy status surfaces yet.
