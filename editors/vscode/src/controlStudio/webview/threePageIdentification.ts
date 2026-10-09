import { formatEngineeringNumber } from "../chartContract";
import { displayEngineeringUnit } from "../unitDisplay";
import { modelStructureEvidence } from "./modelStructureEvidence";
import { renderStepResponse } from "./stepResponseControls";
import { stepResponseSettingsProblem, type StepResponseAnalysis, type StepResponseMeasurement, type StepResponseSettings } from "../stepResponseIdentification";
import type { IdentificationModelSelection, IdentifiedModelPresentation } from "../identification";
import type { IdentificationFormDraft } from "../identificationFormDraft";
import type { SensorLagComparison, StructureChoice } from "../sensorLagComparison";
import { plainReason } from "./studioFailure";

export type ThreePageIdentificationPost = (kind: string, payload?: Record<string, unknown>) => void;
type IdentificationStage = "initial" | "imported" | "qualified" | "validation_required" | "acceptance_required" | "accepted" | "failed";

const FORM_KEYS = [
  "baselineDurationSeconds", "finalDurationSeconds", "maximumWindowRange", "maximumWindowSlopePerSecond",
  "measurementBound", "onsetThreshold", "onsetSamples", "baselineStep", "gainStep", "timestampShiftSeconds",
  "delay.minimum", "delay.maximum", "delay.step", "timeConstant.minimum", "timeConstant.maximum", "timeConstant.step",
  "sensorTimeConstant.minimum", "sensorTimeConstant.maximum", "sensorTimeConstant.step",
  "modelAdequacy.maximumAbsoluteResidual", "modelAdequacy.maximumFinalError",
] as const;

/** Reconnects the existing Identify workflow to the compact three-page surface. */
export class ThreePageIdentification {
  private readonly section: HTMLElement;
  private readonly model: HTMLSelectElement;
  private readonly heldStep: HTMLInputElement;
  private readonly settings: HTMLElement;
  private readonly exclusions: HTMLElement;
  private readonly status: HTMLElement;
  private stage: IdentificationStage = "initial";
  private fitRole: "estimation" | "validation" = "estimation";
  private selectedMethod = false;
  private pending = false;
  private hasRecording = false;
  private comparison: SensorLagComparison | undefined;
  private acceptanceReady = false;
  private readonly openValidationRecording?: () => void;

  constructor(private readonly root: HTMLElement, private readonly post: ThreePageIdentificationPost,
    openValidationRecording?: () => void,
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
    this.openValidationRecording = openValidationRecording;
    const processUnit = displayEngineeringUnit(root.dataset.processUnit ?? "PV");
    const inputUnit = displayEngineeringUnit(root.dataset.outputUnit ?? "MV");
    for (const label of Array.from(section.querySelectorAll<HTMLElement>("[data-three-page-identification-label]"))) {
      label.textContent = (label.textContent ?? "").replace("PV/MV", `${processUnit}/${inputUnit}`).replace(/PV/g, processUnit);
      if (label.dataset.threePageIdentificationLabel === "baselineStep") label.textContent += ` (${processUnit})`;
    }
    this.model.addEventListener("change", () => this.edit());
    this.heldStep.addEventListener("change", () => {
      if (this.heldStep.checked) this.model.value = "fopdt";
      this.edit();
    });
    section.querySelectorAll<HTMLInputElement>("[data-three-page-identification-setting],[data-three-page-identification-excluded-start],[data-three-page-identification-excluded-end]")
      .forEach(input => input.addEventListener("input", () => this.edit()));
    this.render();
  }

  setSelectedMethod(selected: boolean): void {
    this.selectedMethod = selected;
    this.section.hidden = !selected;
    this.render();
  }

  synchronize(): void { this.render(); }

