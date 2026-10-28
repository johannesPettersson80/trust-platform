import { displayEngineeringUnit } from "../unitDisplay";
import { formatEngineeringNumber } from "../chartContract";
import { modelStructureEvidence } from "./modelStructureEvidence";
import { showModelChoices } from "./modelChoiceSelect";
import { renderStepResponse } from "./stepResponseControls";
import type { IdentificationFormDraft, IdentifiedModelPresentation, ManualIdentificationAnswer, StepResponseAnalysis,
  StepResponseMeasurement } from "../generated/ManualIdentificationAnswer";

/** The model a manual identification is run for, by the engine's names (the form's model field). */
type IdentificationModelSelection = IdentificationFormDraft["model"];

export type ThreePageIdentificationPost = (kind: string, payload?: Record<string, unknown>) => void;

/** The pause after the last change of a settings field before the changed form is sent to the engine (presentation only). */
const EDIT_DEBOUNCE_MS = 250;

/**
 * The Recorded-data fit's page (specification 35, B3.3). The engine decides every step: its answers name the stage, the labels, the status,
 * the gates and the models that may be chosen, and the page shows them and sends the form as the engineer typed it. The engine reads and
 * checks that text, and refuses a value under the form field it names.
 */
export class ThreePageIdentification {
  private readonly section: HTMLElement;
  private readonly model: HTMLSelectElement;
  private readonly heldStep: HTMLInputElement;
  private readonly settings: HTMLElement;
  private readonly exclusions: HTMLElement;
  private readonly status: HTMLElement;
  private selectedMethod = false;
  private pending = false;
  /** The engine's last answer to the Recorded-data fit. */
  private answer: ManualIdentificationAnswer | undefined;
  /** The models the engine's last answer lets the page enable (`modelChoices`, B3.3); none before an answer. */
  private modelChoices: readonly ModelChoice[] = [];
  /** The timer that sends the changed form after the last change. */
  private editTimer: ReturnType<typeof setTimeout> | undefined;
  /** True while a changed form has not been sent to the engine. */
  private editPending = false;

  constructor(private readonly root: HTMLElement, private readonly post: ThreePageIdentificationPost,
    /** The shared stage gate; Identify stays disabled while any page gate reports a block. */
    private readonly blocked: () => string | undefined = () => undefined) {
    const section = root.querySelector<HTMLElement>("[data-three-page-identification]");
    if (!section) throw new Error("The recorded identification workspace is missing.");
    this.section = section;
    this.model = section.querySelector<HTMLSelectElement>("[data-three-page-identification-model]")!;
    this.heldStep = section.querySelector<HTMLInputElement>("[data-three-page-identification-held-step]")!;
    this.settings = section.querySelector<HTMLElement>("[data-three-page-identification-settings]")!;
    this.exclusions = section.querySelector<HTMLElement>("[data-three-page-identification-exclusions]")!;
    this.status = section.querySelector<HTMLElement>("[data-three-page-identification-status]")!;
    const processUnit = displayEngineeringUnit(root.dataset.processUnit ?? "PV");
    const inputUnit = displayEngineeringUnit(root.dataset.outputUnit ?? "MV");
    for (const label of Array.from(section.querySelectorAll<HTMLElement>("[data-three-page-identification-label]"))) {
      label.textContent = (label.textContent ?? "").replace("PV/MV", `${processUnit}/${inputUnit}`).replace(/PV/g, processUnit);
      if (label.dataset.threePageIdentificationLabel === "baselineStep") label.textContent += ` (${processUnit})`;
    }
    // The settings the choice shows or hides follow it at once; the changed form goes to the engine after the debounce.
    this.model.addEventListener("change", () => { this.render(); this.scheduleEdit(); });
    this.heldStep.addEventListener("change", () => { this.render(); this.scheduleEdit(); });
    section.querySelectorAll<HTMLInputElement>("[data-three-page-identification-setting],[data-three-page-identification-excluded-start],[data-three-page-identification-excluded-end]")
      .forEach(input => input.addEventListener("input", () => this.scheduleEdit()));
    this.render();
  }

  setSelectedMethod(selected: boolean): void {
    const chosen = selected && !this.selectedMethod;
    this.selectedMethod = selected;
    this.section.hidden = !selected;
    // The engine's answer that came while another method was shown appears when the Recorded-data fit is chosen (A8.6, B3.3).
    if (chosen && this.answer) this.showAnswer();
    else this.render();
  }

  get isSelected(): boolean { return this.selectedMethod; }
  /** Cancel while a command runs, else the label of the action the engine's answer names; before an answer, the file picker's. */
  get primaryActionLabel(): string {
    if (this.pending) return "Cancel";
    return this.answer?.primaryActionLabel ?? "Open recording…";
  }

