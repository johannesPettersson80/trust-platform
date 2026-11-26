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
| `simulate` | run a project's program and its simulation for a machine test, on a clock its test moves |
| `ui` | interactive terminal UI |
| `ctl` | send control requests to a running runtime |
| `new` | create a project folder with the files a new project starts from |
| `identify` | make a runtime show where it is by blinking its identify output |
| `discover` | search for devices: truST runtimes, ADS, Modbus TCP, MQTT, OPC UA, EtherCAT and GPIO |
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
| `plant` | the plant configuration of a stopped project, for trusted local tools |
| `plcopen` | PLCopen import/export/profile |
| `registry` | package registry workflows |
| `fleet` | offline fleet manifests and sibling runtimes |
| `setup` | initialize system I/O config |
| `ide` | serve the browser IDE |
| `config-ui` | deprecated alias for `ide` |
| `agent` | deprecated alias for `trust-dev agent serve` |
| `commit` | deprecated alias for `trust-dev commit` |
| `wizard` | create a new project folder |
| `deploy` / `rollback` | versioned deployment and rollback |
| `bench` | communication/runtime benchmark surfaces |
| `conformance` | deterministic conformance suite runner |
| `completions` | generate shell completions |
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
Usage: trust-runtime new --parent <FOLDER> --name <NAME> [--rust | --rust-first] [--force] [--json]
```

`new` creates the project folder `<FOLDER>/<NAME>` with the files that a new
truST project starts from: `src/Main.st`, `src/config.st`, `trust-lsp.toml`,
`runtime.toml`, `io.toml`, `.vscode/launch.json`, `.vscode/settings.json` and
`.gitignore`. The `runtime.toml` names no control endpoint, so the runtime uses
the platform default, and `.gitignore` lists `/.trust-runtime/`, the folder that
holds the endpoint file, and `/build/`, the folder the project's builds write.

Two flags start the project with Rust POUs instead (specification 48 RU-51). The
command writes each start's generated Structured Text too, so the project builds
as it is created, with the toolchain alone:

- `--rust` writes those files and a Rust crate in `rust/`: `rust/Cargo.toml` and
  `rust/src/lib.rs` with the function block `FILTER_EMA`, an exponential moving
  average, which `src/Main.st` calls, and its declaration in
  `src/generated/rust_pous.st`.
- `--rust-first` writes a Rust-first project, whose crate is the project:
  `Cargo.toml`, `src/lib.rs` with the program `MAIN`, `trust.toml` with the
  resource, a 10 ms task and the program instance, the generated
  `src/generated/trust/rust_pous.st` and `configuration.st`, and the same
  `runtime.toml`, `io.toml`, `trust-lsp.toml` and `.vscode/` files, with the
  simulator's launch on the generated configuration. It writes no `src/Main.st`
  or `src/config.st`.

Both name the crate's package after the project, pin `trust-plc` to this
release's version, and write `rust-toolchain.toml`: the Rust version of the
toolchain bundle, the devices' targets and `rust-src`, so that your own rustup
gives rust-analyzer and `cargo test` the same compiler. Their `.gitignore` also
lists the folder where your own cargo builds the crate.

The name is trimmed and must be one plain folder name on every platform. It must
not be empty or `.` or `..`, and must not contain `/`, `\`, a drive prefix or
another character that Windows refuses (`: * ? " < > |` and control characters).
It must not end with a dot, and must not be a device name that Windows reserves
(`CON`, `PRN`, `AUX`, `NUL`, `COM1` to `COM9`, `LPT1` to `LPT9`, with or without an
extension). An existing folder that is not empty is refused unless `--force` is
given. With `--force`, the scaffold files are overwritten and every other file in
the folder is kept. An existing `.gitignore` is always kept; it only gains each
of the start's lines that it lacks: the `/.trust-runtime/` line when no line
ignores that folder yet.

With `--json`, success prints
`{"project":"<absolute folder>","focus":"<absolute folder>/src/Main.st"}`, where
`focus` is the crate's source for a Rust start (`rust/src/lib.rs` or
`src/lib.rs`). A
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
this computer's CPU and needs no compiler toolchain, unless the project has a
Rust crate with Rust POUs: then it reads the crate as the build does and checks
it with the toolchain's cargo (`cargo check`), and the crate's errors and
warnings are issues where they are, with `truST (rustc)` as their `source`, as
`build` reports them. A warning
leaves the check passed; without the toolchain such a project fails the check
with the code `toolchain`. It aggregates independent configuration and compile
issues in one response instead of stopping at the first failure.

