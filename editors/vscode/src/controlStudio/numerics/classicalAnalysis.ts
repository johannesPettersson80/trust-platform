import { eigenvalues } from "./adapter";
import type { ComplexValue, NumericalTolerance } from "./contracts";

export interface TransferFunction {
  numerator: readonly number[];
  denominator: readonly number[];
  delaySeconds: number;
}

export interface FrequencySweep {
  minimumFrequencyRadPerSecond: number;
  maximumFrequencyRadPerSecond: number;
  pointsPerDecade: number;
}

interface Complex { real: number; imaginary: number }
type Rejected = { status: "rejected"; reason:
  | "nonfinite-input"
  | "invalid-polynomial"
  | "solver-failed"
  | "pole-on-contour"
  | "contour-near-critical-point"
  | "insufficient-contour-closure"
  | "ambiguous-crossover" };

const ROOT_TOLERANCE: NumericalTolerance = { absolute: 1e-12, relative: 1e-10, conditionLimit: 1e12 };

export function analyzeTransferFunction(tf: TransferFunction, frequencies: readonly number[]): any | Rejected {
  const invalid = validateTransferFunction(tf);
  if (invalid) return invalid;
  if (frequencies.length === 0 || !frequencies.every((value) => Number.isFinite(value) && value > 0)) {
    return rejected("nonfinite-input");
  }
  const poles = rootsOfRealPolynomial(tf.denominator);
  const zeros = tf.numerator.length <= 1 ? [] : rootsOfRealPolynomial(tf.numerator);
  if (!poles || !zeros) return rejected("solver-failed");
  const bode = frequencies.map((frequencyRadPerSecond) => {
    const loop = transferAt(tf, frequencyRadPerSecond);
    const sensitivity = divide({ real: 1, imaginary: 0 }, add({ real: 1, imaginary: 0 }, loop));
    const complementarySensitivity = multiply(loop, sensitivity);
    const complementarity = add(sensitivity, complementarySensitivity);
    return {
      frequencyRadPerSecond,
      magnitudeDb: 20 * Math.log10(Math.hypot(loop.real, loop.imaginary)),
      phaseDegrees: Math.atan2(loop.imaginary, loop.real) * 180 / Math.PI,
      sensitivity,
      complementarySensitivity,
      sensitivityComplementarityResidual: Math.hypot(complementarity.real - 1, complementarity.imaginary),
    };
  });
  return { status: "qualified", poles, zeros, bode };
}

export function classicalStabilityMargins(
  tf: TransferFunction,
  sweep: { minimumFrequencyRadPerSecond: number; maximumFrequencyRadPerSecond: number; pointsPerDecade: number }
): any | Rejected {
  const frequencies = logarithmicFrequencies(sweep);
  if (!frequencies) return rejected("nonfinite-input");
  const values = frequencies.map((frequency) => ({ frequency, value: transferAt(tf, frequency) }));
  const phases = unwrap(values.map(({ value }) => Math.atan2(value.imaginary, value.real) * 180 / Math.PI));
  const magnitudes = values.map(({ value }) => 20 * Math.log10(Math.hypot(value.real, value.imaginary)));
  const gainCross = crossing(frequencies, magnitudes, 0);
  if (!gainCross) return rejected("solver-failed");
  const phaseAtGainCross = interpolateSeries(frequencies, phases, gainCross);
  const phaseMarginDegrees = 180 + phaseAtGainCross;
  const phaseCross = crossing(frequencies, phases, -180);
  const gainMarginDb = phaseCross === undefined
    ? "unbounded"
    : -interpolateSeries(frequencies, magnitudes, phaseCross);
  return {
    status: "qualified",
    gainCrossoverRadPerSecond: gainCross,
    phaseCrossoverRadPerSecond: phaseCross,
    gainMarginDb,
    phaseMarginDegrees,
    delayMarginSeconds: phaseMarginDegrees > 0 ? phaseMarginDegrees * Math.PI / 180 / gainCross : 0,
  };
}