  primaryAction(): boolean {
    if (!this.selectedMethod) return false;
    return this.pending ? this.cancel() : this.namedAction();
  }

  cancel(): boolean {
    if (!this.pending) return false;
    // The status is the engine's: the cancelled update of the operation shows its answer again.
    this.post("control-studio.identify.cancel");
    return true;
  }

  /** Sends the file the engineer opened: the engine checks it and keeps it as the manual identification's recording (B3.6). */
  openRecording(json: string): void {
    this.pending = true;
    this.render();
    this.post("control-studio.identify.import", { json });
  }

  /** Shows the engine's running, cancelled and refused updates of the Recorded-data fit, and its answers. */
  receive(stage: string, rawPayload: unknown): void {
    const payload = record(rawPayload) ? rawPayload : {};
    if (stage !== "identify") return;
    if (payload.status === "running") {
      this.pending = true;
      this.status.textContent = text(payload.reason) ?? "";
      this.render();
      return;
    }
    if (payload.status === "cancelled") {
      this.pending = false;
      this.showAnswer();
      return;
    }
    // An answer names its primary action. A recording the check does not qualify and a validation the model fails are answers too: they carry
    // the reason with the evidence the engine scored, which stays in view (B3.3).
    if (typeof payload.primaryAction === "string") {
      this.answer = payload as unknown as ManualIdentificationAnswer;
      this.modelChoices = modelChoicesOf(payload);
      this.pending = false;
      // While another method is shown the answer waits: showing it would write its model over the model that method shows.
      if (this.selectedMethod) this.showAnswer();
      return;
    }
    // A failure without an answer is a refusal of the request, under the field the engine names.
    if (payload.status === "failed") {
      this.pending = false;
      this.refuse(text(payload.reason) ?? "", typeof payload.field === "string" ? payload.field : undefined);
    }
    // A `complete` update without an answer ends an operation whose answer follows it (`control-studio.engine.result`).
  }

  /** Shows the engine's answer: the form it holds (unless the page has a change of its own waiting), the evidence, and the status it words. */
  private showAnswer(): void {
    this.clearRefusals();
    const answer = this.answer;
    if (!answer) { this.render(); return; }
    if (answer.form && !this.editPending) this.restoreForm(answer.form);
    this.renderEvidence(answer as unknown as Record<string, unknown>);
    this.status.textContent = answer.status === "failed" ? answer.reason ?? "" : answer.message ?? "";
    this.render();
  }

  /** The engine's primary action (B3.3): the request it names, with the form the page holds; a stage that asks for none sends nothing. */
  private namedAction(): boolean {
    const answer = this.answer;
    if (!answer) return false;
    switch (answer.primaryAction) {
      case "openRecording":
      case "openValidationRecording":
        window.dispatchEvent(new CustomEvent("trust-control-studio-identification-open-recording"));
        return true;
      case "checkRecording":
        return this.send("control-studio.identify.run", { form: this.readFormDraft() as unknown as Record<string, unknown> });
      case "fitPlant":
      case "compareWithModel":
        return this.send("control-studio.identify.fit");
      case "chooseStructure":
        return answer.structureChoice ? this.send("control-studio.identify.choose-structure", { choice: answer.structureChoice }) : false;
      case "accept":
        return answer.acceptanceReady ? this.send("control-studio.identify.accept") : false;
      case "reviewSettings":
        (this.settings as HTMLDetailsElement).open = true;
        this.input("measurementBound")?.focus();
        return true;
      default:
        return false;
    }
  }

  /** Sends a command, after the form the page changed is sent, so that the engine checks the form it holds. */
  private send(kind: string, payload: Record<string, unknown> = {}): boolean {
    this.flushEdit();
    this.pending = true;
    this.render();
    this.post(kind, payload);
    return true;
  }

  /** Shows a refusal: the status names it, and the form field the engine names shows it under that field (B3.3). */
  private refuse(reason: string, field?: string): void {
    this.clearRefusals();
    this.status.textContent = reason;
    const target = field ? this.fieldOf(field) : undefined;
    if (target) {
      const note = target.ownerDocument.createElement("span");
      note.dataset.threePageIdentificationRefusal = "";
      note.textContent = reason;
      target.setAttribute("aria-invalid", "true");
      (target.closest("label") ?? target).after(note);
    }
    this.render();
  }

  private clearRefusals(): void {
    this.section.querySelectorAll("[data-three-page-identification-refusal]").forEach(note => note.remove());
    this.section.querySelectorAll("[aria-invalid]").forEach(field => field.removeAttribute("aria-invalid"));
  }

