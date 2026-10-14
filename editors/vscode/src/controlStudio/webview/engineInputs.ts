import type { EngineStageReceiver } from "./engineStages";

/** The window event that carries the recorded inputs of an opened loop; its `detail` is the inputs (specification 35, A8.6). */
export const ENGINE_INPUTS_EVENT = "trust-control-studio-engine-inputs";

/** The window event that carries the page's own view choices an opened loop kept (`pageView`, A8.1, A8.6). */
export const ENGINE_VIEW_EVENT = "trust-control-studio-engine-view";

/** The page's words for a change that leaves the validation no longer current (A8.4): the words its stale results already show. */
const STALE_VALIDATION_REASON = "Inputs changed. Tune and verify before applying.";

/**
 * Shows the engine's answer to the study inputs (specification 35, §0.1a `inputs`, A8.4). A change's answer says whether it left
 * the validation current: a change that did not leaves the page as it is, and one that did shows a stale validation the way the
 * page shows any stale result. An answer on open carries the recorded inputs, which the page's editors take as their values. The
 * engine decides; this module only presents.
 */
export const inputsReceiver: EngineStageReceiver = {
  result(root, result, ports) {
    const inputs = record(result.inputs);
    if (inputs) {
      const view = root.ownerDocument.defaultView;
      view?.dispatchEvent(new view.CustomEvent(ENGINE_INPUTS_EVENT, { detail: inputs }));
    }
    if (record(result.invalidated)?.validation === true) {
      ports.receive("engineer", { status: "edited", reason: STALE_VALIDATION_REASON });
    }
  },
};

function record(value: unknown): Record<string, unknown> | undefined {
  return value && typeof value === "object" && !Array.isArray(value) ? (value as Record<string, unknown>) : undefined;
}
