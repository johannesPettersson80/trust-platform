import { updateControlPid, type ControlPidCandidate, type ControlPidState } from "./controllerCandidate";
import type { IdentifiedModel } from "./identification";
import type { FrequencyPoint } from "./tuning";
import { rootLocus } from "./numerics";

export interface EngineeringTimePoint {
  timeSeconds: number;
  currentPv: number;
  candidatePv: number;
  currentMv: number;
  candidateMv: number;
}

export interface EngineeringPlotEvidence {
  evidence: "Model-predicted";
  timeResponse: readonly EngineeringTimePoint[];
  plantBode: readonly FrequencyPoint[];
  candidateOpenLoopBode: readonly FrequencyPoint[];
  gainMarginDb: number;
  phaseMarginDegrees: number;
  nichols: readonly { frequencyRadPerSecond: number; phaseDegrees: number; magnitudeDb: number }[];
  nyquist: readonly { frequencyRadPerSecond: number; real: number; imaginary: number }[];
  sensitivity: readonly {
    frequencyRadPerSecond: number;
    sensitivityMagnitudeDb: number;
    complementarySensitivityMagnitudeDb: number;
    controlSensitivityMagnitudeDb: number;
  }[];
  rootLocus: { status: "qualified"; branches: readonly {
    gain: number; poles: readonly { real: number; imaginary: number }[];
  }[] } | { status: "unavailable"; reason: string; branches: readonly [] };
  requirements: readonly {
    name: string;
    plot: "response" | "bode" | "sensitivity" | "constraints";
    status: "Pass" | "Fail" | "Inconclusive";
    value: number | "unbounded";
    limit: number;
    unit: string;
    direction: "maximum" | "minimum";
  }[];
  uncertainty: {
    status: "evaluated" | "not-evaluated";
    method: string;
    points: readonly { timeSeconds: number; lower: number; nominal: number; upper: number }[];
  };
}

export type EngineeringPlotMessageEvidence = Omit<EngineeringPlotEvidence, "gainMarginDb"> & {
  gainMarginDb: number | "unbounded";
};

/** Converts mathematical infinity to an explicit value that survives VS Code message serialization. */
export function engineeringPlotMessageEvidence(evidence: EngineeringPlotEvidence): EngineeringPlotMessageEvidence {
  return { ...evidence,
    gainMarginDb: evidence.gainMarginDb === Number.POSITIVE_INFINITY ? "unbounded" : evidence.gainMarginDb };
}

