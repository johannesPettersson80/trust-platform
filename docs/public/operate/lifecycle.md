# Runtime Lifecycle

Runtime lifecycle decisions split across start/stop control, hot reload,
release deploy, retain recovery, and safety recovery.

## Lifecycle Paths

| Lifecycle concern | Start here |
| --- | --- |
| start, stop, and operator control | [Runtime UI And Control](runtime-ui-and-control.md) |
| hot reload after source/config changes | [Compile, Validate, Reload](compile-validate-reload.md) |
| deploy and restart after a release | [Deploy And Rollback](deploy-rollback.md) |
| retain behavior, faults, and safe-state recovery | [Safety And Commissioning](safety-and-commissioning.md) |

## Restart Modes

- Normal start brings the runtime up from a stopped state.
- Warm restart is the retain-preserving recovery path when the runtime/platform supports it.
- Cold restart reinitializes more runtime state and should be treated as a stronger reset.
- Compile/reload is the engineering loop for code or config changes.
- Fault recovery starts from the relevant safety or rollback workflow, not from the normal start path.

## Related

- [Compile, Validate, Reload](compile-validate-reload.md)
- [Deploy And Rollback](deploy-rollback.md)
- [Safety And Commissioning](safety-and-commissioning.md)
- [runtime.toml](../reference/config/runtime-toml.md)
