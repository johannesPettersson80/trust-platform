import type { ControlLoopDefinition } from "../loopDefinition";
import type { ControllerTuningPreferences } from "../controllerTuningPreferences";
import type { StudioPost } from "./threePageOperation";
import { initializeManualTuneSettings, type FamilySymbols } from "./manualTuneSettings";
import { renderTunedController } from "./threePageResults";
import { initializeThreePageStateDesign, type StateDesignEditor, type StateDesignFieldView } from "./threePageStateDesign";
import { renderSearchResult, type SearchView } from "./threePageSearch";

/** How long a manual edit waits for a pause before its predicted response is requested (Specification 35 B9.9). */
export const MANUAL_PREVIEW_DELAY_MS = 300;

/** The words of the tuning methods the engine gives (C4); a method without words shows its engine id. */
const METHOD_WORDS: Partial<Record<string, string>> = { "state-space": "State-space design" };

/** Editing preferences stay separate from the host's validated candidate. */
export class ThreePageTuning {
  private values: Record<string, number> = {};
  private pins: Record<string, number> = {};
  private pending: string | undefined;
  private hasSettings = false;
  private design: Record<string, unknown> | undefined;
  /** The host's margins for the candidate (Specification 35 §17.10); a manual edit makes them stale. */
  private robustness: unknown;
  /** A state law's answer (C4): its closed-loop poles, Riccati stages and form fields, as the engine gives them. */
  private stateAnswer: unknown;
  /** The engine's notes on the candidate: its method in words, its outcome and why its margins are missing (B4.4 to B4.6). */
  private notes: Record<string, unknown> = {};
  /** The finite search the engine answered (C5), shown in place of a candidate; undefined when the area shows a candidate. */
  private search: SearchView | undefined;
  private inputRevision = -1;
  private lastMethod: ControllerTuningPreferences["method"] = "automatic";
  private previewTimer: ReturnType<typeof setTimeout> | undefined;
  private readonly fields;
  private readonly method: HTMLSelectElement;
  private readonly stateDesign: StateDesignEditor;
  constructor(private readonly root: HTMLElement, private readonly loop: ControlLoopDefinition,
    private readonly post: StudioPost, private readonly family: () => string, private readonly choice: () => string = family,
    familySymbols?: FamilySymbols) {
    this.fields = initializeManualTuneSettings(root, loop, value => this.previewLater(value.values), familySymbols);
    this.method = root.querySelector<HTMLSelectElement>("[data-engineer-tuning-method]")!;
    const declaredStateDesign = loop.controller.stateFeedback ? {
      ...(loop.controller.stateFeedback.desiredPoles ? { desiredPoles: loop.controller.stateFeedback.desiredPoles } : {}),
      ...(loop.controller.stateFeedback.stateWeights ? { stateWeights: loop.controller.stateFeedback.stateWeights } : {}),
      ...(loop.controller.stateFeedback.inputWeights ? { inputWeights: loop.controller.stateFeedback.inputWeights } : {}),
      ...(loop.controller.stateFeedback.processNoise ? { processNoise: loop.controller.stateFeedback.processNoise } : {}),
      ...(loop.controller.stateFeedback.measurementNoise ? { measurementNoise: loop.controller.stateFeedback.measurementNoise } : {}),
    } : undefined;
    this.stateDesign = initializeThreePageStateDesign(root, declaredStateDesign, () => this.edited());
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
    return { method: this.method.value.startsWith("analysis:") ? this.lastMethod : this.method.value as ControllerTuningPreferences["method"], values: { ...values },
      ...(this.loop.controller.stateFeedback ? { stateDesignDraft: this.stateDesign.draft() } : {}) };
  }
  pinnedValues(): Record<string, number> | null { return Object.keys(this.pins).length ? { ...this.pins } : null; }
  /** Draws the current candidate again, for example once an analysis tool closes and the tuning result returns. */
  refresh(): void { this.render(); }
  /** Offers exactly the tuning methods the engine gives this loop, beside Automatic (`setupDiscover`'s `methods`, C4). */
  offerMethods(methods: readonly string[]): void {
    for (const option of Array.from(this.method.options)) if (option.value !== "automatic") option.remove();
    for (const method of methods) this.offer(method);
    if (!methods.includes(this.method.value) && this.method.value !== "automatic") {
      this.method.value = "automatic";
      this.lastMethod = "automatic";
    }
    this.render();
  }
  /** Shows the tuning the study recorded (A8.6): the tuning method, the settings the manual method takes, the pinned settings and the state draft. */
  restoreInputs(recorded: { method: ControllerTuningPreferences["method"]; settings?: Record<string, number>; pins?: Record<string, number>;
    stateDraft?: Record<string, string> }): void {
    this.lastMethod = recorded.method;
    this.offer(recorded.method);
    this.method.value = recorded.method;
    if (recorded.stateDraft) this.stateDesign.receive(recorded.stateDraft);
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
      this.stateDesign.receive(preferences.stateDesignDraft);
      this.method.value = preferences.method;
      if (payload.status === "restored" && preferences.method === "manual" && preferences.values) {
        this.values = { ...preferences.values }; this.hasSettings = Object.keys(this.values).length > 0;
      }
    }
    // The design form's fields are the engine's list (C4): the study's inputs answer them before any Tune.
    if (Array.isArray(payload.stateFields)) this.stateDesign.setFields(payload.stateFields.filter(isStateField));
    const draft = payload.draft as { values?: Record<string, number> } | undefined;
    if (payload.status === "candidate" && payload.values && typeof payload.values === "object") {
      this.values = { ...payload.values as Record<string, number> }; this.hasSettings = true; this.search = undefined;
      this.design = payload.design as Record<string, unknown> | undefined; this.robustness = payload.robustness; this.stateAnswer = payload.state;
      this.notes = { methodDescription: payload.methodDescription, outcome: payload.outcome, unavailable: payload.unavailable, comparison: payload.comparison };
      this.stateDesign.clearErrors();
    } else if (payload.status === "restored" && draft?.values && preferences?.method !== "manual") {
      this.values = { ...draft.values }; this.hasSettings = true;
      this.design = (payload.draft as { design?: Record<string, unknown> })?.design; this.robustness = undefined; this.stateAnswer = undefined;
    }
    // A finite search's answer (C5) is shown in place of a candidate: its trials, and the sentence it ends with when it did not complete.
    if (payload.status === "search" && payload.search && typeof payload.search === "object") {
      this.search = payload.search as SearchView; this.values = {}; this.hasSettings = false; this.design = undefined; this.robustness = undefined; this.stateAnswer = undefined; this.notes = {};
    }
    // A failed Tune drops the host's candidate, so an earlier one must not stand beside the refusal as its result. A reset drops the
    // candidate with the model it was designed on (A8.4); the manual settings the engineer typed are inputs, and they stay.
    const refused = payload.status === "failed" && payload.flowStage === "Tune" && this.method.value !== "manual";
    if (refused) this.showRefusal(record(payload.failure));
    const dropped = refused || (payload.status === "reset" && this.method.value !== "manual");
    if (dropped) { this.values = {}; this.hasSettings = false; this.design = undefined; this.robustness = undefined; this.stateAnswer = undefined; this.notes = {}; this.search = undefined; }
    if (payload.status === "reset") this.stateDesign.clearErrors();
    if (payload.settingsPins !== undefined && !this.pending) this.pins = { ...(payload.settingsPins as { values?: Record<string, number> } | null)?.values };
    if (dropped || payload.status === "candidate" || payload.status === "restored" || payload.status === "search" || preferences && !this.pending && !acknowledged) this.render();
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
    this.robustness = undefined; this.stateAnswer = undefined; this.notes = {};
    const preferences = this.preferences(); this.pending = JSON.stringify(preferences);
    this.post("control-studio.engineer.invalidate", { tuningPreferences: preferences, pinnedValues: this.pinnedValues(), family: this.method.value === "manual" ? this.family() : this.choice() });
  }
  /** Adds the option of an engine method the page does not hold yet, named by the engine's id. */
  private offer(method: string): void {
    if (Array.from(this.method.options).some(option => option.value === method)) return;
    const option = this.root.ownerDocument.createElement("option");
    option.value = method;
    option.textContent = METHOD_WORDS[method] ?? method;
    this.method.append(option);
  }
  /** A refused Tune shows its sentence under the form field it names (D-B12); a refusal of the whole form shows it at the form's head. */
  private showRefusal(failure: Record<string, unknown> | undefined): void {
    const sentence = [failure?.detail, failure?.message].find((text): text is string => typeof text === "string" && text.length > 0);
    if (sentence === undefined) return;
    const field = typeof failure?.field === "string" ? failure.field : undefined;
    if (field === undefined || !this.stateDesign.showError(field, sentence)) this.stateDesign.showFormError(sentence);
  }
  private render(): void {
    const design = this.root.querySelector<HTMLElement>("[data-studio-state-design]");
    if (design) design.hidden = this.method.value !== "state-space";
    if (this.method.value === "manual" && this.family() === "automatic") {
      const area = this.root.querySelector<HTMLElement>("[data-studio-tuning-result]")!; area.replaceChildren(); area.hidden = true; return;
    }
    const payload = { status: "candidate", family: this.family(), values: this.values, design: this.design, settingsPins: { values: this.pins },
      ...(this.robustness ? { robustness: this.robustness } : {}), ...(this.stateAnswer ? { state: this.stateAnswer } : {}), ...this.notes };
    if (this.method.value === "manual") this.fields.render(payload);
    else if (this.search) renderSearchResult(this.root, this.loop, this.search);
    else if (this.hasSettings) renderTunedController(this.root, this.loop, payload);
    else { const area = this.root.querySelector<HTMLElement>("[data-studio-tuning-result]")!; area.replaceChildren(); area.hidden = true; }
  }
}

function record(value: unknown): Record<string, unknown> | undefined {
  return value && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : undefined;
}

function isStateField(item: unknown): item is StateDesignFieldView {
  const field = record(item);
  return field !== undefined && typeof field.field === "string" && typeof field.label === "string";
}
