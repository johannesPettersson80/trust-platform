# Runtime Cloud Federation Guide

Goal: let a runtime of one site act on a runtime of another site, with an explicit allowlist,
verified HTTPS and an audit trail.

This guide continues `docs/guides/RUNTIME_CLOUD_QUICKSTART.md`. Its two projects,
`~/trust-cloud-demo/runtime-a` and `~/trust-cloud-demo/runtime-b`, become `runtime_a` of `site-a`
and `runtime_b` of `site-b`, both on this computer. Stop both runtimes before step 2. Besides the
quickstart's tools you need OpenSSL 1.1.1 or newer. Run the commands from the root of the truST
repository, as in the quickstart.

## 1) Federation Model

- A runtime belongs to one site, `[runtime.cloud] site`, and to `default-site` without it.
- A runtime names the runtimes of its own site by their resource names and a runtime of another
  site by `<site>/<resource>`: to `runtime_a` of `site-a`, the other runtime is `site-b/runtime_b`.
- In the `wan` profile, a write action (`cfg_apply`, `cmd_invoke`) to another runtime, of its own
  site or another, is denied unless `runtime.cloud.wan.allow_write` allows it. Read actions need no
  rule.
- `plant` and `wan` require token authentication and HTTPS, and a target runtime must serve HTTPS.
- A dispatch passes the caller's token to its target, so the runtimes of a federation accept one
  token.
- For its calls to other runtimes, a runtime trusts the site CA that its `[runtime.tls] ca_path`
  names, and only that CA.
- Preflight every cross-site write as a dry run before you dispatch it.
- Keep the mesh at Zenoh `1.7.2` on every runtime and gateway, as the examples pin it.

## 2) Make a Site CA and a Certificate for Each Runtime

```bash
mkdir -p ~/trust-cloud-demo/ca
openssl req -x509 -newkey rsa:4096 -sha256 -days 365 -nodes \
  -keyout ~/trust-cloud-demo/ca/ca.key -out ~/trust-cloud-demo/ca/ca.crt \
  -subj "/CN=trust-cloud-demo CA" \
  -addext "basicConstraints=critical,CA:TRUE" -addext "keyUsage=critical,keyCertSign,cRLSign"
printf 'subjectAltName=IP:127.0.0.1\nbasicConstraints=CA:FALSE\nextendedKeyUsage=serverAuth,clientAuth\n' \
  > ~/trust-cloud-demo/ca/runtime.ext
for runtime in runtime-a runtime-b; do
  certs=~/trust-cloud-demo/$runtime/certs
  mkdir -p "$certs"
  openssl req -newkey rsa:2048 -sha256 -nodes -subj "/CN=$runtime" \
    -keyout "$certs/runtime.key" -out "$certs/runtime.csr"
  openssl x509 -req -in "$certs/runtime.csr" -days 365 -sha256 \
    -CA ~/trust-cloud-demo/ca/ca.crt -CAkey ~/trust-cloud-demo/ca/ca.key -CAcreateserial \
    -extfile ~/trust-cloud-demo/ca/runtime.ext -out "$certs/runtime.crt"
  cp ~/trust-cloud-demo/ca/ca.crt "$certs/ca.crt"
done
```

Each certificate names the address other runtimes reach the runtime at, here `127.0.0.1`. On a
network, name each runtime's address or host name instead (`IP:<address>` or `DNS:<name>`). Keep
`ca.key` off the runtimes.

## 3) Give the Runtimes the WAN Profile, Their Sites and One Token

```bash
token=$(openssl rand -hex 32)
for runtime in runtime-a runtime-b; do
  sed "s/REPLACE_WITH_THE_SHARED_TOKEN/$token/" "examples/runtime_cloud/$runtime-wan.toml" \
    > ~/trust-cloud-demo/$runtime/runtime.toml
done
printf 'X-Trust-Token: %s\n' "$token" > ~/trust-cloud-demo/token.header
chmod 600 ~/trust-cloud-demo/token.header ~/trust-cloud-demo/runtime-*/runtime.toml \
  ~/trust-cloud-demo/runtime-*/certs/runtime.key
```

The profiles `examples/runtime_cloud/runtime-a-wan.toml` and `runtime-b-wan.toml` are the
quickstart's dev profiles with:

- `[runtime.cloud] profile = "wan"`, and `site = "site-a"` or `site = "site-b"`;
- `[runtime.web] auth = "token"` and `tls = true`, and a mesh with TLS;
- `[runtime.tls] mode = "provisioned"` with the runtime's `certs/runtime.crt` and
  `certs/runtime.key` and the site CA `certs/ca.crt`, relative to the project;
- `auth_token`, the token every runtime of the federation accepts;
- `[runtime.cloud.wan] allow_write = []`: no cross-site write yet.

