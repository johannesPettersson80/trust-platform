import type { IdentificationSample } from "./identification";

export interface StepResponseSettings {
  baselineDurationSeconds: number;
  finalDurationSeconds: number;
  maximumWindowRange: number;
  maximumWindowSlopePerSecond: number;
  measurementBound: number;
  onsetThreshold: number;
  onsetSamples: number;
  delay: { minimum: number; maximum: number; step: number };
  timeConstant: { minimum: number; maximum: number; step: number };
  sensorTimeConstant?: { minimum: number; maximum: number; step: number };
  modelAdequacy?: { maximumAbsoluteResidual: number; maximumFinalError: number };
  baselineStep: number;
  gainStep: number;
  timestampShiftSeconds: number;
}

export interface StepResponseWindow {
  startSeconds: number;
  endSeconds: number;
  count: number;
  mean: number;
  minimum: number;
  maximum: number;
  range: number;
  slopePerSecond: number;
}

interface Interval { minimum: number; maximum: number }

export interface StepResponseAnalysis {
  settings: StepResponseSettings;
  baseline: StepResponseWindow;
  final: StepResponseWindow;
  inputChange: number;
  stepTimeSeconds: number;
  holdEndSeconds: number;
  sampleTimeSeconds: number;
  detectableOnsetSeconds?: number;
  onsetThreshold: number;
  temperature63: number;
  best: {
    baseline: number;
    gain: number;
    delaySeconds: number;
    timeConstantSeconds: number;
    squaredError: number;
  };
  consistency: {
    count: number;
    gain?: Interval;
    delaySeconds?: Interval;
    timeConstantSeconds?: Interval;
    baseline?: Interval;
  };
  boundaryLimited: boolean;
  warnings: string[];
  points: Array<{ timeSeconds: number; measured: number; predicted: number; residual: number }>;
}

export interface StepResponseMeasurement {
  settings: StepResponseSettings;
  baseline: StepResponseWindow;
  final: StepResponseWindow;
  inputChange: number;
  stepTimeSeconds: number;
  holdEndSeconds: number;
  sampleTimeSeconds: number;
  detectableOnsetSeconds?: number;
  onsetThreshold: number;
  temperature63: number;
  warnings: string[];
}

export type StepResponseQualification = { suitable: true } | { suitable: false; reason: string };

const MAX_GRID_EVALUATIONS = 100_000_000;
const MAX_GRID_POINTS = 1_000_000;
const EPSILON = 1e-9;

export function validStepResponseSettings(value: unknown): value is StepResponseSettings {
  return stepResponseSettingsProblem(value) === undefined;
}

