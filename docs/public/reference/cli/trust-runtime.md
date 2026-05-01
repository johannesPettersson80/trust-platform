# `trust-runtime`

`trust-runtime` is the main operator/runtime CLI for truST. Developer and
automation workbench commands are moving to `trust-dev`; deprecated
`trust-runtime` aliases remain during the migration window.

![Top-level `trust-runtime --help`](../../assets/images/terminal/runtime-help.gif)

*Figure:* The top-level `trust-runtime` command families exactly as the shipped
binary reports them.

## Top-level Command Families

| Command | Purpose |
| --- | --- |
| `run` / `play` | start a runtime instance |
| `ui` | interactive terminal UI |
| `ctl` | send control requests to a running runtime |
| `build` | generate `program.stbc` |
| `validate` | validate project config + bundle |
| `test` | deprecated alias for `trust-dev test` |
| `docs` | deprecated alias for `trust-dev docs` |
| `hmi` | scaffold/update/reset `hmi/` |
| `plcopen` | PLCopen import/export/profile |
| `registry` | package registry workflows |
| `setup` | initialize system I/O config |
| `ide` | serve the browser IDE |
| `agent` | deprecated alias for `trust-dev agent serve` |
| `wizard` | create a new project folder |
| `deploy` / `rollback` | versioned deployment and rollback |
| `bench` | communication/runtime benchmark surfaces |
| `conformance` | deterministic conformance suite runner |

## Common Commands

### Build

```text
Usage: trust-runtime build [OPTIONS]
```

Primary options:

- `--project`
- `--sources`
- `--ci`

### Validate

```text
Usage: trust-runtime validate [OPTIONS] --project <PROJECT>
```

Primary options:

- `--project`
- `--ci`

### Test

```text
Usage: trust-runtime test [OPTIONS]
```

This forwards to `trust-dev test` and prints a deprecation warning.

Primary options:

- `--project`
- `--filter`
- `--list`
- `--timeout`
- `--output`
- `--ci`

### Docs

```text
Usage: trust-runtime docs [OPTIONS]
```

This forwards to `trust-dev docs` and prints a deprecation warning.

Primary options:

- `--project`
- `--out-dir`
- `--format`

### Agent

```text
Usage: trust-runtime agent [OPTIONS] <COMMAND>
```

Compatibility subcommand:

- `serve`

This forwards to `trust-dev agent serve` and prints a deprecation warning.

### HMI

```text
Usage: trust-runtime hmi [OPTIONS] <COMMAND>
```

Current subcommands:

- `init`
- `update`
- `reset`

### PLCopen

```text
Usage: trust-runtime plcopen [OPTIONS] <COMMAND>
```

Current subcommands:

- `profile`
- `export`
- `import`

### Control

```text
Usage: trust-runtime ctl [OPTIONS] <COMMAND>
```

Important control subcommands:

- `status`
- `health`
- `stats`
- `io-read`
- `io-write`
- `io-force`
- `io-unforce`
- `restart`
- `shutdown`
- `config-get`
- `config-set`

![`trust-runtime ctl --help`](../../assets/images/terminal/ctl-help.gif)

*Figure:* The `ctl` command family and its subcommands for the scriptable
control API.

## Common Flows

### Build / validate / test

```bash
trust-runtime build --project ./my-plc --sources src
trust-runtime validate --project ./my-plc
trust-dev test --project ./my-plc --output json
```

### Start runtime

```bash
trust-runtime play --project ./my-plc
```

### Run agent API

```bash
trust-dev agent serve --project ./my-plc
```

`trust-runtime agent serve --project ./my-plc` remains a compatibility alias
during the product/workbench split.

![`trust-dev agent serve --help`](../../assets/images/terminal/agent-serve-help.gif)

*Figure:* The `agent serve` entrypoint and its current stable flags. This is the
CLI contract agents and wrappers should target first.

### Serve browser IDE

```bash
trust-runtime ide serve --project ./my-plc --listen 127.0.0.1:18080
```

## Related

- [Build, Validate, Test](../../operate/build-validate-test.md)
- [trust-dev CLI](trust-dev.md)
- [Agent API v1](../agent-api/v1.md)
- [runtime.toml](../config/runtime-toml.md)
