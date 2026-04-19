# Multi Driver

Use this when one project needs more than one driver at once.

## Core rule

Use exactly one of these forms in `io.toml`:

- single driver: `io.driver` + `io.params`
- composed drivers: `io.drivers = [...]`

Do not mix them in one file.

## When multi-driver makes sense

- one runtime needs both device-style and broker-style exchange
- you are bridging multiple protocol planes into one process image
- a gradual migration requires two transports during commissioning

## Example

--8<-- "examples/communication/multi_driver/README.md"

## Full tutorial

--8<-- "examples/tutorials/17_io_backends_and_multi_driver/README.md"
