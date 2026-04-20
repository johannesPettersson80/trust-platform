# Driver Matrix

| Driver | Use it for | Best first doc |
| --- | --- | --- |
| `simulated` | deterministic fake process I/O without hardware | [Simulated And Loopback](simulated-and-loopback.md) |
| `loopback` | fast `%Q -> %I` local sanity checks | [Simulated And Loopback](simulated-and-loopback.md) |
| `gpio` | local edge/device pin mapping | [GPIO](gpio.md) |
| `ethercat` | deterministic fieldbus I/O | [EtherCAT](ethercat.md) |
| `modbus-tcp` | register-mapped external systems | [Modbus TCP](../external-systems/modbus-tcp.md) |
| `mqtt` | broker-backed event/message exchange | [MQTT](../external-systems/mqtt.md) |
| multi-driver | one runtime using more than one driver family | [Multi Driver](multi-driver.md) |

## Selection rule

1. Start with `loopback` or `simulated` when you are still proving logic.
2. Move to `gpio` or `ethercat` for local hardware.
3. Move to `modbus-tcp` or `mqtt` for external-system integration.
4. Use multi-driver only after each individual driver validates cleanly by
   itself.
