# I/O Binding

## Address Examples: %IX0.0, %QX0.0, %IW0, %QW0

Use `%IX0.0` when you need a concrete boolean input example, `%QX0.0` for a
boolean output example, `%IW0` for a word input example, and `%QW0` for a word
output example.

Symbolic variables bind to physical I/O channels through direct addresses,
`VAR_CONFIG`, or `io.toml`.

At this point the problem is signal identity, not transport selection.

Keep one binding path authoritative for each signal so diagnostics and operator
views describe the same process image.

## Guide

--8<-- "docs/guides/PLC_IO_BINDING_GUIDE.md:3"

## Related

- [Driver Matrix](driver-matrix.md)
- [Safety And Commissioning](../../operate/safety-and-commissioning.md)
- [Connectivity examples](../../examples/connectivity.md)
