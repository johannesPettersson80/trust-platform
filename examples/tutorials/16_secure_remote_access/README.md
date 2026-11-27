# Tutorial 16: Secure Remote Access

Open a runtime's web interface to the network safely: a long random token on every request, TLS for
the traffic, the control endpoint kept on the PLC itself, and a token of its own for each other
device, by pairing. It starts from tutorial 13's project.

## What you learn

- `[runtime.web] auth = "token"` makes every request carry a token, the primary token from
  `runtime.toml` or a paired one, and `tls = true` with `[runtime.tls]` makes the interface speak
  only HTTPS.
- The control endpoint stays local: a `tcp://` endpoint must be a loopback address, and
  `trust-runtime ctl` reads the primary token from `runtime.toml`.
- Pairing gives another device its own token with the Operator role, which the PLC lists and can
  revoke.

## What you need

- Tutorial 13's project in `/tmp/trust-tutorial-13`, as its README builds it, and
  `trust-runtime`, from the truST extension for VS Code or a release archive.
- `openssl` and `curl`, Linux or macOS, whose shell runs the commands below, and two terminals.
- About 20 minutes.

## Step by step

1. Copy tutorial 13's project, so that the network settings stay out of it:

    ```bash
    rm -rf /tmp/trust-remote-secure
    cp -R /tmp/trust-tutorial-13 /tmp/trust-remote-secure
    cd /tmp/trust-remote-secure
    ```

2. Make the primary token:

    ```bash
    openssl rand -hex 24
    ```

    It prints 48 hexadecimal digits. The primary token grants the Admin role, on the web interface
    and on the control endpoint alike, so it never goes into git.

3. Make a certificate for TLS:

    ```bash
    mkdir -p security
    openssl req -x509 -newkey rsa:2048 -sha256 -nodes \
      -keyout security/server-key.pem \
      -out security/server-cert.pem \
      -days 365 \
      -subj "/CN=trust-remote-secure"
    ```

    It writes `security/server-key.pem` and `security/server-cert.pem`. A self-signed certificate
    is for testing; a production PLC uses one from your site's certificate authority.

4. Edit `runtime.toml`: replace its `[runtime.control]` and `[runtime.web]` sections with these, add
   `[runtime.tls]`, and put the token of step 2 in place of the placeholder:

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

5. Build and check the project:

    ```bash
    trust-runtime build --project . --sources src
    trust-runtime validate --project .
    ```

    The validation answers `Project ok`.

6. Start the runtime:

    ```bash
    trust-runtime run --project .
    ```

7. In the second terminal, in the project folder, keep the token in a shell variable and ask the
   runtime for its status without it, then with it:

    ```bash
    TOKEN=paste-the-token-from-step-2-here
    curl -k -s -X POST https://127.0.0.1:18084/api/control -H 'Content-Type: application/json' \
      -d '{"id":1,"type":"status"}'
    curl -k -s -X POST https://127.0.0.1:18084/api/control -H 'Content-Type: application/json' \
      -H "X-Trust-Token: $TOKEN" -d '{"id":2,"type":"status"}'
    trust-runtime ctl --project . status
    ```

    Without the token the answer is `{"error":"unauthorized","ok":false}`, with HTTP status 401.
    With it the answer is the runtime's status, which begins
    `{"id":2,"ok":true,"result":{"access":{"io":{"force":true,"release":true,"write":true},"reason":null,"role":"admin"}`:
    the primary token has the Admin role. `ctl` reads the token from `runtime.toml` and answers
    `state=running fault=none rt_profile=disabled rt_active=false`. `-k` accepts the self-signed
    certificate; a client that trusts your site's certificate authority does not need it. Plain HTTP
    gets nothing: `curl -s http://127.0.0.1:18084/` prints nothing and ends with exit status 1.

