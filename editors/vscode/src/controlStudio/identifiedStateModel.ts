import { createHash } from "crypto";
import { solveLeastSquares } from "./numerics";
import { evenlySampled } from "./evenSampling";
import type { GeneralStateSpaceModel, ModelChannel } from "./generalModel";
import type { RecordedRun } from "./runValidation";

export interface IdentificationChannels { inputs: readonly ModelChannel[]; outputs: readonly ModelChannel[] }
export interface ValidationPredictionPoint {
  timeSeconds: number;
  measured: number;
  predicted: number;
  residual: number;
}
export interface ValidationPredictionSeries {
  channelId: string;
  unit: string;
  recordingId: string;
  modelRevision: string;
  points: readonly ValidationPredictionPoint[];
}
export interface IdentifiedStateModelResult {
  model: GeneralStateSpaceModel;
  comparisons: readonly { order: number; normalizedErrors?: readonly number[]; reason: string; adequate: boolean }[];
  validationPredictions: readonly ValidationPredictionSeries[];
  estimationPredictions?: readonly ValidationPredictionSeries[];
}

/** Frozen multichannel autoregressive fits are judged by independent free-running predictions. */
export function identifyStateModel(estimation: RecordedRun, validation: RecordedRun | undefined,
  channels: IdentificationChannels, maximumNormalizedError = 0.15, maximumOrder = 4): IdentifiedStateModelResult {
  if (validation && (estimation.id === validation.id || estimation.conditions.programRevision !== validation.conditions.programRevision
    || estimation.conditions.modelRevision !== validation.conditions.modelRevision)) throw new Error("Independent records must have distinct identities and the same plant/program revision.");
  if (!Number.isFinite(maximumNormalizedError) || maximumNormalizedError <= 0 || maximumNormalizedError >= 1
    || !Number.isInteger(maximumOrder) || maximumOrder < 1 || maximumOrder > 8) throw new Error("Invalid identification adequacy or model-order limit.");
  const comparisonRecord = validation ?? estimation;
  if (!Array.isArray(estimation.conditions.sampleTimesSeconds) || estimation.conditions.sampleTimesSeconds.length < 2
    || !Array.isArray(comparisonRecord.conditions.sampleTimesSeconds) || comparisonRecord.conditions.sampleTimesSeconds.length < 2) {
    throw new Error("Identification requires complete sample timing for both independent records.");
  }
  // A live plant's samples carry the times its task actually ran; both records are compared on the estimation's even grid.
  // Plant outputs are interpolated between scans and inputs are held (A4.3).
  const linear = channels.outputs.map(channel => channel.id);
  const evenly = (run: RecordedRun, step?: number) => {
    try { return evenlySampled(run.samples, step, { linear }); } catch { throw new Error("Identification samples have gaps or inconsistent timing."); }
  };
  if (estimation.samples.length < 30 || comparisonRecord.samples.length < 30) throw new Error("Identification requires complete sample coverage.");
  const sampleTime = evenly(estimation).step;
  if (!Number.isFinite(sampleTime) || sampleTime <= 0) throw new Error("Identification requires a positive sample interval.");
  const read = (run: RecordedRun, samples = evenly(run, sampleTime).samples) => {
    // The resampled grid may hold a different number of points than the raw scans; gaps and missing scans are refused by evenlySampled.
    if (run.samples.length < 30) throw new Error("Identification requires complete sample coverage.");
    for (const channel of [...channels.inputs, ...channels.outputs]) {
      if (!run.channels.some(item => item.id === channel.id && item.unit === channel.unit)) throw new Error(`Missing identification channel ${channel.id} or incompatible unit.`);
    }
    return samples.map(sample => {
      return [...channels.inputs, ...channels.outputs].map(channel => {
        const value = sample.values[channel.id];
        if (value?.quality !== "good" || value.value === null || !Number.isFinite(value.value)) throw new Error(`Unqualified identification data on ${channel.id}.`);
        return value.value;
      });
    });
  };
  const comparisonSamples = evenly(comparisonRecord, sampleTime).samples;
  const train = read(estimation), heldOut = read(comparisonRecord, comparisonSamples);
  const m = channels.inputs.length, p = channels.outputs.length;
  const origin = train[0];
  const centered = (rows: number[][]) => rows.map(row => row.map((value, index) => value - origin[index]));
  const x = centered(train), v = centered(heldOut);
  const comparisons: { order: number; normalizedErrors?: number[]; reason: string; adequate: boolean }[] = [];
  let selected: GeneralStateSpaceModel | undefined;
  let selectedPredictions: readonly ValidationPredictionSeries[] | undefined;
  for (let order = 1; order <= maximumOrder; order++) {
    const count = order * (p + m);
    if (count > 64 || x.length - order < 3 * count) break;
    const regressors = x.slice(order).map((_, offset) => {
      const k = offset + order - 1;
      return [...Array.from({ length: order }, (_, lag) => x[k - lag].slice(m)).flat(),
        ...Array.from({ length: order }, (_, lag) => x[k - lag].slice(0, m)).flat()];
    });
    const solutions = channels.outputs.map((_, output) => solveLeastSquares(regressors, x.slice(order).map(row => row[m + output]),
      { absolute: 1e-10, relative: 1e-9, conditionLimit: 1e10 }));
    if (solutions.some(solution => solution.status !== "qualified")) {
      comparisons.push({ order, adequate: false, reason: "Parameters are not distinguishable with the supplied excitation." }); continue;
    }
    const coefficients = solutions.map(solution => solution.status === "qualified" ? [...solution.solution] : []);
    const n = p * order + m * (order - 1);
    const a = Array.from({ length: n }, () => Array<number>(n).fill(0));
    const b = Array.from({ length: n }, () => Array<number>(m).fill(0));
    for (let output = 0; output < p; output++) {
      for (let state = 0; state < p * order; state++) a[output][state] = coefficients[output][state];
      for (let input = 0; input < m; input++) b[output][input] = coefficients[output][p * order + input];
      for (let history = 0; history < m * (order - 1); history++) a[output][p * order + history] = coefficients[output][p * order + m + history];
    }
    for (let state = p; state < p * order; state++) a[state][state - p] = 1;
    for (let input = 0; input < m && order > 1; input++) b[p * order + input][input] = 1;
    for (let state = p * order + m; state < n; state++) a[state][state - m] = 1;
    let state = [...Array.from({ length: order }, (_, lag) => v[order - 1 - lag].slice(m)).flat(),
      ...Array.from({ length: order - 1 }, (_, lag) => v[order - 2 - lag].slice(0, m)).flat()];
    const errors = channels.outputs.map(() => 0);
    const predictions = channels.outputs.map(() => [] as ValidationPredictionPoint[]);
    for (let k = order - 1; k < v.length - 1; k++) {
      state = a.map((row, index) => row.reduce((sum, value, j) => sum + value * state[j], 0)
        + b[index].reduce((sum, value, j) => sum + value * v[k][j], 0));
      errors.forEach((_, output) => {
        const measured = v[k + 1][m + output] + origin[m + output];
        const predicted = state[output] + origin[m + output];
        predictions[output].push({ timeSeconds: comparisonSamples[k + 1].timeSeconds, measured, predicted, residual: measured - predicted });
        errors[output] += (state[output] - v[k + 1][m + output]) ** 2;
      });
    }
    const normalizedErrors = errors.map((error, output) => Math.sqrt(error / (v.length - order)) /
      Math.max(1e-9, Math.max(...v.map(row => row[m + output])) - Math.min(...v.map(row => row[m + output]))));
    const adequate = normalizedErrors.every(error => Number.isFinite(error) && error <= maximumNormalizedError);
    comparisons.push({ order, normalizedErrors, adequate, reason: validation ? adequate ? "Every held-out output meets the declared error limit." : "Independent free-running predictions exceed the declared error limit."
      : adequate ? "Estimation fit meets the error limit; independent validation is pending." : "Estimation fit exceeds the error limit." });
    if (adequate && !selected) {
      const c = channels.outputs.map((_, output) => Array.from({ length: n }, (_, index) => Number(index === output)));
      const d = channels.outputs.map(() => channels.inputs.map(() => 0));
      selected = { kind: "discrete-state-space", a, b, c, d, sampleTimeSeconds: sampleTime, delaySeconds: 0,
        revision: digest({ a, b, c, d, sampleTime, estimation: estimation.id, validation: validation?.id }), source: "fitted", validation: validation ? "validated" : "unvalidated",
        evidence: validation ? [digest(estimation), digest(validation)] : [digest(estimation)], inputs: channels.inputs, outputs: channels.outputs,
        states: Array.from({ length: n }, (_, index) => {
          const channel = index < p * order ? channels.outputs[index % p] : channels.inputs[(index - p * order) % m];
          return { id: `history-${index}`, unit: channel.unit, operatingRange: {
            minimum: channel.operatingRange.minimum - origin[index < p * order ? m + index % p : (index - p * order) % m],
            maximum: channel.operatingRange.maximum - origin[index < p * order ? m + index % p : (index - p * order) % m] } };
        }), operatingPoint: { inputs: origin.slice(0, m), outputs: origin.slice(m) } };
      selectedPredictions = channels.outputs.map((channel, output) => ({
        channelId: channel.id,
        unit: channel.unit,
        recordingId: comparisonRecord.id,
        modelRevision: selected!.revision,
        points: predictions[output],
      }));
    }
  }
  if (!selected) throw new Error(`No identifiable adequate state model. ${comparisons.map(item => `Order ${item.order}: ${item.reason}`).join(" ")}`);
  return { model: selected, comparisons, validationPredictions: validation ? selectedPredictions ?? [] : [],
    ...(!validation ? { estimationPredictions: selectedPredictions ?? [] } : {}) };
}
function digest(value: unknown): string { return `sha256:${createHash("sha256").update(JSON.stringify(value)).digest("hex")}`; }
