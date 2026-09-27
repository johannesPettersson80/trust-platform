import { setIdentificationActionState } from "./identificationActions";

export function identificationQualificationStatus(value: Record<string, unknown>): string {
  if (value.cancelled === true) return "Calculation cancelled. The recording is retained.";
  return value.nextFitRole === "validation"
    ? "Validation recording qualified. Fit it against the retained estimation run."
    : "Estimation recording qualified. Fit a preview, then record a distinct validation run.";
}

/** Project the existing calculation lifecycle into the shared Identify action row. */
export function initializeIdentificationJobControls(post: (kind: string) => void): void {
  const root = document.querySelector<HTMLElement>("[data-control-studio]");
  const cancel = document.querySelector<HTMLButtonElement>("[data-identify-cancel]");
  if (!root || !cancel) return;
  document.querySelector("[data-identify-fit]")?.addEventListener("click", () => setIdentificationActionState("running"));
  cancel.addEventListener("click", () => { cancel.disabled = true; post("control-studio.identify.cancel"); });
  window.addEventListener("message", event => {
    const message = event.data;
    if (message?.kind !== "control-studio.engineering.result" || message.stage !== "identify"
      || message.projectId !== root.dataset.projectId || message.loopId !== root.dataset.loopId
      || message.editorSessionId !== root.dataset.editorSessionId || message.expectedLoopRevision !== root.dataset.loopRevision) return;
    if (message.payload?.status === "running") setIdentificationActionState("running");
    if (message.payload?.status === "failed") setIdentificationActionState("imported");
  });
}
