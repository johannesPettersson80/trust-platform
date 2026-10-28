import * as assert from "assert";

import type { PlantControlStudioHandoff } from "../../controlStudio/handoff";
import { loadPlantMonitor, type PlantMonitorDependencies } from "../../controlStudio/plantMonitor";
import type { EngineAnswer } from "../../controlStudio/engine/engineClient";
import type { PlantMonitorResult } from "../../controlStudio/generated/PlantMonitorResult";
import type { RuntimeLifecycleSnapshot } from "../../runtimeLifecycle";
import { bindPlantSummary, plantSummaryText } from "../../controlStudio/webview/plantSummary";
import { renderThreePageStudioHtml } from "../../controlStudio/threePageTemplate";
import type { ControlStudioView } from "../../controlStudio/panelTemplate";

// The plant summary is the engine's (`controlStudio/plantMonitor`, specification 35 §0.1a): these tests cover only the host's wiring.
// The decisions (the reads that are made, the decode, the stale and missing plant) are pinned by trust-control's Rust tests.

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

const answer: PlantMonitorResult = {
  availability: "unavailable",
  source: "Simulation",
  plantId: "tank",
  reason: "Plant no longer exists",
};

suite("Control Studio plant monitor", () => {
  test("hands the engine the handoff's plant, the runtime's state and endpoint, and the token, then posts its answer", async () => {
    const requests: unknown[] = [];
    const result = await loadPlantMonitor(handoff, dependencies("running", async request => {
      requests.push(request);
      return { result: answer };
    }));
    assert.deepStrictEqual(result, answer);
    assert.deepStrictEqual(requests, [{
      projectId: "project:sha256:fixture",
      plantId: "tank",
      activeConfigurationDigest: "sha256:active",
      runtimeStopped: false,
      endpointReachable: true,
      connection: { endpoint: "tcp://127.0.0.1:9900", token: "secret" },
    }]);
  });

  test("asks for the token of the endpoint it reads", async () => {
    const endpoints: string[] = [];
    await loadPlantMonitor(handoff, {
      ...dependencies("running", async () => ({ result: answer })),
      async authToken(endpoint) {
        endpoints.push(endpoint);
        return "secret";
      },
    });
    assert.deepStrictEqual(endpoints, ["tcp://127.0.0.1:9900"]);
  });

  test("a stopped runtime is reported to the engine as stopped, with no token", async () => {
    const requests: Array<{ runtimeStopped: boolean; connection: unknown }> = [];
    await loadPlantMonitor(handoff, dependencies("stopped", async request => {
      requests.push(request as { runtimeStopped: boolean; connection: unknown });
      return { result: answer };
    }, null));
    assert.strictEqual(requests[0].runtimeStopped, true);
    assert.deepStrictEqual(requests[0].connection, { endpoint: "" });
  });

  test("a running runtime whose probe did not answer is still sent as running", async () => {
    const requests: Array<{ runtimeStopped: boolean; endpointReachable: boolean }> = [];
    const running = lifecycle("running");
    await loadPlantMonitor(handoff, {
      async lifecycleSnapshot() {
        return { ...running, status: { ...running.status, endpointReachable: false } };
      },
      async authToken() { return "secret"; },
      async monitor(request) {
        requests.push(request as { runtimeStopped: boolean; endpointReachable: boolean });
        return { result: answer };
      },
    });
    assert.strictEqual(requests[0].runtimeStopped, false);
    assert.strictEqual(requests[0].endpointReachable, false);
  });

  test("an engine refusal shows the plant as unavailable with the engine's sentence", async () => {
    const result = await loadPlantMonitor(handoff, dependencies("running", async () => failureAnswer(
      "The engine did not answer.",
      "The engine did not answer the plant status. Retry later.",
    )));
    // The engine named no source in a failure, so the summary names none: the host invents no default.
    assert.deepStrictEqual(result, {
      availability: "unavailable",
      plantId: "tank",
      reason: "The engine did not answer the plant status. Retry later.",
    });
  });

  test("a call that throws shows the plant as unavailable with the error's text", async () => {
    const result = await loadPlantMonitor(handoff, {
      async lifecycleSnapshot() { throw new Error("The lifecycle service is down."); },
      async authToken() { return "secret"; },
      async monitor() { throw new Error("must not be called"); },
    });
    assert.deepStrictEqual(result, {
      availability: "unavailable",
      plantId: "tank",
      reason: "The lifecycle service is down.",
    });
  });
});

