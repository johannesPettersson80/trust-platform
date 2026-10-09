import {
  identifyLowOrderModels,
  type LowOrderModelFit,
  type ParameterInterval,
} from "./numerics";
import { measureStepResponse, type StepResponseAnalysis, type StepResponseSettings } from "./stepResponseIdentification";
import { prepareBoundedStepReview } from "./boundedStepReview";
import { prepareSensorLagComparison, type SensorLagComparison } from "./sensorLagComparison";
import type { IdentificationExperiment } from "./identificationExperiment";
import type { IdentificationRecording } from "./identificationState";
import { blockAveraged, MAXIMUM_FIT_SAMPLES, medianSpacing } from "./evenSampling";
import {
  baselineNoiseRms, heldUnitResponse, leastSquaresGain, MINIMUM_REACHED_FRACTION, noiseRefusal, partialResponseRefusal,
  reachedFraction, type HeldShape,
} from "./numerics/identification";

export type ModelKind = "static_gain" | "integrating" | "first_order" | "first_order_dead_time" | "second_order" | "second_order_dead_time";

export interface IdentificationSample {
  timeSeconds: number;
  input: number;
  output: number;
  quality: "good" | "uncertain" | "bad";
  inputUnit?: string;
  outputUnit?: string;
  clipped?: boolean;
  saturated?: boolean;
}

export interface ExcludedInterval { startSeconds: number; endSeconds: number }

export interface QualifiedIdentificationData {
  stepResponse?: ReturnType<typeof measureStepResponse>;
  rawSamples: readonly IdentificationSample[];
  samples: readonly IdentificationSample[];
  suitable: boolean;
  reason?: string;
  inputUnit: string;
  outputUnit: string;
  sampleTimeSeconds: number;
  suitableForDesign: boolean;
  metrics: IdentificationQualificationMetrics;
}

export interface IdentificationQualificationMetrics {
  rawSampleCount: number;
  retainedSampleCount: number;
  durationSeconds: number;
  medianSampleTimeSeconds: number;
  maximumSampleTimeDeviationPercent: number;
  gapCount: number;
  rejectedQualitySampleCount: number;
  clippedSampleCount: number;
  saturationSampleCount: number;
  baselineDriftPerSecond: number;
  inputSpan: number;
  outputSpan: number;
  signalToNoiseDb: number;
  excitation: "step" | "insufficient";
  coherence: "Not evaluated for time-domain excitation";
  independentValidationAvailable: boolean;
}

export interface IdentifiedModel {
  kind: ModelKind;
  gain: number;
  timeConstantSeconds?: number;
  secondTimeConstantSeconds?: number;
  deadTimeSeconds?: number;
  normalizedError: number;
  normalizedEstimationError?: number;
  normalizedValidationError?: number;
  estimationResiduals?: readonly number[];
  validationResiduals?: readonly number[];
  uncertainty?: {
    method: "grid-resolution-and-validation-residual" | "sample-resolution-envelope" | "measurement-consistent-grid";
    gain: ParameterInterval;
    timeConstantSeconds?: ParameterInterval;
    secondTimeConstantSeconds?: ParameterInterval;
    deadTimeSeconds?: ParameterInterval;
  };
  stable: boolean;
}

export interface IdentificationResult {
  source: "Identified from simulation data" | "Identified from physical data";
  recommended: IdentifiedModel;
  models: readonly IdentifiedModel[];
  partition?: "single-run" | "independent-runs";
  /** The physical refusal of the recommended form: a response too small against the noise, or one that has not reached 63 % of its
   * fitted final change (A10.3). It is reported, never replaced by a state model that could pass it. */
  refusal?: string;
  /** The integrator rule's refusal of an integrating form that the data cannot yet tell apart from a self-regulating form. */
  integratorRefusal?: string;
}

export interface ModelFitPoint {
  timeSeconds: number;
  measured: number;
  predicted: number;
  residual: number;
}

export interface ModelFitEvidence {
  modelStructure?: SensorLagComparison;
  stepResponse?: StepResponseAnalysis;
  stepResponseMeasurement?: ReturnType<typeof measureStepResponse>;
  points: readonly ModelFitPoint[];
  rootMeanSquareError: number;
  outputUnit: string;
  stable: boolean;
  delaySeconds: number;
  operatingRange: { inputMinimum: number; inputMaximum: number; outputMinimum: number; outputMaximum: number };
  warnings: readonly string[];
  /** Fraction of the fitted final change that a held step reached (A10.3); undefined where the fraction is not defined. */
  finalChangeFraction?: number;
}

export interface IdentifiedModelPresentation {
  experiment?: IdentificationExperiment;
  modelStructure?: SensorLagComparison;
  stepResponse?: ReturnType<typeof measureStepResponse>;
  outputId?: string;
  inputChannels?: readonly { id: string; unit: string }[];
  outputFits?: readonly {
    id: string; unit: string; points: ModelFitEvidence["points"];
    normalizedError?: number; residualLagOneCorrelation?: number;
    validityRange: ModelFitEvidence["operatingRange"];
  }[];
  poleZeroPlane?: {
    domain: "s" | "z";
    poles?: readonly { real: number; imaginary: number }[];
    zeros?: readonly { real: number; imaginary: number }[];
    scope: string;
  };
  equation: string;
  units: { input: string; output: string; gain: string; time: "s" };
  parameters: readonly { name: string; value: number; unit: string }[];
  poles: readonly number[];
  zeros: readonly number[];
  delaySeconds: number;
  uncertainty: { label: string; intervals?: IdentifiedModel["uncertainty"] };
  estimation: { normalizedError?: number; residualLagOneCorrelation?: number };
  validation: { normalizedError?: number; residualLagOneCorrelation?: number };
  predictedResponse: ModelFitEvidence["points"];
  validityRange: ModelFitEvidence["operatingRange"];
  warnings: readonly string[];
}

