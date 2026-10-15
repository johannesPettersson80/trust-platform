import type { Failure } from "../../generated/Failure";
import type { IdentifyRequest } from "../../generated/IdentifyRequest";
import type { ParameterWorkAnswer } from "../../generated/ParameterWorkAnswer";
import type { ParameterWorkRequest } from "../../generated/ParameterWorkRequest";
import type { OperationUpdate } from "../../generated/OperationUpdate";
import type { LoopMessageScope } from "../../messages";
import type { EngineAcquisitionSettings } from "../engineIdentify";
import {
  record,
  unclassifiedFailure,
  type EngineLoop,
  type EngineRouteContext,
  type EngineStageRoute,
} from "../engineRouteContext";

/** The project test's stage route: `handle` and `reset`, and `adopt`, which takes the test's campaign the engine kept running (A8.6). */
export interface ExperimentStageRoute extends EngineStageRoute {
  /**
   * Takes the campaign of a project-scripted loop that the engine runs when the panel reopens the loop (A8.6, C12): the test section
   * shows it running, its Abort stops it, and its end is reported as the test's end. The panel calls it only for a loop that declares
   * its execution owner, which is the rule that offers the project test.
   */
  adopt(operationId: string, scope: LoopMessageScope): Promise<void>;
}

/**
 * The project test of a project-scripted loop on the Identify page (specification 35, C12 and §0.1a). The page's review is the
 * engine's Identify review, `controlStudio/identify` with `action: "review"`, for the Automatic method, the declared duration and
 * no amplitude: the project owns the excitation. The review's identity is the engine's (E11.8), so this route mints none. The
 * page confirms the start with that identity and, on a physical target, with the engineer's approval; the engine checks both. The
 * start is the campaign that review describes, `identify` with the same settings, and Abort is `controlStudio/cancel` of that
 * campaign. The parameter tool of a declared analysis (C12, B6.1) is the engine's `controlStudio/parameters`: a served loop whose
 * analysis maps parameters answers its requests through it, and any other served loop answers that the tool is unavailable. The
 * engine decides every rule; this route names the requests and reports the answers. Written in V6 step S10, parameters in V7.
 */
