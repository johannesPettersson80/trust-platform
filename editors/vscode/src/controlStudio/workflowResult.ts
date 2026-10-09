import { validCommissioningObservation } from "./commissioningObservation";
import { decodeIdentificationState, type SavedIdentificationState } from "./identificationState";
import { decodeGainRecoveryContext } from "./gainRecoveryContext";
import { isSavedFiniteSearch, type SavedFiniteSearch } from "./finiteSearchPersistence";
import { isSavedRunValidation, type SavedRunValidation } from "./runValidationPersistence";
import { isSavedGeneralModelAnalysis, isSavedPeriodicFrequencyAnalysis, type SavedGeneralModelAnalysis, type SavedPeriodicFrequencyAnalysis } from "./generalModelPersistence";
import { isSavedLeadLagAnalysis, type SavedLeadLagAnalysis } from "./leadLagSession";
import { recordedChannelIdentity, validRecordedChannels, type RecordedChannel } from "./recordingChannels";
import { isSavedDeclaredAnalysis, type SavedDeclaredAnalysis } from "./declaredAnalysisPersistence";
import { decodeEngineerStudy, type EngineerStudy } from "./engineerStudyPersistence";
import type { ControlPidCandidate } from "./controllerCandidate";
import type { IdentifiedModel, ModelFitEvidence } from "./identification";
import { decodeControlChartSession, type ControlChartSession } from "./chartContract";
import type { CommissioningResult } from "./proportionalCommissioning";
import { assessCommissioningRecording } from "./commissioningAssessment";
import { isSavedRouthAnalysis, type SavedRouthAnalysis } from "./routhPersistence";
import { MAXIMUM_EXPERIMENT_DURATION_MS, MINIMUM_EXPERIMENT_DURATION_MS, RECORDING_SAMPLE_LIMIT } from "./controlStudioLimits";
import { steadyRecordPeriod } from "./recordSequence";
import { shareCopy } from "./recordedEvidence";

export interface WorkflowResultSample {
  additional?: readonly RecordedChannel[];
  elapsedSeconds: number;
  observedAtMs: number;
  actualCycleSeconds: number | null;
  editorObservationIntervalSeconds?: number | null;
  scan: number;
  source: "Simulation" | "Physical";
  loopRevision: string;
  sp: number | null;
  pv: number | null;
  requestedMv: number | null;
  appliedMv: number | null;
  units: { sp: string; pv: string; mv: string };
  quality: "good" | "forced" | "bad";
  freshnessMs: number | null;
  controllerMode: "Auto" | "Manual" | "Track" | "Unavailable";
  outputLimits: { minimum: number; maximum: number };
  rateLimits: { upPerSecond?: number; downPerSecond?: number };
  safetyState: string;
  events: readonly string[];
}

export interface WorkflowRecordingAcquisition {
  readonly recordingId?: string;
  programRevision?: string;
  method: "ui_observed_snapshots" | "runtime_scan_buffer";
  completeness: "incomplete" | "complete";
  observedSampleCount: number;
  scanRange?: { first: number; last: number };
  missingScanRanges: readonly { first: number; last: number }[];
  droppedObservedSamples: number;
  actualCycleTimingAvailable: boolean;
  freshnessAvailable: boolean;
  reason: string;
}

/** Original acquisition identity; importing a record never imports design state. */
export interface WorkflowRecordingProvenance extends Pick<ControlStudioWorkflowResult,
  "resultId" | "resultRevision" | "createdAt" | "projectId" | "source" | "loop" | "controller" | "software" | "units" | "plant" | "experiment"> {
  schemaVersion: 1;
}

