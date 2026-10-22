import * as assert from "assert";
import type { ControlStudioEngine, EngineAnswer } from "../../controlStudio/engine/engineClient";
import type { EngineLoop, EngineMethod, EngineRouteContext, EngineScope } from "../../controlStudio/engine/engineRouteContext";
import { createExperimentRoute } from "../../controlStudio/engine/routes/experimentRoute";
import type { Failure } from "../../controlStudio/generated/Failure";
import type { OperationUpdate } from "../../controlStudio/generated/OperationUpdate";
import type { LoopDefinition } from "../../controlStudio/generated/OpenResult";
import { fillProjectTest, renderThreePageAnalysisParameterTemplate, renderThreePageExperimentTemplate } from "../../controlStudio/threePageExperimentTemplate";
import { renderThreePageStudioHtml } from "../../controlStudio/threePageTemplate";
import type { ControlStudioView } from "../../controlStudio/panelTemplate";
import { ThreePageExperiments } from "../../controlStudio/webview/threePageExperiments";

/**
 * The parameter tool of a declared analysis (specification 35, C12 and B6.1): a served loop's review, Apply and Restore of the parameters
 * its analysis maps are the engine's, and the route only names the requests and reports the answers. The rules (which symbols may be
 * written, their bounds, the identity of a review, whether a confirmation applies or restores, the outcome of a write, the tool's offer,
 * the journal) are proven in Rust (`analysis/parameter_writes_tests.rs`, `analysis_parameters_action_tests.rs`, `parameter_tool_tests.rs`).
 * The route is tested with a fake engine and a fake context, as the project test is.
 */

const SCOPE: EngineScope = {
  projectId: "project-test",
  editorSessionId: "editor-test",
  loopId: "loop-analysis",
  loopRevision: "revision-test",
};
/** The served loop. The route names its identifiers only: which tool the loop has, and its mapping, are the engine's (C12). */
const LOOP: EngineLoop = { id: "loop-analysis", runtimeId: "runtime-test" };
const CONNECTION = { endpoint: "tcp://127.0.0.1:9" };
const IDENTITY = "sha256:review-of-the-gain";
const OTHER_IDENTITY = "sha256:review-of-another-gain";
const VALUES = { "P1.Kp": 4, "P1.Kd": 7 };
/** The engine's review of an Apply of `VALUES`: its identity, the values it read from the text (which an apply sends back), and its changes. */
const REVIEWED = { identity: IDENTITY, values: VALUES, changes: [], physical: false };
/** The engine's refusal of a review text that is not JSON (`parameter_json_invalid`). */
const NOT_JSON = { message: "The parameter values are not valid JSON.", cause: "settings", nextAction: "none" } as unknown as Failure;
const FAILURE = { message: "The runtime refused the write.", cause: "runtime", nextAction: "none" } as unknown as Failure;
/** The engine's refusal of an apply whose review no longer matches the record and the values (`REVIEW_CHANGED`, E11.8). */
const CHANGED = {
  message: "The parameter record or the loop changed since the review. Review the values again before the write.",
  cause: "settings",
  nextAction: "none",
} as unknown as Failure;
const OBSERVED = { values: { "P1.Kp": 4, "P1.Kd": 7, "P1.Other": 5 }, status: "applied" };
/** The engine's sentences of a confirmed write (C12, E7.4; `applied_reason` in parameter_application.rs). */
const APPLIED = "Reviewed values observed after ST execution.";
const RESTORED = "Previous values restored and observed after ST execution.";
/** The engine's offer of a free tool after a write (C12, `parameter_tool_offer`). */
const FREE = { nextStep: "review", available: ["inspect", "review", "restore"] };
/** The binding the engine names for its mapped parameters: the analysis's bounds (0 to 3), narrower than the loop's own (-1 to 4). */
const ENGINE_BINDING = {
  loop_id: "loop-analysis",
  task_name: "MainTask",
  parameters: [{ symbol: "P1.Kp", iec_type: "LREAL", unit: "N·m/rad", convention: "gain", minimum: 0, maximum: 3 }],
  mode: { symbol: "P1.ExperimentEnabled", manual_value: false },
};

/** A page message of the loop's scope, with its payload. */
function pageMessage(kind: string, payload: Record<string, unknown>): Record<string, unknown> {
  return {
    schemaVersion: 1,
    kind,
    projectId: SCOPE.projectId,
    editorSessionId: SCOPE.editorSessionId,
    loopId: SCOPE.loopId,
    expectedLoopRevision: SCOPE.loopRevision,
    payload,
  };
}

