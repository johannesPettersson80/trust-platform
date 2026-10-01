import type { GeneralModel } from "./generalModel";
import { analyzeGeneralModelFrequency } from "./generalModel";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { EngineerRequirements } from "./engineerVerificationPlan";
import { linearTransferModel } from "./linearTransferModel";
import { buildEngineeringPlotEvidence, engineeringPlotMessageEvidence } from "./engineeringPlots";
import { candidateFromControlPid, type CandidateSource, type ControlPidParameters } from "./controllerCandidate";
import type { IdentifiedModel } from "./identification";

/** Reuses the shared engineering-plot simulator for connected P/PI/PD/PID designs. */
export function selectedModelFrequencyAnalysis(model: GeneralModel): { model: GeneralModel; grid: { minimumFrequency: number; maximumFrequency: number; points: number }; result: Extract<ReturnType<typeof analyzeGeneralModelFrequency>, { status: "qualified" }> } | undefined {
  const maximumFrequency = model.kind === "discrete-state-space" ? Math.min(100, 0.8 * Math.PI / model.sampleTimeSeconds!) : 100;
  const grid = { minimumFrequency: Math.min(0.01, maximumFrequency / 100), maximumFrequency, points: 128 };
  const frequencies = Array.from({ length: grid.points }, (_, index) => grid.minimumFrequency *
    Math.pow(grid.maximumFrequency / grid.minimumFrequency, index / (grid.points - 1)));
  const result = analyzeGeneralModelFrequency(model, frequencies);
  return result.status === "qualified" ? { model, grid, result } : undefined;
}

type PredictedGains = { proportionalGain?: number; integralGain?: number; derivativeGain?: number; proportionalSetpointWeight?: number; derivativeSetpointWeight?: number; measurementFilterSeconds?: number };

/**
 * Predicts the tuned controller and, when its settings are known, the running controller under the same conditions.
 * With an input disturbance the setpoint stays at the operating point and the load acts from the start (the load trial).
 */
export function engineerPrediction(model: GeneralModel | undefined, loop: ControlLoopDefinition,
  design: PredictedGains, task?: EngineerRequirements, current?: PredictedGains,
  inputDisturbance?: number): ReturnType<typeof engineeringPlotMessageEvidence> | undefined {
  if (typeof design.proportionalGain !== "number" || !Number.isFinite(design.proportionalGain)) return undefined;
  const identified = identifiedPlotModel(model);
  if (!identified) return undefined;
  const simulate = (gains: PredictedGains, source: CandidateSource) => {
    const parameters: ControlPidParameters = {
      proportionalGain: gains.proportionalGain!,
      integralGain: gains.integralGain ?? 0,
      derivativeGain: gains.derivativeGain ?? 0,
      proportionalSetpointWeight: gains.proportionalSetpointWeight ?? 1, derivativeSetpointWeight: gains.derivativeSetpointWeight ?? 0, bias: 0,
      outputMinimum: loop.limits.outputMinimum, outputMaximum: loop.limits.outputMaximum,
      rateMinimum: -(loop.limits.rateDownPerS ?? 1e6), rateMaximum: loop.limits.rateUpPerS ?? 1e6,
      trackingTime: 1, antiWindup: "BackCalculation",
    };
    const candidate = candidateFromControlPid(parameters, source);
    // Each controller keeps its own measurement filter, so the two runs are simulated separately.
    return { candidate, plots: buildEngineeringPlotEvidence(identified, candidate, candidate, undefined, {
      cycleTimeSeconds: loop.controller.taskPeriodMs / 1000, durationSeconds: task?.durationSeconds,
      referenceChange: inputDisturbance === undefined ? task?.referenceChange : 0, measurementFilterSeconds: gains.measurementFilterSeconds,
      ...(inputDisturbance === undefined ? {} : { inputDisturbance }), operatingPoint: {
        input: model?.operatingPoint?.inputs[0] ?? 0, output: model?.operatingPoint?.outputs[0] ?? 0,
      },
    }) };
  };
  try {
    const { candidate, plots } = simulate(design, "Constrained comparison");
    const running = (() => {
      if (typeof current?.proportionalGain !== "number" || !Number.isFinite(current.proportionalGain)) return undefined;
      try { return simulate(current, "Imported").plots.timeResponse; } catch { return undefined; }
    })();
    plots.timeResponse = plots.timeResponse.map((point, index) => ({ timeSeconds: point.timeSeconds,
      candidatePv: point.candidatePv, candidateMv: point.candidateMv,
      ...(running ? { currentPv: running[index].candidatePv, currentMv: running[index].candidateMv } : {}) }));
    plots.requirements = []; // Actual task requirements are evaluated by the ST verification campaign.
    plots.comparison = { candidate, context: { modelRevision: model!.revision,
      evidenceRevision: JSON.stringify({ model, loop, task }), method: "unit-step-model-prediction-v1" } };
    return engineeringPlotMessageEvidence(plots);
  } catch {
    return undefined;
  }
}

function identifiedPlotModel(model: GeneralModel | undefined): IdentifiedModel | undefined {
  if (!model || model.inputs.length !== 1 || model.outputs.length !== 1) return undefined;
  try { model = linearTransferModel(model); } catch { return undefined; }
  if (model.kind !== "transfer-function") return undefined;
  const denominator = model.denominator;
  const numerator = model.numerator;
  if (denominator.length === 2 && numerator.length === 1 && denominator[1] !== 0) {
    return { kind: "first_order_dead_time", gain: numerator[0] / denominator[1],
      timeConstantSeconds: denominator[0] / denominator[1], deadTimeSeconds: model.delaySeconds,
      normalizedError: 0, stable: denominator[0] > 0 && denominator[1] > 0 };
  }
  if (denominator.length === 1 && numerator.length === 1) {
    return { kind: "static_gain", gain: numerator[0] / denominator[0], deadTimeSeconds: model.delaySeconds,
      normalizedError: 0, stable: true };
  }
  // n/(a·s) and n/(s·(a·s + b)): an integrator, alone or behind a lag, such as a shaft's angle driven by torque.
  if (numerator.length === 1 && denominator.length === 2 && denominator[1] === 0 && denominator[0] !== 0) {
    return { kind: "integrating", gain: numerator[0] / denominator[0], deadTimeSeconds: model.delaySeconds,
      normalizedError: 0, stable: true };
  }
  if (numerator.length === 1 && denominator.length === 3 && denominator[2] === 0 && denominator[1] !== 0) {
    return { kind: "integrating", gain: numerator[0] / denominator[1], timeConstantSeconds: denominator[0] / denominator[1],
      deadTimeSeconds: model.delaySeconds, normalizedError: 0, stable: true };
  }
  return undefined;
}