/**
 * Evaluates the physically distinct closed-loop paths on one common grid.
 * Keeping G and C separate avoids treating load and sensor noise as though
 * they passed through the same transfer function.
 */
export function analyzeLoopShape(
  plant: TransferFunction,
  controller: TransferFunction,
  sweep: FrequencySweep
): any | Rejected {
  const invalid = validateTransferFunction(plant) ?? validateTransferFunction(controller);
  const frequencies = logarithmicFrequencies(sweep);
  if (invalid) return invalid;
  if (!frequencies) return rejected("nonfinite-input");
  if (plant.numerator.length > plant.denominator.length
      || controller.numerator.length > controller.denominator.length) {
    return rejected("invalid-polynomial");
  }

  const points = frequencies.map((frequencyRadPerSecond) => {
    const plantResponse = transferAt(plant, frequencyRadPerSecond);
    const controllerResponse = transferAt(controller, frequencyRadPerSecond);
    const loop = multiply(plantResponse, controllerResponse);
    const sensitivity = divide({ real: 1, imaginary: 0 }, add({ real: 1, imaginary: 0 }, loop));
    const complementarySensitivity = multiply(loop, sensitivity);
    const inputDisturbance = multiply(plantResponse, sensitivity);
    const controlSensitivity = multiply(controllerResponse, sensitivity);
    const complementarity = add(sensitivity, complementarySensitivity);
    return {
      frequencyRadPerSecond,
      loopMagnitude: magnitude(loop),
      loopPhaseDegrees: Math.atan2(loop.imaginary, loop.real) * 180 / Math.PI,
      sensitivityMagnitude: magnitude(sensitivity),
      complementarySensitivityMagnitude: magnitude(complementarySensitivity),
      inputDisturbanceMagnitude: magnitude(inputDisturbance),
      controlSensitivityMagnitude: magnitude(controlSensitivity),
      complementarityResidual: magnitude({
        real: complementarity.real - 1,
        imaginary: complementarity.imaginary,
      }),
    };
  });
  if (!points.every((point) => Object.values(point).every(Number.isFinite))) {
    return rejected("nonfinite-input");
  }

  const loopMagnitudeDb = points.map((point) => 20 * Math.log10(point.loopMagnitude));
  const gainCrossovers = crossings(frequencies, loopMagnitudeDb, 0);
  if (gainCrossovers.length !== 1) {
    return rejected(gainCrossovers.length === 0 ? "solver-failed" : "ambiguous-crossover");
  }
  const unwrappedPhase = unwrap(points.map((point) => point.loopPhaseDegrees));
  const gainCrossoverRadPerSecond = gainCrossovers[0];
  const phaseMarginDegrees = 180
    + interpolateSeries(frequencies, unwrappedPhase, gainCrossoverRadPerSecond);
  const lowFrequencyReference = points[0].complementarySensitivityMagnitude;
  const bandwidthRadPerSecond = crossing(
    frequencies,
    points.map((point) => point.complementarySensitivityMagnitude),
    lowFrequencyReference / Math.sqrt(2)
  );
  if (bandwidthRadPerSecond === undefined) return rejected("solver-failed");

  return {
    status: "qualified",
    points,
    gainCrossoversRadPerSecond: gainCrossovers,
    gainCrossoverRadPerSecond,
    phaseMarginDegrees,
    bandwidthRadPerSecond,
    maximumSensitivity: Math.max(...points.map((point) => point.sensitivityMagnitude)),
    maximumComplementarySensitivity: Math.max(
      ...points.map((point) => point.complementarySensitivityMagnitude)
    ),
    maximumComplementarityResidual: Math.max(...points.map((point) => point.complementarityResidual)),
  };
}

