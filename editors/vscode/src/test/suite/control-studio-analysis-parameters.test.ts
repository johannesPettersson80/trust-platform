import * as assert from "assert";
import type { ControlStudioEngine, EngineAnswer } from "../../controlStudio/engine/engineClient";
import type { EngineLoop, EngineMethod, EngineRouteContext } from "../../controlStudio/engine/engineRouteContext";
import { createExperimentRoute } from "../../controlStudio/engine/routes/experimentRoute";
import type { Failure } from "../../controlStudio/generated/Failure";
import type { OperationUpdate } from "../../controlStudio/generated/OperationUpdate";
import type { LoopMessageScope } from "../../controlStudio/messages";
import type { ControlLoopDefinition } from "../../controlStudio/loopDefinition";
import { renderThreePageAnalysisParameterTemplate, renderThreePageExperimentTemplate } from "../../controlStudio/threePageExperimentTemplate";
import { renderThreePageStudioHtml } from "../../controlStudio/threePageTemplate";
import type { ControlStudioView } from "../../controlStudio/panelTemplate";
import { ThreePageExperiments } from "../../controlStudio/webview/threePageExperiments";

/**
 * The parameter tool of a declared analysis (specification 35, C12 and B6.1): a served loop's review, Apply and Restore of the parameters
 * its analysis maps are the engine's, and the route only names the requests and reports the answers. The rules (which symbols may be
 * written, their bounds, the identity of a review, the journal) are proven in Rust (`analysis/parameter_writes_tests.rs`,
 * `analysis_parameters_action_tests.rs`). The route is tested with a fake engine and a fake context, as the project test is.
 */

