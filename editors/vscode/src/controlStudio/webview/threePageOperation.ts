import type { StudioPage } from "./threePageHostState";
import { runLockedControls } from "./runLock";
import { nextActionHasButton, studioFailureOf, type StudioFailure, type StudioNextAction } from "./studioFailure";

export type StudioStart = "find" | "tune" | "verify";
export type StudioPost = (kind: string, payload?: Record<string, unknown>) => void;
const pages: Record<StudioStart, StudioPage> = { find: "model", tune: "tune", verify: "verify" };
const labels = { find: "Identify plant", tune: "Tune controller", verify: "Validate controller" };
const PAGE_IDS: readonly StudioPage[] = ["model", "tune", "verify"];

/**
 * Displays one host-owned operation across all three pages. Navigation never starts work. The page holds the state of its own requests
 * in flight (a Reset, a recording import, an operation) and the words of their progress. A stage's action is enabled from the engine's
 * stage answer (`available` on the stage tab, B9.4) together with those requests and the gates of other modules, and a Reset is sent for
 * the engine to take or refuse with its own sentence and next action.
 */
export class ThreePageOperation {
  busy = false;
  /** The engine is checking a recording the engineer opened (B3.6): the page stays busy until the engine answers, with no run's Abort. */
  importing = false;
  private follows = true;
  private resetting = false;
  private page: StudioPage = "model";
  private resetConfirm?: HTMLElement;
  private readonly pendingActions = new Map<StudioPage, StudioNextAction>();
  constructor(private readonly root: HTMLElement, private readonly post: StudioPost,
    private readonly select: (page: StudioPage, persist: boolean) => void,
    private readonly missing: (field: string) => void,
    /** Gates owned by other modules, such as an incomplete plant mapping or a simulator start in progress. */
    private readonly external: () => string | undefined = () => undefined,
    /** Performs a failure's typed next action on its own page; it never starts work by itself. */
    private readonly nextAction: (kind: string, page: StudioPage) => void = () => undefined,
    /** Re-renders the Apply button after this module's state changed: the application owns that button (threePageApplication.ts). */
    private readonly changed: () => void = () => undefined,
    /** The words of the Identify button for the method chosen now: the project test's or the recorded-data fit's own words, else Identify plant. */
    private readonly findLabel: () => string = () => labels.find) {
    for (const page of PAGE_IDS) {
      this.root.querySelector<HTMLElement>(`[data-studio-message="${page}"]`)?.addEventListener("click", event => {
        const target = event.target as HTMLElement | null;
        const sentence = target && typeof target.closest === "function" ? target.closest<HTMLElement>(".studio-failure-sentence") : null;
        const detail = this.detail(page);
        if (!sentence || !detail) return;
        detail.hidden = !detail.hidden;
        sentence.setAttribute("aria-expanded", String(!detail.hidden));
      });
      this.failureAction(page)?.addEventListener("click", () => {
        const action = this.pendingActions.get(page);
        if (action) this.nextAction(action.kind, page);
      });
    }
  }