export type IdentificationModelSelection = "automatic" | "integrating" | "first_order" |
  "fopdt" | "second_order" | "sopdt";

export interface IdentificationReviewInput {
  recording?: IdentificationRecording;
  stepResponseSettings?: StepResponseSettings;
  /** Original first-order search on this same immutable estimation record. */
  firstOrderStepResponseSettings?: StepResponseSettings;
  maximumNormalizedError?: number;
  estimation: readonly IdentificationSample[];
  validation?: readonly IdentificationSample[];
  /** Immutable recording provenance distinguishes deterministic independent trials. */
  estimationRecordingId?: string;
  validationRecordingId?: string;
  excluded: readonly ExcludedInterval[];
  source: "Simulation" | "Physical";
  requestedKind: IdentificationModelSelection;
  /** The process the loop's identification policy declares; "integrating" accepts an integrating form directly. */
  declaredProcess?: string;
  /** A closed-loop fit (a setpoint step in Auto) is not subject to the integrator rule. */
  closedLoop?: boolean;
}

export type IdentificationReview =
  | {
      stage: "validation_required";
      stepResponse?: StepResponseAnalysis;
      estimationQualification: QualifiedIdentificationData;
      preview: IdentificationResult;
      selected: IdentifiedModel;
      fit: ModelFitEvidence;
      estimation: readonly IdentificationSample[];
      accepted: false;
    }
  | {
      stage: "acceptance_required";
      stepResponse?: StepResponseAnalysis;
      estimationQualification: QualifiedIdentificationData;
      validationQualification: QualifiedIdentificationData;
      result: IdentificationResult;
      selected: IdentifiedModel;
      fit: ModelFitEvidence;
      estimation: readonly IdentificationSample[];
      validation: readonly IdentificationSample[];
      accepted: false;
    };

export interface AcceptedIdentificationModel {
  model: IdentifiedModel;
  fit: ModelFitEvidence;
  evidence: IdentificationResult["source"];
  review: Extract<IdentificationReview, { stage: "acceptance_required" }>;
}

export { IdentificationWorkflow } from "./identificationWorkflow";

/** Builds the same-run preview or the independently validated model review. */
export function prepareIdentificationReview(input: IdentificationReviewInput): IdentificationReview {
  if (input.stepResponseSettings && input.requestedKind === "sopdt") return prepareSensorLagComparison(input);
  if (input.stepResponseSettings) return prepareBoundedStepReview(input);
  const estimation = input.estimation.map((sample) => ({ ...sample }));
  const estimationQualification = qualifyIdentificationData(estimation, input.excluded);
  if (!estimationQualification.suitable) throw new Error(estimationQualification.reason);
  if (input.validation === undefined) {
    const preview = identifyPlant(estimation, input.excluded, input.source, undefined, input.maximumNormalizedError, identifyOptions(input));
    const selected = selectRequestedModel(preview, input.requestedKind);
    refuseRefusedPreview(preview, selected);
    return {
      stage: "validation_required",
      estimationQualification,
      preview,
      selected,
      fit: modelFitEvidence(selected, estimationQualification.samples),
      estimation,
      accepted: false,
    };
  }
  const validation = input.validation.map((sample) => ({ ...sample }));
  if (sameRecording(estimation, validation) && !distinctRecordingIds(input)) {
    throw new Error("A distinct immutable validation run is required before controller design.");
  }
  const validationQualification = qualifyIdentificationData(validation, [], true);
  if (!validationQualification.suitable) {
    throw new Error(`Independent validation: ${validationQualification.reason}`);
  }
  const result = identifyPlant(estimation, input.excluded, input.source, validation, input.maximumNormalizedError, identifyOptions(input));
  const selected = selectRequestedModel(result, input.requestedKind);
  return {
    stage: "acceptance_required",
    estimationQualification,
    validationQualification,
    result,
    selected,
    fit: modelFitEvidence(selected, validationQualification.samples),
    estimation,
    validation,
    accepted: false,
  };
}

function distinctRecordingIds(input: IdentificationReviewInput): boolean {
  return Boolean(input.estimationRecordingId && input.validationRecordingId &&
    input.estimationRecordingId !== input.validationRecordingId);
}

function selectRequestedModel(
  result: IdentificationResult,
  requested: IdentificationModelSelection
): IdentifiedModel {
  if (requested === "automatic") return result.recommended;
  const kind = ({ integrating: "integrating", first_order: "first_order",
    fopdt: "first_order_dead_time", second_order: "second_order",
    sopdt: "second_order_dead_time" } as const)[requested];
  const selected = result.models.find((model) => model.kind === kind);
  if (!selected) throw new Error(`The ${requested} fitter is not available for this recording.`);
  if (selected.normalizedValidationError !== undefined
      && result.recommended.normalizedValidationError !== undefined) {
    const adequacyLimit = Math.min(
      0.35,
      Math.max(0.03, result.recommended.normalizedValidationError * 1.25)
    );
    if (!Number.isFinite(selected.normalizedValidationError)
        || selected.normalizedValidationError > adequacyLimit) {
      throw new Error(
        `The requested ${requested} model failed independent validation; choose another order or Automatic.`
      );
    }
  }
  return selected;
}

/** A recording as a fit receives it: at most MAXIMUM_FIT_SAMPLES points, with no block straddling the largest input change. */
function fitRecording(samples: readonly IdentificationSample[]): IdentificationSample[] {
  const stepIndex = largestInputChange(samples);
  return blockAveraged(samples, MAXIMUM_FIT_SAMPLES, stepIndex >= 1 ? [samples[stepIndex].timeSeconds] : []);
}

