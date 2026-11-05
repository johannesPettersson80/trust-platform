import * as assert from "assert";
import { EventEmitter } from "events";

import type { PlantContractMessage } from "../../networkCanvas/plant/contract";
import { ConfirmedPlants, loadPlantTopology, type PlantTopologyService } from "../../networkCanvas/plant/projection";
import {
  createPlantSessionRegistry,
  type PlantCommandClient,
  PlantSessionRegistry,
  populatePlantTopology,
  ProjectPlantTopologyService,
} from "../../networkCanvas/plant/service";
import type { PlantCommandProcess } from "../../networkCanvas/plant/offlineSession";
import * as plantServiceApi from "../../networkCanvas/plant/service";
import { RuntimeControlError } from "../../runtimeControlClient";
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

  // Specification 47 §20.6: Rust's projection names the project, the status is asked with that
  // project identity, and Rust projects the plants again with the status as it came. Catches the
  // extension reading the saved plants itself, or deciding on the status before Rust does.
  test("asks Rust's projection, the status with its project identity, then the projection with that status", async () => {
    const calls: string[] = [];
    const status = { contract: "trust.plant", operation: "plant.status", available: false, reason_code: "x" };
    let projected: unknown;
    const { plants } = await loadPlantTopology({
      async topology(given) {
        calls.push(given === null ? "topology" : "topology:status");
        projected = given;
        return topologyAnswer("project:sha256:ordered", [{ id: given === null ? "first" : "second" }]);
      },
      async status(projectId) {
        calls.push(`status:${projectId}`);
        return status;
      },
    });

    assert.deepStrictEqual(calls, ["topology", "status:project:sha256:ordered", "topology:status"]);
    assert.strictEqual(projected, status, "the status reaches Rust as it came, not decoded first");
    assert.deepStrictEqual(plants, [{ id: "second" }], "the canvas shows Rust's projection with the status");
  });

  // Catches a failed status hiding the saved plants, and a second projection asked for nothing.
  test("shows Rust's first projection when the runtime gives no status", async () => {
    const calls: string[] = [];
    const { plants } = await loadPlantTopology({
      async topology(given) {
        calls.push(given === null ? "topology" : "topology:status");
        return topologyAnswer("project:sha256:offline", [{ id: "tank", states: ["Status unavailable"] }]);
      },
      async status() {
        calls.push("status");
        throw new Error("runtime stopped");
      },
    });

    assert.deepStrictEqual(calls, ["topology", "status"]);
    assert.deepStrictEqual(plants, [{ id: "tank", states: ["Status unavailable"] }]);
  });

  // Specification 47 §20.6: a rejected answer leaves the last confirmed saved configuration, which Rust's first projection
  // shows as Status unavailable with no live data. Catches a status projection that fails hiding the saved plants.
  test("a second projection that fails leaves the first", async () => {
    const calls: string[] = [];
    const topology = await loadPlantTopology({
      async topology(given) {
        calls.push(given === null ? "topology" : "topology:status");
        if (given !== null) throw new Error("the plant file changed between the two calls");
        return topologyAnswer("project:sha256:changed", [{ id: "tank", states: ["Status unavailable"] }]);
      },
      async status() {
        return { contract: "trust.plant", operation: "plant.status", available: true };
      },
    });

    assert.deepStrictEqual(calls, ["topology", "topology:status"]);
    assert.deepStrictEqual(topology.plants, [{ id: "tank", states: ["Status unavailable"] }]);
    assert.deepStrictEqual(topology.saved, topology.plants);
  });

  // Catches a failed refresh dropping every plant from the canvas (the panel drew none), and one that keeps the last
  // live sample instead of Rust's projection without a status (specification 47 §20.6).
  test("a load that fails keeps the project's last confirmed saved plants, without their live data", async () => {
    const confirmed = new ConfirmedPlants();
    const running = { id: "tank", states: ["Running"], liveSample: { command: 1, feedback: 2 } };
    const saved = { id: "tank", states: ["Status unavailable"] };
    const answering: PlantTopologyService = {
      async topology(given) {
        return topologyAnswer("project:sha256:tank", [given === null ? saved : running]);
      },
      async status() {
        return { contract: "trust.plant", operation: "plant.status", available: true };
      },
    };
    const failing: PlantTopologyService = {
      async topology() {
        throw new Error("session timed out");
      },
      async status() {
        throw new Error("not asked");
      },
    };
    const failures: unknown[] = [];
    const onFailure = (error: unknown) => failures.push(error);

    assert.deepStrictEqual(await confirmed.load("/project", answering, onFailure), [running]);
    assert.deepStrictEqual(await confirmed.load("/project", failing, onFailure), [saved]);
    assert.strictEqual(await confirmed.load("/other", failing, onFailure), undefined, "another project has none");
    assert.strictEqual(failures.length, 2, "each failure is still reported");
  });

  // Catches a stopped runtime's answer dropped: it says that no runtime runs the plants, which Rust projects as
  // Configured, where the first projection shows Status unavailable (specification 47 §20.6).
  test("passes a refusal's answer to Rust's projection as it came", async () => {
    const answer = {
      contract: "trust.plant",
      schema_version: 1,
      operation: "plant.status",
      project_id: "project:sha256:stopped",
      requested_plant_id: null,
      available: false,
      reason_code: "runtime_evidence_unavailable",
    };
    let projected: unknown;
    const { plants } = await loadPlantTopology({
      async topology(given) {
        projected = given;
        const state = given === null ? "Status unavailable" : "Configured";
        return topologyAnswer("project:sha256:stopped", [{ id: "tank", states: [state] }]);
      },
      async status() {
        throw new RuntimeControlError("active plant runtime evidence is unavailable", "status_unavailable", answer);
      },
    });

    assert.strictEqual(projected, answer, "the refusal's answer reaches Rust as it came");
    assert.deepStrictEqual(plants, [{ id: "tank", states: ["Configured"] }]);
  });

  test("adapts stopped projections and live status behind one typed service", async () => {
    const requests: Array<{ source: string; operation: string; params: unknown }> = [];
    const status = { contract: "trust.plant", schema_version: 1, operation: "plant.status", available: true };
    const service = new ProjectPlantTopologyService(
      {
        async request(operation, params) {
          requests.push({ source: "stopped", operation, params });
          return topologyAnswer("project:sha256:service", []);
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
        operation: "plant.topology",
        params: { contract: "trust.plant", schema_version: 1, status: null },
      },
      {
        source: "live",
        operation: "plant.status",
        params: { contract: "trust.plant", schema_version: 1, project_id: "project:sha256:service" },
      },
      {
        source: "stopped",
        operation: "plant.topology",
        params: { contract: "trust.plant", schema_version: 1, status },
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
      async status() { throw new Error("stopped"); },
      async topology() {
        const projectId = read.project_id;
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
