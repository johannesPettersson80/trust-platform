import * as assert from "assert";
import type { ControlStudioEngine } from "../../controlStudio/engine/engineClient";
import type { EngineLoop } from "../../controlStudio/engine/engineRouteContext";
import { createAnalyzeRoute } from "../../controlStudio/engine/routes/analyzeRoute";
import type { OperationUpdate } from "../../controlStudio/generated/OperationUpdate";
import type { LoopMessageScope } from "../../controlStudio/messages";

/**
 * The analysis route's answers to the page (specification 35, B9.9). The engine sends an operation's updates as notifications, and a
 * notification reaches only the listeners that stand when it is sent (`engineProcess.ts`). The fake engine below does the same: it keeps
 * the operation's latest state, as the engine does (`Operations::report`), and delivers each update to the listeners present at once.
 */

const SCOPE: LoopMessageScope = {
  projectId: "project-test",
  editorSessionId: "editor-test",
  loopId: "loop-test",
  loopRevision: "revision-test",
};
const LOOP: EngineLoop = { id: "loop-test", runtimeId: "runtime-test" };
const OPERATION = "operation-1";
const INPUTS = { stepSeconds: 0.1, durationSeconds: 2 };
const STARTED = "control-studio.analysis.started";
const UPDATE = "control-studio.analysis.update";
const RUN = { kind: "control-studio.analysis.run", payload: { tool: "initialState", inputs: INPUTS } };

/** The final update of the `initialState` operation: its envelope names the tool and holds the answer (B9.9). */
function finishedUpdate(): OperationUpdate {
  return {
    operationId: OPERATION,
    command: "analyze",
    state: "finished",
    result: { tool: "initialState", inputs: INPUTS, result: { initialState: { points: [] } } },
  };
}

class FakeEngine {
  /** The requests the engine received, in order. */
  readonly requests: Array<Record<string, unknown>> = [];
  /** Whether the operation ends before the engine answers the run, so that no listener of the route stands yet. */
  finishBeforeReply = false;
  private latest: OperationUpdate = { operationId: OPERATION, command: "analyze", state: "running" };
  private readonly listeners = new Set<(update: OperationUpdate) => void>();

  /** The engine's own delivery: it keeps the latest state, then sends the update to the listeners that stand now. */
  report(update: OperationUpdate): void {
    this.latest = update;
    for (const listener of Array.from(this.listeners)) listener(update);
  }

  /** The methods the analysis route calls: the tools, a run's operation, its status, the page record and the cancel. */
  readonly client = {
    analyze: async (request: Record<string, unknown>) => {
      this.requests.push(request);
      if (request.tool === undefined) return { result: { tools: [] } };
      if (this.finishBeforeReply) this.report(finishedUpdate());
      return { result: { operationId: OPERATION } };
    },
    status: async () => ({ result: this.latest }),
    page: async () => ({ result: {} }),
    cancel: async () => ({ result: this.latest }),
    onUpdate: (_operationId: string, listener: (update: OperationUpdate) => void) => {
      this.listeners.add(listener);
      return { dispose: () => { this.listeners.delete(listener); } };
    },
  };
}

const delay = (ms: number): Promise<void> => new Promise(resolve => setTimeout(resolve, ms));

/** Whether `condition` holds within `timeoutMs`: the route's answers arrive asynchronously. */
async function within(condition: () => boolean, timeoutMs = 2000): Promise<boolean> {
  const deadline = Date.now() + timeoutMs;
  while (!condition()) {
    if (Date.now() > deadline) return false;
    await delay(5);
  }
  return true;
}

function finalUpdates(posted: Array<Record<string, unknown>>): Array<Record<string, unknown>> {
  return posted.filter(message => message.kind === UPDATE && message.state === "finished");
}

function assertStartedFirst(posted: Array<Record<string, unknown>>): void {
  const kinds = posted.map(message => message.kind);
  const started = kinds.indexOf(STARTED);
  assert.ok(started >= 0, "the page is told the operation started");
  assert.ok(kinds.indexOf(UPDATE) > started, "the operation's updates follow `started`");
}

/**
 * Runs one calculation of `initialState` through the route, and returns the messages the page is sent. `finishDuringAck` ends the
 * operation while the page acknowledges `started`, as a quick operation can end while that acknowledgement is on its way.
 */
async function calculate(engine: FakeEngine, finishDuringAck: boolean): Promise<Array<Record<string, unknown>>> {
  const posted: Array<Record<string, unknown>> = [];
  const route = createAnalyzeRoute({
    client: engine.client as unknown as ControlStudioEngine,
    projectRoot: "/project",
    post: async message => {
      posted.push(message);
      if (message.kind !== STARTED) return;
      if (finishDuringAck) setTimeout(() => engine.report(finishedUpdate()), 0);
      await delay(10);
    },
    served: () => true,
    parkedModel: () => undefined,
    settled: async () => undefined,
  });
  await route.opened(LOOP, SCOPE, undefined);
  assert.ok(await route.handle(RUN, SCOPE), "the route takes the page's calculation");
  return posted;
}

suite("Control Studio analysis route", () => {
  test("an operation that ends before the route follows it still reaches the page with its final answer", async function () {
    this.timeout(10000);
    const engine = new FakeEngine();
    engine.finishBeforeReply = true;
    const posted = await calculate(engine, false);
    assert.ok(await within(() => finalUpdates(posted).length > 0), "the page is sent the operation's final update");
    assert.ok(engine.requests.some(request => request.tool === "initialState"), "the engine is asked for the calculation");
    assertStartedFirst(posted);
    const final = finalUpdates(posted)[0];
    assert.strictEqual((final?.envelope as { tool?: unknown } | undefined)?.tool, "initialState");
  });

  test("an operation that ends while the page acknowledges `started` still reaches the page with its final answer", async function () {
    this.timeout(10000);
    const engine = new FakeEngine();
    const posted = await calculate(engine, true);
    assert.ok(await within(() => finalUpdates(posted).length > 0), "the page is sent the operation's final update");
    assert.ok(engine.requests.some(request => request.tool === "initialState"), "the engine is asked for the calculation");
    assertStartedFirst(posted);
    const final = finalUpdates(posted)[0];
    assert.strictEqual((final?.envelope as { tool?: unknown } | undefined)?.tool, "initialState");
  });

  test("a running operation's updates reach the page while it runs, and its final update is sent once", async function () {
    this.timeout(10000);
    const engine = new FakeEngine();
    const posted = await calculate(engine, false);
    engine.report({ operationId: OPERATION, command: "analyze", state: "running", progress: "Calculating…" });
    engine.report(finishedUpdate());
    engine.report(finishedUpdate());
    await delay(20);
    assert.strictEqual(finalUpdates(posted).length, 1, "the final update is sent once");
    assert.ok(posted.some(message => message.kind === UPDATE && message.state === "running"), "the running update reaches the page");
    assertStartedFirst(posted);
  });
});
