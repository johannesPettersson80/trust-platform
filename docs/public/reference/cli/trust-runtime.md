# `trust-runtime`

`trust-runtime` is the main operator/runtime CLI for truST. Developer and
automation workbench commands are moving to `trust-dev`; deprecated
`trust-runtime` aliases remain during the migration window and will not be
removed before 2026-10-05. Removing them requires a separate behavior-change
release note.

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
| `ads` | Beckhoff ADS onboarding backend, symbol import, route artifacts, and validation |
| `comm` | protocol schema, topology, identity discovery, testing, and symbol browsing used by Devices & Connections |
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

This forwards to `trust-dev test` and prints a deprecation warning with the
2026-10-05 removal-not-before date.

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

This forwards to `trust-dev docs` and prints a deprecation warning with the
2026-10-05 removal-not-before date.

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

This forwards to `trust-dev agent serve` and prints a deprecation warning with
the 2026-10-05 removal-not-before date.

### HMI

```text
Usage: trust-runtime hmi [OPTIONS] <COMMAND>
```

Current subcommands:

- `init`
- `update`
- `reset`

### Communication

```text
Usage: trust-runtime comm <COMMAND>
```

`comm discover` performs identity discovery and returns setup-form parameters.
For ADS, `--host` accepts a bare Host/IP. Add `--target-net-id <AMS_NET_ID>`
when UDP Identify is blocked and `--ams-port <1..65535>` to preserve a chosen
logical service in the returned candidate:

```text
trust-runtime comm discover --protocol ads --host 192.168.77.11 \
  --target-net-id 100.67.6.217.1.1 --ams-port 852 --json
```

Logical-service availability is checked separately with `comm browse-symbols`;
`comm discover` does not scan ADS ports.

### ADS

```text
Usage: trust-runtime ads <COMMAND>
```

Current subcommands:

- `discover`: find or identify TwinCAT ADS targets from the current host
  (`ads-wire` build required).
- `browse`: connect to one or all configured ADS client connections in
  `ads.toml` and print the live symbol table (`ads-wire` build required).
- `doctor`: run the ADS connectivity Doctor from the current host (`ads-wire`
  build required).
  Supplying `--target-net-id` makes a known target a first-class path: the
  Doctor uses that AMS Net ID instead of requiring UDP identify to succeed.
  Supplying `--write-symbol`, `--write-type`, and `--write-value` together runs
  a guarded write probe that writes, reads back, restores, and verifies a safe
  writable PLC variable.
- `route-script`: generate PowerShell, StaticRoutes.xml, GUI, or removal
  route artifacts using explicit runtime-host identity values.
- `add-route`: send UDP AddRoute directly to the PLC. The TwinCAT password is
  accepted only through `--password-stdin`.
- `import-symbols`: authoring-only live symbol import that can dry-run generated
  `ads.toml`, snapshot, and ST files for VS Code/web diff review (`ads-wire`
  build required).
- `import`: generate deterministic ST globals from existing `ads.toml` and
  cached ADS symbol snapshots.
- `validate --offline`: compare generated ST against `ads.toml` and cached
  symbol snapshots without opening a PLC connection.
- `validate --live`: upload live TwinCAT symbols for each configured
  connection and compare the checked-in generated ST against the live target
  (`ads-wire` build required).
- `server status`: query `[runtime.ads_server]` status from a running runtime
  control endpoint.
- `server symbols`: list the runtime globals currently exposed as ADS symbols.
- `server doctor`: run the ADS server Doctor against the running runtime. The
  loopback self-test alone is not external-client proof; pass
  `--external-kind` and `--external-name` only after an independent client such
  as pyads has completed its smoke.
- `server route-script`: generate route artifacts for external ADS clients or a
  TwinCAT engineering station so they can reach the truST ADS server.

The normal customer workflow starts in the VS Code ADS panel or the runtime
host setup page at `/setup/ads`; the CLI is the scriptable backend. Common
authoring and offline-validation workflow:

```bash
trust-runtime ads import-symbols \
  --target 192.168.10.5 \
  --target-net-id 5.23.91.12.1.1 \
  --connection line1 \
  --out ads.toml \
  --gen src/generated/ads_generated.st

trust-runtime ads validate --offline \
  --config ads.toml \
  --snapshot ads/snapshots/line1.symbols.json \
  --generated src/generated/ads_generated.st
```

Live validation uses the configured ADS routes:

```bash
trust-runtime ads browse \
  --config ads.toml \
  --connection line1

trust-runtime ads validate --live \
  --config ads.toml \
  --generated src/generated/ads_generated.st
```

Live runtime ADS I/O is not started by these CLI commands. It is enabled by
`[runtime.ads]` in `runtime.toml` and requires a runtime built with feature
`ads-wire`.

ADS server mode is enabled separately by `[runtime.ads_server]` and requires a
runtime built with feature `ads-server`. Common server workflow:

```bash
trust-runtime ads server route-script \
  --route-name trust-runtime-line1 \
  --server-ip 192.168.77.10 \
  --server-net-id 192.168.77.10.1.1 \
  --format powershell

trust-runtime ads server status --project examples/communication/ads_server_basic
trust-runtime ads server symbols --project examples/communication/ads_server_basic
trust-runtime ads server doctor --project examples/communication/ads_server_basic
```

After an independent pyads smoke, attach that evidence to the Doctor report:

```bash
trust-runtime ads server doctor \
  --project examples/communication/ads_server_basic \
  --external-kind pyads \
  --external-name pyads-smoke
```

The ADS server proof ladder is intentionally split: loopback self-test, pyads
independent-client proof, and real TwinCAT engineering-station validation are
separate gates.

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
during the product/workbench split. It will not be removed before 2026-10-05,
and removal requires a separate behavior-change release note.

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
- [ads.toml](../config/ads-toml.md)