export function routhHurwitz(coefficients: readonly number[]): any | Rejected {
  const normalized = normalizePolynomial(coefficients);
  if (!normalized || normalized.length < 2) return rejected("invalid-polynomial");
  const degree = normalized.length - 1;
  const columns = Math.ceil((degree + 1) / 2);
  const table = Array.from({ length: degree + 1 }, () => Array(columns).fill(0) as number[]);
  for (let column = 0; column < columns; column += 1) {
    table[0][column] = normalized[column * 2] ?? 0;
    table[1][column] = normalized[column * 2 + 1] ?? 0;
  }
  const epsilon = 1e-12;
  for (let row = 2; row <= degree; row += 1) {
    if (Math.abs(table[row - 1][0]) < epsilon) table[row - 1][0] = epsilon;
    for (let column = 0; column < columns - 1; column += 1) {
      table[row][column] = (table[row - 1][0] * table[row - 2][column + 1]
        - table[row - 2][0] * table[row - 1][column + 1]) / table[row - 1][0];
    }
  }
  const firstColumn = table.map((row) => row[0]);
  let rightHalfPlaneRoots = 0;
  let previousSign = Math.sign(firstColumn[0]);
  for (const value of firstColumn.slice(1)) {
    const sign = Math.sign(value) || previousSign;
    if (sign !== previousSign) rightHalfPlaneRoots += 1;
    previousSign = sign;
  }
  return { status: "qualified", table, firstColumn, rightHalfPlaneRoots, stable: rightHalfPlaneRoots === 0 };
}

export function rootLocus(tf: TransferFunction, gains: readonly number[]): any | Rejected {
  // A zero numerator is the valid open-loop starting point for a gain sweep.
  // The general transfer-function validator rejects an identically zero
  // numerator because it cannot be normalized; root-locus only needs the
  // denominator and finite numerator coefficients to form the characteristic
  // polynomial.
  const invalid = validateRootLocusTransferFunction(tf);
  if (invalid) return invalid;
  if (tf.delaySeconds !== 0 || gains.length === 0 || !gains.every((value) => Number.isFinite(value) && value >= 0)) {
    return rejected("nonfinite-input");
  }
  const branches = gains.map((gain) => {
    const polynomial = polynomialAdd(tf.denominator, tf.numerator.map((value) => value * gain));
    const poles = rootsOfRealPolynomial(polynomial);
    if (!poles) throw new Error("root solver failed");
    return { gain, poles, closedLoopPoles: poles.map((pole) => ({ ...pole })) };
  });
  return { status: "qualified", branches };
}

export function designLeadAtCrossover(
  plant: TransferFunction,
  crossoverRadPerSecond: number,
  phaseContributionDegrees: number
): any | Rejected {
  const invalid = validateTransferFunction(plant);
  if (invalid) return invalid;
  if (!Number.isFinite(crossoverRadPerSecond) || crossoverRadPerSecond <= 0
      || !Number.isFinite(phaseContributionDegrees) || phaseContributionDegrees <= 0
      || phaseContributionDegrees >= 85) return rejected("nonfinite-input");
  const sine = Math.sin(phaseContributionDegrees * Math.PI / 180);
  const alpha = (1 - sine) / (1 + sine);
  const timeConstant = 1 / (crossoverRadPerSecond * Math.sqrt(alpha));
  const zeroRadPerSecond = 1 / timeConstant;
  const poleRadPerSecond = 1 / (alpha * timeConstant);
  const plantValue = transferAt(plant, crossoverRadPerSecond);
  const leadWithoutGain = divide(
    { real: 1, imaginary: crossoverRadPerSecond * timeConstant },
    { real: 1, imaginary: crossoverRadPerSecond * alpha * timeConstant }
  );
  const gain = 1 / (Math.hypot(plantValue.real, plantValue.imaginary)
    * Math.hypot(leadWithoutGain.real, leadWithoutGain.imaginary));
  return {
    status: "qualified",
    gain,
    zeroRadPerSecond,
    poleRadPerSecond,
    phaseContributionDegrees: Math.atan2(leadWithoutGain.imaginary, leadWithoutGain.real) * 180 / Math.PI,
    openLoopMagnitudeAtCrossover: gain * Math.hypot(plantValue.real, plantValue.imaginary)
      * Math.hypot(leadWithoutGain.real, leadWithoutGain.imaginary),
  };
}

