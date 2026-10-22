import * as assert from "assert";
import type { ControlStudioEngine, EngineAnswer, IdentifyAnswer } from "../../controlStudio/engine/engineClient";
import type { EngineLoop, EngineMethod, EngineRouteContext } from "../../controlStudio/engine/engineRouteContext";
import { createExperimentRoute } from "../../controlStudio/engine/routes/experimentRoute";
import type { Failure } from "../../controlStudio/generated/Failure";
import type { OperationUpdate } from "../../controlStudio/generated/OperationUpdate";
import type { LoopMessageScope } from "../../controlStudio/messages";
import { fillProjectTest, renderThreePageExperimentTemplate } from "../../controlStudio/threePageExperimentTemplate";
import { ThreePageExperiments } from "../../controlStudio/webview/threePageExperiments";

/**
 * The project test of a served loop (specification 35, C12 and §0.1a). The route is tested with a fake engine and a fake context:
 * each request it sends is recorded, and each answer is what the test sets. The page's experiment section is tested on a DOM fixture.
 * The rules are the engine's and are proven in Rust (`identify_review_tests.rs`, `identify_project_test_tests.rs`, `campaign_review_tests.rs`):
 * the identity of a review, the settings of the project test, the next action on a runtime that is not running, the approval of a
 * physical target, the sentences of the test's end, and the refusal of a start that no longer matches its review.
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
    testSignal: "P1.ExperimentEnabled · TRUE while the test runs",
  },
};
/** The engine's sentence of the approval block of a project test on a physical target (campaign_review.rs). */
const APPROVE_TEST = "Physical target. Approve the reviewed test before it starts; nothing is written before your approval.";
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
    message: APPROVE_TEST,
  },
};
/** The engine's refusal of a project test's review when the local simulator is not running (simulator_start.rs, A2.8). */
const START_SIMULATOR = {
  message: "Start the simulator before running a live action.",
  cause: "runtime",
  nextAction: "start-simulator",
  nextActionLabel: "Start simulator",
} as unknown as Failure;
/** The engine's sentences of a project test's end (project_test.rs, operation.rs). */
const COMPLETED_RESTORED = "Project test completed. Outputs restored.";
const CANCELLED = "Identification cancelled.";
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

/** The review of the declared project test, as the page asks for it: with nothing of its own (C12). */
function testRequest(): Record<string, unknown> {
  return pageMessage("control-studio.experiment.review", {});
}

