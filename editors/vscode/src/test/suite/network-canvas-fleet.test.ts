import {
  assert,
  os,
  buildNetworkCanvasModel,
  mergeFleetTopologies,
  offlineTopologyForTarget,
  buildCanvasGraph,
  buildGraph,
  RUNNING,
} from "./network-canvas-fixtures";
import type {
  FleetTopologyResponse,
  RuntimeTarget,
} from "./network-canvas-fixtures";

suite("Network Canvas", function () {


  test("mergeFleetTopologies aggregates multiple runtimes: a host appears once with unioned runtimes", () => {
    const rt = (id: string) => ({
      runtime_id: id,
      name: id,
      mode: "online",
      cycle_ms: 10,
      health: "connected",
      detail: "",
      endpoints: [],
    });
    const host = (id: string, runtimes: ReturnType<typeof rt>[], containers: unknown[] = []) => ({
      host_id: id,
      hostname: id,
      arch: "x",
      os: "linux",
      ips: [],
      containers: containers as never[],
      runtimes,
    });
    const link = (id: string) => ({
      id,
      from: `${id}a`,
      to: `${id}b`,
      protocol: "mesh",
      role: "peer",
      direction: "out",
      same_host: false,
      status: "ok",
      secure: true,
    });
    const a: FleetTopologyResponse = {
      schema_version: 3,
      hosts: [host("H", [rt("A")])],
      links: [link("L1")],
      shared: [{ id: "S1", kind: "broker", name: "b", address: "x", used_by: ["A"] }],
      external: [{ id: "E1", name: "e1", kind: "peer", via_protocol: ["mesh"], direction: "out" }],
    };
    const b: FleetTopologyResponse = {
      schema_version: 2,
      hosts: [host("H", [rt("B")]), host("H2", [rt("C")])],
      links: [link("L1"), link("L2")],
      shared: [{ id: "S1", kind: "broker", name: "b", address: "x", used_by: ["B"] }],
      external: [
        { id: "E1", name: "e1", kind: "peer", via_protocol: ["mesh"], direction: "out" },
        { id: "E2", name: "e2", kind: "peer", via_protocol: ["mesh"], direction: "out" },
      ],
    };

    const merged = mergeFleetTopologies([a, undefined, b]);
    assert.strictEqual(merged.schema_version, 3, "schema_version is the max");
    assert.strictEqual(merged.hosts.length, 2, "host H appears once + H2");
    const h = merged.hosts.find((host) => host.host_id === "H");
    assert.strictEqual(h?.runtimes.length, 2, "H unions runtimes A + B");
    assert.strictEqual(merged.links.length, 2, "L1 deduped, L1 + L2 kept");
    assert.strictEqual(merged.external.length, 2, "E1 deduped, E1 + E2 kept");
    assert.deepStrictEqual(
      merged.shared.find((s) => s.id === "S1")?.used_by.slice().sort(),
      ["A", "B"],
      "shared.used_by is unioned"
    );
  });
  test("mergeFleetTopologies keeps configured endpoints on the same live runtime", () => {
    const runtime = (endpoints: FleetTopologyResponse["hosts"][number]["runtimes"][number]["endpoints"]): FleetTopologyResponse["hosts"][number]["runtimes"][number] => ({
      runtime_id: "RESOURCE",
      name: "Simulator",
      mode: "online",
      cycle_ms: 50,
      health: "connected",
      detail: "Running.",
      endpoints,
    });
    const host = (endpoints: FleetTopologyResponse["hosts"][number]["runtimes"][number]["endpoints"]): FleetTopologyResponse["hosts"][number] => ({
      host_id: "host:local",
      hostname: "This computer",
      arch: "aarch64",
      os: "linux",
      ips: ["127.0.0.1"],
      containers: [],
      runtimes: [runtime(endpoints)],
    });
    const simulated = {
      id: "endpoint:RESOURCE:simulated",
      kind: "field",
      protocol: "simulated",
      name: "Simulated I/O",
      role: "owned_driver",
      health: "connected",
      detail: "Running.",
      owned: true,
      supports_test: true,
    };
    const ads = {
      id: "endpoint:RESOURCE:ads",
      kind: "service",
      protocol: "ads",
      name: "ADS client",
      role: "supervisory_client",
      health: "configured_policy",
      detail: "Configured in ads.toml. Restart the runtime to load it.",
      owned: true,
      supports_test: true,
    };
    const merged = mergeFleetTopologies([
      { schema_version: 4, hosts: [host([simulated])], links: [], shared: [], external: [] },
      { schema_version: 4, hosts: [host([ads])], links: [], shared: [], external: [] },
    ]);

    const endpoints = merged.hosts[0]?.runtimes[0]?.endpoints ?? [];
    assert.deepStrictEqual(
      endpoints.map((endpoint) => endpoint.id).sort(),
      ["endpoint:RESOURCE:ads", "endpoint:RESOURCE:simulated"],
      "same-runtime live topology keeps configured endpoints waiting for restart"
    );
  });
  test("configured ADS overlay on a live simulator says restart required, not stopped", () => {
    const topology: FleetTopologyResponse = {
      schema_version: 4,
      hosts: [
        {
          host_id: "host:local",
          hostname: "raspberrypi",
          arch: "aarch64",
          os: "linux",
          ips: ["127.0.0.1"],
          containers: [],
          runtimes: [
            {
              runtime_id: "ADS live TwinCAT",
              name: "ADS live TwinCAT",
              mode: "simulate",
              cycle_ms: 50,
              health: "simulate",
              detail: "Runtime answered fleet.topology from its control channel.",
              endpoints: [
                {
                  id: "endpoint:ADS live TwinCAT:ads",
                  kind: "service",
                  protocol: "ads",
                  name: "ADS client",
                  role: "client",
                  health: "configured_policy",
                  detail: "Configured in ADS project config; runtime is not running.",
                  owned: true,
                  supports_test: true,
                },
              ],
            },
          ],
        },
      ],
      links: [],
      shared: [],
      external: [],
    };

    const model = buildNetworkCanvasModel({
      stage: "runtime_live",
      runtime: RUNNING,
      topology,
    });
    const runtime = model.fleet?.hosts[0]?.runtimes[0];
    assert.strictEqual(
      runtime?.health,
      "simulate",
      "a running simulator must not become Stopped because a new ADS endpoint needs restart"
    );
    assert.match(
      runtime?.endpoints[0]?.detail ?? "",
      /restart the runtime to apply/i,
      "the configured endpoint should tell the user to restart/apply, not claim the runtime is not running"
    );
  });
  test("a configured-but-unreachable fleet peer synthesizes an UNKNOWN node (not 'stopped'), never green", () => {
    const base: RuntimeTarget = {
      mode: "online",
      endpoint: "10.0.0.9:5510",
      endpointEnabled: true,
      reachable: false,
      status: "online_unreachable",
      label: "cell1",
      credentialChannel: "untrusted_remote_plain_tcp",
    };
    const topo = offlineTopologyForTarget(base);
    assert.ok(topo, "an unreachable configured peer should still appear (synthesized)");
    if (topo) {
      const runtime = topo.hosts[0].runtimes[0];
      // We don't know if it's stopped or just unreachable → "unknown" (grey/ghosted), never green.
      assert.strictEqual(runtime.health, "unknown", "unknown/grey, never connected/green");
      assert.strictEqual(runtime.mode, "unknown");
      assert.strictEqual(runtime.endpoints.length, 0);
      assert.strictEqual(topo.hosts[0].hostname, "cell1");
    }
    assert.strictEqual(
      offlineTopologyForTarget({ ...base, status: "auth_failed" })?.hosts[0].runtimes[0].health,
      "error",
      "auth failure is a real error"
    );
    assert.strictEqual(
      offlineTopologyForTarget({
        ...base,
        status: "auth_failed",
        authFailureKind: "missing",
      })?.hosts[0].runtimes[0].detail,
      "No auth token provided — this runtime requires one."
    );
    assert.strictEqual(
      offlineTopologyForTarget({
        ...base,
        status: "auth_failed",
        authFailureKind: "rejected",
      })?.hosts[0].runtimes[0].detail,
      "Auth token rejected — check it and try again."
    );
    assert.strictEqual(
      offlineTopologyForTarget({ ...base, status: "online_reachable" }),
      undefined,
      "a reachable peer uses its real fleet.topology, not a synthetic node"
    );
    assert.strictEqual(
      offlineTopologyForTarget({ ...base, endpoint: undefined }),
      undefined,
      "no endpoint → nothing to show"
    );
  });
  test("buildCanvasGraph shows added fleet peers even on the stopped local-simulator view", () => {
    const peerTopology: FleetTopologyResponse = {
      schema_version: 3,
      hosts: [
        {
          host_id: "fleet:10.0.0.9:5510",
          hostname: "10.0.0.9:5510",
          arch: "",
          os: "",
          ips: [],
          containers: [],
          runtimes: [
            {
              runtime_id: "fleet:10.0.0.9:5510:runtime",
              name: "cell1",
              mode: "stopped",
              cycle_ms: 0,
              health: "stopped",
              detail: "not reachable",
              endpoints: [],
            },
          ],
        },
      ],
      links: [],
      shared: [],
      external: [],
    };
    // Local sim stopped (no fleet) → localRuntimeGraph; the added peer must STILL appear beside it.
    const graph = buildCanvasGraph(buildNetworkCanvasModel("runtime_live"), undefined, peerTopology);
    const hostIds = graph.hosts.map((h) => h.id);
    assert.ok(hostIds.includes("host:this-computer"), "local simulator node is preserved");
    assert.ok(hostIds.includes("fleet:10.0.0.9:5510"), "added peer appears alongside it");
    const peer = graph.hosts.find((h) => h.id === "fleet:10.0.0.9:5510");
    assert.strictEqual(peer?.runtimes[0].health, "stopped", "peer stays stopped/grey, never green");
  });
  test("auth-failed synthetic runtime nodes keep control endpoint for inspector actions", () => {
    const endpoint = "tcp://127.0.0.1:33101";
    const topo = offlineTopologyForTarget({
      mode: "online",
      endpoint,
      endpointEnabled: true,
      reachable: true,
      status: "auth_failed",
      label: "discoveredcell",
      credentialChannel: "trusted_same_host",
    });
    assert.ok(topo, "auth-failed configured runtime should still synthesize a node");
    const graph = buildCanvasGraph(buildNetworkCanvasModel("runtime_live"), undefined, topo);
    const rendered = buildGraph(graph).nodes.find((node) => node.type === "runtime" && node.id.includes(endpoint));
    assert.ok(rendered, "synthetic auth-failed runtime node should render");
    assert.strictEqual(
      rendered?.data.controlEndpoint,
      endpoint,
      "runtime inspector actions need the endpoint for Set auth token and Set as run target"
    );
  });
  test("a new project with no configured runtime shows the local simulator node, not an empty screen", () => {
    const graph = buildCanvasGraph(buildNetworkCanvasModel("runtime_live"), undefined);
    assert.strictEqual(graph.kind, "graph");
    assert.strictEqual(graph.hosts.length, 1);
    const runtime = graph.hosts[0].runtimes[0];
    assert.ok(runtime, "local simulator runtime node is always present");
    assert.notStrictEqual(runtime.health, "connected", "honest: not green until proven");
  });
  test("a stopped local control socket renders neutral stopped state, not an alarm fault", () => {
    const graph = buildCanvasGraph(
      buildNetworkCanvasModel({
        stage: "runtime_live",
        failure: {
          kind: "stale_runtime",
          message: "Runtime not reachable: unix:///tmp/trust-runtime-line-123.sock",
        },
      }),
      undefined
    );
    const runtime = graph.hosts[0].runtimes[0];
    assert.strictEqual(runtime.health, "stopped", "stopped local runtime is neutral, not red");
    assert.deepStrictEqual(graph.faults, [], "stopped local runtime must not spend the red issue channel");
    assert.strictEqual(graph.banner?.kind, "info", "guidance stays neutral/info");
    assert.ok(
      !JSON.stringify(graph).includes("unix:///tmp/trust-runtime-line"),
      "raw local socket paths must not leak into the rendered graph"
    );
  });
  test("a running local simulator renders as a connected runtime node", () => {
    const graph = buildCanvasGraph(
      buildNetworkCanvasModel({ stage: "runtime_live", runtime: RUNNING }),
      undefined
    );
    assert.strictEqual(graph.hosts[0].runtimes[0].health, "connected");
  });
  test("a live local simulator topology replaces the stopped project overlay instead of twinning", () => {
    const liveTopology: FleetTopologyResponse = {
      schema_version: 3,
      hosts: [
        {
          host_id: "host:this-computer",
          hostname: os.hostname(),
          arch: process.arch,
          os: process.platform,
          ips: ["127.0.0.1"],
          containers: [],
          runtimes: [
            {
              runtime_id: "RESOURCE",
              name: "RESOURCE",
              control_endpoint: "unix:///tmp/trust-local-sim.sock",
              mode: "simulate",
              cycle_ms: 20,
              health: "simulate",
              detail: "Online",
              source: "self",
              endpoints: [
                {
                  id: "endpoint:live:simulated",
                  kind: "field",
                  protocol: "simulated",
                  name: "Simulated I/O",
                  role: "owned_driver",
                  health: "connected",
                  detail: "Live",
                  owned: true,
                  supports_test: true,
                },
              ],
            },
          ],
        },
      ],
      links: [],
      shared: [],
      external: [],
    };
    const offlineProjectTopology: FleetTopologyResponse = {
      schema_version: 4,
      hosts: [
        {
          host_id: "host:this-computer",
          hostname: os.hostname(),
          arch: process.arch,
          os: process.platform,
          ips: ["127.0.0.1"],
          containers: [],
          runtimes: [
            {
              runtime_id: "ADS live TwinCAT",
              name: "ADS live TwinCAT",
              control_endpoint: "unix:///tmp/trust-local-sim.sock",
              mode: "stopped",
              cycle_ms: 20,
              health: "configured_policy",
              detail: "Configured in project files; runtime is not running.",
              source: "config",
              endpoints: [
                {
                  id: "endpoint:ADS live TwinCAT:simulated",
                  kind: "field",
                  protocol: "simulated",
                  name: "Simulated I/O",
                  role: "owned_driver",
                  health: "configured_policy",
                  detail: "Configured in io.toml; runtime is not running.",
                  owned: true,
                  supports_test: true,
                  source: "config",
                },
                {
                  id: "endpoint:ADS live TwinCAT:ads",
                  kind: "service",
                  protocol: "ads",
                  name: "ADS client",
                  role: "client",
                  health: "configured_policy",
                  detail: "Configured in ADS project config; runtime is not running.",
                  owned: true,
                  supports_test: true,
                  source: "config",
                },
              ],
            },
          ],
        },
      ],
      links: [],
      shared: [],
      external: [],
    };
    const merged = mergeFleetTopologies([liveTopology, offlineProjectTopology]);

    const graph = buildCanvasGraph(
      buildNetworkCanvasModel({
        stage: "runtime_live",
        runtime: RUNNING,
        topology: merged,
      }),
      merged
    );

    assert.strictEqual(graph.hosts.length, 1, "local simulator stays on one host");
    assert.strictEqual(graph.summary, "1 host · 1 runtime · 2 endpoints");
    const runtimes = graph.hosts[0].runtimes;
    assert.strictEqual(runtimes.length, 1, "one local simulator runtime, not RESOURCE + configured project twin");
    assert.strictEqual(
      runtimes[0].id,
      "runtime:local",
      "the one owned Simulator must use the canonical local identity so its inspector stays status-only"
    );
    assert.strictEqual(runtimes[0].name, "Simulator", "ST resource names must not replace the run target");
    assert.strictEqual(runtimes[0].health, "connected");
    assert.match(
      runtimes[0].detail,
      /Running/i,
      "the accepted lifecycle must render literal Running, not generic Simulator topology health"
    );
    assert.strictEqual(
      runtimes[0].endpoints.filter((endpoint) => endpoint.protocol === "simulated").length,
      1,
      "the project-file overlay must not duplicate the already-live Simulated I/O endpoint"
    );
    assert.ok(
      runtimes[0].endpoints.some((endpoint) => endpoint.protocol === "ads"),
      "configured ADS endpoint stays visible while it waits for restart"
    );
    assert.match(
      runtimes[0].endpoints.find((endpoint) => endpoint.protocol === "ads")?.detail ?? "",
      /restart the runtime to apply/i,
      "pending ADS endpoint explains the required restart"
    );
  });
  test("a live local simulator topology hides raw ST resource names even when mode is missing", () => {
    const liveTopology: FleetTopologyResponse = {
      schema_version: 3,
      hosts: [
        {
          host_id: "host:this-computer",
          hostname: os.hostname(),
          arch: process.arch,
          os: process.platform,
          ips: ["127.0.0.1"],
          containers: [],
          runtimes: [
            {
              runtime_id: "RESOURCE",
              name: "RESOURCE",
              control_endpoint: "unix:///tmp/trust-local-sim.sock",
              mode: "",
              cycle_ms: 20,
              health: "connected",
              detail: "Online",
              endpoints: [],
            },
          ],
        },
      ],
      links: [],
      shared: [],
      external: [],
    };

    const graph = buildCanvasGraph(
      buildNetworkCanvasModel({
        stage: "runtime_live",
        runtime: RUNNING,
        topology: liveTopology,
      }),
      liveTopology
    );

    assert.strictEqual(graph.hosts[0].runtimes[0].name, "Simulator");
  });
});