export function designLagAtCrossover(
  plant: TransferFunction,
  crossoverRadPerSecond: number,
  lowFrequencyGainRatio: number
): any | Rejected {
  const invalid = validateTransferFunction(plant);
  if (invalid) return invalid;
  if (!Number.isFinite(crossoverRadPerSecond) || crossoverRadPerSecond <= 0
      || !Number.isFinite(lowFrequencyGainRatio) || lowFrequencyGainRatio <= 1) {
    return rejected("nonfinite-input");
  }
  // Keep both break points below crossover so the requested crossover changes
  // little while the low-frequency loop gain increases by the declared ratio.
  const zeroRadPerSecond = crossoverRadPerSecond / 10;
  const poleRadPerSecond = zeroRadPerSecond / lowFrequencyGainRatio;
  const plantValue = transferAt(plant, crossoverRadPerSecond);
  const lagWithoutGain = divide(
    { real: 1, imaginary: crossoverRadPerSecond / zeroRadPerSecond },
    { real: 1, imaginary: crossoverRadPerSecond / poleRadPerSecond }
  );
  const gain = 1 / (Math.hypot(plantValue.real, plantValue.imaginary)
    * Math.hypot(lagWithoutGain.real, lagWithoutGain.imaginary));
  return {
    status: "qualified",
    gain,
    zeroRadPerSecond,
    poleRadPerSecond,
    lowFrequencyGainRatio,
    phaseContributionDegrees: Math.atan2(lagWithoutGain.imaginary, lagWithoutGain.real) * 180 / Math.PI,
    openLoopMagnitudeAtCrossover: gain * Math.hypot(plantValue.real, plantValue.imaginary)
      * Math.hypot(lagWithoutGain.real, lagWithoutGain.imaginary),
  };
}

export function nyquistStability(
  tf: TransferFunction,
  sweep: { minimumFrequencyRadPerSecond: number; maximumFrequencyRadPerSecond: number; pointsPerDecade: number }
): any | Rejected {
  const frequencies = logarithmicFrequencies(sweep);
  const invalid = validateTransferFunction(tf);
  if (!frequencies || invalid) return invalid ?? rejected("nonfinite-input");
  const openLoopPoles = rootsOfRealPolynomial(tf.denominator);
  if (!openLoopPoles) return rejected("solver-failed");
  if (openLoopPoles.some((pole) => Math.abs(pole.real) <= 1e-9)) {
    return rejected("pole-on-contour");
  }
  const positive = frequencies.map((frequency) => transferAt(tf, frequency));
  const endpointMagnitude = Math.hypot(
    positive[positive.length - 1].real,
    positive[positive.length - 1].imaginary
  );
  if (endpointMagnitude > 0.02) return rejected("insufficient-contour-closure");
  const zeroFrequency = transferAt(tf, 0);
  const contour = [...positive.slice().reverse().map(conjugate), zeroFrequency, ...positive];
  const minimumDistanceToMinusOne = Math.min(...contour.map((value) =>
    Math.hypot(value.real + 1, value.imaginary)));
  if (minimumDistanceToMinusOne < 0.005) return rejected("contour-near-critical-point");
  const shiftedAngles = unwrapRadians(contour.map((value) => Math.atan2(value.imaginary, value.real + 1)));
  const roundedEncirclements = Math.round(
    (shiftedAngles[shiftedAngles.length - 1] - shiftedAngles[0]) / (2 * Math.PI)
  );
  const encirclementsCounterClockwise = Object.is(roundedEncirclements, -0) ? 0 : roundedEncirclements;
  const openLoopRightHalfPlanePoles = openLoopPoles.filter((pole) => pole.real > 1e-9).length;
  // With this negative-feedback traversal, counter-clockwise encirclements
  // subtract from the open-loop right-half-plane pole count.
  const closedLoopRightHalfPlanePoles = openLoopRightHalfPlanePoles - encirclementsCounterClockwise;
  return {
    status: "qualified",
    contourTraversal: "negative-high-to-positive-high",
    encirclementsCounterClockwise,
    openLoopRightHalfPlanePoles,
    closedLoopRightHalfPlanePoles,
    closedLoopStable: closedLoopRightHalfPlanePoles === 0,
    minimumDistanceToMinusOne,
    endpointMagnitude,
    contour,
  };
}

