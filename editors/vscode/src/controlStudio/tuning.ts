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

export function frequencyResponse(_model: IdentifiedModel): FrequencyResult {
  return loopFrequency(_model, 1, Number.POSITIVE_INFINITY);
}

export function tuneController(
  model: IdentifiedModel,
  options: { method: TuningMethod; objective: TuningObjective }
): TuningResult {
  validateTuningModel(model, options.method);
  if (model.kind !== "first_order" && model.kind !== "first_order_dead_time") {
    throw new Error(`${options.method} PI tuning requires a stable first-order model.`);
  }
  const tau = model.timeConstantSeconds!;
  const delay = model.deadTimeSeconds ?? 0;
  let kp: number;
  let tiSeconds: number;
  let closedLoopTimeSeconds: number | undefined;
  if (options.method === "SIMC") {
    const fastest = Math.max(delay, tau / 3);
    closedLoopTimeSeconds = fastest * (options.objective === "fast" ? 1 : options.objective === "balanced" ? 2 : 4);
    kp = tau / (model.gain * (closedLoopTimeSeconds + delay));
    tiSeconds = Math.min(tau, 4 * (closedLoopTimeSeconds + delay));
  } else {
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
  const baseTau = model.timeConstantSeconds ?? model.secondTimeConstantSeconds ?? 1;
  const points: FrequencyPoint[] = [];
  for (let index = 0; index < 80; index += 1) {
    const frequency = (1e-3 / baseTau) * Math.pow(1e6, index / 79);
    let magnitude = Math.abs(model.gain);
    let phase = model.gain < 0 ? -180 : 0;
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
