import { updateControlPid, type ControlPidCandidate, type ControlPidState } from "./controllerCandidate";
import type { IdentifiedModel } from "./identification";
import { buildEngineeringPlotEvidence } from "./engineeringPlots";
import { formatEngineeringNumber } from "./chartContract";

export interface ValidationScenario { kind: "setpoint" | "disturbance" | "noise"; amplitude: number }
export interface ValidationScenarioConditions {
  kind: ValidationScenario["kind"];
  event: { affectedSignal: "SP" | "Load disturbance" | "PV"; atSeconds: number; amplitude: number };
  disturbanceTrajectory: readonly { atSeconds: number; value: number }[];
  noiseAmplitude: number;
}
export interface ValidationConditions {
  initialCondition: { processValue: number; firstLagState: number; delayedInput: number;
    controller: ControlPidState };
  sampleTimesSeconds: readonly number[];
  durationSeconds: number;
  noise: { generator: "xorshift32-uniform"; seed: number };
  limits: { outputMinimum: number; outputMaximum: number; rateMinimum: number; rateMaximum: number };
  variationFractions: readonly number[];
  scenarios: readonly ValidationScenarioConditions[];
}
export type ValidationEvidence = "Model-predicted" | "Simulation-observed" | "Recorded" | "Physical-observed";
export interface ValidationRequirement { name: string; status: "Pass" | "Fail" | "Inconclusive"; value?: number; limit?: number; unit: string; source?: ValidationEvidence; reason: string }
export interface ValidationRequirementRow extends Omit<ValidationRequirement, "value" | "limit"> {
  value?: number | "unbounded";
  limit?: number | "unbounded";
}
export interface ValidationResult {
  evidence: ValidationEvidence;
  applied: false;
  conditions: ValidationConditions;
  current: { scenarios: readonly ValidationScenario[] };
  candidate: { scenarios: readonly ValidationScenario[] };
  requirements: readonly ValidationRequirement[];
  metrics?: { current: ControlPerformanceMetrics; candidate: ControlPerformanceMetrics };
}

export interface ControlPerformanceMetrics {
  iae: number;
  ise: number;
  itae: number;
  mvMovement: number;
  peakAbsoluteMv: number;
  maximumMvRate: number;
  noiseMvRms: number;
  worstCaseVariationIae: number;
}

export function formatValidationRequirementValue(
  value: number | "unbounded" | undefined,
  unit: string,
  limit = false
): string {
  if (value === undefined) return "—";
  if (unit === "boolean") {
    if (limit) return "Required";
    return value === 1 ? "Stable" : "Unstable";
  }
  return `${formatEngineeringNumber(value)} ${unit}`;
}

/**
 * Copies the complete validation contract for presentation. Keeping this
 * projection explicit prevents compact status summaries from silently
 * dropping the measured value, acceptance limit, evidence source, or reason.
 */
export function validationRequirementRows(
  requirements: readonly ValidationRequirement[]
): ValidationRequirementRow[] {
  return requirements.map((requirement) => ({
    ...requirement,
    ...(requirement.value === Number.POSITIVE_INFINITY ? { value: "unbounded" as const } : {}),
    ...(requirement.limit === Number.POSITIVE_INFINITY ? { limit: "unbounded" as const } : {}),
  }));
}

