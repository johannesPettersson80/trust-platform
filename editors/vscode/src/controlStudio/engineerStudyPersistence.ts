import { decodeIndependentModelValidation, type IndependentModelValidation } from "./independentModelValidation";
import { decodeConnectedIdentificationRecord } from "./connectedIdentificationRecord";
import { validLoopChannels, type LoopChannels } from "./loopChannels";
import { decodeModelComparison, type ModelComparison } from "./modelComparison";
import { controllerTuningPreferences, type ControllerTuningPreferences } from "./controllerTuningPreferences";
import { acquisitionSettings, type AcquisitionSettings } from "./acquisitionSettings";
import { decodeRecordingCatalog, type RecordingSignalCatalog } from "./openedRecordingChannels";
import { validEngineerScenarios, type EngineerScenario } from "./engineerVerificationCampaign";
import { isControllerFamily, CONTROLLER_FAMILIES, type ControllerFamily } from "./controllerFamilies";
import { decodeEngineerDraft, type EngineerDraft } from "./engineerDraft";
import type { EngineerScenarioResult } from "./engineerVerificationCampaign";
import { decodeControllerSettingsPins, type ControllerSettingsPins } from "./controllerSettingsPins";
import { decodeEngineerGainChoice, type EngineerGainChoice } from "./engineerGainChoice";
import { type IdentificationCampaignRecord } from "./automaticIdentification";
import { createHash } from "crypto";
import { engineerRequirements, type EngineerRequirements } from "./engineerVerificationPlan";
import { decodeGeneralModel, type GeneralModel } from "./generalModel";
import { evaluateRecordedRun } from "./runValidation";
import type { RuntimeVerificationAttempt, RuntimeVerificationResult } from "./verificationClient";

const MAX_CAMPAIGNS = 16;
const MAX_CAMPAIGN_RUNS = 16;
export const MAX_ENGINEER_STUDY_BYTES = 16 * 1024 * 1024;

export interface SavedEngineerCampaign extends IdentificationCampaignRecord {
  id: string;
}

export interface SavedEngineerFamilyEvaluation {
  family: ControllerFamily;
  status: "model-qualified" | "actual-pass" | "actual-fail" | "unavailable";
  reason: string;
}

/** Historical evidence only; it deliberately has no runtime client, job, review, or approval state. */
export interface EngineerStudy {
  recordingCatalog?: RecordingSignalCatalog;
  tuningPreferences?: ControllerTuningPreferences;
  acquisitionSettings?: AcquisitionSettings;
  scenarioDefinitions?: readonly EngineerScenario[];
  selectedModel?: GeneralModel;
  modelComparison?: ModelComparison;
  draft?: EngineerDraft;
  tuningRuns?: readonly RuntimeVerificationResult[];
  modelValidation?: IndependentModelValidation;
  modelPrediction?: { baseline: RuntimeVerificationResult; candidate: RuntimeVerificationResult };
  settingsPins?: ControllerSettingsPins;
  channelMapping?: LoopChannels;
  requiredScenarios?: readonly string[];
  scenarioResults?: readonly EngineerScenarioResult[];
  task?: EngineerRequirements;
  family?: ControllerFamily;
  controllerChoice?: "automatic" | ControllerFamily;
  gainChoice?: EngineerGainChoice;
  verificationAttempts?: readonly RuntimeVerificationAttempt[];
  campaigns: readonly SavedEngineerCampaign[];
  baseline?: RuntimeVerificationResult;
  candidate?: RuntimeVerificationResult;
  initialization?: RuntimeVerificationResult;
  familyEvaluations?: readonly SavedEngineerFamilyEvaluation[];
  applicationMode?: "auto" | "manual";
}

