import type { PlantContractMessage } from "../networkCanvas/plant/contract";
import { tuneController, type FrequencyResult } from "./tuning";
import type { ValidationRequirement } from "./validation";
import { compareControlPidCandidates } from "./validation";
import {
  candidateFromControlPid,
  rankQualifiedCandidates,
  type ControlPidCandidate,
  type ControlPidParameters,
  type QualifiedCandidateAssessment,
} from "./controllerCandidate";
import { buildEngineeringPlotEvidence } from "./engineeringPlots";

export type AutomaticTuneObjective = "balanced" | "robust" | "fast";

export interface AutomaticTunePlan {
  objective: AutomaticTuneObjective;
  source: "Simulation";
  stages: readonly ["Test", "Identify", "Tune", "Validate"];
  evidenceReuse: string;
}

export interface AutomaticTuneResult {
  applied: false;
  modelEvidence: "Exact simulation model";
  structure: "P" | "PI" | "PD" | "PID";
  method: "SIMC" | "AMIGO" | "Constrained comparison";
  kp: number;
  tiSeconds?: number;
  validation: "Model-based validation · Pass";
  model: { kind: "first_order_dead_time"; gain: number; timeConstantSeconds: number; deadTimeSeconds: number };
  metrics: { finalError: number; overshootPercent: number; settlingTimeSeconds: number };
  rationale: string;
  frequency: FrequencyResult;
  requirements: readonly ValidationRequirement[];
  validationResult: ReturnType<typeof compareControlPidCandidates>;
  evaluatedStructures: readonly ["P", "PI", "PD", "PID"];
  parameters: ControlPidParameters;
  attempts: readonly AutomaticTuneAttempt[];
  closedLoopTimeSeconds?: number;
}

export interface AutomaticTuneAttempt {
  id: string;
  method: "SIMC" | "AMIGO" | "Constrained comparison";
  structure?: "P" | "PI" | "PD" | "PID";
  status: "Feasible" | "Rejected" | "Unavailable";
  score?: number;
  closedLoopTimeSeconds?: number;
  reason?: string;
}

export type AutomaticTuneOutcome =
  | { status: "complete"; completedStages: readonly ["Test", "Identify", "Tune", "Validate"]; result: AutomaticTuneResult }
  | { status: "failed"; completedStages: readonly ("Test" | "Identify" | "Tune" | "Validate")[];
      failedStage: "Test" | "Identify" | "Tune" | "Validate"; reason: string; recovery: string };

export function runSavedSimulationAutomaticTune(
  response: PlantContractMessage,
  plantId: string,
  objective: AutomaticTuneObjective,
  currentController?: ControlPidCandidate,
  units?: { processUnit: string; outputUnit: string }
): AutomaticTuneOutcome {
  const configuration = record(response.saved_configuration);
  const plants = Array.isArray(configuration?.plants) ? configuration.plants : [];
  const selected = plants.map(record).find((plant) => plant?.id === plantId);
  if (!selected) return failed([], "Test",
    `Plant ${plantId} is not present in the saved simulation configuration.`,
    "Save the simulated plant configuration and retry Automatic Tune.");
  if (selected.model !== "first_order_dead_time") return failed(["Test"], "Identify",
    "Automatic Tune currently requires a first-order-plus-dead-time simulation model.",
    "Configure a FOPDT simulation plant, or record data and identify a supported model.");
  try {
    const parameters = record(selected.parameters);
    positive(parameters?.gain, "gain");
    positive(parameters?.time_constant_s, "time constant");
    nonnegative(parameters?.dead_time_s, "dead time");
  } catch (error) {
    return failed(["Test"], "Identify", message(error),
      "Correct the saved model parameters, then retry identification.");
  }
  try {
    const result = tuneSavedSimulationPlant(response, plantId, objective, currentController, units);
    return { status: "complete", completedStages: ["Test", "Identify", "Tune", "Validate"], result };
  } catch (error) {
    const reason = message(error);
    const validationFailure = /no acceptable|validation|stability|did not meet/i.test(reason);
    return failed(validationFailure ? ["Test", "Identify", "Tune"] : ["Test", "Identify"],
      validationFailure ? "Validate" : "Tune", reason,
      validationFailure
        ? "Review the failed requirements or choose a more robust objective; mandatory limits are not weakened."
        : "Review the model and selected tuning objective, then retry tuning.");
  }
}

