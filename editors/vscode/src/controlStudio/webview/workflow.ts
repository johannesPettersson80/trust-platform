declare function acquireVsCodeApi(): { postMessage(message: unknown): void };

import {
  compactRevisionLabel,
  formatEngineeringNumber,
  pidEngineeringUnits,
  workflowResultStatusText,
} from "../chartContract";
import {
  validationRequirementRows,
  formatValidationRequirementValue,
  type ValidationRequirement,
  type ValidationRequirementRow,
} from "../validation";
import {
  experimentRestoringMessage,
  experimentTerminalMessage,
  modelKindLabel,
  staleCandidateNotice,
} from "../failureMessages";
import {
  experimentApprovalButtonClass,
  experimentFormMatchesReviewedPlan,
  type ExperimentFormSnapshot,
} from "../planPresentation";
import { formatLocalTimestamp } from "../timestampFormat";

type Mode = "Monitor" | "Test" | "Identify" | "Tune" | "Validate";

interface StudioIdentity {
  schemaVersion: 1;
  projectId: string;
  editorSessionId: string;
  plantId?: string;
  expectedPlantRevision?: string;
  loopId?: string;
  expectedLoopRevision?: string;
}

const root = document.querySelector<HTMLElement>("[data-control-studio]");
const buttons = [...document.querySelectorAll<HTMLButtonElement>("[data-control-studio-mode]")];
const panes = [...document.querySelectorAll<HTMLElement>("[data-workflow-pane]")];
const vscodeApi = root ? acquireVsCodeApi() : undefined;
let importedPidSettings: Record<string, unknown> | undefined;

function identity(): StudioIdentity | undefined {
  if (!root?.dataset.projectId || !root.dataset.editorSessionId) return undefined;
  const value: StudioIdentity = {
    schemaVersion: 1,
    projectId: root.dataset.projectId,
    editorSessionId: root.dataset.editorSessionId,
  };
  if (root.dataset.plantId && root.dataset.plantRevision) {
    value.plantId = root.dataset.plantId;
    value.expectedPlantRevision = root.dataset.plantRevision;
  }
  if (root.dataset.loopId && root.dataset.loopRevision) {
    value.loopId = root.dataset.loopId;
    value.expectedLoopRevision = root.dataset.loopRevision;
  }
  return value.plantId || value.loopId ? value : undefined;
}

function envelope(kind: string, payload: Record<string, unknown> = {}) {
  const current = identity();
  return current ? { ...current, kind, messageId: crypto.randomUUID(), payload } : undefined;
}

function post(kind: string, payload: Record<string, unknown> = {}): void {
  const message = envelope(kind, payload);
  if (message) vscodeApi?.postMessage(message);
}

function activate(button: HTMLButtonElement, focus: boolean): void {
  const mode = (button.textContent ?? "Monitor").trim() as Mode;
  for (const candidate of buttons) {
    const selected = candidate === button;
    candidate.setAttribute("aria-selected", String(selected));
    candidate.tabIndex = selected ? 0 : -1;
  }
  for (const pane of panes) pane.hidden = pane.dataset.workflowPane !== mode;
  window.dispatchEvent(new CustomEvent("trust-control-studio-stage", { detail: { stage: mode } }));
  if (focus) button.focus();
}

for (const button of buttons) {
  button.addEventListener("click", () => activate(button, false));
  button.addEventListener("keydown", (event) => {
    let index = buttons.indexOf(button);
    if (event.key === "ArrowRight") index = (index + 1) % buttons.length;
    else if (event.key === "ArrowLeft") index = (index - 1 + buttons.length) % buttons.length;
    else if (event.key === "Home") index = 0;
    else if (event.key === "End") index = buttons.length - 1;
    else return;
    event.preventDefault();
    activate(buttons[index], true);
  });
}

// A restored analysis session owns the active workflow stage as well as its
// plot state. Route restoration through the same activation path as a click so
// the stage tab, pane, and chart workspace cannot disagree.
window.addEventListener("trust-control-studio-restore-stage", (event) => {
  const stage = (event as CustomEvent<{ stage?: Mode }>).detail?.stage;
  const button = buttons.find((candidate) => (candidate.textContent ?? "").trim() === stage);
  if (button) activate(button, false);
});

document.querySelector("[data-control-studio-configure-plant]")?.addEventListener("click", () => post("control-studio.navigate-plant"));
const recordStart = document.querySelector<HTMLButtonElement>("[data-control-studio-record-start]");
const recordStop = document.querySelector<HTMLButtonElement>("[data-control-studio-record-stop]");
function setWorkflowActionAvailability(update: {
  recording?: boolean; model?: boolean; candidate?: boolean; result?: boolean;
}): void {
  if (update.recording !== undefined) {
    const button = document.querySelector<HTMLButtonElement>("[data-identify-run]");
    if (button) button.disabled = !update.recording;
  }
  if (update.model !== undefined) {
    const button = document.querySelector<HTMLButtonElement>("[data-tune-generate]");
    if (button) button.disabled = !update.model;
  }
  if (update.candidate !== undefined) {
    const button = document.querySelector<HTMLButtonElement>("[data-validate-run]");
    if (button) button.disabled = !update.candidate;
  }
  if (update.result !== undefined) {
    for (const selector of ["[data-result-save]", "[data-result-export-csv]", "[data-result-export-json]"]) {
      const button = document.querySelector<HTMLButtonElement>(selector);
      if (button) button.disabled = !update.result;
    }
  }
}
recordStart?.addEventListener("click", () => {
  post("control-studio.recording.start");
  recordStart.disabled = true;
  if (recordStop) recordStop.disabled = false;
});
recordStop?.addEventListener("click", () => {
  post("control-studio.recording.stop");
  recordStop.disabled = true;
  if (recordStart) recordStart.disabled = false;
  setWorkflowActionAvailability({ recording: true });
});
document.querySelector("[data-control-studio-automatic-tune]")?.addEventListener("click", () => {
  const plan = document.querySelector<HTMLElement>("[data-auto-tune-plan]");
  if (plan) plan.hidden = false;
});
document.querySelector("[data-auto-tune-close]")?.addEventListener("click", () => {
  const plan = document.querySelector<HTMLElement>("[data-auto-tune-plan]");
  if (plan) plan.hidden = true;
});
document.querySelector("[data-auto-tune-approve]")?.addEventListener("click", (event) => {
  const button = event.currentTarget as HTMLButtonElement;
  const objective = (document.querySelector<HTMLSelectElement>("[data-auto-tune-objective]")?.value ?? "balanced");
  button.disabled = true;
  button.textContent = "Running…";
  const previous = document.querySelector<HTMLElement>("[data-auto-tune-result]");
  if (previous) previous.hidden = true;
  showError();
  post("control-studio.automatic.approve", { objective });
});

