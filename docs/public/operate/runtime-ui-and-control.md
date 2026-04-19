# Runtime UI, Control, And Control Endpoint

Use this page when you want to operate a running runtime from terminal or built
in UI surfaces.

This is also the main user-facing page for runtime control endpoint workflows.

## Primary surfaces

| Surface | Best for |
| --- | --- |
| `trust-runtime ui` | interactive operator and developer terminal UI |
| `trust-runtime ctl` | scriptable control actions against a running runtime |
| web UI | browser-based operator and runtime workflows |

## `trust-runtime ui`

Use the terminal UI when you want a live operator-style view without opening
the browser or VS Code.

```bash
trust-runtime ui --project ./my-plc
```

Useful options:

- `--refresh`
- `--no-input`
- `--beginner`

## `trust-runtime ctl`

Use `ctl` when the workflow must be scriptable.

```bash
trust-runtime ctl --project ./my-plc status
trust-runtime ctl --project ./my-plc io-read
trust-runtime ctl --project ./my-plc restart
```

Important command families:

- `status`, `health`, `stats`
- `io-read`, `io-write`, `io-force`, `io-unforce`
- `restart`, `shutdown`
- `config-get`, `config-set`

## Use this page for

- manual control
- operator terminal workflows
- read and write checks against a live runtime
- control endpoint usage

## Related

- [Operator Guide](operator-guide.md)
- [Compile, Validate, Reload](compile-validate-reload.md)
- [trust-runtime CLI](../reference/cli/trust-runtime.md)
