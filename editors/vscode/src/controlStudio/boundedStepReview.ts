import { analyzeStepResponse, measureStepResponse, type StepResponseAnalysis } from "./stepResponseIdentification";
import { modelFitEvidence, qualifyIdentificationData, type IdentificationReview, type IdentificationReviewInput,
  type IdentificationResult, type IdentifiedModel, type IdentificationSample, type ModelFitEvidence } from "./identification";

/** One explicit step method inside the shared estimation/validation state machine. */
export function prepareBoundedStepReview(input: IdentificationReviewInput): IdentificationReview {
  const settings = input.stepResponseSettings!;
  if (input.requestedKind !== "fopdt" || input.excluded.length) throw new Error("Bounded step fitting needs FOPDT and the complete recording.");
  const estimation = input.estimation.map(sample => ({ ...sample }));
  const analysis = analyzeStepResponse(estimation, settings);
  const { best, consistency } = analysis;
  const rms = Math.sqrt(best.squaredError / analysis.points.length);
  const response = Math.abs(analysis.final.mean - analysis.baseline.mean);
  if (!(response > 1e-9)) throw new Error("The measured step has no identifiable output response.");
  const selected: IdentifiedModel = { kind: "first_order_dead_time", gain: best.gain,
    timeConstantSeconds: best.timeConstantSeconds, deadTimeSeconds: best.delaySeconds,
    stable: true, normalizedError: rms / response, normalizedEstimationError: rms / response,
    estimationResiduals: analysis.points.map(point => point.residual),
    ...(consistency.count && consistency.gain ? { uncertainty: { method: "measurement-consistent-grid" as const,
      gain: { ...consistency.gain }, timeConstantSeconds: { ...consistency.timeConstantSeconds! },
      deadTimeSeconds: { ...consistency.delaySeconds! } } } : {}) };
  const source = input.source === "Physical" ? "Identified from physical data" : "Identified from simulation data";
  const result: IdentificationResult = { source, recommended: selected, models: [selected], partition: "single-run" };
  const estimationQualification = { ...qualifyIdentificationData(estimation, []), stepResponse: measureStepResponse(estimation, settings) };
  const measured = estimation.filter(sample => sample.timeSeconds >= analysis.baseline.startSeconds && sample.timeSeconds < analysis.holdEndSeconds);
  const fit: ModelFitEvidence = { ...modelFitEvidence(selected, measured), stepResponse: analysis,
    points: [...measured.filter(sample => sample.timeSeconds < analysis.stepTimeSeconds).map(sample => ({
      timeSeconds: sample.timeSeconds, measured: sample.output, predicted: analysis.baseline.mean, residual: sample.output - analysis.baseline.mean })),
    ...analysis.points.map(point => ({ ...point }))], rootMeanSquareError: rms, warnings: [...analysis.warnings] };
  if (!input.validation) return { stage: "validation_required", estimationQualification,
    preview: result, selected, fit, estimation, stepResponse: analysis, accepted: false };
  if (!input.estimationRecordingId || !input.validationRecordingId || input.estimationRecordingId === input.validationRecordingId) {
    throw new Error("Bounded step validation requires two distinct immutable recording identities.");
  }
  if (!consistency.count || analysis.boundaryLimited || settings.timestampShiftSeconds !== 0) {
    throw new Error("Resolve the estimation grid, search boundary and nominal timestamp assumptions before accepting a model. The estimation record is retained.");
  }
  if (analysis.points.some(point => Math.abs(point.residual) > settings.measurementBound + 1e-9)) {
    throw new Error("The best estimation fit exceeds the measurement bound; retain the consistent set and refine the analysis before acceptance.");
  }
  const validation = input.validation.map(sample => ({ ...sample }));
  const validationStep = measureStepResponse(validation, settings);
  if (Math.abs(validationStep.inputChange - analysis.inputChange) <= 1e-9) {
    throw new Error("Use a distinct validation step with a different applied amplitude.");
  }
  const validationFit = scoreFrozenStep(selected, validation, validationStep);
  const exceedsMeasurementBound = validationFit.points.some(point => Math.abs(point.residual) > settings.measurementBound + 1e-9);
  const validationQualification = { ...qualifyIdentificationData(validation, [], true), stepResponse: validationStep,
    ...(exceedsMeasurementBound ? { suitable: false, suitableForDesign: false,
      reason: "The frozen estimation model fails the independent measurement bound. Retain both recordings and revise the model or stated assumptions." } : {}) };
  selected.normalizedValidationError = validationFit.rootMeanSquareError / Math.abs(validationStep.final.mean - validationStep.baseline.mean);
  selected.validationResiduals = validationFit.points.map(point => point.residual);
  return { stage: "acceptance_required", estimationQualification,
    validationQualification,
    result: { ...result, partition: "independent-runs" }, selected,
    fit: { ...validationFit, stepResponse: analysis, stepResponseMeasurement: validationStep, warnings: [...analysis.warnings] },
    estimation, validation, stepResponse: analysis, accepted: false };
}

function scoreFrozenStep(model: IdentifiedModel, samples: readonly IdentificationSample[], measured: ReturnType<typeof measureStepResponse>): ModelFitEvidence {
  const held = samples.filter(sample => sample.timeSeconds >= measured.baseline.startSeconds && sample.timeSeconds < measured.holdEndSeconds);
  const evidence = modelFitEvidence(model, held);
  const points = held.filter(sample => sample.timeSeconds >= measured.stepTimeSeconds).map(sample => {
    const elapsed = sample.timeSeconds - measured.stepTimeSeconds - model.deadTimeSeconds!;
    const predicted = measured.baseline.mean + (elapsed < 0 ? 0 : model.gain * measured.inputChange * (1 - Math.exp(-elapsed / model.timeConstantSeconds!)));
    return { timeSeconds: sample.timeSeconds, measured: sample.output, predicted, residual: sample.output - predicted };
  });
  return { ...evidence, points, rootMeanSquareError: Math.sqrt(points.reduce((sum, point) => sum + point.residual ** 2, 0) / points.length) };
}
