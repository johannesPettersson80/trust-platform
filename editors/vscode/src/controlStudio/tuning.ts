import type { IdentifiedModel } from "./identification";
import { gainSignContradiction, requiredGainSign } from "./numerics/identification";

export interface FrequencyPoint { frequencyRadPerSecond: number; magnitudeDb: number; phaseDegrees: number }
export interface FrequencyResult { points: readonly FrequencyPoint[]; gainMarginDb: number; phaseMarginDegrees: number }
export type TuningMethod = "SIMC" | "AMIGO";
export type TuningObjective = "balanced" | "robust" | "fast";
export interface TuningResult {
  applied: false;
  controller: { structure: "PI" | "PD" | "PID"; kp: number; tiSeconds?: number; tdSeconds?: number };
  frequency: FrequencyResult;
  method: TuningMethod;
  objective: TuningObjective;
  convention: "Parallel Control.PID";
  closedLoopTimeSeconds?: number;
}

export interface TuningMethodDescription {
  method: TuningMethod;
  title: string;
  status: "Feasible" | "Unavailable";
  applied: false;
  convention: "Parallel Control.PID";
  assumptions: readonly string[];
  equations: readonly string[];
  units: { plantGain: string; proportionalGain: string; integralGain: string; derivativeGain: string };
  inputs: readonly { name: string; value: number | string; unit: string }[];
  result?: {
    proportionalGain: number;
    integralGain: number;
    derivativeGain: 0;
    integralTimeSeconds: number;
    gainMarginDb: number;
    phaseMarginDegrees: number;
  };
  reason?: string;
}

/** Describes the exact qualified formula and its domain without applying the result. */
export function describeTuningMethod(
  model: IdentifiedModel,
  method: TuningMethod,
  objective: TuningObjective,
  plantInputUnit: string,
  plantOutputUnit: string,
  action: "direct" | "reverse"
): TuningMethodDescription {
  const title = method === "SIMC" ? "SIMC PI (Skogestad 2003)" : "AMIGO PI (Åström and Hägglund 2004)";
  const integrating = model.kind === "integrating";
  // N6.1 and N6.4: the formulas run on gain magnitudes; s multiplies Kp and Kd by the action's sign.
  const sign = "s = +1 when the output increases while the measurement is below the setpoint, and -1 when the output increases while the measurement is above the setpoint; the formulas use gain magnitudes.";
  const assumptions = method === "SIMC" && integrating
    ? ["Accepted monotone integrating model with resolved positive effective delay and a nonzero rate gain: a positive rate gain suits a loop that increases the output while the measurement is below the setpoint; a negative rate gain suits a loop that increases the output while the measurement is above the setpoint.",
      "The response choice selects lambda; limits and acceptance requirements do not change.", sign]
    : method === "SIMC"
      ? ["Accepted stable monotone model: a positive-gain first-order or FOPDT model suits a loop that increases the output while the measurement is below the setpoint; a negative-gain model suits a loop that increases the output while the measurement is above the setpoint.",
      "The response choice selects lambda; limits and acceptance requirements do not change.", sign]
    : ["Accepted stable monotone model with resolved positive dead time: a positive-gain FOPDT model suits a loop that increases the output while the measurement is below the setpoint; a negative-gain model suits a loop that increases the output while the measurement is above the setpoint.",
      "The qualified domain is 0 < L/T <= 1; Response changes ranking, not the published coefficients.", sign];
  const equations = method === "SIMC" && integrating
    ? ["Kp = s / (|Kprime| (lambda + L))", "Ti = 4 (lambda + L)", "Ki = Kp / Ti", "Kd = 0"]
    : method === "SIMC"
      ? ["Kp = s T / (|K| (lambda + L))", "Ti = min(T, 4 (lambda + L))", "Ki = Kp / Ti", "Kd = 0"]
    : ["Kp = s (0.15/|K| + (0.35 - LT/(L+T)^2) T/(|K| L))",
      "Ti = 0.35L + 13LT^2/(T^2 + 12LT + 7L^2)", "Ki = Kp / Ti", "Kd = 0"];
  const units = {
    plantGain: integrating
      ? `${plantOutputUnit}/(${plantInputUnit}·s)`
      : `${plantOutputUnit}/${plantInputUnit}`,
    proportionalGain: `${plantInputUnit}/${plantOutputUnit}`,
    integralGain: `${plantInputUnit}/(${plantOutputUnit}·s)`,
    derivativeGain: `(${plantInputUnit}·s)/${plantOutputUnit}`,
  };
  const base = { method, title, applied: false as const, convention: "Parallel Control.PID" as const,
    assumptions, equations, units };
  try {
    // The same gain unit as Identify's refusal, so both messages read alike (recordedModelIdentification.ts checkGainSign).
    const result = tuneController(model, { method, objective, action, unit: `${plantOutputUnit}/${plantInputUnit}${integrating ? "/s" : ""}` });
    const ti = result.controller.tiSeconds!;
    const inputs: Array<{ name: string; value: number | string; unit: string }> = [
      { name: integrating ? "Kprime" : "K", value: model.gain, unit: units.plantGain },
      { name: "L", value: model.deadTimeSeconds ?? 0, unit: "s" },
      { name: "Objective", value: objective, unit: "selection" },
    ];
    if (!integrating) inputs.splice(1, 0,
      { name: "T", value: model.timeConstantSeconds!, unit: "s" });
    if (result.closedLoopTimeSeconds !== undefined) {
      inputs.push({ name: "lambda", value: result.closedLoopTimeSeconds, unit: "s" });
    }
    return { ...base, status: "Feasible", inputs, result: {
      proportionalGain: result.controller.kp,
      integralGain: rounded(result.controller.kp / ti),
      derivativeGain: 0,
      integralTimeSeconds: ti,
      gainMarginDb: result.frequency.gainMarginDb,
      phaseMarginDegrees: result.frequency.phaseMarginDegrees,
    } };
  } catch (error) {
    return { ...base, status: "Unavailable", inputs: [
      { name: integrating ? "Kprime" : "K", value: model.gain, unit: units.plantGain },
      ...(!integrating ? [{ name: "T", value: model.timeConstantSeconds ?? "Unavailable", unit: "s" }] : []),
      { name: "L", value: model.deadTimeSeconds ?? 0, unit: "s" },
      { name: "Objective", value: objective, unit: "selection" },
    ], reason: error instanceof Error ? error.message : String(error) };
  }
}