  get isSelected(): boolean { return this.selectedMethod; }
  get busy(): boolean { return this.pending; }
  get primaryActionLabel(): string {
    if (this.pending) return "Cancel";
    if (this.stage === "acceptance_required") return this.acceptanceReady ? "Use fitted model" : "Validation not qualified";
    if (this.stage === "validation_required") {
      if (this.comparison && !this.comparison.choice) return this.comparison.recommended === "first_order" ? "Keep first-order model"
        : this.comparison.recommended === "additional_lag" ? "Keep additional-lag model" : "Review settings";
      return "Open validation recording";
    }
    if (this.stage === "qualified") return this.fitRole === "validation" ? "Compare with model" : "Fit plant model";
    if (this.stage === "imported" || this.stage === "failed" && this.hasRecording) return "Check recording";
    if (this.stage === "accepted") return "Model ready";
    return "Open recording…";
  }

  primaryAction(): boolean {
    if (!this.selectedMethod || this.stage === "accepted") return false;
    if (this.pending) return this.cancel();
    if (this.stage === "validation_required") {
      if (this.comparison && !this.comparison.choice) {
        if (this.comparison.recommended !== "neither") return this.chooseStructure(this.comparison.recommended);
        (this.settings as HTMLDetailsElement).open = true;
        this.input("measurementBound")?.focus();
        return true;
      }
      this.openValidationRecording?.();
      return Boolean(this.openValidationRecording);
    }
    if (this.stage === "acceptance_required") {
      if (!this.acceptanceReady) {
        this.status.textContent = "Independent validation or model-structure evidence is not qualified for acceptance.";
        this.render();
        return false;
      }
      return this.send("control-studio.identify.accept");
    }
    if (this.stage === "qualified") return this.send("control-studio.identify.fit");
    if (!this.hasRecording) {
      window.dispatchEvent(new CustomEvent("trust-control-studio-identification-open-recording"));
      return true;
    }
    const settings = this.readStepResponseSettings();
    if (settings === null) return false;
    const excludedIntervals = this.readExcludedIntervals();
    if (excludedIntervals === null) return false;
    const payload: Record<string, unknown> = { model: this.model.value };
    if (settings) payload.stepResponseSettings = settings;
    else if (excludedIntervals.length) payload.excludedIntervals = excludedIntervals;
    return this.send("control-studio.identify.run", payload);
  }

  cancel(): boolean {
    if (!this.pending) return false;
    this.status.textContent = "Cancelling identification…";
    this.post("control-studio.identify.cancel");
    return true;
  }

  chooseStructure(choice: StructureChoice): boolean {
    if (this.pending || !this.comparison || this.comparison.recommended !== choice || this.comparison.choice) return false;
    this.pending = true;
    this.status.textContent = "Retaining the selected model structure…";
    this.render();
    this.post("control-studio.identify.choose-structure", { choice });
    return true;
  }

  bindPrimaryButton(button: HTMLButtonElement): void {
    button.addEventListener("click", event => { event.preventDefault(); this.primaryAction(); });
    this.renderPrimaryButton(button);
  }

  /** Shared Open recording picker calls this with the selected JSON. */
  openRecording(json: string): void {
    this.pending = true;
    this.status.textContent = "Opening the complete recording…";
    this.render();
    this.post("control-studio.identify.import", { json });
  }

