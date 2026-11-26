# Control Roles And Tokens

Every request to a runtime's control endpoint, from `trust-runtime ctl` or from a service, runs with
one of four roles: Viewer, Operator, Engineer or Admin. Each role may do what the roles before it
may, and the runtime refuses a request whose role is too low before the request changes anything.

The steps below give a monitor read-only access and an operator panel the right to restart the
program. Their outputs come from truST 0.44.1 on Linux.

## What you need

- `trust-runtime` and a project, such as the one `trust-runtime new` writes.
- A second terminal, and about 10 minutes.

## The roles

| Role | May do, besides what the roles above it may |
| --- | --- |
| Viewer | read: status, health, events and faults, configuration, variables and I/O, the debugger's state |
| Operator | pause, resume and restart the program, and make the runtime identify itself |
| Engineer | write, force and release variables and I/O, step and evaluate in the debugger, record cycle traces, call methods, apply communication settings |
| Admin | shut the runtime down, install programs and firmware, change a chip's device settings, manage pairing and service tokens, and change security settings |

Specification 11, §6.9.5 "Runtime control authorization", lists every request with its role.

## Step by step

1. Give the runtime a TCP endpoint and a control token in its project's `runtime.toml`, then start
   it with `trust-runtime run --project .`. An explicit `tcp://` endpoint needs a token, and it
   listens on a loopback address only:

    ```toml
    [runtime.control]
    endpoint = "tcp://127.0.0.1:9917"
    auth_token = "a long random token, kept as a password is kept"
    mode = "production"
    debug_enabled = false
    ```

2. In the second terminal, read its status with the project's token, which is the Admin's:

    ```text
    trust-runtime ctl --project . status
    ```

    It answers:

    ```text
    state=running fault=none rt_profile=disabled rt_active=false
    ```

3. Make a Viewer's token for the monitor, and an Operator's for the panel. Each goes into its file,
   never to the screen:

    ```text
    trust-runtime ctl --project . service-token monitor --out monitor.token
    trust-runtime ctl --project . service-token panel --role operator --out panel.token
    ```

    They print:

    ```text
    id=service-monitor role=viewer token_file=monitor.token
    id=service-panel role=operator token_file=panel.token
    ```

4. Act as the monitor. `TRUST_CTL_TOKEN` passes the token, which keeps it out of the computer's list
   of processes:

    ```text
    export TRUST_CTL_TOKEN=$(cat monitor.token)
    trust-runtime ctl --endpoint tcp://127.0.0.1:9917 status
    trust-runtime ctl --endpoint tcp://127.0.0.1:9917 restart warm
    ```

    The status comes back as in step 2, and the restart is refused:

    ```text
    Error: forbidden: requires role operator (insufficient_role)
    ```

5. Act as the panel: `export TRUST_CTL_TOKEN=$(cat panel.token)`, then run the same `restart warm`.
   It is accepted, and the runtime gives the change its audit id:

    ```text
    {"audit_id":"<id>","id":1,"ok":true,"result":{"status":"restart queued"}}
    ```

    `trust-runtime ctl --endpoint tcp://127.0.0.1:9917 shutdown` is still refused, with
    `Error: forbidden: requires role admin (insufficient_role)`.

6. Without a token, `unset TRUST_CTL_TOKEN`, every request is refused before its role is
   looked at: `Error: missing auth token (missing_auth_token)`.

## What happened

**Where a request's role comes from.**

- The project's control token, `auth_token` in `[runtime.control]`, is the Admin's;
  `trust-runtime ctl --project .` reads it from the project.
- A service token, which `ctl service-token` makes, carries the role it was made with: Viewer, unless
  `--role operator` or `--role engineer` asks for more. Making it again for the same name replaces
  it.
- A pairing token carries the role it was paired with, Operator by default.
- A local Unix socket without a token has the Admin's authority: the runtime creates it with mode
  0600, for its own user only. A network client without configured authentication has only the
  Viewer's.

**What a refusal means.** The runtime checks the role before it runs the request, so a refused
request changes nothing: no variable, no I/O, no configuration. `insufficient_role` is the stable
code of a role that is too low, with the role needed in its text; a missing or wrong token has codes
of its own, such as `missing_auth_token`. A client tells them apart by the code, never by the text.

**Choosing a role.** Give each client the lowest role that does its work: a display or a historian
a Viewer's token, an operator panel an Operator's, a service that writes setpoints an Engineer's,
and keep the Admin's token for installs and shutdowns. Every accepted change carries an `audit_id`
for the [Audit Log](audit-log.md), and [Secrets](secrets.md) says how to keep and rotate tokens.

## Reference

- Specification 11, §6.9.5 "Runtime control authorization" (every request's role) and §6.9.6
  (service tokens).
- [`trust-runtime ctl`](../reference/cli/trust-runtime.md#control): `service-token` and every
  request, each with its role.
- [`runtime.toml`](../reference/config/runtime-toml.md): `[runtime.control]`'s `endpoint` and
  `auth_token`.
- [Secrets](secrets.md) and [Audit Log](audit-log.md).
