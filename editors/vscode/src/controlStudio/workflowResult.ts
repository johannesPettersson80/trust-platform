import type { ControlPidCandidate } from "./controllerCandidate";
import type { IdentifiedModel, ModelFitEvidence } from "./identification";
import type { ValidationResult } from "./validation";

export interface WorkflowResultSample {
  elapsedSeconds: number;
  observedAtMs: number;
  actualCycleSeconds: number;
  scan: number;
  source: "Simulation" | "Physical";
  loopRevision: string;
  sp: number | null;
  pv: number | null;
  mv: number | null;
  units: { sp: string; pv: string; mv: string };
  quality: "good" | "forced" | "bad";
  freshnessMs: number;
  controllerMode: "Auto" | "Manual" | "Track" | "Unavailable";
  outputLimits: { minimum: number; maximum: number };
  rateLimits: { upPerSecond?: number; downPerSecond?: number };
  safetyState: string;
  events: readonly string[];
}

export interface ControlStudioWorkflowResult {
  schemaVersion: 2;
  resultId: string;
  resultRevision: number;
  createdAt: string;
  projectId: string;
  source: "Simulation" | "Physical";
  loop: { id: string; revision: string };
  controller: { instance: string; revision: string };
  software: { extensionVersion: string; runtimeVersion?: string;
    numericalAdapters: Readonly<Record<string, string>> };
  units: { sp: string; pv: string; mv: string };
  recording: { rawSamples: readonly WorkflowResultSample[]; selectedIntervals: readonly { startSeconds: number; endSeconds: number }[] };
  model?: IdentifiedModel;
  modelFit?: ModelFitEvidence;
  currentController?: ControlPidCandidate;
  candidate?: ControlPidCandidate;
  validation?: ValidationResult;
  method?: string;
  tuningComparison?: WorkflowTuningComparison;
  experiment?: WorkflowExperimentContext;
  plant?: WorkflowPlantContext;
  evidence: { observation: "Observed"; model: "Not evaluated" | "Exact simulation model" |
    "Identified from simulation data" | "Identified from physical data" | "Model-predicted" };
  conclusion: string;
}

export interface WorkflowTuningComparison {
  objective: "fast" | "balanced" | "robust";
  winnerMethod: "SIMC" | "AMIGO" | "Constrained comparison";
  closedLoopTimeSeconds?: number;
  attempts: readonly {
    id: string;
    method: "SIMC" | "AMIGO" | "Constrained comparison";
    structure?: "P" | "PI" | "PD" | "PID";
    status: "Feasible" | "Rejected" | "Unavailable";
    score?: number;
    closedLoopTimeSeconds?: number;
    reason?: string;
  }[];
}

export interface WorkflowExperimentContext {
  kind: "observation" | "setpoint_step" | "output_step";
  requestedValue: number;
  appliedValue: number;
  minimum: number;
  maximum: number;
  approval: string;
  status: "completed" | "aborted" | "failed";
  stopReason: string;
  aborted: boolean;
  restoration: { value: number; status: "confirmed" | "failed" | "not_required" };
}

export interface WorkflowPlantContext {
  source: "Simulation" | "Physical";
  id: string;
  configurationRevision: string;
  configuration?: Readonly<Record<string, unknown>>;
  physicalTopologyReference?: string;
  model: string;
  effectiveSeed?: string;
  uncertainties: readonly string[];
  scheduledEvents: readonly string[];
  actuator: string;
  validityRange: string;
}

export function createWorkflowResult(
  input: Omit<ControlStudioWorkflowResult, "schemaVersion" | "createdAt">,
  createdAt: string
): ControlStudioWorkflowResult {
  const decoded = decodeWorkflowResult({ schemaVersion: 2, createdAt, ...input });
  if (!decoded) throw new Error("Control Studio workflow result is incomplete or invalid.");
  return decoded;
}