  reset(): void {
    if (this.busy || this.resetting) return;
    this.resetting = true; this.render();
    this.post("control-studio.study.reset");
  }
  /**
   * Reset starts a new working study and moves the current one, with its model, tuned controller and validation results, to the study's
   * history (A8.4), so the engineer confirms it in the page; window.confirm does not work in a webview. The confirmed Reset is sent, and
   * the engine refuses it while an operation runs or an Apply's checkpoint waits for its Restore, with its own sentence (B6.7). The button
   * is one of the run-locked controls, so it is disabled only while a request of the page is in flight.
   */
  bindReset(button: HTMLButtonElement): void {
    const doc = this.root.ownerDocument;
    const row = doc.createElement("div");
    row.className = "studio-reset-confirm"; row.hidden = true; row.setAttribute("role", "group"); row.setAttribute("aria-label", "Confirm reset");
    const text = doc.createElement("span");
    text.textContent = "Reset starts a new study. The model, the tuned controller and the validation results move to the study's history.";
    const confirm = doc.createElement("button"); confirm.type = "button"; confirm.className = "trust-button"; confirm.textContent = "Reset study";
    const keep = doc.createElement("button"); keep.type = "button"; keep.className = "trust-button"; keep.textContent = "Keep";
    row.append(text, confirm, keep);
    button.after(row);
    this.resetConfirm = row;
    button.addEventListener("click", () => { row.hidden = false; keep.focus(); });
    keep.addEventListener("click", () => { row.hidden = true; button.focus(); });
    confirm.addEventListener("click", () => { row.hidden = true; this.reset(); });
  }
  /**
   * Why an idle stage action must stay unavailable while a request of the page is pending or another gate holds it, or undefined when
   * every gate agrees: a Reset in progress, or the reason of an external gate such as an incomplete plant mapping or a simulator start
   * in progress. Which stage may start is the engine's (`available`, B9.4).
   */
  stageBlock(): string | undefined {
    if (this.busy) return undefined; // Abort stays reachable while work runs.
    return this.resetting ? "Reset is in progress." : this.external();
  }
  navigated(): void { if (this.busy) this.follows = false; }
  start(from: StudioStart, payload: Record<string, unknown>): void {
    if (this.busy) { this.post("control-studio.flow.cancel"); this.notice(this.page, "Aborting…"); return; }
    this.busy = true; this.follows = true; this.page = pages[from];
    this.select(this.page, true);
    this.render();
    this.notice(this.page, this.progress());
    this.post("control-studio.flow.start", { ...payload, from });
  }
  receive(stage: string, payload: Record<string, unknown>): void {
    const status = String(payload.status);
    // A failed or reset study ends the page's Reset. The engine answers a reset in place (the page is not reloaded), so its answer ends it here.
    // A failure also ends a recording import the engine did not answer (the engine could not be reached).
    if (status === "failed" || status === "reset") { this.resetting = false; this.importing = false; }
    if (status === "reset") this.notice("model", "");
    if (payload.flowStage === "Identify") this.page = "model";
    if (payload.flowStage === "Tune") this.page = "tune";
    if (payload.flowStage === "Validate") this.page = "verify";
    if (status === "candidate" || status === "verified") this.notice(this.page, "");
    if (status === "candidate" || status === "verified") this.notice("verify", "");
    // The engine's automatic flow sends its progress without a flow stage (`run`): it shows on the page the flow started on.
    if (status === "running" && (payload.flowStage || stage === "flow")) {
      this.busy = true;
      if (this.follows) this.select(this.page, true);
      // The host's own progress sentence, such as the settling time left, replaces the generic one when it sends one.
      this.notice(this.page, typeof payload.reason === "string" && payload.reason ? payload.reason : this.progress());
    }
    if (stage === "flow" && ["paused", "completed", "cancelled"].includes(status)) {
      this.busy = false;
      if (this.follows) this.select(this.page, true);
      // A pause or a cancellation shows the engine's sentence (`reason`); the page writes none.
      this.notice(this.page, status !== "completed" && typeof payload.reason === "string" ? payload.reason : "");
    } else if (!payload.flowStage && ["failed", "verification-failed"].includes(status)) {
      this.busy = false;
      // A failure that names the page it belongs to (a refused Channels… selection belongs to Tune) shows there; any other on the operation's
      // page. The sentence is the engine's failure; an answer without one, such as a Validate answer that offers no Apply, shows none.
      this.show(PAGE_IDS.find(page => page === payload.page) ?? this.page, payload);
    }
    // A change that left the validation not current (A8.4) says so on the Validate page, in the words the answer carries.
    if (status === "edited" && typeof payload.reason === "string" && payload.reason) this.notice("verify", payload.reason);
    this.render();
    if (stage === "flow" && status === "paused" && typeof payload.missing === "string") this.missing(payload.missing);
  }
  /** A plain message in a page's action area. It replaces any failure shown there. */
  notice(page: StudioPage, message: string): void {
    const area = this.message(page);
    area.textContent = message; area.hidden = !message;
    this.clearFailure(page);
  }
  /** A failure: one sentence, its technical reason in place when the sentence is selected, and at most one button for its fix. */
  failure(page: StudioPage, failure: StudioFailure): void {
    const area = this.message(page);
    const doc = this.root.ownerDocument;
    area.hidden = false;
    if (failure.detail) {
      const sentence = doc.createElement("button");
      sentence.type = "button"; sentence.className = "studio-failure-sentence"; sentence.textContent = failure.message;
      sentence.setAttribute("aria-expanded", "false"); sentence.setAttribute("aria-controls", `studio-failure-detail-${page}`);
      area.replaceChildren(sentence);
    } else area.textContent = failure.message;
    const detail = this.detail(page);
    if (detail) { detail.textContent = failure.detail ?? ""; detail.hidden = true; }
    const action = this.failureAction(page);
    if (action) {
      const button = nextActionHasButton(failure.nextAction);
      action.hidden = !button;
      action.textContent = button ? failure.nextAction.label : "";
      if (button) this.pendingActions.set(page, failure.nextAction); else this.pendingActions.delete(page);
    }
  }
  /** Shows the engine's failure that `payload` carries on `page`, or clears the page's message when it carries none. */
  private show(page: StudioPage, payload: Record<string, unknown>): void {
    const failure = studioFailureOf(payload);
    if (failure) this.failure(page, failure);
    else this.notice(page, "");
  }
  private clearFailure(page: StudioPage): void {
    const detail = this.detail(page);
    if (detail) { detail.textContent = ""; detail.hidden = true; }
    const action = this.failureAction(page);
    if (action) { action.hidden = true; action.textContent = ""; }
    this.pendingActions.delete(page);
  }
  private message(page: StudioPage): HTMLElement { return this.root.querySelector<HTMLElement>(`[data-studio-message="${page}"]`)!; }
  private detail(page: StudioPage): HTMLElement | null { return this.root.querySelector<HTMLElement>(`[data-studio-failure-detail="${page}"]`); }
  private failureAction(page: StudioPage): HTMLButtonElement | null { return this.root.querySelector<HTMLButtonElement>(`[data-studio-failure-action="${page}"]`); }
  private progress(): string { return this.page === "model" ? "Identifying plant…" : this.page === "tune" ? "Tuning controller…" : "Validating controller…"; }
  /**
   * Whether the engine says the stage's own action can start now (B9.4): the stage tab keeps the answer (`applyStageStatuses`). Tune and
   * Validate start only once an answer says so. Identify, which a bound loop can always start, waits while an answer says it cannot, as
   * while an Apply's checkpoint waits for its Restore (B6.4); before the engine's open answer nothing is sent from the page anyway.
   */
  private available(action: StudioStart): boolean {
    const answer = this.root.querySelector<HTMLElement>(`[data-engineer-stage-tab="${pages[action]}"]`)?.dataset.available;
    return action === "find" ? answer !== "false" : answer === "true";
  }
  synchronize(): void { this.render(); }
  private render(): void {
    const blocked = this.stageBlock() !== undefined;
    for (const action of ["find", "tune", "verify"] as const) {
      const button = this.root.querySelector<HTMLButtonElement>(`[data-engineer-${action}]`)!;
      // The Identify button's words belong to the method chosen (the project test's, the recorded-data fit's); an analysis chosen on
      // Tune sets its own button, so the stage's gate does not apply to it.
      button.textContent = this.busy ? "Abort" : action === "find" ? this.findLabel() : labels[action];
      const analysis = action === "tune" && this.root.dataset.analysisSelected === "true";
      const unavailable = !analysis && !this.available(action);
      button.disabled = blocked || (!this.busy && unavailable);
    }
    // The idle lock never re-enables an import that the bindings gate or another gate has closed. The editors stay usable while an Apply's
    // checkpoint waits for its Restore: the engine refuses a change of what the stages read, and Reset, with its reason (B6.4).
    for (const control of runLockedControls(this.root)) {
      control.disabled = this.busy || this.importing || this.resetting || (blocked && control.matches("[data-identify-import]"));
    }
    if (this.resetConfirm && (this.busy || this.resetting)) this.resetConfirm.hidden = true;
    this.root.dataset.busy = String(this.busy || this.importing || this.resetting);
    this.changed();
  }
}
