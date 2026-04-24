# I/O Binding

## Address Examples: %IX0.0, %QX0.0, %IW0, %QW0

Use `%IX0.0` when you need a concrete boolean input example, `%QX0.0` for a
boolean output example, `%IW0` for a word input example, and `%QW0` for a word
output example. This page covers those address forms and how symbolic variables
bind to physical I/O channels.

Success means you can trace one ST variable from declaration to direct address
to driver channel, then decide whether the binding belongs in source,
`VAR_CONFIG`, or `io.toml`.

Use this page before protocol pages when the problem is still "which signal is
this?" rather than "which transport should carry it?"

Keep one binding path authoritative for each signal so diagnostics and operator
views describe the same process image.

## Guide

--8<-- "docs/guides/PLC_IO_BINDING_GUIDE.md:3"

## Related

- [Driver Matrix](driver-matrix.md)
- [Safety And Commissioning](../../operate/safety-and-commissioning.md)
- [Connectivity examples](../../examples/connectivity.md)
