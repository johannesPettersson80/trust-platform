import { textOf } from "./engineIdentify";
import type { EngineHostPorts } from "./engineOpened";
import { record, studioFailure } from "./engineShared";
import type { EngineStageReceiver } from "./engineStages";

/**
 * Shows the engine's Apply, Restore and Keep in project answers (specification 35, B6 and B9.3). The three methods share this
 * receiver, so an answer names its method by its own fields: a review has an `action` (Apply or Restore) and `parameters`, a Keep
 * review has `files`, an Apply outcome has a `checkpointId`, a Restore outcome `restored`, and a Keep result `kept`. Each answer
 * becomes the engineer-stage payload that `threePageApplication.ts` shows and confirms; the engine's availability flags pass on
 * unchanged, and no rule of when an action is allowed is decided here.
 */
export const applicationReceiver: EngineStageReceiver = {
  /** A reopened loop offers Restore, Keep in project and the resume of an Apply observation as the engine answers them (B6.4, A8.6). */
  opened(_root: HTMLElement, opened: Record<string, unknown>, ports: EngineHostPorts): void {
    ports.receive("engineer", openedApplicationPayload(opened));
  },
  result(_root: HTMLElement, result: Record<string, unknown>, ports: EngineHostPorts): void {
    ports.receive("engineer", applicationPayload(result));
  },
  failure(_root: HTMLElement, failure: Record<string, unknown>, ports: EngineHostPorts): void {
    ports.receive("engineer", failurePayload(failure));
  },
  /**
   * The progress of a running Apply, Restore or Keep, and its end when the engineer cancelled it, so that Abort does not leave the
   * button on "Applying…". A finished operation's answer follows as a result and a failed one as a failure, so neither is shown here.
   */
  update(_root: HTMLElement, update: Record<string, unknown>, ports: EngineHostPorts): void {
    const applicationAction = applicationActionOf(update.command);
    if (!applicationAction) return;
    if (update.state === "running") {
      const progress = textOf(update.progress);
      ports.receive("engineer", { status: "running", applicationAction, ...(progress ? { reason: progress } : {}) });
    } else if (update.state === "cancelled") {
      ports.receive("engineer", { status: "cancelled", applicationAction, reason: "The operation was cancelled." });
    }
  },
};

/**
 * What an opened loop offers for Apply (A8.6, B7): Restore and Keep in project as the engine answers them, and whether a journaled
 * Apply observation waits to be resumed. The `control-studio.engine.opened` answer carries these fields; the Apply receiver's `opened`
 * hook sends this payload to the page.
 */
export function openedApplicationPayload(opened: Record<string, unknown>): Record<string, unknown> {
  return {
    status: "opened",
    engineOutcome: engineOutcome(opened),
    applyObservationResumable: opened.applyObservationResumable === true,
  };
}

/** The engineer-stage payload of one Apply, Restore or Keep answer. */
function applicationPayload(result: Record<string, unknown>): Record<string, unknown> {
  if (typeof result.action === "string" && Array.isArray(result.parameters)) {
    return { status: "review", applicationAction: result.action, engineReview: result };
  }
  if (Array.isArray(result.files)) return { status: "review", applicationAction: "keep", engineKeepReview: result };
  if ("checkpointId" in result) return applyOutcomePayload(result);
  if ("restored" in result) return restoreOutcomePayload(result);
  if ("kept" in result && "record" in result) return { status: "kept", applicationAction: "keep", reason: textOf(result.reason) };
  return { status: "failed", reason: "The engine answered the request with a result the page does not show." };
}

/**
 * A failed call or a failed operation, with the engine's next action (B6.7). A failure whose next action is Restore also opens the
 * page's recovery action, which asks the engine for the Restore review (`control-studio.engineer.rollback`).
 */
function failurePayload(failure: Record<string, unknown>): Record<string, unknown> {
  return {
    status: "failed",
    reason: textOf(failure.message),
    failure: studioFailure(failure),
    ...(failure.nextAction === "restore" ? { commissioningCheckpoint: true } : {}),
  };
}

/**
 * The outcome of an Apply: passed, cancelled, or failed with the engine's sentence and next action (B6.3, B6.7). The availability
 * of Restore and Keep in project is the engine's answer, passed on unchanged.
 */
function applyOutcomePayload(outcome: Record<string, unknown>): Record<string, unknown> {
  const commissioning = record(outcome.result) ?? {};
  const status = commissioning.status === "passed" ? "applied" : commissioning.status === "cancelled" ? "cancelled" : "failed";
  const failure = record(commissioning.failure);
  return {
    status,
    applicationAction: "apply",
    reason: textOf(commissioning.reason),
    ...(status === "failed" && failure ? { failure: studioFailure(failure) } : {}),
    engineOutcome: engineOutcome(outcome),
  };
}

/**
 * The outcome of a Restore. Restored is shown only after the runtime confirmed it (B6.4), and the availability of Restore and Keep in
 * project comes from the engine: a confirmed Restore offers neither (B6.4, B6.5).
 */
function restoreOutcomePayload(outcome: Record<string, unknown>): Record<string, unknown> {
  return {
    status: outcome.restored === true ? "commissioned" : "failed",
    applicationAction: "restore",
    reason: textOf(outcome.reason),
    engineOutcome: engineOutcome(outcome),
  };
}

/** The availability of Restore and Keep in project that an answer carries (B6.4, B6.5): the page shows it and decides nothing from it. */
function engineOutcome(answer: Record<string, unknown>): Record<string, unknown> {
  return { restoreAvailable: answer.restoreAvailable === true, keepAvailable: answer.keepAvailable === true };
}

/** The method an operation's command names, when it is one of the three reviewed actions. */
function applicationActionOf(command: unknown): "apply" | "restore" | "keep" | undefined {
  return command === "apply" || command === "restore" || command === "keep" ? command : undefined;
}
