import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

import type { FleetTopologyResponse } from "../../networkCanvas/fleetTopology";
import {
  DEVICES_TOPOLOGY_COMMAND,
  requestCanvasTopology,
  type CanvasRuntimeReport,
  type CanvasTopologyAnswer,
  type CanvasTopologyRequest,
} from "../../networkCanvas/canvasCommands";
import { canvasGraph } from "../../networkCanvas/graphData";
import { buildGraph } from "../../networkCanvas/webview/layout";
import type { EndpointNodeData, NCGraph } from "../../networkCanvas/webview/types";
import { classifyRuntimeStartFailure } from "../../networkCanvas/runtimeFailures";
import { ADD_PICKER_GROUPS, groupForAddPicker } from "../../networkCanvas/webview/grouping";
import { applyFilter, filterReport } from "../../networkCanvas/webview/filter";
import {
  connectorConnectionLabel,
  connectorHealthLabel,
  connectorSignalsSummary,
  discoveryConfidenceLabel,
  discoverySourceLabel,
} from "../../networkCanvas/webview/connectorPresentation";
import {
  visibleSchemaFields,
  type CommProtocolSchema,
} from "../../communication/schemaForm";
import { protocolColor, protocolName } from "../../networkCanvas/webview/protocolMeta";
import { t } from "../../networkCanvas/webview/theme";
import {
  formatExposedGlobals,
  serverEndpointSummaryRows,
} from "../../networkCanvas/webview/serverEndpointSummary";
import { visibleFaultsForValidationState } from "../../networkCanvas/webview/faults";

/** The extension's lifecycle while the simulator runs. */
const RUNNING = { running: true, runtime_state: "running", starting: false, io_values: false };

/** The endpoint the canvas reached the runtime of these tests at. */
const LINE_ENDPOINT = "tcp://192.168.77.10:5680";
/** The control endpoint of the extension's own simulator in these tests. */
const SIMULATOR_ENDPOINT = "unix:///tmp/trust-local-sim.sock";

