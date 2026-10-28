import type { ControlStudioEngine, EngineAnswer } from "../engine/engineClient";
import type { EngineProcess } from "../engine/engineProcess";
import { unclassifiedFailure } from "../engine/engineRouteContext";
import type { Failure } from "../generated/Failure";
import type { ConfiguredLoopRequest } from "../generated/ProjectLoopsResult";
import type { SetupSaveRequest } from "../generated/SetupSaveRequest";
import type { SetupSaveResult } from "../generated/SetupSaveResult";
import type { SetupStepRequest } from "../generated/SetupStepRequest";
import type { SetupStepResult } from "../generated/SetupStepResult";

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

/** What the engine's answer to a save means for the setup (B1.3): the file it wrote and the open request of its first loop, or its refusal. */
export type SetupSaveOutcome =
  | { readonly kind: "saved"; readonly file: string; readonly openRequest?: ConfiguredLoopRequest }
  | { readonly kind: "refused"; readonly failure: Failure };

/** The save's outcome: the first loop the engine wrote, with the request that opens it when the engine names one, or the engine's refusal (B1.3). */
export function setupSaveOutcome(answer: EngineAnswer<SetupSaveResult>): SetupSaveOutcome {
  if ("failure" in answer) return { kind: "refused", failure: answer.failure };
  const [first] = answer.result.loops;
  if (!first) return { kind: "refused", failure: unclassifiedFailure("The engine wrote no loop.") };
  // The key is left out when the engine names no request, so the outcome has only the fields the engine answered.
  const { openRequest } = answer.result;
  return openRequest ? { kind: "saved", file: first.file, openRequest } : { kind: "saved", file: first.file };
}

/** Saves a confirmed draft through the engine and takes its answer apart (B1.3): the file it wrote, or a refusal. */
export async function saveSetupDraft(engine: Pick<ControlStudioEngine, "setupSave">, request: SetupSaveRequest): Promise<SetupSaveOutcome> {
  return setupSaveOutcome(await engineAnswer(() => engine.setupSave(request)));
}

/** One step of the step-by-step setup (B1.3): the engine's answer to the session the page holds and the answer, or Back, the engineer gave. */
export function setupStep(engine: EngineProcess, request: SetupStepRequest): Promise<EngineAnswer<SetupStepResult>> {
  return engineAnswer(async () => (await engine.connection()).sendRequest<EngineAnswer<SetupStepResult>>("controlStudio/setupStep", request));
}
