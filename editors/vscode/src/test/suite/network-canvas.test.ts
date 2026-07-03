import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
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
import { buildGraph } from "../../networkCanvas/webview/layout";
import type { RuntimeTarget } from "../../runtimeTarget";
import { classifyRuntimeStartFailure } from "../../networkCanvas/runtimeFailures";
import { runtimeNodeControls } from "../../networkCanvas/webview/runtimeNodeControls";
import { ADD_PICKER_GROUPS, groupForAddPicker } from "../../networkCanvas/webview/grouping";
import { applyFilter, filterReport } from "../../networkCanvas/webview/filter";
import { buildExposeApplyParams } from "../../networkCanvas/exposeConfig";
import { commTestMessage } from "../../communication/runtimeComm";
import { protocolColor, protocolName } from "../../networkCanvas/webview/protocolMeta";
import { visibleFaultsForValidationState } from "../../networkCanvas/webview/faults";

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

  test("runtime rolls health up from raw endpoint evidence; host stays reachability", () => {
    const model = buildNetworkCanvasModel({
      stage: "runtime_live",
      runtime: RUNNING,
      topology: fleetTopology(),
    });
    assert.ok(model.fleet, "expected a fleet view");
    // Host status is MACHINE reachability, not a health rollup: a host we reached (arch/os/ips present)
    // is "connected" even with a degraded endpoint inside — the degradation surfaces on the RUNTIME.
    assert.strictEqual(model.fleet?.hosts[0]?.health, "connected");
    assert.strictEqual(model.fleet?.hosts[0]?.runtimes[0]?.health, "degraded");
  });

  test("fleet host headlines are user-facing, not raw lab machine names", () => {
    const local = fleetTopology();
    const localHostname = os.hostname();
    local.hosts[0].hostname = localHostname;
    local.hosts[0].ips = ["127.0.0.1"];
    local.hosts[0].runtimes[0].control_endpoint = "tcp://127.0.0.1:39855";
    const localModel = buildNetworkCanvasModel({
      stage: "runtime_live",
      runtime: RUNNING,
      topology: local,
    });
    assert.strictEqual(localModel.fleet?.hosts[0]?.hostname, "This computer");
    assert.ok(
      localHostname.length === 0 || localModel.fleet?.hosts[0]?.label.includes(localHostname),
      "the raw local hostname stays available as supporting detail"
    );

    const remote = fleetTopology();
    remote.hosts[0].hostname = "scena-rust-builder";
    remote.hosts[0].ips = ["192.168.77.10"];
    remote.hosts[0].runtimes[0].control_endpoint = "tcp://192.168.77.10:5680";
    const remoteModel = buildNetworkCanvasModel({
      stage: "runtime_live",
      runtime: RUNNING,
      topology: remote,
    });
    assert.strictEqual(remoteModel.fleet?.hosts[0]?.hostname, "Computer 192.168.77.10");
    assert.ok(
      remoteModel.fleet?.hosts[0]?.label.includes("scena-rust-builder"),
      "the raw hostname remains available as supporting detail"
    );
  });

  test("unreachable configured peers keep their configured label, not 'This computer'", () => {
    const topology: FleetTopologyResponse = {
      schema_version: 3,
      hosts: [
        {
          host_id: "fleet:tcp://127.0.0.1:5510",
          hostname: "discoveredcell",
          arch: "",
          os: "",
          ips: [],
          containers: [],
          runtimes: [
            {
              runtime_id: "fleet:tcp://127.0.0.1:5510:runtime",
              name: "discoveredcell",
              control_endpoint: "tcp://127.0.0.1:5510",
              mode: "error",
              cycle_ms: 0,
              health: "error",
              detail: "Authentication failed — check the runtime's auth token.",
              endpoints: [],
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
    assert.strictEqual(model.fleet?.hosts[0]?.hostname, "discoveredcell");
  });

  test("fleet search never hides degraded endpoints from the runtime rollup", () => {
    const model = buildNetworkCanvasModel({
      stage: "runtime_live",
      runtime: RUNNING,
      topology: fleetTopology(),
      searchQuery: "modbus", // does NOT match the degraded mqtt endpoint
    });
    assert.strictEqual(model.fleet?.hosts[0]?.health, "connected"); // reachability, unaffected by search
    assert.strictEqual(model.fleet?.hosts[0]?.runtimes[0]?.health, "degraded");
    const mqtt = model.fleet?.hosts[0]?.runtimes[0]?.endpoints.find(
      (e) => e.protocol === "mqtt"
    );
    assert.strictEqual(mqtt?.health, "degraded", "raw health preserved");
    assert.strictEqual(mqtt?.dimmed, true, "non-match dimmed for display only");
  });

  test("fleet search dims external counterparts and wires without hiding warnings", () => {
    const topology = fleetTopology();
    const model = buildNetworkCanvasModel({
      stage: "runtime_live",
      runtime: RUNNING,
      topology,
      searchQuery: "modbus",
    });
    const graph = buildCanvasGraph(model, topology);
    const rendered = buildGraph(graph, undefined, false);
    const mqttEndpoint = rendered.nodes.find((node) => node.id === "endpoint:runtime-a:mqtt");
    const mqttBroker = rendered.nodes.find((node) => node.id === "shared:mqtt:broker");
    const mqttLink = rendered.edges.find((edge) => edge.id === "link:mqtt:broker");
    const warning = graph.faults.find((fault) => /mqtt/i.test(fault.label));

    assert.strictEqual(mqttEndpoint?.data.dimmed, true, "nonmatching endpoint is dimmed");
    assert.strictEqual(mqttBroker?.data.dimmed, true, "counterpart external node is dimmed too");
    assert.strictEqual(mqttLink?.data?.dimmed, true, "counterpart wire is dimmed too");
    assert.ok(warning, "search preserves the degraded endpoint warning");
  });

  test("editing a rejected add form hides stale apply faults without hiding real faults", () => {
    const faults = [
      {
        id: "apply:modbus_tcp",
        label: "Configuration was not applied. Fix the highlighted fields and try again.",
        targetNodeId: "draft:modbus_tcp",
        severity: "error" as const,
      },
      {
        id: "device:modbus",
        label: "Modbus TCP: unreachable",
        targetNodeId: "endpoint:modbus",
        severity: "warning" as const,
      },
    ];

    assert.deepStrictEqual(visibleFaultsForValidationState(faults, false), faults);
    assert.deepStrictEqual(visibleFaultsForValidationState(faults, true), [faults[1]]);
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
      graph.external.some((x) => x.id === "shared:mqtt:broker"),
      "shared MQTT broker nodes must render so publish/subscribe links are not dangling"
    );
    assert.ok(graph.links.some((l) => l.to === "shared:mqtt:broker"));
    assert.ok(
      graph.faults.some((f) => f.targetNodeId === "endpoint:runtime-a:mqtt"),
      "degraded endpoint surfaces as a fault"
    );
  });

  test("buildCanvasGraph does not duplicate a runtime as an external system", () => {
    const topology = fleetTopology();
    topology.external.push({
      id: "external:self",
      kind: "runtime",
      name: "Line runtime",
      via_protocol: ["discovery"],
      direction: "outbound",
    });
    const model = buildNetworkCanvasModel({
      stage: "runtime_live",
      runtime: RUNNING,
      topology,
    });
    const graph = buildCanvasGraph(model, topology);
    assert.ok(
      graph.hosts[0].runtimes.some((runtime) => runtime.name === "Line runtime"),
      "the runtime itself stays visible"
    );
    assert.ok(
      !graph.external.some((external) => external.name === "Line runtime"),
      "the same runtime must not also appear as an external system"
    );
    assert.ok(
      graph.external.some((external) => external.id === "external:mesh:0"),
      "unrelated external systems stay visible"
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

  test("draft/pending wire links render dashed, connected links stay solid (honest: not yet a live link)", () => {
    const graph = buildGraph({
      kind: "graph",
      title: "Devices & Connections",
      summary: "",
      hosts: [
        {
          id: "host:local",
          hostname: "this computer",
          label: "local host",
          health: "connected",
          containers: [],
          runtimes: [
            {
              id: "runtime:local",
              name: "truST runtime",
              mode: "simulate",
              health: "stopped",
              detail: "",
              endpoints: [
                {
                  id: "endpoint:openot",
                  kind: "field",
                  protocol: "openot",
                  name: "OpenOT",
                  role: "client",
                  health: "pending",
                  detail: "",
                },
                {
                  id: "endpoint:ads",
                  kind: "field",
                  protocol: "ads",
                  name: "ADS",
                  role: "client",
                  health: "connected",
                  detail: "",
                },
              ],
            },
          ],
        },
      ],
      links: [
        {
          id: "link:openot",
          from: "endpoint:openot",
          to: "external:openot",
          protocol: "openot",
          role: "client",
          status: "pending",
          secure: false,
        },
        {
          id: "link:ads",
          from: "endpoint:ads",
          to: "external:ads",
          protocol: "ads",
          role: "client",
          status: "connected",
          secure: false,
        },
      ],
      external: [
        { id: "external:openot", name: "openot peer", kind: "server" },
        { id: "external:ads", name: "ads server", kind: "server" },
      ],
      faults: [],
    });
    const draftEdge = graph.edges.find((edge) => edge.id === "link:openot");
    const liveEdge = graph.edges.find((edge) => edge.id === "link:ads");
    assert.ok(draftEdge, "expected the pending openot wire");
    assert.ok(liveEdge, "expected the connected ads wire");
    assert.strictEqual(draftEdge?.data?.dashed, true, "a pending link must render dashed, not as a live connection");
    assert.ok(!liveEdge?.data?.dashed, "a connected link must render solid");
  });

  test("draft/pending mesh peers drop a dashed bus wire; connected peers stay solid", () => {
    const graph = buildGraph({
      kind: "graph",
      title: "Devices & Connections",
      summary: "",
      hosts: [
        {
          id: "host:local",
          hostname: "this computer",
          label: "local host",
          health: "connected",
          containers: [],
          runtimes: [
            {
              id: "runtime:local",
              name: "truST runtime",
              mode: "simulate",
              health: "stopped",
              detail: "",
              endpoints: [
                {
                  id: "endpoint:mesh-draft",
                  kind: "field",
                  protocol: "mesh",
                  name: "Mesh (draft)",
                  role: "peer",
                  health: "pending",
                  detail: "",
                },
                {
                  id: "endpoint:mesh-live",
                  kind: "field",
                  protocol: "mesh",
                  name: "Mesh (live)",
                  role: "peer",
                  health: "connected",
                  detail: "",
                },
              ],
            },
          ],
        },
      ],
      links: [],
      external: [],
      faults: [],
    });
    const draftMesh = graph.edges.find((edge) => edge.id === "mesh-endpoint:mesh-draft");
    const liveMesh = graph.edges.find((edge) => edge.id === "mesh-endpoint:mesh-live");
    const meshBus = graph.nodes.find((node) => node.id === "bus:mesh");
    assert.ok(draftMesh, "expected a bus wire for the draft mesh peer");
    assert.ok(liveMesh, "expected a bus wire for the live mesh peer");
    assert.strictEqual(draftMesh?.data?.dashed, true, "a pending mesh peer must render dashed");
    assert.strictEqual(liveMesh?.data?.dashed, true, "a mixed draft/live fabric stays dashed until all peers are live");
    assert.strictEqual(meshBus?.data?.draft, true, "mesh fabric must visibly carry the DRAFT state");
    assert.strictEqual(meshBus?.data?.showLabel, true, "a multi-peer mesh fabric needs its shared-bus label");
    assert.notStrictEqual(meshBus?.data?.color, "rgb(137,209,133)", "draft fabric must not use the live green");
    assert.strictEqual(draftMesh?.data?.color, meshBus?.data?.color, "draft mesh wires must use the same muted draft role as the bus");
    assert.strictEqual(liveMesh?.data?.color, meshBus?.data?.color, "mixed mesh fabric wires stay muted while any peer is draft");
  });

  test("single-peer mesh fabric suppresses redundant bus label", () => {
    const graph = buildGraph({
      kind: "graph",
      title: "Devices & Connections",
      summary: "",
      hosts: [
        {
          id: "host:local",
          hostname: "this computer",
          label: "local host",
          health: "connected",
          containers: [],
          runtimes: [
            {
              id: "runtime:local",
              name: "truST runtime",
              mode: "simulate",
              health: "stopped",
              detail: "",
              endpoints: [
                {
                  id: "endpoint:mesh-draft",
                  kind: "field",
                  protocol: "mesh",
                  name: "Mesh (draft)",
                  role: "peer",
                  health: "pending",
                  detail: "",
                },
              ],
            },
          ],
        },
      ],
      links: [],
      external: [],
      faults: [],
    });
    const meshBus = graph.nodes.find((node) => node.id === "bus:mesh");
    const meshBusData = meshBus?.data as { handles?: Array<{ x: number }> } | undefined;
    const meshBusWidth = Number(meshBus?.style?.width ?? 0);
    assert.ok(meshBus, "expected a mesh fabric bus node");
    assert.strictEqual(meshBus?.data?.showLabel, false, "a one-peer mesh fabric should not add a redundant floating label");
    assert.strictEqual(
      meshBusData?.handles?.[0]?.x,
      meshBusWidth / 2,
      "a one-peer bus keeps the endpoint handle centered"
    );
  });

  test("external protocol nodes render display names, not raw driver ids", () => {
    const graph = buildGraph({
      kind: "graph",
      title: "Devices & Connections",
      summary: "",
      hosts: [
        {
          id: "host:local",
          hostname: "this computer",
          label: "local host",
          health: "connected",
          containers: [],
          runtimes: [
            {
              id: "runtime:local",
              name: "truST runtime",
              mode: "simulate",
              health: "stopped",
              detail: "",
              endpoints: [
                {
                  id: "endpoint:modbus",
                  kind: "field",
                  protocol: "modbus_tcp",
                  name: "Modbus",
                  role: "client",
                  health: "pending",
                  detail: "",
                },
              ],
            },
          ],
        },
      ],
      links: [
        {
          id: "link:modbus",
          from: "endpoint:modbus",
          to: "external:modbus",
          protocol: "modbus_tcp",
          role: "client",
          status: "configured_policy",
          secure: false,
        },
      ],
      external: [{ id: "external:modbus", name: "modbus_tcp 127.0.0.1:502", kind: "server" }],
      faults: [],
    });
    const external = graph.nodes.find((node) => node.id === "external:modbus");
    assert.ok(external, "expected external Modbus node");
    assert.strictEqual(external?.data.label, "Modbus TCP 127.0.0.1:502");
    assert.strictEqual(external?.data.sub, "Modbus TCP server");
  });

  test("OPC UA client links label the external counterpart as an OPC UA server", () => {
    const graph = buildGraph({
      kind: "graph",
      title: "Devices & Connections",
      summary: "1 host · 1 runtime · 1 endpoint",
      hosts: [
        {
          id: "host:local",
          hostname: "This computer",
          label: "local",
          health: "connected",
          containers: [],
          runtimes: [
            {
              id: "runtime:local",
              name: "truST runtime",
              mode: "simulate",
              health: "stopped",
              detail: "",
              endpoints: [
                {
                  id: "endpoint:opcua-client",
                  kind: "field",
                  protocol: "opcua_client",
                  name: "OPC UA client",
                  role: "client",
                  health: "pending",
                  detail: "",
                },
              ],
            },
          ],
        },
      ],
      links: [
        {
          id: "link:opcua-client",
          from: "endpoint:opcua-client",
          to: "external:opcua:server",
          protocol: "opcua_client",
          role: "client",
          status: "configured_policy",
          secure: false,
        },
      ],
      external: [{ id: "external:opcua:server", name: "OPC UA server line-a", kind: "server" }],
      faults: [],
    });
    const external = graph.nodes.find((node) => node.id === "external:opcua:server");
    assert.ok(external, "expected external OPC UA server node");
    assert.strictEqual(external?.data.label, "OPC UA server line-a");
    assert.strictEqual(external?.data.sub, "OPC UA server");
    assert.notStrictEqual(external?.data.sub, "OPC UA client server");
  });

  test("OPC UA server links label the external counterpart as an OPC UA client", () => {
    const graph = buildGraph({
      kind: "graph",
      title: "Devices & Connections",
      summary: "1 host · 1 runtime · 1 endpoint",
      hosts: [
        {
          id: "host:local",
          hostname: "This computer",
          label: "local",
          health: "connected",
          containers: [],
          runtimes: [
            {
              id: "runtime:local",
              name: "truST runtime",
              mode: "simulate",
              health: "connected",
              detail: "",
              endpoints: [
                {
                  id: "endpoint:opcua-server",
                  kind: "service",
                  protocol: "opcua",
                  name: "OPC UA server",
                  role: "server",
                  health: "connected",
                  detail: "",
                },
              ],
            },
          ],
        },
      ],
      links: [
        {
          id: "link:opcua-server",
          from: "endpoint:opcua-server",
          to: "external:opcua:client",
          protocol: "opcua",
          role: "server",
          status: "connected",
          secure: false,
        },
      ],
      external: [{ id: "external:opcua:client", name: "External client", kind: "client" }],
      faults: [],
    });
    const external = graph.nodes.find((node) => node.id === "external:opcua:client");
    assert.ok(external, "expected external OPC UA client node");
    assert.strictEqual(external?.data.sub, "OPC UA client");
  });

  test("ADS client links label the external counterpart as an ADS server", () => {
    const graph = buildGraph({
      kind: "graph",
      title: "Devices & Connections",
      summary: "1 host · 1 runtime · 1 endpoint",
      hosts: [
        {
          id: "host:local",
          hostname: "This computer",
          label: "local",
          health: "connected",
          containers: [],
          runtimes: [
            {
              id: "runtime:local",
              name: "truST runtime",
              mode: "simulate",
              health: "stopped",
              detail: "",
              endpoints: [
                {
                  id: "endpoint:ads-client",
                  kind: "field",
                  protocol: "ads",
                  name: "ADS client",
                  role: "client",
                  health: "pending",
                  detail: "",
                },
              ],
            },
          ],
        },
      ],
      links: [
        {
          id: "link:ads-client",
          from: "endpoint:ads-client",
          to: "external:ads:server",
          protocol: "ads",
          role: "client",
          status: "configured_policy",
          secure: false,
        },
      ],
      external: [{ id: "external:ads:server", name: "TwinCAT 5.23.91.12.1.1", kind: "server" }],
      faults: [],
    });
    const external = graph.nodes.find((node) => node.id === "external:ads:server");
    assert.ok(external, "expected external ADS server node");
    assert.strictEqual(external?.data.sub, "ADS server");
    assert.notStrictEqual(external?.data.sub, "ADS client server");
  });

  test("protocol filtering reports hidden degraded/faulted endpoints instead of silently losing them", () => {
    const topology = fleetTopology();
    const graph = buildCanvasGraph(
      buildNetworkCanvasModel({
        stage: "runtime_live",
        runtime: RUNNING,
        topology,
      }),
      topology
    );
    const hidden = new Set(["mqtt"]);
    const filtered = applyFilter(graph, hidden);
    const report = filterReport(graph, hidden);

    const remainingEndpoints = filtered.hosts.flatMap((host) =>
      host.runtimes.flatMap((runtime) => runtime.endpoints)
    );
    assert.ok(
      !remainingEndpoints.some((endpoint) => endpoint.protocol === "mqtt"),
      "the protocol filter hides MQTT endpoints"
    );
    assert.strictEqual(report.hiddenEndpointCount, 1);
    assert.strictEqual(report.hiddenAttentionCount, 1);
    assert.strictEqual(report.hiddenFaultCount, 1);
    assert.strictEqual(report.hiddenWarningCount, 1);
    assert.strictEqual(report.hiddenErrorCount, 0);
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

  test("managed local runtimes are injected under the existing This computer host", () => {
    const graph = buildCanvasGraph(
      buildNetworkCanvasModel("runtime_live"),
      undefined,
      undefined,
      undefined,
      [
        { name: "cell1", controlEndpoint: "tcp://127.0.0.1:9902", state: "stopped" },
        { name: "cell2", controlEndpoint: "tcp://127.0.0.1:9903", state: "running" },
      ]
    );
    assert.strictEqual(
      graph.hosts.filter((host) => host.hostname === "This computer").length,
      1,
      "local simulator and managed local runtimes share one This computer host"
    );
    assert.strictEqual(graph.hosts.length, 1, "managed injection must not draw a duplicate host");
    assert.strictEqual(graph.summary, "1 host · 3 runtimes · 0 endpoints");
    const managedHost = graph.hosts[0];
    const cell1 = managedHost?.runtimes.find((r) => r.managedName === "cell1");
    const cell2 = managedHost?.runtimes.find((r) => r.managedName === "cell2");
    assert.ok(cell1?.managed === true && cell2?.managed === true, "nodes are marked managed");
    assert.strictEqual(cell1?.health, "stopped", "stopped managed runtime is honest grey");
    assert.strictEqual(cell2?.health, "connected", "running managed runtime is green");
  });

  test("a managed runtime already shown via fleet.topology is NOT doubled, and the surviving node stays OWNED (managed)", () => {
    const peerTopology = {
      schema_version: 3 as const,
      hosts: [
        {
          host_id: "fleet:peer",
          hostname: "peer",
          arch: "",
          os: "",
          ips: [],
          containers: [],
          runtimes: [
            {
              runtime_id: "fleet:peer:rt",
              name: "cell1",
              control_endpoint: "tcp://127.0.0.1:9902",
              mode: "online",
              cycle_ms: 0,
              health: "connected",
              detail: "",
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
      buildNetworkCanvasModel("runtime_live"),
      undefined,
      peerTopology,
      undefined,
      [{ name: "cell1", controlEndpoint: "tcp://127.0.0.1:9902", state: "running" }]
    );
    // The endpoint is already shown via the peer topology → no separate managed host injected …
    assert.ok(
      !graph.hosts.some((h) => h.id === "host:managed-local"),
      "no duplicate managed node for an endpoint already on the canvas"
    );
    // … and the EXISTING node is annotated managed, so it keeps owned Start/Stop/Logs (not a remote).
    const surviving = graph.hosts
      .flatMap((h) => h.runtimes)
      .find((r) => r.controlEndpoint === "tcp://127.0.0.1:9902");
    assert.ok(surviving, "the topology node survives");
    assert.strictEqual(surviving?.managed, true, "surviving node is marked managed (owned lifecycle)");
    assert.strictEqual(surviving?.managedName, "cell1", "carries the managed runtime name");
    // It must therefore render managed Start/Stop (running → Stop), not remote Connect/Disconnect.
    const controls = runtimeNodeControls({
      isLocal: false,
      managed: surviving?.managed === true,
      health: String(surviving?.health ?? ""),
      attached: false,
      controlEndpoint: surviving?.controlEndpoint,
      logsAvailable: true,
    });
    assert.strictEqual(controls[0].action, "managedStop");
    assert.ok(controls.some((c) => c.action === "openRuntimeLogs"), "owned node offers Logs");
  });

  test("live managed topology preserves the stopped project runtime instead of morphing the canvas", () => {
    const liveTopology: FleetTopologyResponse = {
      schema_version: 3,
      hosts: [
        {
          host_id: "host:this-computer",
          hostname: "This computer",
          arch: "aarch64",
          os: "linux",
          ips: ["127.0.0.1"],
          containers: [],
          runtimes: [
            {
              runtime_id: "runtime:cell1",
              name: "cell1",
              control_endpoint: "tcp://127.0.0.1:9902",
              mode: "online",
              cycle_ms: 10,
              health: "connected",
              detail: "Running (managed local runtime).",
              endpoints: [
                {
                  id: "endpoint:cell1:simulated",
                  kind: "field",
                  protocol: "simulated",
                  name: "Simulated",
                  role: "owned_driver",
                  health: "connected",
                  detail: "Driver is healthy.",
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
      schema_version: 3,
      hosts: [
        {
          host_id: "host:this-computer",
          hostname: "This computer",
          arch: "aarch64",
          os: "linux",
          ips: ["127.0.0.1"],
          containers: [],
          runtimes: [
            {
              runtime_id: "runtime:project",
              name: "truST runtime",
              mode: "simulate",
              cycle_ms: 10,
              health: "configured_policy",
              detail: "Stopped - configured in this project.",
              endpoints: [
                {
                  id: "endpoint:project:simulated",
                  kind: "field",
                  protocol: "simulated",
                  name: "Simulated",
                  role: "owned_driver",
                  health: "configured_policy",
                  detail: "Configured in io.toml.",
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

    const merged = mergeFleetTopologies([liveTopology, offlineProjectTopology]);
    const graph = buildCanvasGraph(
      buildNetworkCanvasModel({
        stage: "runtime_live",
        runtime: RUNNING,
        topology: merged,
      }),
      merged,
      undefined,
      undefined,
      [{ name: "cell1", controlEndpoint: "tcp://127.0.0.1:9902", state: "running" }]
    );

    assert.strictEqual(graph.hosts.length, 1, "same computer stays one host");
    assert.strictEqual(graph.summary, "1 host · 2 runtimes · 2 endpoints");
    const runtimes = graph.hosts[0].runtimes;
    assert.deepStrictEqual(
      runtimes.map((runtime) => runtime.name).sort(),
      ["cell1", "truST runtime"],
      "managed Start must not delete the project runtime the user just saw"
    );
    const cell1 = runtimes.find((runtime) => runtime.name === "cell1");
    const projectRuntime = runtimes.find((runtime) => runtime.name === "truST runtime");
    assert.strictEqual(cell1?.managed, true, "managed runtime keeps owned lifecycle controls");
    assert.strictEqual(cell1?.health, "connected", "started managed runtime is honestly running");
    assert.strictEqual(
      projectRuntime?.health,
      "stopped",
      "project runtime remains visible with an honest stopped state"
    );
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

  test("OPC UA client test failures render user-facing recovery text, not raw backend tokens", () => {
    const message = commTestMessage("opcua_client", {
      protocol: "opcua_client",
      supported: true,
      ok: false,
      detail: "OPC UA endpoint handshake failed: OPC UA status: BadNotConnected",
      error: {
        code: "endpoint_unreachable",
        message: "OPC UA endpoint handshake failed: OPC UA status: BadNotConnected",
      },
    });
    assert.ok(message.includes("OPC UA server is not reachable"));
    assert.ok(!message.includes("BadNotConnected"), "raw OPC UA status must not leak into the user-facing result");
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
        id: "link:mqtt:broker",
        from: "endpoint:runtime-a:mqtt",
        to: "shared:mqtt:broker",
        protocol: "mqtt",
        role: "publish_subscribe",
        direction: "publish_subscribe",
        same_host: false,
        status: "configured_policy",
        secure: false,
        detail: "MQTT broker referenced by io.toml",
      },
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
    shared: [
      {
        id: "shared:mqtt:broker",
        kind: "broker",
        name: "MQTT broker",
        address: "127.0.0.1:1883",
        used_by: ["runtime-a"],
      },
    ],
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

// F-043/S-09: the Add picker presents backend protocols as first-user choices, not raw
// Field/Supervisory/Peer schema categories.
suite("Network Canvas — add picker taxonomy", function () {
  type P = { id: string; title: string; purpose?: string; category?: string };
  const p = (id: string, category?: string): P => ({ id, title: id, purpose: `${id} purpose`, category });

  test("groups protocols by user intent in the S-09 order", () => {
    const protos: P[] = [
      p("opcua", "supervisory_service"),
      p("modbus_tcp", "field_device"),
      p("mqtt", "field_device"),
      p("mesh", "peer_link"),
      p("gpio", "field_device"),
      p("opcua_client", "peer_link"),
      p("ads_server", "supervisory_service"),
      p("ads", "peer_link"),
    ];
    const groups = groupForAddPicker(protos);
    assert.deepStrictEqual(groups.map((g) => g.key), [
      "devices_io",
      "read_tags",
      "share_values",
      "messages",
      "advanced",
    ]);
    assert.deepStrictEqual(groups.map((g) => g.label), [
      "Devices and I/O",
      "Read tags from another PLC or server",
      "Share truST values",
      "Send and receive messages",
      "Advanced integrations",
    ]);
    assert.deepStrictEqual(groups[0].items.map((item) => item.protocol.id), ["modbus_tcp", "gpio"]);
  });

  test("omits empty groups and keeps advanced choices separate", () => {
    const groups = groupForAddPicker<P>([p("modbus_tcp"), p("mesh")]);
    assert.deepStrictEqual(groups.map((g) => g.key), ["devices_io", "advanced"]);
    assert.strictEqual(groups[1].advanced, true);
  });

  test("does not render runtime discovery as a protocol card", () => {
    const groups = groupForAddPicker<P>([p("discovery"), p("modbus_tcp")]);
    assert.deepStrictEqual(groups.map((g) => g.key), ["devices_io"]);
    assert.deepStrictEqual(groups.flatMap((g) => g.items.map((item) => item.protocol.id)), ["modbus_tcp"]);
  });

  test("routes unknown protocols to a trailing advanced Other choices group and never drops anything", () => {
    const groups = groupForAddPicker<P>([p("modbus_tcp"), p("mystery"), p("blank")]);
    assert.deepStrictEqual(groups.map((g) => g.key), ["devices_io", "other"]);
    assert.strictEqual(groups[1].advanced, true);
    assert.deepStrictEqual(groups[1].items.map((item) => item.protocol.id), ["mystery", "blank"]);
    assert.strictEqual(groups.reduce((n, g) => n + g.items.length, 0), 3);
  });

  test("server and client pairs have distinct badges and direction copy", () => {
    const groups = groupForAddPicker<P>([p("opcua"), p("opcua_client"), p("ads_server"), p("ads")]);
    const items = new Map(groups.flatMap((g) => g.items.map((item) => [item.protocol.id, item])));
    assert.strictEqual(items.get("opcua")?.badge, "UA OUT");
    assert.strictEqual(items.get("opcua_client")?.badge, "UA IN");
    assert.strictEqual(items.get("ads_server")?.badge, "ADS OUT");
    assert.strictEqual(items.get("ads")?.badge, "ADS IN");
    assert.ok(items.get("opcua_client")?.purpose.includes("Read selected tags"));
    assert.ok(items.get("opcua")?.purpose.includes("Share truST values"));
  });

  test("advanced picker copy is user-facing and not backend review prose", () => {
    const groups = groupForAddPicker<P>([
      p("mesh"),
      p("openot"),
      p("realtime_t0"),
      p("runtime_cloud"),
    ]);
    const items = new Map(groups.flatMap((g) => g.items.map((item) => [item.protocol.id, item])));
    assert.strictEqual(items.get("mesh")?.title, "Mesh / Zenoh");
    assert.strictEqual(items.get("mesh")?.badge, "MESH");
    assert.strictEqual(items.get("openot")?.badge, "OT");
    assert.strictEqual(items.get("realtime_t0")?.badge, "RT");
    assert.strictEqual(items.get("runtime_cloud")?.badge, "CLOUD");
    assert.ok(items.get("mesh")?.purpose.includes("peer network"));
    assert.ok(items.get("openot")?.purpose.includes("OpenOT evidence"));
    assert.ok(items.get("realtime_t0")?.purpose.includes("deterministic"));
    assert.ok(items.get("runtime_cloud")?.purpose.includes("federation"));
    assert.ok(!items.get("runtime_cloud")?.purpose.includes("pretending"));
  });

  test("canvas protocol names spell ADS direction instead of relying on the role band", () => {
    assert.strictEqual(protocolName("ads"), "ADS client");
    assert.strictEqual(protocolName("ads_server"), "ADS server");
    assert.strictEqual(
      protocolColor("ads_server"),
      protocolColor("opcua"),
      "equivalent benign server endpoints share a non-alarm protocol accent"
    );
  });

  test("local I/O endpoint titles leave the I/O role to the node band", () => {
    assert.strictEqual(protocolName("simulated"), "Simulated");
    assert.strictEqual(protocolName("loopback"), "Loopback");
  });

  test("ADD_PICKER_GROUPS is the canonical S-09 group order", () => {
    assert.deepStrictEqual(ADD_PICKER_GROUPS.map((c) => c.key), [
      "devices_io",
      "read_tags",
      "share_values",
      "messages",
      "advanced",
    ]);
  });
});

suite("Network Canvas — expose globals apply params", function () {
  test("drops topology-only evidence fields before re-applying ADS/OPC UA server config", () => {
    const { names, params } = buildExposeApplyParams(
      {
        enabled: true,
        listen: "0.0.0.0",
        ams_net_id: "127.0.0.1.1.1",
        port: 48898,
        expose: ["TankLevel"],
        writable: [],
        clients: [],
        clients_count: 0,
        username_set: true,
      },
      ["global.Setpoint"],
      true
    );

    assert.deepStrictEqual(names, ["Setpoint"]);
    assert.deepStrictEqual(params.expose, ["TankLevel", "Setpoint"]);
    assert.deepStrictEqual(params.writable, ["Setpoint"]);
    assert.strictEqual(params.clients_count, undefined);
    assert.strictEqual(params.username_set, undefined);
  });
});
