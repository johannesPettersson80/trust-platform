import { renderThreePageStudioHtml } from "./threePageTemplate";
// Control Studio panel HTML template: builds the webview document from a
// plain view model so the markup can be unit tested without a vscode host.

import type { PlantControlStudioHandoff } from "./handoff";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { SharedPidDesignAvailability } from "./loopDefinition";
import type { ControlExperimentDefaults } from "./experiment";
import {
  CONTROL_STUDIO_STAGE_ORDER,
  type ControlStudioStageId,
  type StageAvailability,
} from "./stageAvailability";

export function escapeHtml(value: string): string {
  return value.replace(/[&<>"']/g, (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[character] ?? character));
}

export interface AutomaticTuneButtonView {
  readonly hidden?: boolean;
  readonly disabled: boolean;
  /** False demotes the header button to secondary styling for a project-scripted loop. */
  readonly primary: boolean;
  readonly title: string;
}

/** Renders the header Automatic Tune action; unit tested independently of the full document. */
export function renderAutomaticTuneButton(view: AutomaticTuneButtonView): string {
  if (view.hidden) return "";
  const className = view.primary ? "automatic-tune" : "automatic-tune secondary";
  const disabledAttribute = view.disabled ? " disabled" : "";
  return `<button class="${className}" data-control-studio-automatic-tune${disabledAttribute} title="${escapeHtml(view.title)}">Automatic Tune</button>`;
}

/** Renders the five workflow tabs with the Specification 35 section 4.2 status vocabulary; unit tested independently of the full document. */
export function renderModeTabs(availability: Record<ControlStudioStageId, StageAvailability>): string {
  return CONTROL_STUDIO_STAGE_ORDER.map((stage, index) => {
    const info = availability[stage];
    const disabledAttribute = info.disabled ? " disabled" : "";
    return `<button class="mode" data-control-studio-mode role="tab" aria-selected="${index === 0}" tabindex="${index === 0 ? 0 : -1}" data-status="${info.status}" aria-label="${stage}, ${escapeHtml(info.label)}"${disabledAttribute}${info.reason ? ` title="${escapeHtml(info.reason)}"` : ""}>${stage}</button>`;
  }).join("");
}

export interface ControlStudioView {
  readonly cspSource: string;
  readonly nonce: string;
  readonly chartScript: string;
  readonly themeStyles?: string;
  readonly workflowScript: string;
  readonly plant: string;
  readonly sourceLabel: string;
  readonly runtime: string;
  readonly controllerInstance: string;
  readonly project: string;
  readonly session: string;
  readonly revision: string;
  readonly loop: string;
  readonly loopRevision: string;
  readonly processUnit: string;
  readonly outputUnit: string;
  readonly experimentDefaults: ControlExperimentDefaults;
  readonly experimentOptions: string;
  readonly pidDesign: SharedPidDesignAvailability;
  readonly tuneDescription: string;
  readonly tuneDisabledReason: string;
  readonly tuneStateText: string;
  readonly automaticTune: AutomaticTuneButtonView;
  readonly stageAvailability: Record<ControlStudioStageId, StageAvailability>;
  readonly loopDefinition?: ControlLoopDefinition;
  readonly guidedWorkflow: boolean;
  readonly handoff?: PlantControlStudioHandoff;
}

/** Composes the full Control Studio webview document from the precomputed view model. */
export function renderControlStudioHtml(view: ControlStudioView): string {
  return renderThreePageStudioHtml(view, escapeHtml);
}
