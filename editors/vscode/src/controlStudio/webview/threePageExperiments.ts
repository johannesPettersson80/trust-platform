import { experimentStatusLabel } from "../failureMessages";
import { plainReason } from "./studioFailure";
import type { ControlExperimentPlan, ControlExperimentRuntimePhase } from "../experiment";
import type { ControlLoopDefinition } from "../loopDefinition";
import { displayEngineeringUnit, formatEngineeringNumber } from "../chartContract";
import { ParameterFields } from "./parameterFields";
import type { ParameterBinding } from "../parameterBinding";

export type ThreePageExperimentPost = (kind: string, payload?: Record<string, unknown>) => void;
type ExperimentPayload = Record<string, unknown>;
type PendingAction = "review" | "approve" | "abort" | "parameter-inspect" | "parameter-review" | "parameter-apply" | undefined;
type ParameterPhase = "idle" | "inspected" | "edited" | "reviewed" | "applied";

/**
 * Adapts the existing project Test owner to the connected Identify page.
 * Review, runtime writes, recording, restoration and terminal evidence remain
 * owned by the host; this class only owns contextual state and signal routing.
 */
export class ThreePageExperiments {
  private readonly section: HTMLElement;
  private readonly signals: HTMLSelectElement;
  private readonly status: HTMLElement;
  private readonly planView: HTMLElement;
  private readonly parameterSection?: HTMLElement;
  private readonly parameterStatus?: HTMLElement;
  private readonly parameterValues?: HTMLTextAreaElement;
  private readonly parameterFields?: ParameterFields;
  private readonly parameterBinding?: ParameterBinding;
  private plan: ControlExperimentPlan | undefined;
  private experimentId: string | undefined;
  private pending: PendingAction = undefined;
  private parameterPhase: ParameterPhase = "idle";
  private parameterReviewId: string | undefined;
  private selectedMethod = false;

  constructor(private readonly root: HTMLElement, private readonly loop: ControlLoopDefinition,
    private readonly post: ThreePageExperimentPost,
    /** The shared stage gate; Identify stays disabled while any page gate reports a block. */
    private readonly blocked: () => string | undefined = () => undefined) {
    const section = root.querySelector<HTMLElement>("[data-three-page-experiment]");
    if (!section) throw new Error("The project experiment workspace is missing.");
    this.section = section;
    this.signals = section.querySelector<HTMLSelectElement>("[data-three-page-experiment-signals]")!;
    this.status = section.querySelector<HTMLElement>("[data-three-page-experiment-status]")!;
    this.planView = section.querySelector<HTMLElement>("[data-three-page-experiment-plan]")!;
    this.parameterSection = section.querySelector<HTMLElement>("[data-three-page-experiment-parameters]") ?? undefined;
    this.parameterStatus = section.querySelector<HTMLElement>("[data-three-page-experiment-parameter-status]") ?? undefined;
    this.parameterValues = section.querySelector<HTMLTextAreaElement>("[data-three-page-experiment-parameter-values]") ?? undefined;
    this.parameterBinding = loop.parameterPurpose === "experiment" ? loop.parameterBinding : undefined;
    const parameterHost = section.querySelector<HTMLElement>("[data-three-page-experiment-parameter-fields]");
    if (parameterHost && this.parameterValues && this.parameterBinding) {
      this.parameterFields = new ParameterFields(parameterHost, this.parameterValues, () => this.editParameters());
      this.parameterFields.configure(this.parameterBinding);
      const mode = section.querySelector<HTMLElement>("[data-three-page-experiment-parameter-mode]");
      if (mode) mode.textContent = `Activation: ${this.parameterBinding.mode.symbol} = ${String(this.parameterBinding.mode.manual_value)}`;
    }
    this.signals.addEventListener("change", () => this.publishSignalSelection());
    this.render();
  }

  /** Called by the Identify method selector. */
  setSelectedMethod(selected: boolean): void {
    this.selectedMethod = selected;
    this.section.hidden = !selected;
    this.render();
  }

  synchronize(): void { this.render(); }