document.querySelector("[data-identify-run]")?.addEventListener("click", () => {
  const start = Number(document.querySelector<HTMLInputElement>("#identify-exclude-start")?.value);
  const end = Number(document.querySelector<HTMLInputElement>("#identify-exclude-end")?.value);
  const excludedIntervals = Number.isFinite(start) && Number.isFinite(end) && end >= start && end > 0
    ? [{ startSeconds: start, endSeconds: end }] : [];
  post("control-studio.identify.run", {
    model: document.querySelector<HTMLSelectElement>("#identify-model")?.value ?? "automatic",
    ...(excludedIntervals.length > 0 ? { excludedIntervals } : {}),
  });
  setText("[data-identify-state]", "Qualifying the retained recording…");
  setHidden("[data-identify-fit]", true);
  setHidden("[data-identify-accept]", true);
});
document.querySelector("[data-identify-fit]")?.addEventListener("click", () => {
  post("control-studio.identify.fit");
  setText("[data-identify-state]", "Fitting and checking the selected low-order model…");
  setHidden("[data-identify-fit]", true);
});
document.querySelector("[data-identify-accept]")?.addEventListener("click", () => {
  post("control-studio.identify.accept");
  setText("[data-identify-state]", "Accepting the independently checked model…");
  setHidden("[data-identify-accept]", true);
});
const tuneMethod = document.querySelector<HTMLSelectElement>("#tune-method");
tuneMethod?.addEventListener("change", updateTuneMethod);
document.querySelector<HTMLInputElement>("#tune-import-file")?.addEventListener("change", async (event) => {
  const file = (event.currentTarget as HTMLInputElement).files?.[0];
  importedPidSettings = undefined;
  if (!file) { setText("[data-tune-import-state]", "No file selected"); return; }
  try {
    const parsed = JSON.parse(await file.text()) as unknown;
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) throw new Error("JSON must contain an object");
    const record = parsed as Record<string, unknown>;
    importedPidSettings = record.parameters && typeof record.parameters === "object" && !Array.isArray(record.parameters)
      ? record.parameters as Record<string, unknown> : record;
    setText("[data-tune-import-state]", `${file.name} ready; settings will be validated without conversion.`);
  } catch (error) {
    setText("[data-tune-import-state]", `Invalid JSON: ${error instanceof Error ? error.message : String(error)}`);
  }
});
document.querySelector("[data-tune-generate]")?.addEventListener("click", () => {
  const method = tuneMethod?.value ?? "SIMC";
  if (method === "Manual") post("control-studio.tune.run", { method, settings: readManualPidSettings() });
  else if (method === "Imported") {
    if (!importedPidSettings) { setText("[data-tune-state]", "Select a Control.PID JSON file first."); return; }
    post("control-studio.tune.run", { method, settings: importedPidSettings });
  } else post("control-studio.tune.run", { method,
    objective: document.querySelector<HTMLSelectElement>("#tune-objective")?.value ?? "balanced" });
  setText("[data-tune-state]", "Generating and checking the controller candidate…");
});
window.addEventListener("trust-control-studio-design-edit", (event) => {
  const settings = (event as CustomEvent<{ settings?: Record<string, unknown> }>).detail?.settings;
  if (!settings) return;
  post("control-studio.tune.run", { method: "Manual", settings });
  setText("[data-tune-state]", "Updating the unapplied candidate and linked analysis…");
});
document.querySelector("[data-validate-run]")?.addEventListener("click", () => {
  post("control-studio.validate.run"); setText("[data-validate-state]", "Comparing current and candidate controllers…");
});
// The plan card must always describe exactly the form that was reviewed (W-17):
// the form is frozen the moment a plan is pending, and the reviewed snapshot is
// re-checked immediately before Approve is allowed to fire.
const experimentFormFields = ["#test-kind", "#test-amplitude", "#test-duration", "[data-physical-topology-reviewed]"]
  .map((selector) => document.querySelector<HTMLInputElement | HTMLSelectElement>(selector))
  .filter((field): field is HTMLInputElement | HTMLSelectElement => field !== null);
const experimentFormFieldsInitiallyDisabled = new Map(experimentFormFields.map((field) => [field, field.disabled]));
// The snapshot submitted with the last "Review test" click; promoted to
// reviewedExperimentForm only once the runtime actually accepts that exact
// submission, so a rejected re-review can never leave a stale plan approvable.
let lastSubmittedExperimentForm: ExperimentFormSnapshot | undefined;
let reviewedExperimentForm: ExperimentFormSnapshot | undefined;

function currentExperimentFormSnapshot(): ExperimentFormSnapshot {
  return {
    kind: document.querySelector<HTMLSelectElement>("#test-kind")?.value ?? "setpoint_step",
    amplitude: Number(document.querySelector<HTMLInputElement>("#test-amplitude")?.value),
    durationSeconds: Number(document.querySelector<HTMLInputElement>("#test-duration")?.value),
  };
}

function setExperimentFormLocked(locked: boolean): void {
  for (const field of experimentFormFields) {
    field.disabled = locked ? true : (experimentFormFieldsInitiallyDisabled.get(field) ?? false);
  }
}

