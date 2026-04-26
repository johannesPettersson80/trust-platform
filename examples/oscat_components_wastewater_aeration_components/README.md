# Wastewater Aeration Components

This example composes `HysteresisSwitch` and `PulseGenerator` for a blower duty
cycle.

The OOP version names the two responsibilities: demand detection and pulse
generation. This is easier to test and easier to replace in a larger treatment
plant module.

Compare it with `examples/oscat_components_wastewater_aeration_classic`.

## Pattern Shown

- Demand object feeding a pulse object.
- Interface-backed composition.
- Parity test against the same classic `HYST` and `GEN_PULSE` calls.

## Validate

```bash
trust-runtime test --project examples/oscat_components_wastewater_aeration_components
```