export function createExperimentRoute(context: EngineRouteContext): ExperimentStageRoute {
  /** The review the engine answered last: the settings its start runs, and the identity the page confirms (`planId`). */
  let review: { planId: string; settings: EngineAcquisitionSettings } | undefined;
  /** The campaign the engine runs for the page's test; Abort and the page's Cancel stop it. */
  let running: string | undefined;
  /**
   * The parameter review the engine answered last (C12): its identity, which the page's apply names, the values it reviewed, and
   * whether it restores the values from before. An apply is spent with its start, so the page reviews again after it.
   */
  let parameterReview: { identity: string; values: Record<string, number>; rollback: boolean } | undefined;
  /** The listeners of the campaigns the route started; each one ends with its campaign. */
  const watchers = new Set<{ dispose(): void }>();

  /** Posts one answer of the test to the page (`control-studio.experiment.result`, read by the page's experiment section). */
  function phase(scope: LoopMessageScope, name: string, payload: Record<string, unknown>): Promise<void> {
    return context.post(scope, { kind: "control-studio.experiment.result", phase: name, payload });
  }

  /** The settings of the project test: the Automatic method, which runs the project's scenario whatever the mode (C12). */
  function testSettings(durationSeconds: number): EngineAcquisitionSettings {
    return { method: "automatic", durationSeconds, amplitude: 0, reuseRecordings: false };
  }

  /** Answers the review of the declared project test with the identity the engine gives it. */
  async function reviewTest(loop: EngineLoop, scope: LoopMessageScope, payload: Record<string, unknown>): Promise<boolean> {
    // The page asks for the declared project test only: an observation with no amplitude of its own. The duration is checked by
    // the engine (A7), so the route forwards the duration the page sends.
    if (payload.kind !== "observation" || payload.amplitude !== 0 || typeof payload.durationSeconds !== "number") {
      await phase(scope, "review", { kind: "rejected" });
      return true;
    }
    const settings = testSettings(payload.durationSeconds);
    review = undefined;
    const request: IdentifyRequest = {
      action: { action: "review", settings },
      loopId: loop.id,
      projectRoot: context.projectRoot,
      runtimeId: loop.runtimeId,
      storageFolder: context.storageFolder,
    };
    const reply = await context.ask(scope, "identify", () => context.client.identify(request));
    // The engine serves the loop even when it cannot be reached: its start failure is shown, and the review is not repeated.
    if (!reply) {
      await phase(scope, "review", { kind: "rejected" });
      return true;
    }
    if ("failure" in reply) {
      await context.postFailure(scope, "identify", reply.failure);
      await phase(scope, "review", { kind: "rejected", reason: reply.failure.message });
      return true;
    }
    const answer = reply.result;
    if (!("maximumDurationSeconds" in answer) || typeof answer.identity !== "string") {
      await context.postFailure(scope, "identify", unclassifiedFailure("The engine answered the test review with a result the page does not show."));
      await phase(scope, "review", { kind: "rejected" });
      return true;
    }
    review = { planId: answer.identity, settings };
    await phase(scope, "review", { kind: "reviewed", planId: answer.identity, review: answer });
    return true;
  }

  /** Follows one campaign to its end: its completion, its failure or its Abort ends the page's test (`engine-ended`). */
  function watch(operationId: string, scope: LoopMessageScope): void {
    const subscription = context.client.onUpdate(operationId, (update: OperationUpdate) => {
      if (update.state === "running") return;
      if (running === operationId) running = undefined;
      subscription.dispose();
      watchers.delete(subscription);
      const failure = record(update.failure);
      void phase(scope, "engine-ended", {
        status: update.state,
        ...(typeof failure?.message === "string" ? { reason: failure.message } : {}),
      });
    });
    watchers.add(subscription);
  }

  /**
   * Starts the campaign the review describes. The engine compares the identity the page confirms with the campaign as it is now,
   * and on a physical target checks the approval the page passes. A refused start consumes the review: the page reviews again.
   */
  async function startTest(loop: EngineLoop, scope: LoopMessageScope, payload: Record<string, unknown>): Promise<boolean> {
    const current = review;
    const planId = payload.planId;
    if (!current || typeof planId !== "string") {
      await phase(scope, "failed", { reason: "The reviewed plan changed. Review it again before starting." });
      return true;
    }
    if (running) {
      await phase(scope, "failed", { reason: "A test is already running." });
      return true;
    }
    const connection = await context.connection();
    const request: IdentifyRequest = {
      action: {
        action: "identify",
        settings: current.settings,
        review: planId,
        physicalApproved: payload.physicalApproved === true,
      },
      ...(connection ? { connection } : {}),
      loopId: loop.id,
      projectRoot: context.projectRoot,
      runtimeId: loop.runtimeId,
      storageFolder: context.storageFolder,
    };
    const reply = await context.ask(scope, "identify", () => context.client.identify(request));
    review = undefined;
    if (!reply) {
      await phase(scope, "failed", {});
      return true;
    }
    if ("failure" in reply) {
      await context.postFailure(scope, "identify", reply.failure);
      await phase(scope, "failed", { reason: reply.failure.message });
      return true;
    }
    const answer = reply.result;
    const operationId = "operationId" in answer && typeof answer.operationId === "string" ? answer.operationId : undefined;
    if (!operationId) {
      await context.postFailure(scope, "identify", unclassifiedFailure("The engine answered the test start with a result the page does not show."));
      await phase(scope, "failed", {});
      return true;
    }
    running = operationId;
    context.follow(operationId, scope);
    watch(operationId, scope);
    await phase(scope, "running", { experiment_id: operationId });
    return true;
  }

  /** The parameter tool of a declared analysis (C12, B6.1): the engine reviews and writes the parameters the analysis maps. */
  async function parameters(loop: EngineLoop, action: string, payload: Record<string, unknown>, scope: LoopMessageScope): Promise<boolean> {
    // A loop whose analysis maps no parameter has no tool: the page shows none, and nothing is sent.
    if (!loop.analysisParameterSymbols) {
      await phase(scope, "parameters", { status: "unavailable" });
      return true;
    }
    switch (action) {
      case "inspect":
        return inspectParameters(loop, scope);
      case "review":
        return reviewParameters(loop, "review", payload.json, scope);
      case "review-rollback":
        return reviewParameters(loop, "reviewRestore", undefined, scope);
      case "apply":
        return applyParameters(loop, payload, scope);
      case "selected-analysis":
        return draftParameters(loop, scope);
      case "invalidate":
        // The page's edit changes nothing the engine holds: the review is spent, and the page reviews again.
        parameterReview = undefined;
        await phase(scope, "parameters", { status: "edited", reason: "Values changed. Review again before applying." });
        return true;
      default:
        return false;
    }
  }

  /**
   * The binding the engine names for the mapped parameters, when its answer carries one (C12): the page configures its fields from it.
   * It is read as data, so the route needs no type that the engine's generated answer does not declare yet.
   */
  function namedBinding(result: unknown): Record<string, unknown> {
    const binding = (result as { binding?: unknown } | undefined)?.binding;
    return binding === undefined ? {} : { binding };
  }

  /** The engine's record of the mapped parameters, for the page to show (`inspect`). */
  async function inspectParameters(loop: EngineLoop, scope: LoopMessageScope): Promise<boolean> {
    const reply = await context.ask(scope, "parameters", async () => context.client.parameters(await parameterRequest(loop, "inspect", {})));
    if (!reply) {
      await phase(scope, "parameters", { status: "failed" });
      return true;
    }
    if ("failure" in reply) {
      await parameterFailed(scope, reply.failure);
      return true;
    }
    const answer = reply.result as ParameterWorkAnswer;
    await phase(scope, "parameters", { status: "inspected", values: answer.values, observed: answer.observed, ...namedBinding(reply.result) });
    return true;
  }

  /**
   * The review of an Apply (`review`) or of a Restore (`reviewRestore`). The values are the page's JSON, which the engine checks: a
   * text that is not JSON is the only refusal the route makes itself.
   */
  async function reviewParameters(loop: EngineLoop, action: "review" | "reviewRestore", json: unknown, scope: LoopMessageScope): Promise<boolean> {
    let values: Record<string, number> = {};
    if (action === "review") {
      try {
        values = JSON.parse(String(json)) as Record<string, number>;
      } catch {
        await phase(scope, "parameters", { status: "failed", reason: "The parameter values are not valid JSON." });
        return true;
      }
    }
    parameterReview = undefined;
    const reply = await context.ask(scope, "parameters", async () => context.client.parameters(await parameterRequest(loop, action, action === "review" ? { values } : {})));
    if (!reply) {
      await phase(scope, "parameters", { status: "failed" });
      return true;
    }
    if ("failure" in reply) {
      await parameterFailed(scope, reply.failure);
      return true;
    }
    const identity = (reply.result as { identity?: unknown }).identity;
    if (typeof identity !== "string") {
      await context.postFailure(scope, "parameters", unclassifiedFailure("The engine answered the parameter review with a result the page does not show."));
      await phase(scope, "parameters", { status: "failed" });
      return true;
    }
    parameterReview = { identity, values, rollback: action === "reviewRestore" };
    await phase(scope, "parameters", { status: "reviewed", reviewId: identity, values, rollback: action === "reviewRestore" });
    return true;
  }

  /**
   * The confirmed write of the page's reviewed values, or of the values from before (`apply`, `restore`). The review is spent with its
   * start, and the start names its identity: the engine refuses a record that changed since the review (E11.8). The write's answer
   * follows as its operation's update (`watchParameters`).
   */
  async function applyParameters(loop: EngineLoop, payload: Record<string, unknown>, scope: LoopMessageScope): Promise<boolean> {
    const review = parameterReview;
    parameterReview = undefined;
    if (!review || payload.reviewId !== review.identity) {
      await phase(scope, "parameters", { status: "failed", reason: "This review expired. Review the exact values again." });
      return true;
    }
    const action = review.rollback ? "restore" : "apply";
    const extra = {
      review: review.identity,
      physicalApproved: payload.physicalApproved === true,
      ...(review.rollback ? {} : { values: review.values }),
    };
    const reply = await context.ask(scope, "parameters", async () => context.client.parameters(await parameterRequest(loop, action, extra)));
    if (!reply) {
      await phase(scope, "parameters", { status: "failed" });
      return true;
    }
    if ("failure" in reply) {
      await parameterFailed(scope, reply.failure);
      return true;
    }
    const operationId = (reply.result as { operationId?: unknown }).operationId;
    if (typeof operationId !== "string") {
      await context.postFailure(scope, "parameters", unclassifiedFailure("The engine answered the parameter write with a result the page does not show."));
      await phase(scope, "parameters", { status: "failed" });
      return true;
    }
    watchParameters(operationId, scope, review.rollback);
    return true;
  }

  /** The engine's selected values of the analysis, copied for a review (`selected-analysis`). Nothing is written. */
  async function draftParameters(loop: EngineLoop, scope: LoopMessageScope): Promise<boolean> {
    const reply = await context.ask(scope, "parameters", async () => context.client.parameters(await parameterRequest(loop, "draft", {})));
    if (!reply) {
      await phase(scope, "parameters", { status: "failed" });
      return true;
    }
    if ("failure" in reply) {
      await parameterFailed(scope, reply.failure);
      return true;
    }
    await phase(scope, "parameters", { status: "draft", values: (reply.result as { values?: unknown }).values, ...namedBinding(reply.result), reason: "Selected analysis values copied for review. The runtime has not changed." });
    return true;
  }

  /** The request of one parameter action, with the loop, the study's journal folder and, when the action reads the runtime, its connection. */
  async function parameterRequest(
    loop: EngineLoop,
    action: ParameterWorkRequest["action"],
    extra: Partial<Pick<ParameterWorkRequest, "values" | "review" | "physicalApproved">>,
  ): Promise<ParameterWorkRequest> {
    const connection = action === "draft" ? undefined : await context.connection();
    return {
      projectRoot: context.projectRoot,
      runtimeId: loop.runtimeId,
      loopId: loop.id,
      storageFolder: context.storageFolder,
      action,
      ...(connection ? { connection } : {}),
      ...extra,
    };
  }

  /** Shows a failure the engine answered for a parameter action: the failure itself, and the page's own refusal. */
  async function parameterFailed(scope: LoopMessageScope, failure: Failure): Promise<void> {
    await context.postFailure(scope, "parameters", failure);
    await phase(scope, "parameters", { status: "failed", reason: failure.message });
  }

  /**
   * Reports the parameter write when its operation ends (C12, E7.4): the record the runtime confirmed, or the failure that ended it.
   */
  function watchParameters(operationId: string, scope: LoopMessageScope, rollback: boolean): void {
    const subscription = context.client.onUpdate(operationId, (update: OperationUpdate) => {
      if (update.state === "running") return;
      subscription.dispose();
      watchers.delete(subscription);
      const observed = update.result?.observed;
      const values = update.result?.values;
      if (update.state === "finished" && observed !== undefined) {
        void phase(scope, "parameters", { status: "applied", values, observed, reason: rollback
          ? "Previous values restored and observed after ST execution."
          : "Reviewed values observed after ST execution." });
        return;
      }
      void phase(scope, "parameters", { status: "failed", reason: update.failure?.message ?? "The parameter write was not confirmed." });
    });
    watchers.add(subscription);
  }

  /** Stops the campaign the route started, through its restoration path (`controlStudio/cancel`). */
  async function abortTest(scope: LoopMessageScope): Promise<boolean> {
    const operationId = running;
    if (!operationId) {
      // The page shows a test that the engine does not run: its busy state ends with the answer.
      await phase(scope, "engine-ended", { status: "unknown", reason: "No test is running." });
      return true;
    }
    const reply = await context.ask(scope, "cancel", () => context.client.cancel({ operationId }));
    // The engine serves the loop even when it cannot be reached: its start failure is shown, and the Abort is not repeated.
    if (reply && "failure" in reply) await context.postFailure(scope, "cancel", reply.failure);
    return true;
  }

  return {
    async handle(message, scope) {
      const loop = context.served();
      if (!loop) return false;
      const payload = record(message.payload) ?? {};
      switch (message.kind) {
        case "control-studio.experiment.review":
          return reviewTest(loop, scope, payload);
        case "control-studio.experiment.approve":
          return startTest(loop, scope, payload);
        case "control-studio.experiment.abort":
          return abortTest(scope);
        // The page's Cancel stops the test while the route runs one; the other stages' flows keep their own Cancel.
        case "control-studio.flow.cancel":
          return running ? abortTest(scope) : false;
        // The parameter tool of a declared analysis (C12, B6.1); a loop without one answers that the tool is unavailable.
        case "control-studio.parameters.inspect":
        case "control-studio.parameters.review":
        case "control-studio.parameters.review-rollback":
        case "control-studio.parameters.apply":
        case "control-studio.parameters.invalidate":
        case "control-studio.parameters.selected-analysis":
          return parameters(loop, message.kind.slice("control-studio.parameters.".length), payload, scope);
        default:
          return false;
      }
    },
    reset(): void {
      review = undefined;
      running = undefined;
      parameterReview = undefined;
      for (const watcher of watchers) watcher.dispose();
      watchers.clear();
    },
    async adopt(operationId: string, scope: LoopMessageScope): Promise<void> {
      // The campaign is the test's (`running`), so Abort reaches it; the watch reports its end and the page's test section shows it running.
      running = operationId;
      watch(operationId, scope);
      await phase(scope, "running", { experiment_id: operationId });
    },
  };
}