export function buildEngineeringPlotEvidence(
  model: IdentifiedModel,
  current: ControlPidCandidate,
  candidate: ControlPidCandidate
): EngineeringPlotEvidence {
  validateModel(model);
  const horizon = Math.max(10, 10 * ((model.timeConstantSeconds ?? 1)
    + (model.secondTimeConstantSeconds ?? 0) + (model.deadTimeSeconds ?? 0)));
  const steps = Math.min(1_000, Math.max(200, Math.ceil(horizon / 0.05)));
  const cycleTime = horizon / steps;
  const currentSimulation = simulationState(model, current, cycleTime);
  const candidateSimulation = simulationState(model, candidate, cycleTime);
  const timeResponse: EngineeringTimePoint[] = [];
  for (let step = 0; step <= steps; step += 1) {
    const currentPoint = advance(currentSimulation, model, current, cycleTime);
    const candidatePoint = advance(candidateSimulation, model, candidate, cycleTime);
    timeResponse.push({
      timeSeconds: step * cycleTime,
      currentPv: currentPoint.pv,
      candidatePv: candidatePoint.pv,
      currentMv: currentPoint.mv,
      candidateMv: candidatePoint.mv,
    });
  }

  const plantBode: FrequencyPoint[] = [];
  const candidateOpenLoopBode: FrequencyPoint[] = [];
  const baseTime = Math.max(1e-6, model.timeConstantSeconds ?? model.secondTimeConstantSeconds ?? 1);
  const delay = model.deadTimeSeconds ?? 0;
  // Show the frequencies that explain the controller decision. Extending a
  // dead-time model through many rotations hides crossover and margins in an
  // otherwise technically valid but unusable plot.
  const minimumFrequency = 1e-2 / baseTime;
  const maximumFrequency = Math.max(10 / baseTime, delay > 0 ? 2 * Math.PI / delay : 0);
  const decades = Math.log10(maximumFrequency / minimumFrequency);
  const frequencyPoints = Math.min(2_000, Math.max(100, Math.ceil(decades * 40) + 1));
  for (let index = 0; index < frequencyPoints; index += 1) {
    const frequency = minimumFrequency * Math.pow(maximumFrequency / minimumFrequency, index / (frequencyPoints - 1));
    const plant = plantFrequency(model, frequency);
    const controller = controllerFrequency(candidate, frequency);
    const openLoop = multiply(plant, controller);
    plantBode.push(point(frequency, plant));
    candidateOpenLoopBode.push(point(frequency, openLoop));
  }
  const unwrappedPlant = unwrapPhase(plantBode);
  const unwrappedOpenLoop = unwrapPhase(candidateOpenLoopBode);
  const margins = stabilityMargins(unwrappedOpenLoop, delay);
  const nichols = unwrappedOpenLoop.map((item) => ({ ...item }));
  const positiveNyquist = unwrappedOpenLoop.map(frequencyPointToComplex);
  const nyquist = [
    ...positiveNyquist.slice().reverse().map((point) => ({ ...point, imaginary: -point.imaginary })),
    ...positiveNyquist.slice(1),
  ];
  const sensitivity = unwrappedOpenLoop.map((item) => sensitivityPoint(item, candidate));
  const locus = buildRootLocus(model, candidate);
  const uncertainty = buildUncertaintyEnvelope(model, candidate, timeResponse, cycleTime);
  const requirements = graphicalRequirements(timeResponse, margins, sensitivity);
  return { evidence: "Model-predicted", timeResponse, plantBode: unwrappedPlant,
    candidateOpenLoopBode: unwrappedOpenLoop, nichols, nyquist, sensitivity,
    rootLocus: locus, requirements, uncertainty, ...margins };
}

interface Complex { re: number; im: number }
interface SimulationState { pid: ControlPidState; pv: number; delay: number[]; lag2: number }

function simulationState(model: IdentifiedModel, candidate: ControlPidCandidate, cycleTime: number): SimulationState {
  const delaySamples = Math.max(0, Math.round((model.deadTimeSeconds ?? 0) / cycleTime));
  return {
    pid: { setpoint: 0, setpointRate: 0, processValue: 0, processValueRate: 0,
      feedForward: 0, nominalOutput: 0, saturatedOutput: Math.max(0, candidate.parameters.outputMinimum) },
    pv: 0,
    delay: Array(delaySamples + 1).fill(0) as number[],
    lag2: 0,
  };
}

function advance(state: SimulationState, model: IdentifiedModel, candidate: ControlPidCandidate, cycleTime: number) {
  const updated = updateControlPid(candidate.parameters, state.pid, {
    setpoint: 1, processValue: state.pv, feedForward: 0, manualOutput: 0,
    trackingOutput: 0, mode: "Auto", cycleTime,
  });
  state.pid = updated.state;
  state.delay.push(updated.output);
  const delayed = state.delay.shift() ?? 0;
  if (model.kind === "integrating") {
    state.pv += cycleTime * model.gain * delayed;
  } else {
    const firstTime = model.timeConstantSeconds ?? 1;
    const firstTarget = model.gain * delayed;
    if (model.secondTimeConstantSeconds !== undefined) {
      state.lag2 += cycleTime * (firstTarget - state.lag2) / firstTime;
      state.pv += cycleTime * (state.lag2 - state.pv) / model.secondTimeConstantSeconds;
    } else {
      state.pv += cycleTime * (firstTarget - state.pv) / firstTime;
    }
  }
  if (![state.pv, updated.output].every(Number.isFinite)) throw new Error("Controller prediction became non-finite.");
  return { pv: state.pv, mv: updated.output };
}

function plantFrequency(model: IdentifiedModel, frequency: number): Complex {
  let value: Complex = { re: model.gain, im: 0 };
  if (model.kind === "integrating") value = multiply(value, { re: 0, im: -1 / frequency });
  for (const time of [model.timeConstantSeconds, model.secondTimeConstantSeconds]) {
    if (time !== undefined) value = divide(value, { re: 1, im: frequency * time });
  }
  const angle = -frequency * (model.deadTimeSeconds ?? 0);
  return multiply(value, { re: Math.cos(angle), im: Math.sin(angle) });
}

