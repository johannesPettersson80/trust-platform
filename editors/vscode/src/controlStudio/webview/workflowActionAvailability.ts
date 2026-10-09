import { controlStudioStageAvailability, type ControlStudioStageId } from "../stageAvailability";
import { setIdentificationActionState } from "./identificationActions";

export interface WorkflowActionAvailability {
  recording?: boolean;
  recordedResponse?: boolean;
  model?: boolean;
  candidate?: boolean;
  result?: boolean;
  validationPassed?: boolean;
  identificationFailed?: boolean;
}

interface PresentationState extends WorkflowActionAvailability {
  loopBound?: boolean;
  hasDeclaredAnalysis?: boolean;
  hasManualParameters?: boolean;
  engineerRecording?: boolean;
}
const states = new WeakMap<Document, PresentationState>();

function currentState(): PresentationState {
  return states.get(document) ?? {
    hasManualParameters: Boolean(document.querySelector('[data-workflow-pane="Tune"] [data-parameters]')),
    hasDeclaredAnalysis: Boolean(document.querySelector('[data-declared-root-run]')),
  };
}

/** Owns dependent-action availability so workflow stages share one visible gate. */
export function setWorkflowActionAvailability(update: WorkflowActionAvailability): void {
  const state = { ...currentState(), ...update };
  if (update.identificationFailed === undefined && (update.recording !== undefined || update.model === true)) state.identificationFailed = false;
  if (update.candidate !== undefined && update.validationPassed === undefined) state.validationPassed = undefined;
  if (update.recording !== undefined && update.recordedResponse === undefined) state.recordedResponse = update.recording;
  states.set(document, state);
  if (update.recording !== undefined) {
    const button = document.querySelector<HTMLButtonElement>("[data-identify-run]");
    if (button) button.disabled = !update.recording;
    setIdentificationActionState(update.recording ? "imported" : "initial");
  }
  if (update.model !== undefined) {
    if (update.model) setIdentificationActionState("accepted");
  }
  const evidence = { loopBound: state.loopBound, hasDeclaredAnalysis: state.hasDeclaredAnalysis,
    hasManualParameters: state.hasManualParameters, hasModel: state.model,
    hasCandidate: state.candidate, hasRecording: Boolean(state.recording || state.engineerRecording),
    identificationFailed: state.identificationFailed,
    validationHasResult: state.validationPassed !== undefined, validationPassed: state.validationPassed,
  };
  const stages = controlStudioStageAvailability("control-studio", evidence);
  stages.Validate = controlStudioStageAvailability("control-studio", {
    ...evidence, hasRecording: Boolean(state.recordedResponse || state.engineerRecording),
  }).Validate;
  for (const tab of document.querySelectorAll<HTMLButtonElement>("[data-control-studio-mode]")) {
    const stage = tab.textContent?.trim() as ControlStudioStageId;
    const availability = stages[stage];
    if (!availability || stage === "Test" && tab.dataset.statusLabel && state.loopBound !== false) continue;
    delete tab.dataset.statusLabel;
    tab.dataset.status = availability.status;
    tab.setAttribute("aria-label", `${stage}, ${availability.label}`);
  }
}