`--json` and `--ci` emit response version 1 with `ok`, `status`, issue counts,
stable issue codes, and source/dependency lists. Each compiler diagnostic is
one issue with its code (`E101`), `severity`, `message`, `file`, and where it
is: `line`, `column`, `end_line`, `end_column`, 1-based, columns counted as the
editor and `trust-lsp check` count them (UTF-16 code units). Each construct that
the build does not compile yet is one issue with the code `compile`, at the
`file`, `line` and `column` of its first token when the build knows them; the
check goes on after one, so it reports every such construct of the statements
and bodies it reaches, in the order of their places. Configuration or
source-layout errors exit 10; compile errors exit 11. A successful check exits 0
and never writes a program image.

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
- `--json`
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
dependency, or compile step preserves any existing artifact. `--ci` and `--json` print the
build report: response version 1 with the target, whether the image is a debug
image, the image and record paths, the image's size, the signing key, and the
memory figure. Sources with errors, or constructs the build does not compile
yet, print, instead of the error text, a failed response whose `issues` are the
compiler's diagnostics or those constructs, one issue each, as `check` reports
them.
Build failures exit 11 with `--ci`, and 1 otherwise.

Every build also writes `build/install.<triple>.bin`, the container that
`trust-runtime ctl install` sends. A computer's module is built for the Linux
target of its CPU on every operating system, so the names read
`x86_64-unknown-linux-gnu` or `aarch64-unknown-linux-gnu` on Windows and macOS
too: the module is freestanding and calls nothing of the operating system
(specification 44 §4.4).

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

### Simulate

```text
Usage: trust-runtime simulate --project <PROJECT>
```

`simulate` runs the project's program and its simulation (`simulation.toml`) for a machine test,
which a test process starts (specification 48 RU-58). Its time moves only when
the test asks with `simulator.advance`, which runs a number of cycles or the cycles due within a
duration, so every run of a test gives the same result.

It answers the control protocol on its standard input and output, one JSON object per line each
way, as `trust-runtime ctl` sends it, and writes its log to standard error. It opens no endpoint and
takes no token: the process that started it is its only client. It starts no web server,
discovery, mesh or I/O driver, keeps retained values in memory, and ends when its input closes
(specification 11 §6.8.1, "Simulator for machine tests").

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

Every released `trust-runtime` has the ADS client and the ADS server: their build
features, `ads-wire` and `ads-server`, are on by default. A `trust-runtime`
built from source without them refuses the commands that need them, as below.

Current subcommands:

- `discover`: find or identify TwinCAT ADS targets from the current host.
- `browse`: connect to one or all configured ADS client connections in
  `ads.toml` and print the live symbol table.
- `doctor`: run the ADS connectivity Doctor from the current host.
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
  `ads.toml`, snapshot, and ST files for VS Code/web diff review.
- `import`: generate deterministic ST globals from existing `ads.toml` and
  cached ADS symbol snapshots.
- `validate --offline`: compare generated ST against `ads.toml` and cached
  symbol snapshots without opening a PLC connection.
- `validate --live`: upload live TwinCAT symbols for each configured
  connection and compare the checked-in generated ST against the live target.
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
`[runtime.ads]` in `runtime.toml`.

Offline `ads import` and `ads validate --offline` read `ads.toml` and cached
symbol snapshots without opening a PLC connection. Snapshot arrays are
canonicalized before generation so output is deterministic. Import creates
missing parent directories, reports `changed: false` when the generated file
already matches byte-for-byte, and refuses to replace different content unless
`--force` is present. Offline validation requires at least one snapshot and
reports generated-source drift with the first differing line.

Live discovery, browse, Doctor, guarded AddRoute, symbol import, and live
validation fail closed in a binary built without the `ads-wire` feature; they do
not synthesize discovery or hardware results and do not write generated files.
Human output summarizes the same report objects emitted by `--json`; machine
consumers should use `--json` rather than parse styled terminal text.

ADS server mode is enabled separately by `[runtime.ads_server]`. Common server
workflow:

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
- `read <path>...` (variables by their paths, one completed scan's values, each with its type;
  a Viewer may read, also in production)
