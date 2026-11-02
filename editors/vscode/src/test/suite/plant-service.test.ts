import * as assert from "assert";
import { EventEmitter } from "events";

import type { PlantContractMessage } from "../../networkCanvas/plant/contract";
import { loadPlantTopology } from "../../networkCanvas/plant/projection";
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

/** A `plant.topology` answer as the stopped-project session gives it. */
function topologyAnswer(projectId: string, plants: unknown[]): PlantContractMessage {
  return {
    contract: "trust.plant",
    schema_version: 1,
    operation: "plant.topology",
    project_id: projectId,
    configuration_revision: "sha256:saved",
    plants,
  };
}

// The projection's rules are Rust's (`plant.topology`, specification 47 §20.6); its tests are
// trust-runtime's `control/plant_handlers/topology/tests.rs`. These tests prove the wiring.
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

  // plant.save validates the candidate itself (specification 47 §20; the Rust test
  // plant_save_refuses_a_candidate_the_simulation_refuses_without_writing), so Save sends no
  // plant.validate first. Catches the extension running a validation of its own before saving.
  test("saves one plant through the existing project session with one plant.save", async () => {
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

  test("asks the status with the read's project identity, then Rust's projection with that status", async () => {
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

    let projected: PlantContractMessage | null | undefined;
    await loadPlantTopology({
      async read() {
        calls.push("read");
        return read;
      },
      async status(projectId) {
        calls.push(`status:${projectId}`);
        return status;
      },
      async topology(projectId, given) {
        calls.push(`topology:${projectId}`);
        projected = given;
        return topologyAnswer(projectId, []);
      },
    });

    assert.deepStrictEqual(calls, [
      "read",
      "status:project:sha256:ordered",
      "topology:project:sha256:ordered",
    ]);
    assert.strictEqual(projected, status, "Rust projects the runtime's own status answer");
  });

  test("hands Rust no status when the runtime gives none, and shows Rust's plants", async () => {
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
    const rustPlants = [{ id: "tank", statusAvailability: "unavailable", states: ["Status unavailable"] }];
    let rejected: unknown;
    let plants;
    let projected: PlantContractMessage | null | undefined;
    try {
      plants = await loadPlantTopology({
        async read() {
          return read;
        },
        async status() {
          throw new Error("runtime stopped");
        },
        async topology(projectId, given) {
          projected = given;
          return topologyAnswer(projectId, rustPlants);
        },
      });
    } catch (error) {
      rejected = error;
    }

    assert.strictEqual(rejected, undefined, "status failure must not remove saved topology");
    assert.strictEqual(projected, null, "a failed status reaches Rust as none");
    assert.deepStrictEqual(plants, rustPlants, "the canvas shows the plants Rust projects");
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
          return operation === "plant.topology" ? topologyAnswer(read.project_id, []) : read;
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
      {
        source: "stopped",
        operation: "plant.topology",
        params: {
          contract: "trust.plant",
          schema_version: 1,
          project_id: "project:sha256:service",
          status,
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
      async topology(projectId) {
        return topologyAnswer(projectId, [{
          id: "tank",
          projectId,
          displayName: "Tank",
          model: "first_order",
          enabled: true,
          commandSources: ["%QW0"],
          feedbackTargets: ["%IW0"],
          statusAvailability: "unavailable",
          savedConfigurationRevision: "sha256:saved",
          states: ["Status unavailable"],
        }]);
      },
    });

    assert.strictEqual((graph.hosts[0].runtimes[0] as { plants?: unknown[] }).plants?.length, 1);
    assert.ok(graph.summary.includes("1 simulated plant"));
  });
});
