# Conveyor Pulse Components

This example wraps OSCAT `GEN_PULSE` in `PulseGenerator`.

The pulse timing is configured once. The scan only updates the enable state and
calls `Update()`, which makes the generator easy to pass into conveyor modules
through `IPulseGenerator`.

Compare it with `examples/oscat_components_conveyor_pulse_classic`.

## Pattern Shown

- Scan object with configuration methods.
- Read-only output property after `Update()`.
- Interface variable for substitutable timing behavior.

## Validate

```bash
trust-runtime test --project examples/oscat_components_conveyor_pulse_components
```
