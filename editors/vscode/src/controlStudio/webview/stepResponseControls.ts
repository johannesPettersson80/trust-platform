import { stepResponseSettingsProblem, type StepResponseAnalysis, type StepResponseMeasurement, type StepResponseSettings } from "../stepResponseIdentification";
import { formatEngineeringNumber } from "../chartContract";
import { displayEngineeringUnit } from "../unitDisplay";
import { setIdentificationActionState } from "./identificationActions";
import { ALL_IDENTIFICATION_FORM_KEYS, type IdentificationFormDraft } from "../identificationFormDraft";
import { setWorkflowActionAvailability } from "./workflowActionAvailability";

const fields: Array<[string, string, number]> = [
  ["baselineDurationSeconds", "Baseline window (s)", 5], ["finalDurationSeconds", "Final window (s)", 5],
  ["maximumWindowRange", "Maximum window range (PV)", .1], ["maximumWindowSlopePerSecond", "Maximum window slope (PV/s)", .01],
  ["measurementBound", "Measurement bound ± (PV)", .05], ["onsetThreshold", "Departure threshold (PV)", .15],
  ["onsetSamples", "Consecutive samples", 3], ["baselineStep", "Baseline grid step (PV)", .01],
  ["gainStep", "Gain grid step (PV/MV)", .001], ["timestampShiftSeconds", "Sample time offset (s)", 0],
  ["delay.minimum", "Delay minimum (s)", 0], ["delay.maximum", "Delay maximum (s)", 2], ["delay.step", "Delay grid step (s)", .1],
  ["timeConstant.minimum", "Time constant minimum (s)", .1], ["timeConstant.maximum", "Time constant maximum (s)", 20], ["timeConstant.step", "Time constant grid step (s)", .1],
];
const comparisonFields: Array<[string, string, number]> = [
  ["sensorTimeConstant.minimum", "Additional time constant minimum (s)", .1],
  ["sensorTimeConstant.maximum", "Additional time constant maximum (s)", 10],
  ["sensorTimeConstant.step", "Additional time constant grid step (s)", .1],
  ["modelAdequacy.maximumAbsoluteResidual", "Maximum model residual (PV)", .2],
  ["modelAdequacy.maximumFinalError", "Maximum final model error (PV)", .1],
];
const allFields = [...fields, ...comparisonFields];

function updateStructureControls(): void {
  const model = document.querySelector<HTMLSelectElement>("#identify-model");
  if (!model) return;
  const held = Boolean(document.querySelector<HTMLInputElement>("[data-step-response-enabled]")?.checked);
  model.disabled = false;
  for (const option of model.options) {
    option.disabled = held && !["fopdt", "sopdt"].includes(option.value);
    option.dataset.originalLabel ??= option.textContent ?? "";
    option.textContent = held && option.value === "fopdt" ? "First-order reference"
      : held && option.value === "sopdt" ? "Compare additional lag" : option.dataset.originalLabel;
  }
  for (const field of document.querySelectorAll<HTMLElement>("[data-comparison-setting]")) field.hidden = !held || model.value !== "sopdt";
}

export function readIdentificationFormDraft(): IdentificationFormDraft {
  const values = Object.fromEntries(ALL_IDENTIFICATION_FORM_KEYS.map(key => [key,
    document.querySelector<HTMLInputElement>(`[data-step-field="${key}"]`)?.value ?? ""])) as IdentificationFormDraft["values"];
  return { schemaVersion: 1,
    model: document.querySelector<HTMLSelectElement>("#identify-model")?.value as IdentificationFormDraft["model"] ?? "automatic",
    heldStep: Boolean(document.querySelector<HTMLInputElement>("[data-step-response-enabled]")?.checked), values,
    excludedStart: document.querySelector<HTMLInputElement>("#identify-exclude-start")?.value ?? "",
    excludedEnd: document.querySelector<HTMLInputElement>("#identify-exclude-end")?.value ?? "" };
}

