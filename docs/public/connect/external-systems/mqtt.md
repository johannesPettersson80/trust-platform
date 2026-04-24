# MQTT

## Good fit vs bad fit

| Good fit | Bad fit |
| --- | --- |
| brokered fan-out and pub/sub | strict register-oriented device control |
| remote telemetry and event distribution | hard real-time same-host coordination |
| systems with existing broker ACL/TLS policy | cases where a direct device protocol already exists |

## First things to decide

- what broker boundary is authoritative?
- what topic names represent input and output clearly?
- what reconnect/keep-alive policy is acceptable?
- what TLS/auth requirements apply in this network?

Success means the broker boundary, topic directions, reconnect policy, and
TLS/auth requirements are explicit before messages are trusted as plant state.

## Example and commissioning guide

--8<-- "examples/communication/mqtt/README.md:3"

## Common MQTT gotchas

- using the same topic for `topic_in` and `topic_out`
- unclear ACL direction between publish and subscribe
- testing only against a local open broker and skipping production auth/TLS
- forgetting safe-state behavior for output-critical projects

## Related

- [I/O binding](../devices-and-fieldbus/io-binding.md)
- [Protocol Matrix](../protocol-matrix.md)
