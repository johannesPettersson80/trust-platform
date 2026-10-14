import { record } from "./engineShared";

/**
 * Passes the requirements that an engine answer holds to the Requirements editor (specification 35, B4.3 and B9.9). An answer
 * that holds an identification (Identify, a run, a reopened loop) names the requirements Tune designs to for that model: the
 * study's values as given, and Automatic's values for the rest, listed in `automaticCriteria`. This module hands that record to
 * the page, which holds it as the requirements its editor opens with and marks the Automatic values. It computes no value.
 */

/** The window event that carries an engine answer's requirements; its `detail` is the requirement record (`TuningRequirements`). */
export const ENGINE_REQUIREMENTS_EVENT = "trust-control-studio-engine-requirements";

/** Dispatches the event for a requirement record. Any other value is no requirement set, so nothing is dispatched. */
export function showEngineRequirements(root: HTMLElement, requirements: unknown): void {
  const detail = record(requirements);
  if (!detail) return;
  const view = root.ownerDocument.defaultView;
  view?.dispatchEvent(new view.CustomEvent(ENGINE_REQUIREMENTS_EVENT, { detail }));
}
