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
| `replay` | replay a cycle trace in a simulator of the project as it is now |
| `rust declarations`, `rust audit`, `rust bench` | write the Structured Text declarations of the project's Rust POUs, check the crates its program is built from against the RustSec advisory database, and measure its Rust POUs for their time budgets |
| `service bindings`, `service install`, `service uninstall` | write a service's bindings to the program's variables, put the service under the computer's service manager, or remove it |
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

## Replay

```text
Usage: trust-dev replay [OPTIONS] <TRACE>
```

Replays a cycle trace, a `.trusttrace` file that `trust-runtime ctl record`
saved, in a simulator of the project as it is now. It builds the project with
the installed trust-runtime, runs the trace's cycles with the recorded tasks,
inputs, changes from outside and clock readings, and compares each cycle's
outputs with the trace's (specification 48 RU-60).

```bash
trust-dev replay traces/commissioning.trusttrace --project examples/room_heating
```

- When every cycle matches, it prints the cycles it replayed and ends with `0`.
- At the first cycle whose outputs differ, it prints the cycle and its logical
  time, each output that differs with the trace's value and the build's, and
  how many later cycles differ, and ends with `1`.
- A trace or a build it cannot replay ends it with `2` and the reason.

A changed program takes the trace's values as an online change carries them,
and the report's notes list what it could not carry over.

Primary options:

- `--project`

The runtime is found as trust-sim finds it: `TRUST_RUNTIME`, else the `PATH`,
else the newest truST VS Code extension.

## Service

```text
Usage: trust-dev service install [OPTIONS] <NAME>
       trust-dev service uninstall <NAME>
```

Primary options of `install`:

- `--project`
- `--binary`

`trust-dev service install recipe` puts the service `[services.recipe]` of the
project's `trust.toml` under the computer's service manager as
`trust-service-recipe` and starts it: a systemd unit on Linux, a launchd daemon
on macOS, a Windows service. From then on it starts at every boot without a
login, and again 2 s after it ends. It runs the release binary of the service's
package, which cargo's metadata names, as the user who owns the project folder;
on Windows as its virtual account, `NT SERVICE\trust-service-recipe`. It runs in
that folder, with `TRUST_PROJECT` naming it, so the service reads the project's
`trust.toml` and its token itself. Build the service first with
`cargo build --release`.

The command takes root, or an administrator on Windows. Run without those
rights, it checks the service, changes nothing and prints the command to run:
with `sudo` on Linux and macOS, in PowerShell opened as administrator on
Windows. That command carries the binary's path, so it needs neither cargo nor
your `PATH`:

```text
sudo /home/ada/.cargo/bin/trust-dev service install recipe --project /home/ada/plant --binary /home/ada/plant/recipe-service/target/release/recipe-service
```

It refuses:

- a service without its `package`, or without its token file;
- a project folder that belongs to root;
- a `trust-service-recipe` installed for another project folder.

Installed again for the same folder, the service is replaced and restarted, as
after a new build. `trust-dev service uninstall recipe` stops and removes it.

On Windows the service's program is a copy of trust-dev in
`%ProgramData%\truST\services\recipe\`, so that updating truST keeps the
service. The copy runs the binary and appends its output to
`logs\recipe.log` there. Specification 48 RU-64 gives the details.

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
