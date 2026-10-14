import type { ControlLoopDefinition } from "../loopDefinition";
import type { ControllerTuningPreferences } from "../controllerTuningPreferences";
import type { StudioPost } from "./threePageOperation";
import { initializeManualTuneSettings } from "./manualTuneSettings";
import { renderTunedController } from "./threePageResults";
import { initializeThreePageStateDesign } from "./threePageStateDesign";

/** How long a manual edit waits for a pause before its predicted response is requested (Specification 35 B9.9). */
export const MANUAL_PREVIEW_DELAY_MS = 300;

/** Editing preferences stay separate from the host's validated candidate. */
export class ThreePageTuning {
  private values: Record<string, number> = {};
  private pins: Record<string, number> = {};
  private pending: string | undefined;
  private hasSettings = false;
  private design: Record<string, unknown> | undefined;
  /** The host's margins for the candidate (Specification 35 §17.10); a manual edit makes them stale. */
  private robustness: unknown;
  /** The engine's notes on the candidate: its method in words, its outcome and why its margins are missing (B4.4 to B4.6). */
  private notes: Record<string, unknown> = {};
  private inputRevision = -1;
  private lastMethod: ControllerTuningPreferences["method"] = "automatic";
  private previewTimer: ReturnType<typeof setTimeout> | undefined;
  private readonly fields;
  private readonly method: HTMLSelectElement;
  private readonly stateDesign;
  constructor(private readonly root: HTMLElement, private readonly loop: ControlLoopDefinition,
    private readonly post: StudioPost, private readonly family: () => string, private readonly choice: () => string = family) {
    this.fields = initializeManualTuneSettings(root, loop, value => this.previewLater(value.values));
    this.method = root.querySelector<HTMLSelectElement>("[data-engineer-tuning-method]")!;
    if (loop.controller.stateFeedback) {
      const option = root.ownerDocument.createElement("option"); option.value = "state-design"; option.textContent = "State-space design"; this.method.append(option);
    }
    const declaredStateDesign = loop.controller.stateFeedback ? {
      ...(loop.controller.stateFeedback.desiredPoles ? { desiredPoles: loop.controller.stateFeedback.desiredPoles } : {}),
      ...(loop.controller.stateFeedback.stateWeights ? { stateWeights: loop.controller.stateFeedback.stateWeights } : {}),
      ...(loop.controller.stateFeedback.inputWeights ? { inputWeights: loop.controller.stateFeedback.inputWeights } : {}),
      ...(loop.controller.stateFeedback.processNoise ? { processNoise: loop.controller.stateFeedback.processNoise } : {}),
      ...(loop.controller.stateFeedback.measurementNoise ? { measurementNoise: loop.controller.stateFeedback.measurementNoise } : {}),
    } : undefined;
    this.stateDesign = initializeThreePageStateDesign(root, declaredStateDesign, () => this.edited(), () => this.family() === "automatic" ? loop.controller.stateFeedback?.family ?? "state-feedback" : this.family());
    this.method.addEventListener("change", () => { this.lastMethod = this.method.value as ControllerTuningPreferences["method"]; this.edited(); this.render(); });
    const area = root.querySelector<HTMLElement>("[data-studio-tuning-result]")!;
    area.addEventListener("input", () => { this.capture(); this.edited(); });
    area.addEventListener("change", event => {
      // A Keep toggle keeps its calculated value for the next tuning run; it is read before the change is sent.
      const keep = event.target as HTMLInputElement | null;
      const symbol = keep?.dataset?.studioKeep;
      if (symbol) {
        if (keep!.checked && Number.isFinite(this.values[symbol])) this.pins[symbol] = this.values[symbol];
        else delete this.pins[symbol];
      }
      this.capture(); this.edited();
    });
    root.querySelector<HTMLSelectElement>("[data-studio-controller]")!.addEventListener("change", () => this.render());
  }
  preferences(validate = false): ControllerTuningPreferences {
    if (validate && this.method.value === "manual" && this.family() === "automatic") throw new Error("Choose a controller configuration before entering manual settings.");
    const values = validate && this.method.value === "manual" ? this.fields.read().values : this.values;
    const stateDesign = this.stateDesign.read(validate && this.method.value === "state-design");
    return { method: this.method.value.startsWith("analysis:") ? this.lastMethod : this.method.value as ControllerTuningPreferences["method"], values: { ...values }, ...(stateDesign ? { stateDesign } : {}), ...(this.loop.controller.stateFeedback ? { stateDesignDraft: this.stateDesign.draft() } : {}) };
  }
  pinnedValues(): Record<string, number> | null { return Object.keys(this.pins).length ? { ...this.pins } : null; }
  /** Draws the current candidate again, for example once an analysis tool closes and the tuning result returns. */
  refresh(): void { this.render(); }
  /** Shows the tuning the study recorded (A8.6): the tuning method, the settings the manual method takes and the pinned settings. */
  restoreInputs(recorded: { method: ControllerTuningPreferences["method"]; settings?: Record<string, number>; pins?: Record<string, number> }): void {
    this.lastMethod = recorded.method;
    this.method.value = recorded.method;
    if (recorded.method === "manual" && recorded.settings) { this.values = { ...recorded.settings }; this.hasSettings = Object.keys(this.values).length > 0; }
    this.pins = { ...recorded.pins };
    this.render();
  }
  receive(payload: Record<string, unknown>): void {
    if (typeof payload.inputRevision === "number") {
      if (payload.inputRevision < this.inputRevision) return;
      this.inputRevision = payload.inputRevision;
    }
    const preferences = payload.tuningPreferences as ControllerTuningPreferences | undefined;
    const acknowledged = preferences && JSON.stringify(preferences) === this.pending;
    if (acknowledged) this.pending = undefined;
    if (preferences && !this.pending) {
      this.lastMethod = preferences.method;
      this.stateDesign.receive(preferences.stateDesign, preferences.stateDesignDraft);
      this.method.value = preferences.method;
      if (payload.status === "restored" && preferences.method === "manual" && preferences.values) {
        this.values = { ...preferences.values }; this.hasSettings = Object.keys(this.values).length > 0;
      }
    }
    const draft = payload.draft as { values?: Record<string, number> } | undefined;
    if (payload.status === "candidate" && payload.values && typeof payload.values === "object") {
      this.values = { ...payload.values as Record<string, number> }; this.hasSettings = true;
      this.design = payload.design as Record<string, unknown> | undefined; this.robustness = payload.robustness;
      this.notes = { methodDescription: payload.methodDescription, outcome: payload.outcome, unavailable: payload.unavailable, comparison: payload.comparison };
    } else if (payload.status === "restored" && draft?.values && preferences?.method !== "manual") {
      this.values = { ...draft.values }; this.hasSettings = true;
      this.design = (payload.draft as { design?: Record<string, unknown> })?.design; this.robustness = undefined;
    }
    // A failed Tune drops the host's candidate, so an earlier one must not stand beside the refusal as its result. A reset drops the
    // candidate with the model it was designed on (A8.4); the manual settings the engineer typed are inputs, and they stay.
    const refused = payload.status === "failed" && payload.flowStage === "Tune" && this.method.value !== "manual";
    const dropped = refused || (payload.status === "reset" && this.method.value !== "manual");
    if (dropped) { this.values = {}; this.hasSettings = false; this.design = undefined; this.robustness = undefined; this.notes = {}; }
    if (payload.settingsPins !== undefined && !this.pending) this.pins = { ...(payload.settingsPins as { values?: Record<string, number> } | null)?.values };
    if (dropped || payload.status === "candidate" || payload.status === "restored" || preferences && !this.pending && !acknowledged) this.render();
  }
  /** Manual values are read from the editable settings; Keep toggles belong to automatic results only. */
  private capture(): void {
    const fields = Array.from(this.root.querySelectorAll<HTMLInputElement>("[data-studio-manual-value]"));
    if (!fields.length) return;
    for (const input of fields) {
      const symbol = input.dataset.studioManualValue!;
      if (Number.isFinite(input.valueAsNumber)) this.values[symbol] = input.valueAsNumber;
      else delete this.values[symbol];
    }
  }
  /** A manual edit waits for a 300 ms pause, then asks the host for its predicted response; a newer edit replaces the wait. */
  private previewLater(values: Record<string, number>): void {
    if (this.previewTimer !== undefined) clearTimeout(this.previewTimer);
    this.previewTimer = setTimeout(() => {
      this.previewTimer = undefined;
      this.post("control-studio.engineer.manual-preview", { settings: { ...values } });
    }, MANUAL_PREVIEW_DELAY_MS);
  }
  private edited(): void {
    this.robustness = undefined; this.notes = {};
    const preferences = this.preferences(); this.pending = JSON.stringify(preferences);
    this.post("control-studio.engineer.invalidate", { tuningPreferences: preferences, pinnedValues: this.pinnedValues(), family: this.method.value === "manual" ? this.family() : this.choice() });
  }
  private render(): void {
    this.stateDesign.synchronize();
    const design = this.root.querySelector<HTMLElement>("[data-studio-state-design]");
    if (design) design.hidden = this.method.value !== "state-design";
    if (this.method.value === "manual" && this.family() === "automatic") {
      const area = this.root.querySelector<HTMLElement>("[data-studio-tuning-result]")!; area.replaceChildren(); area.hidden = true; return;
    }
    const payload = { status: "candidate", family: this.family(), values: this.values, design: this.design, settingsPins: { values: this.pins },
      ...(this.robustness ? { robustness: this.robustness } : {}), ...this.notes };
    if (this.method.value === "manual") this.fields.render(payload);
    else if (this.hasSettings) renderTunedController(this.root, this.loop, payload);
    else { const area = this.root.querySelector<HTMLElement>("[data-studio-tuning-result]")!; area.replaceChildren(); area.hidden = true; }
  }
}
