import { linearTransferModel } from "./linearTransferModel";
import { designIntegratingProportionalController } from "./integratingProportionalDesign";
import { type GeneralTransferModel } from "./generalModel";

export interface SimpleProportionalRequirements {
  referenceChange: number;
  constantInputDisturbance: number;
  maximumSteadyError: number;
  settlingBand: number;
  settlingTimeSeconds: number;
  proportionalGainBounds: { minimum: number; maximum: number };
}

export interface SimpleProportionalDesign {
  status: "provisional";
  candidate: { applied: false; structure: "P"; proportionalGain: number };
  selectedModel: { revision: string; provenance: string };
  formulas: { symbolic: readonly string[]; substituted: readonly string[] };
  prediction: {
    evidence: "model-predicted";
    steadyErrorBound: number;
    settlingResidualBound: number;
    closedLoopPole: number;
  };
  actualSTVerify: true;
  scope: string;
}

/**
 * Designs only the explicitly supported delay-free SISO first-order P path.
 * It returns a prediction and an unapplied candidate; actual ST Verify remains required.
 */
export function designSimpleProportionalController(
  value: unknown,
  requirements: SimpleProportionalRequirements,
): SimpleProportionalDesign {
  validateRequirements(requirements);
  const selected = linearTransferModel(value);
  if (selected?.kind === "transfer-function" && selected.delaySeconds === 0
      && selected.inputs.length === 1 && selected.outputs.length === 1
      && selected.numerator.length === 1 && selected.denominator.length === 2
      && selected.denominator[0] > 0 && selected.denominator[1] === 0) {
    return designIntegratingProportionalController(selected, requirements);
  }
  const model = simplePlant(value);
  const [a1, a0] = model.denominator;
  const b = model.numerator[0];
  const gain = b / a0;
  const timeConstant = a1 / a0;
  const operatingCommand = model.operatingPoint?.inputs[0] ?? 0;
  const referenceMagnitude = Math.max(Math.abs(requirements.referenceChange), Math.abs(gain * operatingCommand));
  const worstFinalError = Math.max(
    Math.abs(requirements.referenceChange + gain * operatingCommand),
    Math.abs(gain * requirements.constantInputDisturbance),
  );
  const steadyThreshold = worstFinalError / requirements.maximumSteadyError - 1;
  const settlingThreshold = timeConstant
    * Math.log(referenceMagnitude / requirements.settlingBand)
    / requirements.settlingTimeSeconds - 1;
  const requiredQ = Math.max(0, steadyThreshold, settlingThreshold);
  const lowerQ = Math.min(
    gain * requirements.proportionalGainBounds.minimum,
    gain * requirements.proportionalGainBounds.maximum,
  );
  const upperQ = Math.max(
    gain * requirements.proportionalGainBounds.minimum,
    gain * requirements.proportionalGainBounds.maximum,
  );
  let q = Math.max(requiredQ, lowerQ);
  if (q > upperQ + tolerance(q, upperQ)) {
    throw new Error("Proportional gain bounds cannot satisfy the declared steady-error and settling requirements.");
  }
  for (let attempt = 0; attempt < 8 && !meetsPredictions(q, referenceMagnitude, timeConstant, worstFinalError, requirements); attempt++) {
    q += tolerance(q, requiredQ);
  }
  if (q > upperQ + tolerance(q, upperQ) || !meetsPredictions(q, referenceMagnitude, timeConstant, worstFinalError, requirements)) {
    throw new Error("Finite proportional gain bounds cannot satisfy the declared prediction requirements.");
  }
  const proportionalGain = q / gain;
  if (!Number.isFinite(proportionalGain)
    || proportionalGain < requirements.proportionalGainBounds.minimum - tolerance(proportionalGain, requirements.proportionalGainBounds.minimum)
    || proportionalGain > requirements.proportionalGainBounds.maximum + tolerance(proportionalGain, requirements.proportionalGainBounds.maximum)) {
    throw new Error("Calculated proportional gain is outside the declared bounds.");
  }
  const prediction = predicted(q, referenceMagnitude, timeConstant, worstFinalError, requirements);
  return {
    status: "provisional",
    candidate: { applied: false, structure: "P", proportionalGain },
    selectedModel: {
      revision: model.revision,
      provenance: `declared-${model.source}-${model.validation}`,
    },
    formulas: {
      symbolic: [
        "K = b/a0; tau = a1/a0; q = K Kp.",
        "u = Kp (r-y); r = y0 + deltaR; u0 is the operating command.",
        "|e(infinity)| <= max(|deltaR + K u0|, |K d|)/(1 + q); d is a separate requested disturbance.",
        "|e_r(T)| <= max(|deltaR|, |K u0|) exp(-(1 + q) T/tau).",
        "pole = -(1 + q)/tau.",
      ],
      substituted: [
        `K = ${gain}; tau = ${timeConstant}; u0 = ${operatingCommand}; E = ${worstFinalError}.`,
        `q = ${q}; Kp = ${proportionalGain}.`,
        `|e(infinity)| <= ${prediction.steadyErrorBound}; |e_r(T)| <= ${prediction.settlingResidualBound}; pole = ${prediction.closedLoopPole}.`,
      ],
    },
    prediction: { evidence: "model-predicted", ...prediction },
    actualSTVerify: true,
    scope: "Provisional P design for the selected first-order model only; this prediction is not controller recommendation or actual-ST verification.",
  };
}