  get isSelected(): boolean { return this.selectedMethod; }
  get busy(): boolean { return this.pending !== undefined || this.experimentId !== undefined; }
  get hasReviewedPlan(): boolean { return this.plan !== undefined; }
  get primaryActionLabel(): string {
    if (this.experimentId) return "Abort test";
    if (this.pending === "review") return "Reviewing…";
    if (this.pending === "approve") return "Starting…";
    if (this.pending === "parameter-inspect") return "Inspecting…";
    if (this.pending === "parameter-review") return "Reviewing values…";
    if (this.pending === "parameter-apply") return "Applying values…";
    if (this.plan) return "Confirm and start";
    if (this.parameterBinding) {
      if (this.parameterPhase === "reviewed") return "Apply reviewed values";
      if (this.parameterPhase === "applied") return "Review project test";
      if (this.parameterPhase === "inspected" || this.parameterPhase === "edited") return "Review parameter values";
      return "Inspect experiment parameters";
    }
    return "Review project test";
  }

  /** The existing Identify button delegates here when Project experiment is selected. */
  primaryAction(): boolean {
    if (!this.selectedMethod || this.pending) return false;
    if (this.experimentId) {
      this.abort();
      return true;
    }
    if (this.plan) {
      this.approve();
      return true;
    }
    if (this.parameterBinding) {
      if (this.parameterPhase === "idle") return this.inspectParameters();
      if (this.parameterPhase === "inspected" || this.parameterPhase === "edited") return this.reviewParameters();
      if (this.parameterPhase === "reviewed") return this.applyParameters();
    }
    this.review();
    return true;
  }

  /** Lets the existing Identify button mirror this adapter's state and label. */
  bindPrimaryButton(button: HTMLButtonElement): void {
    button.addEventListener("click", event => {
      event.preventDefault();
      this.primaryAction();
    });
    this.renderPrimaryButton(button);
  }

  /** Unlocks a reviewed plan after the host has stopped the current operation. */
  cancelReview(): void {
    if (this.busy) return;
    this.plan = undefined;
    this.planView.hidden = true;
    if (this.parameterBinding && this.parameterPhase === "applied") this.parameterPhase = "inspected";
    this.status.textContent = "";
    this.render();
  }

  receive(phase: ControlExperimentRuntimePhase | "review" | "failed" | "restoring" | "recording" | "parameters", rawPayload: unknown): void {
    const payload = record(rawPayload) ? rawPayload : {};
    if (phase === "recording") {
      if (record(payload.experiment)) {
        if (this.experimentId) return;
        const restoration = payload.experiment.restoration?.status;
        this.status.textContent = `${experimentStatusLabel(String(payload.experiment.status))} recording.${restoration === "confirmed" ? " Outputs restored." : restoration === "failed" ? " Restoration unconfirmed." : ""}`;
        this.render();
      }
      this.publishRecording(phase, payload); return;
    }
    if (phase === "parameters") { this.receiveParameters(payload); return; }
    this.pending = undefined;
    this.publishRecording(phase, payload);
    if (phase === "review") {
      if (payload.kind === "accepted" && isControlExperimentPlan(payload.plan)) {
        this.plan = payload.plan;
        this.status.textContent = payload.plan.requiresPhysicalApproval
          ? "Project test reviewed. Physical approval remains with the host before confirmation."
          : "";
        this.renderPlan(payload.plan);
      } else {
        this.status.textContent = text(payload.reason) ?? "The project test could not be reviewed.";
        this.plan = undefined;
        this.planView.hidden = true;
      }
      this.render();
      return;
    }
    if (phase === "running") {
      const id = text(payload.experiment_id);
      if (id && id !== this.experimentId) {
        this.experimentId = id;
        window.dispatchEvent(new CustomEvent("trust-control-studio-experiment-started", { detail: { experimentId: id } }));
      }
      this.status.textContent = "Test running…";
      this.render();
      return;
    }
    if (phase === "awaiting_restoration" || phase === "restoring") {
      this.status.textContent = "Restoring outputs…";
      this.render();
      return;
    }
    if (phase === "terminal") {
      const status = text(payload.status) ?? "unknown";
      const restoration = text(payload.restoration) ?? "unknown";
      const restorable = restoration === "restored" || restoration === "not_required";
      if (restorable) {
        this.experimentId = undefined;
        this.plan = undefined;
        this.planView.hidden = true;
        if (this.parameterBinding && this.parameterPhase === "applied") {
          this.parameterPhase = "inspected";
          this.parameterStatusText("The previous project test restored its declared values. Edit and review them before the next trial.");
        }
        this.status.textContent = `Project test ${status}.${restoration === "restored" ? " Outputs restored." : ""}`;
      } else {
        this.status.textContent = `Project test ${status}; restoration remains ${restoration}. Resolve the runtime state before starting another test.`;
      }
      this.render();
      return;
    }
    // Generic host failures preserve the reviewed plan and active experiment;
    // the host recovery owner remains authoritative until a terminal phase.
    if (phase === "failed") {
      this.status.textContent = text(payload.reason) ?? "The project test could not be completed.";
      this.render();
    }
  }

