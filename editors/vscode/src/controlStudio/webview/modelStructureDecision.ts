import type { SensorLagComparison } from "../sensorLagComparison";
import { modelStructureEvidence } from "./modelStructureEvidence";
import { setIdentificationActionState } from "./identificationActions";
import { setWorkflowActionAvailability } from "./workflowActionAvailability";

/** Projects the host's comparison and explicit choice; it owns no analysis state. */
export function initializeModelStructureDecision(post: (kind: string, payload?: Record<string, unknown>) => void) {
  const root = document.querySelector<HTMLElement>("[data-control-studio]");
  const pane = root?.querySelector<HTMLElement>('[data-workflow-pane="Identify"]');
  const summary = pane?.querySelector<HTMLElement>(".summary-panel");
  const title = summary?.querySelector("h3");
  const originalTitle = title?.textContent ?? "Model qualification";
  const status = pane?.querySelector<HTMLElement>("[data-identify-state]");
  const view = document.createElement("section"); view.dataset.modelStructure = ""; view.hidden = true;
  summary?.append(view);
  const clear = () => { view.hidden = true; delete view.dataset.choice; if (title) title.textContent = originalTitle; };
  const settings = () => {
    const details = pane?.querySelector<HTMLDetailsElement>("[data-step-response-settings]");
    if (!details) return;
    details.hidden = false; details.open = true;
    const first = details.querySelector<HTMLInputElement>("[data-step-field]"); first?.focus(); first?.scrollIntoView({ block: "center" });
  };
  window.addEventListener("message", event => {
    const message = event.data;
    if (!root || message?.projectId !== root.dataset.projectId || message.loopId !== root.dataset.loopId ||
      message.editorSessionId !== root.dataset.editorSessionId || message.expectedLoopRevision !== root.dataset.loopRevision) return;
    if (message.kind === "control-studio.identification-recording" && message.payload?.status === "imported"
      || message.kind === "control-studio.workflow-result" && ["opened", "restored"].includes(message.phase)
      || message.kind === "control-studio.engineering.result" && message.stage === "general-model" && message.payload?.status === "imported") clear();
    if (!view.hidden && message.kind === "control-studio.workflow-result" && status) {
      if (message.phase === "saved") status.textContent = "Comparison saved. The retained model is provisional until independent validation.";
      if (message.phase === "failed") status.textContent = String(message.payload?.reason ?? "The comparison could not be saved.");
    }
  });
  return { clear, render(payload: Record<string, unknown> | undefined): boolean {
    if (["editing", "qualified", "recording-selected"].includes(String(payload?.phase))) clear();
    if (payload?.status === "failed" && !view.hidden && status) {
      const choice = view.querySelector<HTMLButtonElement>("[data-structure-choice]");
      if (choice) choice.disabled = false;
      status.textContent = String(payload.reason ?? "The model choice could not be retained. Try again.");
      return true;
    }
    const comparison = (payload?.fit as { modelStructure?: SensorLagComparison } | undefined)?.modelStructure;
    if (!comparison || !summary || !status || payload?.status !== "complete") return false;
    const focusedChoice = document.activeElement?.matches("[data-structure-choice]");
    view.hidden = false; view.dataset.choice = comparison.choice ?? "";
    if (title) title.textContent = "Model comparison";
    setIdentificationActionState("comparison");
    setWorkflowActionAvailability({ model: false, candidate: false, result: true, identificationFailed: false });
    for (const selector of ["[data-identify-qualification]", "[data-identify-details]", "[data-identify-model-details]"]) {
      const element = pane?.querySelector<HTMLElement>(selector); if (element) element.hidden = true;
    }
    const units = (payload.presentation as { units?: { input: string; output: string } } | undefined)?.units;
    const { tableWrap, explanation, details } = modelStructureEvidence(comparison, units);
    const actions = document.createElement("div"); actions.className = "workspace-actions";
    const action = document.createElement("button"); action.className = "primary"; action.type = "button";
    if (comparison.choice) {
      action.dataset.structureSave = ""; action.textContent = "Save comparison";
      action.addEventListener("click", () => post("control-studio.result.save"));
      status.textContent = `${comparison.choice === "first_order" ? "First-order model" : "Additional-lag model"} retained provisionally. Independent validation is still required.`;
    } else if (comparison.recommended === "neither") {
      action.dataset.structureSettings = ""; action.textContent = "Review settings"; action.addEventListener("click", settings);
      status.textContent = "Neither candidate meets the declared requirements. Review the bounds and retained evidence.";
    } else {
      action.dataset.structureChoice = comparison.recommended;
      action.textContent = comparison.recommended === "first_order" ? "Keep first-order model" : "Keep additional-lag model";
      action.addEventListener("click", () => { action.disabled = true; post("control-studio.identify.choose-structure", { choice: comparison.recommended }); });
      status.textContent = comparison.recommended === "first_order" ? "First order meets the limits; the extra state is not justified."
        : "The additional lag meets the limits and improves the fit beyond measurement uncertainty. Its physical origin remains unresolved.";
    }
    actions.append(action);
    view.replaceChildren(actions, tableWrap, explanation, details);
    if (focusedChoice) action.focus();
    return true;
  } };
}
