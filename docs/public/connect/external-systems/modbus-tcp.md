# Modbus TCP

## Good fit vs bad fit

| Good fit | Bad fit |
| --- | --- |
| PLC-to-device register exchange | pub/sub event distribution |
| gateways, drives, legacy equipment | highly dynamic topic-style systems |
| explicit polling and deterministic offsets | loosely structured payloads |

## First things to decide

- what server endpoint will the runtime talk to?
- what `unit_id` is correct for the device or gateway?
- where do inputs and outputs start in register space?
- should communication faults halt, warn, or degrade gracefully?

Success means the device endpoint, `unit_id`, register map, byte order, and
fault behavior are written down before runtime validation starts.

## Example and commissioning guide

--8<-- "examples/communication/modbus_tcp/README.md:3"

## Common Modbus gotchas

- wrong `unit_id` behind a gateway
- off-by-one mental model around register blocks
- byte/word order mismatches on non-trivial payloads
- accepting a “validate passed” result as proof of runtime connectivity

## Related

- [I/O binding](../devices-and-fieldbus/io-binding.md)
- [Protocol Matrix](../protocol-matrix.md)
