import * as assert from "assert";
import type { ControlStudioEngine } from "../../controlStudio/engine/engineClient";
import type { EngineLoop } from "../../controlStudio/engine/engineRouteContext";
import { createAnalyzeRoute } from "../../controlStudio/engine/routes/analyzeRoute";
import type { LoopMessageScope } from "../../controlStudio/messages";

/**
 * A served loop's measured paths reach the engine as the page's `controlStudio/analyze` request (specification 35, C7 and §0.1a): the
 * windows and the drift tolerance the page sent, for the loop, and no TypeScript model. A parked loop's request carries the TypeScript
 * session's model instead (V7.17 debt).
 */

const SCOPE: LoopMessageScope = {
  projectId: "project-test",
  editorSessionId: "editor-test",
  loopId: "loop-test",
  loopRevision: "revision-test",
};
const LOOP: EngineLoop = { id: "loop-test", runtimeId: "runtime-test" };
const INPUTS = {
  settledTrials: [
    { inputId: "heater-1", baseline: { startSeconds: 0, endSeconds: 1.9 }, settled: { startSeconds: 27.1, endSeconds: 31.9 } },
    { inputId: "heater-2", baseline: { startSeconds: 57.1, endSeconds: 61.9 }, settled: { startSeconds: 87.1, endSeconds: 92 } },
  ],
  maximumUnexcitedInputDrift: 0.001,
};
const RUN = { kind: "control-studio.analysis.run", payload: { tool: "measuredPaths", inputs: INPUTS } };

/** The engine as the route calls it: the tools of a loop, and an operation for each calculation. */
function fakeEngine(requests: Array<Record<string, unknown>>): ControlStudioEngine {
  const client = {
    analyze: async (request: Record<string, unknown>) => {
      requests.push(request);
      return request.tool === undefined ? { result: { tools: [] } } : { result: { operationId: "operation-1" } };
    },
    status: async () => ({ result: { operationId: "operation-1", command: "analyze", state: "running" } }),
    page: async () => ({ result: {} }),
    cancel: async () => ({ result: { operationId: "operation-1", command: "analyze", state: "cancelled" } }),
    onUpdate: () => ({ dispose: () => undefined }),
  };
  return client as unknown as ControlStudioEngine;
}

/** Opens the loop on a route that serves it or not, then sends the page's measured-paths calculation; returns the engine's requests of it. */
async function calculateOn(served: boolean, parkedModel: unknown): Promise<Array<Record<string, unknown>>> {
  const requests: Array<Record<string, unknown>> = [];
  const route = createAnalyzeRoute({
    client: fakeEngine(requests),
    projectRoot: "/project",
    post: async () => undefined,
    served: () => served,
    parkedModel: () => parkedModel,
    settled: async () => undefined,
  });
  await route.opened(LOOP, SCOPE, undefined);
  requests.length = 0;
  await route.handle(RUN, SCOPE);
  return requests;
}

suite("Control Studio measured paths route", () => {
  test("sends a served loop's measured paths to the engine with the page's windows and tolerance, and no TypeScript model", async () => {
    const requests = await calculateOn(true, { selected: "parked-model" });
    const run = requests.find(request => request.tool === "measuredPaths");
    assert.deepStrictEqual(run, { projectRoot: "/project", runtimeId: "runtime-test", loopId: "loop-test", tool: "measuredPaths", inputs: INPUTS });
    assert.strictEqual(run !== undefined && "model" in run, false, "the engine takes the study's model of a served loop");
  });

  test("sends a parked loop's measured paths with the TypeScript session's model (V7.17 debt)", async () => {
    const requests = await calculateOn(false, { selected: "parked-model" });
    const run = requests.find(request => request.tool === "measuredPaths");
    assert.deepStrictEqual(run?.model, { selected: "parked-model" });
  });
});
