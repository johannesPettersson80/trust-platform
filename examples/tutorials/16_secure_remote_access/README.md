# Tutorial 16: Secure Remote Access (Token, TLS, Pairing, Firewall)

This tutorial opens a runtime to other computers without dropping basic security
controls.

## Why this tutorial exists

Remote access is where many PLC systems become unsafe. The goal is not only
"make it reachable" but "make it reachable with explicit authorization and
minimal exposed surface".

In truST, remote tools and browsers reach a runtime through its web interface.
The control endpoint that `trust-runtime ctl` uses stays on the PLC itself: a
local socket, or a TCP port on a loopback address. The runtime refuses a control
endpoint on any other address.

## What you will learn

- why the control endpoint stays local and the web interface carries remote access
- how a token protects the web interface
- how to enable and verify TLS
- how pairing gives another device its own token and role
- how firewall rules reduce exposure

## Prerequisites

- complete Tutorial 13 first
- `openssl` and `curl`
- optionally, a second device on the same network for the remote check

## Step 1: Create an isolated tutorial copy

Why: do not modify your baseline project while testing network settings.

```bash
rm -rf /tmp/trust-remote-secure
cp -R /tmp/trust-tutorial-13 /tmp/trust-remote-secure
cd /tmp/trust-remote-secure
```

## Step 2: Generate the primary token

Why: the primary token grants the Admin role, on the web interface and on the
control endpoint alike. It must be long and random, and it never goes into git.

```bash
openssl rand -hex 24
```

## Step 3: Create a certificate for TLS

Why: the token protects authorization, and TLS protects the token and the
traffic on the network. This self-signed certificate is for testing; a
production PLC uses one from your site's certificate authority.

```bash
mkdir -p security
openssl req -x509 -newkey rsa:2048 -sha256 -nodes \
  -keyout security/server-key.pem \
  -out security/server-cert.pem \
  -days 365 \
  -subj "/CN=trust-remote-secure"
```

## Step 4: Configure the runtime

Why: each setting closes one door.

Edit `runtime.toml`. Replace its `[runtime.control]` and `[runtime.web]`
sections with these, and add `[runtime.tls]`. Put the token from step 2 in
place of the placeholder:

```toml
[runtime.control]
mode = "production"
debug_enabled = false
auth_token = "paste-the-token-from-step-2-here"

[runtime.web]
enabled = true
listen = "0.0.0.0:18084"
auth = "token"
tls = true

[runtime.tls]
mode = "self-managed"
cert_path = "security/server-cert.pem"
key_path = "security/server-key.pem"
require_remote = true
```

What each setting does:

- `[runtime.control]` names no `endpoint`, so the control endpoint stays at the
  platform's local default. `auth_token` is the primary token; `trust-runtime
  ctl` and `trust-runtime ui` read it from this file.
- `listen = "0.0.0.0:18084"` opens the web interface on every network
  interface; name one address instead to open only that network.
- `auth = "token"`: every request needs a token in its `X-Trust-Token` header,
  the primary token or a paired one. It requires `runtime.control.auth_token`.
- `tls = true` with `[runtime.tls]`: the web interface speaks only HTTPS.

## Step 5: Keep network exposure minimal

Why: if every host can reach every port, a token alone is not enough.

Open only TCP `18084` for the web interface, and only to the networks that need
it. The control endpoint needs no open port: it is local. Keep UDP `5353`
(discovery) and the mesh port closed unless you use them.

Prefer a VPN or an SSH tunnel to opening WAN ports directly.

## Step 6: Build and validate before launch

Why: catch configuration errors before the runtime opens network services.

```bash
trust-runtime build --project . --sources src
trust-runtime validate --project .
```

Expected result:
- `Project ok`

## Step 7: Start the runtime

```bash
trust-runtime run --project .
```

Expected result:
- the runtime starts without bind or TLS errors

## Step 8: Verify that the token is required

Why: secure configuration must refuse unauthorized requests, not only accept
authorized ones.

In another terminal, in the project folder, keep the token in a shell variable
and ask the runtime for its status without it, then with it:

```bash
TOKEN=paste-the-token-from-step-2-here
curl -k -s -X POST https://127.0.0.1:18084/api/control -H 'Content-Type: application/json' \
  -d '{"id":1,"type":"status"}'
curl -k -s -X POST https://127.0.0.1:18084/api/control -H 'Content-Type: application/json' \
  -H "X-Trust-Token: $TOKEN" -d '{"id":2,"type":"status"}'
trust-runtime ctl --project . status
```

Expected result:
- without the token: `{"error":"unauthorized","ok":false}` (HTTP 401)
- with the token: `"ok":true` and the runtime's status, with the role `admin`
- `ctl` reads the token from `runtime.toml` and prints the state locally
- `http://127.0.0.1:18084/` answers nothing: the interface speaks only HTTPS

`-k` accepts the self-signed certificate; a client that trusts your site's
certificate authority does not need it.

## Step 9: Pair another device

Why: each device gets its own token and role, which you can list and take away,
instead of everyone sharing the primary token.

On the PLC, start a pairing with the primary token. The answer is a six-digit
code, valid for five minutes:

```bash
curl -k -s -X POST https://127.0.0.1:18084/api/pair/start -H "X-Trust-Token: $TOKEN"
```

On the other device, claim it with the code (no token needed; replace
`<plc-address>` and the code):

```bash
curl -k -s -X POST https://<plc-address>:18084/api/pair/claim -H 'Content-Type: application/json' \
  -d '{"code":"123456"}'
```

The answer is the device's own token. It has the Operator role: it reads the
status, and an Admin request such as `pair/start` answers `{"error":"forbidden"}`
(HTTP 403). On the PLC, list the pairings:

```bash
curl -k -s https://127.0.0.1:18084/api/pairings -H "X-Trust-Token: $TOKEN"
```

## Step 10: Harden for production

Why: remote debugging is a common accidental risk.

- keep `mode = "production"` and `debug_enabled = false`
- rotate the primary token, and never commit `runtime.toml` with a token to git
- use a certificate from your site's certificate authority, which covers the
  name or address that clients use

## Common mistakes

- a control `endpoint` on a network address: the runtime refuses it; remote
  access goes through the web interface
- `auth = "token"` without `runtime.control.auth_token`: validation refuses it
- `auth = "local"` on a network address: every remote request is refused, as only
  the PLC itself is trusted in `local` mode
- opening the web port to the internet instead of a VPN or a tunnel
- sharing the primary token with every device instead of pairing them

## Completion checklist

- [ ] the control endpoint is local and `ctl` works with the token from `runtime.toml`
- [ ] the web interface speaks only HTTPS
- [ ] a request without a token is refused, one with it is accepted
- [ ] another device is paired with its own token and role
- [ ] firewall rules allow only the web port, only to the networks that need it