export function decodeEngineerStudy(value: unknown): EngineerStudy | undefined {
  if (!record(value) || !onlyKeys(value, ["modelValidation", "recordingCatalog", "channelMapping", "requiredScenarios", "tuningPreferences", "modelPrediction", "modelComparison", "acquisitionSettings", "scenarioDefinitions", "selectedModel", "tuningRuns", "draft", "scenarioResults", "settingsPins", "task", "family", "controllerChoice", "gainChoice", "verificationAttempts", "campaigns", "baseline", "candidate", "initialization", "familyEvaluations", "applicationMode"]) ||
      !Array.isArray(value.campaigns) || value.campaigns.length > MAX_CAMPAIGNS ||
      (value.controllerChoice !== undefined && value.controllerChoice !== "automatic" && !isControllerFamily(value.controllerChoice)) ||
      (value.family !== undefined && !isControllerFamily(value.family)) ||
      (value.applicationMode !== undefined && value.applicationMode !== "auto" && value.applicationMode !== "manual")) return undefined;
  if (!withinSerializedLimit(value)) return undefined;
  const recordingCatalog = value.recordingCatalog === undefined ? undefined : decodeRecordingCatalog(value.recordingCatalog);
  if (value.recordingCatalog !== undefined && !recordingCatalog) return undefined;
  if (value.channelMapping !== undefined && !validLoopChannels(value.channelMapping)) return undefined;
  if (value.requiredScenarios !== undefined && (!Array.isArray(value.requiredScenarios) || value.requiredScenarios.length > 66 || value.requiredScenarios.some(id => typeof id !== "string" || !id || id.length > 256) || new Set(value.requiredScenarios).size !== value.requiredScenarios.length)) return undefined;
  if (value.acquisitionSettings !== undefined && !acquisitionSettings(value.acquisitionSettings)) return undefined;
  if (value.scenarioDefinitions !== undefined && !validEngineerScenarios(value.scenarioDefinitions)) return undefined;
  const tuningRuns = value.tuningRuns === undefined ? undefined : Array.isArray(value.tuningRuns) ? value.tuningRuns.map(decodeRuntimeVerificationResult) : [undefined];
  if (tuningRuns && (tuningRuns.length > 256 || tuningRuns.some(run => !run))) return undefined;
  const modelPrediction = decodeModelPrediction(value.modelPrediction);
  if (value.modelPrediction !== undefined && !modelPrediction) return undefined;
  const draft = value.draft === undefined ? undefined : decodeEngineerDraft(value.draft);
  if (value.draft !== undefined && !draft) return undefined;
  const scenarioResults = value.scenarioResults === undefined ? undefined : Array.isArray(value.scenarioResults)
    ? value.scenarioResults.map(entry => record(entry) && text(entry.id) && decodeRuntimeVerificationResult(entry.result)
      ? { id: entry.id, result: decodeRuntimeVerificationResult(entry.result)! } : undefined) : [undefined];
  if (scenarioResults && (scenarioResults.length > 64 || scenarioResults.some(entry => !entry)
    || new Set(scenarioResults.map(entry => entry!.id)).size !== scenarioResults.length)) return undefined;
  const tuningPreferences = value.tuningPreferences === undefined ? undefined : controllerTuningPreferences(value.tuningPreferences);
  if (value.tuningPreferences !== undefined && !tuningPreferences) return undefined;
  const settingsPins = value.settingsPins === undefined ? undefined : decodeControllerSettingsPins(value.settingsPins);
  if (value.settingsPins !== undefined && !settingsPins) return undefined;
  const selectedModel = value.selectedModel === undefined ? undefined : decodeGeneralModel(value.selectedModel);
  const modelComparison = value.modelComparison === undefined ? undefined : decodeModelComparison(value.modelComparison, selectedModel);
  if (value.modelComparison !== undefined && !modelComparison) return undefined;
  const task = value.task === undefined ? undefined : decodeTask(value.task);
  const gainChoice = value.gainChoice === undefined ? undefined : decodeEngineerGainChoice(value.gainChoice, selectedModel?.revision);
  const attempts = value.verificationAttempts === undefined ? undefined : Array.isArray(value.verificationAttempts)
    ? value.verificationAttempts.map(decodeVerificationAttempt) : [undefined];
  if (attempts && (attempts.length > 256 || attempts.some(attempt => !attempt) ||
    new Set(attempts.map(attempt => `${attempt!.projectRevision}:${attempt!.jobId}`)).size !== attempts.length)) return undefined;
  const campaigns = value.campaigns.map(decodeCampaign);
  const baseline = value.baseline === undefined ? undefined : decodeRuntimeVerificationResult(value.baseline);
  const candidate = value.candidate === undefined ? undefined : decodeRuntimeVerificationResult(value.candidate);
  const initialization = value.initialization === undefined ? undefined : decodeRuntimeVerificationResult(value.initialization);
  const familyEvaluations = value.familyEvaluations === undefined ? undefined : decodeFamilyEvaluations(value.familyEvaluations);
  if ((value.gainChoice !== undefined && !gainChoice) || (value.selectedModel !== undefined && !selectedModel) || (value.task !== undefined && !task) ||
      campaigns.some((campaign): campaign is undefined => !campaign) ||
      (value.baseline !== undefined && !baseline) || (value.candidate !== undefined && !candidate) || (value.initialization !== undefined && !initialization) || (value.familyEvaluations !== undefined && !familyEvaluations) ||
      new Set(campaigns.map(campaign => campaign!.id)).size !== campaigns.length) return undefined;
  const modelValidation = value.modelValidation === undefined ? undefined : decodeIndependentModelValidation(value.modelValidation, selectedModel, campaigns.flatMap(campaign => campaign!.runs));
  if (value.modelValidation !== undefined && !modelValidation) return undefined;
  return clone({ ...(modelValidation ? { modelValidation } : {}), ...(recordingCatalog ? { recordingCatalog } : {}), ...(value.channelMapping ? { channelMapping: value.channelMapping as LoopChannels } : {}), ...(value.requiredScenarios ? { requiredScenarios: value.requiredScenarios as string[] } : {}), ...(tuningPreferences ? { tuningPreferences } : {}), ...(modelComparison ? { modelComparison } : {}), ...(value.acquisitionSettings ? { acquisitionSettings: acquisitionSettings(value.acquisitionSettings)! } : {}), ...(value.scenarioDefinitions ? { scenarioDefinitions: value.scenarioDefinitions as EngineerScenario[] } : {}), ...(tuningRuns ? { tuningRuns: tuningRuns as RuntimeVerificationResult[] } : {}), ...(draft ? { draft } : {}), ...(scenarioResults ? { scenarioResults: scenarioResults as EngineerScenarioResult[] } : {}), ...(settingsPins ? { settingsPins } : {}), ...(selectedModel ? { selectedModel } : {}), ...(task ? { task } : {}),
    ...(value.applicationMode ? { applicationMode: value.applicationMode as "auto" | "manual" } : {}),
    ...(value.controllerChoice ? { controllerChoice: value.controllerChoice as EngineerStudy["controllerChoice"] } : {}),
    ...(gainChoice ? { gainChoice } : {}),
    ...(attempts ? { verificationAttempts: attempts as RuntimeVerificationAttempt[] } : {}),
    ...(isControllerFamily(value.family) ? { family: value.family as EngineerStudy["family"] } : {}), campaigns: campaigns as SavedEngineerCampaign[],
    ...(modelPrediction ? { modelPrediction } : {}), ...(baseline ? { baseline } : {}), ...(candidate ? { candidate } : {}), ...(initialization ? { initialization } : {}), ...(familyEvaluations ? { familyEvaluations } : {}) });
}