function controllerFrequency(candidate: ControlPidCandidate, frequency: number): Complex {
  const parameters = candidate.parameters;
  return {
    re: parameters.proportionalGain,
    im: parameters.derivativeGain * frequency - parameters.integralGain / frequency,
  };
}

function point(frequencyRadPerSecond: number, value: Complex): FrequencyPoint {
  return {
    frequencyRadPerSecond,
    magnitudeDb: 20 * Math.log10(Math.hypot(value.re, value.im)),
    phaseDegrees: Math.atan2(value.im, value.re) * 180 / Math.PI,
  };
}

function stabilityMargins(points: readonly FrequencyPoint[], deadTimeSeconds: number): { gainMarginDb: number; phaseMarginDegrees: number } {
  const unwrapped = unwrapPhase(points);
  const gainCross = crossing(unwrapped, (point) => point.magnitudeDb);
  const phaseCross = crossing(unwrapped, (point) => point.phaseDegrees + 180);
  if (!gainCross) throw new Error("The sampled frequency range does not resolve the gain crossover.");
  if (!phaseCross && deadTimeSeconds > 0) {
    throw new Error("The sampled frequency range does not resolve the phase crossover.");
  }
  return {
    gainMarginDb: phaseCross ? -phaseCross.magnitudeDb : Number.POSITIVE_INFINITY,
    phaseMarginDegrees: 180 + gainCross.phaseDegrees,
  };
}

function unwrapPhase(points: readonly FrequencyPoint[]): FrequencyPoint[] {
  let previous = points[0]?.phaseDegrees ?? 0;
  return points.map((point, index) => {
    let phase = point.phaseDegrees;
    if (index > 0) {
      while (phase - previous > 180) phase -= 360;
      while (phase - previous < -180) phase += 360;
    }
    previous = phase;
    return { ...point, phaseDegrees: phase };
  });
}

function crossing(points: readonly FrequencyPoint[], value: (point: FrequencyPoint) => number): FrequencyPoint | undefined {
  for (let index = 1; index < points.length; index += 1) {
    const before = value(points[index - 1]);
    const after = value(points[index]);
    if (before === 0) return points[index - 1];
    if (before * after <= 0) {
      const fraction = Math.abs(before) / (Math.abs(before) + Math.abs(after));
      return {
        frequencyRadPerSecond: interpolate(points[index - 1].frequencyRadPerSecond, points[index].frequencyRadPerSecond, fraction),
        magnitudeDb: interpolate(points[index - 1].magnitudeDb, points[index].magnitudeDb, fraction),
        phaseDegrees: interpolate(points[index - 1].phaseDegrees, points[index].phaseDegrees, fraction),
      };
    }
  }
  return undefined;
}

function validateModel(model: IdentifiedModel): void {
  if (!model.stable || !Number.isFinite(model.gain) || model.gain === 0) {
    throw new Error("Engineering plots require a valid stable plant model.");
  }
  for (const time of [model.timeConstantSeconds, model.secondTimeConstantSeconds]) {
    if (time !== undefined && (!Number.isFinite(time) || time <= 0)) {
      throw new Error("Engineering plots require finite positive time constants.");
    }
  }
}

function multiply(left: Complex, right: Complex): Complex {
  return { re: left.re * right.re - left.im * right.im, im: left.re * right.im + left.im * right.re };
}
function divide(left: Complex, right: Complex): Complex {
  const denominator = right.re ** 2 + right.im ** 2;
  return { re: (left.re * right.re + left.im * right.im) / denominator,
    im: (left.im * right.re - left.re * right.im) / denominator };
}
function interpolate(start: number, end: number, fraction: number): number {
  return start + (end - start) * fraction;
}

function frequencyPointToComplex(point: FrequencyPoint): {
  frequencyRadPerSecond: number; real: number; imaginary: number;
} {
  const magnitude = 10 ** (point.magnitudeDb / 20);
  const phase = point.phaseDegrees * Math.PI / 180;
  return { frequencyRadPerSecond: point.frequencyRadPerSecond,
    real: magnitude * Math.cos(phase), imaginary: magnitude * Math.sin(phase) };
}

