# Tutorial 16: Secure Remote Access (TCP Control, Token, Pairing, Firewall)

This tutorial configures a runtime for remote access without dropping basic
security controls.

## Why this tutorial exists

Remote access is where many PLC systems become unsafe. The goal is not only
"make it reachable" but "make it reachable with explicit authorization and
minimal exposed surface".

## What you will learn

- how to switch control endpoint from local socket to TCP
- why `auth_token` is mandatory for TCP control
- how pairing and firewall rules reduce exposure
- how to enable and verify TLS for remote web/control exposure
- how to validate remote access safely

## Prerequisites

- complete Tutorial 13 first
- two terminals
- optional second device on same LAN for remote verification
- `openssl` available for local certificate generation

## Step 1: Create an isolated tutorial copy

Why: do not modify your baseline project while testing network settings.

```bash
rm -rf /tmp/trust-remote-secure
cp -R /tmp/trust-tutorial-13 /tmp/trust-remote-secure
```

## Step 2: Configure runtime for remote control with auth

Why: Unix sockets are local-only. Remote access requires TCP, and TCP requires
an explicit token.

Edit `/tmp/trust-remote-secure/runtime.toml`:

```toml
[runtime.control]
endpoint = "tcp://0.0.0.0:17777"
auth_token = "replace-with-long-random-token"
mode = "production"
debug_enabled = false

[runtime.web]
enabled = true
listen = "0.0.0.0:18084"
auth = "local"
```

Recommended token generation:

```bash
openssl rand -hex 24
```

Expected result:
- control API is remotely reachable only with token
- Web UI is reachable on port `18084`

## Step 3: Keep network exposure minimal

Why: if every host can reach every port, token-only security is not enough.

Minimum ports to allow:
- TCP `18084` for Web UI
- TCP `17777` for control endpoint

Keep blocked unless needed:
- UDP `5353` discovery (if not used)
- mesh ports (if not used)

Use VPN or SSH tunnel for remote access when possible instead of opening WAN
ports directly.

## Step 4: Build and validate before launch

Why: ensure runtime config changes are valid before opening network services.

```bash
trust-runtime build --project /tmp/trust-remote-secure --sources /tmp/trust-remote-secure/src
trust-runtime validate --project /tmp/trust-remote-secure
```

## Step 5: Start runtime

Why: runtime startup confirms endpoint binding and shows any config/auth errors.

```bash
trust-runtime run --project /tmp/trust-remote-secure
```

Expected result:
- runtime starts without auth or bind errors

## Step 6: Verify local first, then remote

Why: staged verification avoids debugging network and runtime issues at the same
time.

1. Local check: open `http://127.0.0.1:18084`
2. Remote LAN check: open `http://<host-ip>:18084` from another device

If UI asks for access/pairing, complete the claim flow from `Network -> Access
PLC`.

## Step 7: Validate control authorization behavior

Why: secure configuration must reject unauthorized control attempts.

- attempt access without token/claim first
- confirm runtime requires auth
- then access using pairing or token workflow

CLI token handoff example:

```bash
trust-runtime ui --token <your-token>
```

Expected result:
- unauthorized access fails
- authorized access succeeds

## Step 8: Commission TLS for remote access

Why: token auth protects control authorization, but TLS protects transport
confidentiality/integrity for credentials and control traffic.

Create local test certificates:

```bash
mkdir -p /tmp/trust-remote-secure/security
openssl req -x509 -newkey rsa:2048 -sha256 -nodes \
  -keyout /tmp/trust-remote-secure/security/server-key.pem \
  -out /tmp/trust-remote-secure/security/server-cert.pem \
  -days 365 \
  -subj "/CN=trust-remote-secure"
```

Add TLS config in `/tmp/trust-remote-secure/runtime.toml`:

```toml
[runtime.web]
enabled = true
listen = "0.0.0.0:18084"
auth = "local"
tls = true

[runtime.tls]
mode = "self-managed"
cert_path = "security/server-cert.pem"
key_path = "security/server-key.pem"
require_remote = true
```

Then re-validate and restart:

```bash
trust-runtime validate --project /tmp/trust-remote-secure
trust-runtime run --project /tmp/trust-remote-secure
```

Verify HTTPS endpoint:

```bash
curl -k https://127.0.0.1:18084/
```

Expected result:
- runtime starts with TLS enabled
- HTTPS endpoint responds
- plaintext HTTP requests are no longer the commissioning target

## Step 9: Harden production posture

Why: remote debugging is a common accidental risk.

Confirm these production-safe defaults:

```toml
[runtime.control]
mode = "production"
debug_enabled = false
```

Keep `auth_token` rotated and never commit secrets to git.

## Common mistakes

- enabling TCP control without `auth_token`
- enabling remote web/control exposure without TLS
- setting `runtime.tls.require_remote = true` but leaving `runtime.web.tls = false`
- exposing ports to the public internet directly
- leaving debug enabled in production mode
- storing token in tracked files

## Completion checklist

- [ ] TCP control endpoint configured with token
- [ ] web endpoint reachable locally and remotely
- [ ] unauthorized access blocked
- [ ] authorized access works via pairing/token
- [ ] TLS configured and HTTPS verified
- [ ] debug disabled in production mode