function heldStepSamples(samples: readonly IdentificationSample[]): IdentificationSample[] {
  const stepIndex = largestInputChange(samples);
  if (stepIndex < 1) return [...samples];
  const magnitude = Math.abs(samples[stepIndex].input - samples[stepIndex - 1].input);
  const direction = Math.sign(samples[stepIndex].input - samples[stepIndex - 1].input);
  const restorationIndex = samples.findIndex((sample, index) => index > stepIndex &&
    (sample.input - samples[index - 1].input) * direction < 0 &&
    Math.abs(sample.input - samples[index - 1].input) >= magnitude * 0.5);
  return restorationIndex > stepIndex ? samples.slice(0, restorationIndex).map(sample => ({ ...sample })) : [...samples];
}

function sameRecording(
  left: readonly IdentificationSample[],
  right: readonly IdentificationSample[]
): boolean {
  return left.length === right.length && left.every((sample, index) => {
    const other = right[index];
    return sample.timeSeconds === other.timeSeconds && sample.input === other.input &&
      sample.output === other.output && sample.quality === other.quality;
  });
}

/** Measured pre-excitation coordinates, shared by fit evidence and selected models. */
export function identificationOperatingPoint(samples: readonly IdentificationSample[]): {
  stepIndex: number; input0: number; output0: number;
} {
  if (samples.length < 2 || !samples.every(finiteSample)) {
    throw new Error("Model evidence requires finite measured samples.");
  }
  const stepIndex = largestInputChange(samples);
  if (stepIndex < 1) throw new Error("Model evidence requires a measured input change.");
  return { stepIndex,
    input0: mean(samples.slice(0, stepIndex).map(sample => sample.input)),
    output0: mean(samples.slice(0, stepIndex).map(sample => sample.output)) };
}

export function modelFitEvidence(
  model: IdentifiedModel,
  samples: readonly IdentificationSample[]
): ModelFitEvidence {
  const { stepIndex, input0, output0 } = identificationOperatingPoint(samples);
  const stepTime = samples[stepIndex].timeSeconds;
  const inputChanges = samples.slice(stepIndex).map((sample, offset) => ({
    timeSeconds: sample.timeSeconds,
    delta: sample.input - (samples[stepIndex + offset - 1]?.input ?? input0),
  })).filter(change => change.delta !== 0);
  const points = samples.map((sample) => {
    const predicted = sample.timeSeconds < stepTime
      ? output0
      : output0 + model.gain * inputChanges.reduce((response, change) => {
        const time = sample.timeSeconds - change.timeSeconds - (model.deadTimeSeconds ?? 0);
        return time < 0 ? response : response + change.delta * responseFraction(model, time);
      }, 0);
    return { timeSeconds: sample.timeSeconds, measured: sample.output, predicted,
      residual: sample.output - predicted };
  });
  const rootMeanSquareError = Math.sqrt(mean(points.map((point) => point.residual ** 2)));
  const warnings: string[] = [];
  if (!model.stable) warnings.push("The fitted model is not stable.");
  if (model.normalizedError > 0.2) warnings.push("Prediction error is high for controller design.");
  const evidence = heldStepEvidence(model, samples);
  if (evidence?.fraction !== undefined) warnings.push(`The response reached ${Math.round(evidence.fraction * 100)} % of its fitted final change.`);
  if ((model.deadTimeSeconds ?? 0) > 0 && (model.deadTimeSeconds ?? 0) < medianIntervals(samples)) {
    warnings.push("Dead time is shorter than one sample interval and is weakly resolved.");
  }
  return {
    points,
    rootMeanSquareError,
    outputUnit: samples.find((sample) => sample.outputUnit)?.outputUnit ?? "unit",
    stable: model.stable,
    finalChangeFraction: evidence?.fraction,
    delaySeconds: model.deadTimeSeconds ?? 0,
    operatingRange: {
      inputMinimum: Math.min(...samples.map((sample) => sample.input)),
      inputMaximum: Math.max(...samples.map((sample) => sample.input)),
      outputMinimum: Math.min(...samples.map((sample) => sample.output)),
      outputMaximum: Math.max(...samples.map((sample) => sample.output)),
    },
    warnings,
  };
}

/** Produces review evidence without implying statistical confidence not established by the fitter. */
export function identifiedModelPresentation(
  model: IdentifiedModel,
  fit: ModelFitEvidence,
  inputUnit: string,
  outputUnit: string
): IdentifiedModelPresentation {
  const gainUnit = `${outputUnit}/${inputUnit}`;
  const timeConstants = [model.timeConstantSeconds, model.secondTimeConstantSeconds]
    .filter((value): value is number => typeof value === "number");
  const poles = model.kind === "integrating" ? [0] : timeConstants.map((value) => -1 / value);
  const delay = model.deadTimeSeconds ?? 0;
  const equation = identifiedModelEquation(model);
  return {
    equation,
    units: { input: inputUnit, output: outputUnit, gain: gainUnit, time: "s" },
    parameters: [
      { name: "Gain", value: model.gain, unit: gainUnit },
      ...timeConstants.map((value, index) => ({ name: `Time constant ${index + 1}`, value, unit: "s" })),
      { name: "Dead time", value: delay, unit: "s" },
    ],
    poles,
    zeros: [],
    delaySeconds: delay,
    uncertainty: {
      label: model.uncertainty?.method === "measurement-consistent-grid"
        ? "Measurement-consistent parameter grid; marginal ranges, not statistical confidence"
        : model.uncertainty?.method === "sample-resolution-envelope"
        ? "Sample-resolution envelope from the estimation record; not statistical confidence"
        : "Conservative search-resolution and held-out-residual interval",
      ...(model.uncertainty ? { intervals: model.uncertainty } : {}),
    },
    estimation: {
      normalizedError: model.normalizedEstimationError ?? model.normalizedError,
      ...(model.estimationResiduals ? { residualLagOneCorrelation: lagOneCorrelation(model.estimationResiduals) } : {}),
    },
    validation: {
      ...(model.normalizedValidationError === undefined ? {} : { normalizedError: model.normalizedValidationError }),
      ...(model.validationResiduals ? { residualLagOneCorrelation: lagOneCorrelation(model.validationResiduals) } : {}),
    },
    predictedResponse: fit.points.map((point) => ({ ...point })),
    ...(fit.modelStructure ? { modelStructure: structuredClone(fit.modelStructure) } : {}),
    ...(fit.stepResponse ? { stepResponse: fit.stepResponseMeasurement ?? fit.stepResponse } : {}),
    validityRange: { ...fit.operatingRange },
    warnings: [...fit.warnings],
  };
}