const SCOPE: LoopMessageScope = {
  projectId: "project-test",
  editorSessionId: "editor-test",
  loopId: "loop-analysis",
  loopRevision: "revision-test",
};
/** A loop whose declared analysis maps two parameters, as lesson 45 does. */
const MAPPED_LOOP: EngineLoop = {
  id: "loop-analysis",
  runtimeId: "runtime-test",
  analysisParameterSymbols: { Kp: "P1.Kp", Kd: "P1.Kd" },
};
/** A loop without a mapping: it has no parameter tool, whatever the page sends. */
const UNMAPPED_LOOP: EngineLoop = { id: "loop-analysis", runtimeId: "runtime-test" };
const CONNECTION = { endpoint: "tcp://127.0.0.1:9" };
const IDENTITY = "sha256:review-of-the-gain";
const OTHER_IDENTITY = "sha256:review-of-another-gain";
const VALUES = { "P1.Kp": 4, "P1.Kd": 7 };
const FAILURE = { message: "The runtime refused the write.", cause: "runtime", nextAction: "none" } as unknown as Failure;
const OBSERVED = { values: { "P1.Kp": 4, "P1.Kd": 7, "P1.Other": 5 }, status: "applied" };
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
function harness(loop: EngineLoop | undefined = MAPPED_LOOP) {
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
    ask: async <T>(_scope: LoopMessageScope, _method: EngineMethod, run: () => Promise<EngineAnswer<T>>) => run(),
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
  test("a loop whose analysis maps no parameter has no tool: the page is told so, and the engine is asked nothing", async () => {
    const { h, route } = harness(UNMAPPED_LOOP);
    assert.strictEqual(await route.handle(pageMessage("control-studio.parameters.inspect", {}), SCOPE), true);
    assert.strictEqual(h.requests.length, 0);
    assert.deepStrictEqual(lastParameters(h.posted), { status: "unavailable" });
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
    h.answers.set("review", { result: { identity: IDENTITY, changes: [], physical: false } });
    const review = pageMessage("control-studio.parameters.review", { json: JSON.stringify(VALUES) });
    assert.strictEqual(await route.handle(review, SCOPE), true);
    assert.deepStrictEqual(h.requests[0]?.values, VALUES);
    assert.strictEqual(h.requests[0]?.action, "review");
    assert.deepStrictEqual(lastParameters(h.posted), { status: "reviewed", reviewId: IDENTITY, values: VALUES, rollback: false });
  });

  test("refuses a review text that is not JSON without asking the engine", async () => {
    const { h, route } = harness();
    assert.strictEqual(await route.handle(pageMessage("control-studio.parameters.review", { json: "{" }), SCOPE), true);
    assert.strictEqual(h.requests.length, 0);
    assert.deepStrictEqual(lastParameters(h.posted), { status: "failed", reason: "The parameter values are not valid JSON." });
  });

  test("shows the engine's refusal of a review as the failure and as the page's refusal", async () => {
    const { h, route } = harness();
    h.answers.set("review", { failure: FAILURE });
    await route.handle(pageMessage("control-studio.parameters.review", { json: JSON.stringify(VALUES) }), SCOPE);
    assert.deepStrictEqual(h.failures.map((item) => item.method), ["parameters"]);
    assert.deepStrictEqual(lastParameters(h.posted), { status: "failed", reason: FAILURE.message });
  });

  test("applies the review it gave, with its identity, and reports the record the runtime confirmed", async () => {
    const { h, route, emit } = harness();
    h.answers.set("review", { result: { identity: IDENTITY, changes: [], physical: false } });
    h.answers.set("apply", { result: { operationId: "operation-9" } });
    await route.handle(pageMessage("control-studio.parameters.review", { json: JSON.stringify(VALUES) }), SCOPE);
    await route.handle(pageMessage("control-studio.parameters.apply", { reviewId: IDENTITY, physicalApproved: false }), SCOPE);
    const write = h.requests[1];
    assert.strictEqual(write?.action, "apply");
    assert.strictEqual(write?.review, IDENTITY);
    assert.deepStrictEqual(write?.values, VALUES);
    assert.strictEqual(write?.physicalApproved, false);
    emit("operation-9", { operationId: "operation-9", command: "parameters", state: "finished", result: { values: { "P1.Kp": 4, "P1.Kd": 7 }, observed: OBSERVED } });
    assert.deepStrictEqual(lastParameters(h.posted), {
      status: "applied",
      values: { "P1.Kp": 4, "P1.Kd": 7 },
      observed: OBSERVED,
      reason: "Reviewed values observed after ST execution.",
    });
  });

  test("refuses to apply a review that is not the one the page holds, and asks the engine nothing", async () => {
    const { h, route } = harness();
    h.answers.set("review", { result: { identity: IDENTITY, changes: [], physical: false } });
    await route.handle(pageMessage("control-studio.parameters.review", { json: JSON.stringify(VALUES) }), SCOPE);
    await route.handle(pageMessage("control-studio.parameters.apply", { reviewId: OTHER_IDENTITY, physicalApproved: false }), SCOPE);
    assert.deepStrictEqual(h.requests.map((request) => request.action), ["review"]);
    assert.deepStrictEqual(lastParameters(h.posted), { status: "failed", reason: "This review expired. Review the exact values again." });
  });

  test("an edit spends the review: the values are reviewed again before they are applied", async () => {
    const { h, route } = harness();
    h.answers.set("review", { result: { identity: IDENTITY, changes: [], physical: false } });
    await route.handle(pageMessage("control-studio.parameters.review", { json: JSON.stringify(VALUES) }), SCOPE);
    await route.handle(pageMessage("control-studio.parameters.invalidate", {}), SCOPE);
    assert.deepStrictEqual(lastParameters(h.posted), { status: "edited", reason: "Values changed. Review again before applying." });
    await route.handle(pageMessage("control-studio.parameters.apply", { reviewId: IDENTITY, physicalApproved: false }), SCOPE);
    assert.deepStrictEqual(h.requests.map((request) => request.action), ["review"]);
  });

  test("restores the values from before: the review of the retained values, then the engine's restore without values", async () => {
    const { h, route, emit } = harness();
    h.answers.set("reviewRestore", { result: { identity: OTHER_IDENTITY, changes: [], physical: false } });
    h.answers.set("restore", { result: { operationId: "operation-restore" } });
    await route.handle(pageMessage("control-studio.parameters.review-rollback", {}), SCOPE);
    assert.deepStrictEqual(lastParameters(h.posted), { status: "reviewed", reviewId: OTHER_IDENTITY, values: {}, rollback: true });
    await route.handle(pageMessage("control-studio.parameters.apply", { reviewId: OTHER_IDENTITY, physicalApproved: false }), SCOPE);
    const restore = h.requests[1];
    assert.strictEqual(restore?.action, "restore");
    assert.strictEqual(restore?.review, OTHER_IDENTITY);
    assert.ok(!("values" in (restore ?? {})), "a restore sets the values the runtime retains; the page sends none");
    emit("operation-restore", { operationId: "operation-restore", command: "parameters", state: "finished", result: { values: { "P1.Kp": 1, "P1.Kd": 7 }, observed: OBSERVED } });
    assert.deepStrictEqual(lastParameters(h.posted)?.reason, "Previous values restored and observed after ST execution.");
  });

  test("reports a write that its operation ends with a failure, and never as applied", async () => {
    const { h, route, emit } = harness();
    h.answers.set("review", { result: { identity: IDENTITY, changes: [], physical: false } });
    h.answers.set("apply", { result: { operationId: "operation-fail" } });
    await route.handle(pageMessage("control-studio.parameters.review", { json: JSON.stringify(VALUES) }), SCOPE);
    await route.handle(pageMessage("control-studio.parameters.apply", { reviewId: IDENTITY, physicalApproved: false }), SCOPE);
    emit("operation-fail", { operationId: "operation-fail", command: "parameters", state: "failed", failure: FAILURE });
    assert.deepStrictEqual(lastParameters(h.posted), { status: "failed", reason: FAILURE.message });
  });

  test("answers the analysis's own choice as a draft for a review, without a runtime", async () => {
    const { h, route } = harness();
    h.answers.set("draft", { result: { values: { "P1.Kp": 16, "P1.Kd": 7 } } });
    await route.handle(pageMessage("control-studio.parameters.selected-analysis", {}), SCOPE);
    assert.strictEqual(h.requests[0]?.action, "draft");
    assert.ok(!("connection" in (h.requests[0] ?? {})), "the draft reads no runtime");
    assert.deepStrictEqual(lastParameters(h.posted), {
      status: "draft",
      values: { "P1.Kp": 16, "P1.Kd": 7 },
      reason: "Selected analysis values copied for review. The runtime has not changed.",
    });
  });
});