export function rootsOfRealPolynomial(coefficients: readonly number[]): ComplexValue[] | undefined {
  const normalized = normalizePolynomial(coefficients);
  if (!normalized) return undefined;
  const degree = normalized.length - 1;
  if (degree === 0) return [];
  const companion = Array.from({ length: degree }, () => Array(degree).fill(0) as number[]);
  for (let column = 0; column < degree; column += 1) companion[0][column] = -normalized[column + 1];
  for (let row = 1; row < degree; row += 1) companion[row][row - 1] = 1;
  const result = eigenvalues(companion, ROOT_TOLERANCE);
  return result.status === "qualified" ? [...result.values] : undefined;
}

function normalizePolynomial(coefficients: readonly number[]): number[] | undefined {
  if (coefficients.length === 0 || !coefficients.every(Number.isFinite)) return undefined;
  const first = coefficients.findIndex((value) => Math.abs(value) > 1e-15);
  if (first < 0) return undefined;
  const trimmed = coefficients.slice(first);
  return trimmed.map((value) => value / trimmed[0]);
}

function polynomialAdd(left: readonly number[], right: readonly number[]): number[] {
  const length = Math.max(left.length, right.length);
  const paddedLeft = [...Array(length - left.length).fill(0), ...left];
  const paddedRight = [...Array(length - right.length).fill(0), ...right];
  return paddedLeft.map((value, index) => value + paddedRight[index]);
}

function crossings(x: readonly number[], y: readonly number[], target: number): number[] {
  const result: number[] = [];
  for (let index = 1; index < y.length; index += 1) {
    const before = y[index - 1] - target;
    const after = y[index] - target;
    if (before === 0) result.push(x[index - 1]);
    else if (before * after < 0) {
      const fraction = Math.abs(before) / (Math.abs(before) + Math.abs(after));
      result.push(Math.exp(Math.log(x[index - 1])
        + fraction * (Math.log(x[index]) - Math.log(x[index - 1]))));
    }
  }
  return result;
}

function transferAt(tf: TransferFunction, frequency: number): Complex {
  const s = { real: 0, imaginary: frequency };
  let value = divide(evaluate(tf.numerator, s), evaluate(tf.denominator, s));
  if (tf.delaySeconds > 0) {
    const angle = -frequency * tf.delaySeconds;
    value = multiply(value, { real: Math.cos(angle), imaginary: Math.sin(angle) });
  }
  return value;
}

function evaluate(coefficients: readonly number[], value: Complex): Complex {
  return coefficients.reduce((result, coefficient) =>
    add(multiply(result, value), { real: coefficient, imaginary: 0 }), { real: 0, imaginary: 0 });
}

function validateTransferFunction(tf: TransferFunction): Rejected | undefined {
  return !Number.isFinite(tf.delaySeconds) || tf.delaySeconds < 0
    || !normalizePolynomial(tf.denominator) || !normalizePolynomial(tf.numerator)
    ? rejected("invalid-polynomial") : undefined;
}

function validateRootLocusTransferFunction(tf: TransferFunction): Rejected | undefined {
  return !Number.isFinite(tf.delaySeconds) || tf.delaySeconds < 0
    || !normalizePolynomial(tf.denominator) || tf.numerator.length === 0
    || !tf.numerator.every(Number.isFinite)
    ? rejected("invalid-polynomial") : undefined;
}

