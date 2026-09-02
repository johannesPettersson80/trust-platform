import type { IdentifiedModel } from "./identification";

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

export interface TuningAttempt {
  method: TuningMethod;
  status: "Feasible" | "Unavailable";
  result?: TuningResult;
  reason?: string;
}

export interface QualifiedTuningComparison {
  applied: false;
  objective: TuningObjective;
  model: IdentifiedModel;
  attempts: readonly TuningAttempt[];
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
  plantOutputUnit: string
): TuningMethodDescription {
  const title = method === "SIMC" ? "SIMC PI (Skogestad 2003)" : "AMIGO PI (Åström and Hägglund 2004)";
  const integrating = model.kind === "integrating";
  const assumptions = method === "SIMC" && integrating
    ? ["Accepted monotone integrating model with positive rate gain and resolved positive effective delay.",
      "The response choice selects lambda; limits and acceptance requirements do not change."]
    : method === "SIMC"
      ? ["Accepted stable monotone positive-gain first-order or FOPDT model.",
      "The response choice selects lambda; limits and acceptance requirements do not change."]
    : ["Accepted stable monotone positive-gain FOPDT model with resolved positive dead time.",
      "The qualified domain is 0 < L/T <= 1; Response changes ranking, not the published coefficients."];
  const equations = method === "SIMC" && integrating
    ? ["Kp = 1 / (Kprime (lambda + L))", "Ti = 4 (lambda + L)", "Ki = Kp / Ti", "Kd = 0"]
    : method === "SIMC"
      ? ["Kp = T / (K (lambda + L))", "Ti = min(T, 4 (lambda + L))", "Ki = Kp / Ti", "Kd = 0"]
    : ["Kp = 0.15/K + (0.35 - LT/(L+T)^2) T/(KL)",
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
    const result = tuneController(model, { method, objective });
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
  options: { method: TuningMethod; objective: TuningObjective }
): TuningResult {
  validateTuningModel(model, options.method);
  const integrating = model.kind === "integrating";
  if (!integrating && model.kind !== "first_order" && model.kind !== "first_order_dead_time") {
    throw new Error(`${options.method} PI tuning requires a stable first-order model.`);
  }
  const delay = model.deadTimeSeconds ?? 0;
  let kp: number;
  let tiSeconds: number;
  let closedLoopTimeSeconds: number | undefined;
  if (options.method === "SIMC") {
    const tau = model.timeConstantSeconds;
    const fastest = integrating ? delay : Math.max(delay, tau! / 3);
    closedLoopTimeSeconds = fastest * (options.objective === "fast" ? 1 : options.objective === "balanced" ? 2 : 4);
    kp = integrating
      ? 1 / (model.gain * (closedLoopTimeSeconds + delay))
      : tau! / (model.gain * (closedLoopTimeSeconds + delay));
    tiSeconds = integrating
      ? 4 * (closedLoopTimeSeconds + delay)
      : Math.min(tau!, 4 * (closedLoopTimeSeconds + delay));
  } else {
    const tau = model.timeConstantSeconds!;
    const sum = delay + tau;
    kp = 0.15 / model.gain
      + (0.35 - delay * tau / (sum * sum)) * tau / (model.gain * delay);
    tiSeconds = 0.35 * delay
      + 13 * delay * tau * tau / (tau * tau + 12 * delay * tau + 7 * delay * delay);
  }
  if (![kp, tiSeconds].every((value) => Number.isFinite(value) && value > 0)) {
    throw new Error("The model does not yield finite positive PI settings.");
  }
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

/** Runs every qualified formula adapter against one unchanged accepted model. */
export function compareQualifiedTunings(
  model: IdentifiedModel,
  objective: TuningObjective
): QualifiedTuningComparison {
  const attempts = (["SIMC", "AMIGO"] as const).map((method): TuningAttempt => {
    try {
      return { method, status: "Feasible", result: tuneController(model, { method, objective }) };
    } catch (error) {
      return { method, status: "Unavailable",
        reason: error instanceof Error ? error.message : String(error) };
    }
  });
  return { applied: false, objective, model: { ...model }, attempts };
}

function loopFrequency(model: IdentifiedModel, kp: number, tiSeconds: number): FrequencyResult {
  validateModel(model);
  const baseTau = model.timeConstantSeconds ?? model.secondTimeConstantSeconds ??
    Math.max(model.deadTimeSeconds ?? 0, 1);
  const points: FrequencyPoint[] = [];
  for (let index = 0; index < 80; index += 1) {
    const frequency = (1e-3 / baseTau) * Math.pow(1e6, index / 79);
    let magnitude = Math.abs(model.gain);
    let phase = model.gain < 0 ? -180 : 0;
    if (model.kind === "integrating") {
      magnitude /= frequency;
      phase -= 90;
    }
    for (const tau of [model.timeConstantSeconds, model.secondTimeConstantSeconds]) {
      if (tau !== undefined) { magnitude /= Math.sqrt(1 + (frequency * tau) ** 2); phase -= Math.atan(frequency * tau) * 180 / Math.PI; }
    }
    phase -= frequency * (model.deadTimeSeconds ?? 0) * 180 / Math.PI;
    if (Number.isFinite(tiSeconds)) {
      magnitude *= kp * Math.sqrt(1 + 1 / (frequency * tiSeconds) ** 2);
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
  if (!model.stable) throw new Error("A stable model is required for frequency results.");
  if (!Number.isFinite(model.gain) || model.gain === 0) throw new Error("Model gain must be finite and non-zero.");
  for (const value of [model.timeConstantSeconds, model.secondTimeConstantSeconds]) {
    if (value !== undefined && (!Number.isFinite(value) || value <= 0)) throw new Error("Model time constants must be finite and positive.");
  }
}

function validateTuningModel(model: IdentifiedModel, method: TuningMethod): void {
  validateModel(model);
  if (model.gain <= 0) throw new Error(`${method} PI tuning requires a finite positive gain.`);
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
