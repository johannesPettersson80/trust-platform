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

- `crates/trust-runtime-services/src/io_drivers/mqtt/**`: the one implementation
  of the driver's logic that every runtime calls (specification 34 ARCH-11): the
  parameters, the MQTT 3.1.1 packets, typed points and payloads, the Sparkplug B
  profile, and the session with the broker as a state machine without a
  transport or a clock;
- `crates/trust-runtime/src/io/mqtt.rs` and `io/mqtt/{worker,tls}.rs`: the
  computer's transport, a worker thread over TCP with TLS, and its `IoDriver`;
- `crates/trust-platform-esp32/src/drivers/mqtt.rs`: the ESP32-C6's transport
  over its network's client links (specification 34 Appendix D.2);
- `crates/trust-runtime/src/io/mqtt_tag_mapping.rs`: startup-only adapter from
  compiled PLC symbols to typed process-image points;
- `crates/trust-runtime/src/runtime/installed_bindings.rs`: the shared binding
  transaction that installs a complete mapping set, without broker knowledge;
- `crates/trust-runtime/src/io/interface.rs`: process-image bindings, scan
  synchronization and snapshot metadata, without broker knowledge; and
- `crates/trust-runtime/src/connectors/adapters/mqtt.rs`: projection into the
  shared connector-status contract, without transport behavior.

The Rust 1.99 / edition 2024 modernization in
[`34-runtime-portability.md`](34-runtime-portability.md) §16 rewrites nested MQTT
guards as let chains while preserving their evaluation order, short-circuiting
and binding scope. It does not change mapping validation, payload conversion,
topic selection, session/reconnection behavior, security, driver health or the
process-image exchange contract defined here.

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

A redacted web or IDE configuration edit may retain a stored MQTT password only
when its authentication context is unchanged. The same MQTT parser used for
connection setup supplies the effective host/port, TLS material and trimmed
CA/client-certificate/key paths, ALPN, username, client ID and insecure-remote
policy for this comparison, including defaults. This applies to both `mqtt`
and `mqtt-tcp`. Topic, mapping and timing edits do not change that context.
Changing it requires explicit replacement credentials, and a rejected edit
writes nothing. Specification 11 §6.9.7 defines the shared JSON/text/IDE save
contract, literal-marker handling and credential-safe errors. This comparison
does not freeze external DNS or file contents at an unchanged TLS path.

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

This ordering applies equally to a project launched by `trust-runtime` and to
the local `trust-debug` simulator when it loads the project's `io.toml`.
Neither launch path may construct an MQTT driver from unresolved symbolic
mappings.

Existing compatible direct `%I` or `%Q` bindings are reused. Otherwise the
startup adapter allocates deterministic non-overlapping process-image storage
after the declared image. The complete mapping set and any image growth are
installed as one resource-only direct-I/O binding reconfiguration
([34-runtime-portability.md](34-runtime-portability.md) §4.3.1): PLC storage and
values are kept and initializers do not rerun. A reused binding keeps its
position and receives the mapping's codec, including enumeration labels. A
refused reconfiguration leaves the installed resource, bindings and images
unchanged. A runtime admitted from a 1.1 artifact exports the updated 3.0
container afterwards ([12-bytecode.md](12-bytecode.md) §4.4). The worker
receives only typed point descriptions and process-image snapshots; it cannot
access program storage or resolve symbols.

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

Backoff follows connection failures only: a failed connect, or a lost session,
which a transmission that fails ends. A handoff that cannot be encoded, a point
that does not convert or a packet above the packet limit, is a publish condition:
nothing of it is published and the session stays. Input freshness is an input-path condition: the worker checks for
new input without waiting, so an input read never holds the worker or delays
output publication. A read that finds no message newer than the previous read
reports stale input under the policy below, but a connected session stays
connected, starts no backoff and keeps publishing outputs.