function sensitivityPoint(point: FrequencyPoint, candidate: ControlPidCandidate) {
  const loop = frequencyPointToComplex(point);
  const onePlusLoop = { re: 1 + loop.real, im: loop.imaginary };
  const sensitivity = divide({ re: 1, im: 0 }, onePlusLoop);
  const complementary = divide({ re: loop.real, im: loop.imaginary }, onePlusLoop);
  const control = multiply(controllerFrequency(candidate, point.frequencyRadPerSecond), sensitivity);
  return {
    frequencyRadPerSecond: point.frequencyRadPerSecond,
    sensitivityMagnitudeDb: magnitudeDb(sensitivity),
    complementarySensitivityMagnitudeDb: magnitudeDb(complementary),
    controlSensitivityMagnitudeDb: magnitudeDb(control),
  };
}

function magnitudeDb(value: Complex): number {
  return 20 * Math.log10(Math.max(Number.MIN_VALUE, Math.hypot(value.re, value.im)));
}

function buildRootLocus(model: IdentifiedModel, candidate: ControlPidCandidate): EngineeringPlotEvidence["rootLocus"] {
  try {
    const plant = plantPolynomial(model);
    const controller = controllerPolynomial(candidate);
    const numerator = convolve(plant.numerator, controller.numerator);
    const denominator = convolve(plant.denominator, controller.denominator);
    const gains = [0, ...Array.from({ length: 61 }, (_, index) => 10 ** (-3 + index * 0.1))];
    const result = rootLocus({ numerator, denominator, delaySeconds: 0 }, gains) as {
      status: string; reason?: string;
      branches?: readonly { gain: number; poles: readonly { real: number; imaginary: number }[] }[];
    };
    return result.status === "qualified" && result.branches
      ? { status: "qualified", branches: result.branches }
      : { status: "unavailable", reason: result.reason ?? "The root locus could not be qualified.", branches: [] };
  } catch (error) {
    return { status: "unavailable", reason: error instanceof Error ? error.message : String(error), branches: [] };
  }
}

function plantPolynomial(model: IdentifiedModel): { numerator: number[]; denominator: number[] } {
  let numerator = [model.gain];
  let denominator = model.kind === "integrating" ? [1, 0] : [1];
  for (const time of [model.timeConstantSeconds, model.secondTimeConstantSeconds]) {
    if (time !== undefined) denominator = convolve(denominator, [time, 1]);
  }
  const delay = model.deadTimeSeconds ?? 0;
  if (delay > 0) {
    // A first-order Padé term keeps the delay visible in classical design
    // without pretending that an omitted delay has no stability effect.
    numerator = convolve(numerator, [-delay / 2, 1]);
    denominator = convolve(denominator, [delay / 2, 1]);
  }
  return { numerator, denominator };
}

function controllerPolynomial(candidate: ControlPidCandidate): { numerator: number[]; denominator: number[] } {
  const p = candidate.parameters;
  if (p.integralGain !== 0) {
    return { numerator: trimLeadingZeros([p.derivativeGain, p.proportionalGain, p.integralGain]),
      denominator: [1, 0] };
  }
  return { numerator: trimLeadingZeros([p.derivativeGain, p.proportionalGain]), denominator: [1] };
}

function trimLeadingZeros(values: number[]): number[] {
  const first = values.findIndex((value) => Math.abs(value) > 1e-15);
  if (first < 0) throw new Error("The controller has no finite non-zero gain for root-locus analysis.");
  return values.slice(first);
}

function convolve(left: readonly number[], right: readonly number[]): number[] {
  const result = Array(left.length + right.length - 1).fill(0) as number[];
  left.forEach((a, leftIndex) => right.forEach((b, rightIndex) => {
    result[leftIndex + rightIndex] += a * b;
  }));
  return result;
}

