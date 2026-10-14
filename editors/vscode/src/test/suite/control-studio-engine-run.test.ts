import * as assert from "assert";
import type { ControlStudioEngine, EngineAnswer } from "../../controlStudio/engine/engineClient";
import type { EngineLoop, EngineMethod, EngineRouteContext } from "../../controlStudio/engine/engineRouteContext";
import { createRunRoute } from "../../controlStudio/engine/routes/runRoute";
import type { Failure } from "../../controlStudio/generated/Failure";
import type { LoopMessageScope } from "../../controlStudio/messages";
import { receiveEngineMessage, type EngineHostPorts } from "../../controlStudio/webview/engineOpened";
import { runReceiver } from "../../controlStudio/webview/engineRun";
import { ThreePageOperation } from "../../controlStudio/webview/threePageOperation";

const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };

const SCOPE: LoopMessageScope = {
  projectId: "project-test",
  editorSessionId: "editor-test",
  loopId: "loop-test",
  loopRevision: "revision-test",
};
const LOOP: EngineLoop = { id: "loop-test", runtimeId: "runtime-test" };
const CONNECTION = { endpoint: "tcp://127.0.0.1:9" };
const NO_EVIDENCE = { hasModel: false, hasCandidate: false, validationPassed: false };

/**
 * A stage route over a fake context. The engine's client is a fake too: each call is recorded, and answers what the test set.
 * The fake `ask` passes the engine's answer through, as the panel's `ask` does once the engine has started.
 */
function harness() {
  const h = {
    loop: LOOP as EngineLoop | undefined,
    connection: CONNECTION as { endpoint: string } | undefined,
    run: { result: { operationId: "operation-1" } } as EngineAnswer<{ operationId: string }>,
    cancel: { result: { operationId: "operation-1", command: "run", state: "running" } } as EngineAnswer<Record<string, unknown>>,
    runRequests: [] as Array<Record<string, unknown>>,
    cancelRequests: [] as Array<Record<string, unknown>>,
    failures: [] as Array<{ method: EngineMethod; failure: Failure }>,
    followed: [] as string[],
  };
  const client = {
    run: async (request: Record<string, unknown>) => {
      h.runRequests.push(request);
      return h.run;
    },
    cancel: async (request: Record<string, unknown>) => {
      h.cancelRequests.push(request);
      return h.cancel;
    },
  };
  const context: EngineRouteContext = {
    client: client as unknown as ControlStudioEngine,
    projectRoot: "/project",
    storageFolder: "",
    served: () => h.loop,
    post: async () => undefined,
    postFailure: async (_scope, method, failure) => {
      h.failures.push({ method, failure });
    },
    ask: async <T>(_scope: LoopMessageScope, _method: EngineMethod, run: () => Promise<EngineAnswer<T>>) => run(),
    connection: async () => h.connection,
    follow: (operationId) => {
      h.followed.push(operationId);
    },
  };
  return { h, context };
}

/** A page message of the loop's scope, with its payload. */
function pageMessage(kind: string, payload: Record<string, unknown>): Record<string, unknown> {
  return {
    schemaVersion: 1,
    kind,
    messageId: "message-test",
    projectId: SCOPE.projectId,
    editorSessionId: SCOPE.editorSessionId,
    loopId: SCOPE.loopId,
    expectedLoopRevision: SCOPE.loopRevision,
    payload,
  };
}

const IDENTIFY_START = (): Record<string, unknown> => pageMessage("control-studio.flow.start", { from: "find", task: {} });
const ABORT = (): Record<string, unknown> => pageMessage("control-studio.flow.cancel", {});

type Received = Array<{ stage: string; payload: Record<string, unknown> }>;

function portsOf(received: Received): EngineHostPorts {
  return {
    select: () => undefined,
    receive: (stage, payload) => {
      received.push({ stage, payload });
    },
  };
}

/** A DOM fixture of the page's root: the stage tabs and the Identify page's model fields. */
function stageFixture(): any {
  const dom = new JSDOM(`<main>
    <button data-engineer-stage-tab="model"></button>
    <button data-engineer-stage-tab="tune"></button>
    <button data-engineer-stage-tab="verify"></button>
    <p data-three-page-identification-status></p>
    <div data-studio-model-result hidden>
      <span data-studio-model-type></span>
      <span data-studio-model-equation></span>
      <span data-studio-model-numbers></span>
      <span data-studio-model-warnings></span>
    </div>
  </main>`);
  return dom.window.document.querySelector("main");
}

/** The three-page operation's fixture: the message area of each page and the action buttons. */
function operationFixture(): any {
  const dom = new JSDOM(`<main>
    <div data-studio-message="model"></div>
    <div data-studio-message="tune" hidden></div>
    <div data-studio-message="verify" hidden></div>
    <button data-engineer-find>Identify plant</button>
    <button data-engineer-tune disabled>Tune controller</button>
    <button data-engineer-verify disabled>Validate controller</button>
    <button data-engineer-apply disabled>Apply</button>
  </main>`);
  return dom.window.document.querySelector("main");
}

