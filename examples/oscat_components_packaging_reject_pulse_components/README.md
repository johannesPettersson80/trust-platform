# Packaging Reject Pulse Components

This example wraps the packaging reject pulse in `PulseGenerator`.

The OOP version configures the reject timing once and keeps the scan call to
`SetEnabled()` plus `Update()`.

Compare it with `examples/oscat_components_packaging_reject_pulse_classic`.

## Pattern Shown

- Reusable pulse component for a solenoid actuator.
- Configuration method separated from process input.
- Classic parity test for the disabled state.

## Validate

```bash
trust-runtime test --project examples/oscat_components_packaging_reject_pulse_components
```