export function identifiedModelEquation(model: IdentifiedModel): string {
  const timeConstants = [model.timeConstantSeconds, model.secondTimeConstantSeconds].filter((value): value is number => typeof value === "number");
  const denominator = timeConstants.map(value => `(${engineeringCoefficient(value)}s + 1)`).join("");
  const delay = model.deadTimeSeconds ?? 0, delayTerm = delay > 0 ? ` e^(-${engineeringCoefficient(delay)}s)` : "";
  return model.kind === "integrating" ? `G(s) = ${engineeringCoefficient(model.gain)}${delayTerm} / s`
    : denominator ? `G(s) = ${engineeringCoefficient(model.gain)}${delayTerm} / ${denominator}` : `G(s) = ${engineeringCoefficient(model.gain)}`;
}

function engineeringCoefficient(value: number): string {
  return Number(value.toPrecision(6)).toString();
}

export function lagOneCorrelation(values: readonly number[]): number {
  if (values.length < 3 || !values.every(Number.isFinite)) return 0;
  const left = values.slice(0, -1);
  const right = values.slice(1);
  const leftMean = mean(left);
  const rightMean = mean(right);
  const numerator = left.reduce((sum, value, index) => sum + (value - leftMean) * (right[index] - rightMean), 0);
  const leftEnergy = left.reduce((sum, value) => sum + (value - leftMean) ** 2, 0);
  const rightEnergy = right.reduce((sum, value) => sum + (value - rightMean) ** 2, 0);
  const denominator = Math.sqrt(leftEnergy * rightEnergy);
  return denominator > 0 ? numerator / denominator : 0;
}

export function qualifyIdentificationData(
  samples: readonly IdentificationSample[],
  excluded: readonly ExcludedInterval[],
  independentValidationAvailable = false
): QualifiedIdentificationData {
  const rawSamples = samples.map((sample) => ({ ...sample }));
  const usable = rawSamples.filter((sample) =>
    sample.quality === "good" && finiteSample(sample) &&
    !excluded.some((interval) => sample.timeSeconds >= interval.startSeconds && sample.timeSeconds <= interval.endSeconds));
  const sampleTimeSeconds = medianIntervals(usable);
  const metrics = qualificationMetrics(rawSamples, usable, sampleTimeSeconds, independentValidationAvailable);
  const base = {
    rawSamples,
    samples: usable,
    inputUnit: rawSamples.find((sample) => sample.inputUnit)?.inputUnit ?? "unit",
    outputUnit: rawSamples.find((sample) => sample.outputUnit)?.outputUnit ?? "unit",
    sampleTimeSeconds,
    suitableForDesign: false,
    metrics,
  };
  if (usable.length < 20) return { ...base, suitable: false, reason: "At least 20 finite good samples are required." };
  if (!strictlyIncreasing(usable)) return { ...base, suitable: false, reason: "Sample times must be finite and strictly increasing." };
  if (span(usable.map((sample) => sample.input)) <= 1e-9) {
    return { ...base, suitable: false, reason: "Input excitation is too small to identify a model." };
  }
  if (span(usable.map((sample) => sample.output)) <= 1e-9) {
    return { ...base, suitable: false, reason: "Output response is too small to identify a model." };
  }
  return { ...base, suitable: true, suitableForDesign: independentValidationAvailable };
}

