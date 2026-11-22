# Runtime Cloud Examples

Docs category: `docs/public/examples/runtime-cloud.md`

This example pack supports the runtime cloud guides in `docs/guides/`.

## Included Files

Runtime profile configs:
- `examples/runtime_cloud/runtime-a-dev.toml`
- `examples/runtime_cloud/runtime-b-dev.toml`
- `examples/runtime_cloud/runtime-a-wan.toml` and `runtime-b-wan.toml` (the federation guide's two sites)
- `examples/runtime_cloud/runtime-plant.toml`
- `examples/runtime_cloud/runtime-wan.toml`
- `examples/runtime_cloud/multi_host_topology_demo/` (5-runtime topology demo pack)

Dispatch/preflight payloads:
- `examples/runtime_cloud/dispatch-status-read.json`
- `examples/runtime_cloud/dispatch-cmd-invoke-restart.json`
- `examples/runtime_cloud/dispatch-ha-cmd-seq.json`
- `examples/runtime_cloud/preflight-cross-site-deny.json`
- `examples/runtime_cloud/preflight-cross-site-allow.json`
- `examples/runtime_cloud/dispatch-cross-site-cfg-apply.json`
- `examples/runtime_cloud/rollout-create.json`

## Typical Usage

Use these files with:
- `docs/guides/RUNTIME_CLOUD_QUICKSTART.md`
- `docs/guides/RUNTIME_CLOUD_PROFILE_COOKBOOK.md`
- `docs/guides/RUNTIME_CLOUD_FEDERATION_GUIDE.md`
- `docs/guides/RUNTIME_CLOUD_TROUBLESHOOTING.md`

For a richer local topology visual demo (5 runtimes + explicit mesh links +
mixed protocols + preloaded hosts/devices/transport preferences):

```bash
cd examples/runtime_cloud/multi_host_topology_demo
./start-5.sh ~/trust-cloud-topology-demo
```

`start-5.sh` auto-bootstraps missing projects. Use
`TRUST_TOPOLOGY_DEMO_REFRESH=1` to force a full refresh of demo files.

Example preflight command:

```bash
curl -s http://127.0.0.1:18081/api/runtime-cloud/actions/preflight \
  -H 'content-type: application/json' \
  -d @examples/runtime_cloud/dispatch-status-read.json | jq
```

Example rollout command:

```bash
curl -s http://127.0.0.1:18081/api/runtime-cloud/rollouts \
  -H 'content-type: application/json' \
  -d @examples/runtime_cloud/rollout-create.json | jq
```

## Notes

- `runtime-plant.toml` and `runtime-wan.toml` use token authentication: add your own
  `runtime.control.auth_token`, replace the mesh token placeholder and give the certificate paths
  before the first start. The examples commit no token.
- `runtime-wan.toml` ships with sample allowlist rules; tighten these per site and action.
- Mesh examples pin `runtime.mesh.zenohd_version = "1.7.2"` and include explicit
  `runtime.mesh.role/connect/plugin_versions` keys to match the runtime-cloud baseline policy.
- A mesh links only to the peers its `runtime.mesh.connect` entries name and to the runtimes that
  connect to its `listen` endpoint; it does not look for other meshes on the network. The dev
  profiles link runtime-a to runtime-b. `runtime-plant.toml` and `runtime-wan.toml` only listen:
  name the peers each one exchanges values with in its `connect`, or in theirs.