/** The same admission rules provide field-specific guidance in the editor. */
export function stepResponseSettingsProblem(value: unknown): { field: string; message: string } | undefined {
  if (!record(value)) return { field: "baselineDurationSeconds", message: "Enter measurement and search settings." };
  const scalar = ["baselineDurationSeconds", "finalDurationSeconds", "maximumWindowRange", "maximumWindowSlopePerSecond",
    "measurementBound", "onsetThreshold", "baselineStep", "gainStep"];
  for (const field of scalar) {
    if (!finite(value[field])) return { field, message: "Enter a finite number." };
    const zeroAllowed = field === "onsetThreshold";
    if (!(zeroAllowed ? positiveOrZero(value[field]) : positive(value[field]))) {
      return { field, message: zeroAllowed ? "Enter zero or a greater value." : "Enter a value greater than zero." };
    }
  }
  if (!finite(value.timestampShiftSeconds)) return { field: "timestampShiftSeconds", message: "Enter a finite time offset." };
  if (!Number.isInteger(value.onsetSamples) || value.onsetSamples < 1 || value.onsetSamples > 100) {
    return { field: "onsetSamples", message: "Enter a whole number from 1 to 100." };
  }
  let combinations = 1;
  for (const group of ["delay", "timeConstant"]) {
    const grid = value[group];
    if (!record(grid)) return { field: `${group}.minimum`, message: "Enter the grid bounds and step." };
    for (const field of ["minimum", "maximum", "step"]) {
      if (!finite(grid[field])) return { field: `${group}.${field}`, message: "Enter a finite number." };
      const positiveRequired = field === "step" || group === "timeConstant" && field === "minimum";
      if (!(positiveRequired ? positive(grid[field]) : positiveOrZero(grid[field]))) {
        return { field: `${group}.${field}`, message: positiveRequired ? "Enter a value greater than zero." : "Enter zero or a greater value." };
      }
    }
    if (grid.maximum < grid.minimum) return { field: `${group}.maximum`, message: "Enter a maximum at least as large as the minimum." };
    const intervals = (grid.maximum - grid.minimum) / grid.step;
    if (!Number.isFinite(intervals) || Math.abs(intervals - Math.round(intervals)) > EPSILON) {
      return { field: `${group}.step`, message: "Choose a grid step that divides the minimum-to-maximum span exactly." };
    }
    combinations *= gridPointCount(grid.minimum, grid.maximum, grid.step)!;
  }
  if (combinations > 100_000) return { field: "delay.step", message: "Use a coarser grid or narrower delay and time-constant bounds." };
  if (value.sensorTimeConstant !== undefined) {
    const grid = value.sensorTimeConstant;
    if (!record(grid)) return { field: "sensorTimeConstant.minimum", message: "Enter the sensor-time bounds and step." };
    for (const field of ["minimum", "maximum", "step"]) {
      if (!positive(grid[field])) return { field: `sensorTimeConstant.${field}`, message: "Enter a finite value greater than zero." };
    }
    if (grid.maximum < grid.minimum) return { field: "sensorTimeConstant.maximum", message: "Enter a maximum at least as large as the minimum." };
    const intervals = (grid.maximum - grid.minimum) / grid.step;
    if (!Number.isFinite(intervals) || Math.abs(intervals - Math.round(intervals)) > EPSILON || intervals > MAX_GRID_POINTS) {
      return { field: "sensorTimeConstant.step", message: "Choose a bounded grid step that divides the sensor-time span exactly." };
    }
  }
  if (value.modelAdequacy !== undefined) {
    for (const field of ["maximumAbsoluteResidual", "maximumFinalError"]) {
      if (!record(value.modelAdequacy) || !positive(value.modelAdequacy[field])) {
        return { field: `modelAdequacy.${field}`, message: "Enter a finite model-error limit greater than zero." };
      }
    }
  }
  return undefined;
}

export function qualifyStepResponse(samples: readonly IdentificationSample[], settings: StepResponseSettings): StepResponseQualification {
  try {
    const prepared = prepare(samples, settings);
    inspectWindows(prepared, settings);
    return { suitable: true };
  } catch (error) {
    return { suitable: false, reason: error instanceof Error ? error.message : String(error) };
  }
}

export function measureStepResponse(samples: readonly IdentificationSample[], settings: StepResponseSettings): StepResponseMeasurement {
  if (!validStepResponseSettings(settings)) throw new Error("Invalid step-response identification settings.");
  const prepared = prepare(samples, settings);
  const windows = inspectWindows(prepared, settings);
  const hold = prepared.samples.filter((sample) => sample.timeSeconds >= prepared.stepTime && sample.timeSeconds < prepared.holdEnd);
  const direction = Math.sign(windows.final.mean - windows.baseline.mean);
  const onsetThreshold = windows.baseline.mean + (direction || 1) * settings.onsetThreshold;
  const detectableOnsetSeconds = detectOnset(hold, prepared.stepTime, onsetThreshold, settings.onsetSamples, direction);
  const warnings = Math.abs(settings.timestampShiftSeconds) > 0
    ? ["Timestamp-shift tolerance is retained for sensitivity analysis and is not nominal model evidence."]
    : [];
  return {
    settings: cloneSettings(settings), baseline: windows.baseline, final: windows.final,
    inputChange: prepared.inputChange, stepTimeSeconds: prepared.stepTime, holdEndSeconds: prepared.holdEnd,
    sampleTimeSeconds: prepared.sampleTime,
    ...(detectableOnsetSeconds === undefined ? {} : { detectableOnsetSeconds }), onsetThreshold,
    temperature63: windows.baseline.mean + 0.632 * (windows.final.mean - windows.baseline.mean), warnings,
  };
}

