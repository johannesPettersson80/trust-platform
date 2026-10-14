import type { StudioPost } from "./threePageOperation";
import { runLockedControls } from "./runLock";
import { studioFailureOf, type StudioFailure } from "./studioFailure";
import { displayEngineeringUnit } from "../unitDisplay";

/** A review the engine answered, which the page shows until the engineer confirms it or the page leaves it (B6.1, B6.5). */
interface EngineReview {
  readonly action: "apply" | "restore" | "keep";
  readonly identity: string;
  readonly physical: boolean;
}

/** The words of each confirmation button (B9.3). */
const CONFIRM_WORDS = { apply: "Confirm Apply", restore: "Confirm restore", keep: "Confirm Keep in project" } as const;

/** The host owns review, writes and receipts; this class only presents their progress. After an Apply, Keep in project is the
 * next action: the applied values become the project's, so a restart or a new download keeps them (Specification 35 §17.17). */
export class ThreePageApplication {
  active = false;
  private action: "apply" | "restore" = "apply";
  private applyAvailable = false;
  private restoreAvailable = false;
  private keepAvailable = false;
  private keeping = false;
  /** The engine's review on the screen, its panel and the physical approval it needs (engine-served loops, V6 step S8). */
  private review?: EngineReview;
  private reviewPanel?: HTMLElement;
  private approval?: HTMLInputElement;
  /** An Apply observation the engine left journaled, which the page offers to resume (A8.5, A8.6). */
  private resumable = false;
  private resumeButton?: HTMLButtonElement;
  private readonly button: HTMLButtonElement;
  private readonly keep: HTMLButtonElement | null;
  constructor(private readonly root: HTMLElement, private readonly post: StudioPost,
    private readonly operationBusy: () => boolean, private readonly notice: (message: string) => void,
    /** Shows a failure with its typed next action in the Validate page's action area. */
    private readonly fail: (failure: StudioFailure) => void = failure => notice(failure.message),
    /** The gates this application cannot know and that hold Apply and Restore: a reset in progress, an external block such as invalid
     * bindings, a running experiment or an active recovery. The application is the only writer of the button's enabled state. */
    private readonly stageBlocked: () => boolean = () => false) {
    this.button = root.querySelector<HTMLButtonElement>("[data-engineer-apply]")!;
    this.button.addEventListener("click", () => {
      if (this.active || this.keeping || !this.available() || this.operationBusy() || this.stageBlocked()) return;
      this.active = true; this.notice(""); this.render();
      this.post(`control-studio.application.${this.action}`);
    });
    this.keep = root.querySelector<HTMLButtonElement>("[data-engineer-keep]");
    this.keep?.addEventListener("click", () => {
      if (this.active || this.keeping || !this.keepAvailable || this.operationBusy()) return;
      this.keeping = true; this.notice(""); this.render();
      this.post("control-studio.application.keep");
    });
  }
  abort(): void {
    if (!this.active) return;
    this.notice("Aborting…"); this.post("control-studio.engineer.cancel");
  }
  receive(payload: Record<string, unknown>): void {
    if (payload.status === "opened") { this.showOpened(payload); return; }
    if (isRecord(payload.engineReview) || isRecord(payload.engineKeepReview)) { this.showReview(payload); return; }
    if (isRecord(payload.engineOutcome)) { this.showOutcome(payload); return; }
    // Apply needs a passed validation (B5.4): an answer whose evidence says the validation did not pass withdraws the offer.
    if (isRecord(payload.stageEvidence) && payload.stageEvidence.validationPassed !== true) this.applyAvailable = false;
    const status = String(payload.status);
    const operation = payload.applicationAction === "apply" || payload.applicationAction === "restore" || payload.applicationAction === "keep";
    if (status === "cancelled" && (operation || this.active || this.keeping)) {
      // The engineer cancelled the operation: its button is free again, and the engine's words say how it ended (B9.4).
      this.active = false;
      this.keeping = false;
      this.notice(typeof payload.reason === "string" ? payload.reason : "The operation was cancelled.");
      this.render();
      return;
    }
    if (this.keeping && status === "failed" && payload.applicationAction === undefined) {
      // The engine refused a Keep request before it answered a review: the Keep button is free again.
      this.keeping = false;
      const failure = studioFailureOf(payload);
      if (failure) this.fail(failure); else this.notice("The operation could not be confirmed.");
      this.render();
      return;
    }
    if (payload.applicationAction === "keep") {
      // A running Keep keeps its button down until its answer or its failure comes (B6.5).
      this.keeping = status === "running";
      if (status === "kept") this.keepAvailable = false;
      if (typeof payload.reason === "string") this.notice(payload.reason);
      this.render();
      return;
    }
    const observedAvailability = payload.applicationAvailability;
    if (observedAvailability === "restore") this.restoreAvailable = true;
    if (observedAvailability === "unavailable") {
      this.restoreAvailable = false;
      if (payload.reason) this.notice(String(payload.reason));
    }
    if (status === "verified") {
      this.applyAvailable = payload.stageEvidence && typeof payload.stageEvidence === "object"
        ? (payload.stageEvidence as Record<string, unknown>).validationPassed === true
        : true;
      // A newly validated candidate makes Apply the next action again.
      if (this.applyAvailable) this.keepAvailable = false;
    }
    if (payload.flowStage && status === "running" || status === "edited" || status === "candidate") {
      this.applyAvailable = false;
    }
    // A review of a candidate that has since changed is no longer the review the engine would confirm (B6.7).
    if (this.review && ["edited", "candidate", "verified", "running", "model-ready"].includes(status)) this.clearReview();
    if (status === "restored" && observedAvailability === undefined) {
      this.applyAvailable = false;
      this.restoreAvailable = false;
    }
    const application = payload.applicationAction === "apply" || payload.applicationAction === "restore";
    if (application || this.active) {
      if (status === "commissioned" || status === "applied") {
        const restoring = (payload.applicationAction ?? this.action) === "restore";
        this.active = false;
        this.applyAvailable = false;
        this.restoreAvailable = !restoring;
        this.keepAvailable = !restoring;
        this.action = restoring ? "apply" : "restore";
        this.notice(restoring ? "Restored" : "Applied. Keep in project to make these values the project's.");
      } else if (["failed", "recovery-required", "unavailable", "busy"].includes(status)) {
        const restoring = (payload.applicationAction ?? this.action) === "restore";
        const failure = studioFailureOf(payload);
        // A "validate" next action keeps the validated settings applicable, for example once the process has settled.
        const reapply = failure?.nextAction.kind === "validate";
        this.active = false;
        if (restoring) this.restoreAvailable = false;
        else this.applyAvailable = reapply && this.applyAvailable;
        if (failure) this.fail(failure); else this.notice("The operation could not be confirmed.");
        if (this.resumable) this.showResume();
      } else if (status === "running" && typeof payload.reason === "string") {
        // For example while Apply waits for the process to settle at the validated operating point.
        this.notice(payload.reason);
      } else if (status === "review" && payload.applicationAction === "restore") {
        this.restoreAvailable = payload.applicationAvailability === "restore" || payload.applicationAvailability === undefined;
        this.action = "restore";
      }
    }
    this.action = this.applyAvailable ? "apply" : this.restoreAvailable ? "restore" : this.action;
    this.render();
  }
  synchronize(): void { this.render(); }
  private available(): boolean { return this.action === "restore" ? this.restoreAvailable : this.applyAvailable; }
  private render(): void {
    this.button.textContent = this.active ? this.action === "restore" ? "Restoring…" : "Applying…"
      : this.action === "restore" ? "Restore previous settings" : "Apply";
    this.button.disabled = this.active || this.keeping || this.operationBusy() || this.stageBlocked() || !this.available();
    // One primary action: Keep in project once the values are applied, else Apply.
    const keepNext = this.keepAvailable || this.keeping;
    this.button.classList.toggle("trust-button--primary", !keepNext);
    if (this.keep) {
      this.keep.hidden = !keepNext;
      this.keep.classList.toggle("trust-button--primary", keepNext);
      this.keep.textContent = this.keeping ? "Keeping…" : "Keep in project";
      this.keep.disabled = this.keeping || this.active || this.operationBusy();
    }
    if (!this.active) return;
    for (const control of runLockedControls(this.root)) control.disabled = true;
    for (const button of Array.from(this.root.querySelectorAll<HTMLButtonElement>("[data-engineer-find],[data-engineer-tune],[data-engineer-verify]"))) { button.textContent = "Abort"; button.disabled = false; }
  }

