import * as assert from "assert";
import { EventEmitter } from "events";

import type { PlantContractMessage } from "../../networkCanvas/plant/contract";
import {
  loadPlantTopology,
  normalizePlantTopology,
} from "../../networkCanvas/plant/projection";
import {
  createPlantSessionRegistry,
  type PlantCommandClient,
  PlantSessionRegistry,
  populatePlantTopology,
  ProjectPlantTopologyService,
} from "../../networkCanvas/plant/service";
import type { PlantCommandProcess } from "../../networkCanvas/plant/offlineSession";
import * as plantServiceApi from "../../networkCanvas/plant/service";
import { createPlantEditDraft } from "../../networkCanvas/plant/addFlow";

suite("Plant service projection", () => {
  test("reports whether a saved plant is active or still needs a runtime restart", () => {
    const label = (plantServiceApi as any).plantSaveResultLabel;
    assert.strictEqual(
      label?.({ restart_required: false }),
      "Saved"
    );
    assert.strictEqual(
      label?.({ restart_required: true }),
      "Saved · restart required"
    );
  });

  test("preserves only the saved plant identities needed for Control Studio navigation", () => {
    const read = {
      contract: "trust.plant",
      schema_version: 1,
      operation: "plant.read",
      project_id: "project:sha256:handoff",
      configuration_revision: "sha256:saved",
      active_configuration_revision: "sha256:active",
      plants: [{
        id: "tank",
        display_name: "Training tank",
        model: "first_order",
        enabled: true,
        command_sources: ["%QW0"],
        feedback_targets: ["%IW0"],
      }],
    } as PlantContractMessage;

    const plant = normalizePlantTopology(read)[0] as any;
    assert.deepStrictEqual({
      projectId: plant.projectId,
      savedConfigurationRevision: plant.savedConfigurationRevision,
      activeConfigurationRevision: plant.activeConfigurationRevision,
    }, {
      projectId: "project:sha256:handoff",
      savedConfigurationRevision: "sha256:saved",
      activeConfigurationRevision: "sha256:active",
    });
  });

  test("keeps navigation identities when a saved plant is reopened for editing", () => {
    const draft = createPlantEditDraft({
      id: "plant:runtime:local:tank",
      projectId: "project:sha256:handoff",
      runtimeId: "runtime:local",
      runtimeName: "Simulator",
      plantId: "tank",
      displayName: "Training tank",
      description: "",
      enabled: true,
      model: "first_order",
      parameters: { gain: 1, time_constant_s: 5 },
      initialState: [0],
      commandSources: ["%QW0"],
      feedbackTargets: ["%IW0"],
      savedConfigurationRevision: "sha256:saved",
      activeConfigurationDigest: "sha256:active",
    } as any) as any;

    assert.deepStrictEqual({
      projectId: draft.projectId,
      savedConfigurationRevision: draft.savedConfigurationRevision,
      activeConfigurationDigest: draft.activeConfigurationDigest,
    }, {
      projectId: "project:sha256:handoff",
      savedConfigurationRevision: "sha256:saved",
      activeConfigurationDigest: "sha256:active",
    });
  });

  test("carries the saved model values needed to reopen the same plant editor", () => {
    const read = {
      contract: "trust.plant",
      schema_version: 1,
      operation: "plant.read",
      project_id: "project:sha256:edit",
      configuration_revision: "sha256:saved",
      saved_configuration: {
        plants: [{
          id: "tank",
          name: "Training tank",
          description: "Editable",
          model: "first_order",
          enabled: true,
          command_sources: ["%QW0"],
          feedback_targets: ["%IW0"],
          step_ms: 5,
          initial_input: [2],
          input_scale: [0.1],
          input_offset: [-1],
          output_scale: [10],
          output_offset: [4],
          parameters: { gain: 2, time_constant_s: 5 },
          initial_state: [1],
          actuator: { minimum: 0, maximum: 100, lag_s: 0.2 },
        }],
      },
      restart_required: false,
      plants: [{
        id: "tank",
        display_name: "Training tank",
        description: "Editable",
        model: "first_order",
        enabled: true,
        command_sources: ["%QW0"],
        feedback_targets: ["%IW0"],
        saved_order: 0,
      }],
    } as PlantContractMessage;

    const plant = normalizePlantTopology(read)[0] as unknown as {
      configuration?: Record<string, unknown>;
    };
    assert.deepStrictEqual(plant?.configuration, {
      description: "Editable",
      parameters: { gain: 2, time_constant_s: 5 },
      initialState: [1],
      preservedConfiguration: (read.saved_configuration as { plants: unknown[] }).plants[0],
      stepMs: 5,
      initialInput: [2],
      inputScale: [0.1],
      inputOffset: [-1],
      outputScale: [10],
      outputOffset: [4],
      actuator: { minimum: 0, maximum: 100, lag_s: 0.2 },
      scenarioEvents: [],
    });
  });

  test("validates then saves one plant through the existing project session", async () => {
    const requests: Array<{ operation: string; params: Record<string, unknown> }> = [];
    const candidate = {
      id: "tank",
      name: "Training tank",
      model: "first_order",
      enabled: true,
      command_sources: ["%QW0"],
      feedback_targets: ["%IW0"],
      parameters: { gain: 1, time_constant_s: 5 },
      initial_state: [0],
    };
    const client = {
      async request(operation: string, params: Record<string, unknown>) {
        requests.push({ operation, params });
        if (operation === "plant.read") {
          return {
            contract: "trust.plant",
            schema_version: 1,
            operation,
            project_id: "project:sha256:save",
            configuration_revision: "sha256:before",
          } as PlantContractMessage;
        }
        if (operation === "plant.validate") {
          return {
            contract: "trust.plant",
            schema_version: 1,
            operation,
            project_id: "project:sha256:save",
            valid: true,
            issues: [],
            candidate_revision: "sha256:candidate",
          } as PlantContractMessage;
        }
        return {
          contract: "trust.plant",
          schema_version: 1,
          operation,
          project_id: "project:sha256:save",
          operation_id: "save-1",
          configuration_revision: "sha256:after",
          plants: [],
          affected_plant_ids: ["tank"],
          restart_required: false,
          plant_count: 1,
        } as PlantContractMessage;
      },
    };
    const savePlantCandidate = (
      plantServiceApi as typeof plantServiceApi & {
        savePlantCandidate?: (
          client: PlantCommandClient,
          plant: Record<string, unknown>,
          operationId: string,
          expectedRevision?: string
        ) => Promise<PlantContractMessage>;
      }
    ).savePlantCandidate;

    assert.strictEqual(typeof savePlantCandidate, "function");
    if (!savePlantCandidate) return;
    const saved = await savePlantCandidate(
      client,
      candidate,
      "save-1",
      "sha256:drawer-opened"
    );
    assert.strictEqual(saved.operation, "plant.save");
    assert.deepStrictEqual(requests, [
      {
        operation: "plant.read",
        params: { contract: "trust.plant", schema_version: 1 },
      },
      {
        operation: "plant.validate",
        params: {
          contract: "trust.plant",
          schema_version: 1,
          project_id: "project:sha256:save",
          plant: candidate,
        },
      },
      {
        operation: "plant.save",
        params: {
          contract: "trust.plant",
          schema_version: 1,
          project_id: "project:sha256:save",
          operation_id: "save-1",
          expected_revision: "sha256:drawer-opened",
          plant: candidate,
        },
      },
    ]);
  });

  test("removes one saved plant through the existing project session", async () => {
    const requests: Array<{ operation: string; params: Record<string, unknown> }> = [];
    const client: PlantCommandClient = {
      async request(operation, params) {
        requests.push({ operation, params });
        if (operation === "plant.read") {
          return {
            contract: "trust.plant",
            schema_version: 1,
            operation,
            project_id: "project:sha256:remove",
            configuration_revision: "sha256:current",
          } as PlantContractMessage;
        }
        return {
          contract: "trust.plant",
          schema_version: 1,
          operation,
          project_id: "project:sha256:remove",
          operation_id: "remove-1",
          configuration_revision: "sha256:after",
          plants: [],
          affected_plant_ids: ["tank"],
          restart_required: false,
          plant_count: 0,
        } as PlantContractMessage;
      },
    };
    const removePlantCandidate = (
      plantServiceApi as typeof plantServiceApi & {
        removePlantCandidate?: (
          client: PlantCommandClient,
          plantId: string,
          operationId: string,
          expectedRevision?: string
        ) => Promise<PlantContractMessage>;
      }
    ).removePlantCandidate;

    assert.strictEqual(typeof removePlantCandidate, "function");
    if (!removePlantCandidate) return;
    const removed = await removePlantCandidate(
      client,
      "tank",
      "remove-1",
      "sha256:drawer-opened"
    );
    assert.strictEqual(removed.operation, "plant.remove");
    assert.deepStrictEqual(requests, [
      {
        operation: "plant.read",
        params: { contract: "trust.plant", schema_version: 1 },
      },
      {
        operation: "plant.remove",
        params: {
          contract: "trust.plant",
          schema_version: 1,
          project_id: "project:sha256:remove",
          operation_id: "remove-1",
          expected_revision: "sha256:drawer-opened",
          plant_id: "tank",
        },
      },
    ]);
  });

  test("joins current status only to matching saved plant identity", () => {
    const read = {
      contract: "trust.plant",
      schema_version: 1,
      operation: "plant.read",
      project_id: "project:sha256:abc",
      configuration_revision: "sha256:saved",
      active_configuration_revision: "sha256:active",
      saved_configuration: {},
      restart_required: true,
      plants: [
        {
          id: "tank",
          display_name: "Tank level",
          description: "Training tank",
          model: "first_order_dead_time",
          enabled: true,
          command_sources: ["%QL0"],
          feedback_targets: ["%IL0"],
          saved_order: 0,
        },
      ],
    } as PlantContractMessage;
    const status = {
      contract: "trust.plant",
      schema_version: 1,
      operation: "plant.status",
      project_id: "project:sha256:abc",
      available: true,
      active_configuration_revision: "sha256:active",
      simulation_time_ns: "2000000",
      effective_seed: "42",
      random_algorithm: "chacha8",
      random_algorithm_version: 1,
      runtime_version: "0.24.64",
      time_scale: 1,
      restart_policy: "manual",
      plants: [
        {
          id: "tank",
          model: "first_order_dead_time",
          operating_state: "running",
          enabled: true,
          step_ns: "1000000",
          simulation_time_ns: "2000000",
          state: [1.25],
          command: [2],
          applied_input: [1.8],
          ideal_output: [1.4],
          measured_feedback: [1.38],
          published_feedback: [1.38],
          actuators: [],
          active_uncertainties: [],
          active_dropouts: [],
          pending_event_count: 0,
          delay_buffer_depth: 2,
          accumulated_remainder_ns: "0",
          last_substeps: 1,
          current_fault: null,
        },
        {
          id: "runtime_only",
          model: "static_gain",
          operating_state: "running",
          enabled: true,
          step_ns: "1000000",
          simulation_time_ns: "2000000",
          state: [3],
          command: [3],
          applied_input: [3],
          ideal_output: [3],
          measured_feedback: [3],
          published_feedback: [3],
          actuators: [],
          active_uncertainties: [],
          active_dropouts: [],
          pending_event_count: 0,
          delay_buffer_depth: 0,
          accumulated_remainder_ns: "0",
          last_substeps: 1,
          current_fault: null,
        },
      ],
    } as PlantContractMessage;

    assert.deepStrictEqual(normalizePlantTopology(read, status), [
      {
        id: "tank",
        projectId: "project:sha256:abc",
        displayName: "Tank level",
        model: "first_order_dead_time",
        enabled: true,
        commandSources: ["%QL0"],
        feedbackTargets: ["%IL0"],
        statusAvailability: "available",
        activeOperatingState: "running",
        hasCompletedSample: true,
        liveSample: { command: [2], feedback: [1.38] },
        savedConfigurationRevision: "sha256:saved",
        activeConfigurationRevision: "sha256:active",
      },
    ]);
  });

  test("establishes project identity from saved topology before requesting status", async () => {
    const calls: string[] = [];
    const read = {
      contract: "trust.plant",
      schema_version: 1,
      operation: "plant.read",
      project_id: "project:sha256:ordered",
      configuration_revision: "sha256:saved",
      saved_configuration: {},
      restart_required: false,
      plants: [],
    } as PlantContractMessage;
    const status = {
      contract: "trust.plant",
      schema_version: 1,
      operation: "plant.status",
      project_id: read.project_id,
      available: true,
      active_configuration_revision: "sha256:saved",
      simulation_time_ns: "0",
      effective_seed: "1",
      random_algorithm: "chacha8",
      random_algorithm_version: 1,
      runtime_version: "0.24.64",
      time_scale: 1,
      restart_policy: "manual",
      plants: [],
    } as PlantContractMessage;

    await loadPlantTopology({
      async read() {
        calls.push("read");
        return read;
      },
      async status(projectId) {
        calls.push(`status:${projectId}`);
        return status;
      },
    });

    assert.deepStrictEqual(calls, ["read", "status:project:sha256:ordered"]);
  });

  test("keeps saved topology but clears live evidence when status is unavailable", async () => {
    const read = {
      contract: "trust.plant",
      schema_version: 1,
      operation: "plant.read",
      project_id: "project:sha256:offline",
      configuration_revision: "sha256:saved",
      saved_configuration: {},
      restart_required: false,
      plants: [
        {
          id: "tank",
          display_name: "Tank",
          description: null,
          model: "first_order",
          enabled: true,
          command_sources: ["%QW0"],
          feedback_targets: ["%IW0"],
          saved_order: 0,
        },
      ],
    } as PlantContractMessage;
    let rejected: unknown;
    let plants;
    try {
      plants = await loadPlantTopology({
        async read() {
          return read;
        },
        async status() {
          throw new Error("runtime stopped");
        },
      });
    } catch (error) {
      rejected = error;
    }

    assert.strictEqual(rejected, undefined, "status failure must not remove saved topology");
    assert.strictEqual(plants?.[0]?.statusAvailability, "unavailable");
    assert.strictEqual(plants?.[0]?.liveSample, undefined);
    assert.strictEqual(plants?.[0]?.activeOperatingState, undefined);
  });

  test("adapts stopped reads and live status behind one typed service", async () => {
    const requests: Array<{ source: string; operation: string; params: unknown }> = [];
    const read = {
      contract: "trust.plant",
      schema_version: 1,
      operation: "plant.read",
      project_id: "project:sha256:service",
      configuration_revision: "sha256:saved",
      saved_configuration: {},
      restart_required: false,
      plants: [],
    } as PlantContractMessage;
    const status = {
      contract: "trust.plant",
      schema_version: 1,
      operation: "plant.status",
      project_id: read.project_id,
      available: true,
      active_configuration_revision: "sha256:saved",
      simulation_time_ns: "0",
      effective_seed: "1",
      random_algorithm: "chacha8",
      random_algorithm_version: 1,
      runtime_version: "0.24.64",
      time_scale: 1,
      restart_policy: "manual",
      plants: [],
    };
    const service = new ProjectPlantTopologyService(
      {
        async request(operation, params) {
          requests.push({ source: "stopped", operation, params });
          return read;
        },
      },
      async (operation, params) => {
        requests.push({ source: "live", operation, params });
        return status;
      }
    );

    await loadPlantTopology(service);

    assert.deepStrictEqual(requests, [
      {
        source: "stopped",
        operation: "plant.read",
        params: { contract: "trust.plant", schema_version: 1 },
      },
      {
        source: "live",
        operation: "plant.status",
        params: {
          contract: "trust.plant",
          schema_version: 1,
          project_id: "project:sha256:service",
        },
      },
    ]);
  });

  // Specification 47 §20.1: the editor asks the project's session; the project identity and the
  // editor tables come from one capabilities request.
  test("sends plant editor requests through the session with the project identity", async () => {
    const requests: Array<{ operation: string; params: Record<string, unknown> }> = [];
    const client = {
      async request(operation: string, params: Record<string, unknown>) {
        requests.push({ operation, params });
        const common = { contract: "trust.plant", schema_version: 1, project_id: "project:sha256:editor", operation };
        if (operation === "plant.capabilities") return { ...common, editor: { models: [] } } as PlantContractMessage;
        if (operation === "plant.signals") return { ...common, signals: { status: "ready" } } as PlantContractMessage;
        return { ...common, draft: params.draft } as PlantContractMessage;
      },
    };
    const editor = plantServiceApi.plantEditorClient(client);
    assert.strictEqual(plantServiceApi.plantEditorClient(client), editor, "one editor client per session");
    assert.deepStrictEqual(await editor.tables(), { models: [] });
    const answer = await editor.edit({ draft: { plantId: "tank" }, action: { kind: "open" } });
    assert.deepStrictEqual(answer.draft, { plantId: "tank" });
    await editor.edit({ draft: { plantId: "tank" } });
    assert.deepStrictEqual(
      await editor.signals({ compiled: undefined, live: { inputs: [] } }),
      { status: "ready" }
    );
    assert.deepStrictEqual(requests.map((request) => request.operation), [
      "plant.capabilities", "plant.edit", "plant.edit", "plant.signals",
    ]);
    assert.deepStrictEqual(requests[1].params, {
      contract: "trust.plant", schema_version: 1, project_id: "project:sha256:editor",
      draft: { plantId: "tank" }, action: { kind: "open" },
    });
    assert.strictEqual("action" in requests[2].params, false, "a plain check sends no action");
    assert.deepStrictEqual(requests[3].params, {
      contract: "trust.plant", schema_version: 1, project_id: "project:sha256:editor",
      compiled: null, live: { inputs: [] },
    });
  });

  test("reuses one stopped command session for one resolved project", () => {
    let created = 0;
    let disposed = 0;
    const registry = new PlantSessionRegistry((projectRoot) => {
      created += 1;
      return {
        projectRoot,
        async request() {
          throw new Error("not used");
        },
        dispose() {
          disposed += 1;
        },
      };
    });

    const first = registry.forProject("/workspace/demo");
    const second = registry.forProject("/workspace/demo/.");

    assert.strictEqual(first, second);
    assert.strictEqual(created, 1);
    registry.dispose();
    assert.strictEqual(disposed, 1);
  });

  test("starts the stopped-project producer without starting a runtime", () => {
    const calls: Array<{ binary: string; args: string[]; cwd: string }> = [];
    const process = new EventEmitter() as PlantCommandProcess & EventEmitter;
    process.stdin = { write: () => true };
    process.stdout = new EventEmitter();
    process.stderr = new EventEmitter();
    process.kill = () => true;
    const registry = createPlantSessionRegistry(
      "/opt/trust-runtime",
      (binary, args, options) => {
        calls.push({ binary, args, cwd: options.cwd });
        return process;
      }
    );

    registry.forProject("/workspace/demo");

    assert.deepStrictEqual(calls, [
      {
        binary: "/opt/trust-runtime",
        args: ["plant", "serve", "--project", "/workspace/demo"],
        cwd: "/workspace/demo",
      },
    ]);
    registry.dispose();
  });

  test("populates the canvas from the typed host service", async () => {
    const graph = {
      kind: "graph",
      title: "Devices & Connections",
      summary: "",
      hosts: [{
        id: "host:local",
        hostname: "This computer",
        label: "Local",
        health: "connected",
        containers: [],
        runtimes: [{
          id: "runtime:local",
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
    } as const;
    const read = {
      contract: "trust.plant",
      schema_version: 1,
      operation: "plant.read",
      project_id: "project:sha256:canvas",
      configuration_revision: "sha256:saved",
      saved_configuration: {},
      restart_required: false,
      plants: [{
        id: "tank",
        display_name: "Tank",
        description: null,
        model: "first_order",
        enabled: true,
        command_sources: ["%QW0"],
        feedback_targets: ["%IW0"],
        saved_order: 0,
      }],
    } as PlantContractMessage;

    await populatePlantTopology(graph as never, "runtime:local", {
      async read() { return read; },
      async status() { throw new Error("stopped"); },
    });

    assert.strictEqual((graph.hosts[0].runtimes[0] as { plants?: unknown[] }).plants?.length, 1);
    assert.ok(graph.summary.includes("1 simulated plant"));
  });
});
