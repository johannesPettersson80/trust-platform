import type { ParameterBinding, ProjectTestForm } from "../generated/OpenResult";
import type { ParameterToolOffer } from "../generated/ParameterWorkAnswer";
import { formatEngineeringNumber } from "../chartContract";
import { ParameterFields } from "./parameterFields";

/** The answers of the project test the page receives: the phases of the engine route (`engine/routes/experimentRoute.ts`) and the kept recording. */
type ExperimentPhase = "review" | "running" | "failed" | "recording" | "parameters" | "engine-ended";

export type ThreePageExperimentPost = (kind: string, payload?: Record<string, unknown>) => void;
type ExperimentPayload = Record<string, unknown>;
type PendingAction = "review" | "approve" | "abort" | "parameter-inspect" | "parameter-review" | "parameter-apply" | undefined;
type ToolStep = ParameterToolOffer["nextStep"];

/** The words of the tool's step button for each step the engine offers (C12). */
const STEP_LABELS: Record<ToolStep, string> = {
  inspect: "Inspect experiment parameters",
  review: "Review parameter values",
  apply: "Apply reviewed values",
  restore: "Restore previous value",
};

/**
 * The project test's section of the connected Identify page (specification 35, C12), with the parameter tool its form names. It shows
 * the engine's answers and posts the engineer's requests; the engine decides every rule and words every outcome: the review and the
 * identity its start names, the next action of a review that finds no runtime running, the sentences of the test's end, and the tool's
 * offer, the step its button runs next and the actions it takes. This class keeps the identities the engine gave, the request in
 * flight, and the selection of the signals the chart draws.
 */
export class ThreePageExperiments {
  private readonly section: HTMLElement;
  private readonly signals: HTMLSelectElement;
  private readonly status: HTMLElement;
  /** The note a finished test carries under its status: the fit's refusal the engine answers with the test's end (C12). */
  private readonly note: HTMLElement;
  private readonly planView: HTMLElement;
  private readonly parameterSection?: HTMLElement;
  private readonly parameterStatus?: HTMLElement;
  private readonly parameterValues?: HTMLTextAreaElement;
  private readonly parameterFields?: ParameterFields;
  private readonly parameterBinding?: ParameterBinding;
  private readonly restoreButton?: HTMLButtonElement;
  /** The parameter tool's own step button (C12); the Identify button stays the project test's. */
  private readonly parameterAction?: HTMLButtonElement;
  private experimentId: string | undefined;
  private pending: PendingAction = undefined;
  /**
   * The engine's last offer of the parameter tool (C12): the step its button runs next and the actions the engine takes now. The open
   * answer's form names the first one (`parameterOffer`); a form without a tool names none.
   */
  private offer: ParameterToolOffer | undefined;
  /** The identity of the engine's last parameter review, which the confirmation names; none once an answer gives no review. */
  private parameterReviewId: string | undefined;
  /** The engine's last parameter review writes to a physical target, so its confirmation needs the engineer's approval (A2.5). */
  private parameterReviewPhysical = false;
  /** The engineer's approval of a parameter write to a physical target, shown with a review that needs it (A2.5, B9.3). */
  private readonly parameterApproval?: HTMLElement;
  private readonly parameterApprove?: HTMLInputElement;
  /** The engine's review of the project test and the identity its start is confirmed with (C12). */
  private reviewId: string | undefined;
  /** Set when the engine's review carries an approval block: the project test runs on a physical target (A2.5). */
  private reviewNeedsApproval = false;
  private selectedMethod = false;

