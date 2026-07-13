import {
  assert,
  fs,
  os,
  path,
  buildNetworkCanvasModel,
  ensureAdsRuntimeEnabled,
  mergeConnectorStatusIntoTopology,
  buildCanvasGraph,
  buildGraph,
  connectorConnectionLabel,
  connectorHealthLabel,
  connectorSignalsSummary,
  discoveryConfidenceLabel,
  discoverySourceLabel,
  visibleFaultsForValidationState,
  RUNNING,
  fleetTopology,
} from "./network-canvas-fixtures";
import type {
  BuildNetworkCanvasModelInput,
  FleetTopologyResponse,
  EndpointNodeData,
} from "./network-canvas-fixtures";

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
    remote.hosts[0].hostname = "remote-plc-host";
    remote.hosts[0].ips = ["192.0.2.10"];
    remote.hosts[0].runtimes[0].control_endpoint = "tcp://192.0.2.10:5680";
    const remoteModel = buildNetworkCanvasModel({
      stage: "runtime_live",
      runtime: RUNNING,
      topology: remote,
    });
    assert.strictEqual(remoteModel.fleet?.hosts[0]?.hostname, "Computer 192.0.2.10");
    assert.ok(
      remoteModel.fleet?.hosts[0]?.label.includes("remote-plc-host"),
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
  test("connector status surface flows into endpoint graph metadata", () => {
    const topology = mergeConnectorStatusIntoTopology(fleetTopology(), {
      schema_version: 1,
      connectors: [
        {
          connector_id: "io:modbus-tcp",
          protocol: "modbus_tcp",
          kind: "process_image",
          state: "ready",
          health: "ok",
          confidence: "confirmed",
          point_counts: { total: 4, good: 4, degraded: 0, unavailable: 0 },
        },
        {
          connector_id: "io:mqtt",
          protocol: "mqtt",
          kind: "process_image",
          state: "stale",
          health: "degraded",
          confidence: "port_reachable",
          point_counts: { total: 3, good: 1, degraded: 1, unavailable: 1 },
        },
      ],
    });
    assert.ok(topology, "topology should remain present after connector merge");
    const model = buildNetworkCanvasModel({
      stage: "runtime_live",
      runtime: RUNNING,
      topology,
    });
    const graph = buildCanvasGraph(model, topology);
    const rendered = buildGraph(graph, undefined, false);
    const modbus = rendered.nodes.find((node) => node.id === "endpoint:runtime-a:modbus_tcp");
    const mqtt = rendered.nodes.find((node) => node.id === "endpoint:runtime-a:mqtt");
    const modbusData = modbus?.data as EndpointNodeData | undefined;
    const mqttData = mqtt?.data as EndpointNodeData | undefined;
    assert.strictEqual(modbusData?.connector?.state, "ready");
    assert.strictEqual(modbusData?.connector?.health, "ok");
    assert.strictEqual(modbusData?.connector?.confidence, "confirmed");
    assert.deepStrictEqual(modbusData?.connector?.point_counts, {
      total: 4,
      good: 4,
      degraded: 0,
      unavailable: 0,
    });
    assert.strictEqual(mqttData?.connector?.state, "stale");
    assert.strictEqual(mqttData?.connector?.health, "degraded");
    assert.strictEqual(mqttData?.connector?.confidence, "port_reachable");
    assert.deepStrictEqual(mqttData?.connector?.point_counts, {
      total: 3,
      good: 1,
      degraded: 1,
      unavailable: 1,
    });
  });
  test("ADS tag import enables the runtime ADS subsystem", () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), "trust-ads-runtime-"));
    try {
      fs.writeFileSync(
        path.join(dir, "runtime.toml"),
        [
          "[bundle]",
          "version = 1",
          "",
          "[resource]",
          'name = "Simulator"',
          "cycle_interval_ms = 50",
          "",
          "[runtime.ads]",
          "enabled = false",
          'config_path = "old-ads.toml"',
          "",
        ].join("\n")
      );
      const result = ensureAdsRuntimeEnabled(dir);
      assert.deepStrictEqual(result.ok, true);
      const runtimeToml = fs.readFileSync(path.join(dir, "runtime.toml"), "utf8");
      assert.match(runtimeToml, /\[runtime\.ads\]/);
      assert.match(runtimeToml, /^enabled = true$/m);
      assert.match(runtimeToml, /^config_path = "ads\.toml"$/m);
      assert.match(runtimeToml, /^worker_tick_interval_ms = 20$/m);
    } finally {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });
  test("connector status presentation uses first-user vocabulary", () => {
    assert.strictEqual(connectorConnectionLabel("ready"), "Ready");
    assert.strictEqual(connectorConnectionLabel("not_ready"), "Needs attention");
    assert.strictEqual(connectorHealthLabel("ok"), "OK");
    assert.strictEqual(connectorHealthLabel("degraded"), "Degraded");
    assert.strictEqual(discoveryConfidenceLabel("port_reachable"), "Port reachable only");
    assert.strictEqual(discoverySourceLabel("tcp_connect"), "Known address");
    assert.strictEqual(
      connectorSignalsSummary({ good: 1, degraded: 1, unavailable: 1 }),
      "1 good, 2 need attention"
    );
    assert.ok(!discoveryConfidenceLabel("port_reachable").includes("tcp-only"));
    assert.ok(!discoveryConfidenceLabel("port_reachable").includes("port_reachable"));
    assert.ok(!discoverySourceLabel("tcp_connect").includes("tcp_connect"));
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
});
