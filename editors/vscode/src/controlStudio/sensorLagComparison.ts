import { prepareBoundedStepReview } from "./boundedStepReview";
import { identifiedModelEquation, modelFitEvidence, type IdentifiedModel, type IdentificationReview, type IdentificationReviewInput, type ModelFitEvidence } from "./identification";
import { fitSensorLagStep, type SensorLagStepAnalysis } from "./sensorLagStepIdentification";
import { measureStepResponse, type StepResponseAnalysis } from "./stepResponseIdentification";

interface Assessment {
  maximumAbsoluteResidual: number;
  finalError: number;
  rootMeanSquareError: number;
  adequate: boolean;
}
export interface SensorLagComparison {
  candidateA: Assessment & { analysis: StepResponseAnalysis; model: IdentifiedModel; fit: ModelFitEvidence; equation: string };
  candidateB: Assessment & { analysis: SensorLagStepAnalysis; model: IdentifiedModel; fit: ModelFitEvidence; equation: string };
  requirements: { maximumAbsoluteResidual: number; maximumFinalError: number };
  rmsImprovement: number;
  recommended: "first_order" | "additional_lag" | "neither";
  physicalOrigin: "unresolved";
  provisional: true;
  choice?: StructureChoice;
}
export type StructureChoice = "first_order" | "additional_lag";

/** Compare structures on one retained estimation record without promoting design acceptance. */
export function prepareSensorLagComparison(input: IdentificationReviewInput): IdentificationReview {
  if (input.validation) throw new Error("Compare model structures on the retained estimation recording before independent validation.");
  const settings = input.stepResponseSettings!;
  const firstOrderSettings = input.firstOrderStepResponseSettings ?? settings;
  for (const key of ["baselineDurationSeconds", "finalDurationSeconds", "measurementBound", "maximumWindowRange",
    "maximumWindowSlopePerSecond", "timestampShiftSeconds"] as const) {
    if (settings[key] !== firstOrderSettings[key]) throw new Error("Recheck the first-order reference before comparing with changed windows or measurement assumptions.");
  }
  const reference = prepareBoundedStepReview({ ...input, requestedKind: "fopdt", stepResponseSettings: firstOrderSettings });
  if (reference.stage !== "validation_required") throw new Error("A structure comparison needs an estimation preview.");
  const analysis = fitSensorLagStep(reference.estimation, { ...reference.stepResponse!, ...measureStepResponse(reference.estimation, settings) });
  const { best, consistency } = analysis;
  const rms = Math.sqrt(best.squaredError / analysis.points.length);
  const normalizedError = rms / Math.abs(analysis.final.mean - analysis.baseline.mean);
  const model: IdentifiedModel = { kind: "second_order_dead_time", gain: best.gain,
    timeConstantSeconds: best.timeConstantSeconds, secondTimeConstantSeconds: best.secondTimeConstantSeconds,
    deadTimeSeconds: best.delaySeconds, stable: true, normalizedError, normalizedEstimationError: normalizedError,
    estimationResiduals: analysis.points.map(point => point.residual),
    ...(consistency.count ? { uncertainty: { method: "measurement-consistent-grid" as const,
      gain: { ...consistency.gain! }, timeConstantSeconds: { ...consistency.timeConstantSeconds! },
      secondTimeConstantSeconds: { ...consistency.secondTimeConstantSeconds! }, deadTimeSeconds: { ...consistency.delaySeconds! } } } : {}) };
  const measured = reference.estimation.filter(sample => sample.timeSeconds >= analysis.baseline.startSeconds && sample.timeSeconds < analysis.holdEndSeconds);
  const fit: ModelFitEvidence = { ...modelFitEvidence(model, measured),
    points: [...measured.filter(sample => sample.timeSeconds < analysis.stepTimeSeconds).map(sample => ({
      timeSeconds: sample.timeSeconds, measured: sample.output, predicted: best.baseline, residual: sample.output - best.baseline })), ...analysis.points],
    rootMeanSquareError: rms, warnings: analysis.warnings };
  const requirements = input.stepResponseSettings?.modelAdequacy ?? { maximumAbsoluteResidual: .2, maximumFinalError: .1 };
  const candidateA = { ...assess(reference.fit, analysis, requirements), analysis: reference.stepResponse!, model: reference.selected, fit: reference.fit, equation: identifiedModelEquation(reference.selected) };
  const candidateB = { ...assess(fit, analysis, requirements), analysis, model, fit, equation: identifiedModelEquation(model) };
  const rmsImprovement = candidateA.rootMeanSquareError - candidateB.rootMeanSquareError;
  const recommended = candidateA.adequate ? "first_order" : candidateB.adequate && rmsImprovement > analysis.settings.measurementBound ? "additional_lag" : "neither";
  const comparison: SensorLagComparison = { candidateA, candidateB, requirements: { ...requirements }, rmsImprovement,
    recommended, physicalOrigin: "unresolved", provisional: true };
  const useExtended = recommended === "additional_lag";
  const selected = useExtended ? model : reference.selected;
  return { ...reference, selected, preview: { ...reference.preview, recommended: selected, models: [reference.selected, model] },
    fit: { ...(useExtended ? fit : reference.fit), modelStructure: comparison } };
}

/** Retain an evidence-supported provisional choice without fitting or accepting a design. */
export function chooseSensorLagModel(review: IdentificationReview, choice: StructureChoice): IdentificationReview {
  const comparison = review.fit.modelStructure;
  if (review.stage !== "validation_required" || !comparison) throw new Error("Calculate a model-structure comparison before choosing a model.");
  if (!["first_order", "additional_lag"].includes(choice) || choice !== comparison.recommended) {
    throw new Error(comparison.recommended === "first_order" ? "The first-order model is adequate; the extra state is not supported."
      : "That structure does not meet the declared model-error and improvement requirements.");
  }
  const candidate = choice === "first_order" ? comparison.candidateA : comparison.candidateB;
  return { ...review, selected: candidate.model, fit: { ...candidate.fit, modelStructure: { ...comparison, choice } } };
}

function assess(fit: ModelFitEvidence, analysis: StepResponseAnalysis, requirements: SensorLagComparison["requirements"]): Assessment {
  const maximumAbsoluteResidual = Math.max(...fit.points.map(point => Math.abs(point.residual)));
  const final = fit.points.filter(point => point.timeSeconds >= analysis.final.startSeconds && point.timeSeconds < analysis.final.endSeconds);
  const finalError = Math.abs(final.reduce((sum, point) => sum + point.residual, 0) / final.length);
  return { maximumAbsoluteResidual, finalError,
    rootMeanSquareError: Math.sqrt(fit.points.reduce((sum, point) => sum + point.residual ** 2, 0) / fit.points.length),
    adequate: maximumAbsoluteResidual <= requirements.maximumAbsoluteResidual + 1e-9 && finalError <= requirements.maximumFinalError + 1e-9 };
}
