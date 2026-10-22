import type { Failure } from "../../generated/Failure";
import type { IdentifyRequest } from "../../generated/IdentifyRequest";
import type { IdentifyReview } from "../../generated/IdentifyReview";
import type { ParameterWorkAnswer } from "../../generated/ParameterWorkAnswer";
import type { ParameterWorkRequest } from "../../generated/ParameterWorkRequest";
import type { OperationUpdate } from "../../generated/OperationUpdate";
import {
  record,
  type EngineLoop,
  type EngineRouteContext,
  type EngineScope,
  type EngineStageRoute,
} from "../engineRouteContext";

/**
 * The whole refusal the page shows: the failure's detail when it says more than its first sentence, else its message (A11.1).
 * The engine decides the words; the page only chooses which of its two texts to show.
 */
function wholeRefusal(failure: Pick<Failure, "message" | "detail">): string {
  return failure.detail || failure.message;
}

/** The project test's stage route: `handle` and `reset`, and `adopt`, which takes the test's campaign the engine kept running (A8.6). */
export interface ExperimentStageRoute extends EngineStageRoute {
  /**
   * Takes the campaign of a project-scripted loop that the engine runs when the panel reopens the loop (A8.6, C12): the test section
   * shows it running, its Abort stops it, and its end is reported as the test's end. The panel calls it only for a loop that declares
   * its execution owner, which is the rule that offers the project test.
   */
  adopt(operationId: string, scope: EngineScope): Promise<void>;
}

/**
 * The project test of a project-scripted loop on the Identify page (specification 35, C12 and §0.1a). The page's review is the
 * engine's Identify review, `controlStudio/identify` with `action: "review"` and no settings: the project owns the experiment, so the
 * engine reviews the loop's declared one. The review's identity is the engine's (E11.8), so this route mints none. The page confirms
 * the start with that identity and, on a physical target, with the engineer's approval; the engine checks both. A request that finds
 * no runtime running carries the lifecycle's mode, and the engine answers the next action that applies, Start simulator or Connect
 * runtime (A2.8). The start is `identify` with the identity the page names, and Abort is `controlStudio/cancel` of that campaign.
 * The parameter tool of a project-scripted loop (C12, B6.1) is the engine's `controlStudio/parameters`: the route sends each request
 * of the page and posts each answer with the tool's offer, the progress of a write, and the engine's refusal of a loop that has no
 * tool. When a test ends, however it ended, the route asks the tool (`released`) and posts its sentence and its offer with the test's
 * end. Each answer is read by the kind the engine names (§0.1a), and an answer of another kind ends the page's wait with no sentence.
 * The route holds only the identities and the values the engine answered, and every sentence it posts is the engine's.
 * Written in V6 step S10, parameters in V7.
 */
