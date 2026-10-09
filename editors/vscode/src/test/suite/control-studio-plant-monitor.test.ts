import * as assert from "assert";

import type { PlantControlStudioHandoff } from "../../controlStudio/handoff";
import {
  loadPlantMonitor,
  projectPlantMonitor,
} from "../../controlStudio/plantMonitor";
import type { RuntimeLifecycleSnapshot } from "../../runtimeLifecycle";
import type { PlantContractMessage } from "../../networkCanvas/plant/contract";

const handoff: PlantControlStudioHandoff = {
  schemaVersion: 1,
  handoffId: "handoff-1",
  projectId: "project:sha256:fixture",
  runtimeId: "runtime:local",
  plantId: "tank",
  savedConfigurationRevision: "sha256:saved",
  activeConfigurationDigest: "sha256:active",
  source: "Simulation",
  requestedContext: { mode: "Monitor", selection: "plant_summary" },
};

suite("Control Studio plant monitor", () => {
  test("uses the existing lifecycle endpoint and plant.status operation", async () => {
    const requests: unknown[][] = [];
    const result = await loadPlantMonitor(handoff, {
      async lifecycleSnapshot() { return lifecycle("running"); },
      async authToken(endpoint) {
        assert.strictEqual(endpoint, "tcp://127.0.0.1:9900");
        return "secret";
      },
      async request(...request) {
        requests.push(request);
        return status();
      },
    });
    assert.strictEqual(result.availability, "available");
    assert.deepStrictEqual(requests, [[
      "tcp://127.0.0.1:9900",
      "secret",
      "plant.status",
      {
        contract: "trust.plant",
        schema_version: 1,
        project_id: "project:sha256:fixture",
      },
    ]]);
  });

  test("queries the endpoint owned by a running debug session without an external probe flag", async () => {
    const running = lifecycle("running");
    const result = await loadPlantMonitor(handoff, {
      async lifecycleSnapshot() {
        return { ...running, status: { ...running.status, endpointReachable: false } };
      },
      async authToken() { return "secret"; },
      async request() { return status(); },
    });
    assert.strictEqual(result.availability, "available");
  });

  test("queries a reachable configured runtime that was started outside VS Code", async () => {
    const stopped = lifecycle("stopped");
    const result = await loadPlantMonitor(handoff, {
      async lifecycleSnapshot() {
        return {
          ...stopped,
          status: {
            ...stopped.status,
            runtimeMode: "online",
            endpoint: "unix:///tmp/trust-runtime.sock",
            endpointConfigured: true,
            endpointReachable: true,
          },
        };
      },
      async authToken() { return undefined; },
      async request() { return status(); },
    });
    assert.strictEqual(result.availability, "available");
  });

  test("projects the shared decoded status without inventing a setpoint", () => {
    assert.deepStrictEqual(projectPlantMonitor(status(), handoff), {
      availability: "available",
      source: "Simulation",
      plantId: "tank",
      activeConfigurationRevision: "sha256:active",
      runtimeVersion: "0.24.64",
      simulationTimeNs: "2000000",
      operatingState: "running",
      model: "first_order_dead_time",
      command: [2],
      appliedInput: [1.8],
      idealOutput: [1.4],
      measuredFeedback: [1.37],
      publishedFeedback: [1.38],
      state: [1.25],
      timeScale: 1,
      effectiveSeed: "42",
      randomAlgorithm: "chacha8",
      randomAlgorithmVersion: 1,
      actuators: [],
      activeUncertainties: [],
      activeDropouts: [],
      pendingEventCount: 0,
      delayBufferDepth: 2,
      lastSubsteps: 1,
      currentFault: null,
    });
  });

  test("clears current evidence for stopped, deleted, and stale plants", async () => {
    const stopped = await loadPlantMonitor(handoff, {
      async lifecycleSnapshot() { return lifecycle("stopped"); },
      async authToken() { throw new Error("must not request auth"); },
      async request() { throw new Error("must not request status"); },
    });
    assert.deepStrictEqual(stopped, {
      availability: "unavailable",
      source: "Simulation",
      plantId: "tank",
      reason: "Runtime is stopped",
    });
    assert.strictEqual(projectPlantMonitor({ ...status(), plants: [] }, handoff).availability, "unavailable");
    assert.deepStrictEqual(
      projectPlantMonitor(status(), { ...handoff, activeConfigurationDigest: "sha256:other" }),
      {
        availability: "unavailable",
        source: "Simulation",
        plantId: "tank",
        reason: "Running plant revision is stale",
      }
    );
  });
});

function lifecycle(runtimeState: "running" | "stopped"): RuntimeLifecycleSnapshot {
  return {
    status: {
      running: runtimeState === "running",
      inlineValuesEnabled: true,
      runtimeMode: "simulate",
      simulated: runtimeState === "running",
      runtimeState,
      endpoint: runtimeState === "running" ? "tcp://127.0.0.1:9900" : "",
      endpointConfigured: runtimeState === "running",
      endpointEnabled: true,
      endpointReachable: runtimeState === "running",
    },
    ioState: { inputs: [], outputs: [], memory: [] },
    starting: false,
  };
}

function status(): PlantContractMessage {
  return {
    contract: "trust.plant",
    schema_version: 1,
    operation: "plant.status",
    project_id: "project:sha256:fixture",
    available: true,
    active_configuration_revision: "sha256:active",
    simulation_time_ns: "2000000",
    effective_seed: "42",
    random_algorithm: "chacha8",
    random_algorithm_version: 1,
    runtime_version: "0.24.64",
    time_scale: 1,
    restart_policy: "manual",
    plants: [{
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
      measured_feedback: [1.37],
      published_feedback: [1.38],
      actuators: [],
      active_uncertainties: [],
      active_dropouts: [],
      pending_event_count: 0,
      delay_buffer_depth: 2,
      accumulated_remainder_ns: "0",
      last_substeps: 1,
      current_fault: null,
    }],
  };
}