function decodeModelPrediction(value: unknown): EngineerStudy["modelPrediction"] {
  if (!record(value) || !onlyKeys(value, ["baseline", "candidate"])) return undefined;
  const baseline = decodeRuntimeVerificationResult(value.baseline), candidate = decodeRuntimeVerificationResult(value.candidate);
  if (!baseline || !candidate || baseline.projectRevision !== candidate.projectRevision) return undefined;
  const a = baseline.run.conditions, b = candidate.run.conditions;
  if (a.modelRevision !== b.modelRevision || a.programRevision !== b.programRevision || a.parameterSchemaRevision !== b.parameterSchemaRevision
    || a.durationSeconds !== b.durationSeconds || a.seed !== b.seed || canonical(a.sampleTimesSeconds) !== canonical(b.sampleTimesSeconds)
    || canonical(baseline.run.channels) !== canonical(candidate.run.channels)
    || canonical(baseline.request.simulation_override ?? null) !== canonical(candidate.request.simulation_override ?? null)) return undefined;
  return { baseline, candidate };
}

function decodeFamilyEvaluations(value: unknown): SavedEngineerFamilyEvaluation[] | undefined {
  if (!Array.isArray(value) || (value.length === 0 || value.length > CONTROLLER_FAMILIES.length)) return undefined;
  const parsed = value.map(item => record(item) && onlyKeys(item, ["family", "status", "reason"]) &&
    isControllerFamily(item.family) &&
    ["model-qualified", "actual-pass", "actual-fail", "unavailable"].includes(String(item.status)) && text(item.reason)
    ? { family: item.family as SavedEngineerFamilyEvaluation["family"], status: item.status as SavedEngineerFamilyEvaluation["status"], reason: item.reason } : undefined);
  return parsed.some(item => !item) || new Set(parsed.map(item => item!.family)).size !== parsed.length ? undefined : parsed as SavedEngineerFamilyEvaluation[];
}

