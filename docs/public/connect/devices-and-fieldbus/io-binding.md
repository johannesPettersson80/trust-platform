# I/O Binding

Use this page when you need to understand `%I`, `%Q`, address binding, and driver/channel mapping.
If you are searching for concrete IEC addresses such as `%IX0.0`, `%QX0.0`,
`%IW0`, or `%QW0`, start here before digging into vendor-specific examples.

## Address Examples: %IX0.0, %QX0.0, %IW0, %QW0

Use `%IX0.0` when you need a concrete boolean input example, `%QX0.0` for a
boolean output example, `%IW0` for a word input example, and `%QW0` for a word
output example. This page is the canonical public-doc entry point for those
address forms and for how symbolic variables bind to real I/O channels.

## Guide

--8<-- "docs/guides/PLC_IO_BINDING_GUIDE.md"

## Related

- [Driver Matrix](driver-matrix.md)
- [Safety And Commissioning](../../operate/safety-and-commissioning.md)
- [Connectivity examples](../../examples/connectivity.md)