Start both runtimes again, as in the quickstart:

Terminal 1:

```bash
trust-runtime play --project ~/trust-cloud-demo/runtime-a
```

Terminal 2:

```bash
trust-runtime play --project ~/trust-cloud-demo/runtime-b
```

The commands below reach `runtime_a` over HTTPS, verify its certificate against the site CA and
send the token:

```bash
curl -s --cacert ~/trust-cloud-demo/ca/ca.crt -H @"$HOME/trust-cloud-demo/token.header" \
  https://127.0.0.1:18081/api/runtime-cloud/state \
  | jq '{context: .context, nodes: [.topology.nodes[] | {runtime_id, site}]}'
```

Expected:

- `context.connected_via` is `runtime_a` and `context.site_scope` is `["site-a"]`
- `nodes` holds `runtime_a` of `site-a` and `site-b/runtime_b` of `site-b`

Without the token, the runtime answers `401`:

```bash
curl -s -o /dev/null -w '%{http_code}\n' --cacert ~/trust-cloud-demo/ca/ca.crt \
  https://127.0.0.1:18081/api/runtime-cloud/state
```

## 4) Verify the Default Cross-Site Deny

```bash
curl -s --cacert ~/trust-cloud-demo/ca/ca.crt -H @"$HOME/trust-cloud-demo/token.header" \
  https://127.0.0.1:18081/api/runtime-cloud/actions/preflight \
  -H 'content-type: application/json' \
  -d @examples/runtime_cloud/preflight-cross-site-deny.json | jq
```

Expected:

- `allowed: false`
- `denial_code: "permission_denied"`
- `denial_reason`: `cross-site write action 'cfg_apply' from site 'site-a' to runtime
  'site-b/runtime_b' (site 'site-b') requires explicit runtime.cloud.wan.allow_write policy`

## 5) Add an Explicit Allowlist Rule

Stop `runtime_a` with Ctrl+C in terminal 1. In `~/trust-cloud-demo/runtime-a/runtime.toml`,
replace the line `allow_write = []` of `[runtime.cloud.wan]` with:

```toml
allow_write = [
  { action = "cfg_apply", target = "site-b/*" }
]
```

Start `runtime_a` again: a runtime reads its allowlist when it starts. Then preflight again:

```bash
curl -s --cacert ~/trust-cloud-demo/ca/ca.crt -H @"$HOME/trust-cloud-demo/token.header" \
  https://127.0.0.1:18081/api/runtime-cloud/actions/preflight \
  -H 'content-type: application/json' \
  -d @examples/runtime_cloud/preflight-cross-site-allow.json | jq
```

Expected:

- `allowed: true`
- no `denial_code`

## 6) Dispatch and Audit Correlation

```bash
curl -s --cacert ~/trust-cloud-demo/ca/ca.crt -H @"$HOME/trust-cloud-demo/token.header" \
  https://127.0.0.1:18081/api/runtime-cloud/actions/dispatch \
  -H 'content-type: application/json' \
  -d @examples/runtime_cloud/dispatch-cross-site-cfg-apply.json | jq
```

Expected:

- `ok: true` and `request_id: "req-tutorial-cross-site-cfg-apply-1"`
- `results[0].runtime_id == "site-b/runtime_b"`, with `ok: true` and an `audit_id`

`runtime_b` now logs at `debug`:

```bash
trust-runtime ctl --project ~/trust-cloud-demo/runtime-b config-get | jq '.result["log.level"]'
```

## 7) Security Guardrails

- Avoid wildcard `*` targets in production unless formally approved
- Keep action-specific rules (`cfg_apply` and `cmd_invoke`) separate
- Prefer exact target IDs for high-impact commands, such as `site-b/boiler_1`
- Revoke a rule by removing it from `allow_write` (`allow_write = []` removes them all) and
  restarting the runtime
- Keep the CA key off the runtimes, and replace a runtime's certificate before it expires

## 8) Deterministic Failure Modes

- Missing WAN allowlist match -> `permission_denied`
- Target that does not serve HTTPS in `plant`/`wan` -> `not_configured`
- Target whose certificate the site CA did not issue, or whose certificate does not name its
  address -> `target_unreachable`, with `invalid peer certificate` in `denial_reason`
- Request without the token -> HTTP `401`
- Actor role too low -> ACL denial code (`acl_denied_cfg_write` for config writes)

## 9) Evidence Requirements

For each federation policy change, keep:

- the preflight output, deny and allow
- the dispatch output with `request_id` and `audit_id`
- the runtimes' logs

## 10) Back to the Quickstart

Stop both runtimes with Ctrl+C. To run the quickstart again, copy its dev profiles into the
projects again (its step 2).
