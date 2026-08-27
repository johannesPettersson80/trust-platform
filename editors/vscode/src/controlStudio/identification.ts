export type ModelKind = "integrating" | "first_order" | "first_order_dead_time" | "second_order" | "second_order_dead_time";

export interface IdentificationSample {
  timeSeconds: number;
  input: number;
  output: number;
  quality: "good" | "uncertain" | "bad";
  inputUnit?: string;
  outputUnit?: string;
}

export interface ExcludedInterval { startSeconds: number; endSeconds: number }

export interface QualifiedIdentificationData {
  rawSamples: readonly IdentificationSample[];
  samples: readonly IdentificationSample[];
  suitable: boolean;
  reason?: string;
  inputUnit: string;
  outputUnit: string;
  sampleTimeSeconds: number;
}

export interface IdentifiedModel {
  kind: ModelKind;
  gain: number;
  timeConstantSeconds?: number;
  secondTimeConstantSeconds?: number;
  deadTimeSeconds?: number;
  normalizedError: number;
  stable: boolean;
}

export interface IdentificationResult {
  source: "Identified from simulation data" | "Identified from physical data";
  recommended: IdentifiedModel;
  models: readonly IdentifiedModel[];
}

export interface ModelFitPoint {
  timeSeconds: number;
  measured: number;
  predicted: number;
  residual: number;
}

export interface ModelFitEvidence {
  points: readonly ModelFitPoint[];
  rootMeanSquareError: number;
  outputUnit: string;
  stable: boolean;
  delaySeconds: number;
  operatingRange: { inputMinimum: number; inputMaximum: number; outputMinimum: number; outputMaximum: number };
  warnings: readonly string[];
}

export function modelFitEvidence(
  model: IdentifiedModel,
  samples: readonly IdentificationSample[]
): ModelFitEvidence {
  if (samples.length < 2 || !samples.every(finiteSample)) {
    throw new Error("Model evidence requires finite measured samples.");
  }
  const stepIndex = largestInputChange(samples);
  if (stepIndex < 1) throw new Error("Model evidence requires a measured input change.");
  const input0 = mean(samples.slice(0, stepIndex).map((sample) => sample.input));
  const output0 = mean(samples.slice(0, stepIndex).map((sample) => sample.output));
  const stepTime = samples[stepIndex].timeSeconds;
  const points = samples.map((sample) => {
    const time = Math.max(0, sample.timeSeconds - stepTime - (model.deadTimeSeconds ?? 0));
    const inputChange = sample.input - input0;
    const predicted = sample.timeSeconds < stepTime
      ? output0
      : output0 + model.gain * inputChange * responseFraction(model, time);
    return { timeSeconds: sample.timeSeconds, measured: sample.output, predicted,
      residual: sample.output - predicted };
  });
  const rootMeanSquareError = Math.sqrt(mean(points.map((point) => point.residual ** 2)));
  const warnings: string[] = [];
  if (!model.stable) warnings.push("The fitted model is not stable.");
  if (model.normalizedError > 0.2) warnings.push("Prediction error is high for controller design.");
  if ((model.deadTimeSeconds ?? 0) > 0 && (model.deadTimeSeconds ?? 0) < medianIntervals(samples)) {
    warnings.push("Dead time is shorter than one sample interval and is weakly resolved.");
  }
  return {
    points,
    rootMeanSquareError,
    outputUnit: samples.find((sample) => sample.outputUnit)?.outputUnit ?? "unit",
    stable: model.stable,
    delaySeconds: model.deadTimeSeconds ?? 0,
    operatingRange: {
      inputMinimum: Math.min(...samples.map((sample) => sample.input)),
      inputMaximum: Math.max(...samples.map((sample) => sample.input)),
      outputMinimum: Math.min(...samples.map((sample) => sample.output)),
      outputMaximum: Math.max(...samples.map((sample) => sample.output)),
    },
    warnings,
  };
}