export function frequencyResponse(_model: IdentifiedModel): FrequencyResult {
  return loopFrequency(_model, 1, Number.POSITIVE_INFINITY);
}

export function tuneController(
  model: IdentifiedModel,
  options: { method: TuningMethod; objective: TuningObjective; action: "direct" | "reverse"; unit?: string }
): TuningResult {
  validateTuningModel(model, options);
  const integrating = model.kind === "integrating";
  if (!integrating && model.kind !== "first_order" && model.kind !== "first_order_dead_time") {
    throw new Error(`${options.method} PI tuning requires a stable first-order model.`);
  }
  const delay = model.deadTimeSeconds ?? 0;
  // N5.1: the formulas run on the gain magnitude; N6.4 then gives Kp the sign of the action.
  const gain = Math.abs(model.gain);
  let kpMagnitude: number;
  let tiSeconds: number;
  let closedLoopTimeSeconds: number | undefined;
  if (options.method === "SIMC") {
    const tau = model.timeConstantSeconds;
    const fastest = integrating ? delay : Math.max(delay, tau! / 3);
    closedLoopTimeSeconds = fastest * (options.objective === "fast" ? 1 : options.objective === "balanced" ? 2 : 4);
    kpMagnitude = integrating
      ? 1 / (gain * (closedLoopTimeSeconds + delay))
      : tau! / (gain * (closedLoopTimeSeconds + delay));
    tiSeconds = integrating
      ? 4 * (closedLoopTimeSeconds + delay)
      : Math.min(tau!, 4 * (closedLoopTimeSeconds + delay));
  } else {
    const tau = model.timeConstantSeconds!;
    const sum = delay + tau;
    kpMagnitude = 0.15 / gain
      + (0.35 - delay * tau / (sum * sum)) * tau / (gain * delay);
    tiSeconds = 0.35 * delay
      + 13 * delay * tau * tau / (tau * tau + 12 * delay * tau + 7 * delay * delay);
  }
  if (![kpMagnitude, tiSeconds].every((value) => Number.isFinite(value) && value > 0)) {
    throw new Error("The model does not yield finite positive PI settings.");
  }
  // N6.4: Kp = s_a |Kp|. Ki = Kp / Ti then takes the same sign, because Ti > 0.
  const kp = requiredGainSign(options.action) * kpMagnitude;
  return {
    applied: false,
    controller: { structure: "PI", kp: rounded(kp), tiSeconds: rounded(tiSeconds) },
    frequency: loopFrequency(model, kp, tiSeconds),
    method: options.method,
    objective: options.objective,
    convention: "Parallel Control.PID",
    ...(closedLoopTimeSeconds === undefined ? {} : { closedLoopTimeSeconds: rounded(closedLoopTimeSeconds) }),
  };
}

