# Chiller Temperature PID Classic

This example combines classic OSCAT conversion and `CTRL_PID` for a chiller
return-temperature loop.

Compare it with `examples/oscat_components_chiller_temperature_pid_components`.

## Pattern Shown

- Classic conversion plus controller call.
- Manual PID output for deterministic tests.
- Temperature control process scenario.

## Validate

```bash
trust-runtime test --project examples/oscat_components_chiller_temperature_pid_classic
```
