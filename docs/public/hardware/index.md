# Hardware

| Need | Open |
| --- | --- |
| supported host/platform status | [Hardware Compatibility](../reference/hardware-compatibility.md) |
| an ESP32-C6 board from no truST to a running program, over USB and Wi-Fi | [ESP32-C6 Getting Started](esp32c6-getting-started.md) |
| install or update the truST firmware on a chip | [Install And Update truST On A Chip](chip-install-and-update.md) |
| a chip's own settings: name, Wi-Fi, control token, time server, trusted keys | [`device.toml`](device-toml.md) |
| the chips truST runs on: memory, pins, flash layout, what their firmware holds | [Supported Chips](supported-chips.md) |
| Raspberry Pi or Linux target install | [Install On Target](../operate/install-on-target.md) |
| Linux PREEMPT_RT runtime posture | [PREEMPT_RT](../operate/preempt-rt.md) |
| I/O, GPIO, EtherCAT, and driver setup while programming | [Program](../develop/index.md) |

Hardware-dependent behavior is only proven on the target host where it is
tested: board, kernel, permissions, driver, adapter, and workload all matter.