  receive(stage: string, rawPayload: unknown): void {
    const payload = record(rawPayload) ? rawPayload : {};
    if (stage === "study") { this.restoreStudy(payload); return; }
    if (stage === "recording" && record(payload.reference)) this.restoreStudy({ current: payload.reference });
    if (payload.nextFitRole === "estimation" || payload.nextFitRole === "validation") this.fitRole = payload.nextFitRole;
    if (stage === "recording") {
      this.pending = false;
      if (payload.status === "imported") {
        this.hasRecording = true;
        this.stage = "imported";
        this.status.textContent = "Recording opened. Check it before fitting a model.";
      } else {
        this.status.textContent = text(payload.reason) ?? "The recording could not be opened.";
      }
      this.render();
      return;
    }
    if (stage !== "identify") return;
    if (payload.status === "running") {
      this.pending = true;
      this.status.textContent = text(payload.reason) ?? "Identification is running…";
      this.render();
      return;
    }
    const details = this.section.querySelector<HTMLDetailsElement>("[data-step-response-evidence]")!;
    const qualification = record(payload.validationQualification) ? payload.validationQualification
      : record(payload.qualification) ? payload.qualification : record(payload.estimationQualification) ? payload.estimationQualification : undefined;
    if (Array.isArray(qualification?.samples) && qualification.samples.length > 0) this.hasRecording = true;
    const fit = record(payload.fit) ? payload.fit : undefined;
    const measurement = fit?.stepResponseMeasurement ?? qualification?.stepResponse ?? fit?.stepResponse;
    const units = record(payload.presentation) && record(payload.presentation.units) ? payload.presentation.units : {};
    if (record(measurement) && record(measurement.baseline) && record(measurement.final) && record(measurement.settings) && Array.isArray(measurement.warnings)) {
      renderStepResponse(measurement as unknown as StepResponseMeasurement,
        String(qualification?.outputUnit ?? units.output ?? "PV"), String(qualification?.inputUnit ?? units.input ?? "MV"), details,
        fit?.stepResponse as StepResponseAnalysis | undefined, Boolean(payload.validationQualification || payload.nextFitRole === "validation"));
      if (typeof fit?.rootMeanSquareError === "number" && Array.isArray(fit.points) && fit.points.length) {
        const unit = displayEngineeringUnit(String(qualification?.outputUnit ?? units.output ?? "PV"));
        const peak = Math.max(...fit.points.map((point: { residual: number }) => Math.abs(point.residual)));
        for (const [label, value] of [["Prediction RMSE", fit.rootMeanSquareError], ["Maximum absolute residual", peak]] as const) {
          const row = document.createElement("tr"), heading = document.createElement("th"), cell = document.createElement("td");
          heading.scope = "row"; heading.textContent = label; cell.textContent = `${formatEngineeringNumber(value)} ${unit}`;
          row.append(heading, cell); details.querySelector("tbody")!.append(row);
        }
      }
    } else details.hidden = true;
    const phase = text(payload.phase);
    this.pending = false;
    if (phase === "editing") {
      this.hasRecording = payload.hasRecording === true;
      this.stage = this.hasRecording ? "imported" : "initial";
      this.acceptanceReady = false;
      if (record(payload.form)) this.restoreForm(payload.form);
      window.dispatchEvent(new CustomEvent("trust-control-studio-identification-method-required"));
    }
    const presentation = record(payload.presentation) ? payload.presentation as IdentifiedModelPresentation : undefined;
    this.comparison = isSensorLagComparison(presentation?.modelStructure) ? presentation.modelStructure : undefined;
    const comparisonDetails = this.section.querySelector<HTMLDetailsElement>("[data-model-comparison-evidence]")!;
    comparisonDetails.hidden = !this.comparison;
    if (this.comparison?.candidateA.analysis?.consistency && this.comparison.candidateB.analysis?.consistency) {
      const { tableWrap, explanation, details: parameters } = modelStructureEvidence(this.comparison, units);
      comparisonDetails.querySelector("[data-model-comparison-content]")!.replaceChildren(tableWrap, explanation, parameters);
    }
    this.publishPresentation(payload);
    if (payload.status === "complete" && phase && ["qualified", "validation_required", "acceptance_required", "accepted"].includes(phase)) {
      this.stage = phase as Exclude<IdentificationStage, "initial" | "imported" | "failed">;
      this.hasRecording = true;
      this.acceptanceReady = phase === "acceptance_required" && acceptanceEvidenceIsReady(payload, this.comparison);
    } else if (payload.status === "complete" && phase === "recording-selected") {
      this.hasRecording = true;
      this.stage = "imported";
      this.acceptanceReady = false;
    } else if (payload.status === "failed") {
      this.stage = "failed";
      this.acceptanceReady = false;
      this.status.textContent = text(payload.reason) ?? "Identification failed.";
    }
    if (payload.status === "complete") {
      if (phase === "qualified") this.status.textContent = `Recording qualified. ${this.primaryActionLabel}.`;
      if (phase === "validation_required") this.status.textContent = this.comparison && !this.comparison.choice
        ? this.comparison.recommended === "first_order" ? "First order meets the limits; the extra state is not justified."
          : this.comparison.recommended === "additional_lag" ? "The additional lag meets the limits. Its physical origin remains unresolved."
          : "Neither model meets the declared requirements. Review the comparison and settings."
        : "The estimation preview is ready. Open a distinct validation recording.";
      if (phase === "acceptance_required") this.status.textContent = "Independent validation is ready. Review it before using the fitted model.";
      if (phase === "accepted") this.status.textContent = "Model accepted for design.";
    }
    this.render();
  }