/** The engine and the context of the route, with every request, answer and listener the test needs. */
function harness() {
  const h = {
    loop: LOOP as EngineLoop | undefined,
    connection: CONNECTION as { endpoint: string } | undefined,
    /** The mode of the window's runtime lifecycle, which a request without a connection carries. */
    runtimeMode: "simulate" as string | undefined,
    review: { result: REVIEW as unknown as IdentifyAnswer } as EngineAnswer<IdentifyAnswer>,
    start: { result: { kind: "operationStarted", operationId: "operation-1" } as unknown as IdentifyAnswer } as EngineAnswer<IdentifyAnswer>,
    cancel: { result: { operationId: "operation-1", command: "identify", state: "cancelled" } as unknown as OperationUpdate } as EngineAnswer<OperationUpdate>,
    /** The engine's answers to the parameter tool's requests, by action; an action with none is answered with nothing to show. */
    parameterAnswers: new Map<string, EngineAnswer<unknown>>(),
    identifyRequests: [] as Array<{ action: { action: string; settings?: Record<string, unknown> }; connection?: unknown; runtimeMode?: unknown }>,
    parameterRequests: [] as Array<Record<string, unknown>>,
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
    parameters: async (request: Record<string, unknown>) => {
      h.parameterRequests.push(request);
      return (h.parameterAnswers.get(String(request.action)) ?? { result: {} }) as EngineAnswer<unknown>;
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
    runtimeMode: async () => h.runtimeMode,
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

/** Lets the requests the route made after an update settle: the end of an aborted or failed test follows the engine's answer about the settings. */
function settled(): Promise<void> {
  return new Promise<void>((resolve) => setImmediate(resolve));
}

/** The sentence the engine words for released experiment settings (`SETTINGS_RELEASED` in crates/trust-control/src/identification/project_test.rs). */
const RELEASED = "The previous project test restored its declared values. Edit and review them before the next trial.";
/** The tool's offer once the settings are released (`parameter_tool_offer`, C12): review next; inspect, review and restore. */
const FREE_OFFER = { nextStep: "review", available: ["inspect", "review", "restore"] };

suite("Control Studio engine project test", () => {
  test("asks the engine to review the declared project test with no settings, and answers the review with the engine's identity", async () => {
    const { h, route } = harness();
    assert.strictEqual(await route.handle(testRequest(), SCOPE), true);
    assert.deepStrictEqual(h.identifyRequests[0]?.action, { action: "review" }, "the engine reviews the loop's declared experiment (C12)");
    assert.deepStrictEqual(h.identifyRequests[0]?.connection, CONNECTION, "the review reads the connected runtime as its start does (E11.8)");
    assert.ok(!("runtimeMode" in (h.identifyRequests[0] ?? {})), "a request that carries the connection names no lifecycle mode");
    assert.strictEqual(lastPhase(h.posted, "review")?.kind, "control-studio.experiment.result");
    assert.deepStrictEqual(lastPhase(h.posted, "review")?.payload, { kind: "reviewed", planId: REVIEW_ID, review: REVIEW });
  });

  test("reviews the declared project test whatever the page's message names: the route invents no settings (C12)", async () => {
    const { h, route } = harness();
    // The project owns the excitation (C12): the engine reviews the declared experiment, so nothing the message names is read.
    const load = pageMessage("control-studio.experiment.review", { kind: "load_step", amplitude: 3, durationSeconds: "1 h 30 min" });
    assert.strictEqual(await route.handle(load, SCOPE), true);
    assert.deepStrictEqual(h.identifyRequests[0]?.action, { action: "review" });
  });

  test("a review that finds no runtime running carries the lifecycle's mode, and shows the engine's Start simulator refusal (A2.8)", async () => {
    const { h, route } = harness();
    h.connection = undefined;
    h.review = { failure: START_SIMULATOR };
    assert.strictEqual(await route.handle(testRequest(), SCOPE), true);
    assert.strictEqual(h.identifyRequests[0]?.runtimeMode, "simulate", "the engine decides the next action from the lifecycle's mode");
    assert.ok(!("connection" in (h.identifyRequests[0] ?? {})));
    assert.deepStrictEqual(h.failures, [{ method: "identify", failure: START_SIMULATOR }], "the page shows the refusal with its Start simulator button");
    assert.deepStrictEqual(lastPhase(h.posted, "review")?.payload, { kind: "rejected", reason: START_SIMULATOR.message });
    // A runtime attached through its endpoint names its mode too; the engine answers Connect runtime for it.
    h.runtimeMode = "online";
    await route.handle(testRequest(), SCOPE);
    assert.strictEqual(h.identifyRequests[1]?.runtimeMode, "online");
  });

  test("shows the engine's refusal of the review as the failure and as the test's refusal", async () => {
    const { h, route } = harness();
    h.review = { failure: FAILURE };
    assert.strictEqual(await route.handle(testRequest(), SCOPE), true);
    assert.deepStrictEqual(h.failures.map((item) => item.method), ["identify"]);
    assert.deepStrictEqual(lastPhase(h.posted, "review")?.payload, { kind: "rejected", reason: FAILURE.message });
  });

  test("starts the campaign of the confirmed review with the identity and the runtime connection, and no settings", async () => {
    const { h, route } = harness();
    await route.handle(testRequest(), SCOPE);
    const approve = pageMessage("control-studio.experiment.approve", { planId: REVIEW_ID, physicalApproved: false });
    assert.strictEqual(await route.handle(approve, SCOPE), true);
    assert.deepStrictEqual(h.identifyRequests[1]?.action, { action: "identify", review: REVIEW_ID, physicalApproved: false });
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
    assert.deepStrictEqual(h.identifyRequests[1]?.action, { action: "identify", review: "sha256:an-older-review", physicalApproved: false });
  });

  test("forwards the physical approval the page gives, with the identity it confirms", async () => {
    const { h, route } = harness();
    await route.handle(testRequest(), SCOPE);
    await route.handle(pageMessage("control-studio.experiment.approve", { planId: REVIEW_ID, physicalApproved: true }), SCOPE);
    assert.deepStrictEqual(h.identifyRequests[1]?.action, { action: "identify", review: REVIEW_ID, physicalApproved: true });
  });

  test("shows the engine's refusal of a start whose review changed, and refuses nothing itself (E11.8)", async () => {
    const { h, route } = harness();
    await route.handle(testRequest(), SCOPE);
    h.start = { failure: CHANGED_FAILURE };
    await route.handle(pageMessage("control-studio.experiment.approve", { planId: REVIEW_ID, physicalApproved: false }), SCOPE);
    assert.deepStrictEqual(h.failures.map((item) => item.method), ["identify"]);
    assert.deepStrictEqual(lastPhase(h.posted, "failed")?.payload, { reason: CHANGED_FAILURE.message });
    // The engine checks the identity the page names: a second start goes to the engine, which refuses it in its own words.
    await route.handle(pageMessage("control-studio.experiment.approve", { planId: REVIEW_ID, physicalApproved: false }), SCOPE);
    assert.strictEqual(h.identifyRequests.length, 3, "the route holds no review and has no refusal of its own");
    assert.deepStrictEqual(lastPhase(h.posted, "failed")?.payload, { reason: CHANGED_FAILURE.message });
  });

  test("a start the engine does not answer shows the start as failed, and the page is not left running", async () => {
    const { h, route } = harness();
    await route.handle(testRequest(), SCOPE);
    h.start = undefined as unknown as EngineAnswer<IdentifyAnswer>;
    await route.handle(pageMessage("control-studio.experiment.approve", { planId: REVIEW_ID, physicalApproved: false }), SCOPE);
    assert.deepStrictEqual(lastPhase(h.posted, "failed")?.payload, {}, "the engine served no answer: the page shows the start as failed");
    assert.strictEqual(h.identifyRequests.length, 2);
  });

  test("an answer of another kind than the start asks for ends the start as failed, with no sentence of the page's own (§0.1a, C14)", async () => {
    const { h, route } = harness();
    await route.handle(testRequest(), SCOPE);
    h.start = { result: { kind: "reset", stages: [] } as unknown as IdentifyAnswer } as EngineAnswer<IdentifyAnswer>;
    await route.handle(pageMessage("control-studio.experiment.approve", { planId: REVIEW_ID, physicalApproved: false }), SCOPE);
    assert.deepStrictEqual(lastPhase(h.posted, "failed")?.payload, {}, "the page's wait ends");
    assert.deepStrictEqual(h.failures, [], "the route words no failure of its own");
    assert.deepStrictEqual(h.followed, [], "no operation is followed");
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
    await settled();
    const ended = h.posted.filter((message) => message.phase === "engine-ended");
    assert.deepStrictEqual(ended.map((message) => message.payload), [{ status: "finished" }]);
    assert.deepStrictEqual(h.disposed, ["operation-1"]);
  });

  test("a failed campaign reports the engine's reason as the end of the test", async () => {
    const { h, route, emit } = harness();
    await route.handle(testRequest(), SCOPE);
    await route.handle(pageMessage("control-studio.experiment.approve", { planId: REVIEW_ID, physicalApproved: false }), SCOPE);
    emit("operation-1", { operationId: "operation-1", command: "identify", state: "failed", failure: { message: "Restoration not confirmed." } });
    await settled();
    assert.deepStrictEqual(lastPhase(h.posted, "engine-ended")?.payload, { status: "failed", reason: "Restoration not confirmed." });
  });

  test("a finished campaign carries the engine's sentence of the test's end and the restoration it confirmed to the page's end", async () => {
    const { h, route, emit } = harness();
    await route.handle(testRequest(), SCOPE);
    await route.handle(pageMessage("control-studio.experiment.approve", { planId: REVIEW_ID, physicalApproved: false }), SCOPE);
    emit("operation-1", { operationId: "operation-1", command: "identify", state: "finished", result: { restoration: "restored", testOutcome: COMPLETED_RESTORED } } as unknown as OperationUpdate);
    await settled();
    assert.deepStrictEqual(lastPhase(h.posted, "engine-ended")?.payload, { status: "finished", reason: COMPLETED_RESTORED, restoration: "restored" });
  });

  test("a finished campaign whose fit refused carries the whole refusal to the page's end as its note", async () => {
    const { h, route, emit } = harness();
    await route.handle(testRequest(), SCOPE);
    await route.handle(pageMessage("control-studio.experiment.approve", { planId: REVIEW_ID, physicalApproved: false }), SCOPE);
    emit("operation-1", { operationId: "operation-1", command: "identify", state: "finished", result: { restoration: "restored", testOutcome: COMPLETED_RESTORED, fitRefusal: FIT_REFUSAL } } as unknown as OperationUpdate);
    await settled();
    assert.deepStrictEqual(lastPhase(h.posted, "engine-ended")?.payload, { status: "finished", reason: COMPLETED_RESTORED, restoration: "restored", note: FIT_REFUSAL });
  });

  test("a finished campaign asks the engine whether the settings are released, as every end does, and carries its sentence and the tool's offer to the page's end", async () => {
    // C12: the end of every test is answered alike, by the tool's `released` answer, which names the tool's offer that follows.
    const { h, route, emit } = harness();
    h.parameterAnswers.set("released", { result: { settingsReleased: RELEASED, offer: FREE_OFFER } });
    await route.handle(testRequest(), SCOPE);
    await route.handle(pageMessage("control-studio.experiment.approve", { planId: REVIEW_ID, physicalApproved: false }), SCOPE);
    emit("operation-1", { operationId: "operation-1", command: "identify", state: "finished", result: { restoration: "restored", settingsReleased: RELEASED } } as unknown as OperationUpdate);
    await settled();
    assert.deepStrictEqual(h.parameterRequests.map((request) => request.action), ["released"]);
    assert.deepStrictEqual(lastPhase(h.posted, "engine-ended")?.payload, { status: "finished", restoration: "restored", settingsReleased: RELEASED, offer: FREE_OFFER });
  });

  test("an aborted campaign asks the engine whether the settings are released, and carries the engine's sentences to the page's end", async () => {
    const { h, route, emit } = harness();
    h.parameterAnswers.set("released", { result: { settingsReleased: RELEASED, offer: FREE_OFFER } });
    await route.handle(testRequest(), SCOPE);
    await route.handle(pageMessage("control-studio.experiment.approve", { planId: REVIEW_ID, physicalApproved: false }), SCOPE);
    emit("operation-1", { operationId: "operation-1", command: "identify", state: "cancelled", message: CANCELLED } as unknown as OperationUpdate);
    await settled();
    assert.deepStrictEqual(h.parameterRequests, [
      { projectRoot: "/project", runtimeId: "runtime-test", loopId: "loop-test", storageFolder: "", action: "released" },
    ], "the question reads no runtime, so it carries no connection");
    assert.deepStrictEqual(lastPhase(h.posted, "engine-ended")?.payload, { status: "cancelled", reason: CANCELLED, settingsReleased: RELEASED, offer: FREE_OFFER },
      "the end is the engine's sentence of the cancellation, with the engine's release and the tool's offer");
    assert.strictEqual(h.posted.filter((message) => message.phase === "engine-ended").length, 1, "the test ends once");
  });

  test("a failed campaign whose restoration the engine confirmed carries the release too, and one it did not carries none", async () => {
    const { h, route, emit } = harness();
    h.parameterAnswers.set("released", { result: { settingsReleased: RELEASED } });
    await route.handle(testRequest(), SCOPE);
    await route.handle(pageMessage("control-studio.experiment.approve", { planId: REVIEW_ID, physicalApproved: false }), SCOPE);
    emit("operation-1", { operationId: "operation-1", command: "identify", state: "failed", failure: { message: "The test stopped early." } });
    await settled();
    assert.deepStrictEqual(lastPhase(h.posted, "engine-ended")?.payload, { status: "failed", reason: "The test stopped early.", settingsReleased: RELEASED });

    // The engine says why the settings are still held: the end carries no release, and the engine's reason is not the page's to word.
    h.parameterAnswers.set("released", { result: { message: "This loop has an unfinished operation." } });
    await route.handle(testRequest(), SCOPE);
    await route.handle(pageMessage("control-studio.experiment.approve", { planId: REVIEW_ID, physicalApproved: false }), SCOPE);
    emit("operation-1", { operationId: "operation-1", command: "identify", state: "failed", failure: { message: "Restoration not confirmed." } });
    await settled();
    assert.deepStrictEqual(lastPhase(h.posted, "engine-ended")?.payload, { status: "failed", reason: "Restoration not confirmed." });
  });

  test("an end the engine cannot answer the question for still ends the test, with no release, and a finished campaign asks as every end does", async () => {
    const { h, route, emit } = harness();
    h.parameterAnswers.set("released", { failure: FAILURE });
    await route.handle(testRequest(), SCOPE);
    await route.handle(pageMessage("control-studio.experiment.approve", { planId: REVIEW_ID, physicalApproved: false }), SCOPE);
    emit("operation-1", { operationId: "operation-1", command: "identify", state: "cancelled" });
    await settled();
    assert.deepStrictEqual(lastPhase(h.posted, "engine-ended")?.payload, { status: "cancelled" });
    assert.deepStrictEqual(h.failures, [], "the question's refusal is not the engineer's to read");

    await route.handle(testRequest(), SCOPE);
    await route.handle(pageMessage("control-studio.experiment.approve", { planId: REVIEW_ID, physicalApproved: false }), SCOPE);
    h.parameterRequests.length = 0;
    emit("operation-1", { operationId: "operation-1", command: "identify", state: "finished", result: { restoration: "restored" } } as unknown as OperationUpdate);
    await settled();
    assert.deepStrictEqual(lastPhase(h.posted, "engine-ended")?.payload, { status: "finished", restoration: "restored" },
      "the question's refusal leaves the end without a release");
    assert.deepStrictEqual(h.parameterRequests.map((request) => request.action), ["released"], "the route asks after every end alike");
  });

  test("an end that is not a finished campaign takes no release from its result, whatever that holds", async () => {
    const { h, route, emit } = harness();
    await route.handle(testRequest(), SCOPE);
    await route.handle(pageMessage("control-studio.experiment.approve", { planId: REVIEW_ID, physicalApproved: false }), SCOPE);
    emit("operation-1", { operationId: "operation-1", command: "identify", state: "failed", failure: { message: "Restoration not confirmed." }, result: { settingsReleased: "stale" } } as unknown as OperationUpdate);
    await settled();
    assert.deepStrictEqual(lastPhase(h.posted, "engine-ended")?.payload, { status: "failed", reason: "Restoration not confirmed." });
  });

  test("a refused start shows the whole refusal, its detail, and not only its first sentence", async () => {
    const { h, route } = harness();
    await route.handle(testRequest(), SCOPE);
    h.start = { failure: { message: "The test cannot start.", detail: "The test cannot start. Its review is spent, so review it again.", cause: "settings", nextAction: "none" } as unknown as Failure } as unknown as EngineAnswer<IdentifyAnswer>;
    await route.handle(pageMessage("control-studio.experiment.approve", { planId: REVIEW_ID, physicalApproved: false }), SCOPE);
    assert.deepStrictEqual(lastPhase(h.posted, "failed")?.payload, { reason: "The test cannot start. Its review is spent, so review it again." });
  });

  test("Abort with no campaign running ends the page's busy state without asking the engine, and with no sentence of its own", async () => {
    const { h, route } = harness();
    assert.strictEqual(await route.handle(pageMessage("control-studio.experiment.abort", { experimentId: "stale" }), SCOPE), true);
    assert.strictEqual(h.cancelRequests.length, 0);
    assert.deepStrictEqual(lastPhase(h.posted, "engine-ended")?.payload, { status: "unknown" });
  });

  test("the route decides no loop's parameter tool: each request goes to the engine, and the engine's refusal of a loop without one is shown", async () => {
    const { h, route } = harness();
    const noTool = { message: "This loop declares no parameter mapping for its analysis, so Control Studio writes none of its parameters.", cause: "settings", nextAction: "open-loop-setup" } as unknown as Failure;
    h.parameterAnswers.set("inspect", { failure: noTool });
    assert.strictEqual(await route.handle(pageMessage("control-studio.parameters.inspect", {}), SCOPE), true);
    assert.deepStrictEqual(h.parameterRequests.map((request) => request.action), ["inspect"], "the engine is asked");
    assert.deepStrictEqual(h.failures.map((item) => item.failure), [noTool]);
    assert.deepStrictEqual(lastPhase(h.posted, "parameters")?.payload, { status: "failed", reason: noTool.message });
    assert.strictEqual(h.identifyRequests.length, 0);
    assert.strictEqual(h.cancelRequests.length, 0);
  });

  test("a loop the engine does not serve is left to the TypeScript route", async () => {
    const { h, route } = harness();
    h.loop = undefined;
    assert.strictEqual(await route.handle(testRequest(), SCOPE), false);
    assert.strictEqual(await route.handle(pageMessage("control-studio.parameters.inspect", {}), SCOPE), false);
    assert.strictEqual(h.identifyRequests.length, 0);
  });

  test("reset forgets the running test and the parameter review the engine answered", async () => {
    const { h, route } = harness();
    h.parameterAnswers.set("review", { result: { identity: "sha256:parameters", values: { "P1.HoldSeconds": 35 } } });
    h.parameterAnswers.set("confirm", { result: { kind: "operationStarted", operationId: "operation-write" } });
    await route.handle(testRequest(), SCOPE);
    await route.handle(pageMessage("control-studio.experiment.approve", { planId: REVIEW_ID, physicalApproved: false }), SCOPE);
    await route.handle(pageMessage("control-studio.parameters.review", { json: "{}" }), SCOPE);
    route.reset?.();
    await route.handle(pageMessage("control-studio.experiment.abort", { experimentId: "operation-1" }), SCOPE);
    assert.strictEqual(h.cancelRequests.length, 0, "the reopened panel's test is adopted, not remembered");
    await route.handle(pageMessage("control-studio.parameters.apply", { reviewId: "sha256:parameters", physicalApproved: false }), SCOPE);
    const confirm = h.parameterRequests[h.parameterRequests.length - 1];
    assert.strictEqual(confirm?.action, "confirm");
    assert.ok(!("values" in (confirm ?? {})), "the values of a review before the reset are not sent; the engine decides what the identity names");
  });

  test("a parameter write's progress reaches the tool in the engine's words, and its end with the engine's answer (C12, E7.4)", async () => {
    const { h, route, emit } = harness();
    const writing = "Waiting for the owning task and observed values…";
    const applied = "Reviewed values observed after ST execution.";
    h.parameterAnswers.set("review", { result: { identity: "sha256:parameters", values: { "P1.HoldSeconds": 35 } } });
    h.parameterAnswers.set("confirm", { result: { kind: "operationStarted", operationId: "operation-write" } });
    await route.handle(pageMessage("control-studio.parameters.review", { json: "{}" }), SCOPE);
    await route.handle(pageMessage("control-studio.parameters.apply", { reviewId: "sha256:parameters", physicalApproved: false }), SCOPE);
    emit("operation-write", { operationId: "operation-write", command: "parameters", state: "running", progress: writing });
    assert.deepStrictEqual(lastPhase(h.posted, "parameters")?.payload, { status: "running", reason: writing });
    emit("operation-write", { operationId: "operation-write", command: "parameters", state: "finished",
      result: { status: "applied", message: applied, offer: FREE_OFFER } });
    assert.deepStrictEqual(lastPhase(h.posted, "parameters")?.payload, { status: "applied", reason: applied, offer: FREE_OFFER });
  });

  test("a parameter confirmation answered with another kind than a started operation ends the tool's wait with no sentence of the page's own (C14)", async () => {
    const { h, route } = harness();
    h.parameterAnswers.set("confirm", { result: { identity: "sha256:not-an-operation" } });
    await route.handle(pageMessage("control-studio.parameters.apply", { reviewId: "sha256:parameters", physicalApproved: false }), SCOPE);
    assert.deepStrictEqual(lastPhase(h.posted, "parameters")?.payload, { status: "failed" });
    assert.deepStrictEqual(h.failures, [], "the route words no failure of its own");
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
      assert.strictEqual(root.querySelector("[data-three-page-experiment-status]").hidden, true, "a simulated target shows no approval sentence");
      experiments.primaryAction();
      assert.deepStrictEqual(posts.pop(), { kind: "control-studio.experiment.approve", payload: { planId: REVIEW_ID, physicalApproved: false } });
    } finally {
      restore();
    }
  });

  test("a review that carries an approval block asks for Approve with the engine's sentence, and the press of Approve is the engineer's approval", () => {
    const { root, experiments, posts, restore } = pageFixture();
    try {
      experiments.setSelectedMethod(true);
      experiments.receive("review", { kind: "reviewed", planId: PHYSICAL_ID, review: PHYSICAL_REVIEW });
      assert.strictEqual(experiments.primaryActionLabel, "Approve");
      assert.strictEqual(root.querySelector("[data-three-page-experiment-status]").textContent, APPROVE_TEST, "the sentence is the engine's");
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
      assert.strictEqual(experiments.primaryActionLabel, "Review project test");
      assert.strictEqual(root.querySelector("[data-three-page-experiment-status]").textContent, CHANGED_FAILURE.message);
      assert.strictEqual(root.querySelector("[data-three-page-experiment-plan]").hidden, true);
    } finally {
      restore();
    }
  });

  test("the page sends the review with no duration or settings of its own, whatever the loop declares: the engine reviews it (C12)", () => {
    for (const declared of [0.5, undefined]) {
      const { experiments, posts, restore } = pageFixture(projectLoopWithDuration(declared));
      try {
        experiments.setSelectedMethod(true);
        experiments.primaryAction();
        assert.deepStrictEqual(posts.pop(), { kind: "control-studio.experiment.review", payload: undefined });
      } finally {
        restore();
      }
    }
  });

  test("a refused review shows the engine's sentence, and the page adds none of its own", () => {
    const { root, experiments, restore } = pageFixture();
    try {
      experiments.setSelectedMethod(true);
      experiments.primaryAction();
      experiments.receive("review", { kind: "rejected", reason: START_SIMULATOR.message });
      assert.strictEqual(root.querySelector("[data-three-page-experiment-status]").textContent, START_SIMULATOR.message);
      assert.strictEqual(experiments.primaryActionLabel, "Review project test");
      // A review the engine could not take: its failure is shown by the page's failure area, and the test section writes nothing.
      experiments.primaryAction();
      experiments.receive("review", { kind: "rejected" });
      assert.strictEqual(root.querySelector("[data-three-page-experiment-status]").hidden, true);
    } finally {
      restore();
    }
  });

  test("a finished test shows the engine's sentence of its end, and composes none from the restoration", () => {
    const { root, experiments, restore } = pageFixture();
    try {
      experiments.setSelectedMethod(true);
      experiments.receive("review", { kind: "reviewed", planId: REVIEW_ID, review: REVIEW });
      experiments.primaryAction();
      experiments.receive("running", { experiment_id: "operation-1" });
      experiments.receive("engine-ended", { status: "finished", reason: COMPLETED_RESTORED, restoration: "restored" });
      assert.strictEqual(root.querySelector("[data-three-page-experiment-status]").textContent, COMPLETED_RESTORED);
      experiments.receive("running", { experiment_id: "operation-2" });
      experiments.receive("engine-ended", { status: "finished", reason: "Project test completed." });
      assert.strictEqual(root.querySelector("[data-three-page-experiment-status]").textContent, "Project test completed.");
      // An end that carries the restoration but no sentence shows none: the page does not compose "Outputs restored." itself.
      experiments.receive("running", { experiment_id: "operation-3" });
      experiments.receive("engine-ended", { status: "finished", restoration: "restored" });
      assert.strictEqual(root.querySelector("[data-three-page-experiment-status]").textContent, "");
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
      experiments.receive("engine-ended", { status: "finished", reason: COMPLETED_RESTORED, restoration: "restored", note: FIT_REFUSAL });
      assert.strictEqual(root.querySelector("[data-three-page-experiment-status]").textContent, COMPLETED_RESTORED);
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
      experiments.receive("engine-ended", { status: "cancelled", reason: CANCELLED });
      assert.strictEqual(experiments.busy, false);
      assert.strictEqual(experiments.primaryActionLabel, "Review project test", "the review was spent by its start");
      assert.strictEqual(root.querySelector("[data-three-page-experiment-status]").textContent, CANCELLED, "the end is the engine's sentence");
    } finally {
      restore();
    }
  });
});

/**
 * The project test's form the engine answers for a fixture loop (`projectTestForm`, B6.1): the signals it offers, with the labels the engine
 * gives them, and the duration the loop declares. The fixture stands for the engine's answer; the Rust form has its own tests.
 */
function answeredForm(loop: any) {
  const signals = [
    ...(loop.signals.setpoint ? [{ id: "sp", label: `${loop.signals.setpoint.symbol} · Setpoint`, unit: loop.signals.setpoint.unit, selected: true }] : []),
    { id: "pv", label: `${loop.signals.processVariable.symbol} · Process value`, unit: loop.signals.processVariable.unit, selected: true },
    { id: "mv", label: `${loop.signals.manipulatedVariable.symbol} · Applied output`, unit: loop.signals.manipulatedVariable.unit, selected: true },
  ];
  return { signals, durationSeconds: loop.experiment?.durationSeconds ?? 30 };
}

/** The experiment section of a project loop on a DOM, with the page's posts recorded. */
function pageFixture(loopOverride?: Record<string, unknown>) {
  const loop = loopOverride ?? projectLoop();
  const dom = new JSDOM(`<main>${renderThreePageExperimentTemplate()}</main>`);
  const original = { window: globalThis.window, document: globalThis.document, CustomEvent: globalThis.CustomEvent };
  Object.assign(globalThis, { window: dom.window, document: dom.window.document, CustomEvent: dom.window.CustomEvent });
  const posts: Array<{ kind: string; payload?: Record<string, unknown> }> = [];
  const root = dom.window.document.querySelector("main")!;
  const form = answeredForm(loop);
  fillProjectTest(root, form);
  const experiments = new ThreePageExperiments(root, form as any, (kind: string, payload?: Record<string, unknown>) => {
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
