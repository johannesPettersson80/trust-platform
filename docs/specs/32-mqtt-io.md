# MQTT Communication and I/O Contract

## 1. Scope and ownership

This document is the authoritative truST product specification for MQTT broker
communication and MQTT-backed PLC I/O. It owns MQTT configuration, topics,
mapping direction, payload conversion, security, sessions, reconnection,
Sparkplug B behavior, discovery evidence, and broker interoperability.

[`11-runtime-engine.md`](11-runtime-engine.md) owns only the runtime integration
port: when symbolic tags are lowered, how process-image bindings are installed,
how scan calls exchange bounded state with the protocol worker, and how driver
health enters the common runtime fault path. [`23-connector-status.md`](23-connector-status.md)
owns the shared connector vocabulary. Neither shared document may redefine
MQTT protocol behavior.

MQTT is a truST host/product protocol outside IEC 61131-3. These requirements
are not IEC decisions or deviations.

Implementation owners:

- `crates/trust-runtime/src/io/mqtt/**`: MQTT configuration, payload codecs,
  session, worker, driver, and Sparkplug profile;
- `crates/trust-runtime/src/io/mqtt_tag_mapping.rs`: startup-only adapter from
  compiled PLC symbols to typed process-image points;
- `crates/trust-runtime/src/io/interface.rs`: shared binding transaction and
  snapshot metadata, without broker knowledge; and
- `crates/trust-runtime/src/connectors/adapters/mqtt.rs`: projection into the
  shared connector-status contract, without transport behavior.

## 2. Project configuration authority

For a project-folder run, the runtime loads `io.toml` from the project root. An
MQTT driver is selected by `[io] driver = "mqtt"`; its protocol parameters live
under `[io.params]`. `mappings` is valid under `[io.params]` as either an inline
array or TOML array-of-tables such as `[[io.params.mappings]]`.

Unknown MQTT parameter fields and unknown mapping fields are configuration
errors. They are not ignored. Configuration and every symbolic mapping must be
validated completely before the MQTT worker starts; failure leaves no partial
binding or partially started MQTT session.

The shipped traffic-light mapping is:

```toml
[io]
driver = "mqtt"

[io.params]
broker = "mqtt://127.0.0.1:1883"

[[io.params.mappings]]
tag = "MainInstance.Green"
topic = "traffic/north/green"
direction = "write"

[[io.params.mappings]]
tag = "MainInstance.Yellow"
topic = "traffic/north/yellow"
direction = "write"

[[io.params.mappings]]
tag = "MainInstance.Red"
topic = "traffic/north/red"
direction = "write"
```

## 3. Endpoint, credentials, and transport security

The broker endpoint supplies host and port. A plain `mqtt://` or `tcp://`
endpoint is permitted for loopback/local authority. A remote plaintext broker
requires `allow_insecure_remote = true`. `mqtts://` and `ssl://` imply TLS, and
an explicit `tls = false` conflicts with a TLS scheme.

TLS requires a readable `tls_ca_path`. Client certificate and private-key paths
form an all-or-nothing mTLS pair. TLS-only fields are rejected when TLS is
disabled, except that an empty ALPN list has no effect. The broker host name is
used for TLS server-name verification. Successful TLS object construction does
not by itself prove a broker handshake.

Username and password are an all-or-nothing pair. Secrets must not appear in
connector reports, audit details, discovery results, or diagnostics.

## 4. Exchange modes and fallback selection

MQTT supports three explicit exchange forms:

1. Raw mode copies the newest `topic_in` payload to `%I` and publishes `%Q` to
   `topic_out`.
2. Typed point mode uses `input_points` and `output_points` with explicit
   process-image offsets, types, payload formats, and optional scaling.
3. Symbolic mapping mode uses `mappings` to resolve fully qualified PLC scalar
   tags and lower them to typed points before the worker starts.

Raw `topic_in` and `topic_out` behavior is the default only when `mappings` is
absent. Presence selects mapped mode even when `mappings = []`. Therefore:

- write-only symbolic mappings create no implicit `trust/io/in` subscription,
  require no inbound snapshot, and do not mutate input bytes owned by another
  driver;
- read-only symbolic mappings publish neither mapped output nor the raw shared
  output image;
- an explicitly empty mapping list disables both raw directions; and
- independently configured `input_points` and `output_points` remain active in
  mapped mode, including when the symbolic mapping list is empty.

The configured default raw topics are `trust/io/in` and `trust/io/out`. Seeing
a runtime subscription to `trust/io/in` when a write-only mapping list is
present is a configuration-mode defect, not evidence that the PLC scan stopped.

