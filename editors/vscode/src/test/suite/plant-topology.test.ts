import * as assert from "assert";

import {
  attachPlantsToRuntime,
  projectPlantRuntimeId,
} from "../../networkCanvas/graphData";
import { buildGraph } from "../../networkCanvas/webview/layout";
import {
  applyFilter,
  protocolsInGraph,
} from "../../networkCanvas/webview/filter";
import type { NCGraph } from "../../networkCanvas/webview/types";

suite("Plant topology projection", () => {
  test("projects a configured plant through the shared runtime-child brick", () => {
    const graph: NCGraph = {
      kind: "graph",
      title: "Devices & Connections",
      summary: "1 host · 1 runtime · 1 simulated plant",
      hosts: [
        {
          id: "host:local",
          hostname: "Development computer",
          label: "Local project",
          health: "connected",
          containers: [],
          runtimes: [
            {
              id: "runtime:local",
              name: "Simulator",
              mode: "simulation",
              health: "stopped",
              detail: "Project runtime",
              endpoints: [],
              plants: [
                {
                  id: "tank_level",
                  displayName: "Tank level",
                  model: "first_order_dead_time",
                  enabled: true,
                  commandSources: ["%QL0"],
                  feedbackTargets: ["%IL0"],
                  statusAvailability: "not_requested",
                  savedConfigurationRevision: "sha256:saved",
                },
              ],
            },
          ],
        },
      ],
      links: [],
      external: [],
      faults: [],
    };

    const rendered = buildGraph(graph);
    const plant = rendered.nodes.find((node) => node.id === "plant:runtime:local:tank_level");

    assert.ok(plant, "the configured plant must be projected into the canvas");
    assert.strictEqual(
      plant.type,
      "endpoint",
      "a plant must reuse the existing runtime-child brick instead of a parallel node type"
    );
    assert.strictEqual(plant.data.kind, "plant");
    assert.strictEqual(plant.parentId, "runtime:local");
    assert.strictEqual(plant.data.role, "SIMULATED PLANT");
    assert.strictEqual(plant.data.state, "Configured");
    assert.deepStrictEqual(rendered.edges, [], "containment must not invent a plant wire");
  });

  test("keeps plant identity content mappings and saved-active revisions separate", () => {
    const graph = {
      kind: "graph",
      title: "Devices & Connections",
      summary: "",
      hosts: [
        {
          id: "host:local",
          hostname: "Development computer",
          label: "Local project",
          health: "connected",
          containers: [],
          runtimes: [
            {
              id: "runtime:local",
              name: "Simulator",
              mode: "simulation",
              health: "connected",
              detail: "Project runtime",
              endpoints: [],
              plants: [
                {
                  id: "tank_level",
                  displayName: "Tank level",
                  model: "first_order_dead_time",
                  startingPointName: "Temperature or slow process",
                  enabled: true,
                  commandSources: ["%QL0"],
                  feedbackTargets: ["%IL0"],
                  statusAvailability: "available",
                  activeOperatingState: "running",
                  hasCompletedSample: true,
                  savedConfigurationRevision: "sha256:saved",
                  activeConfigurationRevision: "sha256:active",
                },
              ],
            },
          ],
        },
      ],
      links: [],
      external: [],
      faults: [],
    } as unknown as NCGraph;

    const plant = buildGraph(graph).nodes.find(
      (node) => node.id === "plant:runtime:local:tank_level"
    );
    assert.ok(plant);
    assert.strictEqual(plant.data.plantId, "tank_level");
    assert.strictEqual(plant.data.displayName, "Tank level");
    assert.strictEqual(plant.data.modelKind, "first_order_dead_time");
    assert.strictEqual(plant.data.modelName, "First order with dead time");
    assert.strictEqual(plant.data.startingPointName, "Temperature or slow process");
    assert.deepStrictEqual(plant.data.commandSources, ["%QL0"]);
    assert.deepStrictEqual(plant.data.feedbackTargets, ["%IL0"]);
    assert.strictEqual(plant.data.enabled, true);
    assert.strictEqual(plant.data.savedConfigurationRevision, "sha256:saved");
    assert.strictEqual(plant.data.activeConfigurationRevision, "sha256:active");
  });

  test("derives every specified primary state from saved and current runtime evidence", () => {
    const cases: Array<{
      id: string;
      draft?: boolean;
      enabled?: boolean;
      statusAvailability: string;
      activeOperatingState?: string;
      hasCompletedSample?: boolean;
      activeConfigurationRevision?: string;
      expected: string;
    }> = [
      { id: "draft", draft: true, statusAvailability: "not_requested", expected: "Draft" },
      { id: "configured", statusAvailability: "not_requested", expected: "Configured" },
      { id: "disabled", enabled: false, statusAvailability: "not_requested", expected: "Disabled" },
      {
        id: "waiting",
        statusAvailability: "available",
        activeOperatingState: "running",
        hasCompletedSample: false,
        expected: "Waiting for samples",
      },
      {
        id: "running",
        statusAvailability: "available",
        activeOperatingState: "running",
        hasCompletedSample: true,
        expected: "Running",
      },
      {
        id: "restart",
        statusAvailability: "available",
        activeOperatingState: "running",
        hasCompletedSample: true,
        activeConfigurationRevision: "sha256:active",
        expected: "Restart required",
      },
      {
        id: "faulted",
        statusAvailability: "available",
        activeOperatingState: "faulted",
        hasCompletedSample: true,
        expected: "Faulted",
      },
      { id: "unavailable", statusAvailability: "unavailable", expected: "Status unavailable" },
    ];
    const graph = {
      kind: "graph",
      title: "Devices & Connections",
      summary: "",
      hosts: [
        {
          id: "host:local",
          hostname: "Development computer",
          label: "Local project",
          health: "connected",
          containers: [],
          runtimes: [
            {
              id: "runtime:local",
              name: "Simulator",
              mode: "simulation",
              health: "connected",
              detail: "Project runtime",
              endpoints: [],
              plants: cases.map((entry) => ({
                ...entry,
                id: entry.id,
                displayName: entry.id,
                model: "first_order_dead_time",
                enabled: entry.enabled ?? true,
                commandSources: ["%QL0"],
                feedbackTargets: ["%IL0"],
                savedConfigurationRevision: "sha256:saved",
              })),
            },
          ],
        },
      ],
      links: [],
      external: [],
      faults: [],
    } as unknown as NCGraph;

    const rendered = buildGraph(graph);
    for (const entry of cases) {
      const node = rendered.nodes.find(
        (candidate) => candidate.id === (entry.draft
          ? entry.id
          : `plant:runtime:local:${entry.id}`)
      );
      assert.strictEqual(node?.data.state, entry.expected, entry.id);
    }
  });

  test("keeps lower-priority simultaneous conditions as inspector details", () => {
    const graph = {
      kind: "graph",
      title: "Devices & Connections",
      summary: "",
      hosts: [
        {
          id: "host:local",
          hostname: "Development computer",
          label: "Local project",
          health: "connected",
          containers: [],
          runtimes: [
            {
              id: "runtime:local",
              name: "Simulator",
              mode: "simulation",
              health: "connected",
              detail: "Project runtime",
              endpoints: [],
              plants: [
                {
                  id: "faulted_old_revision",
                  displayName: "Faulted old revision",
                  model: "first_order_dead_time",
                  enabled: false,
                  commandSources: ["%QL0"],
                  feedbackTargets: ["%IL0"],
                  statusAvailability: "available",
                  activeOperatingState: "faulted",
                  hasCompletedSample: true,
                  savedConfigurationRevision: "sha256:saved",
                  activeConfigurationRevision: "sha256:active",
                },
              ],
            },
          ],
        },
      ],
      links: [],
      external: [],
      faults: [],
    } as NCGraph;

    const plant = buildGraph(graph).nodes.find(
      (node) => node.id === "plant:runtime:local:faulted_old_revision"
    );
    assert.strictEqual(plant?.data.state, "Faulted");
    assert.deepStrictEqual(plant?.data.secondaryStates, ["Restart required", "Disabled"]);
  });

  test("admits live values only from current available runtime evidence", () => {
    const staleSample = { command: [75], feedback: [61.5] };
    const graph = {
      kind: "graph",
      title: "Devices & Connections",
      summary: "",
      hosts: [
        {
          id: "host:local",
          hostname: "Development computer",
          label: "Local project",
          health: "connected",
          containers: [],
          runtimes: [
            {
              id: "runtime:local",
              name: "Simulator",
              mode: "simulation",
              health: "connected",
              detail: "Project runtime",
              endpoints: [],
              plants: [
                {
                  id: "offline",
                  statusAvailability: "not_requested",
                  activeOperatingState: "running",
                  hasCompletedSample: true,
                  liveSample: staleSample,
                },
                {
                  id: "unavailable",
                  statusAvailability: "unavailable",
                  activeOperatingState: "running",
                  hasCompletedSample: true,
                  liveSample: staleSample,
                },
                {
                  id: "current",
                  statusAvailability: "available",
                  activeOperatingState: "running",
                  hasCompletedSample: true,
                  liveSample: staleSample,
                },
              ].map((plant) => ({
                displayName: plant.id,
                model: "first_order_dead_time",
                enabled: true,
                commandSources: ["%QL0"],
                feedbackTargets: ["%IL0"],
                savedConfigurationRevision: "sha256:saved",
                activeConfigurationRevision: "sha256:saved",
                ...plant,
              })),
            },
          ],
        },
      ],
      links: [],
      external: [],
      faults: [],
    } as unknown as NCGraph;

    const plants = buildGraph(graph).nodes.filter((node) => node.data.kind === "plant");
    const data = (id: string) => plants.find((node) => node.id.endsWith(`:${id}`))?.data;

    assert.strictEqual(data("offline")?.state, "Configured");
    assert.strictEqual(data("offline")?.liveSample, undefined);
    assert.strictEqual(data("unavailable")?.state, "Status unavailable");
    assert.strictEqual(data("unavailable")?.liveSample, undefined);
    assert.strictEqual(data("current")?.state, "Running");
    assert.deepStrictEqual(data("current")?.liveSample, staleSample);
  });

  test("counts plants separately and names the truly empty runtime", () => {
    const graphData = require("../../networkCanvas/graphData") as {
      canvasGraphSummary?: (graph: NCGraph) => string;
      runtimeEmptyMessage?: () => string;
    };
    const graph = {
      kind: "graph",
      title: "Devices & Connections",
      summary: "",
      hosts: [
        {
          id: "host:local",
          hostname: "Development computer",
          label: "Local project",
          health: "connected",
          containers: [],
          runtimes: [
            {
              id: "runtime:local",
              name: "Simulator",
              mode: "simulation",
              health: "connected",
              detail: "Project runtime",
              endpoints: [
                { id: "io:one", kind: "field", protocol: "simulated", name: "I/O one", health: "connected", detail: "", role: "owned_driver" },
                { id: "io:two", kind: "field", protocol: "ads", name: "I/O two", health: "connected", detail: "", role: "client" },
              ],
              plants: [
                {
                  id: "tank",
                  displayName: "Tank",
                  model: "first_order_dead_time",
                  enabled: true,
                  commandSources: ["%QL0"],
                  feedbackTargets: ["%IL0"],
                  statusAvailability: "not_requested",
                  savedConfigurationRevision: "sha256:saved",
                },
              ],
            },
          ],
        },
      ],
      links: [],
      external: [],
      faults: [],
    } as NCGraph;

    assert.deepStrictEqual(
      {
        summary: graphData.canvasGraphSummary?.(graph),
        empty: graphData.runtimeEmptyMessage?.(),
      },
      {
        summary: "1 host · 1 runtime · 2 devices · 1 simulated plant",
        empty: "No devices or simulated plants yet",
      }
    );
  });

  test("searches every plant identity field and filters plants without hiding faults", () => {
    const graph = {
      kind: "graph",
      title: "Devices & Connections",
      summary: "",
      hosts: [
        {
          id: "host:local",
          hostname: "Development computer",
          label: "Local project",
          health: "connected",
          containers: [],
          runtimes: [
            {
              id: "runtime:local",
              name: "Simulator",
              mode: "simulation",
              health: "connected",
              detail: "Project runtime",
              endpoints: [],
              plants: [
                {
                  id: "tank_level",
                  displayName: "Tank level",
                  model: "first_order_dead_time",
                  startingPointName: "Temperature or slow process",
                  mappedSymbols: ["Program.Heater", "Program.Temperature"],
                  enabled: true,
                  commandSources: ["%QL0"],
                  feedbackTargets: ["%IL0"],
                  statusAvailability: "available",
                  activeOperatingState: "running",
                  hasCompletedSample: true,
                  savedConfigurationRevision: "sha256:saved",
                  activeConfigurationRevision: "sha256:saved",
                },
                {
                  id: "faulted_motor",
                  displayName: "Motor speed",
                  model: "first_order",
                  enabled: true,
                  commandSources: ["%QL8"],
                  feedbackTargets: ["%IL8"],
                  statusAvailability: "available",
                  activeOperatingState: "faulted",
                  hasCompletedSample: true,
                  savedConfigurationRevision: "sha256:saved",
                  activeConfigurationRevision: "sha256:saved",
                },
              ],
            },
          ],
        },
      ],
      links: [],
      external: [],
      faults: [],
    } as unknown as NCGraph;

    for (const query of [
      "Tank level",
      "tank_level",
      "First order with dead time",
      "Temperature or slow process",
      "Program.Heater",
      "%QL0",
      "Running",
    ]) {
      const rendered = buildGraph({ ...graph, searchQuery: query });
      const tank = rendered.nodes.find((node) => node.id.endsWith(":tank_level"));
      const motor = rendered.nodes.find((node) => node.id.endsWith(":faulted_motor"));
      assert.strictEqual(tank?.data.dimmed, false, query);
      assert.strictEqual(motor?.data.dimmed, true, query);
    }

    assert.ok(protocolsInGraph(graph).includes("simulated_plants"));
    const filtered = applyFilter(graph, new Set(["simulated_plants"]));
    const remaining = filtered.hosts[0]?.runtimes[0]?.plants ?? [];
    assert.deepStrictEqual(remaining.map((plant) => plant.id), ["faulted_motor"]);
  });

  test("rejects direct plant-to-plant wires even when supplied by malformed topology", () => {
    const plant = (id: string) => ({
      id,
      displayName: id,
      model: "first_order_dead_time",
      enabled: true,
      commandSources: ["%QL0"],
      feedbackTargets: ["%IL0"],
      statusAvailability: "not_requested" as const,
      savedConfigurationRevision: "sha256:saved",
    });
    const graph: NCGraph = {
      kind: "graph",
      title: "Devices & Connections",
      summary: "",
      hosts: [
        {
          id: "host:local",
          hostname: "Development computer",
          label: "Local project",
          health: "connected",
          containers: [],
          runtimes: [
            {
              id: "runtime:local",
              name: "Simulator",
              mode: "simulation",
              health: "connected",
              detail: "Project runtime",
              endpoints: [],
              plants: [plant("one"), plant("two")],
            },
          ],
        },
      ],
      links: [
        {
          id: "invalid:plant-wire",
          from: "plant:runtime:local:one",
          to: "plant:runtime:local:two",
          protocol: "simulated",
          role: "invalid",
          status: "connected",
          secure: true,
        },
      ],
      external: [],
      faults: [],
    };

    assert.deepStrictEqual(buildGraph(graph).edges, []);
  });

  test("attaches saved plants only to their owning runtime and refreshes the summary", () => {
    const runtime = (id: string) => ({
      id,
      name: id,
      mode: "simulation",
      health: "stopped",
      detail: "",
      endpoints: [],
    });
    const graph: NCGraph = {
      kind: "graph",
      title: "Devices & Connections",
      summary: "stale",
      hosts: [
        {
          id: "host:local",
          hostname: "This computer",
          label: "Local",
          health: "connected",
          containers: [],
          runtimes: [runtime("runtime:local"), runtime("runtime:peer")],
        },
      ],
      links: [],
      external: [],
      faults: [],
    };
    const plants = [
      {
        id: "tank",
        displayName: "Tank",
        model: "first_order",
        enabled: true,
        commandSources: ["%QW0"],
        feedbackTargets: ["%IW0"],
        statusAvailability: "unavailable" as const,
      },
    ];

    attachPlantsToRuntime(graph, "runtime:local", plants);

    assert.strictEqual(graph.hosts[0].runtimes[0].plants?.[0]?.id, "tank");
    assert.strictEqual(graph.hosts[0].runtimes[1].plants, undefined);
    assert.strictEqual(graph.summary, "1 host · 2 runtimes · 0 devices · 1 simulated plant");
  });

  test("resolves the sole project runtime without assuming its graph identifier", () => {
    const graph: NCGraph = {
      kind: "graph",
      title: "Devices & Connections",
      summary: "",
      hosts: [{
        id: "host:project",
        hostname: "This computer",
        label: "Project",
        health: "connected",
        containers: [],
        runtimes: [{
          id: "runtime:offline-topology:9f31",
          name: "Simulator",
          mode: "simulation",
          health: "stopped",
          detail: "",
          endpoints: [],
        }],
      }],
      links: [],
      external: [],
      faults: [],
    };

    assert.strictEqual(projectPlantRuntimeId(graph), "runtime:offline-topology:9f31");
  });
});
