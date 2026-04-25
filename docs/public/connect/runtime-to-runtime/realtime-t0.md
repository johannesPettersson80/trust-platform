# Realtime T0

## What T0 is

In the runtime code, T0 is the deterministic same-host HardRT route. It is not
the generic network mesh path.

Relevant contracts in the implementation:

- route class: `RealtimeRoute::T0HardRt`
- QoS tier: `QosTier::T0HardRt`
- transport object: `T0Transport`

The code explicitly rejects using generic mesh/IP routes for T0-only traffic.

## T0 Fits

- publisher and subscriber are on the same host
- you need bounded shared-memory style behavior
- payload layout and schema binding are fixed ahead of time

## Core constraints

T0 assumes:

- fixed-layout payloads
- explicit schema hash binding
- bounded stale-data policy
- bounded spin retries and spin time
- pre-bound channel handles instead of ad hoc generic network requests

## Not for

- plant-wide peer discovery
- normal network mesh sharing
- browser/fleet orchestration
- external-system protocols like MQTT or Modbus TCP

## Benchmark surface

The built-in benchmark evaluates T0 behavior directly:

```bash
trust-runtime bench t0-shm --samples 2000 --output json
```

## Related

- [Transport Matrix](transport-matrix.md)
- [Communication Planes](../../concepts/communication-planes.md)
- [Benchmarks](../../reference/benchmarks.md)
