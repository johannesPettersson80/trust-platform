# OPC UA

## Important feature gate

The shipped runtime config supports `[runtime.opcua]`, but wire-level server
support depends on building `trust-runtime` with the `opcua-wire` feature.

## First things to decide

- which symbols should be exposed?
- what endpoint and namespace URI should clients see?
- what security policy and mode are acceptable for the network?
- is anonymous access ever acceptable for this deployment?

## Example and commissioning guide

--8<-- "examples/communication/opcua/README.md"

## Related

- [runtime.toml -> runtime.opcua](../../reference/config/runtime-toml.md)
- [Protocol Matrix](../protocol-matrix.md)
