# Greenhouse Temperature Components

This example separates conversion and switching policy into two component
objects: `UnitConverter` and `HysteresisSwitch`.

The scan code reads like the process requirement: convert the measured
temperature, then update the ventilation switch. The same switch can be held
behind `IHysteresisSwitch` when a higher-level greenhouse controller should not
care which concrete switch implementation is used.

Compare it with `examples/oscat_components_greenhouse_temperature_classic`.

## Pattern Shown

- Stateless service object for unit conversion.
- Stateful scan object for hysteresis.
- Narrow interface variables for substitutable process devices.

## Validate

```bash
trust-runtime test --project examples/oscat_components_greenhouse_temperature_components
```
