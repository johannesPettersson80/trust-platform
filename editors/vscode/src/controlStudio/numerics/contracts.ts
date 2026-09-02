/** Plain truST-owned numerical records. Library-specific matrix types stay private. */
export type DenseMatrix = readonly (readonly number[])[];

export interface NumericalTolerance {
  absolute: number;
  relative: number;
  conditionLimit: number;
}

export type NumericalRejectionReason =
  | "nonfinite-input"
  | "inconsistent-dimensions"
  | "matrix-not-square"
  | "singular"
  | "ill-conditioned"
  | "solver-failed";

export interface NumericalRejection {
  status: "rejected";
  reason: NumericalRejectionReason;
}

export interface LeastSquaresSolution {
  status: "qualified";
  solution: readonly number[];
  residualNorm: number;
  rank: number;
  conditionNumber: number;
}

export interface ComplexValue {
  real: number;
  imaginary: number;
}

export interface EigenvalueSolution {
  status: "qualified";
  values: readonly ComplexValue[];
}

export type LeastSquaresResult = LeastSquaresSolution | NumericalRejection;
export type EigenvalueResult = EigenvalueSolution | NumericalRejection;

export type LowOrderModelKind =
  | "static_gain"
  | "integrating"
  | "first_order"
  | "first_order_dead_time"
  | "second_order"
  | "second_order_dead_time";

export interface TimeDomainSample {
  timeSeconds: number;
  input: number;
  output: number;
  quality: "good" | "uncertain" | "bad";
}

export interface ParameterInterval { minimum: number; maximum: number }

export interface LowOrderModelFit {
  kind: LowOrderModelKind;
  gain: number;
  timeConstantSeconds?: number;
  secondTimeConstantSeconds?: number;
  deadTimeSeconds?: number;
  estimationResiduals: readonly number[];
  validationResiduals: readonly number[];
  normalizedEstimationError: number;
  normalizedValidationError: number;
  uncertainty: {
    method: "grid-resolution-and-validation-residual";
    gain: ParameterInterval;
    timeConstantSeconds?: ParameterInterval;
    secondTimeConstantSeconds?: ParameterInterval;
    deadTimeSeconds?: ParameterInterval;
  };
}

export interface LowOrderIdentificationOptions {
  allowedKinds: readonly LowOrderModelKind[];
  absoluteTolerance: number;
  minimumSamples: number;
  minimumInputSpan: number;
  maximumNormalizedValidationError: number;
}

export type LowOrderIdentificationResult =
  | { status: "qualified"; recommended: LowOrderModelFit; models: readonly LowOrderModelFit[] }
  | { status: "rejected"; reason: "nonfinite-input" | "insufficient-data" | "poor-excitation"
      | "missing-step" | "no-qualified-model" };

export interface PrbsOptions { order: number; length: number; amplitude: number; seed: number }
export interface MultisineOptions {
  sampleTimeSeconds: number;
  sampleCount: number;
  frequenciesRadPerSecond: readonly number[];
  amplitudes: readonly number[];
  phasesRadians: readonly number[];
  maximumAbsoluteValue: number;
}
export type ExcitationResult =
  | { status: "qualified"; values: readonly number[]; scale: number }
  | { status: "rejected"; reason: "nonfinite-input" | "invalid-configuration" | "alias-risk" };

export interface FrequencyResponsePoint {
  frequencyRadPerSecond: number;
  real: number;
  imaginary: number;
  magnitude: number;
  phaseRadians: number;
  coherence: number;
}
export type FrequencyResponseEstimate =
  | { status: "qualified"; points: readonly FrequencyResponsePoint[] }
  | { status: "rejected"; reason: "nonfinite-input" | "inconsistent-dimensions"
      | "insufficient-data" | "alias-risk" | "poor-excitation" | "low-coherence" };

export interface FirstOrderFrequencyFit {
  status: "qualified";
  model: { kind: "first_order"; gain: number; timeConstantSeconds: number };
  estimationResiduals: readonly number[];
  validationResiduals: readonly number[];
  normalizedValidationError: number;
}
export type FirstOrderFrequencyFitResult = FirstOrderFrequencyFit
  | { status: "rejected"; reason: "nonfinite-input" | "insufficient-data"
      | "low-coherence" | "unstable-or-nonminimum-phase" | "no-qualified-model" };

export interface ContinuousStateSpaceModel {
  a: DenseMatrix;
  b: DenseMatrix;
  c: DenseMatrix;
  d: DenseMatrix;
}
export interface DiscreteStateSpaceModel extends ContinuousStateSpaceModel {
  sampleTimeSeconds: number;
  method: "forward_euler" | "backward_euler" | "tustin" | "exact_zoh";
}