document.querySelector("[data-test-review]")?.addEventListener("click", () => {
  const form = currentExperimentFormSnapshot();
  const physicalTopologyReviewed = document.querySelector<HTMLInputElement>(
    "[data-physical-topology-reviewed]"
  )?.checked === true;
  lastSubmittedExperimentForm = form;
  post("control-studio.experiment.review", { ...form, physicalTopologyReviewed });
  setText("[data-test-state]", "Checking current runtime evidence and safety limits…");
});
document.querySelector("[data-test-approve]")?.addEventListener("click", () => {
  const planId = root?.dataset.experimentPlanId;
  if (!planId) return;
  if (!experimentFormMatchesReviewedPlan(currentExperimentFormSnapshot(), reviewedExperimentForm)) {
    setText("[data-test-state]", "The form changed after review. Review the test again before approving.");
    return;
  }
  post("control-studio.experiment.approve", {
    planId,
    physicalApproved: root?.dataset.experimentSource === "physical",
  });
  setText("[data-test-state]", "Starting the reviewed bounded test…");
});
document.querySelector("[data-test-abort]")?.addEventListener("click", () => {
  const experimentId = root?.dataset.experimentId;
  if (!experimentId) return;
  post("control-studio.experiment.abort", { experimentId });
  setText("[data-test-state]", "Abort requested; waiting for runtime restoration…");
});
for (const button of document.querySelectorAll("[data-auto-tune-apply], [data-candidate-review]")) {
  button.addEventListener("click", () => {
    const review = document.querySelector<HTMLElement>("[data-shared-application-review]");
    if (review) review.hidden = false;
  });
}
document.querySelector("[data-shared-application-confirm]")?.addEventListener("click", () => {
  post("control-studio.candidate.apply", { physicalApproved: root?.dataset.controlSource === "physical" });
  setText("[data-shared-application-state]", "Applying the reviewed candidate through the runtime…");
});
document.querySelector("[data-shared-application-rollback]")?.addEventListener("click", () => {
  post("control-studio.candidate.rollback");
  setText("[data-shared-application-state]", "Rollback requested…");
});
document.querySelector("[data-result-save]")?.addEventListener("click", () => {
  let session: unknown;
  try { session = JSON.parse(root?.dataset.controlChartSession ?? "null"); } catch { session = undefined; }
  post("control-studio.result.save", session ? { session } : {});
});
document.querySelector("[data-result-export-csv]")?.addEventListener("click", () => post("control-studio.result.export-csv"));
document.querySelector("[data-result-export-json]")?.addEventListener("click", () => post("control-studio.result.export-json"));
const resultHistory = document.querySelector<HTMLSelectElement>("[data-result-history]");
const resultComparison = document.querySelector<HTMLSelectElement>("[data-result-compare]");
const resultOpen = document.querySelector<HTMLButtonElement>("[data-result-open]");
const resultCompare = document.querySelector<HTMLButtonElement>("[data-result-compare-run]");
function updateResultActions(): void {
  if (resultOpen) resultOpen.disabled = !resultHistory?.value;
  if (resultCompare) resultCompare.disabled = !resultHistory?.value || !resultComparison?.value ||
    resultHistory.value === resultComparison.value;
}
resultHistory?.addEventListener("change", updateResultActions);
resultComparison?.addEventListener("change", updateResultActions);
resultOpen?.addEventListener("click", () => {
  const resultRevision = Number(resultHistory?.value);
  if (Number.isInteger(resultRevision) && resultRevision > 0) {
    post("control-studio.result.open", { resultRevision });
  }
});
resultCompare?.addEventListener("click", () => {
  const leftRevision = Number(resultHistory?.value);
  const rightRevision = Number(resultComparison?.value);
  if (Number.isInteger(leftRevision) && leftRevision > 0 && Number.isInteger(rightRevision) &&
      rightRevision > 0 && leftRevision !== rightRevision) {
    post("control-studio.result.compare", { leftRevision, rightRevision });
  }
});
document.querySelector("[data-auto-tune-export]")?.addEventListener("click", () => {
  if (identity()?.loopId) { post("control-studio.result.export-json"); return; }
  const content = {
    structure: document.querySelector("[data-auto-tune-structure]")?.textContent,
    method: document.querySelector("[data-auto-tune-method]")?.textContent,
    kp: document.querySelector("[data-auto-tune-kp]")?.textContent,
    ti: document.querySelector("[data-auto-tune-ti]")?.textContent,
    evidence: document.querySelector("[data-auto-tune-validation]")?.textContent,
  };
  const anchor = document.createElement("a");
  anchor.href = `data:application/json;charset=utf-8,${encodeURIComponent(JSON.stringify(content, null, 2))}`;
  anchor.download = "trust-control-candidate.json";
  anchor.click();
});

