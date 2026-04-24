# Runtime Cloud Profile Cookbook

This cookbook defines when and how to use runtime cloud security profiles.

Important:

- The TOML blocks in this document are profile overlays, not complete `runtime.toml` files.
- Start from a generated baseline (`trust-runtime wizard --path <project>`), then merge these sections.
- For complete runnable examples, use `examples/runtime_cloud/runtime-*.toml`.

## 1) Profile Summary

| Profile | Typical Use | Required Security Baseline | Write Policy |
| --- | --- | --- | --- |
| `dev` | local lab, fast iteration | local web auth allowed, TLS optional | local and remote writes allowed by ACL |
| `plant` | same-site production | `runtime.web.auth = "token"` and `runtime.web.tls = true` | writes allowed by ACL, remote targets must expose secure transport metadata |
| `wan` | cross-site/federated production | same as `plant` | remote write denied by default unless `runtime.cloud.wan.allow_write` rule matches |

Mesh baseline for all profiles:

- `runtime.mesh.zenohd_version` must stay on `1.7.2` unless an approved exception is documented.
- Use explicit `runtime.mesh.role` + `runtime.mesh.connect` keys in profile overlays/examples.
- `runtime.mesh.plugin_versions` is required for `role = "router"` deployments.

## 2) `dev` Profile

Use when speed is prioritized over strict transport posture.

```toml
# Overlay only
[runtime.cloud]
profile = "dev"

[runtime.cloud.wan]
allow_write = []
```

Notes:

- Recommended only on trusted local networks
- Keep this out of plant/WAN environments

## 3) `plant` Profile

Use for production inside one site.

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
profile = "plant"
```

Generate local test certificates (self-managed profile) before first start:

```bash
mkdir -p certs
openssl req -x509 -newkey rsa:4096 -sha256 -days 365 -nodes \
  -keyout certs/runtime.key \
  -out certs/runtime.crt \
  -subj "/CN=$(hostname)"
```

Required behavior:

- runtime cloud endpoints deny requests if token auth/TLS preconditions are not met
- remote targets without secure transport metadata are denied in preflight/dispatch

## 4) `wan` Profile

Use for cross-site federation.

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

Important default:

- cross-runtime write actions (`cfg_apply`, `cmd_invoke`) are denied unless explicitly allowlisted

## 5) WAN Allowlist Rules

```toml
[runtime.cloud.wan]
allow_write = [
  { action = "cfg_apply", target = "site-b/*" },
  { action = "cmd_invoke", target = "site-b/runtime-boiler-1" }
]
```

Target matching semantics:

- `*` matches all targets
- `site-b/*` prefix match
- `*/runtime-1` suffix match
- exact string is exact match

## 6) Promotion Path (`dev -> plant -> wan`)

1. Start in `dev` for local functional validation
2. Move to `plant` once token/TLS and cert lifecycle are ready
3. Move to `wan` only after explicit allowlist and federation audit checks are in place

## 7) Verification Checklist

After any profile change:

```bash
trust-runtime validate --project <project>
trust-runtime ctl --project <project> config.get
curl -s http://<host>:<port>/api/runtime-cloud/state | jq '.context'
```

For `wan`, run dry-run preflight before dispatch.

## 8) HardRT Host Readiness (PREEMPT_RT Recommended)

For runtimes carrying T0 workloads, validate Linux host readiness before deployment:

```bash
uname -a
cat /sys/kernel/realtime
ps -eo pid,cls,rtprio,comm | head -n 20
```

Deployment checks:

- kernel should be PREEMPT_RT capable (`/sys/kernel/realtime` reports `1` when enabled)
- realtime tasks should run with explicit RT scheduling class/priority policy
- T0 comms contracts must still be treated as same-host only; do not claim hard determinism over generic IP mesh
