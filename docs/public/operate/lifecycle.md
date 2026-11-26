# Runtime Lifecycle

Runtime lifecycle decisions split across start/stop control, hot reload,
release deploy, retain recovery, and safety recovery.

## Lifecycle Paths

| Lifecycle concern | Start here |
| --- | --- |
| start, stop, and operator control | [Runtime UI And Control](runtime-ui-and-control.md) |
| hot reload after source/config changes | [Compile, Validate, Reload](compile-validate-reload.md) |
| deploy and restart after a release | [Deploy And Rollback](deploy-rollback.md) |
| retained values over restarts, stops and power loss | [Retain Across Restarts](retain-across-restarts.md) |
| faults and safe-state recovery | [Safety And Commissioning](safety-and-commissioning.md) |

## Restart Modes

- Normal start brings the runtime up from a stopped state.
- A warm restart keeps the variables declared `RETAIN` and gives every other variable its initial
  value. A warm start after a stop reads the retained values back from the retain file that
  `[runtime.retain] mode = "file"` keeps, as every `runtime.toml` that truST writes does; with
  `mode = "none"` every start is cold ([Retain Across Restarts](retain-across-restarts.md)).
- A cold restart gives every variable its initial value, the retained ones too, and a cold start
  reads no retain store.
- Compile/reload is the engineering loop for code or config changes.
- Fault recovery starts from the relevant safety or rollback workflow, not from the normal start path.

## Related

- [Compile, Validate, Reload](compile-validate-reload.md)
- [Deploy And Rollback](deploy-rollback.md)
- [Safety And Commissioning](safety-and-commissioning.md)
- [runtime.toml](../reference/config/runtime-toml.md)