window.addEventListener("message", (event: MessageEvent) => {
  const message = event.data as Record<string, unknown> | undefined;
  const current = identity();
  if (!message || !current || message.projectId !== current.projectId ||
      message.editorSessionId !== current.editorSessionId) return;
  if (message.kind === "control-studio.experiment.result" && current.loopId &&
      message.loopId === current.loopId && message.expectedLoopRevision === current.expectedLoopRevision) {
    renderExperimentResult(message);
    return;
  }
  if (message.kind === "control-studio.application.result" && current.loopId &&
      message.loopId === current.loopId && message.expectedLoopRevision === current.expectedLoopRevision) {
    const payload = message.payload as Record<string, unknown> | undefined;
    const phase = String(message.phase ?? "failed");
    const activeRevision = compactRevisionLabel(String(payload?.activeRevision ??
      (phase === "applied" ? "new revision" : "previous revision")));
    const observationSeconds = Number(payload?.observationSeconds);
    const observationState = Number.isFinite(observationSeconds) && observationSeconds > 0
      ? `${formatEngineeringNumber(observationSeconds, 3)} s bounded observation active`
      : "observation duration unavailable";
    const sharedRollback = document.querySelector<HTMLButtonElement>("[data-shared-application-rollback]");
    if (sharedRollback) sharedRollback.hidden = phase !== "applied";
    setText("[data-shared-application-state]", phase === "failed" ? String(payload?.reason ?? "Application failed") :
      phase === "applied" ? `Applied ${activeRevision} · ${observationState}` :
      `Rolled back to ${activeRevision}`);
    setText("[data-control-studio-candidate-revision]", phase === "applied" ? "Applied" : "Unapplied");
    return;
  }
  if (message.kind === "control-studio.workflow-result" && current.loopId &&
      message.loopId === current.loopId && message.expectedLoopRevision === current.expectedLoopRevision) {
    const payload = message.payload as Record<string, unknown> | undefined;
    const phase = String(message.phase ?? "failed");
    // W-16: an opened/restored saved result is historical evidence, not a live
    // reading. The flag lives on the shared studio root so chart.ts (which owns
    // the live-value strip) can badge and dim it; it is cleared for every other
    // phase, since those do not change what is currently on screen.
    if (root) {
      if (phase === "restored" || phase === "opened") root.dataset.controlStudioHistoricalResult = "true";
      else delete root.dataset.controlStudioHistoricalResult;
    }
    if (Array.isArray(payload?.history)) renderResultHistory(payload.history as Record<string, unknown>[]);
    if ((phase === "restored" || phase === "opened") && payload?.result && typeof payload.result === "object") {
      const result = payload.result as Record<string, unknown>;
      const candidate = result.candidate as Record<string, unknown> | undefined;
      if (candidate) { renderApplicationCandidate(candidate); setWorkflowActionAvailability({ candidate: true }); }
      const model = result.model as Record<string, unknown> | undefined;
      if (model) { setText("[data-identify-state]", `Restored ${modelKindLabel(String(model.kind))} · gain ${formatEngineeringNumber(model.gain, 5)} · normalized error ${formatEngineeringNumber(model.normalizedError, 4)}`); setWorkflowActionAvailability({ model: true }); }
      const validation = result.validation as Record<string, unknown> | undefined;
      if (validation && Array.isArray(validation.requirements)) {
        setWorkflowActionAvailability({ result: true });
        const requirements = validationRequirementRows(validation.requirements as ValidationRequirement[]);
        renderValidationRequirements(requirements);
        setText("[data-validate-state]", `${String(result.conclusion ?? "Saved validation restored.")} ${requirements.map((item) => `${item.name}: ${item.status}`).join(" · ")}`);
        setText("[data-application-validation]", requirements.some((item) => item.status === "Fail" || item.status === "Inconclusive") ? "Requirements not met" : "Required validation passed");
      }
      const tuningComparison = result.tuningComparison as Record<string, unknown> | undefined;
      if (tuningComparison && Array.isArray(tuningComparison.attempts)) {
        renderAutomaticTuneAttempts(tuningComparison.attempts, "[data-saved-tuning-attempts]");
        const disclosure = document.querySelector<HTMLElement>("[data-saved-tuning-comparison]");
        if (disclosure) disclosure.hidden = false;
      }
      const openedAt = formatLocalTimestamp(String(result.createdAt ?? ""));
      if (root) root.dataset.controlStudioHistoricalResultAt = openedAt.display;
      setText("[data-result-state]",
        `${phase === "opened" ? "Opened" : "Restored"} · Saved result from ${openedAt.display} · revision ${String(result.resultRevision ?? "—")} · ${String(result.source ?? "Unknown source")}.`,
        openedAt.title);
    } else if (phase === "saved") {
      const savedAt = formatLocalTimestamp(String(payload?.createdAt ?? ""));
      setText("[data-result-state]", `Saved revision ${String(payload?.resultRevision ?? "—")} at ${savedAt.display}.`, savedAt.title);
    } else if (phase === "compared" && payload?.comparison && typeof payload.comparison === "object") {
      const comparison = payload.comparison as Record<string, unknown>;
      const left = comparison.left as Record<string, unknown>;
      const right = comparison.right as Record<string, unknown>;
      renderResultComparison(left, right);
    } else if (phase === "exported") {
      renderExportResult(String(payload?.format ?? "Result"), String(payload?.path ?? ""));
    } else {
      const status = workflowResultStatusText(phase, payload?.reason);
      if (status) setText("[data-result-state]", status);
    }
    return;
  }
  if (message.kind === "control-studio.engineering.result" && current.loopId &&
      message.loopId === current.loopId && message.expectedLoopRevision === current.expectedLoopRevision) {
    const payload = message.payload as Record<string, unknown> | undefined;
    const stage = String(message.stage ?? "");
    const selector = stage === "identify" ? "[data-identify-state]" : stage === "tune" ? "[data-tune-state]" : "[data-validate-state]";
    if (stage === "tune" && payload?.methodEvidence && typeof payload.methodEvidence === "object") {
      renderTuningMethodEvidence(payload.methodEvidence as Record<string, unknown>);
    }
    if (payload?.status !== "complete") {
      const reason = String(payload?.reason ?? `${stage} failed`);
      setText(selector, reason);
      if (stage === "tune") markSharedApplicationCandidateStale(reason);
    }
    else if (stage === "identify") {
      const identifyPhase = String(payload.phase ?? "");
      const estimationQualification = payload.estimationQualification as Record<string, unknown> | undefined;
      const qualification = (payload.validationQualification as Record<string, unknown> | undefined) ??
        (payload.qualification as Record<string, unknown> | undefined) ?? estimationQualification;
      if (qualification) renderIdentificationQualification(qualification);
      if (identifyPhase === "qualified") {
        const nextFitRole = String(payload.nextFitRole ?? "estimation");
        const fit = document.querySelector<HTMLButtonElement>("[data-identify-fit]");
        if (fit) {
          fit.textContent = nextFitRole === "validation" ? "Fit validation run" : "Fit estimation run";
          fit.hidden = false;
        }
        setText(selector, nextFitRole === "validation"
          ? "Validation recording qualified. Fit it against the retained estimation run."
          : "Estimation recording qualified. Fit a preview, then record a distinct validation run.");
      } else {
        const model = payload.model as Record<string, unknown>;
        renderIdentificationModel(model, payload.fit as Record<string, unknown> | undefined);
        renderIdentificationPresentation(payload.presentation as Record<string, unknown> | undefined);
        if (identifyPhase === "validation_required") {
          setText(selector, `Preview only · ${modelKindLabel(String(model.kind))} · normalized estimation error ${formatEngineeringNumber(model.normalizedError, 4)}. Record and qualify a distinct validation run.`);
        } else if (identifyPhase === "acceptance_required") {
          setText(selector, `${String(payload.source)} · ${modelKindLabel(String(model.kind))} · normalized validation error ${formatEngineeringNumber(model.normalizedValidationError, 4)}. Review the evidence before acceptance.`);
          setHidden("[data-identify-accept]", false);
        } else if (identifyPhase === "accepted") {
          setText(selector, `Accepted for design · ${String(payload.source)} · ${modelKindLabel(String(model.kind))} · normalized validation error ${formatEngineeringNumber(model.normalizedValidationError, 4)}`);
          markModeComplete("Identify");
          setWorkflowActionAvailability({ model: true });
        }
      }
    } else if (stage === "tune") {
      const candidate = payload.candidate as Record<string, unknown>;
      const settings = candidate.parameters as Record<string, unknown>;
      const plots = payload.plots as Record<string, unknown>;
      setText(selector, `Unapplied ${String(candidate.structure)} · Kp ${formatEngineeringNumber(settings.proportionalGain, 5)} · Ki ${formatEngineeringNumber(settings.integralGain, 5)} · Kd ${formatEngineeringNumber(settings.derivativeGain, 5)} · margins ${formatEngineeringNumber(plots.gainMarginDb, 2)} dB / ${formatEngineeringNumber(plots.phaseMarginDegrees, 1)}° · ${String(payload.recommendation)}`);
      renderApplicationCandidate(candidate);
      setText("[data-application-validation]", "Not validated");
      markModeComplete("Tune");
      setWorkflowActionAvailability({ candidate: true });
    } else {
      const result = payload.result as Record<string, unknown>;
      const requirements = validationRequirementRows(result.requirements as ValidationRequirement[]);
      setText(selector, requirements.map((item) => `${item.name}: ${item.status}`).join(" · "));
      renderValidationRequirements(requirements);
      const acceptable = payload.acceptable === true;
      const review = document.querySelector<HTMLButtonElement>("[data-candidate-review]");
      if (review) review.hidden = !acceptable;
      setText("[data-application-validation]", acceptable ? "Required validation passed" : "Requirements not met");
      if (acceptable) markModeComplete("Validate");
      setWorkflowActionAvailability({ result: true });
    }
    return;
  }
  if (message.kind !== "control-studio.automatic.result") return;
  const correctAutomaticScope = current.loopId
    ? message.loopId === current.loopId && message.expectedLoopRevision === current.expectedLoopRevision
    : message.plantId === current.plantId && message.expectedPlantRevision === current.expectedPlantRevision;
  if (!correctAutomaticScope) return;
  const payload = message.payload as Record<string, unknown> | undefined;
  const approve = document.querySelector<HTMLButtonElement>("[data-auto-tune-approve]");
  if (approve) { approve.disabled = false; approve.textContent = "Run again"; }
  if (payload?.status !== "complete" || !payload.result || typeof payload.result !== "object") {
    const candidate = document.querySelector<HTMLElement>("[data-auto-tune-result]");
    if (candidate) candidate.hidden = true;
    const completed = Array.isArray(payload?.completedStages) ? payload.completedStages.map(String) : [];
    const failed = String(payload?.failedStage ?? "");
    for (const stage of document.querySelectorAll<HTMLElement>("[data-auto-tune-stage]")) {
      const name = String(stage.dataset.autoTuneStage ?? "");
      stage.dataset.status = completed.includes(name) ? "complete" : name === failed ? "failed" : "waiting";
      stage.textContent = `${name} · ${completed.includes(name) ? "Complete" : name === failed ? "Failed" : "Waiting"}`;
    }
    showError(`${String(payload?.error ?? "Automatic Tune failed.")} ${String(payload?.recovery ?? "")}`.trim());
    markSharedApplicationCandidateStale(String(payload?.error ?? "Automatic Tune failed."));
    return;
  }
  const result = payload.result as Record<string, unknown>;
  for (const stage of document.querySelectorAll<HTMLElement>("[data-auto-tune-stage]")) {
    stage.dataset.status = "complete";
    stage.textContent = `${stage.dataset.autoTuneStage} · Complete`;
  }
  setText("[data-auto-tune-structure]", result.structure);
  setText("[data-auto-tune-method]", result.method);
  const units = controllerUnits();
  setText("[data-auto-tune-kp]", `${formatEngineeringNumber(result.kp, 4)} ${units.proportional}`);
  setText("[data-auto-tune-ti]", `${formatEngineeringNumber(result.tiSeconds, 3)} s`);
  setText("[data-auto-tune-validation]", result.validation);
  const frequency = result.frequency as Record<string, unknown> | undefined;
  setText("[data-auto-tune-gain-margin]", `${formatEngineeringNumber(frequency?.gainMarginDb, 2)} dB`);
  setText("[data-auto-tune-phase-margin]", `${formatEngineeringNumber(frequency?.phaseMarginDegrees, 1)}°`);
  const model = result.model as Record<string, unknown> | undefined;
  const processUnit = root?.dataset.processUnit?.trim() || "normalized process";
  const outputUnit = root?.dataset.outputUnit?.trim() || "normalized output";
  setText("[data-auto-tune-model]", model
    ? `${String(result.modelEvidence)} · K ${formatEngineeringNumber(model.gain, 4)} ${processUnit}/${outputUnit}` +
      ` · T ${formatEngineeringNumber(model.timeConstantSeconds, 3)} s` +
      ` · L ${formatEngineeringNumber(model.deadTimeSeconds, 3)} s`
    : result.modelEvidence);
  setText("[data-auto-tune-rationale]", result.rationale);
  renderAutomaticTuneAttempts(result.attempts);
  if (result.parameters && typeof result.parameters === "object") {
    renderApplicationCandidate({ source: result.method, structure: result.structure, parameters: result.parameters });
  }
  setText("[data-application-validation]", result.validation);
  setWorkflowActionAvailability({ model: true });
  setWorkflowActionAvailability({ candidate: true });
  setWorkflowActionAvailability({ result: true });
  const candidate = document.querySelector<HTMLElement>("[data-auto-tune-result]");
  if (candidate) candidate.hidden = false;
  for (const button of buttons) {
    if ((button.textContent ?? "").trim() !== "Monitor") {
      button.dataset.status = "complete";
      button.setAttribute("aria-label", `${button.textContent?.trim()}, complete`);
    }
  }
  showError();
});

