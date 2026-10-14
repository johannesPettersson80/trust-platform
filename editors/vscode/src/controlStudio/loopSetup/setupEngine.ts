import type { EngineAnswer } from "../engine/engineClient";
import { unclassifiedFailure } from "../engine/engineRouteContext";
import type { Failure } from "../generated/Failure";
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

/** A refusal as the page shows it: the engine's message, then the words of its next action (A11.2) when it names one. */
export function refusalText(failure: Failure): string {
  return [failure.message, failure.nextActionLabel ? `Next: ${failure.nextActionLabel}` : ""].filter(Boolean).join(" ");
}

/** What the engine's answer to a save means for the setup (B1.3). */
export type SetupSaveOutcome =
  | { readonly kind: "saved"; readonly file: string }
  | { readonly kind: "parked" }
  | { readonly kind: "refused"; readonly failure: Failure };

/**
 * The save's outcome. A structure the engine does not run yet is refused as controller-capability until stage V7; the TypeScript
 * generator keeps those loops, so that refusal is `parked` and not shown as a failure.
 */
export function setupSaveOutcome(answer: EngineAnswer<SetupSaveResult>): SetupSaveOutcome {
  if ("failure" in answer) {
    return answer.failure.cause === "controller-capability" ? { kind: "parked" } : { kind: "refused", failure: answer.failure };
  }
  const [first] = answer.result.loops;
  return first ? { kind: "saved", file: first.file } : { kind: "refused", failure: unclassifiedFailure("The engine wrote no loop.") };
}