export function compareControlPidCandidates(
  model: IdentifiedModel,
  current: ControlPidCandidate,
  candidate: ControlPidCandidate,
  options: { evidence: ValidationEvidence; scenarios: readonly ValidationScenario[];
    variationFractions: readonly number[]; processUnit?: string; outputUnit?: string; noiseSeed?: number }
): ValidationResult {
  if (!model.stable || !Number.isFinite(model.gain) || model.gain === 0) {
    throw new Error("Validation requires a finite stable plant model.");
  }
  if (options.scenarios.length === 0) throw new Error("Validation requires at least one bounded scenario.");
  if (!options.variationFractions.every((value) => Number.isFinite(value) && value > -0.9 && value < 2)) {
    throw new Error("Validation variation fractions must be finite and bounded.");
  }
  const limits = commonLimits(current, candidate);
  const conditions = buildValidationConditions(model, options.scenarios, options.variationFractions,
    limits, options.noiseSeed ?? 20260828);
  const currentMetrics = aggregate(model, current, conditions);
  const candidateMetrics = aggregate(model, candidate, conditions);
  const variedModels = options.variationFractions.map((fraction) => ({ ...model,
    gain: model.gain * (1 + fraction),
    ...(model.timeConstantSeconds === undefined ? {} :
      { timeConstantSeconds: model.timeConstantSeconds * (1 + fraction) }),
    ...(model.secondTimeConstantSeconds === undefined ? {} :
      { secondTimeConstantSeconds: model.secondTimeConstantSeconds * (1 + fraction) }),
  }));
  const variationConditions = variedModels.map((varied, index) => buildValidationConditions(
    varied, [{ kind: "setpoint", amplitude: 1 }], [options.variationFractions[index]], limits,
    conditions.noise.seed));
  const currentVariations = variedModels.map((varied, index) =>
    aggregate(varied, current, variationConditions[index]));
  const candidateVariations = variedModels.map((varied, index) =>
    aggregate(varied, candidate, variationConditions[index]));
  const source = options.evidence;
  const outputUnit = options.outputUnit?.trim() || "engineering unit";
  const requirements: ValidationRequirement[] = [
    measuredRequirement("Settling time", candidateMetrics.settlingTimeSeconds,
      Math.max(currentMetrics.settlingTimeSeconds * 1.2, 0.1), "s", source,
      "Candidate settling time must remain within 120 percent of the current controller."),
    measuredRequirement("Overshoot", candidateMetrics.overshootPercent, 20, "%", source,
      "Peak response must remain within the declared overshoot limit."),
    measuredRequirement("Steady-state error", candidateMetrics.steadyStateError, 0.02, "ratio", source,
      "Final absolute error must be at most two percent of the test amplitude."),
    measuredRequirement("MV excursion", candidateMetrics.mvExcursion,
      candidate.parameters.outputMaximum - candidate.parameters.outputMinimum, outputUnit, source,
      "Controller output excursion must remain inside the configured output span."),
    measuredRequirement("MV rate", candidateMetrics.maximumMvRate,
      Math.max(Math.abs(candidate.parameters.rateMinimum), Math.abs(candidate.parameters.rateMaximum)),
      `${outputUnit}/s`, source, "Controller output rate must remain inside the configured rate limits."),
    measuredRequirement("Saturation recovery", candidateMetrics.saturationRecoverySeconds,
      candidateMetrics.horizonSeconds * 0.25, "s", source,
      "The response must recover from saturation within one quarter of the validation horizon."),
  ];
  if (options.scenarios.some((scenario) => scenario.kind === "noise")) {
    const outputSpan = candidate.parameters.outputMaximum - candidate.parameters.outputMinimum;
    requirements.push(measuredRequirement("Noise response", candidateMetrics.noiseMvRms,
      Math.max(outputSpan * 0.001, currentMetrics.noiseMvRms * 1.2), `${outputUnit} RMS`, source,
      "Candidate noise-driven output activity must remain within 120 percent of the current controller or 0.1 percent of the configured output span, whichever limit is larger."));
  } else {
    requirements.push(inconclusive("Noise response", `${outputUnit} RMS`, source,
      "No bounded noise scenario was selected."));
  }

  try {
    const frequency = buildEngineeringPlotEvidence(model, current, candidate);
    requirements.push(
      lowerBoundRequirement("Gain margin", frequency.gainMarginDb, 6, "dB", source,
        "Gain margin must be at least 6 dB."),
      lowerBoundRequirement("Phase margin", frequency.phaseMarginDegrees, 30, "degree", source,
        "Phase margin must be at least 30 degrees."),
    );
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    requirements.push(inconclusive("Gain margin", "dB", source, reason),
      inconclusive("Phase margin", "degree", source, reason));
  }
  const variationStable = candidateVariations.every((variation) => variation.stable);
  requirements.push({ name: "Bounded variation stability", status: variationStable ? "Pass" : "Fail",
    value: variationStable ? 1 : 0, limit: 1, unit: "boolean", source,
    reason: "Every declared bounded model variation must remain finite and bounded." });
  return { evidence: source, applied: false, conditions,
    current: { scenarios: options.scenarios.map((scenario) => ({ ...scenario })) },
    candidate: { scenarios: options.scenarios.map((scenario) => ({ ...scenario })) }, requirements,
    metrics: {
      current: performanceMetrics(currentMetrics, currentVariations),
      candidate: performanceMetrics(candidateMetrics, candidateVariations),
    } };
}

