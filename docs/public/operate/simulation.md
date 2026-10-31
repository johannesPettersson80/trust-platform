# Simulation

Virtual process and fault-injection loop before real I/O. Simulated signals use
the same project and runtime path planned for target hardware.

Use simulation first when the next question can be answered without energizing
real equipment.

Keep the simulated-to-real gap visible in the commissioning notes.

The workflow should make later hardware validation smaller, not optional.

## VS Code workflow

1. Run **truST: Open Devices & Connections**.
2. Open **Add**, then click or drag **Simulated plant** onto an eligible
   runtime brick.
3. Choose a familiar starting point, such as Tank, Temperature, Motor speed,
   or Mechanical position. Advanced models remain available without filling
   the default form.
4. Select the PLC's ordinary numeric `%Q` command and `%I` feedback signals.
5. Review the proposed model values and engineering units, then choose
   **Save**.
6. If the runtime still uses an older configuration, follow the existing
   restart workflow. `Saved · restart required` means the file changed; it does
   not mean the running plant changed.
7. Select the saved plant and choose **Open in Control Studio** to monitor,
   test, identify, tune, and validate the loop.

**Plant Properties** is deliberately configuration-only. Live SP, PV, MV,
quality, limits, and plots stay in [Control Studio](control-studio.md), where
they remain visible during experiments.

## Workflow Guide

--8<-- "docs/guides/PLC_SIMULATION_WORKFLOW.md:3"

## Related

- [Control Studio](control-studio.md)
- [Simulated And Loopback](../connect/devices-and-fieldbus/simulated-and-loopback.md)
- [Build, Validate, Test](build-validate-test.md)
- [Tutorials](../examples/tutorials.md)
