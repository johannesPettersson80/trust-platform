import type { GeneralModel } from "./generalModel";
import { analyzeGeneralModelFrequency } from "./generalModel";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { EngineerRequirements } from "./engineerVerificationPlan";
import { linearTransferModel } from "./linearTransferModel";
import { buildEngineeringPlotEvidence, engineeringPlotMessageEvidence } from "./engineeringPlots";
import { candidateFromControlPid, type ControlPidParameters } from "./controllerCandidate";
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

export function engineerPrediction(model: GeneralModel | undefined, loop: ControlLoopDefinition,
  design: { proportionalGain?: number; integralGain?: number; derivativeGain?: number }, task?: EngineerRequirements): ReturnType<typeof engineeringPlotMessageEvidence> | undefined {
  if (typeof design.proportionalGain !== "number" || !Number.isFinite(design.proportionalGain)) return undefined;
  const identified = identifiedPlotModel(model);
  if (!identified) return undefined;
  const parameters: ControlPidParameters = {
    proportionalGain: design.proportionalGain,
    integralGain: design.integralGain ?? 0,
    derivativeGain: design.derivativeGain ?? 0,
    proportionalSetpointWeight: 1, derivativeSetpointWeight: 0, bias: 0,
    outputMinimum: loop.limits.outputMinimum, outputMaximum: loop.limits.outputMaximum,
    rateMinimum: -(loop.limits.rateDownPerS ?? 1e6), rateMaximum: loop.limits.rateUpPerS ?? 1e6,
    trackingTime: 1, antiWindup: "BackCalculation",
  };
  try {
    const candidate = candidateFromControlPid(parameters, "Constrained comparison");
    const plots = buildEngineeringPlotEvidence(identified, candidate, candidate, undefined, {
      cycleTimeSeconds: loop.controller.taskPeriodMs / 1000, durationSeconds: task?.durationSeconds,
      referenceChange: task?.referenceChange, operatingPoint: {
        input: model?.operatingPoint?.inputs[0] ?? 0, output: model?.operatingPoint?.outputs[0] ?? 0,
      },
    });
    plots.timeResponse = plots.timeResponse.map(point => ({ timeSeconds: point.timeSeconds,
      candidatePv: point.candidatePv, candidateMv: point.candidateMv }));
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
  return undefined;
}
