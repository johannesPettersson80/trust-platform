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
| `check` | validate sources/config without writing `program.stbc` |
| `build` | generate `program.stbc` |
| `validate` | validate project config + bundle |
| `test` | deprecated alias for `trust-dev test` |
| `docs` | deprecated alias for `trust-dev docs` |
| `hmi` | scaffold/update/reset `hmi/` |
| `ads` | Beckhoff ADS onboarding backend, symbol import, route artifacts, and validation |
| `comm` | offline communication schema, configuration, discovery, topology, and symbol browsing |
| `plcopen` | PLCopen import/export/profile |
| `registry` | package registry workflows |
| `setup` | initialize system I/O config |
| `ide` | serve the browser IDE |
| `agent` | deprecated alias for `trust-dev agent serve` |
| `commit` | deprecated alias for `trust-dev commit` |
| `wizard` | create a new project folder |
| `deploy` / `rollback` | versioned deployment and rollback |
| `bench` | communication/runtime benchmark surfaces |
| `conformance` | deterministic conformance suite runner |

## Entry-point and Error Contract

The top-level parser accepts `--verbose` before or after a subcommand. When a
subcommand is not recognized, the CLI computes a bounded edit-distance
suggestion against every shipped top-level command, ignoring global flags when
identifying the mistyped command. Suggestions are emitted only when the best
command is at most two single-character edits away; distant inputs receive the
normal usage error without an invented recommendation.

Errors returned to the top-level entrypoint exit with status 1 outside `--ci`
mode after printing the stable `Error:` prefix and any applicable configuration
tip. Commands with a documented machine contract, including `check`, may exit
directly with their stable command-specific status. In `--ci` mode, returned
errors use the documented command-aware CI exit classes for configuration,
build, test, timeout, and internal failures.

## Common Commands

### Check

```text
Usage: trust-runtime check [OPTIONS]
```

`check` validates required `runtime.toml` and `io.toml`, optional `ads.toml`
and `opcua_client.toml`, the source/dependency layout, and compilation without
writing `program.stbc`. It aggregates independent configuration and compile
issues in one response instead of stopping at the first failure.

`--json` and `--ci` emit response version 1 with `ok`, `status`, issue counts,
stable issue codes, source/dependency lists, and the in-memory bytecode size
when compilation succeeded. Configuration or source-layout errors exit 10;
compile errors exit 11. A successful check exits 0 and never writes the
compiled bytecode artifact.

### Build

```text
Usage: trust-runtime build [OPTIONS]
```

Primary options:

- `--project`
- `--sources`
- `--ci`

The default source root is `<project>/src`; relative `--sources` values are
resolved from the project. A successful build writes `program.stbc`. A failed
source, dependency, or compile step preserves any existing artifact. `--ci`
emits response version 1 with the output path and complete source/dependency
identity; build failures exit 11.

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

### Communication

```text
Usage: trust-runtime comm <COMMAND>
```

`comm` operates through the same communication configuration and inspection
contracts used by the IDE without requiring a running PLC runtime. Successful
responses are JSON objects with a command-specific `schema_version`; `--json`
is accepted on every subcommand for explicit machine-readable invocation.

Subcommands:

- `schema` returns the available protocol configuration schema and can filter
  it with `--protocol`.
- `apply` validates `--params` as a JSON object and applies `add`, `edit`,
  `upsert`, `remove`, `disable`, or `validate` to project configuration files.
- `topology` projects configured hosts, runtimes, endpoints, and links from
  project files. Configured state is not reported as live connectivity.
- `discover` performs host-local, connect/read-only discovery. Candidate
  confidence distinguishes protocol proof from a likely endpoint or mere TCP
  reachability; write probes are not supported.
- `browse-symbols` reads a configured project, target, or cached ADS snapshot
  and returns the canonical tree/import response.
- `opcua-trust list|clear` inspects or clears the selected OPC UA client trust
  store.

Malformed `--params`, unreadable or invalid snapshot files, unknown protocols,
and configuration validation failures are returned through the normal
top-level error contract.

### Commit

```text
Usage: trust-runtime commit [OPTIONS]
```

This preserves `--project`, `--message`, and `--dry-run` while forwarding to
`trust-dev commit`. It prints a deprecation warning and will not be removed
before 2026-10-05; removal requires a separate behavior-change release note.

### HMI

```text
Usage: trust-runtime hmi [OPTIONS] <COMMAND>
```

Current subcommands:

- `init`
- `update`
- `reset`

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

Offline `ads import` and `ads validate --offline` read `ads.toml` and cached
symbol snapshots without opening a PLC connection. Snapshot arrays are
canonicalized before generation so output is deterministic. Import creates
missing parent directories, reports `changed: false` when the generated file
already matches byte-for-byte, and refuses to replace different content unless
`--force` is present. Offline validation requires at least one snapshot and
reports generated-source drift with the first differing line.

Live discovery, browse, Doctor, guarded AddRoute, symbol import, and live
validation fail closed when the binary lacks the `ads-wire` feature; they do
not synthesize discovery or hardware results and do not write generated files.
Human output summarizes the same report objects emitted by `--json`; machine
consumers should use `--json` rather than parse styled terminal text.

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

`--project` defaults to the current directory and may name either one runtime
project or a workspace whose direct child directories are runtime projects. A
root-level `runtime.toml` is the primary runtime when present; otherwise the
first direct-child `runtime.toml` in lexical path order is primary. A malformed
selected runtime configuration stops startup. A directory with no runtime
configuration still starts with the neutral `config-ui` identity so the IDE's
Open Folder flow remains available.

The standalone control state loads the primary project's sources with the same
recursive, case-insensitive `.st`/`.pou` discovery and deterministic ordering
used by project builds, including configured local package dependencies. It
serves the IDE in `standalone-ide` mode with local authentication and does not
start a PLC scan cycle, discovery advertisement, or mesh transport. Runtime
operations that require an executing resource remain unsupported.

`trust-runtime config-ui serve` forwards to the same implementation and prints
a deprecation warning directing callers to `ide serve`.

## Related

- [Build, Validate, Test](../../operate/build-validate-test.md)
- [trust-dev CLI](trust-dev.md)
- [Agent API v1](../agent-api/v1.md)
- [runtime.toml](../config/runtime-toml.md)
- [ads.toml](../config/ads-toml.md)
