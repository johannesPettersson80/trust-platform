# Hardware Compatibility

## Runtime Host Guidance

| Host class | Status | Notes |
| --- | --- | --- |
| Linux x86_64 | best-supported runtime path | natural host for `trust-runtime`, CLI control, and production services |
| Raspberry Pi / edge Linux | common edge/runtime path | verify GPIO permissions, storage, and restart behavior on target |
| Linux x86_64 / ARM64 with `PREEMPT_RT` | tuned soft-real-time path | same runtime binary, but support claims are tied to the measured hardware + kernel + workload combination |
| macOS | tooling-friendly, runtime varies | good for editing/building; validate hardware-dependent paths locally |
| Windows | tooling-friendly, runtime varies | good for VS Code and CLI authoring; validate service/runtime expectations locally |

## Hardware-Dependent Surfaces

| Surface | Notes |
| --- | --- |
| GPIO | confirm group/permission model and real pin mapping on target host |
| EtherCAT | validate NIC/adapter and module chain on the real hardware path |
| Modbus/MQTT/OPC UA | host support is usually straightforward; integration quality depends more on the peer device/service |

## Validation Guidance

- Validate on the exact target hardware before production use.
- Run the relevant driver/backend page together with a project that exercises the real device path.
- Prove permissions, restart behavior, storage durability, and hardware timing on the deployed host.
- Treat fieldbus and GPIO support as hardware-specific until you have target-host evidence.

## Related

- [Install On Target](../operate/install-on-target.md)
- [PREEMPT_RT Deployment](../operate/preempt-rt.md)
- [GPIO](../connect/devices-and-fieldbus/gpio.md)
- [EtherCAT](../connect/devices-and-fieldbus/ethercat.md)
- [Networking And Remote Access](../connect/networking-and-remote-access.md)