export function decodeWorkflowResult(value: unknown): ControlStudioWorkflowResult | undefined {
  if (!record(value) || containsSecretKey(value) || value.schemaVersion !== 2 ||
      !nonempty(value.resultId) || !positiveInteger(value.resultRevision) || !nonempty(value.projectId) ||
      typeof value.createdAt !== "string" || !Number.isFinite(Date.parse(value.createdAt)) ||
      (value.source !== "Simulation" && value.source !== "Physical") || !record(value.loop) ||
      !nonempty(value.loop.id) || !revision(value.loop.revision) || !record(value.controller) ||
      !nonempty(value.controller.instance) || !revision(value.controller.revision) ||
      !software(value.software) || !record(value.units) ||
      ![value.units.sp, value.units.pv, value.units.mv].every(nonempty) ||
      !record(value.recording) || !Array.isArray(value.recording.rawSamples) ||
      !value.recording.rawSamples.every(workflowSample) || !Array.isArray(value.recording.selectedIntervals) ||
      !value.recording.selectedIntervals.every(interval) ||
      (value.method !== undefined && typeof value.method !== "string") ||
      (value.model !== undefined && !record(value.model)) ||
      (value.modelFit !== undefined && !record(value.modelFit)) ||
      (value.currentController !== undefined && !candidate(value.currentController)) ||
      (value.candidate !== undefined && !candidate(value.candidate)) ||
      (value.validation !== undefined && !record(value.validation)) || !evidence(value.evidence) ||
      (value.tuningComparison !== undefined && !tuningComparison(value.tuningComparison)) ||
      (value.experiment !== undefined && !experimentContext(value.experiment)) ||
      (value.plant !== undefined && (!plantContext(value.plant) || value.plant.source !== value.source)) ||
      !nonempty(value.conclusion)) return undefined;
  if (!recordingMatchesResult(value)) return undefined;
  return cloneJson(value) as unknown as ControlStudioWorkflowResult;
}

function tuningComparison(value: unknown): boolean {
  if (!record(value) || !["fast", "balanced", "robust"].includes(String(value.objective)) ||
      !["SIMC", "AMIGO", "Constrained comparison"].includes(String(value.winnerMethod)) ||
      (value.closedLoopTimeSeconds !== undefined &&
        (!finite(value.closedLoopTimeSeconds) || value.closedLoopTimeSeconds <= 0)) ||
      !Array.isArray(value.attempts) || value.attempts.length === 0) return false;
  return value.attempts.every((attempt) => record(attempt) && nonempty(attempt.id) &&
    ["SIMC", "AMIGO", "Constrained comparison"].includes(String(attempt.method)) &&
    (attempt.structure === undefined || ["P", "PI", "PD", "PID"].includes(String(attempt.structure))) &&
    ["Feasible", "Rejected", "Unavailable"].includes(String(attempt.status)) &&
    (attempt.score === undefined || (finite(attempt.score) && attempt.score >= 0)) &&
    (attempt.closedLoopTimeSeconds === undefined ||
      (finite(attempt.closedLoopTimeSeconds) && attempt.closedLoopTimeSeconds > 0)) &&
    (attempt.reason === undefined || nonempty(attempt.reason)));
}

export function exportSamplesCsv(samples: readonly WorkflowResultSample[]): string {
  const header = ["elapsed_seconds", "observed_at_ms", "actual_cycle_seconds", "sp", "pv", "mv",
    "sp_unit", "pv_unit", "mv_unit", "quality", "freshness_ms", "controller_mode",
    "safety_state", "output_minimum", "output_maximum", "rate_down_per_second",
    "rate_up_per_second", "source", "events"];
  return [header.join(","), ...samples.map((sample) => [
    sample.elapsedSeconds, sample.observedAtMs, sample.actualCycleSeconds,
    sample.sp ?? "", sample.pv ?? "", sample.mv ?? "",
    sample.units.sp, sample.units.pv, sample.units.mv, sample.quality, sample.freshnessMs,
    sample.controllerMode, sample.safetyState, sample.outputLimits.minimum, sample.outputLimits.maximum,
    sample.rateLimits.downPerSecond ?? "", sample.rateLimits.upPerSecond ?? "", sample.source,
    sample.events.join("|"),
  ].map(csvCell).join(","))].join("\n");
}
export function exportWorkflowJson(result: unknown): string {
  return JSON.stringify(withoutSecrets(result), null, 2);
}