export function analyzeStepResponse(samples: readonly IdentificationSample[], settings: StepResponseSettings): StepResponseAnalysis {
  if (!validStepResponseSettings(settings)) throw new Error("Invalid step-response identification settings.");
  const prepared = prepare(samples, settings);
  const windows = inspectWindows(prepared, settings);
  const hold = prepared.samples.filter((sample) => sample.timeSeconds >= prepared.stepTime && sample.timeSeconds < prepared.holdEnd);
  const baselineMean = windows.baseline.mean;
  const finalMean = windows.final.mean;
  const deltaInput = prepared.inputChange;
  if (Math.abs(deltaInput) <= EPSILON) throw new Error("Step-response identification requires a nonzero measured input change.");
  const gain = (finalMean - baselineMean) / deltaInput;
  if (!Number.isFinite(gain)) throw new Error("Step-response identification gain is not finite.");
  const delayValues = gridValues(settings.delay);
  const tauValues = gridValues(settings.timeConstant);
  const evaluationCount = delayValues.length * tauValues.length;
  const gainInterval = gainBounds(windows.baseline, windows.final, deltaInput, settings.measurementBound);
  const gainValues = lattice(gainInterval.minimum, gainInterval.maximum, settings.gainStep);
  if (evaluationCount * Math.max(1, gainValues.length) * Math.max(1, hold.length) > MAX_GRID_EVALUATIONS) {
    throw new Error("Step-response identification grid is too large; refine the requested bounds or spacing.");
  }
  let best = { baseline: baselineMean, gain, delaySeconds: delayValues[0], timeConstantSeconds: tauValues[0], squaredError: Number.POSITIVE_INFINITY };
  for (const delaySeconds of delayValues) {
    for (const timeConstantSeconds of tauValues) {
      let squaredError = 0;
      for (const sample of hold) {
        const predicted = predict(baselineMean, gain, deltaInput, sample.timeSeconds - prepared.stepTime, delaySeconds, timeConstantSeconds, settings.timestampShiftSeconds);
        squaredError += (sample.output - predicted) ** 2;
      }
      if (squaredError < best.squaredError) best = { baseline: baselineMean, gain, delaySeconds, timeConstantSeconds, squaredError };
    }
  }
  const consistency = consistentSet(hold, windows, deltaInput, settings, gainValues, delayValues, tauValues);
  const warnings: string[] = [];
  if (best.delaySeconds === settings.delay.maximum && settings.delay.maximum > 0
    || best.delaySeconds === settings.delay.minimum && settings.delay.minimum > 0
    || best.timeConstantSeconds === settings.timeConstant.minimum && settings.timeConstant.minimum > 0
    || best.timeConstantSeconds === settings.timeConstant.maximum) {
    warnings.push("The best grid fit is on a search boundary; inspect or extend the justified range.");
  }
  if (consistency.count === 0) warnings.push("No consistent parameter combination satisfies the declared measurement bound across the held response.");
  const direction = Math.sign(finalMean - baselineMean);
  const onsetThreshold = baselineMean + (direction || 1) * settings.onsetThreshold;
  const detectableOnsetSeconds = detectOnset(hold, prepared.stepTime, onsetThreshold, settings.onsetSamples, direction);
  const points = hold.map((sample) => {
    const predicted = predict(best.baseline, best.gain, deltaInput, sample.timeSeconds - prepared.stepTime, best.delaySeconds, best.timeConstantSeconds, settings.timestampShiftSeconds);
    return { timeSeconds: sample.timeSeconds, measured: sample.output, predicted, residual: sample.output - predicted };
  });
  return {
    settings: cloneSettings(settings), baseline: windows.baseline, final: windows.final, inputChange: deltaInput,
    stepTimeSeconds: prepared.stepTime, holdEndSeconds: prepared.holdEnd, sampleTimeSeconds: prepared.sampleTime,
    ...(detectableOnsetSeconds === undefined ? {} : { detectableOnsetSeconds }), onsetThreshold,
    temperature63: baselineMean + 0.632 * (finalMean - baselineMean), best, consistency,
    boundaryLimited: warnings.some((warning) => /boundary/.test(warning)),
    warnings: [...warnings, ...(Math.abs(settings.timestampShiftSeconds) > 0 ? ["Timestamp-shift tolerance is retained for sensitivity analysis and is not nominal model evidence."] : [])], points,
  };
}

