import { updateControlPid, type ControlPidCandidate, type ControlPidState } from "./controllerCandidate";
import type { IdentifiedModel } from "./identification";
import { buildEngineeringPlotEvidence } from "./engineeringPlots";
import { formatEngineeringNumber } from "./chartContract";

export interface ValidationScenario { kind: "setpoint" | "disturbance" | "noise"; amplitude: number }
export type ValidationEvidence = "Model-predicted" | "Simulation-observed" | "Recorded" | "Physical-observed";
export interface ValidationRequirement { name: string; status: "Pass" | "Fail" | "Not evaluated" | "Inconclusive"; value?: number; limit?: number; unit: string; source?: ValidationEvidence; reason: string }
export interface ValidationRequirementRow extends Omit<ValidationRequirement, "value" | "limit"> {
  value?: number | "unbounded";
  limit?: number | "unbounded";
}
export interface ValidationResult {
  evidence: ValidationEvidence;
  applied: false;
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
    variationFractions: readonly number[]; processUnit?: string; outputUnit?: string }
): ValidationResult {
  if (!model.stable || !Number.isFinite(model.gain) || model.gain === 0) {
    throw new Error("Validation requires a finite stable plant model.");
  }
  if (options.scenarios.length === 0) throw new Error("Validation requires at least one bounded scenario.");
  if (!options.variationFractions.every((value) => Number.isFinite(value) && value > -0.9 && value < 2)) {
    throw new Error("Validation variation fractions must be finite and bounded.");
  }
  const currentMetrics = aggregate(model, current, options.scenarios);
  const candidateMetrics = aggregate(model, candidate, options.scenarios);
  const variedModels = options.variationFractions.map((fraction) => ({ ...model,
    gain: model.gain * (1 + fraction),
    ...(model.timeConstantSeconds === undefined ? {} :
      { timeConstantSeconds: model.timeConstantSeconds * (1 + fraction) }),
    ...(model.secondTimeConstantSeconds === undefined ? {} :
      { secondTimeConstantSeconds: model.secondTimeConstantSeconds * (1 + fraction) }),
  }));
  const currentVariations = variedModels.map((varied) =>
    aggregate(varied, current, [{ kind: "setpoint", amplitude: 1 }]));
  const candidateVariations = variedModels.map((varied) =>
    aggregate(varied, candidate, [{ kind: "setpoint", amplitude: 1 }]));
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
    requirements.push(measuredRequirement("Noise response", candidateMetrics.noiseMvRms,
      Math.max((candidate.parameters.outputMaximum - candidate.parameters.outputMinimum) * 1e-9,
        currentMetrics.noiseMvRms * 1.2), `${outputUnit} RMS`, source,
      "Candidate noise-driven output activity must not materially exceed the current controller."));
  } else {
    requirements.push(notEvaluated("Noise response", `${outputUnit} RMS`, source,
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
    requirements.push(notEvaluated("Gain margin", "dB", source, reason),
      notEvaluated("Phase margin", "degree", source, reason));
  }
  const variationStable = candidateVariations.every((variation) => variation.stable);
  requirements.push({ name: "Bounded variation stability", status: variationStable ? "Pass" : "Fail",
    value: variationStable ? 1 : 0, limit: 1, unit: "boolean", source,
    reason: "Every declared bounded model variation must remain finite and bounded." });
  return { evidence: source, applied: false,
    current: { scenarios: options.scenarios.map((scenario) => ({ ...scenario })) },
    candidate: { scenarios: options.scenarios.map((scenario) => ({ ...scenario })) }, requirements,
    metrics: {
      current: performanceMetrics(currentMetrics, currentVariations),
      candidate: performanceMetrics(candidateMetrics, candidateVariations),
    } };
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
  scenarios: readonly ValidationScenario[]): ScenarioMetrics {
  const results = scenarios.map((scenario) => simulateExact(model, candidate, scenario));
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
  scenario: ValidationScenario): ScenarioMetrics {
  const firstTime = model.timeConstantSeconds ?? 1;
  const secondTime = model.secondTimeConstantSeconds;
  const horizon = Math.max(20, 12 * (firstTime + (secondTime ?? 0) + (model.deadTimeSeconds ?? 0)));
  const dt = Math.min(0.02, firstTime / 100);
  const delay = Array(Math.max(1, Math.round((model.deadTimeSeconds ?? 0) / dt)) + 1).fill(0) as number[];
  let state: ControlPidState = { setpoint: 0, setpointRate: 0, processValue: 0, processValueRate: 0,
    feedForward: 0, nominalOutput: 0, saturatedOutput: Math.max(0, candidate.parameters.outputMinimum) };
  let pv = 0; let firstLag = 0; let peak = 0; let lastOutside = horizon; let lastSaturated = -1;
  let minimumMv = Number.POSITIVE_INFINITY; let maximumMv = Number.NEGATIVE_INFINITY;
  let maximumMvRate = 0; let previousMv = state.saturatedOutput; let noiseMvSquared = 0; let noiseCount = 0;
  let iae = 0; let ise = 0; let itae = 0; let mvMovement = 0; let peakAbsoluteMv = 0;
  let stable = true;
  const target = scenario.kind === "setpoint" ? scenario.amplitude : 1;
  for (let step = 0; step <= Math.ceil(horizon / dt); step += 1) {
    const time = step * dt;
    const measurementNoise = scenario.kind === "noise" ? scenario.amplitude * Math.sin(2 * Math.PI * 0.7 * time) : 0;
    const result = updateControlPid(candidate.parameters, state, { setpoint: target,
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
    if (Math.abs(mv - candidate.parameters.outputMinimum) < 1e-9 ||
        Math.abs(mv - candidate.parameters.outputMaximum) < 1e-9) lastSaturated = time;
    // After the startup transient, output increments are the actuator activity
    // caused by the deterministic measurement-noise excitation. Measuring the
    // saturation error here would incorrectly report zero for unsaturated loops.
    if (scenario.kind === "noise" && time >= horizon * 0.25) {
      noiseMvSquared += mvIncrement ** 2;
      noiseCount += 1;
    }
    delay.push(mv);
    const disturbance = scenario.kind === "disturbance" ? scenario.amplitude : 0;
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
function notEvaluated(name: string, unit: string, source: ValidationEvidence, reason: string): ValidationRequirement {
  return { name, status: "Not evaluated", unit, source, reason };
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
  const currentMetrics = simulate(model, current);
  const candidateMetrics = simulate(model, candidate);
  const requirements: ValidationRequirement[] = [
    requirement("Settling time", candidateMetrics.settling, currentMetrics.settling, "s", "Candidate must not settle slower than current."),
    requirement("Overshoot", candidateMetrics.overshoot, Math.max(10, currentMetrics.overshoot), "%", "Overshoot must remain bounded."),
    requirement("Steady-state error", candidateMetrics.error, 0.02, "ratio", "Final error must be at most 2 percent."),
    requirement("Control output", candidateMetrics.peakOutput, 100, "%", "Predicted output must remain inside configured limits."),
  ];
  return {
    evidence: "Model-predicted", applied: false,
    current: { scenarios: scenarios.map((scenario) => ({ ...scenario })) },
    candidate: { scenarios: scenarios.map((scenario) => ({ ...scenario })) },
    requirements,
  };
}

function simulate(model: { gain: number; timeConstantSeconds: number; deadTimeSeconds: number }, controller: { kp: number; tiSeconds: number }) {
  const dt = Math.min(0.02, model.timeConstantSeconds / 100);
  const horizon = Math.max(20, 12 * (model.timeConstantSeconds + model.deadTimeSeconds));
  const delay = Array(Math.max(1, Math.round(model.deadTimeSeconds / dt)) + 1).fill(0) as number[];
  let pv = 0; let integral = 0; let peak = 0; let peakOutput = 0; let lastOutside = horizon;
  for (let index = 0; index <= horizon / dt; index += 1) {
    const error = 1 - pv;
    integral += error * dt;
    const output = Math.max(0, Math.min(100, controller.kp * (error + integral / controller.tiSeconds)));
    peakOutput = Math.max(peakOutput, Math.abs(output));
    delay.push(output);
    pv += dt * (model.gain * (delay.shift() ?? 0) - pv) / model.timeConstantSeconds;
    peak = Math.max(peak, pv);
    if (Math.abs(error) > 0.02) lastOutside = index * dt;
  }
  return { settling: Math.min(horizon, lastOutside + dt), overshoot: Math.max(0, peak - 1) * 100,
    error: Math.abs(1 - pv), peakOutput };
}
function requirement(name: string, value: number, limit: number, unit: string, reason: string): ValidationRequirement {
  return { name, status: value <= limit ? "Pass" : "Fail", value: rounded(value), limit: rounded(limit), unit, reason };
}
function rounded(value: number): number { return Number(value.toPrecision(8)); }