  /** The control of a form field the engine names: the model, the held-step switch, the exclusion bounds or a setting. */
  private fieldOf(field: string): HTMLElement | undefined {
    if (field === "model") return this.model;
    if (field === "heldStep") return this.heldStep;
    return this.input(field === "excludedStart" ? "excluded-start" : field === "excludedEnd" ? "excluded-end" : field);
  }

  /** Shows the evidence of an answer: the step-response measurement, the sensor-lag comparison and the model's presentation (B3.3). */
  private renderEvidence(payload: Record<string, unknown>): void {
    const details = this.section.querySelector<HTMLDetailsElement>("[data-step-response-evidence]")!;
    const qualification = qualificationOf(payload);
    const fit = record(payload.fit) ? payload.fit : undefined;
    const measurement = fit?.stepResponseMeasurement ?? qualification?.stepResponse ?? fit?.stepResponse;
    const units = record(payload.presentation) && record(payload.presentation.units) ? payload.presentation.units : {};
    if (record(measurement) && record(measurement.baseline) && record(measurement.final) && record(measurement.settings) && Array.isArray(measurement.warnings)) {
      renderStepResponse(measurement as unknown as StepResponseMeasurement,
        String(qualification?.outputUnit ?? units.output ?? "PV"), String(qualification?.inputUnit ?? units.input ?? "MV"), details,
        fit?.stepResponse as StepResponseAnalysis | undefined, Boolean(payload.validationQualification || payload.nextFitRole === "validation"),
        typeof units.gain === "string" ? units.gain : undefined);
      // The prediction's error rows are the engine's numbers: the fit's root mean square error and the largest absolute residual of its
      // samples, which the answer states (B3.3). A row the engine states no number for is not shown.
      const unit = displayEngineeringUnit(String(qualification?.outputUnit ?? units.output ?? "PV"));
      for (const [label, value] of [["Prediction RMSE", fit?.rootMeanSquareError], ["Maximum absolute residual", payload.maximumAbsoluteResidual]] as const) {
        if (typeof value !== "number") continue;
        const row = document.createElement("tr"), heading = document.createElement("th"), cell = document.createElement("td");
        heading.scope = "row"; heading.textContent = label; cell.textContent = `${formatEngineeringNumber(value)} ${unit}`;
        row.append(heading, cell); details.querySelector("tbody")!.append(row);
      }
    } else details.hidden = true;
    const presentation = record(payload.presentation) ? payload.presentation as IdentifiedModelPresentation : undefined;
    // The comparison is the engine's `modelStructure`, shown when the answer carries one (N2.10); the evidence is drawn when both candidates
    // carry the consistency grid that it compares.
    const comparison = presentation?.modelStructure ?? undefined;
    const comparisonDetails = this.section.querySelector<HTMLDetailsElement>("[data-model-comparison-evidence]")!;
    comparisonDetails.hidden = !comparison;
    if (comparison?.candidateA.analysis?.consistency && comparison.candidateB.analysis?.consistency) {
      const { tableWrap, explanation, details: parameters } = modelStructureEvidence(comparison, units);
      comparisonDetails.querySelector("[data-model-comparison-content]")!.replaceChildren(tableWrap, explanation, parameters);
    }
    this.publishPresentation(payload);
  }

  /** A change waits for the debounce: a burst of typing is one edit the engine answers (presentation only). */
  private scheduleEdit(): void {
    if (!this.selectedMethod) return;
    this.editPending = true;
    clearTimeout(this.editTimer);
    this.editTimer = setTimeout(() => this.flushEdit(), EDIT_DEBOUNCE_MS);
  }

  /** Sends the changed form now, when a change waits for the debounce. */
  private flushEdit(): void {
    clearTimeout(this.editTimer);
    this.editTimer = undefined;
    if (!this.editPending) return;
    this.editPending = false;
    this.post("control-studio.identify.edit", { form: this.readFormDraft() as unknown as Record<string, unknown> });
  }

  /** The form as the page holds it: the text of every setting field of the template, by its key, and the other fields as typed. */
  private readFormDraft(): IdentificationFormDraft {
    const values = Object.fromEntries(this.settingInputs().map(input => [settingKey(input), input.value])) as IdentificationFormDraft["values"];
    return { schemaVersion: 1, model: this.model.value as IdentificationModelSelection, heldStep: this.heldStep.checked, values,
      excludedStart: this.input("excluded-start")?.value ?? "", excludedEnd: this.input("excluded-end")?.value ?? "" };
  }