suite("Control Studio engine run route", () => {
  test("takes the Identify page's start as one run through Identify that identifies again, with the runtime connection, and follows its operation", async () => {
    const { h, context } = harness();
    const taken = await createRunRoute(context).handle(IDENTIFY_START(), SCOPE);
    assert.strictEqual(taken, true);
    assert.deepStrictEqual(h.runRequests, [{
      projectRoot: "/project",
      runtimeId: "runtime-test",
      loopId: "loop-test",
      storageFolder: "",
      connection: CONNECTION,
      through: "identify",
      identifyAgain: true,
    }]);
    assert.deepStrictEqual(h.followed, ["operation-1"]);
    assert.deepStrictEqual(h.failures, []);
  });

  test("sends the run without a connection when no runtime runs, so the engine refuses it with its own next action", async () => {
    const { h, context } = harness();
    h.connection = undefined;
    await createRunRoute(context).handle(IDENTIFY_START(), SCOPE);
    assert.strictEqual(h.runRequests.length, 1);
    assert.strictEqual("connection" in h.runRequests[0], false);
  });

  test("leaves a Tune start, a Validate start, a recording's start and other kinds to their own routes, and sends no run for them", async () => {
    for (const from of ["tune", "verify", "recording"]) {
      const { h, context } = harness();
      const taken = await createRunRoute(context).handle(pageMessage("control-studio.flow.start", { from }), SCOPE);
      assert.strictEqual(taken, false, from);
      assert.deepStrictEqual(h.runRequests, [], from);
    }
    const { h, context } = harness();
    assert.strictEqual(await createRunRoute(context).handle(pageMessage("control-studio.identify.run", { model: "first-order" }), SCOPE), false);
    assert.deepStrictEqual(h.runRequests, []);
  });

  test("sends no run for a loop the engine does not serve, which stays on the TypeScript route", async () => {
    const { h, context } = harness();
    h.loop = undefined;
    assert.strictEqual(await createRunRoute(context).handle(IDENTIFY_START(), SCOPE), false);
    assert.deepStrictEqual(h.runRequests, []);
  });

  test("posts the engine's refusal of the run with its next action and label, and takes the message", async () => {
    const { h, context } = harness();
    const failure: Failure = {
      message: "Connect a runtime to identify the plant.",
      cause: "runtime",
      nextAction: "connect-runtime",
      nextActionLabel: "Connect runtime",
    };
    h.run = { failure };
    assert.strictEqual(await createRunRoute(context).handle(IDENTIFY_START(), SCOPE), true);
    assert.deepStrictEqual(h.failures, [{ method: "run", failure }]);
    assert.deepStrictEqual(h.followed, []);
  });

  test("Abort stops the run the route started while it runs, with controlStudio/cancel", async () => {
    const { h, context } = harness();
    const route = createRunRoute(context);
    await route.handle(IDENTIFY_START(), SCOPE);
    assert.strictEqual(await route.handle(ABORT(), SCOPE), true);
    assert.deepStrictEqual(h.cancelRequests, [{ operationId: "operation-1" }]);
  });

  test("an Abort that finds the run ended leaves the command to the other routes, and sends nothing more", async () => {
    const { h, context } = harness();
    h.cancel = { result: { operationId: "operation-1", command: "run", state: "finished" } };
    const route = createRunRoute(context);
    await route.handle(IDENTIFY_START(), SCOPE);
    assert.strictEqual(await route.handle(ABORT(), SCOPE), false);
    assert.strictEqual(await route.handle(ABORT(), SCOPE), false);
    assert.deepStrictEqual(h.cancelRequests, [{ operationId: "operation-1" }]);
  });

  test("an Abort with no run started is left to the other routes", async () => {
    const { h, context } = harness();
    assert.strictEqual(await createRunRoute(context).handle(ABORT(), SCOPE), false);
    assert.deepStrictEqual(h.cancelRequests, []);
  });

  test("a reset forgets the run, so a later Abort is left to the other routes", async () => {
    const { h, context } = harness();
    const route = createRunRoute(context);
    await route.handle(IDENTIFY_START(), SCOPE);
    route.reset?.();
    assert.strictEqual(await route.handle(ABORT(), SCOPE), false);
    assert.deepStrictEqual(h.cancelRequests, []);
  });

  test("an Abort the engine refuses posts its failure with its next action, and takes the message", async () => {
    const { h, context } = harness();
    const failure: Failure = {
      message: "No Control Studio operation is called operation-1.",
      cause: "implementation",
      nextAction: "none",
    };
    const route = createRunRoute(context);
    await route.handle(IDENTIFY_START(), SCOPE);
    h.cancel = { failure };
    assert.strictEqual(await route.handle(ABORT(), SCOPE), true);
    assert.deepStrictEqual(h.failures, [{ method: "run", failure }]);
  });

  test("the page shows the stages of the run on their tabs, the identification it made, and the evidence its gates use", () => {
    const root = stageFixture();
    const received: Received = [];
    receiveEngineMessage(root, {
      kind: "control-studio.engine.result",
      method: "run",
      result: {
        stages: [
          { stage: "Identify", status: "complete", label: "Complete" },
          { stage: "Tune", status: "ready", label: "Ready" },
          { stage: "Validate", status: "needs-data", label: "Needs data" },
        ],
        identification: { modelType: "first-order", equation: "G(s) = 2 / (10 s + 1)", keyNumbers: "gain 2", provisional: false },
        stageEvidence: { hasModel: true, hasCandidate: false, validationPassed: false },
      },
    }, portsOf(received));
    const modelTab = root.querySelector('[data-engineer-stage-tab="model"]');
    assert.strictEqual(modelTab.dataset.status, "complete");
    assert.strictEqual(modelTab.getAttribute("aria-label"), "Identify, Complete");
    assert.strictEqual(root.querySelector("[data-studio-model-type]").textContent, "first-order");
    assert.strictEqual(root.querySelector("[data-studio-model-equation]").textContent, "G(s) = 2 / (10 s + 1)");
    assert.strictEqual(root.querySelector("[data-studio-model-result]").hasAttribute("hidden"), false);
    assert.deepStrictEqual(received, [{
      stage: "flow",
      payload: { status: "completed", stageEvidence: { hasModel: true, hasCandidate: false, validationPassed: false } },
    }]);
  });

  test("the page names the input the run waits for, and hands the flow to it as paused with that input", () => {
    const root = stageFixture();
    const received: Received = [];
    runReceiver.result?.(root, { stages: [], needs: "limits", stageEvidence: NO_EVIDENCE }, portsOf(received));
    assert.deepStrictEqual(received, [{
      stage: "flow",
      payload: {
        status: "paused",
        state: "paused",
        reason: "Set the experiment limits to identify the plant.",
        missing: "limits",
        stageEvidence: NO_EVIDENCE,
      },
    }]);
  });

  test("the Identify page shows the run's progress while it runs, when the update names no flow stage", () => {
    const root = operationFixture();
    const posted: string[] = [];
    const operation = new ThreePageOperation(root, (kind) => {
      posted.push(kind);
    }, () => undefined, () => undefined);
    operation.start("find", {});
    operation.receive("flow", { status: "running", flowStage: undefined, reason: "Recording the plant" });
    const message = root.querySelector('[data-studio-message="model"]');
    assert.strictEqual(message.textContent, "Recording the plant");
    assert.strictEqual(message.hidden, false);
    assert.deepStrictEqual(posted, ["control-studio.flow.start"]);
  });

  test("after the run, the page offers Tune on the model it identified, and keeps Validate locked until a candidate exists", () => {
    const root = operationFixture();
    const operation = new ThreePageOperation(root, () => undefined, () => undefined, () => undefined);
    operation.start("find", {});
    operation.receive("flow", { status: "completed", stageEvidence: { hasModel: true, hasCandidate: false, validationPassed: false } });
    assert.strictEqual(root.querySelector("[data-engineer-tune]").disabled, false);
    assert.strictEqual(root.querySelector("[data-engineer-verify]").disabled, true);
    assert.strictEqual(root.querySelector("[data-engineer-find]").textContent, "Identify plant");
  });

  test("the run's update names the stage it is in, and an update without a stage or an update that ends the run is left to the flow", () => {
    const root = operationFixture();
    const received: Received = [];
    const ports = portsOf(received);
    runReceiver.update?.(root, { state: "running", stage: "Tune", progress: "Designing the controller" }, ports);
    assert.deepStrictEqual(received, [{
      stage: "flow",
      payload: { status: "running", flowStage: "Tune", reason: "Designing the controller" },
    }]);
    runReceiver.update?.(root, { state: "running", progress: "Recording the plant" }, ports);
    runReceiver.update?.(root, { state: "finished", stage: "Tune" }, ports);
    assert.strictEqual(received.length, 1);
  });

  test("the page selects the tab of the stage the run is in, and keeps the tab the engineer navigated to", () => {
    const root = operationFixture();
    const selected: string[] = [];
    const operation = new ThreePageOperation(root, () => undefined, page => {
      selected.push(page);
    }, () => undefined);
    operation.start("find", {});
    operation.receive("flow", { status: "running", flowStage: "Tune", reason: "Designing the controller" });
    assert.strictEqual(selected[selected.length - 1], "tune");
    assert.strictEqual(root.querySelector('[data-studio-message="tune"]').textContent, "Designing the controller");
    operation.navigated();
    const shown = selected.length;
    operation.receive("flow", { status: "running", flowStage: "Validate", reason: "Validating the controller" });
    assert.strictEqual(selected.length, shown, "a tab the engineer chose stays shown while the run goes on");
  });
});