  private send(kind: string, payload: Record<string, unknown> = {}): boolean {
    this.pending = true;
    this.status.textContent = kind.endsWith("run") ? "Checking the complete recording…" : kind.endsWith("fit") ? this.fitRole === "validation" ? "Comparing with the model…" : "Fitting the plant model…" : "Using the fitted model…";
    this.render();
    this.post(kind, payload);
    return true;
  }

  private edit(): void {
    if (!this.selectedMethod || !this.hasRecording) return;
    this.post("control-studio.identify.edit", { form: this.readFormDraft() as unknown as Record<string, unknown> });
    this.status.textContent = "Identification settings changed. Check the recording before fitting.";
    this.stage = "imported";
    this.render();
  }

  private readFormDraft(): IdentificationFormDraft {
    const values = Object.fromEntries(FORM_KEYS.map(key => [key, this.input(key)?.value ?? ""])) as IdentificationFormDraft["values"];
    return { schemaVersion: 1, model: this.model.value as IdentificationModelSelection, heldStep: this.heldStep.checked, values,
      excludedStart: this.input("excluded-start")?.value ?? "", excludedEnd: this.input("excluded-end")?.value ?? "" };
  }

  private readStepResponseSettings(): StepResponseSettings | null | undefined {
    if (!this.heldStep.checked) return undefined;
    const values = (key: string): number => Number(this.input(key)?.value ?? "");
    const comparison = this.model.value === "sopdt";
    const settings: Record<string, unknown> = {
      baselineDurationSeconds: values("baselineDurationSeconds"), finalDurationSeconds: values("finalDurationSeconds"),
      maximumWindowRange: values("maximumWindowRange"), maximumWindowSlopePerSecond: values("maximumWindowSlopePerSecond"),
      measurementBound: values("measurementBound"), onsetThreshold: values("onsetThreshold"), onsetSamples: values("onsetSamples"),
      baselineStep: values("baselineStep"), gainStep: values("gainStep"), timestampShiftSeconds: values("timestampShiftSeconds"),
      delay: { minimum: values("delay.minimum"), maximum: values("delay.maximum"), step: values("delay.step") },
      timeConstant: { minimum: values("timeConstant.minimum"), maximum: values("timeConstant.maximum"), step: values("timeConstant.step") },
    };
    if (comparison) {
      settings.sensorTimeConstant = { minimum: values("sensorTimeConstant.minimum"), maximum: values("sensorTimeConstant.maximum"), step: values("sensorTimeConstant.step") };
      settings.modelAdequacy = { maximumAbsoluteResidual: values("modelAdequacy.maximumAbsoluteResidual"), maximumFinalError: values("modelAdequacy.maximumFinalError") };
    }
    const problem = stepResponseSettingsProblem(settings);
    if (!problem) return settings as unknown as StepResponseSettings;
    this.status.textContent = `${problem.field}: ${problem.message}`;
    return null;
  }

  private readExcludedIntervals(): Array<{ startSeconds: number; endSeconds: number }> | null {
    if (this.heldStep.checked) return [];
    const startText = this.input("excluded-start")?.value.trim() ?? "";
    const endText = this.input("excluded-end")?.value.trim() ?? "";
    if (!startText && !endText) return [];
    const start = Number(startText), end = Number(endText);
    if (!startText || !endText || !Number.isFinite(start) || !Number.isFinite(end) || start < 0 || end <= start) {
      this.status.textContent = "Enter an exclusion interval with a nonnegative start and an end strictly after it.";
      return null;
    }
    return [{ startSeconds: start, endSeconds: end }];
  }

