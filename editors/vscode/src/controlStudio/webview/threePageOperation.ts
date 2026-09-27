import type { StudioPage } from "./threePageHostState";

export type StudioStart = "find" | "recording" | "tune" | "verify";
export type StudioPost = (kind: string, payload?: Record<string, unknown>) => void;
const pages: Record<StudioStart, StudioPage> = { find: "model", recording: "model", tune: "tune", verify: "verify" };
const labels = { find: "Identify plant", tune: "Tune controller", verify: "Validate controller" };

/** Displays one host-owned operation across all three pages. Navigation never starts work. */
export class ThreePageOperation {
  busy = false;
  private follows = true;
  private resetting = false;
  private hasModel = false;
  private hasCandidate = false;
  private passed = false;
  private page: StudioPage = "model";
  constructor(private readonly root: HTMLElement, private readonly post: StudioPost,
    private readonly select: (page: StudioPage, persist: boolean) => void,
    private readonly missing: (field: string) => void) {}

  reset(): void {
    if (this.busy || this.resetting) return;
    this.resetting = true; this.render();
    this.post("control-studio.study.reset");
  }
  navigated(): void { if (this.busy) this.follows = false; }
  start(from: StudioStart, payload: Record<string, unknown>): void {
    if (this.busy) { this.post("control-studio.flow.cancel"); this.notice(this.page, "Aborting…"); return; }
    this.busy = true; this.follows = true; this.page = pages[from]; this.passed = false;
    this.select(this.page, true);
    this.render();
    this.notice(this.page, this.progress());
    this.post("control-studio.flow.start", { ...payload, from });
  }
  receive(stage: string, payload: Record<string, unknown>): void {
    const status = String(payload.status);
    if (status === "failed") this.resetting = false;
    if (payload.flowStage === "Identify") this.page = "model";
    if (payload.flowStage === "Tune") this.page = "tune";
    if (payload.flowStage === "Validate") this.page = "verify";
    if (payload.stageEvidence && typeof payload.stageEvidence === "object") {
      const evidence = payload.stageEvidence as Record<string, unknown>;
      this.hasModel = evidence.hasModel === true;
      this.hasCandidate = evidence.hasCandidate === true;
      this.passed = evidence.validationPassed === true;
    }
    if (!payload.stageEvidence && (payload.model || status === "model-ready")) this.hasModel = payload.compatibleControllerRequired !== true;
    if (status === "candidate") this.hasCandidate = true;
    if (status === "verified") this.passed = true;
    if (["model-ready", "candidate", "verified"].includes(status)) this.notice(this.page, "");
    if (status === "model-ready") this.notice("tune", "");
    if (status === "candidate" || status === "verified") this.notice("verify", "");
    if (status === "running" && payload.flowStage) {
      this.busy = true;
      if (this.follows) this.select(this.page, true);
      this.notice(this.page, this.progress());
    }
    if (status === "running" && payload.activity === "model-check") this.notice(this.page, "Checking model…");
    if (stage === "flow" && ["paused", "completed", "cancelled"].includes(status)) {
      this.busy = false;
      if (this.follows) this.select(this.page, true);
      this.notice(this.page, status === "completed" ? "" : String(payload.reason ?? "Operation stopped."));
    } else if (!payload.flowStage && ["failed", "verification-failed", "unavailable", "recovery-required"].includes(status)) {
      this.busy = false;
      this.notice(this.page, String(payload.reason ?? "The operation could not finish."));
    }
    if (status === "edited") { this.hasCandidate = false; this.passed = false; if (this.hasModel) this.notice("verify", "Tune again to validate the changed settings."); }
    if (status === "restored" && this.hasModel) this.notice("verify", "Tune again to validate new settings.");
    this.render();
    if (stage === "flow" && status === "paused" && typeof payload.missing === "string") this.missing(payload.missing);
  }
  notice(page: StudioPage, message: string): void {
    const area = this.root.querySelector<HTMLElement>(`[data-studio-message="${page}"]`)!;
    area.textContent = message; area.hidden = !message;
  }
  private progress(): string { return this.page === "model" ? "Identifying plant…" : this.page === "tune" ? "Tuning controller…" : "Validating controller…"; }
  synchronize(): void { this.render(); }
  private render(): void {
    for (const action of ["find", "tune", "verify"] as const) {
      const button = this.root.querySelector<HTMLButtonElement>(`[data-engineer-${action}]`)!;
      button.textContent = this.busy ? "Abort" : labels[action];
      button.disabled = this.resetting || !this.busy && (action === "tune" ? !this.hasModel : action === "verify" ? !this.hasCandidate : false);
    }
    for (const control of Array.from(this.root.querySelectorAll<HTMLInputElement | HTMLSelectElement | HTMLButtonElement>("select,input,[data-studio-limits],[data-studio-requirements],[data-identify-import],[data-studio-reset-chart]"))) control.disabled = this.busy || this.resetting;
    this.root.querySelector<HTMLButtonElement>("[data-engineer-apply]")!.disabled = this.busy || this.resetting || !this.passed;
    this.root.dataset.busy = String(this.busy || this.resetting);
  }
}
