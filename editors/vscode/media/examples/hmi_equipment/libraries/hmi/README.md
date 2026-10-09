# truST HMI Library

The HMI library holds `Hmi.Display`, the block a program calls to serve its HMI screens
([specification 39](../../docs/specs/39-hmi-builder.md), §18 Phase 5). Call it once, in a program
running in the task whose interval and priority the screens should have:

```iecst
PROGRAM Screens
VAR
    Hmi : Hmi.Display;
END_VAR
    Hmi();
END_PROGRAM
```

After that task the runtime copies the values the open screens show, applies the operators'
commands and reads the alarms. Without a call the screens show no live values.

Add the library to a project in its `trust-lsp.toml`:

```toml
[dependencies]
Hmi = { path = "../../libraries/hmi", version = "0.1.0" }
```
