# `trust-runtime`

`trust-runtime` is the main operator/runtime CLI for truST. Developer and
automation workbench commands are moving to `trust-dev`; deprecated
`trust-runtime` aliases remain during the migration window and will not be
removed before 2026-10-05. Removing them requires a separate behavior-change
release note.

![Top-level `trust-runtime --help`](../../assets/images/terminal/runtime-help.gif)

*Figure:* The top-level `trust-runtime` command families as the binary reported
them before `new` was added; the table below lists `new`.

## Top-level Command Families

| Command | Purpose |
| --- | --- |
| `run` / `play` | start a runtime instance |
| `ui` | interactive terminal UI |
| `ctl` | send control requests to a running runtime |
| `new` | create a project folder with the files a new project starts from |
| `identify` | make a runtime show where it is by blinking its identify output |
| `check` | validate sources/config without writing a program image |
| `build` | compile the project into the program image `program.tpi` |
| `validate` | validate project config + program image |
| `hmi check` | check the project's HMI files against its program, for CI |
| `hmi test` | compare every HMI page's look with its approved screenshot, for CI |
| `hmi-password` | print the stored hash of an HMI user's password |
| `test` | deprecated alias for `trust-dev test` |
| `docs` | deprecated alias for `trust-dev docs` |
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
| `toolchain` | the compiler toolchain that builds programs: `status`, `install`, `bundle` |
| `firmware` | install truST's firmware on a chip through the chip's own boot loader: `install` |

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

### New

```text
Usage: trust-runtime new --parent <FOLDER> --name <NAME> [--force] [--json]
```

`new` creates the project folder `<FOLDER>/<NAME>` with the files that a new
truST project starts from: `src/Main.st`, `src/config.st`, `trust-lsp.toml`,
`runtime.toml`, `io.toml`, `.vscode/launch.json`, `.vscode/settings.json` and
`.gitignore`. The `runtime.toml` names no control endpoint, so the runtime uses
the platform default, and `.gitignore` lists `/.trust-runtime/`, the folder that
holds the endpoint file.

The name is trimmed and must be one plain folder name on every platform. It must
not be empty or `.` or `..`, and must not contain `/`, `\`, a drive prefix or
another character that Windows refuses (`: * ? " < > |` and control characters).
It must not end with a dot, and must not be a device name that Windows reserves
(`CON`, `PRN`, `AUX`, `NUL`, `COM1` to `COM9`, `LPT1` to `LPT9`, with or without an
extension). An existing folder that is not empty is refused unless `--force` is
given. With `--force`, the scaffold files are overwritten and every other file in
the folder is kept. An existing `.gitignore` is always kept; it only gains the
`/.trust-runtime/` line when no line ignores that folder yet.

With `--json`, success prints
`{"project":"<absolute folder>","focus":"<absolute folder>/src/Main.st"}`. A
refused or failed request prints `{"error":{"code":"<code>","message":"<text>"}}`
and exits 1. The codes are:

| Code | Meaning |
| --- | --- |
| `name` | the name is not one plain folder name on every platform (see above) |
| `exists` | the folder exists, is not empty, and `--force` is not given |
| `not-directory` | the folder, or its parent, exists and is not a folder |
| `io` | a folder or file could not be read or written |

Without `--json`, success prints `Created project <absolute folder>`. A failure
prints the message with the `Error:` prefix and exits 1.

### Check

```text
Usage: trust-runtime check [OPTIONS]
```

`check` validates required `runtime.toml` and `io.toml`, optional `ads.toml`
and `opcua_client.toml`, the source/dependency layout, and compilation without
writing a program image. It runs the build's checks and code generation for
this computer's CPU and needs no compiler toolchain. It aggregates independent
configuration and compile issues in one response instead of stopping at the
first failure.

`--json` and `--ci` emit response version 1 with `ok`, `status`, issue counts,
stable issue codes, and source/dependency lists. Configuration or source-layout
errors exit 10; compile errors exit 11. A successful check exits 0 and never
writes a program image.

When `runtime.toml` sets `[runtime.identify] output`, `check` also makes the
load check of `run` on the program the sources compile to: a program that uses
the identify output is the issue `config.identify` (exit 10), with the same text
`run` prints.

### Build

```text
Usage: trust-runtime build [OPTIONS]
```

Primary options:

- `--project`
- `--sources`
- `--ci`
- `--target`
- `--emit-source`

The default source root is `<project>/src`; relative `--sources` values are
resolved from the project. A successful build writes the program image
`program.tpi` and its deployment record `deployment-record.bin` into the
project, and its report to `build/build.json`. `--target` builds for a device
(`esp32c6`, `stm32u5a9`), a CPU (`x86_64`, `aarch64`, `riscv32imac`,
`thumbv8m`) or a Rust target triple instead, and writes
`build/program.<triple>.tpi` and `build/deployment-record.<triple>.bin`;
`--emit-source` keeps the generated Rust crate in the build folder. The first
build that needs the compiler toolchain installs it. A failed source,
dependency, or compile step preserves any existing artifact. `--ci` prints the
build report: response version 1 with the target, whether the image is a debug
image, the image and record paths, the image's size, the signing key, and the
memory figure; build failures exit 11.

### Validate

```text
Usage: trust-runtime validate [OPTIONS] --project <PROJECT>
```

Primary options:

- `--project`
- `--ci`

`validate` makes the same checks as `run` makes when it loads the project, so a
project it accepts is a project `run` accepts. That includes the identify load
check: with `[runtime.identify] output` set, a program that uses the output, an
`io.toml` that maps the GPIO line, or an output beyond the output image is
refused with the error `run` prints.