  /**
   * Shows the engine's review of a Keep, or of an Apply or a Restore on a physical target, and its confirmation: the confirmation sends
   * the review's identity, and the engine refuses it when the runtime or the project changed since the review (§0.1b, B6.7). A physical
   * target needs the engineer's approval first (A2.5); on a simulated target the press of Apply or Restore confirms its review at once.
   */
  private showReview(payload: Record<string, unknown>): void {
    const keep = isRecord(payload.engineKeepReview);
    const source = (keep ? payload.engineKeepReview : payload.engineReview) as Record<string, unknown>;
    const action = keep ? "keep" : source.action === "restore" ? "restore" : "apply";
    const physical = source.physical === true;
    if (action !== "keep" && !physical) {
      // On a simulated target the press is the confirmation (A2.5, B9.3): the reviewed Apply or Restore runs at once under the
      // review's identity, and the page shows no confirmation. The engine still refuses it if the runtime changed since the review.
      this.clearReview();
      this.keeping = false;
      this.active = true;
      this.action = action;
      this.notice(""); this.render();
      this.post(`control-studio.application.${action}`, { review: text(source.identity), physicalApproved: false });
      return;
    }
    this.active = false;
    this.keeping = false;
    this.review = { action, identity: text(source.identity), physical };
    if (action !== "keep") this.action = action;
    const children: Node[] = [];
    const heading = document.createElement("p");
    heading.textContent = action === "keep" ? "Keep in project writes these values into the project's record." : text(source.description);
    children.push(heading);
    const list = document.createElement("ul");
    for (const line of reviewLines(source, action)) {
      const item = document.createElement("li");
      item.textContent = line;
      list.append(item);
    }
    children.push(list);
    // The approval block of a physical target: what the engine's review says the approval covers (A2.5, B9.3).
    if (physical && action !== "keep" && isRecord(source.approval)) children.push(approvalList(source.approval));
    const confirm = document.createElement("button");
    confirm.type = "button";
    confirm.className = "trust-button trust-button--primary";
    confirm.dataset.engineConfirm = action;
    confirm.textContent = CONFIRM_WORDS[action];
    confirm.addEventListener("click", () => this.confirmReview());
    if (physical) {
      const approval = document.createElement("input");
      approval.type = "checkbox";
      approval.dataset.engineApproval = "";
      approval.addEventListener("change", () => { confirm.disabled = !approval.checked; });
      const label = document.createElement("label");
      label.append(approval, " Approve this write to the physical target");
      this.approval = approval;
      confirm.disabled = true;
      children.push(label, this.cancelButton());
    }
    children.push(confirm);
    const panel = this.reviewArea();
    panel.replaceChildren(...children);
    panel.hidden = false;
    this.render();
  }

