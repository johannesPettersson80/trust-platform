import type {
  ExcitationResult,
  FirstOrderFrequencyFitResult,
  FrequencyResponseEstimate,
  FrequencyResponsePoint,
  MultisineOptions,
  PrbsOptions,
} from "./contracts";

interface Complex { real: number; imaginary: number }

export function generatePrbs(options: PrbsOptions): ExcitationResult {
  if (!Number.isInteger(options.order) || options.order < 2 || options.order > 16
      || !Number.isInteger(options.length) || options.length < 2
      || !Number.isFinite(options.amplitude) || options.amplitude <= 0
      || !Number.isInteger(options.seed)) {
    return { status: "rejected", reason: "invalid-configuration" };
  }
  const mask = (1 << options.order) - 1;
  let state = options.seed & mask;
  if (state === 0) return { status: "rejected", reason: "invalid-configuration" };
  const taps = primitiveTaps(options.order);
  const values: number[] = [];
  for (let index = 0; index < options.length; index += 1) {
    values.push((state & 1) === 1 ? options.amplitude : -options.amplitude);
    let feedback = 0;
    for (const tap of taps) feedback ^= (state >> tap) & 1;
    state = ((state >> 1) | (feedback << (options.order - 1))) & mask;
  }
  return { status: "qualified", values, scale: 1 };
}

export function generateMultisine(options: MultisineOptions): ExcitationResult {
  const vectorsAgree = options.frequenciesRadPerSecond.length > 0
    && options.frequenciesRadPerSecond.length === options.amplitudes.length
    && options.amplitudes.length === options.phasesRadians.length;
  if (!vectorsAgree || !Number.isFinite(options.sampleTimeSeconds) || options.sampleTimeSeconds <= 0
      || !Number.isInteger(options.sampleCount) || options.sampleCount < 8
      || !Number.isFinite(options.maximumAbsoluteValue) || options.maximumAbsoluteValue <= 0
      || ![...options.frequenciesRadPerSecond, ...options.amplitudes, ...options.phasesRadians].every(Number.isFinite)
      || options.frequenciesRadPerSecond.some((value) => value <= 0)
      || options.amplitudes.some((value) => value <= 0)) {
    return { status: "rejected", reason: "invalid-configuration" };
  }
  const nyquist = Math.PI / options.sampleTimeSeconds;
  if (options.frequenciesRadPerSecond.some((value) => value >= nyquist)) {
    return { status: "rejected", reason: "alias-risk" };
  }
  const raw = Array.from({ length: options.sampleCount }, (_, index) => {
    const time = index * options.sampleTimeSeconds;
    return options.frequenciesRadPerSecond.reduce((sum, frequency, tone) =>
      sum + options.amplitudes[tone] * Math.sin(frequency * time + options.phasesRadians[tone]), 0);
  });
  const peak = Math.max(...raw.map(Math.abs));
  const scale = peak > options.maximumAbsoluteValue ? options.maximumAbsoluteValue / peak : 1;
  return { status: "qualified", values: raw.map((value) => value * scale), scale };
}

export function estimateFrequencyResponse(
  input: readonly number[],
  output: readonly number[],
  sampleTimeSeconds: number,
  frequenciesRadPerSecond: readonly number[],
  options: { segmentCount: number; minimumCoherence: number }
): FrequencyResponseEstimate {
  if (input.length !== output.length) return { status: "rejected", reason: "inconsistent-dimensions" };
  if (!Number.isFinite(sampleTimeSeconds) || sampleTimeSeconds <= 0
      || ![...input, ...output, ...frequenciesRadPerSecond, options.minimumCoherence].every(Number.isFinite)) {
    return { status: "rejected", reason: "nonfinite-input" };
  }
  if (!Number.isInteger(options.segmentCount) || options.segmentCount < 2
      || options.minimumCoherence < 0 || options.minimumCoherence > 1
      || frequenciesRadPerSecond.length === 0 || input.length < options.segmentCount * 16) {
    return { status: "rejected", reason: "insufficient-data" };
  }
  if (frequenciesRadPerSecond.some((frequency) => frequency <= 0 || frequency >= Math.PI / sampleTimeSeconds)) {
    return { status: "rejected", reason: "alias-risk" };
  }
  const segmentLength = Math.floor(input.length / options.segmentCount);
  const points: FrequencyResponsePoint[] = [];
  for (const frequency of frequenciesRadPerSecond) {
    let inputPower = 0;
    let outputPower = 0;
    let cross: Complex = { real: 0, imaginary: 0 };
    for (let segment = 0; segment < options.segmentCount; segment += 1) {
      const start = segment * segmentLength;
      const inputCoefficient = coefficient(input, start, segmentLength, sampleTimeSeconds, frequency);
      const outputCoefficient = coefficient(output, start, segmentLength, sampleTimeSeconds, frequency);
      inputPower += magnitudeSquared(inputCoefficient);
      outputPower += magnitudeSquared(outputCoefficient);
      cross = add(cross, multiply(outputCoefficient, conjugate(inputCoefficient)));
    }
    if (inputPower <= Number.EPSILON) return { status: "rejected", reason: "poor-excitation" };
    const transfer = divideByReal(cross, inputPower);
    const coherence = Math.min(1, magnitudeSquared(cross) / Math.max(Number.EPSILON, inputPower * outputPower));
    points.push({
      frequencyRadPerSecond: frequency,
      real: transfer.real,
      imaginary: transfer.imaginary,
      magnitude: Math.hypot(transfer.real, transfer.imaginary),
      phaseRadians: Math.atan2(transfer.imaginary, transfer.real),
      coherence,
    });
  }
  if (points.some((point) => point.coherence < options.minimumCoherence)) {
    return { status: "rejected", reason: "low-coherence" };
  }
  return { status: "qualified", points };
}