function prepare(samples: readonly IdentificationSample[], settings: StepResponseSettings): {
  samples: IdentificationSample[]; stepIndex: number; stepTime: number; holdEnd: number; inputChange: number; sampleTime: number;
} {
  if (!validStepResponseSettings(settings)) throw new Error("Invalid step-response identification settings.");
  if (samples.length < 4 || samples.length > 10_000 || !samples.every((sample) => sample.quality === "good" && !sample.clipped && !sample.saturated &&
    sample.inputUnit === samples[0].inputUnit && sample.outputUnit === samples[0].outputUnit && finite(sample.timeSeconds) && finite(sample.input) && finite(sample.output))) {
    throw new Error("Step-response identification requires complete finite good-quality samples.");
  }
  const intervals = samples.slice(1).map((sample, index) => sample.timeSeconds - samples[index].timeSeconds);
  const sampleTime = median(intervals);
  if (!(sampleTime > 0) || Math.abs(settings.timestampShiftSeconds) > sampleTime + EPSILON
    || intervals.some((interval) => Math.abs(interval - sampleTime) > Math.max(EPSILON, sampleTime * 1e-6))) {
    throw new Error("Step-response samples must have complete uniformly timed coverage.");
  }
  const stepIndex = samples.slice(1).findIndex((sample, index) => Math.abs(sample.input - samples[index].input) > EPSILON) + 1;
  if (stepIndex < 1) throw new Error("Step-response identification requires one nonzero input step.");
  const inputBefore = samples[stepIndex - 1].input;
  const inputAfter = samples[stepIndex].input;
  const restorationIndex = samples.findIndex((sample, index) => index > stepIndex && Math.abs(sample.input - inputAfter) > EPSILON);
  if (restorationIndex <= stepIndex) throw new Error("Step-response recording must include one complete held step and restoration.");
  if (samples.slice(0, stepIndex).some((sample) => Math.abs(sample.input - inputBefore) > EPSILON)
    || samples.slice(stepIndex, restorationIndex).some((sample) => Math.abs(sample.input - inputAfter) > EPSILON)
    || Math.abs(samples[restorationIndex].input - inputBefore) > EPSILON
    || samples.slice(restorationIndex).some((sample) => Math.abs(sample.input - inputBefore) > EPSILON)) {
    throw new Error("Step-response input contains ambiguous excitation before restoration.");
  }
  const holdEnd = samples[restorationIndex].timeSeconds;
  return { samples: samples.map((sample) => ({ ...sample })), stepIndex, stepTime: samples[stepIndex].timeSeconds,
    holdEnd, inputChange: inputAfter - inputBefore, sampleTime };
}

