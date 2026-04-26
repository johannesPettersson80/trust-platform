# Conveyor Pulse Classic

This example drives a conveyor pulse output with the classic OSCAT
`GEN_PULSE` block.

The direct version is useful when a single pulse generator is local to one scan
program. The call site owns all timing and enable inputs.

Compare it with `examples/oscat_components_conveyor_pulse_components`.

## Pattern Shown

- Classic signal-generator FB call.
- Deterministic disabled-state test.
- Scan code reads and publishes `GEN_PULSE.Q` directly.

## Validate

```bash
trust-runtime test --project examples/oscat_components_conveyor_pulse_classic
```
