# External Systems

## Quick Guide

| Protocol | Best for | Go to |
| --- | --- | --- |
| Modbus TCP | register-oriented PLC/device integration | [Modbus TCP](modbus-tcp.md) |
| MQTT | brokered event and message exchange | [MQTT](mqtt.md) |
| OPC UA | exposing runtime variables to OPC UA clients | [OPC UA](opc-ua.md) |
| Beckhoff ADS | importing TwinCAT symbols into truST globals, or exposing truST globals to ADS clients | [Beckhoff ADS](ads.md) |

## Picking The Right Protocol

- Choose Modbus TCP when the other side expects coils/registers and you want a classic industrial register model.
- Choose MQTT when you need pub/sub messaging through a broker.
- Choose OPC UA when the other side is an OPC UA client and needs a richer address-space view of runtime data.
- Choose Beckhoff ADS when TwinCAT or ADS-native tooling is involved. In client
  mode, truST imports named TwinCAT symbols. In server mode, truST exposes
  selected runtime globals as ADS symbols. ADS setup runs from the selected
  runtime host; truST mDNS runtime discovery is not the same thing as ADS target
  discovery or ADS server routing.

## Related

- [Protocol Matrix](../protocol-matrix.md)
- [Networking And Remote Access](../networking-and-remote-access.md)
