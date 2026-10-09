import type { GeneralModel } from "../generalModel";
import { renderIdentifiedModel } from "./threePageModel";
import { applyStudioTargetLabel } from "./studioTargetLabel";

export type StudioPage = "model" | "tune" | "verify";
export function bindThreePageHostState(root: HTMLElement, receive: (stage: string, payload: Record<string, unknown>) => void,
  select: (page: StudioPage, persist: boolean) => void,
  receiveEnvelope?: (message: Record<string, unknown>) => void,
  /** A model reached the page; the operation gate decides whether Tune may run. */
  modelArrived?: () => void): void {
  window.addEventListener("message", event => {
    const message = event.data;
    if (!record(message) || message.schemaVersion !== 1 || message.projectId !== root.dataset.projectId
      || message.editorSessionId !== root.dataset.editorSessionId || message.loopId !== root.dataset.loopId
      || (message.expectedLoopRevision ?? message.loopRevision) !== root.dataset.loopRevision) return;
    if (message.kind === "control-studio.target-label") {
      applyStudioTargetLabel(root, message);
      return;
    }
    if (message.kind === "control-studio.navigation.restore") {
      if (record(message.view)) {
        const view = message.view;
        const method = root.querySelector<HTMLSelectElement>("[data-engineer-acquisition-method]");
        if (method && Array.from(method.options).some(option => option.value === view.acquisitionMethod)) method.value = String(view.acquisitionMethod);
        const signals = root.querySelector<HTMLSelectElement>("[data-three-page-experiment-signals]");
        if (signals && Array.isArray(view.experimentSignalIds)) {
          for (const option of Array.from(signals.options)) {
            option.selected = view.experimentSignalIds.includes(option.value);
            option.toggleAttribute("selected", option.selected);
          }
          if (method?.value === "project") window.dispatchEvent(new CustomEvent("trust-control-studio-experiment-signals", {
            detail: { signalIds: Array.from(signals.selectedOptions, option => option.value) } }));
        }
      }
      receiveEnvelope?.(message);
      if (["model", "tune", "verify", "apply"].includes(String(message.stage))) select(message.stage === "apply" ? "verify" : message.stage as StudioPage, false);
      root.dataset.hostReady = "true";
      return;
    }
    receiveEnvelope?.(message);
    if (message.kind !== "control-studio.engineering.result" || !record(message.payload)) return;
    const payload = message.payload;
    if (record(payload.identificationPreview)) {
      const preview = payload.identificationPreview;
      root.querySelector<HTMLElement>("[data-studio-model-type]")!.textContent = String(preview.name ?? "Plant model");
      root.querySelector<HTMLElement>("[data-studio-model-equation]")!.textContent = String(preview.equation ?? "");
      root.querySelector<HTMLElement>("[data-studio-model-result]")!.hidden = false;
    }
    if (record(payload.model) && ["transfer-function", "continuous-state-space", "discrete-state-space"].includes(String(payload.model.kind))) {
      // Tune is enabled by the page's operation gate when the model reaches it, never directly here.
      renderIdentifiedModel(root, payload.model as unknown as GeneralModel);
      modelArrived?.();
      const notice = root.querySelector<HTMLElement>('[data-studio-message="tune"]')!;
      notice.textContent = ""; notice.hidden = true;
    }
    receive(String(message.stage), payload);
    // A manual-tuning preview draws its predicted response on the same plot as a finished Tune.
    const preview = message.stage === "tune" && payload.status === "preview";
    if (message.stage === "engineer" || message.stage === "flow" || preview) window.dispatchEvent(new CustomEvent("trust-control-studio-engineer", { detail: payload }));
  });
}
function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