8. Pair another device. On the PLC, start a pairing with the primary token:

    ```bash
    curl -k -s -X POST https://127.0.0.1:18084/api/pair/start -H "X-Trust-Token: $TOKEN"
    ```

    The answer is a six-digit code and the second at which it expires, five minutes later, such as
    `{"code":"123456","expires_at":1795778190}`. On the other device, claim it with the code; the
    claim needs no token. Replace `<plc-address>` and the code:

    ```bash
    curl -k -s -X POST https://<plc-address>:18084/api/pair/claim -H 'Content-Type: application/json' \
      -d '{"code":"123456"}'
    ```

    The answer, `{"token":"..."}`, holds the device's own token, 43 characters. It has the Operator
    role: a status request with it answers `"role":"operator"`, with the reason `Operator role —
    connect with an engineer token to write or force.`, and an Admin request such as `pair/start`
    answers `{"error":"forbidden","ok":false}`, with HTTP status 403. On the PLC, list the pairings:

    ```bash
    curl -k -s https://127.0.0.1:18084/api/pairings -H "X-Trust-Token: $TOKEN"
    ```

    Each pairing shows its `id`, such as `pair-1795777890-tgLJLhEq`, its `role`, whether it is
    `enabled`, when it expires, and only the last four characters of its token, as `"tail":"…lMLw"`.

9. Stop the runtime with `trust-runtime ctl --project . shutdown --exit`.

## What happened

**The token.** With `auth = "token"` every request to the web interface carries a token in its
`X-Trust-Token` header, the primary token or a paired one, and its role decides what it may do; the
setting requires `runtime.control.auth_token`. `ctl` reaches the control endpoint on the PLC itself
and reads the same token from `runtime.toml`.

**TLS.** `tls = true` with the certificate and key of `[runtime.tls]` makes the interface speak only
HTTPS, so the token and the traffic stay encrypted on the network. `require_remote = true` makes the
configuration refuse a web interface or a mesh that listens on a network address without TLS.

**The control endpoint.** `[runtime.control]` names no `endpoint`, so it stays at the platform's
local default, a socket in the project folder; a `tcp://` endpoint is admitted only on a loopback
address. Remote access goes through the web interface.

**Pairing.** A pairing code is six digits, valid for five minutes, and claimed without a token. The
claim returns a new random token with the Operator role, valid for 30 days, which the PLC lists by its
ID and last four characters and can revoke. Each device then has a token of its own, instead of
everyone sharing the primary token.

**The network.** Open only TCP port 18084, for the web interface, and only to the networks that need
it. The control endpoint needs no open port. Keep UDP port 5353 (discovery) and the mesh port closed
unless you use them, and prefer a VPN or an SSH tunnel to opening a port to the internet. `listen =
"0.0.0.0:18084"` opens every network interface; name one address to open only that network.

**Production.** Keep `mode = "production"` and `debug_enabled = false`, rotate the primary token,
never commit a `runtime.toml` that holds a token, and use a certificate from your site's certificate
authority that covers the name or address the clients use.

## Try it yourself

- Revoke the paired device. Take its `id` from the list and send `pair.revoke` with the primary
  token: `curl -k -s -X POST https://127.0.0.1:18084/api/control -H 'Content-Type: application/json'
  -H "X-Trust-Token: $TOKEN" -d '{"id":4,"type":"pair.revoke","params":{"id":"<pairing-id>"}}'`. The answer
  ends `"status":"revoked"`; the device's token now gets `{"error":"unauthorized","ok":false}`
  (HTTP status 401), and the list shows the pairing with `"enabled":false`.
- Remove `auth_token` from `[runtime.control]` and check the project again:
  `trust-runtime validate --project .` refuses it with
  `Error: invalid config 'runtime.toml: runtime.web.auth=token requires runtime.control.auth_token'`.
  Put it back.
- Give the control endpoint a network address: add `endpoint = "tcp://0.0.0.0:7000"` under
  `[runtime.control]`. `validate` refuses it with
  `Error: control error 'tcp endpoint must be loopback (use unix:// for local sockets)'`. Take it out
  again.

## Reference

- Specification 11: the web interface's authentication and TLS, the default control endpoint
  (§6.8.0a), and pairing with its roles (§6.9.5, runtime control authorization).
- Tutorial 15, a mesh's `auth_token`.