  /** The engineer's confirmation of the review on the screen; it runs the reviewed change under the review's identity. */
  private confirmReview(): void {
    const review = this.review;
    if (!review || this.active || this.keeping || this.operationBusy()) return;
    const approved = this.approval?.checked === true;
    if (review.physical && !approved) return;
    this.clearReview();
    if (review.action === "keep") this.keeping = true;
    else { this.active = true; this.action = review.action; }
    this.notice(""); this.render();
    this.post(`control-studio.application.${review.action}`, { review: review.identity, physicalApproved: approved });
  }

  /** Shows the outcome of a confirmed Apply or Restore with the availability the engine answered (B6.3, B6.4, B6.5, B6.7). */
  private showOutcome(payload: Record<string, unknown>): void {
    const availability = payload.engineOutcome as Record<string, unknown>;
    this.clearReview();
    // The operation that the observation was resumed as has ended, so the observation is no longer waiting for the page.
    this.resumable = false;
    this.active = false;
    this.keeping = false;
    this.applyAvailable = false;
    this.restoreAvailable = availability.restoreAvailable === true;
    this.keepAvailable = availability.keepAvailable === true;
    this.action = this.restoreAvailable ? "restore" : "apply";
    if (payload.status === "failed") {
      const failure = studioFailureOf(payload);
      if (failure) this.fail(failure); else this.notice("The operation could not be confirmed.");
    } else if (typeof payload.reason === "string") {
      this.notice(payload.reason);
    }
    this.render();
  }

