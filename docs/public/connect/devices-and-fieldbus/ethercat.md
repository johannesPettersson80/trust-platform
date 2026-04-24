# EtherCAT

Use this page when you are evaluating the EtherCAT backend boundary, not when
you only need generic I/O mapping. The guide below explains what the current
backend surface covers, what must be validated on target hardware, and how to
keep safe-state behavior explicit before energizing outputs.

Success means you can name the EtherCAT capability boundary, the target-host
checks still required, and the safe-state condition for every output before a
runtime is allowed near hardware.

Use [I/O Binding](io-binding.md) first if you are still mapping variables to
addresses rather than choosing a fieldbus backend.

The guide is deliberately target-facing because EtherCAT proof is incomplete
until the real host, bus, and devices are involved.

## Backend Guide

--8<-- "docs/guides/ETHERCAT_BACKEND_V1.md:3"

## Related

- [Protocol Matrix](../protocol-matrix.md)
- [Driver Matrix](driver-matrix.md)
- [Connectivity examples](../../examples/connectivity.md)
