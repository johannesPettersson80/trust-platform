# Runtime Cloud Quickstart

Goal: bring two runtimes online, verify discovery/state projection, and perform cross-runtime dispatch with deterministic preflight.

## 1) Prerequisites

- `trust-runtime` available in `PATH`
- Two terminals
- `curl` and `jq`
- Linux/macOS shell

## 2) Bootstrap Two Runtime Projects

Create project folders once (first run creates bundle layout):

```bash
mkdir -p ~/trust-cloud-demo
trust-runtime play --project ~/trust-cloud-demo/runtime-a
# Stop with Ctrl+C after startup messages appear.
trust-runtime play --project ~/trust-cloud-demo/runtime-b
# Stop with Ctrl+C after startup messages appear.
```

## 3) Apply Known-Good Dev Profile Configs

```bash
cp examples/runtime_cloud/runtime-a-dev.toml ~/trust-cloud-demo/runtime-a/runtime.toml
cp examples/runtime_cloud/runtime-b-dev.toml ~/trust-cloud-demo/runtime-b/runtime.toml
```

If ports `18081`, `18082`, `5201`, or `5202` are already in use on your host, update
`listen` values in the copied `runtime.toml` files before startup.

The example configs include explicit mesh baseline keys:

- `runtime.mesh.role = "peer"`
- `runtime.mesh.connect = []`
- `runtime.mesh.zenohd_version = "1.7.2"`

## 4) Start Both Runtimes

Terminal 1:

```bash
trust-runtime play --project ~/trust-cloud-demo/runtime-a
```

Terminal 2:

```bash
trust-runtime play --project ~/trust-cloud-demo/runtime-b
```

## 5) Verify Runtime Cloud State

```bash
curl -s http://127.0.0.1:18081/api/runtime-cloud/state | jq '{context: .context, nodes: .topology.nodes, edges: .topology.edges}'
```

Expected:

- `context.connected_via` exists
- `topology.nodes` contains `runtime-a` and `runtime-b`
- `topology.edges` shows communication links

## 6) Preflight a Cross-Runtime Read

```bash
curl -s http://127.0.0.1:18081/api/runtime-cloud/actions/preflight \
  -H 'content-type: application/json' \
  -d @examples/runtime_cloud/dispatch-status-read.json | jq
```

Expected:

- `allowed: true`
- no `denial_code`

## 7) Dispatch Cross-Runtime Read and Verify Audit Link

```bash
curl -s http://127.0.0.1:18081/api/runtime-cloud/actions/dispatch \
  -H 'content-type: application/json' \
  -d @examples/runtime_cloud/dispatch-status-read.json | jq
```

Expected:

- top-level `ok: true`
- `results[0].runtime_id == "runtime-b"`
- `results[0].audit_id` present

## 8) Optional: Local Config Apply Through Runtime Cloud Dispatch

```bash
curl -s http://127.0.0.1:18081/api/runtime-cloud/actions/dispatch \
  -H 'content-type: application/json' \
  -d '{
    "api_version": "1.0",
    "request_id": "req-quickstart-cfg-1",
    "connected_via": "runtime-a",
    "target_runtimes": ["runtime-a"],
    "actor": "spiffe://trust/default-site/engineer-1",
    "action_type": "cfg_apply",
    "dry_run": false,
    "payload": { "params": { "log.level": "debug" } }
  }' | jq
```

Expected:

- `ok: true`
- per-target `audit_id` present

## 9) Next Steps

- `docs/guides/RUNTIME_CLOUD_PROFILE_COOKBOOK.md`
- `docs/guides/RUNTIME_CLOUD_FEDERATION_GUIDE.md`
- `docs/guides/RUNTIME_CLOUD_UI_WALKTHROUGH.md`
