import { linearTransferModel } from "./linearTransferModel";
import { evaluateManualIntegratingProportionalController } from "./integratingProportionalDesign";
import {
  simplePlant,
  validateRequirements,
  type SimpleProportionalRequirements,
} from "./simpleControllerDesign";

export interface ManualProportionalDesign {
  status: "exploratory";
  candidate: { applied: false; structure: "P"; proportionalGain: number };
  selectedModel: { revision: string; provenance: string };
  formulas: { symbolic: readonly string[]; substituted: readonly string[] };
  prediction: {
    evidence: "model-predicted";
    closedLoopPole: number;
    steadyErrorBound: number | null;
    settlingResidualBound: number | null;
    noAsymptoticBoundReason?: string;
  };
  actualSTVerify: true;
  scope: string;
}

/**
 * Evaluates one engineer-entered P gain in the established delay-free SISO
 * first-order domain. It does not recommend, apply, or verify that gain.
 */
export function designManualProportionalCandidate(
  value: unknown,
  proportionalGain: number,
  requirements: SimpleProportionalRequirements,
): ManualProportionalDesign {
  validateRequirements(requirements);
  const { minimum, maximum } = requirements.proportionalGainBounds;
  if (!Number.isFinite(proportionalGain)) throw new Error("Manual proportional gain must be finite.");
  if (proportionalGain < minimum || proportionalGain > maximum) {
    throw new Error("Manual proportional gain is outside the declared bounds.");
  }
  const selected = linearTransferModel(value);
  if (selected?.kind === "transfer-function" && selected.delaySeconds === 0
      && selected.inputs.length === 1 && selected.outputs.length === 1
      && selected.numerator.length === 1 && selected.denominator.length === 2
      && selected.denominator[0] > 0 && selected.denominator[1] === 0) {
    return evaluateManualIntegratingProportionalController(selected, proportionalGain, requirements);
  }
  const model = simplePlant(value);
  const [a1, a0] = model.denominator;
  const gain = model.numerator[0] / a0;
  const timeConstant = a1 / a0;
  const q = gain * proportionalGain;
  const closedLoopFactor = 1 + q;
  const rawClosedLoopPole = -closedLoopFactor / timeConstant;
  const operatingEffect = gain * (model.operatingPoint?.inputs[0] ?? 0);
  const worstFinalError = Math.max(
    Math.abs(requirements.referenceChange + operatingEffect),
    Math.abs(gain * requirements.constantInputDisturbance),
  );
  if (![gain, timeConstant, q, closedLoopFactor, rawClosedLoopPole, worstFinalError].every(Number.isFinite)
      || (rawClosedLoopPole === 0 && closedLoopFactor !== 0)) {
    throw new Error("Manual proportional prediction has numerical uncertainty.");
  }
  const closedLoopPole = rawClosedLoopPole === 0 ? 0 : rawClosedLoopPole;
  const stable = closedLoopPole < 0;
  const prediction: {
    steadyErrorBound: number | null;
    settlingResidualBound: number | null;
    noAsymptoticBoundReason?: string;
  } = stable ? stablePrediction(worstFinalError, operatingEffect, q, closedLoopFactor, closedLoopPole, requirements) : {
      steadyErrorBound: null,
      settlingResidualBound: null,
      noAsymptoticBoundReason: "No asymptotic steady-error or transient bound exists because the selected closed-loop pole is nonnegative.",
    };
  return {
    status: "exploratory",
    candidate: { applied: false, structure: "P", proportionalGain },
    selectedModel: { revision: model.revision, provenance: `declared-${model.source}-${model.validation}` },
    formulas: {
      symbolic: [
        "K = b/a0; tau = a1/a0; q = K Kp.",
        "pole = -(1 + q)/tau.",
        "u = Kp (r-y); r = y0 + deltaR; u0 is the operating command.",
        "For pole < 0, |e(infinity)| <= max(|deltaR + K u0|, |K d|)/(1 + q); d is a separate requested disturbance.",
        "For pole < 0, |e_r(T)| <= max(|deltaR|, |(q deltaR - K u0)/(1 + q)|) exp(pole T).",
        "When u0 = 0 this reduces to max(1, |q/(1 + q)|) |deltaR| exp(pole T).",
      ],
      substituted: [
        `K = ${gain}; tau = ${timeConstant}; q = ${q}; Kp = ${proportionalGain}.`,
        stable
          ? `|e(infinity)| <= ${prediction.steadyErrorBound}; |e_r(T)| <= ${prediction.settlingResidualBound}; pole = ${closedLoopPole}.`
          : `${prediction.noAsymptoticBoundReason} pole = ${closedLoopPole}.`,
      ],
    },
    prediction: { evidence: "model-predicted", closedLoopPole, ...prediction },
    actualSTVerify: true,
    scope: "Exploratory manual P candidate for the selected first-order model only; actual-ST Verify remains separate and required.",
  };
}

function stablePrediction(
  worstFinalError: number,
  operatingEffect: number,
  q: number,
  closedLoopFactor: number,
  closedLoopPole: number,
  requirements: SimpleProportionalRequirements,
): { steadyErrorBound: number; settlingResidualBound: number } {
  const steadyErrorBound = worstFinalError / closedLoopFactor;
  const referenceTransientAmplitude = Math.max(Math.abs(requirements.referenceChange),
    Math.abs(q / closedLoopFactor * requirements.referenceChange - operatingEffect / closedLoopFactor));
  const settlingResidualBound = referenceTransientAmplitude
    * Math.exp(closedLoopPole * requirements.settlingTimeSeconds);
  if (!Number.isFinite(steadyErrorBound) || !Number.isFinite(settlingResidualBound)) {
    throw new Error("Manual proportional prediction has numerical uncertainty.");
  }
  return { steadyErrorBound, settlingResidualBound };
}
