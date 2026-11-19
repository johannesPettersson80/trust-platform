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
| `service install`, `service uninstall` | put a service of the project under the computer's service manager, or remove it |
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
on macOS. From then on it starts at every boot without a login, and again 2 s
after it ends. It runs the release binary of the service's package, which
cargo's metadata names, as the user who owns the project folder. It runs in that
folder, with `TRUST_PROJECT` naming it, so the service reads the project's
`trust.toml` and its token itself. Build the service first with
`cargo build --release`.

The command takes root. Run as a user, it checks the service, changes nothing
and prints the command to run. That command carries the binary's path, so
`sudo` needs neither cargo nor your `PATH`:

```text
sudo /home/ada/.cargo/bin/trust-dev service install recipe --project /home/ada/plant --binary /home/ada/plant/recipe-service/target/release/recipe-service
```

It refuses:

- a service without its `package` or `endpoint`, or without its token file;
- a project folder that belongs to root;
- a `trust-service-recipe` installed for another project folder.

Installed again for the same folder, the service is replaced and restarted, as
after a new build. `trust-dev service uninstall recipe` stops and removes it.
A Windows service is not installed yet. Specification 48 RU-64 gives the
details.

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