function renderIdentificationQualification(value: Record<string, unknown>): void {
  const metrics = value.metrics as Record<string, unknown> | undefined;
  if (!metrics) return;
  setHidden("[data-identify-qualification]", false);
  setHidden("[data-identify-details]", false);
  setText("[data-identify-samples]", `${String(metrics.retainedSampleCount)} / ${String(metrics.rawSampleCount)}`);
  setText("[data-identify-duration]", `${formatEngineeringNumber(metrics.durationSeconds, 3)} s`);
  setText("[data-identify-interval]", `${formatEngineeringNumber(metrics.medianSampleTimeSeconds, 4)} s`);
  setText("[data-identify-snr]", `${formatEngineeringNumber(metrics.signalToNoiseDb, 2)} dB`);
  replaceEvidenceRows("[data-identify-detail-rows]", [
    ["Input / output units", `${String(value.inputUnit)} / ${String(value.outputUnit)}`],
    ["Maximum interval deviation", `${formatEngineeringNumber(metrics.maximumSampleTimeDeviationPercent, 2)} %`],
    ["Timing gaps", String(metrics.gapCount)],
    ["Rejected quality samples", String(metrics.rejectedQualitySampleCount)],
    ["Clipped / saturated", `${String(metrics.clippedSampleCount)} / ${String(metrics.saturationSampleCount)}`],
    ["Baseline drift", `${formatEngineeringNumber(metrics.baselineDriftPerSecond, 5)} ${String(value.outputUnit)}/s`],
    ["Input / output span", `${formatEngineeringNumber(metrics.inputSpan, 5)} ${String(value.inputUnit)} / ${formatEngineeringNumber(metrics.outputSpan, 5)} ${String(value.outputUnit)}`],
    ["Excitation", String(metrics.excitation)],
    ["Coherence", String(metrics.coherence)],
    ["Independent validation", metrics.independentValidationAvailable === true ? "Available" : "Not yet available"],
  ]);
}