export function restoreIdentificationFormDraft(form: IdentificationFormDraft): void {
  const model = document.querySelector<HTMLSelectElement>("#identify-model");
  if (model) model.value = form.model;
  const enabled = document.querySelector<HTMLInputElement>("[data-step-response-enabled]");
  if (enabled) enabled.checked = form.heldStep;
  const settings = document.querySelector<HTMLElement>("[data-step-response-settings]");
  if (settings) settings.hidden = !form.heldStep;
  const exclusion = document.querySelector("#identify-exclude-start")?.closest("details") as HTMLElement | null;
  if (exclusion) exclusion.hidden = form.heldStep;
  for (const key of ALL_IDENTIFICATION_FORM_KEYS) {
    const input = document.querySelector<HTMLInputElement>(`[data-step-field="${key}"]`);
    if (input) input.value = form.values[key] ?? String(allFields.find(([name]) => name === key)?.[2] ?? "");
  }
  const start = document.querySelector<HTMLInputElement>("#identify-exclude-start");
  const end = document.querySelector<HTMLInputElement>("#identify-exclude-end");
  if (start) start.value = form.excludedStart;
  if (end) end.value = form.excludedEnd;
  updateStructureControls();
}

export function readStepResponseSettings(): StepResponseSettings | null | undefined {
  if (!document.querySelector<HTMLInputElement>("[data-step-response-enabled]")?.checked) return undefined;
  clearStepResponseErrors();
  const comparison = document.querySelector<HTMLSelectElement>("#identify-model")?.value === "sopdt";
  const settings: Record<string, any> = { delay: {}, timeConstant: {}, ...(comparison ? { sensorTimeConstant: {}, modelAdequacy: {} } : {}) };
  for (const [key] of comparison ? allFields : fields) {
    const input = document.querySelector<HTMLInputElement>(`[data-step-field="${key}"]`);
    const value = input?.value.trim() ? Number(input.value) : NaN;
    const [group, field] = key.split(".");
    if (field) settings[group][field] = value; else settings[group] = value;
  }
  const problem = stepResponseSettingsProblem(settings);
  if (!problem) return settings as StepResponseSettings;
  const input = document.querySelector<HTMLInputElement>(`[data-step-field="${problem.field}"]`);
  const label = document.querySelector(`[data-step-label="${problem.field}"]`)?.textContent ?? problem.field;
  if (input) {
    const error = document.createElement("small"); error.className = "error"; error.dataset.stepError = "";
    error.id = `step-error-${problem.field.replace(/\./g, "-")}`; error.textContent = problem.message; error.setAttribute("role", "alert");
    input.after(error); input.setAttribute("aria-invalid", "true"); input.setAttribute("aria-describedby", error.id);
    input.focus(); input.scrollIntoView({ block: "center" });
  }
  setWorkflowActionAvailability({ identificationFailed: true });
  const status = document.querySelector<HTMLElement>("[data-identify-state]");
  if (status) status.textContent = `Review the ${label.charAt(0).toLowerCase() + label.slice(1)}: ${problem.message}`;
  return null;
}

function clearStepResponseErrors(): void {
  document.querySelectorAll("[data-step-error]").forEach(error => error.remove());
  document.querySelectorAll("[data-step-field]").forEach(input => { input.removeAttribute("aria-invalid"); input.removeAttribute("aria-describedby"); });
}

