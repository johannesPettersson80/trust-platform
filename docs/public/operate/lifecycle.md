# Runtime Lifecycle

Use this page when you need the whole runtime story in one place: start, stop,
restart, reload, fault recovery, and cold start behavior.

## Lifecycle Questions

| Question | Go to |
| --- | --- |
| How do I start and stop the runtime? | [Runtime UI And Control](runtime-ui-and-control.md) |
| How do I hot-reload after code changes? | [Compile, Validate, Reload](compile-validate-reload.md) |
| How do I restart after deploy? | [Deploy And Rollback](deploy-rollback.md) |
| What happens on faults and safe state? | [Safety And Commissioning](safety-and-commissioning.md) |

## Modes To Think About

- normal start
- warm restart
- cold restart
- compile/reload loop
- faulted runtime waiting for intervention

## Retain And Recovery

Warm restart generally preserves RETAIN state; cold restart resets more state.
Use the exact runtime/config reference pages before depending on that behavior in
production.

## Related

- [Compile, Validate, Reload](compile-validate-reload.md)
- [Deploy And Rollback](deploy-rollback.md)
- [Safety And Commissioning](safety-and-commissioning.md)
- [runtime.toml](../reference/config/runtime-toml.md)
