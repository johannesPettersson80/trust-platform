# OPC UA

## The server is part of trust-runtime

The released `trust-runtime` serves OPC UA: its build feature `opcua-wire` is on
by default. Only a build from source with `--no-default-features` leaves the
server out, and then `[runtime.opcua] enabled = true` refuses to start.

## First things to decide

- which symbols should be exposed?
- what endpoint and namespace URI should clients see?
- what security policy and mode are acceptable for the network?
- is anonymous access ever acceptable for this deployment?

Success means you can decide whether OPC UA belongs in this deployment, which
symbols are exposed, and whether the runtime has been built with the required
wire feature before a client expects to connect.

Use MQTT or Modbus instead when the integration is clearly pub/sub telemetry or
register-oriented device exchange.

## Example and commissioning guide

--8<-- "examples/communication/opcua/README.md:3"

## Related

- [runtime.toml -> runtime.opcua](../../reference/config/runtime-toml.md)
- [Protocol Matrix](../protocol-matrix.md)
