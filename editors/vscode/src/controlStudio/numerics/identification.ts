import type {
  LowOrderIdentificationOptions,
  LowOrderIdentificationResult,
  LowOrderModelFit,
  LowOrderModelKind,
  ParameterInterval,
  TimeDomainSample,
} from "./contracts";

interface StepData {
  samples: readonly TimeDomainSample[];
  stepIndex: number;
  stepTime: number;
  inputBefore: number;
  inputAfter: number;
  outputBefore: number;
  inputChange: number;
  outputScale: number;
  sampleTime: number;
  horizon: number;
}

interface Parameters { gain: number; tau1?: number; tau2?: number; delay?: number }

/** Fits low-order step-response models and scores them only on a separate run. */
export function identifyLowOrderModels(
  estimationSamples: readonly TimeDomainSample[],
  validationSamples: readonly TimeDomainSample[],
  options: LowOrderIdentificationOptions
): LowOrderIdentificationResult {
  const optionFailure = validateOptions(options);
  if (optionFailure) return optionFailure;
  const estimation = prepareStep(estimationSamples, options);
  if ("reason" in estimation) return estimation;
  const validation = prepareStep(validationSamples, options);
  if ("reason" in validation) return validation;

  const models = options.allowedKinds
    .filter((kind, index, kinds) => kinds.indexOf(kind) === index)
    .map((kind) => fitKind(kind, estimation, validation))
    .filter((model): model is LowOrderModelFit => model !== undefined);
  // Adequacy is a declared task tolerance, independent of a more complex model's score.
  const adequacyLimit = options.maximumNormalizedValidationError;
  const finalWindowSamples = Math.max(3, Math.ceil(validation.samples.length * 0.1));
  const qualified = models.filter((model) =>
    Number.isFinite(model.normalizedValidationError)
    && model.normalizedValidationError <= adequacyLimit
    && normalizedError(model.validationResiduals.slice(-finalWindowSamples), validation.outputScale) <= adequacyLimit);
  if (qualified.length === 0) return { status: "rejected", reason: "no-qualified-model" };
  qualified.sort((left, right) => complexity(left.kind) - complexity(right.kind)
    || left.normalizedValidationError - right.normalizedValidationError);
  const recommended = qualified[0];
  return { status: "qualified", recommended, models };
}