function failed(
  completedStages: readonly ("Test" | "Identify" | "Tune" | "Validate")[],
  failedStage: "Test" | "Identify" | "Tune" | "Validate",
  reason: string,
  recovery: string
): AutomaticTuneOutcome {
  return { status: "failed", completedStages, failedStage, reason, recovery };
}

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function automaticTunePlan(objective: AutomaticTuneObjective): AutomaticTunePlan {
  return {
    objective,
    source: "Simulation",
    stages: ["Test", "Identify", "Tune", "Validate"],
    evidenceReuse: "The saved simulation model is exact configuration evidence; no excitation is required.",
  };
}

export function materializeAutomaticTuneController(
  candidate: ControlPidCandidate
): { kp: number; tiSeconds?: number } {
  const kp = rounded(candidate.parameters.proportionalGain);
  if (candidate.parameters.integralGain === 0) return { kp };
  return { kp, tiSeconds: rounded(candidate.parameters.proportionalGain / candidate.parameters.integralGain) };
}

/**
 * Produces an unapplied SIMC PI candidate from the configured FOPDT model.
 * Keeping this calculation behind a truST-owned boundary prevents UI code and
 * third-party numerical types from becoming part of the product contract.
 */
export function tuneSavedSimulationPlant(
  response: PlantContractMessage,
  plantId: string,
  objective: AutomaticTuneObjective,
  currentController?: ControlPidCandidate,
  units?: { processUnit: string; outputUnit: string }
): AutomaticTuneResult {
  const configuration = record(response.saved_configuration);
  const plants = Array.isArray(configuration?.plants) ? configuration.plants : [];
  const selected = plants.map(record).find((plant) => plant?.id === plantId);
  if (!selected) throw new Error(`Plant ${plantId} is not present in the saved simulation configuration.`);
  if (selected.model !== "first_order_dead_time") {
    throw new Error("Automatic Tune currently requires a first-order-plus-dead-time simulation model.");
  }
  const parameters = record(selected.parameters);
  const gain = positive(parameters?.gain, "gain");
  const timeConstantSeconds = positive(parameters?.time_constant_s, "time constant");
  const deadTimeSeconds = nonnegative(parameters?.dead_time_s, "dead time");
  const model = { kind: "first_order_dead_time" as const, gain, timeConstantSeconds,
    deadTimeSeconds, normalizedError: 0, stable: true };
  const running = currentController?.parameters;
  // Tuning changes the controller gains, not the loop's engineering contract.
  // Reusing the active controller settings keeps every candidate comparable
  // under the same units, actuator limits, setpoint weights, and anti-windup.
  const common: Omit<ControlPidParameters, "proportionalGain" | "integralGain" | "derivativeGain"> = {
    proportionalSetpointWeight: running?.proportionalSetpointWeight ?? 1,
    derivativeSetpointWeight: running?.derivativeSetpointWeight ?? 0,
    bias: running?.bias ?? 0,
    outputMinimum: running?.outputMinimum ?? 0,
    outputMaximum: running?.outputMaximum ?? 100,
    rateMinimum: running?.rateMinimum ?? -100,
    rateMaximum: running?.rateMaximum ?? 100,
    trackingTime: running?.trackingTime ?? Math.max(timeConstantSeconds, 0.001),
    antiWindup: running?.antiWindup ?? "BackCalculation",
  };
  const candidates: { id: string; method: AutomaticTuneAttempt["method"]; candidate: ControlPidCandidate;
    closedLoopTimeSeconds?: number }[] = [];
  const attempts: AutomaticTuneAttempt[] = [];
  for (const method of ["SIMC", "AMIGO"] as const) {
    try {
      const tuned = tuneController(model, { method, objective });
      const kp = tuned.controller.kp;
      const ti = tuned.controller.tiSeconds!;
      candidates.push({ id: method.toLowerCase(), method,
        ...(tuned.closedLoopTimeSeconds === undefined ? {} :
          { closedLoopTimeSeconds: tuned.closedLoopTimeSeconds }),
        candidate: candidateFromControlPid({ ...common,
          proportionalGain: kp, integralGain: kp / ti, derivativeGain: 0 }, method) });
    } catch (error) {
      attempts.push({ id: method.toLowerCase(), method, status: "Unavailable",
        reason: error instanceof Error ? error.message : String(error) });
    }
  }
  const seed = candidates.find((item) => item.method === "SIMC") ?? candidates[0];
  if (seed) {
    const kp = seed.candidate.parameters.proportionalGain;
    const ki = seed.candidate.parameters.integralGain;
    const derivative = kp * Math.min(Math.max(deadTimeSeconds, timeConstantSeconds * 0.05), timeConstantSeconds / 4);
    const constrained = [
      { id: "constrained-p", proportionalGain: kp, integralGain: 0, derivativeGain: 0 },
      { id: "constrained-pi", proportionalGain: kp * 0.8, integralGain: ki * 0.64, derivativeGain: 0 },
      { id: "constrained-pd", proportionalGain: kp, integralGain: 0, derivativeGain: derivative },
      { id: "constrained-pid", proportionalGain: kp, integralGain: ki, derivativeGain: derivative },
    ];
    for (const item of constrained) {
      candidates.push({ id: item.id, method: "Constrained comparison",
        candidate: candidateFromControlPid({ ...common, proportionalGain: item.proportionalGain,
          integralGain: item.integralGain, derivativeGain: item.derivativeGain }, "Constrained comparison") });
    }
  }
  if (candidates.length === 0) throw new Error("No applicable tuning method found.");
  // A plant-only workflow has no running controller to compare. In that case
  // the generated PI is the declared reference for relative requirements;
  // loop-bound callers supply the authoritative runtime controller instead.
  const current = currentController ?? candidates[0].candidate;
  const evaluations = candidates.map((item) => {
    const validation = compareControlPidCandidates(model, current, item.candidate, {
      evidence: "Model-predicted",
      scenarios: [{ kind: "setpoint", amplitude: 1 }, { kind: "disturbance", amplitude: 0.2 },
        { kind: "noise", amplitude: 0.02 }],
      variationFractions: [-0.2, 0, 0.2],
      ...(units?.processUnit ? { processUnit: units.processUnit } : {}),
      ...(units?.outputUnit ? { outputUnit: units.outputUnit } : {}),
    });
    const feasible = validation.requirements.every((requirement) =>
      requirement.status !== "Fail" && requirement.status !== "Inconclusive");
    const metric = validation.metrics?.candidate;
    const gainMarginDb = requirementValue(validation.requirements, "Gain margin");
    const phaseMarginDegrees = requirementValue(validation.requirements, "Phase margin");
    const assessment: QualifiedCandidateAssessment = { id: item.id, method: item.method,
      candidate: item.candidate, feasible: feasible && metric !== undefined,
      metrics: {
        iae: metric?.iae ?? Number.POSITIVE_INFINITY,
        ise: metric?.ise ?? Number.POSITIVE_INFINITY,
        itae: metric?.itae ?? Number.POSITIVE_INFINITY,
        mvMovement: metric?.mvMovement ?? Number.POSITIVE_INFINITY,
        peakAbsoluteMv: metric?.peakAbsoluteMv ?? Number.POSITIVE_INFINITY,
        maximumMvRate: metric?.maximumMvRate ?? Number.POSITIVE_INFINITY,
        noiseMvRms: metric?.noiseMvRms ?? Number.POSITIVE_INFINITY,
        worstCaseVariationIae: metric?.worstCaseVariationIae ?? Number.POSITIVE_INFINITY,
        gainMarginDb, phaseMarginDegrees,
      } };
    return { ...item, validation, assessment };
  });
  const ranking = rankQualifiedCandidates(evaluations.map((item) => item.assessment), objective);
  const recommendation = ranking.winner;
  if (!recommendation) throw new Error("No acceptable controller found.");
  const selectedEvaluation = evaluations.find((evaluation) => evaluation.candidate === recommendation)!;
  for (const item of ranking.ranked) {
    const evaluation = evaluations.find((candidate) => candidate.id === item.id)!;
    attempts.push({ id: item.id, method: item.method, structure: item.candidate.structure,
      status: item.feasible && Number.isFinite(item.score) ? "Feasible" : "Rejected",
      ...(evaluation.closedLoopTimeSeconds === undefined ? {} :
        { closedLoopTimeSeconds: rounded(evaluation.closedLoopTimeSeconds) }),
      ...(Number.isFinite(item.score) ? { score: rounded(item.score) } : {}),
      ...(!item.feasible ? { reason: failedRequirements(evaluation.validation.requirements) } : {}) });
  }
  const controller = materializeAutomaticTuneController(recommendation);
  const metrics = {
    finalError: requirementValue(selectedEvaluation.validation.requirements, "Steady-state error"),
    overshootPercent: requirementValue(selectedEvaluation.validation.requirements, "Overshoot"),
    settlingTimeSeconds: requirementValue(selectedEvaluation.validation.requirements, "Settling time"),
  };
  const plot = buildEngineeringPlotEvidence(model, current, recommendation);
  const selectionExplanation = "Structural simplicity is ranked before response, effort, and robustness scores.";
  return {
    applied: false,
    modelEvidence: "Exact simulation model",
    structure: recommendation.structure,
    method: selectedEvaluation.method,
    ...controller,
    validation: "Model-based validation · Pass",
    model: { kind: "first_order_dead_time", gain, timeConstantSeconds, deadTimeSeconds },
    metrics,
    rationale: `${recommendation.structure} from ${selectedEvaluation.method} is the simplest passing candidate. ${selectionExplanation}; ${attempts.filter((item) => item.status !== "Feasible").length} alternative(s) were retained with their rejection or availability reason.`,
    frequency: { points: plot.candidateOpenLoopBode, gainMarginDb: plot.gainMarginDb,
      phaseMarginDegrees: plot.phaseMarginDegrees },
    requirements: selectedEvaluation.validation.requirements,
    validationResult: selectedEvaluation.validation,
    evaluatedStructures: ["P", "PI", "PD", "PID"],
    parameters: { ...recommendation.parameters },
    attempts,
    ...(selectedEvaluation.closedLoopTimeSeconds === undefined ? {} :
      { closedLoopTimeSeconds: rounded(selectedEvaluation.closedLoopTimeSeconds) }),
  };
}

function requirementValue(requirements: readonly ValidationRequirement[], name: string): number {
  const value = requirements.find((requirement) => requirement.name === name)?.value;
  return typeof value === "number" ? value : Number.POSITIVE_INFINITY;
}

function failedRequirements(requirements: readonly ValidationRequirement[]): string {
  const failed = requirements.filter((item) => item.status === "Fail" || item.status === "Inconclusive")
    .map((item) => item.name);
  return failed.length ? `Failed: ${failed.join(", ")}.` : "Candidate metrics were unavailable.";
}

function record(value: unknown): Record<string, unknown> | undefined {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : undefined;
}

function positive(value: unknown, name: string): number {
  const number = Number(value);
  if (!Number.isFinite(number) || number <= 0) throw new Error(`Plant ${name} must be finite and greater than zero.`);
  return number;
}

function nonnegative(value: unknown, name: string): number {
  const number = Number(value ?? 0);
  if (!Number.isFinite(number) || number < 0) throw new Error(`Plant ${name} must be finite and non-negative.`);
  return number;
}

function rounded(value: number): number {
  return Number(value.toPrecision(8));
}
