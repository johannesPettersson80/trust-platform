import { formatEngineeringNumber } from "../chartContract";
import { experimentRestorationMessage, experimentRestoringMessage, experimentStatusLabel, experimentTerminalMessage } from "../failureMessages";
import { experimentApprovalButtonClass, type ExperimentFormSnapshot } from "../planPresentation";
import { renderRecordingActions } from "./recordingControls";
import type { WorkflowActionAvailability } from "./workflowActionAvailability";

interface SaturationEvidenceView {
  clear(): void;
  receive(value: unknown): void;
}

interface AntiWindupControlsView {
  receive(phase: string, payload: Record<string, unknown> | undefined): void;
}

interface RelayTuningControlsView {
  receive(phase: string, payload: Record<string, unknown> | undefined): void;
  reset(): void;
}

interface ExperimentWorkflowDependencies {
  readonly root: HTMLElement | null;
  readonly setText: (selector: string, value: unknown, title?: string) => void;
  readonly setWorkflowActionAvailability: (update: WorkflowActionAvailability) => void;
  readonly setExperimentFormLocked: (locked: boolean) => void;
  readonly lastSubmittedForm: () => ExperimentFormSnapshot | undefined;
  readonly setReviewedForm: (value: ExperimentFormSnapshot | undefined) => void;
  readonly saturationEvidence: SaturationEvidenceView;
  readonly antiWindupControls: AntiWindupControlsView;
  readonly relayTuningControls: RelayTuningControlsView;
}