export function decodeVerificationAttempt(value: unknown): RuntimeVerificationAttempt | undefined {
  if (!record(value) || !onlyKeys(value, ["status", "jobId", "projectRevision", "request", "reason", "run"]) ||
    !["completed", "failed", "cancelled", "cancellation-unconfirmed"].includes(String(value.status)) ||
    !text(value.jobId) || !text(value.projectRevision) || !text(value.reason) || !jsonRecord(value.request) ||
    value.request.execution !== "isolated-simulation" || value.request.expected_project_revision !== value.projectRevision ||
    !positiveInteger(value.request.step_ms) || !positiveInteger(value.request.duration_ms) ||
    value.request.step_ms > 1000 || value.request.duration_ms > 600000 || value.request.duration_ms / value.request.step_ms + 1 > 12000 ||
    !Array.isArray(value.request.observations) || !value.request.observations.length) return undefined;
  try {
    // Decode raw evidence only; no empty-requirement evaluation becomes qualification.
    const run = value.run === undefined ? undefined : evaluateRecordedRun(value.run, []).run;
    if (run && (run.evidence !== "ST-executed" || !matchesFrozenRequest(value.request, run, value.projectRevision))) return undefined;
    return clone({ status: value.status as RuntimeVerificationAttempt["status"], jobId: value.jobId,
      projectRevision: value.projectRevision, request: value.request, reason: value.reason, ...(run ? { run } : {}) });
  } catch { return undefined; }
}

function decodeTask(value: unknown): EngineerRequirements | undefined {
  if (!record(value) || !onlyKeys(value, ["automaticCriteria", "channelRequirements", "referenceChange", "maximumError", "settlingBand", "settlingTimeSeconds", "durationSeconds", "transferTolerance", "tuningMethod", "tuningObjective", "constantInputDisturbance", "measurementNoiseBand", "gainUncertaintyFraction"])) return undefined;
  try { return engineerRequirements(value); } catch { return undefined; }
}

function decodeCampaign(value: unknown): SavedEngineerCampaign | undefined {
  if (!record(value) || !onlyKeys(value, ["id", "status", "reason", "runs"]) || !text(value.id) ||
      !["completed", "failed", "cancelled"].includes(String(value.status)) ||
      (value.reason !== undefined && !text(value.reason)) ||
      ((value.status === "failed" || value.status === "cancelled") && !text(value.reason)) ||
      !Array.isArray(value.runs) || value.runs.length > MAX_CAMPAIGN_RUNS) return undefined;
  const runs = value.runs.map(run => decodeConnectedIdentificationRecord(run) ?? decodeRuntimeVerificationResult(run));
  if (runs.some((run): run is undefined => !run) ||
      (value.status === "completed" && new Set(runs.map(run => run!.projectRevision)).size > 1)) return undefined;
  return clone({ id: value.id, status: value.status as SavedEngineerCampaign["status"],
    ...(value.reason === undefined ? {} : { reason: value.reason }), runs: runs as RuntimeVerificationResult[] });
}

function decodeRuntimeVerificationResult(value: unknown): RuntimeVerificationResult | undefined {
  if (!record(value) || !onlyKeys(value, ["projectRevision", "request", "run", "evaluation"]) || !text(value.projectRevision) ||
      !jsonRecord(value.request) || !record(value.evaluation) || !Array.isArray(value.evaluation.specification) ||
      value.evaluation.specification.length === 0 || value.evaluation.specification.length > 64) return undefined;
  try {
    const evaluation = evaluateRecordedRun(value.run, value.evaluation.specification);
    if (evaluation.run.evidence !== "ST-executed" || !matchesFrozenRequest(value.request, evaluation.run, value.projectRevision) ||
        canonical(value.evaluation) !== canonical(evaluation)) return undefined;
    return clone({ projectRevision: value.projectRevision, request: value.request, run: evaluation.run, evaluation });
  } catch { return undefined; }
}