function qualificationMetrics(
  raw: readonly IdentificationSample[],
  usable: readonly IdentificationSample[],
  sampleTimeSeconds: number,
  independentValidationAvailable: boolean
): IdentificationQualificationMetrics {
  const intervals = usable.slice(1).map((sample, index) => sample.timeSeconds - usable[index].timeSeconds)
    .filter((value) => Number.isFinite(value) && value > 0);
  const maximumDeviation = sampleTimeSeconds > 0 && intervals.length > 0
    ? Math.max(...intervals.map((value) => Math.abs(value - sampleTimeSeconds))) / sampleTimeSeconds * 100
    : 0;
  const stepIndex = largestInputChange(usable);
  const baseline = stepIndex > 1 ? usable.slice(0, stepIndex) : usable.slice(0, Math.max(2, Math.floor(usable.length / 5)));
  const baselineDuration = baseline.length > 1
    ? baseline[baseline.length - 1].timeSeconds - baseline[0].timeSeconds : 0;
  const baselineDrift = baselineDuration > 0
    ? (baseline[baseline.length - 1].output - baseline[0].output) / baselineDuration : 0;
  const baselineDifferences = baseline.slice(1).map((sample, index) => sample.output - baseline[index].output);
  const noiseRms = baselineDifferences.length > 0
    ? Math.sqrt(mean(baselineDifferences.map((value) => value * value)) / 2) : 0;
  const outputSpan = safeSpan(usable.map((sample) => sample.output));
  const signalToNoiseDb = noiseRms > 1e-12 && outputSpan > 0
    ? 20 * Math.log10(outputSpan / noiseRms) : outputSpan > 0 ? 300 : 0;
  return {
    rawSampleCount: raw.length,
    retainedSampleCount: usable.length,
    durationSeconds: usable.length > 1 ? usable[usable.length - 1].timeSeconds - usable[0].timeSeconds : 0,
    medianSampleTimeSeconds: sampleTimeSeconds,
    maximumSampleTimeDeviationPercent: maximumDeviation,
    gapCount: sampleTimeSeconds > 0 ? intervals.filter((value) => value > sampleTimeSeconds * 1.5).length : 0,
    rejectedQualitySampleCount: raw.filter((sample) => sample.quality !== "good").length,
    clippedSampleCount: raw.filter((sample) => sample.clipped === true).length,
    saturationSampleCount: raw.filter((sample) => sample.saturated === true).length,
    baselineDriftPerSecond: baselineDrift,
    inputSpan: safeSpan(usable.map((sample) => sample.input)),
    outputSpan,
    signalToNoiseDb,
    excitation: stepIndex > 0 ? "step" : "insufficient",
    coherence: "Not evaluated for time-domain excitation",
    independentValidationAvailable,
  };
}

export function identifyPlant(
  samples: readonly IdentificationSample[],
  excluded: readonly ExcludedInterval[] = [],
  source: "Simulation" | "Physical" = "Simulation",
  validationSamples?: readonly IdentificationSample[],
  maximumNormalizedError = 0.35,
  options: IdentifyPlantOptions = {}
): IdentificationResult {
  const qualified = qualifyIdentificationData(samples, excluded);
  if (!qualified.suitable) throw new Error(qualified.reason);
  if (validationSamples !== undefined) {
    const validation = qualifyIdentificationData(validationSamples, []);
    if (!validation.suitable) throw new Error(`Independent validation: ${validation.reason}`);
    const fitted = identifyLowOrderModels(fitRecording(heldStepSamples(qualified.samples)), fitRecording(heldStepSamples(validation.samples)), {
      allowedKinds: ["static_gain", "integrating", "first_order", "first_order_dead_time",
        "second_order", "second_order_dead_time"],
      absoluteTolerance: 1e-9,
      minimumSamples: 20,
      minimumInputSpan: 1e-9,
      maximumNormalizedValidationError: maximumNormalizedError,
    });
    if (fitted.status === "rejected") {
      throw new Error(`No qualified low-order model: ${fitted.reason}.`);
    }
    const recommended = lowOrderModel(fitted.recommended);
    const estimationValues = fitRecording(heldStepSamples(qualified.samples));
    const refusal = unreliableStepRefusal(recommended, qualified.samples, estimationValues);
    const integratorRefusal = integratorRefusalFor(options, estimationValues, maximumNormalizedError);
    return {
      source: source === "Physical" ? "Identified from physical data" : "Identified from simulation data",
      recommended,
      models: fitted.models.map(lowOrderModel),
      partition: "independent-runs",
      ...(refusal === undefined ? {} : { refusal }),
      ...(integratorRefusal === undefined ? {} : { integratorRefusal }),
    };
  }
  const held = heldStepSamples(qualified.samples);
  const values = fitRecording(held);
  const stepIndex = largestInputChange(values);
  if (stepIndex < 2 || stepIndex >= values.length - 10) throw new Error("A bounded input step with pre- and post-step data is required.");
  const before = values.slice(0, stepIndex);
  const after = values.slice(Math.max(stepIndex + 1, values.length - Math.max(5, Math.floor(values.length / 10))));
  const input0 = mean(before.map((sample) => sample.input));
  const output0 = mean(before.map((sample) => sample.output));
  const input1 = mean(after.map((sample) => sample.input));
  const output1 = mean(after.map((sample) => sample.output));
  const inputChange = input1 - input0;
  const gain = (output1 - output0) / inputChange;
  if (!Number.isFinite(gain) || Math.abs(gain) < 1e-9) throw new Error("The measured input/output gain is not identifiable.");
  const stepTime = values[stepIndex].timeSeconds;
  const response = output1 - output0;
  const delayTime = firstCrossing(values, stepIndex, output0 + response * 0.02) - stepTime;
  const t63 = firstCrossing(values, stepIndex, output0 + response * 0.632) - stepTime;
  // Block averaging, when it applies, widens the sample spacing the crossings and time bounds are measured on.
  const dt = values.length === held.length ? qualified.sampleTimeSeconds : medianSpacing(values.map((sample) => sample.timeSeconds));
  const deadTime = Math.max(0, delayTime - dt);
  const tau = Math.max(dt, t63 - deadTime);
  const horizon = values[values.length - 1].timeSeconds - stepTime;
  // Each candidate's gain is the least-squares value for its own dynamics on the recorded instants (N2.2); the final window is only a check.
  const models: IdentifiedModel[] = [
    candidate("integrating", 1, undefined, undefined, rampDelay(values, stepIndex, output0)),
    candidate("first_order", 1, tau + deadTime, undefined, 0),
    candidate("first_order_dead_time", 1, tau, undefined, deadTime),
    candidate("second_order", 1, Math.max(dt, (tau + deadTime) / 2.146), Math.max(dt, (tau + deadTime) / 2.146), 0),
    candidate("second_order_dead_time", 1, Math.max(dt, tau / 2.146), Math.max(dt, tau / 2.146), deadTime),
  ];
  for (const model of models) {
    model.gain = jointGain(model, values, stepIndex, input0, output0);
    model.normalizedError = modelError(model, values, stepIndex, input0, output0, inputChange, Math.abs(response));
  }
  const beforeMinimum = Math.min(...before.map((sample) => sample.output));
  const beforeMaximum = Math.max(...before.map((sample) => sample.output));
  const afterMinimum = Math.min(...after.map((sample) => sample.output));
  const afterMaximum = Math.max(...after.map((sample) => sample.output));
  const endpointGains = [
    (afterMinimum - beforeMaximum) / inputChange,
    (afterMaximum - beforeMinimum) / inputChange,
  ];
  const gainMinimum = Math.min(...endpointGains, gain);
  const gainMaximum = Math.max(...endpointGains, gain);
  for (const model of models) {
    // One estimation run cannot establish statistical confidence. Preserve
    // only endpoint and sample-crossing resolution until validation exists.
    model.uncertainty = {
      method: "sample-resolution-envelope",
      gain: {
        minimum: Math.min(gainMinimum, model.gain),
        maximum: Math.max(gainMaximum, model.gain),
      },
      ...(model.timeConstantSeconds === undefined ? {} : {
        timeConstantSeconds: {
          minimum: Math.max(dt, model.timeConstantSeconds - dt),
          maximum: model.timeConstantSeconds + dt,
        },
      }),
      ...(model.secondTimeConstantSeconds === undefined ? {} : {
        secondTimeConstantSeconds: {
          minimum: Math.max(dt, model.secondTimeConstantSeconds - dt),
          maximum: model.secondTimeConstantSeconds + dt,
        },
      }),
      ...(model.deadTimeSeconds === undefined ? {} : {
        deadTimeSeconds: {
          minimum: Math.max(0, model.deadTimeSeconds),
          maximum: model.deadTimeSeconds + dt,
        },
      }),
    };
  }
  const recommended = selectLowOrderModel(models, dt, deadTime);
  if (horizon <= dt) {
    throw new Error("No acceptable stable low-order model could be identified from the retained data.");
  }
  const refusal = unreliableStepRefusal(recommended, held, values);
  const integratorRefusal = integratorRefusalFor(options, values, maximumNormalizedError);
  return { source: source === "Physical" ? "Identified from physical data" : "Identified from simulation data",
    recommended: { ...recommended }, models: models.map((model) => ({ ...model })), partition: "single-run",
    ...(refusal === undefined ? {} : { refusal }), ...(integratorRefusal === undefined ? {} : { integratorRefusal }) };
}