- `io-read`
- `io-write`
- `io-force`
- `io-unforce`
- `restart`
- `install <container> [--restart cold|warm]` (any `--endpoint`, also `serial:///dev/ttyACM0`)
- `firmware <image> [--wait <seconds>]` (a chip's truST firmware, while its program is stopped)
- `record (--cycles <n> | --duration <t>) [--task <task>] [--out <file>]` (the resource's next
  cycles, saved as a cycle trace)
- `device-get`, `device-set <device.toml>`, `device-reset` (a chip's own settings: name, network,
  control token, time server, trusted keys; they apply at the chip's next boot)
- `shutdown [--exit]`
- `config-get`
- `config-set <key> <value>` (a value that begins with `[` or `{` is sent as the JSON
  it is; a key that takes effect only after a restart is named, with `runtime.toml`, which
  `config-set` does not write)

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

`ctl write <path>=<value>...` writes variables as a service does (`variables.write`):
each value as JSON (`21.5`, `true`, `"pump"`), sent with the service's token from
`--token`, which must be an Engineer service token whose service's `writes` names each
path. The next cycle's start writes the set together; the command prints the reply.

`ctl record` records the resource's next cycles into a cycle trace and saves it as a
`.trusttrace` file (specification 48 RU-59). It records `--cycles <n>` cycles, with `--task
<task>` that task's `n` runs, or the cycles of `--duration <t>` of logical time, a duration such as
`500ms`, `2s` or `T#1m30s`. The runtime reserves the recording's memory first, and refuses a
recording it cannot hold, with the bytes it needs and the bytes it has. A chip records as a
computer does, within the free heap it keeps beside its program ([Supported
Chips](../../hardware/supported-chips.md)). The command writes the file to `--out`, by default
`trace-<UTC date and time>.trusttrace` in the current folder, and prints its cycles, why the
recording ended and its size. Ctrl+C ends the recording early and saves what it holds. It needs the
Engineer role.

```text
trust-runtime ctl --project ./my-plc record --cycles 500 --out pump.trusttrace
recorded 500 cycles (complete) to pump.trusttrace, 412032 bytes
```

`ctl fault-trace` saves the flight recorder's trace of the last fault as a `.trusttrace` file
(`[runtime.trace] flight_recorder` in `runtime.toml`, specification 48 RU-59), read and checked as
`ctl record` reads a recording. It writes the file to `--out`, by default
`fault-<UTC date and time>.trusttrace` in the current folder, and prints its cycles, why it ended
and its size. A runtime that keeps no fault's trace, because no fault has happened since it
started or because it keeps no flight recorder, ends the command with 1 and the reason. It needs
the Engineer role. A runtime with a folder of its files has also written the trace there, as
`traces/fault-<UTC date and time>.trusttrace`.

```text
trust-runtime ctl --project ./my-plc fault-trace --out pump-fault.trusttrace
saved the fault's trace of 1200 cycles (faulted) to pump-fault.trusttrace, 986432 bytes
```

`ctl faults` prints the newest faults, newest first, as `faults` lists them. A fault that the program
raised in a debug build also names where: its POU, its source file and the statement's line, as in
`{"type": "fault", "error": "division by zero", "pou": "Main", "file": "src/Main.st", "line": 17,
"time_ns": 5000000000}`. A fault from outside the program, a release build's fault, and the fault of a
program that a restart or an online change has replaced name no place.

`ctl service-token <name> --out <file>` creates the token a service signs in with
(`pair.service`), or replaces the one it has, and needs the Admin role. It writes
the token into `<file>`, with mode 0600 on Unix, making the file's folder when it is
missing, and prints `id=service-<name> role=<role> token_file=<file>`, never the
token. The role is `viewer` unless `--role operator` or `--role engineer` asks
for another. The token does not expire; running the command again replaces it.

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

### Toolchain

`toolchain` manages the compiler toolchain that builds programs (specification
44 §4.6); [The Compiler Toolchain](../../start/compiler-toolchain.md) walks
through it with its outputs.

- `status` names the toolchain that the tools use, where it is and its version,
  or that none is installed. With `--json` it prints one object: `installed`,
  `folder`, `source` (`TRUST_TOOLCHAIN` or `installed`) and `trust`, the
  version.
- `install` installs this release's toolchain bundle: the one beside the tools,
  as the VS Code extension carries it, else downloaded from the release and
  checked against its `SHA256SUMS`. `--from <file>` installs a bundle file
  instead, without the download. Every file is checked against the bundle's
  manifest before the toolchain is moved into place. With `--json` it writes one
  object per line: `download` (with `bytes` and `total`), `verify`, and
  `installed` (with `folder`) or `failed` (with `error`).
- `bundle <folder> <archive>` writes the bundle of this computer from a
  development toolchain folder (`scripts/native/build_toolchain_libraries.sh`).

```text
trust-runtime toolchain status --json
trust-runtime toolchain install --from trust-toolchain-linux-arm64.tar.gz
trust-runtime toolchain bundle ./dev-toolchain trust-toolchain-linux-x64.tar.gz
```

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
