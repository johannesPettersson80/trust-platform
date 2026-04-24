# Simulation

Use this page when you need a virtual process or fault-injection loop before
touching real I/O. The workflow below explains how to connect simulated signals
to the same build/runtime path so tests and HMI checks stay comparable.

Success means one simulated signal can be driven through the same project,
runtime, and HMI path you plan to use on target hardware, and the remaining
gap to real I/O is explicit.

Use simulation first when the next question can be answered without energizing
real equipment.

Keep the simulated-to-real gap visible in the commissioning notes.

The workflow should make later hardware validation smaller, not optional.

## Workflow Guide

--8<-- "docs/guides/PLC_SIMULATION_WORKFLOW.md:3"

## Related

- [Simulated And Loopback](../connect/devices-and-fieldbus/simulated-and-loopback.md)
- [Build, Validate, Test](build-validate-test.md)
- [Tutorials](../examples/tutorials.md)
