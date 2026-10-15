import * as assert from "assert";
import type { ControlStudioEngine, EngineAnswer, IdentifyAnswer } from "../../controlStudio/engine/engineClient";
import type { EngineLoop, EngineMethod, EngineRouteContext } from "../../controlStudio/engine/engineRouteContext";
import { createExperimentRoute } from "../../controlStudio/engine/routes/experimentRoute";
import type { Failure } from "../../controlStudio/generated/Failure";
import type { OperationUpdate } from "../../controlStudio/generated/OperationUpdate";
import type { LoopMessageScope } from "../../controlStudio/messages";
import { renderThreePageExperimentTemplate } from "../../controlStudio/threePageExperimentTemplate";
import { ThreePageExperiments } from "../../controlStudio/webview/threePageExperiments";

/**
 * The project test of a served loop (specification 35, C12 and §0.1a). The route is tested with a fake engine and a fake context:
 * each request it sends is recorded, and each answer is what the test sets. The page's experiment section is tested on a DOM fixture.
 * The rules are the engine's and are proven in Rust (`identify_review_tests.rs`, `campaign_review_tests.rs`): the identity of a review,
 * the approval of a physical target and the refusal of a start that no longer matches its review.
 */

const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };

const SCOPE: LoopMessageScope = {
  projectId: "project-test",
  editorSessionId: "editor-test",
  loopId: "loop-test",
  loopRevision: "revision-test",
};
const LOOP: EngineLoop = { id: "loop-test", runtimeId: "runtime-test" };
const CONNECTION = { endpoint: "tcp://127.0.0.1:9" };
const FAILURE = { message: "The engine cannot review this test.", cause: "settings", nextAction: "none" } as unknown as Failure;
/** The identity the engine gives the review of the simulated test, and the identity of a physical one. */
const REVIEW_ID = "sha256:review-of-the-test";
const PHYSICAL_ID = "sha256:physical-test";
/** The engine's refusal of a start whose review no longer describes the campaign (E11.8). */
const CHANGED_FAILURE = {
  message: "The campaign changed since it was reviewed: its loop, its mode or its settings differ. Review it again before it starts.",
  cause: "approval",
  nextAction: "none",
} as unknown as Failure;
const REVIEW = {
  identity: REVIEW_ID,
  kind: "project_sequence",
  automatic: true,
  manual: false,
  execution: "project-scripted",
  inputRange: { minimum: 0, maximum: 100 },
  outputRange: { minimum: 0, maximum: 80 },
  maximumDurationSeconds: 30,
  amplitude: 0,
  projectScripted: {
    baselineSeconds: 5,
    activation: { symbol: "P1.ExperimentEnabled", address: "%MX24.1" },
    abort: { symbol: "P1.ExperimentAbort", address: "%MX24.2", assertedValue: true },
    restoration: { activationRestoredTo: false, confirmedAfterExecutions: 3 },
    stopConditions: ["Abort", "30 s timeout", "Abort or failure asserts P1.ExperimentAbort = true; normal completion does not."],
  },
};
/** The review of the same test on a physical target: it carries the approval block the engineer approves (A2.5, B9.3). */
const PHYSICAL_REVIEW = {
  ...REVIEW,
  identity: PHYSICAL_ID,
  approval: {
    loopId: "loop-test",
    signals: [{ symbol: "P1.ExperimentEnabled", unit: "", minimum: null, maximum: null }],
    amplitude: 0,
    durationSeconds: 30,
    stopConditions: REVIEW.projectScripted.stopConditions,
    restoration: "The activation returns to FALSE however the test ends, and its restoration is confirmed after 3 executions of the loop's task (C12, A7).",
  },
};
const SETTINGS = { method: "automatic", durationSeconds: 30, amplitude: 0, reuseRecordings: false };
/** The whole refusal of a fit that the recording cannot yet support, as the engine words it (plant.rs); a finished test carries it as its note. */
const FIT_REFUSAL = "The response is still rising like a ramp after 0.03 min; it cannot yet be told whether it levels off. Hold the step for about 0.9 min (Limits… → maximum duration), or identify with a setpoint step in Auto.";

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