type ValidationModelTiming = {
  timeConstantSeconds?: number;
  secondTimeConstantSeconds?: number;
  deadTimeSeconds?: number;
};

function commonLimits(current: ControlPidCandidate, candidate: ControlPidCandidate): ValidationConditions["limits"] {
  const from = (value: ControlPidCandidate): ValidationConditions["limits"] => ({
    outputMinimum: value.parameters.outputMinimum, outputMaximum: value.parameters.outputMaximum,
    rateMinimum: value.parameters.rateMinimum, rateMaximum: value.parameters.rateMaximum,
  });
  const currentLimits = from(current);
  const candidateLimits = from(candidate);
  if (JSON.stringify(currentLimits) !== JSON.stringify(candidateLimits)) {
    throw new Error("Validation requires identical current and candidate output and rate limits.");
  }
  if (!Object.values(currentLimits).every(Number.isFinite) ||
      currentLimits.outputMinimum >= currentLimits.outputMaximum ||
      currentLimits.rateMinimum > currentLimits.rateMaximum) {
    throw new Error("Validation requires finite ordered output and rate limits.");
  }
  return currentLimits;
}

function buildValidationConditions(
  model: ValidationModelTiming,
  scenarios: readonly ValidationScenario[],
  variationFractions: readonly number[],
  limits: ValidationConditions["limits"],
  noiseSeed: number
): ValidationConditions {
  const firstTime = model.timeConstantSeconds ?? 1;
  const secondTime = model.secondTimeConstantSeconds ?? 0;
  const deadTime = model.deadTimeSeconds ?? 0;
  if (![firstTime, secondTime, deadTime].every(Number.isFinite) || firstTime <= 0 ||
      secondTime < 0 || deadTime < 0) {
    throw new Error("Validation requires finite nonnegative model timing and a positive primary time constant.");
  }
  if (!Number.isSafeInteger(noiseSeed) || noiseSeed <= 0 || noiseSeed > 0xffff_ffff) {
    throw new Error("Validation noise seed must be a positive 32-bit integer.");
  }
  if (!scenarios.every((scenario) => Number.isFinite(scenario.amplitude) &&
      (scenario.kind !== "noise" || scenario.amplitude >= 0))) {
    throw new Error("Validation scenario amplitudes must be finite and noise amplitude must be nonnegative.");
  }
  const nominalHorizon = Math.max(20, 12 * (firstTime + secondTime + deadTime));
  const sampleInterval = Math.min(0.02, firstTime / 100);
  const sampleCount = Math.ceil(nominalHorizon / sampleInterval) + 1;
  const sampleTimesSeconds = Array.from({ length: sampleCount }, (_, index) =>
    rounded(index * sampleInterval));
  const durationSeconds = sampleTimesSeconds[sampleTimesSeconds.length - 1];
  const saturatedOutput = Math.max(limits.outputMinimum, Math.min(limits.outputMaximum, 0));
  return {
    initialCondition: { processValue: 0, firstLagState: 0, delayedInput: 0, controller: {
      setpoint: 0, setpointRate: 0, processValue: 0, processValueRate: 0, feedForward: 0,
      nominalOutput: saturatedOutput, saturatedOutput,
    } },
    sampleTimesSeconds, durationSeconds,
    noise: { generator: "xorshift32-uniform", seed: noiseSeed },
    limits: { ...limits },
    variationFractions: [...variationFractions],
    scenarios: scenarios.map((scenario) => ({
      kind: scenario.kind,
      event: { affectedSignal: scenario.kind === "setpoint" ? "SP" :
        scenario.kind === "disturbance" ? "Load disturbance" : "PV",
        atSeconds: 0, amplitude: scenario.amplitude },
      disturbanceTrajectory: [{ atSeconds: 0,
        value: scenario.kind === "disturbance" ? scenario.amplitude : 0 }],
      noiseAmplitude: scenario.kind === "noise" ? scenario.amplitude : 0,
    })),
  };
}

