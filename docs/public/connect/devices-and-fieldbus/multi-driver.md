# Multi Driver

## Core rule

Use exactly one of these forms in `io.toml`:

- single driver: `io.driver` + `io.params`
- composed drivers: `io.drivers = [...]`

Do not mix them in one file.

## When multi-driver makes sense

- one runtime needs both device-style and broker-style exchange
- you are bridging multiple protocol planes into one process image
- a gradual migration requires two transports during commissioning

Success means each driver has a separate purpose, no config mixes single-driver
and multi-driver forms, and one process-image address is not silently owned by
two paths.

Use the single-driver form first unless commissioning or migration really needs
two planes at once.

The examples show when the extra complexity is justified and how to keep the
config readable.

## Example

--8<-- "examples/communication/multi_driver/README.md:3"

## Full tutorial

--8<-- "examples/tutorials/17_io_backends_and_multi_driver/README.md:3"
