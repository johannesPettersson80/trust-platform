import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import {
  buildNetworkCanvasModel,
  type BuildNetworkCanvasModelInput,
} from "../../networkCanvas/model";
import {
  mergeFleetTopologies,
  offlineTopologyForTarget,
  type FleetTopologyResponse,
} from "../../networkCanvas/fleetTopology";
import { buildCanvasGraph } from "../../networkCanvas/graphData";
import type { RuntimeTarget } from "../../runtimeTarget";
import { classifyRuntimeStartFailure } from "../../networkCanvas/runtimeFailures";

const RUNNING = {
  running: true,
  runtimeState: "running" as const,
  runtimeMode: "simulate" as const,
};

suite("Network Canvas", function () {
  test("package contributes the Network Canvas command", () => {
    const pkg = JSON.parse(
      fs.readFileSync(path.join(__dirname, "../../../package.json"), "utf8")
    );
    const commands: Array<{ command: string }> =
      pkg.contributes?.commands ?? [];
    assert.ok(
      commands.some((c) => c.command === "trust-lsp.networkCanvas.open"),
      "expected trust-lsp.networkCanvas.open command"
    );
  });

  // --- Honesty invariant: status only from real evidence --------------------
  test("stage progression alone never fabricates a running runtime or connected device", () => {
    for (const stage of ["runtime_live", "intent", "add_device", "connected"] as const) {
      const model = buildNetworkCanvasModel(stage);
      assert.notStrictEqual(model.runtime.state, "running", `${stage} faked running`);
      assert.notStrictEqual(model.device?.status, "connected", `${stage} faked connected`);
    }
  });

  test("runtime goes green only from runtime lifecycle evidence", () => {
    assert.notStrictEqual(
      buildNetworkCanvasModel("runtime_live").runtime.state,
      "running",
      "no evidence must never be running"
    );
    assert.strictEqual(
      buildNetworkCanvasModel({ stage: "runtime_live", runtime: RUNNING }).runtime.state,
      "running"
    );
  });

  test("device goes connected only after real I/O values are reported", () => {
    const base: BuildNetworkCanvasModelInput = {
      stage: "connected",
      deviceRequested: true,
      runtime: RUNNING,
    };
    assert.strictEqual(buildNetworkCanvasModel(base).device?.status, "pending");
    const connected = buildNetworkCanvasModel({
      ...base,
      ioState: {
        inputs: [{ address: "%IX0.0", name: "Drive A ready", value: "TRUE" }],
        outputs: [{ address: "%QW0", name: "Command", value: "12" }],
        memory: [],
      },
    });
    assert.strictEqual(connected.device?.status, "connected");
    assert.deepStrictEqual(
      connected.device?.liveValues.map((v) => v.value),
      ["TRUE", "12"]
    );
  });

  test("fleet topology rolls host/runtime health up from raw endpoint evidence", () => {
    const model = buildNetworkCanvasModel({
      stage: "runtime_live",
      runtime: RUNNING,
      topology: fleetTopology(),
    });
    assert.ok(model.fleet, "expected a fleet view");
    assert.strictEqual(model.fleet?.hosts[0]?.health, "degraded");
    assert.strictEqual(model.fleet?.hosts[0]?.runtimes[0]?.health, "degraded");
  });

  test("fleet search never hides degraded endpoints from host or runtime rollups", () => {
    const model = buildNetworkCanvasModel({
      stage: "runtime_live",
      runtime: RUNNING,
      topology: fleetTopology(),
      searchQuery: "modbus", // does NOT match the degraded mqtt endpoint
    });
    assert.strictEqual(model.fleet?.hosts[0]?.health, "degraded");
    assert.strictEqual(model.fleet?.hosts[0]?.runtimes[0]?.health, "degraded");
    const mqtt = model.fleet?.hosts[0]?.runtimes[0]?.endpoints.find(
      (e) => e.protocol === "mqtt"
    );
    assert.strictEqual(mqtt?.health, "degraded", "raw health preserved");
    assert.strictEqual(mqtt?.dimmed, true, "non-match dimmed for display only");
  });

  // --- Graph mapping (the React Flow canvas data) ---------------------------
  test("buildCanvasGraph maps a real fleet to host/runtime/endpoint nodes + links", () => {
    const topology = fleetTopology();
    const model = buildNetworkCanvasModel({
      stage: "runtime_live",
      runtime: RUNNING,
      topology,
    });
    const graph = buildCanvasGraph(model, topology);
    assert.strictEqual(graph.kind, "graph");
    assert.strictEqual(graph.hosts.length, 1);
    const runtime = graph.hosts[0].runtimes[0];
    assert.strictEqual(runtime.id, "runtime-a");
    assert.ok(runtime.endpoints.some((e) => e.id === "endpoint:runtime-a:mqtt"));
    assert.ok(graph.links.some((l) => l.protocol === "mesh"));
    assert.ok(graph.external.some((x) => x.id === "external:mesh:0"));
    assert.ok(
      graph.faults.some((f) => f.targetNodeId === "endpoint:runtime-a:mqtt"),
      "degraded endpoint surfaces as a fault"
    );
  });

  test("buildCanvasGraph never emits an edge to a node that does not exist", () => {
    const topology = fleetTopology();
    const model = buildNetworkCanvasModel({
      stage: "runtime_live",
      runtime: RUNNING,
      topology,
    });
    const graph = buildCanvasGraph(model, topology);
    const ids = new Set<string>();
    for (const host of graph.hosts) {
      ids.add(host.id);
      for (const rt of [...host.runtimes, ...host.containers.flatMap((c) => c.runtimes)]) {
        ids.add(rt.id);
        rt.endpoints.forEach((e) => ids.add(e.id));
      }
    }
    graph.external.forEach((x) => ids.add(x.id));
    for (const link of graph.links) {
      assert.ok(ids.has(link.from) || ids.has(link.to), `link ${link.id} references known nodes`);
    }
  });

  // --- Multi-runtime merge (§10/§12.10) ------------------------------------
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

  test("a new project with no configured runtime shows the local simulator node, not an empty screen", () => {
    const graph = buildCanvasGraph(buildNetworkCanvasModel("runtime_live"), undefined);
    assert.strictEqual(graph.kind, "graph");
    assert.strictEqual(graph.hosts.length, 1);
    const runtime = graph.hosts[0].runtimes[0];
    assert.ok(runtime, "local simulator runtime node is always present");
    assert.notStrictEqual(runtime.health, "connected", "honest: not green until proven");
  });

  test("a running local simulator renders as a connected runtime node", () => {
    const graph = buildCanvasGraph(
      buildNetworkCanvasModel({ stage: "runtime_live", runtime: RUNNING }),
      undefined
    );
    assert.strictEqual(graph.hosts[0].runtimes[0].health, "connected");
  });

  test("a runtime start failure renders an error node + retry banner, not a failure screen", () => {
    const graph = buildCanvasGraph(
      buildNetworkCanvasModel({
        stage: "runtime_live",
        failure: { kind: "port_conflict", message: "The runtime port is already in use." },
      }),
      undefined
    );
    assert.strictEqual(graph.hosts[0].runtimes[0].health, "error");
    assert.ok(graph.banner, "failure surfaces an inline banner");
    assert.ok(graph.banner?.actions.some((a) => a.action === "startLocalSimulator"));
  });

  // --- Failure classification ----------------------------------------------
  test("classifyRuntimeStartFailure maps real error strings to actionable kinds", () => {
    assert.strictEqual(classifyRuntimeStartFailure(new Error("spawn trust-runtime ENOENT")).kind, "missing_binary");
    assert.strictEqual(classifyRuntimeStartFailure("EADDRINUSE: address already in use").kind, "port_conflict");
    assert.strictEqual(classifyRuntimeStartFailure("EACCES: permission denied").kind, "workspace_permission");
    assert.strictEqual(classifyRuntimeStartFailure("debug session timed out").kind, "stale_runtime");
    assert.strictEqual(classifyRuntimeStartFailure("something else broke").kind, "failed_spawn");
  });

  // P2 regression guard (UX overhaul §9): the Network Canvas is the comms front door — it must own
  // communication setup in-canvas and never send the user (by command, panel import, OR copy) back to
  // the old Communication panel. (The shared ../communication/{schemaForm,capability,runtimeComm}
  // modules are fine — they're reused code, not the panel.)
  test("Network Canvas owns comms in-canvas — no Communication-panel command, import, or copy", () => {
    const root = path.resolve(__dirname, "..", "..", "..");
    const panelSource = fs.readFileSync(
      path.join(root, "src", "networkCanvas", "networkCanvasPanel.ts"),
      "utf8"
    );
    for (const forbidden of [
      "communication.openPanel",
      "communicationPanel",
      "Open Communication",
    ]) {
      assert.ok(
        !panelSource.includes(forbidden),
        `Network Canvas must not reference "${forbidden}" — comms setup lives in-canvas, not the old Communication panel.`
      );
    }
  });
});