suite("Control Studio analysis parameters on the project test page", () => {
  /** A project-scripted loop with one mapped parameter in its binding. */
  function loopWith(analysisParameterSymbols: Record<string, string> | undefined): ControlLoopDefinition {
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
    return (analysisParameterSymbols ? { ...base, analysisParameterSymbols } : base) as unknown as ControlLoopDefinition;
  }

  test("the Tune page offers an analysis loop's parameters, their step and the Restore review; another loop offers none", () => {
    const escape = (value: string) => value;
    const analysis = renderThreePageAnalysisParameterTemplate(loopWith({ Kp: "P1.Kp" }));
    assert.ok(analysis.includes("data-three-page-experiment-parameters"));
    assert.ok(analysis.includes("data-three-page-experiment-parameter-action"), "the parameter tool carries its own step");
    assert.ok(analysis.includes("data-three-page-experiment-restore"));
    assert.strictEqual(renderThreePageAnalysisParameterTemplate(loopWith(undefined)), "");
    // The project test on the Identify page keeps no analysis parameters: its own settings belong to experiment loops only.
    const projectTest = renderThreePageExperimentTemplate(loopWith({ Kp: "P1.Kp" }), escape);
    assert.ok(!projectTest.includes("data-three-page-experiment-parameters"));
    assert.ok(!projectTest.includes("data-three-page-experiment-restore"));
  });

  /** The loop with a second entry in its binding that no analysis maps: the page must never show it. */
  function withUnmappedParameter(loop: ControlLoopDefinition): ControlLoopDefinition {
    const binding = loop.parameterBinding!;
    const other = { symbol: "P1.Other", iec_type: "LREAL", unit: "N·m/rad", convention: "unmapped", minimum: -10, maximum: 10 };
    return { ...loop, parameterBinding: { ...binding, parameters: [...binding.parameters, other] } } as unknown as ControlLoopDefinition;
  }

  test("the page's fields are the parameters that the engine's answer names, with its bounds; none shows before an answer", async () => {
    const { JSDOM } = require("jsdom");
    const { h, route } = harness();
    h.answers.set("inspect", { result: { values: { "P1.Kp": 2 }, observed: OBSERVED, binding: ENGINE_BINDING } });
    assert.strictEqual(await route.handle(pageMessage("control-studio.parameters.inspect", {}), SCOPE), true);
    const answer = lastParameters(h.posted);
    assert.deepStrictEqual(answer?.binding, ENGINE_BINDING, "the route passes the binding that the engine names");
    const loop = withUnmappedParameter(loopWith({ Kp: "P1.Kp" }));
    const dom = new JSDOM(`<main>${renderThreePageExperimentTemplate(loop, (value: string) => value)}${renderThreePageAnalysisParameterTemplate(loop)}</main>`);
    const root = dom.window.document.querySelector("main") as HTMLElement;
    const original = { window: globalThis.window, document: globalThis.document, CustomEvent: globalThis.CustomEvent };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document, CustomEvent: dom.window.CustomEvent });
    const experiments = new ThreePageExperiments(root, loop, () => undefined);
    const fields = () => Array.from(root.querySelectorAll<HTMLInputElement>("[data-parameter-symbol]")).map(input => input.dataset.parameterSymbol);
    const step = () => root.querySelector<HTMLButtonElement>("[data-three-page-experiment-parameter-action]")!;
    try {
      assert.deepStrictEqual(fields(), [], "before an answer the page names no parameter");
      assert.strictEqual(step().textContent, "Inspect experiment parameters");
      experiments.receive("parameters", answer);
      assert.deepStrictEqual(fields(), ["P1.Kp"], "the page shows the parameter that the engine names, and not the loop's other entry");
      const input = root.querySelector<HTMLInputElement>('[data-parameter-symbol="P1.Kp"]')!;
      assert.deepStrictEqual([input.min, input.max], ["0", "3"], "its bounds are the analysis's, as the engine names them");
      assert.deepStrictEqual(JSON.parse(root.querySelector<HTMLTextAreaElement>("[data-three-page-experiment-parameter-values]")!.value), { "P1.Kp": 2 });
      experiments.receive("parameters", { status: "reviewed", reviewId: "review-test", values: { "P1.Kp": 2 } });
      assert.deepStrictEqual(fields(), ["P1.Kp"], "a review keeps the fields that the engine named");
      assert.strictEqual(step().textContent, "Apply reviewed values");
    } finally {
      Object.assign(globalThis, original);
      dom.window.close();
    }
  });

  test("the page places an analysis loop's parameter tool on the Tune page, after the analysis it serves", () => {
    const { JSDOM } = require("jsdom");
    const loop = { ...loopWith({ Kp: "P1.Kp" }), controller: { instance: "P1", type: "Main" } } as unknown as ControlLoopDefinition;
    const html = renderThreePageStudioHtml({ cspSource: "x", nonce: "n", chartScript: "chart.js", workflowScript: "workflow.js", project: "p",
      session: "s", loop: "loop-analysis", loopRevision: "r", revision: "v", processUnit: "PV", outputUnit: "MV", loopDefinition: loop } as unknown as ControlStudioView,
    (value: string) => value);
    const dom = new JSDOM(html);
    try {
      const page = (id: string) => dom.window.document.querySelector(`#${id}`)!;
      const tool = page("studio-page-tune").querySelector("[data-three-page-experiment-parameters]");
      assert.ok(tool, "the Tune page holds the analysis parameter tool");
      assert.strictEqual(page("studio-page-model").querySelector("[data-three-page-experiment-parameters]"), null, "the Identify page holds no analysis parameter tool");
      const analysis = page("studio-page-tune").querySelector("[data-three-page-analysis]")!;
      assert.ok(analysis.compareDocumentPosition(tool!) & dom.window.Node.DOCUMENT_POSITION_FOLLOWING, "the tool follows the analysis it serves");
    } finally { dom.window.close(); }
  });
});