interface ScenarioMetrics {
  settlingTimeSeconds: number;
  overshootPercent: number;
  steadyStateError: number;
  mvExcursion: number;
  maximumMvRate: number;
  saturationRecoverySeconds: number;
  noiseMvRms: number;
  iae: number;
  ise: number;
  itae: number;
  mvMovement: number;
  peakAbsoluteMv: number;
  horizonSeconds: number;
  stable: boolean;
}

function aggregate(model: IdentifiedModel, candidate: ControlPidCandidate,
  conditions: ValidationConditions): ScenarioMetrics {
  const results = conditions.scenarios.map((scenario) =>
    simulateExact(model, candidate, scenario, conditions));
  return {
    settlingTimeSeconds: Math.max(...results.map((item) => item.settlingTimeSeconds)),
    overshootPercent: Math.max(...results.map((item) => item.overshootPercent)),
    steadyStateError: Math.max(...results.map((item) => item.steadyStateError)),
    mvExcursion: Math.max(...results.map((item) => item.mvExcursion)),
    maximumMvRate: Math.max(...results.map((item) => item.maximumMvRate)),
    saturationRecoverySeconds: Math.max(...results.map((item) => item.saturationRecoverySeconds)),
    noiseMvRms: Math.max(...results.map((item) => item.noiseMvRms)),
    iae: results.reduce((sum, item) => sum + item.iae, 0),
    ise: results.reduce((sum, item) => sum + item.ise, 0),
    itae: results.reduce((sum, item) => sum + item.itae, 0),
    mvMovement: results.reduce((sum, item) => sum + item.mvMovement, 0),
    peakAbsoluteMv: Math.max(...results.map((item) => item.peakAbsoluteMv)),
    horizonSeconds: Math.max(...results.map((item) => item.horizonSeconds)),
    stable: results.every((item) => item.stable),
  };
}