  /** Shows the form the engine holds. */
  private restoreForm(form: IdentificationFormDraft): void {
    // The options are the engine's choices, listed before the form's model is chosen among them.
    showModelChoices(this.model, this.modelChoices);
    this.model.value = form.model;
    this.heldStep.checked = form.heldStep;
    const values: Record<string, unknown> = record(form.values) ? form.values : {};
    for (const input of this.settingInputs()) {
      const value = values[settingKey(input)];
      if (typeof value === "string") input.value = value;
    }
    const start = this.input("excluded-start"); const end = this.input("excluded-end");
    if (start) start.value = form.excludedStart;
    if (end) end.value = form.excludedEnd;
    this.render();
  }

  /** The setting fields of the template: the form's keys are theirs. */
  private settingInputs(): HTMLInputElement[] {
    return Array.from(this.section.querySelectorAll<HTMLInputElement>("[data-three-page-identification-setting]"));
  }

  private input(key: string): HTMLInputElement | undefined {
    return key.startsWith("excluded-")
      ? this.section.querySelector<HTMLInputElement>(`[data-three-page-identification-${key}]`) ?? undefined
      : this.section.querySelector<HTMLInputElement>(`[data-three-page-identification-setting="${key}"]`) ?? undefined;
  }

  private render(): void {
    this.section.hidden = !this.selectedMethod;
    this.settings.hidden = !this.heldStep.checked;
    this.exclusions.hidden = this.heldStep.checked;
    // The additional-lag settings belong to the comparison of that family (Compare additional lag); the other families do not show them.
    for (const input of this.settingInputs().filter(field => /^(sensorTimeConstant|modelAdequacy)\./.test(settingKey(field)))) {
      const label = input.closest("label");
      if (label) label.hidden = this.model.value !== "sopdt";
    }
    // The models are the engine's `modelChoices`, each with its words and whether the page may enable it (B3.3).
    showModelChoices(this.model, this.modelChoices);
    const button = this.root.querySelector<HTMLButtonElement>("[data-engineer-find]");
    if (button) this.renderPrimaryButton(button);
    // The template's file button repeats the primary button while that already opens the recording picker (before the engine's first answer,
    // and while its next step is a recording to open); otherwise it stays, to open another file.
    const open = this.root.querySelector<HTMLButtonElement>("[data-identify-import]");
    if (open) open.hidden = this.selectedMethod && this.opensRecording();
  }

  /** Whether the primary action is the one that opens the recording picker. */
  private opensRecording(): boolean {
    const action = this.answer?.primaryAction;
    return action === undefined || action === "openRecording" || action === "openValidationRecording";
  }

  private publishPresentation(payload: Record<string, unknown>): void {
    if (!payload.presentation && !payload.fit && !payload.model) return;
    window.dispatchEvent(new CustomEvent("trust-control-studio-identification-presentation", {
      detail: { presentation: payload.presentation, fit: payload.fit, model: payload.model, phase: payload.phase },
    }));
  }

  private renderPrimaryButton(button: HTMLButtonElement): void {
    if (!this.selectedMethod) return;
    // The engine's label; the gates are the answer's: a model that is ready, and an acceptance the engine does not allow, stay disabled.
    const answer = this.answer;
    const gated = !this.pending && (answer?.primaryAction === "modelReady" || (answer?.primaryAction === "accept" && !answer.acceptanceReady));
    button.textContent = this.primaryActionLabel;
    button.disabled = gated || this.blocked() !== undefined;
  }
}

function record(value: unknown): value is Record<string, any> { return Boolean(value) && typeof value === "object" && !Array.isArray(value); }
function text(value: unknown): string | undefined { return typeof value === "string" && value.length > 0 ? value : undefined; }

/** The key the engine's form names a setting field with: the template's own name for it. */
function settingKey(input: HTMLInputElement): string { return input.dataset.threePageIdentificationSetting ?? ""; }

/** One model of the page's model select, its words and whether the engine lets the page enable it (`modelChoices`, B3.3). */
interface ModelChoice {
  readonly model: string;
  readonly enabled: boolean;
  readonly label?: string | null;
}

/** The engine's model choices of an answer. An answer that names none leaves every model enabled. */
function modelChoicesOf(answer: Record<string, unknown>): ModelChoice[] {
  return Array.isArray(answer.modelChoices) ? (answer.modelChoices as ModelChoice[]) : [];
}

/** The qualification an answer shows: the validation's, else the check's, else the estimation's (B3.3). */
function qualificationOf(payload: Record<string, unknown>): Record<string, any> | undefined {
  return record(payload.validationQualification) ? payload.validationQualification
    : record(payload.qualification) ? payload.qualification : record(payload.estimationQualification) ? payload.estimationQualification : undefined;
}
