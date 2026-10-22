import * as assert from "assert";
import type { ControlStudioEngine } from "../../controlStudio/engine/engineClient";
import type { EngineLoop, EngineScope } from "../../controlStudio/engine/engineRouteContext";
import { analysisPageOf, createAnalyzeRoute } from "../../controlStudio/engine/routes/analyzeRoute";
import type { OperationUpdate } from "../../controlStudio/generated/OperationUpdate";

/**
 * The analysis route's answers to the page (specification 35, B9.9). The engine sends an operation's updates as notifications, and a
 * notification reaches only the listeners that stand when it is sent (`engineProcess.ts`). The fake engine below does the same: it keeps
 * the operation's latest state, as the engine does (`Operations::report`), and delivers each update to the listeners present at once.
 */

const SCOPE: EngineScope = {
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
const TOOLS = "control-studio.analysis.tools";
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
  /** The page records the engine received (`controlStudio/page`), in order. */
  readonly pages: Array<Record<string, unknown>> = [];
  /** Whether the operation ends before the engine answers the run, so that no listener of the route stands yet. */
  finishBeforeReply = false;
  /** The engine's answer to a request for the tools (no `tool`): the list it gives, or its failure. */
  listAnswer: Record<string, unknown> = { result: { tools: [] } };
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
      if (request.tool === undefined) return this.listAnswer;
      if (this.finishBeforeReply) this.report(finishedUpdate());
      return { result: { operationId: OPERATION } };
    },
    status: async () => ({ result: this.latest }),
    page: async (request: Record<string, unknown>) => { this.pages.push(request); return { result: {} }; },
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
    settled: async () => undefined,
  });
  await route.opened(LOOP, SCOPE, undefined);
  assert.ok(await route.handle(RUN, SCOPE), "the route takes the page's calculation");
  return posted;
}

/** Opens the loop through the route, and returns the messages the page is sent for it (the tools the engine lists, B9.9). */
async function openLoop(engine: FakeEngine): Promise<Array<Record<string, unknown>>> {
  const posted: Array<Record<string, unknown>> = [];
  const route = createAnalyzeRoute({
    client: engine.client as unknown as ControlStudioEngine,
    projectRoot: "/project",
    post: async message => {
      posted.push(message);
    },
    served: () => true,
    settled: async () => undefined,
  });
  await route.opened(LOOP, SCOPE, undefined);
  return posted;
}

suite("Control Studio analysis route", () => {
  test("a refused loop's empty tool list is the engine's answer, shown as the list with no failure", async function () {
    this.timeout(10000);
    const engine = new FakeEngine();
    const tools = (await openLoop(engine)).filter(message => message.kind === TOOLS);
    assert.deepStrictEqual(tools.map(message => message.tools), [[]], "the page is sent the engine's empty list");
    assert.ok(tools.every(message => !("failure" in message)), "no failure of the list is shown for a refused loop");
  });

  test("a failure the engine answers for the tool list is shown as that failure: the route decides no cause", async function () {
    this.timeout(10000);
    const engine = new FakeEngine();
    const refusal = { message: "The loop uses a structure the engine does not run.", cause: "controller-capability", nextAction: "none" };
    engine.listAnswer = { failure: refusal };
    const tools = (await openLoop(engine)).filter(message => message.kind === TOOLS);
    assert.deepStrictEqual(tools.map(message => message.failure), [refusal]);
    assert.ok(tools.every(message => !("tools" in message)), "no list is invented for a failed answer");
  });

  test("the route sends the engine the answers the page holds and sends the page the engine's list of stale ones, and decides which are stale by nothing", async function () {
    this.timeout(10000);
    const engine = new FakeEngine();
    const posted: Array<Record<string, unknown>> = [];
    const route = createAnalyzeRoute({
      client: engine.client as unknown as ControlStudioEngine,
      projectRoot: "/project",
      post: async message => { posted.push(message); },
      served: () => true,
      settled: async () => undefined,
    });
    await route.opened(LOOP, SCOPE, undefined);
    assert.strictEqual(engine.requests[0].held, undefined, "the request that opens the loop holds no answer");
    const held = [{ tool: "poleZero", model: "fit-1" }, { tool: "rootLocus" }];
    engine.listAnswer = { result: { tools: [], stale: ["poleZero"] } };
    posted.length = 0;
    assert.ok(await route.handle({ kind: "control-studio.analysis.tools", payload: { held } }, SCOPE), "the route takes the page's request for the tools");
    assert.deepStrictEqual(engine.requests[engine.requests.length - 1].held, held, "the engine is told which answers the page holds, as the page named them");
    assert.deepStrictEqual(posted.filter(message => message.kind === TOOLS).map(message => message.stale), [["poleZero"]],
      "the page is sent the tools the engine lists as stale: the route decides none (B9.9)");
    engine.listAnswer = { result: { tools: [] } };
    posted.length = 0;
    await route.handle({ kind: "control-studio.analysis.tools", payload: { held } }, SCOPE);
    assert.ok(posted.filter(message => message.kind === TOOLS).every(message => !("stale" in message)),
      "an engine that lists no stale answer sends the page none");
  });

  test("the route stores no analysis of its own: the engine records a calculation, and the page's own choice goes to the engine as the page names it (B9.9)", async function () {
    this.timeout(10000);
    const engine = new FakeEngine();
    const posted: Array<Record<string, unknown>> = [];
    const route = createAnalyzeRoute({
      client: engine.client as unknown as ControlStudioEngine,
      projectRoot: "/project",
      post: async message => { posted.push(message); },
      served: () => true,
      settled: async () => undefined,
    });
    await route.opened(LOOP, SCOPE, undefined);
    // A calculation, and the operation's end: the engine records the analysis it answers with the inputs it read, so the route sends no page record.
    // A route that recorded the envelope's inputs itself would choose what is stored.
    assert.ok(await route.handle(RUN, SCOPE));
    engine.report(finishedUpdate());
    assert.ok(await within(() => finalUpdates(posted).length > 0));
    assert.deepStrictEqual(engine.pages, [], "the route records no page for a calculation");
    // The tool the engineer selects, with the inputs the page holds, goes as the page names it; closing the analysis names none.
    const grid = { grid: { minimumFrequency: 0.1, maximumFrequency: 10, points: 8 } };
    assert.ok(await route.handle({ kind: "control-studio.analysis.show", payload: { tool: "frequency", inputs: grid } }, SCOPE));
    assert.ok(await route.handle({ kind: "control-studio.analysis.show", payload: {} }, SCOPE));
    const base = { loopId: LOOP.id, page: "Tune", projectRoot: "/project", runtimeId: LOOP.runtimeId };
    assert.deepStrictEqual(engine.pages, [{ ...base, analysis: { tool: "frequency", inputs: grid } }, base]);
  });

  test("the analysis the page names is its tool with the inputs it holds, and a message that names no tool names none", () => {
    assert.deepStrictEqual(analysisPageOf({ tool: "poleZero", inputs: { view: "minimal" } }), { tool: "poleZero", inputs: { view: "minimal" } });
    assert.deepStrictEqual(analysisPageOf({ tool: "poleZero" }), { tool: "poleZero", inputs: {} }, "an analysis without inputs is named with none");
    for (const value of [undefined, null, {}, { inputs: {} }, { tool: 3 }, "frequency", ["frequency"]]) {
      assert.strictEqual(analysisPageOf(value), undefined, JSON.stringify(value));
    }
  });

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