/** The fake engine and context: each request, answer and listener the test needs. */
function harness(loop: EngineLoop | undefined = LOOP) {
  const h = {
    loop: loop as EngineLoop | undefined,
    connection: CONNECTION as { endpoint: string } | undefined,
    answers: new Map<string, EngineAnswer<unknown>>(),
    requests: [] as Array<Record<string, unknown>>,
    failures: [] as Array<{ method: EngineMethod; failure: Failure }>,
    posted: [] as Array<Record<string, unknown>>,
    listeners: new Map<string, Array<(update: OperationUpdate) => void>>(),
  };
  const client = {
    parameters: async (request: Record<string, unknown>) => {
      h.requests.push(request);
      return (h.answers.get(String(request.action)) ?? { result: {} }) as EngineAnswer<unknown>;
    },
    onUpdate: (operationId: string, listener: (update: OperationUpdate) => void) => {
      h.listeners.set(operationId, [...(h.listeners.get(operationId) ?? []), listener]);
      return {
        dispose: () => {
          h.listeners.set(operationId, (h.listeners.get(operationId) ?? []).filter((item) => item !== listener));
        },
      };
    },
  };
  const context: EngineRouteContext = {
    client: client as unknown as ControlStudioEngine,
    projectRoot: "/project",
    storageFolder: "",
    served: () => h.loop,
    post: async (_scope, message) => {
      h.posted.push(message);
    },
    postFailure: async (_scope, method, failure) => {
      h.failures.push({ method, failure });
    },
    ask: async <T>(_scope: EngineScope, _method: EngineMethod, run: () => Promise<EngineAnswer<T>>) => run(),
    connection: async () => h.connection,
    follow: () => undefined,
  };
  /** Delivers one update of an operation to the listeners the route holds for it. */
  const emit = (operationId: string, update: Record<string, unknown>) => {
    for (const listener of h.listeners.get(operationId) ?? []) listener(update as unknown as OperationUpdate);
  };
  return { h, route: createExperimentRoute(context), emit };
}

/** The last answer the page received for the parameter tool. */
function lastParameters(posted: Array<Record<string, unknown>>): Record<string, unknown> | undefined {
  return posted.filter((message) => message.phase === "parameters").pop()?.payload as Record<string, unknown> | undefined;
}