The driver therefore tracks two paths. Input-path health covers stale or missing
input and decoding; output-path health reflects only the broker connection and
the publish outcomes, and it decides whether a safe-state handoff is confirmed
(specification 11). A connection condition (not yet connected, a lost session,
reconnect backoff) clears as soon as any operation finds the link up; a publish
condition (a failed typed-output preflight or a pending handoff) clears only
with a successful publish. The reported driver health is the more severe of the two,
with the output-path detail on a tie, and an incoming-queue overflow degrades it
further (specification 34 §11.1). A direction the configuration disables is
always healthy: a driver without outputs has nothing to deliver for a safe
state, and a driver without inputs reports no input condition.

The session is clean (MQTT 3.1.1 §3.1.2.4), with or without Sparkplug B, so a
broker may also accept an empty `client_id` and assign one (MQTT-3.1.3-6). The
client subscribes at quality of service 0, and with Sparkplug B to its NCMD topic
at 1; a PUBLISH that a broker delivers at quality of service 1 is acknowledged
with PUBACK and taken as any message, and one at 2 is a protocol error that ends
the session.

Under the default `fault` policy, disconnected or stale input, a bounded read
without a snapshot, missed refresh/handoff deadlines, completed connection or
publish failure, and typed-output preflight failure return an error and set
faulted health. Connection errors retain their context rather than becoming a
generic unavailable-snapshot message. A faulted stale read leaves the caller's
input image unchanged. `warn` and `ignore` retain their documented degraded
success behavior. A timed-out queued output may later publish, but that does not
retroactively make the timed-out scan call successful.

For its communication link (specification 42, §4 item 9 and §5 item 4) the driver
also describes its session in words, beside its health: the broker as
`<host>:<port>`, then `connected`; `connecting` while a session waits for its
first acknowledgement; `disconnected: <reason>` with the last session error or
connection failure; or `not connected` before the first attempt. Behind the worker
the link gives the description the worker recorded last. The driver names the
input bytes it fills, so that values located there turn stale while the link
exchanges no data: the bytes of each input point; with a raw input topic and no
input points, the whole input image from its start; with neither, none.

## 9. Sparkplug B edge node profile

The optional Sparkplug B profile makes the driver a Sparkplug edge node (Eclipse
Sparkplug 3.0.0, namespace `spBv1.0`, specification version `3.0.0`) whose
metrics are its typed `output_points`. `group_id` and `edge_node_id` are
required; `birth_death_seq`, 0 to 255 and 0 by default, is the `bdSeq` of the
first connection. The profile requires at least one output point and no input
points. Topics are `<namespace>/<group_id>/<type>/<edge_node_id>` for the types
`NBIRTH`, `NDATA`, `NDEATH` and `NCMD`. Metric names are trimmed, nonempty, and
contain no control characters.

The node follows these rules of Sparkplug 3.0, named by their conformance ids:

- The CONNECT is a clean session
  (`tck-id-principles-persistence-clean-session-311`), and its will is the
  NDEATH at quality of service 1, not retained, on the node's NDEATH topic
  (`tck-id-message-flow-edge-node-birth-publish-will-message-qos`, `-will-retained`,
  `-will-message-topic`).
- `bdSeq` is an `Int64` metric. Each CONNECT's will carries the next value, one
  more than the previous connection's, and 0 after 255; the session's NBIRTH and
  an orderly NDEATH carry the same value
  (`tck-id-message-flow-edge-node-birth-publish-will-message-payload-bdSeq`,
  `tck-id-payloads-nbirth-bdseq-repeat`, `tck-id-payloads-ndeath-bdseq`). A
  runtime that starts again begins again at `birth_death_seq`.
- After the CONNACK the node subscribes to its NCMD topic at quality of service
  1 and then publishes its NBIRTH at quality of service 0, not retained
  (`tck-id-message-flow-edge-node-ncmd-subscribe`, `-birth-publish-nbirth-qos`,
  `-birth-publish-nbirth-retained`, `-birth-publish-connect`). Birth precedes
  data for each session.