export function fitFirstOrderFrequencyResponse(
  estimation: readonly FrequencyResponsePoint[],
  validation: readonly FrequencyResponsePoint[],
  options: { maximumNormalizedValidationError: number }
): FirstOrderFrequencyFitResult {
  if (estimation.length < 2 || validation.length < 1) return { status: "rejected", reason: "insufficient-data" };
  if (!Number.isFinite(options.maximumNormalizedValidationError)
      || options.maximumNormalizedValidationError <= 0
      || ![...estimation, ...validation].every(finiteFrequencyPoint)) {
    return { status: "rejected", reason: "nonfinite-input" };
  }
  if ([...estimation, ...validation].some((point) => point.coherence < 0.8)) {
    return { status: "rejected", reason: "low-coherence" };
  }
  const timeConstants = estimation.map((point) => -Math.tan(point.phaseRadians) / point.frequencyRadPerSecond);
  if (timeConstants.some((value) => !Number.isFinite(value) || value <= 0)) {
    return { status: "rejected", reason: "unstable-or-nonminimum-phase" };
  }
  const timeConstantSeconds = median(timeConstants);
  const gain = median(estimation.map((point) =>
    point.magnitude * Math.hypot(1, timeConstantSeconds * point.frequencyRadPerSecond)));
  const estimationResiduals = estimation.map((point) => complexResidual(point, gain, timeConstantSeconds));
  const validationResiduals = validation.map((point) => complexResidual(point, gain, timeConstantSeconds));
  const scale = Math.max(Number.EPSILON, Math.hypot(...validation.map((point) => point.magnitude))
    / Math.sqrt(validation.length));
  const normalizedValidationError = Math.hypot(...validationResiduals) / Math.sqrt(validation.length) / scale;
  if (normalizedValidationError > options.maximumNormalizedValidationError) {
    return { status: "rejected", reason: "no-qualified-model" };
  }
  return {
    status: "qualified",
    model: { kind: "first_order", gain, timeConstantSeconds },
    estimationResiduals,
    validationResiduals,
    normalizedValidationError,
  };
}

function primitiveTaps(order: number): readonly number[] {
  const known: Record<number, readonly number[]> = {
    2: [0, 1], 3: [0, 2], 4: [0, 3], 5: [1, 4], 6: [0, 5], 7: [0, 6], 8: [1, 2, 3, 7],
    9: [3, 8], 10: [2, 9], 11: [1, 10], 12: [0, 3, 5, 11], 13: [0, 2, 3, 12],
    14: [0, 2, 4, 13], 15: [0, 14], 16: [1, 2, 4, 15],
  };
  return known[order];
}

function coefficient(values: readonly number[], start: number, length: number, dt: number, frequency: number): Complex {
  let real = 0;
  let imaginary = 0;
  for (let offset = 0; offset < length; offset += 1) {
    const angle = frequency * (start + offset) * dt;
    real += values[start + offset] * Math.cos(angle);
    imaginary -= values[start + offset] * Math.sin(angle);
  }
  return { real: 2 * real / length, imaginary: 2 * imaginary / length };
}

function complexResidual(point: FrequencyResponsePoint, gain: number, tau: number): number {
  const denominator = 1 + (tau * point.frequencyRadPerSecond) ** 2;
  const predicted = {
    real: gain / denominator,
    imaginary: -gain * tau * point.frequencyRadPerSecond / denominator,
  };
  return Math.hypot(point.real - predicted.real, point.imaginary - predicted.imaginary);
}

function finiteFrequencyPoint(point: FrequencyResponsePoint): boolean {
  return [point.frequencyRadPerSecond, point.real, point.imaginary, point.magnitude,
    point.phaseRadians, point.coherence].every(Number.isFinite)
    && point.frequencyRadPerSecond > 0 && point.magnitude >= 0;
}

function magnitudeSquared(value: Complex): number { return value.real ** 2 + value.imaginary ** 2; }
function conjugate(value: Complex): Complex { return { real: value.real, imaginary: -value.imaginary }; }
function add(left: Complex, right: Complex): Complex {
  return { real: left.real + right.real, imaginary: left.imaginary + right.imaginary };
}
function multiply(left: Complex, right: Complex): Complex {
  return { real: left.real * right.real - left.imaginary * right.imaginary,
    imaginary: left.real * right.imaginary + left.imaginary * right.real };
}
function divideByReal(value: Complex, denominator: number): Complex {
  return { real: value.real / denominator, imaginary: value.imaginary / denominator };
}
function median(values: readonly number[]): number {
  const sorted = [...values].sort((left, right) => left - right);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 0 ? (sorted[middle - 1] + sorted[middle]) / 2 : sorted[middle];
}