/**
 * Returns a detached, bounded history so a later analysis save cannot mutate
 * either its source recording or an earlier saved revision.
 */
export function appendWorkflowResultRevision<T>(
  history: readonly T[],
  result: T,
  capacity: number
): T[] {
  if (!Number.isInteger(capacity) || capacity <= 0) {
    throw new Error("Control Studio result history capacity must be a positive integer.");
  }
  return [...history, result].map(cloneJson).slice(-capacity);
}

export function decodeWorkflowResultHistory(value: unknown): ControlStudioWorkflowResult[] | undefined {
  if (!Array.isArray(value) || value.length > 20) return undefined;
  const history: ControlStudioWorkflowResult[] = [];
  for (const item of value) {
    const result = decodeWorkflowResult(item);
    if (!result) return undefined;
    const previous = history[history.length - 1];
    if (previous && (result.resultId !== previous.resultId || result.projectId !== previous.projectId ||
        result.loop.id !== previous.loop.id || result.resultRevision <= previous.resultRevision ||
        Date.parse(result.createdAt) < Date.parse(previous.createdAt))) return undefined;
    history.push(result);
  }
  return history;
}

export function decodeWorkflowResultHistoryState(value: unknown): {
  history: ControlStudioWorkflowResult[];
  unavailableReason?: string;
} {
  if (value === undefined) return { history: [] };
  const history = decodeWorkflowResultHistory(value);
  return history
    ? { history }
    : { history: [], unavailableReason: "Saved Control Studio results use an unsupported or invalid format." };
}

export function workflowExperimentContext(
  planValue: unknown,
  terminalValue: unknown
): WorkflowExperimentContext | undefined {
  if (!record(planValue) || !record(terminalValue) ||
      !["setpoint_step", "output_step"].includes(String(planValue.kind)) ||
      ![planValue.to, planValue.minimum, planValue.maximum, planValue.restorationValue].every(finite) ||
      !["completed", "aborted", "failed"].includes(String(terminalValue.status)) ||
      !nonempty(terminalValue.stop_reason) || !nonempty(terminalValue.restoration)) return undefined;
  const source = planValue.source === "physical" ? "physical" : planValue.source === "simulation" ? "simulation" : undefined;
  if (!source) return undefined;
  const restorationStatus = terminalValue.restoration === "restored"
    ? "confirmed"
    : terminalValue.restoration === "not_required" ? "not_required" : "failed";
  return {
    kind: planValue.kind as "setpoint_step" | "output_step",
    requestedValue: planValue.to as number,
    appliedValue: finite(terminalValue.applied_value) ? terminalValue.applied_value : planValue.to as number,
    minimum: planValue.minimum as number,
    maximum: planValue.maximum as number,
    approval: source === "physical" ? "Explicit physical approval after topology review" : "Simulation confirmation",
    status: terminalValue.status as "completed" | "aborted" | "failed",
    stopReason: terminalValue.stop_reason,
    aborted: terminalValue.status === "aborted",
    restoration: { value: planValue.restorationValue as number, status: restorationStatus },
  };
}

export function workflowSimulationPlantContext(
  savedValue: unknown,
  plantId: string,
  validityRange: string
): WorkflowPlantContext | undefined {
  if (!record(savedValue) || !revision(savedValue.configuration_revision) || !record(savedValue.saved_configuration) ||
      !nonempty(plantId) || !nonempty(validityRange)) return undefined;
  const configuration = savedValue.saved_configuration;
  const plants = Array.isArray(configuration.plants) ? configuration.plants.filter(record) : [];
  const plant = plants.find((item) => item.id === plantId);
  if (!plant || !nonempty(plant.model) || !jsonEvidence(plant)) return undefined;
  const simulation = record(configuration.simulation) ? configuration.simulation : undefined;
  const seed = simulation?.seed;
  const events = Array.isArray(configuration.plant_events)
    ? configuration.plant_events.filter((item) => record(item) && item.plant === plantId && jsonEvidence(item))
    : [];
  const uncertainties = Array.isArray(plant.uncertainties)
    ? plant.uncertainties.filter(jsonEvidence).map(stableJson)
    : [];
  return {
    source: "Simulation",
    id: plantId,
    configurationRevision: savedValue.configuration_revision,
    configuration: cloneJson(plant),
    model: plant.model,
    ...(typeof seed === "string" || typeof seed === "number" ? { effectiveSeed: String(seed) } : {}),
    uncertainties,
    scheduledEvents: events.map(stableJson),
    actuator: record(plant.actuator) ? stableJson(plant.actuator) : "Not declared",
    validityRange,
  };
}

