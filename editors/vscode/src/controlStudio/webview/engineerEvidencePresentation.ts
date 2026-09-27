import { formatEngineeringNumber } from "../chartContract";
import type { CommissioningResult } from "../proportionalCommissioning";
import type { SavedEngineerFamilyEvaluation } from "../engineerStudyPersistence";
import type { AutomaticControllerAttempt } from "../automaticControllerRecommendation";
import { renderControllerComparison, renderHistoricalControllerComparison } from "./controllerComparisonPresentation";
import { renderHistoricalParameterComparison, renderObservedValues, renderParameterComparison } from "./parameterComparisonPresentation";
import type { EngineerRecordedResponses, Evaluation } from "./engineerRecordedResponses";

type EvidencePayload = Record<string, unknown>;
type ResponseRenderer = Pick<EngineerRecordedResponses, "render" | "renderScenario" | "renderCommissioning">;

/**
 * Owns the compact, stage-specific Details surface. The returned function is
 * intentionally stateful: restored payloads frequently omit the model after
 * the first event, but the Model section must continue to identify it.
 */
export function initializeEngineerEvidencePresentation(
  evidence: HTMLElement,
  responses: ResponseRenderer,
): (payload: EvidencePayload, status?: string) => void {
  let selectedModel: Record<string, unknown> | undefined;
  let cachedPayload: EvidencePayload = {};
  try {
    const encoded = document.querySelector<HTMLElement>("[data-engineer-workflow]")?.dataset.engineerParameterDescriptors;
    if (encoded) cachedPayload.parameterDescriptors = JSON.parse(decodeURIComponent(encoded));
  } catch { /* Payload metadata remains available for older restored views. */ }
  let renderedVerificationKey: string | undefined;
  let renderedCommissioningKey: string | undefined;
  let modelCurrent = false;
  let designCurrent = false;
  let verificationCurrent = false;

  return (payload, status = String(payload.status ?? "")) => {
    if (isRecord(payload.model)) {
      if (selectedModel?.revision !== payload.model.revision || status === "model-ready") delete cachedPayload.identificationPresentation;
      selectedModel = payload.model; modelCurrent = true;
    }
    cachedPayload = { ...cachedPayload, ...payload };
    const effective = cachedPayload;
    const readiness = isRecord(payload.stageEvidence) ? payload.stageEvidence : {};
    if (typeof readiness.hasModel === "boolean") modelCurrent = readiness.hasModel;
    if (typeof readiness.hasCandidate === "boolean") designCurrent = readiness.hasCandidate;
    if (typeof readiness.validationHasResult === "boolean") verificationCurrent = readiness.validationHasResult;
    if (["edited", "cancelled", "model-ready", "unavailable", "restored"].includes(status)) { designCurrent = false; verificationCurrent = false; }
    if (status === "candidate") { designCurrent = true; verificationCurrent = false; }
    if (["verified", "verification-failed"].includes(status) && payload.baseline && payload.candidate) { designCurrent = true; verificationCurrent = true; }
    if (status === "failed") verificationCurrent = false;
    const historical = !verificationCurrent;
    evidence.replaceChildren();

    evidence.append(stage("model", "Model", modelGroup(selectedModel, effective, !modelCurrent)));
    evidence.append(stage("tune", "Tune", tuneGroup(effective, !designCurrent)));
    evidence.append(stage("verify", "Verify", verifyGroup(effective, responses, status, historical, () => {
      const baseline = evaluation(effective.baseline);
      const candidate = evaluation(effective.candidate);
      if (!baseline || !candidate || historical) return;
      const identity = (value: Evaluation): string => {
        const run = value.run as unknown as Record<string, unknown>;
        return `${run.id ?? ""}:${run.revision ?? ""}:${value.run.conditions?.modelRevision}:${value.run.conditions?.programRevision}:${value.run.samples.length}`;
      };
      const key = `${effective.baseline === payload.baseline ? "current" : "saved"}:${identity(baseline)}:${identity(candidate)}`;
      if (key === renderedVerificationKey) return;
      renderedVerificationKey = key;
      responses.render(baseline, candidate);
    })));
    evidence.append(stage("apply", "Apply", applyGroup(effective, status, responses, (commissioning) => {
      const key = `${commissioning.status}:${commissioning.recording?.id ?? ""}:${commissioning.recording?.revision ?? ""}:${commissioning.recording?.rawSamples?.length ?? 0}:${commissioning.recoveryConfirmed}`;
      if (key === renderedCommissioningKey) return;
      renderedCommissioningKey = key;
      responses.renderCommissioning(commissioning);
    })));
  };
}

