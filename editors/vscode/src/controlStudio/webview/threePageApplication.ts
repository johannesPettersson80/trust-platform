import type { StudioPost } from "./threePageOperation";
import { runLockedControls } from "./runLock";
import { studioFailureOf, type StudioFailure } from "./studioFailure";

/** The host owns review, writes and receipts; this class only presents their progress. After an Apply, Keep in project is the
 * next action: the applied values become the project's, so a restart or a new download keeps them (Specification 35 §17.17). */
export class ThreePageApplication {
  active = false;
  private action: "apply" | "restore" = "apply";
  private applyAvailable = false;
  private restoreAvailable = false;
  private keepAvailable = false;
  private keeping = false;
  private readonly button: HTMLButtonElement;
  private readonly keep: HTMLButtonElement | null;
  constructor(private readonly root: HTMLElement, private readonly post: StudioPost,
    private readonly operationBusy: () => boolean, private readonly notice: (message: string) => void,
    /** Shows a failure with its typed next action in the Validate page's action area. */
    private readonly fail: (failure: StudioFailure) => void = failure => notice(failure.message)) {
    this.button = root.querySelector<HTMLButtonElement>("[data-engineer-apply]")!;
    this.button.addEventListener("click", () => {
      if (this.active || this.keeping || !this.available() || this.operationBusy()) return;
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
    const status = String(payload.status);
    if (payload.applicationAction === "keep") {
      this.keeping = false;
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
    this.button.disabled = this.active || this.keeping || this.operationBusy() || !this.available();
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
}