export function qualifyIdentificationData(
  samples: readonly IdentificationSample[],
  excluded: readonly ExcludedInterval[]
): QualifiedIdentificationData {
  const rawSamples = samples.map((sample) => ({ ...sample }));
  const usable = rawSamples.filter((sample) =>
    sample.quality === "good" && finiteSample(sample) &&
    !excluded.some((interval) => sample.timeSeconds >= interval.startSeconds && sample.timeSeconds <= interval.endSeconds));
  const base = {
    rawSamples,
    samples: usable,
    inputUnit: rawSamples.find((sample) => sample.inputUnit)?.inputUnit ?? "unit",
    outputUnit: rawSamples.find((sample) => sample.outputUnit)?.outputUnit ?? "unit",
    sampleTimeSeconds: medianIntervals(usable),
  };
  if (usable.length < 20) return { ...base, suitable: false, reason: "At least 20 finite good samples are required." };
  if (!strictlyIncreasing(usable)) return { ...base, suitable: false, reason: "Sample times must be finite and strictly increasing." };
  if (span(usable.map((sample) => sample.input)) <= 1e-9) {
    return { ...base, suitable: false, reason: "Input excitation is too small to identify a model." };
  }
  if (span(usable.map((sample) => sample.output)) <= 1e-9) {
    return { ...base, suitable: false, reason: "Output response is too small to identify a model." };
  }
  return { ...base, suitable: true };
}

export function identifyPlant(
  samples: readonly IdentificationSample[],
  excluded: readonly ExcludedInterval[] = [],
  source: "Simulation" | "Physical" = "Simulation"
): IdentificationResult {
  const qualified = qualifyIdentificationData(samples, excluded);
  if (!qualified.suitable) throw new Error(qualified.reason);
  const values = qualified.samples;
  const stepIndex = largestInputChange(values);
  if (stepIndex < 2 || stepIndex >= values.length - 10) throw new Error("A bounded input step with pre- and post-step data is required.");
  const before = values.slice(0, stepIndex);
  const after = values.slice(Math.max(stepIndex + 1, values.length - Math.max(5, Math.floor(values.length / 10))));
  const input0 = mean(before.map((sample) => sample.input));
  const output0 = mean(before.map((sample) => sample.output));
  const input1 = mean(after.map((sample) => sample.input));
  const output1 = mean(after.map((sample) => sample.output));
  const inputChange = input1 - input0;
  const gain = (output1 - output0) / inputChange;
  if (!Number.isFinite(gain) || Math.abs(gain) < 1e-9) throw new Error("The measured input/output gain is not identifiable.");
  const stepTime = values[stepIndex].timeSeconds;
  const response = output1 - output0;
  const delayTime = firstCrossing(values, stepIndex, output0 + response * 0.02) - stepTime;
  const t63 = firstCrossing(values, stepIndex, output0 + response * 0.632) - stepTime;
  const dt = qualified.sampleTimeSeconds;
  const deadTime = Math.max(0, delayTime - dt);
  const tau = Math.max(dt, t63 - deadTime);
  const horizon = values[values.length - 1].timeSeconds - stepTime;
  const models: IdentifiedModel[] = [
    candidate("integrating", slopeGain(values, stepIndex, inputChange), undefined, undefined, 0),
    candidate("first_order", gain, tau + deadTime, undefined, 0),
    candidate("first_order_dead_time", gain, tau, undefined, deadTime),
    candidate("second_order", gain, Math.max(dt, (tau + deadTime) / 2.146), Math.max(dt, (tau + deadTime) / 2.146), 0),
    candidate("second_order_dead_time", gain, Math.max(dt, tau / 2.146), Math.max(dt, tau / 2.146), deadTime),
  ];
  for (const model of models) {
    model.normalizedError = modelError(model, values, stepIndex, input0, output0, inputChange, Math.abs(response));
  }
  const finite = models.filter((model) => Number.isFinite(model.normalizedError) && model.stable);
  const bestError = Math.min(...finite.map((model) => model.normalizedError));
  const acceptable = finite.filter((model) => model.normalizedError <= Math.max(0.03, bestError * 1.25));
  let recommended = (acceptable.length ? acceptable : finite)
    .sort((left, right) => complexity(left.kind) - complexity(right.kind) || left.normalizedError - right.normalizedError)[0];
  const firstOrder = models.find((model) => model.kind === "first_order")!;
  const delayedFirstOrder = models.find((model) => model.kind === "first_order_dead_time")!;
  // Add delay only when it is resolved by more than two samples and materially
  // improves prediction; a marginal fit increase never wins by itself.
  if (deadTime > 2 * dt && delayedFirstOrder.normalizedError < firstOrder.normalizedError * 0.98) {
    recommended = delayedFirstOrder;
  }
  if (!recommended || recommended.normalizedError > 0.35 || horizon <= dt) {
    throw new Error("No acceptable stable low-order model could be identified from the retained data.");
  }
  return { source: source === "Physical" ? "Identified from physical data" : "Identified from simulation data",
    recommended: { ...recommended }, models: models.map((model) => ({ ...model })) };
}

