import { createHash } from "crypto";
import { isDeepStrictEqual } from "util";
import { decodeConnectedIdentificationRecord } from "./connectedIdentificationRecord";
import { evenlySampled } from "./evenSampling";
import { frozenModelPrediction } from "./frozenModelPrediction";
import type { GeneralModel } from "./generalModel";
import type { ValidationPredictionSeries } from "./identifiedStateModel";
import { independentResponseReason, scoreIndependentResponse } from "./independentModelScore";
import type { RuntimeVerificationResult } from "./verificationClient";
import type { RequirementStatus } from "./runValidation";

export interface ModelValidationChannels {
  inputs: readonly { id: string; symbol: string; unit: string }[];
  outputs: readonly { id: string; symbol: string; unit: string }[];
}
export interface IndependentModelValidation {
  modelRevision: string;
  estimationRecordingId: string;
  channels: ModelValidationChannels;
  maximumNormalizedError: number;
  record: RuntimeVerificationResult;
  status: RequirementStatus;
  /** Per measured output: the error over the whole response and over its final 10 %; both must be within the limit (B5.2). */
  rows: readonly { channel: string; normalizedError?: number; finalNormalizedError?: number; limit: number; status: RequirementStatus; reason: string }[];
  predictions: readonly ValidationPredictionSeries[];
}
export function recordingFingerprint(run: RuntimeVerificationResult["run"]): string {
  return `sha256:${createHash("sha256").update(JSON.stringify(run)).digest("hex")}`;
}

/** The independent record never changes fitted coefficients or parameter pins. */
export function validateFrozenModel(model: GeneralModel, estimation: RuntimeVerificationResult, record: RuntimeVerificationResult,
  channels: ModelValidationChannels, maximumNormalizedError: number): IndependentModelValidation {
  const base = { modelRevision: model.revision, estimationRecordingId: estimation.run.id, channels, maximumNormalizedError, record };
  try {
    if (!model.evidence.includes(recordingFingerprint(estimation.run)) || estimation.run.id === record.run.id
      || estimation.projectRevision !== record.projectRevision || estimation.run.conditions.modelRevision !== record.run.conditions.modelRevision) {
      throw new Error("Model validation needs a distinct recording from the same plant/program and the original estimation evidence.");
    }
    if (!Number.isFinite(maximumNormalizedError) || maximumNormalizedError <= 0 || maximumNormalizedError >= 1
      || record.evaluation.status !== "Pass") throw new Error("Model validation requires a declared error limit and a complete bounded plant recording.");
    for (const kind of ["inputs", "outputs"] as const) {
      if (channels[kind].length !== model[kind].length || channels[kind].some((channel, i) => channel.id !== model[kind][i].id
        || channel.unit !== model[kind][i].unit || !record.run.channels.some(entry => entry.id === channel.symbol && entry.unit === channel.unit))) {
        throw new Error("Model validation channel identities or units differ.");
      }
    }
    // The comparison grid holds the plant inputs and interpolates the measured outputs between scans (A4.3).
    const { samples, step } = evenlySampled(record.run.samples, undefined, { linear: channels.outputs.map(channel => channel.symbol) });
    const read = (kind: keyof ModelValidationChannels) => samples.map(sample => channels[kind].map(channel => {
      const measured = sample.values[channel.symbol];
      if (measured?.quality !== "good" || measured.value === null || !Number.isFinite(measured.value)) throw new Error("The independent recording contains unqualified measurements.");
      return measured.value;
    }));
    const inputs = read("inputs"), outputs = read("outputs");
    if (channels.inputs.some((_, i) => Math.max(...inputs.map(row => row[i])) - Math.min(...inputs.map(row => row[i])) <= 1e-9)) throw new Error("Every selected input needs fresh excitation for independent model validation.");
    const predicted = frozenModelPrediction(model, inputs, outputs, step);
    const predictions = channels.outputs.map((channel, i) => ({ channelId: channel.id, unit: channel.unit,
      recordingId: record.run.id, modelRevision: model.revision,
      points: predicted.values.map((row, k) => ({ timeSeconds: samples[k + predicted.start].timeSeconds,
        measured: outputs[k + predicted.start][i], predicted: row[i], residual: outputs[k + predicted.start][i] - row[i] })) }));
    const rows = predictions.map((series, i) => {
      const score = scoreIndependentResponse(series.points, maximumNormalizedError);
      return { channel: channels.outputs[i].symbol, normalizedError: score.normalizedError, finalNormalizedError: score.finalNormalizedError,
        limit: maximumNormalizedError, status: score.status, reason: independentResponseReason(score) };
    });
    return { ...base, status: rows.every(row => row.status === "Pass") ? "Pass" : "Fail", rows, predictions };
  } catch (error) {
    return { ...base, status: "Inconclusive", rows: channels.outputs.map(channel => ({ channel: channel.symbol,
      limit: maximumNormalizedError, status: "Inconclusive", reason: error instanceof Error ? error.message : String(error) })), predictions: [] };
  }
}

export function decodeIndependentModelValidation(value: unknown, model: GeneralModel | undefined,
  records: readonly RuntimeVerificationResult[]): IndependentModelValidation | undefined {
  if (!value || typeof value !== "object" || !model) return undefined;
  const report = value as IndependentModelValidation;
  const record = decodeConnectedIdentificationRecord(report.record);
  const estimation = records.find(item => item.run.id === report.estimationRecordingId);
  if (!record || !estimation || !report.channels || !Array.isArray(report.channels.inputs) || !Array.isArray(report.channels.outputs)) return undefined;
  const recomputed = validateFrozenModel(model, estimation, record, report.channels, report.maximumNormalizedError);
  return isDeepStrictEqual(recomputed, report) ? structuredClone(report) : undefined;
}
