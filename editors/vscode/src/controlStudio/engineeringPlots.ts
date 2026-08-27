import { updateControlPid, type ControlPidCandidate, type ControlPidState } from "./controllerCandidate";
import type { IdentifiedModel } from "./identification";
import type { FrequencyPoint } from "./tuning";

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
  const minimumFrequency = 1e-3 / baseTime;
  const maximumFrequency = Math.max(1e3 / baseTime, delay > 0 ? 4 * Math.PI / delay : 0);
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
  return { evidence: "Model-predicted", timeResponse, plantBode: unwrappedPlant,
    candidateOpenLoopBode: unwrappedOpenLoop, ...margins };
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