function renderIdentificationModel(model: Record<string, unknown>, fit?: Record<string, unknown>): void {
  setHidden("[data-identify-model-details]", false);
  const range = fit?.operatingRange as Record<string, unknown> | undefined;
  const warnings = Array.isArray(fit?.warnings) ? fit.warnings.map(String).join("; ") : "None";
  replaceEvidenceRows("[data-identify-model-rows]", [
    ["Structure", modelKindLabel(String(model.kind))],
    ["Gain", formatEngineeringNumber(model.gain, 6)],
    ["Time constants", [model.timeConstantSeconds, model.secondTimeConstantSeconds]
      .filter((item) => typeof item === "number").map((item) => `${formatEngineeringNumber(item, 5)} s`).join(" / ") || "None"],
    ["Dead time", `${formatEngineeringNumber(model.deadTimeSeconds ?? 0, 5)} s`],
    ["Normalized estimation error", formatEngineeringNumber(model.normalizedEstimationError ?? model.normalizedError, 5)],
    ["Normalized validation error", typeof model.normalizedValidationError === "number"
      ? formatEngineeringNumber(model.normalizedValidationError, 5) : "Not evaluated"],
    ["Validation RMSE", fit ? `${formatEngineeringNumber(fit.rootMeanSquareError, 5)} ${String(fit.outputUnit)}` : "Not evaluated"],
    ["Operating range", range
      ? `MV ${formatEngineeringNumber(range.inputMinimum, 5)}…${formatEngineeringNumber(range.inputMaximum, 5)}; PV ${formatEngineeringNumber(range.outputMinimum, 5)}…${formatEngineeringNumber(range.outputMaximum, 5)}`
      : "Not evaluated"],
    ["Stability", model.stable === true ? "Stable" : "Not stable"],
    ["Warnings", warnings || "None"],
  ]);
}

function renderIdentificationPresentation(presentation?: Record<string, unknown>): void {
  if (!presentation) return;
  const units = presentation.units as Record<string, unknown> | undefined;
  const estimation = presentation.estimation as Record<string, unknown> | undefined;
  const validation = presentation.validation as Record<string, unknown> | undefined;
  const uncertainty = presentation.uncertainty as Record<string, unknown> | undefined;
  const range = presentation.validityRange as Record<string, unknown> | undefined;
  const warnings = Array.isArray(presentation.warnings) ? presentation.warnings.map(String).join("; ") : "None";
  const poles = Array.isArray(presentation.poles)
    ? presentation.poles.map((value) => formatEngineeringNumber(value, 6)).join(", ") : "Not evaluated";
  const zeros = Array.isArray(presentation.zeros) && presentation.zeros.length > 0
    ? presentation.zeros.map((value) => formatEngineeringNumber(value, 6)).join(", ") : "None";
  const rows: Array<[string, string]> = [
    ["Equation", String(presentation.equation)],
    ["Input / output", `${String(units?.input)} / ${String(units?.output)}`],
    ["Poles", poles || "None"],
    ["Zeros", zeros],
    ["Delay", `${formatEngineeringNumber(presentation.delaySeconds, 5)} s`],
    ["Uncertainty", String(uncertainty?.label ?? "Not evaluated")],
    ["Estimation residual correlation (lag 1)", typeof estimation?.residualLagOneCorrelation === "number"
      ? formatEngineeringNumber(estimation.residualLagOneCorrelation, 4) : "Not evaluated"],
    ["Validation residual correlation (lag 1)", typeof validation?.residualLagOneCorrelation === "number"
      ? formatEngineeringNumber(validation.residualLagOneCorrelation, 4) : "Not evaluated"],
    ["Validity range", range
      ? `MV ${formatEngineeringNumber(range.inputMinimum, 5)}…${formatEngineeringNumber(range.inputMaximum, 5)}; PV ${formatEngineeringNumber(range.outputMinimum, 5)}…${formatEngineeringNumber(range.outputMaximum, 5)}`
      : "Not evaluated"],
    ["Warnings", warnings || "None"],
  ];
  replaceEvidenceRows("[data-identify-model-rows]", rows);
}

function renderTuningMethodEvidence(evidence: Record<string, unknown>): void {
  const assumptions = Array.isArray(evidence.assumptions) ? evidence.assumptions.map(String).join(" ") : "Not declared";
  const equations = Array.isArray(evidence.equations) ? evidence.equations.map(String).join("; ") : "Not declared";
  const inputs = Array.isArray(evidence.inputs) ? evidence.inputs.filter((item): item is Record<string, unknown> =>
    Boolean(item) && typeof item === "object" && !Array.isArray(item)).map((input) =>
    `${String(input.name)} = ${typeof input.value === "number" ? formatEngineeringNumber(input.value, 6) : String(input.value)} ${String(input.unit)}`)
    .join("; ") : "Not declared";
  const result = evidence.result as Record<string, unknown> | undefined;
  const units = evidence.units as Record<string, unknown> | undefined;
  replaceEvidenceRows("[data-tune-method-rows]", [
    ["Design method", String(evidence.title)],
    ["Status", String(evidence.status)],
    ["Convention", String(evidence.convention)],
    ["Assumptions", assumptions],
    ["Design inputs", inputs],
    ["Equations", equations],
    ["Result", result
      ? `Kp ${formatEngineeringNumber(result.proportionalGain, 6)} ${String(units?.proportionalGain)}; ` +
        `Ki ${formatEngineeringNumber(result.integralGain, 6)} ${String(units?.integralGain)}; ` +
        `Kd ${formatEngineeringNumber(result.derivativeGain, 6)} ${String(units?.derivativeGain)}`
      : "No candidate produced"],
    ["Unavailable reason", String(evidence.reason ?? "Not applicable")],
  ]);
  setHidden("[data-tune-method-evidence]", false);
}

function replaceEvidenceRows(selector: string, rows: readonly (readonly [string, string])[]): void {
  const host = document.querySelector<HTMLElement>(selector);
  if (!host) return;
  host.replaceChildren(...rows.map(([label, value]) => {
    const row = document.createElement("tr");
    const heading = document.createElement("th");
    heading.scope = "row";
    heading.textContent = label;
    const cell = document.createElement("td");
    cell.textContent = value;
    row.append(heading, cell);
    return row;
  }));
}