function candidate(kind: ModelKind, gain: number, tau?: number, tau2?: number, delay?: number): IdentifiedModel {
  return { kind, gain, ...(tau === undefined ? {} : { timeConstantSeconds: tau }),
    ...(tau2 === undefined ? {} : { secondTimeConstantSeconds: tau2 }),
    ...(delay === undefined ? {} : { deadTimeSeconds: delay }), normalizedError: Number.POSITIVE_INFINITY,
    stable: kind === "integrating" || ((tau ?? 0) > 0 && (tau2 === undefined || tau2 > 0)) };
}

function modelError(model: IdentifiedModel, samples: readonly IdentificationSample[], stepIndex: number,
  input0: number, output0: number, inputChange: number, outputSpan: number): number {
  let squared = 0;
  let count = 0;
  const stepTime = samples[stepIndex].timeSeconds;
  for (const sample of samples.slice(stepIndex)) {
    const time = Math.max(0, sample.timeSeconds - stepTime - (model.deadTimeSeconds ?? 0));
    const fraction = responseFraction(model, time);
    const predicted = output0 + model.gain * (sample.input - input0 || inputChange) * fraction;
    squared += (sample.output - predicted) ** 2;
    count += 1;
  }
  return Math.sqrt(squared / Math.max(1, count)) / Math.max(1e-12, outputSpan);
}

function responseFraction(model: IdentifiedModel, time: number): number {
  if (model.kind === "integrating") return time;
  if (model.secondTimeConstantSeconds !== undefined) {
    const first = model.timeConstantSeconds ?? 1;
    const second = model.secondTimeConstantSeconds;
    if (Math.abs(first - second) < 1e-12) {
      const x = time / first;
      return 1 - (1 + x) * Math.exp(-x);
    }
    return 1 - (first * Math.exp(-time / first) - second * Math.exp(-time / second)) / (first - second);
  }
  return 1 - Math.exp(-time / (model.timeConstantSeconds ?? 1));
}

function slopeGain(samples: readonly IdentificationSample[], stepIndex: number, inputChange: number): number {
  const tail = samples.slice(stepIndex + Math.floor((samples.length - stepIndex) / 2));
  const first = tail[0];
  const last = tail[tail.length - 1];
  return (last.output - first.output) / Math.max(1e-12, (last.timeSeconds - first.timeSeconds) * inputChange);
}

function firstCrossing(samples: readonly IdentificationSample[], start: number, target: number): number {
  const rising = target >= samples[start].output;
  return (samples.slice(start).find((sample) => rising ? sample.output >= target : sample.output <= target) ?? samples[samples.length - 1]).timeSeconds;
}

function largestInputChange(samples: readonly IdentificationSample[]): number {
  let index = -1; let change = 0;
  for (let current = 1; current < samples.length; current += 1) {
    const candidate = Math.abs(samples[current].input - samples[current - 1].input);
    if (candidate > change) { change = candidate; index = current; }
  }
  return index;
}

function finiteSample(sample: IdentificationSample): boolean {
  return Number.isFinite(sample.timeSeconds) && Number.isFinite(sample.input) && Number.isFinite(sample.output);
}
function strictlyIncreasing(samples: readonly IdentificationSample[]): boolean {
  return samples.every((sample, index) => index === 0 || sample.timeSeconds > samples[index - 1].timeSeconds);
}
function medianIntervals(samples: readonly IdentificationSample[]): number {
  const intervals = samples.slice(1).map((sample, index) => sample.timeSeconds - samples[index].timeSeconds)
    .filter((value) => Number.isFinite(value) && value > 0).sort((a, b) => a - b);
  return intervals.length ? intervals[Math.floor(intervals.length / 2)] : 0;
}
function span(values: readonly number[]): number { return Math.max(...values) - Math.min(...values); }
function mean(values: readonly number[]): number { return values.reduce((sum, value) => sum + value, 0) / values.length; }
function complexity(kind: ModelKind): number {
  return ({ integrating: 1, first_order: 2, first_order_dead_time: 3, second_order: 3, second_order_dead_time: 4 })[kind];
}