  /**
   * The Apply availability of an opened loop (A8.6, B7): Restore and Keep in project as the engine answers them, and the journaled Apply
   * observation the engine offers to resume. Validate's answer still decides Apply itself, so `applyAvailable` is left as it is.
   */
  private showOpened(payload: Record<string, unknown>): void {
    const availability: Record<string, unknown> = isRecord(payload.engineOutcome) ? payload.engineOutcome : {};
    this.clearReview();
    this.active = false;
    this.keeping = false;
    this.restoreAvailable = availability.restoreAvailable === true;
    this.keepAvailable = availability.keepAvailable === true;
    this.resumable = payload.applyObservationResumable === true;
    this.action = this.restoreAvailable ? "restore" : "apply";
    if (this.resumable) this.showResume();
    this.render();
  }

  /** The interrupted observation, with the button that follows it to its end (A8.5, A8.6); Restore stays offered beside it. */
  private showResume(): void {
    if (!this.resumeButton) {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "trust-button trust-button--primary";
      button.dataset.engineResume = "";
      button.textContent = "Resume the Apply";
      button.addEventListener("click", () => this.resumeObservation());
      this.resumeButton = button;
    }
    const note = document.createElement("p");
    note.textContent = "An Apply was interrupted before its end. Resume it to see its result, or restore the previous settings.";
    const panel = this.reviewArea();
    panel.replaceChildren(note, this.resumeButton);
    panel.hidden = false;
  }

  /** Follows the journaled observation to its end: the engine's Apply with `resume`, which runs no write (A8.5). */
  private resumeObservation(): void {
    if (!this.resumable || this.active || this.keeping || this.operationBusy()) return;
    this.clearReview();
    this.active = true;
    this.action = "apply";
    this.notice(""); this.render();
    this.post("control-studio.application.apply", { resume: true });
  }

  /** Leaves an approval without writing anything (B9.3): the review leaves the screen and nothing is sent to the engine. */
  private cancelButton(): HTMLButtonElement {
    const cancel = document.createElement("button");
    cancel.type = "button";
    cancel.className = "trust-button";
    cancel.dataset.engineCancelApproval = "";
    cancel.textContent = "Cancel";
    cancel.addEventListener("click", () => { this.clearReview(); this.notice(""); this.render(); });
    return cancel;
  }

  private clearReview(): void {
    this.review = undefined;
    this.approval = undefined;
    if (this.reviewPanel) {
      this.reviewPanel.replaceChildren();
      this.reviewPanel.hidden = true;
    }
  }