function renderAutomaticTuneAttempts(value: unknown, selector = "[data-auto-tune-attempts]"): void {
  const host = document.querySelector<HTMLElement>(selector);
  if (!host) return;
  host.replaceChildren();
  const attempts = Array.isArray(value) ? value.filter((item): item is Record<string, unknown> =>
    item !== null && typeof item === "object" && !Array.isArray(item)) : [];
  if (attempts.length === 0) {
    host.textContent = "No comparison result is available.";
    return;
  }
  const table = document.createElement("table");
  table.innerHTML = "<caption>Qualified tuning candidates</caption><thead><tr><th>Method</th><th>Structure</th><th>Status</th><th>Score</th><th>Lambda</th><th>Reason</th></tr></thead>";
  const body = document.createElement("tbody");
  for (const attempt of attempts) {
    const row = document.createElement("tr");
    for (const value of [attempt.method, attempt.structure ?? "—", attempt.status,
      attempt.score === undefined ? "—" : formatEngineeringNumber(attempt.score, 4),
      attempt.closedLoopTimeSeconds === undefined ? "—" :
        `${formatEngineeringNumber(attempt.closedLoopTimeSeconds, 4)} s`, attempt.reason ?? "—"]) {
      const cell = document.createElement("td");
      cell.textContent = String(value);
      row.append(cell);
    }
    body.append(row);
  }
  table.append(body);
  host.append(table);
}

function renderResultHistory(items: readonly Record<string, unknown>[]): void {
  for (const select of [resultHistory, resultComparison]) {
    if (!select) continue;
    const selected = select.value;
    select.replaceChildren();
    const placeholder = document.createElement("option");
    placeholder.value = "";
    placeholder.textContent = items.length > 0 ? "Select revision" : "No saved results";
    select.append(placeholder);
    for (const item of items) {
      const option = document.createElement("option");
      option.value = String(item.resultRevision);
      const createdAt = formatLocalTimestamp(String(item.createdAt));
      option.textContent = `Revision ${String(item.resultRevision)} · ${String(item.source)} · ${String(item.method)} · ${createdAt.display}`;
      option.title = createdAt.title;
      select.append(option);
    }
    if ([...select.options].some((option) => option.value === selected)) select.value = selected;
  }
  updateResultActions();
}

function renderExperimentResult(message: Record<string, unknown>): void {
  const phase = String(message.phase ?? "failed");
  const payload = message.payload as Record<string, unknown> | undefined;
  const approve = document.querySelector<HTMLButtonElement>("[data-test-approve]");
  const abort = document.querySelector<HTMLButtonElement>("[data-test-abort]");
  const review = document.querySelector<HTMLButtonElement>("[data-test-review]");
  const preview = document.querySelector<HTMLElement>("[data-test-preview]");
  if (phase === "review" && payload?.kind === "accepted" && payload.plan && typeof payload.plan === "object") {
    const plan = payload.plan as Record<string, unknown>;
    if (root) {
      root.dataset.experimentPlanId = String(plan.id ?? "");
      root.dataset.experimentSource = String(plan.source ?? "");
    }
    setText("[data-test-signal]", `${plan.affectedSignal} · ${formatExperimentValue(plan.from)} → ${formatExperimentValue(plan.to)} ${plan.unit}`);
    setText("[data-test-bound]", `${formatEngineeringNumber(plan.minimum)}…${formatEngineeringNumber(plan.maximum)} ${plan.unit}`);
    setText("[data-test-timeout]", `${formatEngineeringNumber(Number(plan.durationMs) / 1_000)} s maximum`);
    setText("[data-test-restoration]", `${formatExperimentValue(plan.restorationValue)} ${plan.unit}`);
    setText("[data-test-stops]", `Stops on: ${(plan.stopConditions as unknown[]).join(", ")}`);
    setText("[data-test-state]", plan.source === "physical"
      ? "Physical test ready. Confirming will authorize this exact plan once."
      : "Simulation test ready. Confirm this exact bounded plan.");
    if (preview) preview.hidden = false;
    if (approve) {
      approve.hidden = false;
      const treatment = experimentApprovalButtonClass(plan.source === "physical" ? "physical" : "simulation");
      approve.classList.toggle("danger", treatment === "danger");
      approve.classList.toggle("primary", treatment === "primary");
    }
    if (review) review.disabled = false;
    reviewedExperimentForm = lastSubmittedExperimentForm;
    setExperimentFormLocked(true);
    return;
  }
  if (phase === "review") {
    setText("[data-test-state]", String(payload?.reason ?? "The test cannot start."));
    if (approve) approve.hidden = true;
    if (review) review.disabled = false;
    if (root) delete root.dataset.experimentPlanId;
    reviewedExperimentForm = undefined;
    setExperimentFormLocked(false);
    return;
  }
  if (phase === "running") {
    const experimentId = String(payload?.experiment_id ?? root?.dataset.experimentPlanId ?? "");
    if (root) root.dataset.experimentId = experimentId;
    setText("[data-test-state]", "Test running. Live values and limits remain visible above; Abort is available.");
    if (approve) approve.hidden = true;
    if (abort) abort.disabled = false;
    if (review) review.disabled = true;
    return;
  }
  if (phase === "restoring") {
    setText("[data-test-state]", experimentRestoringMessage(
      String(payload?.status ?? "unknown"), String(payload?.stop_reason ?? "unknown")));
    if (abort) abort.disabled = true;
    if (review) review.disabled = true;
    return;
  }
  if (phase === "terminal") {
    const restorationReason = typeof payload?.restoration_reason === "string" && payload.restoration_reason.length > 0
      ? payload.restoration_reason : undefined;
    setText("[data-test-state]", experimentTerminalMessage({
      status: String(payload?.status ?? "unknown"),
      stopReason: String(payload?.stop_reason ?? "unknown"),
      restoration: String(payload?.restoration ?? "unknown"),
      ...(restorationReason ? { restorationReason } : {}),
    }));
    if (root) { delete root.dataset.experimentId; delete root.dataset.experimentPlanId; }
    reviewedExperimentForm = undefined;
    if (abort) abort.disabled = true;
    if (review) review.disabled = false;
    setExperimentFormLocked(false);
    return;
  }
  setText("[data-test-state]", String(payload?.reason ?? "The runtime test failed."));
  if (root) delete root.dataset.experimentPlanId;
  reviewedExperimentForm = undefined;
  if (abort) abort.disabled = true;
  if (review) review.disabled = false;
  setExperimentFormLocked(false);
}

function formatExperimentValue(value: unknown): string {
  if (typeof value === "boolean") return value ? "Enabled" : "Disabled";
  return formatEngineeringNumber(Number(value));
}

function openAutomaticTune(): void {
  const plan = document.querySelector<HTMLElement>("[data-auto-tune-plan]");
  if (plan) plan.hidden = false;
}

