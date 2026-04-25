# Simulation

Virtual process and fault-injection loop before real I/O. Simulated signals use
the same project, runtime, and HMI path planned for target hardware.

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
