# `trust-dev`

`trust-dev` is the developer/workbench CLI for truST. It owns automation and
repository-helper commands that are being split out of the product runtime
binary.

## Command Families

| Command | Purpose |
| --- | --- |
| `agent serve` | serve the external agent JSON-RPC contract over stdio |
| `commit` | summarize and commit project changes |

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
product/workbench split.

## Commit

```text
Usage: trust-dev commit [OPTIONS]
```

Primary options:

- `--project`
- `--message`
- `--dry-run`

`trust-runtime commit` is a deprecated forwarding alias during the
product/workbench split.

## Related

- [trust-runtime](trust-runtime.md)
- [Agent API overview](../agent-api/overview.md)
- [Agent API v1](../agent-api/v1.md)