function loopFrequency(model: IdentifiedModel, kp: number, tiSeconds: number): FrequencyResult {
  validateModel(model);
  const baseTau = model.timeConstantSeconds ?? model.secondTimeConstantSeconds ??
    Math.max(model.deadTimeSeconds ?? 0, 1);
  // The phase starts at 0 degrees when the plant and controller gains share a sign (K Kp > 0), at -180 degrees otherwise.
  const startPhase = Math.sign(model.gain) * Math.sign(kp) < 0 ? -180 : 0;
  const points: FrequencyPoint[] = [];
  for (let index = 0; index < 80; index += 1) {
    const frequency = (1e-3 / baseTau) * Math.pow(1e6, index / 79);
    let magnitude = Math.abs(model.gain);
    let phase = startPhase;
    if (model.kind === "integrating") {
      magnitude /= frequency;
      phase -= 90;
    }
    for (const tau of [model.timeConstantSeconds, model.secondTimeConstantSeconds]) {
      if (tau !== undefined) { magnitude /= Math.sqrt(1 + (frequency * tau) ** 2); phase -= Math.atan(frequency * tau) * 180 / Math.PI; }
    }
    phase -= frequency * (model.deadTimeSeconds ?? 0) * 180 / Math.PI;
    if (Number.isFinite(tiSeconds)) {
      magnitude *= Math.abs(kp) * Math.sqrt(1 + 1 / (frequency * tiSeconds) ** 2);
      phase -= Math.atan(1 / (frequency * tiSeconds)) * 180 / Math.PI;
    }
    points.push({ frequencyRadPerSecond: frequency, magnitudeDb: 20 * Math.log10(magnitude), phaseDegrees: phase });
  }
  const gainCrossing = closest(points, (point) => Math.abs(point.magnitudeDb));
  const phaseCrossing = closest(points, (point) => Math.abs(point.phaseDegrees + 180));
  return {
    points,
    // Negative margins are safety evidence. Never turn them into a small
    // positive number merely to make a candidate appear acceptable.
    gainMarginDb: rounded(-phaseCrossing.magnitudeDb),
    phaseMarginDegrees: rounded(180 + gainCrossing.phaseDegrees),
  };
}

function validateModel(model: IdentifiedModel): void {
  if (!model.stable && model.kind !== "integrating") throw new Error("A stable model is required for frequency results.");
  if (!Number.isFinite(model.gain) || model.gain === 0) throw new Error("Model gain must be finite and non-zero.");
  for (const value of [model.timeConstantSeconds, model.secondTimeConstantSeconds]) {
    if (value !== undefined && (!Number.isFinite(value) || value <= 0)) throw new Error("Model time constants must be finite and positive.");
  }
}

function validateTuningModel(model: IdentifiedModel, options: { method: TuningMethod; action: "direct" | "reverse"; unit?: string }): void {
  const method = options.method;
  validateModel(model);
  // N6.3: a gain whose sign contradicts the controller action is refused with the same reason Identify gives.
  const contradiction = gainSignContradiction(model.gain, options.unit ?? "", options.action);
  if (contradiction) throw new Error(contradiction);
  if (!Number.isFinite(model.normalizedError) || model.normalizedError > 0.25) {
    throw new Error(`${method} PI tuning requires accepted model error no greater than 0.25.`);
  }
  const delay = model.deadTimeSeconds ?? 0;
  if (!Number.isFinite(delay) || delay < 0) throw new Error("Model dead time must be finite and non-negative.");
  if (model.kind === "integrating") {
    if (method === "AMIGO") throw new Error("AMIGO is unavailable for integrating models.");
    if (delay <= 0) throw new Error("SIMC integrating PI requires resolved positive effective delay.");
  }
  if (method === "AMIGO") {
    if (delay <= 0) throw new Error("AMIGO requires resolved positive dead time.");
    const tau = model.timeConstantSeconds;
    if (tau !== undefined && delay / tau > 1) {
      throw new Error("AMIGO delay ratio is outside the qualified delay domain L/T <= 1.");
    }
  }
}
function closest(points: readonly FrequencyPoint[], score: (point: FrequencyPoint) => number): FrequencyPoint {
  return points.reduce((best, point) => score(point) < score(best) ? point : best);
}
function rounded(value: number): number { return Number(value.toPrecision(8)); }
