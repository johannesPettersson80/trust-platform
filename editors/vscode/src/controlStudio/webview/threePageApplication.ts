import type { StudioPost } from "./threePageOperation";

/** The host owns review, writes and receipts; this class only presents their progress. */
export class ThreePageApplication {
  active = false;
  private action: "apply" | "restore" = "apply";
  private applyAvailable = false;
  private restoreAvailable = false;
  private readonly button: HTMLButtonElement;
  constructor(private readonly root: HTMLElement, private readonly post: StudioPost,
    private readonly operationBusy: () => boolean, private readonly notice: (message: string) => void) {
    this.button = root.querySelector<HTMLButtonElement>("[data-engineer-apply]")!;
    this.button.addEventListener("click", () => {
      if (this.active || !this.available() || this.operationBusy()) return;
      this.active = true; this.notice(""); this.render();
      this.post(`control-studio.application.${this.action}`);
    });
  }
  abort(): void {
    if (!this.active) return;
    this.notice("Aborting…"); this.post("control-studio.engineer.cancel");
  }
  receive(payload: Record<string, unknown>): void {
    const status = String(payload.status);
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
        this.action = restoring ? "apply" : "restore";
        this.notice(restoring ? "Restored" : "Applied");
      } else if (["failed", "recovery-required", "unavailable", "busy"].includes(status)) {
        const restoring = (payload.applicationAction ?? this.action) === "restore";
        this.active = false;
        if (restoring) this.restoreAvailable = false;
        else this.applyAvailable = false;
        this.notice(String(payload.reason ?? "The operation could not be confirmed."));
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
    this.button.disabled = this.active || this.operationBusy() || !this.available();
    if (!this.active) return;
    for (const control of Array.from(this.root.querySelectorAll<HTMLInputElement | HTMLSelectElement | HTMLButtonElement>("select,input,[data-studio-limits],[data-studio-requirements],[data-identify-import]"))) control.disabled = true;
    for (const button of Array.from(this.root.querySelectorAll<HTMLButtonElement>("[data-engineer-find],[data-engineer-tune],[data-engineer-verify]"))) { button.textContent = "Abort"; button.disabled = false; }
  }
}