export function selectLowOrderModel(
  models: readonly IdentifiedModel[],
  sampleTimeSeconds: number,
  resolvedDeadTimeSeconds: number
): IdentifiedModel {
  const eligible = models.filter((model) =>
    Number.isFinite(model.normalizedError) && model.stable && model.normalizedError <= 0.05);
  let selected = [...eligible]
    .sort((left, right) => complexity(left.kind) - complexity(right.kind)
      || left.normalizedError - right.normalizedError)[0];
  const firstOrder = eligible.find((model) => model.kind === "first_order");
  const delayedFirstOrder = eligible.find((model) => model.kind === "first_order_dead_time");
  // A resolved and materially useful delay is stronger evidence than the
  // normal simplicity preference; otherwise the no-delay model remains easier
  // to understand and maintain.
  if (firstOrder && delayedFirstOrder
    && resolvedDeadTimeSeconds > 2 * sampleTimeSeconds
    && delayedFirstOrder.normalizedError < firstOrder.normalizedError * 0.98) {
    selected = delayedFirstOrder;
  }
  if (!selected) {
    throw new Error("No acceptable stable low-order model could be identified from the retained data.");
  }
  return selected;
}

function lowOrderModel(model: LowOrderModelFit): IdentifiedModel {
  return {
    kind: model.kind,
    gain: model.gain,
    ...(model.timeConstantSeconds === undefined ? {} : { timeConstantSeconds: model.timeConstantSeconds }),
    ...(model.secondTimeConstantSeconds === undefined ? {} : {
      secondTimeConstantSeconds: model.secondTimeConstantSeconds,
    }),
    ...(model.deadTimeSeconds === undefined ? {} : { deadTimeSeconds: model.deadTimeSeconds }),
    normalizedError: model.normalizedValidationError,
    normalizedEstimationError: model.normalizedEstimationError,
    normalizedValidationError: model.normalizedValidationError,
    estimationResiduals: model.estimationResiduals,
    validationResiduals: model.validationResiduals,
    uncertainty: model.uncertainty,
    stable: true,
  };
}

function candidate(kind: ModelKind, gain: number, tau?: number, tau2?: number, delay?: number): IdentifiedModel {
  return { kind, gain, ...(tau === undefined ? {} : { timeConstantSeconds: tau }),
    ...(tau2 === undefined ? {} : { secondTimeConstantSeconds: tau2 }),
    ...(delay === undefined ? {} : { deadTimeSeconds: delay }), normalizedError: Number.POSITIVE_INFINITY,
    stable: kind === "integrating" || ((tau ?? 0) > 0 && (tau2 === undefined || tau2 > 0)) };
}