function buildUncertaintyEnvelope(
  model: IdentifiedModel,
  candidate: ControlPidCandidate,
  nominal: readonly EngineeringTimePoint[],
  cycleTime: number
): EngineeringPlotEvidence["uncertainty"] {
  const models = uncertaintyCornerModels(model);
  if (models.length === 0) return { status: "not-evaluated", method: "Not evaluated", points: [] };
  const traces = models.map((variant) => {
    const state = simulationState(variant, candidate, cycleTime);
    return nominal.map(() => advance(state, variant, candidate, cycleTime).pv);
  });
  return {
    status: "evaluated",
    method: model.uncertainty?.method ?? "declared model set",
    points: nominal.map((point, index) => {
      const values = [point.candidatePv, ...traces.map((trace) => trace[index])];
      return { timeSeconds: point.timeSeconds, lower: Math.min(...values),
        nominal: point.candidatePv, upper: Math.max(...values) };
    }),
  };
}

function uncertaintyCornerModels(model: IdentifiedModel): IdentifiedModel[] {
  const uncertainty = model.uncertainty;
  if (!uncertainty) return [];
  const fields = [
    ["gain", uncertainty.gain],
    ["timeConstantSeconds", uncertainty.timeConstantSeconds],
    ["secondTimeConstantSeconds", uncertainty.secondTimeConstantSeconds],
    ["deadTimeSeconds", uncertainty.deadTimeSeconds],
  ] as const;
  let variants: IdentifiedModel[] = [{ ...model }];
  for (const [field, interval] of fields) {
    if (!interval) continue;
    variants = variants.flatMap((variant) => [interval.minimum, interval.maximum]
      .map((value) => ({ ...variant, [field]: value })));
  }
  return variants.slice(0, 16);
}

function graphicalRequirements(
  time: readonly EngineeringTimePoint[],
  margins: { gainMarginDb: number; phaseMarginDegrees: number },
  sensitivity: EngineeringPlotEvidence["sensitivity"]
): EngineeringPlotEvidence["requirements"] {
  const current = responseMetrics(time.map((point) => ({ time: point.timeSeconds, pv: point.currentPv })));
  const candidate = responseMetrics(time.map((point) => ({ time: point.timeSeconds, pv: point.candidatePv })));
  const maximumOutput = Math.max(...time.map((point) => Math.abs(point.candidateMv)));
  const maximumSensitivity = Math.max(...sensitivity.map((point) => 10 ** (point.sensitivityMagnitudeDb / 20)));
  return [
    requirement("Overshoot", "response", candidate.overshootPercent, Math.max(10, current.overshootPercent), "%", "maximum"),
    requirement("Settling time", "response", candidate.settlingSeconds, current.settlingSeconds, "s", "maximum"),
    requirement("Gain margin", "bode", margins.gainMarginDb, 6, "dB", "minimum"),
    requirement("Phase margin", "bode", margins.phaseMarginDegrees, 30, "°", "minimum"),
    requirement("Peak sensitivity", "sensitivity", maximumSensitivity, 2, "ratio", "maximum"),
    requirement("Controller output", "constraints", maximumOutput, 100, "%", "maximum"),
  ];
}

function responseMetrics(points: readonly { time: number; pv: number }[]): {
  overshootPercent: number; settlingSeconds: number;
} {
  const overshootPercent = Math.max(0, (Math.max(...points.map((point) => point.pv)) - 1) * 100);
  let lastOutside = -1;
  points.forEach((point, index) => { if (Math.abs(point.pv - 1) > 0.02) lastOutside = index; });
  return { overshootPercent,
    settlingSeconds: lastOutside < points.length - 1 ? points[Math.max(0, lastOutside + 1)].time : Number.POSITIVE_INFINITY };
}

function requirement(
  name: string,
  plot: EngineeringPlotEvidence["requirements"][number]["plot"],
  value: number,
  limit: number,
  unit: string,
  direction: "maximum" | "minimum"
): EngineeringPlotEvidence["requirements"][number] {
  const valid = Number.isFinite(value) || value === Number.POSITIVE_INFINITY;
  const passed = valid && (direction === "maximum" ? value <= limit : value >= limit);
  return { name, plot, status: !valid ? "Inconclusive" : passed ? "Pass" : "Fail",
    value: value === Number.POSITIVE_INFINITY ? "unbounded" : value, limit, unit, direction };
}
