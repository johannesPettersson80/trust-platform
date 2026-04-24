# Runtime Cloud Federation Guide

Goal: enable cross-site runtime cloud control safely with explicit allowlists and auditable outcomes.

Important:

- The TOML blocks in this document are policy overlays, not complete `runtime.toml` files.
- Start from a generated baseline (`trust-runtime wizard --path <project>`), then merge these sections.
- For complete runnable examples, use `examples/runtime_cloud/runtime-*.toml`.

## 1) Federation Model

- A local site can act on remote-site runtimes through runtime cloud dispatch
- Cross-site write actions are denied by default in `wan` unless explicitly allowlisted
- Dry-run preflight is mandatory before cross-site writes
- Mesh/version baseline must stay aligned to Zenoh `1.7.2` across runtimes/gateways unless an approved exception exists

## 2) Runtime ID and Actor Conventions

Use consistent identities:

- Runtime IDs: `<site>/<runtime>` (example: `site-b/runtime-b`)
- Actor identities: `spiffe://trust/<site>/<principal>`

## 3) Baseline WAN Configuration

On the egress runtime (originating cross-site actions):

```toml
# Overlay only
[runtime.control]
auth_token = "REPLACE_WITH_LONG_RANDOM_TOKEN"

[runtime.web]
auth = "token"
tls = true

[runtime.tls]
mode = "self-managed"
cert_path = "./certs/runtime.crt"
key_path = "./certs/runtime.key"

[runtime.cloud]
profile = "wan"

[runtime.cloud.wan]
allow_write = []
```

## 4) Verify Default Cross-Site Deny

```bash
curl -s http://127.0.0.1:18081/api/runtime-cloud/actions/preflight \
  -H 'content-type: application/json' \
  -d @examples/runtime_cloud/preflight-cross-site-deny.json | jq
```

Expected:

- `allowed: false`
- `denial_code: "permission_denied"`
- reason references cross-site write + explicit allowlist requirement

## 5) Add Explicit Allowlist Rule

```toml
# Overlay only
[runtime.cloud.wan]
allow_write = [
  { action = "cfg_apply", target = "site-b/*" }
]
```

Re-run preflight:

```bash
curl -s http://127.0.0.1:18081/api/runtime-cloud/actions/preflight \
  -H 'content-type: application/json' \
  -d @examples/runtime_cloud/preflight-cross-site-allow.json | jq
```

Expected:

- `allowed: true`
- no `denial_code`

## 6) Dispatch and Audit Correlation

After preflight success, dispatch to `/api/runtime-cloud/actions/dispatch`.

Required checks:

- top-level `request_id` matches caller-generated ID
- each target result contains `audit_id`
- audit stream contains corresponding event with same request correlation

## 7) Security Guardrails

- Avoid wildcard `*` targets in production unless formally approved
- Keep action-specific rules (`cfg_apply` and `cmd_invoke`) separate
- Prefer exact target IDs for high-impact commands
- Revoke by setting `allow_write = []` and reloading config

## 8) Deterministic Failure Modes

- Missing WAN allowlist match -> `permission_denied`
- Target without secure transport metadata in `plant`/`wan` -> deterministic deny
- Actor role too low -> ACL denial code (`acl_denied_cfg_write` for config writes)

## 9) Evidence Requirements

For each federation policy change:

- capture preflight deny/allow output
- capture dispatch output with `request_id` and `audit_id`
- archive logs under your gate artifact path