/** Mirrors the existing verification client's persisted request-to-run identity, without adding a transport field. */
function matchesFrozenRequest(request: Record<string, unknown>, run: RuntimeVerificationResult["run"], projectRevision: string): boolean {
  const stepMs = request.step_ms;
  const durationMs = request.duration_ms;
  const seed = request.seed;
  if (request.execution !== "isolated-simulation" || request.expected_project_revision !== projectRevision ||
      !positiveInteger(stepMs) || stepMs > 1000 || !positiveInteger(durationMs) || durationMs > 600000 ||
      !nonnegativeInteger(seed) || durationMs % stepMs !== 0 || durationMs / stepMs + 1 > 12000 ||
      !record(request.initial_values) || !Array.isArray(request.events) || !Array.isArray(request.observations) ||
      run.conditions.durationSeconds !== durationMs / 1000 || run.conditions.seed !== seed ||
      canonical(run.conditions.sampleTimesSeconds) !== canonical(Array.from({ length: durationMs / stepMs + 1 },
        (_, index) => index * stepMs / 1000)) ||
      canonical(run.conditions.initialState) !== canonical(request.initial_values) ||
      canonical((run.conditions as typeof run.conditions & { feedbackRelay?: unknown }).feedbackRelay ?? null) !== canonical(request.feedback_relay ?? null) ||
      canonical(run.conditions.events) !== canonical(request.events.map(event => record(event)
        ? { atSeconds: number(event.at_ms) / 1000, channel: event.symbol, value: event.value } : undefined)) ||
      canonical(run.channels) !== canonical(request.observations.map(observation => record(observation)
        ? { id: observation.symbol, unit: observation.unit } : undefined)) || !matchesSimulationOverride(request.simulation_override, run)) return false;
  const overrides = request.parameter_overrides;
  if (overrides !== undefined && !Array.isArray(overrides)) return false;
  const parameters = Array.isArray(overrides) ? Object.fromEntries(overrides.map(override => record(override)
    ? [override.symbol, override.value] : ["", undefined])) : {};
  return canonical(run.parameters) === canonical(parameters);
}

function matchesSimulationOverride(value: unknown, run: RuntimeVerificationResult["run"]): boolean {
  if (value === undefined) return true;
  if (!record(value) || !text(value.model_revision) || !text(value.model_provenance) || typeof value.simulation_toml !== "string") return false;
  const conditions = run.conditions as RuntimeVerificationResult["run"]["conditions"] & {
    modelProvenance?: unknown; simulationConfigRevision?: unknown;
  };
  const digest = `sha256:${createHash("sha256").update(value.simulation_toml, "utf8").digest("hex")}`;
  return conditions.modelRevision === value.model_revision && conditions.modelProvenance === value.model_provenance &&
    conditions.simulationConfigRevision === digest;
}

function withinSerializedLimit(value: unknown): boolean {
  try { return Buffer.byteLength(JSON.stringify(value), "utf8") <= MAX_ENGINEER_STUDY_BYTES; } catch { return false; }
}

function onlyKeys(value: Record<string, unknown>, allowed: readonly string[]): boolean {
  return Object.keys(value).every(key => allowed.includes(key));
}

function jsonRecord(value: unknown): value is Record<string, unknown> {
  return record(value) && jsonEvidence(value);
}

function jsonEvidence(value: unknown): boolean {
  if (value === null || typeof value === "string" || typeof value === "boolean") return true;
  if (typeof value === "number") return Number.isFinite(value);
  if (Array.isArray(value)) return value.every(jsonEvidence);
  return record(value) && Object.values(value).every(jsonEvidence);
}

function canonical(value: unknown): string {
  return JSON.stringify(Array.isArray(value) ? value.map(item => JSON.parse(canonical(item))) : record(value)
    ? Object.fromEntries(Object.keys(value).sort().map(key => [key, JSON.parse(canonical(value[key]))])) : value);
}

function clone<T>(value: T): T { return JSON.parse(JSON.stringify(value)) as T; }
function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
function text(value: unknown): value is string { return typeof value === "string" && value.trim().length > 0; }
function positiveInteger(value: unknown): value is number { return typeof value === "number" && Number.isSafeInteger(value) && value > 0; }
function nonnegativeInteger(value: unknown): value is number { return typeof value === "number" && Number.isSafeInteger(value) && value >= 0; }
function number(value: unknown): number { return typeof value === "number" && Number.isFinite(value) ? value : NaN; }
