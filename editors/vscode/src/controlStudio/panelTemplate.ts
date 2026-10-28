import { renderThreePageStudioHtml } from "./threePageTemplate";
// Control Studio panel HTML template: builds the webview document from a
// plain view model so the markup can be unit tested without a vscode host.

import type { LoopDefinition } from "./generated/OpenResult";

export function escapeHtml(value: string): string {
  return value.replace(/[&<>"']/g, (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[character] ?? character));
}

export interface ControlStudioView {
  readonly cspSource: string;
  readonly nonce: string;
  readonly chartScript: string;
  readonly themeStyles?: string;
  readonly workflowScript: string;
  readonly project: string;
  readonly session: string;
  readonly revision: string;
  readonly loop: string;
  readonly loopRevision: string;
  readonly processUnit: string;
  readonly outputUnit: string;
  readonly loopDefinition?: LoopDefinition;
  /** The selected target's name and speed, such as "Simulation ×10" (specification 35, B9.2). */
  readonly targetLabel?: string;
}

/** Composes the full Control Studio webview document from the precomputed view model. */
export function renderControlStudioHtml(view: ControlStudioView): string {
  return renderThreePageStudioHtml(view, escapeHtml, view.targetLabel);
}
