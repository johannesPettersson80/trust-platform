import type { ControlLoopDefinition } from "../loopDefinition";
import type { ControllerTuningPreferences } from "../controllerTuningPreferences";
import type { StudioPost } from "./threePageOperation";
import { initializeManualTuneSettings } from "./manualTuneSettings";
import { renderTunedController } from "./threePageResults";
import { initializeThreePageStateDesign } from "./threePageStateDesign";

/** Editing preferences stay separate from the host's validated candidate. */
export class ThreePageTuning {
  private values: Record<string, number> = {};
  private pins: Record<string, number> = {};
  private pending: string | undefined;
  private hasSettings = false;
  private design: Record<string, unknown> | undefined;
  private inputRevision = -1;
  private lastMethod: ControllerTuningPreferences["method"] = "automatic";
  private readonly fields;
  private readonly method: HTMLSelectElement;
  private readonly stateDesign;
  constructor(private readonly root: HTMLElement, private readonly loop: ControlLoopDefinition,
    private readonly post: StudioPost, private readonly family: () => string, private readonly choice: () => string = family) {
    this.fields = initializeManualTuneSettings(root, loop);
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
    this.method.addEventListener("change", () => { if (this.method.value.startsWith("analysis:")) return; this.lastMethod = this.method.value as ControllerTuningPreferences["method"]; this.edited(); this.render(); });
    const area = root.querySelector<HTMLElement>("[data-studio-tuning-result]")!;
    area.addEventListener("input", () => { this.capture(); this.edited(); });
    area.addEventListener("change", () => { this.capture(); this.edited(); });
    root.querySelector<HTMLSelectElement>("[data-studio-controller]")!.addEventListener("change", () => this.render());
  }
  preferences(validate = false): ControllerTuningPreferences {
    if (validate && this.method.value === "manual" && this.family() === "automatic") throw new Error("Choose a controller configuration before entering manual settings.");
    const values = validate && this.method.value === "manual" ? this.fields.read().values : this.values;
    const stateDesign = this.stateDesign.read(validate && this.method.value === "state-design");
    return { method: this.method.value.startsWith("analysis:") ? this.lastMethod : this.method.value as ControllerTuningPreferences["method"], values: { ...values }, ...(stateDesign ? { stateDesign } : {}), ...(this.loop.controller.stateFeedback ? { stateDesignDraft: this.stateDesign.draft() } : {}) };
  }
  pinnedValues(): Record<string, number> | null { return Object.keys(this.pins).length ? { ...this.pins } : null; }
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
      if (!this.method.value.startsWith("analysis:")) this.method.value = preferences.method;
      if (payload.status === "restored" && preferences.method === "manual" && preferences.values) {
        this.values = { ...preferences.values }; this.hasSettings = Object.keys(this.values).length > 0;
      }
    }
    const draft = payload.draft as { values?: Record<string, number> } | undefined;
    if (payload.status === "candidate" && payload.values && typeof payload.values === "object") {
      this.values = { ...payload.values as Record<string, number> }; this.hasSettings = true;
      this.design = payload.design as Record<string, unknown> | undefined;
    } else if (payload.status === "restored" && draft?.values && preferences?.method !== "manual") {
      this.values = { ...draft.values }; this.hasSettings = true;
      this.design = (payload.draft as { design?: Record<string, unknown> })?.design;
    }
    if (payload.settingsPins !== undefined && !this.pending) this.pins = { ...(payload.settingsPins as { values?: Record<string, number> } | null)?.values };
    if (payload.status === "candidate" || payload.status === "restored" || preferences && !this.pending && !acknowledged) this.render();
  }
  private capture(): void {
    const fields = Array.from(this.root.querySelectorAll<HTMLInputElement>("[data-studio-manual-value]"));
    if (!fields.length) return;
    for (const input of fields) {
      const symbol = input.dataset.studioManualValue!;
      if (Number.isFinite(input.valueAsNumber)) this.values[symbol] = input.valueAsNumber;
      else delete this.values[symbol];
      const pin = Array.from(this.root.querySelectorAll<HTMLInputElement>("[data-studio-manual-pin]")).find(item => item.dataset.studioManualPin === symbol);
      if (pin?.checked && Number.isFinite(input.valueAsNumber)) this.pins[symbol] = input.valueAsNumber;
      else delete this.pins[symbol];
    }
  }
  private edited(): void {
    const preferences = this.preferences(); this.pending = JSON.stringify(preferences);
    this.post("control-studio.engineer.invalidate", { tuningPreferences: preferences, pinnedValues: this.pinnedValues(), family: this.method.value === "manual" ? this.family() : this.choice() });
  }
  private render(): void {
    this.stateDesign.synchronize();
    const design = this.root.querySelector<HTMLElement>("[data-studio-state-design]");
    if (design) design.hidden = this.method.value !== "state-design";
    if (this.method.value.startsWith("analysis:")) return;
    if (this.method.value === "manual" && this.family() === "automatic") {
      const area = this.root.querySelector<HTMLElement>("[data-studio-tuning-result]")!; area.replaceChildren(); area.hidden = true; return;
    }
    const payload = { status: "candidate", family: this.family(), values: this.values, design: this.design, settingsPins: { values: this.pins } };
    if (this.method.value === "manual") this.fields.render(payload);
    else if (this.hasSettings) renderTunedController(this.root, this.loop, payload);
    else { const area = this.root.querySelector<HTMLElement>("[data-studio-tuning-result]")!; area.replaceChildren(); area.hidden = true; }
  }
}