/** The declared project test the page asks for. */
function testRequest(durationSeconds = 30): Record<string, unknown> {
  return pageMessage("control-studio.experiment.review", { kind: "observation", amplitude: 0, durationSeconds, physicalTopologyReviewed: false });
}

/** The engine and the context of the route, with every request, answer and listener the test needs. */
function harness() {
  const h = {
    loop: LOOP as EngineLoop | undefined,
    connection: CONNECTION as { endpoint: string } | undefined,
    review: { result: REVIEW as unknown as IdentifyAnswer } as EngineAnswer<IdentifyAnswer>,
    start: { result: { operationId: "operation-1" } as unknown as IdentifyAnswer } as EngineAnswer<IdentifyAnswer>,
    cancel: { result: { operationId: "operation-1", command: "identify", state: "cancelled" } as unknown as OperationUpdate } as EngineAnswer<OperationUpdate>,
    identifyRequests: [] as Array<{ action: { action: string; settings?: Record<string, unknown> }; connection?: unknown }>,
    cancelRequests: [] as Array<Record<string, unknown>>,
    failures: [] as Array<{ method: EngineMethod; failure: Failure }>,
    followed: [] as string[],
    posted: [] as Array<Record<string, unknown>>,
    listeners: new Map<string, Array<(update: OperationUpdate) => void>>(),
    disposed: [] as string[],
  };
  const client = {
    identify: async (request: unknown) => {
      h.identifyRequests.push(request as (typeof h.identifyRequests)[number]);
      return (request as (typeof h.identifyRequests)[number]).action.action === "review" ? h.review : h.start;
    },
    cancel: async (request: Record<string, unknown>) => {
      h.cancelRequests.push(request);
      return h.cancel;
    },
    onUpdate: (operationId: string, listener: (update: OperationUpdate) => void) => {
      h.listeners.set(operationId, [...(h.listeners.get(operationId) ?? []), listener]);
      return {
        dispose: () => {
          h.disposed.push(operationId);
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
    follow: (operationId) => {
      h.followed.push(operationId);
    },
  };
  /** Delivers one update of a campaign to the listeners the route holds for it. */
  const emit = (operationId: string, update: Record<string, unknown>) => {
    for (const listener of h.listeners.get(operationId) ?? []) listener(update as unknown as OperationUpdate);
  };
  return { h, context, route: createExperimentRoute(context), emit };
}

/** The last answer the page received for one test phase. */
function lastPhase(posted: Array<Record<string, unknown>>, phase: string): Record<string, unknown> | undefined {
  return posted.filter((message) => message.phase === phase).pop();
}

suite("Control Studio engine project test", () => {
  test("asks the engine to review the declared project test and answers the review with the engine's identity", async () => {
    const { h, route } = harness();
    assert.strictEqual(await route.handle(testRequest(), SCOPE), true);
    assert.deepStrictEqual(h.identifyRequests[0]?.action, { action: "review", settings: SETTINGS });
    assert.strictEqual(h.identifyRequests[0]?.connection, undefined, "a review needs no runtime connection");
    assert.strictEqual(lastPhase(h.posted, "review")?.kind, "control-studio.experiment.result");
    assert.deepStrictEqual(lastPhase(h.posted, "review")?.payload, { kind: "reviewed", planId: REVIEW_ID, review: REVIEW });
  });

  test("refuses a request for anything but the declared observation, and asks the engine nothing", async () => {
    const { h, route } = harness();
    const load = pageMessage("control-studio.experiment.review", { kind: "load_step", amplitude: 0, durationSeconds: 30 });
    assert.strictEqual(await route.handle(load, SCOPE), true);
    assert.strictEqual(h.identifyRequests.length, 0);
    assert.deepStrictEqual(lastPhase(h.posted, "review")?.payload, { kind: "rejected" });
  });

  test("forwards a declared duration as it is, so that the engine checks its range", async () => {
    const { h, route } = harness();
    assert.strictEqual(await route.handle(testRequest(0.5), SCOPE), true);
    assert.deepStrictEqual(h.identifyRequests[0]?.action, { action: "review", settings: { ...SETTINGS, durationSeconds: 0.5 } });
  });

  test("shows the engine's refusal of the review as the failure and as the test's refusal", async () => {
    const { h, route } = harness();
    h.review = { failure: FAILURE };
    assert.strictEqual(await route.handle(testRequest(), SCOPE), true);
    assert.deepStrictEqual(h.failures.map((item) => item.method), ["identify"]);
    assert.deepStrictEqual(lastPhase(h.posted, "review")?.payload, { kind: "rejected", reason: FAILURE.message });
  });

  test("starts the campaign of the confirmed review with the same settings, the identity and the runtime connection", async () => {
    const { h, route } = harness();
    await route.handle(testRequest(), SCOPE);
    const approve = pageMessage("control-studio.experiment.approve", { planId: REVIEW_ID, physicalApproved: false });
    assert.strictEqual(await route.handle(approve, SCOPE), true);
    assert.deepStrictEqual(h.identifyRequests[1]?.action, { action: "identify", settings: SETTINGS, review: REVIEW_ID, physicalApproved: false });
    assert.deepStrictEqual(h.identifyRequests[1]?.connection, CONNECTION);
    assert.deepStrictEqual(h.followed, ["operation-1"]);
    assert.deepStrictEqual(lastPhase(h.posted, "running")?.payload, { experiment_id: "operation-1" });
  });

  test("sends the identity the page confirms and leaves the match to the engine", async () => {
    const { h, route } = harness();
    await route.handle(testRequest(), SCOPE);
    const older = pageMessage("control-studio.experiment.approve", { planId: "sha256:an-older-review", physicalApproved: false });
    assert.strictEqual(await route.handle(older, SCOPE), true);
    assert.strictEqual(h.identifyRequests.length, 2, "the engine decides whether the identity still matches");
    assert.deepStrictEqual(h.identifyRequests[1]?.action, { action: "identify", settings: SETTINGS, review: "sha256:an-older-review", physicalApproved: false });
  });

  test("forwards the physical approval the page gives, with the identity it confirms", async () => {
    const { h, route } = harness();
    await route.handle(testRequest(), SCOPE);
    await route.handle(pageMessage("control-studio.experiment.approve", { planId: REVIEW_ID, physicalApproved: true }), SCOPE);
    assert.deepStrictEqual(h.identifyRequests[1]?.action, { action: "identify", settings: SETTINGS, review: REVIEW_ID, physicalApproved: true });
  });

  test("shows the engine's refusal of a start whose review changed, and a refused start consumes the review", async () => {
    const { h, route } = harness();
    await route.handle(testRequest(), SCOPE);
    h.start = { failure: CHANGED_FAILURE };
    await route.handle(pageMessage("control-studio.experiment.approve", { planId: REVIEW_ID, physicalApproved: false }), SCOPE);
    assert.deepStrictEqual(h.failures.map((item) => item.method), ["identify"]);
    assert.deepStrictEqual(lastPhase(h.posted, "failed")?.payload, { reason: CHANGED_FAILURE.message });
    await route.handle(pageMessage("control-studio.experiment.approve", { planId: REVIEW_ID, physicalApproved: false }), SCOPE);
    assert.strictEqual(h.identifyRequests.length, 2, "the second start asks the engine nothing: the page reviews again first");
    assert.deepStrictEqual(lastPhase(h.posted, "failed")?.payload, { reason: "The reviewed plan changed. Review it again before starting." });
  });

  test("a start the engine does not answer shows the start as failed, and the page is not left running", async () => {
    const { h, route } = harness();
    await route.handle(testRequest(), SCOPE);
    h.start = undefined as unknown as EngineAnswer<IdentifyAnswer>;
    await route.handle(pageMessage("control-studio.experiment.approve", { planId: REVIEW_ID, physicalApproved: false }), SCOPE);
    assert.deepStrictEqual(lastPhase(h.posted, "failed")?.payload, {}, "the engine served no answer: the page shows the start as failed");
    assert.strictEqual(h.identifyRequests.length, 2);
  });

  test("Abort stops the running campaign through controlStudio/cancel", async () => {
    const { h, route } = harness();
    await route.handle(testRequest(), SCOPE);
    await route.handle(pageMessage("control-studio.experiment.approve", { planId: REVIEW_ID, physicalApproved: false }), SCOPE);
    assert.strictEqual(await route.handle(pageMessage("control-studio.experiment.abort", { experimentId: "operation-1" }), SCOPE), true);
    assert.deepStrictEqual(h.cancelRequests, [{ operationId: "operation-1" }]);
  });

  test("the page's Cancel stops a running test, and an idle Cancel is left to the other routes", async () => {
    const { h, route } = harness();
    assert.strictEqual(await route.handle(pageMessage("control-studio.flow.cancel", {}), SCOPE), false);
    await route.handle(testRequest(), SCOPE);
    await route.handle(pageMessage("control-studio.experiment.approve", { planId: REVIEW_ID, physicalApproved: false }), SCOPE);
    assert.strictEqual(await route.handle(pageMessage("control-studio.flow.cancel", {}), SCOPE), true);
    assert.deepStrictEqual(h.cancelRequests, [{ operationId: "operation-1" }]);
  });

  test("a finished campaign ends the test on the page once, and releases its listener", async () => {
    const { h, route, emit } = harness();
    await route.handle(testRequest(), SCOPE);
    await route.handle(pageMessage("control-studio.experiment.approve", { planId: REVIEW_ID, physicalApproved: false }), SCOPE);
    emit("operation-1", { operationId: "operation-1", command: "identify", state: "finished" });
    emit("operation-1", { operationId: "operation-1", command: "identify", state: "finished" });
    const ended = h.posted.filter((message) => message.phase === "engine-ended");
    assert.deepStrictEqual(ended.map((message) => message.payload), [{ status: "finished" }]);
    assert.deepStrictEqual(h.disposed, ["operation-1"]);
  });

  test("a failed campaign reports the engine's reason as the end of the test", async () => {
    const { h, route, emit } = harness();
    await route.handle(testRequest(), SCOPE);
    await route.handle(pageMessage("control-studio.experiment.approve", { planId: REVIEW_ID, physicalApproved: false }), SCOPE);
    emit("operation-1", { operationId: "operation-1", command: "identify", state: "failed", failure: { message: "Restoration not confirmed." } });
    assert.deepStrictEqual(lastPhase(h.posted, "engine-ended")?.payload, { status: "failed", reason: "Restoration not confirmed." });
  });

  test("a finished campaign names the restoration the engine confirmed, which the page's end carries", async () => {
    const { h, route, emit } = harness();
    await route.handle(testRequest(), SCOPE);
    await route.handle(pageMessage("control-studio.experiment.approve", { planId: REVIEW_ID, physicalApproved: false }), SCOPE);
    emit("operation-1", { operationId: "operation-1", command: "identify", state: "finished", result: { restoration: "restored" } } as unknown as OperationUpdate);
    assert.deepStrictEqual(lastPhase(h.posted, "engine-ended")?.payload, { status: "finished", restoration: "restored" });
  });

  test("a finished campaign whose fit refused carries the whole refusal to the page's end as its note", async () => {
    const { h, route, emit } = harness();
    await route.handle(testRequest(), SCOPE);
    await route.handle(pageMessage("control-studio.experiment.approve", { planId: REVIEW_ID, physicalApproved: false }), SCOPE);
    emit("operation-1", { operationId: "operation-1", command: "identify", state: "finished", result: { restoration: "restored", fitRefusal: FIT_REFUSAL } } as unknown as OperationUpdate);
    assert.deepStrictEqual(lastPhase(h.posted, "engine-ended")?.payload, { status: "finished", restoration: "restored", note: FIT_REFUSAL });
  });

  test("a refused start shows the whole refusal, its detail, and not only its first sentence", async () => {
    const { h, route } = harness();
    await route.handle(testRequest(), SCOPE);
    h.start = { failure: { message: "The test cannot start.", detail: "The test cannot start. Its review is spent, so review it again.", cause: "settings", nextAction: "none" } as unknown as Failure } as unknown as EngineAnswer<IdentifyAnswer>;
    await route.handle(pageMessage("control-studio.experiment.approve", { planId: REVIEW_ID, physicalApproved: false }), SCOPE);
    assert.deepStrictEqual(lastPhase(h.posted, "failed")?.payload, { reason: "The test cannot start. Its review is spent, so review it again." });
  });

  test("Abort with no campaign running ends the page's busy state without asking the engine", async () => {
    const { h, route } = harness();
    assert.strictEqual(await route.handle(pageMessage("control-studio.experiment.abort", { experimentId: "stale" }), SCOPE), true);
    assert.strictEqual(h.cancelRequests.length, 0);
    assert.deepStrictEqual(lastPhase(h.posted, "engine-ended")?.payload, { status: "unknown", reason: "No test is running." });
  });

  test("the manual parameter tool is refused for a served loop, and nothing reaches the runtime", async () => {
    const { h, route } = harness();
    for (const kind of ["inspect", "review", "apply", "invalidate"]) {
      assert.strictEqual(await route.handle(pageMessage(`control-studio.parameters.${kind}`, {}), SCOPE), true, kind);
    }
    assert.strictEqual(h.identifyRequests.length, 0);
    assert.strictEqual(h.cancelRequests.length, 0);
    assert.strictEqual(h.posted.filter((message) => message.phase === "parameters").length, 4);
    assert.deepStrictEqual(lastPhase(h.posted, "parameters")?.payload, { status: "unavailable" });
  });

  test("a loop the engine does not serve is left to the TypeScript route", async () => {
    const { h, route } = harness();
    h.loop = undefined;
    assert.strictEqual(await route.handle(testRequest(), SCOPE), false);
    assert.strictEqual(await route.handle(pageMessage("control-studio.parameters.inspect", {}), SCOPE), false);
    assert.strictEqual(h.identifyRequests.length, 0);
  });

  test("reset forgets the review, so that an old identity cannot start a test", async () => {
    const { h, route } = harness();
    await route.handle(testRequest(), SCOPE);
    route.reset?.();
    await route.handle(pageMessage("control-studio.experiment.approve", { planId: REVIEW_ID, physicalApproved: false }), SCOPE);
    assert.strictEqual(h.identifyRequests.length, 1, "the start after a reset is refused without a request");
    assert.deepStrictEqual(lastPhase(h.posted, "failed")?.payload, { reason: "The reviewed plan changed. Review it again before starting." });
  });

  test("messages of other stages are left to the other routes", async () => {
    const { route } = harness();
    assert.strictEqual(await route.handle(pageMessage("control-studio.engineer.find", {}), SCOPE), false);
    assert.strictEqual(await route.handle(pageMessage("control-studio.identify.run", {}), SCOPE), false);
  });
});

suite("Control Studio three-page project test on the engine", () => {
  test("shows the engine's review on the plan rows and confirms its start with the review's identity", () => {
    const { root, experiments, posts, restore } = pageFixture();
    try {
      experiments.setSelectedMethod(true);
      experiments.receive("review", { kind: "reviewed", planId: REVIEW_ID, review: REVIEW });
      const plan = root.querySelector("[data-three-page-experiment-plan]") as HTMLElement;
      assert.strictEqual(plan.hidden, false);
      assert.strictEqual(root.querySelector("[data-three-page-experiment-bound]").textContent, "P1.ExperimentEnabled · TRUE while the test runs");
      assert.strictEqual(root.querySelector("[data-three-page-experiment-duration]").textContent, "30 s · 5 s baseline");
      assert.strictEqual(root.querySelector("[data-three-page-experiment-restoration]").textContent, "P1.ExperimentEnabled = FALSE");
      assert.match(root.querySelector("[data-three-page-experiment-stops]").textContent, /^Abort, 30 s timeout, Abort or failure asserts/);
      assert.strictEqual(experiments.primaryActionLabel, "Confirm and start");
      assert.strictEqual(experiments.hasReviewedPlan, true, "the page does not start the simulator again over a reviewed test");
      experiments.primaryAction();
      assert.deepStrictEqual(posts.pop(), { kind: "control-studio.experiment.approve", payload: { planId: REVIEW_ID, physicalApproved: false } });
    } finally {
      restore();
    }
  });

  test("a review that carries an approval block asks for Approve, and the press of Approve is the engineer's approval", () => {
    const { root, experiments, posts, restore } = pageFixture();
    try {
      experiments.setSelectedMethod(true);
      experiments.receive("review", { kind: "reviewed", planId: PHYSICAL_ID, review: PHYSICAL_REVIEW });
      assert.strictEqual(experiments.primaryActionLabel, "Approve");
      assert.match(root.querySelector("[data-three-page-experiment-status]").textContent, /^Physical target\./);
      experiments.primaryAction();
      assert.deepStrictEqual(posts.pop(), { kind: "control-studio.experiment.approve", payload: { planId: PHYSICAL_ID, physicalApproved: true } });
    } finally {
      restore();
    }
  });

  test("a refused start consumes the engine's review, and the page offers the review again", () => {
    const { root, experiments, restore } = pageFixture();
    try {
      experiments.setSelectedMethod(true);
      experiments.receive("review", { kind: "reviewed", planId: REVIEW_ID, review: REVIEW });
      experiments.primaryAction();
      experiments.receive("failed", { reason: CHANGED_FAILURE.message });
      assert.strictEqual(experiments.hasReviewedPlan, false);
      assert.strictEqual(experiments.primaryActionLabel, "Review project test");
      assert.strictEqual(root.querySelector("[data-three-page-experiment-plan]").hidden, true);
    } finally {
      restore();
    }
  });

  test("the page sends the declared duration as the loop declares it, and zero when the loop declares none", () => {
    for (const [declared, sent] of [[0.5, 0.5], [undefined, 0]] as const) {
      const { experiments, posts, restore } = pageFixture(false, projectLoopWithDuration(declared));
      try {
        experiments.setSelectedMethod(true);
        experiments.primaryAction();
        assert.deepStrictEqual(posts.pop(), {
          kind: "control-studio.experiment.review",
          payload: { kind: "observation", amplitude: 0, durationSeconds: sent, physicalTopologyReviewed: false },
        });
      } finally {
        restore();
      }
    }
  });

  test("a finished test says its outputs are restored when the engine confirmed the restoration, and says nothing more", () => {
    const { root, experiments, restore } = pageFixture();
    try {
      experiments.setSelectedMethod(true);
      experiments.receive("review", { kind: "reviewed", planId: REVIEW_ID, review: REVIEW });
      experiments.primaryAction();
      experiments.receive("running", { experiment_id: "operation-1" });
      experiments.receive("engine-ended", { status: "finished", restoration: "restored" });
      assert.strictEqual(root.querySelector("[data-three-page-experiment-status]").textContent, "Project test completed. Outputs restored.");
      experiments.receive("running", { experiment_id: "operation-2" });
      experiments.receive("engine-ended", { status: "finished" });
      assert.strictEqual(root.querySelector("[data-three-page-experiment-status]").textContent, "Project test completed.",
        "a test whose campaign confirmed no restoration says only that it completed");
    } finally {
      restore();
    }
  });

  test("a finished test whose fit refused shows the refusal as a note under its completed status, until the next test", () => {
    const { root, experiments, restore } = pageFixture();
    try {
      experiments.setSelectedMethod(true);
      experiments.receive("review", { kind: "reviewed", planId: REVIEW_ID, review: REVIEW });
      experiments.primaryAction();
      experiments.receive("running", { experiment_id: "operation-1" });
      const note = root.querySelector("[data-three-page-experiment-note]");
      assert.strictEqual(note.hidden, true, "no note while the test runs");
      experiments.receive("engine-ended", { status: "finished", restoration: "restored", note: FIT_REFUSAL });
      assert.strictEqual(root.querySelector("[data-three-page-experiment-status]").textContent, "Project test completed. Outputs restored.");
      assert.strictEqual(note.textContent, FIT_REFUSAL, "the status stays the engine's sentence; the refusal is its note");
      assert.strictEqual(note.hidden, false, "the note is shown under the completed test");
      experiments.receive("running", { experiment_id: "operation-2" });
      assert.strictEqual(note.hidden, true, "a new test hides the previous test's note");
    } finally {
      restore();
    }
  });

  test("shows the running test as Abort, and the end the engine reports as the test's end", () => {
    const { root, experiments, posts, restore } = pageFixture();
    try {
      experiments.setSelectedMethod(true);
      experiments.receive("review", { kind: "reviewed", planId: REVIEW_ID, review: REVIEW });
      experiments.primaryAction();
      experiments.receive("running", { experiment_id: "operation-1" });
      assert.strictEqual(experiments.busy, true);
      assert.strictEqual(experiments.primaryActionLabel, "Abort test");
      experiments.primaryAction();
      assert.deepStrictEqual(posts.pop(), { kind: "control-studio.experiment.abort", payload: { experimentId: "operation-1" } });
      experiments.receive("engine-ended", { status: "cancelled" });
      assert.strictEqual(experiments.busy, false);
      assert.strictEqual(experiments.primaryActionLabel, "Review project test", "the review was spent by its start");
      assert.strictEqual(root.querySelector("[data-three-page-experiment-status]").textContent, "Project test aborted.");
    } finally {
      restore();
    }
  });

  test("hides the manual parameter tool once the engine says it is unavailable for a served loop", () => {
    const { root, experiments, posts, restore } = pageFixture(true);
    try {
      experiments.setSelectedMethod(true);
      const parameters = root.querySelector("[data-three-page-experiment-parameters]") as HTMLElement;
      assert.strictEqual(parameters.hidden, false);
      experiments.receive("parameters", { status: "unavailable" });
      assert.strictEqual(parameters.hidden, true);
      assert.strictEqual(experiments.primaryActionLabel, "Review project test", "the test's review is the primary action now");
      experiments.primaryAction();
      assert.deepStrictEqual(posts.pop(), { kind: "control-studio.experiment.review", payload: { kind: "observation", amplitude: 0, durationSeconds: 30.1, physicalTopologyReviewed: false } });
    } finally {
      restore();
    }
  });
});

/** The experiment section of a project loop on a DOM, with the page's posts recorded. */
function pageFixture(withParameters = false, loopOverride?: Record<string, unknown>) {
  const loop = loopOverride ?? (withParameters ? experimentParameterLoop() : projectLoop());
  const dom = new JSDOM(`<main>${renderThreePageExperimentTemplate(loop as any, (value: string) => value)}</main>`);
  const original = { window: globalThis.window, document: globalThis.document, CustomEvent: globalThis.CustomEvent };
  Object.assign(globalThis, { window: dom.window, document: dom.window.document, CustomEvent: dom.window.CustomEvent });
  const posts: Array<{ kind: string; payload?: Record<string, unknown> }> = [];
  const root = dom.window.document.querySelector("main");
  const experiments = new ThreePageExperiments(root, loop as any, (kind: string, payload?: Record<string, unknown>) => {
    posts.push({ kind, payload });
  });
  return {
    dom,
    root,
    experiments,
    posts,
    restore: () => {
      Object.assign(globalThis, original);
      dom.window.close();
    },
  };
}

function projectLoop() {
  return {
    id: "control-lesson-04",
    projectId: "project:control-lesson-04",
    runtimeId: "simulator",
    revision: "sha256:loop",
    plantId: "proportional-water-heater",
    controller: { instance: "P1.Controller", type: "Control.PID", design: "project", revision: "sha256:controller", action: "direct", taskPeriodMs: 100 },
    signals: {
      processVariable: { symbol: "P1.ProcessValue", address: "%IL0", iecType: "LREAL", direction: "read", unit: "degC", range: { minimum: 0, maximum: 80 } },
      manipulatedVariable: { symbol: "P1.AppliedHeaterPower", address: "%QL0", iecType: "LREAL", direction: "read", unit: "%", range: { minimum: 0, maximum: 100 } },
      additional: [],
    },
    limits: { outputMinimum: 0, outputMaximum: 100 },
    observedSampleTiming: { nominalPeriodMs: 100 },
    experiment: { execution: "project-scripted", kind: "observation", amplitude: 0, durationSeconds: 30.1, seed: 1,
      activation: { symbol: "P1.ExperimentEnabled", address: "%MX24.1", iecType: "BOOL", direction: "write" } },
  } as any;
}

/** The project loop with its declared test duration changed; `undefined` is a loop that declares none. */
function projectLoopWithDuration(durationSeconds: number | undefined) {
  const loop = projectLoop();
  return { ...loop, experiment: { ...loop.experiment, durationSeconds } } as any;
}

function experimentParameterLoop() {
  return {
    ...projectLoop(),
    parameterPurpose: "experiment",
    parameterBinding: {
      loop_id: "control-lesson-04", task_name: "MainTask",
      mode: { symbol: "P1.ExperimentEnabled", manual_value: false },
      parameters: [{ symbol: "P1.FlowDeviation", iec_type: "LREAL", unit: "L/s", convention: "Flow deviation for the declared trial", minimum: -1, maximum: 1 }],
    },
  } as any;
}