function stage(id: string, title: string, content: HTMLElement[]): HTMLElement {
  const section = document.createElement("section");
  section.dataset.engineerEvidenceStage = id;
  section.setAttribute("aria-labelledby", `engineer-evidence-${id}`);
  const heading = document.createElement("p");
  heading.className = "engineer-evidence-stage-label";
  heading.setAttribute("role", "heading");
  heading.setAttribute("aria-level", "3");
  heading.id = `engineer-evidence-${id}`;
  heading.textContent = title;
  section.append(heading, ...content);
  return section;
}

function modelGroup(model: Record<string, unknown> | undefined, payload: EvidencePayload, historical: boolean): HTMLElement[] {
  if (!model) return [message("No model selected yet. Find or supply a model to begin.")];
  const values: Array<[string, string]> = [
    ["Source", text(model.source ?? payload.source ?? "Available")],
    ["Validation", text(model.validation ?? payload.validation ?? payload.reason ?? "Ready")],
    ["Inputs", channels(model.inputs ?? model.inputChannels)],
    ["Outputs", channels(model.outputs ?? model.outputChannels)],
  ];
  const result = [table(values)];
  const comparison = isRecord(payload.identificationPresentation) ? payload.identificationPresentation : undefined;
  if (comparison) {
    const validation = isRecord(comparison.validation) ? comparison.validation : undefined;
    if (typeof validation?.normalizedError === "number") result.push(table([["Independent fit error", `${formatEngineeringNumber(validation.normalizedError * 100, 4)}% of recorded PV span`]]));
    result.push(disclosure("Model representation", text(comparison.equation)));
  }
  if (historical) result.push(message("Previously selected model. Choose a current model before tuning."));
  const identity = [model.revision, model.modelRevision, model.programRevision].filter(value => value !== undefined);
  if (identity.length) result.push(disclosure("Model and revision identity", identity.map(value => text(value)).join(" · ")));
  if (model.operatingPoint) result.push(disclosure("Operating point", formatValue(model.operatingPoint)));
  return result;
}

function tuneGroup(payload: EvidencePayload, historical: boolean): HTMLElement[] {
  const result: HTMLElement[] = [];
  const task = isRecord(payload.task) ? payload.task : undefined;
  const draft = isRecord(payload.draft) ? payload.draft : undefined;
  const design = isRecord(payload.design) ? payload.design : undefined;
  const settings: Array<[string, string]> = [];
  for (const [key, label] of [["family", "Controller"], ["tuningMethod", "Method"], ["tuningObjective", "Objective"], ["applicationMode", "Application mode"]] as const) {
    const value = task?.[key] ?? payload[key];
    if (value !== undefined) settings.push([label, text(value)]);
  }
  if (draft && isRecord(draft.values)) settings.push(["Retained draft", entries(draft.values)]);
  if (settings.length) result.push(table(settings));
  const constraints = [
    ["Target change", payload.referenceChange], ["Maximum error", payload.maximumError],
    ["Settling band", payload.settlingBand], ["Settling time", payload.settlingTimeSeconds],
    ["Transfer tolerance", payload.transferTolerance], ["Disturbance", payload.constantInputDisturbance],
    ["Measurement noise", payload.measurementNoiseBand], ["Gain uncertainty", payload.gainUncertaintyFraction],
  ].filter(([, value]) => value !== undefined) as Array<[string, unknown]>;
  if (constraints.length) result.push(table(constraints.map(([label, value]) => [label, text(value)])));
  if (design) {
    const candidate = isRecord(design.candidate) ? design.candidate : undefined;
    if (candidate?.proportionalGain !== undefined) result.push(message(`Unapplied P gain: ${formatEngineeringNumber(Number(candidate.proportionalGain))}.`));
    const formulas = isRecord(design.formulas) ? design.formulas : undefined;
    const lines = [...arrayOfStrings(formulas?.symbolic), ...arrayOfStrings(formulas?.substituted)];
    if (lines.length) result.push(disclosure("Design formulas", lines.join(" ")));
  }
  const stability = isRecord(payload.stability) ? payload.stability : undefined;
  if (stability) result.push(disclosure("Stability derivation", formatValue(stability)));
  const runs = Array.isArray(payload.tuningRuns) ? payload.tuningRuns : [];
  if (runs.length) result.push(disclosure(`Retained native tuning runs (${runs.length})`, runs.map(formatValue).join(" ")));
  if (!result.length) result.push(message("Tune settings and constraints will appear here after a model is ready."));
  if (historical) result.unshift(message("Retained settings are historical. Tune again before fresh verification."));
  return result;
}

