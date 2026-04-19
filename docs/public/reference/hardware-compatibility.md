# Hardware Compatibility

Use this page when you need the public compatibility summary for runtime hosts,
tooling hosts, and common field-I/O assumptions.

## Runtime Host Guidance

| Host class | Status | Notes |
| --- | --- | --- |
| Linux x86_64 | best-supported runtime path | natural host for `trust-runtime`, CLI control, and production services |
| Raspberry Pi / edge Linux | common edge/runtime path | verify GPIO permissions, storage, and restart behavior on target |
| macOS | tooling-friendly, runtime varies | good for editing/building; validate hardware-dependent paths locally |
| Windows | tooling-friendly, runtime varies | good for VS Code and CLI authoring; validate service/runtime expectations locally |

## Hardware-Dependent Surfaces

| Surface | Notes |
| --- | --- |
| GPIO | confirm group/permission model and real pin mapping on target host |
| EtherCAT | validate NIC/adapter and module chain on the real hardware path |
| Modbus/MQTT/OPC UA | host support is usually straightforward; integration quality depends more on the peer device/service |

## What This Page Does Not Promise

- this page is not a substitute for plant-side validation
- fieldbus and hardware timing always need target-host proof
- use the example project plus the backend page together before production use

## Related

- [Install On Target](../operate/install-on-target.md)
- [GPIO](../connect/devices-and-fieldbus/gpio.md)
- [EtherCAT](../connect/devices-and-fieldbus/ethercat.md)
- [Networking And Remote Access](../connect/networking-and-remote-access.md)