export function initializeStepResponseControls(post?: (kind: string, payload: Record<string, unknown>) => void): void {
  const root = document.querySelector<HTMLElement>("[data-control-studio]");
  const pane = root?.querySelector<HTMLElement>('[data-workflow-pane="Identify"]');
  const model = pane?.querySelector<HTMLSelectElement>("#identify-model");
  if (!root || !pane || !model) return;
  const controls = document.createElement("div");
  controls.innerHTML = `<label style="display:flex;align-items:center;gap:6px"><input style="width:auto;margin:0" type="checkbox" data-step-response-enabled><span>Measure a held step</span></label>
    <details class="advanced" data-step-response-settings hidden><summary>Measurement and search settings</summary>
    <p>PV and MV use the recording's units. A nonzero time offset checks timestamp sensitivity.</p><div class="parameter-grid">${allFields.map(([key, label, value]) =>
      `<label${comparisonFields.some(([name]) => name === key) ? " data-comparison-setting hidden" : ""}><span data-step-label="${key}">${label}</span><input data-step-field="${key}" type="number" step="any" value="${value}"></label>`).join("")}</div></details>`;
  model.closest(".field")?.after(controls);
  const evidence = document.createElement("details"); evidence.className = "advanced";
  evidence.dataset.stepResponseEvidence = ""; evidence.hidden = true;
  evidence.innerHTML = '<summary>Step-response measurements</summary><div class="chart-table" style="max-height:none;overflow:visible"><table><tbody data-step-response-report></tbody></table></div>';
  pane.append(evidence);
  const enabled = controls.querySelector<HTMLInputElement>("[data-step-response-enabled]")!;
  const exclusion = pane.querySelector("#identify-exclude-start")?.closest("details");
  const invalidate = () => {
    clearStepResponseErrors();
    evidence.hidden = true;
    const hasRecording = pane.querySelector<HTMLButtonElement>("[data-identify-run]")?.disabled === false;
    setWorkflowActionAvailability({ model: false, candidate: false, identificationFailed: false });
    setIdentificationActionState(hasRecording ? "imported" : "initial");
    const status = pane.querySelector("[data-identify-state]");
    if (status) status.textContent = "Analysis settings changed. Check the recording before fitting.";
    post?.("control-studio.identify.edit", { form: readIdentificationFormDraft() });
  };
  enabled.addEventListener("change", () => {
    controls.querySelector<HTMLElement>("[data-step-response-settings]")!.hidden = !enabled.checked;
    if (exclusion) exclusion.hidden = enabled.checked;
    if (enabled.checked) model.value = "fopdt";
    updateStructureControls();
    invalidate();
  });
  controls.querySelectorAll("[data-step-field]").forEach(input => input.addEventListener("input", invalidate));
  model.addEventListener("change", () => { updateStructureControls(); invalidate(); });
  pane.querySelectorAll("#identify-exclude-start, #identify-exclude-end").forEach(input => input.addEventListener("input", invalidate));
  window.addEventListener("message", event => {
    const message = event.data;
    if (message?.projectId !== root.dataset.projectId || message.loopId !== root.dataset.loopId ||
      message.editorSessionId !== root.dataset.editorSessionId || message.expectedLoopRevision !== root.dataset.loopRevision) return;
    if (message.kind === "control-studio.identification-recording" || message.kind === "control-studio.workflow-result") {
      if (message.kind === "control-studio.workflow-result" && ["restored", "opened"].includes(message.phase)
        || message.kind === "control-studio.identification-recording" && message.payload?.reference) {
        const study = message.payload?.reference ? { current: message.payload.reference } : message.payload?.result?.identificationStudy;
        const saved = study?.current;
        if (study?.form) restoreIdentificationFormDraft(study.form as IdentificationFormDraft);
        const settings = saved?.stepResponseSettings;
        enabled.checked = study?.form ? study.form.heldStep : Boolean(settings);
        controls.querySelector<HTMLElement>("[data-step-response-settings]")!.hidden = study?.form ? !study.form.heldStep : !settings;
        if (exclusion) exclusion.hidden = study?.form ? study.form.heldStep : Boolean(settings);
        if (saved && !study?.form) model.value = saved.requestedKind;
        if (settings && !study?.form) restoreIdentificationFormDraft({ schemaVersion: 1, model: saved.requestedKind, heldStep: true,
          values: Object.fromEntries(allFields.map(([key, , defaultValue]) => {
            const [group, field] = key.split(".");
            return [key, String((field ? settings[group]?.[field] : settings[group]) ?? defaultValue)];
          })) as IdentificationFormDraft["values"], excludedStart: "", excludedEnd: "" });
        updateStructureControls();
      }
      const units = message.payload?.recording?.rawSamples?.[0]?.units ?? message.payload?.result?.units;
      if (units) for (const [key, label] of allFields) {
        const span = controls.querySelector(`[data-step-label="${key}"]`);
        if (span) span.textContent = label.replace(/PV/g, displayEngineeringUnit(units.pv)).replace(/MV/g, displayEngineeringUnit(units.mv));
      }
      evidence.hidden = true; return;
    }
    if (message.kind !== "control-studio.engineering.result" || message.stage !== "identify") return;
    const payload = message.payload;
    const qualification = payload?.validationQualification ?? payload?.qualification ?? payload?.estimationQualification;
    const measurement = payload?.fit?.stepResponseMeasurement ?? qualification?.stepResponse ?? payload?.fit?.stepResponse;
    if (!measurement) { evidence.hidden = true; return; }
    renderStepResponse(measurement, qualification?.outputUnit ?? payload?.presentation?.units?.output ?? "PV",
      qualification?.inputUnit ?? payload?.presentation?.units?.input ?? "MV", evidence, payload?.fit?.stepResponse,
      Boolean(payload?.validationQualification || payload?.nextFitRole === "validation"));
  });
}

