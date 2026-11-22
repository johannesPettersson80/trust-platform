# Runtime Cloud Quickstart

Goal: bring two runtimes online, verify discovery/state projection, and perform cross-runtime dispatch with deterministic preflight.

## 1) Prerequisites

- `trust-runtime` available in `PATH`
- Two terminals
- `curl` and `jq`
- Linux/macOS shell

## 2) Create Two Runtime Projects From the Dev Profiles

Each project starts as an empty folder with its dev profile as `runtime.toml`:

```bash
mkdir -p ~/trust-cloud-demo/runtime-a ~/trust-cloud-demo/runtime-b
cp examples/runtime_cloud/runtime-a-dev.toml ~/trust-cloud-demo/runtime-a/runtime.toml
cp examples/runtime_cloud/runtime-b-dev.toml ~/trust-cloud-demo/runtime-b/runtime.toml
```

The profiles name the resources `runtime_a` and `runtime_b`, which are also the runtimes' IDs in the
runtime cloud. A resource name is an IEC identifier (letters, digits and underscores), and a build
refuses a `runtime.toml` resource that is not the `RESOURCE` its program declares, so the first start
below writes each program for its profile's resource.

If ports `18081`, `18082`, `5201`, or `5202` are already in use on your host, update
`listen` values in the copied `runtime.toml` files before startup.

The example configs include explicit mesh baseline keys:

- `runtime.mesh.role = "peer"`
- `runtime.mesh.connect = ["127.0.0.1:5202"]` in runtime-a and `[]` in runtime-b: a mesh links
  only to the peers its `connect` entries name and to the runtimes that connect to its `listen`
  endpoint, so runtime-a links to runtime-b's mesh port and runtime-b accepts it
- `runtime.mesh.zenohd_version = "1.7.2"`

If you change runtime-b's mesh `listen` port, change runtime-a's `connect` entry with it.

## 3) Start Both Runtimes

The first start creates each project's program (`src/` and `io.toml`) and builds it.

Terminal 1:

```bash
trust-runtime play --project ~/trust-cloud-demo/runtime-a
```

Terminal 2:

```bash
trust-runtime play --project ~/trust-cloud-demo/runtime-b
```

## 4) Verify Runtime Cloud State

```bash
curl -s http://127.0.0.1:18081/api/runtime-cloud/state | jq '{context: .context, nodes: .topology.nodes, edges: .topology.edges}'
```

Expected:

- `context.connected_via` exists
- `topology.nodes` contains `runtime_a` and `runtime_b`
- `topology.edges` shows communication links

## 5) Preflight a Cross-Runtime Read

```bash
curl -s http://127.0.0.1:18081/api/runtime-cloud/actions/preflight \
  -H 'content-type: application/json' \
  -d @examples/runtime_cloud/dispatch-status-read.json | jq
```

Expected:

- `allowed: true`
- no `denial_code`

## 6) Dispatch Cross-Runtime Read and Verify Audit Link

```bash
curl -s http://127.0.0.1:18081/api/runtime-cloud/actions/dispatch \
  -H 'content-type: application/json' \
  -d @examples/runtime_cloud/dispatch-status-read.json | jq
```

Expected:

- top-level `ok: true`
- `results[0].runtime_id == "runtime_b"`
- `results[0].audit_id` present

## 7) Optional: Local Config Apply Through Runtime Cloud Dispatch

```bash
curl -s http://127.0.0.1:18081/api/runtime-cloud/actions/dispatch \
  -H 'content-type: application/json' \
  -d '{
    "api_version": "1.0",
    "request_id": "req-quickstart-cfg-1",
    "connected_via": "runtime_a",
    "target_runtimes": ["runtime_a"],
    "actor": "spiffe://trust/default-site/engineer-1",
    "action_type": "cfg_apply",
    "dry_run": false,
    "payload": { "params": { "log.level": "debug" } }
  }' | jq
```

Expected:

- `ok: true`
- per-target `audit_id` present

## 8) Next Steps

- `docs/guides/RUNTIME_CLOUD_PROFILE_COOKBOOK.md`
- `docs/guides/RUNTIME_CLOUD_FEDERATION_GUIDE.md`
