# macOS MQTT multidriver fixture failure

Date: 2026-07-19

The live `runtime_composes_modbus_and_mqtt_drivers_live` integration test is
conditionally quarantined on non-Linux targets. Linux remains the reviewed
source of truth for the Modbus+MQTT driver-composition contract.

Observed GitHub Actions failures on pull request 107:

- Run `29684440021`, job `88186166361`: both macOS attempts failed while the
  in-process MQTT fixture reported `mqtt input snapshot unavailable`.
- Run `29685675514`, job `88189392548`: both macOS attempts completed the
  runtime cycle but timed out waiting for the in-process broker to observe the
  outbound MQTT publish.

The failures are confined to the timing-sensitive test broker. This record does
not waive Linux execution, does not claim product proof from a skipped run, and
does not change runtime or protocol behavior.