export interface ControlStudioWorkflowResult {
  schemaVersion: 3 | 4;
  resultId: string;
  resultRevision: number;
  createdAt: string;
  projectId: string;
  source: "Simulation" | "Physical";
  loop: { id: string; revision: string; topology?: "open-loop" | "closed-loop" };
  controller: { instance: string; revision: string };
  software: { extensionVersion: string; runtimeVersion?: string;
    numericalAdapters: Readonly<Record<string, string>> };
  units: { sp: string; pv: string; mv: string };
  recording: { rawSamples: readonly WorkflowResultSample[]; selectedIntervals: readonly { startSeconds: number; endSeconds: number }[];
    acquisition?: WorkflowRecordingAcquisition; provenance?: WorkflowRecordingProvenance };
  model?: IdentifiedModel;
  declaredAnalysis?: SavedDeclaredAnalysis;
  routhAnalysis?: SavedRouthAnalysis;
  generalModelAnalysis?: SavedGeneralModelAnalysis;
  periodicFrequencyAnalysis?: SavedPeriodicFrequencyAnalysis;
  leadLagAnalysis?: SavedLeadLagAnalysis;
  runValidation?: SavedRunValidation;
  finiteSearch?: SavedFiniteSearch;
  modelFit?: ModelFitEvidence;
  identificationStudy?: SavedIdentificationState;
  currentController?: ControlPidCandidate;
  candidate?: ControlPidCandidate;
  method?: string;
  tuningComparison?: WorkflowTuningComparison;
  experiment?: WorkflowExperimentContext;
  plant?: WorkflowPlantContext;
  chartSession?: ControlChartSession;
  engineerStudy?: EngineerStudy;
  commissioning?: CommissioningResult;
  evidence: { observation: "Observed" | "Not observed"; model: "Not evaluated" | "Exact simulation model" |
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
  kind: "observation" | "setpoint_step" | "load_step" | "mv_step" | "mv_pulse" |
    "relay" | "prbs" | "multisine" | "output_step";
  affectedSignal?: string;
  address?: string;
  requestedValue: number;
  appliedValue: number;
  durationMs?: number;
  intervalMs?: number;
  seed?: number;
  frequenciesHz?: readonly number[];
  trajectory?: readonly { atMs: number; value: number }[];
  stopConditions?: readonly string[];
  minimum: number;
  maximum: number;
  approval: string;
  status: "completed" | "aborted" | "failed" | "failed_to_restore";
  stopReason: string;
  aborted: boolean;
  restoration: { value: number; status: "confirmed" | "failed" | "not_required"; reason?: string };
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
  const decoded = decodeWorkflowResult({ schemaVersion: 4, createdAt, ...input });
  if (!decoded) throw new Error("Control Studio workflow result is incomplete or invalid.");
  return decoded;
}

export function decodeWorkflowResult(value: unknown): ControlStudioWorkflowResult | undefined {
  if (!record(value) || containsSecretKey(value) || (value.schemaVersion !== 3 && value.schemaVersion !== 4) ||
      !nonempty(value.resultId) || !positiveInteger(value.resultRevision) || !nonempty(value.projectId) ||
      typeof value.createdAt !== "string" || !Number.isFinite(Date.parse(value.createdAt)) ||
      (value.source !== "Simulation" && value.source !== "Physical") || !record(value.loop) ||
      !nonempty(value.loop.id) || !revision(value.loop.revision) || !record(value.controller) ||
      !nonempty(value.controller.instance) || !revision(value.controller.revision) ||
      !software(value.software) || !record(value.units) ||
      ![value.units.pv, value.units.mv].every(nonempty) ||
      !(value.schemaVersion === 4 && value.loop.topology === "open-loop" ? value.units.sp === "" : nonempty(value.units.sp)) ||
      (value.loop.topology !== undefined && value.loop.topology !== "open-loop" && value.loop.topology !== "closed-loop") ||
      !record(value.recording) || !Array.isArray(value.recording.rawSamples) ||
      !value.recording.rawSamples.every(workflowSample) || !Array.isArray(value.recording.selectedIntervals) ||
      !value.recording.selectedIntervals.every(interval) ||
      (value.method !== undefined && typeof value.method !== "string") ||
      (value.model !== undefined && !record(value.model)) ||
      (value.finiteSearch !== undefined && (value.schemaVersion !== 4 || !isSavedFiniteSearch(value.finiteSearch))) ||
      (value.runValidation !== undefined && (value.schemaVersion !== 4 || !isSavedRunValidation(value.runValidation))) ||
      (value.generalModelAnalysis !== undefined && (value.schemaVersion !== 4 || !isSavedGeneralModelAnalysis(value.generalModelAnalysis))) ||
      (value.periodicFrequencyAnalysis !== undefined && (value.schemaVersion !== 4 || !isSavedPeriodicFrequencyAnalysis(value.periodicFrequencyAnalysis))) ||
      (value.leadLagAnalysis !== undefined && (value.schemaVersion !== 4 || !isSavedLeadLagAnalysis(value.leadLagAnalysis))) ||
      (value.declaredAnalysis !== undefined && (value.schemaVersion !== 4 || !isSavedDeclaredAnalysis(value.declaredAnalysis))) ||
      (value.routhAnalysis !== undefined && (value.schemaVersion !== 4 || !isSavedRouthAnalysis(value.routhAnalysis))) ||
      (value.modelFit !== undefined && !record(value.modelFit)) ||
      (value.identificationStudy !== undefined && (value.schemaVersion !== 4 || !decodeIdentificationState(value.identificationStudy))) ||
      (value.currentController !== undefined && !candidate(value.currentController)) ||
      (value.candidate !== undefined && !candidate(value.candidate)) ||
      !evidence(value.evidence) ||
      (value.tuningComparison !== undefined && !tuningComparison(value.tuningComparison)) ||
      (value.experiment !== undefined && !experimentContext(value.experiment)) ||
      (value.plant !== undefined && (!plantContext(value.plant) || value.plant.source !== value.source)) ||
      (value.chartSession !== undefined && !decodeControlChartSession(
        value.chartSession, `${value.loop.id}:${value.loop.revision}`
      )) ||
      (value.engineerStudy !== undefined && (value.schemaVersion !== 4 || !decodeEngineerStudy(value.engineerStudy))) ||
      !nonempty(value.conclusion)) return undefined;
  if (!recordingMatchesResult(value)) return undefined;
  if (!identificationRecordingsMatchResult(value)) return undefined;
  if (value.commissioning !== undefined && (value.schemaVersion !== 4 || !validCommissioning(value.commissioning, value))) return undefined;
  return shareCopy(value) as unknown as ControlStudioWorkflowResult;
}

function identificationRecordingsMatchResult(outer: Record<string, unknown>): boolean {
  if (!record(outer.identificationStudy)) return true;
  for (const input of [outer.identificationStudy.current, outer.identificationStudy.estimation]) {
    if (input === undefined) continue;
    if (!record(input) || !record(input.recording) || !record(input.recording.data)) return false;
    const data = input.recording.data;
    if (!Array.isArray(data.rawSamples) || !data.rawSamples.every(workflowSample) ||
      !Array.isArray(data.selectedIntervals) || !data.selectedIntervals.every(interval)) return false;
    const origin = record(data.provenance) ? data.provenance : undefined;
    if (!recordingMatchesResult({ ...outer, source: input.source, units: origin?.units ?? outer.units, recording: data })) return false;
  }
  return true;
}

function validCommissioning(value: unknown, outer: Record<string, unknown>): value is CommissioningResult {
  if (!record(value) || !["passed", "failed", "cancelled"].includes(String(value.status)) || !nonempty(value.reason)
    || !nonempty(value.modelRevision) || !finite(value.target) || typeof value.recoveryConfirmed !== "boolean"
    || !Array.isArray(value.channels) || value.channels.length < 3 || value.channels.length > 32
    || !value.channels.every(channel => record(channel) && nonempty(channel.id) && typeof channel.unit === "string")
    || new Set(value.channels.map(channel => (channel as Record<string, unknown>).id)).size !== value.channels.length
    || !Array.isArray(value.transfers) || value.transfers.length > 16
    || JSON.stringify(value).length > 16 * 1024 * 1024 || !record(value.criteria)
    || (value.foundMode !== undefined && value.foundMode !== "auto" && value.foundMode !== "manual")) return false;
  const gainRecovery = value.gainRecovery === undefined ? undefined : decodeGainRecoveryContext(value.gainRecovery);
  if (value.gainRecovery !== undefined && !gainRecovery) return false;
  const criteria = value.criteria;
  if (criteria.target !== value.target || !finite(criteria.durationSeconds) || criteria.durationSeconds < MINIMUM_EXPERIMENT_DURATION_MS / 1_000 || criteria.durationSeconds > MAXIMUM_EXPERIMENT_DURATION_MS / 1_000
    || !finite(criteria.settlingTimeSeconds) || criteria.settlingTimeSeconds < 0 || criteria.settlingTimeSeconds >= criteria.durationSeconds
    || !finite(criteria.maximumError) || criteria.maximumError <= 0 || !finite(criteria.taskPeriodMs) || criteria.taskPeriodMs <= 0
    || !limits(criteria.processRange) || !limits(criteria.outputLimits)) return false;
  if (criteria.observed !== undefined && !validCommissioningObservation(criteria.observed, criteria.durationSeconds, criteria.taskPeriodMs)) return false;
  if (criteria.switching !== undefined && (!record(criteria.switching) || !finite(criteria.switching.low)
    || !finite(criteria.switching.high) || criteria.switching.low >= criteria.switching.high
    || !finite(criteria.switching.minimumOnSeconds) || criteria.switching.minimumOnSeconds < 0
    || !finite(criteria.switching.minimumOffSeconds) || criteria.switching.minimumOffSeconds < 0)) return false;
  const values = (item: unknown) => record(item) && Object.keys(item).length <= 34 && Object.values(item).every(finiteOrBoolean);
  if (!value.transfers.every(item => record(item) && revision(item.program_revision) && revision(item.binding_revision)
    && values(item.values) && ["applied", "rejected", "failed", "cancelled"].includes(String(item.status))
    && record(item.transaction) && nonempty(item.transaction.id) && revision(item.transaction.program_revision)
    && revision(item.transaction.binding_revision) && item.transaction.status === item.status
    && (item.transaction.observed_values === null || values(item.transaction.observed_values))
    && (item.transaction.initial_values === undefined || item.transaction.initial_values === null || values(item.transaction.initial_values))
    && (item.transaction.applied_values === undefined || item.transaction.applied_values === null || values(item.transaction.applied_values))
    && [null, "post-st", "output-image"].includes(item.transaction.confirmation as null))) return false;
  if (value.recording !== undefined) {
    if (!record(value.recording) || !Array.isArray(value.recording.rawSamples) || value.recording.rawSamples.length > RECORDING_SAMPLE_LIMIT
      || !value.recording.rawSamples.every(workflowSample) || !record(value.recording.evidence)
      || value.recording.evidence.method !== "runtime_scan_buffer" || !revision(value.recording.evidence.programRevision)
      || !recordingMatchesResult({ ...outer, recording: { rawSamples: value.recording.rawSamples,
        selectedIntervals: [], acquisition: value.recording.evidence } })) return false;
  }
  if (gainRecovery && record(value.recording) && record(value.recording.evidence)
    && gainRecovery.origin.program_revision !== value.recording.evidence.programRevision) return false;
  if (value.maximumError !== undefined && !finite(value.maximumError)) return false;
  if (value.status === "passed") {
    if (!record(value.recording) || !record(value.recording.evidence) || !Array.isArray(value.recording.rawSamples)
      || !value.recoveryConfirmed || value.transfers.length < 2 || !record(value.parameters)
      || value.parameters.program_revision !== value.recording.evidence.programRevision
      || !value.transfers.every(item => (item as any).status === "applied"
        && (item as any).transaction.confirmation === "output-image"
        && (item as any).transaction.program_revision === (value.recording as any).evidence.programRevision)) return false;
    const raw = value.recording.rawSamples as WorkflowResultSample[];
    if (!raw.length || !steadyRecordPeriod(raw.map(sample => sample.scan), true)) return false;
    try {
      if (assessCommissioningRecording(value.recording as any, criteria as any) !== value.maximumError) return false;
    } catch { return false; }
  }
  return true;
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
  const extraIds = [...new Set(samples.flatMap(sample => sample.additional?.map(channel => channel.id) ?? []))].sort();
  const extraFields = ["value", "unit", "quality", "channel", "role"] as const;
  const header = ["elapsed_seconds", "observed_at_ms", "actual_cycle_seconds", "sp", "pv",
    "requested_mv", "applied_mv",
    "sp_unit", "pv_unit", "mv_unit", "quality", "freshness_ms", "editor_observation_interval_seconds", "controller_mode",
    "safety_state", "output_minimum", "output_maximum", "rate_down_per_second",
    "rate_up_per_second", "source", "events",
    ...extraIds.flatMap(id => extraFields.map(field => `additional.${id}.${field}`))];
  return [header.map(csvCell).join(","), ...samples.map((sample) => [
    sample.elapsedSeconds, sample.observedAtMs, sample.actualCycleSeconds ?? "",
    sample.sp ?? "", sample.pv ?? "", sample.requestedMv ?? "", sample.appliedMv ?? "",
    sample.units.sp, sample.units.pv, sample.units.mv, sample.quality, sample.freshnessMs ?? "",
    sample.editorObservationIntervalSeconds ?? "",
    sample.controllerMode, sample.safetyState, sample.outputLimits.minimum, sample.outputLimits.maximum,
    sample.rateLimits.downPerSecond ?? "", sample.rateLimits.upPerSecond ?? "", sample.source,
    sample.events.join("|"),
    ...extraIds.flatMap(id => {
      const channel = sample.additional?.find(channel => channel.id === id);
      return extraFields.map(field => channel?.[field] ?? "");
    }),
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
      !["observation", "setpoint_step", "load_step", "mv_step", "mv_pulse", "relay", "prbs", "multisine", "output_step", "project_sequence"]
        .includes(String(planValue.kind)) ||
      ![planValue.minimum, planValue.maximum].every(finite) ||
      ![planValue.to, planValue.restorationValue].every(finiteOrBoolean) ||
      !["completed", "aborted", "failed", "failed_to_restore"].includes(String(terminalValue.status)) ||
      !nonempty(terminalValue.stop_reason) || !nonempty(terminalValue.restoration)) return undefined;
  const source = planValue.source === "physical" ? "physical" : planValue.source === "simulation" ? "simulation" : undefined;
  if (!source) return undefined;
  const restorationStatus = terminalValue.restoration === "restored"
    ? "confirmed"
    : terminalValue.restoration === "not_required" ? "not_required" : "failed";
  // A variable written by name has no address (Specification 35 §17.1).
  const richPlan = nonempty(planValue.affectedSignal) && (planValue.address === undefined || nonempty(planValue.address)) &&
    Number.isSafeInteger(planValue.durationMs) && (planValue.durationMs as number) > 0 &&
    trajectory(planValue.trajectory) && stringList(planValue.stopConditions);
  return {
    kind: (planValue.kind === "project_sequence" ? "observation" : planValue.kind) as WorkflowExperimentContext["kind"],
    ...(richPlan ? {
      affectedSignal: planValue.affectedSignal as string,
      ...(nonempty(planValue.address) ? { address: planValue.address } : {}),
      durationMs: planValue.durationMs as number,
      ...(finite(planValue.intervalMs) ? { intervalMs: planValue.intervalMs } : {}),
      ...(Number.isSafeInteger(planValue.seed) ? { seed: planValue.seed as number } : {}),
      ...(positiveFiniteList(planValue.frequenciesHz) ? { frequenciesHz: planValue.frequenciesHz } : {}),
      trajectory: cloneJson(planValue.trajectory) as Array<{ atMs: number; value: number }>,
      stopConditions: [...planValue.stopConditions as string[]],
    } : {}),
    requestedValue: numericExperimentValue(planValue.to),
    appliedValue: finiteOrBoolean(terminalValue.applied_value) ? numericExperimentValue(terminalValue.applied_value) : numericExperimentValue(planValue.to),
    minimum: planValue.minimum as number,
    maximum: planValue.maximum as number,
    approval: source === "physical" ? "Explicit physical approval after topology review" : "Simulation confirmation",
    status: terminalValue.status as WorkflowExperimentContext["status"],
    stopReason: terminalValue.stop_reason,
    aborted: terminalValue.status === "aborted",
    restoration: {
      value: numericExperimentValue(planValue.restorationValue),
      status: restorationStatus,
      ...(nonempty(terminalValue.restoration_reason) ? { reason: terminalValue.restoration_reason } : {}),
    },
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
  createDraft: () => T | Promise<T>,
  viewingHistory = true
): Promise<T> {
  return viewingHistory && opened !== undefined ? opened : await createDraft();
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
  return record(value) && (value.additional === undefined || validRecordedChannels(value.additional)) && finite(value.elapsedSeconds) && finite(value.observedAtMs) && value.observedAtMs >= 0 &&
    (value.actualCycleSeconds === null || (finite(value.actualCycleSeconds) && value.actualCycleSeconds > 0)) &&
    (value.editorObservationIntervalSeconds === undefined || value.editorObservationIntervalSeconds === null ||
      (finite(value.editorObservationIntervalSeconds) && value.editorObservationIntervalSeconds > 0)) &&
    Number.isInteger(value.scan) && (value.scan as number) >= 0 &&
    (value.source === "Simulation" || value.source === "Physical") && revision(value.loopRevision) &&
    nullableFinite(value.sp) && nullableFinite(value.pv) && nullableFinite(value.requestedMv) &&
    nullableFinite(value.appliedMv) && record(value.units) &&
    typeof value.units.sp === "string" && [value.units.pv, value.units.mv].every(nonempty) &&
    ["good", "forced", "bad"].includes(String(value.quality)) &&
    (value.freshnessMs === null || (finite(value.freshnessMs) && value.freshnessMs >= 0)) &&
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
  return record(value) && (value.observation === "Observed" || value.observation === "Not observed") && ["Not evaluated", "Exact simulation model",
    "Identified from simulation data", "Identified from physical data", "Model-predicted"].includes(String(value.model));
}

function experimentContext(value: unknown): value is WorkflowExperimentContext {
  return record(value) &&
    ["observation", "setpoint_step", "load_step", "mv_step", "mv_pulse", "relay", "prbs", "multisine", "output_step"]
      .includes(String(value.kind)) &&
    finite(value.requestedValue) && finite(value.appliedValue) && finite(value.minimum) &&
    finite(value.maximum) && value.minimum <= value.maximum && nonempty(value.approval) &&
    ["completed", "aborted", "failed", "failed_to_restore"].includes(String(value.status)) && nonempty(value.stopReason) &&
    typeof value.aborted === "boolean" && record(value.restoration) && finite(value.restoration.value) &&
    ["confirmed", "failed", "not_required"].includes(String(value.restoration.status)) &&
    (value.restoration.reason === undefined || nonempty(value.restoration.reason)) &&
    ((value.affectedSignal === undefined && value.address === undefined && value.durationMs === undefined &&
      value.trajectory === undefined && value.stopConditions === undefined) ||
      (nonempty(value.affectedSignal) && (value.address === undefined || nonempty(value.address)) && Number.isSafeInteger(value.durationMs) &&
        (value.durationMs as number) > 0 && trajectory(value.trajectory) && stringList(value.stopConditions))) &&
    (value.intervalMs === undefined || (finite(value.intervalMs) && value.intervalMs > 0)) &&
    (value.seed === undefined || (typeof value.seed === "number" && Number.isSafeInteger(value.seed) &&
      value.seed > 0 && value.seed <= 0xffff_ffff)) &&
    (value.frequenciesHz === undefined || positiveFiniteList(value.frequenciesHz));
}

function trajectory(value: unknown): value is Array<{ atMs: number; value: number }> {
  if (!Array.isArray(value) || value.length > 10_000) return false;
  let previous = -1;
  return value.every((point) => {
    const atMs = record(point) && typeof point.atMs === "number" ? point.atMs : Number.NaN;
    if (!record(point) || Object.keys(point).sort().join(",") !== "atMs,value" ||
        !Number.isSafeInteger(atMs) || atMs < 0 || atMs <= previous || !finite(point.value)) return false;
    previous = atMs;
    return true;
  });
}

function positiveFiniteList(value: unknown): value is number[] {
  return Array.isArray(value) && value.length <= 64 && value.every((item) => finite(item) && item > 0);
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
  const recording = value.recording as Record<string, unknown>;
  if (recording.provenance !== undefined && (value.schemaVersion !== 4 || !recordingProvenance(recording.provenance))) return false;
  const origin = recording.provenance as Record<string, unknown> | undefined;
  const loop = (origin?.loop ?? value.loop) as Record<string, unknown>;
  const units = (origin?.units ?? value.units) as Record<string, unknown>;
  if (origin && (origin.source !== value.source || units.pv !== (value.units as Record<string, unknown>).pv ||
      units.mv !== (value.units as Record<string, unknown>).mv)) return false;
  const samples = recording.rawSamples as Record<string, unknown>[];
  if (recording.acquisition !== undefined && !recordingAcquisition(recording.acquisition, samples)) return false;
  const runtimeOrdering = value.schemaVersion === 4 && record(recording.acquisition) && recording.acquisition.method === "runtime_scan_buffer";
  const channelIdentity = recordedChannelIdentity(samples[0]?.additional as readonly RecordedChannel[] | undefined);
  let previousElapsed = -1;
  let previousObservedAt = -1;
  let previousScan = -1;
  for (const sample of samples) {
    const sampleUnits = sample.units as Record<string, unknown>;
    if (recordedChannelIdentity(sample.additional as readonly RecordedChannel[] | undefined) !== channelIdentity ||
        sample.source !== value.source || sample.loopRevision !== loop.revision ||
        (value.schemaVersion === 4 && loop.topology === "open-loop" && sample.sp !== null) ||
        sampleUnits.sp !== units.sp || sampleUnits.pv !== units.pv || sampleUnits.mv !== units.mv ||
        (sample.elapsedSeconds as number) < 0 || (sample.elapsedSeconds as number) <= previousElapsed ||
        (!runtimeOrdering && (sample.observedAtMs as number) <= previousObservedAt) || (sample.scan as number) <= previousScan) {
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

function recordingProvenance(value: unknown): value is WorkflowRecordingProvenance {
  return record(value) && value.schemaVersion === 1 &&
    Object.keys(value).every(key => ["schemaVersion", "resultId", "resultRevision", "createdAt", "projectId", "source",
      "loop", "controller", "software", "units", "plant", "experiment"].includes(key)) &&
    nonempty(value.resultId) && positiveInteger(value.resultRevision) && nonempty(value.projectId) &&
    typeof value.createdAt === "string" && Number.isFinite(Date.parse(value.createdAt)) &&
    (value.source === "Simulation" || value.source === "Physical") &&
    record(value.loop) && nonempty(value.loop.id) && revision(value.loop.revision) &&
    (value.loop.topology === undefined || value.loop.topology === "open-loop" || value.loop.topology === "closed-loop") &&
    record(value.controller) && nonempty(value.controller.instance) && revision(value.controller.revision) &&
    software(value.software) && record(value.units) && [value.units.pv, value.units.mv].every(nonempty) &&
    (value.loop.topology === "open-loop" ? value.units.sp === "" : nonempty(value.units.sp)) &&
    (value.plant === undefined || plantContext(value.plant) && value.plant.source === value.source) &&
    (value.experiment === undefined || experimentContext(value.experiment));
}

function recordingAcquisition(value: unknown, samples: readonly Record<string, unknown>[]): boolean {
  if (!record(value) || !["ui_observed_snapshots", "runtime_scan_buffer"].includes(String(value.method)) ||
      !["incomplete", "complete"].includes(String(value.completeness)) ||
      !Number.isSafeInteger(value.observedSampleCount) || (value.observedSampleCount as number) < samples.length ||
      !Number.isSafeInteger(value.droppedObservedSamples) || (value.droppedObservedSamples as number) < 0 ||
      typeof value.actualCycleTimingAvailable !== "boolean" || typeof value.freshnessAvailable !== "boolean" ||
      !nonempty(value.reason) || !Array.isArray(value.missingScanRanges) ||
      !value.missingScanRanges.every(scanRange)) return false;
  if (value.programRevision !== undefined && !nonempty(value.programRevision)) return false;
  if (value.recordingId !== undefined && !nonempty(value.recordingId)) return false;
  if (value.scanRange !== undefined && !scanRange(value.scanRange)) return false;
  return !(value.method === "ui_observed_snapshots" &&
    (value.completeness !== "incomplete" || value.actualCycleTimingAvailable || value.freshnessAvailable));
}

function scanRange(value: unknown): boolean {
  return record(value) && Number.isSafeInteger(value.first) && Number.isSafeInteger(value.last) &&
    (value.first as number) >= 0 && (value.last as number) >= (value.first as number);
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
function finiteOrBoolean(value: unknown): value is number | boolean {
  return typeof value === "boolean" || finite(value);
}
function numericExperimentValue(value: unknown): number {
  return typeof value === "boolean" ? Number(value) : value as number;
}
function positiveInteger(value: unknown): value is number { return Number.isInteger(value) && (value as number) > 0; }
function nullableFinite(value: unknown): boolean { return value === null || finite(value); }