function modelError(model: IdentifiedModel, samples: readonly IdentificationSample[], stepIndex: number,
  input0: number, output0: number, inputChange: number, outputSpan: number): number {
  let squared = 0;
  let count = 0;
  const stepTime = samples[stepIndex].timeSeconds;
  for (const sample of samples.slice(stepIndex)) {
    const time = Math.max(0, sample.timeSeconds - stepTime - (model.deadTimeSeconds ?? 0));
    const fraction = responseFraction(model, time);
    // The preview describes the retained excitation step. A bounded test may
    // restore the actuator in its final sample; using that restoration as a
    // second excitation would make the fitted step size and prediction disagree.
    const predicted = output0 + model.gain * inputChange * fraction;
    squared += (sample.output - predicted) ** 2;
    count += 1;
  }
  return Math.sqrt(squared / Math.max(1, count)) / Math.max(1e-12, outputSpan);
}

function responseFraction(model: IdentifiedModel, time: number): number {
  if (model.kind === "static_gain") return 1;
  if (model.kind === "integrating") return time;
  if (model.secondTimeConstantSeconds !== undefined) {
    const first = model.timeConstantSeconds ?? 1;
    const second = model.secondTimeConstantSeconds;
    if (Math.abs(first - second) < 1e-12) {
      const x = time / first;
      return 1 - (1 + x) * Math.exp(-x);
    }
    return 1 - (first * Math.exp(-time / first) - second * Math.exp(-time / second)) / (first - second);
  }
  return 1 - Math.exp(-time / (model.timeConstantSeconds ?? 1));
}

/** Least-squares gain of a candidate's unit response to the held input deviations, from the steady baseline (N2.2). Dynamic forms
 * are simulated on the recorded instants; second-order forms use their exact step response at those instants. */
function jointGain(model: IdentifiedModel, samples: readonly IdentificationSample[], stepIndex: number, input0: number, output0: number): number {
  const times = samples.map((sample) => sample.timeSeconds);
  const deviations = samples.map((sample) => sample.input - input0);
  const response = model.kind === "second_order" || model.kind === "second_order_dead_time"
    ? samples.map((sample, index) => deviations[index] * responseFraction(model,
      Math.max(0, sample.timeSeconds - times[stepIndex] - (model.deadTimeSeconds ?? 0))))
    : heldUnitResponse(times, deviations, heldShape(model));
  return leastSquaresGain(samples.slice(stepIndex).map((sample) => sample.output), output0, response.slice(stepIndex)).gain;
}

function heldShape(model: IdentifiedModel): HeldShape {
  const delaySeconds = model.deadTimeSeconds ?? 0;
  if (model.kind === "integrating") return { kind: "integrating", delaySeconds };
  if (model.kind === "static_gain") return { kind: "static_gain" };
  return { kind: "first_order", timeConstantSeconds: model.timeConstantSeconds ?? 1, delaySeconds };
}

/** The fitted response of a held step: its size, and the fraction of its final change the held response reached (A10.3). Undefined
 * for a recording with more than one excitation (a PRBS, multisine or relay), where neither is defined. */
function heldStepEvidence(model: IdentifiedModel, samples: readonly IdentificationSample[]):
  { responseSize: number; holdSeconds: number; fraction?: number } | undefined {
  if (inputChangeCount(samples) > 2) return undefined;
  const held = heldStepSamples(samples);
  const { stepIndex, input0, output0 } = identificationOperatingPoint(held);
  const holdSeconds = held[held.length - 1].timeSeconds - held[stepIndex].timeSeconds;
  const finalChange = model.gain * (held[stepIndex].input - input0);
  if (model.kind === "integrating") return { responseSize: Math.abs(finalChange * holdSeconds), holdSeconds };
  const fraction = Math.abs(finalChange) > 1e-12
    ? reachedFraction(held.slice(stepIndex).map((sample) => sample.output), output0, finalChange) : undefined;
  return { responseSize: Math.abs(finalChange), holdSeconds, ...(fraction === undefined ? {} : { fraction }) };
}

/** The two refusals of a held-step fit: a response buried in the baseline noise, and a held response that has not reached 63 % of
 * its fitted final change (A10.3). `raw` keeps the baseline before the step; `values` is the fitted record. */
function unreliableStepRefusal(model: IdentifiedModel, raw: readonly IdentificationSample[], values: readonly IdentificationSample[]): string | undefined {
  const evidence = heldStepEvidence(model, values);
  if (!evidence) return undefined;
  const rawStep = largestInputChange(raw);
  const noiseRms = rawStep > 0 ? baselineNoiseRms(raw.slice(0, rawStep).map((sample) => sample.output)) : 0;
  const noiseMessage = noiseRefusal(evidence.responseSize, noiseRms, raw.find((sample) => sample.outputUnit)?.outputUnit ?? "", "step");
  if (noiseMessage) return noiseMessage;
  if (evidence.fraction !== undefined && evidence.fraction < MINIMUM_REACHED_FRACTION) {
    return partialResponseRefusal(evidence.fraction, evidence.holdSeconds, model.deadTimeSeconds ?? 0, model.timeConstantSeconds ?? 1);
  }
  return undefined;
}

/** What the caller states about the process: the process its identification policy declares, and whether the fit is closed loop. */
export interface IdentifyPlantOptions { declaredProcess?: string; closedLoop?: boolean }

function identifyOptions(input: IdentificationReviewInput): IdentifyPlantOptions {
  return { declaredProcess: input.declaredProcess, closedLoop: input.closedLoop };
}

/** The integrator rule for a held step in open loop. An integrating form is accepted only when the data can tell it apart from a
 * self-regulating form: a first-order-with-delay fit that meets the error limit with a time constant at least the hold would fit the
 * same data, so the response has not yet shown that it levels off. Undefined when the rule does not refuse. */
