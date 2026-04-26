# Wastewater Aeration Classic

This example gates an aeration pulse with a classic hysteresis demand signal.

The classic version wires `HYST.Q` directly into `GEN_PULSE.ENQ` in the scan
body.

Compare it with `examples/oscat_components_wastewater_aeration_components`.

## Pattern Shown

- Classic alarm/demand block feeding a classic pulse generator.
- Real wastewater blower duty-cycle pattern.
- Test locks the demand-gated pulse behavior.

## Validate

```bash
trust-runtime test --project examples/oscat_components_wastewater_aeration_classic
```
