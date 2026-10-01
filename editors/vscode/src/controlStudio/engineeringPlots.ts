import { updateControlPid, type ControlPidCandidate, type ControlPidState } from "./controllerCandidate";
import type { IdentifiedModel } from "./identification";
import type { FrequencyPoint } from "./tuning";
import { rootLocus } from "./numerics";
import type { DesignComparisonContext } from "./chartContract";

const MAX_PREDICTION_PERIODS = 10_000;

export type RootLocusParameter = "proportionalGain" | "integralGain" | "derivativeGain";
export interface RootLocusSweep {
  parameter: RootLocusParameter;
  values: readonly number[];
  selectedValue: number;
}

export interface EngineeringPredictionOptions {
  cycleTimeSeconds?: number;
  durationSeconds?: number;
  referenceChange?: number;
  operatingPoint?: { input: number; output: number };
  /** First-order filter time Tf on the measurement the controller uses; 0 or absent bypasses it. */
  measurementFilterSeconds?: number;
  /** A constant load added to the plant input from the start, before its transport delay, as Validate's load trial adds it. */
  inputDisturbance?: number;
}

export interface EngineeringTimePoint {
  timeSeconds: number;
  currentPv?: number;
  candidatePv: number;
  currentMv?: number;
  candidateMv: number;
}

