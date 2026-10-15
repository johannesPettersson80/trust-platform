import type { ControlStudioEngine, EngineAnswer } from "../engine/engineClient";
import { unclassifiedFailure } from "../engine/engineRouteContext";
import type { Failure } from "../generated/Failure";
import type { SetupSaveRequest } from "../generated/SetupSaveRequest";
import type { SetupSaveResult } from "../generated/SetupSaveResult";

/*
 * The loop setup's engine answers, as the setup page and the three-page binding route show them (specification 35, B1.2, B1.3 and
 * §0.1a `setupDiscover` and `setupSave`). The engine decides every rule; these helpers only make the call and take its answer apart.
 */

/** One engine call: its answer, or the failure of a call that could not be made, such as an engine that did not start. */
export async function engineAnswer<T>(call: () => Promise<EngineAnswer<T>>): Promise<EngineAnswer<T>> {
  try {
    return await call();
  } catch (error) {
    return { failure: unclassifiedFailure(error) };
  }
}

/** A refusal as the page shows it: the engine's whole text (its detail when it says more than its message), then the words of its next action (A11.2) when it names one. */
export function refusalText(failure: Failure): string {
  return [failure.detail || failure.message, failure.nextActionLabel ? `Next: ${failure.nextActionLabel}` : ""].filter(Boolean).join(" ");
}

/** What the engine's answer to a save means for the setup (B1.3). */
export type SetupSaveOutcome =
  | { readonly kind: "saved"; readonly file: string }
  | { readonly kind: "parked" }
  | { readonly kind: "refused"; readonly failure: Failure };

/**
 * The save's outcome. A structure the engine parks is refused as controller-capability; the TypeScript generator keeps those loops, so that
 * refusal is `parked` and not shown as a failure. No structure is parked now.
 */
export function setupSaveOutcome(answer: EngineAnswer<SetupSaveResult>): SetupSaveOutcome {
  if ("failure" in answer) {
    return answer.failure.cause === "controller-capability" ? { kind: "parked" } : { kind: "refused", failure: answer.failure };
  }
  const [first] = answer.result.loops;
  return first ? { kind: "saved", file: first.file } : { kind: "refused", failure: unclassifiedFailure("The engine wrote no loop.") };
}

/** Saves a confirmed draft through the engine and takes its answer apart (B1.3): the file it wrote, a parked structure, or a refusal. */
export async function saveSetupDraft(engine: Pick<ControlStudioEngine, "setupSave">, request: SetupSaveRequest): Promise<SetupSaveOutcome> {
  return setupSaveOutcome(await engineAnswer(() => engine.setupSave(request)));
}