function verifyGroup(payload: EvidencePayload, responses: ResponseRenderer, status: string, historical: boolean, renderVerification: () => void): HTMLElement[] {
  const result: HTMLElement[] = [];
  const baseline = evaluation(payload.baseline);
  const candidate = evaluation(payload.candidate);
  if (baseline && candidate) {
    renderControllerComparisonFragment(result, payload);
    renderVerification();
    if (historical) result.push(message("Historical verification retained; current qualification is invalid until Verify runs again."));
  } else if (Array.isArray(payload.familyEvaluations)) {
    const holder = document.createElement("div");
    renderHistoricalControllerComparison(holder, payload.familyEvaluations as SavedEngineerFamilyEvaluation[]);
    result.push(holder);
  }
  const scenarios = Array.isArray(payload.scenarioResults) ? payload.scenarioResults : [];
  for (const entry of scenarios) {
    if (!isRecord(entry)) continue;
    const evaluated = isRecord(entry.result) && evaluation(entry.result.evaluation);
    if (!evaluated) continue;
    const label = text(entry.id ?? "Scenario");
    result.push(table([[label, evaluated.status]]));
    const button = document.createElement("button");
    button.type = "button";
    button.className = "trust-button";
    button.textContent = "View scenario response";
    button.addEventListener("click", () => responses.renderScenario(label, evaluated));
    result.push(button);
    result.push(requirementsTable(evaluated));
  }
  for (const [role, evaluationValue] of [["Baseline", baseline], ["Candidate", candidate]] as const) {
    if (!evaluationValue) continue;
    result.push(table([[role, `${evaluationValue.status} · ${evaluationValue.run.evidence}`]]));
    result.push(requirementsTable(evaluationValue));
  }
  if (status === "verification-failed") result.push(message("Verification did not pass. Review each requirement and run again after correcting the settings."));
  if (!result.length) result.push(message("Verify records per-scenario requirements and actual ST evidence here."));
  return result;
}

function applyGroup(
  payload: EvidencePayload,
  status: string,
  _responses: ResponseRenderer,
  renderCommissioning: (result: CommissioningResult & { recording?: { rawSamples?: unknown[]; id?: unknown; revision?: unknown }; target?: unknown }) => void,
): HTMLElement[] {
  const result: HTMLElement[] = [];
  const observed = isRecord(payload.observed) ? payload.observed.values : payload.observed;
  const comparisonKeys = new Set([...Object.keys(isRecord(payload.previous) ? payload.previous : {}), ...Object.keys(isRecord(payload.next) ? payload.next : {})]);
  const redundantObservation = status === "review" && isRecord(observed) && Object.keys(observed).every(key => comparisonKeys.has(key));
  if (observed && !redundantObservation) {
    const observedTable = renderObservedValues(observed, payload.parameterDescriptors);
    if (observedTable) result.push(observedTable);
  }
  if (isRecord(payload.initialization)) result.push(table([["Transfer verification", text(payload.initialization.status ?? "Unavailable") + " · actual ST."]]));
  if (isRecord(payload.previous) || isRecord(payload.next)) {
    const comparison = { previous: payload.previous, next: payload.next, parameterDescriptors: payload.parameterDescriptors };
    if (status === "review") result.push(...renderParameterComparison(comparison));
    else {
      const historical = renderHistoricalParameterComparison(comparison);
      if (historical) result.push(historical);
    }
  }
  if (isRecord(payload.commissioning)) {
    const commissioning = payload.commissioning as unknown as CommissioningResult & { recording?: { rawSamples?: unknown[]; id?: unknown; revision?: unknown }; target?: unknown };
    result.push(table([["Recorded run", `${commissioning.recording?.rawSamples?.length ?? 0} samples · target ${text(commissioning.target)}`], ["Recovery", commissioning.recoveryConfirmed === false ? "Recovery required" : "Confirmed"]]));
    renderCommissioning(commissioning);
    responsesSafeCommissioning(result, commissioning);
  }
  if (payload.coordinates) result.push(disclosure("Application coordinates", text(payload.coordinates)));
  if (!result.length) result.push(message(status === "review" ? "Review the current and proposed application before confirming." : "Observed adoption, transfer and recovery records will appear here after Apply."));
  return result;
}

