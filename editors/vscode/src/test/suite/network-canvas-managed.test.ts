import {
  assert,
  fs,
  os,
  path,
  buildNetworkCanvasModel,
  mergeFleetTopologies,
  buildCanvasGraph,
  buildGraph,
  runtimeNodeControls,
  commTestMessage,
  validateSchemaValues,
  visibleSchemaFields,
  RUNNING,
} from "./network-canvas-fixtures";
import type {
  FleetTopologyResponse,
  CommProtocolSchema,
} from "./network-canvas-fixtures";

suite("Network Canvas", function () {


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
  test("selected run target is projected onto the graph node and rendered node data", () => {
    const graph = buildCanvasGraph(
      buildNetworkCanvasModel("runtime_live"),
      undefined,
      undefined,
      undefined,
      [
        { name: "cell1", controlEndpoint: "tcp://127.0.0.1:9902", state: "stopped" },
        { name: "cell2", controlEndpoint: "tcp://127.0.0.1:9903", state: "running" },
      ],
      "cell1"
    );
    const cell1 = graph.hosts[0].runtimes.find((runtime) => runtime.managedName === "cell1");
    const cell2 = graph.hosts[0].runtimes.find((runtime) => runtime.managedName === "cell2");
    const simulator = graph.hosts[0].runtimes.find((runtime) => runtime.id === "runtime:local");

    assert.strictEqual(cell1?.runTarget, true, "the selected managed runtime gets a run-target flag");
    assert.strictEqual(cell2?.runTarget, false, "non-selected managed runtimes are not flagged");
    assert.strictEqual(simulator?.runTarget, false, "the simulator is not flagged when a managed runtime is selected");

    const rendered = buildGraph(graph).nodes.find((node) => node.type === "runtime" && node.id === cell1?.id);
    assert.strictEqual(rendered?.data.runTarget, true, "React Flow runtime node data carries the flag");
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



  // --- Failure classification ----------------------------------------------
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
  test("conditional schema fields render and validate only for the selected backend", () => {
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
    assert.deepStrictEqual(
      validateSchemaValues(gpioSchema, { backend: "libgpiod", chip: "/dev/gpiochip0" }),
      [],
      "hidden sysfs_base must not block libgpiod validation"
    );
    assert.deepStrictEqual(
      validateSchemaValues(gpioSchema, { backend: "sysfs", sysfs_base: "/sys/class/gpio" }),
      [],
      "hidden chip must not block sysfs validation"
    );
    const schemaFieldsSource = fs.readFileSync(
      path.join(__dirname, "..", "..", "..", "src", "networkCanvas", "webview", "SchemaFields.tsx"),
      "utf8"
    );
    const inspectorSource = fs.readFileSync(
      path.join(__dirname, "..", "..", "..", "src", "networkCanvas", "webview", "NodeInspector.tsx"),
      "utf8"
    );
    assert.ok(
      schemaFieldsSource.includes("visibleSchemaFields(protocol, values)"),
      "endpoint edits must omit hidden conditional fields when building params"
    );
    assert.ok(
      inspectorSource.includes("visibleSchemaFields(protoSchema, values)") &&
        inspectorSource.includes("visibleFields.map((field)"),
      "endpoint edits must render only visible conditional fields"
    );
  });
});