function inspectWindows(prepared: ReturnType<typeof prepare>, settings: StepResponseSettings): { baseline: StepResponseWindow; final: StepResponseWindow } {
  const baselineStart = prepared.stepTime - settings.baselineDurationSeconds;
  const finalStart = prepared.holdEnd - settings.finalDurationSeconds;
  if (finalStart < prepared.stepTime - EPSILON) throw new Error("Final window must lie within the held step before restoration. Shorten the final window.");
  const baseline = window(prepared.samples, baselineStart, prepared.stepTime, prepared.sampleTime, "baseline");
  const final = window(prepared.samples, finalStart, prepared.holdEnd, prepared.sampleTime, "final");
  for (const item of [baseline, final]) {
    if (item.range > settings.maximumWindowRange + EPSILON || Math.abs(item.slopePerSecond) > settings.maximumWindowSlopePerSecond + EPSILON) {
      throw new Error(`${item.count > 0 ? item.startSeconds === baselineStart ? "Baseline" : "Final" : "Step-response"} window is not steady within the declared range and slope limits.`);
    }
  }
  return { baseline, final };
}

function window(samples: readonly IdentificationSample[], start: number, end: number, sampleTime: number, label: string): StepResponseWindow {
  const values = samples.filter((sample) => sample.timeSeconds >= start - EPSILON && sample.timeSeconds < end - EPSILON);
  const name = label[0].toUpperCase() + label.slice(1);
  if (values.length < 2) throw new Error(`${name} window lacks the required time coverage. Include at least two recorded samples.`);
  if (values[0].timeSeconds >= start + sampleTime - EPSILON || values[values.length - 1].timeSeconds < end - sampleTime - EPSILON) {
    throw new Error(`${name} window lacks the required time coverage. Shorten the ${label} window or import a longer complete recording.`);
  }
  const numbers = values.map((sample) => sample.output);
  const duration = values[values.length - 1].timeSeconds - values[0].timeSeconds;
  return { startSeconds: start, endSeconds: end, count: values.length, mean: mean(numbers), minimum: Math.min(...numbers), maximum: Math.max(...numbers),
    range: Math.max(...numbers) - Math.min(...numbers), slopePerSecond: duration > 0 ? (numbers[numbers.length - 1] - numbers[0]) / duration : 0 };
}

function consistentSet(hold: readonly IdentificationSample[], windows: { baseline: StepResponseWindow; final: StepResponseWindow }, deltaInput: number,
  settings: StepResponseSettings, gains: readonly number[], delays: readonly number[], taus: readonly number[]): StepResponseAnalysis["consistency"] {
  let count = 0;
  let gainMin = Number.POSITIVE_INFINITY, gainMax = Number.NEGATIVE_INFINITY;
  let delayMin = Number.POSITIVE_INFINITY, delayMax = Number.NEGATIVE_INFINITY;
  let tauMin = Number.POSITIVE_INFINITY, tauMax = Number.NEGATIVE_INFINITY;
  let baselineMin = Number.POSITIVE_INFINITY, baselineMax = Number.NEGATIVE_INFINITY;
  for (const delay of delays) for (const tau of taus) {
    const responses = hold.map((sample) => responseFraction(sample.timeSeconds - hold[0].timeSeconds, delay, tau, settings.timestampShiftSeconds));
    for (const gain of gains) {
    let lower = Math.max(windows.baseline.mean - settings.measurementBound, windows.final.mean - settings.measurementBound - gain * deltaInput);
    let upper = Math.min(windows.baseline.mean + settings.measurementBound, windows.final.mean + settings.measurementBound - gain * deltaInput);
    for (let index = 0; index < hold.length; index += 1) {
      const sample = hold[index];
      const response = responses[index];
      lower = Math.max(lower, sample.output - settings.measurementBound - gain * deltaInput * response);
      upper = Math.min(upper, sample.output + settings.measurementBound - gain * deltaInput * response);
      if (lower > upper + EPSILON) break;
    }
    const first = Math.ceil((lower - (windows.baseline.mean - settings.measurementBound) - EPSILON) / settings.baselineStep);
    const last = Math.floor((upper - (windows.baseline.mean - settings.measurementBound) + EPSILON) / settings.baselineStep);
    if (first > last) continue;
    const candidateCount = last - first + 1;
    count += candidateCount;
    gainMin = Math.min(gainMin, gain); gainMax = Math.max(gainMax, gain);
    delayMin = Math.min(delayMin, delay); delayMax = Math.max(delayMax, delay);
    tauMin = Math.min(tauMin, tau); tauMax = Math.max(tauMax, tau);
    baselineMin = Math.min(baselineMin, windows.baseline.mean - settings.measurementBound + first * settings.baselineStep);
    baselineMax = Math.max(baselineMax, windows.baseline.mean - settings.measurementBound + last * settings.baselineStep);
    }
  }
  return count === 0 ? { count: 0 } : { count, gain: { minimum: gainMin, maximum: gainMax }, delaySeconds: { minimum: delayMin, maximum: delayMax },
    timeConstantSeconds: { minimum: tauMin, maximum: tauMax }, baseline: { minimum: baselineMin, maximum: baselineMax } };
}

