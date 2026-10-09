import type { StudioPost } from "./threePageOperation";
import { runLockedControls } from "./runLock";
import { plainReason } from "./studioFailure";

type RecoveryAction = "cancel" | "retry-recovery" | "rollback" | "review-runtime" | "confirm-rollback" | "confirm-runtime";

interface RecoveryProjection {
  action: RecoveryAction;
  label: string;
  kind: string;
}

/** Projects Engineer-owned recovery into the one Validate action area. */
export class ThreePageRecovery {
  active = false;
  busy = false;
  private projection?: RecoveryProjection;
  private readonly originalDisabled = new Map<HTMLElement, boolean>();
  private originalApplyHidden?: boolean;
  private originalApplicationHidden?: boolean;
  private readonly originalPrimaryHidden = new Map<HTMLButtonElement, boolean>();
  private readonly originalMessage = new Map<HTMLElement, { text: string; hidden: boolean }>();
  private reason = "";
  private action?: HTMLButtonElement;

  constructor(private readonly root: HTMLElement, private readonly post: StudioPost,
    private readonly operationBusy: () => boolean) {}

  receive(payload: Record<string, unknown>): void {
    if (this.isResolved(payload)) {
      this.clear();
      return;
    }
    const projection = this.projectionFor(payload);
    if (!projection) {
      if (this.active && ["failed", "unavailable", "busy"].includes(String(payload.status))) {
        this.busy = false;
        if (typeof payload.reason === "string") this.reason = plainReason(payload.reason);
        this.render();
      }
      return;
    }
    if (!this.active) this.captureAndLock();
    this.active = true;
    this.busy = false;
    this.projection = projection;
    this.reason = payload.reason === undefined ? "Recovery is required before continuing." : plainReason(String(payload.reason));
    this.render();
  }

  /** Call after ThreePageOperation/Application render to restore recovery precedence. */
  synchronize(): void { this.render(); }

  clear(): void {
    if (!this.active && this.originalDisabled.size === 0) return;
    this.active = false;
    this.busy = false;
    this.projection = undefined;
    this.reason = "";
    this.action?.remove();
    this.action = undefined;
    for (const [control, disabled] of this.originalDisabled) (control as HTMLInputElement).disabled = disabled;
    this.originalDisabled.clear();
    const apply = this.root.querySelector<HTMLButtonElement>("[data-engineer-apply]");
    if (apply && this.originalApplyHidden !== undefined) apply.hidden = this.originalApplyHidden;
    this.originalApplyHidden = undefined;
    const application = this.root.querySelector<HTMLElement>("[data-studio-application]");
    if (application && this.originalApplicationHidden !== undefined) application.hidden = this.originalApplicationHidden;
    this.originalApplicationHidden = undefined;
    for (const [button, hidden] of this.originalPrimaryHidden) button.hidden = hidden;
    this.originalPrimaryHidden.clear();
    for (const [message, original] of this.originalMessage) { message.textContent = original.text; message.hidden = original.hidden; }
    this.originalMessage.clear();
  }

  private projectionFor(payload: Record<string, unknown>): RecoveryProjection | undefined {
    const status = String(payload.status ?? "");
    if (["recovery-required", "failed", "restored"].includes(status) && payload.cancellationUnconfirmed === true) {
      return { action: "cancel", label: "Confirm cancellation", kind: "control-studio.engineer.cancel" };
    }
    if (status === "recovery-required" && payload.retryManualRecovery === true) {
      return { action: "retry-recovery", label: "Retry recovery", kind: "control-studio.engineer.retry-recovery" };
    }
    if (["recovery-required", "failed"].includes(status) && (payload.commissioningCheckpoint === true || (payload.commissioning as { recoveryConfirmed?: boolean } | undefined)?.recoveryConfirmed === false)) {
      return { action: "rollback", label: "Review restore", kind: "control-studio.engineer.rollback" };
    }
    if (status === "failed" && payload.replacementRuntimeAvailable === true) {
      return { action: "review-runtime", label: "Review restarted runtime", kind: "control-studio.engineer.review-runtime" };
    }
    if (this.active && status === "review" && payload.rollback === true && payload.applicationAction !== "restore") {
      return { action: "confirm-rollback", label: "Restore previous settings", kind: "control-studio.engineer.confirm" };
    }
    if (status === "review" && payload.runtimeReplacementReview === true) {
      return { action: "confirm-runtime", label: "Confirm new runtime", kind: "control-studio.engineer.confirm" };
    }
    return undefined;
  }