  constructor(private readonly root: HTMLElement, form: ProjectTestForm,
    private readonly post: ThreePageExperimentPost,
    /** The shared stage gate; Identify stays disabled while any page gate reports a block. */
    private readonly blocked: () => string | undefined = () => undefined) {
    const section = root.querySelector<HTMLElement>("[data-three-page-experiment]");
    if (!section) throw new Error("The project experiment workspace is missing.");
    this.section = section;
    this.signals = section.querySelector<HTMLSelectElement>("[data-three-page-experiment-signals]")!;
    this.status = section.querySelector<HTMLElement>("[data-three-page-experiment-status]")!;
    // The note is a line of its own under the status, which stays the sentence the engine ends the test with (C12).
    this.note = document.createElement("p");
    this.note.className = "studio-action-message";
    this.note.dataset.threePageExperimentNote = "";
    this.note.hidden = true;
    this.status.after(this.note);
    this.planView = section.querySelector<HTMLElement>("[data-three-page-experiment-plan]")!;
    // The engine's form names the parameter tool (B6.1, C12), and the page fills one slot for it: the experiment's settings inside the
    // project test on the Identify page, or the declared analysis's parameters on the Tune page after the analysis it serves.
    this.parameterSection = root.querySelector<HTMLElement>("[data-three-page-experiment-parameters]") ?? undefined;
    this.parameterStatus = root.querySelector<HTMLElement>("[data-three-page-experiment-parameter-status]") ?? undefined;
    this.parameterValues = root.querySelector<HTMLTextAreaElement>("[data-three-page-experiment-parameter-values]") ?? undefined;
    // The engine's form names the tool, the parameters it starts from and its first offer (B6.1, C12); a form without a binding has no tool.
    this.parameterBinding = form.parameterBinding ?? undefined;
    this.offer = form.parameterOffer ?? undefined;
    this.restoreButton = root.querySelector<HTMLButtonElement>("[data-three-page-experiment-restore]") ?? undefined;
    this.restoreButton?.addEventListener("click", () => this.reviewRestore());
    this.parameterAction = root.querySelector<HTMLButtonElement>("[data-three-page-experiment-parameter-action]") ?? undefined;
    this.parameterAction?.addEventListener("click", () => this.parameterStep());
    this.parameterApproval = root.querySelector<HTMLElement>("[data-three-page-experiment-parameter-approval]") ?? undefined;
    this.parameterApprove = root.querySelector<HTMLInputElement>("[data-three-page-experiment-parameter-approve]") ?? undefined;
    this.parameterApprove?.addEventListener("change", () => this.render());
    const parameterHost = root.querySelector<HTMLElement>("[data-three-page-experiment-parameter-fields]");
    if (parameterHost && this.parameterValues && this.parameterBinding) {
      this.parameterFields = new ParameterFields(parameterHost, this.parameterValues, () => this.editParameters());
      this.parameterFields.configure(this.parameterBinding);
      const mode = root.querySelector<HTMLElement>("[data-three-page-experiment-parameter-mode]");
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

  get isSelected(): boolean { return this.selectedMethod; }
  get busy(): boolean { return this.pending !== undefined || this.experimentId !== undefined; }
  /** A test the page can start: the engine's review of the project test. */
  private get hasReview(): boolean { return this.reviewId !== undefined; }
  /** The engine's review needs the engineer's approval of its block (A2.5). */
  private get approvalNeeded(): boolean { return this.reviewId !== undefined && this.reviewNeedsApproval; }
  /** The parameter tool is offered when the engine's form names it (B6.1, C12). */
  private get parameterFlow(): boolean { return this.parameterBinding !== undefined; }
  /** The Identify button's words while Project experiment is the method: the project test's own steps, review, start and Abort (C12). */
  get primaryActionLabel(): string {
    if (this.experimentId) return "Abort test";
    if (this.pending === "review") return "Reviewing…";
    if (this.pending === "approve") return "Starting…";
    if (this.hasReview) return this.approvalNeeded ? "Approve" : "Confirm and start";
    return "Review project test";
  }
  /** The words of the parameter tool's own step button (B6.1, C12): the step the engine offers next, or the request in flight. */
  get parameterStepLabel(): string {
    if (this.pending === "parameter-inspect") return "Inspecting…";
    if (this.pending === "parameter-review") return "Reviewing values…";
    if (this.pending === "parameter-apply") return "Applying values…";
    return this.offer ? STEP_LABELS[this.offer.nextStep] : "";
  }

  /** The existing Identify button delegates here when Project experiment is selected: it reviews, starts or aborts the project test. */
  primaryAction(): boolean {
    if (!this.selectedMethod || this.pending) return false;
    if (this.experimentId) {
      this.abort();
      return true;
    }
    if (this.hasReview) {
      this.approve();
      return true;
    }
    this.review();
    return true;
  }

  /** The parameter tool's step (B6.1, C12): the one the engine offers next. The button waits while the page's own request or test runs. */
  private parameterStep(): boolean {
    if (this.pending || this.experimentId !== undefined || !this.parameterFlow) return false;
    switch (this.offer?.nextStep) {
      case "inspect": return this.inspectParameters();
      case "review": return this.reviewParameters();
      case "apply": return this.applyParameters();
      case "restore": return this.reviewRestore();
      default: return false;
    }
  }

  receive(phase: ExperimentPhase, rawPayload: unknown): void {
    const payload = record(rawPayload) ? rawPayload : {};
    if (phase === "recording") { this.publishRecording(phase, payload); return; }
    if (phase === "parameters") { this.receiveParameters(payload); return; }
    this.pending = undefined;
    this.publishRecording(phase, payload);
    // A review, a start or a refused start begins again, so the note of a finished test is no longer its note.
    if (phase === "review" || phase === "running" || phase === "failed") this.showNote(undefined);
    if (phase === "review") {
      if (payload.kind === "reviewed" && typeof payload.planId === "string" && record(payload.review)) {
        // The engine's review of the project test: it names what the start will run (specification 35, §0.1a and C12). On a physical
        // target it carries the approval block, whose sentence the page shows.
        this.reviewId = payload.planId;
        const approval = record(payload.review.approval) ? payload.review.approval : undefined;
        this.reviewNeedsApproval = approval !== undefined;
        this.status.textContent = text(approval?.message) ?? "";
        this.renderReview(payload.review);
      } else {
        // The engine's refusal, with its next action shown by the page's failure (a runtime that is not running names how to start it).
        this.status.textContent = text(payload.reason) ?? "";
        this.reviewId = undefined;
        this.planView.hidden = true;
      }
      this.render();
      return;
    }
    if (phase === "running") {
      // A started test is no longer a review: the engine's review is spent with its start (specification 35, C12).
      this.reviewId = undefined;
      const id = text(payload.experiment_id);
      if (id && id !== this.experimentId) {
        this.experimentId = id;
        window.dispatchEvent(new CustomEvent("trust-control-studio-experiment-started", { detail: { experimentId: id } }));
      }
      this.status.textContent = "Test running…";
      this.render();
      return;
    }
    if (phase === "engine-ended") {
      // The engine ended the campaign it ran for the test: its Abort, its completion or its failure (specification 35, C12).
      this.experimentId = undefined;
      this.reviewId = undefined;
      this.planView.hidden = true;
      // The engine's sentence of the end: a finished test's outcome, the sentence of the cancellation, or the failure.
      this.status.textContent = text(payload.reason) ?? "";
      // A finished test whose fit refused names the refusal under its status; the page shows the engine's words only.
      this.showNote(text(payload.note));
      // After every test's end the engine says whether the test released the experiment settings (C12), in words (`settingsReleased`),
      // which the tool shows, and names the tool's offer that follows, which the tool takes; an end without them leaves the tool as it was.
      const released = text(payload.settingsReleased);
      if (released && this.parameterBinding) this.parameterStatusText(released);
      const offer = toolOffer(payload.offer);
      if (offer && this.parameterBinding) {
        this.offer = offer;
        this.parameterReviewId = undefined;
      }
      this.render();
      return;
    }
    // A host failure keeps the active experiment; the host recovery owner remains authoritative until the engine ends the test.
    if (phase === "failed") {
      // A refused start consumes the engine's review (E11.8): the page reviews the test again before it starts it.
      if (this.reviewId !== undefined) {
        this.reviewId = undefined;
        this.reviewNeedsApproval = false;
        this.planView.hidden = true;
      }
      this.status.textContent = text(payload.reason) ?? "";
      this.render();
    }
  }

  private review(): void {
    // The project test is the loop's declared experiment, which the engine reviews; the page sends no settings of its own (C12).
    this.pending = "review";
    this.status.textContent = "Reviewing test…";
    this.render();
    this.post("control-studio.experiment.review");
  }

  /**
   * The tool's requests (C12): each names its request on the tool's button while it is in flight, and clears the tool's sentence, which
   * the engine's answer, or the write's progress, then states.
   */
  private inspectParameters(): boolean {
    if (!this.parameterBinding) return false;
    this.pending = "parameter-inspect";
    this.parameterStatusText("");
    this.render();
    this.post("control-studio.parameters.inspect");
    return true;
  }

  private reviewParameters(): boolean {
    if (!this.parameterBinding || !this.parameterValues) return false;
    this.pending = "parameter-review";
    this.parameterStatusText("");
    this.render();
    this.post("control-studio.parameters.review", { json: this.parameterValues.value });
    return true;
  }

  /** Reviews the values that the runtime retains from before the last write (C12, E7.5); the review is then confirmed as any review. */
  private reviewRestore(): boolean {
    if (!this.parameterBinding || this.pending || this.experimentId) return false;
    this.pending = "parameter-review";
    this.parameterStatusText("");
    this.render();
    this.post("control-studio.parameters.review-rollback");
    return true;
  }

  private applyParameters(): boolean {
    if (!this.parameterReviewId) return false;
    this.pending = "parameter-apply";
    this.parameterStatusText("");
    this.render();
    // On a physical target the write carries the engineer's approval of the review (A2.5); the engine refuses it without one.
    const approved = this.parameterReviewPhysical && this.parameterApprove?.checked === true;
    this.post("control-studio.parameters.apply", { reviewId: this.parameterReviewId, physicalApproved: approved });
    return true;
  }

  /** The engineer edits a value: the review the page holds no longer shows the values on screen, and the engine answers the edit (C12). */
  private editParameters(): void {
    if (!this.parameterBinding || this.pending || this.experimentId) return;
    this.parameterReviewId = undefined;
    this.parameterStatusText("");
    this.post("control-studio.parameters.invalidate");
    this.render();
  }

  /**
   * An answer of the parameter tool (C12): the values and the binding it names, the engine's sentence, and the tool's offer with the
   * identity its confirmation names when the answer is a review. A refusal carries no offer and leaves the tool as it was.
   */
  private receiveParameters(payload: ExperimentPayload): void {
    if (!this.parameterBinding) return;
    // The progress of the tool's write, in the engine's words (C12): the write is still in flight.
    if (payload.status === "running") {
      this.parameterStatusText(text(payload.reason) ?? "");
      return;
    }
    this.pending = undefined;
    // The fields follow the binding that the engine names in its answer (C12). An answer without one, such as a review, keeps them.
    const named = payload.binding as ParameterBinding | null | undefined;
    if (named) this.parameterFields?.configure(named);
    // The values the engine answered: the record it read, the values a review read, or the values a write confirmed.
    if (record(payload.values) && this.parameterValues) {
      this.parameterValues.value = JSON.stringify(payload.values, null, 2);
      this.parameterFields?.sync();
    }
    const offer = toolOffer(payload.offer);
    if (offer) {
      this.offer = offer;
      this.parameterReviewId = text(payload.reviewId);
      // Each answer with an offer replaces the review, and the approval the engineer gave belongs to the review it was given for.
      this.parameterReviewPhysical = this.parameterReviewId !== undefined && payload.physical === true;
      if (this.parameterApprove) this.parameterApprove.checked = false;
    }
    this.parameterStatusText(text(payload.reason) ?? "");
    this.render();
  }

  private approve(): void {
    if (!this.reviewId) return;
    // The engine's review is confirmed by its identity; the engine starts the campaign that it describes.
    this.pending = "approve";
    this.status.textContent = "Starting test…";
    this.render();
    // The start names the reviewed identity; on a physical target the press of Approve is the engineer's approval (A2.5).
    this.post("control-studio.experiment.approve", { planId: this.reviewId, physicalApproved: this.approvalNeeded });
  }

  private abort(): void {
    if (!this.experimentId) return;
    this.pending = "abort";
    this.status.textContent = "Abort requested; waiting for project restoration…";
    this.render();
    this.post("control-studio.experiment.abort", { experimentId: this.experimentId });
  }

  /** Shows the engine's review of the project test (C12): the signal it sets, its duration, its restoration and its stop conditions. */
  private renderReview(review: Record<string, unknown>): void {
    const scripted = record(review.projectScripted) ? review.projectScripted : undefined;
    const activation = scripted && record(scripted.activation) ? scripted.activation : undefined;
    const restoration = scripted && record(scripted.restoration) ? scripted.restoration : undefined;
    const baseline = scripted && typeof scripted.baselineSeconds === "number" ? scripted.baselineSeconds : undefined;
    const stops = scripted && Array.isArray(scripted.stopConditions)
      ? scripted.stopConditions.filter((stop): stop is string => typeof stop === "string") : [];
    const duration = typeof review.maximumDurationSeconds === "number" ? review.maximumDurationSeconds : undefined;
    this.planView.hidden = false;
    // The test signal in the engine's words: the activation the start writes TRUE, or that the test is observation only (C12).
    this.setText("[data-three-page-experiment-bound]", text(scripted?.testSignal) ?? "");
    this.setText("[data-three-page-experiment-duration]", duration === undefined ? ""
      : `${formatEngineeringNumber(duration)} s${baseline === undefined ? "" : ` · ${formatEngineeringNumber(baseline)} s baseline`}`);
    this.setText("[data-three-page-experiment-restoration]", activation && restoration
      ? `${text(activation.symbol) ?? ""} = ${restoration.activationRestoredTo === true ? "TRUE" : "FALSE"}` : "");
    this.setText("[data-three-page-experiment-stops]", stops.join(", "));
  }

  private render(): void {
    this.section.hidden = !this.selectedMethod;
    this.planView.hidden = !this.hasReview || Boolean(this.experimentId);
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

  /** Draws the tool as the engine's offer says (C12); its controls wait while the page's own request or test runs. */
  private renderParameters(): void {
    if (!this.parameterSection) return;
    this.parameterSection.hidden = !this.parameterFlow;
    const waiting = this.pending !== undefined || this.experimentId !== undefined;
    const offers = (step: ToolStep): boolean => this.offer?.available.includes(step) === true;
    for (const input of Array.from(this.parameterSection.querySelectorAll<HTMLInputElement>("input:not([data-three-page-experiment-parameter-approve])"))) {
      input.disabled = waiting || !offers("review");
    }
    if (this.restoreButton) this.restoreButton.disabled = waiting || !offers("restore");
    // A review of a write to a physical target shows the approval, and its confirmation waits for it (A2.5, B9.3).
    const approving = this.parameterReviewPhysical && this.offer?.nextStep === "apply";
    if (this.parameterApproval) this.parameterApproval.hidden = !approving;
    if (this.parameterApprove) this.parameterApprove.disabled = waiting;
    if (this.parameterAction) {
      // Its step is the tool's own; the review and start of the project test stay on the Identify page.
      this.parameterAction.hidden = !this.parameterFlow;
      this.parameterAction.textContent = this.parameterStepLabel;
      this.parameterAction.disabled = waiting || this.blocked() !== undefined || (approving && this.parameterApprove?.checked !== true);
    }
    if (this.parameterStatus) this.parameterStatus.hidden = false;
  }

  /** Shows the note a finished test carries under its status, or hides it when the test carries none (C12). */
  private showNote(note: string | undefined): void {
    this.note.textContent = note ?? "";
    this.note.hidden = note === undefined;
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

function record(value: unknown): value is Record<string, any> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function text(value: unknown): string | undefined {
  return typeof value === "string" && value.length > 0 ? value : undefined;
}

/** The engine's offer of the tool in an answer the route posted (C12), typed as the engine wrote it; undefined when the answer has none. */
function toolOffer(value: unknown): ParameterToolOffer | undefined {
  if (!record(value) || typeof value.nextStep !== "string" || !(value.nextStep in STEP_LABELS) || !Array.isArray(value.available)) return undefined;
  return value as ParameterToolOffer;
}