export function renderStepResponse(analysis: StepResponseMeasurement, output: string, input: string, evidence: HTMLDetailsElement,
  estimation?: StepResponseAnalysis, validation = false): void {
  const n = (value: number | undefined) => value === undefined ? "Not detected" : formatEngineeringNumber(value);
  const unit = displayEngineeringUnit(output), gainUnit = `${unit}/${displayEngineeringUnit(input)}`;
  const rows: Array<[string, string]> = [];
  for (const [name, window] of [["Baseline", analysis.baseline], ["Final", analysis.final]] as const) {
    rows.push([name, `${n(window.mean)} ${unit} · ${n(window.startSeconds)}–${n(window.endSeconds)} s · ${window.count} samples`],
      [`${name} range / slope`, `${n(window.range)} ${unit} / ${n(window.slopePerSecond)} ${unit}/s`]);
  }
  rows.push(["Applied change", `${n(analysis.inputChange)} ${displayEngineeringUnit(input)}`],
    ["Step / restoration", `${n(analysis.stepTimeSeconds)} / ${n(analysis.holdEndSeconds)} s`],
    ["Detectable onset", `${n(analysis.detectableOnsetSeconds)} s after step · ${n(analysis.sampleTimeSeconds)} s sampling`],
    ["Departure / 63.2% level", `${n(analysis.onsetThreshold)} / ${n(analysis.level63)} ${unit}`],
    ["Onset criterion", `${analysis.settings.onsetSamples} consecutive samples · ${n(analysis.settings.onsetThreshold)} ${unit} ${analysis.final.mean < analysis.baseline.mean ? "below" : "above"} baseline`]);
  if (estimation) {
    rows.push([validation ? "Estimation gain" : "Best gain", `${n(estimation.best.gain)} ${gainUnit}`], [validation ? "Estimation delay" : "Transport delay", `${n(estimation.best.delaySeconds)} s`],
      [validation ? "Estimation time constant" : "Time constant", `${n(estimation.best.timeConstantSeconds)} s`], [validation ? "Estimation consistent grid" : "Consistent grid", `${estimation.consistency.count} candidates · ±${n(estimation.settings.measurementBound)} ${unit}`]);
    for (const [label, range, unit] of [["Gain range", estimation.consistency.gain, gainUnit], ["Delay range", estimation.consistency.delaySeconds, "s"],
      ["Time constant range", estimation.consistency.timeConstantSeconds, "s"]] as const) {
      rows.push([label, range ? `${n(range.minimum)}–${n(range.maximum)} ${unit}` : "No consistent candidates"]);
    }
    rows.push(["Range meaning", "Grid-resolved marginal ranges; not statistical confidence"]);
  }
  if (analysis.warnings.length) rows.push(["Review", analysis.warnings.join(" ")]);
  evidence.querySelector("tbody")!.replaceChildren(...rows.map(([name, value]) => {
    const row = document.createElement("tr"), heading = document.createElement("th"), cell = document.createElement("td");
    heading.scope = "row"; heading.textContent = name; cell.textContent = value; row.append(heading, cell); return row;
  }));
  evidence.querySelector("summary")!.textContent = validation ? "Validation step measurements" : "Step-response measurements";
  evidence.hidden = false;
}