suite("Control Studio analysis parameters", () => {
  test("the route decides no loop's tool: the request of a loop with no mapping goes to the engine, and the engine's refusal is shown", async () => {
    const { h, route } = harness();
    const refused = {
      message: "This loop declares no parameter mapping for its analysis, so Control Studio writes none of its parameters.",
      cause: "settings",
      nextAction: "open-loop-setup",
    } as unknown as Failure;
    h.answers.set("inspect", { failure: refused });
    assert.strictEqual(await route.handle(pageMessage("control-studio.parameters.inspect", {}), SCOPE), true);
    assert.strictEqual(h.requests.length, 1, "the engine decides which loops have a tool: the experiment's settings declare no mapping either");
    assert.deepStrictEqual(h.failures.map((item) => item.method), ["parameters"]);
    assert.deepStrictEqual(lastParameters(h.posted), { status: "failed", reason: refused.message });
  });

  test("an experiment loop's settings go through the same requests: inspect, review with the typed text and confirm", async () => {
    const { h, route, emit } = harness();
    const settings = { "P1.SelectedStepPoints": 12, "P1.HoldSeconds": 35 };
    h.answers.set("inspect", { result: { values: { "P1.SelectedStepPoints": 10, "P1.HoldSeconds": 30 }, observed: OBSERVED } });
    h.answers.set("review", { result: { identity: IDENTITY, values: settings, changes: [], physical: false } });
    h.answers.set("confirm", { result: { kind: "operationStarted", operationId: "operation-settings" } });
    await route.handle(pageMessage("control-studio.parameters.inspect", {}), SCOPE);
    assert.deepStrictEqual(lastParameters(h.posted), { status: "inspected", values: { "P1.SelectedStepPoints": 10, "P1.HoldSeconds": 30 }, observed: OBSERVED });
    await route.handle(pageMessage("control-studio.parameters.review", { json: JSON.stringify(settings) }), SCOPE);
    assert.strictEqual(h.requests[1]?.json, JSON.stringify(settings), "the text goes to the engine as the engineer typed it");
    await route.handle(pageMessage("control-studio.parameters.apply", { reviewId: IDENTITY, physicalApproved: false }), SCOPE);
    assert.deepStrictEqual(h.requests[2]?.values, settings, "the confirmation names the values the engine read");
    emit("operation-settings", { operationId: "operation-settings", command: "parameters", state: "finished", result: { status: "applied", message: APPLIED, values: settings, observed: OBSERVED, offer: FREE } });
    assert.deepStrictEqual(lastParameters(h.posted)?.status, "applied");
    assert.deepStrictEqual(h.requests.map((request) => request.action), ["inspect", "review", "confirm"]);
  });

  test("inspects the mapped values through the engine, with the runtime connection", async () => {
    const { h, route } = harness();
    h.answers.set("inspect", { result: { values: { "P1.Kp": 4, "P1.Kd": 7 }, observed: OBSERVED } });
    assert.strictEqual(await route.handle(pageMessage("control-studio.parameters.inspect", {}), SCOPE), true);
    assert.deepStrictEqual(h.requests[0], {
      projectRoot: "/project",
      runtimeId: "runtime-test",
      loopId: "loop-analysis",
      storageFolder: "",
      action: "inspect",
      connection: CONNECTION,
    });
    assert.deepStrictEqual(lastParameters(h.posted), { status: "inspected", values: { "P1.Kp": 4, "P1.Kd": 7 }, observed: OBSERVED });
  });

  test("reviews the values the page sends and keeps the identity the engine gives the review", async () => {
    const { h, route } = harness();
    h.answers.set("review", { result: REVIEWED });
    const review = pageMessage("control-studio.parameters.review", { json: JSON.stringify(VALUES) });
    assert.strictEqual(await route.handle(review, SCOPE), true);
    assert.strictEqual(h.requests[0]?.json, JSON.stringify(VALUES), "the text goes to the engine as the engineer typed it");
    assert.ok(!("values" in (h.requests[0] ?? {})), "the route reads no JSON: the engine answers the values it read");
    assert.strictEqual(h.requests[0]?.action, "review");
    assert.deepStrictEqual(lastParameters(h.posted), { status: "reviewed", reviewId: IDENTITY, values: VALUES });
  });

  test("sends a review text that is not JSON to the engine, which refuses it with its own sentence", async () => {
    const { h, route } = harness();
    h.answers.set("review", { failure: NOT_JSON });
    assert.strictEqual(await route.handle(pageMessage("control-studio.parameters.review", { json: "{" }), SCOPE), true);
    assert.strictEqual(h.requests[0]?.json, "{", "the route does not read the text: the engine does");
    assert.deepStrictEqual(h.failures.map((item) => item.method), ["parameters"]);
    assert.deepStrictEqual(lastParameters(h.posted), { status: "failed", reason: NOT_JSON.message });
  });

  test("shows the engine's refusal of a review as the failure and as the page's refusal", async () => {
    const { h, route } = harness();
    h.answers.set("review", { failure: FAILURE });
    await route.handle(pageMessage("control-studio.parameters.review", { json: JSON.stringify(VALUES) }), SCOPE);
    assert.deepStrictEqual(h.failures.map((item) => item.method), ["parameters"]);
    assert.deepStrictEqual(lastParameters(h.posted), { status: "failed", reason: FAILURE.message });
  });

  test("confirms the review it gave, with its identity, and reports the engine's outcome of the write", async () => {
    const { h, route, emit } = harness();
    h.answers.set("review", { result: REVIEWED });
    h.answers.set("confirm", { result: { kind: "operationStarted", operationId: "operation-9" } });
    await route.handle(pageMessage("control-studio.parameters.review", { json: JSON.stringify(VALUES) }), SCOPE);
    await route.handle(pageMessage("control-studio.parameters.apply", { reviewId: IDENTITY, physicalApproved: false }), SCOPE);
    const write = h.requests[1];
    assert.strictEqual(write?.action, "confirm", "the engine decides from the review whether the write applies or restores");
    assert.strictEqual(write?.review, IDENTITY);
    assert.deepStrictEqual(write?.values, VALUES);
    assert.strictEqual(write?.physicalApproved, false);
    emit("operation-9", { operationId: "operation-9", command: "parameters", state: "finished", result: { status: "applied", message: APPLIED, values: { "P1.Kp": 4, "P1.Kd": 7 }, observed: OBSERVED, offer: FREE } });
    assert.deepStrictEqual(lastParameters(h.posted), {
      status: "applied",
      values: { "P1.Kp": 4, "P1.Kd": 7 },
      observed: OBSERVED,
      reason: APPLIED,
      offer: FREE,
    });
  });

  test("reports a write that the engine answers as failed with the engine's sentence, and decides no outcome itself", async () => {
    const { h, route, emit } = harness();
    const notConfirmed = "The reviewed values were not confirmed after controller execution.";
    h.answers.set("review", { result: REVIEWED });
    h.answers.set("confirm", { result: { kind: "operationStarted", operationId: "operation-8" } });
    await route.handle(pageMessage("control-studio.parameters.review", { json: JSON.stringify(VALUES) }), SCOPE);
    await route.handle(pageMessage("control-studio.parameters.apply", { reviewId: IDENTITY, physicalApproved: false }), SCOPE);
    emit("operation-8", { operationId: "operation-8", command: "parameters", state: "finished", result: { status: "failed", message: notConfirmed, offer: FREE } });
    assert.deepStrictEqual(lastParameters(h.posted), { status: "failed", reason: notConfirmed, offer: FREE });
  });

  test("sends a confirmation that names a review the route no longer holds to the engine, which refuses it with its own sentence", async () => {
    const { h, route } = harness();
    h.answers.set("review", { result: REVIEWED });
    h.answers.set("confirm", { failure: CHANGED });
    await route.handle(pageMessage("control-studio.parameters.review", { json: JSON.stringify(VALUES) }), SCOPE);
    await route.handle(pageMessage("control-studio.parameters.apply", { reviewId: OTHER_IDENTITY, physicalApproved: false }), SCOPE);
    // The engine checks the identity against the record and the values it is given (E11.8): the route sends what the page shows.
    assert.deepStrictEqual(h.requests.map((request) => request.action), ["review", "confirm"]);
    assert.strictEqual(h.requests[1]?.review, OTHER_IDENTITY);
    assert.deepStrictEqual(h.requests[1]?.values, VALUES);
    assert.deepStrictEqual(lastParameters(h.posted), { status: "failed", reason: CHANGED.message });
  });

  test("an edit spends the review, and the engine answers the sentence the page shows, with no runtime connection (C12)", async () => {
    const { h, route } = harness();
    const edited = "Values changed. Review again before applying.";
    h.answers.set("review", { result: REVIEWED });
    h.answers.set("invalidate", { result: { message: edited } });
    await route.handle(pageMessage("control-studio.parameters.review", { json: JSON.stringify(VALUES) }), SCOPE);
    await route.handle(pageMessage("control-studio.parameters.invalidate", {}), SCOPE);
    assert.deepStrictEqual(lastParameters(h.posted), { status: "edited", reason: edited });
    assert.deepStrictEqual(h.requests.map((request) => request.action), ["review", "invalidate"]);
    assert.ok(!("connection" in (h.requests[1] ?? {})), "the edit reads no runtime, so it is sent without its connection");
  });

  test("restores the values from before: the review of the retained values, then the confirmation of its identity without values", async () => {
    const { h, route, emit } = harness();
    h.answers.set("reviewRestore", { result: { identity: OTHER_IDENTITY, changes: [], physical: false } });
    h.answers.set("confirm", { result: { kind: "operationStarted", operationId: "operation-restore" } });
    await route.handle(pageMessage("control-studio.parameters.review-rollback", {}), SCOPE);
    assert.deepStrictEqual(lastParameters(h.posted), { status: "reviewed", reviewId: OTHER_IDENTITY });
    await route.handle(pageMessage("control-studio.parameters.apply", { reviewId: OTHER_IDENTITY, physicalApproved: false }), SCOPE);
    const restore = h.requests[1];
    assert.strictEqual(restore?.action, "confirm");
    assert.strictEqual(restore?.review, OTHER_IDENTITY);
    assert.ok(!("values" in (restore ?? {})), "a Restore's review read no values, so the confirmation sends none");
    emit("operation-restore", { operationId: "operation-restore", command: "parameters", state: "finished", result: { status: "applied", message: RESTORED, values: { "P1.Kp": 1, "P1.Kd": 7 }, observed: OBSERVED } });
    assert.deepStrictEqual(lastParameters(h.posted)?.reason, RESTORED, "the sentence is the engine's");
  });

  test("reports a write that its operation ends with a failure, and never as applied", async () => {
    const { h, route, emit } = harness();
    h.answers.set("review", { result: REVIEWED });
    h.answers.set("confirm", { result: { kind: "operationStarted", operationId: "operation-fail" } });
    await route.handle(pageMessage("control-studio.parameters.review", { json: JSON.stringify(VALUES) }), SCOPE);
    await route.handle(pageMessage("control-studio.parameters.apply", { reviewId: IDENTITY, physicalApproved: false }), SCOPE);
    emit("operation-fail", { operationId: "operation-fail", command: "parameters", state: "failed", failure: FAILURE });
    assert.deepStrictEqual(lastParameters(h.posted), { status: "failed", reason: FAILURE.message });
  });
});

