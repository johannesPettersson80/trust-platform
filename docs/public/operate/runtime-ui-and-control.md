# Runtime UI, Control, And Control Endpoint

Use the terminal UI, CLI, and web UI to inspect or control a running runtime.

## Control options

| Interface | Best for |
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

![`ctl status` against a running runtime](../assets/images/terminal/ctl-status.gif)

*Figure:* `ctl status` returning the live runtime state from a running local
bundle. This is the quickest scriptable “is it alive?” check.

![`ctl io-read` returning a live snapshot](../assets/images/terminal/ctl-io-read.gif)

*Figure:* `ctl io-read` returning the current `%M` and `%Q` snapshot. Use this
when you need a terminal proof of actual signal values.

![`ctl io-write` in debug mode](../assets/images/terminal/ctl-io-write.gif)

*Figure:* `ctl io-write` against a debug-mode runtime followed by `io-read`.
This is the write path you use in development, not the guarded production path.

Important command families:

- `status`, `health`, `stats`
- `io-read`, `io-write`, `io-force`, `io-unforce`
- `restart`, `shutdown`
- `config-get`, `config-set`

`--endpoint` overrides only the project's endpoint. Without it, `ctl --project`
finds the running runtime through the project's endpoint file, then its
`runtime.toml`, then the platform default (see
[runtime.toml](../reference/config/runtime-toml.md#default-control-endpoint-and-the-endpoint-file)).
An endpoint file that you cannot read is skipped, and a runtime that does not
answer at the endpoint from the file or at the default is reported as not
running. Authentication uses `--token`, then `TRUST_CTL_TOKEN`, then the token
that the runtime generated (in the endpoint file, or in your per-user token file
on Windows), then the `runtime.toml` token. A rejected control response makes
`ctl` exit non-zero, so shell scripts can treat a successful process exit as an
accepted runtime command.

The `ui` console looks the endpoint up again whenever it reconnects, so it
follows a runtime that restarted on another port.

## Capabilities

- manual control
- operator terminal workflows
- read and write checks against a live runtime
- control endpoint usage

## Related

- [Operator Guide](operator-guide.md)
- [Compile, Validate, Reload](compile-validate-reload.md)
- [trust-runtime CLI](../reference/cli/trust-runtime.md)