export function createExperimentRoute(context: EngineRouteContext): ExperimentStageRoute {
  /** The campaign the engine runs for the page's test; Abort and the page's Cancel stop it. */
  let running: string | undefined;
  /**
   * The parameter review the engine answered last (C12): its identity, and the values an Apply's review read, which the confirmation
   * sends back. A confirmation spends it, so the page reviews again after it.
   */
  let parameterReview: { identity: string; values?: Record<string, number> } | undefined;
  /** The listeners of the campaigns the route started; each one ends with its campaign. */
  const watchers = new Set<{ dispose(): void }>();

  /** Posts one answer of the test to the page (`control-studio.experiment.result`, read by the page's experiment section). */
  function phase(scope: EngineScope, name: string, payload: Record<string, unknown>): Promise<void> {
    return context.post(scope, { kind: "control-studio.experiment.result", phase: name, payload });
  }

  /** What a live request tells the engine of the runtime: its connection, or, when none is running, the lifecycle's mode (A2.8, E11.1). */
  async function liveFields(): Promise<Pick<IdentifyRequest, "connection" | "runtimeMode">> {
    const connection = await context.connection();
    if (connection) return { connection };
    const runtimeMode = await context.runtimeMode?.();
    return runtimeMode ? { runtimeMode } : {};
  }

  /** Answers the review of the declared project test with the identity the engine gives it. */
  async function reviewTest(loop: EngineLoop, scope: EngineScope): Promise<boolean> {
    // The review reads the runtime's sample when one is connected, as its start does (E11.8), so both give the same identity.
    const request: IdentifyRequest = {
      action: { action: "review" },
      ...(await liveFields()),
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
      await phase(scope, "review", { kind: "rejected", reason: wholeRefusal(reply.failure) });
      return true;
    }
    // A `review` is answered by the review (§0.1a), whose identity the start names; the page shows a review without one as no review.
    const review = reply.result as IdentifyReview;
    await phase(scope, "review", { kind: "reviewed", planId: review.identity, review });
    return true;
  }

  /** Follows one campaign to its end: its completion, its failure or its Abort ends the page's test (`engine-ended`). */
  function watch(operationId: string, scope: EngineScope): void {
    const subscription = context.client.onUpdate(operationId, (update: OperationUpdate) => {
      if (update.state === "running") return;
      if (running === operationId) running = undefined;
      subscription.dispose();
      watchers.delete(subscription);
      const finished = update.state === "finished";
      const result = record(update.result);
      const failure = record(update.failure);
      // The engine's sentence of the test's end (C12): a finished test's outcome, the sentence of a cancelled update, or the whole failure.
      const reason = finished ? text(result?.testOutcome)
        : update.state === "cancelled" ? text(update.message)
          : text(failure?.detail) ?? text(failure?.message);
      // The restoration the finished campaign confirmed (C12), in the engine's word, which the test's end carries.
      const restoration = finished ? text(result?.restoration) : undefined;
      // The fit's refusal a finished test carries (C12): its recording is kept and no model is. The engine words it; the page shows it.
      const note = finished ? text(result?.fitRefusal) : undefined;
      const ended = {
        status: update.state,
        ...(reason !== undefined ? { reason } : {}),
        ...(restoration !== undefined ? { restoration } : {}),
        ...(note !== undefined ? { note } : {}),
      };
      // The tool's state after the test (C12): the sentence that says the test released the experiment settings, which the engine words
      // exactly when the restoration is confirmed, and the tool's offer that follows. The engine is asked after every end alike, and the
      // test's end follows its answer.
      void askReleased(scope).then(released => phase(scope, "engine-ended", { ...ended, ...released }));
    });
    watchers.add(subscription);
  }

  /**
   * Asks the parameter tool whether the test that has just ended left the experiment settings released (C12): the engine answers its
   * sentence exactly when no operation works on the loop and its journal holds no unfinished operation, which is the case once the test's
   * restoration is confirmed, and the tool's offer that follows. Answers the fields the page takes with the test's end: the sentence and
   * the offer when the engine gives them; a refusal and a lost answer give none, and leave the tool as it was.
   */
  async function askReleased(scope: EngineScope): Promise<Record<string, unknown>> {
    const loop = context.served();
    if (!loop) return {};
    try {
      const reply = await context.ask(scope, "parameters", async () => context.client.parameters(await parameterRequest(loop, "released", {})));
      if (!reply || "failure" in reply) return {};
      const answer = reply.result as ParameterWorkAnswer;
      const released = text(answer.settingsReleased);
      return { ...(released !== undefined ? { settingsReleased: released } : {}), ...(answer.offer ? { offer: answer.offer } : {}) };
    } catch {
      return {};
    }
  }

  /**
   * Starts the campaign of the review the page confirms. The start names the identity the page shows, and the engine compares it with
   * the campaign as it is now, and on a physical target checks the approval the page passes. A refused start consumes the review: the
   * page reviews again.
   */
  async function startTest(loop: EngineLoop, scope: EngineScope, payload: Record<string, unknown>): Promise<boolean> {
    // A start while a test runs is the engine's refusal: the loop is claimed by the running test (A8.5), and the refusal is shown.
    const request: IdentifyRequest = {
      action: {
        action: "identify",
        ...(typeof payload.planId === "string" ? { review: payload.planId } : {}),
        physicalApproved: payload.physicalApproved === true,
      },
      ...(await liveFields()),
      loopId: loop.id,
      projectRoot: context.projectRoot,
      runtimeId: loop.runtimeId,
      storageFolder: context.storageFolder,
    };
    const reply = await context.ask(scope, "identify", () => context.client.identify(request));
    if (!reply) {
      await phase(scope, "failed", {});
      return true;
    }
    if ("failure" in reply) {
      await context.postFailure(scope, "identify", reply.failure);
      await phase(scope, "failed", { reason: wholeRefusal(reply.failure) });
      return true;
    }
    // A confirmed start names the operation it started (`operationStarted`, §0.1a).
    const started = record(reply.result);
    const operationId = started?.kind === "operationStarted" ? text(started.operationId) : undefined;
    if (!operationId) {
      await phase(scope, "failed", {});
      return true;
    }
    running = operationId;
    context.follow(operationId, scope);
    watch(operationId, scope);
    await phase(scope, "running", { experiment_id: operationId });
    return true;
  }

  /**
   * The parameter tool of a project-scripted loop (C12, B6.1): the engine reviews and writes the parameters of a declared analysis, or the
   * experiment's settings. Which tool a loop has is the engine's decision, so each request goes to the engine, and a loop without a tool is
   * refused there. The page asks only for the tool its form named.
   */
  async function parameters(loop: EngineLoop, action: string, payload: Record<string, unknown>, scope: EngineScope): Promise<boolean> {
    switch (action) {
      case "inspect":
        return inspectParameters(loop, scope);
      case "review":
        return reviewParameters(loop, "review", payload.json, scope);
      case "review-rollback":
        return reviewParameters(loop, "reviewRestore", undefined, scope);
      case "apply":
        return confirmParameters(loop, payload, scope);
      case "invalidate":
        // The page's edit changes nothing the engine holds: the review is spent, and the engine answers the sentence the page shows.
        parameterReview = undefined;
        return invalidateParameters(loop, scope);
      default:
        return false;
    }
  }

  /**
   * What the page shows of an answer of the tool (C12): the values, the record and the binding the answer names, the engine's sentence,
   * and the tool's offer, which the page renders as its step and its actions. A field the answer does not carry is left out.
   */
  function toolAnswer(status: string, answer: ParameterWorkAnswer, extra: Record<string, unknown> = {}): Record<string, unknown> {
    return {
      status,
      ...extra,
      ...(answer.values ? { values: answer.values } : {}),
      ...(answer.observed ? { observed: answer.observed } : {}),
      ...(answer.binding ? { binding: answer.binding } : {}),
      ...(typeof answer.message === "string" ? { reason: answer.message } : {}),
      ...(answer.offer ? { offer: answer.offer } : {}),
    };
  }

  /** The page's edit after a review: the engine answers the sentence the page shows and the tool's offer (`invalidate`, C12). Nothing is written. */
  async function invalidateParameters(loop: EngineLoop, scope: EngineScope): Promise<boolean> {
    const reply = await context.ask(scope, "parameters", async () => context.client.parameters(await parameterRequest(loop, "invalidate", {})));
    if (!reply) {
      await phase(scope, "parameters", { status: "failed" });
      return true;
    }
    if ("failure" in reply) {
      await parameterFailed(scope, reply.failure);
      return true;
    }
    await phase(scope, "parameters", toolAnswer("edited", reply.result as ParameterWorkAnswer));
    return true;
  }

  /** The engine's record of the tool's parameters, for the page to show (`inspect`), with the binding the engine names and its offer. */
  async function inspectParameters(loop: EngineLoop, scope: EngineScope): Promise<boolean> {
    const reply = await context.ask(scope, "parameters", async () => context.client.parameters(await parameterRequest(loop, "inspect", {})));
    if (!reply) {
      await phase(scope, "parameters", { status: "failed" });
      return true;
    }
    if ("failure" in reply) {
      await parameterFailed(scope, reply.failure);
      return true;
    }
    await phase(scope, "parameters", toolAnswer("inspected", reply.result as ParameterWorkAnswer));
    return true;
  }

  /**
   * The review of an Apply (`review`) or of a Restore (`reviewRestore`). The values go as the text the engineer typed: the engine reads
   * the text, refuses one that is no object of numbers with its own sentence, and answers the values it read, which the confirmation
   * sends back (C12). The route reads no JSON.
   */
  async function reviewParameters(loop: EngineLoop, action: "review" | "reviewRestore", json: unknown, scope: EngineScope): Promise<boolean> {
    parameterReview = undefined;
    const typed = typeof json === "string" ? json : "";
    const reply = await context.ask(scope, "parameters", async () => context.client.parameters(await parameterRequest(loop, action, action === "review" ? { json: typed } : {})));
    if (!reply) {
      await phase(scope, "parameters", { status: "failed" });
      return true;
    }
    if ("failure" in reply) {
      await parameterFailed(scope, reply.failure);
      return true;
    }
    // A review names the identity that its confirmation names (C12, E11.8).
    const answer = reply.result as ParameterWorkAnswer;
    const identity = answer.identity;
    if (typeof identity !== "string") {
      await phase(scope, "parameters", { status: "failed" });
      return true;
    }
    // The values are the ones the engine read from the text; a Restore's review has none, and its confirmation sends none.
    parameterReview = { identity, ...(answer.values ? { values: answer.values } : {}) };
    await phase(scope, "parameters", toolAnswer("reviewed", answer, { reviewId: identity }));
    return true;
  }

  /**
   * The confirmation of the review the page shows (`confirm`). The start names the page's identity with the values the engine's last
   * review read, and the engine decides from the review that identity names whether the write applies the values or restores the
   * values from before, and refuses a record that changed since (E11.8). The write's answer follows as its operation's update.
   */
  async function confirmParameters(loop: EngineLoop, payload: Record<string, unknown>, scope: EngineScope): Promise<boolean> {
    const review = parameterReview;
    parameterReview = undefined;
    const extra = {
      ...(typeof payload.reviewId === "string" ? { review: payload.reviewId } : {}),
      physicalApproved: payload.physicalApproved === true,
      ...(review?.values ? { values: review.values } : {}),
    };
    const reply = await context.ask(scope, "parameters", async () => context.client.parameters(await parameterRequest(loop, "confirm", extra)));
    if (!reply) {
      await phase(scope, "parameters", { status: "failed" });
      return true;
    }
    if ("failure" in reply) {
      await parameterFailed(scope, reply.failure);
      return true;
    }
    // A confirmation names the write's operation (`operationStarted`, §0.1a), whose updates follow.
    const started = record(reply.result);
    const operationId = started?.kind === "operationStarted" ? text(started.operationId) : undefined;
    if (!operationId) {
      await phase(scope, "parameters", { status: "failed" });
      return true;
    }
    watchParameters(operationId, scope);
    return true;
  }

  /** The request of one parameter action, with the loop, the study's journal folder and, when the action reads the runtime, its connection. */
  async function parameterRequest(
    loop: EngineLoop,
    action: ParameterWorkRequest["action"],
    extra: Partial<Pick<ParameterWorkRequest, "values" | "review" | "physicalApproved">> & { json?: string },
  ): Promise<ParameterWorkRequest> {
    // The actions that read no runtime are sent without its connection: the edit and the question whether the settings are released
    // (E11.1).
    const connection = action === "invalidate" || action === "released" ? undefined : await context.connection();
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

  /** Shows a failure the engine answered for a parameter action: the failure itself, and its sentence in the tool. */
  async function parameterFailed(scope: EngineScope, failure: Failure): Promise<void> {
    await context.postFailure(scope, "parameters", failure);
    await phase(scope, "parameters", { status: "failed", reason: failure.message });
  }

  /**
   * Reports the parameter write (C12, E7.4): its progress while it waits, in the engine's words, and, when its operation ends, the
   * engine's final answer, which says whether the write applied its values, in the engine's words, and names the tool's offer. An
   * operation that ends without that answer is reported with its own sentence.
   */
  function watchParameters(operationId: string, scope: EngineScope): void {
    const subscription = context.client.onUpdate(operationId, (update: OperationUpdate) => {
      if (update.state === "running") {
        const progress = text(update.progress);
        if (progress !== undefined) void phase(scope, "parameters", { status: "running", reason: progress });
        return;
      }
      subscription.dispose();
      watchers.delete(subscription);
      const outcome = update.state === "finished" ? update.result as ParameterWorkAnswer | undefined : undefined;
      if (outcome?.status) {
        void phase(scope, "parameters", toolAnswer(outcome.status, outcome));
        return;
      }
      const failure = record(update.failure);
      const reason = text(failure?.detail) ?? text(failure?.message) ?? text(update.message);
      void phase(scope, "parameters", { status: "failed", ...(reason !== undefined ? { reason } : {}) });
    });
    watchers.add(subscription);
  }

  /** Stops the campaign the route started, through its restoration path (`controlStudio/cancel`). */
  async function abortTest(scope: EngineScope): Promise<boolean> {
    const operationId = running;
    if (!operationId) {
      // The page shows a test that the engine does not run: its busy state ends with the answer, which has no sentence of its own.
      await phase(scope, "engine-ended", { status: "unknown" });
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
          return reviewTest(loop, scope);
        case "control-studio.experiment.approve":
          return startTest(loop, scope, payload);
        case "control-studio.experiment.abort":
          return abortTest(scope);
        // The page's Cancel stops the test while the route runs one; the other stages' flows keep their own Cancel.
        case "control-studio.flow.cancel":
          return running ? abortTest(scope) : false;
        // The parameter tool of the loop (C12, B6.1): the declared analysis's parameters, or the experiment's settings.
        case "control-studio.parameters.inspect":
        case "control-studio.parameters.review":
        case "control-studio.parameters.review-rollback":
        case "control-studio.parameters.apply":
        case "control-studio.parameters.invalidate":
          return parameters(loop, message.kind.slice("control-studio.parameters.".length), payload, scope);
        default:
          return false;
      }
    },
    reset(): void {
      running = undefined;
      parameterReview = undefined;
      for (const watcher of watchers) watcher.dispose();
      watchers.clear();
    },
    async adopt(operationId: string, scope: EngineScope): Promise<void> {
      // The campaign is the test's (`running`), so Abort reaches it; the watch reports its end and the page's test section shows it running.
      running = operationId;
      watch(operationId, scope);
      await phase(scope, "running", { experiment_id: operationId });
    },
  };
}

/** A text of an answer the engine wrote, or undefined for any other value and for an empty one. */
function text(value: unknown): string | undefined {
  return typeof value === "string" && value.length > 0 ? value : undefined;
}
