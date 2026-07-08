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
- are messages raw process-image bytes or typed scalar point topics?
- what reconnect/keep-alive policy is acceptable?
- what TLS/auth requirements apply in this network?

Success means the broker boundary, topic directions, payload format, reconnect
policy, and TLS/auth requirements are explicit before messages are trusted as
plant state.

The default profile is a raw byte bridge: `topic_in` payload bytes are copied
into `%I`, and `%Q` bytes are published to `topic_out`. Use `input_points` and
`output_points` when MQTT topics carry typed scalar values instead. Each point
names the topic, process-image byte offset, optional bit for `bool`, data type
(`bool`, `u16`, `i16`, `u32`, `i32`, or `f32`), payload format (`text`, `json`,
`binary_le`, or `binary_be`), and optional `scale`/`offset`. Scaling converts
input raw values as `engineering = raw * scale + offset`; output writes invert
that formula before publishing.

This typed scalar map is intentionally separate from Sparkplug B. For Sparkplug
B outbound node metrics, set `[io.params.sparkplug]` with `enabled = true`,
`namespace = "spBv1.0"`, `spec_version = "3.0.0"`, `group_id`, and
`edge_node_id`, then define typed `output_points` with stable `metric_name`
values. The runtime publishes NBIRTH on connect, configures NDEATH as the MQTT
last will, and publishes NDATA payloads from typed output points using the Tahu
scalar protobuf wire shape.

Sparkplug support is intentionally bounded today: outbound node metrics only.
Commands, device-level DBIRTH/DDATA topics, aliases, templates, and
store-and-forward are not part of this profile.

At runtime, MQTT exchange is worker-backed. Scan-cycle reads copy the latest
fresh worker snapshot or return the configured `on_error` policy result, and
scan-cycle writes hand off the latest desired output without waiting for broker
readiness or publish latency. The MQTT worker owns broker connect/poll/publish,
reconnect backoff, Sparkplug birth/data publishing, and stale/degraded health
reporting through the normal driver status surface.

## Example and commissioning guide

--8<-- "examples/communication/mqtt/README.md:3"

## Common MQTT gotchas

- using the same topic for `topic_in` and `topic_out`
- unclear ACL direction between publish and subscribe
- mixing raw byte topics and typed point topics without documenting which
  profile owns the process image
- enabling Sparkplug without stable `group_id`, `edge_node_id`, and
  `metric_name` values
- using binary MQTT payload endianness as if it changed `%IW`/`%ID`
  process-image byte order
- testing only against a local open broker and skipping production auth/TLS
- forgetting safe-state behavior for output-critical projects

## Related

- [I/O binding](../devices-and-fieldbus/io-binding.md)
- [Protocol Matrix](../protocol-matrix.md)
