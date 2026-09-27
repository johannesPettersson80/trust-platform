import { isGeneralStateStructure } from "./generalStateStructure";
import { isSavedGeneralFeedback, type SavedGeneralFeedback } from "./generalFeedbackPersistence";
import { decodeGeneralModel, type GeneralModel, type GeneralModelFrequencyResult } from "./generalModel";
import type { FrequencyResponseEstimate } from "./numerics/contracts";
import { isModelTimeResponse, type ModelTimeResponse } from "./generalModelTime";
import { isModelPoleView, type ModelPoleView } from "./generalModelPoleView";
export interface ModelFrequencyGrid {
  minimumFrequency: number;
  maximumFrequency: number;
  points: number;
}
export interface SavedGeneralModelAnalysis {
  response?: ModelTimeResponse;
  poleView?: ModelPoleView;
  feedback?: SavedGeneralFeedback;
  periodic?: SavedPeriodicFrequencyAnalysis;
  model: GeneralModel;
  grid: ModelFrequencyGrid;
  result: Extract<GeneralModelFrequencyResult, {
    status: "qualified";
  }>;
}
export interface SavedPeriodicFrequencyAnalysis {
  startupSamples: number;
  sampleTimeSeconds: number;
  frequencies: readonly number[];
  segmentCount: number;
  minimumCoherence: number;
  recordingId?: string;
  recordingRevision?: string;
  result: Extract<FrequencyResponseEstimate, { status: "qualified" }>;
}
export function validModelGrid(value: unknown, channels: number): value is ModelFrequencyGrid {
  if (!record(value))
    return false;
  return finite(value.minimumFrequency) && finite(value.maximumFrequency) && value.minimumFrequency > 0 && value.maximumFrequency > value.minimumFrequency
    && Number.isSafeInteger(value.points) && (value.points as number) >= 2 && (value.points as number) <= 512 && (value.points as number) * channels <= 16384;
}
export function isSavedGeneralModelAnalysis(value: unknown): value is SavedGeneralModelAnalysis {
  if (!record(value))
    return false;
  const model = decodeGeneralModel(value.model);
  const result = value.result;
  if (!model || !validModelGrid(value.grid, model.inputs.length * model.outputs.length) || !record(result) || result.status !== "qualified"
    || JSON.stringify(result.model) !== JSON.stringify(model) || !Array.isArray(result.channels) || result.channels.length !== model.inputs.length * model.outputs.length
    || !Array.isArray(result.poles) || !result.poles.every(p => record(p) && finite(p.real) && finite(p.imaginary))
    || typeof result.stable !== "boolean" || typeof result.poleScope !== "string" || !record(result.capabilities))
    return false;
  if (value.response !== undefined && !isModelTimeResponse(value.response, model) || value.poleView !== undefined && !isModelPoleView(value.poleView, model)) return false;
  if (result.relativeGainMatrix !== undefined && (!Array.isArray(result.relativeGainMatrix)
    || result.relativeGainMatrix.length !== model.outputs.length
    || !result.relativeGainMatrix.every(row => Array.isArray(row) && row.length === model.inputs.length && row.every(finite)))) return false;
  if (result.stateStructure !== undefined && (model.kind === "transfer-function" || !isGeneralStateStructure(result.stateStructure, model))) return false;
  const grid = value.grid;
  if (value.feedback !== undefined && (!isSavedGeneralFeedback(value.feedback, Array.from({ length: grid.points }, (_, i) => frequencyAt(grid, i)))
    || JSON.stringify(value.feedback.definition.plant) !== JSON.stringify(model))) return false;
  if (value.periodic !== undefined && !isSavedPeriodicFrequencyAnalysis(value.periodic)) return false;
  if (!["relativeGain", "stepResponse", "transferConversion"].every(key => record((result.capabilities as Record<string, unknown>)[key])
    && typeof ((result.capabilities as Record<string, unknown>)[key] as Record<string, unknown>).available === "boolean"))
    return false;
  return result.channels.every((c, index) => record(c) && JSON.stringify(c.input) === JSON.stringify(model.inputs[index % model.inputs.length])
    && JSON.stringify(c.output) === JSON.stringify(model.outputs[Math.floor(index / model.inputs.length)]) && Array.isArray(c.points) && c.points.length === grid.points
    && c.points.every((p, i) => record(p) && [p.frequencyRadPerSecond, p.real, p.imaginary, p.magnitude, p.phaseRadians].every(finite)
      && Math.abs((p.frequencyRadPerSecond as number) - frequencyAt(grid, i)) <= 1e-10 * Math.max(1, frequencyAt(grid, i))));
}
export function isSavedPeriodicFrequencyAnalysis(value: unknown): value is SavedPeriodicFrequencyAnalysis {
  if (!record(value)) return false;
  const frequencies = value.frequencies;
  const result = value.result;
  if (!Number.isSafeInteger(value.startupSamples) || (value.startupSamples as number) < 1
      || !finite(value.sampleTimeSeconds) || (value.sampleTimeSeconds as number) <= 0 || !Array.isArray(frequencies)
      || frequencies.length === 0 || !frequencies.every(finite) || frequencies.some(frequency => frequency <= 0)
      || !Number.isSafeInteger(value.segmentCount) || (value.segmentCount as number) < 2
      || !finite(value.minimumCoherence) || (value.minimumCoherence as number) < 0 || (value.minimumCoherence as number) > 1
      || (value.recordingId !== undefined && typeof value.recordingId !== "string")
      || (value.recordingRevision !== undefined && typeof value.recordingRevision !== "string") || !record(result)
      || result.status !== "qualified" || !Array.isArray(result.points) || result.points.length !== frequencies.length) return false;
  return result.points.every((point, index) => record(point) && point.frequencyRadPerSecond === frequencies[index]
    && [point.real, point.imaginary, point.magnitude, point.phaseRadians, point.coherence].every(finite));
}
export function frequencyAt(grid: ModelFrequencyGrid, index: number): number { return index === grid.points - 1 ? grid.maximumFrequency : grid.minimumFrequency * (grid.maximumFrequency / grid.minimumFrequency) ** (index / (grid.points - 1)); }
function record(v: unknown): v is Record<string, unknown> { return !!v && typeof v === "object" && !Array.isArray(v); }
function finite(v: unknown): v is number { return typeof v === "number" && Number.isFinite(v); }