  /** The panel of the engine's review, inside the Validate page's application area; it is made once. */
  private reviewArea(): HTMLElement {
    if (this.reviewPanel) return this.reviewPanel;
    const panel = document.createElement("div");
    panel.className = "studio-engine-review";
    panel.hidden = true;
    (this.root.querySelector<HTMLElement>("[data-studio-application]") ?? this.root).append(panel);
    this.reviewPanel = panel;
    return panel;
  }
}

/**
 * The lines of a review: for Apply and Restore the mode, each parameter's running and candidate value and the validated operating
 * point (B6.2); for Keep the record, the values it keeps, the files it changes and the fields the equipment has no setting for (B6.5).
 */
function reviewLines(source: Record<string, unknown>, action: "apply" | "restore" | "keep"): string[] {
  if (action === "keep") {
    const values = isRecord(source.values) ? Object.entries(source.values).map(([field, value]) => `${field}: ${String(value)}`) : [];
    const files = Array.isArray(source.files)
      ? source.files.filter(isRecord).map(file => `Changes ${text(file.path)}`) : [];
    const unknown = Array.isArray(source.unknown) && source.unknown.length > 0
      ? [`Not kept, the equipment has no setting for: ${source.unknown.map(String).join(", ")}`] : [];
    return [`Record: ${text(source.record)}`, ...values, ...files, ...unknown];
  }
  const parameters = Array.isArray(source.parameters) ? source.parameters.filter(isRecord).map(parameter => {
    const unit = displayEngineeringUnit(text(parameter.unit));
    const verb = action === "restore" ? "returns to" : "writes";
    return `${text(parameter.symbol)}: ${String(parameter.running)} now, ${verb} ${String(parameter.next)}${unit ? ` ${unit}` : ""}`;
  }) : [];
  const point = isRecord(source.operatingPoint)
    ? `Validated operating point: inputs ${numbers(source.operatingPoint.inputs)}, outputs ${numbers(source.operatingPoint.outputs)}`
    : undefined;
  const mode = text(source.foundMode);
  const modeLine = `${action === "restore" ? "Returns to mode" : "Mode found"}: ${mode || "unknown"}`;
  return [modeLine, ...parameters, ...(point ? [point] : [])];
}

/** The approval block of a physical target as a list (A2.5, B9.3): the lines Rust fills, shown as they come. */
function approvalList(approval: Record<string, unknown>): HTMLElement {
  const list = document.createElement("ul");
  list.dataset.engineApprovalBlock = "";
  for (const line of approvalLines(approval)) {
    const item = document.createElement("li");
    item.textContent = line;
    list.append(item);
  }
  return list;
}

/**
 * The lines of an approval: the target and the loop, every signal written with its value and its declared limits, the duration of the
 * observation, the stop conditions and the restoration. A limit the loop does not declare is shown as not declared, never as a number.
 */
function approvalLines(approval: Record<string, unknown>): string[] {
  const lines = ["Target: physical", `Loop: ${text(approval.loopId)}`];
  const signals = Array.isArray(approval.signals) ? approval.signals.filter(isRecord) : [];
  for (const signal of signals) {
    const unit = displayEngineeringUnit(text(signal.unit));
    lines.push(`Writes ${text(signal.symbol)}: ${String(signal.next)}${unit ? ` ${unit}` : ""}, limits ${limit(signal.minimum)} to ${limit(signal.maximum)}`);
  }
  if (typeof approval.durationSeconds === "number") lines.push(`Observed for ${String(approval.durationSeconds)} s`);
  const stops = Array.isArray(approval.stopConditions) ? approval.stopConditions.map(String) : [];
  if (stops.length > 0) lines.push("Stop when:", ...stops);
  lines.push(`Restoration: ${text(approval.restoration)}`);
  return lines;
}

function limit(value: unknown): string {
  return typeof value === "number" && Number.isFinite(value) ? String(value) : "not declared";
}

function numbers(value: unknown): string {
  return Array.isArray(value) ? value.map(String).join(", ") : "";
}

function text(value: unknown): string {
  return typeof value === "string" ? value : "";
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}
