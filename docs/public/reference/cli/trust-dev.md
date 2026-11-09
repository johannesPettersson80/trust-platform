# `trust-dev`

`trust-dev` is the developer/workbench CLI for truST. It owns automation and
repository-helper commands that are being split out of the product runtime
binary.

## Command Families

| Command | Purpose |
| --- | --- |
| `agent serve` | serve the external agent JSON-RPC contract over stdio |
| `commit` | summarize and commit project changes |
| `docs` | generate API documentation from tagged ST comments |
| `test` | discover and execute ST tests |

## Agent

```text
Usage: trust-dev agent <COMMAND>
```

Current stable subcommand:

- `serve`

Run:

```bash
trust-dev agent serve --project ./my-plc
```

`trust-runtime agent serve` is a deprecated forwarding alias during the
product/workbench split. It will not be removed before 2026-10-05, and removal
requires a separate behavior-change release note.

## Commit

```text
Usage: trust-dev commit [OPTIONS]
```

Primary options:

- `--project`
- `--message`
- `--dry-run`

`trust-runtime commit` is a deprecated forwarding alias during the
product/workbench split. It will not be removed before 2026-10-05, and removal
requires a separate behavior-change release note.

## Docs

```text
Usage: trust-dev docs [OPTIONS]
```

Primary options:

- `--project`
- `--out-dir`
- `--format`

`trust-runtime docs` is a deprecated forwarding alias during the
product/workbench split. It will not be removed before 2026-10-05, and removal
requires a separate behavior-change release note.

## Test

```text
Usage: trust-dev test [OPTIONS]
```

Primary options:

- `--project`
- `--filter`
- `--list`
- `--timeout`
- `--output`
- `--ci`
- `--device`, `--target`, `--token`

Execution mode compiles the project even when it discovers no test POU, so an
invalid project cannot pass as an empty test run. `--list` remains
discovery-only, and a filter that matches none of the discovered tests reports
an empty selection without executing a test case.

`--device <port> --target <target>` runs the tests on a device instead of this
computer: a serial port, or `tcp://<host>:<port>` of a bridge to one, and the
device's target as `trust-runtime build --target` names it (`esp32c6`). When the
device's settings set a control token, every request carries it: `--token`,
else `TRUST_CTL_TOKEN`, else the project's `runtime.control.auth_token`. The
token is never printed.

`trust-runtime test` is a deprecated forwarding alias during the
product/workbench split. It will not be removed before 2026-10-05, and removal
requires a separate behavior-change release note.

## Related

- [trust-runtime](trust-runtime.md)
- [Agent API overview](../agent-api/overview.md)
- [Agent API v1](../agent-api/v1.md)
