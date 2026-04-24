# GPIO

## What to verify first

- line numbers match the target host
- the runtime process has GPIO permissions
- safe-state behavior is explicit for every energized output

## Hardware notes

GPIO projects are host-specific. Always confirm:

- the board family and numbering scheme you are using
- the line ownership model on the OS
- whether your deployment expects direct access or a mediated service

Success means every energized line has a known board numbering scheme,
permission model, owner, and safe-state expectation before a runtime is started.

Use [Simulated And Loopback](simulated-and-loopback.md) first if the project can
be proven without touching host GPIO lines.

Treat the example as a wiring review aid, not as proof that your board exposes
the same line numbers.

## Example and walkthrough

--8<-- "examples/communication/gpio/README.md:3"

## Related

- [I/O binding](io-binding.md)
- [Driver Matrix](driver-matrix.md)
