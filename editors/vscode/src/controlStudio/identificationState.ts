import type { IdentificationReviewInput, IdentificationSample } from "./identification";
import { validStepResponseSettings } from "./stepResponseIdentification";
import { retainedIdentificationSamplesFromRuntimeRecording, type ControlExperimentKind } from "./experiment";
import { importedRecordingId } from "./recordingIdentity";
import type { ControlStudioWorkflowResult } from "./workflowResult";
import type { LoopRecording } from "./loopMonitor";
import { decodeIdentificationFormDraft, type IdentificationFormDraft } from "./identificationFormDraft";
import type { StructureChoice } from "./sensorLagComparison";
import { RECORDING_SAMPLE_LIMIT } from "./controlStudioLimits";

export interface IdentificationRecording {
  kind: ControlExperimentKind;
  data: ControlStudioWorkflowResult["recording"];
}

export type IdentificationPhase = "editing" | "selected" | "qualified" | "preview" | "validation-qualified" | "review" | "accepted" | "rejected" | "failed";
type SavedSample = Omit<IdentificationSample, "input" | "output" | "timeSeconds"> & {
  timeSeconds: number | null; input: number | null; output: number | null;
};
export type SavedIdentificationInput = Omit<IdentificationReviewInput, "estimation" | "validation"> & {
  estimation: readonly SavedSample[];
};
/** Persist inputs and explicit review stage. Derived fits are rechecked on restore. */
export interface SavedIdentificationState {
  schemaVersion: 1;
  maximumNormalizedError: number;
  phase: IdentificationPhase;
  /** Form-only editing snapshots omit this at runtime; keep the legacy type surface for existing readers. */
  current: SavedIdentificationInput;
  estimation?: SavedIdentificationInput;
  form?: IdentificationFormDraft;
  structureChoice?: StructureChoice;
}

export function identificationOriginalRecording(selected: LoopRecording | undefined,
  kind: IdentificationRecording["kind"] = selected?.provenance?.experiment?.kind ?? "observation"): IdentificationRecording | undefined {
  if (!selected?.rawSamples) return undefined;
  return { kind, data: { rawSamples: selected.rawSamples, selectedIntervals: [], acquisition: selected.evidence,
    ...(selected.provenance ? { provenance: selected.provenance } : {}) } };
}

export function saveIdentificationInput(input: IdentificationReviewInput): SavedIdentificationInput {
  return JSON.parse(JSON.stringify(input)) as SavedIdentificationInput;
}
export function restoreIdentificationInput(input: SavedIdentificationInput): IdentificationReviewInput {
  return { ...structuredClone(input), estimation: input.estimation.map(sample => ({ ...sample,
    timeSeconds: sample.timeSeconds ?? NaN, input: sample.input ?? NaN, output: sample.output ?? NaN })) };
}