export function simplePlant(value: unknown): GeneralTransferModel {
  const model = linearTransferModel(value);
  if (!model || model.kind !== "transfer-function" || !["provided", "fitted"].includes(model.source)) {
    throw new Error("Simple P design requires a selected supplied or fitted transfer-function model.");
  }
  if (model.delaySeconds !== 0) throw new Error("Simple P design has no transport-delay adapter.");
  if (model.inputs.length !== 1 || model.outputs.length !== 1
    || model.numerator.length !== 1 || model.denominator.length !== 2) {
    throw new Error("Simple P design supports only a SISO first-order transfer function with no zeros.");
  }
  const [a1, a0] = model.denominator;
  const b = model.numerator[0];
  if (!(a1 > 0) || !(a0 > 0) || b === 0) {
    throw new Error("Simple P design requires finite a1>0, a0>0, and nonzero b.");
  }
  return model;
}

export function validateRequirements(value: SimpleProportionalRequirements): void {
  const values = [
    value.referenceChange, value.constantInputDisturbance, value.maximumSteadyError,
    value.settlingBand, value.settlingTimeSeconds, value.proportionalGainBounds?.minimum,
    value.proportionalGainBounds?.maximum,
  ];
  if (!values.every(Number.isFinite) || value.referenceChange === 0
    || value.maximumSteadyError <= 0 || value.settlingBand <= 0 || value.settlingTimeSeconds <= 0
    || value.proportionalGainBounds.minimum > value.proportionalGainBounds.maximum) {
    throw new Error("Simple P design requires finite nonzero reference, positive error/settling limits, and ordered gain bounds.");
  }
}

function predicted(
  q: number,
  referenceMagnitude: number,
  timeConstant: number,
  worstFinalError: number,
  requirements: SimpleProportionalRequirements,
): { steadyErrorBound: number; settlingResidualBound: number; closedLoopPole: number } {
  const closedLoopFactor = 1 + q;
  return {
    steadyErrorBound: worstFinalError / closedLoopFactor,
    settlingResidualBound: referenceMagnitude
      * Math.exp(-closedLoopFactor * requirements.settlingTimeSeconds / timeConstant),
    closedLoopPole: -closedLoopFactor / timeConstant,
  };
}

function meetsPredictions(
  q: number,
  referenceMagnitude: number,
  timeConstant: number,
  worstFinalError: number,
  requirements: SimpleProportionalRequirements,
): boolean {
  const prediction = predicted(q, referenceMagnitude, timeConstant, worstFinalError, requirements);
  return Number.isFinite(prediction.closedLoopPole) && prediction.closedLoopPole < 0
    && prediction.steadyErrorBound <= requirements.maximumSteadyError + tolerance(prediction.steadyErrorBound, requirements.maximumSteadyError)
    && prediction.settlingResidualBound <= requirements.settlingBand + tolerance(prediction.settlingResidualBound, requirements.settlingBand);
}

function tolerance(...values: number[]): number {
  return 64 * Number.EPSILON * Math.max(1, ...values.map(value => Math.abs(value)));
}
