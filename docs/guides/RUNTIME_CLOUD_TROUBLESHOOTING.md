# Runtime Cloud Troubleshooting

Use this runbook when runtime cloud behavior is not as expected.

## 1) First-Tier Checks

```bash
trust-runtime ctl --project <project> status
trust-runtime ctl --project <project> config.get
curl -s http://<host>:<port>/api/runtime-cloud/state | jq
```

Confirm:

- runtime is healthy
- config/profile matches intent
- runtime cloud state/topology is populated

HardRT truth (non-negotiable):

- T0 HardRT is same-host only.
- Generic IP mesh (`T1/T2/T3`) is non-HardRT by design and must never be treated as a deterministic fallback for T0.

## 2) Symptom Matrix

| Symptom | Likely Cause | Deterministic Fix |
| --- | --- | --- |
| Peer runtime not visible | discovery disabled or blocked | enable `[runtime.discovery]`, verify network/mDNS, check service name uniqueness |
| Peer visible but stale/degraded | mesh metadata/path unavailable | verify `[runtime.mesh]` listen/auth/tls and peer reachability |
| `not_configured` deny on preflight | profile preconditions not met | for `plant`/`wan`, enforce `runtime.web.auth = "token"` and `runtime.web.tls = true` |
| `permission_denied` on cross-site write | WAN allowlist missing | add `runtime.cloud.wan.allow_write` rule for action+target and retry dry-run preflight |
| `acl_denied_cfg_write` | actor role too low for config write | use actor with required role (`operator`/`admin` by policy) |
| `contract_violation` with T0 + mesh/IP route text | attempted non-HardRT fallback | re-bind handles to `T0_HardRt`; keep mesh path for ops/diag only |
| revision/etag conflict | concurrent config write | re-fetch config snapshot, rebase change, retry |
| dispatch lacks `audit_id` | outdated runtime or failed dispatch path | verify runtime version and inspect per-target result |
| UI shows healthy when peer unreachable | stale transition not applied yet | re-check `/api/runtime-cloud/state` status and mesh liveliness metadata |

## 3) Profile-Specific Checks

### `dev`
- `runtime.cloud.profile = "dev"`
- local auth/TLS flexibility is expected

### `plant`
- requires token auth and TLS
- denies remote dispatch when target secure metadata is missing

### `wan`
- all `plant` requirements, plus cross-runtime write default-deny
- explicit allowlist required for write actions

## 4) Preflight Before Dispatch

Use `/api/runtime-cloud/actions/preflight` and inspect:

- `allowed`
- `denial_code`
- `denial_reason`
- per-target state (`reachable`, `stale`, `partitioned`)

Do not skip preflight for cross-site or multi-target writes.

## 5) Reliability and Stability Gates

```bash
cargo test -p trust-runtime --test runtime_reliability
./scripts/runtime_load_test.sh tests/fixtures/runtime_reliability_bundle
./scripts/runtime_mesh_tls_stability_gate.sh --iterations 3
./scripts/runtime_comms_conformance_gate.sh
./scripts/check_zenoh_baseline.sh
```

## 6) Evidence Capture Pattern

Store:

- request payload (`request_id` retained)
- preflight/dispatch responses
- relevant `/api/runtime-cloud/state` snapshot
- runtime logs and gate logs

Recommended location:

- `target/gate-artifacts/<timestamp>/...`

## 7) Escalation Criteria

Escalate immediately if:

- HA split-brain safeguards fail
- cross-runtime writes bypass expected deny rules
- realtime/T0 behavior appears to fall back to mesh/IP
- audit correlation (`request_id`/`audit_id`) is missing for protected actions