function fleetTopology(): FleetTopologyResponse {
  return {
    schema_version: 1,
    hosts: [
      {
        host_id: "host:trust-pi",
        hostname: "trust-pi",
        arch: "aarch64",
        os: "linux",
        ips: ["192.168.77.10"],
        containers: [],
        runtimes: [
          {
            runtime_id: "runtime-a",
            name: "Line runtime",
            web_listen: "0.0.0.0:8080",
            mode: "simulate",
            cycle_ms: 10,
            health: "connected",
            detail: "Runtime answered fleet.topology.",
            endpoints: [
              {
                id: "endpoint:runtime-a:modbus_tcp",
                kind: "field",
                protocol: "modbus_tcp",
                name: "Modbus meter",
                role: "owned_driver",
                health: "connected",
                detail: "Driver is healthy.",
                owned: true,
                supports_test: true,
              },
              {
                id: "endpoint:runtime-a:mqtt",
                kind: "field",
                protocol: "mqtt",
                name: "MQTT broker",
                role: "owned_driver",
                health: "degraded",
                detail: "Broker connection refused.",
                owned: true,
                supports_test: true,
              },
            ],
          },
        ],
      },
    ],
    links: [
      {
        from: "endpoint:runtime-a:mqtt",
        to: "external:mesh:0",
        protocol: "mesh",
        direction: "outbound",
        same_host: false,
        status: "degraded",
        secure: true,
        detail: "tcp/192.168.77.11:7447",
      },
    ],
    shared: [],
    external: [
      {
        id: "external:mesh:0",
        kind: "peer",
        name: "tcp/192.168.77.11:7447",
        via_protocol: ["mesh"],
        direction: "outbound",
      },
    ],
  };
}