/** Waits until the language client has registered the server's devices commands. */
async function languageServerReady(): Promise<void> {
  const deadline = Date.now() + 30000;
  while (!(await vscode.commands.getCommands(true)).includes(DEVICES_TOPOLOGY_COMMAND)) {
    if (Date.now() > deadline) assert.fail("the language server's devices commands are not available");
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
}

/**
 * One drawing of the canvas as the panel runs it: `topology` is what the runtime at `LINE_ENDPOINT`
 * (or, with `simulator`, the extension's own simulator) answered, the language server merges it with
 * the extension's own facts `facts` (`trust-lsp.devicesTopology`), and the canvas maps the answer into
 * its graph. The merge's rules are the Rust tests' (crates/trust-devices/src/canvas); these tests
 * prove the wiring and what is drawn.
 */
async function served(
  topology: FleetTopologyResponse | undefined,
  options: {
    readonly connectors?: unknown;
    readonly searchQuery?: string;
    readonly peers?: readonly CanvasRuntimeReport[];
    readonly project?: FleetTopologyResponse;
    readonly simulator?: boolean;
    readonly runMode?: string;
    readonly endpoint?: string;
    readonly facts?: Partial<CanvasTopologyRequest>;
  } = {}
): Promise<{
  answer: CanvasTopologyAnswer;
  graph: NCGraph;
}> {
  const endpoint = options.endpoint ?? (options.simulator ? SIMULATOR_ENDPOINT : LINE_ENDPOINT);
  const answer = await requestCanvasTopology({
    primary: topology
      ? {
          endpoint,
          label: "Line",
          reach: "reachable",
          topology: { ok: topology },
          connectors: { ok: options.connectors ?? { connectors: [] } },
        }
      : undefined,
    peers: options.peers ?? [],
    project: options.project,
    simulator_endpoint: options.simulator ? SIMULATOR_ENDPOINT : undefined,
    run_mode: options.runMode ?? (options.simulator ? "simulate" : "online"),
    search_query: options.searchQuery ?? "",
    discovered: [],
    ...options.facts,
  });
  const graph = canvasGraph(answer.fleet, {
    searchQuery: options.searchQuery,
    topologyError: answer.topology_error,
  });
  return { answer, graph };
}

/** The graph's runtimes, on every host and in every container. */
function graphRuntimes(graph: NCGraph) {
  return graph.hosts.flatMap((host) => [
    ...host.runtimes,
    ...host.containers.flatMap((container) => container.runtimes),
  ]);
}

/** The graph's connection named `name`. */
function graphEndpoint(graph: NCGraph, name: string) {
  const endpoint = graphRuntimes(graph)
    .flatMap((runtime) => runtime.endpoints)
    .find((candidate) => candidate.name === name);
  assert.ok(endpoint, `the graph draws the connection ${name}`);
  return endpoint;
}

suite("Network Canvas", function () {
  this.timeout(60000);
  suiteSetup(languageServerReady);

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
  // The rules are the Rust tests `the_drawn_simulator_shows_its_lifecycle_in_its_node_banner_and_summary`,
  // `the_first_device_waits_for_io_values_before_it_is_connected` and
  // `the_simulator_holds_the_catalogs_local_connections_connected_only_with_evidence`
  // (crates/trust-devices/src/canvas/local_tests.rs); these tests prove that the canvas draws them.
  test("no step of the add flow draws a running simulator or a connected device without evidence", async () => {
    for (const stage of ["runtime_live", "intent", "add_device", "connected"] as const) {
      const { graph } = await served(undefined, {
        runMode: "simulate",
        facts: { setup: { stage, active_protocol: "simulated", device_requested: true } },
      });
      const simulator = graph.hosts[0]?.runtimes[0];
      assert.notStrictEqual(simulator?.health, "connected", `${stage} faked running`);
      assert.ok(
        simulator?.endpoints.every((endpoint) => endpoint.health !== "connected"),
        `${stage} faked a connected device`
      );
    }
  });

  test("the simulator goes green only from lifecycle evidence, and its first device only with I/O values", async () => {
    const stopped = (await served(undefined, { runMode: "simulate" })).graph;
    assert.notStrictEqual(stopped.hosts[0]?.runtimes[0]?.health, "connected");
    const asked = { setup: { stage: "connected", active_protocol: "simulated", device_requested: true } };
    const running = (await served(undefined, { runMode: "simulate", facts: { simulator: RUNNING, ...asked } })).graph;
    const simulator = running.hosts[0]?.runtimes[0];
    assert.strictEqual(simulator?.health, "connected");
    assert.strictEqual(simulator?.endpoints[0]?.health, "pending", "no I/O values yet");
    const live = (
      await served(undefined, {
        runMode: "simulate",
        facts: { simulator: { ...RUNNING, io_values: true }, ...asked },
      })
    ).graph;
    assert.strictEqual(live.hosts[0]?.runtimes[0]?.endpoints[0]?.health, "connected");
  });

  test("runtime rolls health up from raw endpoint evidence; host stays reachability", async () => {
    const { answer, graph } = await served(fleetTopology());
    assert.ok(answer.fleet, "expected the server's fleet view");
    // Host status is MACHINE reachability, not a health rollup: a host we reached (arch/os/ips present)
    // is "connected" even with a degraded endpoint inside — the degradation surfaces on the RUNTIME.
    assert.strictEqual(graph.hosts[0]?.health, "connected");
    assert.strictEqual(graph.hosts[0]?.runtimes[0]?.health, "degraded");
  });

  test("fleet host headlines are user-facing, not raw lab machine names", async () => {
    // The project's configuration is read on this computer, so the host it names is "This computer"
    // (specification 25 §5.5); the raw hostname stays as supporting detail.
    const local = fleetTopology();
    const { graph: localGraph } = await served(local, { project: local });
    assert.strictEqual(localGraph.hosts[0]?.hostname, "This computer");
    assert.ok(
      localGraph.hosts[0]?.label.includes("trust-pi"),
      "the raw local hostname stays available as supporting detail"
    );

    const remote = fleetTopology();
    remote.hosts[0].hostname = "remote-plc-host";
    remote.hosts[0].ips = ["192.168.77.10"];
    const { graph: remoteGraph } = await served(remote);
    assert.strictEqual(remoteGraph.hosts[0]?.hostname, "Computer 192.168.77.10");
    assert.ok(
      remoteGraph.hosts[0]?.label.includes("remote-plc-host"),
      "the raw hostname remains available as supporting detail"
    );
  });

  test("unreachable configured peers keep their configured label, not 'This computer'", async () => {
    const { graph } = await served(undefined, {
      peers: [{ endpoint: "tcp://127.0.0.1:5510", label: "discoveredcell", reach: "auth_rejected" }],
    });
    assert.deepStrictEqual(
      graph.hosts.map((host) => host.hostname),
      ["This computer", "discoveredcell"]
    );
  });

  test("fleet search never hides degraded endpoints from the runtime rollup", async () => {
    // "modbus" does NOT match the degraded mqtt endpoint.
    const { graph } = await served(fleetTopology(), { searchQuery: "modbus" });
    assert.strictEqual(graph.hosts[0]?.health, "connected"); // reachability, unaffected by search
    assert.strictEqual(graph.hosts[0]?.runtimes[0]?.health, "degraded");
    const mqtt = graph.hosts[0]?.runtimes[0]?.endpoints.find(
      (e) => e.protocol === "mqtt"
    );
    assert.strictEqual(mqtt?.health, "degraded", "raw health preserved");
    assert.strictEqual(mqtt?.dimmed, true, "non-match dimmed for display only");
  });

  test("connector status surface flows into endpoint graph metadata", async () => {
    const { graph } = await served(fleetTopology(), { connectors: {
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
    } });
    const rendered = buildGraph(graph, undefined, false);
    const modbus = rendered.nodes.find((node) => node.id === graphEndpoint(graph, "Modbus meter").id);
    const mqtt = rendered.nodes.find((node) => node.id === graphEndpoint(graph, "MQTT broker").id);
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

  test("fleet search dims external counterparts and wires without hiding warnings", async () => {
    const { graph } = await served(fleetTopology(), { searchQuery: "modbus" });
    const rendered = buildGraph(graph, undefined, false);
    const mqttEndpoint = rendered.nodes.find((node) => node.id === graphEndpoint(graph, "MQTT broker").id);
    const mqttBroker = rendered.nodes.find((node) => node.id === "shared:mqtt:broker");
    const mqttLink = rendered.edges.find((edge) => edge.target === "shared:mqtt:broker");
    const warning = graph.faults.find((fault) => /mqtt/i.test(fault.label));

    assert.strictEqual(mqttEndpoint?.data.dimmed, true, "nonmatching endpoint is dimmed");
    assert.strictEqual(mqttBroker?.data.dimmed, true, "counterpart external node is dimmed too");
    assert.strictEqual(mqttLink?.data?.dimmed, true, "counterpart wire is dimmed too");
    assert.ok(warning, "search preserves the degraded endpoint warning");
  });

  test("editing a rejected add form hides stale apply faults without hiding real faults", () => {
    // Which fault is the form's is the server's (`form`; the Rust test
    // `the_add_form_draws_its_draft_and_its_refused_result_is_the_forms_own_fault`).
    const faults = [
      {
        id: "apply:modbus_tcp",
        label: "Configuration was not applied. Fix the highlighted fields and try again.",
        targetNodeId: "draft:modbus_tcp",
        severity: "error" as const,
        form: true,
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
  test("the canvas maps a real fleet to host/runtime/endpoint nodes + links", async () => {
    const { graph } = await served(fleetTopology());
    assert.strictEqual(graph.kind, "graph");
    assert.strictEqual(graph.hosts.length, 1);
    const runtime = graph.hosts[0].runtimes[0];
    assert.strictEqual(runtime.name, "Line runtime");
    const mqtt = graphEndpoint(graph, "MQTT broker");
    assert.ok(runtime.endpoints.some((e) => e.id === mqtt.id));
    assert.ok(graph.links.some((l) => l.protocol === "mesh"));
    assert.ok(graph.external.some((x) => x.name === "tcp/192.168.77.11:7447"));
    assert.ok(
      graph.external.some((x) => x.id === "shared:mqtt:broker"),
      "shared MQTT broker nodes must render so publish/subscribe links are not dangling"
    );
    assert.ok(graph.links.some((l) => l.to === "shared:mqtt:broker"));
    assert.ok(
      graph.faults.some((f) => f.targetNodeId === mqtt.id),
      "degraded endpoint surfaces as a fault"
    );
  });

  test("degraded and error fleet-link status and detail survive full canvas projection", async () => {
    const topology = fleetTopology();
    const degradedSource = topology.links.find((link) => link.status === "degraded");
    assert.ok(degradedSource, "expected a degraded fleet link fixture");
    degradedSource.detail = "Peer handshake is retrying.";

    topology.links.push({
      id: "link:mesh:error",
      from: degradedSource.from,
      to: degradedSource.to,
      protocol: degradedSource.protocol,
      role: degradedSource.role,
      direction: degradedSource.direction,
      same_host: degradedSource.same_host,
      status: "error",
      secure: degradedSource.secure,
      detail: "Peer certificate was rejected.",
    });

    const { graph: canvas } = await served(topology);
    const degraded = canvas.links.find((link) => link.detail === "Peer handshake is retrying.");
    const error = canvas.links.find((link) => link.detail === "Peer certificate was rejected.");
    assert.ok(degraded && error, "both links survive the merge");
    assert.strictEqual(
      (degraded as unknown as { detail?: string })?.detail,
      "Peer handshake is retrying."
    );
    assert.strictEqual(
      (error as unknown as { detail?: string })?.detail,
      "Peer certificate was rejected."
    );

    const rendered = buildGraph(canvas);
    const degradedEdge = rendered.edges.find((edge) => edge.id === degraded?.id);
    const errorEdge = rendered.edges.find((edge) => edge.id === error?.id);
    assert.deepStrictEqual(
      {
        status: degradedEdge?.data?.status,
        detail: degradedEdge?.data?.detail,
      },
      { status: "degraded", detail: "Peer handshake is retrying." }
    );
    assert.deepStrictEqual(
      { status: errorEdge?.data?.status, detail: errorEdge?.data?.detail },
      { status: "error", detail: "Peer certificate was rejected." }
    );
  });

  test("the canvas does not duplicate a runtime as an external system", async () => {
    const topology = fleetTopology();
    topology.external.push({
      id: "external:self",
      kind: "runtime",
      name: "Line runtime",
      via_protocol: ["discovery"],
      direction: "outbound",
    });
    const { graph } = await served(topology);
    assert.ok(
      graph.hosts[0].runtimes.some((runtime) => runtime.name === "Line runtime"),
      "the runtime itself stays visible"
    );
    assert.ok(
      !graph.external.some((external) => external.name === "Line runtime"),
      "the same runtime must not also appear as an external system"
    );
    assert.ok(
      graph.external.some((external) => external.name === "tcp/192.168.77.11:7447"),
      "unrelated external systems stay visible"
    );
  });

  test("the canvas never emits an edge to a node that does not exist", async () => {
    const { graph } = await served(fleetTopology());
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
      assert.ok(
        ids.has(link.from) && ids.has(link.to),
        `link ${link.id} references known nodes`
      );
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
        {
          id: "link:degraded",
          from: "endpoint:ads",
          to: "external:ads",
          protocol: "ads",
          role: "client",
          status: "degraded",
          secure: false,
        },
        {
          id: "link:configured",
          from: "endpoint:ads",
          to: "external:ads",
          protocol: "ads",
          role: "client",
          status: "configured_policy",
          secure: false,
        },
        {
          id: "link:error",
          from: "endpoint:ads",
          to: "external:ads",
          protocol: "ads",
          role: "client",
          status: "error",
          secure: false,
        },
        {
          id: "link:future-status",
          from: "endpoint:ads",
          to: "external:ads",
          protocol: "ads",
          role: "client",
          status: "future_status",
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
    const degradedEdge = graph.edges.find((edge) => edge.id === "link:degraded");
    const configuredEdge = graph.edges.find((edge) => edge.id === "link:configured");
    const errorEdge = graph.edges.find((edge) => edge.id === "link:error");
    const futureStatusEdge = graph.edges.find((edge) => edge.id === "link:future-status");
    assert.ok(draftEdge, "expected the pending openot wire");
    assert.ok(liveEdge, "expected the connected ads wire");
    assert.strictEqual(draftEdge?.data?.dashed, true, "a pending link must render dashed, not as a live connection");
    assert.ok(!liveEdge?.data?.dashed, "a connected link must render solid");
    assert.strictEqual(configuredEdge?.data?.dashed, true, "configured intent is not a proven live link");
    assert.ok(!degradedEdge?.data?.dashed, "a proven degraded link stays solid and uses health styling");
    assert.ok(!errorEdge?.data?.dashed, "a proven failed link stays solid and uses error styling");
    assert.strictEqual(
      futureStatusEdge?.data?.dashed,
      true,
      "an unrecognized status must fail closed as unproven"
    );
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
                {
                  id: "endpoint:mesh-future",
                  kind: "field",
                  protocol: "mesh",
                  name: "Mesh (future status)",
                  role: "peer",
                  health: "future_status",
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
    const futureMesh = graph.edges.find((edge) => edge.id === "mesh-endpoint:mesh-future");
    const meshBus = graph.nodes.find((node) => node.id === "bus:mesh");
    assert.ok(draftMesh, "expected a bus wire for the draft mesh peer");
    assert.ok(liveMesh, "expected a bus wire for the live mesh peer");
    assert.strictEqual(draftMesh?.data?.dashed, true, "a pending mesh peer must render dashed");
    assert.strictEqual(liveMesh?.data?.dashed, true, "a mixed draft/live fabric stays dashed until all peers are live");
    assert.strictEqual(futureMesh?.data?.dashed, true, "an unrecognized mesh status must fail closed as unproven");
    assert.strictEqual(meshBus?.data?.draft, true, "mesh fabric must visibly carry the DRAFT state");
    assert.strictEqual(meshBus?.data?.showLabel, true, "a multi-peer mesh fabric needs its shared-bus label");
    assert.notStrictEqual(meshBus?.data?.color, "rgb(137,209,133)", "draft fabric must not use the live green");
    assert.strictEqual(draftMesh?.data?.color, meshBus?.data?.color, "draft mesh wires must use the same muted draft role as the bus");
    assert.strictEqual(liveMesh?.data?.color, meshBus?.data?.color, "mixed mesh fabric wires stay muted while any peer is draft");
  });

  test("an all-proven degraded/error mesh renders solid", () => {
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
              health: "connected",
              detail: "",
              endpoints: [
                {
                  id: "endpoint:mesh-degraded",
                  kind: "field",
                  protocol: "mesh",
                  name: "Mesh (degraded)",
                  role: "peer",
                  health: "degraded",
                  detail: "Mesh peer latency is elevated.",
                },
                {
                  id: "endpoint:mesh-error",
                  kind: "field",
                  protocol: "mesh",
                  name: "Mesh (error)",
                  role: "peer",
                  health: "error",
                  detail: "Mesh peer authentication failed.",
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

    const degradedMesh = graph.edges.find((edge) => edge.id === "mesh-endpoint:mesh-degraded");
    const errorMesh = graph.edges.find((edge) => edge.id === "mesh-endpoint:mesh-error");
    const meshBus = graph.nodes.find((node) => node.id === "bus:mesh");
    assert.strictEqual(degradedMesh?.data?.dashed, false);
    assert.strictEqual(errorMesh?.data?.dashed, false);
    assert.deepStrictEqual(
      {
        color: degradedMesh?.data?.color,
        status: degradedMesh?.data?.status,
        detail: degradedMesh?.data?.detail,
      },
      {
        color: t.warn,
        status: "degraded",
        detail: "Mesh peer latency is elevated.",
      }
    );
    assert.deepStrictEqual(
      {
        color: errorMesh?.data?.color,
        status: errorMesh?.data?.status,
        detail: errorMesh?.data?.detail,
      },
      {
        color: t.danger,
        status: "error",
        detail: "Mesh peer authentication failed.",
      }
    );
    assert.strictEqual(meshBus?.data?.draft, false);
  });

  test("production-shaped mesh links retain each configured peer status and detail on the shared fabric", async () => {
    const { graph: canvas } = await served(fleetTopology());
    const rendered = buildGraph(canvas);
    const peer = rendered.edges.find((edge) => edge.data?.detail === "tcp/192.168.77.11:7447");

    assert.ok(peer, "the runtime's configured mesh target must remain wired to the shared fabric");
    assert.deepStrictEqual(
      {
        dashed: peer?.data?.dashed,
        status: peer?.data?.status,
        detail: peer?.data?.detail,
      },
      {
        dashed: false,
        status: "degraded",
        detail: "tcp/192.168.77.11:7447",
      }
    );
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

  test("ADS server live client count survives topology to endpoint node data", async () => {
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

    const { graph: canvas } = await served(topology);
    const endpoint = canvas.hosts[0]?.runtimes[0]?.endpoints[0];
    assert.deepStrictEqual(endpoint?.live?.value, {
      connected_clients: 3,
      proof_status: "external_client_verified",
      external_client_verified: true,
      external_client_kind: "loopback-ads-client",
      external_client_name: "trust-runtime-doctor",
    });

    const graph = buildGraph(canvas);
    const node = graph.nodes.find((item) => item.id === endpoint?.id);
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
      external: [{ id: "external:ads:server", name: "ads server", kind: "server" }],
      faults: [],
    });
    const external = graph.nodes.find((node) => node.id === "external:ads:server");
    assert.ok(external, "expected external ADS server node");
    assert.strictEqual(external?.data.label, "ADS server");
    assert.strictEqual(external?.data.sub, "ADS server");
    assert.notStrictEqual(external?.data.sub, "ADS client server");
  });

  test("protocol filtering reports hidden degraded/faulted endpoints instead of silently losing them", async () => {
    const { graph } = await served(fleetTopology());
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
      "1 host · 1 runtime · 2 endpoints",
      "footer summary follows visible endpoint nodes after filtering"
    );
  });

  // --- Multi-runtime merge (§10/§12.10) ------------------------------------
  // The merge's rules (one host per identity, unioned runtimes, configured endpoints staying on
  // their live runtime) are the Rust tests `merge_unions_hosts_and_keeps_the_first_host_facts` and
  // `configured_endpoints_stay_on_the_live_runtime` (crates/trust-devices/src/topology/tests).
  test("configured ADS overlay on a live simulator says restart required, not stopped", async () => {
    const live: FleetTopologyResponse = {
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
              runtime_id: "RESOURCE",
              name: "RESOURCE",
              mode: "simulate",
              cycle_ms: 50,
              health: "simulate",
              detail: "Runtime answered fleet.topology from its control channel.",
              source: "self",
              endpoints: [],
            },
          ],
        },
      ],
      links: [],
      shared: [],
      external: [],
    };
    const configured: FleetTopologyResponse = JSON.parse(JSON.stringify(live));
    configured.hosts[0].runtimes[0] = {
      runtime_id: "ADS live TwinCAT",
      name: "ADS live TwinCAT",
      mode: "stopped",
      cycle_ms: 50,
      health: "configured_policy",
      detail: "Configured in project files; runtime is not running.",
      source: "config",
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
          source: "config",
        },
      ],
    };

    const { graph } = await served(live, { simulator: true, project: configured });
    const runtime = graph.hosts[0]?.runtimes[0];
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

  test("a configured-but-unreachable fleet peer synthesizes an UNKNOWN node (not 'stopped'), never green", async () => {
    const peer = (
      reach: CanvasRuntimeReport["reach"],
      endpoint = "tcp://10.0.0.9:5510"
    ): CanvasRuntimeReport => ({ endpoint, label: "cell1", reach });
    // With nothing that reports the project's runtime the simulator stands for it first on this
    // computer (specification 25 §5.6); the peer's computer follows.
    const peerHost = (graph: NCGraph) => graph.hosts[graph.hosts.length - 1];
    const unreachable = (await served(undefined, { peers: [peer("unreachable")] })).graph;
    const runtime = peerHost(unreachable).runtimes[0];
    // We don't know if it's stopped or just unreachable → "unknown" (grey/ghosted), never green.
    assert.strictEqual(runtime.health, "unknown", "unknown/grey, never connected/green");
    assert.strictEqual(runtime.mode, "unknown");
    assert.strictEqual(runtime.endpoints.length, 0);
    assert.strictEqual(peerHost(unreachable).hostname, "cell1");
    const missing = (await served(undefined, { peers: [peer("auth_missing")] })).graph;
    assert.strictEqual(peerHost(missing).runtimes[0].health, "error", "auth failure is a real error");
    assert.strictEqual(
      peerHost(missing).runtimes[0].detail,
      "No auth token provided — this runtime requires one."
    );
    const rejected = (await served(undefined, { peers: [peer("auth_rejected")] })).graph;
    assert.strictEqual(
      peerHost(rejected).runtimes[0].detail,
      "Auth token rejected — check it and try again."
    );
    const reachable = (
      await served(undefined, {
        peers: [
          {
            ...peer("reachable"),
            topology: { ok: fleetTopology() },
            connectors: { ok: { connectors: [] } },
          },
        ],
      })
    ).graph;
    assert.strictEqual(
      peerHost(reachable).runtimes[0].name,
      "Line runtime",
      "a reachable peer uses its real fleet.topology, not a synthetic node"
    );
    const endpointless = await served(undefined, { peers: [peer("unreachable", "")] });
    assert.ok(
      endpointless.graph.hosts.every((host) => host.hostname !== "cell1"),
      "no endpoint → no node for the peer"
    );
  });

  test("peer topology failures show in the banner without hiding the local view", async () => {
    const { graph } = await served(undefined, {
      runMode: "simulate",
      peers: [
        {
          endpoint: "tcp://10.0.0.9:5510",
          label: "peer-a",
          reach: "reachable",
          topology: { error: "unknown connector confidence" },
          connectors: { ok: { connectors: [] } },
        },
      ],
    });
    assert.ok(
      graph.hosts.some((host) => host.id === "host:this-computer"),
      "the local runtime remains visible"
    );
    assert.strictEqual(graph.banner?.kind, "error");
    assert.match(graph.banner?.text ?? "", /^Peer topology degraded: .*unknown connector confidence/);
  });

  test("auth-failed synthetic runtime nodes keep control endpoint for inspector actions", async () => {
    const endpoint = "tcp://127.0.0.1:33101";
    const { graph } = await served(undefined, {
      peers: [{ endpoint, label: "discoveredcell", reach: "auth_rejected" }],
    });
    const rendered = buildGraph(graph).nodes.find(
      (node) => node.type === "runtime" && node.data.label === "discoveredcell"
    );
    assert.ok(rendered, "synthetic auth-failed runtime node should render");
    assert.strictEqual(
      rendered?.data.controlEndpoint,
      endpoint,
      "runtime inspector actions need the endpoint for Set auth token and Set as run target"
    );
  });

  test("a new project with no configured runtime shows the local simulator node, not an empty screen", async () => {
    const { graph } = await served(undefined, { runMode: "simulate" });
    assert.strictEqual(graph.kind, "graph");
    assert.strictEqual(graph.hosts.length, 1);
    const runtime = graph.hosts[0].runtimes[0];
    assert.ok(runtime, "local simulator runtime node is always present");
    assert.notStrictEqual(runtime.health, "connected", "honest: not green until proven");
  });

  test("a stopped local control socket renders neutral stopped state, not an alarm fault", async () => {
    const { graph } = await served(undefined, {
      runMode: "simulate",
      facts: {
        failure: {
          kind: "stale_runtime",
          message: "Runtime not reachable: unix:///tmp/trust-runtime-line-123.sock",
        },
      },
    });
    const runtime = graph.hosts[0].runtimes[0];
    assert.strictEqual(runtime.health, "stopped", "stopped local runtime is neutral, not red");
    assert.deepStrictEqual(graph.faults, [], "stopped local runtime must not spend the red issue channel");
    assert.strictEqual(graph.banner?.kind, "info", "guidance stays neutral/info");
    assert.ok(
      !JSON.stringify(graph).includes("unix:///tmp/trust-runtime-line"),
      "raw local socket paths must not leak into the rendered graph"
    );
  });

  test("a running local simulator renders as a connected runtime node", async () => {
    const { graph } = await served(undefined, { runMode: "simulate", facts: { simulator: RUNNING } });
    assert.strictEqual(graph.hosts[0].runtimes[0].health, "connected");
  });

  test("a live local simulator topology replaces the stopped project overlay instead of twinning", async () => {
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
    const { graph } = await served(liveTopology, {
      simulator: true,
      project: offlineProjectTopology,
    });

    assert.strictEqual(graph.hosts.length, 1, "local simulator stays on one host");
    assert.strictEqual(graph.summary, "1 host · 1 runtime · 2 endpoints");
    const runtimes = graph.hosts[0].runtimes;
    assert.strictEqual(runtimes.length, 1, "one local simulator runtime, not RESOURCE + configured project twin");
    assert.strictEqual(runtimes[0].name, "Simulator", "ST resource names must not replace the run target");
    // The runtime's own state names its health (specification 25 §5.5): it simulates, drawn
    // "Running" in green, as a connected runtime is.
    assert.strictEqual(runtimes[0].health, "simulate");
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

  test("a live local simulator topology hides raw ST resource names even when mode is missing", async () => {
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

    const { graph } = await served(liveTopology, { simulator: true });

    assert.strictEqual(graph.hosts[0].runtimes[0].name, "Simulator");
  });

  test("managed local runtimes are injected under the existing This computer host", async () => {
    const { graph } = await served(undefined, {
      runMode: "simulate",
      facts: {
        managed: [
          { name: "cell1", control_endpoint: "tcp://127.0.0.1:9902", state: "stopped" },
          { name: "cell2", control_endpoint: "tcp://127.0.0.1:9903", state: "running" },
        ],
      },
    });
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

  test("managed starting runtime is pending with an Updating control", async () => {
    const { graph } = await served(undefined, {
      runMode: "simulate",
      facts: { managed: [{ name: "cell1", control_endpoint: "tcp://127.0.0.1:9902", state: "starting" }] },
    });
    const runtime = graph.hosts
      .flatMap((host) => host.runtimes)
      .find((candidate) => candidate.managedName === "cell1");
    assert.strictEqual(runtime?.health, "pending");
    assert.strictEqual(runtime?.lifecycleState, "starting");
    assert.strictEqual(runtime?.detail, "Starting managed local runtime…");

    const controls = runtime?.controls ?? [];
    assert.strictEqual(controls[0].action, "none");
    assert.strictEqual(controls[0].label, "Updating…");
    assert.strictEqual(controls[0].enabled, false);
  });

  test("managed stopping runtime is pending with an Updating control", async () => {
    const { graph } = await served(undefined, {
      runMode: "simulate",
      facts: { managed: [{ name: "cell1", control_endpoint: "tcp://127.0.0.1:9902", state: "stopping" }] },
    });
    const runtime = graph.hosts
      .flatMap((host) => host.runtimes)
      .find((candidate) => candidate.managedName === "cell1");
    assert.strictEqual(runtime?.health, "pending");
    assert.strictEqual(runtime?.lifecycleState, "stopping");
    assert.strictEqual(runtime?.detail, "Stopping managed local runtime…");

    const controls = runtime?.controls ?? [];
    assert.strictEqual(controls[0].action, "none");
    assert.strictEqual(controls[0].label, "Updating…");
    assert.strictEqual(controls[0].enabled, false);
  });

  test("managed unavailable runtime fails closed with Start disabled", async () => {
    const { graph } = await served(undefined, {
      runMode: "simulate",
      facts: { managed: [{ name: "cell1", control_endpoint: "tcp://127.0.0.1:9902", state: "unavailable" }] },
    });
    const runtime = graph.hosts
      .flatMap((host) => host.runtimes)
      .find((candidate) => candidate.managedName === "cell1");
    assert.strictEqual(runtime?.health, "error");
    assert.strictEqual(runtime?.lifecycleState, "unavailable");
    assert.strictEqual(runtime?.detail, "Status unavailable — refresh before starting.");

    const controls = runtime?.controls ?? [];
    assert.strictEqual(controls[0].action, "managedStart");
    assert.strictEqual(controls[0].label, "Start");
    assert.strictEqual(controls[0].enabled, false);
  });

  test("selected run target is projected onto the graph node and rendered node data", async () => {
    const { graph } = await served(undefined, {
      runMode: "simulate",
      facts: {
        managed: [
          { name: "cell1", control_endpoint: "tcp://127.0.0.1:9902", state: "stopped" },
          { name: "cell2", control_endpoint: "tcp://127.0.0.1:9903", state: "running" },
        ],
        session: { run_target: "cell1" },
      },
    });
    const cell1 = graph.hosts[0].runtimes.find((runtime) => runtime.managedName === "cell1");
    const cell2 = graph.hosts[0].runtimes.find((runtime) => runtime.managedName === "cell2");
    const simulator = graph.hosts[0].runtimes.find((runtime) => runtime.id === "runtime:local");

    assert.strictEqual(cell1?.runTarget, true, "the selected managed runtime gets a run-target flag");
    assert.strictEqual(cell2?.runTarget, false, "non-selected managed runtimes are not flagged");
    assert.strictEqual(simulator?.runTarget, false, "the simulator is not flagged when a managed runtime is selected");

    const rendered = buildGraph(graph).nodes.find((node) => node.type === "runtime" && node.id === cell1?.id);
    assert.strictEqual(rendered?.data.runTarget, true, "React Flow runtime node data carries the flag");
  });

  test("a managed runtime already shown via fleet.topology is NOT doubled, and the surviving node stays OWNED (managed)", async () => {
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
    const { graph } = await served(undefined, {
      peers: [
        {
          endpoint: "tcp://127.0.0.1:9902",
          label: "peer",
          reach: "reachable",
          topology: { ok: peerTopology },
          connectors: { ok: { connectors: [] } },
        },
      ],
      facts: { managed: [{ name: "cell1", control_endpoint: "tcp://127.0.0.1:9902", state: "running" }] },
    });
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
    const controls = surviving?.controls ?? [];
    assert.strictEqual(controls[0].action, "managedStop");
    assert.ok(controls.some((c) => c.action === "openRuntimeLogs"), "owned node offers Logs");
  });

  test("managed status overrides stale health on an existing fleet topology node", async () => {
    const expected = [
      ["running", "connected", "Running (managed local runtime).", "managedStop", true],
      ["stopped", "stopped", "Stopped — Start it from this node.", "managedStart", true],
      ["starting", "pending", "Starting managed local runtime…", "none", false],
      ["stopping", "pending", "Stopping managed local runtime…", "none", false],
      [
        "unavailable",
        "error",
        "Status unavailable — refresh before starting.",
        "managedStart",
        false,
      ],
    ] as const;

    for (const [state, health, detail, action, enabled] of expected) {
      const topology: FleetTopologyResponse = {
        schema_version: 3,
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
                detail: "Stale topology health.",
                endpoints: [],
              },
            ],
          },
        ],
        links: [],
        shared: [],
        external: [],
      };
      const { graph } = await served(undefined, {
        peers: [
          {
            endpoint: "tcp://127.0.0.1:9902",
            label: "peer",
            reach: "reachable",
            topology: { ok: topology },
            connectors: { ok: { connectors: [] } },
          },
        ],
        facts: { managed: [{ name: "cell1", control_endpoint: "tcp://127.0.0.1:9902", state }] },
      });
      const runtime = graph.hosts.flatMap((host) => host.runtimes).find((item) => item.name === "cell1");
      assert.strictEqual(runtime?.health, health, `${state} health must come from managed status`);
      assert.strictEqual(runtime?.lifecycleState, state, `${state} lifecycle label must remain authoritative`);
      assert.strictEqual(runtime?.detail, detail, `${state} detail must come from managed status`);
      const controls = runtime?.controls ?? [];
      assert.strictEqual(controls[0].action, action, `${state} must expose the correct primary action`);
      assert.strictEqual(controls[0].enabled, enabled, `${state} must use authoritative action availability`);
    }
  });

  test("live managed topology preserves the stopped project runtime instead of morphing the canvas", async () => {
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

    const { graph } = await served(liveTopology, {
      endpoint: "tcp://127.0.0.1:9902",
      project: offlineProjectTopology,
      runMode: "simulate",
      facts: { managed: [{ name: "cell1", control_endpoint: "tcp://127.0.0.1:9902", state: "running" }] },
    });

    assert.strictEqual(graph.hosts.length, 1, "same computer stays one host");
    assert.strictEqual(graph.summary, "1 host · 2 runtimes · 2 endpoints");
    const runtimes = graph.hosts[0].runtimes;
    assert.deepStrictEqual(
      runtimes.map((runtime) => runtime.name).sort(),
      ["Simulator", "cell1"],
      "managed Start must not delete the project runtime the user just saw"
    );
    const cell1 = runtimes.find((runtime) => runtime.name === "cell1");
    const projectRuntime = runtimes.find((runtime) => runtime.name === "Simulator");
    assert.strictEqual(cell1?.managed, true, "managed runtime keeps owned lifecycle controls");
    assert.strictEqual(cell1?.health, "connected", "started managed runtime is honestly running");
    assert.strictEqual(
      projectRuntime?.health,
      "stopped",
      "project runtime remains visible with an honest stopped state"
    );
  });

  test("a runtime start failure renders an error node + retry banner, not a failure screen", async () => {
    const { graph } = await served(undefined, {
      runMode: "simulate",
      facts: { failure: { kind: "port_conflict", message: "The runtime port is already in use." } },
    });
    assert.strictEqual(graph.hosts[0].runtimes[0].health, "error");
    assert.ok(graph.banner, "failure surfaces an inline banner");
    assert.ok(graph.banner?.actions.some((a) => a.action === "startLocalSimulator"));
  });

  // --- Failure classification ----------------------------------------------
  // The kinds are the language server's (`trust-lsp.runTargets`, action `failure`), proven by
  // crates/trust-lsp/src/handlers/commands/run_targets/connect_tests.rs. Catches a failure that does not reach the server,
  // or comes back without its text.
  test("classifyRuntimeStartFailure takes the failure's kind from the language server", async () => {
    const failure = await classifyRuntimeStartFailure(new Error("spawn trust-runtime ENOENT"));
    assert.strictEqual(failure.kind, "missing_binary");
    assert.strictEqual(failure.message, "Required runtime/debug binary was not found.");
    assert.strictEqual(failure.detail, "spawn trust-runtime ENOENT");
  });

  // OPC UA client test failures render user-facing recovery text, not raw backend tokens: the Rust
  // test `opcua_test_failures_have_recovery_text` (crates/trust-devices/src/actions/tests/config.rs).

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

  test("conditional schema fields render only for the selected backend", () => {
    const gpioSchema: CommProtocolSchema = {
      id: "gpio",
      driver: "Gpio",
      title: "GPIO",
      purpose: "Configure GPIO lines.",
      lifecycle_effect: "restart_required",
      supports_test: false,
      supports_multi_instance: true,
      actions: ["add", "upsert"],
      fields: [
        {
          id: "backend",
          label: "Backend",
          type: "enum",
          required: true,
          advanced: false,
          secret: false,
          help: "Backend.",
          default: "libgpiod",
          options: ["libgpiod", "sysfs"],
        },
        {
          id: "chip",
          label: "GPIO chip",
          type: "path",
          required: true,
          advanced: false,
          secret: false,
          help: "libgpiod chip.",
          default: "/dev/gpiochip0",
          visible_when: { field: "backend", equals: "libgpiod" },
        },
        {
          id: "sysfs_base",
          label: "Sysfs base",
          type: "path",
          required: true,
          advanced: false,
          secret: false,
          help: "sysfs root.",
          default: "/sys/class/gpio",
          visible_when: { field: "backend", equals: "sysfs" },
        },
      ],
    };

    assert.deepStrictEqual(
      visibleSchemaFields(gpioSchema, { backend: "libgpiod" }).map((field) => field.id),
      ["backend", "chip"]
    );
    assert.deepStrictEqual(
      visibleSchemaFields(gpioSchema, { backend: "sysfs" }).map((field) => field.id),
      ["backend", "sysfs_base"]
    );
    // On submit the server evaluates the predicates again and saves only the shown fields: the Rust
    // tests `a_saved_form_writes_only_the_fields_its_backend_shows` (crates/trust-devices/src/canvas)
    // and `hidden_conditional_fields_are_not_submitted`.
    const inspectorSource = fs.readFileSync(
      path.join(__dirname, "..", "..", "..", "src", "networkCanvas", "webview", "NodeInspector.tsx"),
      "utf8"
    );
    assert.ok(
      inspectorSource.includes("visibleSchemaFields(protoSchema, values)") &&
        inspectorSource.includes("visibleFields.map((field)"),
      "endpoint edits must render only visible conditional fields"
    );
  });
});

// A runtime's `fleet.topology` answer of schema version 3 (links with IDs and roles); the server
// reads versions 2 to 4 (specification 25 §5.6).
function fleetTopology(): FleetTopologyResponse {
  return {
    schema_version: 3,
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
              {
                id: "endpoint:runtime-a:mesh",
                kind: "peer",
                protocol: "mesh",
                name: "Mesh / Zenoh",
                role: "peer",
                health: "degraded",
                detail: "One configured peer.",
                owned: true,
                supports_test: false,
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
        id: "link:mesh:peer",
        from: "endpoint:runtime-a:mesh",
        to: "external:runtime-a:mesh:0",
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
        id: "external:runtime-a:mesh:0",
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
suite("Network Canvas — the endpoint a live runtime answered at", function () {
  // Specification 25, section 5.1. A runtime's own topology names no control endpoint (it does not know the address it
  // was reached at), so its node had Connect disabled and "Set as run target" selected the Simulator (found while fixing
  // acceptance finding F20). The language server gives the answering runtime its endpoint; these tests prove that the
  // canvas draws it.
  this.timeout(60000);
  suiteSetup(languageServerReady);

  const runtime = (runtime_id: string, source: string, control_endpoint?: string) => ({
    runtime_id, name: runtime_id, mode: "online", cycle_ms: 100, health: "connected", detail: "", endpoints: [], source,
    ...(control_endpoint ? { control_endpoint } : {}),
  });
  const topology: FleetTopologyResponse = {
    schema_version: 4,
    hosts: [{
      host_id: "host:cell", hostname: "cell", arch: "x86_64", os: "linux", ips: [],
      runtimes: [runtime("runtime:self", "self"), runtime("runtime:peer", "mesh", "tcp://10.0.0.9:9100")],
      containers: [],
    }],
    links: [], shared: [], external: [],
  };

  test("the runtime that answered gets the endpoint it answered at; a peer keeps its own", async () => {
    const { graph } = await served(topology, { endpoint: "tcp://127.0.0.1:9100" });
    const byName = new Map(graphRuntimes(graph).map((item) => [item.name, item]));
    assert.strictEqual(byName.get("runtime:self")?.controlEndpoint, "tcp://127.0.0.1:9100");
    assert.strictEqual(byName.get("runtime:peer")?.controlEndpoint, "tcp://10.0.0.9:9100", "a mesh peer names its own");
    assert.strictEqual(topology.hosts[0].runtimes[0].control_endpoint, undefined, "the answer itself is not changed");
  });

  test("the extension's own simulator gets no endpoint, so its node stays the Simulator with Start and Stop", async () => {
    const { graph } = await served(topology, { simulator: true });
    const simulator = graphRuntimes(graph).find((item) => item.name === "Simulator");
    assert.ok(simulator, "the simulator's node is named Simulator");
    assert.strictEqual(simulator?.controlEndpoint, undefined);
  });
});

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
    assert.strictEqual(protocolName("simulated"), "Simulated I/O");
    assert.strictEqual(protocolName("loopback"), "Loopback I/O");
  });

  test("ADD_PICKER_GROUPS is the canonical S-09 group order", () => {
    assert.deepStrictEqual(ADD_PICKER_GROUPS.map((c) => c.key), [
      "simulation",
      "devices_io",
      "read_tags",
      "share_values",
      "messages",
      "advanced",
    ]);
  });
});

// The expose plan's rules (topology-only evidence dropped, exposure merged, reviewed writable names
// added) are the Rust test `expose_merges_into_the_owning_server_and_keeps_clients`
// (crates/trust-devices/src/actions/tests/plans.rs).