suite("Control Studio analysis parameters on the project test page", () => {
  /** A project-scripted loop with one mapped parameter in its binding. */
  function loopWith(analysisParameterSymbols: Record<string, string> | undefined): LoopDefinition {
    const base = {
      id: "loop-analysis",
      name: "Routh-Hurwitz stability",
      signals: {
        processVariable: { symbol: "P1.Angle", unit: "rad" },
        manipulatedVariable: { symbol: "P1.RequestedTorque", unit: "N·m" },
        additional: [],
      },
      experiment: { execution: "project-scripted", kind: "observation", amplitude: 0, durationSeconds: 10, seed: 1 },
      parameterBinding: {
        loop_id: "loop-analysis",
        task_name: "MainTask",
        parameters: [{ symbol: "P1.Kp", iec_type: "LREAL", unit: "N·m/rad", convention: "gain", minimum: -1, maximum: 4 }],
        mode: { symbol: "P1.ExperimentEnabled", manual_value: false },
      },
    };
    return (analysisParameterSymbols ? { ...base, analysisParameterSymbols } : base) as unknown as LoopDefinition;
  }

  /**
   * The project test's form the engine answers for an analysis loop (`projectTestForm`, B6.1, C12): the parameter tool the page fills, and
   * the binding it starts from, which lists no parameter until an engine answer names them.
   */
  function analysisForm(loop?: LoopDefinition) {
    return {
      signals: [{ id: "pv", label: "P1.ProcessValue · Process value", unit: "degC", selected: true }],
      durationSeconds: 10,
      parameterTool: "analysis" as const,
      ...(loop?.parameterBinding ? { parameterBinding: { ...loop.parameterBinding, parameters: [] } } : {}),
      // The tool's first offer, which the engine's form names (C12, `first_tool_offer`): Inspect, which reads only.
      parameterOffer: { nextStep: "inspect" as const, available: ["inspect" as const] },
    };
  }

  test("the Tune page offers an analysis loop's parameters, their step and the Restore review; another loop offers none", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main>${renderThreePageExperimentTemplate()}${renderThreePageAnalysisParameterTemplate()}</main>`);
    const root = dom.window.document.querySelector("main") as HTMLElement;
    try {
      assert.strictEqual(root.querySelector("[data-three-page-analysis-parameter-slot]")!.children.length, 0, "the slot waits for the engine's form");
      fillProjectTest(root, analysisForm());
      const tool = root.querySelector("[data-three-page-analysis-parameter-slot]")!;
      assert.ok(tool.querySelector("[data-three-page-experiment-parameters]"));
      assert.ok(tool.querySelector("[data-three-page-experiment-parameter-action]"), "the parameter tool carries its own step");
      assert.ok(tool.querySelector("[data-three-page-experiment-restore]"));
      // The project test on the Identify page keeps no parameter tool of its own: the engine names the tool it shows.
      assert.strictEqual(root.querySelector("[data-three-page-experiment] [data-three-page-experiment-parameters]"), null);
    } finally {
      dom.window.close();
    }
  });

  /** The loop with a second entry in its binding that no analysis maps: the page must never show it. */
  function withUnmappedParameter(loop: LoopDefinition): LoopDefinition {
    const binding = loop.parameterBinding!;
    const other = { symbol: "P1.Other", iec_type: "LREAL", unit: "N·m/rad", convention: "unmapped", minimum: -10, maximum: 10 };
    return { ...loop, parameterBinding: { ...binding, parameters: [...binding.parameters, other] } } as unknown as LoopDefinition;
  }

  test("the page's fields are the parameters that the engine's answer names, with its bounds; none shows before an answer", async () => {
    const { JSDOM } = require("jsdom");
    const { h, route } = harness();
    h.answers.set("inspect", { result: { values: { "P1.Kp": 2 }, observed: OBSERVED, binding: ENGINE_BINDING, offer: FREE } });
    assert.strictEqual(await route.handle(pageMessage("control-studio.parameters.inspect", {}), SCOPE), true);
    const answer = lastParameters(h.posted);
    assert.deepStrictEqual(answer?.binding, ENGINE_BINDING, "the route passes the binding that the engine names");
    assert.deepStrictEqual(answer?.offer, FREE, "and the tool's offer");
    const loop = withUnmappedParameter(loopWith({ Kp: "P1.Kp" }));
    const dom = new JSDOM(`<main>${renderThreePageExperimentTemplate()}${renderThreePageAnalysisParameterTemplate()}</main>`);
    const root = dom.window.document.querySelector("main") as HTMLElement;
    fillProjectTest(root, analysisForm());
    const original = { window: globalThis.window, document: globalThis.document, CustomEvent: globalThis.CustomEvent };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document, CustomEvent: dom.window.CustomEvent });
    const experiments = new ThreePageExperiments(root, analysisForm(loop) as any, () => undefined);
    const fields = () => Array.from(root.querySelectorAll<HTMLInputElement>("[data-parameter-symbol]")).map(input => input.dataset.parameterSymbol);
    const step = () => root.querySelector<HTMLButtonElement>("[data-three-page-experiment-parameter-action]")!;
    try {
      assert.deepStrictEqual(fields(), [], "before an answer the page names no parameter");
      assert.strictEqual(step().textContent, "Inspect experiment parameters", "the first step is the one the engine's form offers");
      experiments.receive("parameters", answer);
      assert.deepStrictEqual(fields(), ["P1.Kp"], "the page shows the parameter that the engine names, and not the loop's other entry");
      const input = root.querySelector<HTMLInputElement>('[data-parameter-symbol="P1.Kp"]')!;
      assert.deepStrictEqual([input.min, input.max], ["0", "3"], "its bounds are the analysis's, as the engine names them");
      assert.deepStrictEqual(JSON.parse(root.querySelector<HTMLTextAreaElement>("[data-three-page-experiment-parameter-values]")!.value), { "P1.Kp": 2 });
      assert.strictEqual(step().textContent, "Review parameter values", "the step the engine offers next");
      experiments.receive("parameters", { status: "reviewed", reviewId: "review-test", values: { "P1.Kp": 2 }, offer: { nextStep: "apply", available: ["inspect", "review", "apply", "restore"] } });
      assert.deepStrictEqual(fields(), ["P1.Kp"], "a review keeps the fields that the engine named");
      assert.strictEqual(step().textContent, "Apply reviewed values");
    } finally {
      Object.assign(globalThis, original);
      dom.window.close();
    }
  });

  test("the page places an analysis loop's parameter tool on the Tune page, after the analysis it serves", () => {
    const { JSDOM } = require("jsdom");
    const loop = { ...loopWith({ Kp: "P1.Kp" }), controller: { instance: "P1", type: "Main" } } as unknown as LoopDefinition;
    const html = renderThreePageStudioHtml({ cspSource: "x", nonce: "n", chartScript: "chart.js", workflowScript: "workflow.js", project: "p",
      session: "s", loop: "loop-analysis", loopRevision: "r", revision: "v", processUnit: "PV", outputUnit: "MV", loopDefinition: loop } as unknown as ControlStudioView,
    (value: string) => value);
    const dom = new JSDOM(html);
    try {
      const page = (id: string) => dom.window.document.querySelector(`#${id}`)!;
      // The engine's form names the tool, and the page fills the Tune page's slot with it (B6.1, C12).
      fillProjectTest(dom.window.document.querySelector("main") as HTMLElement, analysisForm());
      const tool = page("studio-page-tune").querySelector("[data-three-page-experiment-parameters]");
      assert.ok(tool, "the Tune page holds the analysis parameter tool");
      assert.strictEqual(page("studio-page-model").querySelector("[data-three-page-experiment-parameters]"), null, "the Identify page holds no analysis parameter tool");
      const analysis = page("studio-page-tune").querySelector("[data-three-page-analysis]")!;
      assert.ok(analysis.compareDocumentPosition(tool!) & dom.window.Node.DOCUMENT_POSITION_FOLLOWING, "the tool follows the analysis it serves");
    } finally { dom.window.close(); }
  });
});
