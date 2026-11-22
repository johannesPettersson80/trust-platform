# Runtime Cloud Topology Demo Pack (5 runtimes, mixed connections + protocols)

This demo pack gives a complete local Runtime Cloud topology scenario:

- 5 runtimes (`runtime_a`..`runtime_e`): each runtime's resource name, and so its ID in the runtime
  cloud, is an IEC identifier; its project folder is `runtime-a`..`runtime-e`
- explicit runtime mesh links (`runtime.mesh.connect`, non-empty on all nodes)
- mixed I/O communication (`mqtt`, `modbus-tcp`, `ethercat`, `simulated`, `loopback`)
  with illustrative devices and brokers: without them on your network, each driver warns
  (`on_error = "warn"`) and the runtimes keep running
- TOML/API-driven operational topology (no manual topology JSON overlay path)
- TOML-defined link transport preferences on `runtime_a` (`realtime` + `zenoh`)
- TOML-defined host grouping (`runtime.discovery.host_group`) for multi-host layout on localhost

The pack is intended for local UI/demo testing.

## 1) Start the full demo (auto-bootstrap)

```bash
cd examples/runtime_cloud/multi_host_topology_demo
./start-5.sh ~/trust-cloud-topology-demo
```

`start-5.sh` auto-runs `bootstrap.sh` if projects are missing/incomplete.
It also clears stale runtime-cloud local state on each run by default, so the canvas stays TOML/API-driven.

Force a full refresh of demo files:

```bash
TRUST_TOPOLOGY_DEMO_REFRESH=1 ./start-5.sh ~/trust-cloud-topology-demo
```

Optional config validation before start:

```bash
TRUST_TOPOLOGY_DEMO_VALIDATE=1 ./start-5.sh ~/trust-cloud-topology-demo
```

## 2) Stop all 5 runtimes

```bash
./stop-5.sh ~/trust-cloud-topology-demo
```

Optional legacy link-transport JSON seed (off by default):

```bash
TRUST_TOPOLOGY_DEMO_SEED_LINK_JSON=1 ./start-5.sh ~/trust-cloud-topology-demo
```

## Runtime Mesh Pattern

- `runtime_a -> runtime_b, runtime_d, runtime_e`
- `runtime_b -> runtime_a, runtime_c`
- `runtime_c -> runtime_b, runtime_d`
- `runtime_d -> runtime_a, runtime_c, runtime_e`
- `runtime_e -> runtime_a, runtime_d`

## Seeded Topology/Comms Data

- Operational topology is derived from `runtime-*.toml` + `io-*.toml` via runtime APIs.
- Runtime cards and adapter/device cards are rendered from runtime-cloud state + I/O config APIs.
- `runtime-a.toml` defines link transport preferences in `[runtime.cloud.links]`:
  - `runtime_a -> runtime_b` = `realtime`
  - `runtime_a -> runtime_c` = `zenoh`
  - `runtime_a -> runtime_d` = `zenoh`
  - `runtime_a -> runtime_e` = `zenoh`
- Runtime TOML files define host groups in `runtime.discovery.host_group`:
  - `runtime_a`, `runtime_b` -> `hq-vm-cluster`
  - `runtime_c`, `runtime_d` -> `line-b-ipc`
  - `runtime_e` -> `remote-site`
- I/O configs intentionally mix local and remote endpoints to show host-attached
  and deterministic external-host endpoint behavior (no free-floating operational endpoints).

## Dashboard URLs

- `runtime_a`: <http://127.0.0.1:19081/>
- `runtime_b`: <http://127.0.0.1:19082/>
- `runtime_c`: <http://127.0.0.1:19083/>
- `runtime_d`: <http://127.0.0.1:19084/>
- `runtime_e`: <http://127.0.0.1:19085/>

## Realistic 3-runtime baseline (2 + 1)

This is the recommended realistic topology baseline:

- Cell A Edge IPC (`runtime.discovery.host_group = "cell-a-edge-ipc"`):
  - `runtime_a_control` (strict realtime I/O)
  - `runtime_a_gateway` (non-realtime comms)
- Cell B Edge IPC (`runtime.discovery.host_group = "cell-b-edge-ipc"`):
  - `runtime_b_control` (strict realtime I/O)

Start it:

```bash
./start-3-realistic.sh ~/trust-cloud-topology-demo
```

`start-3-realistic.sh` also clears stale runtime-cloud local state by default.

Stop it:

```bash
./stop-3-realistic.sh ~/trust-cloud-topology-demo
```

Dashboards:

- `http://127.0.0.1:19181/` (`runtime_a_control`)
- `http://127.0.0.1:19182/` (`runtime_a_gateway`)
- `http://127.0.0.1:19183/` (`runtime_b_control`)

Notes:

- No manual topology JSON is used.
- Host separation is driven by `runtime.discovery.host_group` in TOML.
- Realtime overlay is driven by `[runtime.cloud.links].transports` in TOML.