export function decodeIdentificationState(value: unknown): SavedIdentificationState | undefined {
  if (!record(value) || value.schemaVersion !== 1 || !finite(value.maximumNormalizedError) || value.maximumNormalizedError <= 0 ||
    !["editing", "selected", "qualified", "preview", "validation-qualified", "review", "accepted", "rejected", "failed"].includes(String(value.phase)) ||
    (value.current !== undefined && !input(value.current)) || (value.estimation !== undefined && !input(value.estimation)) ||
    (value.form !== undefined && !decodeIdentificationFormDraft(value.form)) ||
    (value.structureChoice !== undefined && (!['first_order', 'additional_lag'].includes(String(value.structureChoice))
      || value.phase !== "preview" || !record(value.current) || value.current.requestedKind !== "sopdt")) ||
    Object.keys(value).some(key => !["schemaVersion", "maximumNormalizedError", "phase", "current", "estimation", "form", "structureChoice"].includes(key))) return undefined;
  if (value.phase === "editing" && value.form === undefined) return undefined;
  if (value.phase === "editing" && value.current === undefined && value.estimation !== undefined) return undefined;
  if (["preview", "validation-qualified", "review", "accepted"].includes(String(value.phase)) && !value.estimation) return undefined;
  if (value.phase === "qualified" && value.estimation) return undefined;
  if (value.phase === "preview" && (!value.current || JSON.stringify(value.current) !== JSON.stringify(value.estimation))) return undefined;
  for (const candidate of [value.current, value.estimation]) {
    if (record(candidate) && candidate.maximumNormalizedError !== value.maximumNormalizedError) return undefined;
  }
  if (JSON.stringify(value).length > 32 * 1024 * 1024) return undefined;
  return structuredClone(value) as unknown as SavedIdentificationState;
}
function input(value: unknown): boolean {
  return record(value) && ["Simulation", "Physical"].includes(String(value.source)) &&
    ["automatic", "integrating", "first_order", "fopdt", "second_order", "sopdt"].includes(String(value.requestedKind)) &&
    Array.isArray(value.estimation) && value.estimation.length <= RECORDING_SAMPLE_LIMIT && value.estimation.every(sample) &&
    Array.isArray(value.excluded) && value.excluded.length <= RECORDING_SAMPLE_LIMIT && value.excluded.every(interval =>
      record(interval) && finite(interval.startSeconds) && finite(interval.endSeconds) && interval.endSeconds >= interval.startSeconds) &&
    (value.estimationRecordingId === undefined || typeof value.estimationRecordingId === "string" && value.estimationRecordingId.length > 0 && value.estimationRecordingId.length <= 1024) &&
    (value.stepResponseSettings === undefined || validStepResponseSettings(value.stepResponseSettings)) &&
    (value.firstOrderStepResponseSettings === undefined || value.requestedKind === "sopdt" && validStepResponseSettings(value.firstOrderStepResponseSettings)) &&
    recordingMatchesInput(value) &&
    Object.keys(value).every(key => ["estimation", "excluded", "source", "requestedKind", "maximumNormalizedError", "estimationRecordingId", "stepResponseSettings", "firstOrderStepResponseSettings", "recording"].includes(key));
}
function recordingMatchesInput(input: Record<string, unknown>): boolean {
  if (input.recording === undefined) return true;
  const value = input.recording;
  if (!record(value) || !["observation", "setpoint_step", "load_step", "mv_step", "mv_pulse", "relay", "prbs", "multisine", "output_step", "project_sequence"].includes(String(value.kind)) ||
    !record(value.data) || !Array.isArray(value.data.rawSamples) || value.data.rawSamples.length > RECORDING_SAMPLE_LIMIT ||
    !record(value.data.acquisition) || !value.data.rawSamples.every(sample => record(sample) && sample.source === input.source)) return false;
  try {
    const retained = value as unknown as IdentificationRecording;
    const id = retained.data.provenance ? importedRecordingId(retained.data.provenance) : retained.data.acquisition!.recordingId;
    if (!id || id !== input.estimationRecordingId) return false;
    const projection = retainedIdentificationSamplesFromRuntimeRecording({ kind: retained.kind }, {
      samples: [], rawSamples: retained.data.rawSamples, evidence: retained.data.acquisition!, provenance: retained.data.provenance,
    });
    return JSON.stringify(projection) === JSON.stringify(input.estimation);
  } catch { return false; }
}
function sample(value: unknown): boolean {
  return record(value) && [value.timeSeconds, value.input, value.output].every(number => number === null || finite(number)) &&
    ["good", "uncertain", "bad"].includes(String(value.quality)) &&
    [value.inputUnit, value.outputUnit].every(unit => unit === undefined || typeof unit === "string" && unit.length <= 256) &&
    [value.clipped, value.saturated].every(flag => flag === undefined || typeof flag === "boolean") &&
    Object.keys(value).every(key => ["timeSeconds", "input", "output", "quality", "inputUnit", "outputUnit", "clipped", "saturated"].includes(key));
}
function finite(value: unknown): value is number { return typeof value === "number" && Number.isFinite(value); }
function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