export async function saveWorkflowResultRevision(
  storage: { get(key: string): unknown; update(key: string, value: unknown): PromiseLike<void> },
  key: string,
  result: ControlStudioWorkflowResult,
  capacity: number
): Promise<ControlStudioWorkflowResult[]> {
  const stored = storage.get(key);
  const history = stored === undefined ? [] : decodeWorkflowResultHistory(stored);
  if (!history) throw new Error("Existing Control Studio result history is invalid and was not replaced.");
  const decoded = decodeWorkflowResult(result);
  if (!decoded) throw new Error("The Control Studio result is invalid and was not saved.");
  const next = appendWorkflowResultRevision(history, decoded, capacity);
  if (!decodeWorkflowResultHistory(next)) {
    throw new Error("The Control Studio result revision does not follow the saved history.");
  }
  await storage.update(key, next);
  return next;
}

export interface WorkflowResultHistoryItem {
  resultRevision: number;
  createdAt: string;
  source: "Simulation" | "Physical";
  controllerRevision: string;
  method: string;
  sampleCount: number;
}

export function workflowResultHistoryItems(
  history: readonly ControlStudioWorkflowResult[]
): WorkflowResultHistoryItem[] {
  return history.map((result) => ({
    resultRevision: result.resultRevision,
    createdAt: result.createdAt,
    source: result.source,
    controllerRevision: result.controller.revision,
    method: result.method ?? "Not selected",
    sampleCount: result.recording.rawSamples.length,
  }));
}

export function compareWorkflowResultRevisions(
  left: ControlStudioWorkflowResult,
  right: ControlStudioWorkflowResult
): {
  left: WorkflowResultComparisonSide;
  right: WorkflowResultComparisonSide;
  sameEvidenceSource: boolean;
} {
  return { left: comparisonSide(left), right: comparisonSide(right),
    sameEvidenceSource: left.source === right.source };
}

export async function workflowResultForExport<T>(
  opened: T | undefined,
  createDraft: () => T | Promise<T>
): Promise<T> {
  return opened ?? await createDraft();
}

interface WorkflowResultComparisonSide {
  resultRevision: number;
  source: "Simulation" | "Physical";
  controllerRevision: string;
  method: string;
  createdAt: string;
  conclusion: string;
}

function comparisonSide(result: ControlStudioWorkflowResult): WorkflowResultComparisonSide {
  return { resultRevision: result.resultRevision, source: result.source,
    controllerRevision: result.controller.revision, method: result.method ?? "Not selected",
    createdAt: result.createdAt, conclusion: result.conclusion };
}

/** Rebuilds user-excluded gaps from the selected intervals saved as evidence. */
export function excludedIntervalsFromSelection(
  endSeconds: number,
  selected: readonly { startSeconds: number; endSeconds: number }[]
): Array<{ startSeconds: number; endSeconds: number }> {
  if (!Number.isFinite(endSeconds) || endSeconds <= 0) return [];
  const ordered = selected
    .filter((item) => Number.isFinite(item.startSeconds) && Number.isFinite(item.endSeconds) &&
      item.endSeconds >= 0 && item.startSeconds <= endSeconds && item.endSeconds >= item.startSeconds)
    .map((item) => ({ startSeconds: Math.max(0, item.startSeconds), endSeconds: Math.min(endSeconds, item.endSeconds) }))
    .sort((left, right) => left.startSeconds - right.startSeconds);
  const excluded: Array<{ startSeconds: number; endSeconds: number }> = [];
  let selectedThrough = 0;
  for (const item of ordered) {
    if (item.startSeconds > selectedThrough) {
      excluded.push({ startSeconds: selectedThrough, endSeconds: item.startSeconds });
    }
    selectedThrough = Math.max(selectedThrough, item.endSeconds);
  }
  if (selectedThrough < endSeconds) excluded.push({ startSeconds: selectedThrough, endSeconds });
  return excluded;
}