  private isResolved(payload: Record<string, unknown>): boolean {
    return payload.recoveryResolved === true || payload.status === "cancelled" &&
      payload.cancellationUnconfirmed !== true && payload.commissioningCheckpoint !== true;
  }

  private captureAndLock(): void {
    // Recovery locks every button as well as the run-locked fields; only its own recovery action stays available.
    const controls = new Set<HTMLElement>([...runLockedControls(this.root), ...Array.from(this.root.querySelectorAll<HTMLElement>("button"))]);
    for (const control of Array.from(controls)) {
      if (control.matches("[data-studio-recovery-action],[data-engineer-stage-tab]")) continue;
      if ("disabled" in control) this.originalDisabled.set(control, Boolean((control as HTMLInputElement).disabled));
    }
    this.lockControls();
    const primary = this.primaryAction();
    if (primary) { this.originalPrimaryHidden.set(primary, primary.hidden); primary.hidden = true; }
    const apply = this.root.querySelector<HTMLButtonElement>("[data-engineer-apply]");
    if (apply) {
      this.originalApplyHidden = apply.hidden;
      apply.hidden = true;
    }
    const application = this.root.querySelector<HTMLElement>("[data-studio-application]");
    if (application) {
      this.originalApplicationHidden = application.hidden;
      application.hidden = false;
    }
  }

  private lockControls(): void {
    for (const control of this.originalDisabled.keys()) (control as HTMLInputElement).disabled = true;
  }

  private render(): void {
    const area = this.actionArea();
    if (!area) return;
    if (!this.active || !this.projection) {
      this.action?.remove(); this.action = undefined;
      return;
    }
    this.lockControls();
    const primary = this.primaryAction();
    if (primary) {
      if (!this.originalPrimaryHidden.has(primary)) this.originalPrimaryHidden.set(primary, primary.hidden);
      primary.hidden = true;
    }
    const application = this.root.querySelector<HTMLElement>("[data-studio-application]");
    if (application && area === application) application.hidden = false;
    if (!this.action) {
      this.action = document.createElement("button");
      this.action.type = "button";
      this.action.dataset.studioRecoveryAction = "";
      this.action.className = "trust-button trust-button--primary";
      this.action.addEventListener("click", () => this.dispatch());
      area.prepend(this.action);
    }
    if (this.action.parentElement !== area) area.prepend(this.action);
    this.action.dataset.recoveryAction = this.projection.action;
    this.action.textContent = this.busy ? "Working…" : this.projection.label;
    this.action.disabled = this.busy || this.operationBusy();
    this.action.hidden = false;
    const message = this.root.querySelector<HTMLElement>(`[data-studio-message="${this.selectedPage()}"]`);
    if (message) {
      if (!this.originalMessage.has(message)) this.originalMessage.set(message, { text: message.textContent ?? "", hidden: message.hidden });
      message.textContent = this.reason; message.hidden = false;
    }
  }

  private selectedPage(): string { return this.root.dataset.selectedStage ?? "verify"; }
  private actionArea(): HTMLElement | null {
    return this.root.querySelector<HTMLElement>(`[data-studio-page="${this.selectedPage()}"] .studio-actions`)
      ?? this.root.querySelector<HTMLElement>("[data-studio-application]");
  }
  private primaryAction(): HTMLButtonElement | null {
    const selector = this.selectedPage() === "model" ? "[data-engineer-find]" : this.selectedPage() === "tune" ? "[data-engineer-tune]" : "[data-engineer-verify]";
    return this.root.querySelector<HTMLButtonElement>(selector);
  }

  private dispatch(): void {
    if (!this.projection || this.busy || this.operationBusy()) return;
    this.busy = true;
    this.render();
    this.post(this.projection.kind);
  }
}