export interface EngineeringPlotEvidence {
  comparison?: { context: DesignComparisonContext; candidate: ControlPidCandidate };
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
  /** Ms: the largest |S| at any frequency, beyond the plotted range too, where a strictly proper loop's |S| tends to one. */
  peakSensitivity: number;
  rootLocus: { status: "qualified";
    parameter: RootLocusParameter;
    selectedValue: number;
    fixedParameters: Partial<Record<RootLocusParameter, number>>;
    approximation: string;
    branches: readonly {
    gain: number; poles: readonly { real: number; imaginary: number }[];
  }[] } | { status: "unavailable"; reason: string; branches: readonly [] };
  // The exact open-loop point where the candidate crosses 0 dB (used to read phase
  // margin on the phase/Nichols plot) and, when resolved, where it crosses -180 degrees
  // (used to read gain margin on the magnitude/Nichols plot). A loop with no delay and no
  // phase crossover leaves phaseCrossover undefined, matching an unbounded gain margin; a
  // loop whose gain stays below one leaves gainCrossover undefined, an unbounded phase margin.
  gainCrossover: FrequencyPoint | undefined;
  phaseCrossover: FrequencyPoint | undefined;
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

export type EngineeringPlotMessageEvidence = Omit<EngineeringPlotEvidence, "gainMarginDb" | "phaseMarginDegrees"> & {
  gainMarginDb: number | "unbounded";
  phaseMarginDegrees: number | "unbounded";
};

/** Converts mathematical infinity to an explicit value that survives VS Code message serialization. */
export function engineeringPlotMessageEvidence(evidence: EngineeringPlotEvidence): EngineeringPlotMessageEvidence {
  return { ...evidence,
    gainMarginDb: evidence.gainMarginDb === Number.POSITIVE_INFINITY ? "unbounded" : evidence.gainMarginDb,
    phaseMarginDegrees: evidence.phaseMarginDegrees === Number.POSITIVE_INFINITY ? "unbounded" : evidence.phaseMarginDegrees };
}

export function buildEngineeringPlotEvidence(
  model: IdentifiedModel,
  current: ControlPidCandidate,
  candidate: ControlPidCandidate,
  sweep?: RootLocusSweep,
  options: EngineeringPredictionOptions = {}
): EngineeringPlotEvidence {
  validateModel(model);
  const horizon = options.durationSeconds ?? Math.max(10, 10 * ((model.timeConstantSeconds ?? 1)
    + (model.secondTimeConstantSeconds ?? 0) + (model.deadTimeSeconds ?? 0)));
  const steps = options.cycleTimeSeconds === undefined ? Math.min(1_000, Math.max(200, Math.ceil(horizon / 0.05)))
    : Math.ceil(horizon / options.cycleTimeSeconds);
  const cycleTime = options.cycleTimeSeconds ?? horizon / steps;
  const operating = options.operatingPoint ?? { input: 0, output: 0 };
  const target = operating.output + (options.referenceChange ?? 1);
  if (![horizon, cycleTime, operating.input, operating.output, target].every(Number.isFinite) || horizon <= 0 || cycleTime <= 0 || steps > MAX_PREDICTION_PERIODS) {
    throw new Error("Prediction needs finite coordinates and at most 10000 controller periods.");
  }
  const filterSeconds = options.measurementFilterSeconds ?? 0;
  if (!Number.isFinite(filterSeconds) || filterSeconds < 0) throw new Error("The measurement filter time must be finite and not negative.");
  const disturbance = options.inputDisturbance ?? 0;
  if (!Number.isFinite(disturbance)) throw new Error("The input disturbance must be finite.");
  const currentSimulation = simulationState(model, current, cycleTime, operating, target, filterSeconds, disturbance);
  const candidateSimulation = simulationState(model, candidate, cycleTime, operating, target, filterSeconds, disturbance);
  const timeResponse: EngineeringTimePoint[] = [{ timeSeconds: 0, currentPv: operating.output,
    candidatePv: operating.output, currentMv: operating.input, candidateMv: operating.input }];
  for (let step = 1; step <= steps; step += 1) {
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
  let minimumFrequency = 1e-2 / baseTime;
  let maximumFrequency = Math.max(10 / baseTime, delay > 0 ? 2 * Math.PI / delay : 0);
  const loopMagnitude = (frequency: number) => {
    const value = multiply(plantFrequency(model, frequency), controllerFrequency(candidate, frequency, filterSeconds));
    return Math.hypot(value.re, value.im);
  };
  for (let expansion = 0; expansion < 16 && loopMagnitude(maximumFrequency) > 1; expansion++) maximumFrequency *= 2;
  for (let expansion = 0; expansion < 16 && loopMagnitude(minimumFrequency) < 1; expansion++) minimumFrequency /= 2;
  const decades = Math.log10(maximumFrequency / minimumFrequency);
  const frequencyPoints = Math.min(2_000, Math.max(100, Math.ceil(decades * 40) + 1));
  for (let index = 0; index < frequencyPoints; index += 1) {
    const frequency = minimumFrequency * Math.pow(maximumFrequency / minimumFrequency, index / (frequencyPoints - 1));
    const plant = plantFrequency(model, frequency);
    const controller = controllerFrequency(candidate, frequency, filterSeconds);
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
  const sensitivity = unwrappedOpenLoop.map((item) => sensitivityPoint(item, candidate, filterSeconds));
  const far = maximumFrequency * 1e6;
  const farLoop = multiply(plantFrequency(model, far), controllerFrequency(candidate, far, filterSeconds));
  const farSensitivity = divide({ re: 1, im: 0 }, { re: 1 + farLoop.re, im: farLoop.im });
  const peakSensitivity = Math.max(...sensitivity.map((item) => 10 ** (item.sensitivityMagnitudeDb / 20)), Math.hypot(farSensitivity.re, farSensitivity.im));
  const locus = buildRootLocus(model, candidate, sweep);
  const uncertainty = buildUncertaintyEnvelope(model, candidate, timeResponse, cycleTime);
  const requirements = graphicalRequirements(timeResponse, margins, peakSensitivity);
  return { evidence: "Model-predicted", comparison: { candidate,
    context: { modelRevision: JSON.stringify(model),
      evidenceRevision: JSON.stringify({ baseline: current.parameters, horizon, steps, options }),
      method: "unit-step-model-prediction-v1" } }, timeResponse, plantBode: unwrappedPlant,
    candidateOpenLoopBode: unwrappedOpenLoop, nichols, nyquist, sensitivity, peakSensitivity,
    rootLocus: locus, requirements, uncertainty, ...margins };
}

interface Complex { re: number; im: number }
interface SimulationState { pid: ControlPidState; pv: number; delay: number[]; lag2: number;
  operating: { input: number; output: number }; target: number; filtered: number; alpha: number; disturbance: number }

function simulationState(model: IdentifiedModel, candidate: ControlPidCandidate, cycleTime: number,
  operating = { input: 0, output: 0 }, target = 1, filterSeconds = 0, disturbance = 0): SimulationState {
  const delaySamples = Math.min(MAX_PREDICTION_PERIODS + 1, Math.max(0, Math.round((model.deadTimeSeconds ?? 0) / cycleTime)));
  return {
    pid: { setpoint: operating.output, setpointRate: 0, processValue: operating.output, processValueRate: 0,
      feedForward: 0, nominalOutput: operating.input, saturatedOutput: operating.input },
    pv: operating.output,
    delay: Array(delaySamples).fill(operating.input) as number[],
    // An integrating model keeps its rate here; a two-lag model its first lag.
    lag2: model.kind === "integrating" ? 0 : operating.output, operating, target,
    // alpha = dt/(Tf + dt): the backward-difference update of Tf·dyf/dt + yf = y.
    filtered: operating.output, alpha: cycleTime / (filterSeconds + cycleTime), disturbance,
  };
}

function advance(state: SimulationState, model: IdentifiedModel, candidate: ControlPidCandidate, cycleTime: number) {
  state.filtered += state.alpha * (state.pv - state.filtered);
  const updated = updateControlPid(candidate.parameters, state.pid, {
    setpoint: state.target, processValue: state.filtered, feedForward: 0, manualOutput: 0,
    trackingOutput: 0, mode: "Auto", cycleTime,
  });
  state.pid = updated.state;
  state.delay.push(updated.output + state.disturbance);
  const delayed = state.delay.shift() ?? 0;
  if (model.kind === "integrating") {
    const rate = model.gain * (delayed - state.operating.input);
    const lag = model.timeConstantSeconds;
    if (lag === undefined) {
      state.pv += cycleTime * rate;
    } else {
      // Exact held-input step of K/(s(τs + 1)): the rate approaches K·u with τ, the output integrates the rate.
      const decay = Math.exp(-cycleTime / lag);
      state.pv += rate * cycleTime + (state.lag2 - rate) * lag * (1 - decay);
      state.lag2 = rate + (state.lag2 - rate) * decay;
    }
  } else {
    const firstTime = model.timeConstantSeconds ?? 1;
    const firstTarget = state.operating.output + model.gain * (delayed - state.operating.input);
    if (model.secondTimeConstantSeconds !== undefined) {
      state.lag2 += cycleTime * (firstTarget - state.lag2) / firstTime;
      state.pv += cycleTime * (state.lag2 - state.pv) / model.secondTimeConstantSeconds;
    } else {
      state.pv = firstTarget + (state.pv - firstTarget) * Math.exp(-cycleTime / firstTime);
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

function controllerFrequency(candidate: ControlPidCandidate, frequency: number, filterSeconds = 0): Complex {
  const parameters = candidate.parameters;
  const controller = {
    re: parameters.proportionalGain,
    im: parameters.derivativeGain * frequency - parameters.integralGain / frequency,
  };
  // The controller acts on the filtered measurement: feedback passes 1/(1 + jωTf) first.
  return filterSeconds > 0 ? divide(controller, { re: 1, im: frequency * filterSeconds }) : controller;
}

function point(frequencyRadPerSecond: number, value: Complex): FrequencyPoint {
  return {
    frequencyRadPerSecond,
    magnitudeDb: 20 * Math.log10(Math.hypot(value.re, value.im)),
    phaseDegrees: Math.atan2(value.im, value.re) * 180 / Math.PI,
  };
}

function stabilityMargins(points: readonly FrequencyPoint[], deadTimeSeconds: number): {
  gainMarginDb: number; phaseMarginDegrees: number;
  gainCrossover: FrequencyPoint | undefined; phaseCrossover: FrequencyPoint | undefined;
} {
  const unwrapped = unwrapPhase(points);
  const gainCross = crossing(unwrapped, (point) => point.magnitudeDb);
  const phaseCross = crossing(unwrapped, (point) => point.phaseDegrees + 180);
  // Below 0 dB at every sampled frequency the loop never crosses: its phase margin is unbounded.
  if (!gainCross && !unwrapped.every((point) => point.magnitudeDb < 0)) {
    throw new Error("The sampled frequency range does not resolve the gain crossover.");
  }
  if (!phaseCross && deadTimeSeconds > 0) {
    throw new Error("The sampled frequency range does not resolve the phase crossover.");
  }
  return {
    gainMarginDb: phaseCross ? -phaseCross.magnitudeDb : Number.POSITIVE_INFINITY,
    phaseMarginDegrees: gainCross ? 180 + gainCross.phaseDegrees : Number.POSITIVE_INFINITY,
    gainCrossover: gainCross,
    phaseCrossover: phaseCross,
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

function sensitivityPoint(point: FrequencyPoint, candidate: ControlPidCandidate, filterSeconds = 0) {
  const loop = frequencyPointToComplex(point);
  const onePlusLoop = { re: 1 + loop.real, im: loop.imaginary };
  const sensitivity = divide({ re: 1, im: 0 }, onePlusLoop);
  const complementary = divide({ re: loop.real, im: loop.imaginary }, onePlusLoop);
  const control = multiply(controllerFrequency(candidate, point.frequencyRadPerSecond, filterSeconds), sensitivity);
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

function buildRootLocus(
  model: IdentifiedModel,
  candidate: ControlPidCandidate,
  requested?: RootLocusSweep
): EngineeringPlotEvidence["rootLocus"] {
  try {
    const sweep: RootLocusSweep = requested ?? {
      parameter: "proportionalGain",
      values: [...new Set([0, candidate.parameters.proportionalGain,
        ...Array.from({ length: 61 }, (_, index) => 10 ** (-3 + index * 0.1))])].sort((a, b) => a - b),
      selectedValue: candidate.parameters.proportionalGain,
    };
    if (!["proportionalGain", "integralGain", "derivativeGain"].includes(sweep.parameter)
        || !Array.isArray(sweep.values) || sweep.values.length === 0 || sweep.values.length > 1001
        || !sweep.values.every(value => Number.isFinite(value) && value >= 0)
        || !sweep.values.includes(sweep.selectedValue)) {
      throw new Error("Choose one gain, 1–1001 finite nonnegative values, and a selected value in that sweep.");
    }
    const plant = plantPolynomial(model);
    const branches = sweep.values.map(gain => {
      // Substitute only the selected coefficient. Multiplying the complete
      // controller would also change fixed derivative and integral feedback.
      const controller = controllerPolynomial({ ...candidate,
        parameters: { ...candidate.parameters, [sweep.parameter]: gain } });
      const numerator = convolve(plant.numerator, controller.numerator);
      const denominator = convolve(plant.denominator, controller.denominator);
      const result = rootLocus({ numerator, denominator, delaySeconds: 0 }, [1]) as {
        status: string; reason?: string;
        branches?: readonly { poles: readonly { real: number; imaginary: number }[] }[];
      };
      if (result.status !== "qualified" || !result.branches?.[0]) {
        throw new Error(result.reason ?? "The root locus could not be qualified.");
      }
      return { gain, poles: result.branches[0].poles };
    });
    const fixedParameters: Partial<Record<RootLocusParameter, number>> = {
      proportionalGain: candidate.parameters.proportionalGain,
      integralGain: candidate.parameters.integralGain,
      derivativeGain: candidate.parameters.derivativeGain,
    };
    delete fixedParameters[sweep.parameter];
    return { status: "qualified", branches, parameter: sweep.parameter,
      selectedValue: sweep.selectedValue, fixedParameters,
      approximation: `Continuous ideal controller; sample/hold, derivative filtering and output constraints are not represented.${(model.deadTimeSeconds ?? 0) > 0 ? " Delay uses first-order Padé approximation." : " No model delay."}` };
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
  if (first < 0) return [0];
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
    return nominal.map((_, index) => index === 0 ? state.pv : advance(state, variant, candidate, cycleTime).pv);
  });
  return {
    status: "evaluated",
    method: uncertaintyMethodLabel(model.uncertainty?.method),
    points: nominal.map((point, index) => {
      const values = [point.candidatePv, ...traces.map((trace) => trace[index])];
      return { timeSeconds: point.timeSeconds, lower: Math.min(...values),
        nominal: point.candidatePv, upper: Math.max(...values) };
    }),
  };
}

const UNCERTAINTY_METHOD_LABELS: Record<string, string> = {
  "sample-resolution-envelope": "sample-resolution envelope from the estimation record",
  "grid-resolution-and-validation-residual": "grid-resolution and held-out-residual interval",
  "measurement-consistent-grid": "box enclosing the measurement-consistent grid; parameter correlations omitted",
};

/** Maps the uncertainty method's internal identifier to words a student reads; no raw
 * enum value may reach the plot legend or its accessible description (finding V-17). */
function uncertaintyMethodLabel(method: NonNullable<IdentifiedModel["uncertainty"]>["method"] | undefined): string {
  if (!method) return "declared model set";
  return UNCERTAINTY_METHOD_LABELS[method] ?? method;
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
  maximumSensitivity: number
): EngineeringPlotEvidence["requirements"] {
  const current = responseMetrics(time.flatMap((point) => point.currentPv === undefined ? [] : [{ time: point.timeSeconds, pv: point.currentPv }]));
  const candidate = responseMetrics(time.map((point) => ({ time: point.timeSeconds, pv: point.candidatePv })));
  const maximumOutput = Math.max(...time.map((point) => Math.abs(point.candidateMv)));
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
