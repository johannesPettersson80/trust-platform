import { renderEngineerStageEvidence } from "./engineerStageEvidence";

/** Keep connected controller readiness separate from an attempted model import. */
export function renderGeneralModelResult(
  payload: Record<string, unknown> | undefined,
  actions: { imported: () => void; resultAvailable: () => void },
): void {
  if (payload?.status === "imported") actions.imported();
  if (payload?.stageEvidence) renderEngineerStageEvidence(payload.stageEvidence);
  else {
    const tune = [...document.querySelectorAll<HTMLButtonElement>("[data-control-studio-mode]")]
      .find(button => button.textContent?.trim() === "Tune");
    if (tune) {
      const state = payload?.status === "complete" ? "complete" : payload?.status === "running" ? "running" : "needs-data";
      tune.dataset.status = state;
      tune.setAttribute("aria-label", `Tune, ${state === "complete" ? "Complete" : state === "running" ? "Running" : "Needs data"}`);
      tune.title = payload?.status === "complete" ? "Supplied plant model analysis complete" : String(payload?.reason ?? "Import and analyze a plant model");
    }
  }
  window.dispatchEvent(new CustomEvent("trust-control-studio-general-model", { detail: payload }));
  if (payload?.status === "complete") actions.resultAvailable();
}