/** The lifecycle service's facts for `state`. `token` is the control token the service holds for the endpoint: `null` is none (an `undefined` argument would take the default). */
function dependencies(
  state: "running" | "stopped",
  monitor: PlantMonitorDependencies["monitor"],
  token: string | null = "secret",
): PlantMonitorDependencies {
  return {
    async lifecycleSnapshot() { return lifecycle(state); },
    async authToken() { return token ?? undefined; },
    monitor,
  };
}

function failureAnswer(message: string, detail: string): EngineAnswer<PlantMonitorResult> {
  return { failure: { message, detail, cause: "runtime", nextAction: "none" } } as unknown as EngineAnswer<PlantMonitorResult>;
}

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
    ioState: { inputs: [], outputs: [], memory: [], ads: [] },
    starting: false,
  };
}

// The page shows the engine's answer for the plant handoff in one line (specification 35, B1.1); the engine decides every word of the
// availability, which these tests do not pin (the Rust tests of plant_monitor.rs do).
suite("Control Studio plant summary line", () => {
  /** The engine's answer for an available plant; only the fields the line shows are filled. */
  const availableWith = (status: Record<string, unknown>): PlantMonitorResult =>
    ({ availability: "available", source: "Simulation", plantId: "tank", status }) as unknown as PlantMonitorResult;
  const available = availableWith({ model: "first-order", operatingState: "running", currentFault: null });

  test("words the engine's answer: the plant and its source, then its model and state, or the engine's reason", () => {
    assert.strictEqual(plantSummaryText(available), "tank · Simulation · first-order · running");
    const faulted = availableWith({ model: "first-order", operatingState: "faulted", currentFault: "Pump stuck" });
    assert.strictEqual(plantSummaryText(faulted), "tank · Simulation · first-order · faulted (Pump stuck)");
    assert.strictEqual(plantSummaryText(answer), "tank · Simulation · unavailable: Plant no longer exists");
    assert.strictEqual(plantSummaryText({ availability: "unavailable", source: "Simulation", plantId: "tank" }), "tank · Simulation · unavailable");
    // A failure's sentence names no source (the host invents none), and the line names none.
    assert.strictEqual(plantSummaryText({ availability: "unavailable", plantId: "tank", reason: "The engine did not answer." }),
      "tank · unavailable: The engine did not answer.");
  });

  test("the page holds a hidden summary line, and a snapshot of its project, session and plant revision fills and shows it", () => {
    const { JSDOM } = require("jsdom");
    const html = renderThreePageStudioHtml({ cspSource: "x", nonce: "n", chartScript: "chart.js", workflowScript: "workflow.js", project: "project-1",
      session: "session-1", loop: "", loopRevision: "", revision: "sha256:saved", processUnit: "", outputUnit: "" } as unknown as ControlStudioView, text => text);
    const dom = new JSDOM(html);
    try {
      const root = dom.window.document.querySelector("[data-studio-three-page]") as HTMLElement;
      const line = root.querySelector("[data-studio-plant-summary]") as HTMLElement;
      assert.strictEqual(line.hidden, true, "a page without a snapshot shows no summary");
      bindPlantSummary(root);
      const post = (data: unknown) => dom.window.dispatchEvent(new dom.window.MessageEvent("message", { data }));
      const snapshot = (overrides: Record<string, unknown>, payload: PlantMonitorResult = answer) => ({
        schemaVersion: 1, kind: "plant.monitor.snapshot", messageId: "m", projectId: "project-1", editorSessionId: "session-1",
        plantId: "tank", expectedPlantRevision: "sha256:saved", payload, ...overrides });
      // Another project, another session, another plant revision and another kind of message are not this page's.
      post(snapshot({ projectId: "project-2" }));
      post(snapshot({ editorSessionId: "session-2" }));
      post(snapshot({ expectedPlantRevision: "sha256:other" }));
      post(snapshot({ kind: "loop.monitor.snapshot" }));
      assert.strictEqual(line.hidden, true);
      post(snapshot({}));
      assert.strictEqual(line.hidden, false);
      assert.strictEqual(line.textContent, "tank · Simulation · unavailable: Plant no longer exists");
      post(snapshot({}, available));
      assert.strictEqual(line.textContent, "tank · Simulation · first-order · running", "the next answer replaces the line");
    } finally { dom.window.close(); }
  });
});
