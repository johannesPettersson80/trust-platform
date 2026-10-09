import { linearTransferModel } from "./linearTransferModel";
import type { IdentifiedModel } from "./identification";
import type { GeneralModel } from "./generalModel";
import { tuneController, type TuningMethod, type TuningObjective } from "./tuning";

export interface PidPiDesign {
  status: "provisional";
  candidate: { applied: false; structure: "P" | "PI" | "PD" | "PID"; proportionalGain: number; integralGain: number; derivativeGain: number; backCalculationGain?: number; proportionalSetpointWeight?: number; derivativeSetpointWeight?: number; measurementFilterSeconds?: number };
  selectedModel: { revision: string; provenance: string };
  actualSTVerify: true;
  scope: string;
}

/** Converts a supported selected plant into the existing parallel PI tuning service. */
export function designTypedPiCandidate(value: GeneralModel, options: { method: TuningMethod; objective: TuningObjective; action: "direct" | "reverse" }): PidPiDesign {
  const model = linearTransferModel(value);
  if (model.kind === "transfer-function" && model.numerator.length === 1 && model.denominator.length === 3) return halfRulePiCandidate(model, options);
  if (model.kind !== "transfer-function" || model.numerator.length !== 1 || model.denominator.length !== 2) {
    throw new Error("Typed PI tuning requires a selected SISO first- or second-order transfer-function model without zeros.");
  }
  const [a1, a0] = model.denominator;
  if (!(a1 > 0) || a0 < 0 || model.numerator[0] === 0) throw new Error("Typed PI tuning requires a nonzero-gain stable or integrating first-order model.");
  const integrating = a0 === 0;
  // The signed plant gain goes to Tune, which refuses a sign that contradicts the action (N6.3) and returns the signed Kp (N6.4).
  const tuningModel: IdentifiedModel = integrating
    ? { kind: "integrating", gain: model.numerator[0] / a1, deadTimeSeconds: model.delaySeconds,
      normalizedError: 0, stable: false }
    : { kind: "first_order", gain: model.numerator[0] / a0, timeConstantSeconds: a1 / a0,
      deadTimeSeconds: model.delaySeconds, normalizedError: 0, stable: true };
  const tuning = tuneController(tuningModel, { ...options, unit: `${model.outputs[0].unit}/${model.inputs[0].unit}${integrating ? "/s" : ""}` });
  const proportionalGain = tuning.controller.kp;
  const integralGain = proportionalGain / tuning.controller.tiSeconds!;
  return { status: "provisional", candidate: { applied: false, structure: "PI", proportionalGain, integralGain, derivativeGain: 0 },
    selectedModel: { revision: model.revision, provenance: `declared-${model.source}-${model.validation}` }, actualSTVerify: true,
    scope: `Provisional typed PI record for the selected ${integrating ? "integrating" : "first-order"} model; actual-ST Verify is required.` };
}

/** SIMC's half rule: K/((τ₁s+1)(τ₂s+1))·e^(−θs), τ₁ ≥ τ₂, is tuned as a first-order model with τ = τ₁ + τ₂/2 and delay θ + τ₂/2. */
function halfRulePiCandidate(model: GeneralModel & { kind: "transfer-function" }, options: { method: TuningMethod; objective: TuningObjective; action: "direct" | "reverse" }): PidPiDesign {
  const [a2, a1, a0] = model.denominator;
  if (!(a2 > 0 && a1 > 0 && a0 > 0) || model.numerator[0] === 0) throw new Error("SIMC's half rule requires a stable second-order model with a nonzero gain.");
  const sum = a1 / a0, product = a2 / a0, discriminant = sum * sum - 4 * product;
  if (discriminant < 0) throw new Error("The second-order model has complex poles, so SIMC's half rule does not apply. Choose Automatic or another tuning rule.");
  const slow = (sum + Math.sqrt(discriminant)) / 2, fast = (sum - Math.sqrt(discriminant)) / 2;
  const timeConstantSeconds = slow + fast / 2, deadTimeSeconds = model.delaySeconds + fast / 2;
  const tuning = tuneController({ kind: "first_order_dead_time", gain: model.numerator[0] / a0, timeConstantSeconds, deadTimeSeconds,
    normalizedError: 0, stable: true }, { ...options, unit: `${model.outputs[0].unit}/${model.inputs[0].unit}` });
  const proportionalGain = tuning.controller.kp;
  return { status: "provisional", candidate: { applied: false, structure: "PI", proportionalGain, integralGain: proportionalGain / tuning.controller.tiSeconds!, derivativeGain: 0 },
    selectedModel: { revision: model.revision, provenance: `declared-${model.source}-${model.validation}` }, actualSTVerify: true,
    scope: `Provisional typed PI record for the selected second-order model, reduced by SIMC's half rule to τ = ${Number(timeConstantSeconds.toPrecision(4))} s `
      + `and θ = ${Number(deadTimeSeconds.toPrecision(4))} s; actual-ST Verify is required.` };
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
export function designTypedDerivativeCandidate(value: GeneralModel, structure: "PD" | "PID", options: { method: TuningMethod; objective: TuningObjective; action: "direct" | "reverse" }): PidPiDesign {
  const model = linearTransferModel(value);
  if (model.kind !== "transfer-function" || model.denominator.length !== 2) throw new Error("Typed derivative design requires a first-order transfer-function model.");
  const pi = designTypedPiCandidate(model, options);
  if (!(model.denominator[1] > 0)) throw new Error("The constrained derivative design requires a stable first-order model with a finite time constant.");
  const timeConstant = model.denominator[0] / model.denominator[1];
  const derivativeGain = rounded(pi.candidate.proportionalGain * Math.min(Math.max(model.delaySeconds, 0.05 * timeConstant), 0.25 * timeConstant));
  if (!Number.isFinite(derivativeGain) || derivativeGain === 0) throw new Error("Typed derivative design did not produce a finite nonzero derivative gain.");
  return { ...pi, candidate: { applied: false, structure, proportionalGain: pi.candidate.proportionalGain,
    integralGain: structure === "PID" ? pi.candidate.integralGain : 0, derivativeGain },
    scope: `Constrained ${structure} alternative from the qualified first-order PI reference; actual-ST Verify is required before application.` };
}

function rounded(value: number): number { return Math.round(value * 1e12) / 1e12; }
