# GPIO

Use GPIO when IEC `%IX/%QX` addresses need to map directly to local host or
edge lines.

## What to verify first

- line numbers match the target host
- the runtime process has GPIO permissions
- safe-state behavior is explicit for every energized output

## Hardware notes

GPIO projects are host-specific. Always confirm:

- the board family and numbering scheme you are using
- the line ownership model on the OS
- whether your deployment expects direct access or a mediated service

## Example and walkthrough

--8<-- "examples/communication/gpio/README.md"

## Related

- [I/O binding](io-binding.md)
- [Driver Matrix](driver-matrix.md)