function logarithmicFrequencies(sweep: {
  minimumFrequencyRadPerSecond: number; maximumFrequencyRadPerSecond: number; pointsPerDecade: number;
}): number[] | undefined {
  if (!Number.isFinite(sweep.minimumFrequencyRadPerSecond) || sweep.minimumFrequencyRadPerSecond <= 0
      || !Number.isFinite(sweep.maximumFrequencyRadPerSecond)
      || sweep.maximumFrequencyRadPerSecond <= sweep.minimumFrequencyRadPerSecond
      || !Number.isInteger(sweep.pointsPerDecade) || sweep.pointsPerDecade < 10) return undefined;
  const count = Math.ceil(Math.log10(sweep.maximumFrequencyRadPerSecond / sweep.minimumFrequencyRadPerSecond)
    * sweep.pointsPerDecade) + 1;
  return Array.from({ length: count }, (_, index) => sweep.minimumFrequencyRadPerSecond
    * Math.pow(sweep.maximumFrequencyRadPerSecond / sweep.minimumFrequencyRadPerSecond, index / (count - 1)));
}

function crossing(x: readonly number[], y: readonly number[], target: number): number | undefined {
  for (let index = 1; index < y.length; index += 1) {
    const before = y[index - 1] - target;
    const after = y[index] - target;
    if (before === 0) return x[index - 1];
    if (before * after <= 0) {
      const fraction = Math.abs(before) / (Math.abs(before) + Math.abs(after));
      // Interpolate log frequency because the sweep is logarithmic.
      return Math.exp(Math.log(x[index - 1])
        + fraction * (Math.log(x[index]) - Math.log(x[index - 1])));
    }
  }
  return undefined;
}

function interpolateSeries(x: readonly number[], y: readonly number[], value: number): number {
  const upper = x.findIndex((candidate) => candidate >= value);
  if (upper <= 0) return y[Math.max(0, upper)];
  const fraction = (Math.log(value) - Math.log(x[upper - 1])) / (Math.log(x[upper]) - Math.log(x[upper - 1]));
  return y[upper - 1] + fraction * (y[upper] - y[upper - 1]);
}

function unwrap(values: readonly number[]): number[] {
  const result: number[] = [];
  for (const value of values) {
    let current = value;
    const previous = result[result.length - 1];
    if (previous !== undefined) {
      while (current - previous > 180) current -= 360;
      while (current - previous < -180) current += 360;
    }
    result.push(current);
  }
  return result;
}

function unwrapRadians(values: readonly number[]): number[] {
  const result: number[] = [];
  for (const value of values) {
    let current = value;
    const previous = result[result.length - 1];
    if (previous !== undefined) {
      while (current - previous > Math.PI) current -= 2 * Math.PI;
      while (current - previous < -Math.PI) current += 2 * Math.PI;
    }
    result.push(current);
  }
  return result;
}

function rejected(reason: Rejected["reason"]): Rejected { return { status: "rejected", reason }; }
function magnitude(value: Complex): number { return Math.hypot(value.real, value.imaginary); }
function conjugate(value: Complex): Complex { return { real: value.real, imaginary: -value.imaginary }; }
function add(left: Complex, right: Complex): Complex {
  return { real: left.real + right.real, imaginary: left.imaginary + right.imaginary };
}
function multiply(left: Complex, right: Complex): Complex {
  return { real: left.real * right.real - left.imaginary * right.imaginary,
    imaginary: left.real * right.imaginary + left.imaginary * right.real };
}
function divide(left: Complex, right: Complex): Complex {
  const denominator = right.real ** 2 + right.imaginary ** 2;
  return { real: (left.real * right.real + left.imaginary * right.imaginary) / denominator,
    imaginary: (left.imaginary * right.real - left.real * right.imaginary) / denominator };
}