function simulateExact(model: IdentifiedModel, candidate: ControlPidCandidate,
  scenario: ValidationScenarioConditions, conditions: ValidationConditions): ScenarioMetrics {
  const firstTime = model.timeConstantSeconds ?? 1;
  const secondTime = model.secondTimeConstantSeconds;
  const horizon = conditions.durationSeconds;
  const dt = conditions.sampleTimesSeconds[1] - conditions.sampleTimesSeconds[0];
  const delay = Array(Math.max(1, Math.round((model.deadTimeSeconds ?? 0) / dt)) + 1)
    .fill(conditions.initialCondition.delayedInput) as number[];
  let state: ControlPidState = { ...conditions.initialCondition.controller };
  let pv = conditions.initialCondition.processValue;
  let firstLag = conditions.initialCondition.firstLagState;
  let peak = pv; let lastOutside = horizon; let lastSaturated = -1;
  let minimumMv = Number.POSITIVE_INFINITY; let maximumMv = Number.NEGATIVE_INFINITY;
  let maximumMvRate = 0; let previousMv = state.saturatedOutput; let noiseMvSquared = 0; let noiseCount = 0;
  let iae = 0; let ise = 0; let itae = 0; let mvMovement = 0; let peakAbsoluteMv = 0;
  let stable = true;
  let noiseState = conditions.noise.seed >>> 0;
  const parameters = { ...candidate.parameters, ...conditions.limits };
  const target = scenario.kind === "setpoint" ? scenario.event.amplitude : 1;
  for (const time of conditions.sampleTimesSeconds) {
    noiseState ^= noiseState << 13;
    noiseState ^= noiseState >>> 17;
    noiseState ^= noiseState << 5;
    noiseState >>>= 0;
    const measurementNoise = scenario.kind === "noise" && time >= scenario.event.atSeconds
      ? scenario.noiseAmplitude * (2 * noiseState / 0x1_0000_0000 - 1) : 0;
    const result = updateControlPid(parameters, state, { setpoint: target,
      processValue: pv + measurementNoise, feedForward: 0, manualOutput: 0, trackingOutput: 0,
      mode: "Auto", cycleTime: dt });
    state = result.state;
    const mv = result.output;
    const mvIncrement = mv - previousMv;
    const error = target - pv;
    iae += Math.abs(error) * dt;
    ise += error * error * dt;
    itae += time * Math.abs(error) * dt;
    mvMovement += Math.abs(mvIncrement);
    peakAbsoluteMv = Math.max(peakAbsoluteMv, Math.abs(mv));
    minimumMv = Math.min(minimumMv, mv); maximumMv = Math.max(maximumMv, mv);
    maximumMvRate = Math.max(maximumMvRate, Math.abs(mvIncrement) / dt); previousMv = mv;
    if (Math.abs(mv - conditions.limits.outputMinimum) < 1e-9 ||
        Math.abs(mv - conditions.limits.outputMaximum) < 1e-9) lastSaturated = time;
    // After the startup transient, output increments are the actuator activity
    // caused by the deterministic measurement-noise excitation. Measuring the
    // saturation error here would incorrectly report zero for unsaturated loops.
    if (scenario.kind === "noise" && time >= horizon * 0.25) {
      noiseMvSquared += mvIncrement ** 2;
      noiseCount += 1;
    }
    delay.push(mv);
    const disturbance = trajectoryValue(scenario.disturbanceTrajectory, time);
    const plantInput = (delay.shift() ?? 0) + disturbance;
    if (model.kind === "integrating") pv += dt * model.gain * plantInput;
    else if (secondTime !== undefined) {
      firstLag += dt * (model.gain * plantInput - firstLag) / firstTime;
      pv += dt * (firstLag - pv) / secondTime;
    } else pv += dt * (model.gain * plantInput - pv) / firstTime;
    if (!Number.isFinite(pv) || Math.abs(pv) > 1_000_000) { stable = false; break; }
    peak = Math.max(peak, pv);
    if (Math.abs(target - pv) > Math.max(0.02, Math.abs(target) * 0.02)) lastOutside = time;
  }
  return {
    settlingTimeSeconds: Math.min(horizon, lastOutside + dt),
    overshootPercent: Math.max(0, (peak - target) / Math.max(Math.abs(target), 1e-9)) * 100,
    steadyStateError: Math.abs(target - pv) / Math.max(Math.abs(target), 1e-9),
    mvExcursion: maximumMv - minimumMv,
    maximumMvRate,
    // Recovery starts when limiting ends and finishes when the response last
    // leaves its settling band. The unused simulation tail is not recovery.
    saturationRecoverySeconds: lastSaturated < 0 ? 0 : Math.max(0, lastOutside - lastSaturated),
    noiseMvRms: Math.sqrt(noiseMvSquared / Math.max(1, noiseCount)),
    iae,
    ise,
    itae,
    mvMovement,
    peakAbsoluteMv,
    horizonSeconds: horizon,
    stable,
  };
}

function trajectoryValue(
  trajectory: readonly { atSeconds: number; value: number }[],
  timeSeconds: number
): number {
  let value = 0;
  for (const point of trajectory) {
    if (point.atSeconds > timeSeconds) break;
    value = point.value;
  }
  return value;
}

function performanceMetrics(
  metrics: ScenarioMetrics,
  variations: readonly ScenarioMetrics[]
): ControlPerformanceMetrics {
  return {
    iae: rounded(metrics.iae),
    ise: rounded(metrics.ise),
    itae: rounded(metrics.itae),
    mvMovement: rounded(metrics.mvMovement),
    peakAbsoluteMv: rounded(metrics.peakAbsoluteMv),
    maximumMvRate: rounded(metrics.maximumMvRate),
    noiseMvRms: rounded(metrics.noiseMvRms),
    worstCaseVariationIae: rounded(Math.max(...variations.map((variation) => variation.iae))),
  };
}

