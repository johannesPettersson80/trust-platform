# Energy Normalization Components

This example uses `UnitConverter` as a service object for telemetry
normalization.

Unlike the classic conversion FBs, the component surface exposes named methods
for the conversion actually needed at the call site. That keeps telemetry code
compact without losing parity with OSCAT.

Compare it with `examples/oscat_components_energy_normalization_classic`.

## Pattern Shown

- Service-object facade for pure conversion behavior.
- Interface variable `IUnitConverter`.
- Classic conversion FBs remain the parity oracle in the ST test.

## Validate

```bash
trust-runtime test --project examples/oscat_components_energy_normalization_components
```