### HMI check

```text
Usage: trust-runtime hmi check [--project <dir>] [--json]
```

Runs the HMI Builder's checks over every HMI file of the project, for its own CI: the files
load, the assets the pages use are there, every binding resolves in the program with a
compatible type and direction, and every control is within the write policy of
`runtime.toml`. It compiles the program with its libraries first. Each problem prints as
`file:line: message`; any problem exits 1, a clean project exits 0 with
`HMI check passed: N files`. `--json` prints `{ "ok", "problems": [{ "file", "line",
"message" }] }`.

### HMI test

```text
Usage: trust-runtime hmi test [--project <dir>] [--update] [--chromium <path>]
```

Draws every HMI page in headless Chromium as the runtime display lays it out at the screen's
design size, with design samples in place of live values, and compares it with its approved
screenshot `hmi/screens/<page>.png`. A page whose look changed, or that has no approved
screenshot, fails the command (exit 1) and leaves its new look as
`hmi/screens/<page>.actual.png` to review; `--update` approves every page's new look. Chromium
is taken from `--chromium`, `CHROMIUM`, the usual names on the path, or Playwright's cache.

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

- `endpoint [--json]`
- `status`
- `health`
- `stats`
- `events [--limit N]`
- `faults [--limit N]`
- `io-read`
- `io-write`
- `io-force`
- `io-unforce`
- `restart`
- `install <container> [--restart cold|warm]` (any `--endpoint`, also `serial:///dev/ttyACM0`)
- `firmware <image> [--wait <seconds>]` (a chip's truST firmware, while its program is stopped)
- `device-get`, `device-set <device.toml>`, `device-reset` (a chip's own settings: name, network,
  control token, time server, trusted keys; they apply at the chip's next boot)
- `shutdown [--exit]`
- `config-get`
- `config-set`

`shutdown` stops the program and leaves the runtime running in the Stopped state, with its
control server, web UI and other services; `restart cold` or `restart warm` starts the
program again. `shutdown --exit` also ends the runtime's process. Both need the Admin role. A
chip answers `--exit` as a plain `shutdown`, because its firmware never ends. The process also
ends on SIGINT or SIGTERM, and `trust-runtime fleet runtime stop` sends the exit.

`firmware` updates a chip's truST firmware: the image goes into the firmware slot that does not
run, the chip restarts into it and runs it on trial until its boot is done, and the command waits
until the chip runs it. A firmware that does not finish its boot is rolled back by the chip's boot
loader, and the command says so. The first install on a chip that does not run truST yet goes
through the chip's own boot loader:

```text
trust-runtime firmware install --chip esp32c6 ./trust-firmware-esp32c6
trust-runtime ctl --endpoint serial:///dev/ttyACM0 shutdown
trust-runtime ctl --endpoint serial:///dev/ttyACM0 firmware ./trust-firmware-esp32c6/firmware.bin
```

A chip whose `device.toml` names a Wi-Fi network and sets a control token serves its control
connection on that network, by default on TCP port 9901, and `ctl` reaches it there with the token,
which the chip then also asks for on its USB port. `capabilities` reports the chip's address:

```text
# device.toml
[network.wifi]
ssid = "plant-net"
password = "the network's passphrase"

[control]
token = "a token of 16 to 128 characters"
```

```text
trust-runtime ctl --endpoint serial:///dev/ttyACM0 device-set device.toml
trust-runtime ctl --endpoint serial:///dev/ttyACM0 device-reset
trust-runtime ctl --endpoint tcp://192.168.1.50:9901 --token <token> status
```

`ctl --project <dir> endpoint` prints the endpoint that a client of the project
uses, and sends nothing to the runtime. With `--json` it prints
`{"endpoint","auth_token","source","endpoint_file_skipped"}`, where `source` is
`endpoint_file`, `configured` or `default`. Every failure prints
`{"error":{"code":"<code>","message":"<text>"}}` and exits 1. The codes are
`no_project`, `invalid_argument`, `invalid_config`, `endpoint_file_invalid`,
`endpoint_file_unreadable`, `not_running` and `default_unavailable`; see
[runtime.toml](../config/runtime-toml.md#default-control-endpoint-and-the-endpoint-file).

![`trust-runtime ctl --help`](../../assets/images/terminal/ctl-help.gif)

*Figure:* The `ctl` command family and its subcommands for the scriptable
control API. The capture predates `ctl endpoint`.

### Identify

```text
Usage: trust-runtime identify [OPTIONS] --duration-s <DURATION_S>
```

`identify` makes a runtime show where it is: for the time asked it blinks the
output that the runtime's `runtime.toml` names in `[runtime.identify] output`
(an I/O address such as `%QX0.7`, or a GPIO line such as `gpio:17`) at 2 Hz. It
works in every state of the resource: running, paused, stopped or faulted. It
needs the Operator role.

Primary options:

- `--duration-s` (required): how long the runtime blinks, a whole number of
  seconds from 1 through 60.
- `--endpoint`: the control endpoint of the runtime, `tcp://host:port` or
  `unix:///path`.
- `--token`: the control token. Without it, `TRUST_CTL_TOKEN` is used.
- `--project`: a project folder whose `runtime.toml` gives the endpoint and the
  token that `--endpoint` and `--token` do not.

The command prints the runtime's result as one JSON line,
`{"identifying":true,"duration_s":10}`. A refused request is a command failure
that prints the runtime's error text and its error code: for example
`identify_not_configured` for a runtime without `[runtime.identify] output`
(a chip answers it too until its firmware drives an output), or
`identify_output_unavailable` when the output cannot be made ready.

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