## 5. Mapping identity and direction

Every symbolic mapping contains exactly `tag`, `topic`, and `direction`.
Direction is relative to the PLC:

- `direction = "write"` transfers the PLC value to the broker; and
- `direction = "read"` transfers the broker value to the PLC.

A tag names one fully qualified scalar program-instance variable, for example
`MainInstance.Green`. Startup resolves every tag after bytecode/program metadata
is available and before constructing the protocol worker. A missing, ambiguous,
non-scalar, or unsupported tag is a configuration error. Resolution is atomic:
no subset may remain installed after one mapping fails.

Existing compatible direct `%I` or `%Q` bindings are reused. Otherwise the
startup adapter allocates deterministic non-overlapping process-image storage
after the declared image. The worker receives only typed point descriptions and
process-image snapshots; it cannot access program storage or resolve symbols.

## 6. Scalar and enumeration contract

The common MQTT scalar set is `bool`, `u16`, `i16`, `u32`, `i32`, and `f32`.
Documented IEC-style and explicit-width aliases select the same scalar
case-insensitively. Symbolic mappings infer the PLC scalar type and use text
payloads. A subrange uses its declared base scalar representation.

An enumeration mapping carries the numeric value of its declared IEC member on
the MQTT wire while preserving the declared enumeration TypeId in program
storage. Inbound numeric values reconstruct the matching member. An undeclared
numeric value is rejected before any input binding is committed. Aliases retain
the declared enumeration identity rather than degrading to the integer base.

Runtime I/O snapshots retain the declared enumeration type name in `valueType`
for resolved values, unresolved values, and I/O conversion errors. Shared
control and debug serialization consume this metadata without learning MQTT
semantics.

## 7. Typed points and payload conversion

An MQTT point topic is trimmed, nonempty, contains no control character, and is
an exact publish/delivery topic rather than a `+` or `#` subscription filter.
Within one input or output map, duplicate topics and overlapping process-image
storage are configuration errors. Input and output may use the same offset
because `%I` and `%Q` are distinct images.

Boolean points require `image_bit` in `0..=7`; numeric points reject it. Numeric
process-image values use little-endian storage and occupy exactly their declared
width. MQTT binary payload byte order affects the wire payload only.

Payload formats are text, JSON, binary little-endian, and binary big-endian,
including documented aliases. Omitted format means text. Text and JSON numeric
strings may contain surrounding whitespace but must contain one complete finite
scalar. JSON Boolean input accepts a Boolean, number, or recognized Boolean
string. JSON numeric input accepts a number or numeric string. Binary Boolean
payloads contain exactly one byte; numeric binary payloads contain exactly the
declared width. Truncated or extra bytes are rejected.

Inbound scaling is `engineering = raw * scale + offset`; outbound scaling is
`raw = (engineering - offset) / scale`. Scale is finite and nonzero, offset is
finite, and Boolean points reject scaling. Integer conversion rounds half-way
values away from zero and then checks the declared range. F32 conversion must
remain finite after narrowing. Any failed conversion leaves the complete
destination unchanged; a partially converted MQTT batch is never committed.

## 8. Session, worker, freshness, and failure behavior

Broker connect, poll, publish, and reconnect run on the MQTT worker. PLC
scan-cycle reads and writes use bounded in-memory snapshot/handoff state. A
broker outage, absent inbound message, pending publish, or reconnect attempt
cannot block the scheduler indefinitely or hold the PLC at scan zero.

When one raw-input worker operation drains multiple messages, the newest payload
is the authoritative snapshot. Reconnection uses a nonzero bounded backoff and
cannot busy-loop. Driver drop requests shutdown and returns within its bounded
deadline even while a first connection is pending; this does not claim that a
detached worker has already joined.

Under the default `fault` policy, disconnected or stale input, a bounded read
without a snapshot, missed refresh/handoff deadlines, completed connection or
publish failure, and typed-output preflight failure return an error and set
faulted health. Connection errors retain their context rather than becoming a
generic unavailable-snapshot message. A faulted stale read leaves the caller's
input image unchanged. `warn` and `ignore` retain their documented degraded
success behavior. A timed-out queued output may later publish, but that does not
retroactively make the timed-out scan call successful.

## 9. Sparkplug B outbound profile

The optional Sparkplug B node profile applies to typed `output_points` and uses
namespace `spBv1.0`, specification version `3.0.0`, required `group_id`, and
required `edge_node_id`. It requires at least one output point and no input
points.