function withoutSecrets(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(withoutSecrets);
  if (value === null || typeof value !== "object") return value;
  return Object.fromEntries(Object.entries(value as Record<string, unknown>)
    .filter(([key]) => !/(token|password|credential|secret|authorization)/i.test(key))
    .map(([key, item]) => [key, withoutSecrets(item)]));
}

function candidate(value: unknown): boolean {
  if (!record(value) || value.applied !== false || !["P", "PI", "PD", "PID"].includes(String(value.structure)) ||
      !["Manual", "Imported", "SIMC", "AMIGO", "Constrained comparison"].includes(String(value.source)) ||
      !record(value.parameters)) return false;
  const parameters = value.parameters;
  const numeric = ["proportionalGain", "integralGain", "derivativeGain", "proportionalSetpointWeight",
    "derivativeSetpointWeight", "bias", "outputMinimum", "outputMaximum", "rateMinimum", "rateMaximum", "trackingTime"];
  return numeric.every((key) => typeof parameters[key] === "number" && Number.isFinite(parameters[key])) &&
    (parameters.antiWindup === "BackCalculation" || parameters.antiWindup === "Combined");
}

function workflowSample(value: unknown): boolean {
  return record(value) && finite(value.elapsedSeconds) && finite(value.observedAtMs) && value.observedAtMs >= 0 &&
    finite(value.actualCycleSeconds) && value.actualCycleSeconds > 0 &&
    Number.isInteger(value.scan) && (value.scan as number) >= 0 &&
    (value.source === "Simulation" || value.source === "Physical") && revision(value.loopRevision) &&
    nullableFinite(value.sp) && nullableFinite(value.pv) && nullableFinite(value.mv) && record(value.units) &&
    [value.units.sp, value.units.pv, value.units.mv].every(nonempty) &&
    ["good", "forced", "bad"].includes(String(value.quality)) && finite(value.freshnessMs) && value.freshnessMs >= 0 &&
    ["Auto", "Manual", "Track", "Unavailable"].includes(String(value.controllerMode)) &&
    limits(value.outputLimits) && rateLimits(value.rateLimits) && nonempty(value.safetyState) &&
    Array.isArray(value.events) && value.events.length <= 128 && value.events.every(nonempty);
}

function software(value: unknown): boolean {
  if (!record(value) || !nonempty(value.extensionVersion) ||
      (value.runtimeVersion !== undefined && !nonempty(value.runtimeVersion)) ||
      !record(value.numericalAdapters)) return false;
  return Object.entries(value.numericalAdapters).every(([name, version]) => nonempty(name) && nonempty(version));
}

function evidence(value: unknown): boolean {
  return record(value) && value.observation === "Observed" && ["Not evaluated", "Exact simulation model",
    "Identified from simulation data", "Identified from physical data", "Model-predicted"].includes(String(value.model));
}

function experimentContext(value: unknown): value is WorkflowExperimentContext {
  return record(value) && ["observation", "setpoint_step", "output_step"].includes(String(value.kind)) &&
    finite(value.requestedValue) && finite(value.appliedValue) && finite(value.minimum) &&
    finite(value.maximum) && value.minimum <= value.maximum && nonempty(value.approval) &&
    ["completed", "aborted", "failed"].includes(String(value.status)) && nonempty(value.stopReason) &&
    typeof value.aborted === "boolean" && record(value.restoration) && finite(value.restoration.value) &&
    ["confirmed", "failed", "not_required"].includes(String(value.restoration.status));
}

