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
    const preview = identifyPlant(estimation, input.excluded, input.source);
    const selected = selectRequestedModel(preview, input.requestedKind);
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
  const result = identifyPlant(estimation, input.excluded, input.source, validation, input.maximumNormalizedError);
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
  if ((model.deadTimeSeconds ?? 0) > 0 && (model.deadTimeSeconds ?? 0) < medianIntervals(samples)) {
    warnings.push("Dead time is shorter than one sample interval and is weakly resolved.");
  }
  return {
    points,
    rootMeanSquareError,
    outputUnit: samples.find((sample) => sample.outputUnit)?.outputUnit ?? "unit",
    stable: model.stable,
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
  maximumNormalizedError = 0.35
): IdentificationResult {
  const qualified = qualifyIdentificationData(samples, excluded);
  if (!qualified.suitable) throw new Error(qualified.reason);
  if (validationSamples !== undefined) {
    const validation = qualifyIdentificationData(validationSamples, []);
    if (!validation.suitable) throw new Error(`Independent validation: ${validation.reason}`);
    const fitted = identifyLowOrderModels(heldStepSamples(qualified.samples), heldStepSamples(validation.samples), {
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
    return {
      source: source === "Physical" ? "Identified from physical data" : "Identified from simulation data",
      recommended: lowOrderModel(fitted.recommended),
      models: fitted.models.map(lowOrderModel),
      partition: "independent-runs",
    };
  }
  const values = heldStepSamples(qualified.samples);
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
  const dt = qualified.sampleTimeSeconds;
  const deadTime = Math.max(0, delayTime - dt);
  const tau = Math.max(dt, t63 - deadTime);
  const horizon = values[values.length - 1].timeSeconds - stepTime;
  const models: IdentifiedModel[] = [
    candidate("integrating", slopeGain(values, stepIndex, inputChange), undefined, undefined, 0),
    candidate("first_order", gain, tau + deadTime, undefined, 0),
    candidate("first_order_dead_time", gain, tau, undefined, deadTime),
    candidate("second_order", gain, Math.max(dt, (tau + deadTime) / 2.146), Math.max(dt, (tau + deadTime) / 2.146), 0),
    candidate("second_order_dead_time", gain, Math.max(dt, tau / 2.146), Math.max(dt, tau / 2.146), deadTime),
  ];
  for (const model of models) {
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
  return { source: source === "Physical" ? "Identified from physical data" : "Identified from simulation data",
    recommended: { ...recommended }, models: models.map((model) => ({ ...model })), partition: "single-run" };
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

function slopeGain(samples: readonly IdentificationSample[], stepIndex: number, inputChange: number): number {
  const tail = samples.slice(stepIndex + Math.floor((samples.length - stepIndex) / 2));
  const first = tail[0];
  const last = tail[tail.length - 1];
  return (last.output - first.output) / Math.max(1e-12, (last.timeSeconds - first.timeSeconds) * inputChange);
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