function predict(baseline: number, gain: number, inputChange: number, elapsed: number, delay: number, tau: number, timestampShiftSeconds: number): number {
  return baseline + gain * inputChange * responseFraction(elapsed, delay, tau, timestampShiftSeconds);
}
function responseFraction(elapsed: number, delay: number, tau: number, timestampShiftSeconds = 0): number {
  const afterDelay = elapsed + timestampShiftSeconds - delay;
  return afterDelay < 0 ? 0 : 1 - Math.exp(-afterDelay / tau);
}
function detectOnset(samples: readonly IdentificationSample[], stepTime: number, threshold: number, required: number, direction: number): number | undefined {
  for (let index = 0; index <= samples.length - required; index += 1) {
    if (samples.slice(index, index + required).every((sample) => direction * (sample.output - threshold) > 0)) return samples[index].timeSeconds - stepTime;
  }
  return undefined;
}
export function gainBounds(baseline: StepResponseWindow, final: StepResponseWindow, inputChange: number, measurementBound: number): Interval {
  const values = [(final.mean - measurementBound - (baseline.mean + measurementBound)) / inputChange,
    (final.mean + measurementBound - (baseline.mean - measurementBound)) / inputChange];
  return { minimum: Math.min(...values), maximum: Math.max(...values) };
}
function gridValues(grid: { minimum: number; maximum: number; step: number }): number[] { return lattice(grid.minimum, grid.maximum, grid.step); }
export function lattice(minimum: number, maximum: number, step: number): number[] {
  const count = gridPointCount(minimum, maximum, step);
  if (count === undefined || count > MAX_GRID_POINTS) throw new Error("Step-response identification grid is too large; refine the requested bounds or spacing.");
  return Array.from({ length: Math.max(0, count + 1) }, (_, index) => minimum + index * step);
}
function gridPointCount(minimum: number, maximum: number, step: number): number | undefined {
  const raw = (maximum - minimum) / step;
  return Number.isFinite(raw) && raw >= -EPSILON ? Math.max(0, Math.floor(raw + EPSILON)) : undefined;
}
function cloneSettings(settings: StepResponseSettings): StepResponseSettings {
  return structuredClone(settings);
}
function record(value: unknown): value is Record<string, any> { return !!value && typeof value === "object" && !Array.isArray(value); }
function finite(value: unknown): value is number { return typeof value === "number" && Number.isFinite(value); }
function positive(value: unknown): value is number { return finite(value) && value > 0; }
function positiveOrZero(value: unknown): value is number { return finite(value) && value >= 0; }
function mean(values: readonly number[]): number { return values.reduce((sum, value) => sum + value, 0) / values.length; }
function median(values: readonly number[]): number { const sorted = [...values].sort((left, right) => left - right); return sorted[Math.floor(sorted.length / 2)]; }