function integratorRefusalFor(options: IdentifyPlantOptions, values: readonly IdentificationSample[], maximumNormalizedError: number): string | undefined {
  if (options.closedLoop || options.declaredProcess === "integrating") return undefined;
  const stepIndex = largestInputChange(values);
  const sampleTime = medianIntervals(values);
  if (stepIndex < 1 || !(sampleTime > 0)) return undefined;
  const { input0, output0 } = identificationOperatingPoint(values);
  const times = values.map((sample) => sample.timeSeconds);
  const deviations = values.map((sample) => sample.input - input0);
  const measured = values.slice(stepIndex).map((sample) => sample.output);
  const holdSeconds = times[times.length - 1] - times[stepIndex];
  const span = Math.max(...measured) - Math.min(...measured);
  if (!(holdSeconds > 0) || !(span > 0)) return undefined;
  const timeConstants = logarithmicValues(sampleTime, 20 * holdSeconds, 60);
  let best: { timeConstantSeconds: number; error: number } | undefined;
  for (let index = 0; index < 9; index += 1) {
    const delaySeconds = index * 0.3 * holdSeconds / 8;
    for (const timeConstantSeconds of timeConstants) {
      const response = heldUnitResponse(times, deviations, { kind: "first_order", timeConstantSeconds, delaySeconds });
      const fitted = leastSquaresGain(measured, output0, response.slice(stepIndex));
      if (!Number.isFinite(fitted.gain)) continue;
      const error = Math.sqrt(fitted.squaredError / measured.length) / span;
      if (!best || error < best.error) best = { timeConstantSeconds, error };
    }
  }
  if (!best || !(best.error <= maximumNormalizedError) || best.timeConstantSeconds < holdSeconds) return undefined;
  return `The response is still rising like a ramp after ${minutesText(holdSeconds)} min; it cannot yet be told whether it levels off. Hold the step longer (Limits… → maximum duration), or identify with a setpoint step in Auto.`;
}

/** A preview is not offered when the physical rules refuse it: the recommended form's refusal, or a requested integrator's refusal. */
function refuseRefusedPreview(result: IdentificationResult, selected: IdentifiedModel): void {
  if (result.refusal) throw new Error(result.refusal);
  if (selected.kind === "integrating" && result.integratorRefusal) throw new Error(result.integratorRefusal);
}

function logarithmicValues(minimum: number, maximum: number, count: number): number[] {
  return Array.from({ length: count }, (_, index) => minimum * Math.pow(maximum / minimum, index / (count - 1)));
}

function minutesText(seconds: number): string {
  return String(Number((seconds / 60).toPrecision(2)));
}

/** Number of samples at which the input differs from the sample before. */
function inputChangeCount(samples: readonly IdentificationSample[]): number {
  return samples.slice(1).filter((sample, index) => Math.abs(sample.input - samples[index].input) > 1e-9 * Math.max(1, Math.abs(sample.input))).length;
}

/** Where the late ramp, a least-squares line extended back, meets the level before the step: an integrator's delay. */
function rampDelay(samples: readonly IdentificationSample[], stepIndex: number, output0: number): number {
  const tail = samples.slice(stepIndex + Math.floor((samples.length - stepIndex) / 2));
  const meanTime = mean(tail.map((sample) => sample.timeSeconds)), meanOutput = mean(tail.map((sample) => sample.output));
  const covariance = tail.reduce((sum, sample) => sum + (sample.timeSeconds - meanTime) * (sample.output - meanOutput), 0);
  const variance = tail.reduce((sum, sample) => sum + (sample.timeSeconds - meanTime) ** 2, 0);
  const slope = covariance / Math.max(1e-12, variance);
  if (!(Math.abs(slope) > 1e-12)) return 0;
  const start = meanTime - (meanOutput - output0) / slope;
  return Number.isFinite(start) ? Math.max(0, start - samples[stepIndex].timeSeconds) : 0;
}

function firstCrossing(samples: readonly IdentificationSample[], start: number, target: number): number {
  const rising = target >= samples[start].output;
  return (samples.slice(start).find((sample) => rising ? sample.output >= target : sample.output <= target) ?? samples[samples.length - 1]).timeSeconds;
}

function largestInputChange(samples: readonly IdentificationSample[]): number {
  let index = -1; let change = 0;
  for (let current = 1; current < samples.length; current += 1) {
    const candidate = Math.abs(samples[current].input - samples[current - 1].input);
    if (candidate > change) { change = candidate; index = current; }
  }
  return index;
}

function finiteSample(sample: IdentificationSample): boolean {
  return Number.isFinite(sample.timeSeconds) && Number.isFinite(sample.input) && Number.isFinite(sample.output);
}
function strictlyIncreasing(samples: readonly IdentificationSample[]): boolean {
  return samples.every((sample, index) => index === 0 || sample.timeSeconds > samples[index - 1].timeSeconds);
}
function medianIntervals(samples: readonly IdentificationSample[]): number {
  const intervals = samples.slice(1).map((sample, index) => sample.timeSeconds - samples[index].timeSeconds)
    .filter((value) => Number.isFinite(value) && value > 0).sort((a, b) => a - b);
  return intervals.length ? intervals[Math.floor(intervals.length / 2)] : 0;
}
function span(values: readonly number[]): number { return Math.max(...values) - Math.min(...values); }
function safeSpan(values: readonly number[]): number { return values.length > 0 ? span(values) : 0; }
function mean(values: readonly number[]): number { return values.reduce((sum, value) => sum + value, 0) / values.length; }
function complexity(kind: ModelKind): number {
  return ({ static_gain: 1, integrating: 1, first_order: 2, first_order_dead_time: 3,
    second_order: 3, second_order_dead_time: 4 })[kind];
}
