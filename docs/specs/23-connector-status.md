# Connector Status Contract

This document defines the canonical connector-status vocabulary shared by the
runtime API, CLI and reports, browser HMI, and VS Code surfaces.

## Canonical Vocabulary

Connector lifecycle state is exactly one of `disabled`, `configured`,
`starting`, `ready`, `degraded`, `reconnecting`, `stale`, `not_ready`, or
`faulted`. Connector health is exactly one of `ok`, `degraded`, `faulted`, or
`unknown`.

Discovery confidence is exactly one of `confirmed`, `likely`,
`port_reachable`, or `unavailable`. Point quality is exactly one of `good`,
`stale`, `bad`, `unsupported`, `unavailable`, `write_pending`, or
`write_failed`.

Connector protocol identifiers are stable lowercase wire values. The closed
set is `ads`, `opcua`, `modbus_tcp`, `mqtt`, `ethercat`, `gpio`, `simulated`,
`loopback`, and `unknown`. An explicitly unclassified connector serializes as
`unknown` and never implies healthy protocol evidence.

## Versioned Report Shape

A connector report serializes its schema version, stable connector ID,
protocol, kind, state, health, confidence, and point-count summary. Display
name, endpoint, reconnect policy, last error, last transition time, freshness,
and point rows are present only when the producer has that authority. Each
point row preserves name, optional source and data type, read/write direction,
quality, optional last update, and optional detail.

Point counts are derived from the emitted point rows or from an authoritative
protocol report and contain `total`, `good`, `degraded`, and `unavailable`.
`write_pending`, `write_failed`, `stale`, and `bad` contribute to degraded
rather than good. A serializer-only test that constructs `ready`/`ok` values
proves this versioned shape, not that a transport earned those values.

## Projection Rules

Missing freshness evidence is not itself a `stale` state. A stale state needs
positive evidence that previously usable data exceeded its freshness bound.
`degraded` and `faulted` projections preserve available error detail and point
quality rather than replacing them with a generic label. `disabled`,
`reconnecting`, `stale`, `degraded`, and `faulted` remain distinct.

Wire identifiers are stable lowercase values. Human-facing surfaces may add
presentation text, but HMI, CLI, reports, hover, and VS Code must preserve the
same state and health meaning. An unknown wire identifier fails visibly at a
closed mapping boundary; it is not silently promoted to a healthy state.

`IoDriverHealth::Ok` projects to `ready`/`ok`. Degraded health preserves its
detail and projects to `degraded`/`degraded`. Faulted health under `fault`
projects to `faulted`/`faulted`; under `warn` or `ignore`, the runtime may keep
running but the connector remains `degraded`/`degraded` with the failure
detail.

ADS point quality maps `Good` to `good`, ordinary stale data to `stale`, the
explicit detail `ADS write pending` to `write_pending`, ordinary errors to
`bad`, and an ADS write-failure detail to `write_failed`. ADS client reports
derive stable role-qualified IDs and endpoint text from target identity;
degraded-point counts lower a connected report from `ready`/`ok` to
`degraded`/`degraded`. An active ADS device snapshot emits the same point rows
and counts as the corresponding connector report.

OPC UA client reports preserve the configured connection name, endpoint, node
ID, declared IEC-facing data type, access direction, freshness, and point
quality. A connected client with no degraded points is `ready`/`ok`; degraded
points produce `degraded`/`degraded`. A server without its first runtime
snapshot is `not_ready`/`unknown`; a current snapshot may become `ready`/`ok`.

For process-image drivers, a fresh MQTT session is `ready`, a stale session is
`stale`, a Modbus timeout is degraded, a faulted Modbus worker is faulted, an
operational EtherCAT driver is healthy, and an EtherCAT reconnect is
`reconnecting`. These projections report runtime evidence only and do not turn
mock or TCP-only observations into physical-device proof.

Peer topology is evaluated per peer. An invalid peer state, health,
confidence, or point-quality value must preserve the peer and the remaining
valid topology while rendering a visible error for the malformed peer. One
malformed peer must not make itself or other configured peers silently vanish.