  private restoreStudy(study: Record<string, unknown>): void {
    if (record(study.form)) { this.restoreForm(study.form); return; }
    const saved = record(study.current) ? study.current : undefined;
    if (!saved) return;
    const settings = record(saved.stepResponseSettings) ? saved.stepResponseSettings : undefined;
    const excluded = Array.isArray(saved.excluded) ? saved.excluded[0] : undefined;
    this.restoreForm({ model: saved.requestedKind, heldStep: Boolean(settings),
      values: Object.fromEntries(FORM_KEYS.map(key => {
        const [group, field] = key.split(".");
        const value = settings && (field ? settings[group]?.[field] : settings[group]);
        return [key, value === undefined ? this.input(key)?.defaultValue ?? "" : String(value)];
      })), excludedStart: excluded ? String(excluded.startSeconds) : "", excludedEnd: excluded ? String(excluded.endSeconds) : "" });
  }

  private restoreForm(form: Record<string, unknown>): void {
    if (typeof form.model === "string") this.model.value = form.model;
    this.heldStep.checked = form.heldStep === true;
    if (this.heldStep.checked && !["fopdt", "sopdt"].includes(this.model.value)) this.model.value = "fopdt";
    const values = record(form.values) ? form.values : {};
    for (const key of FORM_KEYS) if (typeof values[key] === "string") { const input = this.input(key); if (input) input.value = values[key]; }
    const start = this.input("excluded-start"); const end = this.input("excluded-end");
    if (typeof form.excludedStart === "string" && start) start.value = form.excludedStart;
    if (typeof form.excludedEnd === "string" && end) end.value = form.excludedEnd;
    this.render();
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
    for (const key of FORM_KEYS.filter(key => key.startsWith("sensorTimeConstant.") || key.startsWith("modelAdequacy."))) {
      const label = this.input(key)?.closest("label");
      if (label) label.hidden = this.model.value !== "sopdt";
    }
    for (const option of Array.from(this.model.options)) {
      option.disabled = this.heldStep.checked && !["fopdt", "sopdt"].includes(option.value);
    }
    const button = this.root.querySelector<HTMLButtonElement>("[data-engineer-find]");
    if (button) this.renderPrimaryButton(button);
    const open = this.root.querySelector<HTMLButtonElement>("[data-identify-import]");
    if (open) open.hidden = this.selectedMethod && (!this.hasRecording || this.stage === "validation_required");
  }

  private publishPresentation(payload: Record<string, unknown>): void {
    if (!payload.presentation && !payload.fit && !payload.model) return;
    window.dispatchEvent(new CustomEvent("trust-control-studio-identification-presentation", {
      detail: { presentation: payload.presentation, fit: payload.fit, model: payload.model, phase: payload.phase },
    }));
  }

  private renderPrimaryButton(button: HTMLButtonElement): void {
    if (!this.selectedMethod) return;
    button.textContent = this.primaryActionLabel;
    button.disabled = this.stage === "accepted" || (this.stage === "acceptance_required" && !this.acceptanceReady) || this.blocked() !== undefined;
  }


}

function record(value: unknown): value is Record<string, any> { return Boolean(value) && typeof value === "object" && !Array.isArray(value); }
function text(value: unknown): string | undefined { return typeof value === "string" && value.length > 0 ? plainReason(value) : undefined; }

function isSensorLagComparison(value: unknown): value is SensorLagComparison {
  return record(value) && record(value.candidateA) && record(value.candidateB) && record(value.requirements) &&
    typeof value.rmsImprovement === "number" && ["first_order", "additional_lag", "neither"].includes(String(value.recommended)) &&
    value.physicalOrigin === "unresolved" && value.provisional === true;
}

function acceptanceEvidenceIsReady(payload: Record<string, any>, comparison: SensorLagComparison | undefined): boolean {
  const validation = record(payload.validationQualification) && payload.validationQualification.suitable === true;
  if (!validation) return false;
  return !comparison || (comparison.recommended !== "neither" && comparison.choice === comparison.recommended);
}