- The NBIRTH holds a timestamp, the `bdSeq` metric, the metric
  `Node Control/Rebirth`, a `Boolean` with false and without an alias, and each
  output point's metric with its name, its data type and the value of the newest
  output image, or `is_null` before the first image
  (`tck-id-payloads-nbirth-rebirth-req`, `tck-id-payloads-metric-datatype-req`);
  its sequence number is the next of the node's count, 0 to 255
  (`tck-id-payloads-nbirth-seq`).
- Each handoff publishes one NDATA with every output metric and its value, and a
  sequence number one more than the node's previous message, 0 after 255
  (`tck-id-payloads-ndata-seq-inc`). An image that does not convert publishes
  nothing and takes no number.
- The NDEATH holds the `bdSeq` metric alone and no sequence number
  (`tck-id-payloads-ndeath-seq`). An orderly stop publishes it, then sends
  DISCONNECT, after which the broker drops the will
  (`tck-id-operational-behavior-edge-node-intentional-disconnect-ndeath`;
  DISCONNECT is allowed, `-intentional-disconnect-packet`).
- An NCMD whose metric `Node Control/Rebirth` is true asks for a rebirth: the
  node publishes a new NBIRTH of the session, with its `bdSeq` and the values of
  the newest output image, before any later NDATA
  (`tck-id-operational-behavior-data-commands-rebirth-action-1` to `-3`). A
  chip whose outgoing bound has no room for the NBIRTH ends the session instead,
  and the next session begins with its birth. Other NCMD metrics are not taken:
  the metrics are the program's outputs, which the program owns.
- Timestamps are the milliseconds since the Unix epoch of the runtime's wall
  clock, in UTC; they are 0 until the clock is set, as on a chip before its time
  service has answered.

Device topics, DBIRTH, DDATA, DDEATH and DCMD, templates, aliases, properties,
datasets, primary host awareness (`STATE`), store-and-forward and writes to
metrics through NCMD are not claimed. Structural protobuf bytes or substring
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

In the one search, the MQTT source finds brokers through mDNS
`_mqtt._tcp.local.` and through the search's directed hosts. A `comm.discover`
request has no mDNS browse and is directed only: a missing host returns no
candidates and an explicit directed-host warning. A directed target accepts a
host, IPv4 address, or bracketed IPv6 address. With no explicit port, every
distinct resolved address is probed at 1883 and 8883 in deterministic
address/port order; an explicit nonzero port probes only that endpoint.
Userinfo, paths, queries, fragments, invalid ports, ambiguous unbracketed
IPv6, duplicate socket addresses, and unresolved targets reject before
probing. A port-8883 TLS handshake without an MQTT exchange is MQTTS
reachability, never identified; a failed TLS handshake is only
`port_reachable`.

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
  fallback selection, empty mapping presence, mixed points, mapped output, and
  installation into a runtime admitted from a 1.1 artifact;
- `crates/trust-runtime/src/bin/trust-runtime/run/tests/mqtt_tag_mapping.rs`
  for bindings installed before the worker starts and atomic refusal of a
  mapping set with an unknown tag;
- `crates/trust-runtime-services/src/io_drivers/mqtt/tests/*.rs` for the
  shared rules byte by byte: parameters, packets, typed points and payloads, the
  session and its health, a chip's bounds, and the Sparkplug B payloads and
  session;
- `crates/trust-runtime/src/io/mqtt/tests/point_map_contract.rs` for typed
  payload conversion and transaction integrity on the computer;
- `crates/trust-runtime/src/io/mqtt/tests.rs` and `io/mqtt/tests/*.rs` for the
  computer's wiring against a broker stand-in: configuration, session,
  security and TLS, worker deadlines, freshness, reconnect, and Sparkplug;
- runtime interface/control/DAP contract tests for enumeration TypeId and
  `valueType` preservation; and
- `scripts/mqtt_mosquitto_e2e.sh`, invoked by
  `scripts/runtime_comms_conformance_gate.sh`, for real-broker traffic-light
  interoperability.

These tests prove only the assertions they execute. Unit doubles do not replace
the real-broker case, and the real-broker case does not replace TLS, Sparkplug,
failure-policy, or malformed-payload tests.