function measuredRequirement(name: string, value: number, limit: number, unit: string,
  source: ValidationEvidence, reason: string): ValidationRequirement {
  if (!Number.isFinite(value) || !Number.isFinite(limit)) return { name, status: "Inconclusive", unit, source,
    reason: `${reason} A finite comparison was not available.` };
  return { name, status: value <= limit ? "Pass" : "Fail", value: rounded(value), limit: rounded(limit), unit, source, reason };
}
function lowerBoundRequirement(name: string, value: number, limit: number, unit: string,
  source: ValidationEvidence, reason: string): ValidationRequirement {
  if (value === Number.POSITIVE_INFINITY) return { name, status: "Pass", value, limit, unit, source,
    reason: `${reason} No finite phase crossover exists, so the positive gain margin is unbounded.` };
  if (!Number.isFinite(value)) return { name, status: "Inconclusive", unit, source, reason };
  return { name, status: value >= limit ? "Pass" : "Fail", value: rounded(value), limit, unit, source, reason };
}
function inconclusive(name: string, unit: string, source: ValidationEvidence, reason: string): ValidationRequirement {
  return { name, status: "Inconclusive", unit, source, reason };
}
export function compareControllers(
  model: { gain: number; timeConstantSeconds: number; deadTimeSeconds: number },
  current: { structure: "PI"; kp: number; tiSeconds: number },
  candidate: { structure: "PI"; kp: number; tiSeconds: number },
  scenarios: readonly ValidationScenario[]
): ValidationResult {
  for (const value of [model.gain, model.timeConstantSeconds, model.deadTimeSeconds, current.kp,
    current.tiSeconds, candidate.kp, candidate.tiSeconds]) {
    if (!Number.isFinite(value)) throw new Error("Validation inputs must be finite.");
  }
  const conditions = buildValidationConditions(model, scenarios, [0],
    { outputMinimum: 0, outputMaximum: 100, rateMinimum: -100, rateMaximum: 100 }, 20260828);
  const currentMetrics = simulate(model, current, conditions);
  const candidateMetrics = simulate(model, candidate, conditions);
  const requirements: ValidationRequirement[] = [
    requirement("Settling time", candidateMetrics.settling, currentMetrics.settling, "s", "Candidate must not settle slower than current."),
    requirement("Overshoot", candidateMetrics.overshoot, Math.max(10, currentMetrics.overshoot), "%", "Overshoot must remain bounded."),
    requirement("Steady-state error", candidateMetrics.error, 0.02, "ratio", "Final error must be at most 2 percent."),
    requirement("Control output", candidateMetrics.peakOutput, 100, "%", "Predicted output must remain inside configured limits."),
  ];
  return {
    evidence: "Model-predicted", applied: false, conditions,
    current: { scenarios: scenarios.map((scenario) => ({ ...scenario })) },
    candidate: { scenarios: scenarios.map((scenario) => ({ ...scenario })) },
    requirements,
  };
}

function simulate(model: { gain: number; timeConstantSeconds: number; deadTimeSeconds: number },
  controller: { kp: number; tiSeconds: number }, conditions: ValidationConditions) {
  const dt = conditions.sampleTimesSeconds[1] - conditions.sampleTimesSeconds[0];
  const horizon = conditions.durationSeconds;
  const delay = Array(Math.max(1, Math.round(model.deadTimeSeconds / dt)) + 1).fill(0) as number[];
  let pv = 0; let integral = 0; let peak = 0; let peakOutput = 0; let lastOutside = horizon;
  for (let index = 0; index < conditions.sampleTimesSeconds.length; index += 1) {
    const error = 1 - pv;
    integral += error * dt;
    const output = Math.max(0, Math.min(100, controller.kp * (error + integral / controller.tiSeconds)));
    peakOutput = Math.max(peakOutput, Math.abs(output));
    delay.push(output);
    pv += dt * (model.gain * (delay.shift() ?? 0) - pv) / model.timeConstantSeconds;
    peak = Math.max(peak, pv);
    if (Math.abs(error) > 0.02) lastOutside = conditions.sampleTimesSeconds[index];
  }
  return { settling: Math.min(horizon, lastOutside + dt), overshoot: Math.max(0, peak - 1) * 100,
    error: Math.abs(1 - pv), peakOutput };
}
function requirement(name: string, value: number, limit: number, unit: string, reason: string): ValidationRequirement {
  return { name, status: value <= limit ? "Pass" : "Fail", value: rounded(value), limit: rounded(limit), unit, reason };
}
function rounded(value: number): number { return Number(value.toPrecision(8)); }