  private review(): void {
    const duration = this.loop.experiment?.durationSeconds;
    if (!Number.isFinite(duration) || duration! < 1) {
      this.status.textContent = "The project has no valid declared test duration.";
      return;
    }
    this.pending = "review";
    this.status.textContent = "Reviewing test…";
    this.render();
    this.post("control-studio.experiment.review", {
      kind: "observation",
      amplitude: 0,
      durationSeconds: duration,
      physicalTopologyReviewed: false,
    });
  }

  private inspectParameters(): boolean {
    if (!this.parameterBinding) return false;
    this.pending = "parameter-inspect";
    this.parameterStatusText("Inspect the running simulator before editing the declared trial values.");
    this.render();
    this.post("control-studio.parameters.inspect");
    return true;
  }

  private reviewParameters(): boolean {
    if (!this.parameterBinding || !this.parameterValues) return false;
    this.pending = "parameter-review";
    this.parameterStatusText("Reviewing the exact declared experiment values; the runtime has not changed.");
    this.render();
    this.post("control-studio.parameters.review", { json: this.parameterValues.value });
    return true;
  }

  private applyParameters(): boolean {
    if (!this.parameterReviewId) return false;
    this.pending = "parameter-apply";
    this.parameterStatusText("Applying the reviewed experiment values and waiting for task confirmation…");
    this.render();
    this.post("control-studio.parameters.apply", { reviewId: this.parameterReviewId, physicalApproved: false });
    return true;
  }

  private editParameters(): void {
    if (!this.parameterBinding || this.pending || this.experimentId) return;
    this.parameterPhase = "edited";
    this.parameterReviewId = undefined;
    this.parameterStatusText("Values changed. Review the exact experiment settings before starting the project test.");
    this.post("control-studio.parameters.invalidate");
    this.render();
  }

  private receiveParameters(payload: ExperimentPayload): void {
    if (!this.parameterBinding) return;
    const pending = this.pending;
    this.pending = undefined;
    const binding = isParameterBinding(payload.binding) ? payload.binding : this.parameterBinding;
    this.parameterFields?.configure(binding);
    if (payload.status === "inspected" || payload.status === "draft") {
      const values = record(payload.values) ? payload.values : record(payload.observed) && record(payload.observed.values) ? payload.observed.values : undefined;
      if (values && this.parameterValues) {
        this.parameterValues.value = JSON.stringify(values, null, 2);
        this.parameterFields?.sync();
      }
      this.parameterReviewId = undefined;
      this.parameterPhase = "inspected";
      this.parameterStatusText(text(payload.reason) ?? "Current experiment values loaded. Edit them, then review the exact change.");
    } else if (payload.status === "reviewed") {
      this.parameterReviewId = text(payload.reviewId);
      this.parameterPhase = this.parameterReviewId ? "reviewed" : "edited";
      this.parameterStatusText(this.parameterReviewId ? "Review the exact experiment values before applying them." : "The parameter review was incomplete; review again.");
    } else if (payload.status === "applied") {
      const observed = record(payload.observed) && record(payload.observed.values) ? payload.observed.values : undefined;
      if (observed && this.parameterValues) {
        this.parameterValues.value = JSON.stringify(observed, null, 2);
        this.parameterFields?.sync();
      }
      this.parameterReviewId = undefined;
      this.parameterPhase = "applied";
      this.parameterStatusText("Reviewed experiment values applied and confirmed. Review the declared project test next.");
    } else if (payload.status === "edited") {
      this.parameterReviewId = undefined;
      this.parameterPhase = "edited";
      this.parameterStatusText(text(payload.reason) ?? "Values changed. Review again before applying.");
    } else if (payload.status === "failed") {
      this.parameterReviewId = undefined;
      this.parameterPhase = pending === "parameter-inspect" ? "idle" : "edited";
      this.parameterStatusText(text(payload.reason) ?? "The experiment parameter operation failed.");
    }
    this.render();
  }

  private approve(): void {
    if (!this.plan || this.plan.requiresPhysicalApproval) {
      this.status.textContent = "The host must complete physical approval before this project test can start.";
      return;
    }
    this.pending = "approve";
    this.status.textContent = "Starting test…";
    this.render();
    this.post("control-studio.experiment.approve", { planId: this.plan.id, physicalApproved: false });
  }

