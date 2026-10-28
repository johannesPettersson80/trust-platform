import { applyStudioTargetLabel } from "./studioTargetLabel";
import { receiveEngineMessage } from "./engineOpened";

export type StudioPage = "model" | "tune" | "verify";
/**
 * Tells the chart and the Tune and Requirements editors what a stage brought: the `engineer` and `flow` stages, and a manual
 * preview's answer (specification 35, B9.6). Every answer of the engine reaches the page through it, so each listener hears the same
 * answers.
 */
export function dispatchEngineerEvent(stage: string, payload: Record<string, unknown>): void {
  const preview = stage === "tune" && payload.status === "preview";
  if (stage === "engineer" || stage === "flow" || preview) window.dispatchEvent(new CustomEvent("trust-control-studio-engineer", { detail: payload }));
}
export function bindThreePageHostState(root: HTMLElement, receive: (stage: string, payload: Record<string, unknown>) => void,
  select: (page: StudioPage, persist: boolean) => void,
  receiveEnvelope?: (message: Record<string, unknown>) => void): void {
  /** The stage router, then the engineer event: the engine's answers reach the page's listeners in that order. */
  const receiveEngine = (stage: string, payload: Record<string, unknown>): void => {
    receive(stage, payload);
    dispatchEngineerEvent(stage, payload);
  };
  window.addEventListener("message", event => {
    const message = event.data;
    if (!record(message) || message.schemaVersion !== 1 || message.projectId !== root.dataset.projectId
      || message.editorSessionId !== root.dataset.editorSessionId || message.loopId !== root.dataset.loopId
      || (message.expectedLoopRevision ?? message.loopRevision) !== root.dataset.loopRevision) return;
    if (message.kind === "control-studio.target-label") {
      applyStudioTargetLabel(root, message);
      return;
    }
    if (typeof message.kind === "string" && message.kind.startsWith("control-studio.engine.")) {
      receiveEngineMessage(root, message, { select, receive: receiveEngine });
      return;
    }
    receiveEnvelope?.(message);
    if (message.kind !== "control-studio.engineering.result" || !record(message.payload)) return;
    const payload = message.payload;
    receive(String(message.stage), payload);
    dispatchEngineerEvent(String(message.stage), payload);
  });
}
function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