function renderControllerComparisonFragment(target: HTMLElement[], payload: EvidencePayload): void {
  if (!Array.isArray(payload.attempts)) return;
  const holder = document.createElement("div");
  renderControllerComparison(holder, payload.attempts as AutomaticControllerAttempt[]);
  target.push(holder);
}

function responsesSafeCommissioning(target: HTMLElement[], result: CommissioningResult): void {
  // The chart is mounted by EngineerRecordedResponses in the shared monitor
  // workspace; this marker keeps the Apply record discoverable in Details.
  const marker = document.createElement("p");
  marker.dataset.engineerCommissioningRecord = "";
  marker.textContent = `Commissioning status: ${result.status}. The full native recording is available in the recorded-response workspace.`;
  target.push(marker);
}

function requirementsTable(evaluation: Evaluation): HTMLElement {
  const recorded = (evaluation as unknown as { requirements?: unknown }).requirements;
  const requirements = Array.isArray(recorded)
    ? recorded
    : Array.isArray(evaluation.specification) ? evaluation.specification : [];
  if (!requirements.length) return message("No requirement rows were recorded.");
  return table(requirements.map(requirement => {
    if (!isRecord(requirement)) return ["Requirement", text(requirement)];
    const limit = requirement.limit !== undefined ? ` · limit ${formatEngineeringNumber(Number(requirement.limit))}` : "";
    const measured = requirement.value !== undefined ? ` · ${formatEngineeringNumber(Number(requirement.value))} ${text(requirement.unit)}` : "";
    const reason = requirement.reason ? ` · ${text(requirement.reason)}` : "";
    return [text(requirement.id ?? requirement.kind), `${text(requirement.status ?? requirement.kind)}${measured}${limit}${reason}`];
  }));
}

function evaluation(value: unknown): Evaluation | undefined {
  if (!isRecord(value) || !isRecord(value.run) || !Array.isArray(value.run.channels) || !Array.isArray(value.run.samples)) return undefined;
  return value as unknown as Evaluation;
}

function table(rows: Array<[string, string]>): HTMLElement {
  const table = document.createElement("table");
  table.className = "engineer-evidence-table";
  const body = document.createElement("tbody");
  for (const [label, value] of rows) {
    const row = document.createElement("tr");
    const heading = document.createElement("th"); heading.scope = "row"; heading.textContent = label;
    const cell = document.createElement("td"); cell.textContent = value;
    row.append(heading, cell); body.append(row);
  }
  table.append(body); return table;
}

function disclosure(summaryText: string, value: string): HTMLElement {
  const details = document.createElement("details");
  const summary = document.createElement("summary"); summary.textContent = summaryText;
  const body = document.createElement("p"); body.textContent = value;
  details.append(summary, body); return details;
}

function message(value: string): HTMLElement { const p = document.createElement("p"); p.textContent = value; return p; }
function channels(value: unknown): string { return Array.isArray(value) ? value.map(channel => isRecord(channel) ? `${text(channel.id ?? channel.name ?? "channel")} (${text(channel.unit ?? "")})` : text(channel)).join(", ") || "None" : "Unavailable"; }
function entries(value: Record<string, unknown>): string { return Object.entries(value).map(([key, item]) => `${key} = ${formatEngineeringNumber(Number(item))}`).join(", "); }
function arrayOfStrings(value: unknown): string[] { return Array.isArray(value) ? value.filter(item => typeof item === "string") : []; }
function formatValue(value: unknown): string {
  if (typeof value === "string") return value;
  if (typeof value === "number") return formatEngineeringNumber(value);
  if (Array.isArray(value)) return value.map(item => formatValue(item)).join(", ");
  if (isRecord(value)) return Object.entries(value).map(([key, item]) => `${key}: ${formatValue(item)}`).join(" · ");
  return text(value);
}
function text(value: unknown): string { return value === undefined || value === null || value === "" ? "Unavailable" : String(value); }
function isRecord(value: unknown): value is Record<string, unknown> { return Boolean(value && typeof value === "object" && !Array.isArray(value)); }
