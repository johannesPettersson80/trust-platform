import type { StudioPost } from "./threePageOperation";
import { runLockedControls } from "./runLock";
import { studioFailureOf, type StudioFailure } from "./studioFailure";
import { displayEngineeringUnit } from "../unitDisplay";
import type { PrimaryAction } from "../generated/OpenResult";

/** The three reviewed actions, named as the engine's methods name them (specification 35, B6). */
type ReviewedAction = "apply" | "restore" | "keep";

/** A review the engine answered, which the page shows until the engineer confirms it or the page leaves it (B6.1, B6.5). */
interface EngineReview {
  readonly action: ReviewedAction;
  readonly identity: string;
}

/** The words of each confirmation button (B9.3). */
const CONFIRM_WORDS = { apply: "Confirm Apply", restore: "Confirm restore", keep: "Confirm Keep in project" } as const;

/**
 * What the engine last said the Validate page offers besides Apply (B6.4, B6.5, B7, A8.6), taken from the answer that carried it: the
 * open answer, an inputs answer, a Validate answer, the end of an Apply, a Restore or a Keep. The page replaces each flag an answer
 * carries, and the action that holds the position, which every such answer states. Apply itself follows the Apply stage (B9.4).
 */
interface ApplicationOffers {
  restore: boolean;
  keep: boolean;
  /** The action that holds the position (`primaryAction`, B7); undefined when the engine offers none. */
  primary?: PrimaryAction;
}

/**
 * The Validate page's Apply area. The host owns review, writes and receipts; this class presents their progress. It holds the engine's
 * offers and the state of the request it has in flight (a press waiting for its review, a review on the screen, a confirmed operation).
 * The engine states which actions are offered and which one holds the position (`primaryAction`), whether a review needs the engineer's
 * approval, and what is offered after an Apply, a Restore or a Keep (Specification 35 B6, A2.5, B7); the class puts the action the
 * engine names in the position, enables Restore and Keep in project while the engine offers them and Apply while the engine's Apply
 * stage can start, which in every answer that states the offers is exactly when Apply is offered (B7, B9.4), and sends the request of
 * the button that is pressed.
 */
