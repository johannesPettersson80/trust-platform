import { linearTransferModel } from "./linearTransferModel";
import type { IdentifiedModel } from "./identification";
import type { GeneralModel } from "./generalModel";
import { tuneController, type TuningMethod, type TuningObjective } from "./tuning";

export interface PidPiDesign {
  status: "provisional";
  candidate: { applied: false; structure: "P" | "PI" | "PD" | "PID"; proportionalGain: number; integralGain: number; derivativeGain: number; backCalculationGain?: number };
  selectedModel: { revision: string; provenance: string };
  actualSTVerify: true;
  scope: string;
}

/** Converts a supported selected plant into the existing parallel PI tuning service. */
export function designTypedPiCandidate(value: GeneralModel, options: { method: TuningMethod; objective: TuningObjective } = { method: "SIMC", objective: "balanced" }): PidPiDesign {
  const model = linearTransferModel(value);
  if (model.kind !== "transfer-function" || model.numerator.length !== 1 || model.denominator.length !== 2) {
    throw new Error("Typed PI tuning requires a selected SISO first-order transfer-function model.");
  }
  const [a1, a0] = model.denominator;
  if (!(a1 > 0) || a0 < 0 || model.numerator[0] === 0) throw new Error("Typed PI tuning requires a nonzero-gain stable or integrating first-order model.");
  const integrating = a0 === 0;
  const tuningModel: IdentifiedModel = integrating
    ? { kind: "integrating", gain: Math.abs(model.numerator[0] / a1), deadTimeSeconds: model.delaySeconds,
      normalizedError: 0, stable: false }
    : { kind: "first_order", gain: Math.abs(model.numerator[0] / a0), timeConstantSeconds: a1 / a0,
      deadTimeSeconds: model.delaySeconds, normalizedError: 0, stable: true };
  const tuning = tuneController(tuningModel, options);
  const proportionalGain = Math.sign(model.numerator[0]) * tuning.controller.kp;
  const integralGain = proportionalGain / tuning.controller.tiSeconds!;
  return { status: "provisional", candidate: { applied: false, structure: "PI", proportionalGain, integralGain, derivativeGain: 0 },
    selectedModel: { revision: model.revision, provenance: `declared-${model.source}-${model.validation}` }, actualSTVerify: true,
    scope: `Provisional typed PI record for the selected ${integrating ? "integrating" : "first-order"} model; actual-ST Verify is required.` };
}

export function designTypedProportionalCandidate(model: GeneralModel, proportionalGain: number): PidPiDesign {
  if (!Number.isFinite(proportionalGain)) throw new Error("Typed proportional design requires a finite gain.");
  return { status: "provisional", candidate: { applied: false, structure: "P", proportionalGain, integralGain: 0, derivativeGain: 0 },
    selectedModel: { revision: model.revision, provenance: `declared-${model.source}-${model.validation}` }, actualSTVerify: true,
    scope: "Provisional typed P record for the selected first-order model; actual-ST Verify is required before application." };
}

/**
 * Produces explicitly labelled constrained PD/PID alternatives from the same
 * qualified first-order PI reference. Their actual ST evidence still decides
 * whether they are usable; this function never presents them as SIMC formulas.
 */
export function designTypedDerivativeCandidate(value: GeneralModel, structure: "PD" | "PID", options: { method: TuningMethod; objective: TuningObjective } = { method: "SIMC", objective: "balanced" }): PidPiDesign {
  const model = linearTransferModel(value);
  const pi = designTypedPiCandidate(model, options);
  if (model.kind !== "transfer-function") throw new Error("Typed derivative design requires a first-order transfer-function model.");
  if (!(model.denominator[1] > 0)) throw new Error("The constrained derivative design requires a stable first-order model with a finite time constant.");
  const timeConstant = model.denominator[0] / model.denominator[1];
  const derivativeGain = rounded(pi.candidate.proportionalGain * Math.min(Math.max(model.delaySeconds, 0.05 * timeConstant), 0.25 * timeConstant));
  if (!Number.isFinite(derivativeGain) || derivativeGain === 0) throw new Error("Typed derivative design did not produce a finite nonzero derivative gain.");
  return { ...pi, candidate: { applied: false, structure, proportionalGain: pi.candidate.proportionalGain,
    integralGain: structure === "PID" ? pi.candidate.integralGain : 0, derivativeGain },
    scope: `Constrained ${structure} alternative from the qualified first-order PI reference; actual-ST Verify is required before application.` };
}

function rounded(value: number): number { return Math.round(value * 1e12) / 1e12; }