function fitKind(
  kind: LowOrderModelKind,
  estimation: StepData,
  validation: StepData
): LowOrderModelFit | undefined {
  let best: Parameters | undefined;
  let bestError = Number.POSITIVE_INFINITY;
  const delays = kind.endsWith("dead_time")
    ? linearGrid(0, Math.min(estimation.horizon * 0.3, 5), estimation.sampleTime)
    : [0];
  const taus = logarithmicGrid(estimation.sampleTime, Math.max(estimation.sampleTime, estimation.horizon), 45);

  if (kind === "static_gain" || kind === "integrating") {
    const fitted = optimalGain(kind, estimation, {});
    best = { gain: fitted.gain };
    bestError = fitted.error;
  } else if (kind === "first_order" || kind === "first_order_dead_time") {
    for (const delay of delays) {
      for (const tau1 of taus) {
        const fitted = optimalGain(kind, estimation, { tau1, delay });
        if (fitted.error < bestError) {
          bestError = fitted.error;
          best = { gain: fitted.gain, tau1, ...(delay > 0 ? { delay } : {}) };
        }
      }
    }
  } else {
    const secondOrderTaus = logarithmicGrid(estimation.sampleTime, Math.max(estimation.sampleTime, estimation.horizon), 28);
    for (const delay of delays) {
      for (let firstIndex = 0; firstIndex < secondOrderTaus.length; firstIndex += 1) {
        for (let secondIndex = firstIndex; secondIndex < secondOrderTaus.length; secondIndex += 1) {
          const tau1 = secondOrderTaus[firstIndex];
          const tau2 = secondOrderTaus[secondIndex];
          const fitted = optimalGain(kind, estimation, { tau1, tau2, delay });
          if (fitted.error < bestError) {
            bestError = fitted.error;
            best = { gain: fitted.gain, tau1, tau2, ...(delay > 0 ? { delay } : {}) };
          }
        }
      }
    }
  }
  if (!best || !Number.isFinite(best.gain)) return undefined;
  const estimationResiduals = residuals(kind, best, estimation);
  const validationResiduals = residuals(kind, best, validation);
  const estimationError = normalizedError(estimationResiduals, estimation.outputScale);
  const validationError = normalizedError(validationResiduals, validation.outputScale);
  const uncertaintyFraction = Math.min(0.5, Math.max(0.02, 2 * validationError));
  const timeFraction = Math.min(0.6, Math.max(0.08, 3 * validationError));
  return {
    kind,
    gain: best.gain,
    ...(best.tau1 === undefined ? {} : { timeConstantSeconds: best.tau1 }),
    ...(best.tau2 === undefined ? {} : { secondTimeConstantSeconds: best.tau2 }),
    ...(best.delay === undefined ? {} : { deadTimeSeconds: best.delay }),
    estimationResiduals,
    validationResiduals,
    normalizedEstimationError: estimationError,
    normalizedValidationError: validationError,
    uncertainty: {
      method: "grid-resolution-and-validation-residual",
      gain: symmetricInterval(best.gain, Math.max(Math.abs(best.gain) * uncertaintyFraction, 1e-12)),
      ...(best.tau1 === undefined ? {} : {
        timeConstantSeconds: positiveInterval(best.tau1, best.tau1 * timeFraction),
      }),
      ...(best.tau2 === undefined ? {} : {
        secondTimeConstantSeconds: positiveInterval(best.tau2, best.tau2 * timeFraction),
      }),
      ...(best.delay === undefined ? {} : {
        deadTimeSeconds: positiveInterval(best.delay, Math.max(estimation.sampleTime, best.delay * timeFraction)),
      }),
    },
  };
}

function optimalGain(kind: LowOrderModelKind, data: StepData, shape: Omit<Parameters, "gain">): { gain: number; error: number } {
  let numerator = 0;
  let denominator = 0;
  for (const sample of data.samples) {
    const basis = data.inputChange * response(kind, sample.timeSeconds - data.stepTime, shape);
    numerator += basis * (sample.output - data.outputBefore);
    denominator += basis * basis;
  }
  if (denominator <= Number.EPSILON) return { gain: Number.NaN, error: Number.POSITIVE_INFINITY };
  const gain = numerator / denominator;
  return { gain, error: normalizedError(residuals(kind, { ...shape, gain }, data), data.outputScale) };
}

function residuals(kind: LowOrderModelKind, parameters: Parameters, data: StepData): number[] {
  return data.samples.map((sample) => sample.output - (
    data.outputBefore + data.inputChange * parameters.gain
      * response(kind, sample.timeSeconds - data.stepTime, parameters)
  ));
}

function response(kind: LowOrderModelKind, elapsed: number, parameters: Omit<Parameters, "gain">): number {
  const time = elapsed - (parameters.delay ?? 0);
  if (time < 0) return 0;
  if (kind === "static_gain") return 1;
  if (kind === "integrating") return time;
  const first = parameters.tau1 ?? 1;
  if (kind === "first_order" || kind === "first_order_dead_time") return 1 - Math.exp(-time / first);
  const second = parameters.tau2 ?? first;
  if (Math.abs(first - second) <= 1e-12) {
    const normalized = time / first;
    return 1 - (1 + normalized) * Math.exp(-normalized);
  }
  return 1 - (first * Math.exp(-time / first) - second * Math.exp(-time / second)) / (first - second);
}

/** Shared held-step response, including the equal-time-constant limit. */
export { response as lowOrderStepResponse };

