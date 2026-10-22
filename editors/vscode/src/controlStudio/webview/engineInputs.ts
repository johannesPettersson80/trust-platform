import type { EngineHostPorts } from "./engineOpened";
import type { EngineStageReceiver } from "./engineStages";
import { showChannelsEditor } from "./engineShared";

/** The window event that carries the recorded inputs of an opened loop; its `detail` is the inputs (specification 35, A8.6). */
export const ENGINE_INPUTS_EVENT = "trust-control-studio-engine-inputs";

/** The window event that carries the page's own view choices an opened loop kept (`pageView`, A8.1, A8.6). */
export const ENGINE_VIEW_EVENT = "trust-control-studio-engine-view";

/**
 * The window event that tells the page the engine answered one change of the study inputs, in the order the page sent them (§0.1a `inputs`):
 * `refused` says the engine left the study as it was, and `limits` that the refusal is the Limits editor's (the page reopens it, B3.2). The echo
 * of the recorded inputs that follows an open answers no change, so it sends none.
 */
export const ENGINE_INPUTS_ANSWERED_EVENT = "trust-control-studio-engine-inputs-answered";

/** The `detail` of [`ENGINE_INPUTS_ANSWERED_EVENT`]. */
export interface InputsAnswered {
  readonly refused: boolean;
  readonly limits: boolean;
}

/**
 * Shows the engine's answer to the study inputs (specification 35, §0.1a `inputs`, A8.4). A change's answer says whether it left
 * the validation current: a change that did not leaves the page as it is, and one that did shows a stale validation the way the
 * page shows any stale result. An answer on open carries the recorded inputs, which the page's editors take as their values. The
 * engine decides; this module only presents.
 */
export const inputsReceiver: EngineStageReceiver = {
  opened(root, opened, ports) {
    showChannelsEditor(root, opened.channelChoices);
    showStateFields(opened, ports);
    // The open answer says whether the loop needs a measurement noise band (a declared filter; B4.3). The requirements dialog reads it.
    if (opened.measurementNoiseBandRequired === true) ports.receive("engineer", { measurementNoiseBandRequired: true });
  },
  result(root, result, ports) {
    const inputs = record(result.inputs);
    if (inputs) {
      const view = root.ownerDocument.defaultView;
      view?.dispatchEvent(new view.CustomEvent(ENGINE_INPUTS_EVENT, { detail: inputs }));
    }
    // The engine's answer to a change carries the whole result: the Channels… rows after the change, or none for a loop with one input and one
    // output. The echo of the recorded inputs after an open carries the inputs alone, and leaves the rows that the open answer gave.
    const changeAnswer = typeof result.changed === "boolean";
    if (changeAnswer) showChannelsEditor(root, result.channelChoices);
    showStateFields(result, ports);
    // The engine says the change left the validation not current (A8.4): the page hides that result, as the stage evidence says too, and says
    // why in the engine's words (`invalidated.reason`); it writes none of its own.
    const invalidated = record(result.invalidated);
    if (invalidated?.validation === true) {
      ports.receive("engineer", { status: "edited", reason: invalidated.reason, stageEvidence: record(result.stageEvidence), validationInvalidated: true });
    }
    // The engine lists the recorded analysis its change made stale (`stale`, B9.9): the Tune page's analysis drops those answers and asks nothing.
    if (Array.isArray(result.stale) && result.stale.length > 0) ports.receive("engineer", { staleAnalyses: result.stale });
    if (changeAnswer) tellAnswered(root, { refused: false, limits: false });
  },
  failure(root, failure) {
    // A refused change leaves the study as it was (A8.1): the page shows what the engine holds again where it shows the refused value.
    tellAnswered(root, { refused: true, limits: failure.nextAction === "open-limits" });
  },
};

/** Tells the page that the engine answered one change of the study inputs (see [`ENGINE_INPUTS_ANSWERED_EVENT`]). */
function tellAnswered(root: HTMLElement, detail: InputsAnswered): void {
  const view = root.ownerDocument.defaultView;
  view?.dispatchEvent(new view.CustomEvent(ENGINE_INPUTS_ANSWERED_EVENT, { detail }));
}

/** The design form's fields that the engine lists for the recorded state law (C4); an answer without them keeps the form as it is. */
function showStateFields(answer: Record<string, unknown>, ports: EngineHostPorts): void {
  if (Array.isArray(answer.stateFields)) ports.receive("engineer", { stateFields: answer.stateFields });
}

function record(value: unknown): Record<string, unknown> | undefined {
  return value && typeof value === "object" && !Array.isArray(value) ? (value as Record<string, unknown>) : undefined;
}