  private abort(): void {
    if (!this.experimentId) return;
    this.pending = "abort";
    this.status.textContent = "Abort requested; waiting for project restoration…";
    this.render();
    this.post("control-studio.experiment.abort", { experimentId: this.experimentId });
  }

  private renderPlan(plan: ControlExperimentPlan): void {
    this.planView.hidden = false;
    this.setText("[data-three-page-experiment-bound]", `${plan.affectedSignal} · ${formatValue(plan.from, plan.unit)} → ${formatValue(plan.to, plan.unit)}`);
    this.setText("[data-three-page-experiment-duration]", `${formatEngineeringNumber(plan.durationMs / 1_000)} s`);
    this.setText("[data-three-page-experiment-restoration]", `${plan.affectedSignal} = ${formatValue(plan.restorationValue, plan.unit)}`);
    this.setText("[data-three-page-experiment-stops]", plan.stopConditions.join(", "));
  }

  private render(): void {
    this.section.hidden = !this.selectedMethod;
    this.planView.hidden = !this.plan || Boolean(this.experimentId);
    this.status.hidden = !this.status.textContent;
    this.renderParameters();
    const button = this.root.querySelector<HTMLButtonElement>("[data-engineer-find]");
    if (button) this.renderPrimaryButton(button);
  }

  private renderPrimaryButton(button: HTMLButtonElement): void {
    if (!this.selectedMethod) return;
    button.textContent = this.primaryActionLabel;
    button.disabled = this.pending !== undefined || this.blocked() !== undefined;
  }

  private renderParameters(): void {
    if (!this.parameterSection) return;
    const editable = !this.pending && !this.experimentId && !this.plan &&
      (this.parameterPhase === "inspected" || this.parameterPhase === "edited");
    for (const input of Array.from(this.parameterSection.querySelectorAll<HTMLInputElement>("input"))) input.disabled = !editable;
    if (this.parameterStatus) this.parameterStatus.hidden = false;
  }

  private parameterStatusText(value: string): void {
    if (this.parameterStatus) this.parameterStatus.textContent = value;
  }

  private publishSignalSelection(): void {
    const signalIds = Array.from(this.signals.selectedOptions).map(option => option.value);
    window.dispatchEvent(new CustomEvent("trust-control-studio-experiment-signals", { detail: { signalIds } }));
  }

  private publishRecording(phase: string, payload: ExperimentPayload): void {
    const recording = record(payload.recording) ? payload.recording : undefined;
    if (!recording) return;
    window.dispatchEvent(new CustomEvent("trust-control-studio-experiment-recording", {
      detail: { phase, recording, signalIds: Array.from(this.signals.selectedOptions).map(option => option.value) },
    }));
  }

  private setText(selector: string, value: string): void {
    const node = this.section.querySelector<HTMLElement>(selector);
    if (node) node.textContent = value;
  }
}

function isControlExperimentPlan(value: unknown): value is ControlExperimentPlan {
  if (!record(value) || typeof value.id !== "string" || typeof value.source !== "string" || typeof value.kind !== "string"
    || typeof value.affectedSignal !== "string" || value.address !== undefined && typeof value.address !== "string" || typeof value.valueType !== "string"
    || typeof value.unit !== "string" || !Number.isFinite(value.minimum) || !Number.isFinite(value.maximum)
    || !Number.isFinite(value.durationMs) || !Number.isFinite(value.expectedScan)
    || !Array.isArray(value.stopConditions) || typeof value.restorationValue !== "boolean" && typeof value.restorationValue !== "number"
    || typeof value.requiresPhysicalApproval !== "boolean" || !Array.isArray(value.trajectory)) return false;
  return ["simulation", "physical"].includes(value.source) && ["observation", "setpoint_step", "load_step", "mv_step", "mv_pulse", "relay", "prbs", "multisine", "output_step", "project_sequence"].includes(value.kind);
}

function formatValue(value: number | boolean, unit: string): string {
  if (typeof value === "boolean") return value ? "TRUE" : "FALSE";
  return `${formatEngineeringNumber(value)}${unit ? ` ${displayEngineeringUnit(unit)}` : ""}`;
}

function record(value: unknown): value is Record<string, any> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function text(value: unknown): string | undefined {
  return typeof value === "string" && value.length > 0 ? plainReason(value) : undefined;
}

function isParameterBinding(value: unknown): value is ParameterBinding {
  return record(value) && Array.isArray(value.parameters) && record(value.mode) &&
    typeof value.mode.symbol === "string" && typeof value.mode.manual_value === "boolean";
}
