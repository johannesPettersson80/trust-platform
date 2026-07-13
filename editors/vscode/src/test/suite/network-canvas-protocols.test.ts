import {
  assert,
  os,
  buildNetworkCanvasModel,
  buildCanvasGraph,
  buildGraph,
  applyFilter,
  filterReport,
  formatExposedGlobals,
  serverEndpointSummaryRows,
  RUNNING,
  fleetTopology,
} from "./network-canvas-fixtures";
import type {
  FleetTopologyResponse,
} from "./network-canvas-fixtures";

suite("Network Canvas", function () {


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
  test("server endpoint summaries answer where the server is and what it exposes", () => {
    assert.strictEqual(
      formatExposedGlobals(["global.TankLevel", "global.PumpRunning"]),
      "2 globals: global.TankLevel, global.PumpRunning"
    );
    assert.deepStrictEqual(
      serverEndpointSummaryRows("opcua", {
        listen: "127.0.0.1:4840",
        endpoint_path: "/trust",
      }),
      [{ label: "Server endpoint", value: "opc.tcp://127.0.0.1:4840/trust" }]
    );
    assert.deepStrictEqual(
      serverEndpointSummaryRows(
        "ads_server",
        {
          listen: "127.0.0.1:48898",
          ams_net_id: "127.0.0.1.1.1",
          ads_port: 851,
        },
        { value: { connected_clients: 2 }, last_seen_ms: 12 }
      ),
      [
        {
          label: "Server endpoint",
          value: "127.0.0.1:48898 · AMS Net ID 127.0.0.1.1.1 · ADS port 851",
        },
        { label: "Connected clients", value: "2 clients connected" },
        { label: "Verification", value: "Self-test available; no external client verified" },
      ]
    );
    assert.deepStrictEqual(
      serverEndpointSummaryRows(
        "ads_server",
        {
          listen: "127.0.0.1:48898",
          ams_net_id: "127.0.0.1.1.1",
          ads_port: 851,
        },
        {
          value: {
            connected_clients: 0,
            proof_status: "external_client_verified",
            external_client_verified: true,
            external_client_kind: "loopback-ads-client",
            external_client_name: "trust-runtime-doctor",
          },
          last_seen_ms: 12,
        }
      ),
      [
        {
          label: "Server endpoint",
          value: "127.0.0.1:48898 · AMS Net ID 127.0.0.1.1.1 · ADS port 851",
        },
        { label: "Connected clients", value: "0 clients connected" },
        {
          label: "Verification",
          value: "Verified by loopback-ads-client trust-runtime-doctor",
        },
      ]
    );
  });
  test("ADS server live client count survives topology to endpoint node data", () => {
    const topology: FleetTopologyResponse = {
      schema_version: 3,
      hosts: [
        {
          host_id: "host:local",
          hostname: "This computer",
          arch: "aarch64",
          os: "linux",
          ips: ["127.0.0.1"],
          containers: [],
          runtimes: [
            {
              runtime_id: "runtime:local",
              name: "truST runtime",
              control_endpoint: "tcp://127.0.0.1:9000",
              mode: "simulate",
              cycle_ms: 10,
              health: "connected",
              detail: "Running.",
              endpoints: [
                {
                  id: "endpoint:ads-server",
                  kind: "service",
                  protocol: "ads_server",
                  name: "ADS server",
                  role: "server",
                  health: "connected",
                  detail: "ADS server runtime is active and listening.",
                  live: {
                    value: {
                      connected_clients: 3,
                      proof_status: "external_client_verified",
                      external_client_verified: true,
                      external_client_kind: "loopback-ads-client",
                      external_client_name: "trust-runtime-doctor",
                    },
                    last_seen_ms: 99,
                  },
                  params: {
                    listen: "127.0.0.1:48898",
                    ams_net_id: "127.0.0.1.1.1",
                    ads_port: 851,
                    expose: ["global.TankLevel"],
                  },
                  owned: true,
                  supports_test: false,
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

    const model = buildNetworkCanvasModel({ topology });
    const canvas = buildCanvasGraph(model, topology);
    const endpoint = canvas.hosts[0]?.runtimes[0]?.endpoints[0];
    assert.deepStrictEqual(endpoint?.live?.value, {
      connected_clients: 3,
      proof_status: "external_client_verified",
      external_client_verified: true,
      external_client_kind: "loopback-ads-client",
      external_client_name: "trust-runtime-doctor",
    });

    const graph = buildGraph(canvas);
    const node = graph.nodes.find((item) => item.id === "endpoint:ads-server");
    assert.deepStrictEqual(node?.data.live, {
      value: {
        connected_clients: 3,
        proof_status: "external_client_verified",
        external_client_verified: true,
        external_client_kind: "loopback-ads-client",
        external_client_name: "trust-runtime-doctor",
      },
      last_seen_ms: 99,
    });
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
      external: [{ id: "external:ads:server", name: "ADS device 5.23.91.12.1.1", kind: "server" }],
      faults: [],
    });
    const external = graph.nodes.find((node) => node.id === "external:ads:server");
    assert.ok(external, "expected external ADS server node");
    assert.strictEqual(external?.data.label, "ADS device 5.23.91.12.1.1");
    assert.strictEqual(external?.data.sub, "ADS server");
    assert.notStrictEqual(external?.data.sub, "ADS client server");
    assert.ok(!String(external?.data.label).includes("TwinCAT"));
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
    assert.strictEqual(
      filtered.summary,
      "1 host · 1 runtime · 1 endpoint",
      "footer summary follows visible endpoint nodes after filtering"
    );
  });

  // --- Multi-runtime merge (§10/§12.10) ------------------------------------
});