function setText(selector: string, value: unknown, title?: string): void {
  const element = document.querySelector<HTMLElement>(selector);
  if (!element) return;
  element.textContent = String(value ?? "—");
  if (title !== undefined) element.title = title;
}

function setHidden(selector: string, hidden: boolean): void {
  const element = document.querySelector<HTMLElement>(selector);
  if (element) element.hidden = hidden;
}

function renderResultComparison(left: Record<string, unknown>, right: Record<string, unknown>): void {
  const element = document.querySelector<HTMLElement>("[data-result-compare-state]");
  if (!element) return;
  const identity = (side: Record<string, unknown>) => {
    const createdAt = formatLocalTimestamp(String(side.createdAt));
    return {
      text: `Revision ${String(side.resultRevision)} · ${String(side.source)} · ${String(side.method)} · ` +
        `controller ${compactRevisionLabel(String(side.controllerRevision))} · created ${createdAt.display}`,
      title: createdAt.title,
    };
  };
  const leftConclusion = String(left.conclusion ?? "No conclusion recorded.");
  const rightConclusion = String(right.conclusion ?? "No conclusion recorded.");
  const conclusions = (leftConclusion === rightConclusion
    ? [`Conclusion (both): ${leftConclusion}`]
    : [`Revision ${String(left.resultRevision)}: ${leftConclusion}`,
      `Revision ${String(right.resultRevision)}: ${rightConclusion}`]
  ).map((text) => ({ text, title: undefined as string | undefined }));
  element.replaceChildren(...[identity(left), identity(right), ...conclusions].map(({ text, title }) => {
    const line = document.createElement("div");
    line.textContent = text;
    if (title !== undefined) line.title = title;
    return line;
  }));
}

function renderExportResult(format: string, fullPath: string): void {
  const element = document.querySelector<HTMLElement>("[data-result-state]");
  if (!element) return;
  const filename = fullPath.split(/[\\/]/).pop() || "selected file";
  element.textContent = `${format} exported: ${filename}.`;
  element.title = fullPath;
}

function renderValidationRequirements(requirements: readonly ValidationRequirementRow[]): void {
  const body = document.querySelector<HTMLTableSectionElement>("[data-validation-requirements]");
  if (!body) return;
  body.replaceChildren(...requirements.map((requirement) => {
    const row = document.createElement("tr");
    const cells = [
      requirement.name,
      requirement.status,
      formatValidationRequirementValue(requirement.value, requirement.unit),
      formatValidationRequirementValue(requirement.limit, requirement.unit, true),
      requirement.source ?? "—",
      requirement.reason,
    ];
    row.append(...cells.map((value) => {
      const cell = document.createElement("td");
      cell.textContent = value;
      return cell;
    }));
    return row;
  }));
}

function showError(message?: string): void {
  const error = document.querySelector<HTMLElement>("[data-auto-tune-error]");
  if (!error) return;
  error.hidden = !message;
  error.textContent = message ?? "";
}

// W-15: a failed Tune or Automatic Tune attempt must not leave a candidate on
// screen that looks like it belongs to the failure. Spec §14 requires an error
// to preserve earlier valid results, so the candidate is marked stale (dimmed,
// with an explicit note) rather than cleared.
function markSharedApplicationCandidateStale(reason: string): void {
  const section = document.querySelector<HTMLElement>("[data-shared-application-review]");
  const state = document.querySelector<HTMLElement>("[data-shared-application-state]");
  if (!section || section.hidden || !state) return;
  state.textContent = staleCandidateNotice("Tune", reason);
  state.title = "This candidate predates the latest failed attempt; earlier valid results are preserved.";
  section.style.opacity = "0.6";
}

function renderApplicationCandidate(candidate: Record<string, unknown>): void {
  const review = document.querySelector<HTMLElement>("[data-shared-application-review]");
  if (review) review.style.opacity = "";
  const settings = candidate.parameters as Record<string, unknown> | undefined;
  setText("[data-application-source]", candidate.source);
  setText("[data-application-structure]", candidate.structure);
  setText("[data-control-studio-candidate-revision]", "Unapplied");
  if (!settings) return;
  const units = controllerUnits();
  setText("[data-application-gains]",
    `Kp ${formatEngineeringNumber(settings.proportionalGain, 5)} ${units.proportional} · ` +
    `Ki ${formatEngineeringNumber(settings.integralGain, 5)} ${units.integral} · ` +
    `Kd ${formatEngineeringNumber(settings.derivativeGain, 5)} ${units.derivative}`);
  setText("[data-application-limits]", `${formatEngineeringNumber(settings.outputMinimum)}…${formatEngineeringNumber(settings.outputMaximum)} ${units.output}`);
  setText("[data-application-settings]", JSON.stringify(settings, null, 2));
}

function controllerUnits() {
  return pidEngineeringUnits(root?.dataset.processUnit ?? "", root?.dataset.outputUnit ?? "");
}

function markModeComplete(mode: Mode): void {
  const button = buttons.find((candidate) => (candidate.textContent ?? "").trim() === mode);
  if (!button) return;
  button.dataset.status = "complete";
  button.setAttribute("aria-label", `${mode}, complete`);
}

function updateTuneMethod(): void {
  const method = tuneMethod?.value ?? "SIMC";
  const objective = document.querySelector<HTMLElement>("[data-tune-objective-field]");
  const settings = document.querySelector<HTMLDetailsElement>("[data-tune-settings]");
  const imported = document.querySelector<HTMLElement>("[data-tune-import]");
  if (objective) objective.hidden = method === "Manual" || method === "Imported";
  if (settings) { settings.hidden = method !== "Manual"; settings.open = method === "Manual"; }
  if (imported) imported.hidden = method !== "Imported";
}

function readManualPidSettings(): Record<string, unknown> {
  const result: Record<string, unknown> = {};
  for (const input of document.querySelectorAll<HTMLInputElement | HTMLSelectElement>("[data-pid-setting]")) {
    const name = input.dataset.pidSetting;
    if (name) result[name] = name === "antiWindup" ? input.value : Number(input.value);
  }
  return result;
}

// V-16: the acceptance-evidence requirement table is the evidence a learner must
// judge, so it starts open (the learner can still collapse it); the apply path
// stays available but visually secondary to the primary validation action.
document.querySelector<HTMLElement>("[data-validation-requirements]")?.closest("details")?.setAttribute("open", "");
const candidateReviewButton = document.querySelector<HTMLButtonElement>("[data-candidate-review]");
if (candidateReviewButton) {
  candidateReviewButton.classList.remove("primary");
  candidateReviewButton.classList.add("secondary");
}

updateTuneMethod();

post("control-studio.ready");