function plantContext(value: unknown): value is WorkflowPlantContext {
  return record(value) && (value.source === "Simulation" || value.source === "Physical") &&
    nonempty(value.id) && nonempty(value.configurationRevision) && nonempty(value.model) &&
    (value.source === "Simulation"
      ? record(value.configuration) && jsonEvidence(value.configuration) && value.physicalTopologyReference === undefined
      : value.configuration === undefined && nonempty(value.physicalTopologyReference)) &&
    (value.effectiveSeed === undefined || nonempty(value.effectiveSeed)) && stringList(value.uncertainties) &&
    stringList(value.scheduledEvents) && nonempty(value.actuator) && nonempty(value.validityRange);
}

function jsonEvidence(value: unknown): boolean {
  if (value === null || typeof value === "string" || typeof value === "boolean") return true;
  if (typeof value === "number") return Number.isFinite(value);
  if (Array.isArray(value)) return value.every(jsonEvidence);
  return record(value) && Object.values(value).every(jsonEvidence);
}

function stableJson(value: unknown): string {
  return JSON.stringify(value);
}

function stringList(value: unknown): boolean {
  return Array.isArray(value) && value.length <= 128 && value.every(nonempty);
}

function limits(value: unknown): boolean {
  return record(value) && finite(value.minimum) && finite(value.maximum) && value.minimum <= value.maximum;
}

function rateLimits(value: unknown): boolean {
  return record(value) && (value.upPerSecond === undefined || finite(value.upPerSecond)) &&
    (value.downPerSecond === undefined || finite(value.downPerSecond));
}

function recordingMatchesResult(value: Record<string, unknown>): boolean {
  const loop = value.loop as Record<string, unknown>;
  const units = value.units as Record<string, unknown>;
  const recording = value.recording as Record<string, unknown>;
  const samples = recording.rawSamples as Record<string, unknown>[];
  let previousElapsed = -1;
  let previousObservedAt = -1;
  let previousScan = -1;
  for (const sample of samples) {
    const sampleUnits = sample.units as Record<string, unknown>;
    if (sample.source !== value.source || sample.loopRevision !== loop.revision ||
        sampleUnits.sp !== units.sp || sampleUnits.pv !== units.pv || sampleUnits.mv !== units.mv ||
        (sample.elapsedSeconds as number) < 0 || (sample.elapsedSeconds as number) <= previousElapsed ||
        (sample.observedAtMs as number) <= previousObservedAt || (sample.scan as number) <= previousScan) {
      return false;
    }
    previousElapsed = sample.elapsedSeconds as number;
    previousObservedAt = sample.observedAtMs as number;
    previousScan = sample.scan as number;
  }
  const end = samples.length > 0
    ? samples[samples.length - 1].elapsedSeconds as number
    : undefined;
  return (recording.selectedIntervals as Record<string, unknown>[]).every((selected) =>
    end !== undefined && (selected.endSeconds as number) <= end);
}

function interval(value: unknown): boolean {
  return record(value) && finite(value.startSeconds) && finite(value.endSeconds) &&
    value.startSeconds >= 0 && value.endSeconds >= value.startSeconds;
}

function containsSecretKey(value: unknown): boolean {
  if (Array.isArray(value)) return value.some(containsSecretKey);
  if (!record(value)) return false;
  return Object.entries(value).some(([key, item]) =>
    /(token|password|credential|secret|authorization)/i.test(key) || containsSecretKey(item));
}

function cloneJson<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

function csvCell(value: string | number): string {
  const text = String(value);
  return /[",\r\n]/.test(text) ? `"${text.replace(/"/g, '""')}"` : text;
}

function record(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
function nonempty(value: unknown): value is string { return typeof value === "string" && value.trim().length > 0; }
function revision(value: unknown): value is string { return nonempty(value) && value.startsWith("sha256:"); }
function finite(value: unknown): value is number { return typeof value === "number" && Number.isFinite(value); }
function positiveInteger(value: unknown): value is number { return Number.isInteger(value) && (value as number) > 0; }
function nullableFinite(value: unknown): boolean { return value === null || finite(value); }