export class ThreePageApplication {
  /** An Apply or a Restore is in flight: from its press to the engine's end of it (view state of the request). */
  active = false;
  /** The action in flight, which the button names while it runs. */
  private inFlight: "apply" | "restore" = "apply";
  private keeping = false;
  private readonly offers: ApplicationOffers = { restore: false, keep: false };
  /** The engine's review on the screen, its panel and the approval box the engineer ticks when the review asks for one (A2.5). */
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
    /** The gates of the page's own requests in flight that hold Apply and Restore: a reset in progress, an external block such as
     * invalid bindings, or a running experiment. The application is the only writer of the button's enabled state. */
    private readonly stageBlocked: () => boolean = () => false) {
    this.button = root.querySelector<HTMLButtonElement>("[data-engineer-apply]")!;
    this.button.addEventListener("click", () => {
      const action = this.positionAction();
      if (this.active || this.keeping || !this.offered(action) || this.operationBusy() || this.stageBlocked()) return;
      this.begin(action);
      this.post(`control-studio.application.${action}`);
    });
    this.keep = root.querySelector<HTMLButtonElement>("[data-engineer-keep]");
    this.keep?.addEventListener("click", () => {
      if (this.active || this.keeping || !this.offers.keep || this.operationBusy()) return;
      this.keeping = true; this.notice(""); this.render();
      this.post("control-studio.application.keep");
    });
  }
  abort(): void {
    if (!this.active) return;
    this.notice("Aborting…"); this.post("control-studio.engineer.cancel");
  }
  /**
   * Asks the engine for the Restore review: the control of a failure whose typed next action is Restore previous settings (A11.2, B6.4).
   * The review, or the engine's refusal, comes back to this area, as after a press of Restore in the position.
   */
  restore(): void {
    if (this.active || this.keeping || this.operationBusy()) return;
    this.begin("restore");
    this.post("control-studio.application.restore");
  }

  /**
   * Takes one engine answer relayed to the engineer stage: the open answer, a review, the offers of a Validate answer, the end of an
   * Apply, a Restore or a Keep, and the end of the request the page has in flight.
   */
  receive(payload: Record<string, unknown>): void {
    if (payload.status === "opened") { this.showOpened(payload); return; }
    if (isRecord(payload.engineReview) || isRecord(payload.engineKeepReview)) { this.showReview(payload); return; }
    if (payload.status === "offers") { this.takeOffers(payload.engineOutcome); this.render(); return; }
    if (isRecord(payload.engineOutcome)) { this.showOutcome(payload); return; }
    const status = String(payload.status);
    const named = applicationActionOf(payload.applicationAction);
    if (status === "cancelled" && (named || this.active || this.keeping)) {
      // The engineer cancelled the operation: its button is free again, and the engine's words say how it ended (B9.4).
      this.active = false;
      this.keeping = false;
      this.notice(text(payload.reason));
      this.render();
      return;
    }
    if (status === "failed" && (this.active || this.keeping)) { this.showFailure(payload); return; }
    if (named && status === "running") {
      // For example while Apply waits for the process to settle at the validated operating point, or while a Keep writes.
      if (named === "keep") this.keeping = true;
      if (typeof payload.reason === "string") this.notice(payload.reason);
    }
    this.render();
  }
  synchronize(): void { this.render(); }

  /**
   * Replaces the offers with the engine's statement in an answer: each flag it carries (a flag it does not carry stays as it was), and
   * the action that holds the position, which every answer that states offers names, or leaves out when none is offered. The answer's
   * Apply offer is its Apply stage's `available`, which the page reads from the stage (B7, B9.4).
   */
  private takeOffers(source: unknown): void {
    if (!isRecord(source)) return;
    if (typeof source.restoreAvailable === "boolean") this.offers.restore = source.restoreAvailable;
    if (typeof source.keepAvailable === "boolean") this.offers.keep = source.keepAvailable;
    this.offers.primary = primaryActionOf(source.primaryAction);
  }

  /**
   * The action in the Validate page's one position, as the engine's `primaryAction` names it (B7, the Validate page specification: Apply
   * and Restore never compete): Apply, or Restore previous settings, which keeps the position beside a primary Keep in project. With
   * nothing offered the position shows Apply, unavailable.
   */
  private positionAction(): "apply" | "restore" {
    return this.offers.primary === "restore" || this.offers.primary === "keep" ? "restore" : "apply";
  }
  /** Whether the engine offers `action`: Restore as the offers say, Apply while the engine's Apply stage can start (B7, B9.4). */
  private offered(action: "apply" | "restore"): boolean {
    return action === "apply" ? this.applyStageAvailable() : this.offers.restore;
  }
  /**
   * Whether the engine's Apply stage can start now (B9.4): the application area keeps the stage answer (`applyStageStatuses`), which every
   * answer with stages renews; in every answer that states the offers it is exactly the Apply offer (B7). Before the first stage answer
   * it holds none, and Apply waits.
   */
  private applyStageAvailable(): boolean {
    return this.root.querySelector<HTMLElement>("[data-studio-application]")?.dataset.available === "true";
  }
  /** Starts the page's wait for an Apply or a Restore: its button names the action, and nothing else is offered meanwhile. */
  private begin(action: "apply" | "restore"): void {
    this.active = true;
    this.inFlight = action;
    this.notice(""); this.render();
  }
  private render(): void {
    const position = this.positionAction();
    this.button.textContent = this.active ? this.inFlight === "restore" ? "Restoring…" : "Applying…"
      : position === "restore" ? "Restore previous settings" : "Apply";
    this.button.disabled = this.active || this.keeping || this.operationBusy() || this.stageBlocked() || !this.offered(position);
    // The one primary action is the engine's (`primaryAction`): Keep in project when it names Keep, else the position's action.
    const keepPrimary = this.offers.primary === "keep";
    this.button.classList.toggle("trust-button--primary", !keepPrimary);
    if (this.keep) {
      this.keep.hidden = !(this.offers.keep || this.keeping);
      this.keep.classList.toggle("trust-button--primary", keepPrimary);
      this.keep.textContent = this.keeping ? "Keeping…" : "Keep in project";
      this.keep.disabled = this.keeping || this.active || this.operationBusy();
    }
    if (!this.active) return;
    for (const control of runLockedControls(this.root)) control.disabled = true;
    for (const button of Array.from(this.root.querySelectorAll<HTMLButtonElement>("[data-engineer-find],[data-engineer-tune],[data-engineer-verify]"))) { button.textContent = "Abort"; button.disabled = false; }
  }

  /** Shows the failure that ends the request in flight, with the engine's words and next action (B6.7); a Resume still offered is shown again. */
  private showFailure(payload: Record<string, unknown>): void {
    const keeping = this.keeping;
    this.active = false;
    this.keeping = false;
    const failure = studioFailureOf(payload);
    if (failure) this.fail(failure); else this.notice("");
    if (!keeping && this.resumable) this.showResume();
    this.render();
  }

  /**
   * Shows the engine's review of a Keep, or of an Apply or a Restore, and its confirmation: the confirmation sends the review's identity,
   * and the engine refuses it when the runtime or the project changed since the review (§0.1b, B6.7). How the review is confirmed is the
   * review's own `confirmation` (A2.5, B9.3): on a simulated target the press of Apply or Restore confirms it at once and the page shows
   * no confirmation, a physical target needs the engineer's approval first, and a Keep is confirmed with its own button.
   */
  private showReview(payload: Record<string, unknown>): void {
    const keep = isRecord(payload.engineKeepReview);
    const source = (keep ? payload.engineKeepReview : payload.engineReview) as Record<string, unknown>;
    const action: ReviewedAction = keep ? "keep" : source.action === "restore" ? "restore" : "apply";
    const identity = text(source.identity);
    if (source.confirmation === "press" && action !== "keep") {
      this.clearReview();
      this.keeping = false;
      this.begin(action);
      this.post(`control-studio.application.${action}`, { review: identity, physicalApproved: false });
      return;
    }
    this.active = false;
    this.keeping = false;
    this.review = { action, identity };
    const needsApproval = source.confirmation === "approval" && action !== "keep";
    const children: Node[] = [];
    const heading = document.createElement("p");
    heading.textContent = text(source.description);
    children.push(heading);
    const list = document.createElement("ul");
    for (const line of reviewLines(source, action)) {
      const item = document.createElement("li");
      item.textContent = line;
      list.append(item);
    }
    children.push(list);
    // The approval block of a physical target: what the engine's review says the approval covers (A2.5, B9.3).
    if (needsApproval && isRecord(source.approval)) children.push(approvalList(source.approval));
    const confirm = document.createElement("button");
    confirm.type = "button";
    confirm.className = "trust-button trust-button--primary";
    confirm.dataset.engineConfirm = action;
    confirm.textContent = CONFIRM_WORDS[action];
    confirm.addEventListener("click", () => this.confirmReview());
    if (needsApproval) {
      const box = document.createElement("input");
      box.type = "checkbox";
      box.dataset.engineApproval = "";
      box.addEventListener("change", () => { confirm.disabled = !box.checked; });
      const label = document.createElement("label");
      label.append(box, " Approve this write to the physical target");
      this.approval = box;
      confirm.disabled = true;
      children.push(label, this.cancelButton());
    }
    children.push(confirm);
    const panel = this.reviewArea();
    panel.replaceChildren(...children);
    panel.hidden = false;
    this.render();
  }

  /**
   * The engineer's confirmation of the review on the screen; it runs the reviewed change under the review's identity, with the approval the
   * engineer gave. The Confirm button waits for the approval box, and the engine refuses a write to a physical target without it (A2.4).
   */
  private confirmReview(): void {
    const review = this.review;
    if (!review || this.active || this.keeping || this.operationBusy()) return;
    const approved = this.approval?.checked === true;
    this.clearReview();
    if (review.action === "keep") this.keeping = true;
    else { this.active = true; this.inFlight = review.action; }
    this.notice(""); this.render();
    this.post(`control-studio.application.${review.action}`, { review: review.identity, physicalApproved: approved });
  }

  /** Shows the end of a confirmed Apply, Restore or Keep with the offers the engine answered (B6.3, B6.4, B6.5, B7). */
  private showOutcome(payload: Record<string, unknown>): void {
    this.clearReview();
    // The operation that the observation was resumed as has ended, so the observation is no longer waiting for the page.
    this.resumable = false;
    this.active = false;
    this.keeping = false;
    this.takeOffers(payload.engineOutcome);
    if (payload.status === "failed") {
      const failure = studioFailureOf(payload);
      if (failure) this.fail(failure); else this.notice("");
    } else if (typeof payload.reason === "string") {
      this.notice(payload.reason);
    }
    this.render();
  }

  /**
   * The Apply offers of an opened loop (A8.6, B7): Apply, Restore and Keep in project as the engine answers them, and the journaled Apply
   * observation the engine offers to resume.
   */
  private showOpened(payload: Record<string, unknown>): void {
    this.clearReview();
    this.active = false;
    this.keeping = false;
    this.takeOffers(payload.engineOutcome);
    this.resumable = payload.applyObservationResumable === true;
    if (this.resumable) this.showResume();
    this.render();
  }

  /**
   * The button that follows the interrupted observation to its end (A8.5, A8.6); Restore stays offered beside it. The engine's reason,
   * which the stage statuses carry while the observation waits, says why on the page (B6.4).
   */
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
    const panel = this.reviewArea();
    panel.replaceChildren(this.resumeButton);
    panel.hidden = false;
  }

  /** Follows the journaled observation to its end: the engine's Apply with `resume`, which runs no write (A8.5). */
  private resumeObservation(): void {
    if (!this.resumable || this.active || this.keeping || this.operationBusy()) return;
    this.clearReview();
    this.begin("apply");
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

/** The reviewed action an answer names, when it names one. */
function applicationActionOf(value: unknown): ReviewedAction | undefined {
  return value === "apply" || value === "restore" || value === "keep" ? value : undefined;
}

/** The engine's `primaryAction` as the generated type names it; undefined when the answer names none. */
function primaryActionOf(value: unknown): PrimaryAction | undefined {
  return value === "apply" || value === "restore" || value === "keep" ? value : undefined;
}

/**
 * The lines of a review: for Apply and Restore the mode, each parameter's running and candidate value and the validated operating
 * point (B6.2); for Keep the record, the values it keeps, the files it changes and the fields the equipment has no setting for (B6.5).
 */
function reviewLines(source: Record<string, unknown>, action: ReviewedAction): string[] {
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

/** The approval block of a physical target as a list (A2.5, B9.3): the lines `approvalLines` writes from the fields of the engine's approval. */
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
