# Packaging Reject Pulse Classic

This example drives a reject solenoid with classic OSCAT `GEN_PULSE`.

The classic version keeps the defect input and pulse timing in the scan call.

Compare it with `examples/oscat_components_packaging_reject_pulse_components`.

## Pattern Shown

- Classic pulse generator for packaging reject gates.
- Disabled-state test for deterministic behavior.
- Timing parameters visible at the call site.

## Validate

```bash
trust-runtime test --project examples/oscat_components_packaging_reject_pulse_classic
```
