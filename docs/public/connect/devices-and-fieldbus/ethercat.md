# EtherCAT

EtherCAT proof is incomplete until the real host, bus, and devices are
validated. Keep the safe-state condition explicit before energizing outputs.

Use [I/O Binding](io-binding.md) first if you are still mapping variables to
addresses rather than choosing a fieldbus backend.

## Backend Guide

--8<-- "docs/guides/ETHERCAT_BACKEND_V1.md:3"

## Related

- [Protocol Matrix](../protocol-matrix.md)
- [Driver Matrix](driver-matrix.md)
- [Connectivity examples](../../examples/connectivity.md)