/** Owns Test review, runtime-terminal, and retained-evidence presentation. */
export function initializeExperimentWorkflow(dependencies: ExperimentWorkflowDependencies) {
  const changeReview = document.querySelector<HTMLButtonElement>("[data-test-change-review]");
  changeReview?.addEventListener("click", () => {
    if (dependencies.root) {
      delete dependencies.root.dataset.experimentPlanId;
      delete dependencies.root.dataset.experimentSource;
    }
    dependencies.setReviewedForm(undefined);
    dependencies.setExperimentFormLocked(false);
    const approve = document.querySelector<HTMLButtonElement>("[data-test-approve]");
    const review = document.querySelector<HTMLButtonElement>("[data-test-review]");
    const preview = document.querySelector<HTMLElement>("[data-test-preview]");
    if (approve) approve.hidden = true;
    if (review) { review.hidden = false; review.disabled = false; }
    if (changeReview) { changeReview.hidden = true; changeReview.disabled = true; }
    if (preview) preview.hidden = true;
    dependencies.setText("[data-test-state]", "Test settings unlocked. Review the test again before starting.");
  });
  return {
    beginStart(): void {
      const approve = document.querySelector<HTMLButtonElement>("[data-test-approve]");
      const change = document.querySelector<HTMLButtonElement>("[data-test-change-review]");
      if (!dependencies.root?.dataset.experimentPlanId) return;
      if (approve) approve.disabled = true;
      if (change) { change.hidden = true; change.disabled = true; }
    },
    render(message: Record<string, unknown>): void {
      const phase = String(message.phase ?? "failed");
      const payload = message.payload as Record<string, unknown> | undefined;
      if (phase === "anti-windup") {
        dependencies.antiWindupControls.receive(phase, payload);
        return;
      }
      if (phase === "relay-candidate") {
        dependencies.relayTuningControls.receive(phase, payload);
        return;
      }
      const approve = document.querySelector<HTMLButtonElement>("[data-test-approve]");
      const changeReview = document.querySelector<HTMLButtonElement>("[data-test-change-review]");
      const abort = document.querySelector<HTMLButtonElement>("[data-test-abort]");
      const review = document.querySelector<HTMLButtonElement>("[data-test-review]");
      const preview = document.querySelector<HTMLElement>("[data-test-preview]");
      if (phase === "review" && payload?.kind === "accepted" && payload.plan && typeof payload.plan === "object") {
        dependencies.saturationEvidence.clear();
        dependencies.relayTuningControls.reset();
        const plan = payload.plan as Record<string, unknown>;
        if (dependencies.root) {
          dependencies.root.dataset.experimentPlanId = String(plan.id ?? "");
          dependencies.root.dataset.experimentSource = String(plan.source ?? "");
        }
        const booleanSignal = typeof plan.from === "boolean" && typeof plan.to === "boolean";
        const unit = booleanSignal ? "" : ` ${plan.unit}`;
        dependencies.setText("[data-test-signal]", `${plan.affectedSignal} · ${formatExperimentValue(plan.from)} → ${formatExperimentValue(plan.to)}${unit}`);
        dependencies.setText("[data-test-bound]", booleanSignal
          ? `${formatExperimentValue(Boolean(plan.minimum))} … ${formatExperimentValue(Boolean(plan.maximum))}`
          : `${formatEngineeringNumber(plan.minimum)}…${formatEngineeringNumber(plan.maximum)}${unit}`);
        const baselineMs = plan.kind === "project_sequence" && Array.isArray(plan.trajectory)
          ? Number(plan.trajectory.find(point => point?.value === 1)?.atMs ?? 0) : 0;
        dependencies.setText("[data-test-timeout]", `${formatEngineeringNumber(Number(plan.durationMs) / 1_000)} s maximum${baselineMs > 0 ? ` · ${formatEngineeringNumber(baselineMs / 1_000)} s baseline before activation` : ""}`);
        dependencies.setText("[data-test-restoration]", `${formatExperimentValue(plan.restorationValue)}${unit}`);
        dependencies.setText("[data-test-stops]", `Stops on: ${(plan.stopConditions as unknown[]).join(", ")}`);
        dependencies.setText("[data-test-state]", plan.source === "physical"
          ? "Physical test ready. Confirming will authorize this exact plan once."
          : "Simulation test ready. Confirm this exact bounded plan.");
        setTestStageStatus("ready", "Ready");
        if (preview) preview.hidden = false;
        if (approve) {
          approve.hidden = false;
          approve.disabled = false;
          const treatment = experimentApprovalButtonClass(plan.source === "physical" ? "physical" : "simulation");
          approve.classList.toggle("danger", treatment === "danger");
          approve.classList.toggle("primary", treatment === "primary");
        }
        if (review) { review.hidden = true; review.disabled = false; }
        if (changeReview) { changeReview.hidden = false; changeReview.disabled = false; }
        dependencies.setReviewedForm(dependencies.lastSubmittedForm());
        dependencies.setExperimentFormLocked(true);
        return;
      }
      if (phase === "review") {
        dependencies.relayTuningControls.reset();
        dependencies.setText("[data-test-state]", String(payload?.reason ?? "The test cannot start."));
        if (approve) approve.hidden = true;
        if (review) { review.hidden = false; review.disabled = false; }
        if (changeReview) { changeReview.hidden = true; changeReview.disabled = true; }
        if (dependencies.root) delete dependencies.root.dataset.experimentPlanId;
        dependencies.setReviewedForm(undefined);
        dependencies.setExperimentFormLocked(false);
        return;
      }
      if (phase === "running") {
        dependencies.saturationEvidence.clear();
        const experimentId = String(payload?.experiment_id ?? dependencies.root?.dataset.experimentPlanId ?? "");
        if (dependencies.root) dependencies.root.dataset.experimentId = experimentId;
        renderRecordingActions(true);
        setTestStageStatus("in-progress", "In progress");
        dependencies.setText("[data-control-studio-recording-evidence]", "Test recording in progress. Use Abort test to stop.");
        dependencies.setWorkflowActionAvailability({ recording: false });
        dependencies.setText("[data-test-state]", "Test running. Live values and limits remain visible above; Abort is available.");
        if (approve) approve.hidden = true;
        if (changeReview) changeReview.hidden = true;
        if (abort) { abort.hidden = false; abort.disabled = false; }
        if (review) review.disabled = true;
        return;
      }
      if (phase === "restoring") {
        renderRecordingActions(true);
        dependencies.setText("[data-control-studio-recording-evidence]", "Test stopped. Confirming restoration…");
        dependencies.setText("[data-test-state]", experimentRestoringMessage(
          String(payload?.status ?? "unknown"), String(payload?.stop_reason ?? "unknown")));
        if (abort) { abort.hidden = true; abort.disabled = true; }
        if (changeReview) changeReview.hidden = true;
        if (review) review.disabled = true;
        return;
      }
      if (phase === "terminal") {
        const restorationReason = typeof payload?.restoration_reason === "string" && payload.restoration_reason.length > 0
          ? payload.restoration_reason : undefined;
        dependencies.setText("[data-test-state]", experimentTerminalMessage({
          status: String(payload?.status ?? "unknown"),
          stopReason: String(payload?.stop_reason ?? "unknown"),
          restoration: String(payload?.restoration ?? "unknown"),
          ...(restorationReason ? { restorationReason } : {}),
        }));
        if (dependencies.root) { delete dependencies.root.dataset.experimentId; delete dependencies.root.dataset.experimentPlanId; }
        renderRecordingActions(false);
        const completed = payload?.status === "completed" && payload?.restoration !== "failed";
        const aborted = payload?.status === "aborted" && payload?.restoration !== "failed";
        setTestStageStatus(completed ? "complete" : aborted ? "warning" : "failed",
          completed ? "Last complete" : aborted ? "Last aborted" : "Last failed");
        const recordingEvidence = payload?.recordingEvidence;
        if (payload?.saturationWindup) dependencies.saturationEvidence.receive(payload.saturationWindup);
        dependencies.antiWindupControls.receive(phase, payload);
        dependencies.relayTuningControls.receive(phase, payload);
        const identification = payload?.identification as Record<string, unknown> | undefined;
        const notQualified = identification?.qualification === "not_qualified";
        if (recordingEvidence && typeof recordingEvidence === "object") {
          const evidence = recordingEvidence as Record<string, unknown>;
          const runtimeComplete = evidence.method === "runtime_scan_buffer" && evidence.completeness === "complete" &&
            typeof evidence.reason === "string";
          const complete = runtimeComplete && !notQualified;
          const recoveryRequired = payload?.status === "failed_to_restore" ||
            (payload?.restoration !== "restored" && payload?.restoration !== "not_required");
          const outcome = recoveryRequired
            ? `Test recovery required. ${experimentRestorationMessage(String(payload?.restoration ?? "unknown"), restorationReason)}`
            : `Last test: ${experimentStatusLabel(String(payload?.status ?? "unknown"))}.`;
          const recording = runtimeComplete
            ? `Recording retained.${notQualified ? " Not qualified for Identify." : ""}`
            : `Recording incomplete. ${String(evidence.reason ?? "Test runtime acquisition is unavailable.")}`;
          dependencies.setText("[data-control-studio-recording-evidence]", `${outcome} ${recording}`);
          dependencies.setWorkflowActionAvailability({ recording: complete, recordedResponse: runtimeComplete,
            ...(runtimeComplete ? { result: true } : {}) });
        }
        if (notQualified) {
          const state = document.querySelector<HTMLElement>("[data-test-state]");
          if (state) state.textContent += " This retained Test record is not qualified for Identify; review a new Test plan to retry.";
          if (!recordingEvidence || typeof recordingEvidence !== "object") dependencies.setWorkflowActionAvailability({ recording: false });
        }
        dependencies.setReviewedForm(undefined);
        if (abort) { abort.hidden = true; abort.disabled = true; }
        if (changeReview) changeReview.hidden = true;
        if (review) { review.hidden = false; review.disabled = false; }
        dependencies.setExperimentFormLocked(false);
        return;
      }
      const reason = String(payload?.reason ?? "The runtime test failed.");
      setTestStageStatus("failed", "Failed");
      dependencies.setText("[data-test-state]", reason);
      if (dependencies.root) delete dependencies.root.dataset.experimentPlanId;
      dependencies.setReviewedForm(undefined);
      if (approve) approve.hidden = true;
      if (changeReview) changeReview.hidden = true;
      if (abort) { abort.hidden = true; abort.disabled = true; }
      const recoveryUnconfirmed = /Test recording recovery remains unconfirmed|reviewed owner or plan is missing/i.test(reason);
      if (review) { review.hidden = recoveryUnconfirmed; review.disabled = recoveryUnconfirmed; }
      dependencies.setExperimentFormLocked(recoveryUnconfirmed);
    },
  };
}

function setTestStageStatus(status: string, label: string): void {
  const tab = document.querySelector<HTMLButtonElement>('[data-control-studio-mode][aria-label^="Test,"]');
  if (tab) { tab.dataset.status = status; tab.dataset.statusLabel = label; tab.setAttribute("aria-label", `Test, ${label}`); }
}

function formatExperimentValue(value: unknown): string {
  if (typeof value === "boolean") return value ? "Enabled" : "Disabled";
  return formatEngineeringNumber(Number(value));
}