function prepareStep(
  samples: readonly TimeDomainSample[],
  options: LowOrderIdentificationOptions
): StepData | { status: "rejected"; reason: "nonfinite-input" | "insufficient-data" | "poor-excitation" | "missing-step" } {
  if (samples.length < options.minimumSamples) return { status: "rejected", reason: "insufficient-data" };
  if (!samples.every((sample, index) => sample.quality === "good"
      && Number.isFinite(sample.timeSeconds) && Number.isFinite(sample.input) && Number.isFinite(sample.output)
      && (index === 0 || sample.timeSeconds > samples[index - 1].timeSeconds))) {
    return { status: "rejected", reason: "nonfinite-input" };
  }
  const inputSpan = span(samples.map((sample) => sample.input));
  if (inputSpan < options.minimumInputSpan) return { status: "rejected", reason: "poor-excitation" };
  let stepIndex = -1;
  let stepSize = 0;
  for (let index = 1; index < samples.length; index += 1) {
    const change = Math.abs(samples[index].input - samples[index - 1].input);
    if (change > stepSize) { stepSize = change; stepIndex = index; }
  }
  if (stepIndex < 3 || stepIndex >= samples.length - 10) return { status: "rejected", reason: "missing-step" };
  const before = samples.slice(0, stepIndex);
  const tail = samples.slice(Math.max(stepIndex + 1, samples.length - Math.max(8, Math.floor(samples.length / 10))));
  const inputBefore = mean(before.map((sample) => sample.input));
  const inputAfter = mean(tail.map((sample) => sample.input));
  const outputBefore = mean(before.map((sample) => sample.output));
  const inputChange = inputAfter - inputBefore;
  if (Math.abs(inputChange) < options.minimumInputSpan) return { status: "rejected", reason: "poor-excitation" };
  const intervals = samples.slice(1).map((sample, index) => sample.timeSeconds - samples[index].timeSeconds)
    .sort((left, right) => left - right);
  const sampleTime = intervals[Math.floor(intervals.length / 2)];
  return {
    samples,
    stepIndex,
    stepTime: samples[stepIndex].timeSeconds,
    inputBefore,
    inputAfter,
    outputBefore,
    inputChange,
    outputScale: Math.max(options.absoluteTolerance, span(samples.map((sample) => sample.output))),
    sampleTime,
    horizon: samples[samples.length - 1].timeSeconds - samples[stepIndex].timeSeconds,
  };
}

function validateOptions(options: LowOrderIdentificationOptions): { status: "rejected"; reason: "nonfinite-input" } | undefined {
  return !Number.isFinite(options.absoluteTolerance) || options.absoluteTolerance <= 0
    || !Number.isInteger(options.minimumSamples) || options.minimumSamples < 4
    || !Number.isFinite(options.minimumInputSpan) || options.minimumInputSpan <= 0
    || !Number.isFinite(options.maximumNormalizedValidationError)
    || options.maximumNormalizedValidationError <= 0
    ? { status: "rejected", reason: "nonfinite-input" }
    : undefined;
}

function linearGrid(start: number, end: number, step: number): number[] {
  const values: number[] = [];
  for (let value = start; value <= end + step * 0.25; value += step) values.push(value);
  return values;
}

function logarithmicGrid(minimum: number, maximum: number, count: number): number[] {
  if (maximum <= minimum) return [minimum];
  return Array.from({ length: count }, (_, index) =>
    minimum * Math.pow(maximum / minimum, index / (count - 1)));
}

function normalizedError(residuals: readonly number[], scale: number): number {
  return Math.hypot(...residuals) / Math.sqrt(Math.max(1, residuals.length)) / scale;
}

function symmetricInterval(value: number, halfWidth: number): ParameterInterval {
  return { minimum: value - halfWidth, maximum: value + halfWidth };
}

function positiveInterval(value: number, halfWidth: number): ParameterInterval {
  return { minimum: Math.max(0, value - halfWidth), maximum: value + halfWidth };
}

function span(values: readonly number[]): number { return Math.max(...values) - Math.min(...values); }
function mean(values: readonly number[]): number { return values.reduce((sum, value) => sum + value, 0) / values.length; }
function complexity(kind: LowOrderModelKind): number {
  return ({ static_gain: 1, integrating: 1, first_order: 2, first_order_dead_time: 3,
    second_order: 3, second_order_dead_time: 4 })[kind];
}