Session establishment publishes NBIRTH, configures NDEATH as the MQTT last
will, and then publishes NDATA scalar metrics. Topics are
`<namespace>/<group_id>/NBIRTH/<edge_node_id>`,
`<namespace>/<group_id>/NDEATH/<edge_node_id>`, and
`<namespace>/<group_id>/NDATA/<edge_node_id>`. Birth precedes data for each
accepted session. Metric names are trimmed, nonempty, and contain no control
characters.

Device topics, DBIRTH/DDATA, templates, aliases, command subscriptions, and
store-and-forward are not claimed. Structural protobuf bytes or substring
assertions alone do not prove reference Eclipse Tahu interoperability.

## 10. Discovery and test evidence

An MQTT discovery probe uses MQTT 3.1.1 CONNECT with clean-session true and
immediately sends DISCONNECT after a valid CONNACK. Authentication rejection is
distinct from a non-MQTT endpoint. A TCP listener on port 1883 without a valid
MQTT exchange is only `port_reachable`, never confirmed MQTT. MQTTS port
reachability without a TLS MQTT exchange is likewise not protocol proof.

The discovery CONNECT packet uses keepalive zero, no credential or will flags,
and a bounded UTF-8 client ID. Remaining Length uses the canonical shortest
MQTT base-128 representation, is limited to four bytes and 268,435,455, and
rejects overlong, non-minimal, truncated, or overflowing encodings. CONNACK is
exactly packet type `0x20`, remaining length 2, session-present zero or one, and
return code 0 through 5. Reserved acknowledgement flags, extra bytes, unknown
return codes, and session-present on a rejected connection are malformed input.
Return code zero is `confirmed`; codes 4 and 5 are `likely` with
`auth_required = true`; other valid rejection codes are `likely` without that
flag. Every valid CONNACK, including a rejection, is followed by DISCONNECT.

Discovery is directed only. A missing host returns no candidates and an
explicit directed-host warning. A directed target accepts a host, IPv4 address,
or bracketed IPv6 address. With no explicit port, distinct resolved addresses
are probed at 1883 and 8883 in deterministic address/port order; an explicit
nonzero port probes only that endpoint. Userinfo, paths, queries, fragments,
invalid ports, ambiguous unbracketed IPv6, duplicate socket addresses, and
unresolved targets reject before probing. A port-8883 TCP connection remains
only MQTTS reachability until a TLS MQTT exchange succeeds.

The communication Test operation is intentionally weaker than discovery. It is
a bounded TCP-reachability probe, not an MQTT handshake. It accepts `mqtt://`
and `tcp://` with default port 1883 and `mqtts://` and `ssl://` with default
port 8883. Its positive evidence may claim only that the resolved socket
accepted a TCP connection.

Release interoperability for symbolic output mapping runs the shipped
traffic-light project with the shipped runtime against a real Mosquitto process
and observes it with a real `mosquitto_sub` client. It must observe
`traffic/north/green`, `traffic/north/yellow`, and `traffic/north/red` changing
between Boolean true and false. Broker evidence must show that the runtime did
not subscribe to `trust/io/in`. An in-process transport double cannot satisfy
this interoperability requirement.

The interoperability harness may report the broker version after those
assertions. A Mosquitto help command that emits a usable version line and then
returns a nonzero help-status code must not turn otherwise passing protocol
evidence into a conformance failure. Missing version output remains reportable,
but version-reporting status is not an MQTT behavior assertion.

## 11. Native executable proof map

The closest native owners include:

- `crates/trust-runtime/src/io/mqtt/tests/tag_mapping.rs` for direction, raw
  fallback selection, empty mapping presence, mixed points, and mapped output;
- `crates/trust-runtime/src/io/mqtt/tests/point_map_contract.rs` and
  `io/mqtt/point_map.rs` tests for typed payload conversion and transaction
  integrity;
- `crates/trust-runtime/src/io/mqtt/tests.rs` for configuration, session,
  security, worker deadlines, freshness, reconnect, and Sparkplug behavior;
- runtime interface/control/DAP contract tests for enumeration TypeId and
  `valueType` preservation; and
- `scripts/mqtt_mosquitto_e2e.sh`, invoked by
  `scripts/runtime_comms_conformance_gate.sh`, for real-broker traffic-light
  interoperability.

These tests prove only the assertions they execute. Unit doubles do not replace
the real-broker case, and the real-broker case does not replace TLS, Sparkplug,
failure-policy, or malformed-payload tests.
