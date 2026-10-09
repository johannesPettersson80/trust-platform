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

/** Smallest response a fit accepts, as a multiple of the measured noise. */
export const MINIMUM_RESPONSE_TO_NOISE_RATIO = 20;
/** Smallest fraction of its fitted final change a held step must reach to be accepted (A10.3, N2.3). */
export const MINIMUM_REACHED_FRACTION = 0.63;

/** The sign a plant gain must have for the controller action. "direct" (increase the output while the measurement is below the
 * setpoint, heating) needs a positive gain; "reverse" (increase the output while the measurement is above the setpoint, cooling)
 * needs a negative gain. */
export function requiredGainSign(action: "direct" | "reverse"): 1 | -1 {
  return action === "reverse" ? -1 : 1;
}

/** The refusal when a gain's sign contradicts the controller action (B3.9, N6.3); undefined when they agree. */
export function gainSignContradiction(gain: number, unit: string, action: "direct" | "reverse"): string | undefined {
  if (!Number.isFinite(gain) || gain === 0 || Math.sign(gain) === requiredGainSign(action)) return undefined;
  // Engineer-facing text states the direction in plain words, never as the stored action names.
  const response = gain > 0 ? "rises" : "falls";
  const setup = action === "reverse"
    ? "increase the output while the measurement is above the setpoint"
    : "increase the output while the measurement is below the setpoint";
  return `The identified gain is ${engineeringText(gain)}${unit ? ` ${unit}` : ""} (the measurement ${response} when the output rises), but the loop is set up to ${setup}. Check the measured output, the plant input and the controller action in the loop setup.`;
}

/** RMS noise of a steady baseline: root of half the mean squared first difference of its measured outputs. Zero with fewer than three samples. */
export function baselineNoiseRms(outputs: readonly number[]): number {
  if (outputs.length < 3) return 0;
  const differences = outputs.slice(1).map((value, index) => value - outputs[index]);
  return Math.sqrt(differences.reduce((sum, value) => sum + value * value, 0) / differences.length / 2);
}

/** The refusal for a response too small against the measured noise, or undefined when it is large enough or the noise is unknown. */
export function noiseRefusal(responseSize: number, noiseRms: number, unit: string, excitation: "step" | "excitation"): string | undefined {
  if (!(noiseRms > 0) || !(responseSize < MINIMUM_RESPONSE_TO_NOISE_RATIO * noiseRms)) return undefined;
  const suffix = unit ? ` ${unit}` : "";
  return `The response (${engineeringText(responseSize)}${suffix}) is smaller than ${MINIMUM_RESPONSE_TO_NOISE_RATIO} times the measured noise (${engineeringText(noiseRms)}${suffix} RMS). Permit a larger ${excitation} in Limits…, or choose another method.`;
}

/** The refusal for a held step that has not reached the required fraction of its fitted final change (A10.3). */
export function partialResponseRefusal(fraction: number, holdSoFarSeconds: number, delaySeconds: number, timeConstantSeconds: number): string {
  return `The response reached ${Math.round(fraction * 100)} % of its fitted final change in ${minutesText(holdSoFarSeconds)} min; hold the step for about ${minutesText(delaySeconds + 3 * timeConstantSeconds)} min (Limits… → maximum duration).`;
}

/** Fraction of its fitted final change that a held response reached: the measured mean over its last 5 % of samples, less the baseline, over the change (A10.3). */
export function reachedFraction(measured: readonly number[], baseline: number, finalChange: number): number {
  const tail = measured.slice(-Math.max(1, Math.ceil(measured.length * 0.05)));
  return (mean(tail) - baseline) / finalChange;
}

/** The shape of a held-input model whose unit-gain response is simulated on the recorded sample times. */
export type HeldShape =
  | { kind: "static_gain" }
  | { kind: "integrating"; delaySeconds: number }
  | { kind: "first_order"; timeConstantSeconds: number; delaySeconds: number };

/** Unit-gain response to input deviations recorded at `times`, each held until the next sample (A4.3). A dynamic form advances by
 * exact per-interval discretization, a = exp(-dt / tau), and reads its delayed input at t - L from the recorded history: the latest
 * deviation at or before that time, zero before the first sample. A static gain responds at its own sample time. */
export function heldUnitResponse(times: readonly number[], deviations: readonly number[], shape: HeldShape): number[] {
  const delayed = delayedDeviations(times, deviations, shape.kind === "static_gain" ? 0 : shape.delaySeconds);
  if (shape.kind === "static_gain") return delayed;
  const response = new Array<number>(times.length).fill(0);
  for (let k = 1; k < times.length; k += 1) {
    const dt = times[k] - times[k - 1];
    response[k] = shape.kind === "integrating"
      ? response[k - 1] + dt * delayed[k - 1]
      : Math.exp(-dt / shape.timeConstantSeconds) * response[k - 1] + (1 - Math.exp(-dt / shape.timeConstantSeconds)) * delayed[k - 1];
  }
  return response;
}

/** Least-squares gain of a unit response against measured outputs above their baseline (N2.2), with the squared error it leaves. */
export function leastSquaresGain(measured: readonly number[], baseline: number, response: readonly number[]): { gain: number; squaredError: number } {
  let numerator = 0, denominator = 0;
  measured.forEach((value, k) => { numerator += (value - baseline) * response[k]; denominator += response[k] * response[k]; });
  const gain = denominator > Number.EPSILON ? numerator / denominator : Number.NaN;
  return { gain, squaredError: measured.reduce((sum, value, k) => sum + (value - baseline - gain * response[k]) ** 2, 0) };
}

const TIME_TOLERANCE = 1e-9;

function delayedDeviations(times: readonly number[], deviations: readonly number[], delaySeconds: number): number[] {
  const values = new Array<number>(times.length).fill(0);
  let latest = -1;
  for (let k = 0; k < times.length; k += 1) {
    while (latest + 1 < times.length && times[latest + 1] <= times[k] - delaySeconds + TIME_TOLERANCE) latest += 1;
    values[k] = latest >= 0 ? deviations[latest] : 0;
  }
  return values;
}

function engineeringText(value: number): string {
  return Number(value.toPrecision(3)).toString().replace("-", "−");
}

function minutesText(seconds: number): string {
  return String(Number((seconds / 60).toPrecision(2)));
}
function complexity(kind: LowOrderModelKind): number {
  return ({ static_gain: 1, integrating: 1, first_order: 2, first_order_dead_time: 3,
    second_order: 3, second_order_dead_time: 4 })[kind];
}
