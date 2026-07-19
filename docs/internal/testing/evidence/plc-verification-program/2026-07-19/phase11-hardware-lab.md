# Phase 11 Hardware-Lab Program

- Source commit: `9d21b8b266378bf7d8c948bf4ca50cfa95430ed5`
- Branch: `plc-verification-program`
- Timestamp: `2026-07-19T09:00:00+02:00`
- Platform: `Linux-7.0.0-15-generic-x86_64-with-glibc2.43`
- JSON SHA-256: `b0c54524e76faf733612b16634936ca46c368d70f01837a812d2eeba99b53e8f`
- Input digest: `sha256:1db35a8f4cbcddfd7a5f84288626d1363549dc42e24d0d11d6851badd662d3e5`
- Cases: 6
- Protocols: 5
- Strict-harness cases: 5
- Manual-script cases: 1
- Skipped/unproven: 6
- Durable lab evidence records: 0

No hardware execution is claimed by this report.

## Public Claim Boundary

Status: `preview_unverified`. Hardware qualified: `false`.

No case has a reviewed passing named-topology lab artifact; public hardware documentation remains preview/unverified.

## Cases

| Case | Board row | Protocol | Binding | Proof status | Evidence |
|---|---|---|---|---|---:|
| `LAB_MODBUS_TCP_001` | `VERIF-P11-002` | `modbus_tcp` | `strict_harness` | `skipped_unproven` | 0 |
| `LAB_MQTT_BROKER_001` | `VERIF-P11-003` | `mqtt` | `strict_harness` | `skipped_unproven` | 0 |
| `LAB_ADS_TWINCAT_001` | `VERIF-P11-004` | `ads_twincat` | `strict_harness` | `skipped_unproven` | 0 |
| `LAB_ETHERCAT_DISCOVERY_001` | `VERIF-P11-005` | `ethercat` | `strict_harness` | `skipped_unproven` | 0 |
| `LAB_ETHERCAT_STORAGE_001` | `VERIF-P11-005` | `ethercat` | `strict_harness` | `skipped_unproven` | 0 |
| `LAB_GPIO_OUTPUT_001` | `VERIF-P11-006` | `gpio` | `manual_script` | `skipped_unproven` | 0 |

## Case Details

### LAB_MODBUS_TCP_001 - Modbus TCP target discovery and bounded safe-read probe

- Command: `scripts/runtime_device_in_loop_gate.sh`
- Required environment: `TRUST_DIT_MODBUS_HOST`
- Topology: A named reachable Modbus TCP target connected to the reviewed lab runner.
- Topology reference: `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-05-phase-07/device-in-loop-gates.md`
- Expected artifacts: `modbus-discovery.json`
- Public-claim impact: No Modbus hardware or interoperability claim is established until a strict named-topology run passes.
- Assertions:
  - The configured target responds to discovery.
  - An explicitly configured safe-read probe remains bounded and visible in the JSON artifact.

### LAB_MQTT_BROKER_001 - MQTT broker authentication, TLS, reconnect, and disconnect exercise

- Command: `scripts/runtime_device_in_loop_gate.sh`
- Required environment: `TRUST_DIT_MQTT_BROKER`
- Topology: A named reachable MQTT broker, with reviewed authentication and TLS settings when those claims are exercised.
- Topology reference: `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-05-phase-07/device-in-loop-gates.md`
- Expected artifacts: `mqtt-interop.json`
- Public-claim impact: No MQTT broker interoperability, authentication, TLS, or reconnect claim is established until a strict named-topology run passes.
- Assertions:
  - The broker connection and publish/subscribe exchange are visible.
  - Configured authentication, TLS, reconnect, and disconnect outcomes are recorded rather than inferred.

### LAB_ADS_TWINCAT_001 - ADS/TwinCAT doctor and optional guarded write probe

- Command: `scripts/runtime_device_in_loop_gate.sh`
- Required environment: `TRUST_DIT_ADS_TARGET`
- Topology: A named reachable TwinCAT/ADS target connected to the reviewed lab runner.
- Topology reference: `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-05-phase-07/device-in-loop-gates.md`
- Expected artifacts: `ads-doctor.json`
- Public-claim impact: No ADS/TwinCAT hardware-readiness claim is established until a strict named-topology run passes.
- Assertions:
  - The ADS doctor result is machine-readable and fail-visible.
  - Any configured write probe is complete, guarded, and recorded in the artifact.

### LAB_ETHERCAT_DISCOVERY_001 - EtherCAT adapter and module-topology discovery

- Command: `scripts/runtime_device_in_loop_gate.sh`
- Required environment: `TRUST_DIT_ETHERCAT_ADAPTER`
- Topology: A named EtherCAT adapter with a reviewed discoverable bus and module profile.
- Topology reference: `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-05-phase-07/device-in-loop-gates.md`
- Expected artifacts: `ethercat-discovery.json`
- Public-claim impact: No EtherCAT topology or field-hardware claim is established until a strict named-topology run passes.
- Assertions:
  - The adapter opens and the discovered module topology is recorded.
  - Input/output byte counts and modules are explicit in the artifact.

### LAB_ETHERCAT_STORAGE_001 - EtherCAT repeated PDU storage stress

- Command: `scripts/runtime_device_in_loop_gate.sh`
- Required environment: `TRUST_DIT_ETHERCAT_ADAPTER`, `TRUST_DIT_ETHERCAT_STORAGE_STRESS`
- Topology: The reviewed EtherCAT topology with explicit bounded storage-stress opt-in.
- Topology reference: `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-05-phase-07/device-in-loop-gates.md`
- Expected artifacts: `ethercat-storage-stress.json`
- Public-claim impact: No EtherCAT storage-stability claim is established until a strict named-topology stress run passes.
- Assertions:
  - Every requested discovery cycle is recorded.
  - Process RSS observations and any failing cycle remain visible in the artifact.

### LAB_GPIO_OUTPUT_001 - GPIO output transition and optional looped-back input

- Command: `scripts/gpio_hardware_test.sh examples/communication/gpio`
- Required environment: none
- Topology: A reviewed Linux GPIO host using BCM input line 17 and output line 27, with an optional physical loopback between them.
- Topology reference: `examples/communication/gpio/README.md`
- Expected artifacts: `gpio-hardware-<timestamp>.log`, `gpio-hardware-<timestamp>.log.runtime.log`
- Public-claim impact: No Raspberry Pi, GPIO board, numbering, permission, wiring, or safe-state claim is established until a named-topology run is reviewed.
- Assertions:
  - The configured output reads TRUE after the commanded transition and FALSE after reset.
  - When physical loopback checking is enabled, the input follows both transitions.

## Limitations

- The report defines reviewed hardware-lab cases and bindings; it does not execute hardware.
- Every case remains skipped/unproven until a strict named-topology result is recorded as durable lab evidence.
- The GPIO case binds an existing manual script and tracked example but is not part of the strict device-in-loop harness.
- Public hardware documentation remains preview/unverified and no physical target is qualified by this report.
