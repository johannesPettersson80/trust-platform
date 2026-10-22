import { textOf } from "./engineIdentify";
import type { EngineHostPorts } from "./engineOpened";
import { record, studioFailure } from "./engineShared";
import type { EngineStageReceiver } from "./engineStages";

/**
 * Shows the engine's Apply, Restore and Keep in project answers (specification 35, B6 and B9.3). The three methods share this
 * receiver, and each answer names its kind (§0.1a): the review of an Apply or a Restore (`applicationReview`), the review of a Keep
 * (`keepReview`), and the end of an Apply, a Restore or a Keep (`applyOutcome`, `restoreOutcome`, `keepResult`). Each answer
 * becomes the engineer-stage payload that `threePageApplication.ts` shows and confirms. The engine's offers (Apply, Restore, Keep in
 * project) and the action that holds the position pass on as the engine states them; the receiver reads the outcome's own words
 * (`APPLY_STATUS`) for the page's status and adds no offer, lock or checkpoint of its own.
 */
export const applicationReceiver: EngineStageReceiver = {
  /** A reopened loop offers Apply, Restore, Keep in project and the resume of an Apply observation as the engine answers them (B6.4, A8.6). */
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
   * The cancelled update carries the engine's sentence (`message`, B9.4); the page writes none.
   */
  update(_root: HTMLElement, update: Record<string, unknown>, ports: EngineHostPorts): void {
    const applicationAction = applicationActionOf(update.command);
    if (!applicationAction) return;
    if (update.state === "running") {
      const progress = textOf(update.progress);
      ports.receive("engineer", { status: "running", applicationAction, ...(progress ? { reason: progress } : {}) });
    } else if (update.state === "cancelled") {
      ports.receive("engineer", { status: "cancelled", applicationAction, reason: textOf(update.message) });
    }
  },
};

/**
 * What an opened loop offers for Apply (A8.6, B7): Apply, Restore and Keep in project and the action that holds the position as the
 * engine answers them, and whether a journaled Apply observation waits to be resumed. The `control-studio.engine.opened` answer carries
 * these fields; the Apply receiver's `opened` hook sends this payload to the page.
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
  switch (result.kind) {
    case "applicationReview":
      return { status: "review", applicationAction: result.action, engineReview: result };
    case "keepReview":
      return { status: "review", applicationAction: "keep", engineKeepReview: result };
    case "applyOutcome":
      return applyOutcomePayload(result);
    case "restoreOutcome":
      return restoreOutcomePayload(result);
    case "keepResult":
      return { status: "kept", applicationAction: "keep", reason: textOf(result.reason), engineOutcome: engineOutcome(result) };
    default:
      return { status: "failed", reason: "The engine answered the request with a result the page does not show." };
  }
}

/**
 * A failed call or a failed operation, with the engine's next action (B6.7). A next action Restore is the page's control for the Restore
 * request (`runNextAction`); while a checkpoint waits for it, the engine's stage statuses hold the other actions (B6.4).
 */
function failurePayload(failure: Record<string, unknown>): Record<string, unknown> {
  return {
    status: "failed",
    reason: textOf(failure.message),
    failure: studioFailure(failure),
  };
}

/** The page's word for each way a commissioning run ended (`CommissioningStatus`, B6.3): the engine's three words, renamed. */
const APPLY_STATUS = new Map<string, string>([["passed", "applied"], ["cancelled", "cancelled"], ["failed", "failed"]]);

/**
 * The outcome of an Apply: passed, cancelled, or failed with the engine's sentence and next action (B6.3, B6.7). The engine's offers
 * (Apply, Restore and Keep in project) are passed on unchanged. A word the page does not know is shown as failed, never as applied.
 */
function applyOutcomePayload(outcome: Record<string, unknown>): Record<string, unknown> {
  const commissioning = record(outcome.result) ?? {};
  const status = APPLY_STATUS.get(String(commissioning.status)) ?? "failed";
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
 * The outcome of a Restore. Restored is shown only after the runtime confirmed it (B6.4), and the engine's offers come with it: a
 * confirmed Restore offers neither Restore nor Keep in project (B6.4, B6.5) and no Apply until a validation passes again (B7). The stage
 * statuses the answer carries end the block of the checkpoint it resolved (`engineOpened.ts` shows them). A Restore on a runtime that
 * was replaced since Apply ends at once, not restored, with the engine's sentence as its reason (B6.7).
 */
function restoreOutcomePayload(outcome: Record<string, unknown>): Record<string, unknown> {
  return {
    status: outcome.restored === true ? "commissioned" : "failed",
    applicationAction: "restore",
    reason: textOf(outcome.reason),
    engineOutcome: engineOutcome(outcome),
  };
}

/**
 * The offers of a Validate answer or of an answer to a change of the study inputs (B5.4, B7, A8.4): Apply and the action that holds the
 * position, as the engine states them with the answer. The Validate answer's table and plot are the Validate receiver's
 * (`engineValidate.ts`), and the inputs answer's editors are the inputs receiver's (`engineInputs.ts`).
 */
export function offersPayload(answer: Record<string, unknown>): Record<string, unknown> {
  return { status: "offers", engineOutcome: engineOutcome(answer) };
}

/**
 * The offers that an answer carries (B6.4, B6.5, B7): Apply, Restore previous settings and Keep in project, each as the engine states
 * it, and the action that holds the position (`primaryAction`), which the answer leaves out when nothing is offered. A flag the answer
 * does not carry is left out, so the page keeps what the engine said before.
 */
function engineOutcome(answer: Record<string, unknown>): Record<string, unknown> {
  const offers: Record<string, unknown> = {};
  for (const key of ["applyAvailable", "restoreAvailable", "keepAvailable"]) {
    if (typeof answer[key] === "boolean") offers[key] = answer[key];
  }
  if (typeof answer.primaryAction === "string") offers.primaryAction = answer.primaryAction;
  return offers;
}

/** The method an operation's command names, when it is one of the three reviewed actions. */
function applicationActionOf(command: unknown): "apply" | "restore" | "keep" | undefined {
  return command === "apply" || command === "restore" || command === "keep" ? command : undefined;
}
