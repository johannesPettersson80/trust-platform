# Protocol truth and unavailable-resource gap closeout

Date: 2026-07-16

## Scope

This record supports closure of these written-contract gaps:

- `SPEC_GAP_PUBLIC_WIRE_CLAIM_001`
- `SPEC_GAP_PROTOCOL_DISCOVERY_HANDSHAKE_001`
- `SPEC_GAP_ETHERCAT_UNAVAILABLE_RESOURCE_001`
- `SPEC_GAP_PROTOCOL_STATUS_MODEL_001`

The owning normative source is `SPEC_RUNTIME_ENGINE_001`, backed by the
connector status/discovery and EtherCAT unavailable-resource sections in
`docs/specs/11-runtime-engine.md`. The public evidence-status table is in
`docs/public/connect/protocol-matrix.md`.

## Written contract

The contract now fixes the connector state, health, discovery-confidence, and
point-quality vocabularies. It requires protocol-level evidence for
`confirmed`, treats an authentication-rejected MQTT CONNACK as `likely`, and
keeps TCP-only reachability at `port_reachable`. MQTT discovery uses a clean
session and sends DISCONNECT after every received CONNACK.

The EtherCAT contract keeps construction non-blocking when a non-mock adapter
is absent. The first wire operation faults visibly. A post-allocation failure
is terminal for that driver instance and subsequent operations fail without a
new allocation or retry until the driver is rebuilt.

## Executable cases

- `TEST_CONNECTOR_STATUS_TRUTH_TRACE_001` exercises ADS and OPC UA lifecycle
  projection plus connector-versus-point staleness.
- `TEST_PROTOCOL_DISCOVERY_CONFIDENCE_TRACE_001` exercises Modbus device-ID,
  safe-read, and TCP-only probes plus MQTT accepted, auth-rejected,
  generically rejected, and TCP-only outcomes.
- `TEST_ETHERCAT_UNAVAILABLE_RESOURCE_TRACE_001` exercises missing-adapter
  terminal behavior and deterministic mock operation.

All three runners execute the committed hand-authored case files through the
shared `verification-cases` artifact contract.

## Defect and proof

The discovery trace found a product defect at clean commit `60e0394d`: an MQTT
authentication rejection was reported as `confirmed`. `prove.py` recorded
`EVID_TEST_PROTOCOL_DISCOVERY_CONFIDENCE_TRACE_001_RED`, with only
`PROTO_DISCOVERY_TRUTH_001_MQTT_AUTH_REJECTED` failing. Commit `ae6d37f4`
changed the classification to `likely` while preserving `auth_required`, the
warning, clean-session CONNECT, and DISCONNECT. `prove.py` then recorded the
paired green evidence.

Connector status and EtherCAT resource behavior already matched the written
contract. Their `prove.py` baseline/compare pairs record behavior locks rather
than manufacturing red failures.

## Honest boundary

This closeout proves reviewed software behavior on loopback and deterministic
mock surfaces. It does not prove a production TwinCAT route, plant Modbus
device, deployed MQTT broker or credential policy, physical OPC UA device, or
physical EtherCAT topology. Optional device-in-loop gates remain separate and
a skipped hardware gate supplies no proof. No CI enforcement or release claim
is changed by this closeout.
